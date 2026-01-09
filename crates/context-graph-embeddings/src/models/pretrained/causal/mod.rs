//! Causal embedding model using allenai/longformer-base-4096.
//!
//! This model (E5) produces 768D vectors optimized for causal reasoning tasks.
//! Uses sliding window attention + global attention for 4096 token context.
//!
//! # Thread Safety
//! - `AtomicBool` for `loaded` state (lock-free reads)
//! - Inner model/tokenizer require explicit synchronization if mutable
//!
//! # Memory Layout
//! - Total estimated: ~750MB for FP32 weights (base model)
//! - With FP16 quantization: ~375MB
//!
//! # Module Structure
//!
//! This module is split into submodules for maintainability:
//! - `config`: Configuration and constants
//! - `weights`: Weight structures for model tensors
//! - `loader`: Main weight loading orchestration
//! - `embeddings_loader`: Embedding layer weight loading
//! - `encoder_loader`: Encoder layer weight loading (attention + FFN)
//! - `forward`: Neural network forward pass
//! - `model`: Main CausalModel struct and trait implementation

mod config;
mod embeddings_loader;
mod encoder_loader;
mod forward;
mod loader;
mod model;
mod weights;

#[cfg(test)]
mod tests;

// Re-export used public types
pub use config::{
    CAUSAL_DIMENSION, CAUSAL_LATENCY_BUDGET_MS, CAUSAL_MAX_TOKENS, DEFAULT_ATTENTION_WINDOW,
};

pub use model::CausalModel;
