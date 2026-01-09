//! GPU-accelerated forward pass for ColBERT late-interaction model.

use candle_core::Tensor;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{BertConfig, BertWeights};
use crate::types::ModelId;

use super::gpu_encoder::run_encoder_layers;
use super::gpu_projection::{convert_to_token_embeddings, project_and_normalize};
use super::gpu_utils::layer_norm;
use super::types::{ColBertProjection, TokenEmbeddings, LATE_INTERACTION_MAX_TOKENS};

/// Run GPU-accelerated ColBERT forward pass for per-token embeddings.
///
/// # GPU Pipeline
///
/// 1. Tokenize input text to token IDs
/// 2. Create GPU tensors for input_ids, attention_mask, token_type_ids
/// 3. Embedding lookup: word + position + token_type
/// 4. Apply LayerNorm to embeddings
/// 5. Run 12 transformer encoder layers (self-attention + FFN)
/// 6. Project hidden states from 768D to 128D per token
/// 7. L2 normalize each token embedding
/// 8. Convert back to TokenEmbeddings
pub(crate) fn gpu_forward_tokens(
    text: &str,
    weights: &BertWeights,
    projection: &ColBertProjection,
    tokenizer: &Tokenizer,
) -> EmbeddingResult<TokenEmbeddings> {
    let device = weights.device();
    let config = &weights.config;

    // Tokenize input text
    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| EmbeddingError::TokenizationError {
            model_id: ModelId::LateInteraction,
            message: format!("LateInteractionModel tokenization failed: {}", e),
        })?;

    let token_ids: Vec<u32> = encoding.get_ids().to_vec();
    let attention_mask: Vec<f32> = encoding
        .get_attention_mask()
        .iter()
        .map(|&m| m as f32)
        .collect();
    let token_strings: Vec<String> = encoding
        .get_tokens()
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Truncate to max_position_embeddings if needed
    let max_len = config
        .max_position_embeddings
        .min(LATE_INTERACTION_MAX_TOKENS);
    let seq_len = token_ids.len().min(max_len);
    let token_ids = &token_ids[..seq_len];
    let attention_mask = &attention_mask[..seq_len];
    let token_strings: Vec<String> = token_strings[..seq_len].to_vec();

    // Create GPU tensors
    let (input_ids, attention_mask_tensor, token_type_tensor, position_tensor) =
        create_input_tensors(token_ids, attention_mask, seq_len, device)?;

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
    let hidden_states = run_encoder_layers(embeddings, &attention_mask_tensor, weights, config)?;

    // === PROJECTION AND NORMALIZATION ===
    let normalized = project_and_normalize(hidden_states, projection)?;

    // === CONVERT TO TokenEmbeddings ===
    convert_to_token_embeddings(normalized, token_strings, attention_mask, seq_len)
}

/// Create GPU tensors for input.
fn create_input_tensors(
    token_ids: &[u32],
    attention_mask: &[f32],
    seq_len: usize,
    device: &candle_core::Device,
) -> EmbeddingResult<(Tensor, Tensor, Tensor, Tensor)> {
    let input_ids = Tensor::from_slice(token_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("LateInteractionModel input_ids tensor failed: {}", e),
        }
    })?;

    let attention_mask_tensor =
        Tensor::from_slice(attention_mask, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("LateInteractionModel attention_mask tensor failed: {}", e),
            }
        })?;

    let token_type_ids: Vec<u32> = vec![0u32; seq_len];
    let token_type_tensor =
        Tensor::from_slice(&token_type_ids, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("LateInteractionModel token_type tensor failed: {}", e),
            }
        })?;

    let position_ids: Vec<u32> = (0..seq_len as u32).collect();
    let position_tensor = Tensor::from_slice(&position_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("LateInteractionModel position_ids tensor failed: {}", e),
        }
    })?;

    Ok((
        input_ids,
        attention_mask_tensor,
        token_type_tensor,
        position_tensor,
    ))
}

/// Compute BERT embeddings: word + position + token_type with LayerNorm.
fn compute_embeddings(
    input_ids: &Tensor,
    position_tensor: &Tensor,
    token_type_tensor: &Tensor,
    weights: &BertWeights,
    config: &BertConfig,
    seq_len: usize,
) -> EmbeddingResult<Tensor> {
    let word_embeds = weights
        .embeddings
        .word_embeddings
        .index_select(
            &input_ids
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("LateInteractionModel flatten input_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel word embedding lookup failed: {}", e),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel word embedding reshape failed: {}", e),
        })?;

    let position_embeds = weights
        .embeddings
        .position_embeddings
        .index_select(
            &position_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("LateInteractionModel flatten position_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel position embedding lookup failed: {}",
                e
            ),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel position embedding reshape failed: {}",
                e
            ),
        })?;

    let token_type_embeds = weights
        .embeddings
        .token_type_embeddings
        .index_select(
            &token_type_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("LateInteractionModel flatten token_type_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel token_type embedding lookup failed: {}",
                e
            ),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "LateInteractionModel token_type embedding reshape failed: {}",
                e
            ),
        })?;

    // Sum embeddings
    let embeddings = ((word_embeds + position_embeds).map_err(|e| EmbeddingError::GpuError {
        message: format!("LateInteractionModel embedding add 1 failed: {}", e),
    })? + token_type_embeds)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel embedding add 2 failed: {}", e),
        })?;

    // Apply LayerNorm to embeddings
    layer_norm(
        &embeddings,
        &weights.embeddings.layer_norm_weight,
        &weights.embeddings.layer_norm_bias,
        config.layer_norm_eps,
    )
}
