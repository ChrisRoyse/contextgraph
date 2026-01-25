//! Full Code Embedding Provider adapter.
//!
//! This adapter wraps a `MultiArrayEmbeddingProvider` to implement
//! the `CodeEmbeddingProvider` trait from `context-graph-core`.
//!
//! # Architecture
//!
//! Per constitution (ARCH-01, ARCH-05): All 13 embedders are required for code.
//! This adapter bridges:
//! - `MultiArrayEmbeddingProvider` from `context-graph-embeddings` (13-embedder orchestrator)
//! - `CodeEmbeddingProvider` trait from `context-graph-core` (for code capture pipeline)
//!
//! # Thread Safety
//!
//! `E7CodeEmbeddingProvider` is `Send + Sync` and can be safely shared across threads.
//!
//! # Example
//!
//! ```rust,no_run
//! use context_graph_embeddings::adapters::E7CodeEmbeddingProvider;
//! use context_graph_embeddings::provider::ProductionMultiArrayProvider;
//! use context_graph_embeddings::config::GpuConfig;
//! use std::sync::Arc;
//! use std::path::PathBuf;
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create and load the multi-array provider
//!     let provider = Arc::new(ProductionMultiArrayProvider::new(
//!         PathBuf::from("models"),
//!         GpuConfig::default(),
//!     )?);
//!     provider.initialize().await?;
//!
//!     // Wrap in code provider adapter
//!     let code_provider = E7CodeEmbeddingProvider::new(provider);
//!
//!     // Now use the provider via the CodeEmbeddingProvider trait
//!     let fingerprint = code_provider.embed_code("fn hello() {}", None).await?;
//!     assert_eq!(fingerprint.e7_code.len(), 1536);  // E7 is 1536D
//!     assert_eq!(fingerprint.e1_semantic.len(), 1024);  // E1 is 1024D
//!
//!     Ok(())
//! }
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, instrument};

use context_graph_core::memory::{CodeEmbedderError, CodeEmbeddingProvider};
use context_graph_core::traits::MultiArrayEmbeddingProvider;
use context_graph_core::types::fingerprint::SemanticFingerprint;

/// E7 Code Embedding Provider (Full 13-Embedder).
///
/// Despite the name (kept for backward compatibility), this provider uses
/// ALL 13 embedders to generate a complete SemanticFingerprint for code.
///
/// # Constitution Compliance
///
/// - ARCH-01: TeleologicalArray is atomic - all 13 embeddings or nothing
/// - ARCH-05: All 13 embedders required - missing = fatal
/// - E7 (V_correctness) provides code-specific patterns
/// - Other embedders (E1 semantic, E5 causal, etc.) provide context
///
/// # GPU Requirements
///
/// - NVIDIA CUDA GPU with sufficient VRAM for all 13 models
/// - CUDA 13.1+ recommended
pub struct E7CodeEmbeddingProvider {
    /// The underlying multi-array provider (all 13 embedders).
    provider: Arc<dyn MultiArrayEmbeddingProvider>,
}

impl E7CodeEmbeddingProvider {
    /// Create a new E7CodeEmbeddingProvider wrapping the given multi-array provider.
    ///
    /// # Arguments
    /// * `provider` - Arc-wrapped MultiArrayEmbeddingProvider instance
    ///
    /// # Note
    /// The provider should be initialized before use.
    pub fn new(provider: Arc<dyn MultiArrayEmbeddingProvider>) -> Self {
        Self { provider }
    }

    /// Check if the underlying provider is initialized and ready.
    pub fn is_ready(&self) -> bool {
        self.provider.is_ready()
    }

    /// Convert CoreError to CodeEmbedderError.
    fn convert_error(e: context_graph_core::error::CoreError) -> CodeEmbedderError {
        match e {
            context_graph_core::error::CoreError::ValidationError { field, message } => {
                CodeEmbedderError::InvalidInput {
                    reason: format!("{}: {}", field, message),
                }
            }
            _ => CodeEmbedderError::ComputationFailed {
                message: format!("{}", e),
            },
        }
    }
}

#[async_trait]
impl CodeEmbeddingProvider for E7CodeEmbeddingProvider {
    /// Embed code content into a full 13-embedding SemanticFingerprint.
    ///
    /// # Arguments
    /// * `code` - The code content to embed
    /// * `context` - Optional context (e.g., file path, language hint)
    ///
    /// # Returns
    /// Complete SemanticFingerprint with all 13 embeddings on success.
    ///
    /// # Errors
    /// - `ComputationFailed` if embedding computation fails
    /// - `InvalidInput` if the input is invalid
    #[instrument(skip(self, code, context), fields(code_len = code.len()))]
    async fn embed_code(
        &self,
        code: &str,
        context: Option<&str>,
    ) -> Result<SemanticFingerprint, CodeEmbedderError> {
        // Prepare input with optional context
        let content = match context {
            Some(ctx) => format!("// Context: {}\n{}", ctx, code),
            None => code.to_string(),
        };

        // Generate all 13 embeddings
        let output = self
            .provider
            .embed_all(&content)
            .await
            .map_err(Self::convert_error)?;

        debug!(
            total_latency_ms = output.total_latency.as_millis(),
            e7_dim = output.fingerprint.e7_code.len(),
            e1_dim = output.fingerprint.e1_semantic.len(),
            "Full 13-embedding fingerprint generated for code"
        );

        Ok(output.fingerprint)
    }

    /// Embed a batch of code snippets.
    ///
    /// More efficient than calling `embed_code` multiple times due to
    /// batched GPU processing.
    ///
    /// # Arguments
    /// * `codes` - Slice of (code, optional_context) tuples
    ///
    /// # Returns
    /// Vector of SemanticFingerprints, one per input.
    #[instrument(skip(self, codes), fields(batch_size = codes.len()))]
    async fn embed_batch(
        &self,
        codes: &[(&str, Option<&str>)],
    ) -> Result<Vec<SemanticFingerprint>, CodeEmbedderError> {
        if codes.is_empty() {
            return Ok(Vec::new());
        }

        // Prepare inputs with optional contexts
        let contents: Vec<String> = codes
            .iter()
            .map(|(code, context)| match context {
                Some(ctx) => format!("// Context: {}\n{}", ctx, code),
                None => (*code).to_string(),
            })
            .collect();

        // Generate all 13 embeddings for each input
        let outputs = self
            .provider
            .embed_batch_all(&contents)
            .await
            .map_err(Self::convert_error)?;

        debug!(
            batch_size = outputs.len(),
            e7_dim = outputs.first().map(|o| o.fingerprint.e7_code.len()).unwrap_or(0),
            "Batch 13-embedding fingerprints generated for code"
        );

        Ok(outputs.into_iter().map(|o| o.fingerprint).collect())
    }

    /// Check if all 13 embedders are initialized and ready.
    fn is_ready(&self) -> bool {
        self.provider.is_ready()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_e7_dimension() {
        // Verify the E7 dimension constant (Qodo-Embed-1-1.5B)
        assert_eq!(1536, crate::models::CODE_NATIVE_DIMENSION);
    }
}
