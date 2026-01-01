//! Traits for embedding model implementations.
//!
//! This module defines the core trait contracts that all 12 embedding models must implement.
//! The `EmbeddingModel` trait provides a unified async interface for embedding generation.
//! The `ModelFactory` trait abstracts model creation for dependency injection.
//!
//! # Thread Safety
//!
//! All trait bounds require `Send + Sync` for safe usage in multi-threaded async runtimes.
//! This enables concurrent model execution across worker threads.
//!
//! # Design Principles
//!
//! - **NO FALLBACKS**: Unsupported inputs return `EmbeddingError::UnsupportedModality`
//! - **FAIL FAST**: Invalid state triggers immediate error via `EmbeddingError`
//! - **ASYNC FIRST**: All operations are async for non-blocking I/O
//! - **CONSERVATIVE ESTIMATES**: Memory estimates are overestimates, never underestimates

mod embedding_model;
mod model_factory;

pub use embedding_model::EmbeddingModel;
pub use model_factory::{
    DevicePlacement,
    ModelFactory,
    QuantizationMode,
    SingleModelConfig,
    MEMORY_ESTIMATES,
    TOTAL_MEMORY_ESTIMATE,
    get_memory_estimate,
};
