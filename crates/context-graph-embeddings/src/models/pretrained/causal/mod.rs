//! Causal embedding model using nomic-ai/nomic-embed-text-v1.5.
//!
//! This model (E5) produces 768D vectors optimized for causal reasoning tasks.
//! Uses rotary position embeddings + SwiGLU FFN for efficient processing.
//!
//! # Thread Safety
//! - `AtomicBool` for `loaded` state (lock-free reads)
//! - Inner model/tokenizer require explicit synchronization if mutable
//!
//! # Memory Layout
//! - Total estimated: ~547MB for FP32 weights (NomicBERT base)
//!
//! # Module Structure
//!
//! This module is split into submodules for maintainability:
//! - `config`: Configuration and constants
//! - `weights`: Weight structures for model tensors
//! - `loader`: Weight loading from safetensors
//! - `forward`: Neural network forward pass (RoPE attention, SwiGLU FFN)
//! - `model`: Main CausalModel struct and trait implementation

pub(crate) mod config;
pub(crate) mod forward;
pub(crate) mod loader;
mod model;
pub(crate) mod weights;

#[cfg(test)]
mod tests;

// Re-export used public types
pub use config::{
    CAUSAL_DIMENSION, CAUSAL_LATENCY_BUDGET_MS, CAUSAL_MAX_TOKENS, CAUSE_INSTRUCTION,
    EFFECT_INSTRUCTION,
};

pub use model::CausalModel;
pub use weights::TrainableProjection;
