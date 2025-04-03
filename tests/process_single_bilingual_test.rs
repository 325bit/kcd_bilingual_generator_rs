#[cfg(test)]
mod tests {
    use kcd_bilingual_generator_rust::core::{
        bilingual_generator::{BilingualGenerator /*, EntryId, Language, XmlFile */},
        bilingual_generator_errors::BilingualGeneratorError,
        path_finder::PathFinder,
    };

    #[test]
    fn process_single_bilingual_test() -> Result<(), BilingualGeneratorError> {
        // Use the path from PathFinder to locate the actual game path
        let mut path_finder = PathFinder::new();
        let game_path = path_finder.find_game_path().unwrap();

        let mut generator = BilingualGenerator::init()?;
        generator.game_path = game_path.to_path_buf();
        println!("Generator's Workspace is :{:?}", generator.working_dir);
        generator.read_xml_from_paks().unwrap_or_else(|e| {
            panic!("Problem reading xml from paks, error:{e}");
        });
        // let extracted_data = generator
        //     .all_data
        //     .get(&XmlFile("text_ui_dialog.xml".to_string()))
        //     .unwrap_or_else(|| {
        //         panic!("Problem reading the file: text_ui_dialog.xml");
        //     });
        // let xml_data = extracted_data
        //     .get(&Language("Chineses".to_string()))
        //     .unwrap();
        // let entry_id = EntryId("achy_alchymist_mel_vrazdu_LaNq".to_string());
        // let content = xml_data.get(&entry_id).unwrap();
        // println!("content it get = {}", content.0);
        // Call the read_xml_from_paks function to parse XML files
        match generator.process_single_bilingual("Chineses", "English") {
            Ok(_) => Ok(()),
            Err(e) => return Err(e),
        }
        // Ok(())
    }
}
