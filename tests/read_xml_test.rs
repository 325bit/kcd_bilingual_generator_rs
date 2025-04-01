#[cfg(test)]
mod tests {
    use kcd_bilingual_generator_rust::core::bilingual_generator::BilingualGenerator;
    use kcd_bilingual_generator_rust::core::path_finder::PathFinder;

    #[test]
    fn test_read_xml_from_paks() -> Result<(), Box<dyn std::error::Error>> {
        // Use the path from PathFinder to locate the actual game path
        let mut path_finder = PathFinder::new();
        let game_path = path_finder.find_game_path().unwrap();

        let mut generator = BilingualGenerator::init()?;
        generator.game_path = game_path.to_path_buf();
        // Call the read_xml_from_paks function to parse XML files
        match generator.read_xml_from_paks() {
            Ok(_) => {
                let extracted_data = generator.all_data.get("Chineses").unwrap();
                let xml_data = extracted_data.get("text_ui_soul.xml").unwrap();
                let entry_id = "buff_alcoholism_level3_desc".to_string();
                let content = xml_data.data.get(&entry_id).unwrap();
                println!("content it get = {}", content);
                Ok(())
            }
            Err(_) => return Err("Failed to read XML files".into()),
        }
    }
}
