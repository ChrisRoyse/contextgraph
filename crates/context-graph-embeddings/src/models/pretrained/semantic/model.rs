//! SemanticModel implementation.
//!
//! Contains the core methods for the SemanticModel struct.

use std::path::Path;
use std::sync::atomic::Ordering;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::SingleModelConfig;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::constants::{PASSAGE_PREFIX, QUERY_PREFIX};
use super::gpu_forward::gpu_forward;
use super::types::{ModelState, SemanticModel};

impl SemanticModel {
    /// Create a new SemanticModel instance.
    ///
    /// Model is NOT loaded after construction. Call `load()` before `embed()`.
    ///
    /// # Arguments
    /// * `model_path` - Path to directory containing model weights:
    ///   - `model.safetensors` or `pytorch_model.bin`
    ///   - `tokenizer.json`
    ///   - `config.json`
    /// * `config` - Device placement and quantization settings
    ///
    /// # Errors
    /// - `EmbeddingError::ConfigError` if config validation fails
    pub fn new(model_path: &Path, config: SingleModelConfig) -> EmbeddingResult<Self> {
        // Validate config batch size
        if config.max_batch_size == 0 {
            return Err(EmbeddingError::ConfigError {
                message: "max_batch_size cannot be zero".to_string(),
            });
        }

        Ok(Self {
            model_state: std::sync::RwLock::new(ModelState::Unloaded),
            model_path: model_path.to_path_buf(),
            config,
            loaded: std::sync::atomic::AtomicBool::new(false),
            memory_size: 0,
        })
    }

    /// Get the instruction prefix for this model.
    ///
    /// # Arguments
    /// * `is_query` - true for search queries, false for documents
    ///
    /// # Returns
    /// - `"query: "` if is_query is true
    /// - `"passage: "` if is_query is false (default for documents)
    #[inline]
    pub fn instruction_prefix(is_query: bool) -> &'static str {
        if is_query {
            QUERY_PREFIX
        } else {
            PASSAGE_PREFIX
        }
    }

    /// Prepare input text with the appropriate instruction prefix.
    ///
    /// Uses the instruction field from ModelInput::Text if present.
    /// If instruction contains "query", treats as query mode.
    /// Otherwise defaults to passage mode.
    pub(crate) fn prepare_input(&self, input: &ModelInput) -> EmbeddingResult<String> {
        match input {
            ModelInput::Text {
                content,
                instruction,
            } => {
                // Check instruction for query mode indicator
                let is_query = instruction
                    .as_ref()
                    .map(|inst: &String| inst.to_lowercase().contains("query"))
                    .unwrap_or(false);

                let prefix = Self::instruction_prefix(is_query);
                Ok(format!("{}{}", prefix, content))
            }
            ModelInput::Code { .. } => Err(EmbeddingError::UnsupportedModality {
                model_id: ModelId::Semantic,
                input_type: InputType::Code,
            }),
            ModelInput::Image { .. } => Err(EmbeddingError::UnsupportedModality {
                model_id: ModelId::Semantic,
                input_type: InputType::Image,
            }),
            ModelInput::Audio { .. } => Err(EmbeddingError::UnsupportedModality {
                model_id: ModelId::Semantic,
                input_type: InputType::Audio,
            }),
        }
    }

    /// Check if the model is initialized.
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    /// Sequential processing of multiple inputs.
    ///
    /// Note: processes items one at a time (no GPU batching).
    /// For true batch inference, see Kepler model.
    ///
    /// # Arguments
    /// * `inputs` - Slice of ModelInput to embed
    ///
    /// # Errors
    /// - `EmbeddingError::NotInitialized` if model not loaded
    /// - `EmbeddingError::UnsupportedModality` if any input is not Text
    /// - `EmbeddingError::TokenizationError` if tokenization fails
    pub async fn embed_batch(&self, inputs: &[ModelInput]) -> EmbeddingResult<Vec<ModelEmbedding>> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Semantic,
            });
        }

        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            results.push(self.embed_single(input).await?);
        }
        Ok(results)
    }

    /// Embed a single input (internal implementation).
    pub(crate) async fn embed_single(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        // Prepare input with instruction prefix
        let prepared = self.prepare_input(input)?;

        let start = std::time::Instant::now();

        // Get loaded weights and tokenizer
        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("SemanticModel failed to acquire read lock: {}", e),
            })?;

        let (weights, tokenizer) = match &*state {
            ModelState::Loaded { weights, tokenizer } => (weights, tokenizer),
            _ => {
                return Err(EmbeddingError::NotInitialized {
                    model_id: ModelId::Semantic,
                });
            }
        };

        // Run GPU-accelerated BERT forward pass
        let vector = gpu_forward(&prepared, weights, tokenizer)?;

        let latency_us = start.elapsed().as_micros() as u64;

        Ok(ModelEmbedding::new(ModelId::Semantic, vector, latency_us))
    }
}
