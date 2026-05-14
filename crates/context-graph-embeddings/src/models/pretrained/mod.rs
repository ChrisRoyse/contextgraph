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

pub(crate) mod shared;

mod bert_batch;
mod bge_m3_dense;
pub(crate) mod causal;
mod code;
mod contextual;
mod graph;
pub mod kepler;
mod late_interaction;
pub mod rgcn;
mod semantic;
mod sparse;
pub mod weight_projection;

pub use bge_m3_dense::{
    BgeM3DenseModel, BGE_M3_DENSE_DIMENSION, BGE_M3_DENSE_LATENCY_BUDGET_MS,
    BGE_M3_DENSE_MAX_TOKENS, XLM_R_BOS_TOKEN_ID, XLM_R_PAD_TOKEN_ID, XLM_R_POSITION_OFFSET,
    XLM_R_WEIGHT_PREFIX,
};
pub use causal::loader::load_nomic_weights;
pub use causal::weights::NomicWeights;
pub use causal::{
    CausalModel, TrainableProjection, CAUSAL_DIMENSION, CAUSAL_LATENCY_BUDGET_MS,
    CAUSAL_MAX_TOKENS, CAUSE_INSTRUCTION, EFFECT_INSTRUCTION,
};
pub use code::{
    CodeModel, CODE_LATENCY_BUDGET_MS, CODE_MAX_TOKENS, CODE_MODEL_NAME, CODE_NATIVE_DIMENSION,
    CODE_PROJECTED_DIMENSION,
};
pub use contextual::{
    ContextualModel,
    CONTEXTUAL_DIMENSION,
    CONTEXTUAL_HIDDEN_SIZE,
    CONTEXTUAL_INTERMEDIATE_SIZE,
    CONTEXTUAL_LATENCY_BUDGET_MS,
    CONTEXTUAL_LAYER_NORM_EPS,
    CONTEXTUAL_MAX_TOKENS,
    CONTEXTUAL_MODEL_NAME,
    CONTEXTUAL_NUM_HEADS,
    CONTEXTUAL_NUM_LAYERS,
    CONTEXTUAL_VOCAB_SIZE,
    CONTEXT_PREFIX,
    // E5-base-v2 prefix constants
    INTENT_PREFIX,
};
pub use graph::{
    GraphModel, GRAPH_DIMENSION, GRAPH_LATENCY_BUDGET_MS, GRAPH_MAX_TOKENS, GRAPH_MODEL_NAME,
    MAX_CONTEXT_NEIGHBORS,
};
pub use kepler::{
    KeplerModel, KEPLER_DIMENSION, KEPLER_LATENCY_BUDGET_MS, KEPLER_MAX_TOKENS, KEPLER_MODEL_NAME,
};
pub use late_interaction::{
    validate_late_interaction_batch_vram_budget, LateInteractionBatchVramPlan,
    LateInteractionModel, TokenEmbeddings, LATE_INTERACTION_DIMENSION,
    LATE_INTERACTION_LATENCY_BUDGET_MS, LATE_INTERACTION_MAX_TOKENS, LATE_INTERACTION_MODEL_NAME,
};
pub use rgcn::{
    RelationalGCN, DEFAULT_CONFIG_PATH, DEFAULT_WEIGHTS_PATH as RGCN_WEIGHTS_PATH, HIDDEN_DIM,
    INPUT_DIM, NUM_BASES, NUM_RELATIONS, OUTPUT_DIM as RGCN_OUTPUT_DIM, RELATION_NAMES,
};
pub use semantic::{
    SemanticModel, PASSAGE_PREFIX, QUERY_PREFIX, SEMANTIC_DIMENSION, SEMANTIC_LATENCY_BUDGET_MS,
    SEMANTIC_MAX_TOKENS,
};
pub use sparse::{
    validate_true_batch_vram_budget, SparseBatchVramPlan, SparseModel, SparseVector,
    SPARSE_EXPECTED_SPARSITY, SPARSE_LATENCY_BUDGET_MS, SPARSE_MAX_TOKENS, SPARSE_MODEL_NAME,
    SPARSE_NATIVE_DIMENSION, SPARSE_PROJECTED_DIMENSION, SPARSE_VOCAB_SIZE,
};
pub use weight_projection::{
    LearnedWeightProjection, DEFAULT_CATEGORY_WEIGHTS, DEFAULT_WEIGHTS_PATH, HIDDEN_DIM_1,
    HIDDEN_DIM_2, MAX_WEIGHTED_AGREEMENT, NUM_EMBEDDERS, OUTPUT_DIM, WEIGHTED_AGREEMENT_THRESHOLD,
};
