//! BgeM3DenseModel implementation.

use std::path::Path;
use std::sync::atomic::Ordering;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::SingleModelConfig;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::gpu_forward::gpu_forward;
use super::types::{BgeM3DenseModel, ModelState};

impl BgeM3DenseModel {
    /// Create a new BgeM3DenseModel instance.
    ///
    /// Model is NOT loaded after construction. Call `load()` before `embed()`.
    ///
    /// # Arguments
    /// * `model_path` - Path to directory containing the BGE-M3 weights:
    ///   - `model.safetensors`
    ///   - `tokenizer.json`
    ///   - `config.json`
    ///   - `sentencepiece.bpe.model` (XLM-R SentencePiece model; referenced by
    ///     `tokenizer.json` and auto-loaded by the `tokenizers` crate).
    /// * `config` - Device placement and quantization settings.
    ///
    /// # Errors
    /// - `EmbeddingError::ConfigError` if config validation fails.
    pub fn new(model_path: &Path, config: SingleModelConfig) -> EmbeddingResult<Self> {
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
        })
    }

    /// Prepare input text for BGE-M3.
    ///
    /// Unlike `SemanticModel` (which prepends `query:` / `passage:`), BGE-M3's
    /// dense retrieval head does NOT require instruction prefixes. The
    /// model author recommends feeding raw text directly, so we just clone
    /// the content string.
    pub(crate) fn prepare_input(&self, input: &ModelInput) -> EmbeddingResult<String> {
        match input {
            ModelInput::Text { content, .. } => Ok(content.clone()),
            ModelInput::Code { .. } => Err(EmbeddingError::UnsupportedModality {
                model_id: ModelId::BgeM3Dense,
                input_type: InputType::Code,
            }),
            ModelInput::Image { .. } => Err(EmbeddingError::UnsupportedModality {
                model_id: ModelId::BgeM3Dense,
                input_type: InputType::Image,
            }),
            ModelInput::Audio { .. } => Err(EmbeddingError::UnsupportedModality {
                model_id: ModelId::BgeM3Dense,
                input_type: InputType::Audio,
            }),
        }
    }

    /// Check if the model is initialised.
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    /// Sequential batch processing. Matches SemanticModel's pattern — a real
    /// batched path would concatenate padded tensors, but that's a separate
    /// optimisation (EMB-L3) that applies to all pretrained models.
    pub async fn embed_batch(&self, inputs: &[ModelInput]) -> EmbeddingResult<Vec<ModelEmbedding>> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::BgeM3Dense,
            });
        }
        let mut out = Vec::with_capacity(inputs.len());
        for input in inputs {
            out.push(self.embed_single(input).await?);
        }
        Ok(out)
    }

    /// Embed a single input (internal).
    pub(crate) async fn embed_single(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        let prepared = self.prepare_input(input)?;
        let start = std::time::Instant::now();

        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("BgeM3DenseModel failed to acquire read lock: {}", e),
            })?;

        let (weights, tokenizer) = match &*state {
            ModelState::Loaded { weights, tokenizer } => (weights, tokenizer),
            _ => {
                return Err(EmbeddingError::NotInitialized {
                    model_id: ModelId::BgeM3Dense,
                });
            }
        };

        let vector = gpu_forward(&prepared, weights, tokenizer)?;
        let latency_us = start.elapsed().as_micros() as u64;

        Ok(ModelEmbedding::new(ModelId::BgeM3Dense, vector, latency_us))
    }
}
