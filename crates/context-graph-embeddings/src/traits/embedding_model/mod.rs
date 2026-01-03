//! Core trait for embedding model implementations.
//!
//! The `EmbeddingModel` trait defines the contract that all 12 embedding models
//! in the fusion pipeline must implement. Each model (E1-E12) has different
//! input requirements, dimensions, and processing characteristics.
//!
//! # Model Compatibility Matrix
//!
//! | Model | Text | Code | Image | Audio |
//! |-------|------|------|-------|-------|
//! | Semantic (E1) | ✓ | ✓* | ✗ | ✗ |
//! | TemporalRecent (E2) | ✓ | ✓ | ✗ | ✗ |
//! | TemporalPeriodic (E3) | ✓ | ✓ | ✗ | ✗ |
//! | TemporalPositional (E4) | ✓ | ✓ | ✗ | ✗ |
//! | Causal (E5) | ✓ | ✓ | ✗ | ✗ |
//! | Sparse (E6) | ✓ | ✓* | ✗ | ✗ |
//! | Code (E7) | ✓* | ✓ | ✗ | ✗ |
//! | Graph (E8) | ✓ | ✓* | ✗ | ✗ |
//! | HDC (E9) | ✓ | ✓ | ✗ | ✗ |
//! | Multimodal (E10) | ✓ | ✗ | ✓ | ✗ |
//! | Entity (E11) | ✓ | ✓* | ✗ | ✗ |
//! | LateInteraction (E12) | ✓ | ✓* | ✗ | ✗ |
//!
//! *Model can process but is not optimized for this type
//!
//! # Thread Safety
//!
//! The trait requires `Send + Sync` bounds to ensure safe usage in
//! multi-threaded async contexts. All implementations must be thread-safe.
//!
//! # Example: Query Model Properties
//!
//! ```
//! # use context_graph_embeddings::types::ModelId;
//! // Models have well-defined properties from ModelId
//! let model_id = ModelId::Semantic;
//!
//! // Query dimension and latency budget
//! assert_eq!(model_id.dimension(), 1024);
//! assert_eq!(model_id.latency_budget_ms(), 5);
//! assert!(model_id.is_pretrained());
//! ```

mod trait_def;

#[cfg(test)]
mod tests;

pub use trait_def::EmbeddingModel;
