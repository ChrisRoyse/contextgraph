//! Embedding behavior tests for the semantic embedding model.

use crate::error::EmbeddingError;
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelId, ModelInput};

use super::super::SEMANTIC_DIMENSION;
use super::helpers::create_and_load_model;

#[tokio::test]
async fn test_unsupported_code_input() {
    let model = create_and_load_model().await;
    let input = ModelInput::code("fn main() {}", "rust").expect("Failed to create code input");

    let result = model.embed(&input).await;
    assert!(result.is_err());
    match result {
        Err(EmbeddingError::UnsupportedModality {
            model_id,
            input_type,
        }) => {
            assert_eq!(model_id, ModelId::Semantic);
            assert_eq!(input_type, InputType::Code);
        }
        other => panic!("Expected UnsupportedModality error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_deterministic_embedding() {
    let model = create_and_load_model().await;
    let input = ModelInput::text("Test determinism").expect("Failed to create input");

    let embedding1 = model.embed(&input).await.unwrap();
    let embedding2 = model.embed(&input).await.unwrap();

    // Same input should produce identical embeddings
    assert_eq!(
        embedding1.vector, embedding2.vector,
        "Embeddings should be deterministic"
    );
}

#[tokio::test]
async fn test_different_inputs_different_embeddings() {
    let model = create_and_load_model().await;
    let input1 = ModelInput::text("First sentence").expect("Failed to create input");
    let input2 = ModelInput::text("Second sentence").expect("Failed to create input");

    let embedding1 = model.embed(&input1).await.unwrap();
    let embedding2 = model.embed(&input2).await.unwrap();

    // Different inputs should produce different embeddings
    assert_ne!(
        embedding1.vector, embedding2.vector,
        "Different inputs should produce different embeddings"
    );
}

#[tokio::test]
async fn test_query_vs_passage_different_embeddings() {
    let model = create_and_load_model().await;
    let content = "How does photosynthesis work?";

    // Passage mode (default)
    let passage_input = ModelInput::text(content).expect("Failed to create input");
    let passage_embedding = model.embed(&passage_input).await.unwrap();

    // Query mode - use text_with_instruction with "query" in instruction
    let query_input =
        ModelInput::text_with_instruction(content, "query:").expect("Failed to create input");
    let query_embedding = model.embed(&query_input).await.unwrap();

    // Query and passage should produce different embeddings due to prefix
    assert_ne!(
        passage_embedding.vector, query_embedding.vector,
        "Query and passage modes should produce different embeddings"
    );
}

#[tokio::test]
async fn test_embed_batch() {
    let model = create_and_load_model().await;
    let inputs = vec![
        ModelInput::text("First").expect("Failed to create input"),
        ModelInput::text("Second").expect("Failed to create input"),
        ModelInput::text("Third").expect("Failed to create input"),
    ];

    let embeddings = model.embed_batch(&inputs).await.unwrap();

    assert_eq!(embeddings.len(), 3);
    for embedding in &embeddings {
        assert_eq!(embedding.vector.len(), SEMANTIC_DIMENSION);
        let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.001,
            "Each embedding must be L2 normalized"
        );
    }
}
