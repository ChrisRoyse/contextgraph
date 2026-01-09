//! Core tests for EmbeddingModel trait methods.

use super::test_model::TestModel;
use crate::error::EmbeddingError;
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelId, ModelInput};
use std::collections::HashSet;

// =========================================================================
// MODEL ID TESTS (3 tests)
// =========================================================================

#[test]
fn test_model_id_returns_correct_value() {
    let model = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    assert_eq!(model.model_id(), ModelId::Semantic);

    let model2 = TestModel::new(ModelId::Code, vec![InputType::Code]);
    assert_eq!(model2.model_id(), ModelId::Code);
}

#[test]
fn test_model_id_for_all_12_models() {
    for model_id in ModelId::all() {
        let model = TestModel::new(*model_id, vec![InputType::Text]);
        assert_eq!(model.model_id(), *model_id);
    }
}

#[test]
fn test_model_id_is_consistent_across_calls() {
    let model = TestModel::new(ModelId::Graph, vec![InputType::Text]);
    assert_eq!(model.model_id(), model.model_id());
    assert_eq!(model.model_id(), model.model_id());
}

// =========================================================================
// SUPPORTED INPUT TYPES TESTS (3 tests)
// =========================================================================

#[test]
fn test_supported_input_types_returns_correct_list() {
    let supported = vec![InputType::Text, InputType::Code];
    let model = TestModel::new(ModelId::Semantic, supported.clone());
    assert_eq!(model.supported_input_types(), supported.as_slice());
}

#[test]
fn test_supports_input_type_true_for_supported() {
    let model = TestModel::new(ModelId::Multimodal, vec![InputType::Text, InputType::Image]);
    assert!(model.supports_input_type(InputType::Text));
    assert!(model.supports_input_type(InputType::Image));
}

#[test]
fn test_supports_input_type_false_for_unsupported() {
    let model = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    assert!(!model.supports_input_type(InputType::Image));
    assert!(!model.supports_input_type(InputType::Audio));
}

// =========================================================================
// EMBED TESTS (5 tests)
// =========================================================================

#[tokio::test]
async fn test_embed_returns_correct_model_id() {
    let model = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    let input = ModelInput::text("Hello, world!").unwrap();
    let embedding = model.embed(&input).await.unwrap();
    assert_eq!(embedding.model_id, ModelId::Semantic);
}

#[tokio::test]
async fn test_embed_returns_correct_dimension() {
    let model = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    let input = ModelInput::text("Test content").unwrap();
    let embedding = model.embed(&input).await.unwrap();
    assert_eq!(embedding.dimension(), ModelId::Semantic.dimension());
    assert_eq!(embedding.dimension(), 1024);
}

#[tokio::test]
async fn test_embed_rejects_unsupported_input_type() {
    let model = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    let input = ModelInput::image(vec![1, 2, 3, 4], crate::types::ImageFormat::Png).unwrap();
    let result = model.embed(&input).await;
    assert!(result.is_err());
    match result {
        Err(EmbeddingError::UnsupportedModality {
            model_id,
            input_type,
        }) => {
            assert_eq!(model_id, ModelId::Semantic);
            assert_eq!(input_type, InputType::Image);
        }
        _ => panic!("Expected UnsupportedModality error"),
    }
}

#[tokio::test]
async fn test_embed_rejects_when_not_initialized() {
    let model = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    model.set_initialized(false);
    let input = ModelInput::text("Test").unwrap();
    let result = model.embed(&input).await;
    assert!(result.is_err());
    match result {
        Err(EmbeddingError::NotInitialized { model_id }) => {
            assert_eq!(model_id, ModelId::Semantic);
        }
        _ => panic!("Expected NotInitialized error"),
    }
}

#[tokio::test]
async fn test_embed_deterministic_for_same_input() {
    let model = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    let input = ModelInput::text("Deterministic test").unwrap();

    let embedding1 = model.embed(&input).await.unwrap();
    let embedding2 = model.embed(&input).await.unwrap();

    assert_eq!(embedding1.vector, embedding2.vector);
}

// =========================================================================
// INITIALIZATION STATE TESTS (2 tests)
// =========================================================================

#[test]
fn test_is_initialized_returns_true_by_default() {
    let model = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    assert!(model.is_initialized());
}

#[test]
fn test_is_initialized_reflects_state_change() {
    let model = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    assert!(model.is_initialized());

    model.set_initialized(false);
    assert!(!model.is_initialized());

    model.set_initialized(true);
    assert!(model.is_initialized());
}

// =========================================================================
// DEFAULT METHOD TESTS (5 tests)
// =========================================================================

#[test]
fn test_dimension_delegates_to_model_id() {
    for model_id in ModelId::all() {
        let model = TestModel::new(*model_id, vec![InputType::Text]);
        assert_eq!(model.dimension(), model_id.dimension());
    }
}

#[test]
fn test_projected_dimension_delegates_to_model_id() {
    let sparse = TestModel::new(ModelId::Sparse, vec![InputType::Text]);
    assert_eq!(sparse.projected_dimension(), 1536);

    let semantic = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    assert_eq!(semantic.projected_dimension(), 1024);
}

#[test]
fn test_latency_budget_ms_delegates_to_model_id() {
    let semantic = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    assert_eq!(semantic.latency_budget_ms(), 5);

    let hdc = TestModel::new(ModelId::Hdc, vec![InputType::Text]);
    assert_eq!(hdc.latency_budget_ms(), 1);
}

#[test]
fn test_max_tokens_delegates_to_model_id() {
    let causal = TestModel::new(ModelId::Causal, vec![InputType::Text]);
    assert_eq!(causal.max_tokens(), 4096);

    let multimodal = TestModel::new(ModelId::Multimodal, vec![InputType::Text]);
    assert_eq!(multimodal.max_tokens(), 77);
}

#[test]
fn test_is_pretrained_delegates_to_model_id() {
    let semantic = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    assert!(semantic.is_pretrained());

    let temporal = TestModel::new(ModelId::TemporalRecent, vec![InputType::Text]);
    assert!(!temporal.is_pretrained());
}

// =========================================================================
// VALIDATE INPUT TESTS (2 tests)
// =========================================================================

#[test]
fn test_validate_input_accepts_supported_type() {
    let model = TestModel::new(ModelId::Code, vec![InputType::Text, InputType::Code]);

    let text_input = ModelInput::text("Hello").unwrap();
    assert!(model.validate_input(&text_input).is_ok());

    let code_input = ModelInput::code("fn main() {}", "rust").unwrap();
    assert!(model.validate_input(&code_input).is_ok());
}

#[test]
fn test_validate_input_rejects_unsupported_type() {
    let model = TestModel::new(ModelId::Semantic, vec![InputType::Text]);

    let image_input = ModelInput::image(vec![1, 2, 3], crate::types::ImageFormat::Png).unwrap();
    let result = model.validate_input(&image_input);

    assert!(result.is_err());
    match result {
        Err(EmbeddingError::UnsupportedModality {
            model_id,
            input_type,
        }) => {
            assert_eq!(model_id, ModelId::Semantic);
            assert_eq!(input_type, InputType::Image);
        }
        _ => panic!("Expected UnsupportedModality error"),
    }
}

// =========================================================================
// SEND + SYNC TESTS (2 tests)
// =========================================================================

#[test]
fn test_embedding_model_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<TestModel>();
}

#[test]
fn test_embedding_model_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<TestModel>();
}

// =========================================================================
// INPUT TYPE COVERAGE TEST (1 test)
// =========================================================================

#[test]
fn test_all_input_types_can_be_supported() {
    let model = TestModel::new(
        ModelId::Multimodal,
        vec![
            InputType::Text,
            InputType::Code,
            InputType::Image,
            InputType::Audio,
        ],
    );

    for input_type in InputType::all() {
        assert!(
            model.supports_input_type(*input_type),
            "Model should support {:?}",
            input_type
        );
    }
}

// =========================================================================
// HASHSET USAGE FOR INPUT TYPE CHECK (1 test)
// =========================================================================

#[test]
fn test_supported_types_can_use_hashset() {
    let model = TestModel::new(ModelId::Multimodal, vec![InputType::Text, InputType::Image]);

    let supported_set: HashSet<InputType> = model.supported_input_types().iter().copied().collect();

    assert!(supported_set.contains(&InputType::Text));
    assert!(supported_set.contains(&InputType::Image));
    assert!(!supported_set.contains(&InputType::Code));
    assert!(!supported_set.contains(&InputType::Audio));
}
