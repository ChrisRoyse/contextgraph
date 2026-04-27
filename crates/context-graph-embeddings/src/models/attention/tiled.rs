//! Tiled memory-efficient attention using online softmax.
//!
//! Implements the FlashAttention-2 algorithm in pure candle tensor ops:
//! same O(n^2) compute but only O(n) memory (O(Br*Bc) per tile instead
//! of the full O(n^2) score matrix).
//!
//! # Algorithm
//!
//! For each tile of Q rows (size Br):
//!   Initialize: m_i = -inf, l_i = 0, O_i = 0
//!   For each tile of K,V columns (size Bc):
//!     S_ij = Q_i @ K_j^T / sqrt(d)        // [Br, Bc] tile
//!     S_ij += mask_ij
//!     m_new = max(m_old, rowmax(S_ij))
//!     P_ij = exp(S_ij - m_new)
//!     l_new = exp(m_old - m_new) * l_old + rowsum(P_ij)
//!     O_i = (exp(m_old - m_new) * l_old / l_new) * O_i + (1/l_new) * P_ij @ V_j
//!
//! This produces bit-identical results to dense attention (within f32 tolerance).
//!
//! # Tile Size Selection
//!
//! Default tile_size=256. For 512-token models: 2x2=4 tiles (negligible overhead).
//! For 32K tokens: 128x128=16,384 tiles (acceptable for 15,000x memory savings).

use candle_core::{DType, Tensor};

use crate::error::{EmbeddingError, EmbeddingResult};

use super::AttentionStrategy;

/// Tiled memory-efficient attention with online softmax.
///
/// Computes exact same output as `DenseAttention` but with O(n) peak memory
/// instead of O(n^2) for the attention score matrix.
pub struct TiledAttention {
    tile_size: usize,
}

impl TiledAttention {
    pub fn new(tile_size: usize) -> Self {
        Self { tile_size }
    }
}

impl AttentionStrategy for TiledAttention {
    fn forward(
        &self,
        q: &Tensor,
        k: &Tensor,
        v: &Tensor,
        mask: &Tensor,
        scale: f64,
    ) -> EmbeddingResult<Tensor> {
        let (batch_size, num_heads, seq_len, head_dim) =
            q.dims4().map_err(|e| EmbeddingError::GpuError {
                message: format!("TiledAttention Q dims failed: {}", e),
            })?;

        // For short sequences, fall back to dense attention to avoid overhead
        if seq_len <= self.tile_size {
            return super::dense::DenseAttention.forward(q, k, v, mask, scale);
        }

        let device = q.device();
        let dtype = q.dtype();

        // Work in F32 for numerical stability of online softmax
        let q_f32 = to_f32(q, "Q")?;
        let k_f32 = to_f32(k, "K")?;
        let v_f32 = to_f32(v, "V")?;
        let mask_f32 = to_f32(mask, "mask")?;

        let br = self.tile_size; // query tile size
        let bc = self.tile_size; // key/value tile size
        let num_q_tiles = seq_len.div_ceil(br);

        // Collect output tiles
        let mut output_tiles: Vec<Tensor> = Vec::with_capacity(num_q_tiles);

        for qi in 0..num_q_tiles {
            let q_start = qi * br;
            let q_len = br.min(seq_len - q_start);

            // Q tile: [batch, heads, q_len, head_dim]
            let q_tile = q_f32
                .narrow(2, q_start, q_len)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("TiledAttention Q tile narrow failed: {}", e),
                })?;

            // Initialize accumulators for this Q tile
            // m: row-wise max of scores seen so far [batch, heads, q_len, 1]
            let mut m_i =
                Tensor::full(f32::NEG_INFINITY, (batch_size, num_heads, q_len, 1), device)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention init m failed: {}", e),
                    })?;

            // l: row-wise sum of exp(scores - m) [batch, heads, q_len, 1]
            let mut l_i = Tensor::zeros((batch_size, num_heads, q_len, 1), DType::F32, device)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("TiledAttention init l failed: {}", e),
                })?;

            // O: accumulated output [batch, heads, q_len, head_dim]
            let mut o_i =
                Tensor::zeros((batch_size, num_heads, q_len, head_dim), DType::F32, device)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention init O failed: {}", e),
                    })?;

            let num_kv_tiles = seq_len.div_ceil(bc);

            for kj in 0..num_kv_tiles {
                let k_start = kj * bc;
                let k_len = bc.min(seq_len - k_start);

                // K tile: [batch, heads, k_len, head_dim]
                let k_tile =
                    k_f32
                        .narrow(2, k_start, k_len)
                        .map_err(|e| EmbeddingError::GpuError {
                            message: format!("TiledAttention K tile narrow failed: {}", e),
                        })?;

                // V tile: [batch, heads, k_len, head_dim]
                let v_tile =
                    v_f32
                        .narrow(2, k_start, k_len)
                        .map_err(|e| EmbeddingError::GpuError {
                            message: format!("TiledAttention V tile narrow failed: {}", e),
                        })?;

                // S_ij = Q_i @ K_j^T / scale: [batch, heads, q_len, k_len]
                let k_tile_t = k_tile
                    .transpose(2, 3)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention K tile transpose failed: {}", e),
                    })?
                    .contiguous()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention K^T contiguous failed: {}", e),
                    })?;

                let s_ij = q_tile
                    .matmul(&k_tile_t)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention tile QK matmul failed: {}", e),
                    })?;

                let s_ij = (s_ij / scale).map_err(|e| EmbeddingError::GpuError {
                    message: format!("TiledAttention tile scale failed: {}", e),
                })?;

                // Apply mask tile. The mask may be [batch, 1, 1, seq_len] or
                // [batch, 1, seq_len, seq_len]. We need the appropriate slice.
                let mask_dims = mask_f32.dims();
                let s_ij = if mask_dims.len() == 4 && mask_dims[2] > 1 {
                    // Mask is [batch, 1, seq_len, seq_len] — slice both dims
                    let mask_tile = mask_f32
                        .narrow(2, q_start, q_len)
                        .map_err(|e| EmbeddingError::GpuError {
                            message: format!("TiledAttention mask narrow q failed: {}", e),
                        })?
                        .narrow(3, k_start, k_len)
                        .map_err(|e| EmbeddingError::GpuError {
                            message: format!("TiledAttention mask narrow k failed: {}", e),
                        })?;
                    s_ij.broadcast_add(&mask_tile)
                        .map_err(|e| EmbeddingError::GpuError {
                            message: format!("TiledAttention mask add (2d) failed: {}", e),
                        })?
                } else {
                    // Mask is [batch, 1, 1, seq_len] — slice last dim only
                    let mask_tile = mask_f32.narrow(3, k_start, k_len).map_err(|e| {
                        EmbeddingError::GpuError {
                            message: format!("TiledAttention mask narrow failed: {}", e),
                        }
                    })?;
                    s_ij.broadcast_add(&mask_tile)
                        .map_err(|e| EmbeddingError::GpuError {
                            message: format!("TiledAttention mask add failed: {}", e),
                        })?
                };

                // Online softmax update
                // m_ij = rowmax(S_ij): [batch, heads, q_len, 1]
                let m_ij = s_ij.max_keepdim(candle_core::D::Minus1).map_err(|e| {
                    EmbeddingError::GpuError {
                        message: format!("TiledAttention rowmax failed: {}", e),
                    }
                })?;

                // m_new = max(m_old, m_ij)
                let m_new = m_i.maximum(&m_ij).map_err(|e| EmbeddingError::GpuError {
                    message: format!("TiledAttention m_new max failed: {}", e),
                })?;

                // P_ij = exp(S_ij - m_new): [batch, heads, q_len, k_len]
                let p_ij = s_ij
                    .broadcast_sub(&m_new)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention S - m_new failed: {}", e),
                    })?
                    .exp()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention exp failed: {}", e),
                    })?;

                // Correction factor: exp(m_old - m_new): [batch, heads, q_len, 1]
                let correction = m_i
                    .broadcast_sub(&m_new)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention m_old - m_new failed: {}", e),
                    })?
                    .exp()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention correction exp failed: {}", e),
                    })?;

                // l_new = correction * l_old + rowsum(P_ij)
                let p_rowsum = p_ij.sum_keepdim(candle_core::D::Minus1).map_err(|e| {
                    EmbeddingError::GpuError {
                        message: format!("TiledAttention P rowsum failed: {}", e),
                    }
                })?;

                let l_new = correction
                    .broadcast_mul(&l_i)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention correction * l_old failed: {}", e),
                    })?
                    .broadcast_add(&p_rowsum)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention l_new add failed: {}", e),
                    })?;

                // O_new = correction * O_old + P_ij @ V_j
                let pv = p_ij.matmul(&v_tile).map_err(|e| EmbeddingError::GpuError {
                    message: format!("TiledAttention PV matmul failed: {}", e),
                })?;

                o_i = correction
                    .broadcast_mul(&o_i)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention correction * O failed: {}", e),
                    })?
                    .add(&pv)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("TiledAttention O + PV failed: {}", e),
                    })?;

                m_i = m_new;
                l_i = l_new;
            }

            // Normalize: O_i = O_i / l_i
            let o_tile = o_i
                .broadcast_div(&l_i)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("TiledAttention O/l normalize failed: {}", e),
                })?;

            output_tiles.push(o_tile);
        }

        // Concatenate all Q-tile outputs along the seq_len dimension
        let output_refs: Vec<&Tensor> = output_tiles.iter().collect();
        let output = Tensor::cat(&output_refs, 2).map_err(|e| EmbeddingError::GpuError {
            message: format!("TiledAttention output cat failed: {}", e),
        })?;

        // Convert back to original dtype
        output
            .to_dtype(dtype)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("TiledAttention dtype conversion failed: {}", e),
            })
    }

    fn name(&self) -> &str {
        "tiled_memory_efficient"
    }
}

/// Convert tensor to F32, no-op if already F32.
fn to_f32(t: &Tensor, name: &str) -> EmbeddingResult<Tensor> {
    if t.dtype() == DType::F32 {
        return Ok(t.clone());
    }
    t.to_dtype(DType::F32)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("TiledAttention {} to F32 failed: {}", name, e),
        })
}
