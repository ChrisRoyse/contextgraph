//! Pretrained embedding models for the 13-model ensemble (E1-E13).
//!
//! This module contains implementations for models that require
//! loading pretrained weights from HuggingFace repositories.
//!
//! # Feature Flags
//!
//! Models require the `candle` feature for actual inference:
//! ```toml
//! context-graph-embeddings = { version = "0.1", features = ["candle"] }
//! ```
//!
//! Without the feature, models use stub implementations for testing.

mod causal;
mod code;
mod contextual;
mod entity;
mod graph;
pub mod kepler;
mod late_interaction;
mod multimodal;
mod semantic;
mod sparse;

pub use causal::{
    CausalModel, CAUSAL_DIMENSION, CAUSAL_LATENCY_BUDGET_MS, CAUSAL_MAX_TOKENS,
    DEFAULT_ATTENTION_WINDOW,
};
pub use contextual::{
    context_pooling_weights, detect_context_markers, intent_pooling_weights,
    ContextMarkerResult, ContextType, ContextualModel,
    CONTEXTUAL_DIMENSION, CONTEXTUAL_HIDDEN_SIZE, CONTEXTUAL_INTERMEDIATE_SIZE,
    CONTEXTUAL_LATENCY_BUDGET_MS, CONTEXTUAL_LAYER_NORM_EPS, CONTEXTUAL_MAX_TOKENS,
    CONTEXTUAL_MODEL_NAME, CONTEXTUAL_NUM_HEADS, CONTEXTUAL_NUM_LAYERS, CONTEXTUAL_VOCAB_SIZE,
    // E5-base-v2 prefix constants
    INTENT_PREFIX, CONTEXT_PREFIX,
};
// Deprecated re-exports for backwards compatibility
#[allow(deprecated)]
pub use contextual::{ContextProjectionWeights, CONTEXT_PROJECTION_SEED};
pub use code::{
    CodeModel, CODE_LATENCY_BUDGET_MS, CODE_MAX_TOKENS, CODE_MODEL_NAME, CODE_NATIVE_DIMENSION,
    CODE_PROJECTED_DIMENSION,
};
pub use entity::{
    EntityModel, ENTITY_DIMENSION, ENTITY_LATENCY_BUDGET_MS, ENTITY_MAX_TOKENS, ENTITY_MODEL_NAME,
};
pub use kepler::{
    KeplerModel, KEPLER_DIMENSION, KEPLER_LATENCY_BUDGET_MS, KEPLER_MAX_TOKENS, KEPLER_MODEL_NAME,
};
pub use graph::{
    GraphModel, GRAPH_DIMENSION, GRAPH_LATENCY_BUDGET_MS, GRAPH_MAX_TOKENS, GRAPH_MODEL_NAME,
    MAX_CONTEXT_NEIGHBORS,
};
pub use late_interaction::{
    LateInteractionModel, TokenEmbeddings, LATE_INTERACTION_DIMENSION,
    LATE_INTERACTION_LATENCY_BUDGET_MS, LATE_INTERACTION_MAX_TOKENS, LATE_INTERACTION_MODEL_NAME,
};
pub use multimodal::{
    ClipTextAttentionWeights, ClipTextConfig, ClipTextLayerWeights, ClipTextMlpWeights,
    ClipTextWeights, ClipVisionAttentionWeights, ClipVisionConfig, ClipVisionLayerWeights,
    ClipVisionMlpWeights, ClipVisionWeights, ClipWeights, ImageProcessor, MultimodalModel,
    CLIP_IMAGE_SIZE, CLIP_MEAN, CLIP_NUM_PATCHES, CLIP_NUM_PATCHES_PER_DIM, CLIP_PATCH_SIZE,
    CLIP_STD, MULTIMODAL_DIMENSION, MULTIMODAL_LATENCY_BUDGET_MS, MULTIMODAL_MAX_TOKENS,
    MULTIMODAL_MODEL_NAME,
};
pub use semantic::{
    SemanticModel, PASSAGE_PREFIX, QUERY_PREFIX, SEMANTIC_DIMENSION, SEMANTIC_LATENCY_BUDGET_MS,
    SEMANTIC_MAX_TOKENS,
};
pub use sparse::{
    SparseModel, SparseVector, SPARSE_EXPECTED_SPARSITY, SPARSE_LATENCY_BUDGET_MS,
    SPARSE_MAX_TOKENS, SPARSE_MODEL_NAME, SPARSE_NATIVE_DIMENSION, SPARSE_PROJECTED_DIMENSION,
};
