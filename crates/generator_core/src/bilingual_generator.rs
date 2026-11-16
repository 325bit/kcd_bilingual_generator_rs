use crate::util::{SEPARATOR_NEWLINE, SEPARATOR_SLASH, create_new_pak, secondary_text_combined};

use super::bilingual_generator_errors::BilingualGeneratorError;
use crate::log::LogSink;
use faststr::FastStr;
use indexmap::IndexMap;
use path_finder::PathFinder;
use quick_xml::Reader;
use quick_xml::events::Event;
use rayon::prelude::*;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    sync::Mutex,
};
use zip::ZipArchive;

// Main generator struct that coordinates all operations
#[derive(Debug)]
pub struct BilingualGenerator {
    pub game_path: PathBuf,
    pub working_dir: PathBuf,
    pub files_to_process: Vec<String>,
    pub language_to_process: Vec<String>,
    pub all_data: HashMap<XmlFile, HashMap<Language, IndexMap<EntryId, TextEntry>>>,
    /// Where non-fatal diagnostics go. `None` falls back to stderr, which only works when the
    /// process actually owns a console (debug builds and headless runs).
    log_sink: Option<LogSink>,
}
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Language(pub FastStr);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct XmlFile(pub FastStr);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct EntryId(pub FastStr);
// #[derive(Debug, Clone, Hash, PartialEq, Eq)]
// pub struct LastTextValue(pub FastStr);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TextEntry {
    pub source_text: FastStr,     // 对应第二个 <Cell>
    pub translated_text: FastStr, // 对应第三个 <Cell>
}
// #[derive(Debug)]
// ///HashMap<Entry id, Text in last cell>
// pub struct single_xml_data {
//     pub data: HashMap<String, String>,
// }
impl BilingualGenerator {
    //Constructor to initialize the generator with the working directory
    pub fn init() -> Result<Self, BilingualGeneratorError> {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        let files_to_process = vec![
            String::from("text_ui_dialog.xml"),    // Dialogues
            String::from("text_ui_quest.xml"),     // Quests
            String::from("text_ui_tutorials.xml"), // Tutorials
            String::from("text_ui_soul.xml"),      // Stats/Effects
            String::from("text_ui_items.xml"),     // Items
            String::from("text_ui_menus.xml"),     // Menus
            String::from("text_ui_misc.xml"),
        ];
        let defaut_language_to_process = vec![
            String::from("Chineses"),
            String::from("English"),
            String::from("Czech"),
            // String::from("German"),
            // String::from("French"),
            // String::from("Spanish"),
            // String::from("Japanese"),
        ];
        let mut path_finder = PathFinder::new();
        // let game_path = path_finder
        //     .find_game_path()
        //     .map(|p| p.to_path_buf())
        //     .unwrap_or_else(|_| PathBuf::new());
        let kcd_path = match path_finder.find_game_path() {
            Ok(path) => path.to_path_buf(),
            Err(_) => PathBuf::new(),
        };
        Ok(Self {
            game_path: kcd_path,
            working_dir,
            files_to_process,
            language_to_process: defaut_language_to_process,
            all_data: HashMap::new(),
            log_sink: None,
        })
    }

    /// Routes non-fatal diagnostics to `sink` instead of stderr.
    pub fn set_log_sink(&mut self, sink: LogSink) {
        self.log_sink = Some(sink);
    }

    /// Reports a recoverable problem. Never aborts the run: generation keeps going and the
    /// message surfaces in the GUI messages area (or stderr when no sink is installed).
    fn warn(&self, message: impl Into<String>) {
        let message = message.into();
        match &self.log_sink {
            Some(sink) => sink.log(message),
            None => eprintln!("{message}"),
        }
    }
    pub fn acquire_bilingual_set(&mut self) -> Result<Vec<(FastStr, FastStr)>, BilingualGeneratorError> {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        // println!("Working directory: {:?}", working_dir);
        let bilingual_set_dir = working_dir.join(r"assets\bilingual_set.txt");
        // println!("Bilingual set directory: {:?}", bilingual_set_dir);
        let bilingual_set_file = File::open(&bilingual_set_dir)
            .map_err(|_| BilingualGeneratorError::InvalidBilingualSet(format!("No bilingual_set.txt in {:?}", bilingual_set_dir)))?;
        let reader = BufReader::new(bilingual_set_file);
        let mut bilingual_set: Vec<(FastStr, FastStr)> = vec![];
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
            bilingual_set.push((primary_language.into(), secondary_language.into()));
        }
        Ok(bilingual_set)
    }

    /// Reads XML files from the pak files located in the Localization folder and stores the Entry id
    /// and secondary text (last cell) for each XML file into self.all_data.
    pub fn read_xml_from_paks(&mut self) -> Result<(), BilingualGeneratorError> {
        // Collect all_data in a thread-safe manner
        let all_data = Mutex::new(&mut self.all_data);

        self.language_to_process.par_iter().try_for_each(|language| {
            let pak_filename = format!("{}_xml.pak", language);
            let pak_path = self.game_path.join("Localization").join(&pak_filename);
            let pak_file = File::open(&pak_path).map_err(|e| BilingualGeneratorError::PakOperationFailed {
                operation: "opening PAK file".to_string(),
                context: pak_path.display().to_string(),
                source: e,
            })?;

            let mut archive = ZipArchive::new(pak_file).map_err(|e| BilingualGeneratorError::PakOperationFailed {
                operation: "creating ZipArchive from PAK".to_string(),
                context: pak_path.display().to_string(),
                source: e.into(),
            })?;

            self.files_to_process.iter().try_for_each(|xml_filename| {
                let mut xml_file = archive.by_name(xml_filename).map_err(|e| BilingualGeneratorError::PakOperationFailed {
                    operation: "finding XML file in PAK".to_string(),
                    context: format!("{} in {}", xml_filename, pak_filename),
                    source: e.into(),
                })?;

                let mut content = String::new();
                xml_file.read_to_string(&mut content).map_err(|e| BilingualGeneratorError::PakOperationFailed {
                    operation: "reading XML content from PAK".to_string(),
                    context: format!("{} in {}", xml_filename, pak_filename),
                    source: e,
                })?;
                let mut reader = Reader::from_str(&content);
                // reader.trim_text(true); // Reduces unnecessary allocations
                let mut buf = Vec::with_capacity(512); // Pre-allocate buffer
                let mut single_data = IndexMap::new();
                let mut current_cells: Vec<String> = Vec::with_capacity(4); // Pre-allocate
                let mut inside_row = false;
                let mut cell_index: usize = 0;

                loop {
                    match reader.read_event_into(&mut buf) {
                        Ok(Event::Start(e)) if e.name().as_ref() == b"Row" => {
                            inside_row = true;
                            current_cells.clear();
                            cell_index = 0; // Reset cell index for a new row
                        }
                        Ok(Event::End(e)) if e.name().as_ref() == b"Row" => {
                            inside_row = false;
                            if !current_cells.is_empty() && current_cells[0] != "Entry id" {
                                // Ensure we have at least 3 cells for EntryId and LastTextValue
                                if current_cells.len() >= 3 {
                                    let entry = TextEntry {
                                        // If the second cell is empty, store an empty string too
                                        source_text: current_cells.get(1).cloned().unwrap_or_default().into(),
                                        // the third cell
                                        translated_text: current_cells.get(2).cloned().unwrap_or_default().into(),
                                    };
                                    single_data.insert(EntryId(current_cells[0].clone().into()), entry);
                                }
                            }
                        }
                        // New: Handle <Cell> start tag
                        Ok(Event::Start(e)) if inside_row && e.name().as_ref() == b"Cell" => {
                            // We are now inside a cell. No action needed here, just a state change.
                            // The text event handler will know to capture text if inside_row is true.
                        }
                        // New: Handle <Cell> end tag
                        Ok(Event::End(e)) if inside_row && e.name().as_ref() == b"Cell" => {
                            cell_index += 1; // Increment cell index after a cell ends
                        }
                        Ok(Event::Text(e)) => {
                            // Only capture text if we are inside a row and this text is within a <Cell>
                            if inside_row {
                                // Read the raw text (including escaped entities) as a string
                                let raw_text = std::str::from_utf8(e.as_ref())
                                    .map_err(|_| BilingualGeneratorError::XmlProcessingFailed("Invalid UTF-8 in XML text".to_string()))?;

                                // Ensure current_cells has enough capacity for the current cell_index
                                // If a cell is empty (e.g., <Cell></Cell>), no text event will be emitted for it.
                                // We need to fill in empty strings for missing cells to maintain correct indexing.
                                while current_cells.len() <= cell_index {
                                    current_cells.push("".to_string()); // Fill with empty strings for skipped cells
                                }
                                current_cells[cell_index] = raw_text.to_string();
                            }
                        }
                        Ok(Event::Eof) => break,
                        Err(e) => return Err(BilingualGeneratorError::XmlProcessingFailed(format!("XML error: {}", e))),
                        _ => {}
                    }
                    buf.clear();
                }

                // Thread-safe insertion into all_data
                let mut guard = all_data.lock().unwrap();
                guard
                    .entry(XmlFile(xml_filename.clone().into()))
                    .or_insert_with(HashMap::new)
                    .insert(Language(language.clone().into()), single_data);
                Ok(())
            })
        })
    }

    pub fn process_single_bilingual(&self, primary_language: &str, secondary_language: &str) -> Result<PathBuf, BilingualGeneratorError> {
        // Create output directory for the bilingual XML files
        let xml_output_dir = self
            .working_dir
            .join(format!("bilingual_xml\\{} + {}\\Localization", primary_language, secondary_language));

        // A thread-safe vector to collect paths of generated XML files for later PAK creation and cleanup.
        let xml_output_set: Mutex<Vec<PathBuf>> = Mutex::new(vec![]);

        // Ensure the output directory exists.
        std::fs::create_dir_all(&xml_output_dir)
            .map_err(|e| BilingualGeneratorError::XmlProcessingFailed(format!("Error creating output directory: {}", e)))?;

        // Prepare language identifiers for efficient lookups.
        let primary_lang = Language(primary_language.to_string().into());
        let secondary_lang = Language(secondary_language.to_string().into());
        let english_lang = Language("English".into());

        // Process each XML file in parallel using Rayon.
        self.files_to_process.par_iter().for_each(|file_name| {
            let xml_file = XmlFile(file_name.clone().into());

            // Get all data for the current XML file. Handle missing file gracefully.
            let file_data = self
                .all_data
                .get(&xml_file)
                .ok_or_else(|| BilingualGeneratorError::XmlProcessingFailed(format!("Could not find the required XML file in `all_data`: {}", file_name)));

            if let Ok(file_data) = file_data {
                // Get entries for the primary language.
                let primary_entries = file_data.get(&primary_lang).ok_or_else(|| {
                    BilingualGeneratorError::XmlProcessingFailed(format!("Could not get primary_entries for primary_language [{:?}].", &primary_lang))
                });

                // Create empty IndexMaps as fallbacks if a language's data is not found.
                let empty_map: IndexMap<EntryId, TextEntry> = IndexMap::new();
                let secondary_entries = file_data.get(&secondary_lang).unwrap_or(&empty_map);
                let english_entries = file_data.get(&english_lang).unwrap_or(&empty_map);

                let mut rows = Vec::new(); // Collect all <Row> elements for this XML file.

                match primary_entries {
                    Ok(entries) => {
                        for (entry_id, primary_entry) in entries {
                            // Extract the primary language's translated text (from the 3rd cell of the primary language's XML).
                            let primary_text_str = primary_entry.translated_text.as_str();

                            // Get the secondary language's translated text. Default to "MISSING" if not found.
                            let mut secondary_text_str = secondary_entries.get(entry_id).map(|e| e.translated_text.as_str()).unwrap_or("MISSING");

                            // Get the English translated text. Default to "MISSING" if not found.
                            let english_text_str = english_entries.get(entry_id).map(|e| e.translated_text.as_str()).unwrap_or("MISSING");

                            // --- START OF NEW LOGIC: Reverse-lookup for missing secondary/English text in text_ui_dialog.xml ---
                            // This block addresses the specific scenario where an entry in the primary language
                            // (e.g., Chinese) exists, but its corresponding secondary language (e.g., English, as source text)
                            // and fallback English are both "MISSING".
                            if file_name == "text_ui_dialog.xml"
                                && secondary_text_str == "MISSING"
                                && english_text_str == "MISSING"
                                && !primary_text_str.is_empty()
                            // Ensure primary text is not empty before attempting lookup
                            {
                                // Iterate over all primary language entries (again) to find a potential match.
                                for (_other_id, other_entry) in entries.iter() {
                                    // If another entry has the same translated text as the current primary text
                                    // AND its `source_text` (from the 2nd cell, typically the original English) is not empty,
                                    // then we've found a suitable replacement.
                                    if other_entry.translated_text == primary_entry.translated_text && !other_entry.source_text.is_empty() {
                                        // Use this non-empty source text to fill the `secondary_text_str` gap.
                                        secondary_text_str = other_entry.source_text.as_str();
                                        break; // Found a suitable replacement, exit this inner search loop.
                                    }
                                }
                            }
                            // --- END OF NEW LOGIC ---

                            // Determine the combined text based on file name and specific rules.
                            let combined_text = match file_name.as_str() {
                                "text_ui_menus.xml" => {
                                    if !entry_id.0.contains("ui_helpoverlay") {
                                        match true {
                                            // Specific handling for loading screens or codex content.
                                            _ if entry_id.0.contains("ui_loading") || entry_id.0.contains("codex_cont") => {
                                                secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_NEWLINE)
                                            }
                                            // Handle very short primary texts or combined texts exceeding character limits.
                                            _ if primary_text_str.chars().count() <= 6
                                                || (primary_text_str.chars().count() + secondary_text_str.chars().count() > 920) =>
                                            {
                                                primary_text_str.to_string().into() // Only primary text
                                            }
                                            // If primary text is long, use a newline separator.
                                            _ if primary_text_str.chars().count() >= 20 => {
                                                secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_NEWLINE)
                                            }
                                            // Default for menus: use a slash separator.
                                            _ => secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_SLASH),
                                        }
                                    } else {
                                        primary_text_str.to_string().into() // For helpoverlay entries, only use primary text.
                                    }
                                }
                                "text_ui_dialog.xml" => {
                                    // Use the (potentially corrected by reverse-lookup) secondary text.
                                    // If still "MISSING", fall back to the English text.
                                    let fallback_text = if secondary_text_str != "MISSING" {
                                        secondary_text_str
                                    } else if english_text_str != "MISSING" {
                                        english_text_str
                                    } else {
                                        #[cfg(debug_assertions)]
                                        {
                                            "MISSING"
                                        }
                                        #[cfg(not(debug_assertions))]
                                        {
                                            ""
                                        }
                                    };
                                    format!("{}{}{}", primary_text_str, SEPARATOR_NEWLINE, fallback_text).into()
                                }
                                "text_ui_items.xml" => {
                                    match true {
                                        // Specific conditions for item step descriptions or certain combat modifiers.
                                        _ if (entry_id.0.contains("step")
                                            && !entry_id.0.contains("_step_1")
                                            && primary_text_str.chars().count() >= 10)
                                            || (entry_id.0.contains("step_1")
                                                && (entry_id.0.contains("scatter") || entry_id.0.contains("longWeak") || entry_id.0.contains("bane"))) =>
                                        {
                                            FastStr::from(primary_text_str.to_string()) // Only primary text for these specific items.
                                        }
                                        // If primary text is moderately long, use a newline separator.
                                        _ if primary_text_str.chars().count() >= 7 => {
                                            secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_NEWLINE)
                                        }
                                        // Default for items: use a slash separator.
                                        _ => secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_SLASH),
                                    }
                                }
                                "text_ui_soul.xml" => match true {
                                    // Short texts or specific stat entries use a slash separator.
                                    _ if primary_text_str.chars().count() <= 7 && primary_text_str != "MISSING"
                                        || primary_text_str.chars().count() <= 12 && entry_id.0.contains("stat_") =>
                                    {
                                        secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_SLASH)
                                    }
                                    // Longer texts, provided they don't exceed a combined character limit, use a newline.
                                    _ if primary_text_str.chars().count() + secondary_text_str.chars().count() < 932 => {
                                        secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_NEWLINE)
                                    }
                                    // Very long texts, just use the primary text.
                                    _ => primary_text_str.to_string().into(),
                                },
                                _ => {
                                    // Default case for any other XML files: combine with slash.
                                    // If secondary text is missing, fall back to English text.
                                    let fallback_text = if secondary_text_str != "MISSING" {
                                        secondary_text_str
                                    } else if english_text_str != "MISSING" {
                                        english_text_str
                                    } else {
                                        #[cfg(debug_assertions)]
                                        {
                                            "MISSING"
                                        }
                                        #[cfg(not(debug_assertions))]
                                        {
                                            ""
                                        }
                                    };
                                    format!("{}{}{}", primary_text_str, SEPARATOR_SLASH, fallback_text).into()
                                }
                            };

                            // Construct the XML Row.
                            // The second <Cell> will contain the original source text (e.g., English from the 2nd cell of the primary language file).
                            // The third <Cell> contains the combined bilingual text.
                            rows.push(format!(
                                "<Row><Cell>{}</Cell><Cell>{}</Cell><Cell>{}</Cell></Row>",
                                entry_id.0, primary_entry.source_text, combined_text
                            ));
                        }
                    }
                    Err(e) => {
                        // Report, then fall back to English so this language pair still gets rows.
                        self.warn(format!(
                            "{file_name}: could not load `{primary_language}` entries ({e}); falling back to English text"
                        ));
                        // As a fallback, use English entries if primary entries are unavailable.
                        for (entry_id, english_entry) in english_entries.iter() {
                            rows.push(format!(
                                "<Row><Cell>{}</Cell><Cell>{}</Cell><Cell>{}</Cell></Row>",
                                entry_id.0, english_entry.source_text, english_entry.translated_text
                            ));
                        }
                    }
                }

                // Assemble the full XML content and determine the output path.
                let xml_content = format!("<Table>\n{}\n</Table>", rows.join("\n"));
                let xml_output_path = xml_output_dir.join(file_name);

                // Write first, then register the path: a failed write must not leave a path in the
                // pak list, otherwise PAK creation would trip over the missing file later.
                match std::fs::write(&xml_output_path, xml_content) {
                    Ok(()) => {
                        let mut xml_output_set_guard = xml_output_set.lock().unwrap();
                        xml_output_set_guard.push(xml_output_path);
                    }
                    Err(e) => {
                        self.warn(format!(
                            "{file_name}: failed to write generated XML to {} ({e}); this file is left out of the pak",
                            xml_output_path.display()
                        ));
                    }
                }
            } else {
                self.warn(format!("{file_name}: skipped, the XML file is missing from the loaded paks"));
            }
        });

        // After all parallel processing is complete, collect the generated XML file paths.
        let generated_xml_paths: Vec<PathBuf> = xml_output_set.into_inner().unwrap();

        // Attempt to create a new PAK file from the generated XML files.
        // Propagate the underlying error verbatim instead of collapsing it to PakCreationFailed:
        // the GUI has no console, so this message is the only diagnostic the user will ever see.
        create_new_pak(generated_xml_paths.clone(), &xml_output_dir, primary_language)?;

        // PAK creation succeeded, so the temporary XML files are no longer needed.
        generated_xml_paths.par_iter().for_each(|path| {
            if let Err(e) = std::fs::remove_file(path) {
                self.warn(format!("could not delete temporary XML file {} ({e})", path.display()));
            }
        });

        Ok(xml_output_dir)
    }
}
