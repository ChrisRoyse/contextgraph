//! Embedding pipeline for Context Graph.
//!
//! This crate provides GPU-accelerated text-to-embedding conversion using
//! local models via the Candle backend (mandatory).
//!
//! # Mandatory Feature: `candle`
//!
//! This crate requires the `candle` feature to be enabled. The crate is designed
//! for GPU-first architecture targeting NVIDIA RTX 5090 with CUDA 13.1.
//! Building without the `candle` feature will result in a compile error.
//!
//! # Architecture
//!
//! - **ModelId**: Enum identifying the 12 models in the ensemble
//! - **EmbeddingProvider**: Trait for embedding generation
//! - **EmbeddingModel**: Trait for individual model implementations
//! - **ModelRegistry**: Manages model lifecycle and GPU resources
//!
//! # Example
//!
//! ```rust
//! use context_graph_embeddings::{ModelId, ModelRegistryConfig};
//!
//! // Check model dimensions statically
//! assert_eq!(ModelId::Semantic.dimension(), 1024);
//! assert_eq!(ModelId::Code.dimension(), 256);  // CodeT5p embed_dim
//! assert_eq!(ModelId::Entity.dimension(), 384);
//!
//! // Verify projected dimensions for FuseMoE
//! assert_eq!(ModelId::Semantic.projected_dimension(), 1024);
//! assert_eq!(ModelId::Code.projected_dimension(), 768);  // Projected from 256
//!
//! // Registry config defaults
//! let config = ModelRegistryConfig::default();
//! assert!(config.max_concurrent_loads > 0);
//! ```

// =============================================================================
// MANDATORY FEATURE GATE - GPU-First Architecture
// =============================================================================
// The `candle` feature is REQUIRED for context-graph-embeddings.
// This crate is designed for GPU-first architecture targeting NVIDIA RTX 5090
// with CUDA 13.1. All embedding operations require Candle's GPU acceleration.
// =============================================================================

#[cfg(not(feature = "candle"))]
compile_error!(
    "The `candle` feature is required for context-graph-embeddings. \
     This crate is designed for GPU-first architecture targeting NVIDIA RTX 5090 with CUDA 13.1. \
     Build with: cargo build --features candle \
     Or enable the default feature: cargo build (candle is now default)"
);

pub mod batch;
pub mod cache;
pub mod config;
pub mod error;
pub mod fusion;
pub mod gpu;
pub mod models;
pub mod provider;
pub mod storage;
pub mod traits;
pub mod types;

pub use config::{
    BatchConfig, CacheConfig, EmbeddingConfig, EvictionPolicy, FusionConfig, GpuConfig,
    ModelPathConfig, PaddingStrategy,
};
pub use error::{EmbeddingError, EmbeddingResult};
pub use provider::EmbeddingProvider;
pub use traits::{
    get_memory_estimate, DevicePlacement, EmbeddingModel, ModelFactory, QuantizationMode,
    SingleModelConfig, MEMORY_ESTIMATES, TOTAL_MEMORY_ESTIMATE,
};

// Type re-exports for public API
pub use types::{
    AuxiliaryEmbeddingData, ConcatenatedEmbedding, FusedEmbedding, ImageFormat, InputType,
    ModelEmbedding, ModelId, ModelInput, TokenizerFamily,
};

// Re-export dimensions module for constant access
pub use types::dimensions;

// Model registry re-exports
pub use models::{MemoryTracker, ModelRegistry, ModelRegistryConfig, RegistryStats};

// Fusion layer re-exports
pub use fusion::{Activation, Expert, ExpertPool, GatingNetwork, LayerNorm, Linear};

/// Default embedding dimension (FuseMoE output, OpenAI ada-002 compatible).
pub const DEFAULT_DIMENSION: usize = 1536;

/// Total concatenated dimension before FuseMoE fusion (all 12 models).
/// Calculated as sum of projected dimensions from all models.
pub const CONCATENATED_DIMENSION: usize = 8320;
