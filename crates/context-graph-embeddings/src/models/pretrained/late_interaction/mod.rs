//! Late-interaction embedding model using colbert-ir/colbertv2.0.
//!
//! This model (E12) produces per-token 128D vectors enabling fine-grained
//! matching via MaxSim scoring. Unlike single-vector models, ColBERT preserves
//! token-level information for more expressive retrieval.
//!
//! # Dimension
//!
//! - Native output: 128D per token (projected from BERT's 768D)
//! - Pooled output: 128D single vector for multi-array storage fusion
//!
//! # GPU Acceleration
//!
//! When the `candle` feature is enabled, this model uses GPU-accelerated BERT inference
//! via Candle with the following pipeline:
//! 1. Tokenization with HuggingFace tokenizers
//! 2. GPU embedding lookup and position encoding
//! 3. GPU-accelerated transformer forward pass (12 layers)
//! 4. Linear projection from 768D to 128D per token
//! 5. L2 normalization per token on GPU
//!
//! # MaxSim Scoring
//!
//! ColBERT's key innovation is the MaxSim function:
//! ```text
//! score(Q, D) = Sigma_i max_j cos(q_i, d_j)
//! ```
//! For each query token, find the maximum similarity to any document token,
//! then sum over all query tokens.
//!
//! # Thread Safety
//! - `AtomicBool` for `loaded` state (lock-free reads)
//! - `RwLock` for model state (thread-safe state transitions)
//!
//! # Memory Layout
//! - Per text (100 tokens): 100 * 128 * 4 bytes = 51.2 KB
//! - Model weights: ~440MB for FP32

mod embedding;
mod gpu_attention;
mod gpu_attention_ops;
mod gpu_encoder;
mod gpu_forward;
mod gpu_projection;
mod gpu_utils;
mod model;
mod scoring;
mod trait_impl;
mod types;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_batch;
#[cfg(test)]
mod tests_extended;
#[cfg(test)]
mod tests_token;

// Re-export all public types for backwards compatibility
pub use model::LateInteractionModel;
pub use types::{
    validate_late_interaction_batch_vram_budget, LateInteractionBatchVramPlan, TokenEmbeddings,
    LATE_INTERACTION_DIMENSION, LATE_INTERACTION_LATENCY_BUDGET_MS, LATE_INTERACTION_MAX_TOKENS,
    LATE_INTERACTION_MODEL_NAME,
};
