//! ModelFactory trait implementation for DefaultModelFactory.

use crate::error::EmbeddingResult;
use crate::traits::{get_memory_estimate, EmbeddingModel, ModelFactory, SingleModelConfig};
use crate::types::ModelId;

use super::DefaultModelFactory;

// ============================================================================
// MODEL FACTORY TRAIT IMPLEMENTATION
// ============================================================================

impl ModelFactory for DefaultModelFactory {
    /// Create a model instance for the given ModelId.
    ///
    /// Returns an unloaded model. Call `model.load().await` before `embed()`.
    ///
    /// # Arguments
    /// * `model_id` - Which model to create (E1-E13)
    /// * `config` - Model-specific configuration
    ///
    /// # Returns
    /// Unloaded model instance as `Box<dyn EmbeddingModel>`.
    ///
    /// # Errors
    /// - `EmbeddingError::ConfigError` if config validation fails
    /// - `EmbeddingError::ModelNotFound` if model_id is unknown
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::models::DefaultModelFactory;
    /// use context_graph_embeddings::config::GpuConfig;
    /// use context_graph_embeddings::traits::{ModelFactory, SingleModelConfig};
    /// use context_graph_embeddings::types::ModelId;
    /// use std::path::PathBuf;
    ///
    /// let factory = DefaultModelFactory::new(
    ///     PathBuf::from("./models"),
    ///     GpuConfig::default(),
    /// );
    /// let config = SingleModelConfig::cuda_fp16();
    ///
    /// // Verify factory supports all 14 models (E1-E13 + Kepler)
    /// assert!(factory.supported_models().contains(&ModelId::Semantic));
    /// assert_eq!(factory.supported_models().len(), 14);
    ///
    /// // Estimate memory before creating
    /// let memory = factory.estimate_memory(ModelId::Semantic);
    /// assert!(memory > 0);
    ///
    /// // create_model returns unloaded model (requires model files)
    /// // let model = factory.create_model(ModelId::Semantic, &config)?;
    /// ```
    fn create_model(
        &self,
        model_id: ModelId,
        config: &SingleModelConfig,
    ) -> EmbeddingResult<Box<dyn EmbeddingModel>> {
        // FAIL FAST: Validate config first
        config.validate()?;

        // Dispatch to appropriate creator based on model type
        match model_id {
            // Pretrained models (require model files)
            ModelId::Semantic
            | ModelId::Causal
            | ModelId::Sparse
            | ModelId::Code
            | ModelId::Graph
            | ModelId::Contextual
            | ModelId::Entity
            | ModelId::Kepler
            | ModelId::LateInteraction
            | ModelId::Splade => self.create_pretrained_model(model_id, config),

            // Custom models (lightweight, no model files)
            ModelId::TemporalRecent
            | ModelId::TemporalPeriodic
            | ModelId::TemporalPositional
            | ModelId::Hdc => self.create_custom_model(model_id, config),
        }
    }

    /// Returns all 12 supported model IDs.
    ///
    /// The factory supports all models defined in the constitution:
    /// - E1-E12 covering semantic, temporal, code, graph, and multimodal embeddings.
    fn supported_models(&self) -> &[ModelId] {
        ModelId::all()
    }

    /// Estimate memory usage for loading a model.
    ///
    /// Returns conservative overestimate in bytes.
    /// Actual memory may be lower, never higher.
    ///
    /// # Memory Estimates (FP32)
    ///
    /// | ModelId | Estimate |
    /// |---------|----------|
    /// | Semantic | 1.4 GB |
    /// | TemporalRecent | 15 MB |
    /// | TemporalPeriodic | 15 MB |
    /// | TemporalPositional | 15 MB |
    /// | Causal | 650 MB |
    /// | Sparse | 550 MB |
    /// | Code | 550 MB |
    /// | Graph | 120 MB |
    /// | Hdc | 60 MB |
    /// | Multimodal | 1.6 GB |
    /// | Entity | 120 MB |
    /// | LateInteraction | 450 MB |
    fn estimate_memory(&self, model_id: ModelId) -> usize {
        get_memory_estimate(model_id)
    }
}
