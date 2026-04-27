//! SearchMatrix: 14x14 weight matrix for cross-embedder correlation search.
//!
//! # Design Philosophy
//!
//! **FAIL FAST. NO FALLBACKS.**
//!
//! All errors are fatal. No recovery attempts.

use crate::teleological::indexes::EmbedderIndex;
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;

// ============================================================================
// SEARCH MATRIX (14x14)
// ============================================================================

/// 14x14 weight matrix for cross-embedder correlation search.
///
/// Diagonal elements weight individual embedder contributions.
/// Off-diagonal elements capture cross-embedder correlations.
///
/// # Example
///
/// ```
/// use context_graph_storage::teleological::search::SearchMatrix;
///
/// // Create identity (diagonal only, no cross-correlation)
/// let identity = SearchMatrix::identity();
///
/// // Use predefined semantic-focused matrix
/// let semantic = SearchMatrix::semantic_focused();
///
/// // Create custom matrix
/// let mut custom = SearchMatrix::zeros();
/// custom.set(0, 0, 1.0);  // E1Semantic full weight
/// custom.set(6, 6, 0.5);  // E7Code half weight
/// custom.set(0, 6, 0.2);  // E1-E7 cross-correlation
/// custom.set(6, 0, 0.2);  // E7-E1 cross-correlation (symmetric)
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatrix {
    /// 14x14 weight matrix. weights[i][j] = weight for embedder i × embedder j correlation.
    weights: [[f32; 14]; 14],
}

impl SearchMatrix {
    /// Create zero matrix.
    pub fn zeros() -> Self {
        Self {
            weights: [[0.0; 14]; 14],
        }
    }

    /// Create identity matrix (diagonal = 1.0, off-diagonal = 0.0).
    pub fn identity() -> Self {
        let mut weights = [[0.0; 14]; 14];
        for i in 0..NUM_EMBEDDERS {
            weights[i][i] = 1.0;
        }
        Self { weights }
    }

    /// Create uniform matrix (all weights = 1/14).
    pub fn uniform() -> Self {
        let w = 1.0 / NUM_EMBEDDERS as f32;
        Self {
            weights: [[w; 14]; 14],
        }
    }

    /// Get weight at (i, j). Panics if i >= 14 or j >= 14.
    #[inline]
    pub fn get(&self, i: usize, j: usize) -> f32 {
        if i >= NUM_EMBEDDERS || j >= NUM_EMBEDDERS {
            panic!(
                "FAIL FAST: matrix index ({}, {}) out of bounds (max {})",
                i,
                j,
                NUM_EMBEDDERS - 1
            );
        }
        self.weights[i][j]
    }

    /// Set weight at (i, j). Panics if i >= 14 or j >= 14.
    #[inline]
    pub fn set(&mut self, i: usize, j: usize, weight: f32) {
        if i >= NUM_EMBEDDERS || j >= NUM_EMBEDDERS {
            panic!(
                "FAIL FAST: matrix index ({}, {}) out of bounds (max {})",
                i,
                j,
                NUM_EMBEDDERS - 1
            );
        }
        self.weights[i][j] = weight;
    }

    /// Get diagonal weight for embedder.
    #[inline]
    pub fn diagonal(&self, embedder: EmbedderIndex) -> f32 {
        if let Some(idx) = embedder.to_index() {
            self.weights[idx][idx]
        } else {
            0.0
        }
    }

    /// Check if matrix is diagonal (all off-diagonal = 0).
    pub fn is_diagonal(&self) -> bool {
        for i in 0..NUM_EMBEDDERS {
            for j in 0..NUM_EMBEDDERS {
                if i != j && self.weights[i][j].abs() > 1e-9 {
                    return false;
                }
            }
        }
        true
    }

    /// Check if matrix has cross-correlations (any off-diagonal > 0).
    pub fn has_cross_correlations(&self) -> bool {
        !self.is_diagonal()
    }

    /// Get sparsity (fraction of zero elements).
    pub fn sparsity(&self) -> f32 {
        let mut zeros = 0;
        for i in 0..NUM_EMBEDDERS {
            for j in 0..NUM_EMBEDDERS {
                if self.weights[i][j].abs() < 1e-9 {
                    zeros += 1;
                }
            }
        }
        zeros as f32 / (NUM_EMBEDDERS * NUM_EMBEDDERS) as f32
    }

    /// Get list of non-zero embedder indices on diagonal.
    pub fn active_embedders(&self) -> Vec<usize> {
        (0..NUM_EMBEDDERS)
            .filter(|&i| self.weights[i][i].abs() > 1e-9)
            .collect()
    }

    // === PREDEFINED MATRICES ===

    /// Semantic-focused: E1Semantic=1.0, E5Causal=0.3, E1-E5 cross=0.2
    pub fn semantic_focused() -> Self {
        let mut m = Self::zeros();
        m.set(0, 0, 1.0); // E1Semantic
        m.set(4, 4, 0.3); // E5Causal
        m.set(0, 4, 0.2); // E1-E5 cross
        m.set(4, 0, 0.2); // E5-E1 cross (symmetric)
        m
    }

    /// Code-heavy: E7Code=1.0, E1Semantic=0.3, E1-E7 cross=0.2
    pub fn code_heavy() -> Self {
        let mut m = Self::zeros();
        m.set(6, 6, 1.0); // E7Code
        m.set(0, 0, 0.3); // E1Semantic
        m.set(0, 6, 0.2); // E1-E7 cross
        m.set(6, 0, 0.2); // E7-E1 cross
        m
    }

    /// Temporal-aware: E2+E3+E4=0.8, E1=0.5, temporal cross=0.1
    pub fn temporal_aware() -> Self {
        let mut m = Self::zeros();
        m.set(0, 0, 0.5); // E1Semantic
        m.set(1, 1, 0.8); // E2TemporalRecent
        m.set(2, 2, 0.8); // E3TemporalPeriodic
        m.set(3, 3, 0.8); // E4TemporalPositional
                          // Temporal cross-correlations
        m.set(1, 2, 0.1);
        m.set(2, 1, 0.1);
        m.set(1, 3, 0.1);
        m.set(3, 1, 0.1);
        m.set(2, 3, 0.1);
        m.set(3, 2, 0.1);
        m
    }

    /// Balanced: all 10 HNSW embedders = 1/10 (excludes E6, E12, E13)
    pub fn balanced() -> Self {
        let w = 0.1;
        let mut m = Self::zeros();
        // Include all HNSW-capable: 0,1,2,3,4,6,7,8,9,10 (skip 5,11,12)
        for i in [0, 1, 2, 3, 4, 6, 7, 8, 9, 10] {
            m.set(i, i, w);
        }
        m
    }

    /// Entity-focused: E11Entity=1.0, E1Semantic=0.4, E8Graph=0.3
    pub fn entity_focused() -> Self {
        let mut m = Self::zeros();
        m.set(10, 10, 1.0); // E11Entity
        m.set(0, 0, 0.4); // E1Semantic
        m.set(7, 7, 0.3); // E8Graph
        m
    }
}

impl Default for SearchMatrix {
    fn default() -> Self {
        Self::balanced()
    }
}
