//! Projected dimension tests for all 13 embedder models.
//!
//! Tests that each model's projected dimension (used for Multi-Array Storage)
//! matches the Constitution.

use context_graph_embeddings::{
    dimensions::{
        projected_dimension_by_index, CAUSAL, CODE, CODE_NATIVE, ENTITY, GRAPH, HDC, HDC_NATIVE,
        LATE_INTERACTION, MULTIMODAL, PROJECTED_DIMENSIONS, SEMANTIC, SPARSE, SPARSE_NATIVE,
        SPLADE, SPLADE_NATIVE, TEMPORAL_PERIODIC, TEMPORAL_POSITIONAL, TEMPORAL_RECENT,
        TOTAL_DIMENSION,
    },
    ModelId,
};

use super::constants::EXPECTED_PROJECTED_DIMS;

/// Test E1 Semantic projected dimension (no projection needed).
#[test]
fn test_e1_semantic_projected_dimension() {
    assert_eq!(
        ModelId::Semantic.projected_dimension(),
        1024,
        "E1 Semantic: expected projected dimension 1024"
    );
    assert_eq!(SEMANTIC, 1024, "SEMANTIC constant mismatch");
}

/// Test E2 TemporalRecent projected dimension (no projection needed).
#[test]
fn test_e2_temporal_recent_projected_dimension() {
    assert_eq!(
        ModelId::TemporalRecent.projected_dimension(),
        512,
        "E2 TemporalRecent: expected projected dimension 512"
    );
    assert_eq!(TEMPORAL_RECENT, 512, "TEMPORAL_RECENT constant mismatch");
}

/// Test E3 TemporalPeriodic projected dimension (no projection needed).
#[test]
fn test_e3_temporal_periodic_projected_dimension() {
    assert_eq!(
        ModelId::TemporalPeriodic.projected_dimension(),
        512,
        "E3 TemporalPeriodic: expected projected dimension 512"
    );
    assert_eq!(
        TEMPORAL_PERIODIC, 512,
        "TEMPORAL_PERIODIC constant mismatch"
    );
}

/// Test E4 TemporalPositional projected dimension (no projection needed).
#[test]
fn test_e4_temporal_positional_projected_dimension() {
    assert_eq!(
        ModelId::TemporalPositional.projected_dimension(),
        512,
        "E4 TemporalPositional: expected projected dimension 512"
    );
    assert_eq!(
        TEMPORAL_POSITIONAL, 512,
        "TEMPORAL_POSITIONAL constant mismatch"
    );
}

/// Test E5 Causal projected dimension (no projection needed).
#[test]
fn test_e5_causal_projected_dimension() {
    assert_eq!(
        ModelId::Causal.projected_dimension(),
        768,
        "E5 Causal: expected projected dimension 768"
    );
    assert_eq!(CAUSAL, 768, "CAUSAL constant mismatch");
}

/// Test E6 Sparse projected dimension (30K -> 1536 projection).
#[test]
fn test_e6_sparse_projected_dimension() {
    assert_eq!(
        ModelId::Sparse.projected_dimension(),
        1536,
        "E6 Sparse: expected projected dimension 1536 (from 30522)"
    );
    assert_eq!(SPARSE, 1536, "SPARSE constant mismatch");
    // Verify compression ratio
    let ratio = SPARSE_NATIVE as f64 / SPARSE as f64;
    assert!(
        ratio > 19.0 && ratio < 20.0,
        "E6 Sparse projection ratio ~19.8x expected, got {}",
        ratio
    );
}

/// Test E7 Code projected dimension (1536D native, no projection needed).
#[test]
fn test_e7_code_projected_dimension() {
    assert_eq!(
        ModelId::Code.projected_dimension(),
        1536,
        "E7 Code: expected projected dimension 1536 (Qodo-Embed native)"
    );
    assert_eq!(CODE, 1536, "CODE constant mismatch");
    // Verify no expansion needed (1:1 ratio)
    assert_eq!(
        CODE, CODE_NATIVE,
        "E7 Code should have no projection (native 1536D)"
    );
}

/// Test E8 Graph projected dimension (no projection needed).
#[test]
fn test_e8_graph_projected_dimension() {
    assert_eq!(
        ModelId::Graph.projected_dimension(),
        1024,
        "E8 Graph: expected projected dimension 1024 (e5-large-v2, upgraded from MiniLM 384D)"
    );
    assert_eq!(GRAPH, 1024, "GRAPH constant mismatch");
}

/// Test E9 Hdc projected dimension (10K -> 1024 projection).
#[test]
fn test_e9_hdc_projected_dimension() {
    assert_eq!(
        ModelId::Hdc.projected_dimension(),
        1024,
        "E9 Hdc: expected projected dimension 1024 (from 10000)"
    );
    assert_eq!(HDC, 1024, "HDC constant mismatch");
    // Verify compression ratio
    let ratio = HDC_NATIVE as f64 / HDC as f64;
    assert!(
        ratio > 9.0 && ratio < 10.0,
        "E9 Hdc projection ratio ~9.77x expected, got {}",
        ratio
    );
}

/// Test E10 Multimodal projected dimension (no projection needed).
#[test]
fn test_e10_multimodal_projected_dimension() {
    assert_eq!(
        ModelId::Multimodal.projected_dimension(),
        768,
        "E10 Multimodal: expected projected dimension 768"
    );
    assert_eq!(MULTIMODAL, 768, "MULTIMODAL constant mismatch");
}

/// Test E11 Entity projected dimension (no projection needed).
#[test]
fn test_e11_entity_projected_dimension() {
    assert_eq!(
        ModelId::Entity.projected_dimension(),
        384,
        "E11 Entity: expected projected dimension 384 (legacy MiniLM; production uses Kepler 768D)"
    );
    assert_eq!(ENTITY, 384, "ENTITY constant mismatch");
}

/// Test E12 LateInteraction projected dimension (pooled to single vector).
#[test]
fn test_e12_late_interaction_projected_dimension() {
    assert_eq!(
        ModelId::LateInteraction.projected_dimension(),
        128,
        "E12 LateInteraction: expected projected dimension 128"
    );
    assert_eq!(LATE_INTERACTION, 128, "LATE_INTERACTION constant mismatch");
}

/// Test E13 Splade projected dimension (30K -> 1536 projection).
#[test]
fn test_e13_splade_projected_dimension() {
    assert_eq!(
        ModelId::Splade.projected_dimension(),
        1536,
        "E13 Splade: expected projected dimension 1536"
    );
    assert_eq!(SPLADE, 1536, "SPLADE projected constant mismatch");
    // Verify compression ratio
    let ratio = SPLADE_NATIVE as f64 / SPLADE as f64;
    assert!(
        ratio > 19.0 && ratio < 20.0,
        "E13 Splade projection ratio ~19.8x expected, got {}",
        ratio
    );
}

/// Test ALL projected dimensions match expected values in one sweep.
/// FAIL FAST: Any single mismatch panics with details.
#[test]
fn test_all_projected_dimensions_match_constitution() {
    for (model_id, expected_dim) in &EXPECTED_PROJECTED_DIMS {
        let actual_dim = model_id.projected_dimension();
        assert_eq!(
            actual_dim, *expected_dim,
            "Projected dimension mismatch for {:?}: expected {}, got {}",
            model_id, expected_dim, actual_dim
        );
    }
    println!("[PASS] All 13 projected dimensions match Constitution");
}

/// Test PROJECTED_DIMENSIONS array consistency.
#[test]
fn test_projected_dimensions_array_consistency() {
    for (i, &expected) in PROJECTED_DIMENSIONS.iter().enumerate() {
        let by_index = projected_dimension_by_index(i);
        assert_eq!(
            by_index, expected,
            "PROJECTED_DIMENSIONS[{}] ({}) != projected_dimension_by_index({}) ({})",
            i, expected, i, by_index
        );
    }
    println!("[PASS] PROJECTED_DIMENSIONS array consistent with helper function");
}

/// Test PROJECTED_DIMENSIONS array sum equals TOTAL_DIMENSION.
#[test]
fn test_projected_dimensions_array_sum() {
    let sum: usize = PROJECTED_DIMENSIONS.iter().sum();
    assert_eq!(
        sum, TOTAL_DIMENSION,
        "Sum of PROJECTED_DIMENSIONS ({}) != TOTAL_DIMENSION ({})",
        sum, TOTAL_DIMENSION
    );
    println!("[PASS] PROJECTED_DIMENSIONS sum equals TOTAL_DIMENSION");
}
