//! Context Graph Core Library
//!
//! Provides core domain types, traits, and stub implementations for the
//! Ultimate Context Graph system implementing UTL (Unified Theory of Learning).
//!
//! # Architecture
//!
//! This crate defines:
//! - Domain types (`TeleologicalFingerprint`, `SemanticFingerprint`, `PurposeVector`, etc.)
//! - Core traits (`TeleologicalMemoryStore`, `MultiArrayEmbeddingProvider`, `NervousLayer`, etc.)
//! - Error types and result aliases
//! - Configuration structures
//!
//! # Example
//!
//! ```
//! use context_graph_core::traits::{TeleologicalMemoryStore, TeleologicalSearchOptions};
//!
//! // Create search options for querying
//! let options = TeleologicalSearchOptions::quick(10)
//!     .with_min_similarity(0.8);
//! assert_eq!(options.top_k, 10);
//! ```

pub mod config;
pub mod error;
pub mod marblestone;
pub mod retrieval;
pub mod stubs;
pub mod traits;
pub mod types;

// Re-exports for convenience
pub use config::Config;
pub use error::{CoreError, CoreResult};
pub use marblestone::{Domain, EdgeType, NeurotransmitterWeights};
