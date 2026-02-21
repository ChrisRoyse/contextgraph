//! Core ModelRegistry struct and initialization.
//!
//! This module contains the main `ModelRegistry` struct definition and its
//! constructor/initialization methods for managing embedding model lifecycle
//! with thread-safe, lazy-loading, memory-aware operations.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{RwLock, Semaphore};

use crate::error::EmbeddingResult;
use crate::traits::{EmbeddingModel, ModelFactory};
use crate::types::ModelId;

use super::super::MemoryTracker;
use super::config::ModelRegistryConfig;
use super::stats::RegistryStatsInternal;

/// Central registry for managing embedding model lifecycle.
///
/// Thread-safe, lazy-loading, memory-aware model management.
///
/// # Thread Safety
///
/// All public methods are safe for concurrent access. The registry uses:
/// - `RwLock<HashMap>` for the model cache
/// - `RwLock<MemoryTracker>` for memory accounting
/// - Per-model `Semaphore` to serialize concurrent load requests
///
/// # Lazy Loading
///
/// Models are loaded on first access via `get_model()`. The per-model
/// semaphore ensures only one load occurs even with concurrent requests.
///
/// # Memory Management
///
/// Before loading, the registry checks if sufficient memory is available.
/// If not, `EmbeddingError::MemoryBudgetExceeded` is returned immediately.
pub struct ModelRegistry {
    /// Currently loaded models (thread-safe access).
    pub(super) models: RwLock<HashMap<ModelId, Arc<dyn EmbeddingModel>>>,

    /// Registry configuration.
    pub(super) config: ModelRegistryConfig,

    /// Per-model loading locks to serialize concurrent load requests.
    /// Each semaphore has 1 permit - only one load at a time per model.
    pub(super) loading_locks: HashMap<ModelId, Arc<Semaphore>>,

    /// Memory usage tracking (thread-safe).
    pub(super) memory_tracker: RwLock<MemoryTracker>,

    /// Factory for creating model instances.
    pub(super) factory: Arc<dyn ModelFactory>,

    /// Statistics counters.
    pub(super) stats: RwLock<RegistryStatsInternal>,
}

impl std::fmt::Debug for ModelRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelRegistry")
            .field("config", &self.config)
            .field("loading_locks_count", &self.loading_locks.len())
            .finish_non_exhaustive()
    }
}

impl ModelRegistry {
    /// Create new registry with configuration and factory.
    ///
    /// # Arguments
    /// * `config` - Registry configuration
    /// * `factory` - Factory for creating model instances
    ///
    /// # Returns
    /// - `Ok(ModelRegistry)` on success
    /// - `Err(EmbeddingError::ConfigError)` if config is invalid
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use context_graph_embeddings::models::{ModelRegistry, ModelRegistryConfig};
    /// use context_graph_embeddings::traits::ModelFactory;
    /// use context_graph_embeddings::error::EmbeddingResult;
    /// use std::sync::Arc;
    ///
    /// async fn example(factory: Arc<dyn ModelFactory>) -> EmbeddingResult<()> {
    ///     let config = ModelRegistryConfig::default();
    ///     let registry = ModelRegistry::new(config, factory).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(
        config: ModelRegistryConfig,
        factory: Arc<dyn ModelFactory>,
    ) -> EmbeddingResult<Self> {
        // FAIL FAST: Validate configuration
        config.validate()?;

        // Create per-model loading locks for all 14 models
        let mut loading_locks = HashMap::new();
        for model_id in ModelId::all() {
            loading_locks.insert(*model_id, Arc::new(Semaphore::new(1)));
        }

        let memory_tracker = MemoryTracker::new(config.memory_budget_bytes);

        tracing::info!(
            memory_budget_bytes = config.memory_budget_bytes,
            max_concurrent_loads = config.max_concurrent_loads,
            preload_count = config.preload_models.len(),
            "ModelRegistry created"
        );

        Ok(Self {
            models: RwLock::new(HashMap::new()),
            config,
            loading_locks,
            memory_tracker: RwLock::new(memory_tracker),
            factory,
            stats: RwLock::new(RegistryStatsInternal::default()),
        })
    }

    /// Initialize registry, preload configured models.
    ///
    /// If preload_models is configured, all specified models are loaded.
    /// **FAILS FAST**: If ANY preload fails, returns error immediately
    /// (no partial success).
    ///
    /// # Returns
    /// - `Ok(())` if all preloads succeed
    /// - `Err(EmbeddingError)` if any preload fails
    pub async fn initialize(&self) -> EmbeddingResult<()> {
        if self.config.preload_models.is_empty() {
            tracing::debug!("No models to preload");
            return Ok(());
        }

        tracing::info!(
            preload_count = self.config.preload_models.len(),
            preload_models = ?self.config.preload_models,
            "Starting model preload"
        );

        for model_id in &self.config.preload_models {
            tracing::debug!(model_id = ?model_id, "Preloading model");

            // FAIL FAST: Any preload failure aborts initialization
            self.load_model(*model_id).await.map_err(|e| {
                tracing::error!(
                    model_id = ?model_id,
                    error = ?e,
                    "Preload FAILED - aborting initialization"
                );
                e
            })?;
        }

        tracing::info!(
            loaded_count = self.config.preload_models.len(),
            "All models preloaded successfully"
        );

        Ok(())
    }
}
