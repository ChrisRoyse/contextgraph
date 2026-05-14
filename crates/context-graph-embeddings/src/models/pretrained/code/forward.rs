//! GPU forward pass implementation for Qwen2 (Qodo-Embed).
//!
//! Contains the main Qwen2 decoder forward pass with:
//! - Token embedding lookup
//! - RoPE position encoding
//! - Grouped-Query Attention (GQA) layers
//! - SwiGLU FFN
//! - Last-token pooling for embedding output

use std::time::Instant;

use candle_core::{DType, Tensor};
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::models::attention::AttentionStrategy;
use crate::types::ModelId;

use super::constants::CODE_MAX_TOKENS;
use super::layers::{decoder_layer_forward, rms_norm};
use super::position::RopeCache;
use super::weights::QwenWeights;

/// GPU-accelerated forward pass for Qwen2 (Qodo-Embed).
///
/// Qwen2 architecture uses:
/// - RoPE (Rotary Position Embedding) instead of absolute position embeddings
/// - RMSNorm instead of LayerNorm
/// - Pre-norm architecture (norm before attention/FFN)
/// - Grouped-Query Attention (GQA) with 12 query heads, 2 KV heads
/// - SwiGLU activation in FFN
/// - Last-token pooling for sentence embedding
pub fn gpu_forward(
    text: &str,
    weights: &QwenWeights,
    tokenizer: &Tokenizer,
    strategy: &dyn AttentionStrategy,
) -> EmbeddingResult<Vec<f32>> {
    let device = weights.device;
    let config = &weights.config;

    // Tokenize input text
    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| EmbeddingError::TokenizationError {
            model_id: ModelId::Code,
            message: format!("Qwen2 tokenization failed: {}", e),
        })?;

    let token_ids: Vec<u32> = encoding.get_ids().to_vec();

    // Get attention mask from tokenizer, or create one if not provided
    // Raw BPE tokenizers may not generate attention masks
    let raw_mask = encoding.get_attention_mask();
    let attention_mask: Vec<f32> = if raw_mask.is_empty() || raw_mask.iter().all(|&m| m == 0) {
        // Create attention mask with all 1s (no padding) if tokenizer doesn't provide one
        vec![1.0f32; token_ids.len()]
    } else {
        raw_mask.iter().map(|&m| m as f32).collect()
    };

    // Truncate to max tokens if needed
    let seq_len = token_ids.len().min(CODE_MAX_TOKENS);
    let token_ids = &token_ids[..seq_len];
    let attention_mask = &attention_mask[..seq_len];

    // Create GPU tensors
    let input_ids = Tensor::from_slice(token_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("Qwen2 input_ids tensor failed: {}", e),
        }
    })?;

    let attention_mask_tensor =
        Tensor::from_slice(attention_mask, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Qwen2 attention_mask tensor failed: {}", e),
            }
        })?;

    // === EMBEDDING LAYER ===
    let hidden_states = embed_tokens(&input_ids, weights, 1, seq_len)?;

    // === COMPUTE ROPE CACHE ===
    let rope_cache = RopeCache::new(
        seq_len,
        config.head_dim,
        config.rope_theta,
        device,
        DType::F16,
    )?;

    // Create extended attention mask for broadcasting
    let extended_attention_mask = create_extended_attention_mask(&attention_mask_tensor)?;

    // === DECODER LAYERS ===
    let mut hidden_states = hidden_states;
    for (layer_idx, layer) in weights.layers.iter().enumerate() {
        hidden_states = decoder_layer_forward(
            &hidden_states,
            layer,
            &extended_attention_mask,
            &rope_cache,
            config,
            layer_idx,
            strategy,
        )?;
    }

    // === FINAL LAYER NORM ===
    hidden_states = rms_norm(&hidden_states, &weights.norm_weight, config.rms_norm_eps)?;

    // === LAST-TOKEN POOLING & NORMALIZE ===
    // Qodo-Embed uses last token pooling (pooling_mode_lasttoken: true)
    let pooled = last_token_pool(&hidden_states, &attention_mask_tensor)?;
    l2_normalize(&pooled)
}

/// GPU-accelerated true-batch forward pass for Qwen2 (Qodo-Embed).
pub fn gpu_forward_batch(
    texts: &[String],
    weights: &QwenWeights,
    tokenizer: &Tokenizer,
    strategy: &dyn AttentionStrategy,
) -> EmbeddingResult<Vec<Vec<f32>>> {
    let started_at = Instant::now();
    if texts.is_empty() {
        return Err(EmbeddingError::TrueBatchEmpty {
            model_id: ModelId::Code,
            recovery_hint: "submit at least one CodeModel input; empty batches are invalid"
                .to_string(),
        });
    }

    let device = weights.device;
    let config = &weights.config;
    let batch_size = texts.len();
    let pad_id = pad_token_id(tokenizer)?;

    let encodings = texts
        .iter()
        .map(|text| {
            tokenizer
                .encode(text.as_str(), true)
                .map_err(|e| EmbeddingError::TokenizationError {
                    model_id: ModelId::Code,
                    message: format!("Qwen2 true-batch tokenization failed: {}", e),
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
        .find(|(_, token_len)| *token_len > CODE_MAX_TOKENS)
    {
        return Err(EmbeddingError::InputTooLong {
            actual,
            max: CODE_MAX_TOKENS,
        });
    }

    if let Some((idx, _)) = token_lengths
        .iter()
        .enumerate()
        .find(|(_, token_len)| **token_len == 0)
    {
        return Err(EmbeddingError::TokenizationError {
            model_id: ModelId::Code,
            message: format!(
                "Qwen2 true-batch tokenization produced zero real tokens at batch_index={}",
                idx
            ),
        });
    }

    let actual_max_len = token_lengths.iter().copied().max().unwrap_or(0);
    if actual_max_len == 0 {
        return Err(EmbeddingError::TrueBatchEmpty {
            model_id: ModelId::Code,
            recovery_hint: "tokenization produced no usable code tokens for the batch".to_string(),
        });
    }

    let mut all_token_ids = vec![pad_id; batch_size * actual_max_len];
    let mut all_attention_mask = vec![0.0f32; batch_size * actual_max_len];
    for (batch_idx, encoding) in encodings.iter().enumerate() {
        let seq_len = token_lengths[batch_idx];
        let offset = batch_idx * actual_max_len;
        let raw_mask = encoding.get_attention_mask();
        for (token_idx, &token_id) in encoding.get_ids()[..seq_len].iter().enumerate() {
            all_token_ids[offset + token_idx] = token_id;
            all_attention_mask[offset + token_idx] =
                raw_mask.get(token_idx).copied().unwrap_or(1) as f32;
        }
    }

    let input_ids = Tensor::from_slice(&all_token_ids, (batch_size, actual_max_len), device)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 true-batch input_ids tensor failed: {}", e),
        })?;
    let attention_mask_tensor =
        Tensor::from_slice(&all_attention_mask, (batch_size, actual_max_len), device).map_err(
            |e| EmbeddingError::GpuError {
                message: format!("Qwen2 true-batch attention_mask tensor failed: {}", e),
            },
        )?;

    let mut hidden_states = embed_tokens(&input_ids, weights, batch_size, actual_max_len)?;
    let rope_cache = RopeCache::new(
        actual_max_len,
        config.head_dim,
        config.rope_theta,
        device,
        DType::F16,
    )?;
    let extended_attention_mask = create_extended_attention_mask(&attention_mask_tensor)?;

    for (layer_idx, layer) in weights.layers.iter().enumerate() {
        hidden_states = decoder_layer_forward(
            &hidden_states,
            layer,
            &extended_attention_mask,
            &rope_cache,
            config,
            layer_idx,
            strategy,
        )?;
    }

    hidden_states = rms_norm(&hidden_states, &weights.norm_weight, config.rms_norm_eps)?;
    let pooled = last_token_pool_batch(&hidden_states, &token_lengths)?;
    let normalized = l2_normalize_tensor(&pooled)?;
    let results = tensor_to_batch_vecs(&normalized, batch_size)?;
    let output_dim = results.first().map_or(0, Vec::len);
    let padding_tokens = token_lengths
        .iter()
        .map(|token_len| actual_max_len - token_len)
        .sum::<usize>();
    let batch_tensor_bytes = all_token_ids.len() * std::mem::size_of::<u32>()
        + all_attention_mask.len() * std::mem::size_of::<f32>();

    tracing::info!(
        target: "context_graph_embeddings::true_batch",
        model_id = ?ModelId::Code,
        model = "CodeModel",
        batch_size,
        max_seq_len = actual_max_len,
        token_lengths = ?token_lengths,
        padding_tokens,
        output_count = results.len(),
        output_dim,
        model_vram_bytes = weights.vram_bytes(),
        batch_tensor_bytes,
        latency_us = started_at.elapsed().as_micros() as u64,
        "Qwen2 true-batch forward completed"
    );

    if results.len() != batch_size {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: ModelId::Code,
            expected: batch_size,
            actual: results.len(),
            recovery_hint:
                "inspect Qwen2 true-batch tensor pooling/chunking; partial outputs are invalid"
                    .to_string(),
        });
    }

    Ok(results)
}

fn pad_token_id(tokenizer: &Tokenizer) -> EmbeddingResult<u32> {
    tokenizer
        .get_padding()
        .map(|padding| padding.pad_id)
        .or_else(|| tokenizer.token_to_id("<|endoftext|>"))
        .or_else(|| tokenizer.token_to_id("<pad>"))
        .ok_or_else(|| EmbeddingError::ConfigError {
            message: "Qwen2 true-batch requires a pad token id; tokenizer has no padding config, <|endoftext|>, or <pad>".to_string(),
        })
}

/// Embed token IDs using token embeddings.
fn embed_tokens(
    input_ids: &Tensor,
    weights: &QwenWeights,
    batch_size: usize,
    seq_len: usize,
) -> EmbeddingResult<Tensor> {
    weights
        .embed_tokens
        .index_select(
            &input_ids
                .flatten_all()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 flatten input_ids failed: {}", e),
                })?,
            0,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 embedding lookup failed: {}", e),
        })?
        .reshape((batch_size, seq_len, weights.config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 embedding reshape failed: {}", e),
        })
}

/// Create extended attention mask for broadcasting.
///
/// Converts [batch, seq_len] mask to [batch, 1, 1, seq_len] for attention.
/// Masked positions (0) become -10000, unmasked positions (1) become 0.
fn create_extended_attention_mask(attention_mask: &Tensor) -> EmbeddingResult<Tensor> {
    let extended = attention_mask
        .unsqueeze(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 attention mask unsqueeze 1 failed: {}", e),
        })?
        .unsqueeze(2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 attention mask unsqueeze 2 failed: {}", e),
        })?;

    let ones = Tensor::ones_like(&extended).map_err(|e| EmbeddingError::GpuError {
        message: format!("Qwen2 create ones tensor failed: {}", e),
    })?;

    let inverted = ones
        .broadcast_sub(&extended)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 attention mask invert failed: {}", e),
        })?;

    // Convert to FP16 to match model dtype
    let inverted = inverted
        .to_dtype(DType::F16)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 attention mask dtype conversion failed: {}", e),
        })?;

    (inverted * (-10000.0f64)).map_err(|e| EmbeddingError::GpuError {
        message: format!("Qwen2 attention mask scale failed: {}", e),
    })
}

/// Last-token pooling for sentence embedding.
///
/// Returns the hidden state of the last non-padding token.
/// This is the pooling method used by Qodo-Embed (pooling_mode_lasttoken: true).
fn last_token_pool(hidden_states: &Tensor, attention_mask: &Tensor) -> EmbeddingResult<Tensor> {
    let (_batch_size, seq_len, _hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Qwen2 last_token_pool get dims failed: {}", e),
            })?;

    // Find the index of the last non-padding token
    // Sum the attention mask to get sequence lengths, then subtract 1 for index
    let seq_lengths = attention_mask
        .sum(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 attention mask sum failed: {}", e),
        })?;

    // Get the sequence length value (for batch size 1)
    let last_idx: f32 = seq_lengths
        .flatten_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 seq_lengths flatten failed: {}", e),
        })?
        .to_vec1()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 seq_lengths to_vec1 failed: {}", e),
        })?[0];

    let last_token_idx = (last_idx as usize).saturating_sub(1).min(seq_len - 1);

    // Extract the last token's hidden state: [1, hidden_size]
    hidden_states
        .narrow(1, last_token_idx, 1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 last_token narrow failed: {}", e),
        })?
        .squeeze(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 last_token squeeze failed: {}", e),
        })
}

fn last_token_pool_batch(
    hidden_states: &Tensor,
    token_lengths: &[usize],
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Qwen2 true-batch last_token_pool dims failed: {}", e),
            })?;
    if batch_size != token_lengths.len() {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: ModelId::Code,
            expected: batch_size,
            actual: token_lengths.len(),
            recovery_hint: "Qwen2 true-batch token length metadata must match tensor batch rows"
                .to_string(),
        });
    }

    let mut rows = Vec::with_capacity(batch_size);
    for (batch_idx, token_len) in token_lengths.iter().copied().enumerate() {
        let last_token_idx = token_len.saturating_sub(1).min(seq_len - 1);
        let row = hidden_states
            .narrow(0, batch_idx, 1)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Qwen2 true-batch row {} narrow failed: {}", batch_idx, e),
            })?
            .narrow(1, last_token_idx, 1)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "Qwen2 true-batch row {} last-token narrow failed: {}",
                    batch_idx, e
                ),
            })?
            .reshape((1, hidden_size))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "Qwen2 true-batch row {} last-token reshape failed: {}",
                    batch_idx, e
                ),
            })?;
        rows.push(row);
    }

    let row_refs = rows.iter().collect::<Vec<_>>();
    Tensor::cat(&row_refs, 0).map_err(|e| EmbeddingError::GpuError {
        message: format!("Qwen2 true-batch pooled row concat failed: {}", e),
    })
}

/// L2 normalize and convert to Vec<f32>.
fn l2_normalize(tensor: &Tensor) -> EmbeddingResult<Vec<f32>> {
    l2_normalize_tensor(tensor)?
        .flatten_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 flatten output failed: {}", e),
        })?
        .to_vec1()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 to_vec1 failed: {}", e),
        })
}

fn l2_normalize_tensor(tensor: &Tensor) -> EmbeddingResult<Tensor> {
    // Convert to F32 for output
    let tensor = tensor
        .to_dtype(DType::F32)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 output dtype conversion failed: {}", e),
        })?;

    let norm = tensor
        .sqr()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 sqr failed: {}", e),
        })?
        .sum_keepdim(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 norm sum failed: {}", e),
        })?
        .sqrt()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 sqrt failed: {}", e),
        })?;

    let eps = Tensor::ones_like(&norm)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 create eps ones failed: {}", e),
        })?
        .affine(1e-12, 0.0)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 eps scale failed: {}", e),
        })?;

    let normalized = tensor
        .broadcast_div(
            &norm
                .broadcast_add(&eps)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Qwen2 norm eps add failed: {}", e),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 normalize div failed: {}", e),
        })?;

    Ok(normalized)
}

fn tensor_to_batch_vecs(tensor: &Tensor, batch_size: usize) -> EmbeddingResult<Vec<Vec<f32>>> {
    let (rows, hidden_size) = tensor.dims2().map_err(|e| EmbeddingError::GpuError {
        message: format!("Qwen2 true-batch output dims failed: {}", e),
    })?;
    if rows != batch_size {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: ModelId::Code,
            expected: batch_size,
            actual: rows,
            recovery_hint: "Qwen2 true-batch output tensor row count must match input batch size"
                .to_string(),
        });
    }

    let flat = tensor
        .flatten_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 true-batch flatten output failed: {}", e),
        })?
        .to_vec1()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 true-batch to_vec1 failed: {}", e),
        })?;
    let results = flat
        .chunks_exact(hidden_size)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<_>>();
    if results.len() != batch_size {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: ModelId::Code,
            expected: batch_size,
            actual: results.len(),
            recovery_hint: "Qwen2 true-batch flattened chunk count must match input batch size"
                .to_string(),
        });
    }
    Ok(results)
}
