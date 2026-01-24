//! DefaultModelFactory struct and core implementation methods.

use std::path::PathBuf;

use crate::config::GpuConfig;
use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::{EmbeddingModel, SingleModelConfig};
use crate::types::ModelId;

// Import all model types
use crate::models::custom::{
    HdcModel, TemporalPeriodicModel, TemporalPositionalModel, TemporalRecentModel,
};
use crate::models::pretrained::{
    CausalModel, CodeModel, EntityModel, GraphModel, KeplerModel, LateInteractionModel,
    MultimodalModel, SemanticModel, SparseModel,
};

// ============================================================================
// DEFAULT MODEL FACTORY
// ============================================================================

/// Production factory for creating all 12 embedding models.
///
/// The factory handles model instantiation with proper configuration.
/// Models are created in unloaded state - call `load()` before `embed()`.
///
/// # Thread Safety
///
/// `DefaultModelFactory` is `Send + Sync` for safe concurrent access.
/// The factory itself is immutable after construction.
///
/// # Memory Management
///
/// Use `estimate_memory()` to check memory requirements before loading.
/// Memory estimates are conservative (actual usage may be lower).
///
/// # Example
///
/// ```rust
/// use context_graph_embeddings::models::DefaultModelFactory;
/// use context_graph_embeddings::config::GpuConfig;
/// use context_graph_embeddings::traits::ModelFactory;
/// use context_graph_embeddings::types::ModelId;
/// use std::path::PathBuf;
///
/// let factory = DefaultModelFactory::new(
///     PathBuf::from("./models"),
///     GpuConfig::default(),
/// );
///
/// // Check memory before creating
/// let memory_needed = factory.estimate_memory(ModelId::Semantic);
/// assert!(memory_needed > 0);
/// println!("Semantic model needs {} bytes", memory_needed);
/// ```
#[derive(Debug, Clone)]
pub struct DefaultModelFactory {
    /// Base directory containing pretrained model files.
    /// Each model expects a subdirectory named after its HuggingFace repo.
    pub(crate) models_dir: PathBuf,

    /// GPU configuration for model inference.
    /// Controls device placement, memory limits, and optimization features.
    pub(crate) gpu_config: GpuConfig,
}

impl DefaultModelFactory {
    /// Create a new DefaultModelFactory.
    ///
    /// # Arguments
    /// * `models_dir` - Base directory for pretrained model files
    /// * `gpu_config` - GPU configuration for inference
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::models::DefaultModelFactory;
    /// use context_graph_embeddings::config::GpuConfig;
    /// use std::path::PathBuf;
    ///
    /// let factory = DefaultModelFactory::new(
    ///     PathBuf::from("./models"),
    ///     GpuConfig::default(),
    /// );
    /// ```
    #[must_use]
    pub fn new(models_dir: PathBuf, gpu_config: GpuConfig) -> Self {
        Self {
            models_dir,
            gpu_config,
        }
    }

    /// Get the models directory path.
    #[must_use]
    pub fn models_dir(&self) -> &PathBuf {
        &self.models_dir
    }

    /// Get the GPU configuration.
    #[must_use]
    pub fn gpu_config(&self) -> &GpuConfig {
        &self.gpu_config
    }

    /// Get the model-specific subdirectory path.
    ///
    /// For pretrained models, returns the path to their model files.
    /// Custom models don't need model files, so this returns None.
    pub(crate) fn get_model_path(&self, model_id: ModelId) -> Option<PathBuf> {
        // Model subdirectory names match actual ./models/ structure
        let subdir = match model_id {
            ModelId::Semantic => "semantic",
            ModelId::Causal => "causal",
            ModelId::Sparse => "sparse",
            ModelId::Code => "code-1536",
            ModelId::Graph => "graph",
            ModelId::Multimodal => "multimodal",
            ModelId::Entity => "entity",
            ModelId::Kepler => "kepler",
            ModelId::LateInteraction => "late-interaction",
            ModelId::Splade => "splade-v3",
            // Custom models don't need model files
            ModelId::TemporalRecent
            | ModelId::TemporalPeriodic
            | ModelId::TemporalPositional
            | ModelId::Hdc => return None,
        };
        Some(self.models_dir.join(subdir))
    }

    /// Create a pretrained model that requires model files.
    pub(crate) fn create_pretrained_model(
        &self,
        model_id: ModelId,
        config: &SingleModelConfig,
    ) -> EmbeddingResult<Box<dyn EmbeddingModel>> {
        let model_path =
            self.get_model_path(model_id)
                .ok_or_else(|| EmbeddingError::ConfigError {
                    message: format!("Model {:?} is a custom model, not pretrained", model_id),
                })?;

        match model_id {
            ModelId::Semantic => {
                let model = SemanticModel::new(&model_path, config.clone())?;
                Ok(Box::new(model))
            }
            ModelId::Causal => {
                let model = CausalModel::new(&model_path, config.clone())?;
                Ok(Box::new(model))
            }
            ModelId::Sparse => {
                let model = SparseModel::new(&model_path, config.clone())?;
                Ok(Box::new(model))
            }
            ModelId::Code => {
                let model = CodeModel::new(&model_path, config.clone())?;
                Ok(Box::new(model))
            }
            ModelId::Graph => {
                let model = GraphModel::new(&model_path, config.clone())?;
                Ok(Box::new(model))
            }
            ModelId::Multimodal => {
                let model = MultimodalModel::new(&model_path, config.clone())?;
                Ok(Box::new(model))
            }
            ModelId::Entity => {
                let model = EntityModel::new(&model_path, config.clone())?;
                Ok(Box::new(model))
            }
            ModelId::Kepler => {
                let model = KeplerModel::new(&model_path, config.clone())?;
                Ok(Box::new(model))
            }
            ModelId::LateInteraction => {
                let model = LateInteractionModel::new(&model_path, config.clone())?;
                Ok(Box::new(model))
            }
            ModelId::Splade => {
                // E13 Splade uses same architecture as E6 Sparse (both SPLADE-based)
                // Use new_splade() to set correct model_id
                let model = SparseModel::new_splade(&model_path, config.clone())?;
                Ok(Box::new(model))
            }
            _ => Err(EmbeddingError::ConfigError {
                message: format!("Model {:?} is not a pretrained model", model_id),
            }),
        }
    }

    /// Create a custom model that doesn't need model files.
    pub(crate) fn create_custom_model(
        &self,
        model_id: ModelId,
        _config: &SingleModelConfig,
    ) -> EmbeddingResult<Box<dyn EmbeddingModel>> {
        match model_id {
            ModelId::TemporalRecent => {
                let model = TemporalRecentModel::new();
                Ok(Box::new(model))
            }
            ModelId::TemporalPeriodic => {
                let model = TemporalPeriodicModel::new();
                Ok(Box::new(model))
            }
            ModelId::TemporalPositional => {
                let model = TemporalPositionalModel::new();
                Ok(Box::new(model))
            }
            ModelId::Hdc => {
                // HDC model with default ngram_size=3 and seed=42
                let model = HdcModel::new(3, 42)?;
                Ok(Box::new(model))
            }
            _ => Err(EmbeddingError::ConfigError {
                message: format!("Model {:?} is not a custom model", model_id),
            }),
        }
    }
}
