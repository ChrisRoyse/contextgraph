//! Aggregate dimensions and compile-time validation.
//!
//! This module defines the total dimension for the 13-model Multi-Array Storage.

use super::constants::{
    CAUSAL, CODE, ENTITY, GRAPH, HDC, LATE_INTERACTION, MULTIMODAL, SEMANTIC, SPARSE, SPLADE,
    TEMPORAL_PERIODIC, TEMPORAL_POSITIONAL, TEMPORAL_RECENT,
};

// =============================================================================
// AGGREGATE DIMENSIONS
// =============================================================================

/// Total dimension across all 13 model embeddings (sum of projected dimensions).
///
/// Each embedding is stored SEPARATELY in Multi-Array Storage at its native dimension.
/// This constant represents the sum of all dimensions for memory allocation.
///
/// Calculated as:
/// E1:1024 + E2:512 + E3:512 + E4:512 + E5:768 + E6:1536 + E7:1536 + E8:1024 + E9:1024 + E10:768 + E11:384 + E12:128 + E13:1536 = 11264
/// (E8 upgraded from MiniLM 384D to e5-large-v2 1024D; E11 remains legacy MiniLM 384D, production E11 uses Kepler 768D)
pub const TOTAL_DIMENSION: usize = SEMANTIC
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

/// Number of models in the ensemble.
pub const MODEL_COUNT: usize = 13;

// =============================================================================
// COMPILE-TIME VALIDATION
// =============================================================================

/// Compile-time assertion that TOTAL_DIMENSION equals expected value.
/// This will cause a compilation error if dimensions change incorrectly.
const _TOTAL_DIMENSION_CHECK: () =
    assert!(TOTAL_DIMENSION == 11264, "TOTAL_DIMENSION must equal 11264");

/// Compile-time assertion that MODEL_COUNT equals 13.
const _MODEL_COUNT_CHECK: () = assert!(MODEL_COUNT == 13, "MODEL_COUNT must equal 13");
