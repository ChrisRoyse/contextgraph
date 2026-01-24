//! KeplerModel construction, loading, and batch embedding methods.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{init_gpu, GpuModelLoader};
use crate::traits::{EmbeddingModel, SingleModelConfig};
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::types::{KeplerModel, ModelState, KEPLER_DIMENSION};

impl KeplerModel {
    /// Create a new KeplerModel instance.
    ///
    /// Model is NOT loaded after construction. Call `load()` before `embed()`.
    ///
    /// # Arguments
    /// * `model_path` - Path to directory containing model weights
    /// * `config` - Device placement and quantization settings
    ///
    /// # Errors
    /// - `EmbeddingError::ConfigError` if config validation fails
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
            loaded: AtomicBool::new(false),
            memory_size: 0,
        })
    }

    /// Load model weights into memory.
    ///
    /// # GPU Pipeline
    ///
    /// 1. Initialize CUDA device
    /// 2. Load config.json and tokenizer.json
    /// 3. Load model.safetensors via memory-mapped VarBuilder
    /// 4. Transfer all weight tensors to GPU VRAM
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - GPU initialization fails (no CUDA, driver mismatch)
    /// - Model files missing (config.json, tokenizer.json, model.safetensors)
    /// - Weight loading fails (shape mismatch, corrupt file)
    /// - Insufficient VRAM (~500MB required for FP32)
    pub async fn load(&self) -> EmbeddingResult<()> {
        tracing::info!(
            target: "context_graph_embeddings::kepler",
            model_path = %self.model_path.display(),
            "Loading KeplerModel (RoBERTa-base + TransE)..."
        );

        // Initialize GPU device
        let _device = init_gpu().map_err(|e| {
            tracing::error!(
                target: "context_graph_embeddings::kepler",
                error = %e,
                "KeplerModel GPU initialization FAILED. \
                 Troubleshooting: 1) Verify CUDA drivers installed 2) Check nvidia-smi output 3) Ensure GPU has 1GB+ VRAM"
            );
            EmbeddingError::GpuError {
                message: format!("KeplerModel GPU init failed: {}", e),
            }
        })?;

        // Load tokenizer from model directory
        let tokenizer_path = self.model_path.join("tokenizer.json");
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            EmbeddingError::ModelLoadError {
                model_id: ModelId::Kepler,
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "KeplerModel tokenizer load failed at {}: {}",
                        tokenizer_path.display(),
                        e
                    ),
                )),
            }
        })?;

        // Load BERT/RoBERTa weights from safetensors
        let loader = GpuModelLoader::new().map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel loader init failed: {}", e),
        })?;

        let weights = loader.load_bert_weights(&self.model_path).map_err(|e| {
            EmbeddingError::ModelLoadError {
                model_id: ModelId::Kepler,
                source: Box::new(std::io::Error::other(format!(
                    "KeplerModel weight load failed: {}",
                    e
                ))),
            }
        })?;

        // Validate loaded config matches expected dimensions
        if weights.config.hidden_size != KEPLER_DIMENSION {
            return Err(EmbeddingError::InvalidDimension {
                expected: KEPLER_DIMENSION,
                actual: weights.config.hidden_size,
            });
        }

        tracing::info!(
            "KeplerModel loaded: {} params, {:.2} MB VRAM, hidden_size={}, layers={}",
            weights.param_count(),
            weights.vram_bytes() as f64 / (1024.0 * 1024.0),
            weights.config.hidden_size,
            weights.config.num_hidden_layers
        );

        // Update state
        let mut state = self
            .model_state
            .write()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("KeplerModel failed to acquire write lock: {}", e),
            })?;

        *state = ModelState::Loaded {
            weights: Box::new(weights),
            tokenizer: Box::new(tokenizer),
        };
        self.loaded.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Unload model weights from memory.
    pub async fn unload(&self) -> EmbeddingResult<()> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }

        let mut state = self
            .model_state
            .write()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("Failed to acquire write lock: {}", e),
            })?;

        *state = ModelState::Unloaded;
        self.loaded.store(false, Ordering::SeqCst);
        tracing::info!("KeplerModel unloaded");
        Ok(())
    }

    /// Embed a batch of inputs (more efficient than single embed).
    pub async fn embed_batch(&self, inputs: &[ModelInput]) -> EmbeddingResult<Vec<ModelEmbedding>> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }

        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            results.push(self.embed(input).await?);
        }
        Ok(results)
    }

    /// Extract text content from model input for embedding.
    pub(crate) fn extract_content(input: &ModelInput) -> EmbeddingResult<String> {
        match input {
            ModelInput::Text {
                content,
                instruction,
            } => {
                let mut full = content.clone();
                if let Some(inst) = instruction {
                    full = format!("{} {}", inst, full);
                }
                Ok(full)
            }
            _ => Err(EmbeddingError::UnsupportedModality {
                model_id: ModelId::Kepler,
                input_type: InputType::from(input),
            }),
        }
    }

    /// Check if model is initialized (loaded).
    pub fn is_initialized(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    /// Get the model ID.
    pub fn model_id(&self) -> ModelId {
        ModelId::Kepler
    }
}
