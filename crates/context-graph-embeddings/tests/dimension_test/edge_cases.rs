//! Edge case tests for the embedder dimension system.
//!
//! Tests invalid inputs, boundary values, and error conditions.

use context_graph_embeddings::dimensions::{
    native_dimension_by_index, offset_by_index, projected_dimension_by_index, MODEL_COUNT,
    NATIVE_DIMENSIONS, OFFSETS, PROJECTED_DIMENSIONS,
};
use context_graph_embeddings::ModelId;

/// Test invalid index for projected_dimension_by_index panics.
#[test]
#[should_panic(expected = "Invalid model index")]
fn test_projected_dimension_invalid_index_panics() {
    let _ = projected_dimension_by_index(14);
}

/// Test invalid index for native_dimension_by_index panics.
#[test]
#[should_panic(expected = "Invalid model index")]
fn test_native_dimension_invalid_index_panics() {
    let _ = native_dimension_by_index(14);
}

/// Test invalid index for offset_by_index panics.
#[test]
#[should_panic(expected = "Invalid model index")]
fn test_offset_invalid_index_panics() {
    let _ = offset_by_index(14);
}

/// Test large invalid index for projected_dimension_by_index panics.
#[test]
#[should_panic(expected = "Invalid model index")]
fn test_projected_dimension_large_index_panics() {
    let _ = projected_dimension_by_index(255);
}

/// Test no model has zero dimension.
#[test]
fn test_no_zero_dimensions() {
    for model_id in ModelId::all() {
        assert!(
            model_id.dimension() > 0,
            "{:?} has zero native dimension",
            model_id
        );
        assert!(
            model_id.projected_dimension() > 0,
            "{:?} has zero projected dimension",
            model_id
        );
    }
    println!("[PASS] No model has zero dimension");
}

/// Test projected >= native for models requiring expansion (E7 Code only).
#[test]
fn test_projection_directions() {
    for model_id in ModelId::all() {
        let native = model_id.dimension();
        let projected = model_id.projected_dimension();

        match model_id {
            // E7 Code: no projection needed (native 1536D from Qodo-Embed)
            ModelId::Code => {
                assert_eq!(
                    projected, native,
                    "E7 Code should have no projection: {} == {} expected",
                    projected, native
                );
            }
            // E6, E9, E13: compression
            ModelId::Sparse | ModelId::Hdc | ModelId::Splade => {
                assert!(
                    projected < native,
                    "{:?} should compress: {} < {} expected",
                    model_id,
                    projected,
                    native
                );
            }
            // All others: no projection
            _ => {
                assert_eq!(
                    projected, native,
                    "{:?} should have no projection: {} == {} expected",
                    model_id, projected, native
                );
            }
        }
    }
    println!("[PASS] Projection directions verified for all models");
}

/// Test boundary values at dimension limits.
#[test]
fn test_dimension_boundary_values() {
    // Minimum dimension is 128 (E12 LateInteraction)
    let min_dim = ModelId::all()
        .iter()
        .map(|m| m.projected_dimension())
        .min()
        .unwrap();
    assert_eq!(min_dim, 128, "Minimum projected dimension should be 128");

    // Maximum native dimension is 30522 (E6 Sparse, E13 Splade)
    let max_native = ModelId::all().iter().map(|m| m.dimension()).max().unwrap();
    assert_eq!(
        max_native, 30522,
        "Maximum native dimension should be 30522"
    );

    // Maximum projected dimension is 1536 (E6 Sparse, E13 Splade)
    let max_projected = ModelId::all()
        .iter()
        .map(|m| m.projected_dimension())
        .max()
        .unwrap();
    assert_eq!(
        max_projected, 1536,
        "Maximum projected dimension should be 1536"
    );

    println!("[PASS] Dimension boundary values verified");
}

/// Test arrays have correct length.
#[test]
fn test_array_lengths() {
    assert_eq!(
        NATIVE_DIMENSIONS.len(),
        MODEL_COUNT,
        "NATIVE_DIMENSIONS length mismatch"
    );
    assert_eq!(
        PROJECTED_DIMENSIONS.len(),
        MODEL_COUNT,
        "PROJECTED_DIMENSIONS length mismatch"
    );
    assert_eq!(OFFSETS.len(), MODEL_COUNT, "OFFSETS length mismatch");
    assert_eq!(
        ModelId::all().len(),
        MODEL_COUNT,
        "ModelId::all() length mismatch"
    );
    println!("[PASS] All arrays have length {}", MODEL_COUNT);
}
