//! Causal Discovery Agent for Context Graph
//!
//! This crate provides automated causal relationship discovery using a local LLM
//! (Hermes 2 Pro Mistral 7B via llama-cpp-2) with grammar-constrained JSON output
//! to analyze existing memories and identify cause-effect relationships.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    CAUSAL DISCOVERY AGENT                        │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Memory Scanner → LLM Analysis (GBNF) → E5 Embedder Activation  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Components
//!
//! - **LLM Module**: llama-cpp-2 based Hermes 2 Pro inference with CUDA and GBNF grammar
//! - **Scanner Module**: Finds candidate memory pairs for causal analysis
//! - **Activator Module**: Triggers E5 embedding for confirmed relationships
//! - **Service Module**: Background service for scheduled discovery
//!
//! # Key Improvement: Grammar-Constrained JSON
//!
//! This implementation uses GBNF grammar constraints to guarantee 100% valid JSON output,
//! solving the JSON parsing issues (~40% success) of the previous Candle/Qwen implementation.
//!
//! # VRAM Budget (RTX 5090 32GB)
//!
//! | Model | Quantization | VRAM | Performance |
//! |-------|--------------|------|-------------|
//! | Hermes 2 Pro 7B | Q5_K_M | ~5GB | ~50 tok/s |
//! | KV Cache (4096 ctx) | - | ~1GB | - |
//! | **Total** | - | **~6GB** | within 9GB budget |
//!
//! # Usage
//!
//! ```rust,ignore
//! use context_graph_causal_agent::{CausalDiscoveryService, CausalDiscoveryConfig};
//! use context_graph_causal_agent::types::MemoryForAnalysis;
//!
//! async fn example() {
//!     let config = CausalDiscoveryConfig::default();
//!     let service = CausalDiscoveryService::new(config).await.unwrap();
//!
//!     // Run a single discovery cycle with memories
//!     let memories: Vec<MemoryForAnalysis> = vec![]; // Load from storage
//!     let result = service.run_discovery_cycle(&memories).await.unwrap();
//!     println!("Discovered {} causal relationships", result.relationships_confirmed);
//! }
//! ```

pub mod activator;
pub mod error;
#[cfg(feature = "llm")]
pub mod llm;
pub mod scanner;
#[cfg(feature = "llm")]
pub mod service;
pub mod types;

// Re-exports
pub use activator::E5EmbedderActivator;
pub use error::{CausalAgentError, CausalAgentResult};
#[cfg(feature = "llm")]
pub use llm::{CausalDiscoveryLLM, GrammarType, LlmConfig};
pub use scanner::MemoryScanner;
// Shared types from core (no LLM required).
pub use context_graph_core::types::{DiscoveryCycleResult, ServiceStatus};
#[cfg(feature = "llm")]
pub use service::{CausalDiscoveryConfig, CausalDiscoveryService, CycleMetrics, DiscoveryCursor};
pub use types::{
    CausalAnalysisResult, CausalCandidate, CausalDirectionHint, CausalHint, CausalLinkDirection,
    MemoryForAnalysis,
};
