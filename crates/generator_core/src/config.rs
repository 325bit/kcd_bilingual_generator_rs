//! The single runtime configuration file.
//!
//! Everything the generator needs to know about a run — where the game is installed and which
//! language pairs to emit — lives in `kcd_bilingual_generator_rust.json` next to the working
//! directory, so a release build needs no other side files. The file is created with sensible
//! defaults the first time it is needed, and the legacy `assets/bilingual_set.txt` is gone: its
//! `Primary + Secondary` lines are now the `bilingual_set` array.

use path_finder::PathFinder;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

/// Name of the only configuration file. No path, no extension games — this is what users edit.
pub const CONFIG_FILE_NAME: &str = "kcd_bilingual_generator_rust.json";

/// Pairs written on first run. `Chineses` is the game's own id for Simplified Chinese.
pub const DEFAULT_BILINGUAL_SET: [(&str, &str); 7] = [
    ("Chineses", "English"),
    ("Chineses", "French"),
    ("Chineses", "German"),
    ("Chineses", "Spanish"),
    ("Chineses", "Czech"),
    ("Chineses", "Japanese"),
    ("English", "Chineses"),
];

/// One output: replace the `secondary` language's text with `primary` + `secondary`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguagePair {
    pub primary: String,
    pub secondary: String,
}

impl LanguagePair {
    pub fn new(primary: impl Into<String>, secondary: impl Into<String>) -> Self {
        Self {
            primary: primary.into(),
            secondary: secondary.into(),
        }
    }

    /// Trims both sides; `None` when either side is blank.
    fn normalised(&self) -> Option<Self> {
        let primary = self.primary.trim();
        let secondary = self.secondary.trim();
        (!primary.is_empty() && !secondary.is_empty()).then(|| Self::new(primary, secondary))
    }
}

/// Contents of [`CONFIG_FILE_NAME`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    /// Game install root (`...\KingdomComeDeliverance2`), empty when it is not known yet.
    #[serde(default)]
    pub game_path: String,
    /// Pairs to generate. Missing from the file means "use the defaults".
    #[serde(default = "default_bilingual_set")]
    pub bilingual_set: Vec<LanguagePair>,
}

/// What [`AppConfig::load_or_create`] ended up doing, plus the notes worth showing the user.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: AppConfig,
    /// Absolute path of the config file, whether it existed or was just written.
    pub path: PathBuf,
    /// One line per decision the user might care about (created / recovered / detected path).
    pub notes: Vec<String>,
}

/// The pairs a fresh install starts with.
pub fn default_bilingual_set() -> Vec<LanguagePair> {
    DEFAULT_BILINGUAL_SET
        .iter()
        .map(|(primary, secondary)| LanguagePair::new(*primary, *secondary))
        .collect()
}

/// Renders pairs as the editable text block shown in the window (`Primary + Secondary` per line).
pub fn pairs_to_text(pairs: &[LanguagePair]) -> String {
    let mut text = String::new();
    for (index, pair) in pairs.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str(&pair.primary);
        text.push_str(" + ");
        text.push_str(&pair.secondary);
    }
    text
}

/// A line the window could not turn into a pair, reported with its 1-based line number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairParseError {
    pub line: usize,
    pub content: String,
}

impl std::fmt::Display for PairParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.content.is_empty() {
            write!(formatter, "at least one `Primary + Secondary` line is required")
        } else {
            write!(formatter, "line {}: \"{}\" is not `Primary + Secondary`", self.line, self.content)
        }
    }
}

impl std::error::Error for PairParseError {}

/// Parses the editable text block back into pairs.
///
/// One pair per line, `#` starts a comment, blank lines are skipped, and the full-width `＋` an
/// IME produces is accepted as the separator. An empty result is an error rather than an empty
/// list: generating nothing is never what the user meant.
pub fn parse_pairs_text(text: &str) -> Result<Vec<LanguagePair>, PairParseError> {
    let mut pairs: Vec<LanguagePair> = Vec::new();
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let normalised = line.replace('＋', "+");
        let mut sides = normalised.split('+').map(str::trim);
        let primary = sides.next().unwrap_or_default();
        let secondary = sides.next().unwrap_or_default();
        if primary.is_empty() || secondary.is_empty() || sides.next().is_some() {
            return Err(PairParseError {
                line: index + 1,
                content: line.to_string(),
            });
        }

        let pair = LanguagePair::new(primary, secondary);
        if !pairs.contains(&pair) {
            pairs.push(pair);
        }
    }

    if pairs.is_empty() {
        return Err(PairParseError {
            line: 1,
            content: String::new(),
        });
    }
    Ok(pairs)
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            game_path: String::new(),
            bilingual_set: default_bilingual_set(),
        }
    }
}

impl AppConfig {
    /// `dir/kcd_bilingual_generator_rust.json`.
    pub fn path_in(dir: &Path) -> PathBuf {
        dir.join(CONFIG_FILE_NAME)
    }

    /// The recorded game path as a `PathBuf` (empty when unknown).
    pub fn game_path_buf(&self) -> PathBuf {
        PathBuf::from(self.game_path.trim())
    }

    /// Pairs as plain strings, ready for `process_single_bilingual`.
    pub fn pairs(&self) -> Vec<(String, String)> {
        self.bilingual_set.iter().map(|pair| (pair.primary.clone(), pair.secondary.clone())).collect()
    }

    /// Languages whose PAK files must be read: every language used by a pair, `English` first-class
    /// because the generator always falls back to it, deduplicated in order of appearance.
    pub fn languages(&self) -> Vec<String> {
        let mut languages: Vec<String> = Vec::new();
        let mut push = |language: &str| {
            let language = language.trim();
            if !language.is_empty() && !languages.iter().any(|known| known == language) {
                languages.push(language.to_string());
            }
        };
        for pair in &self.bilingual_set {
            push(&pair.primary);
            push(&pair.secondary);
        }
        push("English");
        languages
    }

    /// Reads `dir`'s config, writing the file first when it is missing.
    ///
    /// Never fails: a config that cannot be read or parsed is backed up next to the original and
    /// replaced by the defaults, and everything that happened is reported through
    /// [`LoadedConfig::notes`] so the GUI can print it.
    pub fn load_or_create(dir: &Path) -> LoadedConfig {
        let path = Self::path_in(dir);
        let mut notes: Vec<String> = Vec::new();
        let mut must_write = false;

        let mut config = match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<AppConfig>(strip_bom(&text)) {
                Ok(config) => {
                    notes.push(format!("Loaded config {}", path.display()));
                    config
                }
                Err(error) => {
                    let backup = backup_invalid_file(&path, &mut notes);
                    notes.push(format!("{} is not valid JSON ({error}); using the defaults", path.display()));
                    if let Some(backup) = backup {
                        notes.push(format!("the unreadable file was kept as {}", backup.display()));
                    }
                    must_write = true;
                    Self::default()
                }
            },
            Err(error) if error.kind() == ErrorKind::NotFound => {
                notes.push(format!("Created {}", path.display()));
                must_write = true;
                Self::default()
            }
            Err(error) => {
                // Readable-dir-but-unreadable-file (permissions, locked): keep going in memory.
                notes.push(format!("Could not read {} ({error}); using the defaults", path.display()));
                Self::default()
            }
        };

        if config.normalise(&mut notes) {
            must_write = true;
        }

        if config.game_path.is_empty() {
            match PathFinder::new().find_game_path() {
                Ok(found) => {
                    config.game_path = found.display().to_string();
                    notes.push(format!("Detected the game at {}", config.game_path));
                    must_write = true;
                }
                Err(_) => {
                    notes.push("Game path unknown; pick it in the window (it will be saved to the config)".to_string());
                }
            }
        } else if !config.game_path_buf().exists() {
            // Keep it: a moved library or a temporarily unplugged drive should not wipe the setting.
            notes.push(format!("The configured game path does not exist right now: {}", config.game_path));
        }

        if must_write {
            match config.save_to(&path) {
                Ok(()) => notes.push(format!("Saved config {}", path.display())),
                Err(error) => notes.push(format!("Could not write {}: {error} (settings will not persist)", path.display())),
            }
        }

        LoadedConfig { config, path, notes }
    }

    /// Writes the config into `dir`, returning the file path.
    pub fn save(&self, dir: &Path) -> std::io::Result<PathBuf> {
        let path = Self::path_in(dir);
        self.save_to(&path)?;
        Ok(path)
    }

    /// Writes the config to an explicit path, pretty-printed so it stays hand-editable.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        // Write-then-rename: an interrupted save must not leave a truncated config behind.
        let temporary = path.with_extension("json.tmp");
        let mut file = fs::File::create(&temporary)?;
        file.write_all(json.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    }

    /// Cleans up hand-edited input. Returns `true` when the file on disk should be rewritten.
    fn normalise(&mut self, notes: &mut Vec<String>) -> bool {
        let mut changed = false;

        // Pasted paths often arrive quoted and with a trailing separator; both are noise, but a
        // bare `E:` would mean "current directory on E:" so a lone drive keeps its separator.
        let mut cleaned = self.game_path.trim().trim_matches('"').trim().trim_end_matches(['\\', '/']).to_string();
        if cleaned.ends_with(':') {
            cleaned.push('\\');
        }
        // Older path detection copied Steam's VDF escaping verbatim (`E:\\SteamLibrary`), which
        // only works by accident; collapsing runs of separators cleans those configs up.
        let collapsed = collapse_repeated_separators(&cleaned);
        if collapsed != self.game_path {
            self.game_path = collapsed;
            changed = true;
        }

        let mut pairs: Vec<LanguagePair> = Vec::with_capacity(self.bilingual_set.len());
        for pair in &self.bilingual_set {
            match pair.normalised() {
                Some(pair) => {
                    if !pairs.contains(&pair) {
                        pairs.push(pair);
                    } else {
                        notes.push(format!("Dropped the duplicate pair {} + {}", pair.primary, pair.secondary));
                        changed = true;
                    }
                }
                None => {
                    notes.push(format!("Dropped an incomplete pair: {:?} + {:?}", pair.primary, pair.secondary));
                    changed = true;
                }
            }
        }
        if pairs.len() != self.bilingual_set.len() {
            changed = true;
        }
        self.bilingual_set = pairs;

        if self.bilingual_set.is_empty() {
            notes.push("bilingual_set was empty; falling back to the defaults".to_string());
            self.bilingual_set = default_bilingual_set();
            changed = true;
        }

        changed
    }
}

/// A UTF-8 BOM is invisible in editors and makes `serde_json` reject the whole file.
fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// Collapses runs of `\` and `/` into one, keeping a leading UNC `\\` intact.
fn collapse_repeated_separators(path: &str) -> String {
    let (prefix, rest) = match path.strip_prefix(r"\\") {
        Some(rest) => (r"\\", rest),
        None => ("", path),
    };
    let mut collapsed = String::with_capacity(path.len());
    collapsed.push_str(prefix);
    let mut previous_was_separator = false;
    for character in rest.chars() {
        let is_separator = character == '\\' || character == '/';
        if is_separator && previous_was_separator {
            continue;
        }
        previous_was_separator = is_separator;
        collapsed.push(character);
    }
    collapsed
}

/// Moves an unparsable config aside so the fresh defaults can take its place without data loss.
fn backup_invalid_file(path: &Path, notes: &mut Vec<String>) -> Option<PathBuf> {
    let backup = path.with_extension("json.bak");
    match fs::rename(path, &backup) {
        Ok(()) => Some(backup),
        Err(error) => {
            notes.push(format!("Could not back up {}: {error}", path.display()));
            None
        }
    }
}
