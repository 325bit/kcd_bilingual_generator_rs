#[cfg(test)]
mod tests {
    use kcd_bilingual_generator_rust::core::{
        bilingual_generator::{BilingualGenerator, EntryId, Language, XmlFile},
        path_finder::PathFinder,
    };

    #[test]
    fn read_xml_test() -> Result<(), Box<dyn std::error::Error>> {
        // Use the path from PathFinder to locate the actual game path
        let mut path_finder = PathFinder::new();
        let game_path = path_finder.find_game_path().unwrap();

        let mut generator = BilingualGenerator::init()?;
        generator.game_path = game_path.to_path_buf();
        // Call the read_xml_from_paks function to parse XML files
        match generator.read_xml_from_paks() {
            Ok(_) => {
                let extracted_data = generator.all_data.get(&XmlFile("text_ui_soul.xml".to_string())).unwrap();
                let xml_data = extracted_data.get(&Language("Chineses".to_string())).unwrap();
                let entry_id = EntryId("buff_alcoholism_level3_desc".to_string());
                let content = xml_data.get(&entry_id).unwrap();
                println!("content it get = {}", content.0);
                // let text = "酗酒";

                // // Get the length in bytes
                // println!("Byte length (len()) = {}", text.len()); // Output: 6

                // // Get the number of characters (Unicode Scalar Values)
                // println!("Character count (chars().count()) = {}", text.chars().count()); // Output: 2
                Ok(())
            }
            Err(_) => return Err("Failed to read XML files".into()),
        }
    }
}
