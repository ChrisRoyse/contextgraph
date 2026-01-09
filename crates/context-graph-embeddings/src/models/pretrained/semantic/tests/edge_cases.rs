//! Edge case tests for the semantic embedding model.

use crate::error::EmbeddingError;
use crate::traits::EmbeddingModel;
use crate::types::{ModelId, ModelInput};
use serial_test::serial;

use super::super::SEMANTIC_DIMENSION;
use super::helpers::create_test_model;

#[tokio::test]
async fn test_edge_1_empty_input_text() {
    let model = create_test_model().await;
    model.load().await.unwrap();

    // BEFORE state
    println!(
        "EDGE-1 BEFORE: model initialized = {}",
        model.is_initialized()
    );

    // Empty input returns EmptyInput error from ModelInput::text()
    let result = ModelInput::text("");
    assert!(
        result.is_err(),
        "Empty string should error on ModelInput::text"
    );

    // However, if we construct text with just whitespace, it should still work
    let input = ModelInput::text(" ").expect("Whitespace input should work");
    let result = model.embed(&input).await;

    // AFTER state
    println!("EDGE-1 AFTER: result = {:?}", result.is_ok());

    // Should produce a valid 1024D embedding
    assert!(
        result.is_ok(),
        "Whitespace input should produce valid embedding"
    );
    let embedding = result.unwrap();
    assert_eq!(
        embedding.vector.len(),
        SEMANTIC_DIMENSION,
        "Vector dimension must be 1024"
    );
}

#[tokio::test]
async fn test_edge_2_max_tokens_exceeded() {
    let model = create_test_model().await;
    model.load().await.unwrap();

    // Create input > 512 tokens (approximately)
    let long_text = "word ".repeat(1000); // ~1000 tokens

    // BEFORE state
    println!("EDGE-2 BEFORE: input length = {} chars", long_text.len());

    let input = ModelInput::text(&long_text).expect("Failed to create long input");
    let result = model.embed(&input).await;

    // AFTER state
    println!("EDGE-2 AFTER: result = {:?}", result.is_ok());

    // MUST truncate to 512 tokens (stub doesn't actually tokenize, just hashes)
    assert!(result.is_ok(), "Long input should be handled gracefully");
    let embedding = result.unwrap();
    assert_eq!(
        embedding.vector.len(),
        SEMANTIC_DIMENSION,
        "Vector dimension must be 1024"
    );
}

#[tokio::test]
async fn test_edge_3_embed_before_load() {
    let model = create_test_model().await;
    // Deliberately NOT calling load()

    // BEFORE state
    println!(
        "EDGE-3 BEFORE: model initialized = {}",
        model.is_initialized()
    );

    let input = ModelInput::text("test").expect("Failed to create input");
    let result = model.embed(&input).await;

    // AFTER state
    println!("EDGE-3 AFTER: result = {:?}", result);

    // MUST return NotInitialized error immediately
    assert!(result.is_err(), "Should fail when not loaded");
    match result {
        Err(EmbeddingError::NotInitialized { model_id }) => {
            assert_eq!(model_id, ModelId::Semantic);
        }
        other => panic!("Expected NotInitialized error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_embed_batch_before_load() {
    let model = create_test_model().await;
    let inputs = vec![ModelInput::text("Test").expect("Failed to create input")];

    let result = model.embed_batch(&inputs).await;
    assert!(result.is_err());
    match result {
        Err(EmbeddingError::NotInitialized { model_id }) => {
            assert_eq!(model_id, ModelId::Semantic);
        }
        other => panic!("Expected NotInitialized error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_unload_before_load_fails() {
    let model = create_test_model().await;

    let result = model.unload().await;
    assert!(result.is_err());
    match result {
        Err(EmbeddingError::NotInitialized { model_id }) => {
            assert_eq!(model_id, ModelId::Semantic);
        }
        other => panic!("Expected NotInitialized error, got {:?}", other),
    }
}

#[tokio::test]
#[serial]
async fn test_load_twice_is_idempotent() {
    let model = create_test_model().await;

    model.load().await.expect("First load should succeed");
    assert!(model.is_initialized());

    model.load().await.expect("Second load should succeed");
    assert!(model.is_initialized());
}
