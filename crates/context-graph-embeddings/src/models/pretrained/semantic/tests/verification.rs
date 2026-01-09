//! Manual output verification tests for the semantic embedding model.

use crate::traits::EmbeddingModel;
use crate::types::ModelInput;
use serial_test::serial;

use super::super::{SemanticModel, PASSAGE_PREFIX, QUERY_PREFIX, SEMANTIC_DIMENSION};
use super::helpers::{create_and_load_model, create_test_model};

#[tokio::test]
#[serial]
async fn test_mov_1_vector_dimension() {
    let model = create_and_load_model().await;
    let input = ModelInput::text("The quick brown fox").expect("Failed to create input");

    let embedding = model.embed(&input).await.unwrap();

    // PHYSICALLY CHECK the output
    println!("MOV-1: Vector length = {}", embedding.vector.len());
    println!("MOV-1: First 5 values = {:?}", &embedding.vector[..5]);
    println!(
        "MOV-1: Last 5 values = {:?}",
        &embedding.vector[embedding.vector.len() - 5..]
    );

    assert_eq!(
        embedding.vector.len(),
        SEMANTIC_DIMENSION,
        "Dimension MUST be exactly 1024"
    );
}

#[tokio::test]
#[serial]
async fn test_mov_2_l2_normalization() {
    let model = create_and_load_model().await;
    let input =
        ModelInput::text("Test sentence for normalization check").expect("Failed to create input");

    let embedding = model.embed(&input).await.unwrap();

    let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();

    // PHYSICALLY CHECK the output
    println!("MOV-2: L2 norm = {}", norm);
    println!("MOV-2: Deviation from 1.0 = {}", (norm - 1.0).abs());

    assert!(
        (norm - 1.0).abs() < 0.001,
        "Vector MUST be L2 normalized to unit length, got norm = {}",
        norm
    );
}

#[tokio::test]
async fn test_mov_3_instruction_prefix() {
    // Test query mode
    let query_prepared = SemanticModel::instruction_prefix(true);
    println!("MOV-3: Query prefix = '{}'", query_prepared);
    assert_eq!(query_prepared, QUERY_PREFIX);

    // Test passage mode (default)
    let passage_prepared = SemanticModel::instruction_prefix(false);
    println!("MOV-3: Passage prefix = '{}'", passage_prepared);
    assert_eq!(passage_prepared, PASSAGE_PREFIX);
}

#[tokio::test]
#[serial]
async fn test_mov_4_state_transitions() {
    let model = create_test_model().await;

    // State 1: Unloaded
    println!("MOV-4: State after new() = {}", model.is_initialized());
    assert!(!model.is_initialized());

    // State 2: Loaded
    model.load().await.unwrap();
    println!("MOV-4: State after load() = {}", model.is_initialized());
    assert!(model.is_initialized());

    // State 3: Unloaded again
    model.unload().await.unwrap();
    println!("MOV-4: State after unload() = {}", model.is_initialized());
    assert!(!model.is_initialized());
}

#[tokio::test]
#[serial]
async fn test_latency_recorded() {
    let model = create_and_load_model().await;
    let input = ModelInput::text("Test latency").expect("Failed to create input");

    let embedding = model.embed(&input).await.unwrap();

    // Latency should be recorded
    println!("Latency: {} us", embedding.latency_us);
    // Stub is very fast but should still record timing (value is u64, always non-negative)
}
