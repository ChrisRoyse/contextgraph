//! Ablation Study Framework for Multi-Space Embeddings
//!
//! Measures each embedder's contribution by comparing:
//! - Score(all 13 embedders) vs Score(all except Ei)
//!
//! This shows how much each embedder uniquely contributes to overall performance.
//!
//! # Key Insight
//!
//! Ablation delta = Score(all) - Score(without Ei)
//!
//! A high ablation delta means removing Ei significantly hurts performance,
//! indicating Ei provides unique information not captured by other embedders.
//!
//! # Example
//!
//! ```ignore
//! let config = AblationConfig::default();
//! let report = run_full_ablation(config).await?;
//!
//! // E7 Code has delta 0.15 -> removing it drops score by 15%
//! // E9 HDC has delta 0.02 -> removing it barely affects score
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use context_graph_storage::teleological::indexes::EmbedderIndex;

/// Configuration for ablation study.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AblationConfig {
    /// Number of queries to run for each ablation.
    pub num_queries: usize,
    /// Embedders to include in the ablation study.
    pub embedders: Vec<EmbedderIndex>,
    /// Minimum similarity threshold for retrieval.
    pub min_similarity: f32,
    /// Number of results to retrieve per query.
    pub top_k: usize,
}

impl Default for AblationConfig {
    fn default() -> Self {
        Self {
            num_queries: 100,
            embedders: vec![
                EmbedderIndex::E1Semantic,
                EmbedderIndex::E2TemporalRecent,
                EmbedderIndex::E3TemporalPeriodic,
                EmbedderIndex::E4TemporalPositional,
                EmbedderIndex::E5Causal,
                EmbedderIndex::E6Sparse,
                EmbedderIndex::E7Code,
                EmbedderIndex::E8Graph,
                EmbedderIndex::E9HDC,
                EmbedderIndex::E10Multimodal,
                EmbedderIndex::E11Entity,
                EmbedderIndex::E12LateInteraction,
                EmbedderIndex::E13Splade,
            ],
            min_similarity: 0.0,
            top_k: 10,
        }
    }
}

/// Report from a full ablation study.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AblationReport {
    /// Baseline score with all 13 embedders.
    pub baseline_score: f32,
    /// Per-embedder ablation deltas.
    pub deltas: HashMap<String, AblationDelta>,
    /// Total queries run.
    pub total_queries: usize,
    /// Corpus size.
    pub corpus_size: usize,
}

/// Ablation delta for a single embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AblationDelta {
    /// Embedder index.
    pub embedder: String,
    /// Score without this embedder.
    pub score_without: f32,
    /// Delta = baseline - score_without.
    pub delta: f32,
    /// Percentage impact = delta / baseline * 100.
    pub impact_percent: f32,
    /// Whether this embedder is essential (delta > threshold).
    pub is_essential: bool,
}

impl AblationReport {
    /// Create a new ablation report.
    pub fn new(baseline_score: f32, corpus_size: usize, total_queries: usize) -> Self {
        Self {
            baseline_score,
            deltas: HashMap::new(),
            total_queries,
            corpus_size,
        }
    }

    /// Add an ablation delta for an embedder.
    pub fn add_delta(&mut self, embedder: EmbedderIndex, score_without: f32) {
        let delta = self.baseline_score - score_without;
        let impact_percent = if self.baseline_score > 0.0 {
            (delta / self.baseline_score) * 100.0
        } else {
            0.0
        };

        // Essential if removing it drops score by more than 5%
        let is_essential = impact_percent > 5.0;

        let embedder_name = embedder_to_name(embedder);
        self.deltas.insert(
            embedder_name.to_string(),
            AblationDelta {
                embedder: embedder_name.to_string(),
                score_without,
                delta,
                impact_percent,
                is_essential,
            },
        );
    }

    /// Get sorted deltas by impact (highest first).
    pub fn sorted_by_impact(&self) -> Vec<&AblationDelta> {
        let mut deltas: Vec<_> = self.deltas.values().collect();
        deltas.sort_by(|a, b| {
            b.delta
                .partial_cmp(&a.delta)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        deltas
    }

    /// Get essential embedders (impact > 5%).
    pub fn essential_embedders(&self) -> Vec<&str> {
        self.deltas
            .values()
            .filter(|d| d.is_essential)
            .map(|d| d.embedder.as_str())
            .collect()
    }

    /// Get non-essential embedders (impact <= 5%).
    pub fn non_essential_embedders(&self) -> Vec<&str> {
        self.deltas
            .values()
            .filter(|d| !d.is_essential)
            .map(|d| d.embedder.as_str())
            .collect()
    }
}

/// Weight profile that zeros out a specific embedder.
///
/// Used to measure ablation delta by running search without one embedder.
pub fn ablation_weights(exclude: EmbedderIndex, base_weights: &[f32; 14]) -> [f32; 14] {
    let mut weights = *base_weights;

    // Zero out the excluded embedder
    if let Some(idx) = exclude.to_index() {
        weights[idx] = 0.0;
    }

    // Renormalize remaining weights
    let sum: f32 = weights.iter().sum();
    if sum > 0.0 {
        for w in weights.iter_mut() {
            *w /= sum;
        }
    }

    weights
}

/// Get weight profile that only includes a specific embedder.
///
/// Used to measure single-embedder performance in isolation.
pub fn single_embedder_weights(include: EmbedderIndex) -> [f32; 14] {
    let mut weights = [0.0f32; 14];
    if let Some(idx) = include.to_index() {
        weights[idx] = 1.0;
    }
    weights
}

/// Convert embedder index to human-readable name.
fn embedder_to_name(embedder: EmbedderIndex) -> &'static str {
    match embedder {
        EmbedderIndex::E1Semantic => "E1_Semantic",
        EmbedderIndex::E1Matryoshka128 => "E1_Matryoshka128",
        EmbedderIndex::E2TemporalRecent => "E2_Temporal_Recent",
        EmbedderIndex::E3TemporalPeriodic => "E3_Temporal_Periodic",
        EmbedderIndex::E4TemporalPositional => "E4_Temporal_Positional",
        EmbedderIndex::E5Causal => "E5_Causal",
        EmbedderIndex::E5CausalCause => "E5_Causal_Cause",
        EmbedderIndex::E5CausalEffect => "E5_Causal_Effect",
        EmbedderIndex::E6Sparse => "E6_Sparse",
        EmbedderIndex::E7Code => "E7_Code",
        EmbedderIndex::E8Graph => "E8_Graph",
        EmbedderIndex::E9HDC => "E9_HDC",
        EmbedderIndex::E10Multimodal => "E10_Multimodal",
        EmbedderIndex::E10MultimodalParaphrase => "E10_Multimodal_Paraphrase",
        EmbedderIndex::E10MultimodalContext => "E10_Multimodal_Context",
        EmbedderIndex::E11Entity => "E11_Entity",
        EmbedderIndex::E12LateInteraction => "E12_Late_Interaction",
        EmbedderIndex::E13Splade => "E13_SPLADE",
    }
}

/// All 13 core embedders (excluding Matryoshka variant).
pub fn all_core_embedders() -> Vec<EmbedderIndex> {
    vec![
        EmbedderIndex::E1Semantic,
        EmbedderIndex::E2TemporalRecent,
        EmbedderIndex::E3TemporalPeriodic,
        EmbedderIndex::E4TemporalPositional,
        EmbedderIndex::E5Causal,
        EmbedderIndex::E6Sparse,
        EmbedderIndex::E7Code,
        EmbedderIndex::E8Graph,
        EmbedderIndex::E9HDC,
        EmbedderIndex::E10Multimodal,
        EmbedderIndex::E11Entity,
        EmbedderIndex::E12LateInteraction,
        EmbedderIndex::E13Splade,
    ]
}

/// Semantic embedders only (for stress testing).
pub fn semantic_embedders() -> Vec<EmbedderIndex> {
    vec![
        EmbedderIndex::E1Semantic,
        EmbedderIndex::E5Causal,
        EmbedderIndex::E6Sparse,
        EmbedderIndex::E7Code,
        EmbedderIndex::E10Multimodal,
        EmbedderIndex::E12LateInteraction,
        EmbedderIndex::E13Splade,
    ]
}

/// Temporal embedders (for post-retrieval boost testing).
pub fn temporal_embedders() -> Vec<EmbedderIndex> {
    vec![
        EmbedderIndex::E2TemporalRecent,
        EmbedderIndex::E3TemporalPeriodic,
        EmbedderIndex::E4TemporalPositional,
    ]
}

/// Relational embedders.
pub fn relational_embedders() -> Vec<EmbedderIndex> {
    vec![EmbedderIndex::E8Graph, EmbedderIndex::E11Entity]
}

/// Structural embedders.
pub fn structural_embedders() -> Vec<EmbedderIndex> {
    vec![EmbedderIndex::E9HDC]
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_ablation_weights_zeroes_target() {
        let base = [1.0 / 13.0; 14];
        let weights = ablation_weights(EmbedderIndex::E5Causal, &base);

        // E5 is index 4
        assert_eq!(weights[4], 0.0, "E5 should be zeroed");

        // Sum should still be 1.0 (renormalized)
        let sum: f32 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.01, "Weights should sum to 1.0");
    }

    #[test]
    fn test_single_embedder_weights() {
        let weights = single_embedder_weights(EmbedderIndex::E7Code);

        // Only E7 (index 6) should be 1.0
        assert_eq!(weights[6], 1.0, "E7 should be 1.0");

        // All others should be 0.0
        for (i, &w) in weights.iter().enumerate() {
            if i != 6 {
                assert_eq!(w, 0.0, "Index {} should be 0.0", i);
            }
        }
    }

    #[test]
    fn test_ablation_report_sorted() {
        let mut report = AblationReport::new(0.8, 100, 50);
        report.add_delta(EmbedderIndex::E1Semantic, 0.7); // delta = 0.1
        report.add_delta(EmbedderIndex::E5Causal, 0.75); // delta = 0.05
        report.add_delta(EmbedderIndex::E7Code, 0.6); // delta = 0.2

        let sorted = report.sorted_by_impact();
        assert_eq!(sorted[0].embedder, "E7_Code"); // highest delta
        assert_eq!(sorted[1].embedder, "E1_Semantic");
        assert_eq!(sorted[2].embedder, "E5_Causal"); // lowest delta
    }

    #[test]
    fn test_ablation_report_essential() {
        let mut report = AblationReport::new(1.0, 100, 50);
        report.add_delta(EmbedderIndex::E7Code, 0.9); // delta = 0.1 = 10% -> essential
        report.add_delta(EmbedderIndex::E9HDC, 0.98); // delta = 0.02 = 2% -> not essential

        let essential = report.essential_embedders();
        let non_essential = report.non_essential_embedders();

        assert!(essential.contains(&"E7_Code"));
        assert!(!essential.contains(&"E9_HDC"));
        assert!(non_essential.contains(&"E9_HDC"));
    }

    #[test]
    fn test_embedder_category_counts() {
        assert_eq!(all_core_embedders().len(), 13);
        assert_eq!(semantic_embedders().len(), 7);
        assert_eq!(temporal_embedders().len(), 3);
        assert_eq!(relational_embedders().len(), 2);
        assert_eq!(structural_embedders().len(), 1);
    }
}
