//! GPU forward pass implementation for Qwen2 (Qodo-Embed).
//!
//! Contains the main Qwen2 decoder forward pass with:
//! - Token embedding lookup
//! - RoPE position encoding
//! - Grouped-Query Attention (GQA) layers
//! - SwiGLU FFN
//! - Last-token pooling for embedding output

use candle_core::{DType, Tensor};
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
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
    let hidden_states = embed_tokens(&input_ids, weights, seq_len)?;

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
        )?;
    }

    // === FINAL LAYER NORM ===
    hidden_states = rms_norm(&hidden_states, &weights.norm_weight, config.rms_norm_eps)?;

    // === LAST-TOKEN POOLING & NORMALIZE ===
    // Qodo-Embed uses last token pooling (pooling_mode_lasttoken: true)
    let pooled = last_token_pool(&hidden_states, &attention_mask_tensor)?;
    l2_normalize(&pooled)
}

/// Embed token IDs using token embeddings.
fn embed_tokens(
    input_ids: &Tensor,
    weights: &QwenWeights,
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
        .reshape((1, seq_len, weights.config.hidden_size))
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

/// L2 normalize and convert to Vec<f32>.
fn l2_normalize(tensor: &Tensor) -> EmbeddingResult<Vec<f32>> {
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

    normalized
        .flatten_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 flatten output failed: {}", e),
        })?
        .to_vec1()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Qwen2 to_vec1 failed: {}", e),
        })
}
