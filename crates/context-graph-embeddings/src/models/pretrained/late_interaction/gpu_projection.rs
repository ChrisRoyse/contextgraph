//! Projection and output conversion for ColBERT.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};

use super::types::{ColBertProjection, TokenEmbeddings};

/// Project from 768D to 128D and L2 normalize each token.
pub(crate) fn project_and_normalize(
    hidden_states: Tensor,
    projection: &ColBertProjection,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len_proj, hidden_size_proj) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("LateInteractionModel projection get dims failed: {}", e),
            })?;

    let proj_dim = projection
        .weight
        .dim(0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel get proj_dim failed: {}", e),
        })?;

    // Flatten to [batch*seq, hidden] for Candle matmul compatibility
    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len_proj, hidden_size_proj))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel projection flatten failed: {}", e),
        })?;

    let projected = hidden_flat
        .matmul(
            &projection
                .weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("LateInteractionModel projection transpose failed: {}", e),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel projection matmul failed: {}", e),
        })?
        .reshape((batch_size, seq_len_proj, proj_dim))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel projection reshape failed: {}", e),
        })?;

    // L2 normalize each token
    let norm = projected
        .sqr()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel sqr failed: {}", e),
        })?
        .sum_keepdim(candle_core::D::Minus1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel sum norm failed: {}", e),
        })?
        .sqrt()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel sqrt norm failed: {}", e),
        })?;

    projected
        .broadcast_div(&(norm + 1e-9f64).map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel norm add eps failed: {}", e),
        })?)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel normalize failed: {}", e),
        })
}

/// Convert normalized batched tensor to one TokenEmbeddings row per input.
pub(crate) fn convert_to_token_embeddings_batch(
    normalized: Tensor,
    token_strings_by_row: Vec<Vec<String>>,
    attention_masks_by_row: Vec<Vec<f32>>,
    seq_len: usize,
) -> EmbeddingResult<Vec<TokenEmbeddings>> {
    let (batch_size, tensor_seq_len, _dim) =
        normalized.dims3().map_err(|e| EmbeddingError::GpuError {
            message: format!("LateInteractionModel batch convert get dims failed: {}", e),
        })?;

    if tensor_seq_len != seq_len {
        return Err(EmbeddingError::InvalidDimension {
            expected: seq_len,
            actual: tensor_seq_len,
        });
    }
    if token_strings_by_row.len() != batch_size || attention_masks_by_row.len() != batch_size {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: crate::types::ModelId::LateInteraction,
            expected: batch_size,
            actual: token_strings_by_row.len().min(attention_masks_by_row.len()),
            recovery_hint:
                "ColBERT true-batch conversion metadata row count must match tensor batch size"
                    .to_string(),
        });
    }

    let mut rows = Vec::with_capacity(batch_size);
    for row_idx in 0..batch_size {
        let row_tensor = normalized
            .get(row_idx)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("LateInteractionModel get batch row {row_idx} failed: {e}"),
            })?;
        let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(seq_len);
        for token_idx in 0..seq_len {
            let token_vec: Vec<f32> = row_tensor
                .get(token_idx)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "LateInteractionModel get batch row {row_idx} token {token_idx} failed: {e}"
                    ),
                })?
                .to_vec1()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "LateInteractionModel to_vec1 batch row {row_idx} token {token_idx} failed: {e}"
                    ),
                })?;
            vectors.push(token_vec);
        }

        let token_strings = token_strings_by_row[row_idx].clone();
        let attention_mask = &attention_masks_by_row[row_idx];
        if token_strings.len() != seq_len || attention_mask.len() != seq_len {
            return Err(EmbeddingError::InvalidDimension {
                expected: seq_len,
                actual: token_strings.len().min(attention_mask.len()),
            });
        }
        let mask: Vec<bool> = attention_mask.iter().map(|&m| m > 0.5).collect();
        rows.push(TokenEmbeddings::new(vectors, token_strings, mask)?);
    }

    Ok(rows)
}
