#[cfg(test)]
mod tests {
    use generator_core::bilingual_generator::{BilingualGenerator, EntryId, Language, XmlFile};
    use path_finder::PathFinder;
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
                let dialogxml = XmlFile("text_ui_dialog.xml".into());
                let extracted_data = generator.all_data.get(&dialogxml).unwrap();
                let xml_data = extracted_data.get(&Language("English".into())).unwrap();
                let entry_id = EntryId("zlod_henry_zavri_klapacku_2dQP".into());
                let content = xml_data.get(&entry_id).expect(&format!("在 xml_data 中未找到 ID 为 '{:?}' 的条目。", entry_id));
                println!("content it get = {}", content.0);

                // let entry_id_missing = EntryId("ui_hrac_si_p_seq35_49Z0".into());
                // let dialogset_missing = generator.eng_backup_textvalue.get(&dialogxml).unwrap();
                // let content_missing = dialogset_missing
                //     .get(&entry_id_missing)
                //     .expect(&format!("在 dialogset_missing 中未找到 ID 为 '{:?}' 的条目。", entry_id_missing));
                // println!("content_missing it get = {:?}", content_missing);

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
