//! Tests for MultiArrayEmbedding.

use super::MultiArrayEmbedding;
use crate::error::EmbeddingError;
use crate::types::dimensions::{MODEL_COUNT, TOTAL_DIMENSION};
use crate::types::{ModelEmbedding, ModelId};

// ========== Construction Tests ==========

#[test]
fn test_new_creates_empty_struct() {
    let mae = MultiArrayEmbedding::new();

    assert!(mae.embeddings.iter().all(|e| e.is_none()));
    assert_eq!(mae.total_latency_us, 0);
    assert_eq!(mae.content_hash, 0);
    assert!(!mae.is_complete());
    assert_eq!(mae.filled_count(), 0);
}

#[test]
fn test_default_equals_new() {
    let mae1 = MultiArrayEmbedding::new();
    let mae2 = MultiArrayEmbedding::default();

    assert_eq!(mae1.total_latency_us, mae2.total_latency_us);
    assert_eq!(mae1.content_hash, mae2.content_hash);
    assert_eq!(mae1.filled_count(), mae2.filled_count());
}

// ========== Set Tests ==========

#[test]
fn test_set_places_at_correct_index() {
    let mut mae = MultiArrayEmbedding::new();
    let mut emb = ModelEmbedding::new(ModelId::Semantic, vec![0.1; 1024], 1000);
    emb.set_projected(true);
    mae.set(emb);

    assert!(mae.embeddings[0].is_some()); // Semantic = 0
    assert_eq!(mae.filled_count(), 1);
    assert_eq!(mae.total_latency_us, 1000);
}

#[test]
fn test_set_all_models() {
    let mut mae = MultiArrayEmbedding::new();

    for model_id in ModelId::all() {
        let dim = model_id.projected_dimension();
        let mut emb = ModelEmbedding::new(*model_id, vec![0.1; dim], 100);
        emb.set_projected(true);
        mae.set(emb);
    }

    assert!(mae.is_complete());
    assert_eq!(mae.filled_count(), MODEL_COUNT);
    assert_eq!(mae.total_latency_us, MODEL_COUNT as u64 * 100); // 13 * 100
}

#[test]
#[should_panic(expected = "Dimension mismatch")]
fn test_set_wrong_dimension_panics() {
    let mut mae = MultiArrayEmbedding::new();
    // Semantic requires 1024, but we provide 512
    let mut emb = ModelEmbedding::new(ModelId::Semantic, vec![0.1; 512], 1000);
    emb.set_projected(true);
    mae.set(emb); // Should panic
}

// ========== Get Tests ==========

#[test]
fn test_get_returns_correct_embedding() {
    let mut mae = MultiArrayEmbedding::new();
    let mut emb = ModelEmbedding::new(ModelId::Causal, vec![0.1; 768], 500);
    emb.set_projected(true);
    mae.set(emb);

    let got = mae.get(ModelId::Causal);
    assert!(got.is_some());
    assert_eq!(got.unwrap().model_id, ModelId::Causal);
}

#[test]
fn test_get_returns_none_for_missing() {
    let mae = MultiArrayEmbedding::new();
    assert!(mae.get(ModelId::Semantic).is_none());
}

#[test]
fn test_get_vector_returns_slice() {
    let mut mae = MultiArrayEmbedding::new();
    let mut emb = ModelEmbedding::new(ModelId::Semantic, vec![0.5; 1024], 100);
    emb.set_projected(true);
    mae.set(emb);

    let vec = mae.get_vector(ModelId::Semantic);
    assert!(vec.is_some());
    assert_eq!(vec.unwrap().len(), 1024);
    assert!(vec.unwrap().iter().all(|&v| (v - 0.5).abs() < 1e-6));
}

// ========== Completion Tests ==========

#[test]
fn test_is_complete_only_when_all_13() {
    let mut mae = MultiArrayEmbedding::new();

    // Fill 12 models (all but the last)
    for model_id in ModelId::all().iter().take(MODEL_COUNT - 1) {
        let dim = model_id.projected_dimension();
        let mut emb = ModelEmbedding::new(*model_id, vec![0.1; dim], 100);
        emb.set_projected(true);
        mae.set(emb);
    }
    assert!(!mae.is_complete());
    assert_eq!(mae.filled_count(), MODEL_COUNT - 1);

    // Fill last model (Splade)
    let mut emb = ModelEmbedding::new(ModelId::Splade, vec![0.1; 1536], 100);
    emb.set_projected(true);
    mae.set(emb);
    assert!(mae.is_complete());
    assert_eq!(mae.filled_count(), MODEL_COUNT);
}

#[test]
fn test_missing_models_returns_correct_list() {
    let mut mae = MultiArrayEmbedding::new();
    let missing = mae.missing_models();
    assert_eq!(missing.len(), MODEL_COUNT);

    // Set one model
    let mut emb = ModelEmbedding::new(ModelId::Semantic, vec![0.1; 1024], 100);
    emb.set_projected(true);
    mae.set(emb);

    let missing = mae.missing_models();
    assert_eq!(missing.len(), 12); // 13 total - 1 filled = 12 missing
    assert!(!missing.contains(&ModelId::Semantic));
}

// ========== Hash Tests ==========

#[test]
fn test_compute_hash_deterministic() {
    let mut mae1 = create_complete_embedding();
    let mut mae2 = create_complete_embedding();

    mae1.compute_hash();
    mae2.compute_hash();

    assert_eq!(mae1.content_hash, mae2.content_hash);
    assert_ne!(mae1.content_hash, 0);
}

#[test]
fn test_content_hash_differs_for_different_data() {
    let mut mae1 = create_complete_embedding();
    mae1.compute_hash();
    let hash1 = mae1.content_hash;

    // Create another with different values
    let mut mae2 = MultiArrayEmbedding::new();
    for (i, model_id) in ModelId::all().iter().enumerate() {
        let dim = model_id.projected_dimension();
        let mut emb = ModelEmbedding::new(*model_id, vec![(i as f32) * 0.1; dim], 100);
        emb.set_projected(true);
        mae2.set(emb);
    }
    mae2.compute_hash();
    let hash2 = mae2.content_hash;

    assert_ne!(hash1, hash2);
}

#[test]
#[should_panic(expected = "Cannot compute hash")]
fn test_compute_hash_panics_when_incomplete() {
    let mut mae = MultiArrayEmbedding::new();
    mae.compute_hash(); // Should panic
}

// ========== Latency Tests ==========

#[test]
fn test_total_latency_sums_all() {
    let mut mae = MultiArrayEmbedding::new();

    for model_id in ModelId::all() {
        let dim = model_id.projected_dimension();
        let mut emb = ModelEmbedding::new(*model_id, vec![0.1; dim], 100);
        emb.set_projected(true);
        mae.set(emb);
    }

    assert_eq!(mae.total_latency_us, MODEL_COUNT as u64 * 100); // 13 * 100
}

// ========== Total Dimension Tests ==========

#[test]
fn test_total_dimension() {
    let mae = create_complete_embedding();
    // Sum of all projected dimensions (10624 for 13 models)
    assert_eq!(mae.total_dimension(), TOTAL_DIMENSION);
}

#[test]
fn test_total_dimension_partial() {
    let mut mae = MultiArrayEmbedding::new();
    let mut emb = ModelEmbedding::new(ModelId::Semantic, vec![0.1; 1024], 100);
    emb.set_projected(true);
    mae.set(emb);

    assert_eq!(mae.total_dimension(), 1024);
}

// ========== Validation Tests ==========

#[test]
fn test_validate_succeeds_for_valid_embeddings() {
    let mae = create_complete_embedding();
    assert!(mae.validate().is_ok());
}

#[test]
fn test_validate_detects_nan() {
    let mut mae = MultiArrayEmbedding::new();
    let mut vector = vec![0.1; 1024];
    vector[500] = f32::NAN;
    let mut emb = ModelEmbedding::new(ModelId::Semantic, vector, 100);
    emb.set_projected(true);
    mae.set(emb);

    let result = mae.validate();
    assert!(result.is_err());
    match result.unwrap_err() {
        EmbeddingError::InvalidValue { index, value } => {
            assert_eq!(index, 500);
            assert!(value.is_nan());
        }
        _ => panic!("Expected InvalidValue error"),
    }
}

// ========== Iterator Tests ==========

#[test]
fn test_iter_returns_all_present() {
    let mut mae = MultiArrayEmbedding::new();

    // Set 3 models
    for &model_id in &[ModelId::Semantic, ModelId::Causal, ModelId::Code] {
        let dim = model_id.projected_dimension();
        let mut emb = ModelEmbedding::new(model_id, vec![0.1; dim], 100);
        emb.set_projected(true);
        mae.set(emb);
    }

    let items: Vec<_> = mae.iter().collect();
    assert_eq!(items.len(), 3);
}

// ========== Edge Case Tests ==========

#[test]
fn edge_case_empty_struct() {
    let mae = MultiArrayEmbedding::new();
    let missing = mae.missing_models();
    assert_eq!(missing.len(), MODEL_COUNT);
}

#[test]
fn edge_case_overwrite() {
    let mut mae = MultiArrayEmbedding::new();
    let mut emb1 = ModelEmbedding::new(ModelId::Semantic, vec![1.0; 1024], 100);
    emb1.set_projected(true);
    mae.set(emb1);

    let mut emb2 = ModelEmbedding::new(ModelId::Semantic, vec![2.0; 1024], 200);
    emb2.set_projected(true);
    mae.set(emb2);

    // Latency should be replaced (old subtracted, new added)
    assert_eq!(mae.total_latency_us, 200);
    assert_eq!(mae.embeddings[0].as_ref().unwrap().vector[0], 2.0);
}

#[test]
fn edge_case_max_latency() {
    let mut mae = MultiArrayEmbedding::new();
    let mut emb = ModelEmbedding::new(ModelId::Semantic, vec![0.1; 1024], u64::MAX);
    emb.set_projected(true);
    mae.set(emb);

    assert_eq!(mae.total_latency_us, u64::MAX);
}

// ========== Multi-Array Storage Verification ==========

#[test]
fn verify_multi_array_storage() {
    // Each embedding is stored SEPARATELY at its native dimension
    let mut mae = create_complete_embedding();
    mae.compute_hash();

    // 1. Verify all 13 embeddings stored separately
    assert!(mae.is_complete());
    assert_eq!(mae.filled_count(), MODEL_COUNT);

    // 2. Verify each embedding has correct dimension
    for &model_id in ModelId::all() {
        let emb = mae.get(model_id).expect("Should have embedding");
        assert_eq!(emb.vector.len(), model_id.projected_dimension());
    }

    // 3. Verify hash is non-zero
    assert_ne!(mae.content_hash, 0);

    // 4. Verify total dimension is sum of all (10624 for 13 models)
    assert_eq!(mae.total_dimension(), TOTAL_DIMENSION);
}

// ========== Helper Functions ==========

fn create_complete_embedding() -> MultiArrayEmbedding {
    let mut mae = MultiArrayEmbedding::new();
    for model_id in ModelId::all() {
        let dim = model_id.projected_dimension();
        let mut emb = ModelEmbedding::new(*model_id, vec![0.5; dim], 100);
        emb.set_projected(true);
        mae.set(emb);
    }
    mae
}
