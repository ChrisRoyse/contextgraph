//! Integration and concurrency tests for EmbeddingModel trait.

use super::test_model::TestModel;
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelId, ModelInput};
use std::sync::Arc;

// =========================================================================
// OBJECT SAFETY TEST (1 test)
// =========================================================================

#[tokio::test]
async fn test_trait_is_object_safe() {
    // Test that EmbeddingModel can be used as a trait object
    let model: Box<dyn EmbeddingModel> =
        Box::new(TestModel::new(ModelId::Semantic, vec![InputType::Text]));

    assert_eq!(model.model_id(), ModelId::Semantic);
    assert!(model.supports_input_type(InputType::Text));
    assert!(!model.supports_input_type(InputType::Image));

    let input = ModelInput::text("Test").unwrap();
    let embedding = model.embed(&input).await.unwrap();
    assert_eq!(embedding.model_id, ModelId::Semantic);
}

// =========================================================================
// CONCURRENT USAGE TEST (1 test)
// =========================================================================

#[tokio::test]
async fn test_model_can_be_shared_across_tasks() {
    let model = Arc::new(TestModel::new(
        ModelId::Entity,
        vec![InputType::Text, InputType::Code],
    ));

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let m = Arc::clone(&model);
            tokio::spawn(async move {
                let input = ModelInput::text(format!("Task {}", i)).unwrap();
                m.embed(&input).await
            })
        })
        .collect();

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().model_id, ModelId::Entity);
    }
}

// =========================================================================
// ALL 14 MODELS DIMENSION TEST (1 test)
// =========================================================================

#[tokio::test]
async fn test_all_14_models_produce_correct_dimensions() {
    for model_id in ModelId::all() {
        let model = TestModel::new(*model_id, vec![InputType::Text]);
        let input = ModelInput::text("Test embedding").unwrap();
        let embedding = model.embed(&input).await.unwrap();

        assert_eq!(
            embedding.dimension(),
            model_id.dimension(),
            "Model {:?} produced wrong dimension",
            model_id
        );
    }
}

// =========================================================================
// DYN TRAIT REFERENCE TEST (1 test)
// =========================================================================

#[tokio::test]
async fn test_dyn_trait_reference_works() {
    let model = TestModel::new(ModelId::Graph, vec![InputType::Text]);
    let model_ref: &dyn EmbeddingModel = &model;

    assert_eq!(model_ref.model_id(), ModelId::Graph);
    assert_eq!(model_ref.dimension(), 1024); // e5-large-v2 (upgraded from MiniLM 384D)
    assert!(model_ref.is_initialized());

    let input = ModelInput::text("Reference test").unwrap();
    let embedding = model_ref.embed(&input).await.unwrap();
    assert_eq!(embedding.model_id, ModelId::Graph);
}

// =========================================================================
// MULTIPLE MODEL INSTANCES TEST (1 test)
// =========================================================================

#[tokio::test]
async fn test_multiple_model_instances_independent() {
    let semantic = TestModel::new(ModelId::Semantic, vec![InputType::Text]);
    let code_model = TestModel::new(ModelId::Code, vec![InputType::Code]);

    let text_input = ModelInput::text("Hello").unwrap();
    let code_input = ModelInput::code("fn main() {}", "rust").unwrap();

    let sem_emb = semantic.embed(&text_input).await.unwrap();
    let code_emb = code_model.embed(&code_input).await.unwrap();

    assert_eq!(sem_emb.model_id, ModelId::Semantic);
    assert_eq!(code_emb.model_id, ModelId::Code);
    assert_eq!(sem_emb.dimension(), 1024);
    assert_eq!(code_emb.dimension(), 1536);
}

// =========================================================================
// VECTOR TRAIT OBJECTS TEST (1 test)
// =========================================================================

#[tokio::test]
async fn test_vector_of_trait_objects() {
    let models: Vec<Box<dyn EmbeddingModel>> = vec![
        Box::new(TestModel::new(ModelId::Semantic, vec![InputType::Text])),
        Box::new(TestModel::new(ModelId::Code, vec![InputType::Code])),
        Box::new(TestModel::new(ModelId::Graph, vec![InputType::Text])),
    ];

    let expected_ids = [ModelId::Semantic, ModelId::Code, ModelId::Graph];

    for (model, expected_id) in models.iter().zip(expected_ids.iter()) {
        assert_eq!(model.model_id(), *expected_id);
    }
}

// =========================================================================
// ARC TRAIT OBJECT TEST (1 test)
// =========================================================================

#[tokio::test]
async fn test_arc_trait_object() {
    let model: Arc<dyn EmbeddingModel> = Arc::new(TestModel::new(
        ModelId::Causal,
        vec![InputType::Text, InputType::Code],
    ));

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let m = Arc::clone(&model);
            tokio::spawn(async move {
                assert_eq!(m.model_id(), ModelId::Causal);
                let input = ModelInput::text(format!("Arc test {}", i)).unwrap();
                m.embed(&input).await
            })
        })
        .collect();

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}

// =========================================================================
// DIMENSION CONSISTENCY TEST (1 test)
// =========================================================================

#[tokio::test]
async fn test_dimension_consistency_across_calls() {
    let model = TestModel::new(ModelId::LateInteraction, vec![InputType::Text]);

    let inputs = vec![
        ModelInput::text("First").unwrap(),
        ModelInput::text("Second longer input").unwrap(),
        ModelInput::text("Third with even more content here").unwrap(),
    ];

    for input in inputs {
        let embedding = model.embed(&input).await.unwrap();
        assert_eq!(
            embedding.dimension(),
            ModelId::LateInteraction.dimension(),
            "Dimension should be consistent regardless of input length"
        );
    }
}
