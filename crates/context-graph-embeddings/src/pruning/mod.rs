//! Token pruning for E12 (ColBERT) late-interaction embeddings.
//!
//! This module provides configuration types for token pruning,
//! which reduces embedding size by ~50% while maintaining recall quality.
//!
//! # Constitution References
//!
//! - embeddings.models.E12_LateInteraction: "128D/tok, dense_per_token"
//! - perf.quality.info_loss: "<15%"
//!
//! # Example
//!
//! ```rust
//! use context_graph_embeddings::pruning::{TokenPruningConfig, ImportanceScoringMethod};
//!
//! // Default config: 50% compression, 64 min tokens, AttentionBased scoring
//! let config = TokenPruningConfig::default();
//! assert!(config.validate().is_ok());
//!
//! // Custom compression ratio
//! let config = TokenPruningConfig::with_compression(0.7).unwrap();
//! assert_eq!(config.target_compression, 0.7);
//! ```

mod config;
mod token_pruner;

pub use config::{ImportanceScoringMethod, PrunedEmbeddings, TokenPruningConfig};
pub use token_pruner::TokenPruningQuantizer;
