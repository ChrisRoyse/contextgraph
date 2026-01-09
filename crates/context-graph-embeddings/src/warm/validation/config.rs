//! Test inference configuration types.
//!
//! Defines the expected behavior and inputs for validating a model
//! after loading weights into VRAM.

/// Configuration for test inference validation.
///
/// Defines the expected behavior and inputs for validating a model
/// after loading weights into VRAM.
///
/// # Example
///
/// ```rust,ignore
/// let config = TestInferenceConfig::for_embedding_model("E1_Semantic", 1024);
/// let result = validator.validate_model(&config, &handle, &output);
/// ```
#[derive(Debug, Clone)]
pub struct TestInferenceConfig {
    /// Model identifier (e.g., "E1_Semantic").
    pub model_id: String,
    /// Expected output dimension from the model.
    pub expected_dimension: usize,
    /// Test input for inference validation.
    pub test_input: TestInput,
    /// Optional reference output for comparison validation.
    /// If provided, actual output must match within tolerance.
    pub reference_output: Option<Vec<f32>>,
    /// Maximum allowed inference time in milliseconds.
    /// Exceeding this is a validation failure.
    pub max_inference_ms: u64,
}

impl TestInferenceConfig {
    /// Create a test inference config for an embedding model.
    ///
    /// Uses a standard test sentence for text-based embedding models.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Model identifier (e.g., "E1_Semantic")
    /// * `expected_dimension` - Expected output embedding dimension
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = TestInferenceConfig::for_embedding_model("E1_Semantic", 1024);
    /// assert_eq!(config.expected_dimension, 1024);
    /// ```
    #[must_use]
    pub fn for_embedding_model(model_id: &str, expected_dimension: usize) -> Self {
        Self {
            model_id: model_id.to_string(),
            expected_dimension,
            test_input: TestInput::Text("The quick brown fox jumps over the lazy dog.".to_string()),
            reference_output: None,
            max_inference_ms: 1000, // 1 second timeout for embedding
        }
    }

    /// Create a config with a reference output for strict validation.
    ///
    /// The actual inference output must match the reference within tolerance.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Model identifier
    /// * `expected_dimension` - Expected output dimension
    /// * `test_input` - Input for test inference
    /// * `reference_output` - Expected output values
    /// * `max_inference_ms` - Maximum inference time
    #[must_use]
    pub fn with_reference(
        model_id: &str,
        expected_dimension: usize,
        test_input: TestInput,
        reference_output: Vec<f32>,
        max_inference_ms: u64,
    ) -> Self {
        Self {
            model_id: model_id.to_string(),
            expected_dimension,
            test_input,
            reference_output: Some(reference_output),
            max_inference_ms,
        }
    }
}

/// Input types for test inference validation.
///
/// Different models accept different input formats. This enum
/// represents the supported input types for validation.
#[derive(Debug, Clone)]
pub enum TestInput {
    /// Raw text input for text embedding models.
    Text(String),
    /// Tokenized input (e.g., BERT token IDs).
    Tokens(Vec<u32>),
    /// Pre-computed embeddings (for projection models).
    Embeddings(Vec<f32>),
}

impl TestInput {
    /// Get a human-readable description of the input type.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Tokens(_) => "tokens",
            Self::Embeddings(_) => "embeddings",
        }
    }

    /// Get the input length (characters, tokens, or embedding dimensions).
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Text(s) => s.len(),
            Self::Tokens(t) => t.len(),
            Self::Embeddings(e) => e.len(),
        }
    }

    /// Check if the input is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
