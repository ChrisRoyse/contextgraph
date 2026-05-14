//! GPU-accelerated forward pass for ColBERT late-interaction model.

use candle_core::Tensor;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{get_gpu_info, BertConfig, BertWeights};
use crate::types::ModelId;

use super::gpu_encoder::run_encoder_layers;
use super::gpu_projection::{convert_to_token_embeddings_batch, project_and_normalize};
use super::gpu_utils::layer_norm;
use super::types::{
    validate_late_interaction_batch_vram_budget, ColBertProjection, TokenEmbeddings,
    LATE_INTERACTION_MAX_TOKENS,
};

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
    let texts = [text.to_string()];
    let mut rows = gpu_forward_tokens_batch(&texts, weights, projection, tokenizer)?;
    if rows.len() != 1 {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: ModelId::LateInteraction,
            expected: 1,
            actual: rows.len(),
            recovery_hint: "ColBERT single-token forward must return exactly one row".to_string(),
        });
    }
    Ok(rows.remove(0))
}

/// Run GPU-accelerated ColBERT forward pass for a true padded batch.
pub(crate) fn gpu_forward_tokens_batch(
    texts: &[String],
    weights: &BertWeights,
    projection: &ColBertProjection,
    tokenizer: &Tokenizer,
) -> EmbeddingResult<Vec<TokenEmbeddings>> {
    let started_at = std::time::Instant::now();
    if texts.is_empty() {
        return Err(EmbeddingError::TrueBatchEmpty {
            model_id: ModelId::LateInteraction,
            recovery_hint: "submit at least one ColBERT input; empty batches are invalid"
                .to_string(),
        });
    }

    let device = weights.device();
    let config = &weights.config;
    let max_len = config
        .max_position_embeddings
        .min(LATE_INTERACTION_MAX_TOKENS);
    let batch_size = texts.len();
    let pad_id = pad_token_id(tokenizer)?;

    let encodings = tokenizer
        .encode_batch(texts.iter().map(String::as_str).collect::<Vec<_>>(), true)
        .map_err(|e| EmbeddingError::TokenizationError {
            model_id: ModelId::LateInteraction,
            message: format!("LateInteractionModel true-batch tokenization failed: {}", e),
        })?;
    if encodings.len() != batch_size {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: ModelId::LateInteraction,
            expected: batch_size,
            actual: encodings.len(),
            recovery_hint:
                "ColBERT tokenizer encode_batch output count must match input batch size"
                    .to_string(),
        });
    }

    let token_lengths = encodings
        .iter()
        .map(|encoding| encoding.get_ids().len())
        .collect::<Vec<_>>();

    if let Some((_idx, actual)) = token_lengths
        .iter()
        .copied()
        .enumerate()
        .find(|(_, token_len)| *token_len > max_len)
    {
        return Err(EmbeddingError::InputTooLong {
            actual,
            max: max_len,
        });
    }

    if let Some((idx, _)) = token_lengths
        .iter()
        .enumerate()
        .find(|(_, token_len)| **token_len == 0)
    {
        return Err(EmbeddingError::TokenizationError {
            model_id: ModelId::LateInteraction,
            message: format!(
                "LateInteractionModel true-batch tokenization produced zero real tokens at batch_index={idx}"
            ),
        });
    }

    let actual_max_len = token_lengths.iter().copied().max().unwrap_or(0);
    if actual_max_len == 0 {
        return Err(EmbeddingError::TrueBatchEmpty {
            model_id: ModelId::LateInteraction,
            recovery_hint: "tokenization produced no usable ColBERT tokens for the batch"
                .to_string(),
        });
    }

    let pad_token = tokenizer
        .id_to_token(pad_id)
        .unwrap_or_else(|| "[PAD]".to_string());
    let mut all_token_ids = vec![pad_id; batch_size * actual_max_len];
    let mut all_attention_mask = vec![0.0f32; batch_size * actual_max_len];
    let mut all_token_type_ids = vec![0u32; batch_size * actual_max_len];
    let mut all_position_ids = vec![0u32; batch_size * actual_max_len];
    let mut token_strings_by_row = Vec::with_capacity(batch_size);
    let mut attention_masks_by_row = Vec::with_capacity(batch_size);

    for (batch_idx, encoding) in encodings.iter().enumerate() {
        let seq_len = token_lengths[batch_idx];
        let offset = batch_idx * actual_max_len;
        let raw_mask = encoding.get_attention_mask();
        let mut token_strings = encoding
            .get_tokens()
            .iter()
            .map(|token| token.to_string())
            .collect::<Vec<_>>();
        let mut attention_mask = raw_mask.iter().map(|&m| m as f32).collect::<Vec<_>>();

        for (token_idx, &token_id) in encoding.get_ids()[..seq_len].iter().enumerate() {
            all_token_ids[offset + token_idx] = token_id;
            all_attention_mask[offset + token_idx] =
                raw_mask.get(token_idx).copied().unwrap_or(1) as f32;
            all_token_type_ids[offset + token_idx] = 0;
            all_position_ids[offset + token_idx] = token_idx as u32;
        }

        token_strings.resize(actual_max_len, pad_token.clone());
        attention_mask.resize(actual_max_len, 0.0);
        token_strings_by_row.push(token_strings);
        attention_masks_by_row.push(attention_mask);
    }

    let batch_tensor_bytes =
        (all_token_ids.len() + all_token_type_ids.len() + all_position_ids.len())
            * std::mem::size_of::<u32>()
            + all_attention_mask.len() * std::mem::size_of::<f32>();
    let vram_plan =
        validate_late_interaction_batch_vram_budget(batch_tensor_bytes, get_gpu_info().total_vram)?;

    let (input_ids, attention_mask_tensor, token_type_tensor, position_tensor) =
        create_input_tensors(
            &all_token_ids,
            &all_attention_mask,
            &all_token_type_ids,
            &all_position_ids,
            batch_size,
            actual_max_len,
            device,
        )?;

    // === EMBEDDING LAYER ===
    let embeddings = compute_embeddings(
        &input_ids,
        &position_tensor,
        &token_type_tensor,
        weights,
        config,
        batch_size,
        actual_max_len,
    )?;

    // === ENCODER LAYERS ===
    let hidden_states = run_encoder_layers(embeddings, &attention_mask_tensor, weights, config)?;

    // === PROJECTION AND NORMALIZATION ===
    let normalized = project_and_normalize(hidden_states, projection)?;

    // === CONVERT TO TokenEmbeddings ===
    let rows = convert_to_token_embeddings_batch(
        normalized,
        token_strings_by_row,
        attention_masks_by_row,
        actual_max_len,
    )?;

    if rows.len() != batch_size {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: ModelId::LateInteraction,
            expected: batch_size,
            actual: rows.len(),
            recovery_hint:
                "inspect ColBERT true-batch projection/conversion; partial token outputs are invalid"
                    .to_string(),
        });
    }

    let valid_token_counts = rows
        .iter()
        .map(TokenEmbeddings::valid_token_count)
        .collect::<Vec<_>>();
    let padding_tokens = rows
        .iter()
        .map(|row| row.vectors.len().saturating_sub(row.valid_token_count()))
        .sum::<usize>();
    let token_dim = rows
        .first()
        .and_then(|row| row.vectors.first())
        .map_or(0, Vec::len);

    tracing::info!(
        target: "context_graph_embeddings::true_batch",
        model_id = ?ModelId::LateInteraction,
        model = "LateInteractionModel",
        batch_size,
        max_seq_len = actual_max_len,
        token_lengths = ?token_lengths,
        valid_token_counts = ?valid_token_counts,
        padding_tokens,
        output_count = rows.len(),
        token_dim,
        model_vram_bytes = weights.vram_bytes(),
        planned_model_vram_bytes = vram_plan.model_vram_bytes,
        planned_batch_tensor_bytes = vram_plan.batch_tensor_bytes,
        planned_required_vram_bytes = vram_plan.required_bytes,
        planned_available_vram_bytes = vram_plan.available_bytes,
        latency_us = started_at.elapsed().as_micros() as u64,
        "ColBERT true-batch forward completed"
    );

    Ok(rows)
}

/// Create GPU tensors for input.
fn create_input_tensors(
    token_ids: &[u32],
    attention_mask: &[f32],
    token_type_ids: &[u32],
    position_ids: &[u32],
    batch_size: usize,
    seq_len: usize,
    device: &candle_core::Device,
) -> EmbeddingResult<(Tensor, Tensor, Tensor, Tensor)> {
    let input_ids = Tensor::from_slice(token_ids, (batch_size, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("LateInteractionModel input_ids tensor failed: {}", e),
        }
    })?;

    let attention_mask_tensor = Tensor::from_slice(attention_mask, (batch_size, seq_len), device)
        .map_err(|e| EmbeddingError::GpuError {
        message: format!("LateInteractionModel attention_mask tensor failed: {}", e),
    })?;

    let token_type_tensor = Tensor::from_slice(token_type_ids, (batch_size, seq_len), device)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel token_type tensor failed: {}", e),
        })?;

    let position_tensor =
        Tensor::from_slice(position_ids, (batch_size, seq_len), device).map_err(|e| {
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
    batch_size: usize,
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
        .reshape((batch_size, seq_len, config.hidden_size))
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
        .reshape((batch_size, seq_len, config.hidden_size))
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
        .reshape((batch_size, seq_len, config.hidden_size))
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

fn pad_token_id(tokenizer: &Tokenizer) -> EmbeddingResult<u32> {
    tokenizer
        .get_padding()
        .map(|padding| padding.pad_id)
        .or_else(|| tokenizer.token_to_id("[PAD]"))
        .ok_or_else(|| EmbeddingError::ConfigError {
            message:
                "ColBERT true-batch requires a pad token id; tokenizer has no padding config or [PAD] token"
                    .to_string(),
        })
}
