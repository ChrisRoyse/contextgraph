//! E10 Multimodal benchmark metrics.
//!
//! Metrics for evaluating E10 intent/context embeddings:
//!
//! - **Intent Detection**: Accuracy of detecting intent vs context
//! - **Context Matching**: Quality of context-aware retrieval
//! - **Asymmetric Retrieval**: Validation of 1.2/0.8 direction modifiers
//! - **Ablation**: E10 contribution vs E1 baseline

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Combined E10 multimodal metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E10MultimodalMetrics {
    /// Intent detection metrics.
    pub intent_detection: IntentDetectionMetrics,
    /// Context matching metrics.
    pub context_matching: ContextMatchingMetrics,
    /// Asymmetric retrieval metrics.
    pub asymmetric_retrieval: AsymmetricRetrievalMetrics,
    /// Ablation study results.
    pub ablation: Option<E10AblationMetrics>,
}

impl E10MultimodalMetrics {
    /// Compute overall quality score.
    pub fn overall_score(&self) -> f64 {
        let intent_score = self.intent_detection.accuracy;
        let context_score = self.context_matching.mrr;
        let asymmetry_score = if self.asymmetric_retrieval.formula_compliant {
            1.0
        } else {
            0.5
        };

        // 40% intent detection + 40% context matching + 20% asymmetry compliance
        0.4 * intent_score + 0.4 * context_score + 0.2 * asymmetry_score
    }
}

/// Metrics for intent detection evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentDetectionMetrics {
    /// Total queries evaluated.
    pub total_queries: usize,
    /// Correctly classified as intent.
    pub correct_intent: usize,
    /// Correctly classified as context.
    pub correct_context: usize,
    /// Misclassified queries.
    pub misclassified: usize,
    /// Overall accuracy.
    pub accuracy: f64,
    /// Per-domain accuracy breakdown.
    pub per_domain_accuracy: HashMap<String, f64>,
    /// Precision for intent class.
    pub intent_precision: f64,
    /// Recall for intent class.
    pub intent_recall: f64,
    /// F1 score for intent class.
    pub intent_f1: f64,
}

impl Default for IntentDetectionMetrics {
    fn default() -> Self {
        Self {
            total_queries: 0,
            correct_intent: 0,
            correct_context: 0,
            misclassified: 0,
            accuracy: 0.0,
            per_domain_accuracy: HashMap::new(),
            intent_precision: 0.0,
            intent_recall: 0.0,
            intent_f1: 0.0,
        }
    }
}

/// Results from a single intent detection test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentDetectionResult {
    /// Query text.
    pub query: String,
    /// Expected direction.
    pub expected: String,
    /// Detected direction.
    pub detected: String,
    /// Whether detection was correct.
    pub correct: bool,
    /// Domain of the query.
    pub domain: String,
    /// Confidence score (if available).
    pub confidence: Option<f64>,
}

/// Metrics for context matching evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMatchingMetrics {
    /// Mean Reciprocal Rank.
    pub mrr: f64,
    /// Precision at various K values.
    pub precision_at_k: HashMap<usize, f64>,
    /// Recall at various K values.
    pub recall_at_k: HashMap<usize, f64>,
    /// NDCG at various K values.
    pub ndcg_at_k: HashMap<usize, f64>,
    /// Total queries evaluated.
    pub total_queries: usize,
    /// Queries where expected doc was in top 1.
    pub hits_at_1: usize,
    /// Queries where expected doc was in top 5.
    pub hits_at_5: usize,
    /// Queries where expected doc was in top 10.
    pub hits_at_10: usize,
}

impl Default for ContextMatchingMetrics {
    fn default() -> Self {
        Self {
            mrr: 0.0,
            precision_at_k: HashMap::new(),
            recall_at_k: HashMap::new(),
            ndcg_at_k: HashMap::new(),
            total_queries: 0,
            hits_at_1: 0,
            hits_at_5: 0,
            hits_at_10: 0,
        }
    }
}

/// Results from a single context matching query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMatchingResult {
    /// Query text.
    pub query: String,
    /// Expected document IDs.
    pub expected_docs: Vec<String>,
    /// Actual ranked results (doc_id, score).
    pub actual_ranking: Vec<(String, f64)>,
    /// Reciprocal rank for this query.
    pub reciprocal_rank: f64,
    /// Rank of first expected document (0 if not found).
    pub first_relevant_rank: usize,
}

/// Metrics for asymmetric retrieval validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsymmetricRetrievalMetrics {
    /// Total queries evaluated.
    pub total_queries: usize,
    /// Queries where intent→context scored higher.
    pub intent_to_context_wins: usize,
    /// Queries where context→intent scored higher.
    pub context_to_intent_wins: usize,
    /// Ties (equal scores).
    pub ties: usize,
    /// Observed asymmetry ratio (should be ~1.5 = 1.2/0.8).
    pub observed_asymmetry_ratio: f64,
    /// Expected asymmetry ratio (1.5).
    pub expected_asymmetry_ratio: f64,
    /// Whether formula is compliant with Constitution.
    pub formula_compliant: bool,
    /// Direction modifier: intent→context.
    pub intent_to_context_modifier: f32,
    /// Direction modifier: context→intent.
    pub context_to_intent_modifier: f32,
    /// Direction modifier: same direction.
    pub same_direction_modifier: f32,
    /// E10 contribution percentage.
    pub e10_contribution_percentage: f64,
}

impl Default for AsymmetricRetrievalMetrics {
    fn default() -> Self {
        Self {
            total_queries: 0,
            intent_to_context_wins: 0,
            context_to_intent_wins: 0,
            ties: 0,
            observed_asymmetry_ratio: 0.0,
            // E5-base-v2 handles asymmetry via prefixes - expected ratio is 1.0
            expected_asymmetry_ratio: 1.0,
            formula_compliant: false,
            // Neutral modifiers - E5-base-v2 handles asymmetry via query:/passage: prefixes
            intent_to_context_modifier: 1.0,
            context_to_intent_modifier: 1.0,
            same_direction_modifier: 1.0,
            e10_contribution_percentage: 0.0,
        }
    }
}

/// Result from a single asymmetric retrieval test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsymmetricRetrievalResult {
    /// Base cosine similarity.
    pub base_similarity: f64,
    /// Similarity with intent→context direction.
    pub intent_to_context_similarity: f64,
    /// Similarity with context→intent direction.
    pub context_to_intent_similarity: f64,
    /// Observed ratio.
    pub observed_ratio: f64,
    /// Whether this test passed.
    pub passed: bool,
}

/// Ablation study results for E10.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E10AblationMetrics {
    /// MRR with E1 only (baseline).
    pub e1_only_mrr: f64,
    /// MRR with E10 only.
    pub e10_only_mrr: f64,
    /// MRR with E1 + E10 (default blend 0.3).
    pub e1_e10_blend_mrr: f64,
    /// MRR with full 13-space.
    pub full_13_space_mrr: f64,
    /// E10 contribution (E1+E10 - E1 only).
    pub e10_contribution: f64,
    /// E10 contribution percentage.
    pub e10_contribution_percentage: f64,
    /// Blend parameter analysis.
    pub blend_analysis: Vec<BlendAnalysisPoint>,
}

impl Default for E10AblationMetrics {
    fn default() -> Self {
        Self {
            e1_only_mrr: 0.0,
            e10_only_mrr: 0.0,
            e1_e10_blend_mrr: 0.0,
            full_13_space_mrr: 0.0,
            e10_contribution: 0.0,
            e10_contribution_percentage: 0.0,
            blend_analysis: Vec::new(),
        }
    }
}

/// Analysis point for blend parameter sweep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlendAnalysisPoint {
    /// Blend value (0.0 = pure E1, 1.0 = pure E10).
    pub blend_value: f64,
    /// E1 weight (1.0 - blend).
    pub e1_weight: f64,
    /// E10 weight (blend).
    pub e10_weight: f64,
    /// MRR at this blend.
    pub mrr: f64,
    /// Precision@5 at this blend.
    pub precision_at_5: f64,
}

// =============================================================================
// Metric Computation Functions
// =============================================================================

/// Compute MRR from ranked results.
pub fn compute_mrr(results: &[(String, f64)], expected: &[String]) -> f64 {
    for (rank, (doc_id, _)) in results.iter().enumerate() {
        if expected.contains(doc_id) {
            return 1.0 / (rank + 1) as f64;
        }
    }
    0.0
}

/// Compute Precision@K.
pub fn compute_precision_at_k(results: &[(String, f64)], expected: &[String], k: usize) -> f64 {
    let top_k: Vec<_> = results.iter().take(k).map(|(id, _)| id).collect();
    let relevant_in_top_k = top_k.iter().filter(|id| expected.contains(*id)).count();
    relevant_in_top_k as f64 / k as f64
}

/// Compute Recall@K.
pub fn compute_recall_at_k(results: &[(String, f64)], expected: &[String], k: usize) -> f64 {
    if expected.is_empty() {
        return 0.0;
    }
    let top_k: Vec<_> = results.iter().take(k).map(|(id, _)| id).collect();
    let relevant_in_top_k = top_k.iter().filter(|id| expected.contains(*id)).count();
    relevant_in_top_k as f64 / expected.len() as f64
}

/// Compute NDCG@K.
pub fn compute_ndcg_at_k(results: &[(String, f64)], expected: &[String], k: usize) -> f64 {
    // DCG
    let mut dcg = 0.0;
    for (rank, (doc_id, _)) in results.iter().take(k).enumerate() {
        if expected.contains(doc_id) {
            dcg += 1.0 / (rank as f64 + 2.0).log2();
        }
    }

    // Ideal DCG (all relevant docs at top)
    let num_relevant = expected.len().min(k);
    let mut idcg = 0.0;
    for rank in 0..num_relevant {
        idcg += 1.0 / (rank as f64 + 2.0).log2();
    }

    if idcg == 0.0 {
        0.0
    } else {
        dcg / idcg
    }
}

/// Compute intent detection metrics from results.
pub fn compute_intent_detection_metrics(
    results: &[IntentDetectionResult],
) -> IntentDetectionMetrics {
    let total = results.len();
    if total == 0 {
        return IntentDetectionMetrics::default();
    }

    let correct_intent = results
        .iter()
        .filter(|r| r.correct && r.expected == "intent")
        .count();
    let correct_context = results
        .iter()
        .filter(|r| r.correct && r.expected == "context")
        .count();
    let misclassified = results.iter().filter(|r| !r.correct).count();

    let accuracy = (correct_intent + correct_context) as f64 / total as f64;

    // Per-domain accuracy
    let mut domain_correct: HashMap<String, usize> = HashMap::new();
    let mut domain_total: HashMap<String, usize> = HashMap::new();

    for result in results {
        *domain_total.entry(result.domain.clone()).or_insert(0) += 1;
        if result.correct {
            *domain_correct.entry(result.domain.clone()).or_insert(0) += 1;
        }
    }

    let per_domain_accuracy: HashMap<String, f64> = domain_total
        .iter()
        .map(|(domain, &total)| {
            let correct = domain_correct.get(domain).copied().unwrap_or(0);
            (domain.clone(), correct as f64 / total as f64)
        })
        .collect();

    // Precision/Recall/F1 for intent class
    let predicted_intent = results.iter().filter(|r| r.detected == "intent").count();
    let actual_intent = results.iter().filter(|r| r.expected == "intent").count();
    let true_positive_intent = results
        .iter()
        .filter(|r| r.detected == "intent" && r.expected == "intent")
        .count();

    let intent_precision = if predicted_intent > 0 {
        true_positive_intent as f64 / predicted_intent as f64
    } else {
        0.0
    };

    let intent_recall = if actual_intent > 0 {
        true_positive_intent as f64 / actual_intent as f64
    } else {
        0.0
    };

    let intent_f1 = if intent_precision + intent_recall > 0.0 {
        2.0 * intent_precision * intent_recall / (intent_precision + intent_recall)
    } else {
        0.0
    };

    IntentDetectionMetrics {
        total_queries: total,
        correct_intent,
        correct_context,
        misclassified,
        accuracy,
        per_domain_accuracy,
        intent_precision,
        intent_recall,
        intent_f1,
    }
}

/// Compute context matching metrics from results.
pub fn compute_context_matching_metrics(
    results: &[ContextMatchingResult],
    k_values: &[usize],
) -> ContextMatchingMetrics {
    let total = results.len();
    if total == 0 {
        return ContextMatchingMetrics::default();
    }

    // MRR
    let mrr = results.iter().map(|r| r.reciprocal_rank).sum::<f64>() / total as f64;

    // Hits at various K
    let hits_at_1 = results.iter().filter(|r| r.first_relevant_rank == 1).count();
    let hits_at_5 = results
        .iter()
        .filter(|r| r.first_relevant_rank > 0 && r.first_relevant_rank <= 5)
        .count();
    let hits_at_10 = results
        .iter()
        .filter(|r| r.first_relevant_rank > 0 && r.first_relevant_rank <= 10)
        .count();

    // P@K, R@K, NDCG@K
    let mut precision_at_k = HashMap::new();
    let mut recall_at_k = HashMap::new();
    let mut ndcg_at_k = HashMap::new();

    for &k in k_values {
        let p_k: f64 = results
            .iter()
            .map(|r| compute_precision_at_k(&r.actual_ranking, &r.expected_docs, k))
            .sum::<f64>()
            / total as f64;
        let r_k: f64 = results
            .iter()
            .map(|r| compute_recall_at_k(&r.actual_ranking, &r.expected_docs, k))
            .sum::<f64>()
            / total as f64;
        let n_k: f64 = results
            .iter()
            .map(|r| compute_ndcg_at_k(&r.actual_ranking, &r.expected_docs, k))
            .sum::<f64>()
            / total as f64;

        precision_at_k.insert(k, p_k);
        recall_at_k.insert(k, r_k);
        ndcg_at_k.insert(k, n_k);
    }

    ContextMatchingMetrics {
        mrr,
        precision_at_k,
        recall_at_k,
        ndcg_at_k,
        total_queries: total,
        hits_at_1,
        hits_at_5,
        hits_at_10,
    }
}

/// Compute asymmetric retrieval metrics.
pub fn compute_asymmetric_retrieval_metrics(
    results: &[AsymmetricRetrievalResult],
) -> AsymmetricRetrievalMetrics {
    let total = results.len();
    if total == 0 {
        return AsymmetricRetrievalMetrics::default();
    }

    let intent_to_context_wins = results
        .iter()
        .filter(|r| r.intent_to_context_similarity > r.context_to_intent_similarity)
        .count();
    let context_to_intent_wins = results
        .iter()
        .filter(|r| r.context_to_intent_similarity > r.intent_to_context_similarity)
        .count();
    let ties = total - intent_to_context_wins - context_to_intent_wins;

    // Compute observed asymmetry ratio
    let avg_ratio: f64 = results.iter().map(|r| r.observed_ratio).sum::<f64>() / total as f64;

    // Check formula compliance - E5-base-v2 handles asymmetry via prefixes, expected ratio ~1.0
    let expected_ratio = 1.0;
    let tolerance = 0.1;
    let formula_compliant = (avg_ratio - expected_ratio).abs() < tolerance;

    // E10 contribution percentage
    let e10_contribution_percentage = (intent_to_context_wins as f64 / total as f64) * 100.0;

    AsymmetricRetrievalMetrics {
        total_queries: total,
        intent_to_context_wins,
        context_to_intent_wins,
        ties,
        observed_asymmetry_ratio: avg_ratio,
        expected_asymmetry_ratio: expected_ratio,
        formula_compliant,
        // Neutral modifiers - E5-base-v2 handles asymmetry via query:/passage: prefixes
        intent_to_context_modifier: 1.0,
        context_to_intent_modifier: 1.0,
        same_direction_modifier: 1.0,
        e10_contribution_percentage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_mrr() {
        let results = vec![
            ("doc1".to_string(), 0.9),
            ("doc2".to_string(), 0.8),
            ("doc3".to_string(), 0.7),
        ];

        // Expected doc at rank 1
        let mrr1 = compute_mrr(&results, &["doc1".to_string()]);
        assert!((mrr1 - 1.0).abs() < 1e-6);

        // Expected doc at rank 2
        let mrr2 = compute_mrr(&results, &["doc2".to_string()]);
        assert!((mrr2 - 0.5).abs() < 1e-6);

        // Expected doc at rank 3
        let mrr3 = compute_mrr(&results, &["doc3".to_string()]);
        assert!((mrr3 - 1.0 / 3.0).abs() < 1e-6);

        // Expected doc not found
        let mrr0 = compute_mrr(&results, &["doc4".to_string()]);
        assert!(mrr0.abs() < 1e-6);

        println!("[VERIFIED] MRR computation correct");
    }

    #[test]
    fn test_compute_precision_at_k() {
        let results = vec![
            ("doc1".to_string(), 0.9),
            ("doc2".to_string(), 0.8),
            ("doc3".to_string(), 0.7),
            ("doc4".to_string(), 0.6),
            ("doc5".to_string(), 0.5),
        ];
        let expected = vec!["doc1".to_string(), "doc3".to_string()];

        // P@1 = 1/1 = 1.0 (doc1 is relevant)
        let p1 = compute_precision_at_k(&results, &expected, 1);
        assert!((p1 - 1.0).abs() < 1e-6);

        // P@3 = 2/3 (doc1 and doc3 are relevant)
        let p3 = compute_precision_at_k(&results, &expected, 3);
        assert!((p3 - 2.0 / 3.0).abs() < 1e-6);

        // P@5 = 2/5
        let p5 = compute_precision_at_k(&results, &expected, 5);
        assert!((p5 - 0.4).abs() < 1e-6);

        println!("[VERIFIED] Precision@K computation correct");
    }

    #[test]
    fn test_asymmetric_metrics() {
        // E5-base-v2 handles asymmetry via prefixes - ratio should be ~1.0
        let results = vec![
            AsymmetricRetrievalResult {
                base_similarity: 0.8,
                intent_to_context_similarity: 0.8, // 0.8 * 1.0 (neutral)
                context_to_intent_similarity: 0.8, // 0.8 * 1.0 (neutral)
                observed_ratio: 1.0,
                passed: true,
            },
            AsymmetricRetrievalResult {
                base_similarity: 0.6,
                intent_to_context_similarity: 0.6,
                context_to_intent_similarity: 0.6,
                observed_ratio: 1.0,
                passed: true,
            },
        ];

        let metrics = compute_asymmetric_retrieval_metrics(&results);

        assert_eq!(metrics.total_queries, 2);
        // With 1.0/1.0 modifiers, scores are equal so we expect ties
        assert_eq!(metrics.ties, 2);
        assert!(metrics.formula_compliant);
        assert!((metrics.observed_asymmetry_ratio - 1.0).abs() < 0.01);

        println!("[VERIFIED] Asymmetric metrics: ratio={}, compliant={} (E5-base-v2 handles asymmetry via prefixes)",
            metrics.observed_asymmetry_ratio, metrics.formula_compliant);
    }

    #[test]
    fn test_intent_detection_metrics() {
        let results = vec![
            IntentDetectionResult {
                query: "optimize performance".to_string(),
                expected: "intent".to_string(),
                detected: "intent".to_string(),
                correct: true,
                domain: "performance".to_string(),
                confidence: Some(0.9),
            },
            IntentDetectionResult {
                query: "system is slow".to_string(),
                expected: "context".to_string(),
                detected: "context".to_string(),
                correct: true,
                domain: "performance".to_string(),
                confidence: Some(0.85),
            },
            IntentDetectionResult {
                query: "fix the bug".to_string(),
                expected: "intent".to_string(),
                detected: "context".to_string(),
                correct: false,
                domain: "bugfix".to_string(),
                confidence: Some(0.6),
            },
        ];

        let metrics = compute_intent_detection_metrics(&results);

        assert_eq!(metrics.total_queries, 3);
        assert_eq!(metrics.correct_intent, 1);
        assert_eq!(metrics.correct_context, 1);
        assert_eq!(metrics.misclassified, 1);
        assert!((metrics.accuracy - 2.0 / 3.0).abs() < 1e-6);

        println!("[VERIFIED] Intent detection metrics: accuracy={:.2}", metrics.accuracy);
    }
}
