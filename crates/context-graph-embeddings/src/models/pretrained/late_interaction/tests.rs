//! Core tests for LateInteractionModel.

use super::*;
use crate::models::pretrained::shared::pretrained_test_model_path;
use crate::traits::SingleModelConfig;

pub(crate) fn create_test_model() -> LateInteractionModel {
    let model_path = pretrained_test_model_path("late-interaction");
    LateInteractionModel::new(&model_path, SingleModelConfig::default())
        .expect("Failed to create LateInteractionModel")
}

pub(crate) async fn create_and_load_model() -> LateInteractionModel {
    let model = create_test_model();
    model.load().await.expect("Failed to load model");
    model
}

#[test]
fn test_new_creates_unloaded_model() {
    let model = create_test_model();
    assert!(!model.is_initialized());
}
