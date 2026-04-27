//! Error types for code storage operations.

use thiserror::Error;
use uuid::Uuid;

/// Result type for code storage operations.
pub type CodeStorageResult<T> = Result<T, CodeStorageError>;

/// Errors that can occur during code storage operations.
#[derive(Debug, Error)]
pub enum CodeStorageError {
    /// RocksDB operation failed.
    #[error("RocksDB error: {0}")]
    RocksDb(#[from] rocksdb::Error),

    /// Serialization failed.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization failed.
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Entity not found.
    #[error("Code entity not found: {id}")]
    NotFound { id: Uuid },

    /// Embedding not found for entity.
    #[error("Embedding not found for entity: {id}")]
    EmbeddingNotFound { id: Uuid },

    /// File not found in index.
    #[error("File not found in index: {path}")]
    FileNotFound { path: String },

    /// Invalid embedding dimension.
    #[error("Invalid embedding dimension: expected {expected}, got {actual}")]
    InvalidDimension { expected: usize, actual: usize },

    /// Column family not found.
    #[error("Column family not found: {name}")]
    ColumnFamilyNotFound { name: String },

    /// Store not initialized.
    #[error("Code store not initialized")]
    NotInitialized,

    /// Invalid key format.
    #[error("Invalid key format: {0}")]
    InvalidKey(String),

    /// Duplicate entity.
    #[error("Entity already exists: {id}")]
    Duplicate { id: Uuid },

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl CodeStorageError {
    /// Create a serialization error.
    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Serialization(msg.into())
    }

    /// Create a deserialization error.
    pub fn deserialization(msg: impl Into<String>) -> Self {
        Self::Deserialization(msg.into())
    }

    /// Create a not found error.
    pub fn not_found(id: Uuid) -> Self {
        Self::NotFound { id }
    }

    /// Create an embedding not found error.
    pub fn embedding_not_found(id: Uuid) -> Self {
        Self::EmbeddingNotFound { id }
    }

    /// Create a column family not found error.
    pub fn cf_not_found(name: impl Into<String>) -> Self {
        Self::ColumnFamilyNotFound { name: name.into() }
    }
}
