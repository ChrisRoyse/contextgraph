//! Benchmark metrics for causal embedding evaluation.
//!
//! Includes both reused metrics from training/evaluation.rs and new benchmark-specific
//! metrics for ablation, causal gate, intent detection, and cross-domain analysis.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Result structs
// ============================================================================

/// Result of a single benchmark phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseBenchmarkResult {
    pub phase: u8,
    pub phase_name: String,
    pub metrics: HashMap<String, f64>,
    pub targets: HashMap<String, f64>,
    pub pass: bool,
    pub failing_criteria: Vec<String>,
    pub duration_ms: u64,
    /// Whether this phase tests the trained model (true) or infrastructure/heuristics (false).
    /// Phases marked false produce deterministic results regardless of model changes.
    #[serde(default = "default_model_dependent")]
    pub model_dependent: bool,
}

fn default_model_dependent() -> bool {
    true
}

/// Full benchmark report across all phases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullBenchmarkReport {
    pub model_name: String,
    pub timestamp: String,
    pub phases: Vec<PhaseBenchmarkResult>,
    pub overall_pass_count: usize,
    pub overall_total: usize,
    /// Pass count among model-dependent phases only.
    #[serde(default)]
    pub model_pass_count: usize,
    /// Total model-dependent phases.
    #[serde(default)]
    pub model_total: usize,
}

impl FullBenchmarkReport {
    /// Count passing phases.
    pub fn count_pass(&self) -> usize {
        self.phases.iter().filter(|p| p.pass).count()
    }

    /// Count warning phases (>50% criteria met but not all).
    pub fn count_warn(&self) -> usize {
        self.phases
            .iter()
            .filter(|p| {
                !p.pass
                    && !p.failing_criteria.is_empty()
                    && p.failing_criteria.len() < p.targets.len()
            })
            .count()
    }

    /// Count failing phases (>50% criteria failing).
    pub fn count_fail(&self) -> usize {
        self.phases.len() - self.count_pass() - self.count_warn()
    }

    /// Count passing model-dependent phases.
    pub fn count_model_pass(&self) -> usize {
        self.phases
            .iter()
            .filter(|p| p.model_dependent && p.pass)
            .count()
    }

    /// Count total model-dependent phases.
    pub fn count_model_total(&self) -> usize {
        self.phases.iter().filter(|p| p.model_dependent).count()
    }

    /// Count passing infrastructure (non-model-dependent) phases.
    pub fn count_infra_pass(&self) -> usize {
        self.phases
            .iter()
            .filter(|p| !p.model_dependent && p.pass)
            .count()
    }

    /// Count total infrastructure phases.
    pub fn count_infra_total(&self) -> usize {
        self.phases.iter().filter(|p| !p.model_dependent).count()
    }
}

// ============================================================================
// Confusion matrix for intent detection
// ============================================================================

/// 3-class confusion matrix for query intent detection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfusionMatrix {
    /// rows = predicted, cols = actual
    /// [cause][cause], [cause][effect], [cause][unknown]
    /// [effect][cause], [effect][effect], [effect][unknown]
    /// [unknown][cause], [unknown][effect], [unknown][unknown]
    pub matrix: [[usize; 3]; 3],
    pub labels: [String; 3],
}

impl ConfusionMatrix {
    pub fn new() -> Self {
        Self {
            matrix: [[0; 3]; 3],
            labels: [
                "cause".to_string(),
                "effect".to_string(),
                "unknown".to_string(),
            ],
        }
    }

    /// Record a prediction.
    pub fn record(&mut self, predicted: usize, actual: usize) {
        if predicted < 3 && actual < 3 {
            self.matrix[predicted][actual] += 1;
        }
    }

    /// Total predictions.
    pub fn total(&self) -> usize {
        self.matrix.iter().flat_map(|row| row.iter()).sum()
    }

    /// Overall accuracy.
    pub fn accuracy(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            return 0.0;
        }
        let correct: usize = (0..3).map(|i| self.matrix[i][i]).sum();
        correct as f64 / total as f64
    }

    /// Precision for a class.
    pub fn precision(&self, class: usize) -> f64 {
        let predicted: usize = self.matrix[class].iter().sum();
        if predicted == 0 {
            return 0.0;
        }
        self.matrix[class][class] as f64 / predicted as f64
    }

    /// Recall for a class.
    pub fn recall(&self, class: usize) -> f64 {
        let actual: usize = (0..3).map(|i| self.matrix[i][class]).sum();
        if actual == 0 {
            return 0.0;
        }
        self.matrix[class][class] as f64 / actual as f64
    }

    /// F1 for a class.
    pub fn f1(&self, class: usize) -> f64 {
        let p = self.precision(class);
        let r = self.recall(class);
        if p + r < f64::EPSILON {
            return 0.0;
        }
        2.0 * p * r / (p + r)
    }
}

// ============================================================================
// Score spread and anisotropy (reuse logic from training/evaluation.rs)
// ============================================================================

/// Compute score spread: top-1 similarity minus rank-5 similarity.
pub fn score_spread(similarities: &[f32]) -> f32 {
    if similarities.len() < 2 {
        return 0.0;
    }
    let mut sorted = similarities.to_vec();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let top1 = sorted[0];
    let rank5 = sorted.get(4).copied().unwrap_or(*sorted.last().unwrap());
    top1 - rank5
}

/// Measure embedding space anisotropy via average pairwise cosine similarity.
pub fn anisotropy_measure(vectors: &[Vec<f32>]) -> f32 {
    if vectors.len() < 2 {
        return 0.0;
    }

    let max_pairs = 200usize;
    let mut total_sim = 0.0f64;
    let mut count = 0usize;

    let n = vectors.len();
    let total_pairs = n * (n - 1) / 2;
    let step = if total_pairs > max_pairs {
        total_pairs / max_pairs
    } else {
        1
    };

    let mut pair_idx = 0usize;
    'outer: for i in 0..n {
        for j in (i + 1)..n {
            if pair_idx % step == 0 {
                let sim = cosine_similarity(&vectors[i], &vectors[j]);
                total_sim += sim as f64;
                count += 1;
                if count >= max_pairs {
                    break 'outer;
                }
            }
            pair_idx += 1;
        }
    }

    if count == 0 {
        0.0
    } else {
        (total_sim / count as f64) as f32
    }
}

/// Cosine similarity between two vectors (raw [-1, 1] range).
///
/// Delegates to the canonical implementation in `crate::util`.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    crate::util::cosine_similarity_raw(a, b)
}

// ============================================================================
// Directional metrics
// ============================================================================

/// Compute directional accuracy: fraction where forward_sim > reverse_sim.
pub fn directional_accuracy(forward_sims: &[f32], reverse_sims: &[f32]) -> f32 {
    if forward_sims.len() != reverse_sims.len() || forward_sims.is_empty() {
        return 0.0;
    }
    let correct = forward_sims
        .iter()
        .zip(reverse_sims.iter())
        .filter(|(f, r)| f > r)
        .count();
    correct as f32 / forward_sims.len() as f32
}

/// Compute direction ratio: mean(forward) / mean(reverse).
pub fn direction_ratio(forward_sims: &[f32], reverse_sims: &[f32]) -> f32 {
    if forward_sims.is_empty() || reverse_sims.is_empty() {
        return 0.0;
    }
    let mean_forward = forward_sims.iter().sum::<f32>() / forward_sims.len() as f32;
    let mean_reverse = reverse_sims.iter().sum::<f32>() / reverse_sims.len() as f32;
    if mean_reverse.abs() < 1e-8 {
        return f32::INFINITY;
    }
    mean_forward / mean_reverse
}

// ============================================================================
// Ablation metrics
// ============================================================================

/// Compute ablation delta: percentage change in accuracy when removing E5.
pub fn ablation_delta(accuracy_with_e5: f32, accuracy_without_e5: f32) -> f32 {
    if accuracy_without_e5.abs() < 1e-8 {
        return 0.0;
    }
    ((accuracy_with_e5 - accuracy_without_e5) / accuracy_without_e5 * 100.0).abs()
}

/// Compute per-embedder RRF contribution percentages.
pub fn rrf_contribution_breakdown(
    per_embedder_ranks: &[Vec<usize>],
    weights: &[f32],
) -> Vec<(usize, f32)> {
    if per_embedder_ranks.is_empty() || weights.is_empty() {
        return Vec::new();
    }

    let k = 60.0f32; // RRF constant
    let num_embedders = per_embedder_ranks.len();
    let num_docs = per_embedder_ranks.first().map(|r| r.len()).unwrap_or(0);

    if num_docs == 0 {
        return (0..num_embedders).map(|i| (i, 0.0)).collect();
    }

    // Sum RRF contribution per embedder
    let mut contributions = vec![0.0f32; num_embedders];
    let mut total = 0.0f32;

    for (emb_idx, ranks) in per_embedder_ranks.iter().enumerate() {
        let w = weights.get(emb_idx).copied().unwrap_or(0.0);
        for &rank in ranks {
            let rrf = w / (k + rank as f32);
            contributions[emb_idx] += rrf;
            total += rrf;
        }
    }

    // Normalize to percentages
    if total < 1e-8 {
        return (0..num_embedders).map(|i| (i, 0.0)).collect();
    }

    contributions
        .iter()
        .enumerate()
        .map(|(i, &c)| (i, c / total * 100.0))
        .collect()
}

// ============================================================================
// Causal gate metrics
// ============================================================================

/// Compute causal gate true positive rate and true negative rate.
///
/// Returns (TPR, TNR):
/// - TPR = fraction of causal texts correctly boosted (score >= threshold)
/// - TNR = fraction of non-causal texts correctly demoted (score < threshold)
pub fn causal_gate_tpr_tnr(
    e5_scores: &[f32],
    is_causal_labels: &[bool],
    threshold: f32,
) -> (f32, f32) {
    if e5_scores.len() != is_causal_labels.len() || e5_scores.is_empty() {
        return (0.0, 0.0);
    }

    let mut tp = 0usize;
    let mut total_pos = 0usize;
    let mut tn = 0usize;
    let mut total_neg = 0usize;

    for (score, &is_causal) in e5_scores.iter().zip(is_causal_labels.iter()) {
        if is_causal {
            total_pos += 1;
            if *score >= threshold {
                tp += 1;
            }
        } else {
            total_neg += 1;
            if *score < threshold {
                tn += 1;
            }
        }
    }

    let tpr = if total_pos > 0 {
        tp as f32 / total_pos as f32
    } else {
        0.0
    };
    let tnr = if total_neg > 0 {
        tn as f32 / total_neg as f32
    } else {
        0.0
    };

    (tpr, tnr)
}

// ============================================================================
// Query intent accuracy
// ============================================================================

/// Compute query intent detection accuracy and confusion matrix.
///
/// Classes: 0=cause, 1=effect, 2=unknown
pub fn query_intent_accuracy(predicted: &[&str], actual: &[&str]) -> (f32, ConfusionMatrix) {
    let mut cm = ConfusionMatrix::new();

    if predicted.len() != actual.len() || predicted.is_empty() {
        return (0.0, cm);
    }

    for (pred, act) in predicted.iter().zip(actual.iter()) {
        let pred_idx = direction_to_index(pred);
        let act_idx = direction_to_index(act);
        cm.record(pred_idx, act_idx);
    }

    (cm.accuracy() as f32, cm)
}

fn direction_to_index(dir: &str) -> usize {
    match dir.to_lowercase().as_str() {
        "cause" => 0,
        "effect" => 1,
        _ => 2,
    }
}

// ============================================================================
// Cross-domain generalization
// ============================================================================

/// Compute cross-domain gap: difference between train and held-out accuracy.
pub fn cross_domain_gap(train_accuracy: f32, held_out_accuracy: f32) -> f32 {
    (train_accuracy - held_out_accuracy).abs()
}

// ============================================================================
// Retrieval metrics (reimplemented without generic complexity)
// ============================================================================

/// Compute Mean Reciprocal Rank for string-keyed results.
pub fn retrieval_mrr(ranked_results: &[Vec<String>], expected_ids: &[String]) -> f32 {
    if ranked_results.len() != expected_ids.len() || ranked_results.is_empty() {
        return 0.0;
    }

    let mut mrr_sum = 0.0f32;
    for (results, expected) in ranked_results.iter().zip(expected_ids.iter()) {
        if let Some(pos) = results.iter().position(|r| r == expected) {
            mrr_sum += 1.0 / (pos + 1) as f32;
        }
    }

    mrr_sum / ranked_results.len() as f32
}

/// Compute NDCG@K for binary relevance.
pub fn retrieval_ndcg_at_k(ranked_results: &[String], expected_ids: &[String], k: usize) -> f32 {
    if k == 0 || ranked_results.is_empty() {
        return 0.0;
    }

    let relevant: std::collections::HashSet<&str> =
        expected_ids.iter().map(|s| s.as_str()).collect();

    // DCG@K
    let dcg: f64 = ranked_results
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, doc)| {
            let rel = if relevant.contains(doc.as_str()) {
                1.0
            } else {
                0.0
            };
            rel / (i as f64 + 2.0).log2()
        })
        .sum();

    // Ideal DCG@K
    let ideal_k = k.min(expected_ids.len());
    let idcg: f64 = (0..ideal_k).map(|i| 1.0 / (i as f64 + 2.0).log2()).sum();

    if idcg < f64::EPSILON {
        0.0
    } else {
        (dcg / idcg) as f32
    }
}

/// Compute top-1 accuracy: fraction of queries where expected_top1 is rank 1.
pub fn top1_accuracy(ranked_results: &[Vec<String>], expected_top1: &[String]) -> f32 {
    if ranked_results.len() != expected_top1.len() || ranked_results.is_empty() {
        return 0.0;
    }

    let correct = ranked_results
        .iter()
        .zip(expected_top1.iter())
        .filter(|(results, expected)| results.first().map(|r| r == *expected).unwrap_or(false))
        .count();

    correct as f32 / ranked_results.len() as f32
}

/// Compute top-K accuracy: fraction of queries where expected_top1 is in top K.
pub fn top_k_accuracy(ranked_results: &[Vec<String>], expected_top1: &[String], k: usize) -> f32 {
    if ranked_results.len() != expected_top1.len() || ranked_results.is_empty() {
        return 0.0;
    }

    let correct = ranked_results
        .iter()
        .zip(expected_top1.iter())
        .filter(|(results, expected)| results.iter().take(k).any(|r| r == *expected))
        .count();

    correct as f32 / ranked_results.len() as f32
}

// ============================================================================
// Phase result helpers
// ============================================================================

/// Create a phase result, computing pass/fail from metrics vs targets.
pub fn make_phase_result(
    phase: u8,
    phase_name: &str,
    metrics: HashMap<String, f64>,
    targets: HashMap<String, f64>,
    duration_ms: u64,
) -> PhaseBenchmarkResult {
    let mut failing_criteria = Vec::new();

    for (key, &target) in &targets {
        if let Some(&actual) = metrics.get(key) {
            // For metrics with "_max" suffix, check actual <= target
            // Otherwise check actual >= target
            let passes = if key.ends_with("_max")
                || key.contains("anisotropy")
                || key.contains("overhead")
                || key.contains("gap")
                || key.contains("fp")
            {
                actual <= target
            } else {
                actual >= target
            };
            if !passes {
                failing_criteria.push(format!(
                    "{}: {:.4} (target: {}{:.4})",
                    key,
                    actual,
                    if key.ends_with("_max")
                        || key.contains("anisotropy")
                        || key.contains("overhead")
                        || key.contains("gap")
                        || key.contains("fp")
                    {
                        "<="
                    } else {
                        ">="
                    },
                    target
                ));
            }
        }
    }

    PhaseBenchmarkResult {
        phase,
        phase_name: phase_name.to_string(),
        metrics,
        targets,
        pass: failing_criteria.is_empty(),
        failing_criteria,
        duration_ms,
        model_dependent: true,
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_score_spread() {
        let sims = vec![0.95, 0.90, 0.85, 0.80, 0.75, 0.70];
        assert!((score_spread(&sims) - 0.20).abs() < 0.01);
    }

    #[test]
    fn test_score_spread_few() {
        assert_eq!(score_spread(&[0.5]), 0.0);
        assert!((score_spread(&[0.9, 0.8]) - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 1e-6);
    }

    #[test]
    fn test_directional_accuracy() {
        let forward = vec![0.9, 0.8, 0.7, 0.6];
        let reverse = vec![0.7, 0.6, 0.8, 0.5]; // 3/4 correct
        assert!((directional_accuracy(&forward, &reverse) - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_direction_ratio() {
        let forward = vec![0.9, 0.8];
        let reverse = vec![0.6, 0.5];
        let ratio = direction_ratio(&forward, &reverse);
        // mean(0.9,0.8)=0.85, mean(0.6,0.5)=0.55, ratio=1.545
        assert!((ratio - 1.545).abs() < 0.01);
    }

    #[test]
    fn test_ablation_delta() {
        assert!((ablation_delta(0.80, 0.75) - 6.667).abs() < 0.1);
        assert!((ablation_delta(0.75, 0.75) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_causal_gate_tpr_tnr() {
        let scores = vec![0.96, 0.95, 0.91, 0.89];
        let labels = vec![true, true, false, false];
        let (tpr, tnr) = causal_gate_tpr_tnr(&scores, &labels, 0.94);
        assert!((tpr - 1.0).abs() < 0.01); // Both causal >= 0.94
        assert!((tnr - 1.0).abs() < 0.01); // Both non-causal < 0.94
    }

    #[test]
    fn test_confusion_matrix() {
        let mut cm = ConfusionMatrix::new();
        cm.record(0, 0); // cause predicted as cause
        cm.record(0, 0);
        cm.record(1, 1); // effect predicted as effect
        cm.record(0, 1); // effect predicted as cause (wrong)
        assert_eq!(cm.total(), 4);
        assert!((cm.accuracy() - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_query_intent_accuracy() {
        let predicted = vec!["cause", "cause", "effect", "unknown"];
        let actual = vec!["cause", "effect", "effect", "unknown"];
        let (acc, cm) = query_intent_accuracy(&predicted, &actual);
        assert!((acc - 0.75).abs() < 0.01);
        assert_eq!(cm.total(), 4);
    }

    #[test]
    fn test_retrieval_mrr() {
        let results = vec![
            vec!["a".into(), "b".into(), "c".into()],
            vec!["b".into(), "a".into(), "c".into()],
        ];
        let expected = vec!["a".to_string(), "a".to_string()];
        let mrr = retrieval_mrr(&results, &expected);
        // (1/1 + 1/2) / 2 = 0.75
        assert!((mrr - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_ndcg_at_k() {
        let results: Vec<String> = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        let expected: Vec<String> = vec!["a".into(), "c".into()];
        let ndcg = retrieval_ndcg_at_k(&results, &expected, 5);
        assert!(ndcg > 0.5); // a at rank 1, c at rank 3
    }

    #[test]
    fn test_top1_accuracy() {
        let results = vec![
            vec!["correct".into()],
            vec!["wrong".into()],
            vec!["correct".into()],
        ];
        let expected = vec!["correct".into(), "correct".into(), "correct".into()];
        let acc = top1_accuracy(&results, &expected);
        assert!((acc - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_make_phase_result_pass() {
        let mut metrics = HashMap::new();
        metrics.insert("accuracy".to_string(), 0.95);
        let mut targets = HashMap::new();
        targets.insert("accuracy".to_string(), 0.90);
        let result = make_phase_result(1, "Test", metrics, targets, 100);
        assert!(result.pass);
        assert!(result.failing_criteria.is_empty());
    }

    #[test]
    fn test_make_phase_result_fail() {
        let mut metrics = HashMap::new();
        metrics.insert("accuracy".to_string(), 0.80);
        let mut targets = HashMap::new();
        targets.insert("accuracy".to_string(), 0.90);
        let result = make_phase_result(1, "Test", metrics, targets, 100);
        assert!(!result.pass);
        assert_eq!(result.failing_criteria.len(), 1);
    }

    #[test]
    fn test_rrf_contribution() {
        let ranks = vec![
            vec![0, 1, 2], // embedder 0
            vec![2, 0, 1], // embedder 1
        ];
        let weights = vec![0.4, 0.1];
        let contribs = rrf_contribution_breakdown(&ranks, &weights);
        assert_eq!(contribs.len(), 2);
        // Embedder 0 with weight 0.4 should have higher contribution
        assert!(contribs[0].1 > contribs[1].1);
    }

    #[test]
    fn test_anisotropy_orthogonal() {
        // Orthogonal vectors → low anisotropy
        let vecs = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let aniso = anisotropy_measure(&vecs);
        assert!(
            aniso.abs() < 0.1,
            "Orthogonal vectors should have low anisotropy: {}",
            aniso
        );
    }
}
