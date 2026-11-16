//! Locks down how `read_xml_from_paks` handles rows whose cells are empty.
//!
//! The parser used to require three *populated* cells, so any row whose trailing cells were
//! empty (`<Row><Cell>id</Cell><Cell></Cell><Cell></Cell></Row>`) was dropped silently. Those
//! rows are real table entries — dialog lines with no translation, combat shouts, placeholders —
//! and dropping them made the dialog reverse lookup fall through to an unrelated row that
//! happened to share the same translation.

use generator_core::bilingual_generator::{BilingualGenerator, EntryId, Language, XmlFile};
use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;

const DIALOG: &str = "text_ui_dialog.xml";
const ITEMS: &str = "text_ui_items.xml";

fn write_pak(dir: &Path, lang: &str, xml: &str) {
    write_pak_with(dir, lang, DIALOG, xml);
}

fn write_pak_with(dir: &Path, lang: &str, file_name: &str, xml: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(format!("{lang}_xml.pak"));
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file(file_name, SimpleFileOptions::default()).unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    zip.finish().unwrap();
}

/// Tiny `<Row><Cell>…</Cell>…</Row>` scan, immune to rows containing line breaks of their own.
fn parse_rows(xml: &str) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<Row>") {
        let body_start = start + "<Row>".len();
        let Some(body_len) = rest[body_start..].find("</Row>") else { break };
        let body = &rest[body_start..body_start + body_len];

        let mut cells = Vec::new();
        let mut cursor = body;
        while let Some(cell_start) = cursor.find("<Cell>") {
            let after = &cursor[cell_start + "<Cell>".len()..];
            let Some(cell_len) = after.find("</Cell>") else { break };
            cells.push(after[..cell_len].to_string());
            cursor = &after[cell_len + "</Cell>".len()..];
        }
        if let Some(id) = cells.first() {
            rows.push((id.clone(), cells.get(2).cloned().unwrap_or_default()));
        }
        rest = &rest[body_start + body_len + "</Row>".len()..];
    }
    rows
}

/// Runs the pipeline over `files` and returns `(entry id, 3rd cell)` of the produced PAK.
fn generate(dir: &Path, files: &[&str], primary: &str, secondary: &str) -> Vec<(String, String)> {
    let mut generator = BilingualGenerator::init().unwrap();
    generator.game_path = dir.to_path_buf();
    generator.working_dir = dir.join("work");
    generator.files_to_process = files.iter().map(|file| (*file).to_string()).collect();
    // Only read the packs the fixture actually writes.
    generator.language_to_process = [primary, secondary, "English", "Czech"]
        .iter()
        .filter(|language| dir.join("Localization").join(format!("{language}_xml.pak")).exists())
        .map(|language| (*language).to_string())
        .collect();
    generator.read_xml_from_paks().unwrap();
    let out_dir = generator.process_single_bilingual(primary, secondary).unwrap();

    let pak = std::fs::File::open(out_dir.join(format!("{primary}_xml.pak"))).unwrap();
    let mut archive = zip::ZipArchive::new(pak).unwrap();
    let mut rows = Vec::new();
    for file in files {
        let mut xml = String::new();
        archive.by_name(file).unwrap().read_to_string(&mut xml).unwrap();
        rows.extend(parse_rows(&xml));
    }
    rows
}

#[test]
fn rows_with_empty_cells_are_kept() {
    let root = std::env::temp_dir().join("kcd_parser_empty_cells");
    let _ = std::fs::remove_dir_all(&root);
    let localization = root.join("Localization");

    let xml = concat!(
        "<Table>\n",
        // The header row must not become an entry.
        "<Row><Cell>Entry id</Cell><Cell>Source</Cell><Cell>Translation</Cell></Row>\n",
        // Both text cells empty: the original placeholder shape that used to be dropped.
        "<Row><Cell>only_id</Cell><Cell></Cell><Cell></Cell></Row>\n",
        // Trailing cell empty.
        "<Row><Cell>src_only</Cell><Cell>hello</Cell><Cell></Cell></Row>\n",
        // Leading text cell empty; a self-closing cell keeps the columns aligned.
        "<Row><Cell>dst_only</Cell><Cell/><Cell>你好</Cell></Row>\n",
        // No cell elements at all beyond the id.
        "<Row><Cell>bare_id</Cell></Row>\n",
        // Nothing but whitespace text between cells must not leak into the values.
        "<Row><Cell>plain</Cell><Cell>a</Cell><Cell>b</Cell></Row>\n",
        // A row without an id is unusable and must be skipped instead of collapsing onto "".
        "<Row><Cell></Cell><Cell>x</Cell><Cell>y</Cell></Row>\n",
        // Pretty-printed rows: the whitespace between cells is not cell content.
        "<Row>\n  <Cell>indented</Cell>\n  <Cell></Cell>\n  <Cell></Cell>\n</Row>\n",
        "</Table>\n",
    );
    write_pak(&localization, "Chineses", xml);

    let mut generator = BilingualGenerator::init().unwrap();
    generator.game_path = root.clone();
    generator.working_dir = root.join("work");
    generator.files_to_process = vec![DIALOG.to_string()];
    generator.language_to_process = vec!["Chineses".to_string()];
    generator.read_xml_from_paks().unwrap();

    let entries = generator
        .all_data
        .get(&XmlFile(DIALOG.into()))
        .expect("dialog file must be indexed")
        .get(&Language("Chineses".into()))
        .expect("Chineses data must be indexed");

    let get = |id: &str| {
        entries.get(&EntryId(id.to_string().into())).unwrap_or_else(|| {
            panic!(
                "entry {id} is missing; parsed ids: {:?}",
                entries.keys().map(|k| k.0.as_str()).collect::<Vec<_>>()
            )
        })
    };

    assert_eq!(entries.len(), 6, "header row and the id-less row must not be stored");
    assert_eq!(get("only_id").source_text.as_str(), "");
    assert_eq!(get("only_id").translated_text.as_str(), "");
    assert_eq!(get("src_only").source_text.as_str(), "hello");
    assert_eq!(get("src_only").translated_text.as_str(), "");
    assert_eq!(get("dst_only").source_text.as_str(), "");
    assert_eq!(get("dst_only").translated_text.as_str(), "你好");
    assert_eq!(get("bare_id").source_text.as_str(), "");
    assert_eq!(get("bare_id").translated_text.as_str(), "");
    assert_eq!(get("plain").source_text.as_str(), "a");
    assert_eq!(get("plain").translated_text.as_str(), "b");
    assert_eq!(get("indented").source_text.as_str(), "", "indentation must not become cell text");
    assert_eq!(get("indented").translated_text.as_str(), "");
    assert!(!entries.contains_key(&EntryId("Entry id".into())));

    let _ = std::fs::remove_dir_all(&root);
}

/// A secondary-language row that exists but carries an empty cell must fall back to English,
/// exactly like a row that is absent altogether used to.
#[test]
fn empty_secondary_cell_falls_back_to_english() {
    let root = std::env::temp_dir().join("kcd_empty_secondary_fallback");
    let _ = std::fs::remove_dir_all(&root);
    let localization = root.join("Localization");

    write_pak(
        &localization,
        "Chineses",
        concat!(
            "<Table>\n",
            "<Row><Cell>Entry id</Cell><Cell>Source</Cell><Cell>Translation</Cell></Row>\n",
            "<Row><Cell>has_czech</Cell><Cell></Cell><Cell>中文一</Cell></Row>\n",
            "<Row><Cell>empty_czech</Cell><Cell></Cell><Cell>中文二</Cell></Row>\n",
            "</Table>\n",
        ),
    );
    write_pak(
        &localization,
        "English",
        concat!(
            "<Table>\n<Row><Cell>Entry id</Cell><Cell>Source</Cell><Cell>Translation</Cell></Row>\n",
            "<Row><Cell>has_czech</Cell><Cell></Cell><Cell>First line</Cell></Row>\n",
            "<Row><Cell>empty_czech</Cell><Cell></Cell><Cell>Second line</Cell></Row>\n",
            "</Table>\n",
        ),
    );
    // Czech keeps its placeholder row instead of dropping it: the 3rd cell is empty.
    write_pak(
        &localization,
        "Czech",
        concat!(
            "<Table>\n<Row><Cell>Entry id</Cell><Cell>Source</Cell><Cell>Translation</Cell></Row>\n",
            "<Row><Cell>has_czech</Cell><Cell></Cell><Cell>První řádek</Cell></Row>\n",
            "<Row><Cell>empty_czech</Cell><Cell></Cell><Cell></Cell></Row>\n",
            "</Table>\n",
        ),
    );

    let rows = generate(&localization.parent().unwrap().to_path_buf(), &[DIALOG], "Chineses", "Czech");
    let cell = |id: &str| rows.iter().find(|(row_id, _)| row_id == id).map(|(_, text)| text.as_str()).unwrap();

    assert_eq!(cell("has_czech"), "中文一\\n První řádek");
    assert_eq!(
        cell("empty_czech"),
        "中文二\\n Second line",
        "an empty secondary cell must not shadow the English fallback"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Entry ids occasionally carry stray line breaks inside the id cell, and the exact characters
/// differ between language packs (`\n\n` vs `\r\n\r\n`). Such rows must still find their
/// translation; the id written to the PAK stays untouched.
#[test]
fn ids_with_stray_line_breaks_still_match() {
    let root = std::env::temp_dir().join("kcd_trailing_id_whitespace");
    let _ = std::fs::remove_dir_all(&root);
    let localization = root.join("Localization");

    write_pak_with(
        &localization,
        "Chineses",
        ITEMS,
        concat!(
            "<Table>\n<Row><Cell>Entry id</Cell><Cell>Source</Cell><Cell>Translation</Cell></Row>\n",
            "<Row><Cell>armor_plain</Cell><Cell></Cell><Cell>普通外套</Cell></Row>\n",
            "<Row><Cell>armor_broken\n\n</Cell><Cell></Cell><Cell>破洞外套</Cell></Row>\n",
            "</Table>\n",
        ),
    );
    write_pak_with(
        &localization,
        "English",
        ITEMS,
        concat!(
            "<Table>\n<Row><Cell>Entry id</Cell><Cell>Source</Cell><Cell>Translation</Cell></Row>\n",
            "<Row><Cell>armor_plain</Cell><Cell></Cell><Cell>Plain coat</Cell></Row>\n",
            "<Row><Cell>armor_broken\r\n\r\n</Cell><Cell></Cell><Cell>Torn coat</Cell></Row>\n",
            "</Table>\n",
        ),
    );

    let rows = generate(&localization.parent().unwrap().to_path_buf(), &[ITEMS], "Chineses", "English");
    let plain = rows.iter().find(|(row_id, _)| row_id == "armor_plain").map(|(_, text)| text.as_str()).unwrap();
    assert_eq!(plain, "普通外套 / Plain coat");

    let broken = rows
        .iter()
        .find(|(row_id, _)| row_id.trim() == "armor_broken")
        .expect("the row must keep its original id");
    assert!(
        broken.1.contains("Torn coat"),
        "stray line breaks in the id must not hide the translation: {:?}",
        broken.1
    );

    let _ = std::fs::remove_dir_all(&root);
}
