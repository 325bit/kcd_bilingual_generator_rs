#[cfg(test)]
mod tests {

    use path_finder::PathFinder;

    #[test]
    fn path_finder_test() {
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
