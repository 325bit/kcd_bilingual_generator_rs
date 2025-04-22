use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BilingualGeneratorError {
    #[error("Invalid Bilingual Set Format: {0}")]
    InvalidBilingualSet(String),

    #[error("Failed to extract PAK files")]
    PakExtractionFailed,

    #[error("XML processing failed: {0}")]
    XmlProcessingFailed(String),

    #[error("Failed to create new PAK file")]
    PakCreationFailed,

    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("TaskJoinError: {0}")]
    TaskJoinError(String), // lang_str and join_err
}
