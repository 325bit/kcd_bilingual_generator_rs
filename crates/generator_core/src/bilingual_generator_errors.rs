use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BilingualGeneratorError {
    #[error("Invalid Bilingual Set Format: {0}")]
    InvalidBilingualSet(String),

    #[error("Failed to extract PAK files")]
    PakExtractionFailed,

    // PakExtractionFailed更详细的变体
    #[error("PAK operation failed: {operation} for '{context}' due to: {source}")]
    PakOperationFailed {
        operation: String, // 操作类型 (e.g., "opening PAK file", "creating ZipArchive")
        context: String,   // 相关路径或文件名
        source: io::Error, // 底层 IO 错误
    },

    #[error("XML processing failed: {0}")]
    XmlProcessingFailed(String),

    #[error("Failed to create new PAK file")]
    PakCreationFailed, // 这个可能也需要细化，但目前只处理读取部分

    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("TaskJoinError: {0}")]
    TaskJoinError(String), // lang_str and join_err
}
