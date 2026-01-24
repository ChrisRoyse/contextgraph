//! HNSW index configuration types for the index module.
//!
//! These types mirror the definitions in context-graph-storage to avoid
//! cyclic dependencies between crates.
//!
//! # Note
//!
//! When the crate architecture is refactored, these types should be moved
//! to a shared `context-graph-types` crate.

use serde::{Deserialize, Serialize};

// ============================================================================
// Dimension constants (from context-graph-storage/teleological/indexes/hnsw_config/constants.rs)
// ============================================================================

/// E1 Semantic: 1024D (e5-large-v2, Matryoshka-capable)
pub const E1_DIM: usize = 1024;

/// E2 Temporal Recent: 512D (exponential decay)
pub const E2_DIM: usize = 512;

/// E3 Temporal Periodic: 512D (Fourier)
pub const E3_DIM: usize = 512;

/// E4 Temporal Positional: 512D (sinusoidal PE)
pub const E4_DIM: usize = 512;

/// E5 Causal: 768D (Longformer SCM)
pub const E5_DIM: usize = 768;

/// E6 Sparse: 30522 vocab (BERT vocabulary)
pub const E6_SPARSE_VOCAB: usize = 30_522;

/// E7 Code: 1536D (Qodo-Embed-1-1.5B)
pub const E7_DIM: usize = 1536;

/// E8 Graph: 384D (MiniLM)
pub const E8_DIM: usize = 384;

/// E9 HDC: 1024D (projected from 10K-bit hypervector)
pub const E9_DIM: usize = 1024;

/// E10 Multimodal: 768D (CLIP)
pub const E10_DIM: usize = 768;

/// E11 Entity: 768D (KEPLER RoBERTa-base + TransE)
pub const E11_DIM: usize = 768;

/// E12 Late Interaction: 128D per token (ColBERT)
pub const E12_TOKEN_DIM: usize = 128;

/// E13 SPLADE: 30522 vocab (sparse BM25)
pub const E13_SPLADE_VOCAB: usize = 30_522;

/// Number of core embedders (E1-E13)
pub const NUM_EMBEDDERS: usize = 13;

/// E1 Matryoshka truncated dimension for Stage 2
pub const E1_MATRYOSHKA_DIM: usize = 128;


// ============================================================================
// Distance metric (from context-graph-storage/teleological/indexes/hnsw_config/distance.rs)
// ============================================================================

/// Distance metric for vector similarity computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DistanceMetric {
    /// Cosine distance: 1 - cos(a, b). Range [0, 2].
    Cosine,
    /// Dot product (inner product).
    DotProduct,
    /// L2 Euclidean distance. Range [0, inf).
    Euclidean,
    /// Asymmetric cosine for E5 causal.
    AsymmetricCosine,
    /// MaxSim for ColBERT (NOT HNSW-compatible).
    MaxSim,
    /// Jaccard index for sparse vectors (NOT HNSW-compatible).
    Jaccard,
}

impl DistanceMetric {
    /// Check if this metric is compatible with HNSW indexing.
    ///
    /// MaxSim and Jaccard are NOT HNSW-compatible:
    /// - MaxSim requires token-level computation
    /// - Jaccard is for sparse vectors (uses inverted index)
    #[inline]
    pub fn is_hnsw_compatible(&self) -> bool {
        !matches!(self, Self::MaxSim | Self::Jaccard)
    }

    /// Check if this metric requires sparse vector handling.
    #[inline]
    pub fn is_sparse_metric(&self) -> bool {
        matches!(self, Self::Jaccard)
    }
}

// ============================================================================
// EmbedderIndex (from context-graph-storage/teleological/indexes/hnsw_config/embedder.rs)
// ============================================================================

/// Embedder index enum (15 variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmbedderIndex {
    /// E1: 1024D semantic
    E1Semantic,
    /// E1 truncated to 128D for Stage 2
    E1Matryoshka128,
    /// E2: 512D temporal recent
    E2TemporalRecent,
    /// E3: 512D temporal periodic
    E3TemporalPeriodic,
    /// E4: 512D temporal positional
    E4TemporalPositional,
    /// E5: 768D causal
    E5Causal,
    /// E6: sparse (NOT HNSW)
    E6Sparse,
    /// E7: 1536D code (Qodo-Embed)
    E7Code,
    /// E8: 384D graph
    E8Graph,
    /// E9: 1024D HDC (projected)
    E9HDC,
    /// E10: 768D multimodal
    E10Multimodal,
    /// E11: 768D entity (KEPLER)
    E11Entity,
    /// E12: ColBERT (NOT HNSW)
    E12LateInteraction,
    /// E13: SPLADE sparse (NOT HNSW)
    E13Splade,
}

impl EmbedderIndex {
    /// Check if this embedder uses HNSW indexing.
    #[inline]
    pub fn uses_hnsw(&self) -> bool {
        !matches!(
            self,
            Self::E6Sparse | Self::E12LateInteraction | Self::E13Splade
        )
    }

    /// Check if this embedder uses inverted indexing.
    #[inline]
    pub fn uses_inverted_index(&self) -> bool {
        matches!(self, Self::E6Sparse | Self::E13Splade)
    }

    /// Get all HNSW-capable embedder indexes (11 total).
    pub fn all_hnsw() -> Vec<Self> {
        vec![
            Self::E1Semantic,
            Self::E1Matryoshka128,
            Self::E2TemporalRecent,
            Self::E3TemporalPeriodic,
            Self::E4TemporalPositional,
            Self::E5Causal,
            Self::E7Code,
            Self::E8Graph,
            Self::E9HDC,
            Self::E10Multimodal,
            Self::E11Entity,
        ]
    }

    /// Get the embedding dimension for this embedder.
    pub fn dimension(&self) -> Option<usize> {
        match self {
            Self::E1Semantic => Some(E1_DIM),
            Self::E1Matryoshka128 => Some(E1_MATRYOSHKA_DIM),
            Self::E2TemporalRecent => Some(E2_DIM),
            Self::E3TemporalPeriodic => Some(E3_DIM),
            Self::E4TemporalPositional => Some(E4_DIM),
            Self::E5Causal => Some(E5_DIM),
            Self::E6Sparse => None,
            Self::E7Code => Some(E7_DIM),
            Self::E8Graph => Some(E8_DIM),
            Self::E9HDC => Some(E9_DIM),
            Self::E10Multimodal => Some(E10_DIM),
            Self::E11Entity => Some(E11_DIM),
            Self::E12LateInteraction => None,
            Self::E13Splade => None,
        }
    }

    /// Get the recommended distance metric for this embedder.
    pub fn recommended_metric(&self) -> Option<DistanceMetric> {
        match self {
            Self::E5Causal => Some(DistanceMetric::AsymmetricCosine),
            Self::E6Sparse | Self::E13Splade => None,
            Self::E12LateInteraction => Some(DistanceMetric::MaxSim),
            _ => Some(DistanceMetric::Cosine),
        }
    }
}

// ============================================================================
// HnswConfig (from context-graph-storage/teleological/indexes/hnsw_config/config.rs)
// ============================================================================

/// HNSW index configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// Number of bi-directional links per node (M parameter).
    pub m: usize,
    /// Size of dynamic candidate list during construction.
    pub ef_construction: usize,
    /// Size of dynamic candidate list during search.
    pub ef_search: usize,
    /// Distance metric for similarity computation.
    pub metric: DistanceMetric,
    /// Embedding dimension for this index.
    pub dimension: usize,
}

impl HnswConfig {
    /// Create config with explicit parameters.
    ///
    /// # Panics
    ///
    /// Panics if validation fails.
    pub fn new(
        m: usize,
        ef_construction: usize,
        ef_search: usize,
        metric: DistanceMetric,
        dimension: usize,
    ) -> Self {
        if m < 2 {
            panic!("HNSW CONFIG ERROR: M must be >= 2, got {}", m);
        }
        if ef_construction < m {
            panic!(
                "HNSW CONFIG ERROR: ef_construction ({}) must be >= M ({})",
                ef_construction, m
            );
        }
        if ef_search < 1 {
            panic!(
                "HNSW CONFIG ERROR: ef_search must be >= 1, got {}",
                ef_search
            );
        }
        if dimension < 1 {
            panic!(
                "HNSW CONFIG ERROR: dimension must be >= 1, got {}",
                dimension
            );
        }

        Self {
            m,
            ef_construction,
            ef_search,
            metric,
            dimension,
        }
    }

    /// Default per-embedder config: M=16, ef_construction=200, ef_search=100.
    pub fn default_for_dimension(dimension: usize, metric: DistanceMetric) -> Self {
        Self::new(16, 200, 100, metric, dimension)
    }

    /// E1 Matryoshka 128D config: M=32, ef_construction=256, ef_search=128.
    pub fn matryoshka_128d() -> Self {
        Self::new(32, 256, 128, DistanceMetric::Cosine, E1_MATRYOSHKA_DIM)
    }

    /// Estimated memory usage per vector in bytes.
    pub fn estimated_memory_per_vector(&self) -> usize {
        self.dimension * 4 + self.m * 2 * 4
    }
}

// ============================================================================
// InvertedIndexConfig
// ============================================================================

/// Configuration for inverted indexes (E6 sparse, E13 SPLADE).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvertedIndexConfig {
    /// Vocabulary size for term IDs (BERT vocab = 30,522).
    pub vocab_size: usize,
    /// Maximum non-zero entries per vector.
    pub max_nnz: usize,
    /// Whether to use BM25 weighting.
    pub use_bm25: bool,
}

impl InvertedIndexConfig {
    /// E6 sparse config: 30522 vocab, 1500 max_nnz, no BM25.
    pub fn e6_sparse() -> Self {
        Self {
            vocab_size: E6_SPARSE_VOCAB,
            max_nnz: 1_500,
            use_bm25: false,
        }
    }

    /// E13 SPLADE config: 30522 vocab, 1500 max_nnz, with BM25.
    pub fn e13_splade() -> Self {
        Self {
            vocab_size: E13_SPLADE_VOCAB,
            max_nnz: 1_500,
            use_bm25: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_uses_hnsw() {
        assert!(EmbedderIndex::E1Semantic.uses_hnsw());
        assert!(EmbedderIndex::E1Matryoshka128.uses_hnsw());

        assert!(!EmbedderIndex::E6Sparse.uses_hnsw());
        assert!(!EmbedderIndex::E12LateInteraction.uses_hnsw());
        assert!(!EmbedderIndex::E13Splade.uses_hnsw());

        println!("[VERIFIED] EmbedderIndex::uses_hnsw() works correctly");
    }

    #[test]
    fn test_all_hnsw_count() {
        let hnsw = EmbedderIndex::all_hnsw();
        assert_eq!(hnsw.len(), 11);
        println!("[VERIFIED] all_hnsw() returns 11 embedders");
    }

    #[test]
    fn test_dimension() {
        assert_eq!(EmbedderIndex::E1Semantic.dimension(), Some(1024));
        assert_eq!(EmbedderIndex::E1Matryoshka128.dimension(), Some(128));
        assert_eq!(EmbedderIndex::E6Sparse.dimension(), None);
        println!("[VERIFIED] EmbedderIndex::dimension() works correctly");
    }

    #[test]
    fn test_hnsw_config_new() {
        let cfg = HnswConfig::new(16, 200, 100, DistanceMetric::Cosine, 1024);
        assert_eq!(cfg.m, 16);
        assert_eq!(cfg.dimension, 1024);
        println!("[VERIFIED] HnswConfig::new() creates valid config");
    }

    #[test]
    fn test_inverted_config() {
        let cfg = InvertedIndexConfig::e13_splade();
        assert_eq!(cfg.vocab_size, 30_522);
        assert!(cfg.use_bm25);
        println!("[VERIFIED] InvertedIndexConfig::e13_splade() config correct");
    }
}
