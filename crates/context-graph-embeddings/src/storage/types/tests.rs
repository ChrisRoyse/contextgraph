//! Tests for storage types.

#[cfg(test)]
mod tests {
    use crate::quantization::{QuantizationMetadata, QuantizationMethod, QuantizedEmbedding};
    use crate::storage::types::{
        constants::{MAX_QUANTIZED_SIZE_BYTES, STORAGE_VERSION},
        EmbedderQueryResult, IndexEntry, MultiSpaceQueryResult, StoredQuantizedFingerprint,
    };
    use std::collections::HashMap;
    use uuid::Uuid;

    /// Create a valid HashMap of dummy quantized embeddings for testing.
    fn create_test_embeddings() -> HashMap<u8, QuantizedEmbedding> {
        let mut map = HashMap::new();
        for i in 0..14u8 {
            let (method, dim, data_len) = match i {
                0 => (QuantizationMethod::PQ8, 1024, 8), // E1 Semantic (e5-large-v2)
                1..=3 => (QuantizationMethod::Float8E4M3, 512, 512), // E2-E4 Temporal (custom 512D)
                4 => (QuantizationMethod::PQ8, 768, 8),  // E5 Causal (nomic-embed 768D)
                5 | 12 => (QuantizationMethod::SparseNative, 30522, 100), // E6, E13 Sparse
                6 => (QuantizationMethod::PQ8, 1536, 8), // E7 Code (Qodo-Embed 1536D)
                7 => (QuantizationMethod::Float8E4M3, 1024, 1024), // E8 Graph (e5-large-v2 1024D)
                8 => (QuantizationMethod::Binary, 10000, 1250), // E9 HDC (10K-bit)
                9 => (QuantizationMethod::PQ8, 768, 8),  // E10 Multimodal (e5-base-v2 768D)
                10 => (QuantizationMethod::PQ8, 768, 8), // E11 Kepler (production 768D)
                11 => (QuantizationMethod::TokenPruning, 128, 64), // E12 ColBERT (128D/token)
                13 => (QuantizationMethod::PQ8, 1024, 8), // E14 BGE-M3 Dense (1024D, PQ8)
                _ => unreachable!(),
            };

            map.insert(
                i,
                QuantizedEmbedding {
                    method,
                    original_dim: dim,
                    data: vec![0u8; data_len],
                    metadata: match method {
                        QuantizationMethod::PQ8 => QuantizationMetadata::PQ8 {
                            codebook_id: i as u32,
                            num_subvectors: 8,
                        },
                        QuantizationMethod::Float8E4M3 => QuantizationMetadata::Float8 {
                            scale: 1.0,
                            bias: 0.0,
                        },
                        QuantizationMethod::Binary => {
                            QuantizationMetadata::Binary { threshold: 0.0 }
                        }
                        QuantizationMethod::SparseNative => QuantizationMetadata::Sparse {
                            vocab_size: 30522,
                            nnz: 50,
                        },
                        QuantizationMethod::TokenPruning => QuantizationMetadata::TokenPruning {
                            original_tokens: 128,
                            kept_tokens: 64,
                            threshold: 0.5,
                        },
                    },
                },
            );
        }
        map
    }

    #[test]
    fn test_stored_fingerprint_creation() {
        let id = Uuid::new_v4();
        let embeddings = create_test_embeddings();
        let topic_profile = [0.5f32; 14];
        // M-L2 (GH #493, 2026-05-19): use 0x42-fill (matching
        // `no_fake_data_test.rs:418`) instead of all-zero. This test verifies
        // construction-time SHAPE validation only — checksum content is
        // irrelevant to what is under test, but the constant is now
        // consistent with the AP-007 fake-detection convention used elsewhere
        // in the embeddings crate.
        let content_hash = [0x42u8; 32];

        let fp = StoredQuantizedFingerprint::new(id, embeddings, topic_profile, content_hash);

        assert_eq!(fp.id, id);
        assert_eq!(fp.version, STORAGE_VERSION);
        assert_eq!(fp.embeddings.len(), 14);
        assert_eq!(fp.topic_profile.len(), 14);
    }

    #[test]
    #[should_panic(expected = "CONSTRUCTION ERROR")]
    fn test_stored_fingerprint_missing_embeddings() {
        let mut embeddings = create_test_embeddings();
        embeddings.remove(&5); // Remove one embedder

        let _ =
            StoredQuantizedFingerprint::new(Uuid::new_v4(), embeddings, [0.5f32; 14], [0x42u8; 32]);
    }

    #[test]
    fn test_index_entry_normalized() {
        let entry = IndexEntry::new(
            Uuid::new_v4(),
            0,
            vec![3.0, 4.0], // 3-4-5 right triangle
        );

        assert!((entry.norm - 5.0).abs() < f32::EPSILON);

        let normalized = entry.normalized();
        assert!((normalized[0] - 0.6).abs() < f32::EPSILON);
        assert!((normalized[1] - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_index_entry_cosine_similarity() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1.0, 0.0]);

        // Same direction
        let sim = entry.cosine_similarity(&[1.0, 0.0]);
        assert!((sim - 1.0).abs() < 1e-6);

        // Opposite direction
        let sim = entry.cosine_similarity(&[-1.0, 0.0]);
        assert!((sim - (-1.0)).abs() < 1e-6);

        // Perpendicular
        let sim = entry.cosine_similarity(&[0.0, 1.0]);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_embedder_query_result_rrf() {
        let result = EmbedderQueryResult::from_similarity(
            Uuid::new_v4(),
            0,
            0.9,
            0, // rank 0
        );

        // RRF contribution at rank 0: 1/(60+0+1) = 1/61 (1-indexed)
        let expected = 1.0 / 61.0;
        assert!((result.rrf_contribution() - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn test_multi_space_result_aggregation() {
        let id = Uuid::new_v4();
        let results = vec![
            EmbedderQueryResult::from_similarity(id, 0, 0.9, 0),
            EmbedderQueryResult::from_similarity(id, 1, 0.8, 1),
            EmbedderQueryResult::from_similarity(id, 2, 0.7, 2),
        ];

        let multi = MultiSpaceQueryResult::from_embedder_results(id, &results);

        assert_eq!(multi.id, id);
        assert_eq!(multi.embedder_count, 3);
        assert!((multi.embedder_similarities[0] - 0.9).abs() < f32::EPSILON);
        assert!(multi.embedder_similarities[3].is_nan()); // Not searched
    }

    #[test]
    fn test_serde_roundtrip() {
        let result = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 5, 0.85, 10);
        let json = serde_json::to_string(&result).expect("serialize");
        let restored: EmbedderQueryResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(result.id, restored.id);
        assert_eq!(result.embedder_idx, restored.embedder_idx);
    }

    #[test]
    fn test_estimated_size() {
        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings(),
            [0.5f32; 14],
            [0x42u8; 32], // M-L2: 0x42 fill matches no_fake_data_test.rs:418 convention
        );

        let size = fp.estimated_size_bytes();
        // Should be in reasonable range
        assert!(size > 1000, "Size too small: {}", size);
        assert!(size < MAX_QUANTIZED_SIZE_BYTES, "Size too large: {}", size);
    }

    // =============================================================================
    // EDGE CASE TESTS
    // =============================================================================

    /// Edge Case 1: Creating fingerprint with only 12 embedders MUST PANIC
    #[test]
    #[should_panic(expected = "CONSTRUCTION ERROR")]
    fn test_edge_case_missing_embedder_panics() {
        let mut embeddings = create_test_embeddings();
        embeddings.remove(&5); // Remove E6

        // This MUST panic with "CONSTRUCTION ERROR"
        let _ =
            StoredQuantizedFingerprint::new(Uuid::new_v4(), embeddings, [0.5f32; 14], [0x42u8; 32]);
    }

    /// Edge Case 2: Zero-norm vector in IndexEntry
    #[test]
    fn test_edge_case_zero_norm_vector() {
        // Test: Creating index entry with zero vector
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![0.0, 0.0, 0.0]);

        // Normalized should return zero vector (not NaN/Inf)
        let normalized = entry.normalized();
        assert!(
            normalized.iter().all(|&x| x == 0.0),
            "Expected zero vector for normalized"
        );
        assert_eq!(normalized.len(), 3);

        // Cosine similarity with zero vector should be 0.0
        let sim = entry.cosine_similarity(&[1.0, 0.0, 0.0]);
        assert_eq!(sim, 0.0, "Expected 0.0 similarity for zero vector");

        // Verify norm is zero
        assert!(entry.norm.abs() < f32::EPSILON, "Expected zero norm");
    }

    /// Edge Case 3: RRF contribution at high rank
    #[test]
    fn test_edge_case_rrf_high_rank() {
        // Test: RRF contribution diminishes at high ranks
        let result_rank_0 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.9, 0);
        let result_rank_1000 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.9, 1000);

        let rrf_0 = result_rank_0.rrf_contribution(); // 1/61 = 0.0164
        let rrf_1000 = result_rank_1000.rrf_contribution(); // 1/1061 = 0.00094

        // Verify actual values (1-indexed)
        let expected_rrf_0 = 1.0 / 61.0;
        let expected_rrf_1000 = 1.0 / 1061.0;

        assert!(
            (rrf_0 - expected_rrf_0).abs() < f32::EPSILON,
            "Expected rrf_0={}, got {}",
            expected_rrf_0,
            rrf_0
        );
        assert!(
            (rrf_1000 - expected_rrf_1000).abs() < f32::EPSILON,
            "Expected rrf_1000={}, got {}",
            expected_rrf_1000,
            rrf_1000
        );

        // Rank 0 contributes 10x+ more than rank 1000
        assert!(
            rrf_0 > rrf_1000 * 10.0,
            "Rank 0 ({}) should be >10x rank 1000 ({})",
            rrf_0,
            rrf_1000
        );
    }

    /// Test that invalid embedder index in embeddings map panics
    #[test]
    #[should_panic(expected = "CONSTRUCTION ERROR")]
    fn test_invalid_embedder_index() {
        let mut embeddings = create_test_embeddings();
        // Remove valid key 12 and add invalid key 13
        embeddings.remove(&12);
        embeddings.insert(
            13,
            QuantizedEmbedding {
                method: QuantizationMethod::SparseNative,
                original_dim: 30522,
                data: vec![0u8; 100],
                metadata: QuantizationMetadata::Sparse {
                    vocab_size: 30522,
                    nnz: 50,
                },
            },
        );

        let _ =
            StoredQuantizedFingerprint::new(Uuid::new_v4(), embeddings, [0.5f32; 14], [0x42u8; 32]);
    }

    /// Test get_embedding panics for missing index
    #[test]
    #[should_panic(expected = "STORAGE ERROR")]
    fn test_get_embedding_missing_index() {
        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings(),
            [0.5f32; 14],
            [0x42u8; 32], // M-L2
        );

        // This should panic because 15 is not a valid index
        let _ = fp.get_embedding(15);
    }

    /// Test cosine similarity panics on dimension mismatch
    #[test]
    #[should_panic(expected = "SIMILARITY ERROR")]
    fn test_cosine_similarity_dimension_mismatch() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1.0, 0.0, 0.0]);

        // This should panic because query has 2 dims, entry has 3
        let _ = entry.cosine_similarity(&[1.0, 0.0]);
    }

    /// Test MultiSpaceQueryResult panics on empty results
    #[test]
    #[should_panic(expected = "AGGREGATION ERROR")]
    fn test_multi_space_empty_results() {
        let _ = MultiSpaceQueryResult::from_embedder_results(
            Uuid::new_v4(),
            &[], // Empty results
        );
    }

    /// Test validate_quantization_methods
    #[test]
    fn test_validate_quantization_methods() {
        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings(),
            [0.5f32; 14],
            [0x42u8; 32], // M-L2
        );

        // Our test embeddings use the correct methods per Constitution
        assert!(
            fp.validate_quantization_methods(),
            "Test embeddings should use correct quantization methods"
        );
    }
}
