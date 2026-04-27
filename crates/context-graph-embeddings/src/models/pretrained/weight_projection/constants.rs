//! Constants for the learned weight projection model.

pub use context_graph_core::clustering::{
    MAX_WEIGHTED_AGREEMENT, TOPIC_THRESHOLD as WEIGHTED_AGREEMENT_THRESHOLD,
};

/// Number of embedders in the system.
pub const NUM_EMBEDDERS: usize = 14;

/// Hidden dimension of the first layer.
pub const HIDDEN_DIM_1: usize = 64;

/// Hidden dimension of the second layer.
pub const HIDDEN_DIM_2: usize = 32;

/// Output dimension (edge weight).
pub const OUTPUT_DIM: usize = 1;

/// Default model path relative to models directory.
pub const DEFAULT_WEIGHTS_PATH: &str = "models/graph_weights/weights.safetensors";

/// Category weights from constitution (used as initialization).
/// SEMANTIC: E1, E5, E6, E7, E10, E12, E13 (weight=1.0)
/// RELATIONAL: E8, E11 (weight=0.5)
/// STRUCTURAL: E9 (weight=0.5)
/// TEMPORAL: E2, E3, E4 (weight=0.0, excluded per AP-60)
pub const DEFAULT_CATEGORY_WEIGHTS: [f32; NUM_EMBEDDERS] = [
    1.0, // E1 Semantic
    0.0, // E2 Temporal (excluded per AP-60)
    0.0, // E3 Temporal (excluded per AP-60)
    0.0, // E4 Temporal (excluded per AP-60)
    1.0, // E5 Causal (semantic)
    1.0, // E6 Sparse (semantic)
    1.0, // E7 Code (semantic)
    0.5, // E8 Graph (relational)
    0.5, // E9 Robustness (structural)
    1.0, // E10 Paraphrase (semantic)
    0.5, // E11 Entity (relational)
    1.0, // E12 Late Interaction (semantic)
    1.0, // E13 SPLADE (semantic)
    1.0, // E14 BGE-M3 Dense (semantic)
];
