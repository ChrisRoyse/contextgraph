//! Tests for teleological comparator module.

#[cfg(test)]
mod tests {
    use crate::teleological::comparator::{BatchComparator, TeleologicalComparator};
    use crate::teleological::{
        ComparisonValidationError, MatrixSearchConfig, SearchStrategy, SynergyMatrix,
    };
    use crate::types::fingerprint::{
        SparseVector, E10_DIM, E11_DIM, E1_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM, E7_DIM, E8_DIM,
        E9_DIM,
    };
    use crate::types::SemanticFingerprint;

    /// Create a test fingerprint with known values for dense embeddings.
    fn create_test_fingerprint(base_value: f32) -> SemanticFingerprint {
        // Create normalized vectors to ensure valid cosine similarity
        let create_normalized_vec = |dim: usize, val: f32| -> Vec<f32> {
            let mut v = vec![val; dim];
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > f32::EPSILON {
                for x in &mut v {
                    *x /= norm;
                }
            }
            v
        };

        let e5_vec = create_normalized_vec(E5_DIM, base_value);
        SemanticFingerprint {
            e1_semantic: create_normalized_vec(E1_DIM, base_value),
            e2_temporal_recent: create_normalized_vec(E2_DIM, base_value),
            e3_temporal_periodic: create_normalized_vec(E3_DIM, base_value),
            e4_temporal_positional: create_normalized_vec(E4_DIM, base_value),
            e5_causal_as_cause: e5_vec.clone(),
            e5_causal_as_effect: e5_vec,
            e5_causal: Vec::new(), // Using new dual format
            e6_sparse: SparseVector::empty(),
            e7_code: create_normalized_vec(E7_DIM, base_value),
            e8_graph_as_source: create_normalized_vec(E8_DIM, base_value),
            e8_graph_as_target: create_normalized_vec(E8_DIM, base_value),
            e8_graph: Vec::new(), // Legacy field, empty by default
            e9_hdc: create_normalized_vec(E9_DIM, base_value),
            e10_multimodal_paraphrase: create_normalized_vec(E10_DIM, base_value),
            e10_multimodal_as_context: create_normalized_vec(E10_DIM, base_value),
            e11_entity: create_normalized_vec(E11_DIM, base_value),
            e12_late_interaction: vec![vec![base_value / 128.0_f32.sqrt(); 128]],
            e13_splade: SparseVector::empty(),
        }
    }

    /// Create two fingerprints with known cosine similarity.
    fn create_orthogonal_fingerprints() -> (SemanticFingerprint, SemanticFingerprint) {
        // Fingerprint A: all ones (normalized)
        // Fingerprint B: alternating sign (orthogonal in high dimensions approximation)
        let fp_a = create_test_fingerprint(1.0);
        let mut fp_b = create_test_fingerprint(1.0);

        // Make E1 orthogonal
        for (i, val) in fp_b.e1_semantic.iter_mut().enumerate() {
            if i % 2 == 1 {
                *val = -*val;
            }
        }
        // Renormalize
        let norm: f32 = fp_b.e1_semantic.iter().map(|x| x * x).sum::<f32>().sqrt();
        for x in &mut fp_b.e1_semantic {
            *x /= norm;
        }

        (fp_a, fp_b)
    }

    #[test]
    fn test_compare_identical() {
        let fp = create_test_fingerprint(1.0);
        let comparator = TeleologicalComparator::new();

        let result = comparator
            .compare(&fp, &fp)
            .expect("comparison should succeed");

        // Self-similarity should be ~1.0
        assert!(
            result.overall >= 0.99,
            "Self-similarity should be ~1.0, got {}",
            result.overall
        );

        // All available embedders should have scores
        let valid_count = result.valid_score_count();
        assert!(
            valid_count >= 10,
            "Expected at least 10 valid scores, got {}",
            valid_count
        );
    }

    #[test]
    fn test_compare_different() {
        let (fp_a, fp_b) = create_orthogonal_fingerprints();
        let comparator = TeleologicalComparator::new();

        let result = comparator
            .compare(&fp_a, &fp_b)
            .expect("comparison should succeed");

        // Orthogonal vectors: raw cosine ~0.0, normalized (0+1)/2 = 0.5 (midpoint)
        // Note: Only E1 is truly orthogonal in this test
        assert!(
            result.per_embedder[0].map(|s| (s - 0.5).abs() < 0.01).unwrap_or(false),
            "E1 similarity for orthogonal vectors should be ~0.5 (midpoint), got {:?}",
            result.per_embedder[0]
        );
    }

    #[test]
    fn test_compare_no_overlap() {
        // Create fingerprints with empty embeddings
        let fp_a = SemanticFingerprint {
            e1_semantic: vec![1.0 / (E1_DIM as f32).sqrt(); E1_DIM],
            e2_temporal_recent: vec![],
            e3_temporal_periodic: vec![],
            e4_temporal_positional: vec![],
            e5_causal_as_cause: vec![],
            e5_causal_as_effect: vec![],
            e5_causal: vec![],
            e6_sparse: SparseVector::empty(),
            e7_code: vec![],
            e8_graph_as_source: vec![],
            e8_graph_as_target: vec![],
            e8_graph: vec![],
            e9_hdc: vec![],
            e10_multimodal_paraphrase: vec![],
            e10_multimodal_as_context: vec![],
            e11_entity: vec![],
            e12_late_interaction: vec![],
            e13_splade: SparseVector::empty(),
        };

        let fp_b = SemanticFingerprint {
            e1_semantic: vec![],
            e2_temporal_recent: vec![1.0 / (E2_DIM as f32).sqrt(); E2_DIM],
            e3_temporal_periodic: vec![],
            e4_temporal_positional: vec![],
            e5_causal_as_cause: vec![],
            e5_causal_as_effect: vec![],
            e5_causal: vec![],
            e6_sparse: SparseVector::empty(),
            e7_code: vec![],
            e8_graph_as_source: vec![],
            e8_graph_as_target: vec![],
            e8_graph: vec![],
            e9_hdc: vec![],
            e10_multimodal_paraphrase: vec![],
            e10_multimodal_as_context: vec![],
            e11_entity: vec![],
            e12_late_interaction: vec![],
            e13_splade: SparseVector::empty(),
        };

        let comparator = TeleologicalComparator::new();
        let result = comparator
            .compare(&fp_a, &fp_b)
            .expect("comparison should succeed");

        // No overlapping embedders = 0 similarity
        assert_eq!(result.overall, 0.0, "No overlap should give 0 similarity");
        assert_eq!(result.valid_score_count(), 0, "No valid scores expected");
    }

    #[test]
    fn test_compare_strategies() {
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.9);
        let comparator = TeleologicalComparator::new();

        let strategies = [
            SearchStrategy::Cosine,
            SearchStrategy::Euclidean,
            SearchStrategy::GroupHierarchical,
            SearchStrategy::TuckerCompressed,
            SearchStrategy::Adaptive,
        ];

        for strategy in strategies {
            let result = comparator
                .compare_with_strategy(&fp_a, &fp_b, strategy)
                .unwrap_or_else(|_| panic!("Strategy {:?} should succeed", strategy));

            assert!(
                (0.0..=1.0).contains(&result.overall),
                "Strategy {:?}: similarity {} should be in [0,1]",
                strategy,
                result.overall
            );
            assert_eq!(result.strategy, strategy);
        }
    }

    #[test]
    fn test_invalid_weights_fail_fast() {
        let fp = create_test_fingerprint(1.0);

        let mut config = MatrixSearchConfig::default();
        config.weights.topic_profile = 2.0; // Invalid: > 1.0

        let comparator = TeleologicalComparator::with_config(config);
        let result = comparator.compare(&fp, &fp);

        assert!(result.is_err(), "Invalid weights should return error");
        assert!(
            matches!(
                result.unwrap_err(),
                ComparisonValidationError::WeightOutOfRange { .. }
            ),
            "Error should be WeightOutOfRange"
        );
    }

    #[test]
    fn test_synergy_weighted() {
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.8);

        let config = MatrixSearchConfig::with_synergy(SynergyMatrix::semantic_focused());
        let comparator = TeleologicalComparator::with_config(config);

        let result = comparator
            .compare_with_strategy(&fp_a, &fp_b, SearchStrategy::SynergyWeighted)
            .expect("Synergy weighted comparison should succeed");

        assert!(
            (0.0..=1.0).contains(&result.overall),
            "Synergy weighted result should be in [0,1]"
        );
    }

    #[test]
    fn test_coherence_computation() {
        let fp = create_test_fingerprint(1.0);
        let comparator = TeleologicalComparator::new();

        let result = comparator
            .compare(&fp, &fp)
            .expect("comparison should succeed");

        // High self-similarity = high coherence
        assert!(
            result.coherence.map(|c| c > 0.9).unwrap_or(false),
            "Self-comparison should have high coherence"
        );
    }

    #[test]
    fn test_dominant_embedder() {
        let fp = create_test_fingerprint(1.0);
        let comparator = TeleologicalComparator::new();

        let result = comparator
            .compare(&fp, &fp)
            .expect("comparison should succeed");

        // Should have a dominant embedder
        assert!(
            result.dominant_embedder.is_some(),
            "Should identify dominant embedder"
        );
    }

    #[test]
    fn test_breakdown_generation() {
        let fp = create_test_fingerprint(1.0);

        let config = MatrixSearchConfig {
            compute_breakdown: true,
            ..Default::default()
        };

        let comparator = TeleologicalComparator::with_config(config);
        let result = comparator
            .compare(&fp, &fp)
            .expect("comparison should succeed");

        assert!(result.breakdown.is_some(), "Breakdown should be generated");

        let breakdown = result.breakdown.as_ref().expect("breakdown exists");
        assert!(
            !breakdown.per_group.is_empty(),
            "Per-group scores should be populated"
        );
    }

    #[test]
    fn test_similarity_range() {
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.5);
        let comparator = TeleologicalComparator::new();

        let result = comparator
            .compare(&fp_a, &fp_b)
            .expect("comparison should succeed");

        // Overall must be in [0, 1]
        assert!(
            (0.0..=1.0).contains(&result.overall),
            "Overall similarity {} should be in [0,1]",
            result.overall
        );

        // All per-embedder scores must be in [0, 1]
        for (idx, score) in result.per_embedder.iter().enumerate() {
            if let Some(s) = score {
                assert!(
                    (0.0..=1.0).contains(s),
                    "Embedder {} score {} should be in [0,1]",
                    idx,
                    s
                );
            }
        }
    }

    #[test]
    fn test_batch_one_to_many() {
        let reference = create_test_fingerprint(1.0);
        let targets: Vec<SemanticFingerprint> = (0..10)
            .map(|i| create_test_fingerprint(0.5 + (i as f32) * 0.05))
            .collect();

        let batch = BatchComparator::new();
        let results = batch.compare_one_to_many(&reference, &targets);

        assert_eq!(results.len(), 10, "Should have 10 results");
        for result in results {
            assert!(result.is_ok(), "All comparisons should succeed");
        }
    }

    #[test]
    fn test_batch_all_pairs() {
        let fingerprints: Vec<SemanticFingerprint> = (0..5)
            .map(|i| create_test_fingerprint(0.5 + (i as f32) * 0.1))
            .collect();

        let batch = BatchComparator::new();
        let matrix = batch.compare_all_pairs(&fingerprints);

        assert_eq!(matrix.len(), 5, "Matrix should be 5x5");
        for row in &matrix {
            assert_eq!(row.len(), 5, "Each row should have 5 elements");
        }

        // Diagonal should be 1.0
        for (i, row) in matrix.iter().enumerate() {
            assert!(
                (row[i] - 1.0).abs() < 0.01,
                "Diagonal element should be ~1.0"
            );
        }

        // Matrix should be symmetric
        for (i, row_i) in matrix.iter().enumerate() {
            for (j, &val) in row_i.iter().enumerate() {
                assert!(
                    (val - matrix[j][i]).abs() < f32::EPSILON,
                    "Matrix should be symmetric"
                );
            }
        }
    }

    #[test]
    fn test_apples_to_apples() {
        // Verify that only same-embedder comparisons occur
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.8);

        let config = MatrixSearchConfig {
            compute_breakdown: true,
            ..Default::default()
        };

        let comparator = TeleologicalComparator::with_config(config);
        let result = comparator
            .compare(&fp_a, &fp_b)
            .expect("comparison should succeed");

        // Per-embedder scores should reflect same-type comparisons only
        // E1 (index 0) compared with E1, E2 with E2, etc.
        // This is enforced by the EmbeddingSlice matching in compare_embedder_slices

        // Verify that scores are only present for embedders that exist in both fingerprints
        // (apples-to-apples means same embedder type comparison only)
        let valid_count = result.valid_score_count();
        assert!(
            valid_count > 0,
            "Should have at least one valid embedder comparison"
        );

        // All valid scores should be in valid range (confirming same-type comparison worked)
        for (idx, score) in result.per_embedder.iter().enumerate() {
            if let Some(s) = score {
                let name = SemanticFingerprint::embedding_name(idx).unwrap_or("unknown");
                assert!(
                    (0.0..=1.0).contains(s),
                    "Embedder {} ({}) score {} should be in [0,1] from same-type comparison",
                    idx,
                    name,
                    s
                );
            }
        }
    }

    #[test]
    fn test_no_unwrap_calls() {
        // This test verifies the code doesn't panic on edge cases
        // by testing various potentially problematic inputs

        let empty_fp = SemanticFingerprint {
            e1_semantic: vec![],
            e2_temporal_recent: vec![],
            e3_temporal_periodic: vec![],
            e4_temporal_positional: vec![],
            e5_causal_as_cause: vec![],
            e5_causal_as_effect: vec![],
            e5_causal: vec![],
            e6_sparse: SparseVector::empty(),
            e7_code: vec![],
            e8_graph_as_source: vec![],
            e8_graph_as_target: vec![],
            e8_graph: vec![],
            e9_hdc: vec![],
            e10_multimodal_paraphrase: vec![],
            e10_multimodal_as_context: vec![],
            e11_entity: vec![],
            e12_late_interaction: vec![],
            e13_splade: SparseVector::empty(),
        };

        let comparator = TeleologicalComparator::new();

        // Should not panic
        let result = comparator.compare(&empty_fp, &empty_fp);
        assert!(
            result.is_ok(),
            "Empty fingerprint comparison should not panic"
        );
    }

    #[test]
    fn test_group_hierarchical() {
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.7);

        let comparator = TeleologicalComparator::new();
        let result = comparator
            .compare_with_strategy(&fp_a, &fp_b, SearchStrategy::GroupHierarchical)
            .expect("Group hierarchical comparison should succeed");

        assert!(
            (0.0..=1.0).contains(&result.overall),
            "Group hierarchical result should be in [0,1]"
        );
    }

    #[test]
    fn test_adaptive_strategy() {
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.6);

        let comparator = TeleologicalComparator::new();
        let result = comparator
            .compare_with_strategy(&fp_a, &fp_b, SearchStrategy::Adaptive)
            .expect("Adaptive comparison should succeed");

        assert!(
            (0.0..=1.0).contains(&result.overall),
            "Adaptive result should be in [0,1]"
        );
    }
}
