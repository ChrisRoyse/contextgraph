//! CausalModel struct and implementation.
//!
//! This module contains the main CausalModel struct, its constructor,
//! load/unload methods, embed methods, and the EmbeddingModel trait implementation.
//!
//! # Asymmetric Dual Embeddings
//!
//! The `embed_dual()` method produces genuinely different cause and effect vectors
//! through instruction-prefix-based asymmetric encoding. Two separate forward passes
//! with different instruction prefixes produce differentiated embeddings.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::init_gpu;
use crate::models::attention::{self, AttentionStrategy, resolve_attention_mode};
use crate::traits::{EmbeddingModel, SingleModelConfig};
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::forward::{gpu_forward, gpu_forward_dual, gpu_forward_dual_trained, gpu_forward_single_trained};
use super::loader::load_nomic_weights;
use super::weights::{NomicWeights, TrainableProjection};
use crate::training::lora::{LoraConfig, LoraLayers};

/// Trained LoRA + projection weights for fine-tuned inference.
pub(crate) struct TrainedState {
    /// LoRA adapters for Q+V attention layers.
    pub lora: LoraLayers,
    /// Trained cause/effect projection heads.
    pub projection: TrainableProjection,
}

/// Internal state for CausalModel weight management.
#[allow(clippy::large_enum_variant)]
pub(crate) enum ModelState {
    /// Unloaded - no weights in memory.
    Unloaded,

    /// Loaded with NomicBERT model and tokenizer (GPU-accelerated).
    Loaded {
        /// NomicBERT model weights on GPU.
        weights: NomicWeights,
        /// HuggingFace tokenizer for text encoding (boxed to reduce enum size).
        tokenizer: Box<Tokenizer>,
        /// Optional trained LoRA + projection weights.
        trained: Option<TrainedState>,
    },
}

/// Causal embedding model using nomic-ai/nomic-embed-text-v1.5.
///
/// This model produces 768D vectors optimized for causal reasoning.
/// Uses rotary position embeddings and SwiGLU FFN for efficient
/// processing of documents up to 8192 tokens (capped at 512 for causal).
///
/// # Architecture
///
/// NomicBERT with:
/// - Rotary position embeddings (RoPE, base=1000, full head_dim)
/// - Fused QKV projections (no separate Q/K/V weights)
/// - SwiGLU activation in FFN
/// - Contrastive pre-training for isotropic embeddings
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
    model_state: std::sync::RwLock<ModelState>,

    /// Path to model weights directory.
    model_path: PathBuf,

    /// Whether model weights are loaded and ready.
    loaded: AtomicBool,

    /// Attention strategy (dense, tiled, sliding window).
    attention_strategy: Box<dyn AttentionStrategy>,
}

impl CausalModel {
    // =========================================================================
    // INSTRUCTION PREFIXES FOR ASYMMETRIC CAUSAL EMBEDDINGS
    // =========================================================================
    // Per ARCH-15: "E5 Causal MUST use asymmetric similarity with separate
    // cause/effect vector encodings - cause→effect direction matters"
    //
    // These prefixes leverage nomic-embed's contrastive training to produce
    // genuinely different embeddings for cause vs effect roles.
    //
    // Canonical definitions live in config.rs (CAUSE_INSTRUCTION, EFFECT_INSTRUCTION)
    // to ensure gpu_forward_dual() and embed_as_cause/effect use identical strings.
    // =========================================================================

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

        let mode =
            resolve_attention_mode(config.attention_mode.as_ref(), config.use_flash_attention);
        let strategy = attention::create_strategy(&mode);

        tracing::debug!(
            "CausalModel attention strategy: {} (mode: {:?})",
            strategy.name(),
            mode,
        );

        Ok(Self {
            model_state: std::sync::RwLock::new(ModelState::Unloaded),
            model_path: model_path.to_path_buf(),
            loaded: AtomicBool::new(false),
            attention_strategy: strategy,
        })
    }

    /// Load model weights into memory.
    ///
    /// # GPU Pipeline
    ///
    /// 1. Initialize CUDA device
    /// 2. Load config.json and tokenizer.json
    /// 3. Load model.safetensors (NomicBERT weights)
    /// 4. Transfer all weight tensors to GPU VRAM
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

        // Load NomicBERT weights from safetensors
        let weights = load_nomic_weights(&self.model_path, device)?;

        tracing::info!(
            "CausalModel loaded: nomic-embed-text-v1.5, {} layers, hidden_size={}, RoPE base={}",
            weights.config.num_hidden_layers,
            weights.config.hidden_size,
            weights.config.rotary_emb_base
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
            tokenizer: Box::new(tokenizer),
            trained: None,
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

    /// Sequential processing of multiple inputs. Note: processes items one at a
    /// time (no GPU batching). For true batch inference, see Kepler model.
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
    // These methods produce differentiated cause/effect vectors using
    // instruction-prefix-based asymmetric encoding with nomic-embed-text-v1.5.
    //
    // Two separate forward passes with different instruction prefixes produce
    // genuinely different embeddings for cause vs effect roles.
    // =========================================================================

    /// Embed text as a potential CAUSE in causal relationships.
    ///
    /// Uses instruction prefix "search_query: Identify the cause in: " to
    /// produce a cause-role embedding via a single forward pass.
    pub async fn embed_as_cause(&self, content: &str) -> EmbeddingResult<Vec<f32>> {
        self.embed_single_role(content, super::config::CAUSE_INSTRUCTION, true).await
    }

    /// Embed text as a potential EFFECT in causal relationships.
    ///
    /// Uses instruction prefix "search_query: Identify the effect of: " to
    /// produce an effect-role embedding via a single forward pass.
    pub async fn embed_as_effect(&self, content: &str) -> EmbeddingResult<Vec<f32>> {
        self.embed_single_role(content, super::config::EFFECT_INSTRUCTION, false).await
    }

    /// Embed text with a single instruction prefix (one forward pass).
    ///
    /// When trained LoRA + projection weights are loaded, uses `search_document:`
    /// prefix to match the training distribution. The cause/effect asymmetry comes
    /// from the projection heads, not the instruction prefix.
    ///
    /// When no trained weights exist, uses the causal instruction prefix for
    /// nomic-embed's built-in query/document asymmetry.
    async fn embed_single_role(
        &self,
        content: &str,
        instruction: &str,
        is_cause: bool,
    ) -> EmbeddingResult<Vec<f32>> {
        self.ensure_initialized()?;

        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("CausalModel failed to acquire read lock: {}", e),
            })?;

        match &*state {
            ModelState::Loaded { weights, tokenizer, trained } => {
                let strategy = self.attention_strategy.as_ref();
                let vec = if let Some(ref t) = trained {
                    // Use search_document: prefix to match training distribution.
                    // Training pipeline (pipeline.rs) uses "search_document: " for all texts,
                    // so inference must use the same prefix for the projection heads to work
                    // correctly. Using causal instruction prefixes here causes a distribution
                    // shift that compresses all scores toward zero.
                    let text = format!("search_document: {}", content);
                    gpu_forward_single_trained(
                        &text, weights, tokenizer, &t.lora, &t.projection, is_cause, strategy,
                    )?
                } else {
                    // No trained weights: use causal instruction prefix for base model
                    let text = format!("{}{}", instruction, content);
                    gpu_forward(&text, weights, tokenizer, strategy)?
                };

                if vec.len() != 768 {
                    return Err(EmbeddingError::InternalError {
                        message: format!(
                            "E5 single-role embedding dimension error: got {}, expected 768",
                            vec.len()
                        ),
                    });
                }
                Ok(vec)
            }
            ModelState::Unloaded => Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Causal,
            }),
        }
    }

    /// Embed text as BOTH cause and effect roles simultaneously.
    ///
    /// When trained weights are loaded, uses `search_document:` prefix for both
    /// passes (matching training distribution), with cause/effect asymmetry from
    /// the projection heads. When no trained weights, uses causal instruction prefixes.
    ///
    /// ```text
    /// Input Text
    ///     |
    ///     +--------------------------------------+
    ///     |                                      |
    /// "search_document: {text}"          "search_document: {text}"
    ///     |                                      |
    /// [LoRA Forward]                     [LoRA Forward]
    ///     |                                      |
    /// [Cause Projection]                 [Effect Projection]
    ///     |                                      |
    /// cause_vec (768D)                    effect_vec (768D)
    /// ```
    pub async fn embed_dual(&self, content: &str) -> EmbeddingResult<(Vec<f32>, Vec<f32>)> {
        self.embed_dual_guided(content, None).await
    }

    /// Embed text as BOTH cause and effect roles with optional LLM guidance.
    ///
    /// The guidance parameter is accepted for API compatibility with callers
    /// that provide LLM-extracted hints. With nomic-embed, asymmetry comes
    /// from instruction prefixes, so guidance is not used for embedding.
    pub async fn embed_dual_guided(
        &self,
        content: &str,
        _guidance: Option<&context_graph_core::traits::CausalHintGuidance>,
    ) -> EmbeddingResult<(Vec<f32>, Vec<f32>)> {
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
                tokenizer,
                trained,
            } => {
                let strategy = self.attention_strategy.as_ref();
                let (cause_vec, effect_vec) = if let Some(ref t) = trained {
                    gpu_forward_dual_trained(
                        content, weights, tokenizer, &t.lora, &t.projection, strategy,
                    )?
                } else {
                    gpu_forward_dual(content, weights, tokenizer, strategy)?
                };

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

    /// Load trained LoRA + projection weights from a checkpoint directory.
    ///
    /// Looks for `lora_best.safetensors` and `projection_best.safetensors` in
    /// the given directory. When both are found, loads them into the model's
    /// forward path so that `embed()`/`embed_dual()`/`embed_as_cause()`/
    /// `embed_as_effect()` all use the fine-tuned weights.
    ///
    /// # Errors
    /// Returns `EmbeddingError` if:
    /// - Checkpoint directory does not exist
    /// - Either `lora_best.safetensors` or `projection_best.safetensors` is missing
    /// - Weight loading fails
    ///
    /// FAIL FAST: no silent fallback to base model. Without trained weights,
    /// E5 produces near-uniform scores (0.93-0.98) that make the causal gate
    /// a no-op. Per constitution: "FAIL FAST — no silent degradation."
    ///
    /// # Arguments
    /// * `checkpoint_dir` - Directory containing trained weight files
    pub fn load_trained_weights(&self, checkpoint_dir: &Path) -> EmbeddingResult<()> {
        self.ensure_initialized()?;

        let lora_path = checkpoint_dir.join("lora_best.safetensors");
        let projection_path = checkpoint_dir.join("projection_best.safetensors");

        if !lora_path.exists() && !projection_path.exists() {
            tracing::error!(
                checkpoint_dir = %checkpoint_dir.display(),
                lora_path = %lora_path.display(),
                projection_path = %projection_path.display(),
                "E5 FATAL: No trained LoRA weights found. \
                 Without trained weights, E5 produces near-uniform scores (0.93-0.98) \
                 that make the causal gate a no-op. \
                 Train E5 with: cargo run --release --bin train_causal -- --data-dir <path> \
                 Then place lora_best.safetensors and projection_best.safetensors in the checkpoint dir."
            );
            return Err(EmbeddingError::InternalError {
                message: format!(
                    "E5 trained weights not found in {}. Both lora_best.safetensors and \
                     projection_best.safetensors are required. Without trained weights, \
                     the causal gate is non-functional.",
                    checkpoint_dir.display()
                ),
            });
        }

        if !lora_path.exists() || !projection_path.exists() {
            tracing::error!(
                lora_exists = lora_path.exists(),
                projection_exists = projection_path.exists(),
                checkpoint_dir = %checkpoint_dir.display(),
                "E5 FATAL: Incomplete checkpoint — both lora_best.safetensors and \
                 projection_best.safetensors are required"
            );
            return Err(EmbeddingError::InternalError {
                message: format!(
                    "Incomplete E5 checkpoint in {}: lora={}, projection={}. Both required.",
                    checkpoint_dir.display(),
                    lora_path.exists(),
                    projection_path.exists()
                ),
            });
        }

        // Get device from loaded weights
        let device = {
            let state = self.model_state.read().map_err(|e| EmbeddingError::InternalError {
                message: format!("Failed to acquire read lock: {}", e),
            })?;
            match &*state {
                ModelState::Loaded { weights, .. } => weights.device.clone(),
                ModelState::Unloaded => {
                    return Err(EmbeddingError::NotInitialized {
                        model_id: ModelId::Causal,
                    });
                }
            }
        };

        // Load LoRA weights
        let lora_config = LoraConfig::default(); // 12 layers, rank 16, Q+V
        let lora = LoraLayers::load_from_safetensors(&lora_path, lora_config, &device)?;

        // Load projection weights
        let projection = TrainableProjection::load_trained(&projection_path, &device)?;

        tracing::info!(
            "Loaded trained weights: LoRA {} params, projection {}x{} from {}",
            lora.total_params(),
            projection.hidden_size,
            projection.hidden_size,
            checkpoint_dir.display()
        );

        // Store trained weights in model state
        let mut state = self.model_state.write().map_err(|e| EmbeddingError::InternalError {
            message: format!("Failed to acquire write lock: {}", e),
        })?;

        match &mut *state {
            ModelState::Loaded { trained, .. } => {
                *trained = Some(TrainedState { lora, projection });
            }
            ModelState::Unloaded => {
                return Err(EmbeddingError::NotInitialized {
                    model_id: ModelId::Causal,
                });
            }
        }

        Ok(())
    }

    /// Check if trained weights are currently loaded.
    pub fn has_trained_weights(&self) -> bool {
        let state = match self.model_state.read() {
            Ok(s) => s,
            Err(_) => return false,
        };
        matches!(&*state, ModelState::Loaded { trained: Some(_), .. })
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
            ModelState::Loaded { weights, tokenizer, trained } => {
                let strategy = self.attention_strategy.as_ref();
                let vector = if let Some(ts) = trained {
                    // Use trained LoRA + projection (cause direction by default for single embed)
                    gpu_forward_single_trained(&text_content, weights, tokenizer, &ts.lora, &ts.projection, true, strategy)?
                } else {
                    gpu_forward(&text_content, weights, tokenizer, strategy)?
                };
                let latency_us = start.elapsed().as_micros() as u64;
                Ok(ModelEmbedding::new(ModelId::Causal, vector, latency_us))
            }
            ModelState::Unloaded => Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Causal,
            }),
        }
    }
}

// SAFETY: CausalModel is Send + Sync because:
// - model_state: std::sync::RwLock<ModelState> is Send + Sync
// - model_path: PathBuf is Send + Sync
// - loaded: AtomicBool is Send + Sync
// The manual impls are needed because ModelState::Loaded contains Tokenizer
// (from the `tokenizers` crate) and NomicWeights (holding candle Tensors with
// raw GPU pointers). These types are safe to share across threads because:
// - Tokenizer is immutable after construction (only used for encode())
// - Candle Tensors use Arc internally and are thread-safe for reads
// - All GPU operations are serialized through the RwLock
unsafe impl Send for CausalModel {}
unsafe impl Sync for CausalModel {}
