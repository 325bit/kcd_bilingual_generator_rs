//! Covers the single runtime config file: creation, loading, normalisation, and recovery from a
//! hand-edited file that no longer parses.

use generator_core::{
    bilingual_generator::BilingualGenerator,
    config::{AppConfig, CONFIG_FILE_NAME, LanguagePair, default_bilingual_set},
};
use std::{fs, path::PathBuf};

/// Fresh, empty directory per test so runs never see each other's file.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kcd_config_test_{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn read_config_file(dir: &std::path::Path) -> AppConfig {
    let text = fs::read_to_string(dir.join(CONFIG_FILE_NAME)).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn creates_the_file_with_the_default_pairs() {
    let dir = temp_dir("create");
    let loaded = AppConfig::load_or_create(&dir);

    assert_eq!(loaded.path, dir.join(CONFIG_FILE_NAME));
    assert!(loaded.path.exists(), "the config file must be written on first use");
    assert!(loaded.notes.iter().any(|note| note.contains("Created")), "notes: {:?}", loaded.notes);

    // What landed on disk is exactly what the window is now using.
    assert_eq!(read_config_file(&dir), loaded.config);

    let expected: Vec<LanguagePair> = vec![
        LanguagePair::new("Chineses", "English"),
        LanguagePair::new("Chineses", "French"),
        LanguagePair::new("Chineses", "German"),
        LanguagePair::new("Chineses", "Spanish"),
        LanguagePair::new("Chineses", "Czech"),
        LanguagePair::new("Chineses", "Japanese"),
        LanguagePair::new("English", "Chineses"),
    ];
    assert_eq!(loaded.config.bilingual_set, expected);
}

#[test]
fn loads_an_existing_file_without_overwriting_it() {
    let dir = temp_dir("existing");
    // A hand-written config: one real pair, one pair with padding, one duplicate, one incomplete.
    let hand_written = r#"{
  "game_path": "\"C:\\Games\\KCD2\\\"",
  "bilingual_set": [
    { "primary": "Chineses", "secondary": " Czech " },
    { "primary": "Chineses", "secondary": "Czech" },
    { "primary": "English", "secondary": "" }
  ]
}"#;
    fs::write(dir.join(CONFIG_FILE_NAME), hand_written).unwrap();

    let loaded = AppConfig::load_or_create(&dir);

    // Quotes and the trailing separator are stripped; "E:" stays a root, not a relative path.
    assert_eq!(loaded.config.game_path, r"C:\Games\KCD2");
    // Padding trimmed, duplicate and the incomplete pair dropped, nothing else invented.
    assert_eq!(loaded.config.bilingual_set, vec![LanguagePair::new("Chineses", "Czech")]);

    // The cleaned-up version was written back, since the file had something to fix.
    assert_eq!(read_config_file(&dir), loaded.config);
}

#[test]
fn missing_bilingual_set_falls_back_to_defaults() {
    let dir = temp_dir("partial");
    // Only the game path is present: a hand-trimmed file must still produce the default set.
    fs::write(dir.join(CONFIG_FILE_NAME), r#"{ "game_path": "D:\\Games\\KCD2" }"#).unwrap();

    let loaded = AppConfig::load_or_create(&dir);

    assert_eq!(loaded.config.bilingual_set, default_bilingual_set());
    assert_eq!(loaded.config.game_path, r"D:\Games\KCD2");
}

#[test]
fn invalid_json_is_backed_up_and_replaced() {
    let dir = temp_dir("broken");
    fs::write(dir.join(CONFIG_FILE_NAME), "{ this is not json").unwrap();

    let loaded = AppConfig::load_or_create(&dir);

    assert_eq!(loaded.config.bilingual_set, default_bilingual_set());
    assert!(loaded.notes.iter().any(|note| note.contains("not valid JSON")), "notes: {:?}", loaded.notes);
    // The unreadable file is kept, not silently destroyed.
    let backup = dir.join("kcd_bilingual_generator_rust.json.bak");
    assert!(backup.exists(), "the broken file must be preserved");
    assert_eq!(fs::read_to_string(backup).unwrap(), "{ this is not json");
    // And a usable file took its place.
    assert_eq!(read_config_file(&dir), loaded.config);
}

#[test]
fn languages_cover_every_pair_plus_english_once() {
    let config = AppConfig {
        game_path: String::new(),
        bilingual_set: vec![
            LanguagePair::new("Chineses", "English"),
            LanguagePair::new("Chineses", "German"),
            LanguagePair::new("English", "Chineses"),
            LanguagePair::new("French", "Czech"),
        ],
    };

    // Order follows first appearance, duplicates collapse, English is always present.
    assert_eq!(config.languages(), vec!["Chineses", "English", "German", "French", "Czech"]);
    assert_eq!(
        config.pairs(),
        vec![
            ("Chineses".to_string(), "English".to_string()),
            ("Chineses".to_string(), "German".to_string()),
            ("English".to_string(), "Chineses".to_string()),
            ("French".to_string(), "Czech".to_string()),
        ]
    );
}

#[test]
fn generator_takes_pairs_and_game_path_from_the_config() {
    let config = AppConfig {
        game_path: r"D:\Games\KingdomComeDeliverance2".to_string(),
        bilingual_set: vec![LanguagePair::new("German", "English")],
    };

    let generator = BilingualGenerator::from_config(&config).unwrap();

    assert_eq!(generator.game_path, PathBuf::from(r"D:\Games\KingdomComeDeliverance2"));
    let pairs: Vec<(String, String)> = generator
        .bilingual_set
        .iter()
        .map(|(primary, secondary)| (primary.to_string(), secondary.to_string()))
        .collect();
    assert_eq!(pairs, vec![("German".to_string(), "English".to_string())]);
    // Only the pair's languages are read from disk, plus English for the fallback.
    assert_eq!(generator.language_to_process, vec!["German", "English"]);
}

#[test]
fn empty_game_path_in_the_config_keeps_the_detected_one() {
    let config = AppConfig {
        game_path: String::new(),
        bilingual_set: vec![LanguagePair::new("Chineses", "English")],
    };

    let detected = BilingualGenerator::init().unwrap().game_path;
    let generator = BilingualGenerator::from_config(&config).unwrap();

    assert_eq!(generator.game_path, detected);
}

#[test]
fn repeated_separators_in_the_game_path_are_collapsed() {
    let dir = temp_dir("separators");
    // What an older build wrote after copying Steam's VDF escaping verbatim.
    fs::write(
        dir.join(CONFIG_FILE_NAME),
        r#"{ "game_path": "E:////SteamLibrary////steamapps////common////KingdomComeDeliverance2" }"#,
    )
    .unwrap();

    let loaded = AppConfig::load_or_create(&dir);

    assert_eq!(loaded.config.game_path, r"E:/SteamLibrary/steamapps/common/KingdomComeDeliverance2");
    assert_eq!(read_config_file(&dir), loaded.config);
}

#[test]
fn unc_paths_keep_their_leading_double_separator() {
    let dir = temp_dir("unc");
    fs::write(dir.join(CONFIG_FILE_NAME), r#"{ "game_path": "\\\\NAS\\games\\KCD2\\" }"#).unwrap();

    let loaded = AppConfig::load_or_create(&dir);

    assert_eq!(loaded.config.game_path, r"\\NAS\games\KCD2");
}

#[test]
fn pair_text_round_trips_through_the_editor() {
    use generator_core::config::{pairs_to_text, parse_pairs_text};

    let pairs = default_bilingual_set();
    let text = pairs_to_text(&pairs);
    assert_eq!(text.lines().count(), 7);
    assert!(text.starts_with("Chineses + English"));
    assert_eq!(parse_pairs_text(&text).unwrap(), pairs);
}

#[test]
fn pair_text_accepts_comments_blank_lines_and_full_width_plus() {
    use generator_core::config::parse_pairs_text;

    let text = "\n# 注释行\nChineses ＋ English\n\n  Chineses+German  \nChineses + English\n";
    let pairs = parse_pairs_text(text).unwrap();

    // Full-width `＋` works, padding is trimmed, duplicates collapse, comments vanish.
    assert_eq!(pairs, vec![LanguagePair::new("Chineses", "English"), LanguagePair::new("Chineses", "German")]);
}

#[test]
fn pair_text_reports_the_offending_line() {
    use generator_core::config::parse_pairs_text;

    let error = parse_pairs_text("Chineses + English\nChineses +\n").unwrap_err();
    assert_eq!(error.line, 2);
    assert_eq!(error.content, "Chineses +");

    // A third separator is a typo, not a pair.
    assert!(parse_pairs_text("A + B + C").is_err());
    // Empty input must not silently mean "generate nothing".
    assert!(parse_pairs_text("  \n# only a comment\n").is_err());
}
