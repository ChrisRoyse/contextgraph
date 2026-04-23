//! Embedding provider trait definition.
//!
//! This module provides the core abstractions for embedding providers:
//! - [`EmbeddingProvider`]: Trait for single-model embedding providers
//! - [`ProductionMultiArrayProvider`]: Production 13-embedder orchestrator
//! - [`CausalHintProvider`]: LLM-based causal hints for E5 enhancement
//!
//! # Architecture
//!
//! ```text
//! EmbeddingProvider (trait)
//! ├── embed(&str) -> Vec<f32>           // Single text embedding
//! ├── embed_batch(&[&str]) -> Vec<Vec<f32>>  // Batch embedding
//! ├── dimension() -> usize              // Output dimension
//! ├── model_name() -> &str              // Model identifier
//! └── max_tokens() -> usize             // Token limit
//!
//! ProductionMultiArrayProvider
//! ├── embed_all(&str) -> MultiArrayEmbeddingOutput  // All 13 embeddings
//! ├── embed_batch_all(&[String]) -> Vec<Output>     // Batch all 13
//! ├── model_ids() -> [&str; 14]                     // Model identifiers
//! ├── is_ready() -> bool                            // Readiness check
//! └── health_status() -> [bool; 14]                 // Per-embedder health
//!
//! CausalHintProvider (trait)
//! ├── get_hint(&str) -> Option<CausalHint>  // LLM-based causal hint
//! ├── is_available() -> bool                // Check if LLM is ready
//! └── timeout() -> Duration                 // Get timeout duration
//! ```

mod causal_hint;
mod multi_array;

pub use causal_hint::{CausalHintProvider, ExtractionStatus, NoOpCausalHintProvider};
pub use multi_array::ProductionMultiArrayProvider;

use async_trait::async_trait;

use crate::error::EmbeddingResult;

/// Trait for embedding providers.
///
/// Implementations convert text to dense vector representations.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding for a single text.
    async fn embed(&self, text: &str) -> EmbeddingResult<Vec<f32>>;

    /// Generate embeddings for multiple texts.
    ///
    /// Default implementation calls `embed` for each text.
    /// Implementations may override for batch optimization.
    async fn embed_batch(&self, texts: &[&str]) -> EmbeddingResult<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }

    /// Get the output dimension of embeddings.
    fn dimension(&self) -> usize;

    /// Get the model name/identifier.
    fn model_name(&self) -> &str;

    /// Get maximum input token count.
    fn max_tokens(&self) -> usize;
}
