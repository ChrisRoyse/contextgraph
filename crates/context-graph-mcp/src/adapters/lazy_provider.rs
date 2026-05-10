//! Lazy MultiArrayEmbeddingProvider that allows immediate MCP startup.
//!
//! This adapter wraps an optional provider that loads in the background,
//! returning "models loading" errors until the real provider is ready.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::traits::{
    EmbeddingMetadata, MultiArrayEmbeddingOutput, MultiArrayEmbeddingProvider,
};
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_core::weights::E11_ENTITY_ENABLED;

/// Lazy wrapper for MultiArrayEmbeddingProvider that allows immediate MCP startup.
///
/// This adapter holds an optional provider that loads in the background.
/// Until the provider is ready, all embedding calls return a "models loading" error.
///
/// # Design
///
/// The server can respond to MCP `initialize` immediately while models load
/// in the background. This prevents Claude Code from timing out waiting for
/// the server to start (which takes 30+ seconds for 13 models).
///
/// # Thread Safety
///
/// - Uses `Arc<RwLock<Option<...>>>` for the inner provider
/// - Uses `AtomicBool` for loading/failed flags
/// - Safe to share across tokio tasks
pub struct LazyMultiArrayProvider {
    /// The real provider, once loaded
    inner: Arc<RwLock<Option<Arc<dyn MultiArrayEmbeddingProvider>>>>,
    /// True while models are still loading
    loading: Arc<AtomicBool>,
    /// Error message if loading failed
    failed: Arc<RwLock<Option<String>>>,
}

#[allow(dead_code)]
impl LazyMultiArrayProvider {
    /// Create a new lazy provider.
    ///
    /// # Arguments
    ///
    /// * `inner` - Shared slot for the provider (populated by background task)
    /// * `loading` - Flag indicating loading is in progress
    /// * `failed` - Slot for error message if loading fails
    pub fn new(
        inner: Arc<RwLock<Option<Arc<dyn MultiArrayEmbeddingProvider>>>>,
        loading: Arc<AtomicBool>,
        failed: Arc<RwLock<Option<String>>>,
    ) -> Self {
        Self {
            inner,
            loading,
            failed,
        }
    }

    /// Check if the provider is ready.
    pub fn is_loaded(&self) -> bool {
        !self.loading.load(Ordering::SeqCst)
    }

    /// Get the loading status message.
    pub async fn status_message(&self) -> String {
        if self.loading.load(Ordering::SeqCst) {
            "Embedding models are still loading. Please wait...".to_string()
        } else if let Some(ref err) = *self.failed.read().await {
            format!("Embedding model loading failed: {}", err)
        } else {
            "Embedding models loaded and ready.".to_string()
        }
    }
}

#[async_trait]
impl MultiArrayEmbeddingProvider for LazyMultiArrayProvider {
    async fn embed_all(&self, content: &str) -> CoreResult<MultiArrayEmbeddingOutput> {
        // Check if still loading
        if self.loading.load(Ordering::SeqCst) {
            return Err(CoreError::Internal(
                "Embedding models are still loading. Please wait and try again.".to_string(),
            ));
        }

        // Check if loading failed
        if let Some(ref err) = *self.failed.read().await {
            return Err(CoreError::Internal(format!(
                "Embedding model loading failed: {}",
                err
            )));
        }

        // Get the provider
        let guard = self.inner.read().await;
        match guard.as_ref() {
            Some(provider) => provider.embed_all(content).await,
            None => Err(CoreError::Internal(
                "Embedding provider not available. This is a bug.".to_string(),
            )),
        }
    }

    /// MCP-H1 FIX: Override embed_all_with_metadata to delegate to real provider.
    /// Without this, the default trait impl discards E4 session_sequence and E5 causal_hint,
    /// silently falling back to embed_all() which produces incorrect E4/E5 embeddings.
    async fn embed_all_with_metadata(
        &self,
        content: &str,
        metadata: EmbeddingMetadata,
    ) -> CoreResult<MultiArrayEmbeddingOutput> {
        if self.loading.load(Ordering::SeqCst) {
            return Err(CoreError::Internal(
                "Embedding models are still loading. Please wait and try again.".to_string(),
            ));
        }

        if let Some(ref err) = *self.failed.read().await {
            return Err(CoreError::Internal(format!(
                "Embedding model loading failed: {}",
                err
            )));
        }

        let guard = self.inner.read().await;
        match guard.as_ref() {
            Some(provider) => provider.embed_all_with_metadata(content, metadata).await,
            None => Err(CoreError::Internal(
                "Embedding provider not available. This is a bug.".to_string(),
            )),
        }
    }

    async fn embed_batch_all(
        &self,
        contents: &[String],
        metadata: &[EmbeddingMetadata],
    ) -> CoreResult<Vec<MultiArrayEmbeddingOutput>> {
        // Check if still loading
        if self.loading.load(Ordering::SeqCst) {
            return Err(CoreError::Internal(
                "Embedding models are still loading. Please wait and try again.".to_string(),
            ));
        }

        // Check if loading failed
        if let Some(ref err) = *self.failed.read().await {
            return Err(CoreError::Internal(format!(
                "Embedding model loading failed: {}",
                err
            )));
        }

        // Get the provider
        let guard = self.inner.read().await;
        match guard.as_ref() {
            Some(provider) => provider.embed_batch_all(contents, metadata).await,
            None => Err(CoreError::Internal(
                "Embedding provider not available. This is a bug.".to_string(),
            )),
        }
    }

    fn model_ids(&self) -> [&str; NUM_EMBEDDERS] {
        // Return placeholder IDs - these are only used for diagnostics
        [
            "lazy_semantic",
            "lazy_temporal_recent",
            "lazy_temporal_periodic",
            "lazy_temporal_positional",
            "lazy_causal",
            "lazy_sparse",
            "lazy_code",
            "lazy_graph",
            "lazy_hdc",
            "lazy_multimodal",
            "lazy_entity",
            "lazy_late_interaction",
            "lazy_splade",
            "lazy_bge_m3_dense",
        ]
    }

    fn is_ready(&self) -> bool {
        // L1 FIX: Check both loading AND failed flags for honest health reporting.
        if self.loading.load(Ordering::SeqCst) {
            return false;
        }
        // Non-blocking check of failure state (returns not-ready if lock contended)
        if self.failed.try_read().map_or(true, |f| f.is_some()) {
            return false;
        }
        true
    }

    fn health_status(&self) -> [bool; NUM_EMBEDDERS] {
        // L1 FIX: Report unhealthy when loading OR failed
        if !self.is_ready() {
            return [false; NUM_EMBEDDERS];
        }
        // MCP-M2 FIX: Delegate to real provider for per-embedder health.
        // Previously hardcoded [true; 14], hiding post-startup embedder failures.
        match self.inner.try_read() {
            Ok(guard) => match guard.as_ref() {
                Some(provider) => provider.health_status(),
                None => [false; NUM_EMBEDDERS],
            },
            // Audit-10 SRV-L7 FIX: Unknown state ≠ healthy. Return all-false on lock contention
            // so callers don't falsely assume embedders are ready.
            Err(_) => [false; NUM_EMBEDDERS],
        }
    }

    /// Audit-7 MCP-H1 FIX: Override embed_e1_only to delegate to real provider.
    /// Without this, the default trait impl calls self.embed_all() which runs all 14
    /// embedders instead of delegating to inner.embed_e1_only() (runs only E1).
    /// This causes 13x overhead for multi-embedder search in search_causal_relationships.
    async fn embed_e1_only(&self, content: &str) -> CoreResult<Vec<f32>> {
        if self.loading.load(Ordering::SeqCst) {
            return Err(CoreError::Internal(
                "Embedding models are still loading. Please wait and try again.".to_string(),
            ));
        }
        if let Some(ref err) = *self.failed.read().await {
            return Err(CoreError::Internal(format!(
                "Embedding model loading failed: {}",
                err
            )));
        }
        let guard = self.inner.read().await;
        match guard.as_ref() {
            Some(provider) => provider.embed_e1_only(content).await,
            None => Err(CoreError::Internal(
                "Embedding provider not available. This is a bug.".to_string(),
            )),
        }
    }

    /// Audit-7 MCP-H1 FIX: Override embed_e5_dual to delegate to real provider.
    async fn embed_e5_dual(&self, content: &str) -> CoreResult<(Vec<f32>, Vec<f32>)> {
        if self.loading.load(Ordering::SeqCst) {
            return Err(CoreError::Internal(
                "Embedding models are still loading. Please wait and try again.".to_string(),
            ));
        }
        if let Some(ref err) = *self.failed.read().await {
            return Err(CoreError::Internal(format!(
                "Embedding model loading failed: {}",
                err
            )));
        }
        let guard = self.inner.read().await;
        match guard.as_ref() {
            Some(provider) => provider.embed_e5_dual(content).await,
            None => Err(CoreError::Internal(
                "Embedding provider not available. This is a bug.".to_string(),
            )),
        }
    }

    /// Audit-7 MCP-H1 FIX: Override embed_e8_dual to delegate to real provider.
    async fn embed_e8_dual(&self, content: &str) -> CoreResult<(Vec<f32>, Vec<f32>)> {
        if self.loading.load(Ordering::SeqCst) {
            return Err(CoreError::Internal(
                "Embedding models are still loading. Please wait and try again.".to_string(),
            ));
        }
        if let Some(ref err) = *self.failed.read().await {
            return Err(CoreError::Internal(format!(
                "Embedding model loading failed: {}",
                err
            )));
        }
        let guard = self.inner.read().await;
        match guard.as_ref() {
            Some(provider) => provider.embed_e8_dual(content).await,
            None => Err(CoreError::Internal(
                "Embedding provider not available. This is a bug.".to_string(),
            )),
        }
    }

    /// Audit-7 MCP-H1 FIX: Override embed_e11_only to delegate to real provider.
    /// Fails closed when E11 is disabled; direct E11 calls must not synthesize a
    /// placeholder vector.
    async fn embed_e11_only(&self, content: &str) -> CoreResult<Vec<f32>> {
        if !E11_ENTITY_ENABLED {
            return Err(CoreError::Internal(
                "E11 Kepler embedder is disabled; direct E11 embedding is unavailable".to_string(),
            ));
        }
        if self.loading.load(Ordering::SeqCst) {
            return Err(CoreError::Internal(
                "Embedding models are still loading. Please wait and try again.".to_string(),
            ));
        }
        if let Some(ref err) = *self.failed.read().await {
            return Err(CoreError::Internal(format!(
                "Embedding model loading failed: {}",
                err
            )));
        }
        let guard = self.inner.read().await;
        match guard.as_ref() {
            Some(provider) => provider.embed_e11_only(content).await,
            None => Err(CoreError::Internal(
                "Embedding provider not available. This is a bug.".to_string(),
            )),
        }
    }
}

// All fields (Arc<RwLock<...>>, Arc<AtomicBool>) are already Send + Sync.
// Removed manual unsafe impls — compiler auto-trait checking is more reliable.

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_returns_loading_error_while_loading() {
        let inner = Arc::new(RwLock::new(None));
        let loading = Arc::new(AtomicBool::new(true));
        let failed = Arc::new(RwLock::new(None));

        let provider = LazyMultiArrayProvider::new(inner, loading, failed);

        let result = provider.embed_all("test").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("still loading"));
    }

    #[tokio::test]
    async fn test_returns_failed_error_after_failure() {
        let inner = Arc::new(RwLock::new(None));
        let loading = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(RwLock::new(Some("GPU not found".to_string())));

        let provider = LazyMultiArrayProvider::new(inner, loading, failed);

        let result = provider.embed_all("test").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("GPU not found"));
    }

    #[test]
    fn test_is_loaded() {
        let inner = Arc::new(RwLock::new(None));
        let loading = Arc::new(AtomicBool::new(true));
        let failed = Arc::new(RwLock::new(None));

        let provider = LazyMultiArrayProvider::new(
            Arc::clone(&inner),
            Arc::clone(&loading),
            Arc::clone(&failed),
        );

        assert!(!provider.is_loaded());

        loading.store(false, Ordering::SeqCst);
        assert!(provider.is_loaded());
    }
}
