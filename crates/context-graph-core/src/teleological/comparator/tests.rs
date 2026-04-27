//! Tests for teleological comparator module.

#[cfg(test)]
mod tests {
    use crate::teleological::comparator::{BatchComparator, TeleologicalComparator};
    use crate::teleological::{ComparisonValidationError, MatrixSearchConfig, SearchStrategy};
    use crate::types::fingerprint::{
        SparseVector, E10_DIM, E11_DIM, E14_DIM, E1_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM, E7_DIM,
        E8_DIM, E9_DIM,
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
            e14_bge_m3_dense: create_normalized_vec(E14_DIM, base_value),
        }
    }

    /// Create two fingerprints with known cosine similarity.
    fn create_orthogonal_fingerprints() -> (SemanticFingerprint, SemanticFingerprint) {
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

        assert!(
            result.overall >= 0.99,
            "Self-similarity should be ~1.0, got {}",
            result.overall
        );

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
        assert!(
            result.per_embedder[0]
                .map(|s| (s - 0.5).abs() < 0.01)
                .unwrap_or(false),
            "E1 similarity for orthogonal vectors should be ~0.5 (midpoint), got {:?}",
            result.per_embedder[0]
        );
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
}
