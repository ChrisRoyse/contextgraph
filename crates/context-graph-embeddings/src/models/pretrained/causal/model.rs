//! CausalModel struct and implementation.
//!
//! This module contains the main CausalModel struct, its constructor,
//! load/unload methods, embed methods, and the EmbeddingModel trait implementation.
//!
//! # Asymmetric Dual Embeddings
//!
//! The `embed_dual()` method produces genuinely different cause and effect vectors
//! through marker detection, weighted pooling, and learned projections.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::init_gpu;
use crate::traits::{EmbeddingModel, SingleModelConfig};
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::config::DEFAULT_ATTENTION_WINDOW;
use super::forward::{gpu_forward, gpu_forward_dual};
use super::loader::load_longformer_weights;
use super::weights::{CausalProjectionWeights, LongformerWeights, CAUSAL_PROJECTION_SEED};

/// Internal state that varies based on feature flags.
#[allow(dead_code)]
pub(crate) enum ModelState {
    /// Unloaded - no weights in memory.
    Unloaded,

    /// Loaded with candle model and tokenizer (GPU-accelerated).
    Loaded {
        /// Longformer model weights on GPU.
        weights: LongformerWeights,
        /// Causal projection weights for asymmetric embeddings.
        projection: CausalProjectionWeights,
        /// HuggingFace tokenizer for text encoding (boxed to reduce enum size).
        tokenizer: Box<Tokenizer>,
    },
}

/// Causal embedding model using allenai/longformer-base-4096.
///
/// This model produces 768D vectors optimized for causal reasoning.
/// Uses sliding window attention + global attention for efficient
/// processing of documents up to 4096 tokens.
///
/// # Attention Mechanism
///
/// Longformer uses a combination of:
/// - **Sliding window attention**: Each token attends to `window_size` neighbors
/// - **Global attention**: Selected tokens (e.g., [CLS]) attend to all tokens
///
/// This allows O(n x w) complexity instead of O(n^2) for long sequences.
///
/// # Construction
///
/// ```rust,no_run
/// use context_graph_embeddings::models::CausalModel;
/// use context_graph_embeddings::traits::SingleModelConfig;
/// use context_graph_embeddings::error::EmbeddingResult;
/// use std::path::Path;
///
/// async fn example() -> EmbeddingResult<()> {
///     let model = CausalModel::new(
///         Path::new("models/causal"),
///         SingleModelConfig::default(),
///     )?;
///     model.load().await?;  // Must load before embed
///     Ok(())
/// }
/// ```
pub struct CausalModel {
    /// Model weights and inference engine.
    #[allow(dead_code)]
    model_state: std::sync::RwLock<ModelState>,

    /// Path to model weights directory.
    #[allow(dead_code)]
    model_path: PathBuf,

    /// Configuration for this model instance.
    #[allow(dead_code)]
    config: SingleModelConfig,

    /// Whether model weights are loaded and ready.
    loaded: AtomicBool,

    /// Memory used by model weights (bytes).
    #[allow(dead_code)]
    memory_size: usize,

    /// Attention window size for sliding window attention.
    attention_window: usize,

    /// Token indices that receive global attention (e.g., [CLS] at 0).
    global_attention_tokens: Vec<usize>,
}

impl CausalModel {
    // =========================================================================
    // INSTRUCTION PREFIXES FOR ASYMMETRIC CAUSAL EMBEDDINGS
    // =========================================================================
    // Per ARCH-15: "E5 Causal MUST use asymmetric similarity with separate
    // cause/effect vector encodings - cause→effect direction matters"
    //
    // These prefixes instruct the model to encode the text from different
    // causal perspectives, producing genuinely different embeddings for
    // cause vs effect roles.
    // =========================================================================

    /// Instruction prefix for encoding text as a potential CAUSE.
    ///
    /// When text is embedded with this prefix, the resulting vector is optimized
    /// for finding effects that this text could produce.
    pub const CAUSE_INSTRUCTION: &'static str =
        "Represent this text as a cause that produces effects: ";

    /// Instruction prefix for encoding text as a potential EFFECT.
    ///
    /// When text is embedded with this prefix, the resulting vector is optimized
    /// for finding causes that could have produced this text.
    pub const EFFECT_INSTRUCTION: &'static str =
        "Represent this text as an effect produced by causes: ";

    /// Create a new CausalModel instance.
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
            attention_window: DEFAULT_ATTENTION_WINDOW,
            global_attention_tokens: vec![0], // [CLS] token by default
        })
    }

    /// Configure which token indices receive global attention.
    ///
    /// By default, only the [CLS] token (index 0) has global attention.
    /// Additional tokens can be added for task-specific needs.
    ///
    /// # Arguments
    /// * `tokens` - Indices of tokens that should attend globally
    pub fn set_global_attention_tokens(&mut self, tokens: &[usize]) {
        self.global_attention_tokens = tokens.to_vec();
    }

    /// Get the attention window size for sliding attention.
    #[inline]
    #[must_use]
    pub fn attention_window(&self) -> usize {
        self.attention_window
    }

    /// Get the current global attention token indices.
    #[inline]
    #[must_use]
    pub fn global_attention_tokens(&self) -> &[usize] {
        &self.global_attention_tokens
    }

    /// Load model weights into memory.
    ///
    /// # GPU Pipeline
    ///
    /// 1. Initialize CUDA device
    /// 2. Load config.json and tokenizer.json
    /// 3. Load model.safetensors
    /// 4. Transfer all weight tensors to GPU VRAM
    /// 5. Initialize causal projection weights
    pub async fn load(&self) -> EmbeddingResult<()> {
        // Initialize GPU device
        let device = init_gpu().map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel GPU init failed: {}", e),
        })?;

        // Load tokenizer from model directory
        let tokenizer_path = self.model_path.join("tokenizer.json");
        let tokenizer =
            Tokenizer::from_file(&tokenizer_path).map_err(|e| EmbeddingError::ModelLoadError {
                model_id: ModelId::Causal,
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "Tokenizer load failed at {}: {}",
                        tokenizer_path.display(),
                        e
                    ),
                )),
            })?;

        // Load weights from safetensors
        let weights = load_longformer_weights(&self.model_path, device)?;

        // Initialize causal projection weights for asymmetric embeddings
        let projection = CausalProjectionWeights::initialize(
            weights.config.hidden_size,
            device,
            CAUSAL_PROJECTION_SEED,
        )?;

        tracing::info!(
            "CausalModel loaded: {} layers, hidden_size={}, with marker detection + causal projections",
            weights.config.num_hidden_layers,
            weights.config.hidden_size
        );

        // Update state
        let mut state = self
            .model_state
            .write()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("Failed to acquire write lock: {}", e),
            })?;

        *state = ModelState::Loaded {
            weights,
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
        tracing::info!("CausalModel unloaded");
        Ok(())
    }

    /// Embed a batch of inputs (more efficient than single embed).
    pub async fn embed_batch(&self, inputs: &[ModelInput]) -> EmbeddingResult<Vec<ModelEmbedding>> {
        self.ensure_initialized()?;
        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            results.push(self.embed(input).await?);
        }
        Ok(results)
    }

    // =========================================================================
    // ASYMMETRIC DUAL EMBEDDING METHODS
    // =========================================================================
    //
    // These methods produce differentiated cause/effect vectors using:
    //
    // 1. Causal marker detection (cause: "because", "due to"; effect: "therefore", "results in")
    // 2. Marker-weighted pooling (2.5x boost on relevant markers)
    // 3. Learned projections (W_cause, W_effect perturbed identities)
    //
    // The marker-weighted pooling creates genuine asymmetry by emphasizing
    // different parts of the input for cause vs effect embeddings.
    // =========================================================================

    /// Embed text as a potential CAUSE in causal relationships.
    ///
    /// Uses marker-weighted pooling with cause indicators boosted (because, due to, etc.)
    /// and applies the W_cause projection matrix.
    ///
    /// # Arguments
    /// * `content` - Text content to embed as a cause
    ///
    /// # Returns
    /// 768D embedding vector with cause-role semantics
    pub async fn embed_as_cause(&self, content: &str) -> EmbeddingResult<Vec<f32>> {
        let (cause_vec, _) = self.embed_dual(content).await?;
        Ok(cause_vec)
    }

    /// Embed text as a potential EFFECT in causal relationships.
    ///
    /// Uses marker-weighted pooling with effect indicators boosted (therefore, results in, etc.)
    /// and applies the W_effect projection matrix.
    ///
    /// # Arguments
    /// * `content` - Text content to embed as an effect
    ///
    /// # Returns
    /// 768D embedding vector with effect-role semantics
    pub async fn embed_as_effect(&self, content: &str) -> EmbeddingResult<Vec<f32>> {
        let (_, effect_vec) = self.embed_dual(content).await?;
        Ok(effect_vec)
    }

    /// Embed text as BOTH cause and effect roles simultaneously.
    ///
    /// Produces two differentiated 768D vectors from a single encoder pass.
    /// Asymmetry comes from TWO sources:
    /// 1. Marker-weighted pooling with different weights for cause vs effect
    /// 2. Learned projection matrices (W_cause, W_effect)
    ///
    /// # Architecture
    ///
    /// ```text
    /// Input Text
    ///     |
    /// [Tokenize + Detect Causal Markers]
    ///     |
    /// [Encoder] (single pass)
    ///     |
    ///     +---------------------------+
    ///     |                           |
    /// [Cause-Weighted Pool]    [Effect-Weighted Pool]
    /// (boost: because, due to) (boost: therefore, results)
    ///     |                           |
    /// [W_cause Projection]     [W_effect Projection]
    ///     |                           |
    /// [L2 Normalize]           [L2 Normalize]
    ///     |                           |
    /// cause_vec (768D)         effect_vec (768D)
    /// ```
    ///
    /// # Arguments
    /// * `content` - Text content to embed in both roles
    ///
    /// # Returns
    /// Tuple of (cause_vector, effect_vector), each 768D
    pub async fn embed_dual(&self, content: &str) -> EmbeddingResult<(Vec<f32>, Vec<f32>)> {
        self.ensure_initialized()?;

        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("CausalModel failed to acquire read lock: {}", e),
            })?;

        match &*state {
            ModelState::Loaded {
                weights,
                projection,
                tokenizer,
            } => {
                let (cause_vec, effect_vec) =
                    gpu_forward_dual(content, weights, projection, tokenizer)?;

                // Validate dimensions (fail fast on implementation error)
                if cause_vec.len() != 768 || effect_vec.len() != 768 {
                    return Err(EmbeddingError::InternalError {
                        message: format!(
                            "E5 dual embedding dimension error: cause={}, effect={}, expected 768",
                            cause_vec.len(),
                            effect_vec.len()
                        ),
                    });
                }

                Ok((cause_vec, effect_vec))
            }
            ModelState::Unloaded => Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Causal,
            }),
        }
    }

    /// Ensure model is initialized, returning an error if not.
    fn ensure_initialized(&self) -> EmbeddingResult<()> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }
        Ok(())
    }

}

#[async_trait]
impl EmbeddingModel for CausalModel {
    fn model_id(&self) -> ModelId {
        ModelId::Causal
    }

    fn supported_input_types(&self) -> &[InputType] {
        &[InputType::Text]
    }

    fn is_initialized(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    async fn load(&self) -> EmbeddingResult<()> {
        CausalModel::load(self).await
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        self.ensure_initialized()?;
        self.validate_input(input)?;

        let start = std::time::Instant::now();

        // Extract text content
        let text_content = match input {
            ModelInput::Text {
                content,
                instruction,
            } => {
                let mut full = content.clone();
                if let Some(inst) = instruction {
                    full = format!("{} {}", inst, full);
                }
                full
            }
            _ => {
                return Err(EmbeddingError::UnsupportedModality {
                    model_id: ModelId::Causal,
                    input_type: InputType::from(input),
                });
            }
        };

        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("CausalModel failed to acquire read lock: {}", e),
            })?;

        match &*state {
            ModelState::Loaded { weights, tokenizer, .. } => {
                let vector = gpu_forward(&text_content, weights, tokenizer)?;
                let latency_us = start.elapsed().as_micros() as u64;
                Ok(ModelEmbedding::new(ModelId::Causal, vector, latency_us))
            }
            _ => Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Causal,
            }),
        }
    }
}

// Implement Send and Sync manually since RwLock is involved
unsafe impl Send for CausalModel {}
unsafe impl Sync for CausalModel {}
