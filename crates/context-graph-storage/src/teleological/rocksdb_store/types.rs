//! Types, errors, and configuration for RocksDbTeleologicalStore.
//!
//! This module defines the error types, configuration structs, and result aliases
//! used throughout the RocksDB teleological storage implementation.
//!
//! # FAIL FAST Policy
//!
//! All error types include detailed context for immediate debugging:
//! - Operation name
//! - Column family
//! - Key (if applicable)
//! - Underlying cause

use context_graph_core::error::CoreError;
use context_graph_core::teleological::ComparisonValidationError;
use thiserror::Error;
use uuid::Uuid;

// ============================================================================
// Error Types - FAIL FAST with detailed context
// ============================================================================

/// Detailed error type for teleological store operations.
///
/// Every error includes enough context for immediate debugging:
/// - Operation name
/// - Column family
/// - Key (if applicable)
/// - Underlying cause
#[derive(Debug, Error)]
pub enum TeleologicalStoreError {
    /// RocksDB operation failed.
    #[error("RocksDB {operation} failed on CF '{cf}' with key '{key:?}': {source}")]
    RocksDbOperation {
        operation: &'static str,
        cf: &'static str,
        key: Option<String>,
        #[source]
        source: rocksdb::Error,
    },

    /// Database failed to open.
    #[error("Failed to open RocksDB at '{path}': {message}")]
    OpenFailed { path: String, message: String },

    /// Column family not found.
    #[error("Column family '{name}' not found in database")]
    ColumnFamilyNotFound { name: String },

    /// Serialization error.
    #[error("Serialization error for fingerprint {id:?}: {message}")]
    Serialization { id: Option<Uuid>, message: String },

    /// Deserialization error.
    #[error("Deserialization error for key '{key}': {message}")]
    Deserialization { key: String, message: String },

    /// Fingerprint validation failed.
    #[error("Validation error for fingerprint {id:?}: {message}")]
    Validation { id: Option<Uuid>, message: String },

    /// Index operation failed.
    #[error("Index operation failed on '{index_name}': {message}")]
    IndexOperation { index_name: String, message: String },

    /// Checkpoint operation failed.
    #[error("Checkpoint operation failed: {message}")]
    CheckpointFailed { message: String },

    /// Restore operation failed.
    #[error("Restore operation failed from '{path}': {message}")]
    RestoreFailed { path: String, message: String },

    /// Stale lock detected and could not be cleaned.
    #[error("Stale lock detected at '{path}' but cleanup failed: {message}")]
    StaleLockCleanupFailed { path: String, message: String },

    /// Database corruption detected - FAIL FAST.
    ///
    /// This error is raised when MANIFEST references SST files that don't exist,
    /// indicating database corruption (likely from unclean shutdown during
    /// compaction or flush).
    ///
    /// # Recovery
    ///
    /// Manual intervention required. Options:
    /// 1. Delete corrupted database and recreate from backup
    /// 2. Use `ldb repair` tool (may lose some data)
    /// 3. Restore from a known-good snapshot
    ///
    /// NO automatic recovery is attempted - system fails fast for explicit debugging.
    #[error("CORRUPTION DETECTED at '{path}': MANIFEST references {missing_count} missing SST files: [{missing_files}]. \
             Manual intervention required. See error details for recovery options.")]
    CorruptionDetected {
        /// Database path
        path: String,
        /// Number of missing SST files
        missing_count: usize,
        /// Comma-separated list of missing SST file names
        missing_files: String,
        /// MANIFEST file that references the missing files
        manifest_file: String,
    },

    /// Internal error (should never happen).
    #[error("Internal error: {0}")]
    Internal(String),

    /// Comparison validation error (from TASK-CORE-004).
    ///
    /// This error occurs when fingerprint comparison types fail validation,
    /// such as weights not summing to 1.0 or synergy matrix symmetry violations.
    #[error("Comparison validation failed: {0}")]
    ComparisonValidation(#[from] ComparisonValidationError),
}

impl TeleologicalStoreError {
    /// Create a RocksDB operation error.
    pub fn rocksdb_op(
        operation: &'static str,
        cf: &'static str,
        key: Option<Uuid>,
        source: rocksdb::Error,
    ) -> Self {
        Self::RocksDbOperation {
            operation,
            cf,
            key: key.map(|k| k.to_string()),
            source,
        }
    }
}

impl From<TeleologicalStoreError> for CoreError {
    fn from(e: TeleologicalStoreError) -> Self {
        CoreError::StorageError(e.to_string())
    }
}

impl From<CoreError> for TeleologicalStoreError {
    fn from(e: CoreError) -> Self {
        TeleologicalStoreError::Internal(e.to_string())
    }
}

/// Result type for teleological store operations.
pub type TeleologicalStoreResult<T> = Result<T, TeleologicalStoreError>;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for RocksDbTeleologicalStore.
#[derive(Debug, Clone)]
pub struct TeleologicalStoreConfig {
    /// Block cache size in bytes (default: 256MB).
    pub block_cache_size: usize,
    /// Maximum number of open files (default: 1000).
    pub max_open_files: i32,
    /// Enable WAL (write-ahead log) for durability (default: true).
    pub enable_wal: bool,
    /// Create database if it doesn't exist (default: true).
    pub create_if_missing: bool,
    /// Create missing column families if the database exists but is incomplete (default: true).
    pub create_missing_column_families: bool,
    /// Soft-delete garbage collection retention period in seconds.
    /// Soft-deleted entries older than this are permanently hard-deleted by GC.
    /// Default: 7 days (604800 seconds). Set to 0 to GC immediately on next run.
    pub gc_retention_secs: u64,
}

impl Default for TeleologicalStoreConfig {
    fn default() -> Self {
        Self {
            block_cache_size: 256 * 1024 * 1024, // 256MB
            max_open_files: 1000,
            enable_wal: true,
            create_if_missing: true,
            create_missing_column_families: true,
            gc_retention_secs: 7 * 24 * 3600, // 7 days
        }
    }
}
