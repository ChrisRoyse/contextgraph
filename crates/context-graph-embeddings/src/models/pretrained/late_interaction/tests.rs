//! Core tests for LateInteractionModel.

use super::*;
use crate::error::EmbeddingError;
use crate::traits::{EmbeddingModel, SingleModelConfig};
use crate::types::{InputType, ModelId};
use serial_test::serial;
use std::path::PathBuf;

/// Get the workspace root directory for test model paths.
fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(crate) fn create_test_model() -> LateInteractionModel {
    let model_path = workspace_root().join("models/late-interaction");
    LateInteractionModel::new(&model_path, SingleModelConfig::default())
        .expect("Failed to create LateInteractionModel")
}

pub(crate) async fn create_and_load_model() -> LateInteractionModel {
    let model = create_test_model();
    model.load().await.expect("Failed to load model");
    model
}

// ==================== Construction Tests ====================

#[test]
fn test_new_creates_unloaded_model() {
    let model = create_test_model();
    assert!(!model.is_initialized());
}

#[test]
fn test_new_with_zero_batch_size_fails() {
    let config = SingleModelConfig {
        max_batch_size: 0,
        ..Default::default()
    };
    let model_path = workspace_root().join("models/late-interaction");
    let result = LateInteractionModel::new(&model_path, config);
    assert!(matches!(result, Err(EmbeddingError::ConfigError { .. })));
}

// ==================== Trait Implementation Tests ====================

#[test]
fn test_model_id() {
    let model = create_test_model();
    assert_eq!(model.model_id(), ModelId::LateInteraction);
}

#[test]
fn test_dimension_is_128() {
    let model = create_test_model();
    assert_eq!(model.dimension(), 128);
}

#[test]
fn test_projected_dimension_is_128() {
    let model = create_test_model();
    assert_eq!(model.projected_dimension(), 128);
}

#[test]
fn test_max_tokens() {
    let model = create_test_model();
    assert_eq!(model.max_tokens(), 512);
}

#[test]
fn test_latency_budget_ms() {
    let model = create_test_model();
    assert_eq!(model.latency_budget_ms(), 8);
}

#[test]
fn test_is_pretrained() {
    let model = create_test_model();
    assert!(model.is_pretrained());
}

#[test]
fn test_supported_input_types() {
    let model = create_test_model();
    let types = model.supported_input_types();
    assert!(types.contains(&InputType::Text));
    assert!(!types.contains(&InputType::Code));
    assert!(!types.contains(&InputType::Image));
}

#[test]
fn test_model_id_matches_constants() {
    assert_eq!(
        ModelId::LateInteraction.dimension(),
        LATE_INTERACTION_DIMENSION
    );
    assert_eq!(
        ModelId::LateInteraction.max_tokens(),
        LATE_INTERACTION_MAX_TOKENS
    );
    assert_eq!(
        ModelId::LateInteraction.latency_budget_ms() as u64,
        LATE_INTERACTION_LATENCY_BUDGET_MS
    );
}

// ==================== State Transition Tests ====================

#[tokio::test]
async fn test_load_sets_initialized() {
    let model = create_test_model();
    assert!(!model.is_initialized());
    model.load().await.expect("Load should succeed");
    assert!(model.is_initialized());
}

#[tokio::test]
async fn test_unload_clears_initialized() {
    let model = create_and_load_model().await;
    assert!(model.is_initialized());
    model.unload().await.expect("Unload should succeed");
    assert!(!model.is_initialized());
}

#[tokio::test]
async fn test_unload_when_not_loaded_fails() {
    let model = create_test_model();
    let result = model.unload().await;
    assert!(matches!(result, Err(EmbeddingError::NotInitialized { .. })));
}

// Serial: Multiple load/unload cycles require exclusive VRAM access
#[tokio::test]
#[serial]
async fn test_state_transitions_full_cycle() {
    let model = create_test_model();
    assert!(!model.is_initialized());
    model.load().await.unwrap();
    assert!(model.is_initialized());
    model.unload().await.unwrap();
    assert!(!model.is_initialized());
    model.load().await.unwrap();
    assert!(model.is_initialized());
}
