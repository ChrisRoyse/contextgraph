//! Static configuration registry for all 13 embedders.
//!
//! This module provides compile-time configuration for each embedder including
//! dimension, distance metric, quantization, and category classification.
//!
//! # Usage
//!
//! ```ignore
//! use context_graph_core::embeddings::config::get_config;
//! use context_graph_core::teleological::Embedder;
//!
//! let config = get_config(Embedder::Semantic);
//! assert_eq!(config.dimension, 1024);
//! ```

use serde::{Deserialize, Serialize};

use crate::embeddings::category::{category_for, EmbedderCategory};
use crate::index::config::{
    DistanceMetric, E10_DIM, E11_DIM, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E1_DIM, E2_DIM, E3_DIM,
    E4_DIM, E5_DIM, E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM,
};
use crate::teleological::Embedder;

/// Quantization configuration for embeddings.
///
/// Determines how embeddings are compressed for storage and index efficiency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantizationConfig {
    /// Product Quantization with 8-bit codes.
    ///
    /// - `num_subvectors`: Number of subvectors to split embedding into
    /// - `bits_per_code`: Bits per quantized code (typically 8)
    PQ8 {
        num_subvectors: usize,
        bits_per_code: usize,
    },

    /// 8-bit floating point (FP8).
    ///
    /// Good balance of compression and precision for embeddings that don't need PQ.
    Float8,

    /// Binary (1-bit per dimension).
    ///
    /// Used for hyperdimensional computing embeddings.
    Binary,

    /// Inverted index for sparse vectors.
    ///
    /// Used for SPLADE and sparse BoW embeddings.
    Inverted,

    /// No quantization (full 32-bit precision).
    None,
}

/// Static configuration for a single embedder.
///
/// This struct holds all metadata needed to work with an embedder's output,
/// including storage requirements, similarity computation, and classification.
#[derive(Debug, Clone, Copy)]
pub struct EmbedderConfig {
    /// The embedder this config is for.
    pub embedder: Embedder,

    /// Embedding dimension.
    /// For sparse embeddings (E6, E13), this is the vocabulary size.
    /// For token-level (E12), this is dimension per token.
    pub dimension: usize,

    /// Distance metric for similarity computation.
    pub distance_metric: DistanceMetric,

    /// Quantization method for storage.
    pub quantization: QuantizationConfig,

    /// Whether similarity is asymmetric (e.g., E5 Causal: cause->effect != effect->cause).
    pub is_asymmetric: bool,

    /// Whether this embedder produces sparse vectors (E6, E13).
    pub is_sparse: bool,

    /// Whether this embedder produces per-token embeddings (E12 ColBERT).
    pub is_token_level: bool,
}

/// Static configuration for all 13 embedders.
///
/// Index matches Embedder::index() for O(1) lookup.
pub static EMBEDDER_CONFIGS: [EmbedderConfig; 13] = [
    // E1: Semantic (1024D, Cosine, PQ8) - Category: Semantic
    EmbedderConfig {
        embedder: Embedder::Semantic,
        dimension: E1_DIM, // 1024
        distance_metric: DistanceMetric::Cosine,
        quantization: QuantizationConfig::PQ8 {
            num_subvectors: 32,
            bits_per_code: 8,
        },
        is_asymmetric: false,
        is_sparse: false,
        is_token_level: false,
    },
    // E2: Temporal Recent (512D, Cosine, Float8) - Category: Temporal
    EmbedderConfig {
        embedder: Embedder::TemporalRecent,
        dimension: E2_DIM, // 512
        distance_metric: DistanceMetric::Cosine,
        quantization: QuantizationConfig::Float8,
        is_asymmetric: false,
        is_sparse: false,
        is_token_level: false,
    },
    // E3: Temporal Periodic (512D, Cosine, Float8) - Category: Temporal
    EmbedderConfig {
        embedder: Embedder::TemporalPeriodic,
        dimension: E3_DIM, // 512
        distance_metric: DistanceMetric::Cosine,
        quantization: QuantizationConfig::Float8,
        is_asymmetric: false,
        is_sparse: false,
        is_token_level: false,
    },
    // E4: Temporal Positional (512D, Cosine, Float8) - Category: Temporal
    EmbedderConfig {
        embedder: Embedder::TemporalPositional,
        dimension: E4_DIM, // 512
        distance_metric: DistanceMetric::Cosine,
        quantization: QuantizationConfig::Float8,
        is_asymmetric: false,
        is_sparse: false,
        is_token_level: false,
    },
    // E5: Causal (768D, AsymmetricCosine, PQ8, asymmetric) - Category: Semantic
    EmbedderConfig {
        embedder: Embedder::Causal,
        dimension: E5_DIM, // 768
        distance_metric: DistanceMetric::AsymmetricCosine,
        quantization: QuantizationConfig::PQ8 {
            num_subvectors: 24,
            bits_per_code: 8,
        },
        is_asymmetric: true,
        is_sparse: false,
        is_token_level: false,
    },
    // E6: Sparse (30522 vocab, Jaccard, Inverted) - Category: Semantic
    EmbedderConfig {
        embedder: Embedder::Sparse,
        dimension: E6_SPARSE_VOCAB, // 30522
        distance_metric: DistanceMetric::Jaccard,
        quantization: QuantizationConfig::Inverted,
        is_asymmetric: false,
        is_sparse: true,
        is_token_level: false,
    },
    // E7: Code (1536D, Cosine, PQ8) - Category: Semantic
    EmbedderConfig {
        embedder: Embedder::Code,
        dimension: E7_DIM, // 1536
        distance_metric: DistanceMetric::Cosine,
        quantization: QuantizationConfig::PQ8 {
            num_subvectors: 48,
            bits_per_code: 8,
        },
        is_asymmetric: false,
        is_sparse: false,
        is_token_level: false,
    },
    // E8: Emotional/Graph (384D, Cosine, Float8) - Category: Relational
    EmbedderConfig {
        embedder: Embedder::Emotional,
        dimension: E8_DIM, // 384
        distance_metric: DistanceMetric::Cosine,
        quantization: QuantizationConfig::Float8,
        is_asymmetric: false,
        is_sparse: false,
        is_token_level: false,
    },
    // E9: HDC (1024D projected, Cosine, PQ8) - Category: Structural
    // NOTE: Stored as dense Vec<f32> after 10K-bit projection, so Cosine not Hamming
    EmbedderConfig {
        embedder: Embedder::Hdc,
        dimension: E9_DIM, // 1024
        distance_metric: DistanceMetric::Cosine,
        quantization: QuantizationConfig::PQ8 {
            num_subvectors: 32,
            bits_per_code: 8,
        },
        is_asymmetric: false,
        is_sparse: false,
        is_token_level: false,
    },
    // E10: Multimodal (768D, Cosine, PQ8) - Category: Semantic
    EmbedderConfig {
        embedder: Embedder::Multimodal,
        dimension: E10_DIM, // 768
        distance_metric: DistanceMetric::Cosine,
        quantization: QuantizationConfig::PQ8 {
            num_subvectors: 24,
            bits_per_code: 8,
        },
        is_asymmetric: false,
        is_sparse: false,
        is_token_level: false,
    },
    // E11: Entity/KEPLER (768D, Cosine, PQ8) - Category: Relational
    EmbedderConfig {
        embedder: Embedder::Entity,
        dimension: E11_DIM, // 768 (KEPLER)
        distance_metric: DistanceMetric::Cosine,
        quantization: QuantizationConfig::Float8,
        is_asymmetric: false,
        is_sparse: false,
        is_token_level: false,
    },
    // E12: Late Interaction (128D per token, MaxSim, Float8) - Category: Semantic
    EmbedderConfig {
        embedder: Embedder::LateInteraction,
        dimension: E12_TOKEN_DIM, // 128 per token
        distance_metric: DistanceMetric::MaxSim,
        quantization: QuantizationConfig::Float8,
        is_asymmetric: false,
        is_sparse: false,
        is_token_level: true,
    },
    // E13: SPLADE (30522 vocab, Jaccard, Inverted) - Category: Semantic
    EmbedderConfig {
        embedder: Embedder::KeywordSplade,
        dimension: E13_SPLADE_VOCAB, // 30522
        distance_metric: DistanceMetric::Jaccard,
        quantization: QuantizationConfig::Inverted,
        is_asymmetric: false,
        is_sparse: true,
        is_token_level: false,
    },
];

// =============================================================================
// Getter Functions
// =============================================================================

/// Get configuration for a specific embedder.
///
/// Returns a static reference - no allocation.
/// O(1) lookup via embedder index.
#[inline]
pub fn get_config(embedder: Embedder) -> &'static EmbedderConfig {
    &EMBEDDER_CONFIGS[embedder.index()]
}

/// Get the expected dimension for an embedder.
///
/// For sparse embeddings (E6, E13), returns vocabulary size.
/// For token-level (E12), returns dimension per token.
#[inline]
pub fn get_dimension(embedder: Embedder) -> usize {
    get_config(embedder).dimension
}

/// Get the distance metric for an embedder.
#[inline]
pub fn get_distance_metric(embedder: Embedder) -> DistanceMetric {
    get_config(embedder).distance_metric
}

/// Get quantization configuration for an embedder.
#[inline]
pub fn get_quantization(embedder: Embedder) -> QuantizationConfig {
    get_config(embedder).quantization
}

/// Check if embedder uses asymmetric similarity.
///
/// Only E5 (Causal) is asymmetric - cause->effect != effect->cause.
#[inline]
pub fn is_asymmetric(embedder: Embedder) -> bool {
    get_config(embedder).is_asymmetric
}

/// Check if embedder produces sparse vectors.
///
/// E6 (Sparse) and E13 (KeywordSplade) produce sparse vectors.
#[inline]
pub fn is_sparse(embedder: Embedder) -> bool {
    get_config(embedder).is_sparse
}

/// Check if embedder produces per-token embeddings.
///
/// Only E12 (LateInteraction/ColBERT) produces per-token embeddings.
#[inline]
pub fn is_token_level(embedder: Embedder) -> bool {
    get_config(embedder).is_token_level
}

/// Get the category for an embedder.
///
/// This delegates to category_for() from the category module.
#[inline]
pub fn get_category(embedder: Embedder) -> EmbedderCategory {
    category_for(embedder)
}

/// Get the topic weight for an embedder.
///
/// Derived from category: Semantic=1.0, Temporal=0.0, Relational=0.5, Structural=0.5
#[inline]
pub fn get_topic_weight(embedder: Embedder) -> f32 {
    get_category(embedder).topic_weight()
}

/// Check if embedder is in the Semantic category.
///
/// Semantic embedders (E1, E5, E6, E7, E10, E12, E13) have topic_weight 1.0.
#[inline]
pub fn is_semantic(embedder: Embedder) -> bool {
    get_category(embedder).is_semantic()
}

/// Check if embedder is in the Temporal category.
///
/// Temporal embedders (E2, E3, E4) have topic_weight 0.0 (excluded from topic detection).
#[inline]
pub fn is_temporal(embedder: Embedder) -> bool {
    get_category(embedder).is_temporal()
}

/// Check if embedder is in the Relational category.
///
/// Relational embedders (E8, E11) have topic_weight 0.5.
#[inline]
pub fn is_relational(embedder: Embedder) -> bool {
    get_category(embedder).is_relational()
}

/// Check if embedder is in the Structural category.
///
/// Structural embedder (E9) has topic_weight 0.5.
#[inline]
pub fn is_structural(embedder: Embedder) -> bool {
    get_category(embedder).is_structural()
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_embedders_configured() {
        for embedder in Embedder::all() {
            let config = get_config(embedder);
            assert_eq!(config.embedder, embedder);
            assert!(config.dimension > 0);
        }
        assert_eq!(Embedder::all().count(), 13);
        println!("[PASS] All 13 embedders have valid configurations");
    }

    #[test]
    fn test_config_array_index_matches_embedder_index() {
        for (i, config) in EMBEDDER_CONFIGS.iter().enumerate() {
            assert_eq!(
                config.embedder.index(),
                i,
                "Config at index {} has embedder {:?} with index {}",
                i,
                config.embedder,
                config.embedder.index()
            );
        }
        println!("[PASS] Config array indices match embedder indices");
    }

    #[test]
    fn test_e1_semantic_config() {
        let config = get_config(Embedder::Semantic);
        assert_eq!(config.dimension, 1024);
        assert_eq!(config.distance_metric, DistanceMetric::Cosine);
        assert!(!config.is_asymmetric);
        assert!(!config.is_sparse);
        assert!(!config.is_token_level);
        println!("[PASS] E1 Semantic config: 1024D, Cosine, dense");
    }

    #[test]
    fn test_e5_causal_asymmetric() {
        let config = get_config(Embedder::Causal);
        assert_eq!(config.dimension, 768);
        assert_eq!(config.distance_metric, DistanceMetric::AsymmetricCosine);
        assert!(config.is_asymmetric);
        assert!(is_asymmetric(Embedder::Causal));
        assert!(!is_asymmetric(Embedder::Semantic));
        println!("[PASS] E5 Causal: asymmetric, AsymmetricCosine metric");
    }

    #[test]
    fn test_sparse_embedders() {
        // E6 and E13 are sparse
        assert!(is_sparse(Embedder::Sparse));
        assert!(is_sparse(Embedder::KeywordSplade));

        // All others are dense
        assert!(!is_sparse(Embedder::Semantic));
        assert!(!is_sparse(Embedder::Causal));
        assert!(!is_sparse(Embedder::Hdc));
        assert!(!is_sparse(Embedder::LateInteraction));

        // Check dimensions
        assert_eq!(get_dimension(Embedder::Sparse), 30_522);
        assert_eq!(get_dimension(Embedder::KeywordSplade), 30_522);

        // Check Jaccard metric
        assert_eq!(
            get_distance_metric(Embedder::Sparse),
            DistanceMetric::Jaccard
        );
        assert_eq!(
            get_distance_metric(Embedder::KeywordSplade),
            DistanceMetric::Jaccard
        );

        println!("[PASS] E6/E13 sparse: 30522 vocab, Jaccard metric");
    }

    #[test]
    fn test_e9_hdc_is_cosine_not_hamming() {
        // CRITICAL: E9 is stored as projected dense, not binary
        let config = get_config(Embedder::Hdc);
        assert_eq!(config.distance_metric, DistanceMetric::Cosine);
        assert!(!config.is_sparse);
        assert_eq!(config.dimension, 1024);
        println!("[PASS] E9 HDC uses Cosine (projected dense), NOT Hamming");
    }

    #[test]
    fn test_e12_late_interaction() {
        let config = get_config(Embedder::LateInteraction);
        assert_eq!(config.dimension, 128); // Per token
        assert_eq!(config.distance_metric, DistanceMetric::MaxSim);
        assert!(config.is_token_level);
        assert!(is_token_level(Embedder::LateInteraction));
        assert!(!is_token_level(Embedder::Semantic));
        println!("[PASS] E12 LateInteraction: 128D per token, MaxSim");
    }

    #[test]
    fn test_category_assignments() {
        // Semantic embedders (7)
        assert_eq!(get_category(Embedder::Semantic), EmbedderCategory::Semantic);
        assert_eq!(get_category(Embedder::Causal), EmbedderCategory::Semantic);
        assert_eq!(get_category(Embedder::Sparse), EmbedderCategory::Semantic);
        assert_eq!(get_category(Embedder::Code), EmbedderCategory::Semantic);
        assert_eq!(
            get_category(Embedder::Multimodal),
            EmbedderCategory::Semantic
        );
        assert_eq!(
            get_category(Embedder::LateInteraction),
            EmbedderCategory::Semantic
        );
        assert_eq!(
            get_category(Embedder::KeywordSplade),
            EmbedderCategory::Semantic
        );

        // Temporal embedders (3)
        assert_eq!(
            get_category(Embedder::TemporalRecent),
            EmbedderCategory::Temporal
        );
        assert_eq!(
            get_category(Embedder::TemporalPeriodic),
            EmbedderCategory::Temporal
        );
        assert_eq!(
            get_category(Embedder::TemporalPositional),
            EmbedderCategory::Temporal
        );

        // Relational embedders (2)
        assert_eq!(
            get_category(Embedder::Emotional),
            EmbedderCategory::Relational
        );
        assert_eq!(get_category(Embedder::Entity), EmbedderCategory::Relational);

        // Structural embedders (1)
        assert_eq!(get_category(Embedder::Hdc), EmbedderCategory::Structural);

        println!("[PASS] All 13 embedders have correct category assignments");
    }

    #[test]
    fn test_topic_weights() {
        // Semantic = 1.0
        assert_eq!(get_topic_weight(Embedder::Semantic), 1.0);
        assert_eq!(get_topic_weight(Embedder::Code), 1.0);
        assert_eq!(get_topic_weight(Embedder::KeywordSplade), 1.0);

        // Temporal = 0.0
        assert_eq!(get_topic_weight(Embedder::TemporalRecent), 0.0);
        assert_eq!(get_topic_weight(Embedder::TemporalPeriodic), 0.0);
        assert_eq!(get_topic_weight(Embedder::TemporalPositional), 0.0);

        // Relational = 0.5
        assert_eq!(get_topic_weight(Embedder::Emotional), 0.5);
        assert_eq!(get_topic_weight(Embedder::Entity), 0.5);

        // Structural = 0.5
        assert_eq!(get_topic_weight(Embedder::Hdc), 0.5);

        println!("[PASS] Topic weights match category definitions");
    }

    #[test]
    fn test_is_semantic_count() {
        let semantic_count = Embedder::all().filter(|e| is_semantic(*e)).count();
        assert_eq!(semantic_count, 7, "Should have exactly 7 semantic embedders");

        assert!(is_semantic(Embedder::Semantic));
        assert!(is_semantic(Embedder::Causal));
        assert!(is_semantic(Embedder::Sparse));
        assert!(is_semantic(Embedder::Code));
        assert!(is_semantic(Embedder::Multimodal));
        assert!(is_semantic(Embedder::LateInteraction));
        assert!(is_semantic(Embedder::KeywordSplade));

        assert!(!is_semantic(Embedder::TemporalRecent));
        assert!(!is_semantic(Embedder::Emotional));
        assert!(!is_semantic(Embedder::Hdc));

        println!("[PASS] is_semantic() returns true for exactly 7 embedders");
    }

    #[test]
    fn test_is_temporal_count() {
        let temporal_count = Embedder::all().filter(|e| is_temporal(*e)).count();
        assert_eq!(temporal_count, 3, "Should have exactly 3 temporal embedders");

        assert!(is_temporal(Embedder::TemporalRecent));
        assert!(is_temporal(Embedder::TemporalPeriodic));
        assert!(is_temporal(Embedder::TemporalPositional));

        assert!(!is_temporal(Embedder::Semantic));
        assert!(!is_temporal(Embedder::Emotional));
        assert!(!is_temporal(Embedder::Hdc));

        println!("[PASS] is_temporal() returns true for exactly 3 embedders");
    }

    #[test]
    fn test_dimensions_match_constants() {
        use crate::index::config::*;

        assert_eq!(get_dimension(Embedder::Semantic), E1_DIM);
        assert_eq!(get_dimension(Embedder::TemporalRecent), E2_DIM);
        assert_eq!(get_dimension(Embedder::TemporalPeriodic), E3_DIM);
        assert_eq!(get_dimension(Embedder::TemporalPositional), E4_DIM);
        assert_eq!(get_dimension(Embedder::Causal), E5_DIM);
        assert_eq!(get_dimension(Embedder::Sparse), E6_SPARSE_VOCAB);
        assert_eq!(get_dimension(Embedder::Code), E7_DIM);
        assert_eq!(get_dimension(Embedder::Emotional), E8_DIM);
        assert_eq!(get_dimension(Embedder::Hdc), E9_DIM);
        assert_eq!(get_dimension(Embedder::Multimodal), E10_DIM);
        assert_eq!(get_dimension(Embedder::Entity), E11_DIM);
        assert_eq!(get_dimension(Embedder::LateInteraction), E12_TOKEN_DIM);
        assert_eq!(get_dimension(Embedder::KeywordSplade), E13_SPLADE_VOCAB);

        println!("[PASS] All dimensions match semantic/constants.rs");
    }

    #[test]
    fn test_quantization_configs() {
        // PQ8 for large dense embeddings
        assert!(matches!(
            get_quantization(Embedder::Semantic),
            QuantizationConfig::PQ8 {
                num_subvectors: 32,
                bits_per_code: 8
            }
        ));
        assert!(matches!(
            get_quantization(Embedder::Code),
            QuantizationConfig::PQ8 {
                num_subvectors: 48,
                bits_per_code: 8
            }
        ));

        // Float8 for smaller embeddings
        assert_eq!(
            get_quantization(Embedder::TemporalRecent),
            QuantizationConfig::Float8
        );
        assert_eq!(
            get_quantization(Embedder::Emotional),
            QuantizationConfig::Float8
        );

        // Inverted for sparse
        assert_eq!(
            get_quantization(Embedder::Sparse),
            QuantizationConfig::Inverted
        );
        assert_eq!(
            get_quantization(Embedder::KeywordSplade),
            QuantizationConfig::Inverted
        );

        println!("[PASS] Quantization configs are appropriate for each embedder");
    }
}
