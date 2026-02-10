//! Native dimension tests for all 13 embedder models.
//!
//! Tests that each model's native (raw) dimension matches the Constitution.

use context_graph_embeddings::{
    dimensions::{
        native_dimension_by_index, CAUSAL_NATIVE, CODE_NATIVE, ENTITY_NATIVE, GRAPH_NATIVE,
        HDC_NATIVE, LATE_INTERACTION_NATIVE, MULTIMODAL_NATIVE, NATIVE_DIMENSIONS, SEMANTIC_NATIVE,
        SPARSE_NATIVE, SPLADE_NATIVE, TEMPORAL_PERIODIC_NATIVE, TEMPORAL_POSITIONAL_NATIVE,
        TEMPORAL_RECENT_NATIVE,
    },
    ModelId,
};

use super::constants::EXPECTED_NATIVE_DIMS;

/// Test E1 Semantic native dimension matches Constitution.
#[test]
fn test_e1_semantic_native_dimension() {
    assert_eq!(
        ModelId::Semantic.dimension(),
        1024,
        "E1 Semantic: expected native dimension 1024"
    );
    assert_eq!(SEMANTIC_NATIVE, 1024, "SEMANTIC_NATIVE constant mismatch");
}

/// Test E2 TemporalRecent native dimension matches Constitution.
#[test]
fn test_e2_temporal_recent_native_dimension() {
    assert_eq!(
        ModelId::TemporalRecent.dimension(),
        512,
        "E2 TemporalRecent: expected native dimension 512"
    );
    assert_eq!(
        TEMPORAL_RECENT_NATIVE, 512,
        "TEMPORAL_RECENT_NATIVE constant mismatch"
    );
}

/// Test E3 TemporalPeriodic native dimension matches Constitution.
#[test]
fn test_e3_temporal_periodic_native_dimension() {
    assert_eq!(
        ModelId::TemporalPeriodic.dimension(),
        512,
        "E3 TemporalPeriodic: expected native dimension 512"
    );
    assert_eq!(
        TEMPORAL_PERIODIC_NATIVE, 512,
        "TEMPORAL_PERIODIC_NATIVE constant mismatch"
    );
}

/// Test E4 TemporalPositional native dimension matches Constitution.
#[test]
fn test_e4_temporal_positional_native_dimension() {
    assert_eq!(
        ModelId::TemporalPositional.dimension(),
        512,
        "E4 TemporalPositional: expected native dimension 512"
    );
    assert_eq!(
        TEMPORAL_POSITIONAL_NATIVE, 512,
        "TEMPORAL_POSITIONAL_NATIVE constant mismatch"
    );
}

/// Test E5 Causal native dimension matches Constitution.
#[test]
fn test_e5_causal_native_dimension() {
    assert_eq!(
        ModelId::Causal.dimension(),
        768,
        "E5 Causal: expected native dimension 768"
    );
    assert_eq!(CAUSAL_NATIVE, 768, "CAUSAL_NATIVE constant mismatch");
}

/// Test E6 Sparse native dimension matches Constitution.
#[test]
fn test_e6_sparse_native_dimension() {
    assert_eq!(
        ModelId::Sparse.dimension(),
        30522,
        "E6 Sparse: expected native dimension 30522 (SPLADE vocab)"
    );
    assert_eq!(SPARSE_NATIVE, 30522, "SPARSE_NATIVE constant mismatch");
}

/// Test E7 Code native dimension matches Constitution.
#[test]
fn test_e7_code_native_dimension() {
    assert_eq!(
        ModelId::Code.dimension(),
        1536,
        "E7 Code: expected native dimension 1536 (Qodo-Embed-1-1.5B)"
    );
    assert_eq!(CODE_NATIVE, 1536, "CODE_NATIVE constant mismatch");
}

/// Test E8 Graph native dimension matches Constitution.
#[test]
fn test_e8_graph_native_dimension() {
    assert_eq!(
        ModelId::Graph.dimension(),
        1024,
        "E8 Graph: expected native dimension 1024 (e5-large-v2, upgraded from MiniLM 384D)"
    );
    assert_eq!(GRAPH_NATIVE, 1024, "GRAPH_NATIVE constant mismatch");
}

/// Test E9 Hdc native dimension matches Constitution.
#[test]
fn test_e9_hdc_native_dimension() {
    assert_eq!(
        ModelId::Hdc.dimension(),
        10000,
        "E9 Hdc: expected native dimension 10000 (10K-bit)"
    );
    assert_eq!(HDC_NATIVE, 10000, "HDC_NATIVE constant mismatch");
}

/// Test E10 Multimodal native dimension matches Constitution.
#[test]
fn test_e10_multimodal_native_dimension() {
    assert_eq!(
        ModelId::Multimodal.dimension(),
        768,
        "E10 Multimodal: expected native dimension 768 (CLIP)"
    );
    assert_eq!(
        MULTIMODAL_NATIVE, 768,
        "MULTIMODAL_NATIVE constant mismatch"
    );
}

/// Test E11 Entity native dimension matches Constitution.
#[test]
fn test_e11_entity_native_dimension() {
    assert_eq!(
        ModelId::Entity.dimension(),
        384,
        "E11 Entity: expected native dimension 384 (legacy MiniLM; production uses Kepler 768D)"
    );
    assert_eq!(ENTITY_NATIVE, 384, "ENTITY_NATIVE constant mismatch");
}

/// Test E12 LateInteraction native dimension matches Constitution.
#[test]
fn test_e12_late_interaction_native_dimension() {
    assert_eq!(
        ModelId::LateInteraction.dimension(),
        128,
        "E12 LateInteraction: expected native dimension 128 (per token)"
    );
    assert_eq!(
        LATE_INTERACTION_NATIVE, 128,
        "LATE_INTERACTION_NATIVE constant mismatch"
    );
}

/// Test E13 Splade native dimension matches Constitution.
#[test]
fn test_e13_splade_native_dimension() {
    assert_eq!(
        ModelId::Splade.dimension(),
        30522,
        "E13 Splade: expected native dimension 30522"
    );
    assert_eq!(SPLADE_NATIVE, 30522, "SPLADE_NATIVE constant mismatch");
}

/// Test ALL native dimensions match expected values in one sweep.
/// FAIL FAST: Any single mismatch panics with details.
#[test]
fn test_all_native_dimensions_match_constitution() {
    for (model_id, expected_dim) in &EXPECTED_NATIVE_DIMS {
        let actual_dim = model_id.dimension();
        assert_eq!(
            actual_dim, *expected_dim,
            "Native dimension mismatch for {:?}: expected {}, got {}",
            model_id, expected_dim, actual_dim
        );
    }
    println!("[PASS] All 13 native dimensions match Constitution");
}

/// Test NATIVE_DIMENSIONS array matches ModelId::dimension() for all models.
#[test]
fn test_native_dimensions_array_consistency() {
    for (i, &expected) in NATIVE_DIMENSIONS.iter().enumerate() {
        let by_index = native_dimension_by_index(i);
        assert_eq!(
            by_index, expected,
            "NATIVE_DIMENSIONS[{}] ({}) != native_dimension_by_index({}) ({})",
            i, expected, i, by_index
        );
    }
    println!("[PASS] NATIVE_DIMENSIONS array consistent with helper function");
}
