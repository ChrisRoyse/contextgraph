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

pub mod error;
pub mod provider;
pub mod stub;
pub mod traits;
pub mod types;

pub use error::{EmbeddingError, EmbeddingResult};
pub use provider::EmbeddingProvider;
pub use stub::StubEmbedder;
pub use traits::{
    DevicePlacement,
    EmbeddingModel,
    ModelFactory,
    QuantizationMode,
    SingleModelConfig,
};
pub use types::ModelId;

/// Default embedding dimension (FuseMoE output, OpenAI ada-002 compatible).
pub const DEFAULT_DIMENSION: usize = 1536;

/// Total concatenated dimension before FuseMoE fusion (all 12 models).
/// Calculated as sum of projected dimensions from all models.
pub const CONCATENATED_DIMENSION: usize = 8320;
