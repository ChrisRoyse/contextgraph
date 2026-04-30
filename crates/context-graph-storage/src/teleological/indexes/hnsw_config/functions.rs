//! Config accessor functions for embedder-specific configurations.

use std::collections::HashMap;

use super::config::{HnswConfig, InvertedIndexConfig};
use super::constants::*;
use super::distance::DistanceMetric;
use super::embedder::EmbedderIndex;

/// Get HNSW config for index type. Returns None for non-HNSW indexes.
///
/// # Returns
///
/// - `Some(HnswConfig)` for HNSW-compatible embedders (13 total)
/// - `None` for E6Sparse, E12LateInteraction, E13Splade
///
/// # Example
///
/// ```
/// use context_graph_storage::teleological::indexes::{get_hnsw_config, EmbedderIndex};
///
/// let config = get_hnsw_config(EmbedderIndex::E1Semantic);
/// assert!(config.is_some());
/// assert_eq!(config.unwrap().dimension, 1024);
///
/// let none = get_hnsw_config(EmbedderIndex::E6Sparse);
/// assert!(none.is_none());
/// ```
pub fn get_hnsw_config(index: EmbedderIndex) -> Option<HnswConfig> {
    match index {
        // Dense embedders with standard config
        EmbedderIndex::E1Semantic => Some(HnswConfig::default_for_dimension(
            E1_DIM,
            DistanceMetric::Cosine,
        )),
        EmbedderIndex::E2TemporalRecent => Some(HnswConfig::default_for_dimension(
            E2_DIM,
            DistanceMetric::Cosine,
        )),
        EmbedderIndex::E3TemporalPeriodic => Some(HnswConfig::default_for_dimension(
            E3_DIM,
            DistanceMetric::Cosine,
        )),
        EmbedderIndex::E4TemporalPositional => Some(HnswConfig::default_for_dimension(
            E4_DIM,
            DistanceMetric::Cosine,
        )),
        // E5 legacy (active vector) - kept for backward compatibility
        EmbedderIndex::E5Causal => Some(HnswConfig::default_for_dimension(
            E5_DIM,
            DistanceMetric::AsymmetricCosine,
        )),
        // E5 asymmetric indexes for direction-aware retrieval (ARCH-15, AP-77)
        EmbedderIndex::E5CausalCause => Some(HnswConfig::default_for_dimension(
            E5_DIM,
            DistanceMetric::AsymmetricCosine,
        )),
        EmbedderIndex::E5CausalEffect => Some(HnswConfig::default_for_dimension(
            E5_DIM,
            DistanceMetric::AsymmetricCosine,
        )),
        EmbedderIndex::E7Code => Some(HnswConfig::default_for_dimension(
            E7_DIM,
            DistanceMetric::Cosine,
        )),
        EmbedderIndex::E8Graph => Some(HnswConfig::default_for_dimension(
            E8_DIM,
            DistanceMetric::Cosine,
        )),
        EmbedderIndex::E9HDC => Some(HnswConfig::default_for_dimension(
            E9_DIM,
            DistanceMetric::Cosine,
        )),
        // E10 legacy (active vector) - kept for backward compatibility
        EmbedderIndex::E10Multimodal => Some(HnswConfig::default_for_dimension(
            E10_DIM,
            DistanceMetric::AsymmetricCosine,
        )),
        // E10 asymmetric indexes for direction-aware retrieval (ARCH-15, AP-77)
        EmbedderIndex::E10MultimodalParaphrase => Some(HnswConfig::default_for_dimension(
            E10_DIM,
            DistanceMetric::AsymmetricCosine,
        )),
        EmbedderIndex::E10MultimodalContext => Some(HnswConfig::default_for_dimension(
            E10_DIM,
            DistanceMetric::AsymmetricCosine,
        )),
        EmbedderIndex::E11Entity => Some(HnswConfig::default_for_dimension(
            E11_DIM,
            DistanceMetric::Cosine,
        )),
        // E14: BGE-M3 dense (1024D, cosine, HNSW)
        EmbedderIndex::E14BgeM3Dense => Some(HnswConfig::default_for_dimension(
            1024,
            DistanceMetric::Cosine,
        )),

        // Special configs
        EmbedderIndex::E1Matryoshka128 => Some(HnswConfig::matryoshka_128d()),

        // NOT HNSW
        EmbedderIndex::E6Sparse => None,
        EmbedderIndex::E12LateInteraction => None,
        EmbedderIndex::E13Splade => None,
    }
}

/// Get all HNSW configs as a map. Returns 15 entries.
///
/// Excludes E6Sparse, E12LateInteraction, E13Splade (non-HNSW).
/// Includes E5CausalCause and E5CausalEffect for asymmetric retrieval (ARCH-15).
/// Includes E10MultimodalParaphrase and E10MultimodalContext for asymmetric retrieval (ARCH-15).
///
/// # Example
///
/// ```
/// use context_graph_storage::teleological::indexes::{all_hnsw_configs, EmbedderIndex};
///
/// let configs = all_hnsw_configs();
/// assert_eq!(configs.len(), 16);
/// assert!(configs.contains_key(&EmbedderIndex::E1Semantic));
/// assert!(!configs.contains_key(&EmbedderIndex::E6Sparse));
/// ```
pub fn all_hnsw_configs() -> HashMap<EmbedderIndex, HnswConfig> {
    EmbedderIndex::all_hnsw()
        .into_iter()
        .filter_map(|idx| get_hnsw_config(idx).map(|cfg| (idx, cfg)))
        .collect()
}

/// Get inverted index config. Returns None for non-inverted indexes.
///
/// # Returns
///
/// - `Some(InvertedIndexConfig)` for E6Sparse and E13Splade
/// - `None` for all other embedders
pub fn get_inverted_index_config(index: EmbedderIndex) -> Option<InvertedIndexConfig> {
    match index {
        EmbedderIndex::E6Sparse => Some(InvertedIndexConfig::e6_sparse()),
        EmbedderIndex::E13Splade => Some(InvertedIndexConfig::e13_splade()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_hnsw_config_e1() {
        let config = get_hnsw_config(EmbedderIndex::E1Semantic);
        assert!(config.is_some());
        let cfg = config.unwrap();
        assert_eq!(cfg.dimension, 1024);
        assert_eq!(cfg.m, 16);
        assert_eq!(cfg.ef_construction, 200);
        assert_eq!(cfg.ef_search, 100);
        assert_eq!(cfg.metric, DistanceMetric::Cosine);
    }

    #[test]
    fn test_get_hnsw_config_matryoshka() {
        let config = get_hnsw_config(EmbedderIndex::E1Matryoshka128);
        assert!(config.is_some());
        let cfg = config.unwrap();
        assert_eq!(cfg.dimension, 128);
        assert_eq!(cfg.m, 32);
        assert_eq!(cfg.ef_construction, 256);
        assert_eq!(cfg.ef_search, 128);
    }

    #[test]
    fn test_get_hnsw_config_e5_causal() {
        // E5 legacy (active vector)
        let config = get_hnsw_config(EmbedderIndex::E5Causal).unwrap();
        assert_eq!(config.metric, DistanceMetric::AsymmetricCosine);
        assert_eq!(config.dimension, 768);
    }

    #[test]
    fn test_get_hnsw_config_e5_asymmetric() {
        // E5 asymmetric indexes (ARCH-15, AP-77)
        let cause_config = get_hnsw_config(EmbedderIndex::E5CausalCause).unwrap();
        assert_eq!(cause_config.metric, DistanceMetric::AsymmetricCosine);
        assert_eq!(cause_config.dimension, 768);

        let effect_config = get_hnsw_config(EmbedderIndex::E5CausalEffect).unwrap();
        assert_eq!(effect_config.metric, DistanceMetric::AsymmetricCosine);
        assert_eq!(effect_config.dimension, 768);
    }

    #[test]
    fn test_get_hnsw_config_non_hnsw() {
        assert!(get_hnsw_config(EmbedderIndex::E6Sparse).is_none());
        assert!(get_hnsw_config(EmbedderIndex::E12LateInteraction).is_none());
        assert!(get_hnsw_config(EmbedderIndex::E13Splade).is_none());
    }

    #[test]
    fn test_all_hnsw_configs_returns_16() {
        // 11 original + 2 E5 asymmetric + 2 E10 asymmetric + 1 E14 BGE-M3 Dense = 16 HNSW configs
        let configs = all_hnsw_configs();
        assert_eq!(configs.len(), 16);

        assert!(configs.contains_key(&EmbedderIndex::E1Semantic));
        assert!(configs.contains_key(&EmbedderIndex::E1Matryoshka128));
        // E5 asymmetric indexes included
        assert!(configs.contains_key(&EmbedderIndex::E5CausalCause));
        assert!(configs.contains_key(&EmbedderIndex::E5CausalEffect));
        // E10 asymmetric indexes included
        assert!(configs.contains_key(&EmbedderIndex::E10MultimodalParaphrase));
        assert!(configs.contains_key(&EmbedderIndex::E10MultimodalContext));
        // E14 BGE-M3 Dense is a first-class HNSW slot post-E14.
        assert!(configs.contains_key(&EmbedderIndex::E14BgeM3Dense));

        assert!(!configs.contains_key(&EmbedderIndex::E6Sparse));
        assert!(!configs.contains_key(&EmbedderIndex::E12LateInteraction));
        assert!(!configs.contains_key(&EmbedderIndex::E13Splade));
    }

    #[test]
    fn test_get_hnsw_config_e10_multimodal() {
        // E10 legacy (active vector)
        let config = get_hnsw_config(EmbedderIndex::E10Multimodal).unwrap();
        assert_eq!(config.metric, DistanceMetric::AsymmetricCosine);
        assert_eq!(config.dimension, 768);
    }

    #[test]
    fn test_get_hnsw_config_e10_asymmetric() {
        // E10 asymmetric indexes (ARCH-15, AP-77)
        let paraphrase_config = get_hnsw_config(EmbedderIndex::E10MultimodalParaphrase).unwrap();
        assert_eq!(paraphrase_config.metric, DistanceMetric::AsymmetricCosine);
        assert_eq!(paraphrase_config.dimension, 768);

        let context_config = get_hnsw_config(EmbedderIndex::E10MultimodalContext).unwrap();
        assert_eq!(context_config.metric, DistanceMetric::AsymmetricCosine);
        assert_eq!(context_config.dimension, 768);
    }

    #[test]
    fn test_get_inverted_index_config() {
        let e6 = get_inverted_index_config(EmbedderIndex::E6Sparse);
        assert!(e6.is_some());
        assert!(!e6.unwrap().use_bm25);

        let e13 = get_inverted_index_config(EmbedderIndex::E13Splade);
        assert!(e13.is_some());
        assert!(e13.unwrap().use_bm25);

        assert!(get_inverted_index_config(EmbedderIndex::E1Semantic).is_none());
    }

    #[test]
    fn test_all_configs_have_valid_dimensions() {
        let configs = all_hnsw_configs();
        for (idx, cfg) in &configs {
            assert!(cfg.dimension >= 1, "{:?}: dimension must be >= 1", idx);
            assert!(cfg.m >= 2, "{:?}: M must be >= 2", idx);
            assert!(
                cfg.ef_construction >= cfg.m,
                "{:?}: ef_construction must be >= M",
                idx
            );
            assert!(cfg.ef_search >= 1, "{:?}: ef_search must be >= 1", idx);
        }
    }
}
