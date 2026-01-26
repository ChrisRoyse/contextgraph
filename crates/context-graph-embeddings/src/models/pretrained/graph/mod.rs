//! Graph embedding model using sentence-transformers/paraphrase-MiniLM-L6-v2.
//!
//! This model (E8) produces 384D vectors optimized for knowledge graph embeddings,
//! including entity and relation encoding for graph structure understanding.
//!
//! # Dimension
//!
//! - Native output: 384D (final dimension, no projection needed)
//!
//! GraphModel uses 384D directly (unlike CodeModel which uses 1536D via Qodo-Embed)
//! as this is the native MiniLM embedding dimension.
//!
//! # Asymmetric Dual Embeddings
//!
//! Following the E5 Causal pattern (ARCH-15), this model supports asymmetric
//! source/target embeddings via `embed_dual()`:
//!
//! - **Source embedding**: Represents the entity as a source of outgoing relationships
//!   (e.g., "Module A imports B, C, D")
//! - **Target embedding**: Represents the entity as a target of incoming relationships
//!   (e.g., "Module X is imported by A, B, C")
//!
//! # Thread Safety
//! - `AtomicBool` for `loaded` state (lock-free reads)
//! - `RwLock` for model state (thread-safe state transitions)
//!
//! # Memory Layout
//! - Total estimated: ~80MB for FP32 weights (22M parameters)
//! - With FP16 quantization: ~40MB
//!
//! # Module Structure
//!
//! This module is split into submodules for maintainability:
//! - `constants`: Configuration constants (dimensions, tokens, latency)
//! - `state`: Internal model state management
//! - `encoding`: Graph-specific encoding utilities (relations, context)
//! - `projections`: Asymmetric source/target projection weights
//! - `layer_norm`: LayerNorm implementation
//! - `attention`: Self-attention for encoder layers
//! - `ffn`: Feed-forward network implementation
//! - `encoder`: Full encoder layer combining attention + FFN
//! - `forward`: Complete GPU forward pass
//! - `model`: Core GraphModel struct and EmbeddingModel impl

mod attention;
mod constants;
mod encoder;
mod encoding;
mod ffn;
mod forward;
mod layer_norm;
mod model;
pub mod projections;
mod state;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_edge_cases;

// Re-export public API for backwards compatibility
pub use constants::{
    GRAPH_DIMENSION, GRAPH_LATENCY_BUDGET_MS, GRAPH_MAX_TOKENS, GRAPH_MODEL_NAME,
    MAX_CONTEXT_NEIGHBORS,
};
pub use model::GraphModel;
