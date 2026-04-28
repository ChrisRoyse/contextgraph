//! Additional ModelRegistry operations.
//!
//! This module contains unload, query, and statistics methods for ModelRegistry.

use std::sync::Arc;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::ModelId;

use super::core::ModelRegistry;
use super::stats::RegistryStats;

impl ModelRegistry {
    /// Unload a model to free memory.
    ///
    /// # Arguments
    /// * `model_id` - The model to unload
    ///
    /// # Returns
    /// - `Ok(())` if unload succeeds
    /// - `Err(EmbeddingError::ModelNotLoaded)` if model not loaded
    /// - `Err(EmbeddingError::ModelInUse)` if callers still hold model handles
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use context_graph_embeddings::models::{ModelRegistry, ModelRegistryConfig};
    /// use context_graph_embeddings::traits::ModelFactory;
    /// use context_graph_embeddings::error::EmbeddingResult;
    /// use context_graph_embeddings::types::ModelId;
    /// use std::sync::Arc;
    ///
    /// async fn example(registry: &ModelRegistry) -> EmbeddingResult<()> {
    ///     // Unload a model (must have been loaded first)
    ///     // registry.unload_model(ModelId::Semantic).await?;
    ///     // assert!(!registry.is_loaded(ModelId::Semantic).await);
    ///     Ok(())
    /// }
    /// ```
    pub async fn unload_model(&self, model_id: ModelId) -> EmbeddingResult<()> {
        let lock = self
            .loading_locks
            .get(&model_id)
            .ok_or_else(|| EmbeddingError::ModelNotFound { model_id })?;
        let _permit = lock
            .acquire()
            .await
            .map_err(|_| EmbeddingError::InternalError {
                message: format!("Semaphore closed for model {:?}", model_id),
            })?;

        // Remove from cache only when the registry is the sole owner. If a
        // caller still holds an Arc from get_model(), unloading would mutate
        // live inference state out from under that caller.
        let model = {
            let mut models = self.models.write().await;
            let model = models
                .get(&model_id)
                .ok_or_else(|| EmbeddingError::ModelNotLoaded { model_id })?;
            let external_refs = Arc::strong_count(model).saturating_sub(1);
            if external_refs > 0 {
                return Err(EmbeddingError::ModelInUse {
                    model_id,
                    ref_count: external_refs,
                });
            }
            models
                .remove(&model_id)
                .ok_or_else(|| EmbeddingError::ModelNotLoaded { model_id })?
        };

        if let Err(e) = model.unload().await {
            let mut models = self.models.write().await;
            models.insert(model_id, model);
            return Err(e);
        }

        // Deallocate memory
        let freed = {
            let mut tracker = self.memory_tracker.write().await;
            tracker.deallocate(model_id)?
        };

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.unload_count += 1;
        }

        tracing::info!(
            model_id = ?model_id,
            freed_bytes = freed,
            "Model unloaded successfully"
        );

        Ok(())
    }

    /// Check if model is currently loaded.
    ///
    /// # Arguments
    /// * `model_id` - The model to check
    ///
    /// # Returns
    /// `true` if model is loaded, `false` otherwise
    pub async fn is_loaded(&self, model_id: ModelId) -> bool {
        let models = self.models.read().await;
        models.contains_key(&model_id)
    }

    /// List all currently loaded models.
    ///
    /// # Returns
    /// Vector of loaded model IDs.
    pub async fn loaded_models(&self) -> Vec<ModelId> {
        let models = self.models.read().await;
        models.keys().copied().collect()
    }

    /// Get total memory usage across all loaded models.
    ///
    /// # Returns
    /// Total bytes allocated.
    pub async fn total_memory_usage(&self) -> usize {
        let tracker = self.memory_tracker.read().await;
        tracker.current_usage()
    }

    /// Get registry statistics snapshot.
    ///
    /// # Returns
    /// Immutable snapshot of current statistics.
    pub async fn stats(&self) -> RegistryStats {
        let models = self.models.read().await;
        let tracker = self.memory_tracker.read().await;
        let internal = self.stats.read().await;

        RegistryStats {
            loaded_count: models.len(),
            total_memory_bytes: tracker.current_usage(),
            load_count: internal.load_count,
            unload_count: internal.unload_count,
            cache_hits: internal.cache_hits,
            load_failures: internal.load_failures,
        }
    }

    /// Get the configured memory budget.
    pub fn memory_budget(&self) -> usize {
        self.config.memory_budget_bytes
    }

    /// Get remaining memory budget.
    pub async fn remaining_memory(&self) -> usize {
        let tracker = self.memory_tracker.read().await;
        tracker.remaining()
    }

    /// Get number of loaded models.
    pub async fn loaded_count(&self) -> usize {
        let models = self.models.read().await;
        models.len()
    }
}

/// Extension methods for testing access to loaded models.
#[cfg(test)]
impl ModelRegistry {
    /// Get a model directly from the cache without loading.
    /// For testing purposes only.
    pub async fn get_cached_model(
        &self,
        model_id: ModelId,
    ) -> Option<std::sync::Arc<dyn crate::traits::EmbeddingModel>> {
        let models = self.models.read().await;
        models.get(&model_id).cloned()
    }
}
