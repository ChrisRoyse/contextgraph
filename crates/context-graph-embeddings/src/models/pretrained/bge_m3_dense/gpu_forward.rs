//! GPU-accelerated forward pass for BGE-M3 Dense.
//!
//! Pipeline:
//! 1. SentencePiece tokenisation (XLM-R vocab).
//! 2. GPU embedding lookup (word + XLM-R-offset position + token_type).
//! 3. 24 encoder layers of self-attention + FFN + post-LN.
//! 4. CLS pooling.
//! 5. L2 normalisation.

use candle_core::Tensor;
use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::BertWeights;
use crate::types::ModelId;

use super::constants::{BGE_M3_DENSE_DIMENSION, BGE_M3_DENSE_MAX_TOKENS};
use super::embeddings::compute_embeddings;
use super::encoder::encoder_layer_forward;
use super::pooling::pool_and_normalize;

/// Run the GPU-accelerated BGE-M3 Dense forward pass.
pub fn gpu_forward(
    text: &str,
    weights: &BertWeights,
    tokenizer: &Tokenizer,
) -> EmbeddingResult<Vec<f32>> {
    let device = weights.device();
    let config = &weights.config;

    // Tokenise. BGE-M3 does not use an instruction prefix for dense retrieval —
    // input text is fed to the tokenizer unmodified. `add_special_tokens=true`
    // asks the tokenizer to insert `<s>` and `</s>` around the content.
    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| EmbeddingError::TokenizationError {
            model_id: ModelId::BgeM3Dense,
            message: format!("BgeM3Dense tokenisation failed: {}", e),
        })?;

    let token_ids: Vec<u32> = encoding.get_ids().to_vec();
    let attention_mask: Vec<f32> = encoding
        .get_attention_mask()
        .iter()
        .map(|&m| m as f32)
        .collect();

    // XLM-R offsets position IDs by padding_idx+1 = 2, so the effective usable
    // sequence length is `max_position_embeddings - 2`. BGE-M3 ships
    // max_position_embeddings = 8194 to keep the 8192 usable window.
    let effective_max = config
        .max_position_embeddings
        .saturating_sub(super::constants::XLM_R_POSITION_OFFSET as usize);
    let max_len = effective_max.min(BGE_M3_DENSE_MAX_TOKENS);
    let seq_len = token_ids.len().min(max_len);

    if seq_len == 0 {
        return Err(EmbeddingError::TokenizationError {
            model_id: ModelId::BgeM3Dense,
            message: "BgeM3Dense tokeniser produced 0 tokens".to_string(),
        });
    }

    let token_ids = &token_ids[..seq_len];
    let attention_mask = &attention_mask[..seq_len];

    let input_ids = Tensor::from_slice(token_ids, (1, seq_len), device).map_err(|e| {
        EmbeddingError::GpuError {
            message: format!("BgeM3Dense input_ids tensor failed: {}", e),
        }
    })?;

    let attention_mask_tensor =
        Tensor::from_slice(attention_mask, (1, seq_len), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("BgeM3Dense attention_mask tensor failed: {}", e),
            }
        })?;

    let embeddings = compute_embeddings(&input_ids, seq_len, weights, config, device)?;

    let hidden_states = run_encoder_layers(embeddings, &attention_mask_tensor, weights, config)?;

    let out = pool_and_normalize(&hidden_states, config.hidden_size)?;

    // Sanity check: output dim should match the configured 1024-D.
    if out.len() != BGE_M3_DENSE_DIMENSION {
        return Err(EmbeddingError::InvalidDimension {
            expected: BGE_M3_DENSE_DIMENSION,
            actual: out.len(),
        });
    }

    Ok(out)
}

/// Run all encoder layers, applying the standard additive attention mask.
fn run_encoder_layers(
    embeddings: Tensor,
    attention_mask_tensor: &Tensor,
    weights: &BertWeights,
    config: &crate::gpu::BertConfig,
) -> EmbeddingResult<Tensor> {
    let mut hidden_states = embeddings;

    // Build broadcastable mask [batch, 1, 1, seq_len] and convert to additive
    // large-negative offset so softmax zeros padded positions.
    let extended_attention_mask = attention_mask_tensor
        .unsqueeze(1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense attention mask unsqueeze 1 failed: {}", e),
        })?
        .unsqueeze(2)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense attention mask unsqueeze 2 failed: {}", e),
        })?;

    let extended_attention_mask =
        ((extended_attention_mask * (-1.0)).map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense attention mask mul failed: {}", e),
        })? + 1.0)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("BgeM3Dense attention mask add failed: {}", e),
            })?
            * (-10000.0f64);

    let extended_attention_mask =
        extended_attention_mask.map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense attention mask scale failed: {}", e),
        })?;

    for (layer_idx, layer) in weights.encoder_layers.iter().enumerate() {
        hidden_states = encoder_layer_forward(
            &hidden_states,
            layer,
            &extended_attention_mask,
            config,
            layer_idx,
        )?;
    }

    Ok(hidden_states)
}
