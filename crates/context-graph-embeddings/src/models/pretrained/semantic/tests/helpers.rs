//! Test helpers for the semantic embedding model.

use crate::models::pretrained::shared::pretrained_test_model_path;
use crate::traits::SingleModelConfig;

use super::super::SemanticModel;

/// Helper to create a test model.
pub async fn create_test_model() -> SemanticModel {
    let model_path = pretrained_test_model_path("semantic");
    SemanticModel::new(&model_path, SingleModelConfig::default()).expect("Failed to create model")
}

/// Helper to create and load a test model.
pub async fn create_and_load_model() -> SemanticModel {
    let model = create_test_model().await;
    model.load().await.expect("Failed to load model");
    model
}
