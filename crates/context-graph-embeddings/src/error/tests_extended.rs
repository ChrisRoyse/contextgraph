//! Tests for embedding error types - Part 2: Edge cases and comprehensive tests.
//!
//! # SPEC-EMB-001 Tests
//!
//! This module includes comprehensive tests for all 12 SPEC-EMB-001 error codes:
//! - spec_code() returns correct codes (EMB-E001 through EMB-E012)
//! - is_recoverable() returns true ONLY for InputTooLarge/InputTooLong
//! - severity() returns Critical/High/Medium appropriately
//! - Error messages contain required constitution references

use super::*;
use crate::types::{InputType, ModelId};
use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;

// ============================================================
// EDGE CASES (4 tests)
// ============================================================

#[test]
fn test_all_12_model_ids_in_model_not_found() {
    for model_id in ModelId::all() {
        let err = EmbeddingError::ModelNotFound {
            model_id: *model_id,
        };
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
        source: Box::new(std::io::Error::other("test")),
    };
    let _e3 = EmbeddingError::NotInitialized {
        model_id: ModelId::Graph,
    };

    // Validation Errors (4)
    let _e4 = EmbeddingError::InvalidDimension {
        expected: 1536,
        actual: 768,
    };
    let _e5 = EmbeddingError::InvalidValue {
        index: 0,
        value: 0.0,
    };
    let _e6 = EmbeddingError::EmptyInput;
    let _e7 = EmbeddingError::InputTooLong {
        actual: 100,
        max: 50,
    };

    // Processing Errors (2) - FusionError removed (TASK-F006)
    let _e8 = EmbeddingError::BatchError {
        message: "test".to_string(),
    };
    let _e9 = EmbeddingError::TokenizationError {
        model_id: ModelId::Semantic,
        message: "test".to_string(),
    };

    // Infrastructure Errors (4)
    let _e10 = EmbeddingError::GpuError {
        message: "test".to_string(),
    };
    let _e11 = EmbeddingError::CacheError {
        message: "test".to_string(),
    };
    let _e12 = EmbeddingError::IoError(std::io::Error::other("test"));
    let _e13 = EmbeddingError::Timeout { timeout_ms: 1000 };

    // Configuration Errors (2)
    let _e14 = EmbeddingError::UnsupportedModality {
        model_id: ModelId::Semantic,
        input_type: InputType::Image,
    };
    let _e15 = EmbeddingError::ConfigError {
        message: "test".to_string(),
    };

    // Serialization Errors (1)
    let _e16 = EmbeddingError::SerializationError {
        message: "test".to_string(),
    };

    // All variants created successfully (FusionError and InvalidExpertIndex removed in TASK-F006)
    println!("All error variants created successfully!");
}

// ============================================================
// SPECIAL VALUE TESTS (2 tests)
// ============================================================

#[test]
fn test_invalid_value_with_zero() {
    let err = EmbeddingError::InvalidValue {
        index: 100,
        value: 0.0,
    };
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

// ============================================================
// SPEC-EMB-001 ERROR CODE TESTS
// ============================================================

/// Use REAL ModelId variant - no mocks per task requirements
const TEST_MODEL: ModelId = ModelId::Semantic;

#[test]
fn test_cuda_unavailable_emb_e001() {
    let err = EmbeddingError::CudaUnavailable {
        message: "Driver not found".to_string(),
    };

    // Verify spec code
    assert_eq!(err.spec_code(), Some("EMB-E001"));

    // Verify NOT recoverable
    assert!(!err.is_recoverable());

    // Verify severity is Critical
    assert_eq!(err.severity(), ErrorSeverity::Critical);

    // Verify error message contains required constitution references
    let msg = err.to_string();
    assert!(msg.contains("EMB-E001"), "Missing error code in message");
    assert!(
        msg.contains("RTX 5090"),
        "Missing RTX 5090 constitution reference"
    );
    assert!(
        msg.contains("CUDA 13.1"),
        "Missing CUDA 13.1 constitution reference"
    );
    assert!(msg.contains("Remediation"), "Missing remediation guidance");

    println!("BEFORE: Creating CUDA unavailable error");
    println!("AFTER: {}", msg);
}

#[test]
fn test_insufficient_vram_emb_e002() {
    let err = EmbeddingError::InsufficientVram {
        required_bytes: 32_000_000_000,
        available_bytes: 8_000_000_000,
        required_gb: 32.0,
        available_gb: 8.0,
    };

    assert_eq!(err.spec_code(), Some("EMB-E002"));
    assert!(!err.is_recoverable());
    assert_eq!(err.severity(), ErrorSeverity::Critical);

    let msg = err.to_string();
    assert!(msg.contains("EMB-E002"));
    assert!(msg.contains("32.0 GB"));
    assert!(msg.contains("8.0 GB"));
    assert!(
        msg.contains("RTX 5090 (32GB)"),
        "Missing RTX 5090 32GB reference"
    );

    println!("BEFORE: VRAM check with 8GB available, 32GB required");
    println!("AFTER: {}", msg);
}

#[test]
fn test_weight_file_missing_emb_e003() {
    let err = EmbeddingError::WeightFileMissing {
        model_id: TEST_MODEL,
        path: PathBuf::from("/models/semantic/weights.safetensors"),
    };

    assert_eq!(err.spec_code(), Some("EMB-E003"));
    assert!(!err.is_recoverable());
    assert_eq!(err.severity(), ErrorSeverity::Critical);

    let msg = err.to_string();
    assert!(msg.contains("EMB-E003"));
    assert!(msg.contains("Semantic"));
    assert!(msg.contains("weights.safetensors"));
    assert!(
        msg.contains("HuggingFace"),
        "Missing HuggingFace remediation"
    );
}

#[test]
fn test_weight_checksum_mismatch_emb_e004() {
    let err = EmbeddingError::WeightChecksumMismatch {
        model_id: TEST_MODEL,
        expected: "abc123def456".to_string(),
        actual: "xyz789uvw012".to_string(),
    };

    assert_eq!(err.spec_code(), Some("EMB-E004"));
    assert!(!err.is_recoverable());
    assert_eq!(err.severity(), ErrorSeverity::Critical);

    let msg = err.to_string();
    assert!(msg.contains("EMB-E004"));
    assert!(
        msg.contains("SHA256"),
        "Missing SHA256 constitution reference"
    );
    assert!(msg.contains("abc123def456"));
    assert!(msg.contains("xyz789uvw012"));
}

#[test]
fn test_model_dimension_mismatch_emb_e005() {
    let err = EmbeddingError::ModelDimensionMismatch {
        model_id: TEST_MODEL,
        expected: 1024,
        actual: 768,
    };

    assert_eq!(err.spec_code(), Some("EMB-E005"));
    assert!(!err.is_recoverable());
    assert_eq!(err.severity(), ErrorSeverity::Critical);

    let msg = err.to_string();
    assert!(msg.contains("EMB-E005"));
    assert!(msg.contains("1024"));
    assert!(msg.contains("768"));
    assert!(msg.contains("ModelId::dimension()"));
}

#[test]
fn test_projection_matrix_missing_emb_e006() {
    let err = EmbeddingError::ProjectionMatrixMissing {
        path: PathBuf::from("/models/sparse/projection.bin"),
    };

    assert_eq!(err.spec_code(), Some("EMB-E006"));
    assert!(!err.is_recoverable());
    assert_eq!(err.severity(), ErrorSeverity::Critical);

    let msg = err.to_string();
    assert!(msg.contains("EMB-E006"));
    assert!(msg.contains("projection.bin"));
}

#[test]
fn test_oom_during_batch_emb_e007() {
    let err = EmbeddingError::OomDuringBatch {
        batch_size: 64,
        model_id: TEST_MODEL,
    };

    assert_eq!(err.spec_code(), Some("EMB-E007"));
    assert!(!err.is_recoverable());
    assert_eq!(err.severity(), ErrorSeverity::High); // High, not Critical

    let msg = err.to_string();
    assert!(msg.contains("EMB-E007"));
    assert!(msg.contains("64"));
    assert!(msg.contains("Reduce batch size"));
}

#[test]
fn test_inference_validation_failed_emb_e008() {
    let err = EmbeddingError::InferenceValidationFailed {
        model_id: TEST_MODEL,
        reason: "Output contains NaN values".to_string(),
    };

    assert_eq!(err.spec_code(), Some("EMB-E008"));
    assert!(!err.is_recoverable());
    assert_eq!(err.severity(), ErrorSeverity::Critical);

    let msg = err.to_string();
    assert!(msg.contains("EMB-E008"));
    assert!(msg.contains("NaN"));
    assert!(msg.contains("model weights"));
}

#[test]
fn test_input_too_large_emb_e009_is_recoverable() {
    let err = EmbeddingError::InputTooLarge {
        max_tokens: 512,
        actual_tokens: 1024,
    };

    assert_eq!(err.spec_code(), Some("EMB-E009"));
    // THIS IS THE ONLY RECOVERABLE SPEC ERROR
    assert!(err.is_recoverable(), "EMB-E009 must be recoverable");
    assert_eq!(err.severity(), ErrorSeverity::Medium);

    let msg = err.to_string();
    assert!(msg.contains("EMB-E009"));
    assert!(msg.contains("512"));
    assert!(msg.contains("1024"));
    assert!(msg.contains("Truncate") || msg.contains("split"));
}

#[test]
fn test_storage_corruption_emb_e010() {
    let err = EmbeddingError::StorageCorruption {
        id: "fp-12345-abcde".to_string(),
        reason: "CRC32 mismatch".to_string(),
    };

    assert_eq!(err.spec_code(), Some("EMB-E010"));
    assert!(!err.is_recoverable());
    assert_eq!(err.severity(), ErrorSeverity::High);

    let msg = err.to_string();
    assert!(msg.contains("EMB-E010"));
    assert!(msg.contains("fp-12345-abcde"));
    assert!(msg.contains("CRC32"));
    assert!(msg.contains("Re-index"));
}

#[test]
fn test_codebook_missing_emb_e011() {
    let err = EmbeddingError::CodebookMissing {
        model_id: TEST_MODEL,
    };

    assert_eq!(err.spec_code(), Some("EMB-E011"));
    assert!(!err.is_recoverable());
    assert_eq!(err.severity(), ErrorSeverity::High);

    let msg = err.to_string();
    assert!(msg.contains("EMB-E011"));
    assert!(msg.contains("PQ-8"));
    assert!(msg.contains("Train codebook"));
}

#[test]
fn test_recall_loss_exceeded_emb_e012() {
    let err = EmbeddingError::RecallLossExceeded {
        model_id: TEST_MODEL,
        measured: 0.8500,
        max_allowed: 0.0500,
    };

    assert_eq!(err.spec_code(), Some("EMB-E012"));
    assert!(!err.is_recoverable());
    assert_eq!(err.severity(), ErrorSeverity::Medium);

    let msg = err.to_string();
    assert!(msg.contains("EMB-E012"));
    assert!(msg.contains("0.8500"));
    assert!(msg.contains("0.0500"));
    assert!(msg.contains("Retrain codebook"));
}

// ============================================================
// SPEC-EMB-001 COMPREHENSIVE TESTS
// ============================================================

#[test]
fn test_all_12_spec_codes_are_unique() {
    let errors: Vec<EmbeddingError> = vec![
        EmbeddingError::CudaUnavailable { message: "".into() },
        EmbeddingError::InsufficientVram {
            required_bytes: 0,
            available_bytes: 0,
            required_gb: 0.0,
            available_gb: 0.0,
        },
        EmbeddingError::WeightFileMissing {
            model_id: TEST_MODEL,
            path: PathBuf::new(),
        },
        EmbeddingError::WeightChecksumMismatch {
            model_id: TEST_MODEL,
            expected: "".into(),
            actual: "".into(),
        },
        EmbeddingError::ModelDimensionMismatch {
            model_id: TEST_MODEL,
            expected: 0,
            actual: 0,
        },
        EmbeddingError::ProjectionMatrixMissing {
            path: PathBuf::new(),
        },
        EmbeddingError::OomDuringBatch {
            batch_size: 0,
            model_id: TEST_MODEL,
        },
        EmbeddingError::InferenceValidationFailed {
            model_id: TEST_MODEL,
            reason: "".into(),
        },
        EmbeddingError::InputTooLarge {
            max_tokens: 0,
            actual_tokens: 0,
        },
        EmbeddingError::StorageCorruption {
            id: "".into(),
            reason: "".into(),
        },
        EmbeddingError::CodebookMissing {
            model_id: TEST_MODEL,
        },
        EmbeddingError::RecallLossExceeded {
            model_id: TEST_MODEL,
            measured: 0.0,
            max_allowed: 0.0,
        },
    ];

    // Collect all spec codes
    let codes: Vec<Option<&str>> = errors.iter().map(|e| e.spec_code()).collect();

    // All must be Some
    for (i, code) in codes.iter().enumerate() {
        assert!(code.is_some(), "Error variant {} has no spec code", i);
    }

    // All must be unique
    let unique: HashSet<&str> = codes.iter().filter_map(|c| *c).collect();
    assert_eq!(unique.len(), 12, "All 12 spec codes must be unique");

    // Verify specific codes exist
    assert!(unique.contains("EMB-E001"));
    assert!(unique.contains("EMB-E012"));

    println!("BEFORE: Collecting 12 SPEC error codes");
    println!("AFTER: Found {} unique codes: {:?}", unique.len(), unique);
}

#[test]
fn test_only_input_too_large_is_recoverable_among_spec_errors() {
    let non_recoverable_errors: Vec<(&str, EmbeddingError)> = vec![
        (
            "EMB-E001",
            EmbeddingError::CudaUnavailable { message: "".into() },
        ),
        (
            "EMB-E002",
            EmbeddingError::InsufficientVram {
                required_bytes: 0,
                available_bytes: 0,
                required_gb: 0.0,
                available_gb: 0.0,
            },
        ),
        (
            "EMB-E003",
            EmbeddingError::WeightFileMissing {
                model_id: TEST_MODEL,
                path: PathBuf::new(),
            },
        ),
        (
            "EMB-E004",
            EmbeddingError::WeightChecksumMismatch {
                model_id: TEST_MODEL,
                expected: "".into(),
                actual: "".into(),
            },
        ),
        (
            "EMB-E005",
            EmbeddingError::ModelDimensionMismatch {
                model_id: TEST_MODEL,
                expected: 0,
                actual: 0,
            },
        ),
        (
            "EMB-E006",
            EmbeddingError::ProjectionMatrixMissing {
                path: PathBuf::new(),
            },
        ),
        (
            "EMB-E007",
            EmbeddingError::OomDuringBatch {
                batch_size: 0,
                model_id: TEST_MODEL,
            },
        ),
        (
            "EMB-E008",
            EmbeddingError::InferenceValidationFailed {
                model_id: TEST_MODEL,
                reason: "".into(),
            },
        ),
        // EMB-E009 is EXCLUDED - it IS recoverable
        (
            "EMB-E010",
            EmbeddingError::StorageCorruption {
                id: "".into(),
                reason: "".into(),
            },
        ),
        (
            "EMB-E011",
            EmbeddingError::CodebookMissing {
                model_id: TEST_MODEL,
            },
        ),
        (
            "EMB-E012",
            EmbeddingError::RecallLossExceeded {
                model_id: TEST_MODEL,
                measured: 0.0,
                max_allowed: 0.0,
            },
        ),
    ];

    for (code, err) in non_recoverable_errors {
        assert!(
            !err.is_recoverable(),
            "{} should NOT be recoverable but is_recoverable() returned true",
            code
        );
    }

    // The ONLY recoverable error
    let recoverable = EmbeddingError::InputTooLarge {
        max_tokens: 512,
        actual_tokens: 1024,
    };
    assert!(
        recoverable.is_recoverable(),
        "EMB-E009 MUST be recoverable but is_recoverable() returned false"
    );
}

#[test]
fn test_legacy_variant_has_no_spec_code() {
    let legacy_errors: Vec<EmbeddingError> = vec![
        EmbeddingError::ModelNotFound {
            model_id: TEST_MODEL,
        },
        EmbeddingError::EmptyInput,
        EmbeddingError::InvalidDimension {
            expected: 1024,
            actual: 768,
        },
        EmbeddingError::GpuError {
            message: "legacy".to_string(),
        },
        EmbeddingError::Timeout { timeout_ms: 1000 },
        EmbeddingError::ConfigError {
            message: "test".to_string(),
        },
    ];

    for err in legacy_errors {
        assert_eq!(
            err.spec_code(),
            None,
            "Legacy variant {:?} should have spec_code() = None",
            err
        );
    }
}

#[test]
fn test_legacy_input_too_long_is_also_recoverable() {
    // Legacy InputTooLong should also be recoverable for backward compatibility
    let err = EmbeddingError::InputTooLong {
        actual: 1000,
        max: 512,
    };
    assert!(
        err.is_recoverable(),
        "Legacy InputTooLong should be recoverable for backward compatibility"
    );
    // But it has no spec code
    assert_eq!(err.spec_code(), None);
}

#[test]
fn test_severity_classification_matches_spec() {
    // Critical errors (EMB-E001 to EMB-E006, EMB-E008)
    let critical_errors: Vec<EmbeddingError> = vec![
        EmbeddingError::CudaUnavailable { message: "".into() },
        EmbeddingError::InsufficientVram {
            required_bytes: 0,
            available_bytes: 0,
            required_gb: 0.0,
            available_gb: 0.0,
        },
        EmbeddingError::WeightFileMissing {
            model_id: TEST_MODEL,
            path: PathBuf::new(),
        },
        EmbeddingError::WeightChecksumMismatch {
            model_id: TEST_MODEL,
            expected: "".into(),
            actual: "".into(),
        },
        EmbeddingError::ModelDimensionMismatch {
            model_id: TEST_MODEL,
            expected: 0,
            actual: 0,
        },
        EmbeddingError::ProjectionMatrixMissing {
            path: PathBuf::new(),
        },
        EmbeddingError::InferenceValidationFailed {
            model_id: TEST_MODEL,
            reason: "".into(),
        },
    ];

    for err in critical_errors {
        assert_eq!(
            err.severity(),
            ErrorSeverity::Critical,
            "Error {:?} should have Critical severity",
            err.spec_code()
        );
    }

    // High errors (EMB-E007, EMB-E010, EMB-E011)
    let high_errors: Vec<EmbeddingError> = vec![
        EmbeddingError::OomDuringBatch {
            batch_size: 0,
            model_id: TEST_MODEL,
        },
        EmbeddingError::StorageCorruption {
            id: "".into(),
            reason: "".into(),
        },
        EmbeddingError::CodebookMissing {
            model_id: TEST_MODEL,
        },
    ];

    for err in high_errors {
        assert_eq!(
            err.severity(),
            ErrorSeverity::High,
            "Error {:?} should have High severity",
            err.spec_code()
        );
    }

    // Medium errors (EMB-E009, EMB-E012)
    let medium_errors: Vec<EmbeddingError> = vec![
        EmbeddingError::InputTooLarge {
            max_tokens: 0,
            actual_tokens: 0,
        },
        EmbeddingError::RecallLossExceeded {
            model_id: TEST_MODEL,
            measured: 0.0,
            max_allowed: 0.0,
        },
    ];

    for err in medium_errors {
        assert_eq!(
            err.severity(),
            ErrorSeverity::Medium,
            "Error {:?} should have Medium severity",
            err.spec_code()
        );
    }
}

// ============================================================
// EDGE CASE TESTS FOR SPEC ERRORS
// ============================================================

#[test]
fn test_zero_vram_display() {
    let err = EmbeddingError::InsufficientVram {
        required_bytes: 1000,
        available_bytes: 0,
        required_gb: 0.001,
        available_gb: 0.0,
    };
    let msg = err.to_string();
    // Should display 0.0 GB without panic
    assert!(msg.contains("0.0 GB") || msg.contains("0 bytes"));
    println!("BEFORE: Zero VRAM scenario");
    println!("AFTER: {}", msg);
}

#[test]
fn test_nan_recall_display() {
    let err = EmbeddingError::RecallLossExceeded {
        model_id: TEST_MODEL,
        measured: f32::NAN,
        max_allowed: 0.05,
    };
    let msg = err.to_string();
    // Should not panic, should display NaN
    assert!(msg.contains("NaN") || msg.contains("nan"));
    println!("BEFORE: NaN recall value");
    println!("AFTER: {}", msg);
}

#[test]
fn test_empty_path_display() {
    let err = EmbeddingError::WeightFileMissing {
        model_id: TEST_MODEL,
        path: PathBuf::new(),
    };
    let msg = err.to_string();
    // Should not panic with empty path
    assert!(msg.contains("Path:"));
    println!("BEFORE: Empty path scenario");
    println!("AFTER: {}", msg);
}

#[test]
fn test_unicode_in_error_reason() {
    let err = EmbeddingError::InferenceValidationFailed {
        model_id: TEST_MODEL,
        reason: "输出包含NaN值 (Output contains NaN)".to_string(),
    };
    let msg = err.to_string();
    // Unicode should display correctly
    assert!(msg.contains("输出"));
    println!("BEFORE: Unicode reason");
    println!("AFTER: {}", msg);
}

#[test]
fn test_all_13_model_ids_work_with_spec_errors() {
    for model_id in ModelId::all() {
        // Test with WeightFileMissing which uses ModelId
        let err = EmbeddingError::WeightFileMissing {
            model_id: *model_id,
            path: PathBuf::from("/test"),
        };
        let msg = err.to_string();

        // Should not panic for any model
        assert!(msg.contains("EMB-E003"));
        assert_eq!(err.spec_code(), Some("EMB-E003"));

        println!("Model {:?}: OK", model_id);
    }
}
