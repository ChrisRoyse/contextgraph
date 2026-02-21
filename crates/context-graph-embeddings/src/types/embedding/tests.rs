//! Tests for ModelEmbedding.

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::error::EmbeddingError;
    use crate::types::embedding::ModelEmbedding;
    use crate::types::ModelId;

    // ========== Construction Tests ==========

    #[test]
    fn test_new_creates_valid_embedding() {
        let vector = vec![0.1, 0.2, 0.3];
        let embedding = ModelEmbedding::new(ModelId::Semantic, vector.clone(), 1500);

        assert_eq!(embedding.model_id, ModelId::Semantic);
        assert_eq!(embedding.vector, vector);
        assert_eq!(embedding.latency_us, 1500);
        assert!(embedding.attention_weights.is_none());
        assert!(!embedding.is_projected);
    }

    #[test]
    fn test_with_attention_creates_embedding_with_weights() {
        let vector = vec![0.1, 0.2, 0.3];
        let attention = vec![0.5, 0.3, 0.2];
        let embedding = ModelEmbedding::with_attention(
            ModelId::TemporalRecent,
            vector.clone(),
            2000,
            attention.clone(),
        );

        assert_eq!(embedding.attention_weights, Some(attention));
    }

    #[test]
    fn test_default_creates_empty_embedding() {
        let embedding = ModelEmbedding::default();

        assert_eq!(embedding.model_id, ModelId::Semantic);
        assert!(embedding.vector.is_empty());
        assert_eq!(embedding.latency_us, 0);
    }

    // ========== Dimension Tests ==========

    #[test]
    fn test_dimension_returns_vector_length() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![1.0; 1024], 100);
        assert_eq!(embedding.dimension(), 1024);
    }

    #[test]
    fn test_is_empty_for_empty_vector() {
        let embedding = ModelEmbedding::default();
        assert!(embedding.is_empty());
    }

    #[test]
    fn test_is_empty_for_non_empty_vector() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![1.0], 100);
        assert!(!embedding.is_empty());
    }

    // ========== Validation Tests ==========

    #[test]
    fn test_validate_correct_dimension_succeeds() {
        // Semantic has dimension 1024
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![0.1; 1024], 100);
        assert!(embedding.validate().is_ok());
    }

    #[test]
    fn test_validate_wrong_dimension_fails() {
        let embedding = ModelEmbedding::new(
            ModelId::Semantic,
            vec![0.1; 512], // Wrong: should be 1024
            100,
        );

        let err = embedding.validate().unwrap_err();
        match err {
            EmbeddingError::InvalidDimension { expected, actual } => {
                assert_eq!(expected, 1024);
                assert_eq!(actual, 512);
            }
            _ => panic!("Expected InvalidDimension error"),
        }
    }

    #[test]
    fn test_validate_empty_vector_fails() {
        let embedding = ModelEmbedding::default();

        let err = embedding.validate().unwrap_err();
        assert!(
            matches!(err, EmbeddingError::EmptyInput),
            "Expected EmptyInput error for empty vector"
        );
    }

    #[test]
    fn test_validate_nan_value_fails() {
        let mut vector = vec![0.1; 1024];
        vector[500] = f32::NAN;

        let embedding = ModelEmbedding::new(ModelId::Semantic, vector, 100);

        let err = embedding.validate().unwrap_err();
        match err {
            EmbeddingError::InvalidValue { index, value } => {
                assert_eq!(index, 500);
                assert!(value.is_nan());
            }
            _ => panic!("Expected InvalidValue error for NaN"),
        }
    }

    #[test]
    fn test_validate_positive_infinity_fails() {
        let mut vector = vec![0.1; 1024];
        vector[100] = f32::INFINITY;

        let embedding = ModelEmbedding::new(ModelId::Semantic, vector, 100);

        let err = embedding.validate().unwrap_err();
        match err {
            EmbeddingError::InvalidValue { index, value } => {
                assert_eq!(index, 100);
                assert!(value.is_infinite() && value.is_sign_positive());
            }
            _ => panic!("Expected InvalidValue error for Inf"),
        }
    }

    #[test]
    fn test_validate_negative_infinity_fails() {
        let mut vector = vec![0.1; 1024];
        vector[200] = f32::NEG_INFINITY;

        let embedding = ModelEmbedding::new(ModelId::Semantic, vector, 100);

        let err = embedding.validate().unwrap_err();
        match err {
            EmbeddingError::InvalidValue { index, value } => {
                assert_eq!(index, 200);
                assert!(value.is_infinite() && value.is_sign_negative());
            }
            _ => panic!("Expected InvalidValue error for -Inf"),
        }
    }

    #[test]
    fn test_validate_projected_uses_projected_dimension() {
        // Sparse has dimension() = 30522 but projected_dimension() = 1536
        let mut embedding = ModelEmbedding::new(
            ModelId::Sparse,
            vec![0.1; 1536], // projected_dimension() = 1536
            100,
        );
        embedding.set_projected(true);

        assert!(embedding.validate().is_ok());
    }

    // ========== L2 Norm Tests ==========

    #[test]
    fn test_l2_norm_unit_vector() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 0.0, 0.0], 100);
        assert!((embedding.l2_norm() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_norm_known_value() {
        // sqrt(3^2 + 4^2) = sqrt(9 + 16) = sqrt(25) = 5
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![3.0, 4.0], 100);
        assert!((embedding.l2_norm() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_norm_zero_vector() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![0.0, 0.0, 0.0], 100);
        assert_eq!(embedding.l2_norm(), 0.0);
    }

    #[test]
    fn test_l2_norm_empty_vector() {
        let embedding = ModelEmbedding::default();
        assert_eq!(embedding.l2_norm(), 0.0);
    }

    #[test]
    fn test_l2_norm_single_element() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![5.0], 100);
        assert!((embedding.l2_norm() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_norm_negative_values() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![-3.0, -4.0], 100);
        assert!((embedding.l2_norm() - 5.0).abs() < 1e-6);
    }

    // ========== Normalization Tests ==========

    #[test]
    fn test_normalize_produces_unit_vector() {
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, vec![3.0, 4.0, 0.0], 100);
        embedding.normalize();

        let norm = embedding.l2_norm();
        assert!(
            (norm - 1.0).abs() < 1e-6,
            "L2 norm should be 1.0, got {}",
            norm
        );
    }

    #[test]
    fn test_normalize_preserves_direction() {
        let original = vec![3.0, 4.0];
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, original.clone(), 100);
        embedding.normalize();

        // Check ratio is preserved
        let ratio = embedding.vector[0] / embedding.vector[1];
        let expected_ratio = 3.0 / 4.0;
        assert!((ratio - expected_ratio).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_zero_vector_unchanged() {
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, vec![0.0, 0.0, 0.0], 100);
        embedding.normalize();

        assert_eq!(embedding.vector, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_normalize_empty_vector_unchanged() {
        let mut embedding = ModelEmbedding::default();
        embedding.normalize();

        assert!(embedding.vector.is_empty());
    }

    #[test]
    fn test_normalized_returns_new_embedding() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![3.0, 4.0], 100);
        let normalized = embedding.normalized();

        // Original unchanged
        assert_eq!(embedding.vector, vec![3.0, 4.0]);
        // New is normalized
        assert!((normalized.l2_norm() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_is_normalized_true_for_unit_vector() {
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0, 3.0], 100);
        embedding.normalize();

        assert!(embedding.is_normalized(1e-6));
    }

    #[test]
    fn test_is_normalized_false_for_non_unit_vector() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0, 3.0], 100);

        assert!(!embedding.is_normalized(1e-6));
    }

    // ========== Attention Weight Tests ==========

    #[test]
    fn test_validate_attention_correct_length_succeeds() {
        let embedding = ModelEmbedding::with_attention(
            ModelId::Semantic,
            vec![0.1; 1024],
            100,
            vec![0.5, 0.3, 0.2],
        );

        assert!(embedding.validate_attention(3).is_ok());
    }

    #[test]
    fn test_validate_attention_wrong_length_fails() {
        let embedding = ModelEmbedding::with_attention(
            ModelId::Semantic,
            vec![0.1; 1024],
            100,
            vec![0.5, 0.3, 0.2],
        );

        let err = embedding.validate_attention(5).unwrap_err();
        match err {
            EmbeddingError::InvalidDimension { expected, actual } => {
                assert_eq!(expected, 5);
                assert_eq!(actual, 3);
            }
            _ => panic!("Expected InvalidDimension error"),
        }
    }

    #[test]
    fn test_validate_attention_nan_fails() {
        let embedding = ModelEmbedding::with_attention(
            ModelId::Semantic,
            vec![0.1; 1024],
            100,
            vec![0.5, f32::NAN, 0.2],
        );

        let err = embedding.validate_attention(3).unwrap_err();
        match err {
            EmbeddingError::InvalidValue { index, value } => {
                assert_eq!(index, 1);
                assert!(value.is_nan());
            }
            _ => panic!("Expected InvalidValue error"),
        }
    }

    #[test]
    fn test_validate_attention_none_succeeds() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![0.1; 1024], 100);

        // Should succeed when no attention weights present
        assert!(embedding.validate_attention(10).is_ok());
    }

    // ========== Cosine Similarity Tests ==========

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let embedding1 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0, 3.0], 100);
        let embedding2 = embedding1.clone();

        let sim = embedding1.cosine_similarity(&embedding2).unwrap();
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let embedding1 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 0.0, 0.0], 100);
        let embedding2 = ModelEmbedding::new(ModelId::Semantic, vec![-1.0, 0.0, 0.0], 100);

        let sim = embedding1.cosine_similarity(&embedding2).unwrap();
        assert!((sim - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let embedding1 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 0.0], 100);
        let embedding2 = ModelEmbedding::new(ModelId::Semantic, vec![0.0, 1.0], 100);

        let sim = embedding1.cosine_similarity(&embedding2).unwrap();
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_dimension_mismatch_fails() {
        let embedding1 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0, 3.0], 100);
        let embedding2 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0], 100);

        let err = embedding1.cosine_similarity(&embedding2).unwrap_err();
        match err {
            EmbeddingError::InvalidDimension { .. } => {}
            _ => panic!("Expected InvalidDimension error"),
        }
    }

    #[test]
    fn test_cosine_similarity_empty_vector_fails() {
        let embedding1 = ModelEmbedding::default();
        let embedding2 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0], 100);

        let err = embedding1.cosine_similarity(&embedding2).unwrap_err();
        assert!(
            matches!(err, EmbeddingError::EmptyInput),
            "Expected EmptyInput error"
        );
    }

    // ========== All Model Dimension Tests ==========

    #[test]
    fn test_validate_all_model_dimensions() {
        // All 14 models with their native dimensions from ModelId::dimension()
        let models_and_dims = [
            (ModelId::Semantic, 1024),          // E1: e5-large-v2
            (ModelId::TemporalRecent, 512),     // E2: Custom exponential decay
            (ModelId::TemporalPeriodic, 512),   // E3: Custom Fourier basis
            (ModelId::TemporalPositional, 512), // E4: Custom sinusoidal PE
            (ModelId::Causal, 768),             // E5: nomic-embed-text-v1.5
            (ModelId::Sparse, 30522),           // E6: SPLADE (sparse vocab)
            (ModelId::Code, 1536),              // E7: Qodo-Embed-1-1.5B native
            (ModelId::Graph, 1024),             // E8: e5-large-v2 (upgraded from MiniLM 384D)
            (ModelId::Hdc, 10000),              // E9: Hyperdimensional (10K-bit)
            (ModelId::Contextual, 768),         // E10: e5-base-v2
            (ModelId::Entity, 384),             // E11: Legacy MiniLM-L6-v2 (production uses Kepler 768D)
            (ModelId::LateInteraction, 128),    // E12: ColBERT per-token
            (ModelId::Splade, 30522),           // E13: SPLADE v3
            (ModelId::Kepler, 768),             // E11 production: KEPLER (RoBERTa-base)
        ];

        for (model_id, expected_dim) in models_and_dims {
            let embedding = ModelEmbedding::new(model_id, vec![0.1; expected_dim], 100);

            assert!(
                embedding.validate().is_ok(),
                "Validation failed for {:?} with dimension {}",
                model_id,
                expected_dim
            );
        }
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_large_vector_normalization() {
        let large_vec: Vec<f32> = (0..10000).map(|i| i as f32).collect();
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, large_vec, 100);
        embedding.normalize();

        assert!((embedding.l2_norm() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_very_small_values_normalize() {
        // Values with norm < f32::EPSILON should remain unchanged
        let original = vec![1e-20, 1e-20, 1e-20];
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, original.clone(), 100);

        // Verify norm is below EPSILON threshold
        let norm_before = embedding.l2_norm();
        assert!(norm_before < f32::EPSILON, "Test assumes norm < EPSILON");

        embedding.normalize();

        // Vector should remain unchanged (avoid division by near-zero)
        assert_eq!(embedding.vector, original);
        assert_eq!(embedding.l2_norm(), norm_before);
    }

    #[test]
    fn test_clone_preserves_all_fields() {
        let embedding = ModelEmbedding {
            model_id: ModelId::Causal,
            vector: vec![1.0, 2.0, 3.0],
            latency_us: 5000,
            attention_weights: Some(vec![0.5, 0.5]),
            is_projected: true,
        };

        let cloned = embedding.clone();
        assert_eq!(embedding, cloned);
    }
}
