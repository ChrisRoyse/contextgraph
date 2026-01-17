//! Validation tests for SemanticFingerprint.
//!
//! Tests for dimension validation following constitution.yaml:
//! - ARCH-01: "TeleologicalArray is atomic - store all 13 embeddings or nothing"
//! - ARCH-05: "All 13 embedders required - missing embedder is fatal error"

use crate::teleological::Embedder;
use crate::types::fingerprint::semantic::*;
use crate::types::fingerprint::SparseVector;

// =============================================================================
// Happy Path Tests
// =============================================================================

/// Test that a zeroed fingerprint passes validation.
#[test]
fn test_semantic_fingerprint_validate_zeroed_passes() {
    let fp = SemanticFingerprint::zeroed();
    assert!(fp.validate().is_ok(), "Zeroed fingerprint should pass validation");
}

/// Test that valid sparse vectors pass validation.
#[test]
fn test_semantic_fingerprint_validate_with_valid_sparse() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e6_sparse =
        SparseVector::new(vec![100, 200, 30521], vec![0.1, 0.2, 0.3]).expect("valid sparse vector");
    assert!(fp.validate().is_ok(), "Valid E6 sparse should pass");

    let mut fp2 = SemanticFingerprint::zeroed();
    fp2.e13_splade =
        SparseVector::new(vec![100, 200, 30521], vec![0.1, 0.2, 0.3]).expect("valid sparse vector");
    assert!(fp2.validate().is_ok(), "Valid E13 splade should pass");
}

/// Test that valid E12 late-interaction tokens pass validation.
#[test]
fn test_semantic_fingerprint_validate_with_valid_e12() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e12_late_interaction = vec![vec![0.0; E12_TOKEN_DIM]; 10];
    assert!(fp.validate().is_ok(), "Valid E12 tokens should pass");
}

/// Test that empty E12 (0 tokens) is valid.
#[test]
fn test_semantic_fingerprint_validate_e12_empty_valid() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e12_late_interaction = Vec::new();
    assert!(fp.validate().is_ok(), "Empty E12 should be valid");
}

// =============================================================================
// Dimension Mismatch Tests
// =============================================================================

/// Test that wrong E1 dimension fails with DimensionMismatch.
#[test]
fn test_semantic_fingerprint_validate_e1_wrong_dimension() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e1_semantic = vec![0.0; 100]; // Should be 1024

    let err = fp.validate().unwrap_err();
    match err {
        ValidationError::DimensionMismatch {
            embedder,
            expected,
            actual,
        } => {
            assert_eq!(embedder, Embedder::Semantic);
            assert_eq!(expected, E1_DIM);
            assert_eq!(actual, 100);
        }
        _ => panic!("Expected DimensionMismatch, got {:?}", err),
    }
}

/// Test that wrong E7 dimension fails with DimensionMismatch.
#[test]
fn test_semantic_fingerprint_validate_e7_wrong_dimension() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e7_code = vec![0.0; 1024]; // Should be 1536

    let err = fp.validate().unwrap_err();
    match err {
        ValidationError::DimensionMismatch {
            embedder,
            expected,
            actual,
        } => {
            assert_eq!(embedder, Embedder::Code);
            assert_eq!(expected, E7_DIM);
            assert_eq!(actual, 1024);
        }
        _ => panic!("Expected DimensionMismatch, got {:?}", err),
    }
}

/// Test that wrong E9 dimension fails with DimensionMismatch.
#[test]
fn test_semantic_fingerprint_validate_e9_wrong_dimension() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e9_hdc = vec![0.0; 512]; // Should be 1024 (projected)

    let err = fp.validate().unwrap_err();
    match err {
        ValidationError::DimensionMismatch {
            embedder,
            expected,
            actual,
        } => {
            assert_eq!(embedder, Embedder::Hdc);
            assert_eq!(expected, E9_DIM);
            assert_eq!(actual, 512);
        }
        _ => panic!("Expected DimensionMismatch, got {:?}", err),
    }
}

// =============================================================================
// E12 Token Dimension Tests
// =============================================================================

/// Test that wrong E12 token dimension fails with TokenDimensionMismatch.
#[test]
fn test_semantic_fingerprint_validate_e12_wrong_token_dimension() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e12_late_interaction = vec![vec![0.0; 64]]; // Should be 128

    let err = fp.validate().unwrap_err();
    match err {
        ValidationError::TokenDimensionMismatch {
            embedder,
            token_index,
            expected,
            actual,
        } => {
            assert_eq!(embedder, Embedder::LateInteraction);
            assert_eq!(token_index, 0);
            assert_eq!(expected, E12_TOKEN_DIM);
            assert_eq!(actual, 64);
        }
        _ => panic!("Expected TokenDimensionMismatch, got {:?}", err),
    }
}

/// Test that E12 with second token wrong dimension is caught.
#[test]
fn test_semantic_fingerprint_validate_e12_second_token_wrong() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e12_late_interaction = vec![
        vec![0.0; E12_TOKEN_DIM], // Token 0: correct
        vec![0.0; 64],            // Token 1: wrong
    ];

    let err = fp.validate().unwrap_err();
    match err {
        ValidationError::TokenDimensionMismatch {
            embedder,
            token_index,
            expected,
            actual,
        } => {
            assert_eq!(embedder, Embedder::LateInteraction);
            assert_eq!(token_index, 1);
            assert_eq!(expected, E12_TOKEN_DIM);
            assert_eq!(actual, 64);
        }
        _ => panic!("Expected TokenDimensionMismatch, got {:?}", err),
    }
}

// =============================================================================
// Sparse Vector Error Tests
// =============================================================================

/// Test that sparse index > vocab size fails with SparseIndexOutOfBounds.
#[test]
fn test_semantic_fingerprint_validate_e6_sparse_index_overflow() {
    let mut fp = SemanticFingerprint::zeroed();
    // Manually create sparse vector with invalid index (bypassing SparseVector::new validation)
    fp.e6_sparse.indices = vec![40000]; // Exceeds 30522
    fp.e6_sparse.values = vec![0.5];

    let err = fp.validate().unwrap_err();
    match err {
        ValidationError::SparseIndexOutOfBounds {
            embedder,
            index,
            vocab_size,
        } => {
            assert_eq!(embedder, Embedder::Sparse);
            assert_eq!(index, 40000);
            assert_eq!(vocab_size, E6_SPARSE_VOCAB);
        }
        _ => panic!("Expected SparseIndexOutOfBounds, got {:?}", err),
    }
}

/// Test that sparse index exactly at vocab size fails.
#[test]
fn test_semantic_fingerprint_validate_e6_sparse_index_at_boundary() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e6_sparse.indices = vec![30522]; // At vocab size (invalid - should be < 30522)
    fp.e6_sparse.values = vec![0.5];

    let err = fp.validate().unwrap_err();
    match err {
        ValidationError::SparseIndexOutOfBounds {
            embedder,
            index,
            vocab_size,
        } => {
            assert_eq!(embedder, Embedder::Sparse);
            assert_eq!(index, 30522);
            assert_eq!(vocab_size, E6_SPARSE_VOCAB);
        }
        _ => panic!("Expected SparseIndexOutOfBounds, got {:?}", err),
    }
}

/// Test that E13 sparse index overflow fails.
#[test]
fn test_semantic_fingerprint_validate_e13_sparse_index_overflow() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e13_splade.indices = vec![35000]; // Exceeds 30522
    fp.e13_splade.values = vec![0.5];

    let err = fp.validate().unwrap_err();
    match err {
        ValidationError::SparseIndexOutOfBounds {
            embedder,
            index,
            vocab_size,
        } => {
            assert_eq!(embedder, Embedder::KeywordSplade);
            assert_eq!(index, 35000);
            assert_eq!(vocab_size, E13_SPLADE_VOCAB);
        }
        _ => panic!("Expected SparseIndexOutOfBounds, got {:?}", err),
    }
}

/// Test that sparse indices/values length mismatch fails with SparseIndicesValuesMismatch.
#[test]
fn test_semantic_fingerprint_validate_e6_sparse_length_mismatch() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e6_sparse.indices = vec![100, 200, 300, 400, 500]; // 5 indices
    fp.e6_sparse.values = vec![0.1, 0.2, 0.3]; // 3 values

    let err = fp.validate().unwrap_err();
    match err {
        ValidationError::SparseIndicesValuesMismatch {
            embedder,
            indices_len,
            values_len,
        } => {
            assert_eq!(embedder, Embedder::Sparse);
            assert_eq!(indices_len, 5);
            assert_eq!(values_len, 3);
        }
        _ => panic!("Expected SparseIndicesValuesMismatch, got {:?}", err),
    }
}

/// Test that E13 sparse length mismatch fails.
#[test]
fn test_semantic_fingerprint_validate_e13_sparse_length_mismatch() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e13_splade.indices = vec![100, 200]; // 2 indices
    fp.e13_splade.values = vec![0.1, 0.2, 0.3, 0.4]; // 4 values

    let err = fp.validate().unwrap_err();
    match err {
        ValidationError::SparseIndicesValuesMismatch {
            embedder,
            indices_len,
            values_len,
        } => {
            assert_eq!(embedder, Embedder::KeywordSplade);
            assert_eq!(indices_len, 2);
            assert_eq!(values_len, 4);
        }
        _ => panic!("Expected SparseIndicesValuesMismatch, got {:?}", err),
    }
}

// =============================================================================
// validate_all() Tests
// =============================================================================

/// Test that validate_all() collects multiple errors.
#[test]
fn test_semantic_fingerprint_validate_all_multiple_errors() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e1_semantic = vec![0.0; 100]; // Wrong E1
    fp.e5_causal = vec![0.0; 100]; // Wrong E5
    fp.e7_code = vec![0.0; 100]; // Wrong E7

    let result = fp.validate_all();
    assert!(result.is_err(), "Should have errors");

    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 3, "Should have 3 errors, got {}", errors.len());

    // Verify each error is present
    let embedders: Vec<Embedder> = errors
        .iter()
        .filter_map(|e| match e {
            ValidationError::DimensionMismatch { embedder, .. } => Some(*embedder),
            _ => None,
        })
        .collect();

    assert!(embedders.contains(&Embedder::Semantic));
    assert!(embedders.contains(&Embedder::Causal));
    assert!(embedders.contains(&Embedder::Code));
}

/// Test that validate_all() returns Ok when all valid.
#[test]
fn test_semantic_fingerprint_validate_all_ok() {
    let fp = SemanticFingerprint::zeroed();
    assert!(fp.validate_all().is_ok(), "Should be Ok");
}

/// Test that validate() returns first error only.
#[test]
fn test_semantic_fingerprint_validate_first_error_only() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e1_semantic = vec![0.0; 100]; // Wrong E1 (checked first)
    fp.e7_code = vec![0.0; 100]; // Wrong E7 (checked later)

    let err = fp.validate().unwrap_err();
    // Should only get E1 error since validate() is fail-fast
    match err {
        ValidationError::DimensionMismatch { embedder, .. } => {
            assert_eq!(embedder, Embedder::Semantic, "Should get E1 error first");
        }
        _ => panic!("Expected DimensionMismatch for E1"),
    }
}

// =============================================================================
// Error Message Format Tests
// =============================================================================

/// Test that error messages are correctly formatted.
#[test]
fn test_validation_error_display() {
    // Test DimensionMismatch display
    let err = ValidationError::DimensionMismatch {
        embedder: Embedder::Semantic,
        expected: 1024,
        actual: 100,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("Dimension mismatch"),
        "Message should mention dimension mismatch: {}",
        msg
    );
    assert!(msg.contains("1024"), "Message should contain expected: {}", msg);
    assert!(msg.contains("100"), "Message should contain actual: {}", msg);

    // Test TokenDimensionMismatch display
    let err2 = ValidationError::TokenDimensionMismatch {
        embedder: Embedder::LateInteraction,
        token_index: 5,
        expected: 128,
        actual: 64,
    };
    let msg2 = err2.to_string();
    assert!(
        msg2.contains("Token 5"),
        "Message should mention token index: {}",
        msg2
    );

    // Test SparseIndexOutOfBounds display
    let err3 = ValidationError::SparseIndexOutOfBounds {
        embedder: Embedder::Sparse,
        index: 40000,
        vocab_size: 30522,
    };
    let msg3 = err3.to_string();
    assert!(
        msg3.contains("40000"),
        "Message should contain index: {}",
        msg3
    );
    assert!(
        msg3.contains("30522"),
        "Message should contain vocab size: {}",
        msg3
    );

    // Test SparseIndicesValuesMismatch display
    let err4 = ValidationError::SparseIndicesValuesMismatch {
        embedder: Embedder::KeywordSplade,
        indices_len: 5,
        values_len: 3,
    };
    let msg4 = err4.to_string();
    assert!(
        msg4.contains("5") && msg4.contains("3"),
        "Message should contain lengths: {}",
        msg4
    );
}

// =============================================================================
// Additional Boundary Cases from Task Spec
// =============================================================================

/// Test empty dense E1 (0 dimensions).
#[test]
fn test_semantic_fingerprint_validate_empty_dense_e1() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e1_semantic = vec![]; // Empty

    let err = fp.validate().unwrap_err();
    match err {
        ValidationError::DimensionMismatch {
            embedder,
            expected,
            actual,
        } => {
            assert_eq!(embedder, Embedder::Semantic);
            assert_eq!(expected, 1024);
            assert_eq!(actual, 0);
        }
        _ => panic!("Expected DimensionMismatch, got {:?}", err),
    }
}

/// Test E9 is dense (not binary) - confirms 1024D dense projected.
#[test]
fn test_semantic_fingerprint_validate_e9_is_dense_not_binary() {
    // E9 HDC is stored as 1024D dense (projected from 10K-bit)
    let mut fp = SemanticFingerprint::zeroed();
    fp.e9_hdc = vec![0.0; E9_DIM];
    assert!(fp.validate().is_ok(), "E9 should be 1024D dense");

    // Confirm dimension constant
    assert_eq!(E9_DIM, 1024, "E9_DIM should be 1024");
}

/// Test validate_strict() exists and works (from fingerprint.rs).
#[test]
fn test_semantic_fingerprint_validate_strict_exists() {
    let fp = SemanticFingerprint::zeroed();
    // validate_strict is the same as validate() - both return ValidationError
    assert!(fp.validate_strict().is_ok());

    let mut fp2 = SemanticFingerprint::zeroed();
    fp2.e1_semantic = vec![0.0; 100];
    assert!(fp2.validate_strict().is_err());
}

/// Test max valid sparse index (30521) passes.
#[test]
fn test_semantic_fingerprint_validate_sparse_max_valid_index() {
    let mut fp = SemanticFingerprint::zeroed();
    // 30521 is the maximum valid index (vocab size is 30522, so indices must be < 30522)
    fp.e6_sparse.indices = vec![30521];
    fp.e6_sparse.values = vec![0.5];
    assert!(fp.validate().is_ok(), "Index 30521 should be valid");
}
