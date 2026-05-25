//! Embedding output and validation tests for TemporalPeriodicModel.

use crate::models::custom::temporal_periodic::TemporalPeriodicModel;
use crate::traits::EmbeddingModel;
use crate::types::ModelInput;

#[tokio::test]
async fn test_embed_returns_512d_vector() {
    let model = TemporalPeriodicModel::new();
    let input = ModelInput::text_with_instruction("test content", "timestamp:2024-01-15T10:30:00Z")
        .expect("Failed to create input");

    let embedding = model.embed(&input).await.expect("Embed should succeed");

    assert_eq!(embedding.vector.len(), 512, "Must return exactly 512D");
}

#[tokio::test]
async fn test_embed_rejects_missing_timestamp_instruction() {
    let model = TemporalPeriodicModel::new();
    let input = ModelInput::text("test content").expect("Failed to create input");

    let err = model.embed(&input).await.unwrap_err();

    assert!(
        err.to_string().contains("[TEMPORAL_INPUT_INVALID]"),
        "missing timestamp instruction must fail closed, got {err}"
    );
}
