//! Embedding and trait implementation tests for TemporalPositionalModel.

use crate::traits::EmbeddingModel;
use crate::types::ModelInput;

use super::super::TemporalPositionalModel;

#[tokio::test]
async fn test_embed_returns_512d_vector() {
    let model = TemporalPositionalModel::new();
    let input =
        ModelInput::text_with_instruction("test content", "session:test-session sequence:0")
            .expect("Failed to create input");

    let embedding = model.embed(&input).await.expect("Embed should succeed");

    println!("Vector length: {}", embedding.vector.len());
    assert_eq!(embedding.vector.len(), 512, "Must return exactly 512D");
}

#[tokio::test]
async fn test_embed_rejects_missing_position_instruction() {
    let model = TemporalPositionalModel::new();
    let input = ModelInput::text("test content").expect("Failed to create input");

    let err = model.embed(&input).await.unwrap_err();

    assert!(
        err.to_string().contains("[TEMPORAL_INPUT_INVALID]"),
        "missing E4 session sequence instruction must fail closed, got {err}"
    );
}
