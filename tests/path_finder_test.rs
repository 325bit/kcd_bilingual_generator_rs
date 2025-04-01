use kcd_bilingual_generator_rust::core::path_finder::path_finder::PathFinder;

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_find_game_path_steam() {
        let mut path_finder = PathFinder::new();

        // Mock or provide a real Steam installation path for testing
        // You can mock or set the registry/file paths for this test
        let result = path_finder.find_game_path();
        println!("Result: {:?}", result);

        // Optionally, assert that the path is a valid PathBuf
        // let game_path = result.unwrap();
        // assert!(game_path.exists(), "Game path should exist");
    }

    
}
