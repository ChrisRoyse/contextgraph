//! Sparse embedding model using naver/splade-cocondenser-ensembledistil.
//!
//! This model (E6) produces high-dimensional sparse vectors (30522D vocab-sized).
//! Uses BERT backbone with MLM head to compute term importance weights.
//!
//! # SPLADE (Sparse Lexical and Expansion) Mechanism
//!
//! SPLADE learns sparse representations where each dimension corresponds to a
//! vocabulary term. The model predicts term importance via:
//!
//! 1. BERT encoder produces contextualized representations
//! 2. MLM head projects to vocabulary dimension
//! 3. Log-saturating activation: log(1 + ReLU(x))
//! 4. Max-pooling over sequence positions
//!
//! The result is a sparse vector where non-zero entries indicate important terms.
//!
//! # GPU Acceleration
//!
//! When the `candle` feature is enabled, this model uses GPU-accelerated BERT inference
//! via Candle with vocabulary projection for sparse term weights.
//!
//! # Thread Safety
//! - `AtomicBool` for `loaded` state (lock-free reads)
//! - Inner model/tokenizer require explicit synchronization if mutable
//!
//! # Memory Layout
//! - Total estimated: ~440MB for FP32 weights (base BERT + MLM head)
//! - With FP16 quantization: ~220MB
//!
//! # Module Structure
//!
//! This module is organized into submodules:
//! - `types`: Constants, SparseVector, MlmHeadWeights, ModelState
//! - `model`: SparseModel struct and core methods
//! - `loader`: MLM head weight loading
//! - `forward`: GPU forward pass implementation
//! - `embeddings`: Token embedding computation
//! - `encoder`: Encoder layer and FFN forward pass
//! - `attention`: Self-attention forward pass
//! - `mlm_head`: MLM head and SPLADE activation
//! - `traits`: EmbeddingModel trait implementation
//! - `tests`: Unit and integration tests

mod attention;
mod embeddings;
mod encoder;
mod forward;
mod loader;
mod mlm_head;
mod model;
mod projection;
mod traits;
mod types;

#[cfg(test)]
mod tests;

// Re-export used public types
pub use model::SparseModel;
#[allow(unused_imports)]
pub use types::{
    SparseVector, SPARSE_EXPECTED_SPARSITY, SPARSE_HIDDEN_SIZE, SPARSE_LATENCY_BUDGET_MS,
    SPARSE_MAX_TOKENS, SPARSE_MODEL_NAME, SPARSE_NATIVE_DIMENSION, SPARSE_PROJECTED_DIMENSION,
    SPARSE_VOCAB_SIZE,
};
