//! Token embedding and pooling tests for LateInteractionModel.

use super::tests::create_and_load_model;
use super::*;
#[allow(unused_imports)]
use crate::error::EmbeddingError;
#[allow(unused_imports)]
use crate::traits::EmbeddingModel;

// ==================== Token Embedding Tests ====================

#[tokio::test]
async fn test_embed_tokens_produces_per_token_vectors() {
    let model = create_and_load_model().await;
    let tokens = model
        .embed_tokens("hello world test")
        .await
        .expect("embed_tokens");
    assert!(tokens.vectors.len() >= 3);
}

#[tokio::test]
async fn test_each_token_is_128d() {
    let model = create_and_load_model().await;
    let tokens = model
        .embed_tokens("hello world")
        .await
        .expect("embed_tokens");
    for vec in tokens.vectors.iter() {
        assert_eq!(vec.len(), 128);
    }
}

#[tokio::test]
async fn test_token_vectors_l2_normalized() {
    let model = create_and_load_model().await;
    let tokens = model
        .embed_tokens("test embedding normalization")
        .await
        .expect("embed_tokens");
    for (i, vec) in tokens.vectors.iter().enumerate() {
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.001,
            "Token {} L2 norm should be ~1.0, got {}",
            i,
            norm
        );
    }
}

#[tokio::test]
async fn test_embed_tokens_deterministic() {
    let model = create_and_load_model().await;
    let tok1 = model.embed_tokens("hello world").await.expect("embed 1");
    let tok2 = model.embed_tokens("hello world").await.expect("embed 2");
    assert_eq!(tok1.vectors.len(), tok2.vectors.len());
    for i in 0..tok1.vectors.len() {
        assert_eq!(
            tok1.vectors[i], tok2.vectors[i],
            "Token {} should be deterministic",
            i
        );
    }
}

#[tokio::test]
async fn test_embed_tokens_different_inputs_differ() {
    let model = create_and_load_model().await;
    let tok1 = model.embed_tokens("hello").await.expect("embed 1");
    let tok2 = model.embed_tokens("world").await.expect("embed 2");
    assert_ne!(
        tok1.vectors[0], tok2.vectors[0],
        "Different inputs should differ"
    );
}

#[tokio::test]
async fn test_valid_token_count() {
    let model = create_and_load_model().await;
    let tokens = model
        .embed_tokens("one two three")
        .await
        .expect("embed_tokens");
    assert_eq!(tokens.valid_token_count(), tokens.tokens.len());
    assert!(tokens.valid_token_count() >= 3);
}

// ==================== Pooling Tests ====================

#[tokio::test]
async fn test_pool_tokens_produces_128d() {
    let model = create_and_load_model().await;
    let tokens = model
        .embed_tokens("test input")
        .await
        .expect("embed_tokens");
    let pooled = model.pool_tokens(&tokens);
    assert_eq!(pooled.len(), 128);
}

#[tokio::test]
async fn test_pool_tokens_l2_normalized() {
    let model = create_and_load_model().await;
    let tokens = model
        .embed_tokens("test input")
        .await
        .expect("embed_tokens");
    let pooled = model.pool_tokens(&tokens);
    let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 0.001,
        "Pooled vector should be L2 normalized"
    );
}

#[tokio::test]
async fn test_pool_tokens_no_nan_inf() {
    let model = create_and_load_model().await;
    let tokens = model
        .embed_tokens("check for NaN and Inf")
        .await
        .expect("embed_tokens");
    let pooled = model.pool_tokens(&tokens);
    let has_nan = pooled.iter().any(|x| x.is_nan());
    let has_inf = pooled.iter().any(|x| x.is_infinite());
    assert!(!has_nan && !has_inf);
}

#[tokio::test]
async fn test_pool_single_token() {
    let model = create_and_load_model().await;
    let tokens = model.embed_tokens("hello").await.expect("embed_tokens");
    let pooled = model.pool_tokens(&tokens);
    let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert_eq!(pooled.len(), 128);
    assert!((norm - 1.0).abs() < 0.001);
}

// ==================== MaxSim Tests ====================

#[tokio::test]
async fn test_maxsim_score_identical_returns_max() {
    let model = create_and_load_model().await;
    let tokens = model
        .embed_tokens("test query")
        .await
        .expect("embed_tokens");
    let score = LateInteractionModel::maxsim_score(&tokens, &tokens);
    let expected_max = tokens.valid_token_count() as f32;
    assert!(
        (score - expected_max).abs() < 0.01,
        "Score {} != expected {}",
        score,
        expected_max
    );
}

#[tokio::test]
async fn test_maxsim_score_different_docs() {
    let model = create_and_load_model().await;
    let query = model.embed_tokens("search query").await.expect("query");
    let doc1 = model.embed_tokens("search query").await.expect("doc1");
    let doc2 = model.embed_tokens("unrelated text").await.expect("doc2");
    let score1 = LateInteractionModel::maxsim_score(&query, &doc1);
    let score2 = LateInteractionModel::maxsim_score(&query, &doc2);
    assert!(
        score1 > score2,
        "Identical should score higher than different"
    );
}

#[tokio::test]
async fn test_maxsim_score_positive() {
    let model = create_and_load_model().await;
    let query = model.embed_tokens("hello").await.expect("query");
    let doc = model.embed_tokens("world").await.expect("doc");
    let score = LateInteractionModel::maxsim_score(&query, &doc);
    assert!(score.is_finite(), "Score should be finite");
}

#[tokio::test]
async fn test_batch_maxsim() {
    let model = create_and_load_model().await;
    let query = model.embed_tokens("query text").await.expect("query");
    let docs = vec![
        model.embed_tokens("doc one").await.expect("doc1"),
        model.embed_tokens("doc two").await.expect("doc2"),
        model.embed_tokens("doc three").await.expect("doc3"),
    ];
    let scores = LateInteractionModel::batch_maxsim(&query, &docs);
    assert_eq!(scores.len(), 3);
    assert!(scores.iter().all(|s| s.is_finite()));
}
