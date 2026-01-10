//! Error types for context-graph-core.
//!
//! This module defines the central error types used throughout the context-graph system:
//!
//! - [`ContextGraphError`]: Top-level unified error for all crate errors
//! - [`CoreError`]: Legacy error type (retained for compatibility)
//! - Sub-error types: [`EmbeddingError`], [`StorageError`], [`IndexError`],
//!   [`ConfigError`], [`GpuError`], [`McpError`]
//!
//! # Constitution Compliance
//!
//! Per constitution.yaml rust_standards/error_handling (lines 136-141):
//! - Use `thiserror` for library error types
//! - Never panic in library code; return Result
//! - Propagate errors with `?` operator
//! - Add context with `.context()` or `.with_context()`
//!
//! Per AP-14: No `.unwrap()` in library code - Use `.expect()` with context or return Result
//!
//! # Examples
//!
//! ```rust
//! use context_graph_core::error::{ContextGraphError, EmbeddingError, Result};
//! use context_graph_core::Embedder;
//!
//! fn generate_embedding(text: &str) -> Result<Vec<f32>> {
//!     if text.is_empty() {
//!         return Err(ContextGraphError::Embedding(EmbeddingError::EmptyInput));
//!     }
//!     // ... embedding logic
//!     Ok(vec![0.0; 1024])
//! }
//!
//! let result = generate_embedding("");
//! assert!(matches!(
//!     result,
//!     Err(ContextGraphError::Embedding(EmbeddingError::EmptyInput))
//! ));
//! ```

use thiserror::Error;
use uuid::Uuid;

use crate::teleological::embedder::Embedder;

// ============================================================================
// TOP-LEVEL UNIFIED ERROR TYPE
// ============================================================================

/// Top-level unified error type for context-graph library.
///
/// All crate errors should be convertible to this type via `From` implementations.
/// Provides JSON-RPC error code mapping for MCP protocol responses.
///
/// # JSON-RPC Error Codes
///
/// Each error variant maps to a JSON-RPC error code:
/// - `-32600` to `-32603`: Standard JSON-RPC errors
/// - `-32001` to `-32007`: Context Graph specific errors
/// - `-32008`: INDEX_ERROR
/// - `-32009`: GPU_ERROR
///
/// # Recoverability
///
/// Errors are classified as recoverable or non-recoverable:
/// - Recoverable: Can be retried (e.g., rate limiting, model not loaded)
/// - Non-recoverable: Require intervention (e.g., corruption, config errors)
///
/// # Examples
///
/// ```rust
/// use context_graph_core::error::{ContextGraphError, EmbeddingError};
/// use context_graph_core::Embedder;
///
/// let err = ContextGraphError::Embedding(EmbeddingError::ModelNotLoaded(Embedder::Semantic));
/// assert_eq!(err.error_code(), -32005);
/// assert!(err.is_recoverable());
/// assert!(!err.is_critical());
/// ```
#[derive(Debug, Error)]
pub enum ContextGraphError {
    /// Embedding-related error.
    ///
    /// Covers model loading, generation, quantization, and dimension issues.
    #[error("Embedding error: {0}")]
    Embedding(#[from] EmbeddingError),

    /// Storage-related error.
    ///
    /// Covers database operations, serialization, and data integrity.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// Index-related error.
    ///
    /// Covers HNSW index operations, search failures, and corruption.
    #[error("Index error: {0}")]
    Index(#[from] IndexError),

    /// Configuration error.
    ///
    /// Covers missing configs, invalid values, and parse failures.
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// GPU/CUDA error.
    ///
    /// Covers device initialization, memory, and kernel failures.
    #[error("GPU error: {0}")]
    Gpu(#[from] GpuError),

    /// MCP protocol error.
    ///
    /// Covers request validation, authorization, and protocol violations.
    #[error("MCP error: {0}")]
    Mcp(#[from] McpError),

    /// Validation error for input data.
    ///
    /// # When This Occurs
    ///
    /// - Field value out of allowed range
    /// - Invalid format for parameters
    /// - NaN or Infinity in numeric fields
    #[error("Validation error: {0}")]
    Validation(String),

    /// Internal error indicating a bug or system failure.
    ///
    /// # When This Occurs
    ///
    /// - Invariant violation detected
    /// - Unrecoverable state corruption
    /// - Resource exhaustion
    ///
    /// These errors indicate bugs and should be investigated.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ContextGraphError {
    /// Get JSON-RPC error code for MCP responses.
    ///
    /// Maps to codes defined in `crates/context-graph-mcp/src/protocol.rs`.
    ///
    /// # Returns
    ///
    /// Negative i32 error code per JSON-RPC 2.0 specification.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use context_graph_core::error::{ContextGraphError, EmbeddingError, McpError};
    /// use context_graph_core::Embedder;
    ///
    /// let err = ContextGraphError::Embedding(EmbeddingError::EmptyInput);
    /// assert_eq!(err.error_code(), -32005);
    ///
    /// let err = ContextGraphError::Validation("bad input".to_string());
    /// assert_eq!(err.error_code(), -32602);
    /// ```
    #[inline]
    pub fn error_code(&self) -> i32 {
        match self {
            Self::Embedding(_) => -32005, // EMBEDDING_ERROR
            Self::Storage(_) => -32004,   // STORAGE_ERROR
            Self::Index(_) => -32008,     // INDEX_ERROR (new)
            Self::Config(_) => -32603,    // INTERNAL_ERROR (config is internal)
            Self::Gpu(_) => -32009,       // GPU_ERROR (new)
            Self::Mcp(e) => e.error_code(),
            Self::Validation(_) => -32602, // INVALID_PARAMS
            Self::Internal(_) => -32603,   // INTERNAL_ERROR
        }
    }

    /// Check if this error is recoverable via retry.
    ///
    /// Recoverable errors can potentially succeed if retried with:
    /// - Waiting for model to load
    /// - Retrying after backoff (rate limiting)
    /// - Retrying after garbage collection (OOM)
    /// - Retrying transaction
    ///
    /// # Returns
    ///
    /// `true` if retry might succeed, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use context_graph_core::error::{ContextGraphError, EmbeddingError, StorageError};
    /// use context_graph_core::Embedder;
    ///
    /// // Recoverable: model can be loaded
    /// let err = ContextGraphError::Embedding(EmbeddingError::ModelNotLoaded(Embedder::Semantic));
    /// assert!(err.is_recoverable());
    ///
    /// // Not recoverable: data corruption
    /// let err = ContextGraphError::Storage(StorageError::Corruption("bad data".to_string()));
    /// assert!(!err.is_recoverable());
    /// ```
    #[inline]
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Embedding(EmbeddingError::ModelNotLoaded(_))
                | Self::Storage(StorageError::Transaction(_))
                | Self::Index(IndexError::Timeout(_))
                | Self::Mcp(McpError::RateLimited(_))
                | Self::Gpu(GpuError::OutOfMemory { .. })
        )
    }

    /// Check if this error indicates a critical system issue.
    ///
    /// Critical errors indicate system health problems that require
    /// immediate attention and should be logged at ERROR level.
    ///
    /// # Returns
    ///
    /// `true` if this is a critical error, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use context_graph_core::error::{ContextGraphError, StorageError, IndexError, GpuError};
    /// use context_graph_core::Embedder;
    ///
    /// // Critical: data corruption
    /// let err = ContextGraphError::Storage(StorageError::Corruption("bad".to_string()));
    /// assert!(err.is_critical());
    ///
    /// // Critical: index corruption
    /// let err = ContextGraphError::Index(IndexError::Corruption(
    ///     Embedder::Semantic,
    ///     "checksum mismatch".to_string()
    /// ));
    /// assert!(err.is_critical());
    ///
    /// // Not critical: validation error
    /// let err = ContextGraphError::Validation("bad input".to_string());
    /// assert!(!err.is_critical());
    /// ```
    #[inline]
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::Storage(StorageError::Corruption(_))
                | Self::Index(IndexError::Corruption(_, _))
                | Self::Gpu(GpuError::NotAvailable)
                | Self::Internal(_)
        )
    }

    /// Create an internal error from a message.
    ///
    /// Convenience method for creating internal errors.
    #[inline]
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Create a validation error from a message.
    ///
    /// Convenience method for creating validation errors.
    #[inline]
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }
}

// ============================================================================
// SUB-ERROR TYPES
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
}

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
            Self::InvalidRequest(_) => -32600,  // INVALID_REQUEST
            Self::MethodNotFound(_) => -32601,  // METHOD_NOT_FOUND
            Self::InvalidParams(_) => -32602,   // INVALID_PARAMS
            Self::RateLimited(_) => -32005,     // RATE_LIMITED
            Self::Unauthorized(_) => -32006,    // UNAUTHORIZED
            Self::SessionExpired => -32000,     // SESSION_NOT_FOUND
            Self::PiiDetected => -32007,        // PII_DETECTED
        }
    }
}

// ============================================================================
// RESULT TYPE ALIAS
// ============================================================================

/// Result type alias for context-graph operations.
///
/// # Examples
///
/// ```rust
/// use context_graph_core::error::{Result, ContextGraphError};
///
/// fn example_operation() -> Result<String> {
///     Ok("success".to_string())
/// }
///
/// fn failing_operation() -> Result<String> {
///     Err(ContextGraphError::validation("invalid input"))
/// }
/// ```
pub type Result<T> = std::result::Result<T, ContextGraphError>;

// ============================================================================
// FROM IMPLEMENTATIONS - Connect to existing error types
// ============================================================================

/// Convert from `context_graph_core::index::error::IndexError`
impl From<crate::index::error::IndexError> for IndexError {
    fn from(e: crate::index::error::IndexError) -> Self {
        use crate::index::error::IndexError as ExistingIndexError;

        match e {
            ExistingIndexError::DimensionMismatch {
                embedder,
                expected,
                actual,
            } => {
                // Convert EmbedderIndex to Embedder
                let embedder = embedder_index_to_embedder(embedder);
                IndexError::Hnsw(format!(
                    "Dimension mismatch for {:?}: expected {}, got {}",
                    embedder, expected, actual
                ))
            }
            ExistingIndexError::InvalidEmbedder { embedder } => {
                let embedder = embedder_index_to_embedder(embedder);
                IndexError::NotFound(embedder)
            }
            ExistingIndexError::NotInitialized { embedder } => {
                let embedder = embedder_index_to_embedder(embedder);
                IndexError::RebuildRequired(embedder)
            }
            ExistingIndexError::IndexEmpty { embedder } => {
                let embedder = embedder_index_to_embedder(embedder);
                IndexError::RebuildRequired(embedder)
            }
            ExistingIndexError::InvalidTermId { term_id, vocab_size } => {
                IndexError::Inverted(format!(
                    "Invalid term_id {} (vocab_size={})",
                    term_id, vocab_size
                ))
            }
            ExistingIndexError::ZeroNormVector { memory_id } => IndexError::InsertionFailed {
                memory_id,
                message: "Zero-norm vector".to_string(),
            },
            ExistingIndexError::NotFound { memory_id } => IndexError::InsertionFailed {
                memory_id,
                message: "Memory not found in indexes".to_string(),
            },
            ExistingIndexError::StorageError { context, message } => {
                IndexError::Hnsw(format!("{}: {}", context, message))
            }
            ExistingIndexError::CorruptedIndex { path } => {
                IndexError::Corruption(Embedder::Semantic, format!("Corrupted file: {}", path))
            }
            ExistingIndexError::IoError { context, message } => {
                IndexError::Hnsw(format!("IO error - {}: {}", context, message))
            }
            ExistingIndexError::SerializationError { context, message } => {
                IndexError::Hnsw(format!("Serialization - {}: {}", context, message))
            }
            ExistingIndexError::HnswConstructionFailed {
                dimension, message, ..
            } => IndexError::ConstructionFailed { dimension, message },
            ExistingIndexError::HnswInsertionFailed {
                memory_id, message, ..
            } => IndexError::InsertionFailed { memory_id, message },
            ExistingIndexError::HnswSearchFailed { message, .. } => IndexError::Hnsw(message),
            ExistingIndexError::HnswPersistenceFailed { message, .. } => IndexError::Hnsw(message),
            ExistingIndexError::HnswInternalError { message, .. } => IndexError::Hnsw(message),
            ExistingIndexError::LegacyFormatRejected { path, message } => {
                IndexError::Corruption(Embedder::Semantic, format!("Legacy: {} - {}", path, message))
            }
        }
    }
}

/// Helper to convert EmbedderIndex to Embedder
fn embedder_index_to_embedder(idx: crate::index::config::EmbedderIndex) -> Embedder {
    use crate::index::config::EmbedderIndex;

    match idx {
        EmbedderIndex::E1Semantic => Embedder::Semantic,
        EmbedderIndex::E1Matryoshka128 => Embedder::Semantic, // Truncated E1
        EmbedderIndex::E2TemporalRecent => Embedder::TemporalRecent,
        EmbedderIndex::E3TemporalPeriodic => Embedder::TemporalPeriodic,
        EmbedderIndex::E4TemporalPositional => Embedder::TemporalPositional,
        EmbedderIndex::E5Causal => Embedder::Causal,
        EmbedderIndex::E6Sparse => Embedder::Sparse,
        EmbedderIndex::E7Code => Embedder::Code,
        EmbedderIndex::E8Graph => Embedder::Graph,
        EmbedderIndex::E9HDC => Embedder::Hdc,
        EmbedderIndex::E10Multimodal => Embedder::Multimodal,
        EmbedderIndex::E11Entity => Embedder::Entity,
        EmbedderIndex::E12LateInteraction => Embedder::LateInteraction,
        EmbedderIndex::E13Splade => Embedder::KeywordSplade,
        EmbedderIndex::PurposeVector => Embedder::Semantic, // Purpose vector uses semantic as proxy for errors
    }
}

/// Convert from existing index::error::IndexError to ContextGraphError
impl From<crate::index::error::IndexError> for ContextGraphError {
    fn from(e: crate::index::error::IndexError) -> Self {
        ContextGraphError::Index(e.into())
    }
}

// ============================================================================
// LEGACY CORE ERROR (RETAINED FOR COMPATIBILITY)
// ============================================================================

/// Legacy error type for context-graph-core operations.
///
/// # Deprecation Notice
///
/// This type is retained for backwards compatibility. New code should use
/// [`ContextGraphError`] instead.
///
/// # Examples
///
/// ```rust
/// use context_graph_core::CoreError;
/// use uuid::Uuid;
///
/// fn lookup_node(id: Uuid) -> Result<(), CoreError> {
///     Err(CoreError::NodeNotFound { id })
/// }
///
/// let result = lookup_node(Uuid::nil());
/// assert!(result.is_err());
/// ```
#[derive(Debug, Error)]
pub enum CoreError {
    /// A requested node was not found in the graph.
    #[error("Node not found: {id}")]
    NodeNotFound { id: Uuid },

    /// Embedding vector dimension does not match expected size.
    #[error("Invalid embedding dimension: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// A field value failed validation constraints.
    #[error("Validation error: {field} - {message}")]
    ValidationError { field: String, message: String },

    /// An error occurred during storage operations.
    #[error("Storage error: {0}")]
    StorageError(String),

    /// An error occurred with index operations.
    #[error("Index error: {0}")]
    IndexError(String),

    /// Configuration is invalid or missing.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Error during UTL computation.
    #[error("UTL computation error: {0}")]
    UtlError(String),

    /// Error during nervous system layer processing.
    #[error("Layer processing error in {layer}: {message}")]
    LayerError { layer: String, message: String },

    /// Requested feature is disabled.
    #[error("Feature disabled: {feature}")]
    FeatureDisabled { feature: String },

    /// Serialization error.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// Embedding error.
    #[error("Embedding error: {0}")]
    Embedding(String),

    /// Missing required field.
    #[error("Missing required field '{field}': {context}")]
    MissingField { field: String, context: String },

    /// Not implemented.
    #[error("Not implemented: {0}. See documentation for implementation guide.")]
    NotImplemented(String),

    /// Legacy format rejected.
    #[error("Legacy format rejected: {0}. See documentation for migration guide.")]
    LegacyFormatRejected(String),
}

impl From<serde_json::Error> for CoreError {
    fn from(err: serde_json::Error) -> Self {
        CoreError::SerializationError(err.to_string())
    }
}

impl From<config::ConfigError> for CoreError {
    fn from(err: config::ConfigError) -> Self {
        CoreError::ConfigError(err.to_string())
    }
}

/// Convert CoreError to ContextGraphError for interoperability.
impl From<CoreError> for ContextGraphError {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::NodeNotFound { id } => {
                ContextGraphError::Storage(StorageError::NotFound(id))
            }
            CoreError::DimensionMismatch { expected, actual } => {
                ContextGraphError::Embedding(EmbeddingError::DimensionMismatch { expected, actual })
            }
            CoreError::ValidationError { field, message } => {
                ContextGraphError::Validation(format!("{}: {}", field, message))
            }
            CoreError::StorageError(msg) => {
                ContextGraphError::Storage(StorageError::Database(msg))
            }
            CoreError::IndexError(msg) => ContextGraphError::Index(IndexError::Hnsw(msg)),
            CoreError::ConfigError(msg) => ContextGraphError::Config(ConfigError::Missing(msg)),
            CoreError::UtlError(msg) => ContextGraphError::Internal(format!("UTL: {}", msg)),
            CoreError::LayerError { layer, message } => {
                ContextGraphError::Internal(format!("Layer {}: {}", layer, message))
            }
            CoreError::FeatureDisabled { feature } => {
                ContextGraphError::Config(ConfigError::Missing(feature))
            }
            CoreError::SerializationError(msg) => {
                ContextGraphError::Storage(StorageError::Serialization(msg))
            }
            CoreError::Internal(msg) => ContextGraphError::Internal(msg),
            CoreError::Embedding(msg) => {
                ContextGraphError::Embedding(EmbeddingError::GenerationFailed {
                    embedder: Embedder::Semantic, // default
                    reason: msg,
                })
            }
            CoreError::MissingField { field, context } => {
                ContextGraphError::Validation(format!("Missing {}: {}", field, context))
            }
            CoreError::NotImplemented(msg) => ContextGraphError::Internal(format!("Not implemented: {}", msg)),
            CoreError::LegacyFormatRejected(msg) => {
                ContextGraphError::Storage(StorageError::Migration(format!("Legacy: {}", msg)))
            }
        }
    }
}

/// Result type alias for core operations (legacy).
pub type CoreResult<T> = std::result::Result<T, CoreError>;

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========== ContextGraphError Tests ==========

    #[test]
    fn test_context_graph_error_codes() {
        // Test all error variants have correct codes
        let e = ContextGraphError::Embedding(EmbeddingError::EmptyInput);
        assert_eq!(e.error_code(), -32005);

        let e = ContextGraphError::Storage(StorageError::Database("test".to_string()));
        assert_eq!(e.error_code(), -32004);

        let e = ContextGraphError::Index(IndexError::Hnsw("test".to_string()));
        assert_eq!(e.error_code(), -32008);

        let e = ContextGraphError::Config(ConfigError::Missing("test".to_string()));
        assert_eq!(e.error_code(), -32603);

        let e = ContextGraphError::Gpu(GpuError::NotAvailable);
        assert_eq!(e.error_code(), -32009);

        let e = ContextGraphError::Mcp(McpError::InvalidParams("test".to_string()));
        assert_eq!(e.error_code(), -32602);

        let e = ContextGraphError::Validation("test".to_string());
        assert_eq!(e.error_code(), -32602);

        let e = ContextGraphError::Internal("test".to_string());
        assert_eq!(e.error_code(), -32603);

        println!("[PASS] All error codes mapped correctly");
    }

    #[test]
    fn test_is_recoverable() {
        // Recoverable errors
        let e = ContextGraphError::Embedding(EmbeddingError::ModelNotLoaded(Embedder::Semantic));
        assert!(e.is_recoverable());

        let e = ContextGraphError::Storage(StorageError::Transaction("test".to_string()));
        assert!(e.is_recoverable());

        let e = ContextGraphError::Index(IndexError::Timeout(1000));
        assert!(e.is_recoverable());

        let e = ContextGraphError::Mcp(McpError::RateLimited("429".to_string()));
        assert!(e.is_recoverable());

        let e = ContextGraphError::Gpu(GpuError::OutOfMemory {
            requested: 1000,
            available: 500,
        });
        assert!(e.is_recoverable());

        // Non-recoverable errors
        let e = ContextGraphError::Validation("bad".to_string());
        assert!(!e.is_recoverable());

        let e = ContextGraphError::Storage(StorageError::Corruption("bad".to_string()));
        assert!(!e.is_recoverable());

        println!("[PASS] is_recoverable() works correctly");
    }

    #[test]
    fn test_is_critical() {
        // Critical errors
        let e = ContextGraphError::Storage(StorageError::Corruption("bad".to_string()));
        assert!(e.is_critical());

        let e = ContextGraphError::Index(IndexError::Corruption(
            Embedder::Semantic,
            "checksum".to_string(),
        ));
        assert!(e.is_critical());

        let e = ContextGraphError::Gpu(GpuError::NotAvailable);
        assert!(e.is_critical());

        let e = ContextGraphError::Internal("bug".to_string());
        assert!(e.is_critical());

        // Non-critical errors
        let e = ContextGraphError::Validation("bad".to_string());
        assert!(!e.is_critical());

        let e = ContextGraphError::Embedding(EmbeddingError::EmptyInput);
        assert!(!e.is_critical());

        println!("[PASS] is_critical() works correctly");
    }

    #[test]
    fn test_mcp_error_codes() {
        assert_eq!(McpError::InvalidRequest("".to_string()).error_code(), -32600);
        assert_eq!(McpError::MethodNotFound("".to_string()).error_code(), -32601);
        assert_eq!(McpError::InvalidParams("".to_string()).error_code(), -32602);
        assert_eq!(McpError::RateLimited("".to_string()).error_code(), -32005);
        assert_eq!(McpError::Unauthorized("".to_string()).error_code(), -32006);
        assert_eq!(McpError::SessionExpired.error_code(), -32000);
        assert_eq!(McpError::PiiDetected.error_code(), -32007);

        println!("[PASS] McpError codes correct");
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn edge_case_empty_validation_message() {
        println!("=== BEFORE: Empty validation message ===");
        let e = ContextGraphError::Validation("".to_string());
        println!("Error: {}", e);
        println!("Code: {}", e.error_code());
        println!("=== AFTER: Should display 'Validation error: ' ===");
        assert!(e.to_string().contains("Validation"));
        println!("[PASS] Empty message handled");
    }

    #[test]
    fn edge_case_unicode_in_error() {
        println!("=== BEFORE: Unicode in error message ===");
        let e = ContextGraphError::Internal("错误消息 🔥".to_string());
        println!("Error: {}", e);
        println!("=== AFTER: Unicode should be preserved ===");
        assert!(e.to_string().contains("🔥"));
        assert!(e.to_string().contains("错误"));
        println!("[PASS] Unicode preserved");
    }

    #[test]
    fn edge_case_nested_from_conversion() {
        println!("=== BEFORE: From conversion chain ===");
        let storage_err = StorageError::Database("connection lost".to_string());
        let ctx_err: ContextGraphError = storage_err.into();
        println!("Original: connection lost");
        println!("Converted: {}", ctx_err);
        println!("=== AFTER: Message should contain 'connection lost' ===");
        assert!(ctx_err.to_string().contains("connection lost"));
        println!("[PASS] From conversion preserves message");
    }

    #[test]
    fn edge_case_all_embedders_in_errors() {
        println!("=== Testing all 13 embedders in errors ===");
        for embedder in Embedder::all() {
            let e = EmbeddingError::ModelNotLoaded(embedder);
            assert!(e.to_string().contains(&format!("{:?}", embedder)));
        }
        println!("[PASS] All 13 embedders work in errors");
    }

    #[test]
    fn edge_case_zero_values() {
        // Test with zero/nil values
        let e = ContextGraphError::Storage(StorageError::NotFound(Uuid::nil()));
        assert!(e.to_string().contains("00000000"));

        let e = ContextGraphError::Index(IndexError::Timeout(0));
        assert!(e.to_string().contains("0ms"));

        let e = ContextGraphError::Gpu(GpuError::OutOfMemory {
            requested: 0,
            available: 0,
        });
        assert!(e.to_string().contains("0 bytes"));

        println!("[PASS] Zero values handled");
    }

    #[test]
    fn test_convenience_constructors() {
        let e = ContextGraphError::internal("bug found");
        assert!(matches!(e, ContextGraphError::Internal(_)));
        assert!(e.to_string().contains("bug found"));

        let e = ContextGraphError::validation("bad input");
        assert!(matches!(e, ContextGraphError::Validation(_)));
        assert!(e.to_string().contains("bad input"));

        println!("[PASS] Convenience constructors work");
    }

    // ========== Legacy CoreError Tests ==========

    #[test]
    fn test_core_error_display() {
        let err = CoreError::NodeNotFound { id: Uuid::nil() };
        assert!(err.to_string().contains("Node not found"));
    }

    #[test]
    fn test_core_error_dimension_mismatch() {
        let err = CoreError::DimensionMismatch {
            expected: 1536,
            actual: 768,
        };
        assert!(err.to_string().contains("1536"));
        assert!(err.to_string().contains("768"));
    }

    #[test]
    fn test_core_to_context_graph_conversion() {
        let core_err = CoreError::StorageError("db failed".to_string());
        let ctx_err: ContextGraphError = core_err.into();
        assert!(matches!(ctx_err, ContextGraphError::Storage(_)));
        assert!(ctx_err.to_string().contains("db failed"));

        println!("[PASS] CoreError converts to ContextGraphError");
    }
}
