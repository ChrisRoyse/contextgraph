//! E11 Entity Embedder Metrics.
//!
//! This module provides metrics for evaluating the E11 entity embedder
//! (KEPLER, RoBERTa-base + TransE on Wikidata5M, 768D) and its MCP tool integrations.
//!
//! ## KEPLER vs MiniLM (Old E11)
//!
//! KEPLER is trained with TransE objective on Wikidata5M (4.8M entities, 20M triples),
//! making TransE operations (h + r ≈ t) semantically meaningful. Unlike the previous
//! MiniLM model, relationship inference and knowledge validation actually work.
//!
//! ## Target Thresholds (KEPLER-based)
//!
//! | Metric | Target | Priority |
//! |--------|--------|----------|
//! | Entity Extraction F1 | >= 0.85 | Critical |
//! | Canonicalization Accuracy | >= 0.90 | High |
//! | TransE Valid Score | > -5.0 | Critical |
//! | TransE Invalid Score | < -10.0 | Critical |
//! | Score Separation | > 5.0 | Critical |
//! | Relation Inference MRR | >= 0.40 | High |
//! | Knowledge Validation Accuracy | >= 75% | High |
//! | E11 Contribution (hybrid) | >= +10% | Medium |
//! | Entity Search Latency | < 100ms | High |

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Target Thresholds (KEPLER-based, from plan)
// ============================================================================

/// Target thresholds for E11 benchmark metrics.
///
/// These thresholds are calibrated for KEPLER (RoBERTa-base + TransE on Wikidata5M).
/// KEPLER produces more meaningful TransE scores than the previous MiniLM model:
/// - Valid triples: typically score > -5.0
/// - Invalid triples: typically score < -10.0
/// - Score separation: typically > 5.0 (vs ~1.5 with MiniLM)
pub mod thresholds {
    /// Minimum entity extraction F1 score.
    pub const EXTRACTION_F1_MIN: f64 = 0.85;
    /// Minimum canonicalization accuracy.
    pub const CANONICALIZATION_ACCURACY_MIN: f64 = 0.90;
    /// Minimum TransE score for valid triples (KEPLER: typically > -5.0).
    pub const TRANSE_VALID_SCORE_MIN: f32 = -5.0;
    /// Maximum TransE score for invalid triples (KEPLER: typically < -10.0).
    pub const TRANSE_INVALID_SCORE_MAX: f32 = -10.0;
    /// Minimum score separation between valid and invalid (KEPLER: typically > 5.0).
    pub const TRANSE_SEPARATION_MIN: f32 = 5.0;
    /// Minimum relation inference MRR (KEPLER: should achieve >= 0.40).
    pub const RELATION_INFERENCE_MRR_MIN: f64 = 0.40;
    /// Minimum knowledge validation accuracy (KEPLER: should achieve >= 75%).
    pub const KNOWLEDGE_VALIDATION_ACCURACY_MIN: f64 = 0.75;
    /// Minimum E11 contribution over E1-only (KEPLER: should achieve >= 10%).
    pub const E11_CONTRIBUTION_PCT_MIN: f64 = 10.0;
    /// Maximum entity search latency in milliseconds.
    pub const ENTITY_SEARCH_LATENCY_MS_MAX: f64 = 100.0;
}

// ============================================================================
// Entity Extraction Metrics
// ============================================================================

/// Metrics for entity extraction accuracy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionMetrics {
    /// Precision: correct entities / predicted entities.
    pub precision: f64,
    /// Recall: correct entities / actual entities.
    pub recall: f64,
    /// F1 score: harmonic mean of precision and recall.
    pub f1_score: f64,
    /// Canonicalization accuracy: correct canonical forms / total.
    pub canonicalization_accuracy: f64,
    /// True positives (correctly predicted entities).
    pub true_positives: usize,
    /// False positives (incorrectly predicted entities).
    pub false_positives: usize,
    /// False negatives (missed entities).
    pub false_negatives: usize,
    /// Total documents evaluated.
    pub documents_evaluated: usize,
    /// Entity type breakdown.
    pub by_entity_type: HashMap<String, EntityTypeMetrics>,
}

impl ExtractionMetrics {
    /// Check if extraction meets target thresholds.
    pub fn meets_targets(&self) -> bool {
        self.f1_score >= thresholds::EXTRACTION_F1_MIN
            && self.canonicalization_accuracy >= thresholds::CANONICALIZATION_ACCURACY_MIN
    }

    /// Compute from predictions and ground truth.
    pub fn compute(
        predicted: &[String],
        ground_truth: &[String],
    ) -> Self {
        use std::collections::HashSet;

        let pred_set: HashSet<_> = predicted.iter().collect();
        let truth_set: HashSet<_> = ground_truth.iter().collect();

        let true_positives = pred_set.intersection(&truth_set).count();
        let false_positives = pred_set.difference(&truth_set).count();
        let false_negatives = truth_set.difference(&pred_set).count();

        let precision = if predicted.is_empty() {
            0.0
        } else {
            true_positives as f64 / predicted.len() as f64
        };

        let recall = if ground_truth.is_empty() {
            0.0
        } else {
            true_positives as f64 / ground_truth.len() as f64
        };

        let f1_score = if precision + recall > 0.0 {
            2.0 * (precision * recall) / (precision + recall)
        } else {
            0.0
        };

        Self {
            precision,
            recall,
            f1_score,
            canonicalization_accuracy: 0.0, // Computed separately
            true_positives,
            false_positives,
            false_negatives,
            documents_evaluated: 1,
            by_entity_type: HashMap::new(),
        }
    }

    /// Aggregate multiple extraction results.
    pub fn aggregate(metrics: &[Self]) -> Self {
        if metrics.is_empty() {
            return Self::default();
        }

        let total_tp: usize = metrics.iter().map(|m| m.true_positives).sum();
        let total_fp: usize = metrics.iter().map(|m| m.false_positives).sum();
        let total_fn: usize = metrics.iter().map(|m| m.false_negatives).sum();

        let precision = if total_tp + total_fp > 0 {
            total_tp as f64 / (total_tp + total_fp) as f64
        } else {
            0.0
        };

        let recall = if total_tp + total_fn > 0 {
            total_tp as f64 / (total_tp + total_fn) as f64
        } else {
            0.0
        };

        let f1_score = if precision + recall > 0.0 {
            2.0 * (precision * recall) / (precision + recall)
        } else {
            0.0
        };

        let canonicalization_sum: f64 = metrics.iter().map(|m| m.canonicalization_accuracy).sum();
        let canonicalization_accuracy = canonicalization_sum / metrics.len() as f64;

        // Aggregate by entity type
        let mut by_entity_type: HashMap<String, EntityTypeMetrics> = HashMap::new();
        for m in metrics {
            for (entity_type, type_metrics) in &m.by_entity_type {
                by_entity_type
                    .entry(entity_type.clone())
                    .or_default()
                    .merge(type_metrics);
            }
        }

        Self {
            precision,
            recall,
            f1_score,
            canonicalization_accuracy,
            true_positives: total_tp,
            false_positives: total_fp,
            false_negatives: total_fn,
            documents_evaluated: metrics.iter().map(|m| m.documents_evaluated).sum(),
            by_entity_type,
        }
    }
}

/// Metrics for a specific entity type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityTypeMetrics {
    pub true_positives: usize,
    pub false_positives: usize,
    pub false_negatives: usize,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
}

impl EntityTypeMetrics {
    pub fn merge(&mut self, other: &Self) {
        self.true_positives += other.true_positives;
        self.false_positives += other.false_positives;
        self.false_negatives += other.false_negatives;
        self.recompute();
    }

    fn recompute(&mut self) {
        self.precision = if self.true_positives + self.false_positives > 0 {
            self.true_positives as f64 / (self.true_positives + self.false_positives) as f64
        } else {
            0.0
        };

        self.recall = if self.true_positives + self.false_negatives > 0 {
            self.true_positives as f64 / (self.true_positives + self.false_negatives) as f64
        } else {
            0.0
        };

        self.f1_score = if self.precision + self.recall > 0.0 {
            2.0 * (self.precision * self.recall) / (self.precision + self.recall)
        } else {
            0.0
        };
    }
}

// ============================================================================
// Entity Retrieval Metrics
// ============================================================================

/// Metrics for entity-based retrieval comparing E1-only, E11-only, and hybrid.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityRetrievalMetrics {
    /// MRR using E1 semantic only.
    pub mrr_e1_only: f64,
    /// MRR using E11 entity only.
    pub mrr_e11_only: f64,
    /// MRR using E1+E11 hybrid.
    pub mrr_e1_e11_hybrid: f64,
    /// E11 contribution percentage: ((hybrid - e1_only) / e1_only) * 100.
    pub e11_contribution_pct: f64,
    /// Correlation between entity overlap and similarity score.
    pub entity_overlap_correlation: f64,
    /// Precision at K values for each approach.
    pub precision_at_k: HashMap<usize, RetrievalApproachComparison>,
    /// Recall at K values for each approach.
    pub recall_at_k: HashMap<usize, RetrievalApproachComparison>,
    /// Number of queries evaluated.
    pub queries_evaluated: usize,
}

impl EntityRetrievalMetrics {
    /// Check if E11 contribution meets target.
    pub fn meets_targets(&self) -> bool {
        self.e11_contribution_pct >= thresholds::E11_CONTRIBUTION_PCT_MIN
    }

    /// Compute E11 contribution from E1 and hybrid MRR.
    pub fn compute_contribution(e1_mrr: f64, hybrid_mrr: f64) -> f64 {
        if e1_mrr > 0.0 {
            ((hybrid_mrr - e1_mrr) / e1_mrr) * 100.0
        } else if hybrid_mrr > 0.0 {
            100.0 // E1 was 0, hybrid is positive - infinite improvement
        } else {
            0.0
        }
    }
}

/// Comparison of retrieval approaches.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalApproachComparison {
    pub e1_only: f64,
    pub e11_only: f64,
    pub e1_e11_hybrid: f64,
}

// ============================================================================
// TransE Metrics
// ============================================================================

/// Metrics for TransE relationship operations.
///
/// KEPLER produces meaningful TransE scores due to TransE training on Wikidata5M.
/// Unlike generic sentence embedders, the operation h + r ≈ t is semantically valid.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransEMetrics {
    /// Average TransE score for valid triples (KEPLER: should be > -5.0).
    pub valid_triple_avg_score: f32,
    /// Average TransE score for invalid triples (KEPLER: should be < -10.0).
    pub invalid_triple_avg_score: f32,
    /// Score separation: valid_avg - invalid_avg (KEPLER: should be > 5.0).
    pub separation_score: f32,
    /// MRR for relationship inference.
    pub relationship_inference_mrr: f64,
    /// Accuracy of knowledge validation (classifying valid vs invalid).
    pub knowledge_validation_accuracy: f64,
    /// Valid triples evaluated.
    pub valid_triples_evaluated: usize,
    /// Invalid triples evaluated.
    pub invalid_triples_evaluated: usize,
    /// Distribution of valid triple scores.
    pub valid_score_distribution: ScoreDistribution,
    /// Distribution of invalid triple scores.
    pub invalid_score_distribution: ScoreDistribution,
}

impl TransEMetrics {
    /// Check if TransE metrics meet KEPLER-calibrated targets.
    pub fn meets_targets(&self) -> bool {
        self.valid_triple_avg_score > thresholds::TRANSE_VALID_SCORE_MIN
            && self.invalid_triple_avg_score < thresholds::TRANSE_INVALID_SCORE_MAX
            && self.separation_score > thresholds::TRANSE_SEPARATION_MIN
            && self.relationship_inference_mrr >= thresholds::RELATION_INFERENCE_MRR_MIN
            && self.knowledge_validation_accuracy >= thresholds::KNOWLEDGE_VALIDATION_ACCURACY_MIN
    }

    /// Compute from valid and invalid scores.
    pub fn compute(valid_scores: &[f32], invalid_scores: &[f32]) -> Self {
        let valid_triple_avg_score = if valid_scores.is_empty() {
            f32::NEG_INFINITY
        } else {
            valid_scores.iter().sum::<f32>() / valid_scores.len() as f32
        };

        let invalid_triple_avg_score = if invalid_scores.is_empty() {
            f32::INFINITY
        } else {
            invalid_scores.iter().sum::<f32>() / invalid_scores.len() as f32
        };

        let separation_score = valid_triple_avg_score - invalid_triple_avg_score;

        Self {
            valid_triple_avg_score,
            invalid_triple_avg_score,
            separation_score,
            relationship_inference_mrr: 0.0, // Computed separately
            knowledge_validation_accuracy: 0.0, // Computed separately
            valid_triples_evaluated: valid_scores.len(),
            invalid_triples_evaluated: invalid_scores.len(),
            valid_score_distribution: ScoreDistribution::compute(valid_scores),
            invalid_score_distribution: ScoreDistribution::compute(invalid_scores),
        }
    }

    /// Compute knowledge validation accuracy.
    pub fn compute_validation_accuracy(
        valid_scores: &[f32],
        invalid_scores: &[f32],
        threshold: f32,
    ) -> f64 {
        let correct_valid = valid_scores.iter().filter(|&&s| s > threshold).count();
        let correct_invalid = invalid_scores.iter().filter(|&&s| s <= threshold).count();
        let total = valid_scores.len() + invalid_scores.len();

        if total > 0 {
            (correct_valid + correct_invalid) as f64 / total as f64
        } else {
            0.0
        }
    }
}

/// Statistics for score distribution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoreDistribution {
    pub mean: f32,
    pub std_dev: f32,
    pub min: f32,
    pub max: f32,
    pub median: f32,
    pub p25: f32,
    pub p75: f32,
}

impl ScoreDistribution {
    pub fn compute(scores: &[f32]) -> Self {
        if scores.is_empty() {
            return Self::default();
        }

        let mut sorted = scores.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mean = scores.iter().sum::<f32>() / scores.len() as f32;
        let variance = scores.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / scores.len() as f32;
        let std_dev = variance.sqrt();

        let min = sorted[0];
        let max = sorted[sorted.len() - 1];
        let median = sorted[sorted.len() / 2];
        let p25 = sorted[sorted.len() / 4];
        let p75 = sorted[(sorted.len() * 3) / 4];

        Self {
            mean,
            std_dev,
            min,
            max,
            median,
            p25,
            p75,
        }
    }
}

// ============================================================================
// Entity Graph Metrics
// ============================================================================

/// Metrics for entity graph construction.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityGraphMetrics {
    /// Number of entity nodes discovered.
    pub nodes_discovered: usize,
    /// Number of relationship edges inferred.
    pub edges_inferred: usize,
    /// Average edge score.
    pub avg_edge_score: f32,
    /// Edge score standard deviation.
    pub edge_score_std_dev: f32,
    /// Entity type distribution in graph.
    pub entity_type_distribution: HashMap<String, usize>,
    /// Relationship type distribution.
    pub relationship_type_distribution: HashMap<String, usize>,
    /// Graph density: edges / (nodes * (nodes - 1)).
    pub graph_density: f64,
    /// Number of connected components.
    pub connected_components: usize,
}

impl EntityGraphMetrics {
    /// Compute graph metrics from nodes and edges.
    pub fn compute(
        nodes: &[(String, String)], // (id, entity_type)
        edges: &[(String, String, String, f32)], // (source, target, relation, weight)
    ) -> Self {
        let nodes_discovered = nodes.len();
        let edges_inferred = edges.len();

        // Edge score statistics
        let edge_scores: Vec<f32> = edges.iter().map(|e| e.3).collect();
        let (avg_edge_score, edge_score_std_dev) = if edge_scores.is_empty() {
            (0.0, 0.0)
        } else {
            let mean = edge_scores.iter().sum::<f32>() / edge_scores.len() as f32;
            let variance = edge_scores.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / edge_scores.len() as f32;
            (mean, variance.sqrt())
        };

        // Entity type distribution
        let mut entity_type_distribution: HashMap<String, usize> = HashMap::new();
        for (_, entity_type) in nodes {
            *entity_type_distribution.entry(entity_type.clone()).or_insert(0) += 1;
        }

        // Relationship type distribution
        let mut relationship_type_distribution: HashMap<String, usize> = HashMap::new();
        for (_, _, relation, _) in edges {
            *relationship_type_distribution.entry(relation.clone()).or_insert(0) += 1;
        }

        // Graph density
        let graph_density = if nodes_discovered > 1 {
            edges_inferred as f64 / (nodes_discovered * (nodes_discovered - 1)) as f64
        } else {
            0.0
        };

        Self {
            nodes_discovered,
            edges_inferred,
            avg_edge_score,
            edge_score_std_dev,
            entity_type_distribution,
            relationship_type_distribution,
            graph_density,
            connected_components: 0, // Would need graph traversal to compute
        }
    }
}

// ============================================================================
// Performance Metrics
// ============================================================================

/// Performance metrics for E11 operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E11PerformanceMetrics {
    /// Extract entities latency (ms).
    pub extract_entities_latency_ms: LatencyStats,
    /// Search by entities latency (ms).
    pub search_by_entities_latency_ms: LatencyStats,
    /// Infer relationship latency (ms).
    pub infer_relationship_latency_ms: LatencyStats,
    /// Find related entities latency (ms).
    pub find_related_entities_latency_ms: LatencyStats,
    /// Validate knowledge latency (ms).
    pub validate_knowledge_latency_ms: LatencyStats,
    /// Get entity graph latency (ms).
    pub get_entity_graph_latency_ms: LatencyStats,
    /// E11 embedding latency (ms).
    pub e11_embedding_latency_ms: LatencyStats,
}

impl E11PerformanceMetrics {
    /// Check if all latencies meet targets.
    pub fn meets_targets(&self) -> bool {
        self.search_by_entities_latency_ms.p95 < thresholds::ENTITY_SEARCH_LATENCY_MS_MAX
    }
}

/// Latency statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyStats {
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub samples: usize,
}

impl LatencyStats {
    pub fn compute(latencies: &[f64]) -> Self {
        if latencies.is_empty() {
            return Self::default();
        }

        let mut sorted = latencies.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mean = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let min = sorted[0];
        let max = sorted[sorted.len() - 1];

        let p50_idx = sorted.len() / 2;
        let p95_idx = (sorted.len() * 95) / 100;
        let p99_idx = (sorted.len() * 99) / 100;

        Self {
            mean,
            min,
            max,
            p50: sorted[p50_idx],
            p95: sorted[p95_idx.min(sorted.len() - 1)],
            p99: sorted[p99_idx.min(sorted.len() - 1)],
            samples: latencies.len(),
        }
    }
}

// ============================================================================
// Combined E11 Metrics
// ============================================================================

/// Complete E11 entity benchmark metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E11EntityMetrics {
    /// Entity extraction metrics.
    pub extraction: ExtractionMetrics,
    /// Entity-based retrieval metrics.
    pub retrieval: EntityRetrievalMetrics,
    /// TransE operation metrics.
    pub transe: TransEMetrics,
    /// Entity graph metrics.
    pub graph: EntityGraphMetrics,
    /// Performance metrics.
    pub performance: E11PerformanceMetrics,
}

impl E11EntityMetrics {
    /// Check if all metrics meet targets.
    pub fn meets_all_targets(&self) -> bool {
        self.extraction.meets_targets()
            && self.retrieval.meets_targets()
            && self.transe.meets_targets()
            && self.performance.meets_targets()
    }

    /// Get overall quality score (0-100).
    pub fn overall_quality_score(&self) -> f64 {
        let extraction_score = self.extraction.f1_score * 30.0; // 30% weight
        let transe_score = if self.transe.separation_score > 0.0 {
            (self.transe.separation_score.min(3.0) / 3.0) as f64 * 30.0
        } else {
            0.0
        }; // 30% weight
        let retrieval_score = if self.retrieval.e11_contribution_pct > 0.0 {
            (self.retrieval.e11_contribution_pct.min(20.0) / 20.0) * 20.0
        } else {
            0.0
        }; // 20% weight
        let mrr_score = self.transe.relationship_inference_mrr * 20.0; // 20% weight

        extraction_score + transe_score + retrieval_score + mrr_score
    }

    /// Generate summary report.
    pub fn summary_report(&self) -> String {
        let extraction_status = if self.extraction.meets_targets() { "PASS" } else { "FAIL" };
        let retrieval_status = if self.retrieval.meets_targets() { "PASS" } else { "FAIL" };
        let transe_status = if self.transe.meets_targets() { "PASS" } else { "FAIL" };
        let performance_status = if self.performance.meets_targets() { "PASS" } else { "FAIL" };

        format!(
            r#"
E11 Entity Benchmark Results
==============================
Overall Quality Score: {:.1}/100

Extraction [{extraction_status}]
  F1: {:.3} (target >= {:.2})
  Canonicalization: {:.3} (target >= {:.2})
  TP: {}, FP: {}, FN: {}

Retrieval [{retrieval_status}]
  MRR E1-only: {:.3}
  MRR E11-only: {:.3}
  MRR E1+E11: {:.3}
  E11 Contribution: {:.1}% (target >= {:.1}%)

TransE [{transe_status}]
  Valid Triple Avg Score: {:.3} (target > {:.1})
  Invalid Triple Avg Score: {:.3} (target < {:.1})
  Separation Score: {:.3}
  Relation Inference MRR: {:.3} (target >= {:.2})
  Validation Accuracy: {:.1}%

Graph
  Nodes: {}
  Edges: {}
  Density: {:.4}

Performance [{performance_status}]
  Entity Search P95: {:.1}ms (target < {:.0}ms)
"#,
            self.overall_quality_score(),
            self.extraction.f1_score, thresholds::EXTRACTION_F1_MIN,
            self.extraction.canonicalization_accuracy, thresholds::CANONICALIZATION_ACCURACY_MIN,
            self.extraction.true_positives, self.extraction.false_positives, self.extraction.false_negatives,
            self.retrieval.mrr_e1_only,
            self.retrieval.mrr_e11_only,
            self.retrieval.mrr_e1_e11_hybrid,
            self.retrieval.e11_contribution_pct, thresholds::E11_CONTRIBUTION_PCT_MIN,
            self.transe.valid_triple_avg_score, thresholds::TRANSE_VALID_SCORE_MIN,
            self.transe.invalid_triple_avg_score, thresholds::TRANSE_INVALID_SCORE_MAX,
            self.transe.separation_score,
            self.transe.relationship_inference_mrr, thresholds::RELATION_INFERENCE_MRR_MIN,
            self.transe.knowledge_validation_accuracy * 100.0,
            self.graph.nodes_discovered,
            self.graph.edges_inferred,
            self.graph.graph_density,
            self.performance.search_by_entities_latency_ms.p95, thresholds::ENTITY_SEARCH_LATENCY_MS_MAX,
        )
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Compute MRR (Mean Reciprocal Rank).
pub fn compute_mrr(ranks: &[usize]) -> f64 {
    if ranks.is_empty() {
        return 0.0;
    }

    let reciprocal_sum: f64 = ranks
        .iter()
        .filter(|&&r| r > 0)
        .map(|&r| 1.0 / r as f64)
        .sum();

    reciprocal_sum / ranks.len() as f64
}

/// Compute NDCG at K.
pub fn compute_ndcg_at_k(relevances: &[f64], k: usize) -> f64 {
    if relevances.is_empty() || k == 0 {
        return 0.0;
    }

    let k = k.min(relevances.len());

    // DCG
    let dcg: f64 = relevances[..k]
        .iter()
        .enumerate()
        .map(|(i, &rel)| rel / (i as f64 + 2.0).log2())
        .sum();

    // Ideal DCG
    let mut sorted_relevances = relevances.to_vec();
    sorted_relevances.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let idcg: f64 = sorted_relevances[..k]
        .iter()
        .enumerate()
        .map(|(i, &rel)| rel / (i as f64 + 2.0).log2())
        .sum();

    if idcg > 0.0 {
        dcg / idcg
    } else {
        0.0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extraction_metrics_compute() {
        let predicted = vec!["rust".to_string(), "python".to_string(), "unknown".to_string()];
        let ground_truth = vec!["rust".to_string(), "python".to_string(), "java".to_string()];

        let metrics = ExtractionMetrics::compute(&predicted, &ground_truth);

        assert_eq!(metrics.true_positives, 2);
        assert_eq!(metrics.false_positives, 1);
        assert_eq!(metrics.false_negatives, 1);
        assert!((metrics.precision - 0.667).abs() < 0.01);
        assert!((metrics.recall - 0.667).abs() < 0.01);
    }

    #[test]
    fn test_transe_metrics_compute() {
        // KEPLER-appropriate scores (larger magnitude than MiniLM)
        let valid_scores = vec![-3.0, -2.5, -4.0, -4.5, -2.0];
        let invalid_scores = vec![-12.0, -11.5, -13.0, -11.0, -12.5];

        let metrics = TransEMetrics::compute(&valid_scores, &invalid_scores);

        // KEPLER thresholds: valid > -5.0, invalid < -10.0, separation > 5.0
        assert!(metrics.valid_triple_avg_score > thresholds::TRANSE_VALID_SCORE_MIN);
        assert!(metrics.invalid_triple_avg_score < thresholds::TRANSE_INVALID_SCORE_MAX);
        assert!(metrics.separation_score > thresholds::TRANSE_SEPARATION_MIN);
    }

    #[test]
    fn test_score_distribution() {
        let scores = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let dist = ScoreDistribution::compute(&scores);

        assert_eq!(dist.mean, 3.0);
        assert_eq!(dist.min, 1.0);
        assert_eq!(dist.max, 5.0);
        assert_eq!(dist.median, 3.0);
    }

    #[test]
    fn test_compute_mrr() {
        let ranks = vec![1, 2, 3, 4, 5];
        let mrr = compute_mrr(&ranks);
        // 1/1 + 1/2 + 1/3 + 1/4 + 1/5 = 2.283... / 5 = 0.457
        assert!((mrr - 0.457).abs() < 0.01);
    }

    #[test]
    fn test_latency_stats() {
        let latencies = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];
        let stats = LatencyStats::compute(&latencies);

        assert_eq!(stats.mean, 55.0);
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 100.0);
        assert_eq!(stats.samples, 10);
    }

    #[test]
    fn test_retrieval_contribution() {
        let e1_mrr = 0.5;
        let hybrid_mrr = 0.6;
        let contribution = EntityRetrievalMetrics::compute_contribution(e1_mrr, hybrid_mrr);
        assert!((contribution - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_entity_graph_metrics() {
        let nodes = vec![
            ("rust".to_string(), "ProgrammingLanguage".to_string()),
            ("tokio".to_string(), "Framework".to_string()),
            ("postgresql".to_string(), "Database".to_string()),
        ];
        let edges = vec![
            ("tokio".to_string(), "rust".to_string(), "depends_on".to_string(), 0.9),
            ("tokio".to_string(), "postgresql".to_string(), "uses".to_string(), 0.7),
        ];

        let metrics = EntityGraphMetrics::compute(&nodes, &edges);

        assert_eq!(metrics.nodes_discovered, 3);
        assert_eq!(metrics.edges_inferred, 2);
        assert!((metrics.avg_edge_score - 0.8).abs() < 0.01);
    }
}
