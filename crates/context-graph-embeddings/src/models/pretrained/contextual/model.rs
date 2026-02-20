//! Core ContextualModel struct and lifecycle management.
//!
//! # E5-base-v2 Asymmetric Dual Embeddings
//!
//! This model uses intfloat/e5-base-v2, which is specifically trained for
//! asymmetric retrieval using prefix-based encoding:
//!
//! - **Intent embedding**: "query: " + content (what the user wants to find)
//! - **Context embedding**: "passage: " + content (document/passage to be searched)
//!
//! Unlike the previous projection-based approach, E5's asymmetry is learned
//! during training, producing genuinely different semantic representations
//! for queries vs documents.
//!
//! # Model
//!
//! Uses intfloat/e5-base-v2:
//! - Architecture: BERT-base (12 layers)
//! - Dimension: 768D
//! - Max tokens: 512
//! - Training: Trained for asymmetric retrieval on large corpus

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{init_gpu, GpuModelLoader, normalize_gpu};
use crate::traits::{EmbeddingModel, SingleModelConfig};
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::constants::{CONTEXTUAL_DIMENSION, CONTEXTUAL_MAX_TOKENS, INTENT_PREFIX, CONTEXT_PREFIX};
use super::state::{ContextualModelState, ModelState};

/// Contextual embedding model using intfloat/e5-base-v2.
///
/// Produces 768D vectors optimized for asymmetric retrieval. Uses BERT architecture
/// with prefix-based encoding for intent vs context differentiation.
///
/// # Asymmetric Encoding
///
/// E5-base-v2 was trained with prefix-based asymmetric encoding:
/// - Queries (intent): Prefix with "query: "
/// - Documents (context): Prefix with "passage: "
///
/// This creates genuinely learned asymmetric representations, unlike
/// projection-based approaches that add artificial perturbations.
///
/// # Example
/// ```rust,ignore
/// use context_graph_embeddings::models::ContextualModel;
/// use context_graph_embeddings::traits::SingleModelConfig;
/// use context_graph_embeddings::error::EmbeddingResult;
/// use std::path::Path;
///
/// async fn example() -> EmbeddingResult<()> {
///     let model = ContextualModel::new(
///         Path::new("models/contextual"),
///         SingleModelConfig::default(),
///     )?;
///     model.load().await?;
///     let (intent, context): (Vec<f32>, Vec<f32>) =
///         model.embed_dual("User wants to fix the bug").await?;
///     assert_eq!(intent.len(), 768);
///     assert_eq!(context.len(), 768);
///     Ok(())
/// }
/// ```
pub struct ContextualModel {
    #[allow(dead_code)]
    pub(crate) model_state: std::sync::RwLock<ContextualModelState>,
    #[allow(dead_code)]
    pub(crate) model_path: PathBuf,
    #[allow(dead_code)]
    pub(crate) config: SingleModelConfig,
    pub(crate) loaded: AtomicBool,
    #[allow(dead_code)]
    pub(crate) memory_size: usize,
}

impl ContextualModel {
    /// Create a new ContextualModel instance. Call `load()` before `embed()`.
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
    /// Initializes CUDA device, loads tokenizer.json and model.safetensors,
    /// and transfers weight tensors to GPU VRAM.
    ///
    /// # E5-base-v2 Architecture
    ///
    /// E5-base-v2 uses prefix-based asymmetry, so no projection weights are
    /// needed. The asymmetric behavior is achieved through:
    /// - "query: " prefix for intent embeddings
    /// - "passage: " prefix for context embeddings
    pub async fn load(&self) -> EmbeddingResult<()> {
        let _device = init_gpu().map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel GPU init failed: {}", e),
        })?;

        let tokenizer_path = self.model_path.join("tokenizer.json");
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            EmbeddingError::ModelLoadError {
                model_id: ModelId::Multimodal,
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "ContextualModel tokenizer load failed at {}: {}",
                        tokenizer_path.display(),
                        e
                    ),
                )),
            }
        })?;

        let loader = GpuModelLoader::new().map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel loader init failed: {}", e),
        })?;

        let weights = loader.load_bert_weights(&self.model_path).map_err(|e| {
            EmbeddingError::ModelLoadError {
                model_id: ModelId::Multimodal,
                source: Box::new(std::io::Error::other(format!(
                    "ContextualModel weight load failed: {}",
                    e
                ))),
            }
        })?;

        if weights.config.hidden_size != CONTEXTUAL_DIMENSION {
            return Err(EmbeddingError::InvalidDimension {
                expected: CONTEXTUAL_DIMENSION,
                actual: weights.config.hidden_size,
            });
        }

        tracing::info!(
            "ContextualModel loaded: E5-base-v2 with prefix-based asymmetry, {} params, {:.2} MB VRAM, {}D",
            weights.param_count(),
            weights.vram_bytes() as f64 / (1024.0 * 1024.0),
            weights.config.hidden_size
        );

        let mut state = self
            .model_state
            .write()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("ContextualModel failed to acquire write lock: {}", e),
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
        tracing::info!("ContextualModel unloaded");
        Ok(())
    }

    /// Sequential processing of multiple inputs. Note: processes items one at a
    /// time (no GPU batching). For true batch inference, see Kepler model.
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
    fn extract_content(input: &ModelInput) -> EmbeddingResult<String> {
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
                model_id: ModelId::Multimodal,
                input_type: InputType::from(input),
            }),
        }
    }

    // =========================================================================
    // E5-BASE-V2 PREFIX-BASED ASYMMETRIC EMBEDDING
    // =========================================================================
    //
    // E5-base-v2 achieves asymmetry through prefix-based encoding:
    // - "query: " prefix for intent/query embeddings
    // - "passage: " prefix for context/document embeddings
    //
    // This is fundamentally different from projection-based approaches:
    // - Learned asymmetry: The model was trained to produce different
    //   representations based on the prefix
    // - No random perturbations: The asymmetry has semantic meaning
    // - Natural directionality: Queries find passages, not vice versa
    // =========================================================================

    /// Embed text as a potential INTENT in contextual relationships.
    ///
    /// Uses E5's "query: " prefix encoding.
    /// Use this when embedding text that represents "what the user wants to find".
    ///
    /// # Arguments
    /// * `content` - Text content to embed as an intent
    ///
    /// # Returns
    /// 768D embedding vector with intent-role semantics
    pub async fn embed_as_intent(&self, content: &str) -> EmbeddingResult<Vec<f32>> {
        let (intent_vec, _) = self.embed_dual(content).await?;
        Ok(intent_vec)
    }

    /// Embed text as a potential CONTEXT in contextual relationships.
    ///
    /// Uses E5's "passage: " prefix encoding.
    /// Use this when embedding text that represents "contextual background/documents".
    ///
    /// # Arguments
    /// * `content` - Text content to embed as context
    ///
    /// # Returns
    /// 768D embedding vector with context-role semantics
    pub async fn embed_as_context(&self, content: &str) -> EmbeddingResult<Vec<f32>> {
        let (_, context_vec) = self.embed_dual(content).await?;
        Ok(context_vec)
    }

    /// Embed text as BOTH intent and context roles simultaneously.
    ///
    /// Produces two distinct 768D vectors using E5's prefix-based encoding:
    /// - intent_vec: "query: " + content → encoder → L2 normalize
    /// - context_vec: "passage: " + content → encoder → L2 normalize
    ///
    /// # Architecture
    ///
    /// ```text
    /// Input Text: "fix authentication bug"
    ///     |
    ///     +------------------------+
    ///     |                        |
    /// "query: fix auth..."    "passage: fix auth..."
    ///     |                        |
    /// [Tokenize]              [Tokenize]
    ///     |                        |
    /// [Encoder Pass 1]        [Encoder Pass 2]
    ///     |                        |
    /// [Mean Pool]             [Mean Pool]
    ///     |                        |
    /// [L2 Normalize]          [L2 Normalize]
    ///     |                        |
    /// intent_vec (768D)       context_vec (768D)
    /// ```
    ///
    /// # Arguments
    /// * `content` - Text content to embed in both roles
    ///
    /// # Returns
    /// Tuple of (intent_vector, context_vector), each 768D
    ///
    /// # Performance
    /// Requires two encoder forward passes (one per prefix).
    /// This is the tradeoff for genuine learned asymmetry.
    pub async fn embed_dual(&self, content: &str) -> EmbeddingResult<(Vec<f32>, Vec<f32>)> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Multimodal,
            });
        }

        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("ContextualModel failed to acquire read lock: {}", e),
            })?;

        match &*state {
            ModelState::Loaded { weights, tokenizer } => {
                // Intent embedding: "query: " + content
                let intent_text = format!("{}{}", INTENT_PREFIX, content);
                let intent_vec = gpu_forward(&intent_text, weights, tokenizer)?;

                // Context embedding: "passage: " + content
                let context_text = format!("{}{}", CONTEXT_PREFIX, content);
                let context_vec = gpu_forward(&context_text, weights, tokenizer)?;

                // Validate dimensions (fail fast on implementation error)
                if intent_vec.len() != CONTEXTUAL_DIMENSION || context_vec.len() != CONTEXTUAL_DIMENSION {
                    return Err(EmbeddingError::InternalError {
                        message: format!(
                            "E10 dual embedding dimension error: intent={}, context={}, expected {}",
                            intent_vec.len(),
                            context_vec.len(),
                            CONTEXTUAL_DIMENSION
                        ),
                    });
                }

                Ok((intent_vec, context_vec))
            }
            ModelState::Unloaded => Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Multimodal,
            }),
        }
    }
}

/// GPU forward pass for contextual model (BERT-compatible).
fn gpu_forward(
    text: &str,
    weights: &crate::gpu::BertWeights,
    tokenizer: &tokenizers::Tokenizer,
) -> EmbeddingResult<Vec<f32>> {
    let device = weights.device();
    let config = &weights.config;

    // Tokenize input text
    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| EmbeddingError::TokenizationError {
            model_id: ModelId::Multimodal,
            message: format!("ContextualModel tokenization failed: {}", e),
        })?;

    let token_ids: Vec<u32> = encoding.get_ids().to_vec();
    let attention_mask: Vec<f32> = encoding
        .get_attention_mask()
        .iter()
        .map(|&m| m as f32)
        .collect();

    // Truncate to max tokens if needed
    let max_len = config.max_position_embeddings.min(CONTEXTUAL_MAX_TOKENS);
    let seq_len = token_ids.len().min(max_len);
    let token_ids = &token_ids[..seq_len];
    let attention_mask = &attention_mask[..seq_len];

    // Create GPU tensors
    let input_ids = Tensor::from_slice(token_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("ContextualModel input_ids tensor failed: {}", e),
        }
    })?;

    let attention_mask_tensor =
        Tensor::from_slice(attention_mask, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("ContextualModel attention_mask tensor failed: {}", e),
            }
        })?;

    // Token type IDs (all zeros)
    let token_type_ids: Vec<u32> = vec![0u32; seq_len];
    let token_type_tensor =
        Tensor::from_slice(&token_type_ids, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("ContextualModel token_type tensor failed: {}", e),
            }
        })?;

    // Position IDs
    let position_ids: Vec<u32> = (0..seq_len as u32).collect();
    let position_tensor = Tensor::from_slice(&position_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("ContextualModel position_ids tensor failed: {}", e),
        }
    })?;

    // Compute embeddings
    let word_embeds = weights
        .embeddings
        .word_embeddings
        .index_select(
            &input_ids
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("ContextualModel flatten input_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel word embedding lookup failed: {}", e),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel word embedding reshape failed: {}", e),
        })?;

    let position_embeds = weights
        .embeddings
        .position_embeddings
        .index_select(
            &position_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("ContextualModel flatten position_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel position embedding lookup failed: {}", e),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel position embedding reshape failed: {}", e),
        })?;

    let token_type_embeds = weights
        .embeddings
        .token_type_embeddings
        .index_select(
            &token_type_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("ContextualModel flatten token_type_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel token_type embedding lookup failed: {}", e),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel token_type embedding reshape failed: {}", e),
        })?;

    // Sum embeddings
    let embeddings = ((word_embeds + position_embeds).map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel embedding add 1 failed: {}", e),
    })? + token_type_embeds)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel embedding add 2 failed: {}", e),
        })?;

    // Apply LayerNorm
    let embeddings = layer_norm(
        &embeddings,
        &weights.embeddings.layer_norm_weight,
        &weights.embeddings.layer_norm_bias,
        config.layer_norm_eps,
    )?;

    // Run encoder layers
    let mut hidden_states = embeddings;
    let extended_attention_mask = create_extended_attention_mask(&attention_mask_tensor)?;

    for (layer_idx, layer) in weights.encoder_layers.iter().enumerate() {
        hidden_states = encoder_layer_forward(
            &hidden_states,
            layer,
            &extended_attention_mask,
            config,
            layer_idx,
        )?;
    }

    // Mean pooling
    let pooled = mean_pooling(
        &hidden_states,
        &attention_mask_tensor,
        seq_len,
        config.hidden_size,
    )?;

    // L2 normalize
    let normalized = normalize_gpu(&pooled).map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel L2 normalize failed: {}", e),
    })?;

    // Convert to Vec<f32>
    normalized
        .flatten_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel flatten output failed: {}", e),
        })?
        .to_vec1()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel to_vec1 failed: {}", e),
        })
}

/// Apply LayerNorm.
///
/// Uses broadcast operations for proper shape handling with 3D tensors [batch, seq_len, hidden].
fn layer_norm(
    hidden_states: &Tensor,
    weight: &Tensor,
    bias: &Tensor,
    eps: f64,
) -> EmbeddingResult<Tensor> {
    // Compute mean over the last dimension (hidden dimension) using D::Minus1
    let mean = hidden_states
        .mean_keepdim(candle_core::D::Minus1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel layer_norm mean failed: {}", e),
        })?;

    // Center the input using broadcast subtraction (handles shape [B, S, H] - [B, S, 1])
    let x_centered = hidden_states
        .broadcast_sub(&mean)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel layer_norm center failed: {}", e),
        })?;

    // Compute variance over the last dimension
    let variance = x_centered
        .sqr()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel layer_norm sqr failed: {}", e),
        })?
        .mean_keepdim(candle_core::D::Minus1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel layer_norm variance failed: {}", e),
        })?;

    // Compute standard deviation
    let std = (variance + eps)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel layer_norm add eps failed: {}", e),
        })?
        .sqrt()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel layer_norm sqrt failed: {}", e),
        })?;

    // Normalize using broadcast division (handles shape [B, S, H] / [B, S, 1])
    let normalized = x_centered
        .broadcast_div(&std)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel layer_norm div failed: {}", e),
        })?;

    // Scale by weight using broadcast multiply
    let scaled = normalized
        .broadcast_mul(weight)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel layer_norm mul weight failed: {}", e),
        })?;

    // Add bias using broadcast add
    scaled
        .broadcast_add(bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel layer_norm add bias failed: {}", e),
        })
}

/// Create extended attention mask.
fn create_extended_attention_mask(attention_mask_tensor: &Tensor) -> EmbeddingResult<Tensor> {
    let extended = attention_mask_tensor
        .unsqueeze(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention mask unsqueeze 1 failed: {}", e),
        })?
        .unsqueeze(2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention mask unsqueeze 2 failed: {}", e),
        })?;

    // Convert mask: 1.0 -> 0.0, 0.0 -> -10000.0
    let inverted = (extended * (-1.0)).map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel attention mask mul failed: {}", e),
    })?;
    let shifted = (inverted + 1.0).map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel attention mask add failed: {}", e),
    })?;
    (shifted * (-10000.0f64)).map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel attention mask scale failed: {}", e),
    })
}

/// Encoder layer forward pass.
fn encoder_layer_forward(
    hidden_states: &Tensor,
    layer: &crate::gpu::EncoderLayerWeights,
    extended_attention_mask: &Tensor,
    config: &crate::gpu::BertConfig,
    _layer_idx: usize,
) -> EmbeddingResult<Tensor> {
    // Self-attention
    let attention_output = self_attention(hidden_states, layer, extended_attention_mask, config)?;

    // Add & Norm
    let hidden_states = layer_norm(
        &(hidden_states + &attention_output).map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention residual failed: {}", e),
        })?,
        &layer.attention.layer_norm_weight,
        &layer.attention.layer_norm_bias,
        config.layer_norm_eps,
    )?;

    // FFN
    let ffn_output = ffn_forward(&hidden_states, layer, config)?;

    // Add & Norm
    layer_norm(
        &(hidden_states + ffn_output).map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel ffn residual failed: {}", e),
        })?,
        &layer.ffn.layer_norm_weight,
        &layer.ffn.layer_norm_bias,
        config.layer_norm_eps,
    )
}

/// Self-attention forward pass.
///
/// Uses flatten/reshape pattern for Candle matmul compatibility:
/// 1. Flatten hidden_states from [batch, seq, hidden] to [batch*seq, hidden]
/// 2. Perform matmul with 2D weight matrices
/// 3. Reshape back to [batch, seq, hidden]
/// 4. Use broadcast_add for bias
fn self_attention(
    hidden_states: &Tensor,
    layer: &crate::gpu::EncoderLayerWeights,
    extended_attention_mask: &Tensor,
    config: &crate::gpu::BertConfig,
) -> EmbeddingResult<Tensor> {
    let att = &layer.attention;
    let num_heads = config.num_attention_heads;
    let head_dim = config.hidden_size / num_heads;
    let hidden_size = config.hidden_size;

    let (batch, seq_len, _) = hidden_states.dims3().map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel attention dims3 failed: {}", e),
    })?;

    // Flatten to [batch*seq, hidden] for Candle matmul compatibility
    let hidden_flat = hidden_states
        .reshape((batch * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel flatten hidden failed: {}", e),
        })?;

    // Q projection: flatten -> matmul -> reshape -> broadcast_add bias
    let query = hidden_flat
        .matmul(&att.query_weight.t().map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel query weight transpose failed: {}", e),
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel query matmul failed: {}", e),
        })?
        .reshape((batch, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel query reshape failed: {}", e),
        })?
        .broadcast_add(&att.query_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel query bias failed: {}", e),
        })?;

    // K projection
    let key = hidden_flat
        .matmul(&att.key_weight.t().map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel key weight transpose failed: {}", e),
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel key matmul failed: {}", e),
        })?
        .reshape((batch, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel key reshape failed: {}", e),
        })?
        .broadcast_add(&att.key_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel key bias failed: {}", e),
        })?;

    // V projection
    let value = hidden_flat
        .matmul(&att.value_weight.t().map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel value weight transpose failed: {}", e),
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel value matmul failed: {}", e),
        })?
        .reshape((batch, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel value reshape failed: {}", e),
        })?
        .broadcast_add(&att.value_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel value bias failed: {}", e),
        })?;

    // Reshape to [batch, num_heads, seq_len, head_dim]
    // Use contiguous() after transpose for matmul compatibility
    let query = query
        .reshape((batch, seq_len, num_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel query reshape failed: {}", e),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel query transpose failed: {}", e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel query contiguous failed: {}", e),
        })?;

    let key = key
        .reshape((batch, seq_len, num_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel key reshape failed: {}", e),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel key transpose failed: {}", e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel key contiguous failed: {}", e),
        })?;

    let value = value
        .reshape((batch, seq_len, num_heads, head_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel value reshape failed: {}", e),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel value transpose failed: {}", e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel value contiguous failed: {}", e),
        })?;

    // Attention scores: Q @ K^T / sqrt(d)
    let key_t = key
        .transpose(2, 3)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel key transpose for scores failed: {}", e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel key_t contiguous failed: {}", e),
        })?;

    let scores = (query
        .matmul(&key_t)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention scores matmul failed: {}", e),
        })?
        / (head_dim as f64).sqrt())
    .map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel attention scores scale failed: {}", e),
    })?;

    // Add attention mask using broadcast_add for proper shape handling
    let scores = scores
        .broadcast_add(extended_attention_mask)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention mask add failed: {}", e),
        })?;

    // Softmax
    let attention_probs = candle_nn::ops::softmax(&scores, 3).map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel attention softmax failed: {}", e),
    })?;

    // Attention output: probs @ V
    let context = attention_probs
        .matmul(&value)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention context matmul failed: {}", e),
        })?
        .transpose(1, 2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention context transpose failed: {}", e),
        })?
        .contiguous()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention context contiguous failed: {}", e),
        })?
        .reshape((batch, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention context reshape failed: {}", e),
        })?;

    // Output projection: flatten -> matmul -> reshape -> broadcast_add bias
    let context_flat = context
        .reshape((batch * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel output flatten failed: {}", e),
        })?;

    context_flat
        .matmul(&att.output_weight.t().map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention output weight transpose failed: {}", e),
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention output matmul failed: {}", e),
        })?
        .reshape((batch, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel output reshape failed: {}", e),
        })?
        .broadcast_add(&att.output_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel attention output bias failed: {}", e),
        })
}

/// FFN forward pass.
///
/// Uses flatten/reshape pattern for Candle matmul compatibility.
fn ffn_forward(
    hidden_states: &Tensor,
    layer: &crate::gpu::EncoderLayerWeights,
    config: &crate::gpu::BertConfig,
) -> EmbeddingResult<Tensor> {
    let ffn = &layer.ffn;
    let hidden_size = config.hidden_size;
    let intermediate_size = config.intermediate_size;

    let (batch, seq_len, _) = hidden_states.dims3().map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel ffn dims3 failed: {}", e),
    })?;

    // Flatten to [batch*seq, hidden] for Candle matmul compatibility
    let hidden_flat = hidden_states
        .reshape((batch * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel ffn flatten hidden failed: {}", e),
        })?;

    // Intermediate: flatten -> matmul -> reshape -> broadcast_add bias
    let intermediate = hidden_flat
        .matmul(&ffn.intermediate_weight.t().map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel ffn intermediate weight transpose failed: {}", e),
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel ffn intermediate matmul failed: {}", e),
        })?
        .reshape((batch, seq_len, intermediate_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel ffn intermediate reshape failed: {}", e),
        })?
        .broadcast_add(&ffn.intermediate_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel ffn intermediate bias failed: {}", e),
        })?;

    // GELU activation
    let activated = intermediate.gelu().map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel ffn gelu failed: {}", e),
    })?;

    // Output: flatten -> matmul -> reshape -> broadcast_add bias
    let activated_flat = activated
        .reshape((batch * seq_len, intermediate_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel ffn flatten activated failed: {}", e),
        })?;

    activated_flat
        .matmul(&ffn.output_weight.t().map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel ffn output weight transpose failed: {}", e),
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel ffn output matmul failed: {}", e),
        })?
        .reshape((batch, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel ffn output reshape failed: {}", e),
        })?
        .broadcast_add(&ffn.output_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel ffn output bias failed: {}", e),
        })
}

/// Mean pooling.
fn mean_pooling(
    hidden_states: &Tensor,
    attention_mask_tensor: &Tensor,
    seq_len: usize,
    hidden_size: usize,
) -> EmbeddingResult<Tensor> {
    let mask_expanded = attention_mask_tensor
        .unsqueeze(2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel mask expand failed: {}", e),
        })?
        .broadcast_as((1, seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel mask broadcast failed: {}", e),
        })?;

    let masked_hidden = (hidden_states * mask_expanded).map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel masked multiply failed: {}", e),
    })?;

    let sum_hidden = masked_hidden.sum(1).map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel sum hidden failed: {}", e),
    })?;

    let mask_sum = attention_mask_tensor
        .sum(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel mask sum failed: {}", e),
        })?
        .unsqueeze(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel mask sum unsqueeze failed: {}", e),
        })?
        .broadcast_as(sum_hidden.shape())
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel mask sum broadcast failed: {}", e),
        })?;

    (sum_hidden
        / (mask_sum + 1e-9f64).map_err(|e| EmbeddingError::GpuError {
            message: format!("ContextualModel mask sum add eps failed: {}", e),
        })?)
    .map_err(|e| EmbeddingError::GpuError {
        message: format!("ContextualModel mean pooling div failed: {}", e),
    })
}

#[async_trait]
impl EmbeddingModel for ContextualModel {
    fn model_id(&self) -> ModelId {
        ModelId::Multimodal
    }

    fn supported_input_types(&self) -> &[InputType] {
        &[InputType::Text]
    }

    fn is_initialized(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    async fn load(&self) -> EmbeddingResult<()> {
        ContextualModel::load(self).await
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Multimodal,
            });
        }
        self.validate_input(input)?;
        let content = Self::extract_content(input)?;
        let start = std::time::Instant::now();

        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("ContextualModel failed to acquire read lock: {}", e),
            })?;

        let (weights, tokenizer) = match &*state {
            ModelState::Loaded { weights, tokenizer } => (weights, tokenizer),
            _ => {
                return Err(EmbeddingError::NotInitialized {
                    model_id: ModelId::Multimodal,
                })
            }
        };

        // For single embed, use passage prefix (context mode)
        let prefixed_content = format!("{}{}", CONTEXT_PREFIX, content);
        let vector = gpu_forward(&prefixed_content, weights, tokenizer)?;
        let latency_us = start.elapsed().as_micros() as u64;
        Ok(ModelEmbedding::new(ModelId::Multimodal, vector, latency_us))
    }
}

unsafe impl Send for ContextualModel {}
unsafe impl Sync for ContextualModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};

    #[test]
    fn test_layer_norm_shape_compatibility() {
        let device = Device::Cpu;

        // Simulate typical input shape: [batch=1, seq_len=128, hidden=768]
        let hidden_states = Tensor::randn(0.0f32, 1.0, (1, 128, 768), &device).unwrap();
        let weight = Tensor::ones((768,), candle_core::DType::F32, &device).unwrap();
        let bias = Tensor::zeros((768,), candle_core::DType::F32, &device).unwrap();

        let result = layer_norm(&hidden_states, &weight, &bias, 1e-12);

        assert!(
            result.is_ok(),
            "layer_norm should succeed: {:?}",
            result.err()
        );
        let output = result.unwrap();
        assert_eq!(
            output.dims(),
            &[1, 128, 768],
            "Output shape should match input shape"
        );
    }

    #[test]
    fn test_layer_norm_different_batch_sizes() {
        let device = Device::Cpu;

        for batch_size in [1, 2, 4] {
            let hidden_states =
                Tensor::randn(0.0f32, 1.0, (batch_size, 64, 768), &device).unwrap();
            let weight = Tensor::ones((768,), candle_core::DType::F32, &device).unwrap();
            let bias = Tensor::zeros((768,), candle_core::DType::F32, &device).unwrap();

            let result = layer_norm(&hidden_states, &weight, &bias, 1e-12);

            assert!(
                result.is_ok(),
                "layer_norm should succeed for batch_size={}: {:?}",
                batch_size,
                result.err()
            );
            let output = result.unwrap();
            assert_eq!(output.dims(), &[batch_size, 64, 768]);
        }
    }

    #[test]
    fn test_layer_norm_normalizes_output() {
        let device = Device::Cpu;

        // Create input with known non-zero mean
        let hidden_states = Tensor::randn(5.0f32, 2.0, (1, 10, 768), &device).unwrap();
        let weight = Tensor::ones((768,), candle_core::DType::F32, &device).unwrap();
        let bias = Tensor::zeros((768,), candle_core::DType::F32, &device).unwrap();

        let output = layer_norm(&hidden_states, &weight, &bias, 1e-12).unwrap();

        // Check that output mean is close to 0 (per position in the sequence)
        let output_mean = output
            .mean_keepdim(candle_core::D::Minus1)
            .unwrap();
        let mean_vals: Vec<f32> = output_mean.flatten_all().unwrap().to_vec1().unwrap();

        for (i, m) in mean_vals.iter().enumerate() {
            assert!(
                m.abs() < 0.01,
                "Output mean at position {} should be ~0, got {}",
                i,
                m
            );
        }
    }

    #[test]
    fn test_layer_norm_different_sequence_lengths() {
        let device = Device::Cpu;

        // Test various sequence lengths that might occur in practice
        for seq_len in [1, 16, 128, 384] {
            let hidden_states =
                Tensor::randn(0.0f32, 1.0, (1, seq_len, 768), &device).unwrap();
            let weight = Tensor::ones((768,), candle_core::DType::F32, &device).unwrap();
            let bias = Tensor::zeros((768,), candle_core::DType::F32, &device).unwrap();

            let result = layer_norm(&hidden_states, &weight, &bias, 1e-12);

            assert!(
                result.is_ok(),
                "layer_norm should succeed for seq_len={}: {:?}",
                seq_len,
                result.err()
            );
            let output = result.unwrap();
            assert_eq!(output.dims(), &[1, seq_len, 768]);
        }
    }

    #[test]
    fn test_prefix_constants_are_correct() {
        // Verify E5 prefix constants are as expected
        assert_eq!(INTENT_PREFIX, "query: ");
        assert_eq!(CONTEXT_PREFIX, "passage: ");
    }
}
