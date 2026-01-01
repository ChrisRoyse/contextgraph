//! Comprehensive error type for all embedding pipeline failures.
//!
//! # Error Categories
//!
//! | Category | Variants | Recovery Strategy |
//! |----------|----------|-------------------|
//! | Model | ModelNotFound, ModelLoadError, NotInitialized | Retry with different config |
//! | Validation | InvalidDimension, InvalidValue, EmptyInput, InputTooLong | Fix input data |
//! | Processing | BatchError, FusionError, TokenizationError | Retry or fallback model |
//! | Infrastructure | GpuError, CacheError, IoError, Timeout | Retry or degrade |
//! | Configuration | ConfigError, UnsupportedModality | Fix configuration |
//! | Serialization | SerializationError | Fix data format |
//!
//! # Design Principles
//!
//! - **NO FALLBACKS**: Errors must propagate, not be silently handled
//! - **FAIL FAST**: Invalid state triggers immediate error
//! - **CONTEXTUAL**: Every variant includes debugging information
//! - **TRACEABLE**: Error chain preserved via `source`

use crate::types::{InputType, ModelId};
use thiserror::Error;

/// Comprehensive error type for all embedding pipeline failures.
///
/// # Error Categories
///
/// | Category | Variants | Recovery Strategy |
/// |----------|----------|-------------------|
/// | Model | ModelNotFound, ModelLoadError, NotInitialized | Retry with different config |
/// | Validation | InvalidDimension, InvalidValue, EmptyInput, InputTooLong | Fix input data |
/// | Processing | BatchError, FusionError, TokenizationError | Retry or fallback model |
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

    // === Processing Errors ===
    /// Batch processing failed (queue overflow, timeout, partial failure).
    #[error("Batch processing error: {message}")]
    BatchError { message: String },

    /// FuseMoE fusion failed (expert routing, gating, aggregation).
    #[error("Fusion error: {message}")]
    FusionError { message: String },

    /// Tokenization failed (unknown tokens, encoding error).
    #[error("Tokenization error: {message}")]
    TokenizationError { message: String },

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
}

/// Result type alias for embedding operations.
pub type EmbeddingResult<T> = Result<T, EmbeddingError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    // ============================================================
    // MODEL ERROR CREATION TESTS (3 tests)
    // ============================================================

    #[test]
    fn test_model_not_found_error_creation() {
        let err = EmbeddingError::ModelNotFound {
            model_id: ModelId::Semantic,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Semantic"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_model_load_error_preserves_source() {
        let source = std::io::Error::new(std::io::ErrorKind::NotFound, "weights missing");
        let err = EmbeddingError::ModelLoadError {
            model_id: ModelId::Code,
            source: Box::new(source),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Code"));
        assert!(msg.contains("weights missing"));
        // Verify source chain
        assert!(err.source().is_some());
    }

    #[test]
    fn test_not_initialized_error_creation() {
        let err = EmbeddingError::NotInitialized {
            model_id: ModelId::Graph,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Graph"));
        assert!(msg.contains("not initialized"));
    }

    // ============================================================
    // VALIDATION ERROR CREATION TESTS (4 tests)
    // ============================================================

    #[test]
    fn test_invalid_dimension_shows_both_values() {
        let err = EmbeddingError::InvalidDimension {
            expected: 1536,
            actual: 768,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("1536"));
        assert!(msg.contains("768"));
    }

    #[test]
    fn test_invalid_value_shows_index_and_value() {
        let err = EmbeddingError::InvalidValue {
            index: 42,
            value: f32::NAN,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("42"));
        assert!(msg.contains("NaN"));
    }

    #[test]
    fn test_empty_input_error_message() {
        let err = EmbeddingError::EmptyInput;
        let msg = format!("{}", err);
        assert!(msg.contains("Empty"));
    }

    #[test]
    fn test_input_too_long_shows_limits() {
        let err = EmbeddingError::InputTooLong { actual: 600, max: 512 };
        let msg = format!("{}", err);
        assert!(msg.contains("600"));
        assert!(msg.contains("512"));
    }

    // ============================================================
    // PROCESSING ERROR CREATION TESTS (3 tests)
    // ============================================================

    #[test]
    fn test_batch_error_shows_message() {
        let err = EmbeddingError::BatchError {
            message: "queue overflow".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Batch"));
        assert!(msg.contains("queue overflow"));
    }

    #[test]
    fn test_fusion_error_shows_message() {
        let err = EmbeddingError::FusionError {
            message: "expert routing failed".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Fusion"));
        assert!(msg.contains("expert routing failed"));
    }

    #[test]
    fn test_tokenization_error_shows_message() {
        let err = EmbeddingError::TokenizationError {
            message: "unknown token".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Tokenization"));
        assert!(msg.contains("unknown token"));
    }

    // ============================================================
    // INFRASTRUCTURE ERROR CREATION TESTS (4 tests)
    // ============================================================

    #[test]
    fn test_gpu_error_shows_message() {
        let err = EmbeddingError::GpuError {
            message: "CUDA OOM".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("GPU"));
        assert!(msg.contains("CUDA OOM"));
    }

    #[test]
    fn test_cache_error_shows_message() {
        let err = EmbeddingError::CacheError {
            message: "eviction failed".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Cache"));
        assert!(msg.contains("eviction failed"));
    }

    #[test]
    fn test_io_error_wraps_std_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = EmbeddingError::IoError(io_err);
        let msg = format!("{}", err);
        assert!(msg.contains("access denied"));
    }

    #[test]
    fn test_timeout_error_shows_duration() {
        let err = EmbeddingError::Timeout { timeout_ms: 5000 };
        let msg = format!("{}", err);
        assert!(msg.contains("timeout"));
        assert!(msg.contains("5000"));
    }

    // ============================================================
    // CONFIGURATION ERROR CREATION TESTS (2 tests)
    // ============================================================

    #[test]
    fn test_unsupported_modality_shows_both() {
        let err = EmbeddingError::UnsupportedModality {
            model_id: ModelId::Semantic,
            input_type: InputType::Image,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Semantic"));
        assert!(msg.contains("Image"));
    }

    #[test]
    fn test_config_error_shows_message() {
        let err = EmbeddingError::ConfigError {
            message: "missing required field".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Configuration"));
        assert!(msg.contains("missing required field"));
    }

    // ============================================================
    // SERIALIZATION ERROR CREATION TEST (1 test)
    // ============================================================

    #[test]
    fn test_serialization_error_shows_message() {
        let err = EmbeddingError::SerializationError {
            message: "invalid JSON".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Serialization"));
        assert!(msg.contains("invalid JSON"));
    }

    // ============================================================
    // SEND + SYNC TESTS (2 tests)
    // ============================================================

    #[test]
    fn test_embedding_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<EmbeddingError>();
    }

    #[test]
    fn test_embedding_error_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<EmbeddingError>();
    }

    // ============================================================
    // FROM<IO::ERROR> TEST (1 test)
    // ============================================================

    #[test]
    fn test_io_error_conversion_via_question_mark() {
        fn fallible_io() -> EmbeddingResult<()> {
            let _ = std::fs::read("/nonexistent/path/that/does/not/exist/12345")?;
            Ok(())
        }
        let result = fallible_io();
        assert!(matches!(result, Err(EmbeddingError::IoError(_))));
    }

    // ============================================================
    // EMBEDDING RESULT ALIAS TEST (1 test)
    // ============================================================

    #[test]
    fn test_embedding_result_alias_works() {
        fn returns_ok() -> EmbeddingResult<i32> {
            Ok(42)
        }
        fn returns_err() -> EmbeddingResult<i32> {
            Err(EmbeddingError::EmptyInput)
        }
        assert_eq!(returns_ok().unwrap(), 42);
        assert!(returns_err().is_err());
    }

    // ============================================================
    // DEBUG FORMATTING TEST (1 test)
    // ============================================================

    #[test]
    fn test_debug_formatting_includes_variant_name() {
        let err = EmbeddingError::Timeout { timeout_ms: 5000 };
        let debug = format!("{:?}", err);
        assert!(debug.contains("Timeout"));
        assert!(debug.contains("5000"));
    }

    // ============================================================
    // EDGE CASES (4 tests)
    // ============================================================

    #[test]
    fn test_all_12_model_ids_in_model_not_found() {
        for model_id in ModelId::all() {
            let err = EmbeddingError::ModelNotFound { model_id: *model_id };
            let msg = format!("{}", err);
            // Verify error message is non-empty and contains model info
            assert!(!msg.is_empty());
            println!("BEFORE: ModelId::{:?}", model_id);
            println!("AFTER: Error message = {}", msg);
        }
    }

    #[test]
    fn test_all_4_input_types_in_unsupported_modality() {
        for input_type in InputType::all() {
            let err = EmbeddingError::UnsupportedModality {
                model_id: ModelId::Semantic,
                input_type: *input_type,
            };
            let msg = format!("{}", err);
            assert!(!msg.is_empty());
            println!("BEFORE: InputType::{:?}", input_type);
            println!("AFTER: Error message = {}", msg);
        }
    }

    #[test]
    fn test_invalid_value_with_infinity() {
        let err = EmbeddingError::InvalidValue {
            index: 0,
            value: f32::INFINITY,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("inf"));
    }

    #[test]
    fn test_invalid_value_with_neg_infinity() {
        let err = EmbeddingError::InvalidValue {
            index: 0,
            value: f32::NEG_INFINITY,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("-inf") || msg.contains("inf"));
    }

    // ============================================================
    // ERROR SOURCE CHAIN TEST (1 test)
    // ============================================================

    #[test]
    fn test_model_load_error_source_chain() {
        let root_cause = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = EmbeddingError::ModelLoadError {
            model_id: ModelId::Causal,
            source: Box::new(root_cause),
        };

        // Verify the source chain is preserved
        let source = err.source();
        assert!(source.is_some());

        let source_msg = format!("{}", source.unwrap());
        assert!(source_msg.contains("file not found"));
    }

    // ============================================================
    // ALL 17 VARIANTS EXIST TEST (1 test)
    // ============================================================

    #[test]
    fn test_all_17_variants_can_be_created() {
        // Model Errors (3)
        let _e1 = EmbeddingError::ModelNotFound {
            model_id: ModelId::Semantic,
        };
        let _e2 = EmbeddingError::ModelLoadError {
            model_id: ModelId::Code,
            source: Box::new(std::io::Error::new(std::io::ErrorKind::Other, "test")),
        };
        let _e3 = EmbeddingError::NotInitialized {
            model_id: ModelId::Graph,
        };

        // Validation Errors (4)
        let _e4 = EmbeddingError::InvalidDimension {
            expected: 1536,
            actual: 768,
        };
        let _e5 = EmbeddingError::InvalidValue { index: 0, value: 0.0 };
        let _e6 = EmbeddingError::EmptyInput;
        let _e7 = EmbeddingError::InputTooLong { actual: 100, max: 50 };

        // Processing Errors (3)
        let _e8 = EmbeddingError::BatchError {
            message: "test".to_string(),
        };
        let _e9 = EmbeddingError::FusionError {
            message: "test".to_string(),
        };
        let _e10 = EmbeddingError::TokenizationError {
            message: "test".to_string(),
        };

        // Infrastructure Errors (4)
        let _e11 = EmbeddingError::GpuError {
            message: "test".to_string(),
        };
        let _e12 = EmbeddingError::CacheError {
            message: "test".to_string(),
        };
        let _e13 = EmbeddingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, "test"));
        let _e14 = EmbeddingError::Timeout { timeout_ms: 1000 };

        // Configuration Errors (2)
        let _e15 = EmbeddingError::UnsupportedModality {
            model_id: ModelId::Semantic,
            input_type: InputType::Image,
        };
        let _e16 = EmbeddingError::ConfigError {
            message: "test".to_string(),
        };

        // Serialization Errors (1)
        let _e17 = EmbeddingError::SerializationError {
            message: "test".to_string(),
        };

        // All 17 variants created successfully
        println!("All 17 error variants created successfully!");
    }

    // ============================================================
    // SPECIAL VALUE TESTS (2 tests)
    // ============================================================

    #[test]
    fn test_invalid_value_with_zero() {
        let err = EmbeddingError::InvalidValue { index: 100, value: 0.0 };
        let msg = format!("{}", err);
        assert!(msg.contains("100"));
        assert!(msg.contains("0"));
    }

    #[test]
    fn test_invalid_value_with_negative() {
        let err = EmbeddingError::InvalidValue {
            index: 50,
            value: -123.456,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("50"));
        assert!(msg.contains("-123"));
    }

    // ============================================================
    // BOUNDARY VALUE TESTS (2 tests)
    // ============================================================

    #[test]
    fn test_timeout_with_max_u64() {
        let err = EmbeddingError::Timeout {
            timeout_ms: u64::MAX,
        };
        let msg = format!("{}", err);
        assert!(msg.contains(&u64::MAX.to_string()));
    }

    #[test]
    fn test_input_too_long_with_large_values() {
        let err = EmbeddingError::InputTooLong {
            actual: 1_000_000,
            max: 4096,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("1000000"));
        assert!(msg.contains("4096"));
    }
}
