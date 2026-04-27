//! CLS pooling and L2 normalisation for the BGE-M3 Dense model.
//!
//! Unlike `SemanticModel` (mean pooling), BGE-M3's dense head uses the
//! CLS token representation (position 0 in the sequence) followed by
//! L2 normalisation. This matches the official BAAI/bge-m3 reference
//! implementation — see the dense-retrieval path in FlagEmbedding.

use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::normalize_gpu;

/// CLS-pool the encoder output and L2-normalise to produce the dense vector.
///
/// The encoder output is `[batch=1, seq_len, hidden_size]`. We pick the token
/// at position 0 (the `<s>` / CLS slot inserted by the XLM-R tokenizer) and
/// normalise.
///
/// # Arguments
/// * `hidden_states` - Final encoder output, shape `[1, seq_len, hidden_size]`.
/// * `hidden_size`   - Expected output dim (1024 for BGE-M3 Dense).
pub fn pool_and_normalize(hidden_states: &Tensor, hidden_size: usize) -> EmbeddingResult<Vec<f32>> {
    let (batch, seq_len, hidden) = hidden_states
        .dims3()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense pool get_dims failed: {}", e),
        })?;

    if hidden != hidden_size {
        return Err(EmbeddingError::InvalidDimension {
            expected: hidden_size,
            actual: hidden,
        });
    }
    if seq_len == 0 {
        return Err(EmbeddingError::GpuError {
            message: "BgeM3Dense CLS pool: empty sequence (seq_len=0)".to_string(),
        });
    }

    // Slice `hidden_states[:, 0:1, :]` to extract the CLS token representation.
    let cls = hidden_states
        .narrow(1, 0, 1)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense CLS narrow failed: {}", e),
        })?
        .reshape((batch, hidden))
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense CLS reshape failed: {}", e),
        })?;

    let normalized = normalize_gpu(&cls).map_err(|e| EmbeddingError::GpuError {
        message: format!("BgeM3Dense L2 normalise failed: {}", e),
    })?;

    normalized
        .flatten_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense flatten output failed: {}", e),
        })?
        .to_vec1()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("BgeM3Dense to_vec1 failed: {}", e),
        })
}
