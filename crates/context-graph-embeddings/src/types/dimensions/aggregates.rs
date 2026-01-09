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
/// 1024 + 512 + 512 + 512 + 768 + 1536 + 1536 + 384 + 1024 + 768 + 384 + 128 + 1536 = 10624
/// (E7 Code changed from 768 to 1536 for Qodo-Embed migration)
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
    assert!(TOTAL_DIMENSION == 10624, "TOTAL_DIMENSION must equal 10624");

/// Compile-time assertion that MODEL_COUNT equals 13.
const _MODEL_COUNT_CHECK: () = assert!(MODEL_COUNT == 13, "MODEL_COUNT must equal 13");
