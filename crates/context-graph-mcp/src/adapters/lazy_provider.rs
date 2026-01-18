//! Lazy MultiArrayEmbeddingProvider that allows immediate MCP startup.
//!
//! This adapter wraps an optional provider that loads in the background,
//! returning "models loading" errors until the real provider is ready.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::RwLock;

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::traits::{MultiArrayEmbeddingOutput, MultiArrayEmbeddingProvider};
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;

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

    async fn embed_batch_all(
        &self,
        contents: &[String],
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
            Some(provider) => provider.embed_batch_all(contents).await,
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
        ]
    }

    fn is_ready(&self) -> bool {
        // Only ready if loading is complete and succeeded
        if self.loading.load(Ordering::SeqCst) {
            return false;
        }
        // Can't check failed synchronously, so assume ready if not loading
        // The actual check happens in embed_all()
        true
    }

    fn health_status(&self) -> [bool; NUM_EMBEDDERS] {
        // If loading, all are "not ready"
        if self.loading.load(Ordering::SeqCst) {
            return [false; NUM_EMBEDDERS];
        }
        // Otherwise, assume healthy (actual check in embed_all)
        [true; NUM_EMBEDDERS]
    }
}

// Safety: LazyMultiArrayProvider is Send + Sync because all fields are Arc<...>
unsafe impl Send for LazyMultiArrayProvider {}
unsafe impl Sync for LazyMultiArrayProvider {}

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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("still loading"));
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
