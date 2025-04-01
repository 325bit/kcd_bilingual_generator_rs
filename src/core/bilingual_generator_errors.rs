use std::{fmt, io};

pub enum BilingualGeneratorError {
    GameNotFound,
    PakExtractionFailed,
    XmlProcessingFailed,
    PakCreationFailed,
    IoError(std::io::Error),
}

impl fmt::Display for BilingualGeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BilingualGeneratorError::GameNotFound => write!(f, "Game installation not found"),
            BilingualGeneratorError::PakExtractionFailed => {
                write!(f, "Failed to extract PAK files")
            }
            BilingualGeneratorError::XmlProcessingFailed => {
                write!(f, "Failed to process XML files")
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
            BilingualGeneratorError::GameNotFound => write!(f, "GameNotFound"),
            BilingualGeneratorError::PakExtractionFailed => write!(f, "PakExtractionFailed"),
            BilingualGeneratorError::XmlProcessingFailed => write!(f, "XmlProcessingFailed"),
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
