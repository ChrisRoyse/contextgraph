//! Tests for TASK-CORE-003: SemanticFingerprint enhancements.
//!
//! These tests verify:
//! 1. EmbeddingRef enum type-safe access
//! 2. ValidationError structured error handling
//! 3. is_complete() method
//! 4. storage_bytes() method
//! 5. validate_strict() method
//! 6. Serialization roundtrips (bincode)
//! 7. Edge cases for sparse and token-level embeddings

use crate::teleological::Embedder;
use crate::types::fingerprint::semantic::fingerprint::{EmbeddingRef, ValidationError};
use crate::types::fingerprint::semantic::{
    SemanticFingerprint, TeleologicalArray, E10_DIM, E11_DIM, E12_TOKEN_DIM, E1_DIM, E2_DIM,
    E3_DIM, E4_DIM, E5_DIM, E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM,
};
use crate::types::fingerprint::SparseVector;

/// Test: EmbeddingRef correctly categorizes all 13 embeddings.
#[test]
fn test_embedding_ref_categorization() {
    let fp = SemanticFingerprint::zeroed();

    // Dense embeddings (E1, E2-E5, E7-E11)
    for e in [
        Embedder::Semantic,
        Embedder::TemporalRecent,
        Embedder::TemporalPeriodic,
        Embedder::TemporalPositional,
        Embedder::Causal,
        Embedder::Code,
        Embedder::Emotional,
        Embedder::Hdc,
        Embedder::Multimodal,
        Embedder::Entity,
    ] {
        match fp.get(e) {
            EmbeddingRef::Dense(_) => {}
            other => panic!("{:?} should be Dense, got {:?}", e, other),
        }
    }

    // Sparse embeddings (E6, E13)
    match fp.get(Embedder::Sparse) {
        EmbeddingRef::Sparse(_) => {}
        other => panic!("E6 should be Sparse, got {:?}", other),
    }
    match fp.get(Embedder::KeywordSplade) {
        EmbeddingRef::Sparse(_) => {}
        other => panic!("E13 should be Sparse, got {:?}", other),
    }

    // Token-level embedding (E12)
    match fp.get(Embedder::LateInteraction) {
        EmbeddingRef::TokenLevel(_) => {}
        other => panic!("E12 should be TokenLevel, got {:?}", other),
    }

    println!("[PASS] All 13 embeddings correctly categorized by EmbeddingRef");
}

/// Test: get() returns correct dimensions for each embedder.
#[test]
fn test_get_returns_correct_dimensions() {
    let fp = SemanticFingerprint::zeroed();

    // Check each dense embedding has correct dimension
    let expected_dims = [
        (Embedder::Semantic, E1_DIM),
        (Embedder::TemporalRecent, E2_DIM),
        (Embedder::TemporalPeriodic, E3_DIM),
        (Embedder::TemporalPositional, E4_DIM),
        (Embedder::Causal, E5_DIM),
        (Embedder::Code, E7_DIM),
        (Embedder::Emotional, E8_DIM),
        (Embedder::Hdc, E9_DIM),
        (Embedder::Multimodal, E10_DIM),
        (Embedder::Entity, E11_DIM),
    ];

    for (embedder, expected) in expected_dims {
        if let EmbeddingRef::Dense(data) = fp.get(embedder) {
            assert_eq!(
                data.len(),
                expected,
                "{:?} dimension mismatch: expected {}, got {}",
                embedder,
                expected,
                data.len()
            );
        } else {
            panic!("{:?} should return Dense", embedder);
        }
    }

    println!("[PASS] All dense embeddings have correct dimensions");
}

/// Test: is_complete() returns true for valid zeroed fingerprint.
#[test]
fn test_is_complete_zeroed() {
    let fp = SemanticFingerprint::zeroed();
    assert!(fp.is_complete(), "zeroed fingerprint should be complete");
    println!("[PASS] zeroed fingerprint is_complete() returns true");
}

/// Test: is_complete() returns false for invalid dimensions.
#[test]
fn test_is_complete_invalid_dimensions() {
    let mut fp = SemanticFingerprint::zeroed();

    // Corrupt E1 dimension
    fp.e1_semantic = vec![0.0; 100]; // Wrong dimension
    assert!(
        !fp.is_complete(),
        "fingerprint with wrong E1 dimension should not be complete"
    );

    // Fix E1, corrupt E7
    fp.e1_semantic = vec![0.0; E1_DIM];
    fp.e7_code = vec![0.0; 512]; // Wrong dimension
    assert!(
        !fp.is_complete(),
        "fingerprint with wrong E7 dimension should not be complete"
    );

    println!("[PASS] is_complete() correctly detects invalid dimensions");
}

/// Test: storage_bytes() returns correct size.
#[test]
fn test_storage_bytes() {
    let fp = SemanticFingerprint::zeroed();
    let bytes = fp.storage_bytes();

    // Calculate expected size:
    // Dense: 10 embeddings
    let dense_floats =
        E1_DIM + E2_DIM + E3_DIM + E4_DIM + E5_DIM + E7_DIM + E8_DIM + E9_DIM + E10_DIM + E11_DIM;
    let expected_dense_bytes = dense_floats * std::mem::size_of::<f32>();

    // Sparse: empty = 0 bytes each
    // Token-level: empty = 0 bytes

    assert_eq!(
        bytes, expected_dense_bytes,
        "storage_bytes mismatch: expected {}, got {}",
        expected_dense_bytes, bytes
    );

    println!(
        "[PASS] storage_bytes() = {} (expected {})",
        bytes, expected_dense_bytes
    );
}

/// Test: storage_bytes() with non-empty sparse vectors.
#[test]
fn test_storage_bytes_with_sparse() {
    let mut fp = SemanticFingerprint::zeroed();

    // Add sparse entries
    fp.e6_sparse = SparseVector::new(vec![10, 100, 500], vec![0.5, 0.3, 0.8]).unwrap();
    fp.e13_splade = SparseVector::new(vec![1, 2, 3, 4], vec![0.1, 0.2, 0.3, 0.4]).unwrap();

    let bytes = fp.storage_bytes();

    // Calculate expected size:
    let dense_floats =
        E1_DIM + E2_DIM + E3_DIM + E4_DIM + E5_DIM + E7_DIM + E8_DIM + E9_DIM + E10_DIM + E11_DIM;
    let dense_bytes = dense_floats * std::mem::size_of::<f32>();
    let e6_bytes = 3 * (std::mem::size_of::<u16>() + std::mem::size_of::<f32>());
    let e13_bytes = 4 * (std::mem::size_of::<u16>() + std::mem::size_of::<f32>());
    let expected = dense_bytes + e6_bytes + e13_bytes;

    assert_eq!(
        bytes, expected,
        "storage_bytes with sparse mismatch: expected {}, got {}",
        expected, bytes
    );

    println!("[PASS] storage_bytes() with sparse = {}", bytes);
}

/// Test: validate_strict() passes for valid fingerprint.
#[test]
fn test_validate_strict_valid() {
    let fp = SemanticFingerprint::zeroed();
    let result = fp.validate_strict();
    assert!(
        result.is_ok(),
        "zeroed fingerprint should validate: {:?}",
        result
    );
    println!("[PASS] validate_strict() passes for valid fingerprint");
}

/// Test: validate_strict() fails for wrong E1 dimension.
#[test]
fn test_validate_strict_wrong_e1_dimension() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e1_semantic = vec![0.0; 512]; // Wrong dimension

    let result = fp.validate_strict();
    assert!(result.is_err(), "should fail validation for wrong E1 dim");

    match result.unwrap_err() {
        ValidationError::DimensionMismatch {
            embedder,
            expected,
            actual,
        } => {
            assert_eq!(embedder, Embedder::Semantic);
            assert_eq!(expected, E1_DIM);
            assert_eq!(actual, 512);
            println!(
                "[PASS] DimensionMismatch error for E1: expected {}, got {}",
                expected, actual
            );
        }
        other => panic!("Expected DimensionMismatch, got {:?}", other),
    }
}

/// Test: validate_strict() fails for wrong E12 token dimension.
#[test]
fn test_validate_strict_wrong_token_dimension() {
    let mut fp = SemanticFingerprint::zeroed();
    // Add tokens with wrong dimension
    fp.e12_late_interaction = vec![
        vec![0.0; E12_TOKEN_DIM], // Valid
        vec![0.0; 64],            // Wrong dimension
    ];

    let result = fp.validate_strict();
    assert!(
        result.is_err(),
        "should fail validation for wrong token dim"
    );

    match result.unwrap_err() {
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
            println!(
                "[PASS] TokenDimensionMismatch error: token {} expected {}, got {}",
                token_index, expected, actual
            );
        }
        other => panic!("Expected TokenDimensionMismatch, got {:?}", other),
    }
}

/// Test: validate_strict() fails for out-of-bounds sparse index.
#[test]
fn test_validate_strict_sparse_out_of_bounds() {
    let mut fp = SemanticFingerprint::zeroed();
    // Manually create invalid sparse vector (bypassing SparseVector::new validation)
    fp.e6_sparse = SparseVector {
        indices: vec![50000], // Out of bounds
        values: vec![0.5],
    };

    let result = fp.validate_strict();
    assert!(
        result.is_err(),
        "should fail validation for out-of-bounds sparse index"
    );

    match result.unwrap_err() {
        ValidationError::SparseIndexOutOfBounds {
            embedder,
            index,
            vocab_size,
        } => {
            assert_eq!(embedder, Embedder::Sparse);
            assert_eq!(index, 50000);
            assert_eq!(vocab_size, E6_SPARSE_VOCAB);
            println!("[PASS] SparseIndexOutOfBounds for E6");
        }
        other => panic!("Expected SparseIndexOutOfBounds, got {:?}", other),
    }
}

/// Test: TeleologicalArray is an alias for SemanticFingerprint.
#[test]
fn test_teleological_array_alias() {
    let fp: TeleologicalArray = SemanticFingerprint::zeroed();
    assert!(fp.is_complete());
    assert!(fp.validate_strict().is_ok());
    println!("[PASS] TeleologicalArray alias works correctly");
}

/// Test: bincode serialization roundtrip.
#[test]
fn test_bincode_serialization_roundtrip() {
    let fp = SemanticFingerprint::zeroed();
    let serialized = bincode::serialize(&fp).expect("bincode serialize failed");
    let deserialized: SemanticFingerprint =
        bincode::deserialize(&serialized).expect("bincode deserialize failed");

    // Verify all fields match
    assert_eq!(fp.e1_semantic, deserialized.e1_semantic);
    assert_eq!(fp.e2_temporal_recent, deserialized.e2_temporal_recent);
    assert_eq!(fp.e3_temporal_periodic, deserialized.e3_temporal_periodic);
    assert_eq!(
        fp.e4_temporal_positional,
        deserialized.e4_temporal_positional
    );
    assert_eq!(fp.e5_causal, deserialized.e5_causal);
    assert_eq!(fp.e6_sparse, deserialized.e6_sparse);
    assert_eq!(fp.e7_code, deserialized.e7_code);
    assert_eq!(fp.e8_graph, deserialized.e8_graph);
    assert_eq!(fp.e9_hdc, deserialized.e9_hdc);
    assert_eq!(fp.e10_multimodal, deserialized.e10_multimodal);
    assert_eq!(fp.e11_entity, deserialized.e11_entity);
    assert_eq!(fp.e12_late_interaction, deserialized.e12_late_interaction);
    assert_eq!(fp.e13_splade, deserialized.e13_splade);

    assert!(deserialized.is_complete());
    println!("[PASS] bincode serialization roundtrip successful");
}

/// Test: bincode serialization with non-empty sparse and token data.
#[test]
fn test_bincode_serialization_with_data() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e6_sparse = SparseVector::new(vec![10, 100, 500], vec![0.5, 0.3, 0.8]).unwrap();
    fp.e12_late_interaction = vec![vec![1.0; E12_TOKEN_DIM], vec![2.0; E12_TOKEN_DIM]];
    fp.e13_splade = SparseVector::new(vec![1, 2], vec![0.1, 0.2]).unwrap();

    let serialized = bincode::serialize(&fp).expect("serialize failed");
    let deserialized: SemanticFingerprint =
        bincode::deserialize(&serialized).expect("deserialize failed");

    assert_eq!(fp.e6_sparse, deserialized.e6_sparse);
    assert_eq!(fp.e12_late_interaction, deserialized.e12_late_interaction);
    assert_eq!(fp.e13_splade, deserialized.e13_splade);
    assert!(deserialized.validate_strict().is_ok());

    println!("[PASS] bincode serialization with data roundtrip successful");
}

/// Test: ValidationError Display trait.
#[test]
fn test_validation_error_display() {
    let err = ValidationError::DimensionMismatch {
        embedder: Embedder::Semantic,
        expected: 1024,
        actual: 512,
    };
    let msg = format!("{}", err);
    assert!(msg.contains("E1_Semantic"), "msg was: {}", msg);
    assert!(msg.contains("1024"), "msg was: {}", msg);
    assert!(msg.contains("512"), "msg was: {}", msg);
    println!("[PASS] ValidationError Display: {}", msg);

    let err2 = ValidationError::TokenDimensionMismatch {
        embedder: Embedder::LateInteraction,
        token_index: 5,
        expected: 128,
        actual: 64,
    };
    let msg2 = format!("{}", err2);
    assert!(
        msg2.contains("Token 5") || msg2.contains("token 5"),
        "msg2 was: {}",
        msg2
    );
    assert!(msg2.contains("128"), "msg2 was: {}", msg2);
    println!("[PASS] TokenDimensionMismatch Display: {}", msg2);
}

/// Test: Edge case - all embedders return some value via get().
#[test]
fn test_all_embedders_via_get() {
    let fp = SemanticFingerprint::zeroed();

    // Verify all 13 embedders can be accessed
    for embedder in Embedder::all() {
        let _ = fp.get(embedder); // Should not panic
    }

    println!("[PASS] All 13 embedders accessible via get()");
}

/// Test: Edge case - empty token-level embedding is valid.
#[test]
fn test_empty_token_level_valid() {
    let fp = SemanticFingerprint::zeroed();
    assert!(fp.e12_late_interaction.is_empty());
    assert!(fp.validate_strict().is_ok());
    println!("[PASS] Empty token-level embedding is valid");
}

/// Test: Edge case - empty sparse vectors are valid.
#[test]
fn test_empty_sparse_valid() {
    let fp = SemanticFingerprint::zeroed();
    assert!(fp.e6_sparse.is_empty());
    assert!(fp.e13_splade.is_empty());
    assert!(fp.validate_strict().is_ok());
    println!("[PASS] Empty sparse vectors are valid");
}

/// Test: Edge case - maximum valid sparse index (30521).
#[test]
fn test_max_sparse_index_valid() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e6_sparse = SparseVector::new(vec![0, 30521], vec![0.1, 0.9]).unwrap();
    fp.e13_splade = SparseVector::new(vec![30521], vec![0.5]).unwrap();

    assert!(fp.validate_strict().is_ok());
    println!("[PASS] Maximum sparse index (30521) is valid");
}

/// Test: consistency between storage_size() and storage_bytes().
#[test]
fn test_storage_size_bytes_consistency() {
    let fp = SemanticFingerprint::zeroed();
    assert_eq!(
        fp.storage_size(),
        fp.storage_bytes(),
        "storage_size() and storage_bytes() should be equal"
    );
    println!("[PASS] storage_size() == storage_bytes()");
}

/// Test: JSON serialization roundtrip.
#[test]
fn test_json_serialization_roundtrip() {
    let fp = SemanticFingerprint::zeroed();
    let json = serde_json::to_string(&fp).expect("JSON serialize failed");
    let deserialized: SemanticFingerprint =
        serde_json::from_str(&json).expect("JSON deserialize failed");

    assert_eq!(fp, deserialized);
    println!("[PASS] JSON serialization roundtrip successful");
}

/// Test: EmbedderDims matches get() return type.
#[test]
fn test_embedder_dims_match_get_type() {
    use crate::teleological::EmbedderDims;

    let fp = SemanticFingerprint::zeroed();

    for embedder in Embedder::all() {
        let dims = embedder.expected_dims();
        let ref_type = fp.get(embedder);

        match (dims, ref_type) {
            (EmbedderDims::Dense(_), EmbeddingRef::Dense(_)) => {}
            (EmbedderDims::Sparse { .. }, EmbeddingRef::Sparse(_)) => {}
            (EmbedderDims::TokenLevel { .. }, EmbeddingRef::TokenLevel(_)) => {}
            (dims, ref_type) => {
                panic!(
                    "{:?}: dims {:?} doesn't match ref_type {:?}",
                    embedder, dims, ref_type
                );
            }
        }
    }

    println!("[PASS] All EmbedderDims match get() return types");
}

/// Test: Unsorted sparse indices pass validation.
///
/// Sortedness is enforced at construction time by `SparseVector::new()`.
/// The validation layer only checks bounds and length matching.
/// This is by design - validation is for storage invariants, not usage invariants.
#[test]
fn test_validate_strict_sparse_unsorted_passes() {
    let mut fp = SemanticFingerprint::zeroed();
    // Create unsorted indices (bypassing SparseVector::new validation)
    // These are in-bounds and have matching lengths, so validation passes
    fp.e6_sparse = SparseVector {
        indices: vec![100, 50], // Unsorted but in-bounds
        values: vec![0.5, 0.3],
    };

    // Validation passes because bounds and length match
    // Sortedness is a construction-time invariant, not a storage invariant
    let result = fp.validate_strict();
    assert!(result.is_ok(), "unsorted but in-bounds indices should pass validation");
    println!("[PASS] Unsorted indices pass validation (sortedness enforced at construction)");
}

/// Test: Duplicate sparse indices pass validation.
///
/// Uniqueness is enforced at construction time by `SparseVector::new()`.
/// The validation layer only checks bounds and length matching.
#[test]
fn test_validate_strict_sparse_duplicate_passes() {
    let mut fp = SemanticFingerprint::zeroed();
    // Create duplicate indices (bypassing SparseVector::new validation)
    // These are in-bounds and have matching lengths, so validation passes
    fp.e13_splade = SparseVector {
        indices: vec![100, 100], // Duplicate but in-bounds
        values: vec![0.5, 0.3],
    };

    // Validation passes because bounds and length match
    // Uniqueness is a construction-time invariant, not a storage invariant
    let result = fp.validate_strict();
    assert!(result.is_ok(), "duplicate but in-bounds indices should pass validation");
    println!("[PASS] Duplicate indices pass validation (uniqueness enforced at construction)");
}

/// Test: validate_strict() validates mismatched sparse lengths.
#[test]
fn test_validate_strict_sparse_length_mismatch() {
    let mut fp = SemanticFingerprint::zeroed();
    // Create length mismatch
    fp.e6_sparse = SparseVector {
        indices: vec![10, 20, 30],
        values: vec![0.5, 0.3], // Missing one value
    };

    let result = fp.validate_strict();
    assert!(result.is_err(), "should fail for length mismatch");

    match result.unwrap_err() {
        ValidationError::SparseIndicesValuesMismatch {
            embedder,
            indices_len,
            values_len,
        } => {
            assert_eq!(embedder, Embedder::Sparse);
            assert_eq!(indices_len, 3);
            assert_eq!(values_len, 2);
            println!("[PASS] SparseIndicesValuesMismatch for length mismatch");
        }
        other => panic!("Expected SparseIndicesValuesMismatch, got {:?}", other),
    }
}
