//! Times the real pipeline (all seven XML files) against any game's `Localization` directory and
//! reports the reverse-lookup counters next to it.
//!
//! ```text
//! cargo run --release -p generator_core --example pipeline_timing -- \
//!     "D:\path\to\game\Localization" [secondary languages...]
//! ```
//!
//! The first argument points at the folder that holds `<Language>_xml.pak` files; any further
//! arguments are secondary languages (default: English, Czech). Generated PAKs land in the temp
//! directory and are deleted afterwards.

use generator_core::bilingual_generator::BilingualGenerator;
use std::path::PathBuf;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let localization = PathBuf::from(args.next().ok_or("usage: pipeline_timing <Localization dir> [secondary langs...]")?);
    let mut secondaries: Vec<String> = args.collect();
    if secondaries.is_empty() {
        secondaries = vec!["English".to_string(), "Czech".to_string()];
    }
    // `read_xml_from_paks` appends `Localization` itself, so feed it the parent folder.
    let game_root = localization.parent().ok_or("Localization directory must have a parent")?.to_path_buf();

    let work_dir = std::env::temp_dir().join("kcd_pipeline_timing");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir)?;

    let mut generator = BilingualGenerator::init()?;
    generator.game_path = game_root;
    generator.working_dir = work_dir.clone();
    generator.language_to_process = std::iter::once("Chineses".to_string()).chain(secondaries.clone()).collect();

    let started = Instant::now();
    generator.read_xml_from_paks()?;
    println!("read_xml_from_paks            : {:>7.3}s", started.elapsed().as_secs_f64());

    // Entry counts per file and language. Rows whose trailing cells were empty used to be
    // dropped by the parser, so these numbers are the quickest parser-regression signal.
    use generator_core::bilingual_generator::{Language, XmlFile};
    // Clone so the lookups borrow `all_data` alone (the keys own their strings).
    let files = generator.files_to_process.clone();
    let languages = generator.language_to_process.clone();
    for file in &files {
        let file_key = XmlFile(file.clone().into());
        let Some(per_language) = generator.all_data.get(&file_key) else {
            println!("  {file:<28} (missing)");
            continue;
        };
        let counts: Vec<String> = languages
            .iter()
            .map(|language| {
                let language_key = Language(language.clone().into());
                match per_language.get(&language_key) {
                    Some(entries) => {
                        let empty_translation = entries.values().filter(|entry| entry.translated_text.is_empty()).count();
                        format!("{language}={} (empty 3rd cell {empty_translation})", entries.len())
                    }
                    None => format!("{language}=n/a"),
                }
            })
            .collect();
        println!("  {file:<28} {}", counts.join("  "));
    }

    for secondary in &secondaries {
        let started = Instant::now();
        let out_dir = generator.process_single_bilingual("Chineses", secondary)?;
        let elapsed = started.elapsed().as_secs_f64();
        let stats = generator.reverse_lookup_stats();
        let pak_bytes = std::fs::metadata(out_dir.join("Chineses_xml.pak")).map(|m| m.len()).unwrap_or(0);
        println!(
            "Chineses + {secondary:<12}     : {elapsed:>7.3}s   pak={:.1}MB   reverse-lookup: triggers={} scanned={} self={} other={} none={}",
            pak_bytes as f64 / 1024.0 / 1024.0,
            stats.triggers,
            stats.scanned,
            stats.self_hits,
            stats.other_hits,
            stats.no_hits()
        );
    }

    // Keep the generated PAKs around for inspection (`KCD_KEEP_WORK=1`).
    if std::env::var_os("KCD_KEEP_WORK").is_some() {
        println!("generated PAKs kept in {}", work_dir.display());
    } else {
        let _ = std::fs::remove_dir_all(&work_dir);
    }
    Ok(())
}
