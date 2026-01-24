//! E1 Semantic Embedder Metrics.
//!
//! This module provides metrics for evaluating the E1 semantic embedder
//! (intfloat/e5-large-v2, 1024D) as THE semantic foundation per ARCH-12.
//!
//! ## Key Metrics
//!
//! - **Retrieval**: P@K, R@K, MRR, NDCG, MAP (standard IR metrics)
//! - **Topic Separation**: Intra vs inter topic similarity ratio
//! - **Noise Robustness**: MRR degradation under noise
//! - **Ablation**: E1 alone vs E1+enhancers
//!
//! ## Targets (from CLAUDE.md)
//!
//! - E1 high similarity: > 0.75
//! - E1 low similarity (divergence): < 0.30
//! - Single embed latency: < 5ms P95
//! - Topic separation ratio: >= 1.5
//! - MRR: >= 0.70

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::metrics::retrieval::RetrievalMetrics;

// ============================================================================
// Core Metrics Structures
// ============================================================================

/// E1 Semantic Embedder Metrics.
///
/// Top-level structure containing all metrics for evaluating
/// the E1 semantic embedder as THE semantic foundation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E1SemanticMetrics {
    /// Standard retrieval quality metrics.
    pub retrieval: RetrievalMetrics,
    /// Topic separation metrics.
    pub topic_separation: TopicSeparationMetrics,
    /// Noise robustness metrics.
    pub noise_robustness: NoiseRobustnessMetrics,
    /// Domain coverage metrics.
    pub domain_coverage: DomainCoverageMetrics,
    /// Composite score.
    pub composite: CompositeE1Metrics,
}

impl E1SemanticMetrics {
    /// Check if all targets are met.
    pub fn all_targets_met(&self) -> bool {
        self.retrieval.mrr >= 0.70
            && self.retrieval.precision_at.get(&10).copied().unwrap_or(0.0) >= 0.60
            && self.topic_separation.separation_ratio >= 1.5
    }

    /// Get the overall quality score (0.0-1.0).
    pub fn overall_score(&self) -> f64 {
        self.composite.overall_score
    }
}

// ============================================================================
// Topic Separation Metrics
// ============================================================================

/// Topic separation metrics.
///
/// Measures how well E1 embeddings separate different topics.
/// Higher separation ratio indicates better topic discrimination.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TopicSeparationMetrics {
    /// Average E1 cosine similarity within same topic.
    pub intra_topic_similarity: f64,
    /// Average E1 cosine similarity across different topics.
    pub inter_topic_similarity: f64,
    /// Separation ratio: intra / inter (target: >= 1.5).
    ///
    /// A ratio of 1.5 means intra-topic similarity is 1.5x
    /// the inter-topic similarity, indicating good separation.
    pub separation_ratio: f64,
    /// Silhouette score for topic clustering (-1.0 to 1.0).
    pub silhouette_score: f64,
    /// Topic boundary detection F1 score.
    pub boundary_f1: f64,
    /// Number of topics evaluated.
    pub num_topics: usize,
    /// Number of document pairs evaluated.
    pub num_pairs_evaluated: usize,
    /// Similarity distribution within topics (mean, std).
    pub intra_distribution: (f64, f64),
    /// Similarity distribution across topics (mean, std).
    pub inter_distribution: (f64, f64),
}

impl TopicSeparationMetrics {
    /// Check if separation meets target.
    pub fn meets_target(&self) -> bool {
        self.separation_ratio >= 1.5
    }

    /// Get a normalized separation score (0.0-1.0).
    pub fn normalized_score(&self) -> f64 {
        // Normalize separation ratio: 1.0 -> 0.0, 1.5 -> 0.5, 2.0+ -> 1.0
        let separation_score = ((self.separation_ratio - 1.0) / 1.0).clamp(0.0, 1.0);
        // Silhouette is already -1 to 1, map to 0-1
        let silhouette_norm = (self.silhouette_score + 1.0) / 2.0;
        // Weighted average
        0.6 * separation_score + 0.25 * silhouette_norm + 0.15 * self.boundary_f1
    }
}

/// Compute topic separation metrics from E1 embeddings.
///
/// # Arguments
///
/// * `embeddings` - Map of document UUID to E1 embedding
/// * `topic_assignments` - Map of document UUID to topic ID
///
/// # Returns
///
/// Topic separation metrics including separation ratio.
pub fn compute_topic_separation<K: Eq + std::hash::Hash + Clone>(
    embeddings: &HashMap<K, Vec<f32>>,
    topic_assignments: &HashMap<K, usize>,
) -> TopicSeparationMetrics {
    if embeddings.is_empty() || topic_assignments.is_empty() {
        return TopicSeparationMetrics::default();
    }

    let mut intra_similarities: Vec<f64> = Vec::new();
    let mut inter_similarities: Vec<f64> = Vec::new();
    let mut pairs_evaluated = 0;

    // Collect IDs with both embeddings and topic assignments
    let ids: Vec<_> = embeddings
        .keys()
        .filter(|id| topic_assignments.contains_key(*id))
        .cloned()
        .collect();

    // Sample pairs for efficiency
    let max_pairs = 10000;
    let sample_step = if ids.len() * ids.len() / 2 > max_pairs {
        (ids.len() * ids.len() / 2 / max_pairs).max(1)
    } else {
        1
    };

    let mut pair_idx = 0;
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            pair_idx += 1;
            if pair_idx % sample_step != 0 {
                continue;
            }

            let id_a = &ids[i];
            let id_b = &ids[j];

            let (Some(emb_a), Some(emb_b)) = (embeddings.get(id_a), embeddings.get(id_b)) else {
                continue;
            };

            let sim = cosine_similarity(emb_a, emb_b);

            let (Some(topic_a), Some(topic_b)) =
                (topic_assignments.get(id_a), topic_assignments.get(id_b))
            else {
                continue;
            };

            pairs_evaluated += 1;

            if topic_a == topic_b {
                intra_similarities.push(sim);
            } else {
                inter_similarities.push(sim);
            }
        }
    }

    // Compute metrics
    let (intra_mean, intra_std) = compute_mean_std(&intra_similarities);
    let (inter_mean, inter_std) = compute_mean_std(&inter_similarities);

    let separation_ratio = if inter_mean > 0.001 {
        intra_mean / inter_mean
    } else if intra_mean > 0.0 {
        10.0 // Cap at 10x if inter is near zero
    } else {
        1.0
    };

    // Compute silhouette score
    let silhouette_score = compute_silhouette_for_topics(embeddings, topic_assignments);

    // Compute boundary F1
    let boundary_f1 = compute_topic_boundary_f1(&intra_similarities, &inter_similarities);

    let topics: std::collections::HashSet<_> = topic_assignments.values().collect();

    TopicSeparationMetrics {
        intra_topic_similarity: intra_mean,
        inter_topic_similarity: inter_mean,
        separation_ratio,
        silhouette_score,
        boundary_f1,
        num_topics: topics.len(),
        num_pairs_evaluated: pairs_evaluated,
        intra_distribution: (intra_mean, intra_std),
        inter_distribution: (inter_mean, inter_std),
    }
}

// ============================================================================
// Noise Robustness Metrics
// ============================================================================

/// Noise robustness metrics.
///
/// Measures how well E1 retrieval quality degrades under noise.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NoiseRobustnessMetrics {
    /// MRR degradation curve: noise_level -> MRR.
    pub mrr_degradation: Vec<(f64, f64)>,
    /// P@10 degradation curve.
    pub precision_degradation: Vec<(f64, f64)>,
    /// Noise level at which MRR drops below 0.5.
    pub robustness_threshold: Option<f64>,
    /// Average relative degradation across noise levels.
    pub avg_relative_degradation: f64,
    /// Whether robustness target is met (MRR >= 0.55 at 0.2 noise).
    pub meets_robustness_target: bool,
}

impl NoiseRobustnessMetrics {
    /// Get MRR at a specific noise level.
    pub fn mrr_at_noise(&self, noise: f64) -> Option<f64> {
        self.mrr_degradation
            .iter()
            .find(|(n, _)| (*n - noise).abs() < 0.01)
            .map(|(_, mrr)| *mrr)
    }

    /// Get a normalized robustness score (0.0-1.0).
    pub fn normalized_score(&self) -> f64 {
        // Score based on MRR retention at 0.2 noise
        if let Some(mrr_at_02) = self.mrr_at_noise(0.2) {
            let baseline_mrr = self.mrr_at_noise(0.0).unwrap_or(1.0);
            if baseline_mrr > 0.0 {
                (mrr_at_02 / baseline_mrr).clamp(0.0, 1.0)
            } else {
                0.0
            }
        } else {
            0.5 // Default if not measured
        }
    }
}

/// Compute noise robustness by measuring MRR at different noise levels.
///
/// # Arguments
///
/// * `baseline_mrr` - MRR at noise level 0.0
/// * `noisy_results` - Map of noise_level -> MRR
///
/// # Returns
///
/// Noise robustness metrics.
pub fn compute_noise_robustness(
    baseline_mrr: f64,
    noisy_results: &HashMap<String, f64>,
) -> NoiseRobustnessMetrics {
    let mut mrr_degradation = Vec::new();
    mrr_degradation.push((0.0, baseline_mrr));

    let mut precision_degradation = Vec::new();

    for (noise_str, mrr) in noisy_results {
        if let Ok(noise) = noise_str.parse::<f64>() {
            mrr_degradation.push((noise, *mrr));
        }
    }

    mrr_degradation.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Find robustness threshold (where MRR drops below 0.5)
    let robustness_threshold = mrr_degradation
        .iter()
        .find(|(_, mrr)| *mrr < 0.5)
        .map(|(noise, _)| *noise);

    // Compute average relative degradation
    let avg_relative_degradation = if baseline_mrr > 0.0 && mrr_degradation.len() > 1 {
        let degradations: Vec<f64> = mrr_degradation
            .iter()
            .skip(1) // Skip baseline
            .map(|(_, mrr)| (baseline_mrr - mrr) / baseline_mrr)
            .collect();
        degradations.iter().sum::<f64>() / degradations.len() as f64
    } else {
        0.0
    };

    // Check if meets target: MRR >= 0.55 at 0.2 noise
    let meets_robustness_target = mrr_degradation
        .iter()
        .find(|(n, _)| (*n - 0.2).abs() < 0.01)
        .map(|(_, mrr)| *mrr >= 0.55)
        .unwrap_or(true); // Default to true if not tested

    NoiseRobustnessMetrics {
        mrr_degradation,
        precision_degradation,
        robustness_threshold,
        avg_relative_degradation,
        meets_robustness_target,
    }
}

// ============================================================================
// Domain Coverage Metrics
// ============================================================================

/// Domain coverage metrics.
///
/// Measures how well E1 performs across different semantic domains.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DomainCoverageMetrics {
    /// Retrieval metrics per domain.
    pub per_domain_metrics: HashMap<String, RetrievalMetrics>,
    /// Overall variance across domains.
    pub mrr_variance: f64,
    /// Worst performing domain.
    pub worst_domain: Option<String>,
    /// Best performing domain.
    pub best_domain: Option<String>,
    /// Whether all domains meet minimum threshold.
    pub all_domains_pass: bool,
}

impl DomainCoverageMetrics {
    /// Get a normalized coverage score (0.0-1.0).
    pub fn normalized_score(&self) -> f64 {
        if self.per_domain_metrics.is_empty() {
            return 0.0;
        }

        let mrrs: Vec<f64> = self
            .per_domain_metrics
            .values()
            .map(|m| m.mrr)
            .collect();

        let avg_mrr = mrrs.iter().sum::<f64>() / mrrs.len() as f64;

        // Score based on average MRR and low variance
        let variance_penalty = (1.0 - self.mrr_variance.sqrt()).clamp(0.0, 1.0);

        0.7 * avg_mrr + 0.3 * variance_penalty
    }
}

// ============================================================================
// Ablation Metrics
// ============================================================================

/// E1 ablation metrics.
///
/// Compares E1 standalone vs E1 with enhancement embedders.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E1AblationMetrics {
    /// E1 standalone MRR.
    pub e1_only_mrr: f64,
    /// E1 + E5 (causal) MRR.
    pub e1_e5_mrr: Option<f64>,
    /// E1 + E7 (code) MRR.
    pub e1_e7_mrr: Option<f64>,
    /// E1 + all semantic enhancers MRR.
    pub e1_all_semantic_mrr: Option<f64>,
    /// E1 in full multi-space MRR.
    pub full_multispace_mrr: Option<f64>,
    /// E1 provides best foundation confirmation.
    pub e1_is_best_foundation: bool,
    /// Enhancement improvement percentages.
    pub enhancements: HashMap<String, f64>,
}

impl E1AblationMetrics {
    /// Get a normalized ablation score (0.0-1.0).
    pub fn normalized_score(&self) -> f64 {
        // E1 foundation quality is primary
        let e1_score = self.e1_only_mrr;

        // Bonus for good enhancement effects
        let enhancement_bonus = if self.e1_is_best_foundation { 0.1 } else { 0.0 };

        (e1_score + enhancement_bonus).clamp(0.0, 1.0)
    }
}

// ============================================================================
// Composite Metrics
// ============================================================================

/// Composite E1 Metrics.
///
/// Aggregated scores combining retrieval, separation, and robustness.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompositeE1Metrics {
    /// Overall score (0.0-1.0).
    ///
    /// Weighted combination:
    /// - 40% retrieval quality
    /// - 30% topic separation
    /// - 20% noise robustness
    /// - 10% domain coverage
    pub overall_score: f64,
    /// Retrieval component score.
    pub retrieval_score: f64,
    /// Separation component score.
    pub separation_score: f64,
    /// Robustness component score.
    pub robustness_score: f64,
    /// Coverage component score.
    pub coverage_score: f64,
    /// All validation targets met.
    pub all_targets_met: bool,
    /// Number of targets met out of total.
    pub targets_met_count: usize,
    /// Total number of targets.
    pub targets_total: usize,
}

impl CompositeE1Metrics {
    /// Compute composite metrics from components.
    pub fn compute(
        retrieval: &RetrievalMetrics,
        topic_separation: &TopicSeparationMetrics,
        noise_robustness: &NoiseRobustnessMetrics,
        domain_coverage: &DomainCoverageMetrics,
    ) -> Self {
        let retrieval_score = retrieval.overall_score();
        let separation_score = topic_separation.normalized_score();
        let robustness_score = noise_robustness.normalized_score();
        let coverage_score = domain_coverage.normalized_score();

        let overall_score = 0.4 * retrieval_score
            + 0.3 * separation_score
            + 0.2 * robustness_score
            + 0.1 * coverage_score;

        // Count targets met
        let targets = [
            retrieval.mrr >= 0.70,
            retrieval.precision_at.get(&10).copied().unwrap_or(0.0) >= 0.60,
            topic_separation.separation_ratio >= 1.5,
            noise_robustness.meets_robustness_target,
        ];
        let targets_met_count = targets.iter().filter(|&&t| t).count();
        let targets_total = targets.len();
        let all_targets_met = targets_met_count == targets_total;

        Self {
            overall_score,
            retrieval_score,
            separation_score,
            robustness_score,
            coverage_score,
            all_targets_met,
            targets_met_count,
            targets_total,
        }
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Compute cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

/// Compute mean and standard deviation.
fn compute_mean_std(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }

    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;
    let std = variance.sqrt();

    (mean, std)
}

/// Compute silhouette score for topic-based clustering.
fn compute_silhouette_for_topics<K: Eq + std::hash::Hash + Clone>(
    embeddings: &HashMap<K, Vec<f32>>,
    topic_assignments: &HashMap<K, usize>,
) -> f64 {
    // Group embeddings by topic
    let mut topic_embeddings: HashMap<usize, Vec<(&K, &Vec<f32>)>> = HashMap::new();
    for (id, emb) in embeddings {
        if let Some(&topic) = topic_assignments.get(id) {
            topic_embeddings.entry(topic).or_default().push((id, emb));
        }
    }

    if topic_embeddings.len() < 2 {
        return 0.0;
    }

    let mut silhouette_sum = 0.0;
    let mut count = 0;

    for (topic_id, members) in &topic_embeddings {
        if members.len() < 2 {
            continue;
        }

        for (id, emb) in members {
            // a(i) = average distance to other points in same cluster
            let a_i: f64 = members
                .iter()
                .filter(|(other_id, _)| *other_id != *id)
                .map(|(_, other_emb)| 1.0 - cosine_similarity(emb, other_emb))
                .sum::<f64>()
                / (members.len() - 1) as f64;

            // b(i) = minimum average distance to points in other clusters
            let b_i: f64 = topic_embeddings
                .iter()
                .filter(|(tid, _)| *tid != topic_id)
                .filter(|(_, other_members)| !other_members.is_empty())
                .map(|(_, other_members)| {
                    other_members
                        .iter()
                        .map(|(_, other_emb)| 1.0 - cosine_similarity(emb, other_emb))
                        .sum::<f64>()
                        / other_members.len() as f64
                })
                .fold(f64::INFINITY, f64::min);

            if b_i.is_finite() {
                let s_i = if a_i < b_i {
                    1.0 - a_i / b_i
                } else if a_i > b_i {
                    b_i / a_i - 1.0
                } else {
                    0.0
                };

                silhouette_sum += s_i;
                count += 1;
            }
        }
    }

    if count > 0 {
        silhouette_sum / count as f64
    } else {
        0.0
    }
}

/// Compute topic boundary F1 score.
fn compute_topic_boundary_f1(intra_sims: &[f64], inter_sims: &[f64]) -> f64 {
    if intra_sims.is_empty() || inter_sims.is_empty() {
        return 0.0;
    }

    // Use midpoint of means as threshold
    let intra_mean: f64 = intra_sims.iter().sum::<f64>() / intra_sims.len() as f64;
    let inter_mean: f64 = inter_sims.iter().sum::<f64>() / inter_sims.len() as f64;
    let threshold = (intra_mean + inter_mean) / 2.0;

    // Compute F1 with this threshold
    let tp = intra_sims.iter().filter(|&&s| s >= threshold).count() as f64;
    let fp = inter_sims.iter().filter(|&&s| s >= threshold).count() as f64;
    let fn_ = intra_sims.iter().filter(|&&s| s < threshold).count() as f64;

    let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 0.0 };
    let recall = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 0.0 };

    if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    }
}

/// Compute full E1 semantic metrics.
///
/// # Arguments
///
/// * `retrieval` - Standard retrieval metrics
/// * `topic_separation` - Topic separation metrics
/// * `noise_robustness` - Noise robustness metrics
/// * `domain_coverage` - Domain coverage metrics
///
/// # Returns
///
/// Complete E1 semantic metrics.
pub fn compute_e1_semantic_metrics(
    retrieval: RetrievalMetrics,
    topic_separation: TopicSeparationMetrics,
    noise_robustness: NoiseRobustnessMetrics,
    domain_coverage: DomainCoverageMetrics,
) -> E1SemanticMetrics {
    let composite =
        CompositeE1Metrics::compute(&retrieval, &topic_separation, &noise_robustness, &domain_coverage);

    E1SemanticMetrics {
        retrieval,
        topic_separation,
        noise_robustness,
        domain_coverage,
        composite,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_topic_separation_metrics_default() {
        let metrics = TopicSeparationMetrics::default();
        assert_eq!(metrics.separation_ratio, 0.0);
        assert!(!metrics.meets_target());
    }

    #[test]
    fn test_compute_mean_std() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (mean, std) = compute_mean_std(&values);
        assert!((mean - 3.0).abs() < 0.001);
        assert!((std - 1.414).abs() < 0.01);
    }

    #[test]
    fn test_composite_metrics_computation() {
        let retrieval = RetrievalMetrics {
            mrr: 0.75,
            precision_at: [(10, 0.65)].into_iter().collect(),
            recall_at: [(10, 0.70)].into_iter().collect(),
            ndcg_at: [(10, 0.72)].into_iter().collect(),
            map: 0.68,
            query_count: 100,
        };

        let topic_separation = TopicSeparationMetrics {
            separation_ratio: 1.8,
            silhouette_score: 0.4,
            boundary_f1: 0.75,
            ..Default::default()
        };

        let noise_robustness = NoiseRobustnessMetrics {
            mrr_degradation: vec![(0.0, 0.75), (0.2, 0.60)],
            meets_robustness_target: true,
            ..Default::default()
        };

        let domain_coverage = DomainCoverageMetrics::default();

        let composite = CompositeE1Metrics::compute(
            &retrieval,
            &topic_separation,
            &noise_robustness,
            &domain_coverage,
        );

        assert!(composite.overall_score > 0.0);
        assert!(composite.overall_score <= 1.0);
        assert!(composite.all_targets_met);
    }

    #[test]
    fn test_topic_separation_basic() {
        let mut embeddings: HashMap<Uuid, Vec<f32>> = HashMap::new();
        let mut topic_assignments: HashMap<Uuid, usize> = HashMap::new();

        // Create two distinct topics with well-separated embeddings
        for i in 0..5 {
            let id = Uuid::new_v4();
            let mut emb = vec![0.0; 64];
            emb[0] = 1.0;
            emb[1] = 0.1 * i as f32;
            embeddings.insert(id, emb);
            topic_assignments.insert(id, 0);
        }

        for i in 0..5 {
            let id = Uuid::new_v4();
            let mut emb = vec![0.0; 64];
            emb[32] = 1.0;
            emb[33] = 0.1 * i as f32;
            embeddings.insert(id, emb);
            topic_assignments.insert(id, 1);
        }

        let metrics = compute_topic_separation(&embeddings, &topic_assignments);

        assert_eq!(metrics.num_topics, 2);
        assert!(metrics.intra_topic_similarity > metrics.inter_topic_similarity);
    }
}
