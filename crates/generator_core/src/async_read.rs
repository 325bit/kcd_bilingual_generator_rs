use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read, // Removed BufRead, BufReader unless needed elsewhere
    path::PathBuf,
    sync::Arc, // Use Arc for shared data
};

use super::{
    bilingual_generator::{BilingualGenerator, EntryId, Language, LastTextValue, XmlFile},
    bilingual_generator_errors::BilingualGeneratorError,
    util::{SEPARATOR_NEWLINE, SEPARATOR_SLASH, create_new_pak, secondary_text_combined},
};
// Import the utility functions
use indexmap::IndexMap;
use quick_xml::{Reader, events::Event};
use rayon::prelude::*; // Keep for parallel cleanup if desired

use tokio::{
    sync::mpsc,               // Async channel
    task::{self, JoinHandle}, // Async tasks
};
use zip::ZipArchive;
// Type alias for the data of a single language, shared via Arc
type SharedLanguageData = Arc<HashMap<XmlFile, IndexMap<EntryId, LastTextValue>>>;

// Type alias for the data structure received from reader tasks
type ReaderResult = (Language, Result<SharedLanguageData, BilingualGeneratorError>);

impl BilingualGenerator {
    // --- Synchronous Helper: Reads XMLs for ONE language ---
    // Intended to be run inside tokio::task::spawn_blocking
    fn read_single_language_xmls_sync(
        language: String,
        game_path: PathBuf,                 // Pass necessary data
        files_to_process: Arc<Vec<String>>, // Use Arc for shared Vec
    ) -> Result<HashMap<XmlFile, IndexMap<EntryId, LastTextValue>>, BilingualGeneratorError> {
        let pak_filename = format!("{}_xml.pak", language);
        let pak_path = game_path.join("Localization").join(&pak_filename);
        println!("[Reader: {}] Opening PAK: {}", language, pak_path.display());

        let pak_file = File::open(&pak_path).map_err(|e| {
            eprintln!("[Reader: {}] Error opening PAK {}: {}", language, pak_path.display(), e);
            BilingualGeneratorError::PakExtractionFailed
        })?;

        // Reading from ZipArchive is synchronous I/O
        let mut archive = ZipArchive::new(pak_file).map_err(|e| {
            eprintln!("[Reader: {}] Error reading PAK archive {}: {}", language, pak_path.display(), e);
            BilingualGeneratorError::PakExtractionFailed
        })?;

        let mut language_data: HashMap<XmlFile, IndexMap<EntryId, LastTextValue>> = HashMap::new();

        for xml_filename in files_to_process.iter() {
            // println!("[Reader: {}] Attempting to read file: {}", language, xml_filename); // Debug
            let mut xml_file = match archive.by_name(xml_filename) {
                Ok(file) => file,
                Err(e) => {
                    // Log warning but continue? Or return error? Decided to warn and skip.
                    eprintln!(
                        "[Reader: {}] Warning: Could not find {} in {}: {}. Skipping file.",
                        language,
                        xml_filename,
                        pak_path.display(),
                        e
                    );
                    continue; // Skip this file for this language
                    // return Err(BilingualGeneratorError::PakExtractionFailed); // Alternative: fail entire language read
                }
            };

            let mut content = String::new();
            // Reading the file content is synchronous I/O
            xml_file.read_to_string(&mut content).map_err(|e| {
                eprintln!("[Reader: {}] Error reading content of {}: {}", language, xml_filename, e);
                BilingualGeneratorError::PakExtractionFailed
            })?;

            // Parsing XML is CPU-bound work
            let mut reader = Reader::from_str(&content);
            let mut buf = Vec::with_capacity(1024); // Increased capacity slightly
            let mut single_file_data = IndexMap::new();
            let mut current_cells: Vec<String> = Vec::with_capacity(4);
            let mut inside_row = false;

            loop {
                buf.clear(); // Moved clear to start
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(e)) if e.name().as_ref() == b"Row" => {
                        inside_row = true;
                        current_cells.clear();
                    }
                    Ok(Event::End(e)) if e.name().as_ref() == b"Row" => {
                        inside_row = false;
                        // Original logic: Use cell 0 (ID) and cell 2 (Text)
                        if current_cells.len() >= 3 && current_cells[0] != "Entry id" {
                            single_file_data.insert(EntryId(current_cells[0].clone()), LastTextValue(current_cells[2].clone().into()));
                        }
                        // Clear cells after processing row, regardless of content
                        current_cells.clear();
                    }
                    Ok(Event::Text(e)) => {
                        if inside_row {
                            // Using unescape() to handle XML entities like &, <, etc.
                            match e.unescape() {
                                Ok(text) => current_cells.push(text.into_owned()),
                                Err(xml_err) => {
                                    // Handle potential UTF-8 errors during unescaping too
                                    eprintln!("[Reader: {}] XML unescape error in {}: {}", language, xml_filename, xml_err);
                                    return Err(BilingualGeneratorError::XmlProcessingFailed(format!(
                                        "Invalid XML escape sequence in {} for language {}: {}",
                                        xml_filename, language, xml_err
                                    )));
                                }
                            }
                            // Original logic kept raw text:
                            // let raw_text = std::str::from_utf8(e.as_ref())
                            //     .map_err(|utf8_err| BilingualGeneratorError::XmlProcessingFailed(
                            //         format!("Invalid UTF-8 in XML text for {} in {}: {}", xml_filename, language, utf8_err)
                            //     ))?;
                            // current_cells.push(raw_text.to_string());
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(e) => {
                        eprintln!("[Reader: {}] XML parsing error in {}: {}", language, xml_filename, e);
                        return Err(BilingualGeneratorError::XmlProcessingFailed(format!(
                            "XML error in {} for language {}: {}",
                            xml_filename, language, e
                        )));
                    }
                    _ => {} // Ignore other events like comments, processing instructions, etc.
                }
            }
            language_data.insert(XmlFile(xml_filename.clone()), single_file_data);
            // println!("[Reader: {}] Finished processing file: {}", language, xml_filename); // Debug
        }

        println!("[Reader: {}] Successfully finished reading all files.", language);
        Ok(language_data)
    }

    // --- Synchronous Helper: Processes ONE bilingual pair ---
    // Intended to be run inside tokio::task::spawn_blocking
    fn process_single_bilingual_sync(
        // Pass necessary parts of self or cloned data
        working_dir: PathBuf,
        files_to_process: Arc<Vec<String>>,
        // ---
        primary_language: &str,
        secondary_language: &str,
        primary_data: SharedLanguageData,   // Receive Arc'd data
        secondary_data: SharedLanguageData, // Receive Arc'd data
        english_data: SharedLanguageData,   // Receive Arc'd data
    ) -> Result<PathBuf, BilingualGeneratorError> {
        println!("[Processor: {} + {}] Starting processing.", primary_language, secondary_language);
        // Create output directory for this specific pair
        let xml_output_dir = working_dir.join(format!(
            "bilingual_xml\\{} + {}\\Localization", // Using Windows-style separator, consider std::path::MAIN_SEPARATOR
            primary_language, secondary_language
        ));

        // Create dir: This might race if called concurrently, but create_dir_all is idempotent.
        std::fs::create_dir_all(&xml_output_dir)
            .map_err(|e| BilingualGeneratorError::XmlProcessingFailed(format!("Error creating output directory {}: {}", xml_output_dir.display(), e)))?;

        // let primary_lang_id = Language(primary_language.to_string()); // Primarily for error messages now
        // let secondary_lang_id = Language(secondary_language.to_string());
        // let english_lang_id = Language("English".to_string()); // Assuming "English" is the fixed key

        let menutext_too_long = ["ui_state_health_desc", "ui_state_hunger_desc", "ui_DerivStat_MaxStamina_desc"];

        let mut generated_xml_paths = Vec::new(); // Collect paths *for this pair*

        // Process each XML file sequentially within this blocking task.
        // Using Rayon's par_iter here is possible but adds complexity if the per-file
        // processing is already fast enough. Sticking to sequential for simplicity first.
        for file_name in files_to_process.iter() {
            let xml_file_id = XmlFile(file_name.clone());

            // Get data for *this specific file* from the pre-read language data maps
            let primary_entries = primary_data.get(&xml_file_id);
            let secondary_entries = secondary_data.get(&xml_file_id);
            let english_entries = english_data.get(&xml_file_id);

            // If primary data for this specific file doesn't exist, we must skip it for this pair.
            let primary_entries_map = match primary_entries {
                Some(data) => data,
                None => {
                    // This shouldn't happen if the reader task completed successfully and included this file,
                    // but handle defensively.
                    eprintln!(
                        "[Processor: {} + {}] Warning: Missing primary data for file {} in language {}. Skipping file for this pair.",
                        primary_language, secondary_language, file_name, primary_language
                    );
                    continue; // Skip this file
                }
            };

            // Handle potentially missing secondary/english data gracefully (use empty map as fallback)
            let empty_map: IndexMap<EntryId, LastTextValue> = IndexMap::new();
            let secondary_entries_map = secondary_entries.unwrap_or(&empty_map);
            let english_entries_map = english_entries.unwrap_or(&empty_map);

            let mut rows = Vec::new();
            // Iterate through the primary language entries as the base
            for (entry_id, primary_text_val) in primary_entries_map {
                let primary_text = &primary_text_val.0; // Get the String from LastTextValue

                // Find corresponding secondary and English text, defaulting to "MISSING"
                let secondary_text = secondary_entries_map.get(entry_id).map(|lv| lv.0.as_str()).unwrap_or("MISSING");
                let english_text = english_entries_map.get(entry_id).map(|lv| lv.0.as_str()).unwrap_or("MISSING");

                // --- Combine text logic (Copied from original, using static helper) ---
                let combined_text = match file_name.as_str() {
                    "text_ui_menus.xml" => {
                        if !entry_id.0.contains("ui_helpoverlay") {
                            match true {
                                _ if entry_id.0.contains("ui_loading") || entry_id.0.contains("codex_cont") => {
                                    secondary_text_combined(
                                        primary_text_val, // Pass LastTextValue here
                                        secondary_text,
                                        SEPARATOR_NEWLINE,
                                    )
                                }
                                _ if primary_text.chars().count() <= 4 || menutext_too_long.contains(&&*entry_id.0) => {
                                    primary_text.clone() // Just primary
                                }
                                _ if primary_text.chars().count() >= 20 => secondary_text_combined(primary_text_val, secondary_text, SEPARATOR_NEWLINE),
                                _ => secondary_text_combined(primary_text_val, secondary_text, SEPARATOR_SLASH),
                            }
                        } else {
                            primary_text.clone() // Just primary for help overlay
                        }
                    }
                    "text_ui_dialog.xml" => {
                        if secondary_text != "MISSING" {
                            format!("{}{}{}", primary_text, SEPARATOR_NEWLINE, secondary_text).into()
                        } else {
                            // Fallback to English if secondary is missing
                            format!("{}{}{}", primary_text, SEPARATOR_NEWLINE, english_text).into()
                        }
                    }
                    "text_ui_items.xml" => {
                        match true {
                            _ if (entry_id.0.contains("step") && !entry_id.0.contains("_step_1") && primary_text.chars().count() >= 10)
                                || (entry_id.0.contains("step_1")
                                    && (entry_id.0.contains("scatter") || entry_id.0.contains("longWeak") || entry_id.0.contains("bane"))) =>
                            {
                                primary_text.clone() // Just primary
                            }
                            _ if primary_text.chars().count() >= 7 => secondary_text_combined(primary_text_val, secondary_text, SEPARATOR_NEWLINE),
                            _ => secondary_text_combined(primary_text_val, secondary_text, SEPARATOR_SLASH),
                        }
                    }
                    "text_ui_soul.xml" => match true {
                        _ if (primary_text.chars().count() <= 7 && primary_text != "MISSING")
                            || (primary_text.chars().count() <= 12 && entry_id.0.contains("stat_")) =>
                        {
                            secondary_text_combined(primary_text_val, secondary_text, SEPARATOR_SLASH)
                        }
                        _ if (entry_id.0.contains("buff") && entry_id.0.contains("desc") && !entry_id.0.contains("drunkenness_desc"))
                            || (entry_id.0.contains("perk") && entry_id.0.contains("_desc")) =>
                        {
                            secondary_text_combined(primary_text_val, secondary_text, SEPARATOR_NEWLINE)
                        }
                        _ => primary_text.clone(), // Just primary
                    },
                    _ => {
                        // Default case for other files (e.g., text_ui_quest, text_ui_tutorials)
                        if secondary_text != "MISSING" {
                            format!("{}{}{}", primary_text, SEPARATOR_SLASH, secondary_text).into()
                        } else {
                            // Fallback to English
                            format!("{}{}{}", primary_text, SEPARATOR_SLASH, english_text).into()
                        }
                    }
                };
                // --- End Combine text logic ---

                rows.push(format!(
                    "<Row><Cell>{}</Cell><Cell>{}</Cell><Cell>{}</Cell></Row>",
                    entry_id.0, primary_text, combined_text
                ));

                // Escape cell content to prevent invalid XML
                // Note: `quick_xml::escape::escape` returns Cow<str>, use `.as_ref()` if needed elsewhere
                // let escaped_entry_id = quick_xml::escape::escape(&entry_id.0);
                // let escaped_primary_text = quick_xml::escape::escape(primary_text); // Use the &str
                // let escaped_combined_text = quick_xml::escape::escape(&combined_text);

                // rows.push(format!(
                //     "<Row><Cell>{}</Cell><Cell>{}</Cell><Cell>{}</Cell></Row>",
                //     escaped_entry_id, escaped_primary_text, escaped_combined_text
                // ));
            }

            // Write the generated XML content to a file
            let xml_content = format!("<Table>\n{}\n</Table>", rows.join("\n"));
            let xml_output_path = xml_output_dir.join(file_name);

            // Writing file is synchronous I/O
            std::fs::write(&xml_output_path, xml_content)
                .map_err(|e| BilingualGeneratorError::XmlProcessingFailed(format!("Error writing XML file {}: {}", xml_output_path.display(), e)))?;
            generated_xml_paths.push(xml_output_path); // Add path for later PAK creation
        }

        // --- PAK Creation & Cleanup for *this pair* ---
        // This runs synchronously after all files for the pair are written.
        println!("[Processor: {} + {}] Creating PAK file.", primary_language, secondary_language);
        // Use the original static create_new_pak method (assuming it's accessible)
        // Note: If create_new_pak needs `self`, it needs refactoring or pass needed data.
        // Assuming it's a static-like method or associated function:
        match create_new_pak(
            generated_xml_paths.clone(), // Clone paths for PAK function
            &xml_output_dir,             // Pass the specific output dir
            primary_language,
        ) {
            Ok(_) => {
                println!(
                    "[Processor: {} + {}] PAK created successfully. Cleaning up XML files.",
                    primary_language, secondary_language
                );
                // Cleanup generated XML files for this pair *after* PAK is successfully created.
                // Rayon can be used here for parallel deletion if needed.
                generated_xml_paths.par_iter().for_each(|path| {
                    if let Err(e) = std::fs::remove_file(path) {
                        // Log error but don't fail the whole process for a cleanup failure
                        eprintln!(
                            "[Processor: {} + {}] [Cleanup] Failed to delete {}: {}",
                            primary_language,
                            secondary_language,
                            path.display(),
                            e
                        );
                    }
                });
                println!("[Processor: {} + {}] Finished processing.", primary_language, secondary_language);
                Ok(xml_output_dir) // Return the directory path for this pair on success
            }
            Err(e) => {
                eprintln!("[Processor: {} + {}] Failed to create PAK: {:?}", primary_language, secondary_language, e);
                // Return PakCreationFailed or the specific error from create_new_pak
                Err(BilingualGeneratorError::PakCreationFailed)
            }
        }
    }

    // --- Helper to clone necessary data for blocking tasks ---
    // Clones only the immutable fields needed by the sync helpers.
    fn clone_for_processing(&self) -> (PathBuf, PathBuf, Arc<Vec<String>>) {
        (
            self.game_path.clone(),
            self.working_dir.clone(),
            Arc::new(self.files_to_process.clone()), // Clone Vec into new Arc
        )
    }

    // --- Main Asynchronous Orchestrator ---
    pub async fn generate_bilingual_resources_async(
        &mut self, // Takes &mut self to potentially update language_to_process via acquire_bilingual_set
    ) -> Result<Vec<String>, BilingualGeneratorError> {
        // 1. Acquire bilingual pairs and update languages to process
        // Run this synchronously first.
        let bilingual_set = self.acquire_bilingual_set()?; // This might update self.language_to_process
        if bilingual_set.is_empty() {
            println!("No bilingual pairs found in bilingual_set.txt. Exiting.");
            return Ok(Vec::new());
        }

        // 2. Identify all unique languages needed (including English fallback)
        // Use the potentially updated self.language_to_process
        let mut required_languages: HashSet<String> = HashSet::new();
        required_languages.insert("English".to_string()); // Always needed
        for lang in &self.language_to_process {
            required_languages.insert(lang.clone());
        }
        // Also ensure languages from the set are included (acquire_bilingual_set should handle this)
        for (p, s) in &bilingual_set {
            required_languages.insert(p.clone());
            required_languages.insert(s.clone());
        }

        println!("Required languages to read: {:?}", required_languages.iter().collect::<Vec<_>>());

        // 3. Setup communication channel (MPSC: Multi-Producer, Single-Consumer)
        // Channel stores results from reader tasks. Size based on number of languages.
        let (tx, mut rx) = mpsc::channel::<ReaderResult>(required_languages.len());

        // 4. Spawn reader tasks using tokio::spawn
        let mut reader_handles = Vec::new();
        let files_to_process_arc = Arc::new(self.files_to_process.clone()); // Arc once

        for lang_str in required_languages {
            let tx_clone = tx.clone(); // Clone sender for each task
            let game_path_clone = self.game_path.clone();
            let files_arc_clone = Arc::clone(&files_to_process_arc);
            let lang_str_clone_for_blocking = lang_str.clone();
            let handle: JoinHandle<()> = task::spawn(async move {
                // Use spawn_blocking for the synchronous file I/O and parsing
                let result = task::spawn_blocking(move || {
                    Self::read_single_language_xmls_sync(
                        lang_str_clone_for_blocking, // Clone lang_str for the blocking task
                        game_path_clone,
                        files_arc_clone,
                    )
                })
                .await;

                // Process result from spawn_blocking
                let final_result: Result<SharedLanguageData, BilingualGeneratorError> = match result {
                    Ok(Ok(data)) => Ok(Arc::new(data)), // Wrap successful data in Arc
                    Ok(Err(e)) => Err(e),               // Propagate the error from read_single_language_xmls_sync
                    Err(join_err) => {
                        // Error if the blocking task itself panicked or was cancelled
                        eprintln!("[Reader: {}] Task join error: {}", lang_str, join_err);
                        Err(BilingualGeneratorError::TaskJoinError(format!(
                            "Reader task for {} panicked or was cancelled: {}",
                            lang_str, join_err
                        )))
                    }
                };

                // Send result back to the coordinator via the channel
                let lang = Language(lang_str); // Create Language struct
                if let Err(send_err) = tx_clone.send((lang.clone(), final_result)).await {
                    // This error happens if the receiver (rx) has been dropped,
                    // which shouldn't normally occur while readers are running.
                    eprintln!("Error sending data back for language {}: {}. Receiver likely dropped.", lang.0, send_err);
                }
            });
            reader_handles.push(handle);
        }
        // Drop the original sender. The channel closes when all tx_clone instances are dropped.
        drop(tx);
        println!("All reader tasks spawned.");

        // 5. Coordinator: Receive data, manage state, and spawn processing tasks
        let mut read_data: HashMap<Language, SharedLanguageData> = HashMap::new();
        let mut pending_pairs = bilingual_set.clone(); // Track pairs waiting for data
        let mut processing_handles: Vec<JoinHandle<Result<PathBuf, BilingualGeneratorError>>> = Vec::new();
        let mut accumulated_errors: Vec<BilingualGeneratorError> = Vec::new(); // Collect errors

        // Ensure the base output directory exists before spawning processors
        let base_output_dir = self.working_dir.join("bilingual_xml");
        if let Err(e) = std::fs::create_dir_all(&base_output_dir) {
            eprintln!("Failed to create base output directory '{}': {}", base_output_dir.display(), e);
            return Err(BilingualGeneratorError::IoError(e)); // Use appropriate error variant
        }

        println!("Coordinator started. Waiting for language data...");
        // Loop while the receiver channel is open and receiving messages
        while let Some((lang, result)) = rx.recv().await {
            match result {
                Ok(data_arc) => {
                    println!("Coordinator: Received data for language: {}", lang.0);
                    read_data.insert(lang, data_arc); // Store the Arc'd data
                }
                Err(e) => {
                    eprintln!("Coordinator: Received error for language {}: {:?}", lang.0, e);
                    accumulated_errors.push(e);
                    // Note: We don't store failed languages in read_data. Pairs needing it won't start.
                }
            }

            // Try to launch processing for any pairs that are now ready
            let mut still_pending = Vec::new(); // Build the next list of pending pairs
            for pair in pending_pairs {
                let (p_str, s_str) = pair;
                let p_lang = Language(p_str.clone());
                let s_lang = Language(s_str.clone());
                let eng_lang = Language("English".to_string());

                // Check if data for Primary, Secondary, AND English is available (i.e., successfully read)
                let p_data_arc = read_data.get(&p_lang);
                let s_data_arc = read_data.get(&s_lang);
                let eng_data_arc = read_data.get(&eng_lang); // Crucial: Check for English data

                if let (Some(p_arc), Some(s_arc), Some(eng_arc)) = (p_data_arc, s_data_arc, eng_data_arc) {
                    // All data is ready for this pair! Spawn a processing task.
                    println!("Coordinator: Data ready for pair: {} + {}. Spawning processor.", p_str, s_str);

                    // Clone Arcs for the new task
                    let p_clone = Arc::clone(p_arc);
                    let s_clone = Arc::clone(s_arc);
                    let eng_clone = Arc::clone(eng_arc);

                    // Clone necessary context data (paths, file list)
                    let (_game_path_clone, working_dir_clone, files_arc_clone) = self.clone_for_processing(); // Use the helper

                    // Spawn the synchronous processing logic in a blocking task
                    let handle = task::spawn_blocking(move || {
                        Self::process_single_bilingual_sync(
                            working_dir_clone,
                            files_arc_clone,
                            &p_str, // Pass strs
                            &s_str,
                            p_clone, // Pass Arcs
                            s_clone,
                            eng_clone,
                        )
                    });
                    processing_handles.push(handle);
                } else {
                    // Data not yet ready, keep this pair in the pending list for the next check
                    still_pending.push((p_str, s_str));
                }
            }
            pending_pairs = still_pending; // Update the list of pairs still waiting
        }

        // rx loop finished: means all reader tasks have completed and sent their results (or failed trying)
        println!("Coordinator: All language data received or reader tasks finished.");

        // 6. Wait for all processing tasks to complete
        println!("Coordinator: Waiting for {} processing tasks to complete...", processing_handles.len());
        let mut messages = Vec::new();
        for handle in processing_handles {
            match handle.await {
                Ok(Ok(output_dir)) => {
                    // Processing task completed successfully
                    // Try to reconstruct the message from the output dir path
                    if let Some(folder_name) = output_dir
                        .parent() // Go up from Localization
                        .and_then(|p| p.file_name()) // Get "Primary + Secondary" dir name
                        .and_then(|n| n.to_str())
                    {
                        let parts: Vec<&str> = folder_name.split(" + ").collect();
                        if parts.len() == 2 {
                            messages.push(format!("primary_language = {}, secondary_language = {}", parts[0], parts[1]));
                        } else {
                            messages.push(format!("Successfully processed pair (dir: {})", folder_name));
                        }
                    } else {
                        messages.push(format!("Successfully processed pair (path: {})", output_dir.display()));
                    }
                }
                Ok(Err(e)) => {
                    // Processing task returned an error
                    eprintln!("Coordinator: Processing task failed: {:?}", e);
                    accumulated_errors.push(e);
                }
                Err(join_err) => {
                    // Processing task panicked or was cancelled
                    eprintln!("Coordinator: Processing task join error: {}", join_err);
                    accumulated_errors.push(BilingualGeneratorError::TaskJoinError(format!(
                        "Processing task panicked or was cancelled: {}",
                        join_err
                    )));
                }
            }
        }

        println!("Coordinator: All processing tasks finished.");

        // 7. Final Result Aggregation
        // Check if any pairs never got processed
        if !pending_pairs.is_empty() {
            eprintln!(
                "Coordinator: Warning! The following pairs could not be processed due to missing language data (likely reader errors): {:?}",
                pending_pairs
            );
            // Add a general error indicating incomplete processing
            accumulated_errors.push(BilingualGeneratorError::XmlProcessingFailed(format!(
                "Failed to process pairs due to missing data: {:?}",
                pending_pairs
            )));
        }

        if !accumulated_errors.is_empty() {
            // If there were any errors during reading or processing, return the first one encountered.
            // Could potentially return a Vec<Error> or a custom aggregate error type.
            eprintln!("Coordinator: Finished with errors.");
            Err(accumulated_errors.remove(0)) // Return the first error
        } else {
            println!("Coordinator: Finished successfully.");
            // Only return Ok if there were no errors *and* no pairs left pending.
            Ok(messages)
        }
    }
} // end impl BilingualGenerator for async methods
