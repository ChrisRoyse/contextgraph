//! Adapters for bridging embedding models to external trait interfaces.
//!
//! This module provides adapter types that bridge the embedding model implementations
//! to trait interfaces defined in other crates.
//!
//! # Available Adapters
//!
//! - [`E7CodeEmbeddingProvider`]: Wraps `CodeModel` to implement `CodeEmbeddingProvider` trait

pub mod code_provider;

pub use code_provider::E7CodeEmbeddingProvider;
