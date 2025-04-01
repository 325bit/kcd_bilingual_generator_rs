use super::bilingual_generator_errors::BilingualGeneratorError;
use super::path_finder::PathFinder;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

// Main generator struct that coordinates all operations
pub struct BilingualGenerator {
    pub game_path: PathBuf,
    pub working_dir: PathBuf,
    pub files_to_process: Vec<String>,
    pub language_to_process: Vec<String>,
    ///HashMap<Language, HashMap<Which_xml, single_xml_data>>
    pub all_data: HashMap<String, HashMap<String, single_xml_data>>,
}
#[derive(Debug)]
///HashMap<Entry id, Text in last cell>
pub struct single_xml_data {
    pub data: HashMap<String, String>,
}
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
            println!("pak_filename = {}, pak_path = {:?}", pak_filename, pak_path);
            let pak_file =
                File::open(&pak_path).map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

            let mut archive = ZipArchive::new(pak_file)
                .map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

            let lang_map = self
                .all_data
                .entry(language.clone())
                .or_insert_with(HashMap::new);

            for xml_filename in &self.files_to_process {
                let mut xml_file = archive
                    .by_name(xml_filename)
                    .map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

                let mut content = String::new();
                xml_file
                    .read_to_string(&mut content)
                    .map_err(|_| BilingualGeneratorError::PakExtractionFailed)?;

                let mut reader = Reader::from_str(&content);
                // reader.trim_text(true);
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
                                    single_data
                                        .insert(current_cells[0].clone(), current_cells[2].clone());
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
                        Err(_) => return Err(BilingualGeneratorError::XmlProcessingFailed),
                        _ => {}
                    }
                    buf.clear();
                }
                lang_map.insert(xml_filename.clone(), single_xml_data { data: single_data });
            }
        }
        Ok(())
    }

    /// Process each XML file:
    /// 1. Parse original XML
    /// 2. Create bilingual version
    /// 3. Save modified XML to output directory
    /// Return paths to modified files
    fn process_files(
        files: Vec<PathBuf>,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, BilingualGeneratorError> {
        return Err(BilingualGeneratorError::XmlProcessingFailed);
    }

    fn process_single_file(
        input_path: &Path,
        output_dir: &Path,
    ) -> Result<PathBuf, BilingualGeneratorError> {
        return Err(BilingualGeneratorError::XmlProcessingFailed);
    }

    /// Implement PAK creation logic
    /// Create new PAK with modified files
    fn create_new_pak(
        files: Vec<PathBuf>,
        output_dir: &Path,
    ) -> Result<(), BilingualGeneratorError> {
        return Err(BilingualGeneratorError::PakCreationFailed);
    }
}
