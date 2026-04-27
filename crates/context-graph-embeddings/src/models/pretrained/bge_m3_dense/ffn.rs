//! Feed-forward network (FFN) layer for the BGE-M3 Dense encoder.
//!
//! Standard post-LN transformer FFN: Linear → GELU → Linear.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{BertConfig, FfnWeights};

/// Run FFN forward pass: intermediate → GELU → output.
pub fn ffn_forward(
    hidden_states: &Tensor,
    ffn: &FfnWeights,
    config: &BertConfig,
    layer_idx: usize,
) -> EmbeddingResult<Tensor> {
    let (batch_size, seq_len, hidden_size) =
        hidden_states
            .dims3()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("BgeM3Dense layer {} FFN get dims failed: {}", layer_idx, e),
            })?;

    let hidden_flat = hidden_states
        .reshape((batch_size * seq_len, hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense layer {} FFN flatten failed: {}", layer_idx, e),
        })?;

    let intermediate = hidden_flat
        .matmul(
            &ffn.intermediate_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "BgeM3Dense layer {} FFN intermediate transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} FFN intermediate matmul failed: {}",
                layer_idx, e
            ),
        })?;

    let intermediate = intermediate
        .broadcast_add(&ffn.intermediate_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} FFN intermediate bias failed: {}",
                layer_idx, e
            ),
        })?;

    let intermediate = intermediate.gelu().map_err(|e| EmbeddingError::GpuError {
        message: format!("BgeM3Dense layer {} GELU failed: {}", layer_idx, e),
    })?;

    let output = intermediate
        .matmul(
            &ffn.output_weight
                .t()
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!(
                        "BgeM3Dense layer {} FFN output transpose failed: {}",
                        layer_idx, e
                    ),
                })?,
        )
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} FFN output matmul failed: {}",
                layer_idx, e
            ),
        })?;

    let output = output
        .broadcast_add(&ffn.output_bias)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!(
                "BgeM3Dense layer {} FFN output bias failed: {}",
                layer_idx, e
            ),
        })?;

    output
        .reshape((batch_size, seq_len, config.hidden_size))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense layer {} FFN reshape failed: {}", layer_idx, e),
        })
}
