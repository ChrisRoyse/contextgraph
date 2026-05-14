//! LateInteractionModel struct and core implementation.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use candle_core::DType;
use candle_nn::VarBuilder;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{init_gpu, GpuModelLoader};
use crate::traits::SingleModelConfig;
use crate::types::ModelId;

use super::types::{ColBertProjection, ModelState, LATE_INTERACTION_DIMENSION};

/// Late-interaction embedding model using colbert-ir/colbertv2.0.
///
/// This model produces per-token 128D vectors enabling fine-grained
/// matching via MaxSim scoring.
///
/// # Architecture
///
/// ColBERT is a BERT-based model with a linear projection layer that
/// reduces token embeddings from 768D to 128D. Unlike single-vector
/// models, it preserves all token embeddings for late interaction.
///
/// # ColBERT-Specific Features
///
/// - **embed_tokens**: Produces per-token 128D embeddings
/// - **pool_tokens**: Mean pooling to single 128D for fusion
/// - **maxsim_score**: ColBERT MaxSim scoring for retrieval
/// - **batch_maxsim**: Efficient batch scoring
///
/// # Construction
///
/// ```rust,no_run
/// use context_graph_embeddings::models::LateInteractionModel;
/// use context_graph_embeddings::traits::SingleModelConfig;
/// use context_graph_embeddings::error::EmbeddingResult;
/// use std::path::Path;
///
/// async fn example() -> EmbeddingResult<()> {
///     let model = LateInteractionModel::new(
///         Path::new("models/late-interaction"),
///         SingleModelConfig::default(),
///     )?;
///     model.load().await?;  // Must load before embed
///     Ok(())
/// }
/// ```
pub struct LateInteractionModel {
    /// Model weights and inference engine.
    #[allow(dead_code)]
    pub(crate) model_state: std::sync::RwLock<ModelState>,

    /// Path to model weights directory.
    #[allow(dead_code)]
    pub(crate) model_path: PathBuf,

    /// Configuration for this model instance.
    #[allow(dead_code)]
    pub(crate) config: SingleModelConfig,

    /// Whether model weights are loaded and ready.
    pub(crate) loaded: AtomicBool,
}

impl LateInteractionModel {
    /// Create a new LateInteractionModel instance.
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
        })
    }

    /// Load model weights into memory (GPU-accelerated).
    ///
    /// # GPU Pipeline
    ///
    /// 1. Initialize CUDA device
    /// 2. Load config.json and tokenizer.json
    /// 3. Load model.safetensors via memory-mapped VarBuilder
    /// 4. Load ColBERT projection layer (768D -> 128D)
    /// 5. Transfer all weight tensors to GPU VRAM
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - GPU initialization fails (no CUDA, driver mismatch)
    /// - Model files missing (config.json, tokenizer.json, model.safetensors)
    /// - Weight loading fails (shape mismatch, corrupt file)
    /// - Insufficient VRAM (~440MB required for FP32)
    pub async fn load(&self) -> EmbeddingResult<()> {
        use tokenizers::Tokenizer;

        // Initialize GPU device
        let device = init_gpu().map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel GPU init failed: {}", e),
        })?;

        // Load tokenizer from model directory
        let tokenizer_path = self.model_path.join("tokenizer.json");
        let mut tokenizer =
            Tokenizer::from_file(&tokenizer_path).map_err(|e| EmbeddingError::ModelLoadError {
                model_id: ModelId::LateInteraction,
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "Tokenizer load failed at {}: {}",
                        tokenizer_path.display(),
                        e
                    ),
                )),
            })?;
        tokenizer
            .with_truncation(None)
            .map_err(|e| EmbeddingError::ModelLoadError {
                model_id: ModelId::LateInteraction,
                source: Box::new(std::io::Error::other(format!(
                    "LateInteractionModel disable tokenizer truncation failed: {e}"
                ))),
            })?;

        // Load BERT weights from safetensors
        let loader = GpuModelLoader::new().map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel loader init failed: {}", e),
        })?;

        // ColBERT uses "bert." prefix for all weights
        let weights = loader
            .load_bert_weights_with_prefix(&self.model_path, "bert.")
            .map_err(|e| EmbeddingError::ModelLoadError {
                model_id: ModelId::LateInteraction,
                source: Box::new(std::io::Error::other(format!(
                    "LateInteractionModel weight load failed: {}",
                    e
                ))),
            })?;

        // Validate loaded config matches expected dimensions
        // ColBERT uses BERT-base with hidden_size=768
        const COLBERT_HIDDEN_SIZE: usize = 768;
        if weights.config.hidden_size != COLBERT_HIDDEN_SIZE {
            return Err(EmbeddingError::InvalidDimension {
                expected: COLBERT_HIDDEN_SIZE,
                actual: weights.config.hidden_size,
            });
        }

        // Load ColBERT projection layer (768D -> 128D) directly from safetensors
        let safetensors_path = self.model_path.join("model.safetensors");
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[&safetensors_path], DType::F32, device).map_err(
                |e| EmbeddingError::ModelLoadError {
                    model_id: ModelId::LateInteraction,
                    source: Box::new(std::io::Error::other(format!(
                        "Failed to load safetensors for projection: {}",
                        e
                    ))),
                },
            )?
        };

        // Load linear projection: [128, 768]
        let projection_weight = vb
            .get(
                &[LATE_INTERACTION_DIMENSION, COLBERT_HIDDEN_SIZE],
                "linear.weight",
            )
            .map_err(|e| EmbeddingError::ModelLoadError {
                model_id: ModelId::LateInteraction,
                source: Box::new(std::io::Error::other(format!(
                    "Failed to load linear.weight: {}",
                    e
                ))),
            })?;

        let projection = ColBertProjection {
            weight: projection_weight,
        };

        tracing::info!(
            "LateInteractionModel loaded: {} params, {:.2} MB VRAM, hidden_size={}, projection=[{},{}]",
            weights.param_count(),
            weights.vram_bytes() as f64 / (1024.0 * 1024.0),
            weights.config.hidden_size,
            LATE_INTERACTION_DIMENSION,
            COLBERT_HIDDEN_SIZE
        );

        // Update state
        let mut state = self
            .model_state
            .write()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("Failed to acquire write lock: {}", e),
            })?;

        *state = ModelState::Loaded {
            weights: Box::new(weights),
            projection,
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
        tracing::info!("LateInteractionModel unloaded");
        Ok(())
    }

    /// Check if model is initialized.
    pub fn is_initialized(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    /// Get the model ID.
    pub fn model_id(&self) -> ModelId {
        ModelId::LateInteraction
    }
}

// SAFETY: RwLock provides interior mutability with proper synchronization
unsafe impl Send for LateInteractionModel {}
unsafe impl Sync for LateInteractionModel {}
