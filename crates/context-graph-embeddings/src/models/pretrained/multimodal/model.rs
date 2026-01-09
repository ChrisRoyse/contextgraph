//! MultimodalModel implementation.
//!
//! Main model struct for CLIP multimodal embeddings with GPU-accelerated inference.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use candle_core::DType;
use candle_nn::VarBuilder;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::init_gpu;
use crate::traits::{EmbeddingModel, SingleModelConfig};
use crate::types::{ModelEmbedding, ModelId, ModelInput};

use super::image_processor::ImageProcessor;
use super::weights::ClipWeights;

/// Internal state that varies based on feature flags.
#[allow(dead_code)]
pub(crate) enum ModelState {
    /// Unloaded - no weights in memory.
    Unloaded,

    /// Loaded with CLIP weights and tokenizer (GPU-accelerated).
    Loaded {
        /// CLIP model weights on GPU (boxed to reduce enum size).
        weights: Box<ClipWeights>,
        /// HuggingFace tokenizer for text encoding (boxed to reduce enum size).
        tokenizer: Box<Tokenizer>,
    },
}

/// Multimodal embedding model using openai/clip-vit-base-patch32.
///
/// This model produces 768D vectors for both text and images in a shared
/// embedding space, enabling cross-modal similarity search.
pub struct MultimodalModel {
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

    /// Memory used by model weights (bytes).
    #[allow(dead_code)]
    pub(crate) memory_size: usize,

    /// Image processor for CLIP preprocessing.
    pub(crate) image_processor: ImageProcessor,
}

impl MultimodalModel {
    /// Create a new MultimodalModel instance.
    ///
    /// Model is NOT loaded after construction. Call `load()` before `embed()`.
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
            image_processor: ImageProcessor::new(),
        })
    }

    /// Get a reference to the image processor.
    #[must_use]
    pub fn image_processor(&self) -> &ImageProcessor {
        &self.image_processor
    }

    /// Load model weights into memory (GPU-accelerated).
    pub async fn load(&self) -> EmbeddingResult<()> {
        // Initialize GPU device
        let device = init_gpu().map_err(|e| EmbeddingError::GpuError {
            message: format!("MultimodalModel GPU init failed: {}", e),
        })?;

        // Load tokenizer from model directory
        let tokenizer_path = self.model_path.join("tokenizer.json");
        let tokenizer =
            Tokenizer::from_file(&tokenizer_path).map_err(|e| EmbeddingError::ModelLoadError {
                model_id: ModelId::Multimodal,
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "Tokenizer load failed at {}: {}",
                        tokenizer_path.display(),
                        e
                    ),
                )),
            })?;

        // Check for safetensors file
        let safetensors_path = self.model_path.join("model.safetensors");
        if !safetensors_path.exists() {
            return Err(EmbeddingError::ModelLoadError {
                model_id: ModelId::Multimodal,
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Safetensors not found at {}", safetensors_path.display()),
                )),
            });
        }

        // Create VarBuilder from safetensors
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[&safetensors_path], DType::F32, device).map_err(
                |e| EmbeddingError::GpuError {
                    message: format!("Failed to load safetensors: {}", e),
                },
            )?
        };

        // Load CLIP weights
        let weights = Self::load_clip_weights(&vb, device)?;

        tracing::info!(
            "MultimodalModel loaded: {} params, {:.2} MB VRAM",
            weights.param_count(),
            weights.vram_bytes() as f64 / (1024.0 * 1024.0)
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
        tracing::info!("MultimodalModel unloaded");
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
}

// Implement Send and Sync manually since RwLock is involved
unsafe impl Send for MultimodalModel {}
unsafe impl Sync for MultimodalModel {}
