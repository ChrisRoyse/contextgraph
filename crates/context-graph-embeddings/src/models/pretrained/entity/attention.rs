//! Self-attention and encoder layer forward pass.
//!
//! Contains the transformer encoder layer implementation including
//! multi-head self-attention. The actual projection and scoring logic
//! is split into submodules for maintainability.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::BertConfig;

use super::EntityModel;

impl EntityModel {
    /// Run single encoder layer forward pass.
    pub(crate) fn encoder_layer_forward(
        hidden_states: &Tensor,
        layer: &crate::gpu::EncoderLayerWeights,
        attention_mask: &Tensor,
        config: &BertConfig,
        layer_idx: usize,
    ) -> EmbeddingResult<Tensor> {
        // Self-attention
        let attention_output = Self::self_attention_forward(
            hidden_states,
            &layer.attention,
            attention_mask,
            config,
            layer_idx,
        )?;

        // Add & Norm (attention)
        let attention_output =
            (hidden_states + &attention_output).map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "EntityModel layer {} attention residual failed: {}",
                    layer_idx, e
                ),
            })?;

        let attention_output = Self::layer_norm(
            &attention_output,
            &layer.attention.layer_norm_weight,
            &layer.attention.layer_norm_bias,
            config.layer_norm_eps,
        )?;

        // FFN
        let ffn_output = Self::ffn_forward(&attention_output, &layer.ffn, config, layer_idx)?;

        // Add & Norm (FFN)
        let output = (&attention_output + &ffn_output).map_err(|e| EmbeddingError::GpuError {
            message: format!("EntityModel layer {} FFN residual failed: {}", layer_idx, e),
        })?;

        Self::layer_norm(
            &output,
            &layer.ffn.layer_norm_weight,
            &layer.ffn.layer_norm_bias,
            config.layer_norm_eps,
        )
    }

    /// Run self-attention forward pass.
    ///
    /// This orchestrates the full self-attention computation:
    /// 1. Compute Q, K, V projections
    /// 2. Reshape for multi-head attention
    /// 3. Compute attention scores (Q @ K^T / sqrt(d_k))
    /// 4. Apply softmax and compute context (attention @ V)
    /// 5. Apply output projection
    pub(crate) fn self_attention_forward(
        hidden_states: &Tensor,
        attention: &crate::gpu::AttentionWeights,
        attention_mask: &Tensor,
        config: &BertConfig,
        layer_idx: usize,
    ) -> EmbeddingResult<Tensor> {
        let (batch_size, seq_len, _hidden_size) =
            hidden_states
                .dims3()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("EntityModel layer {} get dims failed: {}", layer_idx, e),
                })?;

        let head_dim = config.hidden_size / config.num_attention_heads;
        let hidden_size = config.hidden_size;
        let num_heads = config.num_attention_heads;

        // Flatten to [batch*seq, hidden] for matmul
        let hidden_flat = hidden_states
            .reshape((batch_size * seq_len, hidden_size))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!(
                    "EntityModel layer {} hidden flatten failed: {}",
                    layer_idx, e
                ),
            })?;

        // Compute Q, K, V projections
        let query = Self::compute_query_projection(
            &hidden_flat,
            attention,
            batch_size,
            seq_len,
            hidden_size,
            layer_idx,
        )?;
        let key = Self::compute_key_projection(
            &hidden_flat,
            attention,
            batch_size,
            seq_len,
            hidden_size,
            layer_idx,
        )?;
        let value = Self::compute_value_projection(
            &hidden_flat,
            attention,
            batch_size,
            seq_len,
            hidden_size,
            layer_idx,
        )?;

        // Reshape for multi-head attention: [batch, heads, seq, head_dim]
        let query = Self::reshape_for_attention(
            query, batch_size, seq_len, num_heads, head_dim, layer_idx, "Q",
        )?;
        let key = Self::reshape_for_attention(
            key, batch_size, seq_len, num_heads, head_dim, layer_idx, "K",
        )?;
        let value = Self::reshape_for_attention(
            value, batch_size, seq_len, num_heads, head_dim, layer_idx, "V",
        )?;

        // Compute attention scores with mask
        let scores =
            Self::compute_attention_scores(&query, &key, attention_mask, head_dim, layer_idx)?;

        // Apply softmax
        let attention_probs = Self::apply_attention_softmax(&scores, layer_idx)?;

        // Compute context
        let context = Self::compute_attention_context(
            &attention_probs,
            &value,
            batch_size,
            seq_len,
            hidden_size,
            layer_idx,
        )?;

        // Apply output projection
        Self::apply_output_projection(
            &context,
            attention,
            batch_size,
            seq_len,
            hidden_size,
            layer_idx,
        )
    }
}
