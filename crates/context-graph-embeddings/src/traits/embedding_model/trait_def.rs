//! Core trait definition for embedding models.

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};
use async_trait::async_trait;

/// Core trait for embedding model implementations.
///
/// All 12 embedding models in the Multi-Array Storage pipeline must implement this trait.
/// The trait provides a unified async interface for:
/// - Querying model capabilities (ID, supported input types, dimensions)
/// - Generating embeddings from multi-modal inputs
/// - Managing model lifecycle (initialization state)
///
/// # Thread Safety
///
/// This trait requires `Send + Sync` bounds, ensuring implementations
/// can be safely shared across async tasks and threads.
///
/// # Error Handling
///
/// All methods that can fail return `EmbeddingResult<T>`. Implementations
/// must return appropriate error variants:
/// - `EmbeddingError::UnsupportedModality` for incompatible input types
/// - `EmbeddingError::NotInitialized` if model not ready
/// - `EmbeddingError::EmptyInput` for empty content
/// - Other variants as appropriate
///
/// # Example
///
/// ```
/// # use context_graph_embeddings::types::{ModelInput, InputType, ModelId};
/// // Check model capabilities using ModelId
/// let model_id = ModelId::Semantic;
/// assert_eq!(model_id.dimension(), 1024);
/// assert!(model_id.is_pretrained());
///
/// // Create inputs for embedding
/// let input = ModelInput::text("Hello, world!").unwrap();
/// assert!(matches!(InputType::from(&input), InputType::Text));
/// ```
#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    /// Returns the unique identifier for this model.
    ///
    /// Each implementation returns one of the 12 `ModelId` variants (E1-E12).
    /// This ID is used for:
    /// - Routing inputs to appropriate models
    /// - Validating embedding dimensions
    /// - Logging and debugging
    ///
    /// # Returns
    /// The `ModelId` variant identifying this model.
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::types::ModelId;
    /// // ModelId identifies each of the 14 models
    /// let model_id = ModelId::Semantic;
    /// assert_eq!(model_id.dimension(), 1024);
    /// ```
    fn model_id(&self) -> ModelId;

    /// Returns the list of input types this model supports.
    ///
    /// Models should only list types they are designed to handle well.
    /// Attempting to embed an unsupported type should return
    /// `EmbeddingError::UnsupportedModality`.
    ///
    /// # Returns
    /// Static slice of supported `InputType` variants.
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::types::ModelId;
    /// // Query model properties via ModelId
    /// let multimodal = ModelId::Contextual;
    /// assert_eq!(multimodal.dimension(), 768);
    /// assert!(multimodal.is_pretrained());
    /// ```
    fn supported_input_types(&self) -> &[InputType];

    /// Generate an embedding for the given input.
    ///
    /// This is the core embedding generation method. Implementations must:
    /// 1. Validate the input type is supported
    /// 2. Process the input through the model
    /// 3. Return a properly dimensioned `ModelEmbedding`
    ///
    /// # Arguments
    /// * `input` - The input to embed (text, code, image, or audio)
    ///
    /// # Returns
    /// - `Ok(ModelEmbedding)` with the generated embedding vector
    /// - `Err(EmbeddingError)` on failure
    ///
    /// # Errors
    /// - `EmbeddingError::UnsupportedModality` if input type not supported
    /// - `EmbeddingError::NotInitialized` if model not initialized
    /// - `EmbeddingError::EmptyInput` if input content is empty
    /// - `EmbeddingError::InputTooLong` if input exceeds max tokens
    /// - `EmbeddingError::Timeout` if processing exceeds time budget
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::types::{ModelInput, ModelId};
    /// // Create input for embedding
    /// let input = ModelInput::text("Hello, world!").unwrap();
    /// // Model dimension is known from ModelId
    /// assert_eq!(ModelId::Semantic.dimension(), 1024);
    /// ```
    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding>;

    /// Returns whether the model is initialized and ready for inference.
    ///
    /// Models may require initialization (loading weights, warming up GPU)
    /// before they can process inputs. This method allows checking readiness.
    ///
    /// # Returns
    /// - `true` if model is ready for `embed()` calls
    /// - `false` if model needs initialization
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::error::EmbeddingError;
    /// # use context_graph_embeddings::types::ModelId;
    /// // Check initialization state before embedding
    /// let model_id = ModelId::Semantic;
    /// let is_init = false; // Would come from model.is_initialized()
    /// if !is_init {
    ///     let err = EmbeddingError::NotInitialized { model_id };
    ///     assert!(matches!(err, EmbeddingError::NotInitialized { .. }));
    /// }
    /// ```
    fn is_initialized(&self) -> bool;

    /// Load model weights and initialize for inference.
    ///
    /// This method prepares the model for embedding generation by:
    /// - Loading pretrained weights from disk (for pretrained models)
    /// - Initializing GPU/CPU compute resources
    /// - Setting up tokenizers and preprocessors
    ///
    /// For custom models (Temporal*, HDC) that don't require external weights,
    /// the default implementation returns `Ok(())` immediately.
    ///
    /// # Returns
    /// - `Ok(())` if model loaded successfully
    /// - `Err(EmbeddingError)` on failure
    ///
    /// # Errors
    /// - `EmbeddingError::ModelNotFound` if weights file doesn't exist
    /// - `EmbeddingError::GpuError` if GPU initialization fails
    /// - `EmbeddingError::ConfigError` if model config is invalid
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::types::ModelId;
    /// // Create model and load before embedding
    /// // let model = factory.create_model(ModelId::Semantic, &config)?;
    /// // model.load().await?;  // MUST call before embed()
    /// // let embedding = model.embed(&input).await?;
    /// ```
    async fn load(&self) -> EmbeddingResult<()> {
        // Default: no-op for custom models that don't need loading
        // Pretrained models override this with actual weight loading
        Ok(())
    }

    // =========================================================================
    // Default implementations
    // =========================================================================

    /// Check if this model supports the given input type.
    ///
    /// This is a convenience method that checks if the input type
    /// is in the list returned by `supported_input_types()`.
    ///
    /// # Arguments
    /// * `input_type` - The input type to check
    ///
    /// # Returns
    /// `true` if the model supports this input type
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::types::ModelId;
    /// let model_id = ModelId::Contextual;
    /// // Check model properties
    /// assert_eq!(model_id.dimension(), 768);
    /// assert!(model_id.is_pretrained());
    /// ```
    fn supports_input_type(&self, input_type: InputType) -> bool {
        self.supported_input_types().contains(&input_type)
    }

    /// Returns the native output dimension for this model.
    ///
    /// This is a convenience method that delegates to `ModelId::dimension()`.
    /// Returns the raw model output size before any projection.
    ///
    /// # Returns
    /// The embedding dimension for this model.
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::types::ModelId;
    /// let dim = ModelId::Semantic.dimension();
    /// assert_eq!(dim, 1024); // Semantic model dimension
    /// ```
    fn dimension(&self) -> usize {
        self.model_id().dimension()
    }

    /// Returns the projected dimension for Multi-Array Storage.
    ///
    /// Some models (Sparse, Code, HDC) project their outputs to
    /// different dimensions for per-space storage.
    ///
    /// # Returns
    /// The projected dimension for Multi-Array Storage.
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::types::ModelId;
    /// let proj_dim = ModelId::Sparse.projected_dimension();
    /// assert_eq!(proj_dim, 1536); // Sparse model projected dimension
    /// ```
    fn projected_dimension(&self) -> usize {
        self.model_id().projected_dimension()
    }

    /// Returns the latency budget in milliseconds for this model.
    ///
    /// Each model has a performance target from constitution.yaml.
    /// Implementations should aim to complete within this budget.
    ///
    /// # Returns
    /// Maximum expected latency in milliseconds.
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::types::ModelId;
    /// let budget = ModelId::Semantic.latency_budget_ms();
    /// assert_eq!(budget, 5); // Semantic: 5ms budget
    /// ```
    fn latency_budget_ms(&self) -> u32 {
        self.model_id().latency_budget_ms()
    }

    /// Validate that the input is compatible with this model.
    ///
    /// Checks the input type against supported types and returns
    /// an appropriate error if incompatible.
    ///
    /// # Arguments
    /// * `input` - The input to validate
    ///
    /// # Returns
    /// - `Ok(())` if input is compatible
    /// - `Err(EmbeddingError::UnsupportedModality)` if not supported
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::types::{ModelInput, InputType, ModelId};
    /// let input = ModelInput::text("Test").unwrap();
    /// let input_type = InputType::from(&input);
    /// let model_id = ModelId::Semantic;
    /// // Check input type and model properties
    /// assert!(matches!(input_type, InputType::Text));
    /// assert_eq!(model_id.dimension(), 1024);
    /// ```
    fn validate_input(&self, input: &ModelInput) -> EmbeddingResult<()> {
        let input_type = InputType::from(input);
        if self.supports_input_type(input_type) {
            Ok(())
        } else {
            Err(EmbeddingError::UnsupportedModality {
                model_id: self.model_id(),
                input_type,
            })
        }
    }

    /// Returns the maximum input token count for this model.
    ///
    /// Convenience method delegating to `ModelId::max_tokens()`.
    ///
    /// # Returns
    /// Maximum token count (varies by model: 77-4096, or MAX for custom)
    fn max_tokens(&self) -> usize {
        self.model_id().max_tokens()
    }

    /// Returns whether this model uses pretrained weights.
    ///
    /// Convenience method delegating to `ModelId::is_pretrained()`.
    ///
    /// # Returns
    /// - `true` for models with HuggingFace weights (8 models)
    /// - `false` for custom implementations (Temporal*, HDC)
    fn is_pretrained(&self) -> bool {
        self.model_id().is_pretrained()
    }

    /// Generate a sparse embedding for the given input.
    ///
    /// This method is only implemented by sparse models (E6, E13). Other models
    /// return `EmbeddingError::UnsupportedModality`.
    ///
    /// # Arguments
    /// * `input` - The input to embed (text only for SPLADE models)
    ///
    /// # Returns
    /// - `Ok((indices, values))` with the sparse representation
    /// - `Err(EmbeddingError)` on failure or for non-sparse models
    ///
    /// # Errors
    /// - `EmbeddingError::UnsupportedModality` for models that don't support sparse output
    /// - `EmbeddingError::NotInitialized` if model not initialized
    async fn embed_sparse(&self, input: &ModelInput) -> EmbeddingResult<(Vec<u16>, Vec<f32>)> {
        // Default: sparse embedding not supported
        Err(EmbeddingError::UnsupportedModality {
            model_id: self.model_id(),
            input_type: InputType::from(input),
        })
    }
}
