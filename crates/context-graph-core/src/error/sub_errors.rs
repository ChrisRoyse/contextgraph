//! Sub-error types for context-graph-core.
//!
//! Each error type covers a specific domain of failures.

use thiserror::Error;
use uuid::Uuid;

use crate::teleological::embedder::Embedder;

// ============================================================================
// EMBEDDING ERROR
// ============================================================================

/// Embedding-related errors.
///
/// Covers all failure modes for embedding generation, model management,
/// and vector validation.
#[derive(Debug, Error)]
pub enum EmbeddingError {
    /// Model not loaded for the specified embedder.
    ///
    /// # Recovery
    ///
    /// Wait for model to load via `UnifiedModelLoader::load_model()`.
    #[error("Model not loaded for embedder {0:?}")]
    ModelNotLoaded(Embedder),

    /// Embedding generation failed for a specific embedder.
    #[error("Embedding generation failed for {embedder:?}: {reason}")]
    GenerationFailed {
        /// The embedder that failed
        embedder: Embedder,
        /// Detailed reason for failure
        reason: String,
    },

    /// Quantization operation failed.
    #[error("Quantization error: {0}")]
    Quantization(String),

    /// Vector dimension does not match expected size.
    ///
    /// # When This Occurs
    ///
    /// - Providing embedding with wrong dimension
    /// - Mixing embeddings from different models
    /// - Corrupted embedding data
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension for this embedder
        expected: usize,
        /// Actual dimension received
        actual: usize,
    },

    /// Batch size exceeds maximum allowed.
    #[error("Batch too large: {size} exceeds max {max}")]
    BatchTooLarge {
        /// Requested batch size
        size: usize,
        /// Maximum allowed batch size
        max: usize,
    },

    /// Empty input text provided for embedding.
    #[error("Empty input text")]
    EmptyInput,

    /// Model warm-up failed during initialization.
    #[error("Model warm-up failed: {0}")]
    WarmupFailed(String),

    /// Model file not found at expected path.
    #[error("Model not found: {path}")]
    ModelNotFound {
        /// Path where model was expected
        path: String,
    },

    /// Tensor operation failed (candle/ONNX error).
    #[error("Tensor operation failed: {operation} - {message}")]
    TensorError {
        /// Operation that failed
        operation: String,
        /// Error message
        message: String,
    },

    /// Legacy embedding error where the specific embedder is unknown.
    ///
    /// Used when converting from `CoreError::Embedding(String)` which does not
    /// carry embedder identity. Preserves the original error message without
    /// falsely attributing the failure to a specific embedder.
    #[error("Embedding error (unknown embedder): {0}")]
    LegacyUnknownEmbedder(String),
}

// ============================================================================
// STORAGE ERROR
// ============================================================================

/// Storage-related errors.
///
/// Covers database operations, serialization, and data integrity issues.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Database operation failed.
    #[error("Database error: {0}")]
    Database(String),

    /// Serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Array/memory not found by ID.
    #[error("Array not found: {0}")]
    NotFound(Uuid),

    /// Array already exists (duplicate insert).
    #[error("Array already exists: {0}")]
    AlreadyExists(Uuid),

    /// TeleologicalArray is missing embedding for an embedder.
    ///
    /// # Constitution Compliance
    ///
    /// Per ARCH-05: "All 13 Embedders Must Be Present"
    #[error("Incomplete array: missing embedder {0:?}")]
    IncompleteArray(Embedder),

    /// Schema migration failed.
    #[error("Schema migration failed: {0}")]
    Migration(String),

    /// Data corruption detected.
    ///
    /// # Critical
    ///
    /// This is a critical error requiring investigation.
    #[error("Corruption detected: {0}")]
    Corruption(String),

    /// Transaction failed (can be retried).
    #[error("Transaction failed: {0}")]
    Transaction(String),

    /// Write operation failed.
    #[error("Write failed: {0}")]
    WriteFailed(String),

    /// Read operation failed.
    #[error("Read failed: {0}")]
    ReadFailed(String),
}

// ============================================================================
// INDEX ERROR
// ============================================================================

/// Index-related errors.
///
/// Covers HNSW index operations, search failures, and corruption.
#[derive(Debug, Error)]
pub enum IndexError {
    /// HNSW index operation failed.
    #[error("HNSW index error: {0}")]
    Hnsw(String),

    /// Inverted index operation failed.
    #[error("Inverted index error: {0}")]
    Inverted(String),

    /// Index not found for the specified embedder.
    #[error("Index not found for embedder {0:?}")]
    NotFound(Embedder),

    /// Index rebuild required (outdated or corrupted).
    #[error("Index rebuild required for embedder {0:?}")]
    RebuildRequired(Embedder),

    /// Index corruption detected.
    ///
    /// # Critical
    ///
    /// This is a critical error requiring index rebuild.
    #[error("Index corruption in embedder {0:?}: {1}")]
    Corruption(Embedder, String),

    /// Search operation timed out.
    ///
    /// # Recovery
    ///
    /// Can be retried with longer timeout.
    #[error("Search timeout after {0}ms")]
    Timeout(u64),

    /// Index construction failed.
    #[error("Index construction failed: dimension={dimension}, error={message}")]
    ConstructionFailed {
        /// Dimension of vectors
        dimension: usize,
        /// Error message
        message: String,
    },

    /// Vector insertion failed.
    #[error("Vector insertion failed for {memory_id}: {message}")]
    InsertionFailed {
        /// Memory ID that failed
        memory_id: Uuid,
        /// Error message
        message: String,
    },
}

// ============================================================================
// CONFIG ERROR
// ============================================================================

/// Configuration errors.
///
/// Covers missing configs, invalid values, and environment issues.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Required configuration is missing.
    #[error("Missing configuration: {0}")]
    Missing(String),

    /// Configuration value is invalid.
    #[error("Invalid configuration: {field}: {reason}")]
    Invalid {
        /// Configuration field name
        field: String,
        /// Reason why it's invalid
        reason: String,
    },

    /// Required environment variable is not set.
    #[error("Environment variable not set: {0}")]
    EnvNotSet(String),

    /// Configuration file not found.
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// Configuration file parse error.
    #[error("Parse error in {file}: {reason}")]
    ParseError {
        /// File being parsed
        file: String,
        /// Parse error reason
        reason: String,
    },
}

// ============================================================================
// GPU ERROR
// ============================================================================

/// GPU/CUDA errors.
///
/// Covers device initialization, memory management, and kernel execution.
#[derive(Debug, Error)]
pub enum GpuError {
    /// No GPU device available.
    ///
    /// # Critical
    ///
    /// Per ARCH-08: "CUDA GPU is Required for Production"
    #[error("No GPU available")]
    NotAvailable,

    /// GPU out of memory.
    ///
    /// # Recovery
    ///
    /// Can be retried after garbage collection or reducing batch size.
    #[error("GPU out of memory: requested {requested} bytes, available {available} bytes")]
    OutOfMemory {
        /// Bytes requested
        requested: u64,
        /// Bytes available
        available: u64,
    },

    /// CUDA operation failed.
    #[error("CUDA error: {0}")]
    CudaError(String),

    /// Device initialization failed.
    #[error("Device initialization failed: {0}")]
    InitFailed(String),

    /// Kernel launch failed.
    #[error("Kernel launch failed: {0}")]
    KernelFailed(String),
}

// ============================================================================
// MCP ERROR
// ============================================================================

/// MCP protocol errors.
///
/// Covers request validation, authorization, and protocol violations.
#[derive(Debug, Error)]
pub enum McpError {
    /// Invalid JSON-RPC request.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Method not found in MCP protocol.
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// Invalid parameters for MCP method.
    #[error("Invalid params: {0}")]
    InvalidParams(String),

    /// Rate limit exceeded.
    ///
    /// # Recovery
    ///
    /// Can be retried after backoff period.
    #[error("Rate limited: {0}")]
    RateLimited(String),

    /// Authorization failed.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Session expired.
    #[error("Session expired")]
    SessionExpired,

    /// PII detected in input (security violation).
    ///
    /// Per SEC-02: "Scrub PII pre-embed"
    #[error("PII detected")]
    PiiDetected,
}

impl McpError {
    /// Get JSON-RPC error code for this MCP error.
    ///
    /// Maps to standard JSON-RPC 2.0 codes and Context Graph extensions.
    #[inline]
    pub fn error_code(&self) -> i32 {
        match self {
            Self::InvalidRequest(_) => -32600, // INVALID_REQUEST
            Self::MethodNotFound(_) => -32601, // METHOD_NOT_FOUND
            Self::InvalidParams(_) => -32602,  // INVALID_PARAMS
            Self::RateLimited(_) => -32005,    // RATE_LIMITED
            Self::Unauthorized(_) => -32006,   // UNAUTHORIZED
            Self::SessionExpired => -32000,    // SESSION_NOT_FOUND
            Self::PiiDetected => -32007,       // PII_DETECTED
        }
    }
}
