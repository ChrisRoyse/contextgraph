//! Aggregate dimension tests for the embedder system.
//!
//! Tests that verify TOTAL_DIMENSION, MODEL_COUNT, and sum consistency.

use context_graph_embeddings::dimensions::{
    CAUSAL, CODE, ENTITY, GRAPH, HDC, LATE_INTERACTION, MODEL_COUNT, MULTIMODAL, SEMANTIC, SPARSE,
    SPLADE, TEMPORAL_PERIODIC, TEMPORAL_POSITIONAL, TEMPORAL_RECENT, TOTAL_DIMENSION,
};
use context_graph_embeddings::ModelId;

use super::constants::{EXPECTED_MODEL_COUNT, EXPECTED_TOTAL_DIMENSION};

/// Test TOTAL_DIMENSION constant equals expected value.
#[test]
fn test_total_dimension_constant() {
    assert_eq!(
        TOTAL_DIMENSION, EXPECTED_TOTAL_DIMENSION,
        "TOTAL_DIMENSION should be {} but is {}",
        EXPECTED_TOTAL_DIMENSION, TOTAL_DIMENSION
    );
    println!("[PASS] TOTAL_DIMENSION = {} verified", TOTAL_DIMENSION);
}

/// Test MODEL_COUNT constant equals expected value.
#[test]
fn test_model_count_constant() {
    assert_eq!(
        MODEL_COUNT, EXPECTED_MODEL_COUNT,
        "MODEL_COUNT should be {} but is {}",
        EXPECTED_MODEL_COUNT, MODEL_COUNT
    );
    println!("[PASS] MODEL_COUNT = {} verified", MODEL_COUNT);
}

/// Test ModelId::all() returns exactly 13 models.
#[test]
fn test_model_id_all_count() {
    let all_models = ModelId::all();
    assert_eq!(
        all_models.len(),
        EXPECTED_MODEL_COUNT,
        "ModelId::all() should return {} models, got {}",
        EXPECTED_MODEL_COUNT,
        all_models.len()
    );
    println!("[PASS] ModelId::all() returns 13 models");
}

/// Test manual sum of projected dimensions equals TOTAL_DIMENSION.
#[test]
fn test_manual_sum_equals_total() {
    let manual_sum = SEMANTIC
        + TEMPORAL_RECENT
        + TEMPORAL_PERIODIC
        + TEMPORAL_POSITIONAL
        + CAUSAL
        + SPARSE
        + CODE
        + GRAPH
        + HDC
        + MULTIMODAL
        + ENTITY
        + LATE_INTERACTION
        + SPLADE;

    assert_eq!(
        manual_sum, TOTAL_DIMENSION,
        "Manual sum ({}) != TOTAL_DIMENSION ({})",
        manual_sum, TOTAL_DIMENSION
    );
    println!("[PASS] Manual sum of all projected dimensions = TOTAL_DIMENSION");
}

/// Test sum from ModelId iteration equals TOTAL_DIMENSION.
#[test]
fn test_model_id_iteration_sum() {
    let sum: usize = ModelId::all().iter().map(|m| m.projected_dimension()).sum();

    assert_eq!(
        sum, TOTAL_DIMENSION,
        "Sum from ModelId::all().projected_dimension() ({}) != TOTAL_DIMENSION ({})",
        sum, TOTAL_DIMENSION
    );
    println!("[PASS] Sum from ModelId iteration = TOTAL_DIMENSION");
}

/// Test TOTAL_DIMENSION breakdown matches documented calculation.
#[test]
fn test_total_dimension_breakdown() {
    // Constitution: E1-E13 projected dimensions
    // E8 upgraded 384→1024 (e5-large-v2); E11 remains legacy MiniLM 384D
    // E1:1024 + E2:512 + E3:512 + E4:512 + E5:768 + E6:1536 + E7:1536 + E8:1024 + E9:1024 + E10:768 + E11:384 + E12:128 + E13:1536 = 11264
    let expected_breakdown =
        1024 + 512 + 512 + 512 + 768 + 1536 + 1536 + 1024 + 1024 + 768 + 384 + 128 + 1536;
    assert_eq!(
        expected_breakdown, EXPECTED_TOTAL_DIMENSION,
        "Documented breakdown sum {} != expected {}",
        expected_breakdown, EXPECTED_TOTAL_DIMENSION
    );
    assert_eq!(
        TOTAL_DIMENSION, expected_breakdown,
        "TOTAL_DIMENSION {} != documented breakdown {}",
        TOTAL_DIMENSION, expected_breakdown
    );
    println!("[PASS] TOTAL_DIMENSION breakdown calculation verified");
}
