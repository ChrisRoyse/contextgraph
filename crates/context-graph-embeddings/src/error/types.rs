//! Core error types for the embedding pipeline.
//!
//! # SPEC-EMB-001 Error Taxonomy
//!
//! This module implements the SPEC-EMB-001 error taxonomy with 12 specific error codes
//! (EMB-E001 through EMB-E012). Each error includes:
//! - A unique error code for monitoring/alerting
//! - Remediation guidance in the error message
//! - Severity level for operational response
//! - Recovery classification (only EMB-E009 is recoverable)
//!
//! # Constitution References
//!
//! - AP-007: "No Stub Data in Production"
//! - stack.gpu: RTX 5090 (Blackwell, CC 12.0), CUDA 13.1, 32GB GDDR7
//! - security.checksums.algorithm: SHA256

use crate::types::{InputType, ModelId};
use std::path::PathBuf;
use thiserror::Error;

/// Error severity for monitoring integration.
///
/// Per SPEC-EMB-001, errors are classified by severity:
/// - `Critical`: System cannot function, immediate operator attention required
/// - `High`: Operation failed, retry unlikely to help without intervention
/// - `Medium`: Operation failed, may succeed with input modification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorSeverity {
    /// System cannot function - immediate operator attention required.
    /// Examples: CUDA unavailable, insufficient VRAM, weight corruption
    Critical,
    /// Operation failed - retry unlikely to help without intervention.
    /// Examples: OOM during batch, storage corruption, missing codebook
    High,
    /// Operation failed - may succeed with input modification.
    /// Examples: Input too large, recall loss exceeded
    Medium,
}

/// Comprehensive error type for all embedding pipeline failures.
///
/// # Error Categories
///
/// | Category | Variants | Recovery Strategy |
/// |----------|----------|-------------------|
/// | Model | ModelNotFound, ModelLoadError, NotInitialized | Retry with different config |
/// | Validation | InvalidDimension, InvalidValue, EmptyInput, InputTooLong | Fix input data |
/// | Processing | BatchError, TokenizationError | Retry or fallback model |
/// | Infrastructure | GpuError, CacheError, IoError, Timeout | Retry or degrade |
/// | Configuration | ConfigError, UnsupportedModality | Fix configuration |
/// | Serialization | SerializationError | Fix data format |
///
/// # Design Principles
///
/// - **NO FALLBACKS**: Errors must propagate, not be silently handled
/// - **FAIL FAST**: Invalid state triggers immediate error
/// - **CONTEXTUAL**: Every variant includes debugging information
/// - **TRACEABLE**: Error chain preserved via `source`
#[derive(Debug, Error)]
pub enum EmbeddingError {
    // === Model Errors ===
    /// Model with given ID not registered in ModelRegistry.
    #[error("Model not found: {model_id:?}")]
    ModelNotFound { model_id: ModelId },

    /// Model weight loading failed (HuggingFace download, ONNX parse, etc).
    #[error("Model load failed for {model_id:?}: {source}")]
    ModelLoadError {
        model_id: ModelId,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Model exists but embed() called before initialize().
    #[error("Model not initialized: {model_id:?}")]
    NotInitialized { model_id: ModelId },

    /// Model is already loaded in the registry.
    #[error("Model already loaded: {model_id:?}")]
    ModelAlreadyLoaded { model_id: ModelId },

    /// Model is not loaded in the registry.
    #[error("Model not loaded: {model_id:?}")]
    ModelNotLoaded { model_id: ModelId },

    /// Memory budget exceeded for loading models.
    #[error("Memory budget exceeded: requested {requested_bytes} bytes, available {available_bytes} bytes (budget: {budget_bytes} bytes)")]
    MemoryBudgetExceeded {
        requested_bytes: usize,
        available_bytes: usize,
        budget_bytes: usize,
    },

    /// Internal error (should not occur in normal operation).
    #[error("Internal error: {message}")]
    InternalError { message: String },

    // === Validation Errors ===
    /// Embedding vector dimension mismatch.
    #[error("Invalid dimension: expected {expected}, got {actual}")]
    InvalidDimension { expected: usize, actual: usize },

    /// Embedding contains NaN or Infinity at specific index.
    #[error("Invalid embedding value at index {index}: {value}")]
    InvalidValue { index: usize, value: f32 },

    /// Empty input provided (text, code, bytes).
    #[error("Empty input not allowed")]
    EmptyInput,

    /// Input exceeds model's max token limit.
    #[error("Input too long: {actual} tokens exceeds max {max}")]
    InputTooLong { actual: usize, max: usize },

    /// Invalid image data (decoding failed, corrupt, unsupported format).
    #[error("Invalid image: {reason}")]
    InvalidImage { reason: String },

    // === Processing Errors ===
    /// Batch processing failed (queue overflow, timeout, partial failure).
    #[error("Batch processing error: {message}")]
    BatchError { message: String },

    /// Tokenization failed (unknown tokens, encoding error).
    #[error("Tokenization error for {model_id:?}: {message}")]
    TokenizationError { model_id: ModelId, message: String },

    // === Infrastructure Errors ===
    /// GPU/CUDA operation failed.
    #[error("GPU error: {message}")]
    GpuError { message: String },

    /// Embedding cache operation failed (LRU eviction, disk I/O).
    #[error("Cache error: {message}")]
    CacheError { message: String },

    /// File I/O error (model weights, config files).
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Operation exceeded timeout threshold.
    #[error("Operation timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    // === Configuration Errors ===
    /// Model does not support the given input type.
    #[error("Unsupported input type {input_type:?} for model {model_id:?}")]
    UnsupportedModality {
        model_id: ModelId,
        input_type: InputType,
    },

    /// Configuration file invalid or missing required fields.
    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    // === Serialization Errors ===
    /// Serialization/deserialization failed (JSON, binary, protobuf).
    #[error("Serialization error: {message}")]
    SerializationError { message: String },

    /// Dimension mismatch between expected and actual values.
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    // =========================================================================
    // SPEC-EMB-001 Error Taxonomy (EMB-E001 through EMB-E012)
    // =========================================================================

    /// EMB-E001: CUDA is required but unavailable.
    ///
    /// Constitution: stack.gpu.cuda = "13.1"
    #[error("[EMB-E001] CUDA_UNAVAILABLE: {message}\n  Required: RTX 5090 (Blackwell, CC 12.0), CUDA 13.1+\n  Remediation: Install CUDA 13.1+ and verify GPU with nvidia-smi")]
    CudaUnavailable {
        /// Detailed message about why CUDA is unavailable
        message: String,
    },

    /// EMB-E002: Insufficient GPU VRAM.
    ///
    /// Constitution: stack.gpu.vram = "32GB GDDR7"
    #[error("[EMB-E002] INSUFFICIENT_VRAM: GPU memory insufficient\n  Required: {required_bytes} bytes ({required_gb:.1} GB)\n  Available: {available_bytes} bytes ({available_gb:.1} GB)\n  Remediation: Free GPU memory or upgrade to RTX 5090 (32GB)")]
    InsufficientVram {
        /// Required memory in bytes
        required_bytes: usize,
        /// Available memory in bytes
        available_bytes: usize,
        /// Required memory in GB (for display)
        required_gb: f64,
        /// Available memory in GB (for display)
        available_gb: f64,
    },

    /// EMB-E003: Weight file not found.
    #[error("[EMB-E003] WEIGHT_FILE_MISSING: Model weights not found\n  Model: {model_id:?}\n  Path: {path:?}\n  Remediation: Download weights from HuggingFace model repository")]
    WeightFileMissing {
        /// Model that requires the weights
        model_id: ModelId,
        /// Path where weights were expected
        path: PathBuf,
    },

    /// EMB-E004: Weight file checksum mismatch.
    ///
    /// Constitution: security.checksums.algorithm = "SHA256"
    #[error("[EMB-E004] WEIGHT_CHECKSUM_MISMATCH: Weight file corrupted\n  Model: {model_id:?}\n  Expected SHA256: {expected}\n  Actual SHA256: {actual}\n  Remediation: Re-download weight file from source")]
    WeightChecksumMismatch {
        /// Model with corrupted weights
        model_id: ModelId,
        /// Expected SHA256 checksum
        expected: String,
        /// Actual SHA256 checksum computed from file
        actual: String,
    },

    /// EMB-E005: Dimension mismatch during model validation.
    #[error("[EMB-E005] DIMENSION_MISMATCH: Embedding dimension invalid\n  Model: {model_id:?}\n  Expected: {expected}\n  Actual: {actual}\n  Remediation: Verify model configuration matches ModelId::dimension()")]
    ModelDimensionMismatch {
        /// Model with dimension issue
        model_id: ModelId,
        /// Expected dimension from ModelId::dimension()
        expected: usize,
        /// Actual dimension produced
        actual: usize,
    },

    /// EMB-E006: Projection matrix file missing.
    #[error("[EMB-E006] PROJECTION_MATRIX_MISSING: Sparse projection weights not found\n  Path: {path:?}\n  Remediation: Download from model repository or regenerate projection matrix")]
    ProjectionMatrixMissing {
        /// Path where projection matrix was expected
        path: PathBuf,
    },

    /// EMB-E007: Out of memory during batch processing.
    #[error("[EMB-E007] OOM_DURING_BATCH: GPU OOM during batch inference\n  Batch size: {batch_size}\n  Model: {model_id:?}\n  Remediation: Reduce batch size or free GPU memory")]
    OomDuringBatch {
        /// Size of batch that caused OOM
        batch_size: usize,
        /// Model being used when OOM occurred
        model_id: ModelId,
    },

    /// EMB-E008: Inference validation failed (NaN, Inf, or zero-norm).
    #[error("[EMB-E008] INFERENCE_VALIDATION_FAILED: Model output invalid\n  Model: {model_id:?}\n  Reason: {reason}\n  Remediation: Verify model weights and input preprocessing")]
    InferenceValidationFailed {
        /// Model that produced invalid output
        model_id: ModelId,
        /// Specific reason for validation failure
        reason: String,
    },

    /// EMB-E009: Input exceeds model capacity (ONLY recoverable error).
    #[error("[EMB-E009] INPUT_TOO_LARGE: Input exceeds token limit\n  Max tokens: {max_tokens}\n  Actual tokens: {actual_tokens}\n  Remediation: Truncate input or split into chunks")]
    InputTooLarge {
        /// Maximum tokens the model can handle
        max_tokens: usize,
        /// Actual tokens in the input
        actual_tokens: usize,
    },

    /// EMB-E010: Storage corruption detected.
    #[error("[EMB-E010] STORAGE_CORRUPTION: Stored embedding data corrupted\n  Fingerprint ID: {id}\n  Reason: {reason}\n  Remediation: Re-index from source document")]
    StorageCorruption {
        /// ID of the corrupted fingerprint
        id: String,
        /// Specific corruption reason
        reason: String,
    },

    /// EMB-E011: PQ-8 codebook missing for quantization.
    #[error("[EMB-E011] CODEBOOK_MISSING: PQ-8 codebook not found\n  Model: {model_id:?}\n  Remediation: Train codebook with representative vectors or download pre-trained")]
    CodebookMissing {
        /// Model that requires the codebook
        model_id: ModelId,
    },

    /// EMB-E012: Quantization recall loss exceeded threshold.
    #[error("[EMB-E012] RECALL_LOSS_EXCEEDED: Quantization quality too low\n  Model: {model_id:?}\n  Measured recall: {measured:.4}\n  Max allowed loss: {max_allowed:.4}\n  Remediation: Retrain codebook with more representative data")]
    RecallLossExceeded {
        /// Model with recall loss issue
        model_id: ModelId,
        /// Measured recall value (0.0 to 1.0)
        measured: f32,
        /// Maximum allowed recall loss threshold
        max_allowed: f32,
    },

    // =========================================================================
    // QuantizationRouter Error Variants (TASK-EMB-020)
    // =========================================================================

    /// Quantizer for the specified method is not yet implemented.
    #[error("[{model_id:?}] Quantizer not implemented: {method}")]
    QuantizerNotImplemented {
        /// Model that requires quantization
        model_id: ModelId,
        /// Quantization method not yet implemented
        method: String,
    },

    /// Quantization operation failed.
    #[error("[{model_id:?}] Quantization failed: {reason}")]
    QuantizationFailed {
        /// Model being quantized
        model_id: ModelId,
        /// Reason for failure
        reason: String,
    },

    /// Dequantization operation failed.
    #[error("[{model_id:?}] Dequantization failed: {reason}")]
    DequantizationFailed {
        /// Model being dequantized
        model_id: ModelId,
        /// Reason for failure
        reason: String,
    },

    /// Unsupported operation for this model.
    #[error("[{model_id:?}] Unsupported operation: {operation}")]
    UnsupportedOperation {
        /// Model for which operation is unsupported
        model_id: ModelId,
        /// Operation that is not supported
        operation: String,
    },

    /// Invalid input for the specified model.
    #[error("[{model_id:?}] Invalid input: {reason}")]
    InvalidModelInput {
        /// Model that received invalid input
        model_id: ModelId,
        /// Reason input is invalid
        reason: String,
    },
}

impl EmbeddingError {
    /// Returns the SPEC-EMB-001 error code if applicable.
    ///
    /// Returns `Some("EMB-E0XX")` for SPEC-EMB-001 variants, `None` for legacy variants.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::EmbeddingError;
    ///
    /// let err = EmbeddingError::CudaUnavailable {
    ///     message: "Driver not found".to_string(),
    /// };
    /// assert_eq!(err.spec_code(), Some("EMB-E001"));
    ///
    /// let legacy = EmbeddingError::EmptyInput;
    /// assert_eq!(legacy.spec_code(), None);
    /// ```
    #[must_use]
    pub fn spec_code(&self) -> Option<&'static str> {
        match self {
            Self::CudaUnavailable { .. } => Some("EMB-E001"),
            Self::InsufficientVram { .. } => Some("EMB-E002"),
            Self::WeightFileMissing { .. } => Some("EMB-E003"),
            Self::WeightChecksumMismatch { .. } => Some("EMB-E004"),
            Self::ModelDimensionMismatch { .. } => Some("EMB-E005"),
            Self::ProjectionMatrixMissing { .. } => Some("EMB-E006"),
            Self::OomDuringBatch { .. } => Some("EMB-E007"),
            Self::InferenceValidationFailed { .. } => Some("EMB-E008"),
            Self::InputTooLarge { .. } => Some("EMB-E009"),
            Self::StorageCorruption { .. } => Some("EMB-E010"),
            Self::CodebookMissing { .. } => Some("EMB-E011"),
            Self::RecallLossExceeded { .. } => Some("EMB-E012"),
            // Legacy variants have no spec code
            _ => None,
        }
    }

    /// Check if this error is recoverable.
    ///
    /// Per SPEC-EMB-001, **ONLY** EMB-E009 (INPUT_TOO_LARGE) is recoverable.
    /// The legacy `InputTooLong` variant is also considered recoverable for
    /// backward compatibility in error handling logic.
    ///
    /// All other errors require operator intervention.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::EmbeddingError;
    ///
    /// // Only InputTooLarge is recoverable
    /// let recoverable = EmbeddingError::InputTooLarge {
    ///     max_tokens: 512,
    ///     actual_tokens: 1024,
    /// };
    /// assert!(recoverable.is_recoverable());
    ///
    /// // All other SPEC errors are NOT recoverable
    /// let not_recoverable = EmbeddingError::CudaUnavailable {
    ///     message: "No driver".to_string(),
    /// };
    /// assert!(!not_recoverable.is_recoverable());
    /// ```
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::InputTooLarge { .. } | Self::InputTooLong { .. })
    }

    /// Returns the severity level for monitoring/alerting.
    ///
    /// Per SPEC-EMB-001:
    /// - `Critical`: System cannot function (EMB-E001 to EMB-E006, EMB-E008)
    /// - `High`: Operation failed, retry unlikely to help (EMB-E007, EMB-E010, EMB-E011)
    /// - `Medium`: Operation failed, may succeed with modification (EMB-E009, EMB-E012)
    ///
    /// Legacy variants default to `High` severity.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::{EmbeddingError, error::ErrorSeverity};
    ///
    /// let critical = EmbeddingError::CudaUnavailable {
    ///     message: "No driver".to_string(),
    /// };
    /// assert_eq!(critical.severity(), ErrorSeverity::Critical);
    ///
    /// let medium = EmbeddingError::InputTooLarge {
    ///     max_tokens: 512,
    ///     actual_tokens: 1024,
    /// };
    /// assert_eq!(medium.severity(), ErrorSeverity::Medium);
    /// ```
    #[must_use]
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: System cannot function
            Self::CudaUnavailable { .. }
            | Self::InsufficientVram { .. }
            | Self::WeightFileMissing { .. }
            | Self::WeightChecksumMismatch { .. }
            | Self::ModelDimensionMismatch { .. }
            | Self::ProjectionMatrixMissing { .. }
            | Self::InferenceValidationFailed { .. } => ErrorSeverity::Critical,

            // High: Operation failed, retry unlikely to help
            Self::OomDuringBatch { .. }
            | Self::StorageCorruption { .. }
            | Self::CodebookMissing { .. } => ErrorSeverity::High,

            // Medium: Operation failed, may succeed with modification
            Self::InputTooLarge { .. } | Self::RecallLossExceeded { .. } => ErrorSeverity::Medium,

            // Legacy variants default to High
            _ => ErrorSeverity::High,
        }
    }
}

/// Result type alias for embedding operations.
pub type EmbeddingResult<T> = Result<T, EmbeddingError>;
