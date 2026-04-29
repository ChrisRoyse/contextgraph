//! XLM-RoBERTa embedding computation for the BGE-M3 Dense model.
//!
//! Differs from BERT embeddings in two places:
//!
//! 1. **Position IDs**: XLM-RoBERTa computes position IDs starting at
//!    `padding_idx + 1` and skips pad positions. For a non-padded single-sequence
//!    input this reduces to `position_ids[i] = i + XLM_R_POSITION_OFFSET`, where
//!    `XLM_R_POSITION_OFFSET = 2` (pad=1). BERT uses plain `0..seq_len`.
//!
//! 2. **Token-type IDs**: XLM-RoBERTa still has a `token_type_embeddings` table
//!    of size 1, populated with zeros for single-sequence encodes. We look up
//!    index 0 identically to BERT; the table is simply smaller.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::BertWeights;

use super::constants::XLM_R_POSITION_OFFSET;
use super::layer_norm::layer_norm;

/// Compute initial embeddings (word + position + token_type) for XLM-RoBERTa.
///
/// # Arguments
/// * `input_ids` - Tokenised input, shape `[1, seq_len]`.
/// * `seq_len`   - Real sequence length (post-truncation).
/// * `weights`   - Loaded XLM-R weights (BertWeights flavour).
/// * `config`    - Parsed `config.json` (includes `max_position_embeddings`,
///   which for BGE-M3 is 8194 — 8192 usable positions + the `<pad>` +
///   `<pad>+1` offset).
/// * `device`    - GPU device singleton.
pub fn compute_embeddings(
    input_ids: &Tensor,
    seq_len: usize,
    weights: &BertWeights,
    config: &crate::gpu::BertConfig,
    device: &candle_core::Device,
) -> EmbeddingResult<Tensor> {
    // XLM-RoBERTa position IDs start at padding_idx + 1 = 2.
    // For a non-padded single sequence this is just `[2, 3, 4, ...]`.
    // See HuggingFace `create_position_ids_from_input_ids` helper.
    let position_ids: Vec<u32> = (0..seq_len as u32)
        .map(|i| i + XLM_R_POSITION_OFFSET)
        .collect();
    let position_tensor = Tensor::from_slice(&position_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("BgeM3Dense position_ids tensor failed: {}", e),
        }
    })?;

    // Token type IDs (always 0 for single sequence).
    let token_type_ids: Vec<u32> = vec![0u32; seq_len];
    let token_type_tensor =
        Tensor::from_slice(&token_type_ids, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("BgeM3Dense token_type tensor failed: {}", e),
            }
        })?;

    // Word embedding lookup.
    let word_embeds = weights
        .embeddings
        .word_embeddings
        .index_select(
            &input_ids
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("BgeM3Dense flatten input_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense word embedding lookup failed: {}", e),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense word embedding reshape failed: {}", e),
        })?;

    // Position embedding lookup (offset semantics handled above).
    let position_embeds = weights
        .embeddings
        .position_embeddings
        .index_select(
            &position_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("BgeM3Dense flatten position_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense position embedding lookup failed \
                 (max_position_embeddings={}, requested up to {}): {}",
                config.max_position_embeddings,
                seq_len as u32 + XLM_R_POSITION_OFFSET - 1,
                e
            ),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense position embedding reshape failed: {}", e),
        })?;

    // Token-type embedding lookup (table of size 1 for XLM-R, all zeros).
    let token_type_embeds = weights
        .embeddings
        .token_type_embeddings
        .index_select(
            &token_type_tensor
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("BgeM3Dense flatten token_type_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense token_type embedding lookup failed: {}", e),
        })?
        .reshape((1, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense token_type embedding reshape failed: {}", e),
        })?;

    let embeddings = ((word_embeds + position_embeds).map_err(|e| EmbeddingError::GpuError {
        message: format!("BgeM3Dense embedding add 1 failed: {}", e),
    })? + token_type_embeds)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense embedding add 2 failed: {}", e),
        })?;

    layer_norm(
        &embeddings,
        &weights.embeddings.layer_norm_weight,
        &weights.embeddings.layer_norm_bias,
        config.layer_norm_eps,
    )
}
