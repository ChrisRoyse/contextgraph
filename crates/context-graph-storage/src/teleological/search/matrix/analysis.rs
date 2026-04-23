//! Matrix and correlation analysis types.
//!
//! # Design Philosophy
//!
//! **FAIL FAST. NO FALLBACKS.**

// ============================================================================
// MATRIX ANALYSIS
// ============================================================================

/// Analysis of matrix structure for execution optimization.
#[derive(Debug, Clone)]
pub struct MatrixAnalysis {
    /// Matrix is purely diagonal (no cross-correlations).
    pub is_diagonal: bool,
    /// Matrix has off-diagonal weights.
    pub has_cross_correlations: bool,
    /// Fraction of zero elements.
    pub sparsity: f32,
    /// Embedder indices with non-zero diagonal weights.
    pub active_embedders: Vec<usize>,
    /// Number of non-zero off-diagonal elements.
    pub cross_correlation_count: usize,
}

// ============================================================================
// CORRELATION ANALYSIS
// ============================================================================

/// Analysis of embedder correlations in search results.
#[derive(Debug, Clone)]
pub struct CorrelationAnalysis {
    /// 13x13 Pearson correlation matrix between embedder scores.
    pub correlation_matrix: [[f32; 14]; 14],
    /// Detected correlation patterns.
    pub patterns: Vec<CorrelationPattern>,
    /// Overall coherence score (0-1, higher = more agreement).
    pub coherence: f32,
}

/// Detected correlation patterns between embedders.
#[derive(Debug, Clone)]
pub enum CorrelationPattern {
    /// Multiple embedders strongly agree on relevance.
    ConsensusHigh {
        embedder_indices: Vec<usize>,
        strength: f32,
    },
    /// Temporal embedders align with semantic.
    TemporalSemanticAlign { strength: f32 },
    /// Code and semantic embeddings diverge.
    CodeSemanticDivergence { strength: f32 },
    /// One embedder significantly disagrees with others.
    OutlierEmbedder {
        embedder_index: usize,
        deviation: f32,
    },
}
