//! Causal relationship utilities for ContextGraph.
//!
//! The local LLM discovery backend was removed in the 2026-05-09 ME-JEPA pivot.
//! This crate now keeps the deterministic pieces that still belong in the
//! non-generative system: candidate scanning, causal data types, and E5
//! activation helpers.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    CAUSAL UTILITY SURFACE                        │
//! ├─────────────────────────────────────────────────────────────────┤
//! │              Memory Scanner → E5 Embedder Activation             │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

#[cfg(feature = "llm")]
compile_error!("The local LLM causal backend was retired on 2026-05-09. Use ME-JEPA evidence capture instead.");

pub mod activator;
pub mod error;
pub mod scanner;
pub mod types;

// Re-exports
pub use activator::E5EmbedderActivator;
pub use error::{CausalAgentError, CausalAgentResult};
pub use scanner::MemoryScanner;
// Shared types from core (no LLM required).
pub use context_graph_core::types::{DiscoveryCycleResult, ServiceStatus};
pub use types::{
    CausalAnalysisResult, CausalCandidate, CausalDirectionHint, CausalHint, CausalLinkDirection,
    MemoryForAnalysis,
};
