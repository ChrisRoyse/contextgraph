//! GPU forward pass implementation for the sparse SPLADE model.
//!
//! This module implements the main forward pass through the BERT encoder
//! and MLM head to produce sparse term importance scores.

use std::time::Instant;

use candle_core::Tensor;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{get_gpu_info, BertWeights};
use crate::types::{InputType, ModelId, ModelInput};

use super::embeddings::{compute_embeddings, run_encoder};
use super::mlm_head::{
    apply_splade_activation, convert_to_sparse, convert_to_sparse_batch, run_mlm_head,
};
use super::types::{
    validate_true_batch_vram_budget, MlmHeadWeights, SparseVector, SPARSE_MAX_TOKENS,
    SPARSE_VOCAB_SIZE,
};

/// Extract text content from model input.
pub(crate) fn extract_text(input: &ModelInput) -> EmbeddingResult<String> {
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
            model_id: ModelId::Sparse,
            input_type: InputType::from(input),
        }),
    }
}

/// Run GPU-accelerated SPLADE forward pass returning sparse vector.
pub(crate) fn gpu_forward_sparse(
    text: &str,
    weights: &BertWeights,
    tokenizer: &Tokenizer,
    mlm_head: &MlmHeadWeights,
) -> EmbeddingResult<SparseVector> {
    let device = weights.device();
    let config = &weights.config;

    // Tokenize input text
    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| EmbeddingError::TokenizationError {
            model_id: ModelId::Sparse,
            message: format!("SparseModel tokenization failed: {}", e),
        })?;

    let token_ids: Vec<u32> = encoding.get_ids().to_vec();
    let attention_mask: Vec<f32> = encoding
        .get_attention_mask()
        .iter()
        .map(|&m| m as f32)
        .collect();

    let max_len = config.max_position_embeddings.min(SPARSE_MAX_TOKENS);
    if token_ids.len() > max_len {
        return Err(EmbeddingError::InputTooLong {
            actual: token_ids.len(),
            max: max_len,
        });
    }
    if token_ids.is_empty() {
        return Err(EmbeddingError::TokenizationError {
            model_id: ModelId::Sparse,
            message: "SparseModel tokenization produced zero real tokens".to_string(),
        });
    }
    let seq_len = token_ids.len();
    let token_ids = &token_ids[..seq_len];
    let attention_mask = &attention_mask[..seq_len];

    // Create GPU tensors
    let input_ids = Tensor::from_slice(token_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("SparseModel input_ids tensor failed: {}", e),
        }
    })?;

    let attention_mask_tensor =
        Tensor::from_slice(attention_mask, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("SparseModel attention_mask tensor failed: {}", e),
            }
        })?;

    // Token type IDs (all zeros)
    let token_type_ids: Vec<u32> = vec![0u32; seq_len];
    let token_type_tensor =
        Tensor::from_slice(&token_type_ids, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("SparseModel token_type tensor failed: {}", e),
            }
        })?;

    // Position IDs
    let position_ids: Vec<u32> = (0..seq_len as u32).collect();
    let position_tensor = Tensor::from_slice(&position_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("SparseModel position_ids tensor failed: {}", e),
        }
    })?;

    // === EMBEDDING LAYER ===
    let embeddings = compute_embeddings(
        &input_ids,
        &position_tensor,
        &token_type_tensor,
        weights,
        config,
        1,
        seq_len,
    )?;

    // === ENCODER LAYERS ===
    let hidden_states = run_encoder(embeddings, &attention_mask_tensor, weights, config)?;

    // === MLM HEAD ===
    let logits = run_mlm_head(&hidden_states, mlm_head, config)?;

    // === SPLADE ACTIVATION ===
    let sparse_vector = apply_splade_activation(logits, &attention_mask_tensor)?;

    // Convert to sparse format
    convert_to_sparse(sparse_vector)
}

/// Run GPU-accelerated SPLADE forward pass for a true padded batch.
pub(crate) fn gpu_forward_sparse_batch(
    texts: &[String],
    weights: &BertWeights,
    tokenizer: &Tokenizer,
    mlm_head: &MlmHeadWeights,
    model_id: ModelId,
) -> EmbeddingResult<Vec<SparseVector>> {
    let started_at = Instant::now();
    if texts.is_empty() {
        return Err(EmbeddingError::TrueBatchEmpty {
            model_id,
            recovery_hint: "submit at least one SPLADE input; empty batches are invalid"
                .to_string(),
        });
    }

    let device = weights.device();
    let config = &weights.config;
    let max_len = config.max_position_embeddings.min(SPARSE_MAX_TOKENS);
    let batch_size = texts.len();
    let pad_id = pad_token_id(tokenizer, model_id)?;

    let encodings = texts
        .iter()
        .map(|text| {
            tokenizer
                .encode(text.as_str(), true)
                .map_err(|e| EmbeddingError::TokenizationError {
                    model_id,
                    message: format!("SPLADE true-batch tokenization failed: {}", e),
                })
        })
        .collect::<EmbeddingResult<Vec<_>>>()?;

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
            model_id,
            message: format!(
                "SPLADE true-batch tokenization produced zero real tokens at batch_index={idx}"
            ),
        });
    }

    let actual_max_len = token_lengths.iter().copied().max().unwrap_or(0);
    if actual_max_len == 0 {
        return Err(EmbeddingError::TrueBatchEmpty {
            model_id,
            recovery_hint: "tokenization produced no usable SPLADE tokens for the batch"
                .to_string(),
        });
    }

    let mut all_token_ids = vec![pad_id; batch_size * actual_max_len];
    let mut all_attention_mask = vec![0.0f32; batch_size * actual_max_len];
    let mut all_token_type_ids = vec![0u32; batch_size * actual_max_len];
    let mut all_position_ids = vec![0u32; batch_size * actual_max_len];

    for (batch_idx, encoding) in encodings.iter().enumerate() {
        let seq_len = token_lengths[batch_idx];
        let offset = batch_idx * actual_max_len;
        let raw_mask = encoding.get_attention_mask();
        for (token_idx, &token_id) in encoding.get_ids()[..seq_len].iter().enumerate() {
            all_token_ids[offset + token_idx] = token_id;
            all_attention_mask[offset + token_idx] =
                raw_mask.get(token_idx).copied().unwrap_or(1) as f32;
            all_token_type_ids[offset + token_idx] = 0;
            all_position_ids[offset + token_idx] = token_idx as u32;
        }
    }

    let batch_tensor_bytes =
        (all_token_ids.len() + all_token_type_ids.len() + all_position_ids.len())
            * std::mem::size_of::<u32>()
            + all_attention_mask.len() * std::mem::size_of::<f32>();
    let vram_plan =
        validate_true_batch_vram_budget(model_id, batch_tensor_bytes, get_gpu_info().total_vram)?;

    let input_ids = Tensor::from_slice(&all_token_ids, (batch_size, actual_max_len), device)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("SPLADE true-batch input_ids tensor failed: {}", e),
        })?;
    let attention_mask_tensor =
        Tensor::from_slice(&all_attention_mask, (batch_size, actual_max_len), device).map_err(
            |e| EmbeddingError::GpuError {
                message: format!("SPLADE true-batch attention_mask tensor failed: {}", e),
            },
        )?;
    let token_type_tensor =
        Tensor::from_slice(&all_token_type_ids, (batch_size, actual_max_len), device).map_err(
            |e| EmbeddingError::GpuError {
                message: format!("SPLADE true-batch token_type tensor failed: {}", e),
            },
        )?;
    let position_tensor =
        Tensor::from_slice(&all_position_ids, (batch_size, actual_max_len), device).map_err(
            |e| EmbeddingError::GpuError {
                message: format!("SPLADE true-batch position_ids tensor failed: {}", e),
            },
        )?;

    let embeddings = compute_embeddings(
        &input_ids,
        &position_tensor,
        &token_type_tensor,
        weights,
        config,
        batch_size,
        actual_max_len,
    )?;
    let hidden_states = run_encoder(embeddings, &attention_mask_tensor, weights, config)?;
    let logits = run_mlm_head(&hidden_states, mlm_head, config)?;
    let sparse_scores = apply_splade_activation(logits, &attention_mask_tensor)?;
    let sparse_vectors = convert_to_sparse_batch(sparse_scores, model_id)?;
    let nonzero_counts = sparse_vectors
        .iter()
        .map(SparseVector::nnz)
        .collect::<Vec<_>>();
    let sparse_densities = nonzero_counts
        .iter()
        .map(|nnz| *nnz as f32 / SPARSE_VOCAB_SIZE as f32)
        .collect::<Vec<_>>();
    let padding_tokens = token_lengths
        .iter()
        .map(|token_len| actual_max_len - token_len)
        .sum::<usize>();

    tracing::info!(
        target: "context_graph_embeddings::true_batch",
        model_id = ?model_id,
        model = "SparseModel",
        batch_size,
        max_seq_len = actual_max_len,
        token_lengths = ?token_lengths,
        padding_tokens,
        nonzero_counts = ?nonzero_counts,
        sparse_densities = ?sparse_densities,
        output_count = sparse_vectors.len(),
        sparse_dim = SPARSE_VOCAB_SIZE,
        model_vram_bytes = weights.vram_bytes(),
        planned_model_vram_bytes = vram_plan.model_vram_bytes,
        planned_batch_tensor_bytes = vram_plan.batch_tensor_bytes,
        planned_required_vram_bytes = vram_plan.required_bytes,
        planned_available_vram_bytes = vram_plan.available_bytes,
        latency_us = started_at.elapsed().as_micros() as u64,
        "SPLADE true-batch forward completed"
    );

    if sparse_vectors.len() != batch_size {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id,
            expected: batch_size,
            actual: sparse_vectors.len(),
            recovery_hint:
                "inspect SPLADE true-batch sparse pooling/chunking; partial outputs are invalid"
                    .to_string(),
        });
    }

    Ok(sparse_vectors)
}

fn pad_token_id(tokenizer: &Tokenizer, model_id: ModelId) -> EmbeddingResult<u32> {
    tokenizer
        .get_padding()
        .map(|padding| padding.pad_id)
        .or_else(|| tokenizer.token_to_id("[PAD]"))
        .ok_or_else(|| EmbeddingError::ConfigError {
            message: format!(
                "SPLADE true-batch requires a pad token id for {model_id:?}; tokenizer has no padding config or [PAD] token"
            ),
        })
}
