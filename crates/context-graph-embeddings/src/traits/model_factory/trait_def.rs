//! Model factory trait definition.
//!
//! Defines the factory pattern for creating embedding model instances.

use crate::error::EmbeddingResult;
use crate::traits::EmbeddingModel;
use crate::types::ModelId;

use super::{QuantizationMode, SingleModelConfig};

/// Factory trait for creating embedding model instances.
///
/// This trait abstracts model creation, enabling:
/// - Dependency injection for testing
/// - Configuration-driven model instantiation
/// - Memory estimation before allocation
///
/// # Thread Safety
///
/// Requires `Send + Sync` for concurrent access via `Arc<dyn ModelFactory>`.
///
/// # Lifecycle
///
/// ```text
/// [Factory] --create_model()--> [Unloaded Model] --load()--> [Ready Model]
/// ```
///
/// The factory creates unloaded model instances. Callers must call
/// `EmbeddingModel::load()` before using `embed()`.
///
/// # Example
///
/// ```
/// # use context_graph_embeddings::traits::{SingleModelConfig, get_memory_estimate};
/// # use context_graph_embeddings::types::ModelId;
/// // Check memory requirements using static estimates
/// let model_id = ModelId::Semantic;
/// let memory_needed = get_memory_estimate(model_id);
/// assert!(memory_needed > 0);
/// println!("Model needs {} bytes", memory_needed);
///
/// // Configure model placement
/// let config = SingleModelConfig::cuda_fp16();
/// assert!(config.validate().is_ok());
/// ```
#[async_trait::async_trait]
pub trait ModelFactory: Send + Sync {
    /// Create a model instance for the given ModelId with configuration.
    ///
    /// # Arguments
    /// * `model_id` - The model variant to create (E1-E12)
    /// * `config` - Model-specific configuration (device, quantization, etc.)
    ///
    /// # Returns
    /// A boxed `EmbeddingModel` trait object. The model is **NOT** loaded yet.
    /// Call `model.load().await` before using `embed()`.
    ///
    /// # Errors
    /// - `EmbeddingError::ModelNotFound` if model_id not supported by this factory
    /// - `EmbeddingError::ConfigError` if configuration is invalid
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::traits::SingleModelConfig;
    /// # use context_graph_embeddings::types::ModelId;
    /// // Configure model before creation
    /// let config = SingleModelConfig::cuda_fp16();
    /// let model_id = ModelId::Semantic;
    /// // factory.create_model(model_id, &config) would create unloaded model
    /// assert_eq!(model_id.dimension(), 1024);
    /// ```
    fn create_model(
        &self,
        model_id: ModelId,
        config: &SingleModelConfig,
    ) -> EmbeddingResult<Box<dyn EmbeddingModel>>;

    /// Returns list of ModelIds this factory can create.
    ///
    /// # Returns
    /// Static slice of supported `ModelId` variants.
    /// A full factory supports all 12 models.
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::types::ModelId;
    /// // All 12 model IDs are available
    /// let all_models = ModelId::all();
    /// assert_eq!(all_models.len(), 12);
    /// assert!(all_models.contains(&ModelId::Semantic));
    /// ```
    fn supported_models(&self) -> &[ModelId];

    /// Check if this factory can create the specified model.
    ///
    /// # Arguments
    /// * `model_id` - The model to check
    ///
    /// # Returns
    /// `true` if `create_model()` will succeed for this model_id.
    fn supports_model(&self, model_id: ModelId) -> bool {
        self.supported_models().contains(&model_id)
    }

    /// Estimate memory usage for loading a model.
    ///
    /// Returns a **conservative overestimate** of bytes required.
    /// Actual memory may be lower, but never higher.
    ///
    /// # Arguments
    /// * `model_id` - The model to estimate
    ///
    /// # Returns
    /// Estimated bytes required. Returns 0 only if model_id is unsupported.
    ///
    /// # Memory Estimates (FP32, no quantization)
    ///
    /// | ModelId | Estimate |
    /// |---------|----------|
    /// | Semantic (e5-large) | 1.3 GB |
    /// | TemporalRecent | 10 MB |
    /// | TemporalPeriodic | 10 MB |
    /// | TemporalPositional | 10 MB |
    /// | Causal (Longformer) | 600 MB |
    /// | Sparse (SPLADE) | 500 MB |
    /// | Code (CodeBERT) | 500 MB |
    /// | Graph (MiniLM) | 100 MB |
    /// | Hdc | 50 MB |
    /// | Multimodal (CLIP) | 1.5 GB |
    /// | Entity (MiniLM) | 100 MB |
    /// | LateInteraction (ColBERT) | 400 MB |
    fn estimate_memory(&self, model_id: ModelId) -> usize;

    /// Estimate memory with specific quantization.
    ///
    /// Applies the quantization multiplier to the base estimate.
    ///
    /// # Arguments
    /// * `model_id` - The model to estimate
    /// * `quantization` - The quantization mode to apply
    ///
    /// # Returns
    /// Adjusted memory estimate in bytes.
    fn estimate_memory_quantized(
        &self,
        model_id: ModelId,
        quantization: QuantizationMode,
    ) -> usize {
        let base = self.estimate_memory(model_id);
        (base as f32 * quantization.memory_multiplier()) as usize
    }
}
