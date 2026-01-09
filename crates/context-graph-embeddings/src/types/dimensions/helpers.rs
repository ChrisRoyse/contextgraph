//! Helper functions for dimension lookups and offset calculations.

use super::constants::{
    CAUSAL, CAUSAL_NATIVE, CODE, CODE_NATIVE, ENTITY, ENTITY_NATIVE, GRAPH, GRAPH_NATIVE, HDC,
    HDC_NATIVE, LATE_INTERACTION, LATE_INTERACTION_NATIVE, MULTIMODAL, MULTIMODAL_NATIVE, SEMANTIC,
    SEMANTIC_NATIVE, SPARSE, SPARSE_NATIVE, SPLADE, SPLADE_NATIVE, TEMPORAL_PERIODIC,
    TEMPORAL_PERIODIC_NATIVE, TEMPORAL_POSITIONAL, TEMPORAL_POSITIONAL_NATIVE, TEMPORAL_RECENT,
    TEMPORAL_RECENT_NATIVE,
};

/// Returns the projected dimension for a given model index (0-12).
///
/// # Panics
/// Panics if index >= 13.
///
/// # Example
/// ```rust
/// use context_graph_embeddings::types::dimensions;
///
/// assert_eq!(dimensions::projected_dimension_by_index(0), 1024); // Semantic
/// assert_eq!(dimensions::projected_dimension_by_index(5), 1536); // Sparse
/// ```
#[must_use]
pub const fn projected_dimension_by_index(index: usize) -> usize {
    match index {
        0 => SEMANTIC,
        1 => TEMPORAL_RECENT,
        2 => TEMPORAL_PERIODIC,
        3 => TEMPORAL_POSITIONAL,
        4 => CAUSAL,
        5 => SPARSE,
        6 => CODE,
        7 => GRAPH,
        8 => HDC,
        9 => MULTIMODAL,
        10 => ENTITY,
        11 => LATE_INTERACTION,
        12 => SPLADE,
        _ => panic!("Invalid model index: must be 0-12"),
    }
}

/// Returns the native dimension for a given model index (0-12).
///
/// # Panics
/// Panics if index >= 13.
#[must_use]
pub const fn native_dimension_by_index(index: usize) -> usize {
    match index {
        0 => SEMANTIC_NATIVE,
        1 => TEMPORAL_RECENT_NATIVE,
        2 => TEMPORAL_PERIODIC_NATIVE,
        3 => TEMPORAL_POSITIONAL_NATIVE,
        4 => CAUSAL_NATIVE,
        5 => SPARSE_NATIVE,
        6 => CODE_NATIVE,
        7 => GRAPH_NATIVE,
        8 => HDC_NATIVE,
        9 => MULTIMODAL_NATIVE,
        10 => ENTITY_NATIVE,
        11 => LATE_INTERACTION_NATIVE,
        12 => SPLADE_NATIVE,
        _ => panic!("Invalid model index: must be 0-12"),
    }
}

/// Returns the byte offset for model at index within the total dimension space.
///
/// Used for memory calculations and index offsets (9856D total).
///
/// # Example
/// ```rust
/// use context_graph_embeddings::types::dimensions;
///
/// // Semantic (E1) starts at offset 0
/// assert_eq!(dimensions::offset_by_index(0), 0);
///
/// // TemporalRecent (E2) starts after Semantic's 1024 elements
/// assert_eq!(dimensions::offset_by_index(1), 1024);
///
/// // Causal (E5) starts after Semantic + 3x Temporal
/// assert_eq!(dimensions::offset_by_index(4), 1024 + 512 + 512 + 512);
/// ```
#[must_use]
pub const fn offset_by_index(index: usize) -> usize {
    match index {
        0 => 0,
        1 => SEMANTIC,
        2 => SEMANTIC + TEMPORAL_RECENT,
        3 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC,
        4 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC + TEMPORAL_POSITIONAL,
        5 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC + TEMPORAL_POSITIONAL + CAUSAL,
        6 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC + TEMPORAL_POSITIONAL + CAUSAL + SPARSE,
        7 => {
            SEMANTIC
                + TEMPORAL_RECENT
                + TEMPORAL_PERIODIC
                + TEMPORAL_POSITIONAL
                + CAUSAL
                + SPARSE
                + CODE
        }
        8 => {
            SEMANTIC
                + TEMPORAL_RECENT
                + TEMPORAL_PERIODIC
                + TEMPORAL_POSITIONAL
                + CAUSAL
                + SPARSE
                + CODE
                + GRAPH
        }
        9 => {
            SEMANTIC
                + TEMPORAL_RECENT
                + TEMPORAL_PERIODIC
                + TEMPORAL_POSITIONAL
                + CAUSAL
                + SPARSE
                + CODE
                + GRAPH
                + HDC
        }
        10 => {
            SEMANTIC
                + TEMPORAL_RECENT
                + TEMPORAL_PERIODIC
                + TEMPORAL_POSITIONAL
                + CAUSAL
                + SPARSE
                + CODE
                + GRAPH
                + HDC
                + MULTIMODAL
        }
        11 => {
            SEMANTIC
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
        }
        12 => {
            SEMANTIC
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
        }
        _ => panic!("Invalid model index: must be 0-12"),
    }
}
