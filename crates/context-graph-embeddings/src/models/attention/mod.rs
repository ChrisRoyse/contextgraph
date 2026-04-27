//! Attention strategy abstraction for scaled dot-product attention.
//!
//! Provides pluggable attention mechanisms that all compute the same logical operation:
//!   output = softmax(QK^T / sqrt(d) + mask) @ V
//!
//! Strategies differ in memory usage and computation pattern:
//! - `DenseAttention`: Full O(n^2) memory — current default, bit-exact with legacy code
//! - `TiledAttention`: O(n^2) compute, O(n) memory — tiled online softmax
//! - `SlidingWindowAttention`: O(n*w) — each token attends to a local window
//!
//! # Strategy Selection
//!
//! ```text
//! AttentionMode::Dense              -> DenseAttention (legacy fallback)
//! AttentionMode::MemoryEfficient    -> TiledAttention (recommended default)
//! AttentionMode::SlidingWindow      -> SlidingWindowAttention (local-only models)
//! ```

pub mod dense;
pub mod sliding_window;
pub mod tiled;

use candle_core::Tensor;
use serde::{Deserialize, Serialize};

use crate::error::EmbeddingResult;

/// Strategy for computing scaled dot-product attention.
///
/// All strategies compute the same logical operation:
///   output = softmax(QK^T / sqrt(d) + mask) @ V
///
/// They differ in memory usage and computation pattern.
///
/// Inputs are always in the shape:
/// - q: [batch, heads, seq_len, head_dim]
/// - k: [batch, heads, seq_len, head_dim] (already expanded for GQA)
/// - v: [batch, heads, seq_len, head_dim] (already expanded for GQA)
/// - mask: broadcastable to [batch, heads, seq_len, seq_len]
/// - scale: 1/sqrt(head_dim) or sqrt(head_dim) depending on usage
///
/// The `scale` parameter is the divisor (sqrt(head_dim)). Implementations
/// divide scores by this value.
pub trait AttentionStrategy: Send + Sync {
    /// Compute attention given pre-projected Q, K, V tensors.
    ///
    /// Returns: [batch, heads, seq_len, head_dim]
    fn forward(
        &self,
        q: &Tensor,
        k: &Tensor,
        v: &Tensor,
        mask: &Tensor,
        scale: f64,
    ) -> EmbeddingResult<Tensor>;

    /// Name for logging/debugging.
    fn name(&self) -> &str;
}

/// Configuration for attention strategy selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AttentionMode {
    /// Full O(n^2) dense attention — current default.
    Dense,
    /// Tiled memory-efficient attention — O(n^2) compute, O(n) memory.
    /// Same output as Dense but uses constant memory per tile.
    MemoryEfficient {
        #[serde(default = "default_tile_size")]
        tile_size: usize,
    },
    /// Sliding window — O(n*w) where w = window_size.
    /// Each token attends to at most `window_size` neighbors.
    SlidingWindow {
        #[serde(default = "default_window_size")]
        window_size: usize,
    },
}

fn default_tile_size() -> usize {
    256
}

fn default_window_size() -> usize {
    256
}

impl Default for AttentionMode {
    fn default() -> Self {
        AttentionMode::MemoryEfficient {
            tile_size: default_tile_size(),
        }
    }
}

/// Create an attention strategy from a mode configuration.
pub fn create_strategy(mode: &AttentionMode) -> Box<dyn AttentionStrategy> {
    match mode {
        AttentionMode::Dense => Box::new(dense::DenseAttention),
        AttentionMode::MemoryEfficient { tile_size } => {
            Box::new(tiled::TiledAttention::new(*tile_size))
        }
        AttentionMode::SlidingWindow { window_size } => {
            Box::new(sliding_window::SlidingWindowAttention::new(*window_size))
        }
    }
}

/// Resolve an `AttentionMode` from the config, handling legacy `use_flash_attention` flag.
///
/// Priority:
/// 1. If `attention_mode` is explicitly set in config → use it
/// 2. If only `use_flash_attention: true` (legacy) → MemoryEfficient { tile_size: 256 }
/// 3. If `use_flash_attention: false` → Dense
pub fn resolve_attention_mode(
    attention_mode: Option<&AttentionMode>,
    use_flash_attention: bool,
) -> AttentionMode {
    if let Some(mode) = attention_mode {
        return mode.clone();
    }
    if use_flash_attention {
        AttentionMode::MemoryEfficient {
            tile_size: default_tile_size(),
        }
    } else {
        AttentionMode::Dense
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_attention_mode() {
        let mode = AttentionMode::default();
        assert_eq!(mode, AttentionMode::MemoryEfficient { tile_size: 256 });
    }

    #[test]
    fn test_resolve_with_explicit_mode() {
        let mode = AttentionMode::Dense;
        let resolved = resolve_attention_mode(Some(&mode), true);
        assert_eq!(resolved, AttentionMode::Dense);
    }

    #[test]
    fn test_resolve_flash_attention_true() {
        let resolved = resolve_attention_mode(None, true);
        assert_eq!(resolved, AttentionMode::MemoryEfficient { tile_size: 256 });
    }

    #[test]
    fn test_resolve_flash_attention_false() {
        let resolved = resolve_attention_mode(None, false);
        assert_eq!(resolved, AttentionMode::Dense);
    }

    #[test]
    fn test_create_strategy_dense() {
        let strategy = create_strategy(&AttentionMode::Dense);
        assert_eq!(strategy.name(), "dense");
    }

    #[test]
    fn test_create_strategy_tiled() {
        let strategy = create_strategy(&AttentionMode::MemoryEfficient { tile_size: 128 });
        assert_eq!(strategy.name(), "tiled_memory_efficient");
    }

    #[test]
    fn test_create_strategy_sliding_window() {
        let strategy = create_strategy(&AttentionMode::SlidingWindow { window_size: 512 });
        assert_eq!(strategy.name(), "sliding_window");
    }

    #[test]
    fn test_attention_mode_serde_roundtrip() {
        let modes = vec![
            AttentionMode::Dense,
            AttentionMode::MemoryEfficient { tile_size: 128 },
            AttentionMode::SlidingWindow { window_size: 512 },
        ];
        for mode in &modes {
            let json = serde_json::to_string(mode).unwrap();
            let restored: AttentionMode = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, mode);
        }
    }
}
