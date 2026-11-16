use crate::util::{SEPARATOR_NEWLINE, SEPARATOR_SLASH, create_new_pak, secondary_text_combined};

use super::bilingual_generator_errors::BilingualGeneratorError;
use crate::config::AppConfig;
use crate::log::LogSink;
use faststr::FastStr;
use indexmap::IndexMap;
use path_finder::PathFinder;
use quick_xml::Reader;
use quick_xml::events::Event;
use rayon::prelude::*;
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use zip::ZipArchive;

// Main generator struct that coordinates all operations
#[derive(Debug)]
pub struct BilingualGenerator {
    pub game_path: PathBuf,
    pub working_dir: PathBuf,
    pub files_to_process: Vec<String>,
    pub language_to_process: Vec<String>,
    /// Pairs to emit, from the config file. Empty until a config is applied.
    pub bilingual_set: Vec<(FastStr, FastStr)>,
    pub all_data: HashMap<XmlFile, HashMap<Language, IndexMap<EntryId, TextEntry>>>,
    /// Where non-fatal diagnostics go. `None` falls back to stderr, which only works when the
    /// process actually owns a console (debug builds and headless runs).
    log_sink: Option<LogSink>,
    /// Emit one diagnostic line per dialog reverse-lookup attempt. Off by default.
    trace_reverse_lookup: bool,
    lookup_triggers: AtomicUsize,
    lookup_scanned: AtomicUsize,
    lookup_self_hits: AtomicUsize,
    lookup_other_hits: AtomicUsize,
}

/// How often the dialog reverse-lookup heuristic fired, and what it cost.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReverseLookupStats {
    /// Entries whose secondary *and* English text were both `MISSING` — the only condition that
    /// starts the search.
    pub triggers: usize,
    /// Primary-language entries examined while searching, summed over every trigger.
    pub scanned: usize,
    /// Searches that landed on the entry itself (it had its own `source_text` after all).
    pub self_hits: usize,
    /// Searches that borrowed *another* entry's `source_text` because it happened to carry the
    /// same translation. This is the risky case: repeated lines ("Yes.", "Of course.") cannot be
    /// told apart by text alone.
    pub other_hits: usize,
}

impl ReverseLookupStats {
    /// Triggers that ended with an empty secondary text.
    pub fn no_hits(&self) -> usize {
        self.triggers.saturating_sub(self.self_hits + self.other_hits)
    }
}

/// Joins the two halves of a bilingual cell with `separator`, skipping empty halves.
///
/// The parser keeps rows whose text cells are empty (they are real table entries), so joining
/// unconditionally would emit a lone separator — a blank subtitle line or a stray " / " in a
/// menu. Dropping the empty half keeps such rows harmless.
fn combine_text(primary: &str, secondary: &str, separator: &str) -> FastStr {
    match (primary.is_empty(), secondary.is_empty()) {
        (true, true) => String::new().into(),
        (true, false) => secondary.to_string().into(),
        (false, true) => primary.to_string().into(),
        (false, false) => format!("{primary}{separator}{secondary}").into(),
    }
}

/// Is there usable text in this cell?
///
/// A cell has no text in two shapes: the entry is missing from the table (`"MISSING"` sentinel
/// used by the lookups below) or the entry exists with an empty cell (`""`, which the parser now
/// keeps instead of dropping the row). Both must fall through to the next fallback.
fn has_text(text: &str) -> bool {
    !text.is_empty() && text != "MISSING"
}

/// Looks an entry up by its id, tolerating stray whitespace inside the id cell.
///
/// A few table rows carry line breaks or no-break spaces inside the id cell, and the exact
/// characters differ per language pack — `ui_nm_TorsoPainter_m01\n\n` in the Chinese table is
/// `ui_nm_TorsoPainter_m01\r\n\r\n` in the English one, so a plain key lookup misses. The common
/// case is still an exact hit; the scan only runs when the id actually carries such whitespace.
fn find_entry<'a>(entries: &'a IndexMap<EntryId, TextEntry>, id: &EntryId) -> Option<&'a TextEntry> {
    if let Some(entry) = entries.get(id) {
        return Some(entry);
    }
    let trimmed = id.0.trim();
    if trimmed.len() == id.0.len() {
        // Nothing to normalise, the entry is genuinely absent.
        return None;
    }
    entries.iter().find(|(other, _)| other.0.trim() == trimmed).map(|(_, entry)| entry)
}
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Language(pub FastStr);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct XmlFile(pub FastStr);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct EntryId(pub FastStr);
// #[derive(Debug, Clone, Hash, PartialEq, Eq)]
// pub struct LastTextValue(pub FastStr);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TextEntry {
    pub source_text: FastStr,     // 对应第二个 <Cell>
    pub translated_text: FastStr, // 对应第三个 <Cell>
}
// #[derive(Debug)]
// ///HashMap<Entry id, Text in last cell>
// pub struct single_xml_data {
//     pub data: HashMap<String, String>,
// }
impl BilingualGenerator {
    //Constructor to initialize the generator with the working directory
    pub fn init() -> Result<Self, BilingualGeneratorError> {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        let files_to_process = vec![
            String::from("text_ui_dialog.xml"),    // Dialogues
            String::from("text_ui_quest.xml"),     // Quests
            String::from("text_ui_tutorials.xml"), // Tutorials
            String::from("text_ui_soul.xml"),      // Stats/Effects
            String::from("text_ui_items.xml"),     // Items
            String::from("text_ui_menus.xml"),     // Menus
            String::from("text_ui_misc.xml"),
        ];
        // Kept in step with the config defaults so a plain `init()` is usable on its own
        // (tests, examples); the GUI applies the real config through [`Self::from_config`].
        let default_config = AppConfig::default();
        let defaut_language_to_process = default_config.languages();
        let mut path_finder = PathFinder::new();
        // let game_path = path_finder
        //     .find_game_path()
        //     .map(|p| p.to_path_buf())
        //     .unwrap_or_else(|_| PathBuf::new());
        let kcd_path = match path_finder.find_game_path() {
            Ok(path) => path.to_path_buf(),
            Err(_) => PathBuf::new(),
        };
        let bilingual_set: Vec<(FastStr, FastStr)> = default_config
            .bilingual_set
            .into_iter()
            .map(|pair| (pair.primary.into(), pair.secondary.into()))
            .collect();
        Ok(Self {
            game_path: kcd_path,
            working_dir,
            files_to_process,
            language_to_process: defaut_language_to_process,
            bilingual_set,
            all_data: HashMap::new(),
            log_sink: None,
            trace_reverse_lookup: false,
            lookup_triggers: AtomicUsize::new(0),
            lookup_scanned: AtomicUsize::new(0),
            lookup_self_hits: AtomicUsize::new(0),
            lookup_other_hits: AtomicUsize::new(0),
        })
    }

    /// Routes non-fatal diagnostics to `sink` instead of stderr.
    pub fn set_log_sink(&mut self, sink: LogSink) {
        self.log_sink = Some(sink);
    }

    /// Reports a recoverable problem. Never aborts the run: generation keeps going and the
    /// message surfaces in the GUI messages area (or stderr when no sink is installed).
    fn warn(&self, message: impl Into<String>) {
        self.emit(message.into());
    }

    /// Verbose diagnostics for the dialog reverse-lookup heuristic. Only called while
    /// [`Self::set_trace_reverse_lookup`] is on, so normal runs stay quiet.
    fn trace(&self, message: impl Into<String>) {
        self.emit(message.into());
    }

    fn emit(&self, message: String) {
        match &self.log_sink {
            Some(sink) => sink.log(message),
            None => eprintln!("{message}"),
        }
    }

    /// Turns on per-lookup tracing for the dialog reverse-lookup heuristic.
    ///
    /// Each trigger then emits one line describing what was searched for, how many entries were
    /// scanned, and which entry (if any) supplied the fallback text. Counters are always
    /// collected; this only controls the output.
    pub fn set_trace_reverse_lookup(&mut self, enabled: bool) {
        self.trace_reverse_lookup = enabled;
    }

    /// Snapshot of the dialog reverse-lookup heuristic for the **most recent**
    /// [`Self::process_single_bilingual`] call.
    pub fn reverse_lookup_stats(&self) -> ReverseLookupStats {
        ReverseLookupStats {
            triggers: self.lookup_triggers.load(Ordering::Relaxed),
            scanned: self.lookup_scanned.load(Ordering::Relaxed),
            self_hits: self.lookup_self_hits.load(Ordering::Relaxed),
            other_hits: self.lookup_other_hits.load(Ordering::Relaxed),
        }
    }
    /// Builds a generator driven by an already-loaded config file.
    ///
    /// The config is the single source of truth for both the game path and the pairs to emit, so
    /// callers only have to load it once (see [`AppConfig::load_or_create`]).
    pub fn from_config(config: &AppConfig) -> Result<Self, BilingualGeneratorError> {
        let mut generator = Self::init()?;
        generator.apply_config(config);
        Ok(generator)
    }

    /// Replaces the game path, the pairs to generate, and the languages to read from the PAKs.
    ///
    /// An empty `game_path` keeps whatever auto-detection found, so a config created before the
    /// game was installed still works.
    pub fn apply_config(&mut self, config: &AppConfig) {
        let game_path = config.game_path_buf();
        if !game_path.as_os_str().is_empty() {
            self.game_path = game_path;
        }
        self.bilingual_set = config
            .bilingual_set
            .iter()
            .map(|pair| (pair.primary.clone().into(), pair.secondary.clone().into()))
            .collect();
        self.language_to_process = config.languages();
    }

    /// Reads XML files from the pak files located in the Localization folder and stores the Entry id
    /// and secondary text (last cell) for each XML file into self.all_data.
    pub fn read_xml_from_paks(&mut self) -> Result<(), BilingualGeneratorError> {
        // Collect all_data in a thread-safe manner
        let all_data = Mutex::new(&mut self.all_data);

        self.language_to_process.par_iter().try_for_each(|language| {
            let pak_filename = format!("{}_xml.pak", language);
            let pak_path = self.game_path.join("Localization").join(&pak_filename);
            let pak_file = File::open(&pak_path).map_err(|e| BilingualGeneratorError::PakOperationFailed {
                operation: "opening PAK file".to_string(),
                context: pak_path.display().to_string(),
                source: e,
            })?;

            let mut archive = ZipArchive::new(pak_file).map_err(|e| BilingualGeneratorError::PakOperationFailed {
                operation: "creating ZipArchive from PAK".to_string(),
                context: pak_path.display().to_string(),
                source: e.into(),
            })?;

            self.files_to_process.iter().try_for_each(|xml_filename| {
                let mut xml_file = archive.by_name(xml_filename).map_err(|e| BilingualGeneratorError::PakOperationFailed {
                    operation: "finding XML file in PAK".to_string(),
                    context: format!("{} in {}", xml_filename, pak_filename),
                    source: e.into(),
                })?;

                let mut content = String::new();
                xml_file.read_to_string(&mut content).map_err(|e| BilingualGeneratorError::PakOperationFailed {
                    operation: "reading XML content from PAK".to_string(),
                    context: format!("{} in {}", xml_filename, pak_filename),
                    source: e,
                })?;
                let mut reader = Reader::from_str(&content);
                // reader.trim_text(true); // Reduces unnecessary allocations
                let mut buf = Vec::with_capacity(512); // Pre-allocate buffer
                let mut single_data = IndexMap::new();
                let mut current_cells: Vec<String> = Vec::with_capacity(4); // Pre-allocate
                let mut inside_row = false;
                // Text is only meaningful between `<Cell>` and `</Cell>`; the whitespace that
                // separates cells in a pretty-printed table must not become cell content.
                let mut inside_cell = false;
                let mut cell_index: usize = 0;

                loop {
                    match reader.read_event_into(&mut buf) {
                        Ok(Event::Start(e)) if e.name().as_ref() == b"Row" => {
                            inside_row = true;
                            inside_cell = false;
                            current_cells.clear();
                            cell_index = 0; // Reset cell index for a new row
                        }
                        Ok(Event::End(e)) if e.name().as_ref() == b"Row" => {
                            inside_row = false;
                            inside_cell = false;
                            // Only the entry id is mandatory. Text cells are allowed to be
                            // empty (`<Cell></Cell>` / `<Cell/>`), so rows must not be dropped
                            // just because they carry fewer than three cells: dropping them
                            // made every later reverse lookup fall through to an unrelated
                            // row whose text happened to be equal.
                            match current_cells.first() {
                                Some(id) if !id.is_empty() && id != "Entry id" => {
                                    let entry = TextEntry {
                                        // Missing or empty second cell becomes an empty string.
                                        source_text: current_cells.get(1).cloned().unwrap_or_default().into(),
                                        // Missing or empty third cell becomes an empty string.
                                        translated_text: current_cells.get(2).cloned().unwrap_or_default().into(),
                                    };
                                    single_data.insert(EntryId(id.clone().into()), entry);
                                }
                                _ => {}
                            }
                        }
                        // We are now inside a cell: the text handler may capture content.
                        Ok(Event::Start(e)) if inside_row && e.name().as_ref() == b"Cell" => {
                            inside_cell = true;
                        }
                        Ok(Event::End(e)) if inside_row && e.name().as_ref() == b"Cell" => {
                            inside_cell = false;
                            cell_index += 1; // Increment cell index after a cell ends
                        }
                        // A self-closing cell (`<Cell/>`) emits no text event, but it still
                        // occupies a position inside the row, so advance the index to keep
                        // the remaining columns aligned.
                        Ok(Event::Empty(e)) if inside_row && e.name().as_ref() == b"Cell" => {
                            cell_index += 1;
                        }
                        Ok(Event::Text(e)) => {
                            // Only capture text that actually belongs to a <Cell>.
                            if inside_row && inside_cell {
                                // Read the raw text (including escaped entities) as a string
                                let raw_text = std::str::from_utf8(e.as_ref())
                                    .map_err(|_| BilingualGeneratorError::XmlProcessingFailed("Invalid UTF-8 in XML text".to_string()))?;

                                // Ensure current_cells has enough capacity for the current cell_index
                                // If a cell is empty (e.g., <Cell></Cell>), no text event will be emitted for it.
                                // We need to fill in empty strings for missing cells to maintain correct indexing.
                                while current_cells.len() <= cell_index {
                                    current_cells.push("".to_string()); // Fill with empty strings for skipped cells
                                }
                                current_cells[cell_index] = raw_text.to_string();
                            }
                        }
                        Ok(Event::Eof) => break,
                        Err(e) => return Err(BilingualGeneratorError::XmlProcessingFailed(format!("XML error: {}", e))),
                        _ => {}
                    }
                    buf.clear();
                }

                // Thread-safe insertion into all_data
                let mut guard = all_data.lock().unwrap();
                guard
                    .entry(XmlFile(xml_filename.clone().into()))
                    .or_insert_with(HashMap::new)
                    .insert(Language(language.clone().into()), single_data);
                Ok(())
            })
        })
    }

    pub fn process_single_bilingual(&self, primary_language: &str, secondary_language: &str) -> Result<PathBuf, BilingualGeneratorError> {
        // The counters describe a single call, so start from zero every time.
        self.lookup_triggers.store(0, Ordering::Relaxed);
        self.lookup_scanned.store(0, Ordering::Relaxed);
        self.lookup_self_hits.store(0, Ordering::Relaxed);
        self.lookup_other_hits.store(0, Ordering::Relaxed);

        // Create output directory for the bilingual XML files
        let xml_output_dir = self
            .working_dir
            .join(format!("bilingual_xml\\{} + {}\\Localization", primary_language, secondary_language));

        // A thread-safe vector to collect paths of generated XML files for later PAK creation and cleanup.
        let xml_output_set: Mutex<Vec<PathBuf>> = Mutex::new(vec![]);

        // Ensure the output directory exists.
        std::fs::create_dir_all(&xml_output_dir)
            .map_err(|e| BilingualGeneratorError::XmlProcessingFailed(format!("Error creating output directory: {}", e)))?;

        // Prepare language identifiers for efficient lookups.
        let primary_lang = Language(primary_language.to_string().into());
        let secondary_lang = Language(secondary_language.to_string().into());
        let english_lang = Language("English".into());

        // Process each XML file in parallel using Rayon.
        self.files_to_process.par_iter().for_each(|file_name| {
            let xml_file = XmlFile(file_name.clone().into());

            // Get all data for the current XML file. Handle missing file gracefully.
            let file_data = self
                .all_data
                .get(&xml_file)
                .ok_or_else(|| BilingualGeneratorError::XmlProcessingFailed(format!("Could not find the required XML file in `all_data`: {}", file_name)));

            if let Ok(file_data) = file_data {
                // Get entries for the primary language.
                let primary_entries = file_data.get(&primary_lang).ok_or_else(|| {
                    BilingualGeneratorError::XmlProcessingFailed(format!("Could not get primary_entries for primary_language [{:?}].", &primary_lang))
                });

                // Create empty IndexMaps as fallbacks if a language's data is not found.
                let empty_map: IndexMap<EntryId, TextEntry> = IndexMap::new();
                let secondary_entries = file_data.get(&secondary_lang).unwrap_or(&empty_map);
                let english_entries = file_data.get(&english_lang).unwrap_or(&empty_map);

                let mut rows = Vec::new(); // Collect all <Row> elements for this XML file.

                match primary_entries {
                    Ok(entries) => {
                        for (entry_id, primary_entry) in entries {
                            // Extract the primary language's translated text (from the 3rd cell of the primary language's XML).
                            let primary_text_str = primary_entry.translated_text.as_str();

                            // Get the secondary language's translated text. Default to "MISSING" if not found.
                            let mut secondary_text_str = find_entry(secondary_entries, entry_id).map(|e| e.translated_text.as_str()).unwrap_or("MISSING");

                            // Get the English translated text. Default to "MISSING" if not found.
                            let english_text_str = find_entry(english_entries, entry_id).map(|e| e.translated_text.as_str()).unwrap_or("MISSING");

                            // --- START OF NEW LOGIC: Reverse-lookup for missing secondary/English text in text_ui_dialog.xml ---
                            // This block addresses the specific scenario where an entry in the primary language
                            // (e.g., Chinese) exists, but its corresponding secondary language (e.g., English, as source text)
                            // and fallback English are both missing — either because the row is absent from the
                            // table or because the table carries an empty cell for it.
                            if file_name == "text_ui_dialog.xml"
                                && !has_text(secondary_text_str)
                                && !has_text(english_text_str)
                                && !primary_text_str.is_empty()
                            // Ensure primary text is not empty before attempting lookup
                            {
                                // Iterate over all primary language entries (again) to find a potential match.
                                let mut scanned = 0usize;
                                let mut hit: Option<(&str, &str)> = None;
                                for (other_id, other_entry) in entries.iter() {
                                    scanned += 1;
                                    // If another entry has the same translated text as the current primary text
                                    // AND its `source_text` (from the 2nd cell, typically the original English) is not empty,
                                    // then we've found a suitable replacement.
                                    if other_entry.translated_text == primary_entry.translated_text && !other_entry.source_text.is_empty() {
                                        // Use this non-empty source text to fill the `secondary_text_str` gap.
                                        let other_id: &str = &other_id.0;
                                        let source_text: &str = &other_entry.source_text;
                                        hit = Some((other_id, source_text));
                                        secondary_text_str = source_text;
                                        break; // Found a suitable replacement, exit this inner search loop.
                                    }
                                }

                                let this_id: &str = &entry_id.0;
                                let self_hit = matches!(hit, Some((id, _)) if id == this_id);
                                self.lookup_triggers.fetch_add(1, Ordering::Relaxed);
                                self.lookup_scanned.fetch_add(scanned, Ordering::Relaxed);
                                match (hit, self_hit) {
                                    (Some(_), true) => self.lookup_self_hits.fetch_add(1, Ordering::Relaxed),
                                    (Some(_), false) => self.lookup_other_hits.fetch_add(1, Ordering::Relaxed),
                                    (None, _) => 0,
                                };

                                if self.trace_reverse_lookup {
                                    let outcome = match hit {
                                        Some((id, source)) if self_hit => format!("hit=self id={id} source={source:?}"),
                                        Some((id, source)) => {
                                            format!("hit=OTHER id={id} source={source:?} (borrowed from a different line)")
                                        }
                                        None => "hit=none".to_string(),
                                    };
                                    self.trace(format!(
                                        "[lookup] file={file_name} id={this_id} text={primary_text_str:?} scanned={scanned}/{total} -> {outcome}",
                                        total = entries.len()
                                    ));
                                }
                            }
                            // --- END OF NEW LOGIC ---

                            // Determine the combined text based on file name and specific rules.
                            let combined_text = match file_name.as_str() {
                                "text_ui_menus.xml" => {
                                    if !entry_id.0.contains("ui_helpoverlay") {
                                        match true {
                                            // Specific handling for loading screens or codex content.
                                            _ if entry_id.0.contains("ui_loading") || entry_id.0.contains("codex_cont") => {
                                                secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_NEWLINE)
                                            }
                                            // Handle very short primary texts or combined texts exceeding character limits.
                                            _ if primary_text_str.chars().count() <= 6
                                                || (primary_text_str.chars().count() + secondary_text_str.chars().count() > 920) =>
                                            {
                                                primary_text_str.to_string().into() // Only primary text
                                            }
                                            // If primary text is long, use a newline separator.
                                            _ if primary_text_str.chars().count() >= 20 => {
                                                secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_NEWLINE)
                                            }
                                            // Default for menus: use a slash separator.
                                            _ => secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_SLASH),
                                        }
                                    } else {
                                        primary_text_str.to_string().into() // For helpoverlay entries, only use primary text.
                                    }
                                }
                                "text_ui_dialog.xml" => {
                                    // Use the (potentially corrected by reverse-lookup) secondary text.
                                    // If it has no text at all, fall back to the English text.
                                    let fallback_text = if has_text(secondary_text_str) {
                                        secondary_text_str
                                    } else if has_text(english_text_str) {
                                        english_text_str
                                    } else {
                                        #[cfg(debug_assertions)]
                                        {
                                            "MISSING"
                                        }
                                        #[cfg(not(debug_assertions))]
                                        {
                                            ""
                                        }
                                    };
                                    // Rows from the source table may legitimately have empty
                                    // cells. Joining them unconditionally would produce a lone
                                    // separator, i.e. a blank subtitle line or a stray slash.
                                    combine_text(primary_text_str, fallback_text, SEPARATOR_NEWLINE)
                                }
                                "text_ui_items.xml" => {
                                    match true {
                                        // Specific conditions for item step descriptions or certain combat modifiers.
                                        _ if (entry_id.0.contains("step")
                                            && !entry_id.0.contains("_step_1")
                                            && primary_text_str.chars().count() >= 10)
                                            || (entry_id.0.contains("step_1")
                                                && (entry_id.0.contains("scatter") || entry_id.0.contains("longWeak") || entry_id.0.contains("bane"))) =>
                                        {
                                            FastStr::from(primary_text_str.to_string()) // Only primary text for these specific items.
                                        }
                                        // If primary text is moderately long, use a newline separator.
                                        _ if primary_text_str.chars().count() >= 7 => {
                                            secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_NEWLINE)
                                        }
                                        // Default for items: use a slash separator.
                                        _ => secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_SLASH),
                                    }
                                }
                                "text_ui_soul.xml" => match true {
                                    // Short texts or specific stat entries use a slash separator.
                                    _ if primary_text_str.chars().count() <= 7 && primary_text_str != "MISSING"
                                        || primary_text_str.chars().count() <= 12 && entry_id.0.contains("stat_") =>
                                    {
                                        secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_SLASH)
                                    }
                                    // Longer texts, provided they don't exceed a combined character limit, use a newline.
                                    _ if primary_text_str.chars().count() + secondary_text_str.chars().count() < 932 => {
                                        secondary_text_combined(primary_text_str, secondary_text_str, SEPARATOR_NEWLINE)
                                    }
                                    // Very long texts, just use the primary text.
                                    _ => primary_text_str.to_string().into(),
                                },
                                _ => {
                                    // Default case for any other XML files: combine with slash.
                                    // If secondary text has no text at all, fall back to English text.
                                    let fallback_text = if has_text(secondary_text_str) {
                                        secondary_text_str
                                    } else if has_text(english_text_str) {
                                        english_text_str
                                    } else {
                                        #[cfg(debug_assertions)]
                                        {
                                            "MISSING"
                                        }
                                        #[cfg(not(debug_assertions))]
                                        {
                                            ""
                                        }
                                    };
                                    combine_text(primary_text_str, fallback_text, SEPARATOR_SLASH)
                                }
                            };

                            // Construct the XML Row.
                            // The second <Cell> will contain the original source text (e.g., English from the 2nd cell of the primary language file).
                            // The third <Cell> contains the combined bilingual text.
                            rows.push(format!(
                                "<Row><Cell>{}</Cell><Cell>{}</Cell><Cell>{}</Cell></Row>",
                                entry_id.0, primary_entry.source_text, combined_text
                            ));
                        }
                    }
                    Err(e) => {
                        // Report, then fall back to English so this language pair still gets rows.
                        self.warn(format!(
                            "{file_name}: could not load `{primary_language}` entries ({e}); falling back to English text"
                        ));
                        // As a fallback, use English entries if primary entries are unavailable.
                        for (entry_id, english_entry) in english_entries.iter() {
                            rows.push(format!(
                                "<Row><Cell>{}</Cell><Cell>{}</Cell><Cell>{}</Cell></Row>",
                                entry_id.0, english_entry.source_text, english_entry.translated_text
                            ));
                        }
                    }
                }

                // Assemble the full XML content and determine the output path.
                let xml_content = format!("<Table>\n{}\n</Table>", rows.join("\n"));
                let xml_output_path = xml_output_dir.join(file_name);

                // Write first, then register the path: a failed write must not leave a path in the
                // pak list, otherwise PAK creation would trip over the missing file later.
                match std::fs::write(&xml_output_path, xml_content) {
                    Ok(()) => {
                        let mut xml_output_set_guard = xml_output_set.lock().unwrap();
                        xml_output_set_guard.push(xml_output_path);
                    }
                    Err(e) => {
                        self.warn(format!(
                            "{file_name}: failed to write generated XML to {} ({e}); this file is left out of the pak",
                            xml_output_path.display()
                        ));
                    }
                }
            } else {
                self.warn(format!("{file_name}: skipped, the XML file is missing from the loaded paks"));
            }
        });

        // After all parallel processing is complete, collect the generated XML file paths.
        let generated_xml_paths: Vec<PathBuf> = xml_output_set.into_inner().unwrap();

        // Attempt to create a new PAK file from the generated XML files.
        // Propagate the underlying error verbatim instead of collapsing it to PakCreationFailed:
        // the GUI has no console, so this message is the only diagnostic the user will ever see.
        create_new_pak(generated_xml_paths.clone(), &xml_output_dir, primary_language)?;

        // PAK creation succeeded, so the temporary XML files are no longer needed.
        generated_xml_paths.par_iter().for_each(|path| {
            if let Err(e) = std::fs::remove_file(path) {
                self.warn(format!("could not delete temporary XML file {} ({e})", path.display()));
            }
        });

        Ok(xml_output_dir)
    }
}
