use crate::util::{SEPARATOR_NEWLINE, SEPARATOR_SLASH, create_new_pak, secondary_text_combined};

use super::bilingual_generator_errors::BilingualGeneratorError;
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
    pub all_data: HashMap<XmlFile, HashMap<Language, IndexMap<EntryId, LastTextValue>>>,
}
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Language(pub FastStr);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct XmlFile(pub FastStr);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct EntryId(pub FastStr);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct LastTextValue(pub FastStr);

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
        ];
        let defaut_language_to_process = vec![
            String::from("Chineses"),
            String::from("English"),
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
        })
    }
    pub fn acquire_bilingual_set(&mut self) -> Result<Vec<(FastStr, FastStr)>, BilingualGeneratorError> {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        let bilingual_set_dir = working_dir.join("..\\..\\assets\\bilingual_set.txt");
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
            let pak_file = File::open(&pak_path).map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

            let mut archive = ZipArchive::new(pak_file).map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

            self.files_to_process.iter().try_for_each(|xml_filename| {
                let mut xml_file = archive.by_name(xml_filename).map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

                let mut content = String::new();
                xml_file
                    .read_to_string(&mut content)
                    .map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

                let mut reader = Reader::from_str(&content);
                // reader.trim_text(true); // Reduces unnecessary allocations
                let mut buf = Vec::with_capacity(512); // Pre-allocate buffer
                let mut single_data = IndexMap::new();
                let mut current_cells: Vec<String> = Vec::with_capacity(4); // Pre-allocate
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
                                    single_data.insert(EntryId(current_cells[0].clone().into()), LastTextValue(current_cells[2].clone().into()));
                                }
                            }
                        }
                        Ok(Event::Text(e)) => {
                            if inside_row {
                                // Read the raw text (including escaped entities) as a string
                                let raw_text = std::str::from_utf8(e.as_ref())
                                    .map_err(|_| BilingualGeneratorError::XmlProcessingFailed("Invalid UTF-8 in XML text".to_string()))?;
                                current_cells.push(raw_text.to_string());
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
        // Create output directory
        let xml_output_dir = self
            .working_dir
            .join(format!("bilingual_xml\\{} + {}\\Localization", primary_language, secondary_language));
        let xml_output_set: Mutex<Vec<PathBuf>> = Mutex::new(vec![]); // Use Mutex for thread safety

        std::fs::create_dir_all(&xml_output_dir).map_err(|e| BilingualGeneratorError::XmlProcessingFailed(format!("Error processing XML: {}", e)))?;

        // Prepare language identifiers
        let primary_lang = Language(primary_language.to_string().into());
        let secondary_lang = Language(secondary_language.to_string().into());

        // Define separators
        // let SEPARATOR_SLASH = "/";
        // let SEPARATOR_NEWLINE = "\\n";
        let menutext_too_long = ["ui_state_health_desc", "ui_state_hunger_desc", "ui_DerivStat_MaxStamina_desc"];
        // Process each XML file in parallel
        self.files_to_process.par_iter().for_each(|file_name| {
            let xml_file = XmlFile(file_name.clone().into());
            let file_data = self.all_data.get(&xml_file).ok_or(BilingualGeneratorError::XmlProcessingFailed(format!(
                "Could not find the required XML file: {}",
                file_name
            )));

            if let Ok(file_data) = file_data {
                // Get entries for both languages
                let primary_entries = file_data.get(&primary_lang).ok_or(BilingualGeneratorError::XmlProcessingFailed(format!(
                    "Could not Get primary_entries for primary_language [{:?}].",
                    &primary_lang
                )));
                let empty_map: IndexMap<EntryId, LastTextValue> = IndexMap::new();
                let secondary_entries = file_data.get(&secondary_lang).unwrap_or(&empty_map);
                let empty_map_cloned = empty_map.clone();
                let english_entries = file_data.get(&Language("English".into())).unwrap_or(&empty_map_cloned);

                // Build XML content
                let mut rows = Vec::new();
                match primary_entries {
                    Ok(entries) => {
                        for (entry_id, primary_text) in entries {
                            let secondary_text = secondary_entries.get(entry_id).map(|lv| lv.0.as_str()).unwrap_or("MISSING");
                            let english_text = english_entries.get(entry_id).map(|lv| lv.0.as_str()).unwrap_or("MISSING");

                            let combined_text = match file_name.as_str() {
                                "text_ui_menus.xml" => {
                                    if !entry_id.0.contains("ui_helpoverlay") {
                                        match true {
                                            _ if entry_id.0.contains("ui_loading") || entry_id.0.contains("codex_cont") => {
                                                secondary_text_combined(primary_text, secondary_text, SEPARATOR_NEWLINE)
                                            }
                                            _ if primary_text.0.chars().count() <= 4 || menutext_too_long.contains(&&*entry_id.0) => {
                                                primary_text.0.clone().into()
                                            }
                                            _ if primary_text.0.chars().count() >= 20 => {
                                                secondary_text_combined(primary_text, secondary_text, SEPARATOR_NEWLINE)
                                            }

                                            _ => secondary_text_combined(primary_text, secondary_text, SEPARATOR_SLASH),
                                        }
                                    } else {
                                        primary_text.0.clone().into()
                                    }
                                }
                                "text_ui_dialog.xml" => {
                                    if secondary_text != "MISSING" {
                                        format!("{}{}{}", primary_text.0, SEPARATOR_NEWLINE, secondary_text).into()
                                    } else {
                                        format!("{}{}{}", primary_text.0, SEPARATOR_NEWLINE, english_text).into()
                                    }
                                }
                                "text_ui_items.xml" => {
                                    match true {
                                        _ if (entry_id.0.contains("step") && !entry_id.0.contains("_step_1") && primary_text.0.chars().count() >= 10)
                                            || (entry_id.0.contains("step_1")
                                                && (entry_id.0.contains("scatter") || entry_id.0.contains("longWeak") || entry_id.0.contains("bane"))) =>
                                        {
                                            primary_text.0.clone()
                                        }
                                        _ if primary_text.0.chars().count() >= 7 => {
                                            secondary_text_combined(primary_text, secondary_text, SEPARATOR_NEWLINE)
                                        }
                                        // else
                                        _ => secondary_text_combined(primary_text, secondary_text, SEPARATOR_SLASH),
                                    }
                                }
                                "text_ui_soul.xml" => match true {
                                    _ if primary_text.0.chars().count() <= 7 && primary_text.0 != "MISSING"
                                        || primary_text.0.chars().count() <= 12 && entry_id.0.contains("stat_") =>
                                    {
                                        secondary_text_combined(primary_text, secondary_text, SEPARATOR_SLASH)
                                    }
                                    _ if (entry_id.0.contains("buff") && entry_id.0.contains("desc") && !entry_id.0.contains("drunkenness_desc"))
                                        || (entry_id.0.contains("perk") && entry_id.0.contains("_desc")) =>
                                    {
                                        secondary_text_combined(primary_text, secondary_text, SEPARATOR_NEWLINE)
                                    }
                                    _ => primary_text.0.clone().into(),
                                },
                                _ => {
                                    // Default case for other files
                                    if secondary_text != "MISSING" {
                                        format!("{}{}{}", primary_text.0, SEPARATOR_SLASH, secondary_text).into()
                                    } else {
                                        format!("{}{}{}", primary_text.0, SEPARATOR_SLASH, english_text).into()
                                    }
                                }
                            };

                            rows.push(format!(
                                "<Row><Cell>{}</Cell><Cell>{}</Cell><Cell>{}</Cell></Row>",
                                entry_id.0, primary_text.0, combined_text
                            ));
                        }
                    }
                    Err(e) => {
                        // Log the error or store it for further handling
                        eprintln!("Error: {:?}", e);
                        // Continue with an empty map for primary_entries
                        for (entry_id, _) in empty_map {
                            let english_text = english_entries.get(&entry_id).map(|lv| lv.0.as_str()).unwrap_or("MISSING");

                            rows.push(format!(
                                "<Row><Cell>{}</Cell><Cell>{}</Cell><Cell>{}</Cell></Row>",
                                entry_id.0, english_text, english_text
                            ));
                        }
                    }
                }
                // Write to file
                let xml_content = format!("<Table>\n{}\n</Table>", rows.join("\n"));
                let xml_output_path = xml_output_dir.join(file_name);

                // Push the path to the output set
                let mut xml_output_set = xml_output_set.lock().unwrap();
                xml_output_set.push(xml_output_path.clone());

                std::fs::write(&xml_output_path, xml_content)
                    .map_err(|e| BilingualGeneratorError::XmlProcessingFailed(format!("Error processing XML: {}", e)))
                    .unwrap();
            }
        });

        // Get locked access to the path list
        let generated_xml_paths: Vec<PathBuf> = xml_output_set.into_inner().unwrap();
        // After parallel processing, merge results and proceed
        match create_new_pak(generated_xml_paths.clone(), &xml_output_dir, primary_language) {
            Ok(_) => {
                // Parallel file deletion
                generated_xml_paths.par_iter().for_each(|path| {
                    if let Err(e) = std::fs::remove_file(path) {
                        eprintln!("[Cleanup] Failed to delete {}: {}", path.display(), e);
                    }
                });

                Ok(xml_output_dir)
            }
            Err(_) => Err(BilingualGeneratorError::PakCreationFailed),
        }
    }
}
