//! Embedding pipeline for Context Graph.
//!
//! This crate provides text-to-embedding conversion using local models.
//! For Phase 0 (Ghost System), stub implementations return deterministic
//! random embeddings.
//!
//! # Architecture
//!
//! - **ModelId**: Enum identifying the 12 models in the ensemble
//! - **EmbeddingProvider**: Trait for embedding generation
//! - **StubEmbedder**: Deterministic stub for development
//! - **LocalEmbedder**: Future ONNX/Candle implementation
//!
//! # Example
//!
//! ```rust,ignore
//! use context_graph_embeddings::{EmbeddingProvider, StubEmbedder, types::ModelId};
//!
//! let embedder = StubEmbedder::new(1536);
//! let embedding = embedder.embed("Hello world").await?;
//! assert_eq!(embedding.len(), 1536);
//!
//! // Check model dimensions
//! assert_eq!(ModelId::Semantic.dimension(), 1024);
//! ```

pub mod config;
pub mod error;
pub mod models;
pub mod provider;
pub mod storage;
pub mod stub;
pub mod traits;
pub mod types;

pub use config::{
    BatchConfig,
    CacheConfig,
    EmbeddingConfig,
    EvictionPolicy,
    FusionConfig,
    GpuConfig,
    ModelPathConfig,
    PaddingStrategy,
};
pub use error::{EmbeddingError, EmbeddingResult};
pub use provider::EmbeddingProvider;
pub use stub::StubEmbedder;
pub use traits::{
    DevicePlacement,
    EmbeddingModel,
    ModelFactory,
    QuantizationMode,
    SingleModelConfig,
    MEMORY_ESTIMATES,
    TOTAL_MEMORY_ESTIMATE,
    get_memory_estimate,
};

// Type re-exports for public API
pub use types::{
    AuxiliaryEmbeddingData,
    ConcatenatedEmbedding,
    FusedEmbedding,
    ImageFormat,
    InputType,
    ModelEmbedding,
    ModelId,
    ModelInput,
    TokenizerFamily,
};

// Re-export dimensions module for constant access
pub use types::dimensions;

// Model registry re-exports
pub use models::{MemoryTracker, ModelRegistry, ModelRegistryConfig, RegistryStats};

/// Default embedding dimension (FuseMoE output, OpenAI ada-002 compatible).
pub const DEFAULT_DIMENSION: usize = 1536;

/// Total concatenated dimension before FuseMoE fusion (all 12 models).
/// Calculated as sum of projected dimensions from all models.
pub const CONCATENATED_DIMENSION: usize = 8320;
