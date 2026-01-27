//! Embedder index enum matching constitution.yaml embedder list.

use serde::{Deserialize, Serialize};

use super::constants::*;
use super::distance::DistanceMetric;

/// Embedder index enum matching constitution.yaml embedder list.
///
/// 18 variants total:
/// - E1-E13: Core embedders (13)
/// - E1Matryoshka128: E1 truncated to 128D for Stage 2 fast filtering
/// - E5CausalCause: E5 cause vector for asymmetric retrieval (ARCH-15)
/// - E5CausalEffect: E5 effect vector for asymmetric retrieval (ARCH-15)
/// - E10MultimodalIntent: E10 intent vector for asymmetric retrieval (ARCH-15)
/// - E10MultimodalContext: E10 context vector for asymmetric retrieval (ARCH-15)
///
/// # Non-HNSW Embedders
/// - E6Sparse: Inverted index (legacy sparse slot)
/// - E12LateInteraction: ColBERT MaxSim (token-level)
/// - E13Splade: Inverted index with BM25 (Stage 1 recall)
///
/// # Asymmetric E5 Indexes (ARCH-15, AP-77)
///
/// E5CausalCause and E5CausalEffect enable direction-aware retrieval:
/// - Cause-seeking queries search E5CausalEffect index using query.e5_as_cause
/// - Effect-seeking queries search E5CausalCause index using query.e5_as_effect
///
/// # Asymmetric E10 Indexes (ARCH-15, AP-77)
///
/// E10MultimodalIntent and E10MultimodalContext enable direction-aware retrieval:
/// - Intent-seeking queries search E10MultimodalContext index using query.e10_as_intent
/// - Context-seeking queries search E10MultimodalIntent index using query.e10_as_context
///
/// This ensures complementary vectors are compared (cause→effect, intent→context).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmbedderIndex {
    /// E1: 1024D semantic (e5-large-v2, Matryoshka-capable)
    E1Semantic,
    /// E1 truncated to 128D for Stage 2 fast filtering
    E1Matryoshka128,
    /// E2: 512D temporal recent (exponential decay)
    E2TemporalRecent,
    /// E3: 512D temporal periodic (Fourier)
    E3TemporalPeriodic,
    /// E4: 512D temporal positional (sinusoidal PE)
    E4TemporalPositional,
    /// E5: 768D causal (Longformer SCM, asymmetric similarity)
    /// Legacy index using active vector - prefer E5CausalCause/E5CausalEffect
    E5Causal,
    /// E5 Causal Cause: 768D cause vector (ARCH-15)
    /// Search this index when query seeks effects (what happens when X?)
    E5CausalCause,
    /// E5 Causal Effect: 768D effect vector (ARCH-15)
    /// Search this index when query seeks causes (why does X happen?)
    E5CausalEffect,
    /// E6: ~30K sparse (inverted index, NOT HNSW)
    E6Sparse,
    /// E7: 1536D code (Qodo-Embed-1-1.5B)
    E7Code,
    /// E8: 1024D graph (e5-large-v2)
    E8Graph,
    /// E9: 1024D HDC (projected from 10K-bit)
    E9HDC,
    /// E10: 768D multimodal (CLIP)
    /// Legacy index using active vector - prefer E10MultimodalIntent/E10MultimodalContext
    E10Multimodal,
    /// E10 Multimodal Intent: 768D intent vector (ARCH-15)
    /// Search this index when query seeks context/answers (what context satisfies intent X?)
    E10MultimodalIntent,
    /// E10 Multimodal Context: 768D context vector (ARCH-15)
    /// Search this index when query seeks intents/goals (what intent does context Y serve?)
    E10MultimodalContext,
    /// E11: 768D entity (KEPLER)
    E11Entity,
    /// E12: 128D per-token ColBERT (MaxSim, NOT HNSW)
    E12LateInteraction,
    /// E13: ~30K SPLADE sparse (inverted index, NOT HNSW)
    E13Splade,
}

impl EmbedderIndex {
    /// Map 0-12 index to embedder. Panics on out of bounds.
    ///
    /// # Panics
    ///
    /// Panics with "INDEX ERROR" if `idx >= 13`.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_storage::teleological::indexes::EmbedderIndex;
    ///
    /// let e1 = EmbedderIndex::from_index(0);
    /// assert_eq!(e1, EmbedderIndex::E1Semantic);
    /// ```
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::E1Semantic,
            1 => Self::E2TemporalRecent,
            2 => Self::E3TemporalPeriodic,
            3 => Self::E4TemporalPositional,
            4 => Self::E5Causal,
            5 => Self::E6Sparse,
            6 => Self::E7Code,
            7 => Self::E8Graph,
            8 => Self::E9HDC,
            9 => Self::E10Multimodal,
            10 => Self::E11Entity,
            11 => Self::E12LateInteraction,
            12 => Self::E13Splade,
            _ => panic!("INDEX ERROR: embedder index {} out of bounds (max 12)", idx),
        }
    }

    /// Get 0-12 index from embedder. Returns None for special indexes.
    ///
    /// Returns None for:
    /// - E1Matryoshka128: Special fast-filter variant
    /// - E5CausalCause: Asymmetric index (not part of core 13-array)
    /// - E5CausalEffect: Asymmetric index (not part of core 13-array)
    /// - E10MultimodalIntent: Asymmetric index (not part of core 13-array)
    /// - E10MultimodalContext: Asymmetric index (not part of core 13-array)
    pub fn to_index(&self) -> Option<usize> {
        match self {
            Self::E1Semantic => Some(0),
            Self::E2TemporalRecent => Some(1),
            Self::E3TemporalPeriodic => Some(2),
            Self::E4TemporalPositional => Some(3),
            Self::E5Causal => Some(4),
            Self::E6Sparse => Some(5),
            Self::E7Code => Some(6),
            Self::E8Graph => Some(7),
            Self::E9HDC => Some(8),
            Self::E10Multimodal => Some(9),
            Self::E11Entity => Some(10),
            Self::E12LateInteraction => Some(11),
            Self::E13Splade => Some(12),
            // Special indexes not part of core 13-array
            Self::E1Matryoshka128 => None,
            Self::E5CausalCause => None,
            Self::E5CausalEffect => None,
            Self::E10MultimodalIntent => None,
            Self::E10MultimodalContext => None,
        }
    }

    /// Check if this embedder uses HNSW indexing.
    ///
    /// Returns false for:
    /// - E6Sparse (inverted index)
    /// - E12LateInteraction (MaxSim token-level)
    /// - E13Splade (inverted index with BM25)
    ///
    /// Returns true for E5CausalCause and E5CausalEffect (asymmetric HNSW indexes).
    #[inline]
    pub fn uses_hnsw(&self) -> bool {
        !matches!(
            self,
            Self::E6Sparse | Self::E12LateInteraction | Self::E13Splade
        )
    }

    /// Check if this embedder uses inverted indexing.
    ///
    /// Returns true for E6Sparse and E13Splade only.
    #[inline]
    pub fn uses_inverted_index(&self) -> bool {
        matches!(self, Self::E6Sparse | Self::E13Splade)
    }

    /// Get all HNSW-capable embedder indexes.
    ///
    /// Returns 15 entries (excludes E6, E12, E13):
    /// - 10 dense embedders (E1-E5, E7-E11)
    /// - E1Matryoshka128 (Stage 2 fast filter)
    /// - E5CausalCause (asymmetric cause index, ARCH-15)
    /// - E5CausalEffect (asymmetric effect index, ARCH-15)
    /// - E10MultimodalIntent (asymmetric intent index, ARCH-15)
    /// - E10MultimodalContext (asymmetric context index, ARCH-15)
    pub fn all_hnsw() -> Vec<Self> {
        vec![
            Self::E1Semantic,
            Self::E1Matryoshka128,
            Self::E2TemporalRecent,
            Self::E3TemporalPeriodic,
            Self::E4TemporalPositional,
            Self::E5Causal,
            Self::E5CausalCause,
            Self::E5CausalEffect,
            Self::E7Code,
            Self::E8Graph,
            Self::E9HDC,
            Self::E10Multimodal,
            Self::E10MultimodalIntent,
            Self::E10MultimodalContext,
            Self::E11Entity,
        ]
    }

    /// Get the embedding dimension for this embedder.
    ///
    /// Returns None for E6Sparse, E12LateInteraction, E13Splade (non-dense).
    pub fn dimension(&self) -> Option<usize> {
        match self {
            Self::E1Semantic => Some(E1_DIM),
            Self::E1Matryoshka128 => Some(E1_MATRYOSHKA_DIM),
            Self::E2TemporalRecent => Some(E2_DIM),
            Self::E3TemporalPeriodic => Some(E3_DIM),
            Self::E4TemporalPositional => Some(E4_DIM),
            Self::E5Causal => Some(E5_DIM),
            Self::E5CausalCause => Some(E5_DIM),       // 768D cause vector
            Self::E5CausalEffect => Some(E5_DIM),     // 768D effect vector
            Self::E6Sparse => None,                   // Inverted index
            Self::E7Code => Some(E7_DIM),
            Self::E8Graph => Some(E8_DIM),
            Self::E9HDC => Some(E9_DIM),
            Self::E10Multimodal => Some(E10_DIM),
            Self::E10MultimodalIntent => Some(E10_DIM),   // 768D intent vector
            Self::E10MultimodalContext => Some(E10_DIM), // 768D context vector
            Self::E11Entity => Some(E11_DIM),
            Self::E12LateInteraction => None, // Token-level
            Self::E13Splade => None,          // Inverted index
        }
    }

    /// Get the recommended distance metric for this embedder.
    ///
    /// E5 and E10 variants use AsymmetricCosine per ARCH-15, AP-77.
    pub fn recommended_metric(&self) -> Option<DistanceMetric> {
        match self {
            // E5 asymmetric: causal relationships are directional
            Self::E5Causal | Self::E5CausalCause | Self::E5CausalEffect => {
                Some(DistanceMetric::AsymmetricCosine)
            }
            // E10 asymmetric: intent/context relationships are directional
            Self::E10Multimodal | Self::E10MultimodalIntent | Self::E10MultimodalContext => {
                Some(DistanceMetric::AsymmetricCosine)
            }
            Self::E6Sparse | Self::E13Splade => None, // Inverted index
            Self::E12LateInteraction => Some(DistanceMetric::MaxSim),
            _ => Some(DistanceMetric::Cosine),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_index_all_valid() {
        for i in 0..13 {
            let idx = EmbedderIndex::from_index(i);
            assert_eq!(
                idx.to_index(),
                Some(i),
                "Index {} should map back to Some({})",
                i,
                i
            );
        }
    }

    #[test]
    #[should_panic(expected = "INDEX ERROR")]
    fn test_panic_on_index_13() {
        let _ = EmbedderIndex::from_index(13);
    }

    #[test]
    fn test_embedder_uses_hnsw() {
        assert!(EmbedderIndex::E1Semantic.uses_hnsw());
        assert!(EmbedderIndex::E1Matryoshka128.uses_hnsw());
        // E5 asymmetric indexes use HNSW (ARCH-15)
        assert!(EmbedderIndex::E5CausalCause.uses_hnsw());
        assert!(EmbedderIndex::E5CausalEffect.uses_hnsw());

        assert!(!EmbedderIndex::E6Sparse.uses_hnsw());
        assert!(!EmbedderIndex::E12LateInteraction.uses_hnsw());
        assert!(!EmbedderIndex::E13Splade.uses_hnsw());
    }

    #[test]
    fn test_embedder_uses_inverted_index() {
        assert!(EmbedderIndex::E6Sparse.uses_inverted_index());
        assert!(EmbedderIndex::E13Splade.uses_inverted_index());

        assert!(!EmbedderIndex::E1Semantic.uses_inverted_index());
        assert!(!EmbedderIndex::E12LateInteraction.uses_inverted_index());
        // E5 asymmetric indexes do NOT use inverted index
        assert!(!EmbedderIndex::E5CausalCause.uses_inverted_index());
        assert!(!EmbedderIndex::E5CausalEffect.uses_inverted_index());
    }

    #[test]
    fn test_all_hnsw_count_is_15() {
        // 11 original + 2 E5 asymmetric + 2 E10 asymmetric = 15 HNSW indexes
        let hnsw_embedders = EmbedderIndex::all_hnsw();
        assert_eq!(hnsw_embedders.len(), 15);
        // Verify E5 asymmetric indexes are included
        assert!(hnsw_embedders.contains(&EmbedderIndex::E5CausalCause));
        assert!(hnsw_embedders.contains(&EmbedderIndex::E5CausalEffect));
        // Verify E10 asymmetric indexes are included
        assert!(hnsw_embedders.contains(&EmbedderIndex::E10MultimodalIntent));
        assert!(hnsw_embedders.contains(&EmbedderIndex::E10MultimodalContext));
    }

    #[test]
    fn test_to_index_special_embedders() {
        // E1Matryoshka128 is not part of core 13-array
        assert_eq!(EmbedderIndex::E1Matryoshka128.to_index(), None);
        // E5 asymmetric indexes are not part of core 13-array
        assert_eq!(EmbedderIndex::E5CausalCause.to_index(), None);
        assert_eq!(EmbedderIndex::E5CausalEffect.to_index(), None);
        // E10 asymmetric indexes are not part of core 13-array
        assert_eq!(EmbedderIndex::E10MultimodalIntent.to_index(), None);
        assert_eq!(EmbedderIndex::E10MultimodalContext.to_index(), None);
    }

    #[test]
    fn test_max_index_is_12() {
        let idx = EmbedderIndex::from_index(12);
        assert_eq!(idx, EmbedderIndex::E13Splade);
    }

    #[test]
    fn test_e5_asymmetric_dimensions() {
        // Both E5 asymmetric indexes have 768D (same as E5Causal)
        assert_eq!(EmbedderIndex::E5CausalCause.dimension(), Some(E5_DIM));
        assert_eq!(EmbedderIndex::E5CausalEffect.dimension(), Some(E5_DIM));
        assert_eq!(EmbedderIndex::E5Causal.dimension(), Some(E5_DIM));
    }

    #[test]
    fn test_e5_asymmetric_metric() {
        // All E5 variants use AsymmetricCosine per ARCH-15, AP-77
        assert_eq!(
            EmbedderIndex::E5Causal.recommended_metric(),
            Some(DistanceMetric::AsymmetricCosine)
        );
        assert_eq!(
            EmbedderIndex::E5CausalCause.recommended_metric(),
            Some(DistanceMetric::AsymmetricCosine)
        );
        assert_eq!(
            EmbedderIndex::E5CausalEffect.recommended_metric(),
            Some(DistanceMetric::AsymmetricCosine)
        );
    }

    #[test]
    fn test_e10_asymmetric_dimensions() {
        // Both E10 asymmetric indexes have 768D (same as E10Multimodal)
        assert_eq!(EmbedderIndex::E10MultimodalIntent.dimension(), Some(E10_DIM));
        assert_eq!(EmbedderIndex::E10MultimodalContext.dimension(), Some(E10_DIM));
        assert_eq!(EmbedderIndex::E10Multimodal.dimension(), Some(E10_DIM));
    }

    #[test]
    fn test_e10_asymmetric_metric() {
        // All E10 variants use AsymmetricCosine per ARCH-15, AP-77
        assert_eq!(
            EmbedderIndex::E10Multimodal.recommended_metric(),
            Some(DistanceMetric::AsymmetricCosine)
        );
        assert_eq!(
            EmbedderIndex::E10MultimodalIntent.recommended_metric(),
            Some(DistanceMetric::AsymmetricCosine)
        );
        assert_eq!(
            EmbedderIndex::E10MultimodalContext.recommended_metric(),
            Some(DistanceMetric::AsymmetricCosine)
        );
    }

    #[test]
    fn test_e10_asymmetric_uses_hnsw() {
        // E10 asymmetric indexes use HNSW (ARCH-15)
        assert!(EmbedderIndex::E10MultimodalIntent.uses_hnsw());
        assert!(EmbedderIndex::E10MultimodalContext.uses_hnsw());
        // Verify they don't use inverted index
        assert!(!EmbedderIndex::E10MultimodalIntent.uses_inverted_index());
        assert!(!EmbedderIndex::E10MultimodalContext.uses_inverted_index());
    }
}
