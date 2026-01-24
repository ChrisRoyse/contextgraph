//! Pooling operations for KEPLER embeddings.
//!
//! Provides mean pooling over the sequence dimension to produce
//! fixed-size sentence/entity embeddings.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::BertConfig;

use super::KeplerModel;

impl KeplerModel {
    /// Mean pooling over sequence dimension with attention mask.
    ///
    /// Computes the mean of hidden states, weighted by the attention mask.
    /// This produces a single 768D vector from the sequence of token embeddings.
    ///
    /// # Formula
    /// ```text
    /// pooled[i] = sum(hidden_states[t,i] * mask[t]) / sum(mask[t])
    /// ```
    ///
    /// # Arguments
    /// * `hidden_states` - [1, seq_len, hidden_size] tensor
    /// * `attention_mask` - [1, seq_len] tensor (1.0 for real tokens, 0.0 for padding)
    /// * `config` - Model configuration
    /// * `seq_len` - Sequence length
    ///
    /// # Returns
    /// Pooled embedding of shape [hidden_size].
    pub(crate) fn mean_pool(
        hidden_states: &Tensor,
        attention_mask: &Tensor,
        config: &BertConfig,
        seq_len: usize,
    ) -> EmbeddingResult<Tensor> {
        // Expand mask for broadcasting: [1, seq_len] -> [1, seq_len, hidden_size]
        let mask_expanded = attention_mask
            .unsqueeze(2)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel mean_pool unsqueeze failed: {}", e),
            })?
            .expand((1, seq_len, config.hidden_size))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel mean_pool expand failed: {}", e),
            })?;

        // Multiply hidden states by mask
        let masked = (hidden_states * &mask_expanded).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel mean_pool mul failed: {}", e),
        })?;

        // Sum over sequence dimension
        let summed = masked.sum(1).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel mean_pool sum failed: {}", e),
        })?;

        // Sum mask for denominator
        let mask_sum = mask_expanded
            .sum(1)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel mean_pool mask_sum failed: {}", e),
            })?
            .clamp(1e-9, f64::INFINITY)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel mean_pool clamp failed: {}", e),
            })?;

        // Divide to get mean
        let pooled = (summed / mask_sum).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel mean_pool div failed: {}", e),
        })?;

        // Squeeze batch dimension
        pooled.squeeze(0).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel mean_pool squeeze failed: {}", e),
        })
    }

    /// L2 normalize the embedding vector.
    ///
    /// Normalizes the vector to unit length: v / ||v||_2
    ///
    /// # Arguments
    /// * `embedding` - [hidden_size] tensor
    ///
    /// # Returns
    /// L2-normalized embedding of shape [hidden_size].
    pub(crate) fn l2_normalize(embedding: &Tensor) -> EmbeddingResult<Tensor> {
        let norm = embedding
            .sqr()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel l2_normalize sqr failed: {}", e),
            })?
            .sum_all()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel l2_normalize sum failed: {}", e),
            })?
            .sqrt()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel l2_normalize sqrt failed: {}", e),
            })?
            .clamp(1e-12, f64::INFINITY)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel l2_normalize clamp failed: {}", e),
            })?;

        embedding
            .broadcast_div(&norm)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel l2_normalize div failed: {}", e),
            })
    }

    /// Convert tensor to Vec<f32>.
    pub(crate) fn tensor_to_vec(tensor: &Tensor) -> EmbeddingResult<Vec<f32>> {
        tensor
            .to_vec1::<f32>()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel tensor_to_vec failed: {}", e),
            })
    }

    /// Layer normalization.
    pub(crate) fn layer_norm(
        input: &Tensor,
        weight: &Tensor,
        bias: &Tensor,
        eps: f64,
    ) -> EmbeddingResult<Tensor> {
        // Compute mean over last dimension
        let mean = input
            .mean_keepdim(candle_core::D::Minus1)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel layer_norm mean failed: {}", e),
            })?;

        // Compute variance
        let diff = input.broadcast_sub(&mean).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel layer_norm sub failed: {}", e),
        })?;

        let variance = diff
            .sqr()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel layer_norm sqr failed: {}", e),
            })?
            .mean_keepdim(candle_core::D::Minus1)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel layer_norm var_mean failed: {}", e),
            })?;

        // Normalize: (x - mean) / sqrt(var + eps)
        let std = (variance + eps)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel layer_norm add_eps failed: {}", e),
            })?
            .sqrt()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel layer_norm sqrt failed: {}", e),
            })?;

        let normalized = diff.broadcast_div(&std).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel layer_norm div failed: {}", e),
        })?;

        // Apply affine transformation: weight * normalized + bias
        let scaled = normalized
            .broadcast_mul(weight)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel layer_norm scale failed: {}", e),
            })?;

        scaled
            .broadcast_add(bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel layer_norm bias failed: {}", e),
            })
    }

    /// Run a single encoder layer forward pass.
    pub(crate) fn encoder_layer_forward(
        hidden_states: &Tensor,
        layer: &crate::gpu::EncoderLayerWeights,
        attention_mask: &Tensor,
        config: &BertConfig,
        _layer_idx: usize,
    ) -> EmbeddingResult<Tensor> {
        // Self-attention
        let attention_output =
            Self::self_attention_forward(hidden_states, &layer.attention, attention_mask, config)?;

        // Add & norm (attention)
        let attention_output = (hidden_states + &attention_output).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("KeplerModel encoder attention residual failed: {}", e),
            }
        })?;

        let attention_output = Self::layer_norm(
            &attention_output,
            &layer.attention.layer_norm_weight,
            &layer.attention.layer_norm_bias,
            config.layer_norm_eps,
        )?;

        // FFN
        let ffn_output = Self::ffn_forward(&attention_output, &layer.ffn, config)?;

        // Add & norm (FFN)
        let output =
            (attention_output + &ffn_output).map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel encoder ffn residual failed: {}", e),
            })?;

        Self::layer_norm(
            &output,
            &layer.ffn.layer_norm_weight,
            &layer.ffn.layer_norm_bias,
            config.layer_norm_eps,
        )
    }

    /// Self-attention forward pass.
    fn self_attention_forward(
        hidden_states: &Tensor,
        attention: &crate::gpu::AttentionWeights,
        attention_mask: &Tensor,
        config: &BertConfig,
    ) -> EmbeddingResult<Tensor> {
        let (batch, seq_len, _hidden) = hidden_states.dims3().map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("KeplerModel attention dims failed: {}", e),
            }
        })?;

        let head_dim = config.hidden_size / config.num_attention_heads;

        // Project Q, K, V
        let query = hidden_states
            .matmul(&attention.query_weight.t().map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel query transpose failed: {}", e),
            })?)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel query matmul failed: {}", e),
            })?
            .broadcast_add(&attention.query_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel query bias failed: {}", e),
            })?;

        let key = hidden_states
            .matmul(&attention.key_weight.t().map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel key transpose failed: {}", e),
            })?)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel key matmul failed: {}", e),
            })?
            .broadcast_add(&attention.key_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel key bias failed: {}", e),
            })?;

        let value = hidden_states
            .matmul(&attention.value_weight.t().map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel value transpose failed: {}", e),
            })?)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel value matmul failed: {}", e),
            })?
            .broadcast_add(&attention.value_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel value bias failed: {}", e),
            })?;

        // Reshape to [batch, heads, seq, head_dim]
        let query = query
            .reshape((batch, seq_len, config.num_attention_heads, head_dim))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel query reshape failed: {}", e),
            })?
            .transpose(1, 2)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel query transpose failed: {}", e),
            })?;

        let key = key
            .reshape((batch, seq_len, config.num_attention_heads, head_dim))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel key reshape failed: {}", e),
            })?
            .transpose(1, 2)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel key transpose failed: {}", e),
            })?;

        let value = value
            .reshape((batch, seq_len, config.num_attention_heads, head_dim))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel value reshape failed: {}", e),
            })?
            .transpose(1, 2)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel value transpose failed: {}", e),
            })?;

        // Compute attention scores
        let scale = 1.0 / (head_dim as f64).sqrt();
        let scores = query
            .matmul(
                &key.transpose(2, 3)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("KeplerModel key transpose for scores failed: {}", e),
                    })?,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel attention scores matmul failed: {}", e),
            })?
            * scale;

        let scores = scores.map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel attention scale failed: {}", e),
        })?;

        // Apply attention mask
        let scores = (scores + attention_mask).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel attention mask add failed: {}", e),
        })?;

        // Softmax
        let probs = candle_nn::ops::softmax(&scores, candle_core::D::Minus1).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("KeplerModel attention softmax failed: {}", e),
            }
        })?;

        // Apply to values
        let context = probs.matmul(&value).map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel attention context matmul failed: {}", e),
        })?;

        // Reshape back to [batch, seq, hidden]
        let context = context
            .transpose(1, 2)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel context transpose failed: {}", e),
            })?
            .reshape((batch, seq_len, config.hidden_size))
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel context reshape failed: {}", e),
            })?;

        // Output projection
        context
            .matmul(
                &attention
                    .output_weight
                    .t()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("KeplerModel output transpose failed: {}", e),
                    })?,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel output matmul failed: {}", e),
            })?
            .broadcast_add(&attention.output_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel output bias failed: {}", e),
            })
    }

    /// Feed-forward network forward pass.
    fn ffn_forward(
        hidden_states: &Tensor,
        ffn: &crate::gpu::FfnWeights,
        _config: &BertConfig,
    ) -> EmbeddingResult<Tensor> {
        // Intermediate projection
        let intermediate = hidden_states
            .matmul(&ffn.intermediate_weight.t().map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("KeplerModel ffn intermediate transpose failed: {}", e),
                }
            })?)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel ffn intermediate matmul failed: {}", e),
            })?
            .broadcast_add(&ffn.intermediate_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel ffn intermediate bias failed: {}", e),
            })?;

        // GELU activation
        let activated = intermediate.gelu().map_err(|e| EmbeddingError::GpuError {
            message: format!("KeplerModel ffn gelu failed: {}", e),
        })?;

        // Output projection
        activated
            .matmul(&ffn.output_weight.t().map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel ffn output transpose failed: {}", e),
            })?)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel ffn output matmul failed: {}", e),
            })?
            .broadcast_add(&ffn.output_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("KeplerModel ffn output bias failed: {}", e),
            })
    }
}
