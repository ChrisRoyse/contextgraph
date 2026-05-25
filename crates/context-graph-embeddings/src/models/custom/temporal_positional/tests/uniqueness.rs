//! Uniqueness and determinism tests for TemporalPositionalModel.

use crate::traits::EmbeddingModel;
use crate::types::ModelInput;

use super::super::TemporalPositionalModel;

#[tokio::test]
async fn test_deterministic_with_same_session_sequence() {
    let model = TemporalPositionalModel::new();

    let position = "session:test-session sequence:10";
    let input1 = ModelInput::text_with_instruction("content", position).expect("Failed to create");
    let input2 = ModelInput::text_with_instruction("content", position).expect("Failed to create");

    let embedding1 = model.embed(&input1).await.expect("First embed");
    let embedding2 = model.embed(&input2).await.expect("Second embed");

    assert_eq!(
        embedding1.vector, embedding2.vector,
        "Same session sequence must produce identical embeddings"
    );
}
