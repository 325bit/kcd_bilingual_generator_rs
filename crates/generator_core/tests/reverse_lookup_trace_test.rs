//! Traces the dialog reverse-lookup heuristic that `process_single_bilingual` runs when both the
//! secondary and the English text of an entry are `MISSING`.
//!
//! Every attempt is printed as one line: the id and text that were searched for, how many
//! primary-language entries were examined, and which entry supplied the fallback (if any).
//!
//! * `synthetic_reverse_lookup_trace` — builds tiny PAKs in a temp dir, so the trigger actually
//!   fires and the printed lines are deterministic. Also demonstrates the misattribution case:
//!   an entry with no `source_text` of its own silently borrows a *different* line's source
//!   text because both lines happen to share the same translation.
//! * `real_paks_reverse_lookup_trace` — same trace against a real `Localization` directory
//!   pointed at by `KCD_TEST_LOCALIZATION`. Skipped when the variable is unset.

use generator_core::bilingual_generator::{BilingualGenerator, ReverseLookupStats};
use generator_core::log::LogSink;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zip::write::SimpleFileOptions;

const DIALOG: &str = "text_ui_dialog.xml";

/// Writes `<dir>/<lang>_xml.pak` containing a minimal `text_ui_dialog.xml`.
///
/// Each row is `(entry id, 2nd cell, 3rd cell)`. The 2nd cell is what the generator keeps as
/// `source_text`, the 3rd as `translated_text`.
fn write_dialog_pak(dir: &Path, lang: &str, rows: &[(&str, &str, &str)]) {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(format!("{lang}_xml.pak"));
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options: SimpleFileOptions = SimpleFileOptions::default();

    let mut xml = String::from("<Table>\n<Row><Cell>Entry id</Cell><Cell>Source</Cell><Cell>Translation</Cell></Row>\n");
    for (id, source, translated) in rows {
        // Empty cells are intentional: they model an entry with no source text.
        xml.push_str(&format!("<Row><Cell>{id}</Cell><Cell>{source}</Cell><Cell>{translated}</Cell></Row>\n"));
    }
    xml.push_str("</Table>\n");

    zip.start_file(DIALOG, options).unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    zip.finish().unwrap();
}

struct TraceRun {
    lines: Vec<String>,
    stats: ReverseLookupStats,
    /// Rows of the generated PAK: `(entry id, combined text)`.
    written: Vec<(String, String)>,
}

/// Drives the real pipeline with tracing enabled and collects the generated output.
///
/// `game_root` is the folder that contains `Localization`; `work_dir` receives the generated
/// `bilingual_xml` tree and PAK.
fn run_traced(game_root: &Path, work_dir: PathBuf, primary: &str, secondary: &str) -> TraceRun {
    std::fs::create_dir_all(&work_dir).unwrap();

    let mut generator = BilingualGenerator::init().unwrap();
    generator.game_path = game_root.to_path_buf();
    generator.working_dir = work_dir;
    // Only the dialog file matters here, and only these three languages.
    generator.files_to_process = vec![DIALOG.to_string()];
    generator.language_to_process = vec![primary.to_string(), secondary.to_string(), "English".to_string()];

    let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink_lines = Arc::clone(&lines);
    generator.set_log_sink(LogSink::new(move |message| sink_lines.lock().unwrap().push(message)));
    generator.set_trace_reverse_lookup(true);

    generator.read_xml_from_paks().unwrap();
    let out_dir = generator.process_single_bilingual(primary, secondary).unwrap();
    let stats = generator.reverse_lookup_stats();

    // Read the produced PAK back so the test can show what actually reached the game.
    let pak = std::fs::File::open(out_dir.join(format!("{primary}_xml.pak"))).unwrap();
    let mut archive = zip::ZipArchive::new(pak).unwrap();
    let mut xml = String::new();
    {
        use std::io::Read;
        archive.by_name(DIALOG).unwrap().read_to_string(&mut xml).unwrap();
    }
    let written = xml
        .lines()
        .filter(|line| line.starts_with("<Row>"))
        .map(|line| {
            let cells: Vec<&str> = line.split("<Cell>").skip(1).map(|cell| cell.split("</Cell>").next().unwrap_or("")).collect();
            (
                cells.first().copied().unwrap_or("").to_string(),
                cells.get(2).copied().unwrap_or("").to_string(),
            )
        })
        .collect();

    let lines = std::mem::take(&mut *lines.lock().unwrap());
    TraceRun { lines, stats, written }
}

fn print_trace(label: &str, run: &TraceRun) {
    println!("\n===== {label} =====");
    println!("触发反查的条目数 (triggers)      = {}", run.stats.triggers);
    println!("  自身命中 (self_hits)           = {}", run.stats.self_hits);
    println!("  借用别的条目 (other_hits)      = {}", run.stats.other_hits);
    println!("  无匹配 (no_hits)               = {}", run.stats.no_hits());
    println!("累计扫描的条目数 (scanned)       = {}", run.stats.scanned);
    println!("--- 逐条追踪 ---");
    if run.lines.is_empty() {
        println!("(没有任何一次反查被触发)");
    }
    for line in &run.lines {
        println!("{line}");
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kcd_reverse_lookup_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn synthetic_reverse_lookup_trace() {
    let root = temp_dir("synthetic");
    let loc = root.join("Localization");

    // Chineses: our primary language. English and Czech deliberately only contain
    // `dlg_in_english`, so every other row ends up with both texts `MISSING`.
    write_dialog_pak(
        &loc,
        "Chineses",
        &[
            ("dlg_self", "Hello there", "你好啊"),
            ("dlg_in_english", "Present in English", "英文里有"),
            ("dlg_dup_a", "Yes.", "是的。"),
            ("dlg_dup_b", "", "是的。"),
            ("dlg_orphan", "", "绝无仅有的句子"),
        ],
    );
    write_dialog_pak(&loc, "Czech", &[("dlg_in_english", "Present in English", "Přítomno v angličtině")]);
    write_dialog_pak(&loc, "English", &[("dlg_in_english", "Present in English", "Present in English")]);

    let run = run_traced(&root, root.join("work"), "Chineses", "Czech");
    print_trace("synthetic: Chineses + Czech", &run);

    // Four rows trigger: dlg_self, dlg_dup_a, dlg_dup_b and dlg_orphan.
    assert_eq!(run.stats.triggers, 4, "trigger count");
    assert_eq!(run.stats.self_hits, 2, "dlg_self and dlg_dup_a match themselves");
    assert_eq!(run.stats.other_hits, 1, "dlg_dup_b borrows dlg_dup_a's source text");
    assert_eq!(run.stats.no_hits(), 1, "dlg_orphan matches nothing");
    // dlg_self scans 1 entry, both dup rows scan 3, dlg_orphan scans all 5.
    assert_eq!(run.stats.scanned, 1 + 3 + 3 + 5);

    // The risky line: dlg_dup_b has no source of its own, so it inherits "Yes." from dlg_dup_a.
    println!("\n--- 生成的 PAK 内容 ---");
    for (id, text) in &run.written {
        println!("{id:16} => {text:?}");
    }
    let dup_b = run.written.iter().find(|(id, _)| id == "dlg_dup_b").map(|(_, t)| t.as_str()).unwrap();
    assert!(dup_b.contains("Yes."), "misattributed fallback should be visible: {dup_b:?}");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn real_paks_reverse_lookup_trace() {
    let Some(loc) = std::env::var_os("KCD_TEST_LOCALIZATION") else {
        eprintln!("skipped: set KCD_TEST_LOCALIZATION to a game Localization directory to run against real PAKs");
        return;
    };
    let loc = PathBuf::from(&loc);
    assert!(loc.join("Chineses_xml.pak").exists(), "no Chineses_xml.pak in {}", loc.display());
    // `read_xml_from_paks` appends `Localization` itself, so the generator wants the parent folder.
    let game_root = loc.parent().expect("Localization directory must have a parent").to_path_buf();

    let mut ran_any = false;
    for secondary in ["English", "Czech"] {
        if !loc.join(format!("{secondary}_xml.pak")).exists() {
            continue;
        }
        ran_any = true;
        let run = run_traced(&game_root, temp_dir(&format!("real_work_{secondary}")), "Chineses", secondary);
        print_trace(&format!("real paks: {} + {secondary}", loc.display()), &run);

        // Sanity check: the counters must agree with the number of printed lines.
        assert_eq!(run.lines.len(), run.stats.triggers);
        assert_eq!(run.stats.triggers, run.stats.self_hits + run.stats.other_hits + run.stats.no_hits());
    }
    assert!(ran_any, "no usable secondary-language PAK next to Chineses_xml.pak");
}
