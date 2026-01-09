//! Basic tests for the semantic embedding model.

use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelId};

use super::super::{
    PASSAGE_PREFIX, QUERY_PREFIX, SEMANTIC_DIMENSION, SEMANTIC_LATENCY_BUDGET_MS,
    SEMANTIC_MAX_TOKENS,
};
use super::helpers::create_test_model;

#[tokio::test]
async fn test_model_id_is_semantic() {
    let model = create_test_model().await;
    assert_eq!(model.model_id(), ModelId::Semantic);
}

#[tokio::test]
async fn test_supported_input_types_is_text() {
    let model = create_test_model().await;
    let types = model.supported_input_types();
    assert_eq!(types.len(), 1);
    assert_eq!(types[0], InputType::Text);
}

#[tokio::test]
async fn test_constants_are_correct() {
    assert_eq!(SEMANTIC_DIMENSION, 1024);
    assert_eq!(SEMANTIC_MAX_TOKENS, 512);
    assert_eq!(SEMANTIC_LATENCY_BUDGET_MS, 5);
    assert_eq!(QUERY_PREFIX, "query: ");
    assert_eq!(PASSAGE_PREFIX, "passage: ");
}
