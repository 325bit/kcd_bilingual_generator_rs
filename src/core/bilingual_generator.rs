use super::bilingual_generator_errors::BilingualGeneratorError;
use super::path_finder::PathFinder;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zip::{
    write::{ExtendedFileOptions, FileOptions},
    CompressionMethod, ZipArchive, ZipWriter,
};

// Main generator struct that coordinates all operations
#[derive(Debug)]
pub struct BilingualGenerator {
    pub game_path: PathBuf,
    pub working_dir: PathBuf,
    pub files_to_process: Vec<String>,
    pub language_to_process: Vec<String>,
    pub all_data: HashMap<XmlFile, HashMap<Language, HashMap<EntryId, LastTextValue>>>,
}
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Language(pub String);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct XmlFile(pub String);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct EntryId(pub String);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct LastTextValue(pub String);

// #[derive(Debug)]
// ///HashMap<Entry id, Text in last cell>
// pub struct single_xml_data {
//     pub data: HashMap<String, String>,
// }
impl BilingualGenerator {
    //Constructor to initialize the generator with the working directory
    pub fn init() -> Result<Self, BilingualGeneratorError> {
        let working_dir = std::env::current_dir()?;
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
            String::from("German"),
            String::from("French"),
            String::from("Spanish"),
            String::from("Japanese"),
        ];
        Ok(Self {
            game_path: PathBuf::new(),
            working_dir,
            files_to_process,
            language_to_process: defaut_language_to_process,
            all_data: HashMap::new(),
        })
    }

    // Main workflow method
    pub fn generate_bilingual_resources(&mut self) -> Result<(), BilingualGeneratorError> {
        // 1. Locate game installation
        self.locate_game()?;

        // 2. Find and extract XML PAKs
        // let extracted_files =

        // 3. Process XML files
        // let modified_files = self.process_xml_files(extracted_files)?;

        // 4. Create new PAK file
        // self.create_new_pak(modified_files)?;

        Ok(())
    }
    fn locate_game(&mut self) -> Result<(), BilingualGeneratorError> {
        let mut path_finder = PathFinder::new();
        self.game_path = path_finder.find_game_path()?.to_path_buf();
        Ok(())
    }

    /// Reads XML files from the pak files located in the Localization folder and stores the Entry id
    /// and secondary text (last cell) for each XML file into self.all_data.
    pub fn read_xml_from_paks(&mut self) -> Result<(), BilingualGeneratorError> {
        for language in &self.language_to_process {
            let pak_filename = format!("{}_xml.pak", language);
            let pak_path = self.game_path.join("Localization").join(&pak_filename);
            let pak_file =
                File::open(&pak_path).map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

            let mut archive = ZipArchive::new(pak_file)
                .map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

            for xml_filename in &self.files_to_process {
                let mut xml_file = archive
                    .by_name(xml_filename)
                    .map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

                let mut content = String::new();
                xml_file
                    .read_to_string(&mut content)
                    .map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

                let mut reader = Reader::from_str(&content);
                let mut buf = Vec::new();
                let mut single_data = HashMap::new();
                let mut current_cells: Vec<String> = Vec::new();
                let mut inside_row = false;

                loop {
                    match reader.read_event_into(&mut buf) {
                        Ok(Event::Start(ref e)) if e.name().as_ref() == b"Row" => {
                            inside_row = true;
                            current_cells.clear();
                        }
                        Ok(Event::End(ref e)) if e.name().as_ref() == b"Row" => {
                            inside_row = false;
                            if !current_cells.is_empty() && current_cells[0] != "Entry id" {
                                if current_cells.len() >= 3 {
                                    single_data.insert(
                                        EntryId(current_cells[0].clone()),
                                        LastTextValue(current_cells[2].clone()),
                                    );
                                }
                            }
                        }
                        Ok(Event::Text(e)) => {
                            if inside_row {
                                if let Ok(text) = e.unescape().map(|s| s.into_owned()) {
                                    current_cells.push(text);
                                }
                            }
                        }
                        Ok(Event::Eof) => break,
                        Err(_) => {
                            return Err(BilingualGeneratorError::XmlProcessingFailed(
                                "Could not parse XML structure.".to_string(),
                            ))
                        }
                        _ => {}
                    }
                    buf.clear();
                }

                // Insert data into the restructured all_data
                let xml_file_entry = self
                    .all_data
                    .entry(XmlFile(xml_filename.clone()))
                    .or_insert_with(HashMap::new);
                xml_file_entry.insert(Language(language.clone()), single_data);
            }
        }
        Ok(())
    }

    /// Process each XML file:
    /// 1. Parse original XML
    /// 2. Create bilingual version
    /// 3. Save modified XML to output directory
    /// Return paths to modified files
    // fn process_files(
    //     files: Vec<PathBuf>,
    //     output_dir: &Path,
    // ) -> Result<Vec<PathBuf>, BilingualGeneratorError> {
    //     return Err(BilingualGeneratorError::XmlProcessingFailed);
    // }

    pub fn process_single_bilingual(
        &self,
        primary_language: &str,
        secondary_language: &str,
    ) -> Result<PathBuf, BilingualGeneratorError> {
        // Create output directory
        let xml_output_dir = self.working_dir.join("bilingual_xml");
        let mut xml_output_set: Vec<PathBuf> = vec![];
        std::fs::create_dir_all(&xml_output_dir).map_err(|e| {
            BilingualGeneratorError::XmlProcessingFailed(format!("Error processing XML: {}", e))
        })?;

        // Prepare language identifiers
        let primary_lang = Language(primary_language.to_string());
        let secondary_lang = Language(secondary_language.to_string());

        // Process each XML file
        for file_name in &self.files_to_process {
            let xml_file = XmlFile(file_name.clone());
            // println!("file name = {}", file_name);
            // Get data for this specific XML file
            let file_data = self.all_data.get(&xml_file).ok_or(
                BilingualGeneratorError::XmlProcessingFailed(format!(
                    "Could not find the required XML file: {}",
                    file_name
                )),
            )?;

            // Get entries for both languages
            let primary_entries = file_data.get(&primary_lang).ok_or(
                BilingualGeneratorError::XmlProcessingFailed(
                    "Could not Get primary_entries for primary_language.".to_string(),
                ),
            )?;
            let empty_map: HashMap<EntryId, LastTextValue> = HashMap::new();
            let secondary_entries = file_data.get(&secondary_lang).unwrap_or(&empty_map);

            // Build XML content
            let mut rows = Vec::new();
            for (entry_id, primary_text) in primary_entries {
                let secondary_text = secondary_entries
                    .get(entry_id)
                    .map(|lv| lv.0.as_str())
                    .unwrap_or("");

                rows.push(format!(
                    "<Row><Cell>{}</Cell><Cell>{}</Cell><Cell>{} / {}</Cell></Row>",
                    entry_id.0, primary_text.0, primary_text.0, secondary_text
                ));
            }

            // Write to file
            let xml_content = format!("<Table>\n{}\n</Table>", rows.join("\n"));
            let xml_output_path = xml_output_dir.join(file_name);
            xml_output_set.push(xml_output_path.clone());
            std::fs::write(&xml_output_path, xml_content).map_err(|e| {
                BilingualGeneratorError::XmlProcessingFailed(format!("Error processing XML: {}", e))
            })?;
        }

        match create_new_pak(xml_output_set, &xml_output_dir, primary_language) {
            Ok(_) => Ok(xml_output_dir),
            Err(_) => Err(BilingualGeneratorError::PakCreationFailed),
        }
    }
}
fn create_new_pak(
    files: Vec<PathBuf>,
    output_dir: &Path,
    primary_language: &str,
) -> Result<(), BilingualGeneratorError> {
    let pak_name = format!("{}_xml.pak", primary_language);
    let pak_path = output_dir.join(pak_name);

    let file = File::create(&pak_path).map_err(|_| BilingualGeneratorError::PakCreationFailed)?;
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<'_, ExtendedFileOptions> =
        FileOptions::default().compression_method(CompressionMethod::Stored);

    for path in files {
        let file_name = path
            .file_name()
            .ok_or(BilingualGeneratorError::PakCreationFailed)?;
        let file_name_str = file_name
            .to_str()
            .ok_or(BilingualGeneratorError::PakCreationFailed)?;

        zip.start_file(file_name_str, options.clone())
            .map_err(|_| BilingualGeneratorError::PakCreationFailed)?;
        let content =
            std::fs::read(&path).map_err(|_| BilingualGeneratorError::PakCreationFailed)?;
        zip.write_all(&content)
            .map_err(|_| BilingualGeneratorError::PakCreationFailed)?;
    }

    zip.finish()
        .map_err(|_| BilingualGeneratorError::PakCreationFailed)?;

    Ok(())
}
