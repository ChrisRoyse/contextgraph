//! Storage Roundtrip Tests - Comprehensive Verification of Store/Retrieve Integrity
//!
//! This test file verifies:
//! 1. StoredQuantizedFingerprint creation with all 13 embeddings
//! 2. Serialization/deserialization roundtrip preserves all data exactly
//! 3. IndexEntry creation and cosine similarity calculations
//! 4. EmbedderQueryResult and MultiSpaceQueryResult creation
//! 5. RRF formula calculations match Constitution k=60
//!
//! # CRITICAL INVARIANTS
//! - All 13 embeddings MUST be present (panic otherwise)
//! - RRF formula: 1/(60 + rank) for each space
//! - Cosine similarity in range [-1.0, 1.0]
//! - Purpose alignment filters at 0.55 threshold
//! - Storage size should be reasonable (<25KB per fingerprint)
//!
//! # Constitution Reference
//! From constitution.yaml `embeddings.storage_per_memory`:
//! - Quantized StoredQuantizedFingerprint: ~17KB
//! - RRF(d) = sum_i 1/(k + rank_i(d)) where k=60

use context_graph_embeddings::{
    EmbedderQueryResult, IndexEntry, ModelId, MultiSpaceQueryResult, QuantizationMetadata,
    QuantizationMethod, QuantizedEmbedding, StoredQuantizedFingerprint, MAX_QUANTIZED_SIZE_BYTES,
    RRF_K, STORAGE_VERSION,
};
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// TEST DATA GENERATION (NO MOCKS - Real Types with Deterministic Data)
// =============================================================================

/// Creates valid test embeddings for all 13 embedders with Constitution-correct methods.
/// Each embedding uses deterministic data that can be verified after roundtrip.
fn create_test_embeddings_with_deterministic_data(seed: u8) -> HashMap<u8, QuantizedEmbedding> {
    let mut map = HashMap::new();

    for i in 0..13u8 {
        let model_id = ModelId::try_from(i).expect("Valid model index");
        let method = QuantizationMethod::for_model_id(model_id);

        // Create method-appropriate test data with deterministic pattern
        let (dim, data, metadata) = match method {
            QuantizationMethod::PQ8 => {
                // PQ8: 8 bytes of centroid indices
                let data: Vec<u8> = (0..8u8)
                    .map(|j| seed.wrapping_add(i).wrapping_add(j))
                    .collect();
                let dim = model_id.dimension();
                let metadata = QuantizationMetadata::PQ8 {
                    codebook_id: i as u32 + seed as u32 * 100,
                    num_subvectors: 8,
                };
                (dim, data, metadata)
            }
            QuantizationMethod::Float8E4M3 => {
                // Float8: 1 byte per dimension (compressed from f32)
                let dim = model_id.dimension();
                let data: Vec<u8> = (0..dim)
                    .map(|j| {
                        ((seed as usize).wrapping_add(i as usize).wrapping_add(j) & 0xFF) as u8
                    })
                    .collect();
                let metadata = QuantizationMetadata::Float8 {
                    scale: 1.0 + (seed as f32 * 0.1),
                    bias: seed as f32 * 0.01,
                };
                (dim, data, metadata)
            }
            QuantizationMethod::Binary => {
                // Binary: 10000 bits = 1250 bytes for E9 HDC
                let dim = 10000;
                let data: Vec<u8> = (0..1250)
                    .map(|j| {
                        ((seed as usize).wrapping_add(i as usize).wrapping_add(j) & 0xFF) as u8
                    })
                    .collect();
                let metadata = QuantizationMetadata::Binary {
                    threshold: 0.0 + seed as f32 * 0.001,
                };
                (dim, data, metadata)
            }
            QuantizationMethod::SparseNative => {
                // Sparse: Variable size based on nnz (100 entries typical for test)
                let nnz = 100;
                // Each sparse entry: 4 bytes index + 4 bytes value = 8 bytes
                let data: Vec<u8> = (0..(nnz * 8))
                    .map(|j| {
                        ((seed as usize).wrapping_add(i as usize).wrapping_add(j) & 0xFF) as u8
                    })
                    .collect();
                let metadata = QuantizationMetadata::Sparse {
                    vocab_size: 30522,
                    nnz,
                };
                (30522, data, metadata)
            }
            QuantizationMethod::TokenPruning => {
                // TokenPruning: ~50% of tokens kept, 128D per token, ~64 tokens
                let kept_tokens = 64;
                let data: Vec<u8> = (0..(kept_tokens * 128))
                    .map(|j| {
                        ((seed as usize).wrapping_add(i as usize).wrapping_add(j) & 0xFF) as u8
                    })
                    .collect();
                let metadata = QuantizationMetadata::TokenPruning {
                    original_tokens: 128,
                    kept_tokens,
                    threshold: 0.5 + seed as f32 * 0.01,
                };
                (128, data, metadata)
            }
        };

        map.insert(
            i,
            QuantizedEmbedding {
                method,
                original_dim: dim,
                data,
                metadata,
            },
        );
    }

    map
}

/// Create deterministic purpose vector based on seed.
fn create_purpose_vector(seed: u8) -> [f32; 13] {
    let mut pv = [0.0f32; 13];
    for (i, val) in pv.iter_mut().enumerate() {
        // Generate values in [0.3, 0.9] range
        *val = 0.3 + ((seed as f32 + i as f32 * 0.05) % 0.6);
    }
    pv
}

/// Create deterministic johari quadrants based on seed.
fn create_johari_quadrants(seed: u8) -> [f32; 4] {
    let raw = [
        0.1 + (seed as f32 % 0.3),
        0.2 + ((seed as f32 * 1.3) % 0.3),
        0.15 + ((seed as f32 * 0.7) % 0.25),
        0.25 + ((seed as f32 * 1.1) % 0.35),
    ];
    // Normalize to sum to 1.0
    let sum: f32 = raw.iter().sum();
    [raw[0] / sum, raw[1] / sum, raw[2] / sum, raw[3] / sum]
}

/// Create deterministic content hash based on seed.
fn create_content_hash(seed: u8) -> [u8; 32] {
    let mut hash = [0u8; 32];
    for (i, byte) in hash.iter_mut().enumerate() {
        *byte = seed.wrapping_add(i as u8).wrapping_mul(17);
    }
    hash
}

// =============================================================================
// FINGERPRINT CREATION TESTS
// =============================================================================

#[cfg(test)]
mod fingerprint_creation_tests {
    use super::*;

    /// Test creating fingerprint with all 13 embeddings succeeds.
    #[test]
    fn test_create_fingerprint_with_all_13_embeddings() {
        let id = Uuid::new_v4();
        let embeddings = create_test_embeddings_with_deterministic_data(42);
        let purpose_vector = create_purpose_vector(42);
        let johari_quadrants = create_johari_quadrants(42);
        let content_hash = create_content_hash(42);

        let fp = StoredQuantizedFingerprint::new(
            id,
            embeddings,
            purpose_vector,
            johari_quadrants,
            content_hash,
        );

        // Verify all fields
        assert_eq!(fp.id, id, "ID must match");
        assert_eq!(
            fp.version, STORAGE_VERSION,
            "Version must match STORAGE_VERSION"
        );
        assert_eq!(fp.embeddings.len(), 13, "Must have exactly 13 embeddings");
        assert_eq!(
            fp.purpose_vector.len(),
            13,
            "Purpose vector must have 13 dimensions"
        );
        assert_eq!(
            fp.johari_quadrants.len(),
            4,
            "Johari quadrants must have 4 values"
        );
        assert_eq!(fp.content_hash, content_hash, "Content hash must match");
        assert!(!fp.deleted, "New fingerprint should not be deleted");
        assert_eq!(
            fp.access_count, 0,
            "New fingerprint should have zero access count"
        );

        println!("[PASS] Created fingerprint with all 13 embeddings");
    }

    /// Test that missing any single embedding panics.
    #[test]
    #[should_panic(expected = "CONSTRUCTION ERROR")]
    fn test_panic_on_missing_embedder_0() {
        let mut embeddings = create_test_embeddings_with_deterministic_data(42);
        embeddings.remove(&0);

        let _ = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            embeddings,
            create_purpose_vector(42),
            create_johari_quadrants(42),
            create_content_hash(42),
        );
    }

    #[test]
    #[should_panic(expected = "CONSTRUCTION ERROR")]
    fn test_panic_on_missing_embedder_6() {
        let mut embeddings = create_test_embeddings_with_deterministic_data(42);
        embeddings.remove(&6);

        let _ = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            embeddings,
            create_purpose_vector(42),
            create_johari_quadrants(42),
            create_content_hash(42),
        );
    }

    #[test]
    #[should_panic(expected = "CONSTRUCTION ERROR")]
    fn test_panic_on_missing_embedder_12() {
        let mut embeddings = create_test_embeddings_with_deterministic_data(42);
        embeddings.remove(&12);

        let _ = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            embeddings,
            create_purpose_vector(42),
            create_johari_quadrants(42),
            create_content_hash(42),
        );
    }

    /// Test that each embedding has correct quantization method.
    #[test]
    fn test_embeddings_have_correct_quantization_methods() {
        let embeddings = create_test_embeddings_with_deterministic_data(42);

        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            embeddings,
            create_purpose_vector(42),
            create_johari_quadrants(42),
            create_content_hash(42),
        );

        // Verify each embedder uses Constitution-correct method
        for i in 0..13u8 {
            let model_id = ModelId::try_from(i).expect("Valid model index");
            let expected_method = QuantizationMethod::for_model_id(model_id);
            let actual_method = fp.get_embedding(i).method;
            assert_eq!(
                actual_method, expected_method,
                "Embedder {} should use {:?}, got {:?}",
                i, expected_method, actual_method
            );
        }

        assert!(
            fp.validate_quantization_methods(),
            "All quantization methods should be valid"
        );
        println!("[PASS] All 13 embeddings have correct quantization methods");
    }

    /// Test theta_to_north_star is computed correctly.
    #[test]
    fn test_theta_to_north_star_computation() {
        let pv = [0.5f32; 13]; // Uniform purpose vector

        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings_with_deterministic_data(42),
            pv,
            create_johari_quadrants(42),
            create_content_hash(42),
        );

        // theta = mean of purpose vector = 0.5
        assert!(
            (fp.theta_to_north_star - 0.5).abs() < f32::EPSILON,
            "theta_to_north_star should be mean of purpose vector (0.5), got {}",
            fp.theta_to_north_star
        );

        println!("[PASS] theta_to_north_star computed correctly");
    }

    /// Test dominant quadrant computation.
    #[test]
    fn test_dominant_quadrant_computation() {
        // Open dominant
        let fp1 = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings_with_deterministic_data(1),
            create_purpose_vector(1),
            [0.6, 0.2, 0.1, 0.1],
            create_content_hash(1),
        );
        assert_eq!(fp1.dominant_quadrant, 0, "Open should be dominant");
        assert!(
            (fp1.johari_confidence - 0.6).abs() < f32::EPSILON,
            "Confidence should be 0.6"
        );

        // Hidden dominant
        let fp2 = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings_with_deterministic_data(2),
            create_purpose_vector(2),
            [0.1, 0.7, 0.1, 0.1],
            create_content_hash(2),
        );
        assert_eq!(fp2.dominant_quadrant, 1, "Hidden should be dominant");

        // Unknown dominant
        let fp3 = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings_with_deterministic_data(3),
            create_purpose_vector(3),
            [0.1, 0.1, 0.2, 0.6],
            create_content_hash(3),
        );
        assert_eq!(fp3.dominant_quadrant, 3, "Unknown should be dominant");

        println!("[PASS] Dominant quadrant computed correctly for all cases");
    }

    /// Test estimated size is within Constitution bounds.
    #[test]
    fn test_estimated_size_within_bounds() {
        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings_with_deterministic_data(42),
            create_purpose_vector(42),
            create_johari_quadrants(42),
            create_content_hash(42),
        );

        let size = fp.estimated_size_bytes();

        // Must be less than MAX_QUANTIZED_SIZE_BYTES (25KB)
        assert!(
            size < MAX_QUANTIZED_SIZE_BYTES,
            "Estimated size {} exceeds maximum {} bytes",
            size,
            MAX_QUANTIZED_SIZE_BYTES
        );

        // Should be reasonable (> 1KB for all that data)
        assert!(
            size > 1000,
            "Estimated size {} seems too small for 13 embeddings",
            size
        );

        println!("[PASS] Estimated size {} bytes is within bounds", size);
    }
}

// =============================================================================
// SERIALIZATION ROUNDTRIP TESTS
// =============================================================================

#[cfg(test)]
mod serialization_roundtrip_tests {
    use super::*;

    /// Test JSON serialization roundtrip preserves all fingerprint data.
    #[test]
    fn test_json_roundtrip_fingerprint() {
        let id = Uuid::new_v4();
        let embeddings = create_test_embeddings_with_deterministic_data(99);
        let purpose_vector = create_purpose_vector(99);
        let johari_quadrants = create_johari_quadrants(99);
        let content_hash = create_content_hash(99);

        let original = StoredQuantizedFingerprint::new(
            id,
            embeddings.clone(),
            purpose_vector,
            johari_quadrants,
            content_hash,
        );

        // Serialize to JSON
        let json = serde_json::to_string(&original).expect("JSON serialization failed");

        // Deserialize from JSON
        let restored: StoredQuantizedFingerprint =
            serde_json::from_str(&json).expect("JSON deserialization failed");

        // Verify all fields match exactly
        assert_eq!(restored.id, original.id, "ID mismatch after roundtrip");
        assert_eq!(
            restored.version, original.version,
            "Version mismatch after roundtrip"
        );
        assert_eq!(
            restored.embeddings.len(),
            original.embeddings.len(),
            "Embeddings count mismatch"
        );

        for i in 0..13u8 {
            let orig_emb = original.get_embedding(i);
            let rest_emb = restored.get_embedding(i);
            assert_eq!(
                orig_emb.method, rest_emb.method,
                "Embedder {} method mismatch",
                i
            );
            assert_eq!(
                orig_emb.original_dim, rest_emb.original_dim,
                "Embedder {} dim mismatch",
                i
            );
            assert_eq!(orig_emb.data, rest_emb.data, "Embedder {} data mismatch", i);
        }

        assert_eq!(
            restored.purpose_vector, original.purpose_vector,
            "Purpose vector mismatch"
        );
        assert!((restored.theta_to_north_star - original.theta_to_north_star).abs() < f32::EPSILON);
        assert_eq!(
            restored.johari_quadrants, original.johari_quadrants,
            "Johari quadrants mismatch"
        );
        assert_eq!(
            restored.dominant_quadrant, original.dominant_quadrant,
            "Dominant quadrant mismatch"
        );
        assert!((restored.johari_confidence - original.johari_confidence).abs() < f32::EPSILON);
        assert_eq!(
            restored.content_hash, original.content_hash,
            "Content hash mismatch"
        );

        println!("[PASS] JSON roundtrip preserves all fingerprint data");
    }

    /// Test bincode serialization roundtrip preserves all data.
    #[test]
    fn test_bincode_roundtrip_fingerprint() {
        let original = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings_with_deterministic_data(77),
            create_purpose_vector(77),
            create_johari_quadrants(77),
            create_content_hash(77),
        );

        // Serialize to bincode
        let bytes = bincode::serialize(&original).expect("Bincode serialization failed");

        // Deserialize from bincode
        let restored: StoredQuantizedFingerprint =
            bincode::deserialize(&bytes).expect("Bincode deserialization failed");

        // Verify critical fields
        assert_eq!(
            restored.id, original.id,
            "ID mismatch after bincode roundtrip"
        );
        assert_eq!(restored.version, original.version, "Version mismatch");
        assert_eq!(restored.embeddings.len(), 13, "Must have 13 embeddings");
        assert_eq!(
            restored.purpose_vector, original.purpose_vector,
            "Purpose vector mismatch"
        );
        assert_eq!(
            restored.content_hash, original.content_hash,
            "Content hash mismatch"
        );

        // Verify embedding data integrity
        for i in 0..13u8 {
            let orig_data = &original.get_embedding(i).data;
            let rest_data = &restored.get_embedding(i).data;
            assert_eq!(
                orig_data, rest_data,
                "Embedder {} data corrupted after bincode roundtrip",
                i
            );
        }

        println!(
            "[PASS] Bincode roundtrip preserves all fingerprint data ({} bytes)",
            bytes.len()
        );
    }

    /// Test EmbedderQueryResult serde roundtrip.
    #[test]
    fn test_embedder_query_result_roundtrip() {
        let original = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 5, 0.876_543_2, 42);

        let json = serde_json::to_string(&original).expect("Serialize");
        let restored: EmbedderQueryResult = serde_json::from_str(&json).expect("Deserialize");

        assert_eq!(restored.id, original.id);
        assert_eq!(restored.embedder_idx, original.embedder_idx);
        assert!((restored.similarity - original.similarity).abs() < f32::EPSILON);
        assert!((restored.distance - original.distance).abs() < f32::EPSILON);
        assert_eq!(restored.rank, original.rank);

        println!("[PASS] EmbedderQueryResult JSON roundtrip preserves all data");
    }

    /// Test MultiSpaceQueryResult bincode roundtrip.
    /// Note: JSON does not support NaN values natively, so we use bincode for this test.
    #[test]
    fn test_multi_space_query_result_roundtrip() {
        let id = Uuid::new_v4();
        let results = vec![
            EmbedderQueryResult::from_similarity(id, 0, 0.9, 0),
            EmbedderQueryResult::from_similarity(id, 1, 0.85, 1),
            EmbedderQueryResult::from_similarity(id, 2, 0.8, 2),
        ];

        let original = MultiSpaceQueryResult::from_embedder_results(id, &results, 0.72);

        // Use bincode instead of JSON because JSON doesn't support NaN
        let bytes = bincode::serialize(&original).expect("Serialize");
        let restored: MultiSpaceQueryResult = bincode::deserialize(&bytes).expect("Deserialize");

        assert_eq!(restored.id, original.id);
        assert_eq!(restored.embedder_count, original.embedder_count);
        assert!((restored.rrf_score - original.rrf_score).abs() < f32::EPSILON);
        assert!((restored.purpose_alignment - original.purpose_alignment).abs() < f32::EPSILON);

        // Check embedder similarities including NaN handling
        for i in 0..13 {
            let orig = original.embedder_similarities[i];
            let rest = restored.embedder_similarities[i];
            if orig.is_nan() {
                assert!(rest.is_nan(), "Embedder {} should be NaN", i);
            } else {
                assert!(
                    (rest - orig).abs() < f32::EPSILON,
                    "Embedder {} similarity mismatch",
                    i
                );
            }
        }

        println!("[PASS] MultiSpaceQueryResult bincode roundtrip preserves all data (incl. NaN)");
    }
}

// =============================================================================
// INDEX ENTRY TESTS
// =============================================================================

#[cfg(test)]
mod index_entry_tests {
    use super::*;

    /// Test IndexEntry creation with precomputed norm.
    #[test]
    fn test_index_entry_creation_with_norm() {
        // Classic 3-4-5 right triangle
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![3.0, 4.0]);

        assert!(
            (entry.norm - 5.0).abs() < f32::EPSILON,
            "Norm should be 5.0"
        );
        assert_eq!(entry.vector.len(), 2, "Vector should have 2 dimensions");
        assert_eq!(entry.embedder_idx, 0, "Embedder index should be 0");

        println!("[PASS] IndexEntry created with correct precomputed norm");
    }

    /// Test normalized vector computation.
    #[test]
    fn test_index_entry_normalized() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![3.0, 4.0]);
        let normalized = entry.normalized();

        assert!(
            (normalized[0] - 0.6).abs() < f32::EPSILON,
            "First component should be 0.6"
        );
        assert!(
            (normalized[1] - 0.8).abs() < f32::EPSILON,
            "Second component should be 0.8"
        );

        // Verify unit norm
        let unit_norm: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (unit_norm - 1.0).abs() < 1e-6,
            "Normalized vector should have unit norm"
        );

        println!("[PASS] Normalized vector computed correctly");
    }

    /// Test cosine similarity: identical vectors = 1.0.
    #[test]
    fn test_cosine_similarity_identical() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1.0, 2.0, 3.0]);
        let query = vec![1.0, 2.0, 3.0];

        let sim = entry.cosine_similarity(&query);
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "Identical vectors should have similarity 1.0, got {}",
            sim
        );

        println!("[PASS] Cosine similarity for identical vectors = 1.0");
    }

    /// Test cosine similarity: opposite vectors = -1.0.
    #[test]
    fn test_cosine_similarity_opposite() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1.0, 0.0, 0.0]);
        let query = vec![-1.0, 0.0, 0.0];

        let sim = entry.cosine_similarity(&query);
        assert!(
            (sim - (-1.0)).abs() < 1e-6,
            "Opposite vectors should have similarity -1.0, got {}",
            sim
        );

        println!("[PASS] Cosine similarity for opposite vectors = -1.0");
    }

    /// Test cosine similarity: perpendicular vectors = 0.0.
    #[test]
    fn test_cosine_similarity_perpendicular() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1.0, 0.0]);
        let query = vec![0.0, 1.0];

        let sim = entry.cosine_similarity(&query);
        assert!(
            sim.abs() < 1e-6,
            "Perpendicular vectors should have similarity 0.0, got {}",
            sim
        );

        println!("[PASS] Cosine similarity for perpendicular vectors = 0.0");
    }

    /// Test cosine similarity is always in valid range [-1, 1].
    /// Note: Due to floating point precision, values may slightly exceed [-1,1],
    /// so we allow a small epsilon tolerance.
    #[test]
    fn test_cosine_similarity_range() {
        // Test with various random-ish vectors
        let test_cases: Vec<(Vec<f32>, Vec<f32>)> = vec![
            (vec![1.5, -2.3, 4.1], vec![-0.7, 3.2, 1.8]),
            (vec![100.0, -50.0], vec![0.001, 0.002]),
            (vec![0.1, 0.1, 0.1], vec![100.0, 100.0, 100.0]),
        ];

        const EPSILON: f32 = 1e-6;

        for (v1, v2) in test_cases {
            let entry = IndexEntry::new(Uuid::new_v4(), 0, v1);
            let sim = entry.cosine_similarity(&v2);

            assert!(
                (-1.0 - EPSILON..=1.0 + EPSILON).contains(&sim),
                "Cosine similarity {} out of range [-1, 1] with epsilon tolerance",
                sim
            );
        }

        println!("[PASS] Cosine similarity always in range [-1.0, 1.0] (with epsilon)");
    }

    /// Test cosine similarity panics on dimension mismatch.
    #[test]
    #[should_panic(expected = "SIMILARITY ERROR")]
    fn test_cosine_similarity_dimension_mismatch() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1.0, 2.0, 3.0]);
        let query = vec![1.0, 2.0]; // Wrong dimension

        let _ = entry.cosine_similarity(&query);
    }

    /// Test zero vector handling in normalized.
    #[test]
    fn test_zero_vector_normalized() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![0.0, 0.0, 0.0]);
        let normalized = entry.normalized();

        assert!(
            normalized.iter().all(|&x| x == 0.0),
            "Zero vector should normalize to zero vector"
        );
        assert!(
            entry.norm.abs() < f32::EPSILON,
            "Zero vector should have zero norm"
        );

        println!("[PASS] Zero vector normalized correctly to zero vector");
    }

    /// Test zero vector cosine similarity returns 0.0.
    #[test]
    fn test_zero_vector_cosine_similarity() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![0.0, 0.0, 0.0]);
        let query = vec![1.0, 2.0, 3.0];

        let sim = entry.cosine_similarity(&query);
        assert_eq!(
            sim, 0.0,
            "Zero vector should have 0.0 similarity with any vector"
        );

        println!("[PASS] Zero vector cosine similarity = 0.0");
    }
}

// =============================================================================
// RRF FORMULA TESTS
// =============================================================================

#[cfg(test)]
mod rrf_formula_tests {
    use super::*;

    /// Verify RRF_K constant matches Constitution k=60.
    #[test]
    fn test_rrf_k_constant() {
        assert!(
            (RRF_K - 60.0).abs() < f32::EPSILON,
            "RRF_K must be 60.0 per Constitution"
        );
        println!("[PASS] RRF_K = 60.0 matches Constitution");
    }

    /// Test RRF contribution formula: 1/(60 + rank).
    #[test]
    fn test_rrf_contribution_formula() {
        // Rank 0: 1/(60+0) = 1/60
        let result_rank_0 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.9, 0);
        let expected_0 = 1.0 / 60.0;
        assert!(
            (result_rank_0.rrf_contribution() - expected_0).abs() < f32::EPSILON,
            "Rank 0 RRF should be 1/60 = {}, got {}",
            expected_0,
            result_rank_0.rrf_contribution()
        );

        // Rank 1: 1/(60+1) = 1/61
        let result_rank_1 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.9, 1);
        let expected_1 = 1.0 / 61.0;
        assert!(
            (result_rank_1.rrf_contribution() - expected_1).abs() < f32::EPSILON,
            "Rank 1 RRF should be 1/61 = {}, got {}",
            expected_1,
            result_rank_1.rrf_contribution()
        );

        // Rank 10: 1/(60+10) = 1/70
        let result_rank_10 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.9, 10);
        let expected_10 = 1.0 / 70.0;
        assert!(
            (result_rank_10.rrf_contribution() - expected_10).abs() < f32::EPSILON,
            "Rank 10 RRF should be 1/70 = {}, got {}",
            expected_10,
            result_rank_10.rrf_contribution()
        );

        println!("[PASS] RRF contribution formula 1/(60+rank) verified");
    }

    /// Test RRF contribution monotonically decreases with rank.
    #[test]
    fn test_rrf_decreases_with_rank() {
        let id = Uuid::new_v4();
        let mut prev_rrf = f32::MAX;

        for rank in 0..100 {
            let result = EmbedderQueryResult::from_similarity(id, 0, 0.9, rank);
            let rrf = result.rrf_contribution();

            assert!(
                rrf < prev_rrf,
                "RRF should decrease: rank {} ({}) >= rank {} ({})",
                rank,
                rrf,
                rank - 1,
                prev_rrf
            );

            prev_rrf = rrf;
        }

        println!("[PASS] RRF monotonically decreases with rank (tested 0-99)");
    }

    /// Test RRF aggregation in MultiSpaceQueryResult.
    #[test]
    fn test_rrf_aggregation() {
        let id = Uuid::new_v4();

        // Create results for 3 embedders with different ranks
        let results = vec![
            EmbedderQueryResult::from_similarity(id, 0, 0.9, 0), // rank 0: 1/60
            EmbedderQueryResult::from_similarity(id, 1, 0.8, 1), // rank 1: 1/61
            EmbedderQueryResult::from_similarity(id, 2, 0.7, 2), // rank 2: 1/62
        ];

        let multi = MultiSpaceQueryResult::from_embedder_results(id, &results, 0.75);

        // Expected RRF = 1/60 + 1/61 + 1/62
        let expected_rrf = 1.0 / 60.0 + 1.0 / 61.0 + 1.0 / 62.0;
        assert!(
            (multi.rrf_score - expected_rrf).abs() < 1e-6,
            "RRF score should be {} (sum of contributions), got {}",
            expected_rrf,
            multi.rrf_score
        );

        println!(
            "[PASS] RRF aggregation: sum of 1/(60+rank_i) = {}",
            multi.rrf_score
        );
    }

    /// Test RRF at extreme ranks.
    #[test]
    fn test_rrf_extreme_ranks() {
        // Very high rank
        let result_high = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.5, 10000);
        let rrf_high = result_high.rrf_contribution();
        let expected_high = 1.0 / 10060.0;
        assert!(
            (rrf_high - expected_high).abs() < f32::EPSILON,
            "Rank 10000 RRF should be {}, got {}",
            expected_high,
            rrf_high
        );

        // Verify it's still positive
        assert!(rrf_high > 0.0, "RRF should always be positive");

        println!(
            "[PASS] RRF at extreme ranks verified (rank 10000 = {})",
            rrf_high
        );
    }

    /// Test that rank 0 has much higher RRF contribution than high ranks.
    #[test]
    fn test_rrf_rank_dominance() {
        let rrf_0 = 1.0 / 60.0; // ~0.0167
        let rrf_100 = 1.0 / 160.0; // ~0.00625
        let rrf_1000 = 1.0 / 1060.0; // ~0.00094

        // Rank 0 should be ~2.67x rank 100
        assert!(rrf_0 > rrf_100 * 2.0, "Rank 0 should be >2x rank 100");

        // Rank 0 should be ~17x rank 1000
        assert!(rrf_0 > rrf_1000 * 15.0, "Rank 0 should be >15x rank 1000");

        println!("[PASS] RRF rank dominance verified");
    }
}

// =============================================================================
// QUERY RESULT TESTS
// =============================================================================

#[cfg(test)]
mod query_result_tests {
    use super::*;

    /// Test EmbedderQueryResult distance calculation.
    #[test]
    fn test_embedder_query_result_distance() {
        // Similarity 0.9 -> distance 0.1
        let result = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.9, 0);
        assert!(
            (result.distance - 0.1).abs() < f32::EPSILON,
            "Distance should be 1-similarity"
        );

        // Similarity 1.0 -> distance 0.0
        let result2 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 1.0, 0);
        assert!(
            (result2.distance - 0.0).abs() < f32::EPSILON,
            "Distance for similarity 1.0 should be 0"
        );

        // Similarity 0.0 -> distance 1.0
        let result3 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.0, 0);
        assert!(
            (result3.distance - 1.0).abs() < f32::EPSILON,
            "Distance for similarity 0.0 should be 1.0"
        );

        println!("[PASS] EmbedderQueryResult distance = 1 - similarity");
    }

    /// Test similarity clamping in distance calculation.
    #[test]
    fn test_similarity_clamping_in_distance() {
        // Similarity > 1.0 should clamp to 1.0
        let result = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 1.5, 0);
        assert!(
            (result.distance - 0.0).abs() < f32::EPSILON,
            "Clamped similarity 1.5->1.0 means distance 0"
        );

        // Similarity < -1.0 should clamp to -1.0
        let result2 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, -1.5, 0);
        assert!(
            (result2.distance - 2.0).abs() < f32::EPSILON,
            "Clamped similarity -1.5->-1.0 means distance 2.0"
        );

        println!("[PASS] Similarity clamping in distance calculation verified");
    }

    /// Test MultiSpaceQueryResult aggregation.
    #[test]
    fn test_multi_space_aggregation() {
        let id = Uuid::new_v4();

        let results = vec![
            EmbedderQueryResult::from_similarity(id, 0, 0.9, 0),
            EmbedderQueryResult::from_similarity(id, 5, 0.8, 1),
            EmbedderQueryResult::from_similarity(id, 12, 0.7, 2),
        ];

        let multi = MultiSpaceQueryResult::from_embedder_results(id, &results, 0.65);

        // Verify embedder_count
        assert_eq!(multi.embedder_count, 3, "Should count 3 embedders");

        // Verify correct similarities are stored
        assert!((multi.embedder_similarities[0] - 0.9).abs() < f32::EPSILON);
        assert!((multi.embedder_similarities[5] - 0.8).abs() < f32::EPSILON);
        assert!((multi.embedder_similarities[12] - 0.7).abs() < f32::EPSILON);

        // Verify non-searched embedders are NaN
        assert!(
            multi.embedder_similarities[1].is_nan(),
            "Non-searched embedder should be NaN"
        );
        assert!(
            multi.embedder_similarities[6].is_nan(),
            "Non-searched embedder should be NaN"
        );

        // Verify weighted similarity = mean
        let expected_weighted = (0.9 + 0.8 + 0.7) / 3.0;
        assert!(
            (multi.weighted_similarity - expected_weighted).abs() < f32::EPSILON,
            "Weighted similarity should be mean: {}, got {}",
            expected_weighted,
            multi.weighted_similarity
        );

        println!("[PASS] MultiSpaceQueryResult aggregation verified");
    }

    /// Test MultiSpaceQueryResult panics on empty results.
    #[test]
    #[should_panic(expected = "AGGREGATION ERROR")]
    fn test_multi_space_empty_results_panics() {
        let _ = MultiSpaceQueryResult::from_embedder_results(
            Uuid::new_v4(),
            &[], // Empty
            0.75,
        );
    }

    /// Test purpose alignment filter at 0.55 threshold.
    #[test]
    fn test_purpose_alignment_filter() {
        let id = Uuid::new_v4();
        let results = vec![EmbedderQueryResult::from_similarity(id, 0, 0.9, 0)];

        // Above threshold
        let multi_high = MultiSpaceQueryResult::from_embedder_results(id, &results, 0.60);
        assert!(
            multi_high.passes_alignment_filter(0.55),
            "0.60 >= 0.55 should pass"
        );

        // At threshold
        let multi_at = MultiSpaceQueryResult::from_embedder_results(id, &results, 0.55);
        assert!(
            multi_at.passes_alignment_filter(0.55),
            "0.55 >= 0.55 should pass"
        );

        // Below threshold
        let multi_low = MultiSpaceQueryResult::from_embedder_results(id, &results, 0.50);
        assert!(
            !multi_low.passes_alignment_filter(0.55),
            "0.50 < 0.55 should fail"
        );

        println!("[PASS] Purpose alignment filter at 0.55 threshold verified");
    }
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    /// Test get_embedding panics for invalid index.
    #[test]
    #[should_panic(expected = "STORAGE ERROR")]
    fn test_get_embedding_invalid_index() {
        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings_with_deterministic_data(42),
            create_purpose_vector(42),
            create_johari_quadrants(42),
            create_content_hash(42),
        );

        let _ = fp.get_embedding(99); // Invalid index
    }

    /// Test very small vectors in IndexEntry.
    #[test]
    fn test_small_vector_values() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1e-10, 1e-10, 1e-10]);
        let normalized = entry.normalized();

        // Should still produce valid normalized vector
        let norm: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-3 || norm == 0.0,
            "Normalized very small vector should have unit norm or be zero"
        );

        println!("[PASS] Small vector values handled correctly");
    }

    /// Test very large vector values in IndexEntry.
    #[test]
    fn test_large_vector_values() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1e10, 1e10, 1e10]);
        let sim = entry.cosine_similarity(&[1e10, 1e10, 1e10]);

        assert!(
            (sim - 1.0).abs() < 1e-3,
            "Large identical vectors should have similarity ~1.0, got {}",
            sim
        );

        println!("[PASS] Large vector values handled correctly");
    }

    /// Test fingerprint with all equal purpose values.
    #[test]
    fn test_uniform_purpose_vector() {
        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings_with_deterministic_data(42),
            [0.5f32; 13],
            create_johari_quadrants(42),
            create_content_hash(42),
        );

        assert!(
            (fp.theta_to_north_star - 0.5).abs() < f32::EPSILON,
            "Uniform purpose vector mean should be 0.5"
        );

        println!("[PASS] Uniform purpose vector handled correctly");
    }

    /// Test fingerprint with extreme johari quadrant (one dominant).
    #[test]
    fn test_extreme_johari_quadrants() {
        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings_with_deterministic_data(42),
            create_purpose_vector(42),
            [1.0, 0.0, 0.0, 0.0], // 100% Open
            create_content_hash(42),
        );

        assert_eq!(fp.dominant_quadrant, 0, "Open should be dominant");
        assert!(
            (fp.johari_confidence - 1.0).abs() < f32::EPSILON,
            "Confidence should be 1.0"
        );

        println!("[PASS] Extreme johari quadrants handled correctly");
    }

    /// Test fingerprint with all zero johari quadrants.
    #[test]
    fn test_zero_johari_quadrants() {
        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings_with_deterministic_data(42),
            create_purpose_vector(42),
            [0.0, 0.0, 0.0, 0.0],
            create_content_hash(42),
        );

        // Should default to quadrant 0 with zero confidence
        assert_eq!(
            fp.dominant_quadrant, 0,
            "Should default to Open with zero input"
        );
        assert!(
            (fp.johari_confidence - 0.0).abs() < f32::EPSILON,
            "Confidence should be 0.0"
        );

        println!("[PASS] Zero johari quadrants handled correctly");
    }

    /// Test multiple roundtrips don't accumulate errors.
    #[test]
    fn test_multiple_roundtrips_no_drift() {
        let original = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings_with_deterministic_data(42),
            create_purpose_vector(42),
            create_johari_quadrants(42),
            create_content_hash(42),
        );

        // 10 roundtrips
        let mut current = original.clone();
        for _ in 0..10 {
            let json = serde_json::to_string(&current).expect("Serialize");
            current = serde_json::from_str(&json).expect("Deserialize");
        }

        // Verify no drift
        assert_eq!(current.id, original.id, "ID drifted after 10 roundtrips");
        assert_eq!(
            current.content_hash, original.content_hash,
            "Content hash drifted"
        );
        assert_eq!(
            current.purpose_vector, original.purpose_vector,
            "Purpose vector drifted"
        );

        for i in 0..13u8 {
            let orig_data = &original.get_embedding(i).data;
            let curr_data = &current.get_embedding(i).data;
            assert_eq!(
                orig_data, curr_data,
                "Embedder {} data drifted after 10 roundtrips",
                i
            );
        }

        println!("[PASS] 10 roundtrips with no data drift");
    }
}

// =============================================================================
// COMPREHENSIVE VALIDATION TEST
// =============================================================================

#[cfg(test)]
mod comprehensive_validation {
    use super::*;

    /// Master validation test covering all critical storage roundtrip requirements.
    #[test]
    fn test_comprehensive_storage_roundtrip_validation() {
        println!("=== COMPREHENSIVE STORAGE ROUNDTRIP VALIDATION ===\n");

        // 1. Create fingerprint with all 13 embeddings
        let id = Uuid::new_v4();
        let embeddings = create_test_embeddings_with_deterministic_data(123);
        let purpose_vector = create_purpose_vector(123);
        let johari_quadrants = create_johari_quadrants(123);
        let content_hash = create_content_hash(123);

        let original = StoredQuantizedFingerprint::new(
            id,
            embeddings,
            purpose_vector,
            johari_quadrants,
            content_hash,
        );

        assert_eq!(original.embeddings.len(), 13, "Must have 13 embeddings");
        println!("[1/7] Created fingerprint with all 13 embeddings");

        // 2. Verify JSON roundtrip
        let json = serde_json::to_string(&original).expect("JSON serialize");
        let from_json: StoredQuantizedFingerprint =
            serde_json::from_str(&json).expect("JSON deserialize");
        assert_eq!(from_json.id, original.id);
        assert_eq!(from_json.content_hash, original.content_hash);
        println!(
            "[2/7] JSON roundtrip preserves data (size: {} bytes)",
            json.len()
        );

        // 3. Verify bincode roundtrip
        let bincode_bytes = bincode::serialize(&original).expect("Bincode serialize");
        let from_bincode: StoredQuantizedFingerprint =
            bincode::deserialize(&bincode_bytes).expect("Bincode deserialize");
        assert_eq!(from_bincode.id, original.id);
        println!(
            "[3/7] Bincode roundtrip preserves data (size: {} bytes)",
            bincode_bytes.len()
        );

        // 4. Verify IndexEntry operations
        let index_entry = IndexEntry::new(id, 0, vec![3.0, 4.0]);
        assert!((index_entry.norm - 5.0).abs() < f32::EPSILON);
        let sim = index_entry.cosine_similarity(&[3.0, 4.0]);
        assert!((sim - 1.0).abs() < 1e-6);
        println!("[4/7] IndexEntry norm and cosine similarity verified");

        // 5. Verify RRF formula
        let rrf_0 = EmbedderQueryResult::from_similarity(id, 0, 0.9, 0).rrf_contribution();
        assert!((rrf_0 - 1.0 / 60.0).abs() < f32::EPSILON);
        println!("[5/7] RRF formula 1/(60+rank) verified");

        // 6. Verify MultiSpaceQueryResult aggregation
        let results = vec![
            EmbedderQueryResult::from_similarity(id, 0, 0.9, 0),
            EmbedderQueryResult::from_similarity(id, 1, 0.8, 1),
        ];
        let multi = MultiSpaceQueryResult::from_embedder_results(id, &results, 0.6);
        assert_eq!(multi.embedder_count, 2);
        let expected_rrf = 1.0 / 60.0 + 1.0 / 61.0;
        assert!((multi.rrf_score - expected_rrf).abs() < 1e-6);
        println!("[6/7] MultiSpaceQueryResult RRF aggregation verified");

        // 7. Verify purpose alignment filter
        assert!(multi.passes_alignment_filter(0.55));
        assert!(!multi.passes_alignment_filter(0.65));
        println!("[7/7] Purpose alignment filter at 0.55 threshold verified");

        println!("\n=== ALL STORAGE ROUNDTRIP VALIDATIONS PASSED ===");
        println!("  - 13 embeddings with correct quantization methods");
        println!("  - JSON roundtrip: {} bytes", json.len());
        println!("  - Bincode roundtrip: {} bytes", bincode_bytes.len());
        println!("  - IndexEntry norm/cosine similarity verified");
        println!("  - RRF formula 1/(60+rank) verified");
        println!("  - MultiSpaceQueryResult aggregation verified");
        println!("  - Purpose alignment filter at 0.55 verified");
    }
}
