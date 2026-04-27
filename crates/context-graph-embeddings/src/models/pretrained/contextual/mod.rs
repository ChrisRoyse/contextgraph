//! Contextual embedding model using intfloat/e5-base-v2.
//!
//! This model (part of E10) produces 768D vectors optimized for asymmetric
//! retrieval. It uses E5-base-v2's prefix-based encoding for intent vs
//! context differentiation:
//!
//! - "query: " prefix for intent/query embeddings
//! - "passage: " prefix for context/document embeddings
//!
//! # GPU Acceleration
//!
//! When the `candle` feature is enabled, this model uses GPU-accelerated
//! BERT inference via Candle with the following pipeline:
//! 1. Tokenization with HuggingFace tokenizers (with prefix prepending)
//! 2. GPU embedding lookup and position encoding
//! 3. GPU-accelerated transformer forward pass (12 layers)
//! 4. Mean pooling over sequence dimension
//! 5. L2 normalization on GPU
//!
//! # Asymmetric Embeddings (E5-base-v2)
//!
//! Unlike the previous MPNet + projection approach, E5-base-v2 achieves
//! asymmetry through learned prefix-based encoding:
//! - Queries (intent): "query: " prefix encodes search intent
//! - Documents (context): "passage: " prefix encodes document content
//!
//! This creates genuinely learned asymmetric representations without
//! requiring separate projection matrices.
//!
//! # Thread Safety
//! - `AtomicBool` for `loaded` state (lock-free reads)
//! - `RwLock` for model state (thread-safe state transitions)
//!
//! # Memory Layout
//! - Total estimated: ~440MB for FP32 weights (110M parameters)
//! - With FP16 quantization: ~220MB

mod constants;
mod model;
mod state;

// Re-export public API
pub use constants::{
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
    // E5-base-v2 prefix constants for asymmetric encoding
    INTENT_PREFIX,
};

pub use model::ContextualModel;
