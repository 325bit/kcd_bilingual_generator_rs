#[cfg(test)]
mod tests {
    use kcd_bilingual_generator_rust::core::{
        bilingual_generator::BilingualGenerator,
        bilingual_generator_errors::BilingualGeneratorError, path_finder::path_finder::PathFinder,
    };
    use std::path::PathBuf;

    #[test]
    fn create_pak_test() -> Result<(), BilingualGeneratorError> {
        let mut path_finder = PathFinder::new();
        let game_path = path_finder.find_game_path().unwrap();

        let mut generator = BilingualGenerator::init()?;
        generator.game_path = game_path.to_path_buf();

        let mut xml_output_set: Vec<PathBuf> = vec![];
        let xml_output_dir = generator.working_dir.join("bilingual_xml");
        for file_name in generator.files_to_process {
            let xml_output_path = xml_output_dir.join(file_name);
            xml_output_set.push(xml_output_path.clone());
        }
        let result = BilingualGenerator::create_new_pak(
            xml_output_set,
            &generator.working_dir.join("bilingual_xml"),
            "Chineses",
        )
        .map_err(|_| BilingualGeneratorError::PakCreationFailed);
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}
