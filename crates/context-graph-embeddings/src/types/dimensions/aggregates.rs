//! Aggregate dimensions and compile-time validation.
//!
//! This module defines the total dimension for the 14-model Multi-Array Storage.

use super::constants::{
    CAUSAL, CODE, ENTITY, GRAPH, HDC, KEPLER, LATE_INTERACTION, MULTIMODAL, SEMANTIC, SPARSE,
    SPLADE, TEMPORAL_PERIODIC, TEMPORAL_POSITIONAL, TEMPORAL_RECENT,
};

// =============================================================================
// AGGREGATE DIMENSIONS
// =============================================================================

/// Total dimension across all 14 model embeddings (sum of projected dimensions).
///
/// Each embedding is stored SEPARATELY in Multi-Array Storage at its native dimension.
/// This constant represents the sum of all dimensions for memory allocation.
///
/// Calculated as:
/// E1:1024 + E2:512 + E3:512 + E4:512 + E5:768 + E6:1536 + E7:1536 + E8:1024 + E9:1024
/// + E10:768 + E11:384 + E12:128 + E13:1536 + Kepler:768 = 12032
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
    + SPLADE
    + KEPLER;

/// Number of models in the ensemble (13 pipeline + Kepler production E11).
pub const MODEL_COUNT: usize = 14;

// =============================================================================
// COMPILE-TIME VALIDATION
// =============================================================================

/// Compile-time assertion that TOTAL_DIMENSION equals expected value.
/// This will cause a compilation error if dimensions change incorrectly.
const _TOTAL_DIMENSION_CHECK: () =
    assert!(TOTAL_DIMENSION == 12032, "TOTAL_DIMENSION must equal 12032");

/// Compile-time assertion that MODEL_COUNT equals 14.
const _MODEL_COUNT_CHECK: () = assert!(MODEL_COUNT == 14, "MODEL_COUNT must equal 14");
