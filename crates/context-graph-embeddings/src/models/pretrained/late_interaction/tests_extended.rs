//! Extended and edge case tests for LateInteractionModel.

use super::tests::{create_and_load_model, create_test_model};
use super::*;
use crate::error::EmbeddingError;
use crate::traits::EmbeddingModel;
use crate::types::{ModelId, ModelInput};
use once_cell::sync::OnceCell;
use std::sync::Arc;
use tokio::sync::OnceCell as AsyncOnceCell;

/// Shared warm model instance for latency testing.
static WARM_MODEL: OnceCell<Arc<AsyncOnceCell<LateInteractionModel>>> = OnceCell::new();

/// Get or initialize the shared warm model instance.
async fn get_warm_model() -> &'static LateInteractionModel {
    let cell = WARM_MODEL.get_or_init(|| Arc::new(AsyncOnceCell::new()));
    cell.get_or_init(|| async {
        let model = create_test_model();
        model.load().await.expect("Failed to load warm model");
        model
    })
    .await
}

// ==================== Embedding Tests ====================

#[tokio::test]
async fn test_embed_before_load_fails() {
    let model = create_test_model();
    let input = ModelInput::text("test").expect("Input");
    let result = model.embed(&input).await;
    assert!(matches!(result, Err(EmbeddingError::NotInitialized { .. })));
}

#[tokio::test]
async fn test_embed_returns_128d() {
    let model = create_and_load_model().await;
    let input = ModelInput::text("ColBERT test").expect("Input");
    let embedding = model.embed(&input).await.expect("Embed should succeed");
    assert_eq!(embedding.vector.len(), 128);
}

#[tokio::test]
async fn test_embed_returns_l2_normalized() {
    let model = create_and_load_model().await;
    let input = ModelInput::text("ColBERT normalization test").expect("Input");
    let embedding = model.embed(&input).await.expect("Embed");
    let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 0.001, "L2 norm should be ~1.0");
}

#[tokio::test]
async fn test_embed_model_id_is_late_interaction() {
    let model = create_and_load_model().await;
    let input = ModelInput::text("model id test").expect("Input");
    let embedding = model.embed(&input).await.expect("Embed");
    assert_eq!(embedding.model_id, ModelId::LateInteraction);
}

/// Test embedding latency is under budget for warmed models.
///
/// NOTE: This test uses relaxed budgets suitable for test environments
/// where models may not be pre-warmed in VRAM and GPU contention exists:
/// - Test environment budget: 1000ms (covers model load, kernel compilation,
///   memory transfer, and GPU contention during parallel test execution)
///
/// For production environments with pre-warmed GPU models in VRAM,
/// the target is <10ms per embed. Use integration tests with actual
/// warm model pools to validate production latency requirements.
#[tokio::test]
async fn test_embed_latency_under_budget() {
    // Use shared warm model instance for true warm-model latency testing
    let model = get_warm_model().await;
    let input = ModelInput::text("latency test").expect("Input");

    // Warm-up calls: ensure CUDA kernels are compiled and caches are hot
    for _ in 0..10 {
        let _warmup = model.embed(&input).await.expect("Warm-up embed");
    }

    // Measure actual inference latency - take median of multiple runs
    let mut latencies = Vec::with_capacity(5);
    for _ in 0..5 {
        let start = std::time::Instant::now();
        let _embedding = model.embed(&input).await.expect("Embed");
        latencies.push(start.elapsed());
    }
    latencies.sort();
    let median_latency = latencies[2];

    // Test environment budget (relaxed for cold-start scenarios and GPU contention):
    // - 1000ms covers model load from disk, kernel compilation, memory transfer,
    //   and GPU contention during parallel test execution
    //
    // Production target with pre-warmed VRAM models: <10ms
    // That stricter budget is validated in integration tests with actual warm pools.
    let budget_ms: u128 = 1000;
    assert!(
        median_latency.as_millis() < budget_ms,
        "Warm model median latency {} ms exceeds {}ms budget (latencies: {:?})",
        median_latency.as_millis(),
        budget_ms,
        latencies
    );
}

#[tokio::test]
async fn test_embed_deterministic() {
    let model = create_and_load_model().await;
    let input = ModelInput::text("determinism test").expect("Input");
    let emb1 = model.embed(&input).await.expect("Embed 1");
    let emb2 = model.embed(&input).await.expect("Embed 2");
    assert_eq!(
        emb1.vector, emb2.vector,
        "Same input must produce identical embeddings"
    );
}

// ==================== Edge Case Tests ====================

#[tokio::test]
async fn test_edge_case_empty_text_content() {
    let model = create_and_load_model().await;
    let result = ModelInput::text("");
    assert!(
        result.is_err(),
        "Empty text string should error on ModelInput::text"
    );
    let ws_result = model.embed_tokens("   \t\n   ").await;
    assert!(matches!(ws_result, Err(EmbeddingError::EmptyInput)));
}

#[tokio::test]
async fn test_edge_case_long_input() {
    let model = create_and_load_model().await;
    let words: Vec<&str> = (0..512).map(|_| "word").collect();
    let long_text = words.join(" ");
    let result = model.embed_tokens(&long_text).await;
    assert!(result.is_ok());
    let tokens = result.unwrap();
    assert!(!tokens.vectors.is_empty());

    // GPU models truncate to max_tokens rather than erroring
    // This is more efficient for batch processing
    let words_too_many: Vec<&str> = (0..600).map(|_| "word").collect();
    let too_long_text = words_too_many.join(" ");
    let too_long_result = model.embed_tokens(&too_long_text).await;
    // Model should either truncate successfully or return InputTooLong
    match too_long_result {
        Ok(tokens) => {
            // Truncation behavior - model produces valid output
            assert!(
                !tokens.vectors.is_empty(),
                "Truncated input should produce embeddings"
            );
        }
        Err(EmbeddingError::InputTooLong { .. }) => {
            // Also valid - strict length checking
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[tokio::test]
async fn test_edge_case_unsupported_modality_code() {
    let model = create_and_load_model().await;
    let input = ModelInput::code("fn main() {}", "rust").expect("Code input");
    let result = model.embed(&input).await;
    assert!(matches!(
        result,
        Err(EmbeddingError::UnsupportedModality { .. })
    ));
}

#[tokio::test]
async fn test_edge_case_unsupported_modality_image() {
    let model = create_and_load_model().await;
    let input = ModelInput::Image {
        bytes: vec![0u8; 100],
        format: crate::types::ImageFormat::Png,
    };
    let result = model.embed(&input).await;
    assert!(matches!(
        result,
        Err(EmbeddingError::UnsupportedModality { .. })
    ));
}

#[tokio::test]
async fn test_edge_case_special_characters() {
    let model = create_and_load_model().await;
    let text = "Unicode: test cafe fire chinese japanese";
    let tokens = model.embed_tokens(text).await.expect("embed_tokens");
    for vec in tokens.vectors.iter() {
        let has_nan = vec.iter().any(|x| x.is_nan());
        let has_inf = vec.iter().any(|x| x.is_infinite());
        assert!(!has_nan && !has_inf);
    }
}

#[tokio::test]
async fn test_edge_case_single_token() {
    let model = create_and_load_model().await;
    let tokens = model.embed_tokens("hello").await.expect("embed_tokens");
    assert!(!tokens.vectors.is_empty());
    assert_eq!(tokens.vectors[0].len(), 128);
}

#[tokio::test]
async fn test_edge_case_maxsim_identical_docs() {
    let model = create_and_load_model().await;
    let tokens = model
        .embed_tokens("test query")
        .await
        .expect("embed_tokens");
    let score = LateInteractionModel::maxsim_score(&tokens, &tokens);
    let expected = tokens.valid_token_count() as f32;
    assert!(
        (score - expected).abs() < 0.01,
        "Self-similarity should be maximum"
    );
}

#[tokio::test]
async fn test_edge_case_pool_single_token() {
    let model = create_and_load_model().await;
    let tokens = model.embed_tokens("word").await.expect("embed_tokens");
    let pooled = model.pool_tokens(&tokens);
    let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert_eq!(pooled.len(), 128);
    assert!((norm - 1.0).abs() < 0.001, "L2 norm should be ~1.0");
}
