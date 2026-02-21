//! Static arrays containing dimension information for all 14 models.

use super::aggregates::MODEL_COUNT;
use super::constants::{
    CAUSAL, CAUSAL_NATIVE, CODE, CODE_NATIVE, ENTITY, ENTITY_NATIVE, GRAPH, GRAPH_NATIVE, HDC,
    HDC_NATIVE, KEPLER, KEPLER_NATIVE, LATE_INTERACTION, LATE_INTERACTION_NATIVE, MULTIMODAL,
    MULTIMODAL_NATIVE, SEMANTIC, SEMANTIC_NATIVE, SPARSE, SPARSE_NATIVE, SPLADE, SPLADE_NATIVE,
    TEMPORAL_PERIODIC, TEMPORAL_PERIODIC_NATIVE, TEMPORAL_POSITIONAL, TEMPORAL_POSITIONAL_NATIVE,
    TEMPORAL_RECENT, TEMPORAL_RECENT_NATIVE,
};
use super::helpers::offset_by_index;

/// All projected dimensions in order (E1-E13 + Kepler).
pub const PROJECTED_DIMENSIONS: [usize; MODEL_COUNT] = [
    SEMANTIC,
    TEMPORAL_RECENT,
    TEMPORAL_PERIODIC,
    TEMPORAL_POSITIONAL,
    CAUSAL,
    SPARSE,
    CODE,
    GRAPH,
    HDC,
    MULTIMODAL,
    ENTITY,
    LATE_INTERACTION,
    SPLADE,
    KEPLER,
];

/// All native dimensions in order (E1-E13 + Kepler).
pub const NATIVE_DIMENSIONS: [usize; MODEL_COUNT] = [
    SEMANTIC_NATIVE,
    TEMPORAL_RECENT_NATIVE,
    TEMPORAL_PERIODIC_NATIVE,
    TEMPORAL_POSITIONAL_NATIVE,
    CAUSAL_NATIVE,
    SPARSE_NATIVE,
    CODE_NATIVE,
    GRAPH_NATIVE,
    HDC_NATIVE,
    MULTIMODAL_NATIVE,
    ENTITY_NATIVE,
    LATE_INTERACTION_NATIVE,
    SPLADE_NATIVE,
    KEPLER_NATIVE,
];

/// All offsets for each model in order (E1-E13 + Kepler).
pub const OFFSETS: [usize; MODEL_COUNT] = [
    offset_by_index(0),
    offset_by_index(1),
    offset_by_index(2),
    offset_by_index(3),
    offset_by_index(4),
    offset_by_index(5),
    offset_by_index(6),
    offset_by_index(7),
    offset_by_index(8),
    offset_by_index(9),
    offset_by_index(10),
    offset_by_index(11),
    offset_by_index(12),
    offset_by_index(13),
];
