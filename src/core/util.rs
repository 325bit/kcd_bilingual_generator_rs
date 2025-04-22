use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use zip::{
    write::{ExtendedFileOptions, FileOptions},
    CompressionMethod, ZipWriter,
};

use super::{bilingual_generator::LastTextValue, bilingual_generator_errors::BilingualGeneratorError};

pub fn secondary_text_combined(primary_text: &LastTextValue, secondary_text: &str, separator: &str) -> String {
    if secondary_text != "MISSING" && !secondary_text.is_empty() {
        format!("{}{}{}", primary_text.0, separator, secondary_text)
    } else {
        primary_text.0.clone() // Fallback to primary if secondary is missing/empty
    }
}

pub fn create_new_pak(files: Vec<PathBuf>, output_dir: &Path, primary_language: &str) -> Result<(), BilingualGeneratorError> {
    let pak_name = format!("{}_xml.pak", primary_language);
    let pak_path = output_dir.join(pak_name);

    let file = File::create(&pak_path).map_err(|_| BilingualGeneratorError::PakCreationFailed)?;
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<'_, ExtendedFileOptions> = FileOptions::default().compression_method(CompressionMethod::Deflated);

    for path in files {
        let file_name = path.file_name().ok_or(BilingualGeneratorError::PakCreationFailed)?;
        let file_name_str = file_name.to_str().ok_or(BilingualGeneratorError::PakCreationFailed)?;

        zip.start_file(file_name_str, options.clone())
            .map_err(|_| BilingualGeneratorError::PakCreationFailed)?;
        let content = std::fs::read(&path).map_err(|_| BilingualGeneratorError::PakCreationFailed)?;
        zip.write_all(&content).map_err(|_| BilingualGeneratorError::PakCreationFailed)?;
    }

    zip.finish().map_err(|_| BilingualGeneratorError::PakCreationFailed)?;

    Ok(())
}
