//! Batch, validation, concurrency and constants tests for LateInteractionModel.

use super::tests::{create_and_load_model, create_test_model};
use super::*;
use crate::error::EmbeddingError;
use crate::traits::EmbeddingModel;
use crate::types::{ModelId, ModelInput};

// ==================== TokenEmbeddings Validation Tests ====================

#[test]
fn test_token_embeddings_new_empty_vectors_fails() {
    let result = TokenEmbeddings::new(vec![], vec![], vec![]);
    assert!(matches!(result, Err(EmbeddingError::EmptyInput)));
}

#[test]
fn test_token_embeddings_new_wrong_dimension_fails() {
    let wrong_dim_vec = vec![0.0f32; 64];
    let result = TokenEmbeddings::new(vec![wrong_dim_vec], vec!["token".to_string()], vec![true]);
    assert!(matches!(
        result,
        Err(EmbeddingError::InvalidDimension { .. })
    ));
}

#[test]
fn test_token_embeddings_new_mismatched_lengths_fails() {
    let vec_128d = vec![0.0f32; 128];
    let result = TokenEmbeddings::new(
        vec![vec_128d.clone(), vec_128d],
        vec!["token1".to_string()],
        vec![true, true],
    );
    assert!(matches!(
        result,
        Err(EmbeddingError::InvalidDimension { .. })
    ));
}

// ==================== Batch Tests ====================

#[tokio::test]
async fn test_embed_batch_multiple_inputs() {
    let model = create_and_load_model().await;
    let inputs = vec![
        ModelInput::text("first document").expect("Input"),
        ModelInput::text("second document").expect("Input"),
        ModelInput::text("third document").expect("Input"),
    ];
    let embeddings = model.embed_batch(&inputs).await.expect("Batch embed");
    assert_eq!(embeddings.len(), 3);
    for emb in &embeddings {
        assert_eq!(emb.vector.len(), 128);
        assert_eq!(emb.model_id, ModelId::LateInteraction);
    }
}

#[tokio::test]
async fn test_embed_batch_before_load_fails() {
    let model = create_test_model();
    let inputs = vec![ModelInput::text("test").expect("Input")];
    let result = model.embed_batch(&inputs).await;
    assert!(matches!(result, Err(EmbeddingError::NotInitialized { .. })));
}

// ==================== Thread Safety Tests ====================

#[tokio::test]
async fn test_concurrent_embed_calls() {
    let model = std::sync::Arc::new(create_and_load_model().await);
    let mut handles = Vec::new();
    for i in 0..10 {
        let model = model.clone();
        let handle = tokio::spawn(async move {
            let text = format!("concurrent embed {}", i);
            let input = ModelInput::text(&text).expect("Input");
            model.embed(&input).await
        });
        handles.push(handle);
    }
    for handle in handles {
        let result = handle.await;
        let embedding = result
            .expect("Task should not panic")
            .expect("Embed should succeed");
        assert_eq!(embedding.vector.len(), 128);
    }
}

// ==================== Constants Tests ====================

#[test]
fn test_constants_are_correct() {
    assert_eq!(LATE_INTERACTION_DIMENSION, 128);
    assert_eq!(LATE_INTERACTION_MAX_TOKENS, 512);
    assert_eq!(LATE_INTERACTION_LATENCY_BUDGET_MS, 8);
    assert_eq!(LATE_INTERACTION_MODEL_NAME, "colbert-ir/colbertv2.0");
}

#[test]
fn test_model_id_dimension_matches_constant() {
    assert_eq!(
        ModelId::LateInteraction.dimension(),
        LATE_INTERACTION_DIMENSION
    );
    assert_eq!(
        ModelId::LateInteraction.projected_dimension(),
        LATE_INTERACTION_DIMENSION
    );
}

#[test]
fn test_model_id_latency_matches_constant() {
    assert_eq!(
        ModelId::LateInteraction.latency_budget_ms() as u64,
        LATE_INTERACTION_LATENCY_BUDGET_MS
    );
}

// ==================== Evidence of Success ====================

#[tokio::test]
async fn test_evidence_of_success() {
    let model = create_and_load_model().await;

    assert_eq!(model.model_id(), ModelId::LateInteraction);
    assert_eq!(model.dimension(), 128);
    assert_eq!(model.projected_dimension(), 128);
    assert_eq!(model.max_tokens(), 512);
    assert_eq!(model.latency_budget_ms(), 8);
    assert!(model.is_pretrained());

    let tokens = model
        .embed_tokens("ColBERT late interaction test")
        .await
        .expect("embed");
    for vec in tokens.vectors.iter() {
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }

    let pooled = model.pool_tokens(&tokens);
    let pooled_norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert_eq!(pooled.len(), 128);
    assert!((pooled_norm - 1.0).abs() < 0.001);

    let score = LateInteractionModel::maxsim_score(&tokens, &tokens);
    assert!(score > 0.0);
}
