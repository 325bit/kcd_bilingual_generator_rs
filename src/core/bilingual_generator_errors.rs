use std::{fmt, io};

pub enum BilingualGeneratorError {
    // GameNotFound,
    InvalidBilingualSet(String),
    PakExtractionFailed,
    XmlProcessingFailed(String),
    PakCreationFailed,
    IoError(std::io::Error),
}

impl fmt::Display for BilingualGeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // BilingualGeneratorError::GameNotFound => write!(f, "Game installation not found"),
            BilingualGeneratorError::InvalidBilingualSet(ref msg) => {
                write!(f, "Invalid Bilingual Set Format: {}", msg)
            }
            BilingualGeneratorError::PakExtractionFailed => {
                write!(f, "Failed to extract PAK files")
            }
            BilingualGeneratorError::XmlProcessingFailed(ref msg) => {
                write!(f, "XML processing failed: {}", msg)
            }

            BilingualGeneratorError::PakCreationFailed => {
                write!(f, "Failed to create new PAK file")
            }
            BilingualGeneratorError::IoError(err) => write!(f, "IO error: {}", err),
        }
    }
}

impl fmt::Debug for BilingualGeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // BilingualGeneratorError::GameNotFound => write!(f, "GameNotFound"),
            BilingualGeneratorError::InvalidBilingualSet(ref msg) => {
                write!(f, "Invalid Bilingual Set Format: {}", msg)
            }
            BilingualGeneratorError::PakExtractionFailed => write!(f, "PakExtractionFailed"),
            BilingualGeneratorError::XmlProcessingFailed(ref msg) => {
                write!(f, "XML processing failed: {}", msg)
            }
            BilingualGeneratorError::PakCreationFailed => write!(f, "PakCreationFailed"),
            BilingualGeneratorError::IoError(_) => write!(f, "IoError"),
        }
    }
}

impl std::error::Error for BilingualGeneratorError {}

impl From<io::Error> for BilingualGeneratorError {
    fn from(err: io::Error) -> BilingualGeneratorError {
        BilingualGeneratorError::IoError(err)
    }
}
