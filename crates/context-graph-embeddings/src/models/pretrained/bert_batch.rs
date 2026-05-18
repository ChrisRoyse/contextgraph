//! Shared true-batch CUDA forward path for BERT-family dense embedders.
//!
//! This module is intentionally separate from the single-input model paths so
//! callers can distinguish real tensor batching from queue batching or loops.

use std::time::Instant;

use candle_core::Tensor;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{normalize_gpu, AttentionWeights, BertConfig, BertWeights, EncoderLayerWeights};
use crate::types::ModelId;

#[derive(Clone, Copy, Debug)]
pub(crate) enum BertBatchPooling {
    Mean,
    Cls,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BertBatchSpec {
    pub(crate) model_id: ModelId,
    pub(crate) model_label: &'static str,
    pub(crate) max_tokens: usize,
    pub(crate) position_offset: u32,
    pub(crate) position_padding_id: u32,
    pub(crate) pooling: BertBatchPooling,
}

#[derive(Clone, Copy)]
struct ProjectionShape {
    batch_size: usize,
    seq_len: usize,
    hidden_size: usize,
}

pub(crate) fn gpu_forward_text_batch(
    texts: &[String],
    weights: &BertWeights,
    tokenizer: &Tokenizer,
    spec: BertBatchSpec,
) -> EmbeddingResult<Vec<Vec<f32>>> {
    let started_at = Instant::now();
    if texts.is_empty() {
        return Err(EmbeddingError::TrueBatchEmpty {
            model_id: spec.model_id,
            recovery_hint: "submit at least one input; empty true batches are caller bugs"
                .to_string(),
        });
    }

    let device = weights.device();
    let config = &weights.config;
    let batch_size = texts.len();
    let usable_max_len = usable_max_len(config, spec)?;
    let pad_id = pad_token_id(tokenizer, config);

    let encodings = texts
        .iter()
        .map(|text| {
            tokenizer
                .encode(text.as_str(), true)
                .map_err(|e| EmbeddingError::TokenizationError {
                    model_id: spec.model_id,
                    message: format!("{} true-batch tokenization failed: {}", spec.model_label, e),
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
        .find(|(_, token_len)| *token_len > usable_max_len)
    {
        return Err(EmbeddingError::InputTooLong {
            actual,
            max: usable_max_len,
        });
    }

    if let Some((idx, _)) = token_lengths
        .iter()
        .enumerate()
        .find(|(_, token_len)| **token_len == 0)
    {
        return Err(EmbeddingError::TokenizationError {
            model_id: spec.model_id,
            message: format!(
                "{} true-batch tokenization produced zero real tokens at batch_index={}",
                spec.model_label, idx
            ),
        });
    }

    let actual_max_len = token_lengths.iter().copied().max().unwrap_or(0);
    if actual_max_len == 0 {
        return Err(EmbeddingError::TrueBatchEmpty {
            model_id: spec.model_id,
            recovery_hint: "tokenization produced no usable tokens for the batch".to_string(),
        });
    }

    let mut all_token_ids = vec![pad_id; batch_size * actual_max_len];
    let mut all_attention_mask = vec![0.0f32; batch_size * actual_max_len];
    let mut all_position_ids = vec![spec.position_padding_id; batch_size * actual_max_len];
    let all_token_type_ids = vec![0u32; batch_size * actual_max_len];

    for (batch_idx, encoding) in encodings.iter().enumerate() {
        let seq_len = token_lengths[batch_idx];
        let offset = batch_idx * actual_max_len;
        for (token_idx, &token_id) in encoding.get_ids()[..seq_len].iter().enumerate() {
            all_token_ids[offset + token_idx] = token_id;
            all_attention_mask[offset + token_idx] = 1.0;
            all_position_ids[offset + token_idx] = token_idx as u32 + spec.position_offset;
        }
    }

    let input_ids = Tensor::from_slice(&all_token_ids, (batch_size, actual_max_len), device)
        .map_err(|e| gpu_error(spec, format!("input_ids tensor creation failed: {}", e)))?;
    let attention_mask =
        Tensor::from_slice(&all_attention_mask, (batch_size, actual_max_len), device).map_err(
            |e| {
                gpu_error(
                    spec,
                    format!("attention_mask tensor creation failed: {}", e),
                )
            },
        )?;
    let position_ids = Tensor::from_slice(&all_position_ids, (batch_size, actual_max_len), device)
        .map_err(|e| gpu_error(spec, format!("position_ids tensor creation failed: {}", e)))?;
    let token_type_ids =
        Tensor::from_slice(&all_token_type_ids, (batch_size, actual_max_len), device).map_err(
            |e| {
                gpu_error(
                    spec,
                    format!("token_type_ids tensor creation failed: {}", e),
                )
            },
        )?;

    let mut hidden_states = compute_embeddings_batch(
        &input_ids,
        &position_ids,
        &token_type_ids,
        weights,
        batch_size,
        actual_max_len,
        spec,
    )?;
    hidden_states = layer_norm(
        &hidden_states,
        &weights.embeddings.layer_norm_weight,
        &weights.embeddings.layer_norm_bias,
        config.layer_norm_eps,
        spec,
    )?;

    let extended_attention_mask = create_extended_attention_mask(&attention_mask, spec)?;
    for (layer_idx, layer) in weights.encoder_layers.iter().enumerate() {
        hidden_states = encoder_layer_forward(
            &hidden_states,
            layer,
            &extended_attention_mask,
            config,
            layer_idx,
            spec,
        )?;
    }

    let pooled = match spec.pooling {
        BertBatchPooling::Mean => mean_pool_batch(
            &hidden_states,
            &attention_mask,
            config,
            batch_size,
            actual_max_len,
            spec,
        )?,
        BertBatchPooling::Cls => {
            cls_pool_batch(&hidden_states, batch_size, config.hidden_size, spec)?
        }
    };
    let normalized = normalize_gpu(&pooled)
        .map_err(|e| gpu_error(spec, format!("batch L2 normalization failed: {}", e)))?;
    let results = tensor_to_batch_vecs(&normalized, batch_size, spec)?;

    let output_dim = results.first().map_or(0, Vec::len);
    let padding_tokens = token_lengths
        .iter()
        .map(|token_len| actual_max_len - token_len)
        .sum::<usize>();
    let batch_tensor_bytes = all_token_ids.len() * std::mem::size_of::<u32>()
        + all_position_ids.len() * std::mem::size_of::<u32>()
        + all_token_type_ids.len() * std::mem::size_of::<u32>()
        + all_attention_mask.len() * std::mem::size_of::<f32>();

    tracing::info!(
        target: "context_graph_embeddings::true_batch",
        model_id = ?spec.model_id,
        model = spec.model_label,
        batch_size,
        max_seq_len = actual_max_len,
        token_lengths = ?token_lengths,
        padding_tokens,
        output_count = results.len(),
        output_dim,
        model_vram_bytes = weights.vram_bytes(),
        batch_tensor_bytes,
        latency_us = started_at.elapsed().as_micros() as u64,
        "BERT-family true-batch forward completed"
    );

    if results.len() != batch_size {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: spec.model_id,
            expected: batch_size,
            actual: results.len(),
            recovery_hint:
                "inspect true-batch tensor flattening/chunking; partial batch outputs are invalid"
                    .to_string(),
        });
    }

    Ok(results)
}

fn usable_max_len(config: &BertConfig, spec: BertBatchSpec) -> EmbeddingResult<usize> {
    let offset = spec.position_offset as usize;
    if config.max_position_embeddings <= offset {
        return Err(EmbeddingError::ConfigError {
            message: format!(
                "{} true-batch invalid position capacity: max_position_embeddings={} <= position_offset={}",
                spec.model_label, config.max_position_embeddings, offset
            ),
        });
    }

    let usable = (config.max_position_embeddings - offset).min(spec.max_tokens);
    if usable == 0 {
        return Err(EmbeddingError::ConfigError {
            message: format!(
                "{} true-batch usable max length resolved to zero",
                spec.model_label
            ),
        });
    }
    Ok(usable)
}

fn pad_token_id(tokenizer: &Tokenizer, config: &BertConfig) -> u32 {
    tokenizer
        .get_padding()
        .map(|padding| padding.pad_id)
        .or_else(|| tokenizer.token_to_id("[PAD]"))
        .or_else(|| tokenizer.token_to_id("<pad>"))
        .unwrap_or(config.pad_token_id as u32)
}

fn compute_embeddings_batch(
    input_ids: &Tensor,
    position_ids: &Tensor,
    token_type_ids: &Tensor,
    weights: &BertWeights,
    batch_size: usize,
    seq_len: usize,
    spec: BertBatchSpec,
) -> EmbeddingResult<Tensor> {
    let config = &weights.config;
    let input_flat = input_ids
        .flatten_all()
        .map_err(|e| gpu_error(spec, format!("flatten input_ids failed: {}", e)))?;
    let word_embeds = weights
        .embeddings
        .word_embeddings
        .index_select(&input_flat, 0)
        .map_err(|e| gpu_error(spec, format!("word embedding lookup failed: {}", e)))?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| gpu_error(spec, format!("word embedding reshape failed: {}", e)))?;

    let position_flat = position_ids
        .flatten_all()
        .map_err(|e| gpu_error(spec, format!("flatten position_ids failed: {}", e)))?;
    let position_embeds = weights
        .embeddings
        .position_embeddings
        .index_select(&position_flat, 0)
        .map_err(|e| {
            gpu_error(
                spec,
                format!(
                    "position embedding lookup failed: max_position_embeddings={}, max_position_id={}, source_error={}",
                    config.max_position_embeddings,
                    seq_len as u32 + spec.position_offset - 1,
                    e
                ),
            )
        })?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| gpu_error(spec, format!("position embedding reshape failed: {}", e)))?;

    let token_type_flat = token_type_ids
        .flatten_all()
        .map_err(|e| gpu_error(spec, format!("flatten token_type_ids failed: {}", e)))?;
    let token_type_embeds = weights
        .embeddings
        .token_type_embeddings
        .index_select(&token_type_flat, 0)
        .map_err(|e| gpu_error(spec, format!("token type embedding lookup failed: {}", e)))?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| gpu_error(spec, format!("token type embedding reshape failed: {}", e)))?;

    ((word_embeds + position_embeds)
        .map_err(|e| gpu_error(spec, format!("embedding add 1 failed: {}", e)))?
        + token_type_embeds)
        .map_err(|e| gpu_error(spec, format!("embedding add 2 failed: {}", e)))
}

fn create_extended_attention_mask(
    attention_mask: &Tensor,
    spec: BertBatchSpec,
) -> EmbeddingResult<Tensor> {
    let extended = attention_mask
        .unsqueeze(1)
        .map_err(|e| gpu_error(spec, format!("attention mask unsqueeze 1 failed: {}", e)))?
        .unsqueeze(2)
        .map_err(|e| gpu_error(spec, format!("attention mask unsqueeze 2 failed: {}", e)))?;
    let inverted = (extended * (-1.0))
        .map_err(|e| gpu_error(spec, format!("attention mask invert failed: {}", e)))?;
    let shifted = (inverted + 1.0)
        .map_err(|e| gpu_error(spec, format!("attention mask shift failed: {}", e)))?;
    (shifted * (-10000.0f64))
        .map_err(|e| gpu_error(spec, format!("attention mask scale failed: {}", e)))
}

fn encoder_layer_forward(
    hidden_states: &Tensor,
    layer: &EncoderLayerWeights,
    attention_mask: &Tensor,
    config: &BertConfig,
    layer_idx: usize,
    spec: BertBatchSpec,
) -> EmbeddingResult<Tensor> {
    let attention_output = self_attention_forward(
        hidden_states,
        &layer.attention,
        attention_mask,
        config,
        layer_idx,
        spec,
    )?;
    let attention_output = (hidden_states + &attention_output).map_err(|e| {
        gpu_error(
            spec,
            format!("layer {} attention residual failed: {}", layer_idx, e),
        )
    })?;
    let attention_output = layer_norm(
        &attention_output,
        &layer.attention.layer_norm_weight,
        &layer.attention.layer_norm_bias,
        config.layer_norm_eps,
        spec,
    )?;

    let ffn_output = ffn_forward(&attention_output, layer, config, layer_idx, spec)?;
    let output = (&attention_output + &ffn_output).map_err(|e| {
        gpu_error(
            spec,
            format!("layer {} FFN residual failed: {}", layer_idx, e),
        )
    })?;
    layer_norm(
        &output,
        &layer.ffn.layer_norm_weight,
        &layer.ffn.layer_norm_bias,
        config.layer_norm_eps,
        spec,
    )
}

fn self_attention_forward(
    hidden_states: &Tensor,
    attention: &AttentionWeights,
    attention_mask: &Tensor,
    config: &BertConfig,
    layer_idx: usize,
    spec: BertBatchSpec,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, _) = hidden_states.dims3().map_err(|e| {
        gpu_error(
            spec,
            format!("layer {} hidden dims failed: {}", layer_idx, e),
        )
    })?;
    let head_dim = config.hidden_size / config.num_attention_heads;
    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len, config.hidden_size))
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} hidden flatten failed: {}", layer_idx, e),
            )
        })?;

    let projection_shape = ProjectionShape {
        batch_size,
        seq_len,
        hidden_size: config.hidden_size,
    };

    let query = qkv_projection(
        &hidden_flat,
        &attention.query_weight,
        &attention.query_bias,
        projection_shape,
        layer_idx,
        "query",
        spec,
    )?;
    let key = qkv_projection(
        &hidden_flat,
        &attention.key_weight,
        &attention.key_bias,
        projection_shape,
        layer_idx,
        "key",
        spec,
    )?;
    let value = qkv_projection(
        &hidden_flat,
        &attention.value_weight,
        &attention.value_bias,
        projection_shape,
        layer_idx,
        "value",
        spec,
    )?;

    let query = reshape_for_attention(
        &query,
        batch_size,
        seq_len,
        config.num_attention_heads,
        head_dim,
        layer_idx,
        "query",
        spec,
    )?;
    let key = reshape_for_attention(
        &key,
        batch_size,
        seq_len,
        config.num_attention_heads,
        head_dim,
        layer_idx,
        "key",
        spec,
    )?;
    let value = reshape_for_attention(
        &value,
        batch_size,
        seq_len,
        config.num_attention_heads,
        head_dim,
        layer_idx,
        "value",
        spec,
    )?;

    let key_t = key
        .transpose(2, 3)
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} key transpose failed: {}", layer_idx, e),
            )
        })?
        .contiguous()
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} key contiguous failed: {}", layer_idx, e),
            )
        })?;
    let scores = query
        .matmul(&key_t)
        .map_err(|e| gpu_error(spec, format!("layer {} QK matmul failed: {}", layer_idx, e)))?;
    let scores = (scores / (head_dim as f64).sqrt()).map_err(|e| {
        gpu_error(
            spec,
            format!("layer {} attention scale failed: {}", layer_idx, e),
        )
    })?;
    let scores = scores.broadcast_add(attention_mask).map_err(|e| {
        gpu_error(
            spec,
            format!("layer {} attention mask add failed: {}", layer_idx, e),
        )
    })?;
    let attention_probs = candle_nn::ops::softmax(&scores, candle_core::D::Minus1)
        .map_err(|e| gpu_error(spec, format!("layer {} softmax failed: {}", layer_idx, e)))?;
    let context = attention_probs.matmul(&value).map_err(|e| {
        gpu_error(
            spec,
            format!("layer {} context matmul failed: {}", layer_idx, e),
        )
    })?;
    let context = context
        .transpose(1, 2)
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} context transpose failed: {}", layer_idx, e),
            )
        })?
        .contiguous()
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} context contiguous failed: {}", layer_idx, e),
            )
        })?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} context reshape failed: {}", layer_idx, e),
            )
        })?;

    let context_flat = context
        .reshape((batch_size * seq_len, config.hidden_size))
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} context flatten failed: {}", layer_idx, e),
            )
        })?;
    let output = context_flat
        .matmul(&attention.output_weight.t().map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} output weight transpose failed: {}", layer_idx, e),
            )
        })?)
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} output matmul failed: {}", layer_idx, e),
            )
        })?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} output reshape failed: {}", layer_idx, e),
            )
        })?;
    output.broadcast_add(&attention.output_bias).map_err(|e| {
        gpu_error(
            spec,
            format!("layer {} output bias failed: {}", layer_idx, e),
        )
    })
}

fn qkv_projection(
    hidden_flat: &Tensor,
    weight: &Tensor,
    bias: &Tensor,
    shape: ProjectionShape,
    layer_idx: usize,
    name: &str,
    spec: BertBatchSpec,
) -> EmbeddingResult<Tensor> {
    hidden_flat
        .matmul(&weight.t().map_err(|e| {
            gpu_error(
                spec,
                format!(
                    "layer {} {} weight transpose failed: {}",
                    layer_idx, name, e
                ),
            )
        })?)
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} {} matmul failed: {}", layer_idx, name, e),
            )
        })?
        .reshape((shape.batch_size, shape.seq_len, shape.hidden_size))
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} {} reshape failed: {}", layer_idx, name, e),
            )
        })?
        .broadcast_add(bias)
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} {} bias failed: {}", layer_idx, name, e),
            )
        })
}

fn reshape_for_attention(
    tensor: &Tensor,
    batch_size: usize,
    seq_len: usize,
    num_heads: usize,
    head_dim: usize,
    layer_idx: usize,
    name: &str,
    spec: BertBatchSpec,
) -> EmbeddingResult<Tensor> {
    tensor
        .reshape((batch_size, seq_len, num_heads, head_dim))
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} {} head reshape failed: {}", layer_idx, name, e),
            )
        })?
        .transpose(1, 2)
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} {} head transpose failed: {}", layer_idx, name, e),
            )
        })?
        .contiguous()
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} {} contiguous failed: {}", layer_idx, name, e),
            )
        })
}

fn ffn_forward(
    hidden_states: &Tensor,
    layer: &EncoderLayerWeights,
    config: &BertConfig,
    layer_idx: usize,
    spec: BertBatchSpec,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, hidden_size) = hidden_states
        .dims3()
        .map_err(|e| gpu_error(spec, format!("layer {} FFN dims failed: {}", layer_idx, e)))?;
    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} FFN flatten failed: {}", layer_idx, e),
            )
        })?;
    let intermediate = hidden_flat
        .matmul(&layer.ffn.intermediate_weight.t().map_err(|e| {
            gpu_error(
                spec,
                format!(
                    "layer {} FFN intermediate transpose failed: {}",
                    layer_idx, e
                ),
            )
        })?)
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} FFN intermediate matmul failed: {}", layer_idx, e),
            )
        })?
        .broadcast_add(&layer.ffn.intermediate_bias)
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} FFN intermediate bias failed: {}", layer_idx, e),
            )
        })?
        .gelu()
        .map_err(|e| gpu_error(spec, format!("layer {} FFN GELU failed: {}", layer_idx, e)))?;
    intermediate
        .matmul(&layer.ffn.output_weight.t().map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} FFN output transpose failed: {}", layer_idx, e),
            )
        })?)
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} FFN output matmul failed: {}", layer_idx, e),
            )
        })?
        .broadcast_add(&layer.ffn.output_bias)
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} FFN output bias failed: {}", layer_idx, e),
            )
        })?
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| {
            gpu_error(
                spec,
                format!("layer {} FFN output reshape failed: {}", layer_idx, e),
            )
        })
}

fn layer_norm(
    x: &Tensor,
    weight: &Tensor,
    bias: &Tensor,
    eps: f64,
    spec: BertBatchSpec,
) -> EmbeddingResult<Tensor> {
    let mean = x
        .mean_keepdim(candle_core::D::Minus1)
        .map_err(|e| gpu_error(spec, format!("LayerNorm mean failed: {}", e)))?;
    let centered = x
        .broadcast_sub(&mean)
        .map_err(|e| gpu_error(spec, format!("LayerNorm center failed: {}", e)))?;
    let variance = centered
        .sqr()
        .map_err(|e| gpu_error(spec, format!("LayerNorm sqr failed: {}", e)))?
        .mean_keepdim(candle_core::D::Minus1)
        .map_err(|e| gpu_error(spec, format!("LayerNorm variance failed: {}", e)))?;
    let std = (variance + eps)
        .map_err(|e| gpu_error(spec, format!("LayerNorm add eps failed: {}", e)))?
        .sqrt()
        .map_err(|e| gpu_error(spec, format!("LayerNorm sqrt failed: {}", e)))?;
    let normalized = centered
        .broadcast_div(&std)
        .map_err(|e| gpu_error(spec, format!("LayerNorm div failed: {}", e)))?;
    normalized
        .broadcast_mul(weight)
        .map_err(|e| gpu_error(spec, format!("LayerNorm scale failed: {}", e)))?
        .broadcast_add(bias)
        .map_err(|e| gpu_error(spec, format!("LayerNorm bias failed: {}", e)))
}

fn mean_pool_batch(
    hidden_states: &Tensor,
    attention_mask: &Tensor,
    config: &BertConfig,
    batch_size: usize,
    seq_len: usize,
    spec: BertBatchSpec,
) -> EmbeddingResult<Tensor> {
    let mask_expanded = attention_mask
        .unsqueeze(2)
        .map_err(|e| gpu_error(spec, format!("mean pool mask unsqueeze failed: {}", e)))?
        .broadcast_as((batch_size, seq_len, config.hidden_size))
        .map_err(|e| gpu_error(spec, format!("mean pool mask broadcast failed: {}", e)))?;
    let masked = (hidden_states * &mask_expanded)
        .map_err(|e| gpu_error(spec, format!("mean pool mask multiply failed: {}", e)))?;
    let summed = masked
        .sum(1)
        .map_err(|e| gpu_error(spec, format!("mean pool hidden sum failed: {}", e)))?;
    let mask_sum = mask_expanded
        .sum(1)
        .map_err(|e| gpu_error(spec, format!("mean pool mask sum failed: {}", e)))?
        .clamp(1e-9, f64::INFINITY)
        .map_err(|e| gpu_error(spec, format!("mean pool mask clamp failed: {}", e)))?;
    (summed / mask_sum).map_err(|e| gpu_error(spec, format!("mean pool divide failed: {}", e)))
}

fn cls_pool_batch(
    hidden_states: &Tensor,
    batch_size: usize,
    hidden_size: usize,
    spec: BertBatchSpec,
) -> EmbeddingResult<Tensor> {
    hidden_states
        .narrow(1, 0, 1)
        .map_err(|e| gpu_error(spec, format!("CLS pool narrow failed: {}", e)))?
        .reshape((batch_size, hidden_size))
        .map_err(|e| gpu_error(spec, format!("CLS pool reshape failed: {}", e)))
}

fn tensor_to_batch_vecs(
    tensor: &Tensor,
    batch_size: usize,
    spec: BertBatchSpec,
) -> EmbeddingResult<Vec<Vec<f32>>> {
    let (rows, hidden_size) = tensor
        .dims2()
        .map_err(|e| gpu_error(spec, format!("output tensor dims failed: {}", e)))?;
    if rows != batch_size {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: spec.model_id,
            expected: batch_size,
            actual: rows,
            recovery_hint: "true-batch output tensor row count must match input batch size"
                .to_string(),
        });
    }

    let flat = tensor
        .flatten_all()
        .map_err(|e| gpu_error(spec, format!("output tensor flatten failed: {}", e)))?
        .to_vec1::<f32>()
        .map_err(|e| gpu_error(spec, format!("output tensor to_vec failed: {}", e)))?;
    let results = flat
        .chunks_exact(hidden_size)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<_>>();
    if results.len() != batch_size {
        return Err(EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: spec.model_id,
            expected: batch_size,
            actual: results.len(),
            recovery_hint: "true-batch flattened output chunk count must match input batch size"
                .to_string(),
        });
    }
    Ok(results)
}

fn gpu_error(spec: BertBatchSpec, message: String) -> EmbeddingError {
    EmbeddingError::GpuError {
        message: format!("{} true-batch {}", spec.model_label, message),
    }
}
