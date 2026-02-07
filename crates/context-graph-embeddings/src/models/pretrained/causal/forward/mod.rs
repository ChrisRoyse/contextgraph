//! Forward pass functions for Longformer model.
//!
//! This module implements the neural network forward pass including
//! embeddings computation, encoder layers, and output pooling.
//!
//! # Submodules
//!
//! - `ops`: LayerNorm, mean pooling, marker-weighted pooling, L2 normalization
//! - `attention`: Multi-head self-attention (standard, not sliding window)
//! - `encoder`: Encoder layers and FFN
//!
//! # Dual Forward Pass for Asymmetric Embeddings
//!
//! The `gpu_forward_dual()` function produces differentiated cause/effect embeddings:
//! 1. Tokenize input text + detect causal markers (because, causes, therefore, etc.)
//! 2. Run encoder once with standard attention
//! 3. Marker-weighted pooling with differentiated weights:
//!    - Cause embedding: boost cause markers (because, due to), reduce effect markers
//!    - Effect embedding: boost effect markers (therefore, results in), reduce cause markers
//! 4. Apply W_cause/W_effect projections
//! 5. L2 normalize both vectors
//!
//! # Asymmetry Sources
//!
//! Meaningful asymmetry comes from TWO sources:
//! 1. **Marker-weighted pooling**: Cause markers get 2.5x weight for cause embedding,
//!    effect markers get 2.5x weight for effect embedding
//! 2. **Learned projections**: W_cause and W_effect are perturbed identity matrices

mod attention;
mod encoder;
mod ops;

use candle_core::Tensor;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::ModelId;

use super::config::{LongformerConfig, CAUSAL_MAX_TOKENS};
use super::marker_detection::detect_causal_markers_with_hints;
use super::weights::{CausalProjectionWeights, LongformerWeights};

use encoder::run_encoder;
pub use ops::layer_norm;
use ops::{l2_normalize, marker_weighted_pooling, mean_pooling};

/// GPU-accelerated forward pass for Longformer.
///
/// Note: This implementation uses standard full attention (not sliding window)
/// for simplicity. For very long sequences (>512 tokens), sliding window
/// attention would be more efficient.
pub fn gpu_forward(
    text: &str,
    weights: &LongformerWeights,
    tokenizer: &Tokenizer,
) -> EmbeddingResult<Vec<f32>> {
    let device = weights.device;
    let config = &weights.config;

    // Tokenize input text
    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| EmbeddingError::TokenizationError {
            model_id: ModelId::Causal,
            message: format!("CausalModel tokenization failed: {}", e),
        })?;

    let token_ids: Vec<u32> = encoding.get_ids().to_vec();
    let attention_mask: Vec<f32> = encoding
        .get_attention_mask()
        .iter()
        .map(|&m| m as f32)
        .collect();

    // Truncate to max_position_embeddings if needed
    let max_len = config.max_position_embeddings.min(CAUSAL_MAX_TOKENS);
    let seq_len = token_ids.len().min(max_len);
    let token_ids = &token_ids[..seq_len];
    let attention_mask = &attention_mask[..seq_len];

    // Create GPU tensors
    let input_ids = Tensor::from_slice(token_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("CausalModel input_ids tensor failed: {}", e),
        }
    })?;

    let attention_mask_tensor =
        Tensor::from_slice(attention_mask, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("CausalModel attention_mask tensor failed: {}", e),
            }
        })?;

    // Token type IDs (all zeros for Longformer)
    let token_type_ids: Vec<u32> = vec![0u32; seq_len];
    let token_type_tensor =
        Tensor::from_slice(&token_type_ids, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("CausalModel token_type tensor failed: {}", e),
            }
        })?;

    // Position IDs
    let position_ids: Vec<u32> = (0..seq_len as u32).collect();
    let position_tensor = Tensor::from_slice(&position_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("CausalModel position_ids tensor failed: {}", e),
        }
    })?;

    // === EMBEDDING LAYER ===
    let embeddings = compute_embeddings(
        &input_ids,
        &position_tensor,
        &token_type_tensor,
        weights,
        config,
        seq_len,
    )?;

    // === ENCODER LAYERS ===
    let hidden_states = run_encoder(embeddings, &attention_mask_tensor, weights, config, seq_len)?;

    // === POOLING ===
    let pooled = mean_pooling(&hidden_states, &attention_mask_tensor)?;

    // L2 normalize
    let normalized = l2_normalize(&pooled)?;

    // Convert to Vec<f32>
    let vector: Vec<f32> = normalized
        .flatten_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel flatten output failed: {}", e),
        })?
        .to_vec1()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel to_vec1 failed: {}", e),
        })?;

    Ok(vector)
}

/// Compute embeddings from input tokens.
fn compute_embeddings(
    input_ids: &Tensor,
    position_tensor: &Tensor,
    token_type_tensor: &Tensor,
    weights: &LongformerWeights,
    config: &LongformerConfig,
    seq_len: usize,
) -> EmbeddingResult<Tensor> {
    let word_embeds = weights
        .embeddings
        .word_embeddings
        .index_select(
            &input_ids
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("CausalModel flatten input_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel word embedding lookup failed: {}", e),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel word embedding reshape failed: {}", e),
        })?;

    let position_embeds = weights
        .embeddings
        .position_embeddings
        .index_select(
            &position_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("CausalModel flatten position_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel position embedding lookup failed: {}", e),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel position embedding reshape failed: {}", e),
        })?;

    let token_type_embeds = weights
        .embeddings
        .token_type_embeddings
        .index_select(
            &token_type_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("CausalModel flatten token_type_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel token_type embedding lookup failed: {}", e),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel token_type embedding reshape failed: {}", e),
        })?;

    // Sum embeddings
    let embeddings = word_embeds
        .add(&position_embeds)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel embedding add 1 failed: {}", e),
        })?
        .add(&token_type_embeds)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel embedding add 2 failed: {}", e),
        })?;

    // Apply LayerNorm to embeddings
    layer_norm(
        &embeddings,
        &weights.embeddings.layer_norm_weight,
        &weights.embeddings.layer_norm_bias,
        config.layer_norm_eps,
    )
}

// =============================================================================
// Dual Forward Pass for Asymmetric Causal Embeddings
// =============================================================================

/// GPU-accelerated dual forward pass for cause/effect embeddings.
///
/// Produces two distinct 768D vectors from a single encoder pass:
/// - cause_vec: Marker-weighted pooling emphasizing cause indicators + W_cause projection
/// - effect_vec: Marker-weighted pooling emphasizing effect indicators + W_effect projection
///
/// # Architecture
///
/// Asymmetry comes from TWO sources:
/// 1. Marker-weighted pooling: Different weights for cause/effect marker tokens
/// 2. Learned projections: W_cause and W_effect with different perturbations
///
/// ```text
/// Input Text
///     |
/// [Tokenize + Detect Causal Markers]
///     |
/// [Compute Embeddings]
///     |
/// [Encoder Layers] (single pass)
///     |
///     +---------------------------+
///     |                           |
/// [Cause-Weighted Pooling]   [Effect-Weighted Pooling]
/// (boost: because, due to)   (boost: therefore, results in)
///     |                           |
/// [W_cause Projection]       [W_effect Projection]
///     |                           |
/// [L2 Normalize]             [L2 Normalize]
///     |                           |
/// cause_vec (768D)           effect_vec (768D)
/// ```
///
/// # Arguments
///
/// * `text` - Input text content
/// * `weights` - Longformer model weights
/// * `projection` - Causal projection weights (W_cause, W_effect)
/// * `tokenizer` - HuggingFace tokenizer
///
/// # Returns
///
/// Tuple of (cause_vec, effect_vec), each 768D L2-normalized
pub fn gpu_forward_dual(
    text: &str,
    weights: &LongformerWeights,
    projection: &CausalProjectionWeights,
    tokenizer: &Tokenizer,
    hint_guidance: Option<&context_graph_core::traits::CausalHintGuidance>,
) -> EmbeddingResult<(Vec<f32>, Vec<f32>)> {
    let device = weights.device;
    let config = &weights.config;

    // === TOKENIZATION ===
    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| EmbeddingError::TokenizationError {
            model_id: ModelId::Causal,
            message: format!("CausalModel tokenization failed: {}", e),
        })?;

    let token_ids: Vec<u32> = encoding.get_ids().to_vec();
    let attention_mask: Vec<f32> = encoding
        .get_attention_mask()
        .iter()
        .map(|&m| m as f32)
        .collect();

    // Truncate to max_position_embeddings if needed
    let max_len = config.max_position_embeddings.min(CAUSAL_MAX_TOKENS);
    let seq_len = token_ids.len().min(max_len);
    let token_ids = &token_ids[..seq_len];
    let attention_mask = &attention_mask[..seq_len];

    // === CAUSAL MARKER DETECTION ===
    // Detect cause/effect indicator words, enhanced with LLM guidance if available
    let markers = detect_causal_markers_with_hints(text, &encoding, hint_guidance);

    // Generate per-token weights for cause and effect embeddings
    let cause_weights = markers.cause_weights(seq_len);
    let effect_weights = markers.effect_weights(seq_len);

    // Log marker detection for debugging
    if markers.cause_marker_indices.len() + markers.effect_marker_indices.len() > 0 {
        tracing::debug!(
            "E5 causal markers detected: {} cause ({} static + {} LLM), {} effect ({} static + {} LLM), strength: {:.2}, boost: {:.2}",
            markers.cause_marker_indices.len(),
            markers.static_cause_count,
            markers.llm_cause_count,
            markers.effect_marker_indices.len(),
            markers.static_effect_count,
            markers.llm_effect_count,
            markers.causal_strength,
            markers.effective_boost,
        );
    }

    // === CREATE GPU TENSORS ===
    let input_ids = Tensor::from_slice(token_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("CausalModel input_ids tensor failed: {}", e),
        }
    })?;

    let attention_mask_tensor =
        Tensor::from_slice(attention_mask, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("CausalModel attention_mask tensor failed: {}", e),
            }
        })?;

    // Token type IDs (all zeros for Longformer)
    let token_type_ids: Vec<u32> = vec![0u32; seq_len];
    let token_type_tensor =
        Tensor::from_slice(&token_type_ids, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("CausalModel token_type tensor failed: {}", e),
            }
        })?;

    // Position IDs
    let position_ids: Vec<u32> = (0..seq_len as u32).collect();
    let position_tensor = Tensor::from_slice(&position_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("CausalModel position_ids tensor failed: {}", e),
        }
    })?;

    // === EMBEDDING LAYER ===
    let embeddings = compute_embeddings(
        &input_ids,
        &position_tensor,
        &token_type_tensor,
        weights,
        config,
        seq_len,
    )?;

    // === ENCODER LAYERS (single pass) ===
    let hidden_states = run_encoder(embeddings, &attention_mask_tensor, weights, config, seq_len)?;

    // === MARKER-WEIGHTED POOLING ===
    // Create differentiated embeddings by weighting causal markers differently
    let cause_pooled = marker_weighted_pooling(&hidden_states, &attention_mask_tensor, &cause_weights)?;
    let effect_pooled = marker_weighted_pooling(&hidden_states, &attention_mask_tensor, &effect_weights)?;

    // === DUAL PROJECTION ===
    // Apply W_cause projection to cause-pooled embedding
    let cause_projected = projection.project_cause(&cause_pooled)?;
    // Apply W_effect projection to effect-pooled embedding
    let effect_projected = projection.project_effect(&effect_pooled)?;

    // === L2 NORMALIZATION ===
    let cause_normalized = l2_normalize(&cause_projected)?;
    let effect_normalized = l2_normalize(&effect_projected)?;

    // === CONVERT TO VEC<F32> ===
    let cause_vec: Vec<f32> = cause_normalized
        .flatten_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel flatten cause output failed: {}", e),
        })?
        .to_vec1()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel cause to_vec1 failed: {}", e),
        })?;

    let effect_vec: Vec<f32> = effect_normalized
        .flatten_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel flatten effect output failed: {}", e),
        })?
        .to_vec1()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("CausalModel effect to_vec1 failed: {}", e),
        })?;

    Ok((cause_vec, effect_vec))
}
