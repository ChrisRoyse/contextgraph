//! Benchmark configuration: tier definitions, thresholds, and performance targets.
//!
//! # Tier System
//!
//! The benchmark system uses 6 tiers to test scaling behavior from small to enterprise scale.
//! Each tier doubles or 10x's the corpus size to identify breaking points.

use serde::{Deserialize, Serialize};

/// Tier identifiers for scaling analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Tier {
    /// 100 memories, 5 topics - baseline calibration
    Tier0 = 0,
    /// 1,000 memories, 20 topics - small project
    Tier1 = 1,
    /// 10,000 memories, 50 topics - medium project
    Tier2 = 2,
    /// 100,000 memories, 100 topics - large project
    Tier3 = 3,
    /// 1,000,000 memories, 200 topics - enterprise
    Tier4 = 4,
    /// 10,000,000 memories, 500 topics - theoretical limit
    Tier5 = 5,
}

impl Tier {
    /// Get all tiers in order.
    pub fn all() -> &'static [Tier] {
        &[
            Tier::Tier0,
            Tier::Tier1,
            Tier::Tier2,
            Tier::Tier3,
            Tier::Tier4,
            Tier::Tier5,
        ]
    }

    /// Get tiers suitable for CI (0-3).
    pub fn ci_tiers() -> &'static [Tier] {
        &[Tier::Tier0, Tier::Tier1, Tier::Tier2, Tier::Tier3]
    }

    /// Get the numeric value (0-5).
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tier{}", self.as_u8())
    }
}

/// Configuration for a specific tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    /// Tier identifier.
    pub tier: Tier,
    /// Number of memories/documents in the corpus.
    pub memory_count: usize,
    /// Number of distinct topics to generate.
    pub topic_count: usize,
    /// Minimum memories per topic.
    pub min_memories_per_topic: usize,
    /// Maximum memories per topic.
    pub max_memories_per_topic: usize,
    /// Number of queries to generate for evaluation.
    pub query_count: usize,
    /// Number of relevant documents per query (for ground truth).
    pub relevant_docs_per_query: usize,
    /// Target intra-topic similarity (cosine).
    pub intra_topic_similarity: f32,
    /// Target inter-topic similarity (cosine) - should be much lower.
    pub inter_topic_similarity: f32,
}

impl TierConfig {
    /// Create configuration for a specific tier.
    pub fn for_tier(tier: Tier) -> Self {
        match tier {
            Tier::Tier0 => Self {
                tier,
                memory_count: 100,
                topic_count: 5,
                min_memories_per_topic: 10,
                max_memories_per_topic: 30,
                query_count: 50,
                relevant_docs_per_query: 10,
                intra_topic_similarity: 0.85,
                inter_topic_similarity: 0.25,
            },
            Tier::Tier1 => Self {
                tier,
                memory_count: 1_000,
                topic_count: 20,
                min_memories_per_topic: 30,
                max_memories_per_topic: 80,
                query_count: 100,
                relevant_docs_per_query: 20,
                intra_topic_similarity: 0.82,
                inter_topic_similarity: 0.28,
            },
            Tier::Tier2 => Self {
                tier,
                memory_count: 10_000,
                topic_count: 50,
                min_memories_per_topic: 100,
                max_memories_per_topic: 300,
                query_count: 200,
                relevant_docs_per_query: 30,
                intra_topic_similarity: 0.80,
                inter_topic_similarity: 0.30,
            },
            Tier::Tier3 => Self {
                tier,
                memory_count: 100_000,
                topic_count: 100,
                min_memories_per_topic: 500,
                max_memories_per_topic: 1500,
                query_count: 500,
                relevant_docs_per_query: 50,
                intra_topic_similarity: 0.78,
                inter_topic_similarity: 0.32,
            },
            Tier::Tier4 => Self {
                tier,
                memory_count: 1_000_000,
                topic_count: 200,
                min_memories_per_topic: 2500,
                max_memories_per_topic: 7500,
                query_count: 1000,
                relevant_docs_per_query: 100,
                intra_topic_similarity: 0.75,
                inter_topic_similarity: 0.35,
            },
            Tier::Tier5 => Self {
                tier,
                memory_count: 10_000_000,
                topic_count: 500,
                min_memories_per_topic: 10000,
                max_memories_per_topic: 30000,
                query_count: 2000,
                relevant_docs_per_query: 200,
                intra_topic_similarity: 0.72,
                inter_topic_similarity: 0.38,
            },
        }
    }
}

/// Main benchmark configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Tiers to benchmark.
    pub tiers: Vec<TierConfig>,
    /// K values for P@K and R@K metrics.
    pub k_values: Vec<usize>,
    /// Random seed for reproducibility.
    pub seed: u64,
    /// Whether to run in CI mode (skip Tier4+).
    pub ci_mode: bool,
    /// Latency percentiles to track.
    pub latency_percentiles: Vec<f64>,
    /// Number of warmup iterations before measurement.
    pub warmup_iterations: usize,
    /// Number of measurement iterations.
    pub measurement_iterations: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            tiers: Tier::ci_tiers()
                .iter()
                .map(|&t| TierConfig::for_tier(t))
                .collect(),
            k_values: vec![1, 5, 10, 20, 50],
            seed: 42,
            ci_mode: true,
            latency_percentiles: vec![0.5, 0.95, 0.99],
            warmup_iterations: 5,
            measurement_iterations: 100,
        }
    }
}

impl BenchmarkConfig {
    /// Create config for full benchmark (all tiers).
    pub fn full() -> Self {
        Self {
            tiers: Tier::all()
                .iter()
                .map(|&t| TierConfig::for_tier(t))
                .collect(),
            ci_mode: false,
            ..Default::default()
        }
    }

    /// Create config for CI (Tiers 0-3 only).
    pub fn ci() -> Self {
        Self::default()
    }

    /// Create config for a single tier.
    pub fn single_tier(tier: Tier) -> Self {
        Self {
            tiers: vec![TierConfig::for_tier(tier)],
            ..Default::default()
        }
    }
}

/// Performance targets from constitution.yaml.
pub mod targets {
    /// Search latency target (milliseconds).
    pub const SEARCH_LATENCY_MS: u64 = 2000;

    /// Store latency target (milliseconds).
    pub const STORE_LATENCY_MS: u64 = 2500;

    /// Single embedding target (milliseconds).
    pub const EMBED_SINGLE_MS: u64 = 1000;

    /// FAISS 1M k=100 target (milliseconds).
    pub const FAISS_1M_K100_MS: u64 = 5;

    /// Minimum acceptable precision@10.
    pub const MIN_PRECISION_10: f64 = 0.70;

    /// Minimum acceptable recall@10.
    pub const MIN_RECALL_10: f64 = 0.60;

    /// Minimum acceptable MRR.
    pub const MIN_MRR: f64 = 0.50;

    /// Degradation threshold (80% of Tier 0 performance).
    pub const DEGRADATION_THRESHOLD: f64 = 0.80;
}

/// Embedder categories for weighted agreement.
pub mod embedder_categories {
    use std::collections::HashSet;

    /// Semantic embedders (weight 1.0) - primary topic triggers.
    pub fn semantic() -> HashSet<usize> {
        [0, 4, 5, 6, 9, 11, 12].into_iter().collect() // E1, E5, E6, E7, E10, E12, E13
    }

    /// Temporal embedders (weight 0.0) - excluded from topic detection.
    pub fn temporal() -> HashSet<usize> {
        [1, 2, 3].into_iter().collect() // E2, E3, E4
    }

    /// Relational embedders (weight 0.5) - supporting role.
    pub fn relational() -> HashSet<usize> {
        [7, 10].into_iter().collect() // E8, E11
    }

    /// Structural embedders (weight 0.5) - format patterns.
    pub fn structural() -> HashSet<usize> {
        [8].into_iter().collect() // E9
    }

    /// Get weight for an embedder index.
    pub fn weight(embedder_idx: usize) -> f64 {
        if semantic().contains(&embedder_idx) {
            1.0
        } else if temporal().contains(&embedder_idx) {
            0.0
        } else if relational().contains(&embedder_idx) || structural().contains(&embedder_idx) {
            0.5
        } else {
            0.0
        }
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_tier_configs() {
        for tier in Tier::all() {
            let config = TierConfig::for_tier(*tier);
            assert!(config.memory_count > 0);
            assert!(config.topic_count > 0);
            assert!(config.intra_topic_similarity > config.inter_topic_similarity);
        }
    }

    #[test]
    fn test_embedder_weights() {
        // E1 (semantic) should have weight 1.0
        assert_eq!(embedder_categories::weight(0), 1.0);
        // E2 (temporal) should have weight 0.0
        assert_eq!(embedder_categories::weight(1), 0.0);
        // E8 (relational) should have weight 0.5
        assert_eq!(embedder_categories::weight(7), 0.5);
    }
}
