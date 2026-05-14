//! BgeM3DenseModel implementation.

use std::path::Path;
use std::sync::atomic::Ordering;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::{EmbeddingModel, SingleModelConfig};
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::super::bert_batch::{gpu_forward_text_batch, BertBatchPooling, BertBatchSpec};
use super::constants::{BGE_M3_DENSE_MAX_TOKENS, XLM_R_PAD_TOKEN_ID, XLM_R_POSITION_OFFSET};
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

    /// True CUDA batch processing using one padded XLM-R tensor forward pass.
    pub async fn embed_batch(&self, inputs: &[ModelInput]) -> EmbeddingResult<Vec<ModelEmbedding>> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::BgeM3Dense,
            });
        }

        if inputs.is_empty() {
            return Err(EmbeddingError::TrueBatchEmpty {
                model_id: ModelId::BgeM3Dense,
                recovery_hint:
                    "submit at least one BgeM3DenseModel input; empty batches are invalid"
                        .to_string(),
            });
        }

        let mut prepared = Vec::with_capacity(inputs.len());
        for input in inputs {
            self.validate_input(input)?;
            prepared.push(self.prepare_input(input)?);
        }

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

        let vectors = gpu_forward_text_batch(
            &prepared,
            weights,
            tokenizer,
            BertBatchSpec {
                model_id: ModelId::BgeM3Dense,
                model_label: "BgeM3DenseModel",
                max_tokens: BGE_M3_DENSE_MAX_TOKENS,
                position_offset: XLM_R_POSITION_OFFSET,
                position_padding_id: XLM_R_PAD_TOKEN_ID,
                pooling: BertBatchPooling::Cls,
            },
        )?;

        let latency_us = ((start.elapsed().as_micros() as u64) / inputs.len() as u64).max(1);
        vectors
            .into_iter()
            .map(|vector| {
                let embedding = ModelEmbedding::new(ModelId::BgeM3Dense, vector, latency_us);
                embedding.validate()?;
                Ok(embedding)
            })
            .collect()
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
