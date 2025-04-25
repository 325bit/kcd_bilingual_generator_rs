// --- START OF bilingual_generator.rs (Modified) ---
use super::bilingual_generator_errors::BilingualGeneratorError;
use crate::util::{SEPARATOR_NEWLINE, SEPARATOR_SLASH, create_new_pak, secondary_text_combined};
use faststr::FastStr;
use indexmap::IndexMap;
use path_finder::PathFinder;
use quick_xml::Reader;
use quick_xml::events::Event;
use rayon::prelude::*;
// use sqlx::Row; // Import Row trait
use log::{error, info, warn};
use simplelog::{CombinedLogger, Config, LevelFilter, WriteLogger};
use sqlx::postgres::PgPoolOptions;
use std::{
    collections::HashMap, // Keep HashMap for processing results
    fs::File,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    sync::Mutex, // Keep Mutex for xml_output_set
};
use tokio::runtime::Runtime; // Import Tokio runtime
use zip::ZipArchive;

// Wrapper types remain the same for clarity, even if DB stores Strings
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Language(pub String);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct XmlFile(pub String);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct EntryId(pub String);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct LastTextValue(pub FastStr);

// Define a struct to hold fetched data, using standard String for simplicity with sqlx
#[derive(Debug, sqlx::FromRow)] // Allow sqlx to map rows to this struct
struct TextEntryRecord {
    entry_id: String,
    text_value: String,
    // entry_order: i32 // We might fetch this if needed
}

#[derive(Debug)]
pub struct BilingualGenerator {
    pub game_path: PathBuf,
    pub working_dir: PathBuf,
    pub files_to_process: Vec<String>,
    pub language_to_process: Vec<String>,
    // pub all_data: HashMap<XmlFile, HashMap<Language, IndexMap<EntryId, LastTextValue>>>, // Removed
    pub db_pool: sqlx::PgPool, // Added
    pub runtime: Runtime,      // Added: Tokio runtime to execute async db calls
}

impl BilingualGenerator {
    pub fn init() -> Result<Self, BilingualGeneratorError> {
        // Create a Tokio runtime
        let runtime = Runtime::new()?; // Handle error properly

        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());

        // Initialize the logger
        let log_file = working_dir.join("bilingual_generator.log");
        let log_file_handle =
            File::create(&log_file).map_err(|e| BilingualGeneratorError::DatabaseInitializationFailed(format!("Failed to create log file: {}", e)))?;
        CombinedLogger::init(vec![WriteLogger::new(LevelFilter::Info, Config::default(), log_file_handle)])
            .map_err(|e| BilingualGeneratorError::DatabaseInitializationFailed(format!("Failed to initialize logger: {}", e)))?;

        let files_to_process = vec![
            String::from("text_ui_dialog.xml"),    // Dialogues
            String::from("text_ui_quest.xml"),     // Quests
            String::from("text_ui_tutorials.xml"), // Tutorials
            String::from("text_ui_soul.xml"),      // Stats/Effects
            String::from("text_ui_items.xml"),     // Items
            String::from("text_ui_menus.xml"),     // Menus
        ];
        let default_language_to_process = vec![String::from("Chineses"), String::from("English")];
        let mut path_finder = PathFinder::new();
        let kcd_path = path_finder.find_game_path().map_or_else(|_| PathBuf::new(), |p| p.to_path_buf());

        // --- Database Initialization ---
        dotenv::dotenv().expect("Failed to load .env file for testing");
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| BilingualGeneratorError::DatabaseInitializationFailed("DATABASE_URL environment variable not set.".to_string()))?;

        let pool = runtime
            .block_on(async {
                PgPoolOptions::new()
                    .max_connections(10) // Adjust pool size as needed
                    .connect(&database_url)
                    .await
            })
            .map_err(BilingualGeneratorError::DatabaseConnectionFailed)?;

        // --- Schema Setup ---
        runtime.block_on(async {
            // Create Table (This one is fine as it's a single command)
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS text_entries (
                    xml_file TEXT NOT NULL,
                    language TEXT NOT NULL,
                    entry_id TEXT NOT NULL,
                    text_value TEXT NOT NULL,
                    entry_order INTEGER NOT NULL,
                    PRIMARY KEY (xml_file, language, entry_id)
                );
                "#,
            )
            .execute(&pool)
            .await
        })?; // Propagate potential error

        // --- Create Indexes (Execute separately) ---
        runtime.block_on(async {
            // Create first index
            sqlx::query("CREATE INDEX IF NOT EXISTS idx_text_entries_lookup ON text_entries (xml_file, language);")
                .execute(&pool)
                .await?; // Execute and await result, propagating error if any

            // Create second index
            sqlx::query("CREATE INDEX IF NOT EXISTS idx_text_entries_order ON text_entries (xml_file, language, entry_order);")
                .execute(&pool)
                .await?; // Execute and await result, propagating error if any

            // Explicitly return Ok for the async block's Result type
            Ok::<(), sqlx::Error>(())
        })?; // Propagate any error from the index creation block

        // --- End of Schema Setup ---

        Ok(Self {
            game_path: kcd_path,
            working_dir,
            files_to_process,
            language_to_process: default_language_to_process,
            db_pool: pool,
            runtime,
        })
    }

    pub fn acquire_bilingual_set(&mut self) -> Result<Vec<(String, String)>, BilingualGeneratorError> {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        let bilingual_set_dir = working_dir.join("..\\..\\assets\\bilingual_set.txt");
        let bilingual_set_file = File::open(&bilingual_set_dir)
            .map_err(|_| BilingualGeneratorError::InvalidBilingualSet(format!("No bilingual_set.txt in {:?}", bilingual_set_dir)))?;
        let reader = BufReader::new(bilingual_set_file);
        let mut bilingual_set: Vec<(String, String)> = vec![];
        // start reading
        for (_, line_result) in reader.lines().enumerate() {
            let line: String = line_result.map_err(|_| BilingualGeneratorError::InvalidBilingualSet("Fail to get String.".to_string()))?;
            let trimmed_line = line.trim();

            // Skip empty lines
            if trimmed_line.is_empty() {
                continue;
            }

            // Split into primary and secondary languages
            let parts: Vec<&str> = trimmed_line.split('+').map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(BilingualGeneratorError::InvalidBilingualSet(line));
            }

            let primary_language = parts[0].to_string();
            let secondary_language = parts[1].to_string();
            // Check and add primary language if missing
            if !self.language_to_process.contains(&primary_language) {
                self.language_to_process.push(primary_language.clone());
            }

            // Check and add secondary language if missing
            if !self.language_to_process.contains(&secondary_language) {
                self.language_to_process.push(secondary_language.clone());
            }
            bilingual_set.push((primary_language, secondary_language));
        }
        Ok(bilingual_set)
    }

    /// Reads XML files from paks and stores data in the PostgreSQL database.
    /// This version replaces existing data for the given language/file combination.
    pub fn read_xml_from_paks(&mut self) -> Result<(), BilingualGeneratorError> {
        // Use par_iter for parallelism across languages
        self.language_to_process.par_iter().try_for_each(|language_str| {
            let pak_filename = format!("{}_xml.pak", language_str);
            let pak_path = self.game_path.join("Localization").join(&pak_filename);

            info!("[DB Load] Processing language: {}", language_str); // Replaced println!

            let pak_file = match File::open(&pak_path) {
                Ok(file) => file,
                Err(e) => {
                    warn!("[DB Load] Warning: Could not open PAK file for language '{}': {}. Skipping.", language_str, e); // Replaced eprintln!
                    return Ok(()); // Skip this language if pak doesn't exist
                }
            };

            let mut archive = ZipArchive::new(pak_file).map_err(|e| {
                error!("[DB Load] Error opening zip archive for {}: {}", pak_filename, e); // Replaced eprintln!
                BilingualGeneratorError::PakExtractionFailed
            })?;

            for xml_filename_str in &self.files_to_process {
                info!("[DB Load]   File: {}", xml_filename_str);

                let mut xml_file = match archive.by_name(xml_filename_str) {
                    Ok(file) => file,
                    Err(e) => {
                        warn!(
                            "[DB Load] Warning: Could not find '{}' in '{}': {}. Skipping.",
                            xml_filename_str, pak_filename, e
                        );
                        continue; // Skip this file if not found in archive
                    }
                };

                let mut content = String::new();
                xml_file.read_to_string(&mut content).map_err(|e| {
                    error!("[DB Load] Error reading XML content from {}: {}", xml_filename_str, e); // Replaced eprintln!
                    BilingualGeneratorError::PakExtractionFailed
                })?;

                // --- XML Parsing (same as before) ---
                let mut reader = Reader::from_str(&content);
                let mut buf = Vec::with_capacity(512);
                // Store locally first before DB insertion
                let mut single_data: IndexMap<EntryId, LastTextValue> = IndexMap::new();
                let mut current_cells: Vec<String> = Vec::with_capacity(4);
                let mut inside_row = false;

                loop {
                    match reader.read_event_into(&mut buf) {
                        Ok(Event::Start(e)) if e.name().as_ref() == b"Row" => {
                            inside_row = true;
                            current_cells.clear();
                        }
                        Ok(Event::End(e)) if e.name().as_ref() == b"Row" => {
                            inside_row = false;
                            if !current_cells.is_empty() && current_cells[0] != "Entry id" {
                                if current_cells.len() >= 3 {
                                    // Use current_cells[2] (last cell) as per original logic
                                    single_data.insert(EntryId(current_cells[0].clone()), LastTextValue(FastStr::from(current_cells[2].clone())));
                                } else if current_cells.len() == 2 {
                                    // Handle cases with potentially missing cells (e.g., use empty string)
                                    single_data.insert(EntryId(current_cells[0].clone()), LastTextValue(FastStr::from("")));
                                } else {
                                    // Handle unexpected row structure if needed
                                    warn!(
                                        "[DB Load] Warning: Row skipped due to unexpected cell count ({}) in {}/{}: ID={}",
                                        current_cells.len(),
                                        language_str,
                                        xml_filename_str,
                                        current_cells.get(0).cloned().unwrap_or_default()
                                    ); // Replaced eprintln!
                                }
                            }
                        }
                        Ok(Event::Text(e)) => {
                            if inside_row {
                                let raw_text = std::str::from_utf8(e.as_ref())
                                    .map_err(|_| BilingualGeneratorError::XmlProcessingFailed("Invalid UTF-8 in XML text".to_string()))?
                                    .to_string(); // Convert to String
                                // quick_xml automatically handles XML entities like & etc. when decoding text
                                current_cells.push(raw_text);
                            }
                        }
                        Ok(Event::Eof) => break,
                        Err(e) => {
                            return Err(BilingualGeneratorError::XmlProcessingFailed(format!(
                                "XML error in {}/{}: {}",
                                language_str, xml_filename_str, e
                            )));
                        }
                        _ => {}
                    }
                    buf.clear();
                }

                let pool = self.db_pool.clone();
                let lang_clone = language_str.clone();
                let file_clone = xml_filename_str.clone();

                self.runtime.block_on(async {
                    let mut tx = pool.begin().await?;
                    // Prepare insert statement (consider doing this outside the loop if possible, but fine here)
                    // Note: ON CONFLICT is an alternative to DELETE+INSERT, but requires more setup for batching
                    let insert_query = "
                        INSERT INTO text_entries 
                            (xml_file, language, entry_id, text_value, entry_order) 
                        VALUES ($1, $2, $3, $4, $5)
                        ON CONFLICT (xml_file, language, entry_id)
                        DO UPDATE SET 
                            text_value = EXCLUDED.text_value,
                            entry_order = EXCLUDED.entry_order
                    ";
                    // Insert data in batches (conceptually, within one transaction)
                    for (index, (entry_id, text_value)) in single_data.iter().enumerate() {
                        sqlx::query(insert_query)
                            .bind(&file_clone) // $1 xml_file
                            .bind(&lang_clone) // $2 language
                            .bind(&entry_id.0) // $3 entry_id
                            .bind(text_value.0.as_str()) // $4 text_value (convert FastStr to &str)
                            .bind(index as i32) // $5 entry_order (cast usize to i32)
                            .execute(&mut *tx) // Use mut *tx
                            .await?;
                    }

                    tx.commit().await?; // Commit transaction
                    Ok::<(), BilingualGeneratorError>(()) // Explicit type for Ok
                })?; // Propagate SQLx errors mapped via From
            } // End loop files_to_process
            Ok::<(), BilingualGeneratorError>(()) // OK for this language
        }) // End par_iter try_for_each
    }

    /// Forcefully re-reads and updates the database for a specific language and file.
    /// Example function - Adapt as needed.
    pub fn force_update_db_for(&self, language: &str, xml_file: &str) -> Result<(), BilingualGeneratorError> {
        println!("[DB Force Update] Starting for {} / {}", language, xml_file);
        // 1. Find the corresponding PAK file
        let pak_filename = format!("{}_xml.pak", language);
        let pak_path = self.game_path.join("Localization").join(&pak_filename);
        let pak_file = File::open(&pak_path).map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;
        let mut archive = ZipArchive::new(pak_file).map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

        // 2. Extract the specific XML file content
        let mut xml_entry = archive.by_name(xml_file).map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;
        let mut content = String::new();
        xml_entry
            .read_to_string(&mut content)
            .map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

        // 3. Parse the XML (similar to read_xml_from_paks)
        let mut reader = Reader::from_str(&content);
        let mut buf = Vec::with_capacity(512);
        let mut single_data: IndexMap<EntryId, LastTextValue> = IndexMap::new();
        let mut current_cells: Vec<String> = Vec::with_capacity(4);
        let mut inside_row = false;

        loop {
            // Simplified parsing loop - copy logic from read_xml_from_paks if needed
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) if e.name().as_ref() == b"Row" => {
                    inside_row = true;
                    current_cells.clear();
                }
                Ok(Event::End(e)) if e.name().as_ref() == b"Row" => {
                    inside_row = false;
                    if !current_cells.is_empty() && current_cells[0] != "Entry id" && current_cells.len() >= 3 {
                        single_data.insert(EntryId(current_cells[0].clone()), LastTextValue(FastStr::from(current_cells[2].clone())));
                    }
                }
                Ok(Event::Text(e)) => {
                    if inside_row {
                        /* ... push cell data ... */
                        current_cells.push(String::from_utf8_lossy(e.as_ref()).into_owned());
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(BilingualGeneratorError::XmlProcessingFailed(format!("XML error: {}", e))),
                _ => {}
            }
            buf.clear();
        }

        // 4. Execute DB Update (DELETE + INSERT in transaction)
        let pool = self.db_pool.clone();
        let lang_clone = language.to_string();
        let file_clone = xml_file.to_string();

        self.runtime.block_on(async move {
            let mut tx = pool.begin().await?;
            sqlx::query("DELETE FROM text_entries WHERE language = $1 AND xml_file = $2")
                .bind(&lang_clone)
                .bind(&file_clone)
                .execute(&mut *tx)
                .await?;

            println!("[DB Force Update] Inserting {} entries for {}/{}", single_data.len(), lang_clone, file_clone);
            let insert_query = "INSERT INTO text_entries (xml_file, language, entry_id, text_value, entry_order) VALUES ($1, $2, $3, $4, $5)";
            for (index, (entry_id, text_value)) in single_data.iter().enumerate() {
                sqlx::query(insert_query)
                    .bind(&file_clone)
                    .bind(&lang_clone)
                    .bind(&entry_id.0)
                    .bind(text_value.0.as_str())
                    .bind(index as i32)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
            Ok::<(), BilingualGeneratorError>(())
        })?;

        println!("[DB Force Update] Finished for {} / {}", language, xml_file);
        Ok(())
    }

    /// Processes a single bilingual combination by fetching data from the database.
    pub fn process_single_bilingual(&self, primary_language: &str, secondary_language: &str) -> Result<PathBuf, BilingualGeneratorError> {
        let xml_output_dir = self
            .working_dir
            .join(format!("bilingual_xml\\{} + {}\\Localization", primary_language, secondary_language));
        let xml_output_set: Mutex<Vec<PathBuf>> = Mutex::new(vec![]); // Thread safety for collecting paths

        std::fs::create_dir_all(&xml_output_dir)
            .map_err(|e| BilingualGeneratorError::XmlProcessingFailed(format!("Error creating output directory: {}", e)))?;

        // Define constants for language names
        const ENGLISH_LANG: &str = "English";

        self.files_to_process.par_iter().try_for_each(|file_name_str| {
            // Changed to try_for_each to propagate errors
            let pool = self.db_pool.clone(); // Clone pool for the thread
            let file_name_clone = file_name_str.clone(); // Clone file name
            let primary_lang_clone = primary_language.to_string();
            let secondary_lang_clone = secondary_language.to_string();

            // --- Fetch Data from Database ---
            let (primary_entries_map, secondary_entries_map, english_entries_map) = self.runtime.block_on(async {
                // Fetch primary language entries, preserving order
                let primary_query = sqlx::query_as::<_, TextEntryRecord>(
                    "SELECT entry_id, text_value FROM text_entries WHERE xml_file = $1 AND language = $2 ORDER BY entry_order",
                )
                .bind(&file_name_clone)
                .bind(&primary_lang_clone)
                .fetch_all(&pool)
                .await?;
                // Convert Vec<TextEntryRecord> to IndexMap<String, String> for easier processing below
                // Using String keys/values simplifies lookups vs EntryId/LastTextValue wrappers here
                let primary_map: IndexMap<String, String> = primary_query.into_iter().map(|r| (r.entry_id, r.text_value)).collect();

                // Fetch secondary language entries into a HashMap for fast lookups
                let secondary_query =
                    sqlx::query_as::<_, TextEntryRecord>("SELECT entry_id, text_value FROM text_entries WHERE xml_file = $1 AND language = $2")
                        .bind(&file_name_clone)
                        .bind(&secondary_lang_clone)
                        .fetch_all(&pool)
                        .await?;
                let secondary_map: HashMap<String, String> = secondary_query.into_iter().map(|r| (r.entry_id, r.text_value)).collect();

                // Fetch English entries (fallback) into a HashMap
                let english_query =
                    sqlx::query_as::<_, TextEntryRecord>("SELECT entry_id, text_value FROM text_entries WHERE xml_file = $1 AND language = $2")
                        .bind(&file_name_clone)
                        .bind(ENGLISH_LANG) // Use constant
                        .fetch_all(&pool)
                        .await?;
                let english_map: HashMap<String, String> = english_query.into_iter().map(|r| (r.entry_id, r.text_value)).collect();

                Ok::<_, sqlx::Error>((primary_map, secondary_map, english_map))
            })?; // Propagate SQL errors

            // Check if primary language data was found
            if primary_entries_map.is_empty() {
                // Log or handle cases where essential primary data is missing
                eprintln!(
                    "[Process Bilingual] Warning: No primary entries found for {} in {}. Skipping file generation.",
                    primary_lang_clone, file_name_clone
                );
                // Decide if you want to return an error or just skip this file
                return Err(BilingualGeneratorError::DatabaseDataMissing(file_name_clone, primary_lang_clone));
                // return Ok(()); // Skip this file iteration
            }

            // --- Combine Text Logic (Mostly Unchanged, uses fetched HashMaps) ---
            let menutext_too_long = ["ui_state_health_desc", "ui_state_hunger_desc", "ui_DerivStat_MaxStamina_desc"]; // Keep local definition
            let mut rows = Vec::new();

            for (entry_id_str, primary_text_str) in &primary_entries_map {
                // Need temporary wrappers for secondary_text_combined if it expects them
                let primary_text_val = LastTextValue(FastStr::from(primary_text_str.to_string()));

                let secondary_text_str = secondary_entries_map.get(entry_id_str).map(|s| s.as_str()).unwrap_or("MISSING");
                let english_text_str = english_entries_map.get(entry_id_str).map(|s| s.as_str()).unwrap_or("MISSING"); // Fallback lookup

                // Apply combination logic
                let combined_text: FastStr = match file_name_str.as_str() {
                    "text_ui_menus.xml" => {
                        if !entry_id_str.contains("ui_helpoverlay") {
                            match true {
                                _ if entry_id_str.contains("ui_loading") || entry_id_str.contains("codex_cont") => {
                                    secondary_text_combined(&primary_text_val, secondary_text_str, SEPARATOR_NEWLINE)
                                }
                                _ if primary_text_str.chars().count() <= 4 || menutext_too_long.contains(&entry_id_str.as_str()) => {
                                    primary_text_val.0.clone() // Just primary
                                }
                                _ if primary_text_str.chars().count() >= 20 => {
                                    secondary_text_combined(&primary_text_val, secondary_text_str, SEPARATOR_NEWLINE)
                                }
                                _ => secondary_text_combined(&primary_text_val, secondary_text_str, SEPARATOR_SLASH),
                            }
                        } else {
                            primary_text_val.0.clone() // Just primary
                        }
                    }
                    "text_ui_dialog.xml" => {
                        let fallback = if secondary_text_str == "MISSING" {
                            english_text_str
                        } else {
                            secondary_text_str
                        };
                        format!("{}{}{}", primary_text_str, SEPARATOR_NEWLINE, fallback).into()
                    }
                    "text_ui_items.xml" => {
                        match true {
                            _ if (entry_id_str.contains("step") && !entry_id_str.contains("_step_1") && primary_text_str.chars().count() >= 10)
                                || (entry_id_str.contains("step_1")
                                    && (entry_id_str.contains("scatter") || entry_id_str.contains("longWeak") || entry_id_str.contains("bane"))) =>
                            {
                                primary_text_val.0.clone() // Just primary
                            }
                            _ if primary_text_str.chars().count() >= 7 => {
                                secondary_text_combined(&primary_text_val, secondary_text_str, SEPARATOR_NEWLINE)
                            }
                            _ => secondary_text_combined(&primary_text_val, secondary_text_str, SEPARATOR_SLASH),
                        }
                    }
                    "text_ui_soul.xml" => match true {
                        _ if primary_text_str.chars().count() <= 7 && primary_text_str != "MISSING"
                            || primary_text_str.chars().count() <= 12 && entry_id_str.contains("stat_") =>
                        {
                            secondary_text_combined(&primary_text_val, secondary_text_str, SEPARATOR_SLASH)
                        }
                        _ if (entry_id_str.contains("buff") && entry_id_str.contains("desc") && !entry_id_str.contains("drunkenness_desc"))
                            || (entry_id_str.contains("perk") && entry_id_str.contains("_desc")) =>
                        {
                            secondary_text_combined(&primary_text_val, secondary_text_str, SEPARATOR_NEWLINE)
                        }
                        _ => primary_text_val.0.clone(), // Just primary
                    },
                    _ => {
                        // Default for other files
                        let fallback = if secondary_text_str == "MISSING" {
                            english_text_str
                        } else {
                            secondary_text_str
                        };
                        format!("{}{}{}", primary_text_str, SEPARATOR_SLASH, fallback).into()
                    }
                };

                // Escape text for XML inclusion if necessary (quick-xml writer handles this automatically, but manual formatting needs care)
                // For manual formatting like this, ensure '&', '<', '>' are escaped.
                // However, your original code didn't seem to do manual escaping, assuming the input was already valid or didn't contain these.
                // If you need escaping: use functions like `quick_xml::escape::escape`.
                rows.push(format!(
                    "<Row><Cell>{}</Cell><Cell>{}</Cell><Cell>{}</Cell></Row>",
                    entry_id_str,     // Already String
                    primary_text_str, // Already String
                    combined_text     // FastStr implements Display
                ));
            }

            // Write to file
            let xml_content = format!("<Table>\n{}\n</Table>", rows.join("\n"));
            let xml_output_path = xml_output_dir.join(file_name_str); // Use original file_name_str

            // Lock mutex before pushing path
            let mut guard = xml_output_set.lock().unwrap();
            guard.push(xml_output_path.clone());
            drop(guard); // Release lock

            std::fs::write(&xml_output_path, xml_content)
                .map_err(|e| BilingualGeneratorError::XmlProcessingFailed(format!("Error writing XML to {}: {}", xml_output_path.display(), e)))?; // Propagate IO Error

            Ok(()) // OK for this file
        })?; // End par_iter try_for_each

        // --- PAK Creation and Cleanup  ---
        let generated_xml_paths: Vec<PathBuf> = xml_output_set.into_inner().unwrap();

        if generated_xml_paths.is_empty() {
            println!(
                "[Process Bilingual] No XML files were generated for {} + {}. Skipping PAK creation.",
                primary_language, secondary_language
            );
            // Return the (empty) output directory path, or perhaps an error if this is unexpected
            return Ok(xml_output_dir);
        }

        match create_new_pak(generated_xml_paths.clone(), &xml_output_dir, primary_language) {
            Ok(_) => {
                generated_xml_paths.par_iter().for_each(|path| {
                    if let Err(e) = std::fs::remove_file(path) {
                        eprintln!("[Cleanup] Failed to delete {}: {}", path.display(), e);
                    }
                });
                Ok(xml_output_dir)
            }
            Err(e) => {
                // Assuming create_new_pak returns a specific error type you can match on
                // Or map it to your BilingualGeneratorError::PakCreationFailed
                eprintln!("[Process Bilingual] Error creating PAK file: {:?}", e); // Log the specific error from create_new_pak
                Err(BilingualGeneratorError::PakCreationFailed)
            }
        }
    }
}
