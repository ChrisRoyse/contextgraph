//! Single encoder-layer orchestration for the BGE-M3 Dense model.
//!
//! Combines self-attention and FFN with residual + post-LayerNorm in the
//! order used by both BERT and XLM-RoBERTa:
//!
//! ```text
//!   x1 = LN_att(x + self_attention(x))
//!   x2 = LN_ffn(x1 + ffn(x1))
//! ```

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{BertConfig, EncoderLayerWeights};

use super::attention::self_attention_forward;
use super::ffn::ffn_forward;
use super::layer_norm::layer_norm;

/// Run a single encoder layer forward pass.
pub fn encoder_layer_forward(
    hidden_states: &Tensor,
    layer: &EncoderLayerWeights,
    attention_mask: &Tensor,
    config: &BertConfig,
    layer_idx: usize,
) -> EmbeddingResult<Tensor> {
    let attention_output = self_attention_forward(
        hidden_states,
        &layer.attention,
        attention_mask,
        config,
        layer_idx,
    )?;

    let attention_output =
        (hidden_states + &attention_output).map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} attention residual failed: {}",
                layer_idx, e
            ),
        })?;

    let attention_output = layer_norm(
        &attention_output,
        &layer.attention.layer_norm_weight,
        &layer.attention.layer_norm_bias,
        config.layer_norm_eps,
    )?;

    let ffn_output = ffn_forward(&attention_output, &layer.ffn, config, layer_idx)?;

    let output = (&attention_output + &ffn_output).map_err(|e| EmbeddingError::GpuError {
        message: format!("BgeM3Dense layer {} FFN residual failed: {}", layer_idx, e),
    })?;

    layer_norm(
        &output,
        &layer.ffn.layer_norm_weight,
        &layer.ffn.layer_norm_bias,
        config.layer_norm_eps,
    )
}
