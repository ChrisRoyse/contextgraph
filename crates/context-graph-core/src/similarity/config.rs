//! Configuration types for cross-space similarity computation.
//!
//! This module provides configuration structures for controlling how
//! similarity is computed across the 13 embedding spaces.

use crate::types::fingerprint::NUM_EMBEDDERS;
use serde::{Deserialize, Serialize};

/// Configuration for cross-space similarity computation.
///
/// # Default Configuration
///
/// The default configuration uses RRF (Reciprocal Rank Fusion) with k=60,
/// which is the primary aggregation strategy per constitution.yaml.
///
/// # Example
///
/// ```
/// use context_graph_core::similarity::{CrossSpaceConfig, WeightingStrategy};
///
/// // Default RRF configuration
/// let config = CrossSpaceConfig::default();
///
/// // Custom configuration with topic weighting
/// let custom = CrossSpaceConfig {
///     weighting_strategy: WeightingStrategy::TopicAligned,
///     include_breakdown: true,
///     ..Default::default()
/// };
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossSpaceConfig {
    /// Weighting strategy for combining spaces.
    /// Default: RRF with k=60 (per constitution.yaml).
    pub weighting_strategy: WeightingStrategy,

    /// Minimum embedding spaces required for valid similarity.
    /// If fewer spaces are active, returns `SimilarityError::InsufficientSpaces`.
    /// Default: 1
    pub min_active_spaces: usize,

    /// How to handle missing embeddings in a fingerprint.
    /// Default: Skip (reduce denominator)
    pub missing_space_handling: MissingSpaceHandling,

    /// Whether to include per-space breakdown in result.
    /// Enabling this adds overhead but provides detailed diagnostics.
    /// Default: false
    pub include_breakdown: bool,
}

impl Default for CrossSpaceConfig {
    fn default() -> Self {
        Self {
            weighting_strategy: WeightingStrategy::RRF { k: 60.0 },
            min_active_spaces: 1,
            missing_space_handling: MissingSpaceHandling::Skip,
            include_breakdown: false,
        }
    }
}

impl CrossSpaceConfig {
    /// Create a configuration optimized for RRF-based retrieval.
    ///
    /// This is the primary retrieval strategy per constitution.yaml.
    #[inline]
    pub fn rrf(k: f32) -> Self {
        Self {
            weighting_strategy: WeightingStrategy::RRF { k },
            ..Default::default()
        }
    }

    /// Create a configuration for topic-profile-aligned similarity.
    ///
    /// Uses topic profile strengths as weights for each embedding space.
    #[inline]
    pub fn topic_aligned() -> Self {
        Self {
            weighting_strategy: WeightingStrategy::TopicAligned,
            ..Default::default()
        }
    }

    /// Create a configuration with detailed breakdown enabled.
    #[inline]
    pub fn with_breakdown(mut self) -> Self {
        self.include_breakdown = true;
        self
    }

    /// Set minimum required active spaces.
    #[inline]
    pub fn with_min_spaces(mut self, min: usize) -> Self {
        self.min_active_spaces = min;
        self
    }
}

/// Strategy for weighting embedding spaces during aggregation.
///
/// # Primary Strategy: RRF
///
/// Per constitution.yaml, RRF (Reciprocal Rank Fusion) with k=60 is the
/// primary aggregation strategy:
///
/// ```text
/// RRF(d) = SUM_i 1/(k + rank_i(d) + 1) where k=60
/// ```
///
/// # Variants
///
/// - `Uniform`: Equal weight to all active spaces (1/13 each)
/// - `Static`: User-provided fixed weights (must sum to 1.0)
/// - `TopicAligned`: Weight by topic profile strength values
/// - `RRF`: Reciprocal Rank Fusion (primary strategy)
/// - `TopicWeightedRRF`: RRF with topic profile modulation
/// - `LateInteraction`: MaxSim for E12 ColBERT embeddings
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WeightingStrategy {
    /// Equal weight to all active spaces: 1/13 each.
    Uniform,

    /// Static weights per space.
    ///
    /// # Constraint
    /// Weights should sum to 1.0 for normalized scores.
    /// The implementation will normalize if they don't.
    Static([f32; NUM_EMBEDDERS]),

    /// Weight by topic profile strength values.
    ///
    /// Uses the topic profile's per-embedder strengths as weights.
    TopicAligned,

    /// Reciprocal Rank Fusion - PRIMARY STRATEGY.
    ///
    /// Formula: `RRF(d) = SUM_i 1/(k + rank_i(d) + 1)`
    ///
    /// # Parameters
    /// - `k`: Ranking constant (default: 60 per RRF literature)
    RRF {
        /// RRF constant k. Higher k reduces the impact of rank differences.
        /// Default: 60.0
        k: f32,
    },

    /// RRF with topic profile modulation.
    ///
    /// Formula: `RRF_weighted(d) = SUM_i (tau_i / (k + rank_i(d) + 1))`
    /// where `tau_i` is the topic alignment for space i.
    TopicWeightedRRF {
        /// RRF constant k.
        k: f32,
    },

    /// Late interaction scoring for E12 ColBERT embeddings.
    ///
    /// Uses MaxSim: `score = sum(max_j(q_i . d_j))` for query tokens i
    /// and document tokens j.
    LateInteraction,
}

impl Default for WeightingStrategy {
    fn default() -> Self {
        Self::RRF { k: 60.0 }
    }
}

impl WeightingStrategy {
    /// Check if this strategy requires rank-based input (vs. similarity scores).
    #[inline]
    pub fn requires_ranks(&self) -> bool {
        matches!(self, Self::RRF { .. } | Self::TopicWeightedRRF { .. })
    }

    /// Get uniform weights (1/NUM_EMBEDDERS for each space).
    #[inline]
    pub fn uniform_weights() -> [f32; NUM_EMBEDDERS] {
        [1.0 / NUM_EMBEDDERS as f32; NUM_EMBEDDERS]
    }

    /// Create static weights that emphasize semantic search.
    ///
    /// Delegates to canonical WEIGHT_PROFILES to avoid value divergence.
    pub fn semantic_search_weights() -> [f32; NUM_EMBEDDERS] {
        crate::weights::get_weight_profile("semantic_search")
            .expect("semantic_search profile must exist in WEIGHT_PROFILES")
    }

    /// Create static weights that emphasize code search.
    ///
    /// Delegates to canonical WEIGHT_PROFILES to avoid value divergence.
    pub fn code_search_weights() -> [f32; NUM_EMBEDDERS] {
        crate::weights::get_weight_profile("code_search")
            .expect("code_search profile must exist in WEIGHT_PROFILES")
    }

    /// Create static weights that emphasize causal reasoning.
    ///
    /// Delegates to canonical WEIGHT_PROFILES to avoid value divergence.
    pub fn causal_reasoning_weights() -> [f32; NUM_EMBEDDERS] {
        crate::weights::get_weight_profile("causal_reasoning")
            .expect("causal_reasoning profile must exist in WEIGHT_PROFILES")
    }

    /// Create static weights that emphasize graph/relational reasoning.
    ///
    /// Per E8 upgrade specification (Phase 5):
    /// Create static weights that emphasize graph/relational reasoning.
    ///
    /// Delegates to canonical WEIGHT_PROFILES to avoid value divergence.
    pub fn graph_reasoning_weights() -> [f32; NUM_EMBEDDERS] {
        crate::weights::get_weight_profile("graph_reasoning")
            .expect("graph_reasoning profile must exist in WEIGHT_PROFILES")
    }
}

/// How to handle missing embeddings in a fingerprint.
///
/// Some fingerprints may not have all 13 embedding spaces populated
/// (e.g., E7 Code only for code content, E10 Multimodal only for images).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MissingSpaceHandling {
    /// Skip missing spaces (reduce denominator).
    /// This is the safest default - doesn't assume anything about missing data.
    #[default]
    Skip,

    /// Treat missing as zero similarity.
    /// Use when missing embedding indicates dissimilarity.
    ZeroFill,

    /// Use average of present spaces for missing.
    /// Use when missing is due to data availability, not content mismatch.
    AverageFill,

    /// Error if any required space is missing.
    /// Use when all 13 spaces are mandatory for the use case.
    RequireAll,
}

impl MissingSpaceHandling {
    /// Check if this handling requires all spaces to be present.
    #[inline]
    pub fn requires_all(&self) -> bool {
        matches!(self, Self::RequireAll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_uses_rrf() {
        let config = CrossSpaceConfig::default();
        match config.weighting_strategy {
            WeightingStrategy::RRF { k } => {
                assert!((k - 60.0).abs() < f32::EPSILON);
                println!("[PASS] Default config uses RRF with k=60.0");
            }
            _ => panic!("Default should be RRF"),
        }
    }

    #[test]
    fn test_uniform_weights_sum_to_one() {
        let weights = WeightingStrategy::uniform_weights();
        let sum: f32 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "Uniform weights should sum to 1.0, got {}",
            sum
        );
        println!("[PASS] Uniform weights sum to {:.6} (expected 1.0)", sum);
    }

    #[test]
    fn test_semantic_search_weights_sum_to_one() {
        let weights = WeightingStrategy::semantic_search_weights();
        let sum: f32 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 0.01,
            "Semantic weights should sum close to 1.0, got {}",
            sum
        );
        assert!(
            weights[0] > weights[1],
            "E1 Semantic should have highest weight"
        );
        println!(
            "[PASS] Semantic search weights: E1={:.2}, E5={:.2}, E11={:.2}",
            weights[0], weights[4], weights[10]
        );
    }

    #[test]
    fn test_weighting_strategy_requires_ranks() {
        assert!(WeightingStrategy::RRF { k: 60.0 }.requires_ranks());
        assert!(WeightingStrategy::TopicWeightedRRF { k: 60.0 }.requires_ranks());
        assert!(!WeightingStrategy::Uniform.requires_ranks());
        assert!(!WeightingStrategy::TopicAligned.requires_ranks());
        assert!(!WeightingStrategy::LateInteraction.requires_ranks());
        println!("[PASS] requires_ranks correctly identifies RRF strategies");
    }

    #[test]
    fn test_missing_space_handling_requires_all() {
        assert!(!MissingSpaceHandling::Skip.requires_all());
        assert!(!MissingSpaceHandling::ZeroFill.requires_all());
        assert!(!MissingSpaceHandling::AverageFill.requires_all());
        assert!(MissingSpaceHandling::RequireAll.requires_all());
        println!("[PASS] requires_all correctly identifies RequireAll");
    }

    #[test]
    fn test_config_builder_methods() {
        let config = CrossSpaceConfig::rrf(30.0)
            .with_breakdown()
            .with_min_spaces(5);

        assert!(config.include_breakdown);
        assert_eq!(config.min_active_spaces, 5);
        match config.weighting_strategy {
            WeightingStrategy::RRF { k } => assert!((k - 30.0).abs() < f32::EPSILON),
            _ => panic!("Should be RRF"),
        }
        println!("[PASS] Config builder methods work correctly");
    }

    #[test]
    fn test_graph_reasoning_weights_sum_to_one() {
        let weights = WeightingStrategy::graph_reasoning_weights();
        let sum: f32 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 0.01,
            "Graph reasoning weights should sum close to 1.0, got {}",
            sum
        );
        // Canonical values from WEIGHT_PROFILES (weights/mod.rs)
        assert!(
            weights[7] > weights[0],
            "E8 Graph should have highest weight in graph_reasoning"
        );
        assert_eq!(weights[7], 0.40, "E8 Graph should be 0.40");
        assert_eq!(weights[0], 0.15, "E1 Semantic should be 0.15");
        assert_eq!(weights[10], 0.20, "E11 Entity should be 0.20");
        println!(
            "[PASS] Graph reasoning weights: E8={:.2}, E1={:.2}, E11={:.2}",
            weights[7], weights[0], weights[10]
        );
    }

    #[test]
    fn test_code_search_weights_include_e8() {
        let weights = WeightingStrategy::code_search_weights();
        // Canonical: E8=0.0 in code_search (WEIGHT_PROFILES), E7=0.40 primary
        assert_eq!(weights[7], 0.0, "E8 Graph should be 0.0 in code_search");
        assert!(
            weights[6] > weights[7],
            "E7 Code should be higher than E8 in code_search"
        );
        assert_eq!(weights[6], 0.40, "E7 Code should be 0.40 in code_search");
        println!(
            "[PASS] Code search weights: E7={:.2}, E1={:.2}, E8={:.2}",
            weights[6], weights[0], weights[7]
        );
    }
}
