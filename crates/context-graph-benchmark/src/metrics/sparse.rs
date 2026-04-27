//! E6 Sparse benchmark metrics for evaluating keyword precision.
//!
//! This module provides comprehensive metrics for evaluating E6 sparse embedder:
//!
//! - **Keyword Precision**: Exact match accuracy, anti-match avoidance
//! - **Retrieval Quality**: MRR, P@K, R@K, NDCG
//! - **Sparsity Analysis**: Active terms, sparsity ratio, weight distribution
//! - **Ablation**: E6 contribution vs baseline
//!
//! ## Key Concepts
//!
//! - **Sparsity Ratio**: Proportion of zero values (~0.95 expected)
//! - **Active Terms**: Number of non-zero entries (~1500 expected)
//! - **Keyword Hit Rate**: Whether target keyword appears in top-k

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// E6 Sparse metrics for evaluating keyword precision.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E6SparseMetrics {
    /// Keyword precision metrics.
    pub keyword_precision: KeywordPrecisionMetrics,

    /// Retrieval quality metrics.
    pub retrieval_quality: RetrievalQualityMetrics,

    /// Sparsity analysis metrics.
    pub sparsity_analysis: SparsityAnalysisMetrics,

    /// Ablation metrics (E6 contribution).
    pub ablation: E6AblationMetrics,

    /// Number of queries used.
    pub query_count: usize,
}

/// Keyword precision metrics: exact term matching effectiveness.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeywordPrecisionMetrics {
    /// Exact match accuracy: target keyword doc is top-1.
    pub exact_match_accuracy: f64,

    /// Anti-match avoidance: anti-expected docs not in top-3.
    pub anti_match_avoidance: f64,

    /// MRR at various K values.
    pub mrr_at_k: HashMap<usize, f64>,

    /// Keyword hit rate: expected doc appears in top-k.
    pub keyword_hit_rate_at_k: HashMap<usize, f64>,

    /// Per-domain accuracy breakdown.
    pub accuracy_by_domain: HashMap<String, f64>,

    /// Per-keyword accuracy breakdown.
    pub accuracy_by_keyword: HashMap<String, f64>,
}

/// Retrieval quality metrics: standard IR metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalQualityMetrics {
    /// Precision at various K values.
    pub precision_at_k: HashMap<usize, f64>,

    /// Recall at various K values.
    pub recall_at_k: HashMap<usize, f64>,

    /// Mean Reciprocal Rank overall.
    pub mrr: f64,

    /// Normalized Discounted Cumulative Gain at 10.
    pub ndcg_at_10: f64,

    /// Mean Average Precision.
    pub map: f64,
}

/// Sparsity analysis metrics: E6 vector characteristics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SparsityAnalysisMetrics {
    /// Average number of active (non-zero) terms.
    pub avg_active_terms: f64,

    /// Minimum active terms observed.
    pub min_active_terms: usize,

    /// Maximum active terms observed.
    pub max_active_terms: usize,

    /// Sparsity ratio (proportion of zeros).
    pub sparsity_ratio: f64,

    /// Vocabulary size (max dimension).
    pub vocabulary_size: usize,

    /// Average weight of active terms.
    pub avg_term_weight: f64,

    /// Max weight observed.
    pub max_term_weight: f32,

    /// Weight distribution histogram (buckets).
    pub weight_distribution: Vec<usize>,

    /// Percentage of weights in each bucket.
    pub weight_percentages: Vec<f64>,
}

/// E6 ablation metrics: contribution analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E6AblationMetrics {
    /// Score with E6 enabled (multi-space).
    pub score_with_e6: f64,

    /// Score without E6 (E1 only or multi-space minus E6).
    pub score_without_e6: f64,

    /// Delta (improvement from E6).
    pub delta: f64,

    /// Impact percentage.
    pub impact_percent: f64,

    /// Is E6 essential (delta > 5%)?
    pub is_essential: bool,

    /// Per-query deltas.
    pub per_query_deltas: Vec<f64>,

    /// Queries where E6 helped most.
    pub top_improvement_queries: Vec<String>,

    /// Queries where E6 hurt.
    pub hurt_queries: Vec<String>,
}

impl E6SparseMetrics {
    /// Overall quality score (weighted combination).
    pub fn quality_score(&self) -> f64 {
        0.40 * self.keyword_precision.overall_score()
            + 0.30 * self.retrieval_quality.overall_score()
            + 0.20 * self.sparsity_analysis.health_score()
            + 0.10 * self.ablation.contribution_score()
    }

    /// Check if metrics meet minimum thresholds.
    pub fn meets_targets(&self) -> bool {
        // E6 MRR@10 >= 0.50 on keyword queries
        let mrr_ok = self
            .keyword_precision
            .mrr_at_k
            .get(&10)
            .copied()
            .unwrap_or(0.0)
            >= 0.50;

        // E6 vs E1 delta: E6 > E1 on exact keyword queries
        let ablation_ok = self.ablation.delta > 0.0;

        // Ablation impact > 5% (essential embedder)
        let essential_ok = self.ablation.impact_percent > 5.0;

        // Sparsity ratio > 0.95
        let sparsity_ok = self.sparsity_analysis.sparsity_ratio > 0.95;

        mrr_ok && ablation_ok && essential_ok && sparsity_ok
    }
}

impl KeywordPrecisionMetrics {
    /// Overall keyword precision score.
    pub fn overall_score(&self) -> f64 {
        let mrr_10 = self.mrr_at_k.get(&10).copied().unwrap_or(0.0);
        0.4 * self.exact_match_accuracy + 0.3 * mrr_10 + 0.3 * self.anti_match_avoidance
    }
}

impl RetrievalQualityMetrics {
    /// Overall retrieval score.
    pub fn overall_score(&self) -> f64 {
        let p10 = self.precision_at_k.get(&10).copied().unwrap_or(0.0);
        let r10 = self.recall_at_k.get(&10).copied().unwrap_or(0.0);
        0.3 * self.mrr + 0.25 * p10 + 0.25 * r10 + 0.2 * self.ndcg_at_10
    }
}

impl SparsityAnalysisMetrics {
    /// Sparsity health score (how well it matches expectations).
    pub fn health_score(&self) -> f64 {
        // Expected: ~1500 active terms, ~0.95 sparsity
        let active_terms_score =
            if self.avg_active_terms >= 500.0 && self.avg_active_terms <= 3000.0 {
                1.0
            } else {
                0.5
            };

        let sparsity_score = if self.sparsity_ratio > 0.90 { 1.0 } else { 0.5 };

        (active_terms_score + sparsity_score) / 2.0
    }
}

impl E6AblationMetrics {
    /// Contribution score (normalized).
    pub fn contribution_score(&self) -> f64 {
        // Clamp impact to [0, 1] for scoring
        (self.impact_percent / 100.0).clamp(0.0, 1.0)
    }
}

// =============================================================================
// METRIC COMPUTATION FUNCTIONS
// =============================================================================

/// Result from a keyword precision query.
#[derive(Debug, Clone)]
pub struct KeywordQueryResult {
    /// Query ID.
    pub query_id: uuid::Uuid,

    /// Query keyword being tested.
    pub keyword: String,

    /// Domain of the query.
    pub domain: String,

    /// Ranked document IDs from retrieval.
    pub ranked_docs: Vec<uuid::Uuid>,

    /// Relevant document IDs (ground truth).
    pub relevant_docs: Vec<uuid::Uuid>,

    /// Anti-relevant document IDs (should not appear high).
    pub anti_relevant_docs: Vec<uuid::Uuid>,

    /// Scores for ranked documents.
    pub scores: Vec<f32>,
}

/// Compute keyword precision metrics from query results.
pub fn compute_keyword_precision_metrics(
    results: &[KeywordQueryResult],
    k_values: &[usize],
) -> KeywordPrecisionMetrics {
    if results.is_empty() {
        return KeywordPrecisionMetrics::default();
    }

    let mut exact_match_count = 0;
    let mut anti_match_avoidance_count = 0;
    let mut total_queries = 0;
    let mut mrr_sums: HashMap<usize, f64> = HashMap::new();
    let mut hit_counts: HashMap<usize, usize> = HashMap::new();
    let mut domain_correct: HashMap<String, usize> = HashMap::new();
    let mut domain_total: HashMap<String, usize> = HashMap::new();
    let mut keyword_correct: HashMap<String, usize> = HashMap::new();
    let mut keyword_total: HashMap<String, usize> = HashMap::new();

    for k in k_values {
        mrr_sums.insert(*k, 0.0);
        hit_counts.insert(*k, 0);
    }

    for result in results {
        total_queries += 1;

        // Exact match: first relevant doc is at position 0
        if let Some(first_relevant) = result.relevant_docs.first() {
            if result.ranked_docs.first() == Some(first_relevant) {
                exact_match_count += 1;
                *domain_correct.entry(result.domain.clone()).or_default() += 1;
                *keyword_correct.entry(result.keyword.clone()).or_default() += 1;
            }
        }

        *domain_total.entry(result.domain.clone()).or_default() += 1;
        *keyword_total.entry(result.keyword.clone()).or_default() += 1;

        // Anti-match avoidance: no anti-relevant docs in top 3
        let top_3: Vec<_> = result.ranked_docs.iter().take(3).collect();
        let has_anti = result.anti_relevant_docs.iter().any(|d| top_3.contains(&d));
        if !has_anti {
            anti_match_avoidance_count += 1;
        }

        // MRR at various K
        for &k in k_values {
            if let Some(pos) = result
                .ranked_docs
                .iter()
                .take(k)
                .position(|d| result.relevant_docs.contains(d))
            {
                *mrr_sums.get_mut(&k).unwrap() += 1.0 / (pos + 1) as f64;
                *hit_counts.get_mut(&k).unwrap() += 1;
            }
        }
    }

    let exact_match_accuracy = exact_match_count as f64 / total_queries as f64;
    let anti_match_avoidance = anti_match_avoidance_count as f64 / total_queries as f64;

    let mrr_at_k: HashMap<usize, f64> = k_values
        .iter()
        .map(|&k| (k, mrr_sums[&k] / total_queries as f64))
        .collect();

    let keyword_hit_rate_at_k: HashMap<usize, f64> = k_values
        .iter()
        .map(|&k| (k, hit_counts[&k] as f64 / total_queries as f64))
        .collect();

    let accuracy_by_domain: HashMap<String, f64> = domain_total
        .iter()
        .map(|(d, &total)| {
            let correct = domain_correct.get(d).copied().unwrap_or(0);
            (d.clone(), correct as f64 / total as f64)
        })
        .collect();

    let accuracy_by_keyword: HashMap<String, f64> = keyword_total
        .iter()
        .map(|(k, &total)| {
            let correct = keyword_correct.get(k).copied().unwrap_or(0);
            (k.clone(), correct as f64 / total as f64)
        })
        .collect();

    KeywordPrecisionMetrics {
        exact_match_accuracy,
        anti_match_avoidance,
        mrr_at_k,
        keyword_hit_rate_at_k,
        accuracy_by_domain,
        accuracy_by_keyword,
    }
}

/// Compute retrieval quality metrics.
pub fn compute_retrieval_quality_metrics(
    results: &[KeywordQueryResult],
    k_values: &[usize],
) -> RetrievalQualityMetrics {
    if results.is_empty() {
        return RetrievalQualityMetrics::default();
    }

    let mut precision_sums: HashMap<usize, f64> = HashMap::new();
    let mut recall_sums: HashMap<usize, f64> = HashMap::new();
    let mut mrr_sum = 0.0;
    let mut ndcg_sum = 0.0;
    let mut ap_sum = 0.0;

    for k in k_values {
        precision_sums.insert(*k, 0.0);
        recall_sums.insert(*k, 0.0);
    }

    for result in results {
        if result.relevant_docs.is_empty() {
            continue;
        }

        let relevant_set: std::collections::HashSet<_> =
            result.relevant_docs.iter().cloned().collect();

        // MRR
        if let Some(pos) = result
            .ranked_docs
            .iter()
            .position(|d| relevant_set.contains(d))
        {
            mrr_sum += 1.0 / (pos + 1) as f64;
        }

        // P@K and R@K
        for &k in k_values {
            let top_k: Vec<_> = result.ranked_docs.iter().take(k).collect();
            let relevant_in_k = top_k.iter().filter(|d| relevant_set.contains(d)).count();
            *precision_sums.get_mut(&k).unwrap() += relevant_in_k as f64 / k as f64;
            *recall_sums.get_mut(&k).unwrap() +=
                relevant_in_k as f64 / result.relevant_docs.len() as f64;
        }

        // NDCG@10
        let dcg = compute_dcg(&result.ranked_docs, &relevant_set, 10);
        let ideal_dcg = compute_ideal_dcg(result.relevant_docs.len(), 10);
        if ideal_dcg > 0.0 {
            ndcg_sum += dcg / ideal_dcg;
        }

        // Average Precision
        ap_sum += compute_average_precision(&result.ranked_docs, &relevant_set);
    }

    let n = results.len() as f64;

    RetrievalQualityMetrics {
        precision_at_k: precision_sums
            .into_iter()
            .map(|(k, sum)| (k, sum / n))
            .collect(),
        recall_at_k: recall_sums
            .into_iter()
            .map(|(k, sum)| (k, sum / n))
            .collect(),
        mrr: mrr_sum / n,
        ndcg_at_10: ndcg_sum / n,
        map: ap_sum / n,
    }
}

/// Compute DCG for a ranked list.
fn compute_dcg(
    ranked: &[uuid::Uuid],
    relevant: &std::collections::HashSet<uuid::Uuid>,
    k: usize,
) -> f64 {
    ranked
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, doc)| {
            let rel = if relevant.contains(doc) { 1.0 } else { 0.0 };
            rel / (i as f64 + 2.0).log2()
        })
        .sum()
}

/// Compute ideal DCG for n relevant documents.
fn compute_ideal_dcg(n_relevant: usize, k: usize) -> f64 {
    (0..n_relevant.min(k))
        .map(|i| 1.0 / (i as f64 + 2.0).log2())
        .sum()
}

/// Compute Average Precision for a ranked list.
fn compute_average_precision(
    ranked: &[uuid::Uuid],
    relevant: &std::collections::HashSet<uuid::Uuid>,
) -> f64 {
    let mut sum = 0.0;
    let mut relevant_count = 0;

    for (i, doc) in ranked.iter().enumerate() {
        if relevant.contains(doc) {
            relevant_count += 1;
            sum += relevant_count as f64 / (i + 1) as f64;
        }
    }

    if relevant.is_empty() {
        0.0
    } else {
        sum / relevant.len() as f64
    }
}

/// Sparse vector for sparsity analysis.
#[derive(Debug, Clone)]
pub struct SparseVectorStats {
    /// Number of active (non-zero) terms.
    pub active_terms: usize,

    /// Total dimension (vocabulary size).
    pub total_dimension: usize,

    /// Term weights (non-zero values).
    pub weights: Vec<f32>,
}

/// Compute sparsity analysis metrics from sparse vectors.
pub fn compute_sparsity_analysis_metrics(vectors: &[SparseVectorStats]) -> SparsityAnalysisMetrics {
    if vectors.is_empty() {
        return SparsityAnalysisMetrics::default();
    }

    let active_terms: Vec<usize> = vectors.iter().map(|v| v.active_terms).collect();
    let avg_active_terms = active_terms.iter().sum::<usize>() as f64 / vectors.len() as f64;
    let min_active_terms = *active_terms.iter().min().unwrap_or(&0);
    let max_active_terms = *active_terms.iter().max().unwrap_or(&0);

    let vocabulary_size = vectors.iter().map(|v| v.total_dimension).max().unwrap_or(0);

    let total_zeros: usize = vectors
        .iter()
        .map(|v| v.total_dimension - v.active_terms)
        .sum();
    let total_elements: usize = vectors.iter().map(|v| v.total_dimension).sum();
    let sparsity_ratio = total_zeros as f64 / total_elements as f64;

    // Weight statistics
    let all_weights: Vec<f32> = vectors
        .iter()
        .flat_map(|v| v.weights.iter().copied())
        .collect();
    let avg_term_weight = if all_weights.is_empty() {
        0.0
    } else {
        all_weights.iter().map(|&w| w as f64).sum::<f64>() / all_weights.len() as f64
    };
    let max_term_weight = all_weights
        .iter()
        .copied()
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.0);

    // Weight distribution histogram (10 buckets)
    let bucket_count = 10;
    let mut weight_distribution = vec![0usize; bucket_count];
    let max_w = max_term_weight.max(1.0);

    for &w in &all_weights {
        let bucket = ((w / max_w) * (bucket_count - 1) as f32).round() as usize;
        let bucket = bucket.min(bucket_count - 1);
        weight_distribution[bucket] += 1;
    }

    let total_weights = all_weights.len().max(1) as f64;
    let weight_percentages: Vec<f64> = weight_distribution
        .iter()
        .map(|&count| count as f64 / total_weights * 100.0)
        .collect();

    SparsityAnalysisMetrics {
        avg_active_terms,
        min_active_terms,
        max_active_terms,
        sparsity_ratio,
        vocabulary_size,
        avg_term_weight,
        max_term_weight,
        weight_distribution,
        weight_percentages,
    }
}

/// Compute E6 ablation metrics.
pub fn compute_e6_ablation_metrics(
    with_e6_scores: &[f64],
    without_e6_scores: &[f64],
    query_descriptions: &[String],
) -> E6AblationMetrics {
    if with_e6_scores.is_empty() || with_e6_scores.len() != without_e6_scores.len() {
        return E6AblationMetrics::default();
    }

    let n = with_e6_scores.len();
    let score_with_e6 = with_e6_scores.iter().sum::<f64>() / n as f64;
    let score_without_e6 = without_e6_scores.iter().sum::<f64>() / n as f64;
    let delta = score_with_e6 - score_without_e6;

    let impact_percent = if score_without_e6 > 0.0 {
        (delta / score_without_e6) * 100.0
    } else {
        0.0
    };

    let per_query_deltas: Vec<f64> = with_e6_scores
        .iter()
        .zip(without_e6_scores.iter())
        .map(|(w, wo)| w - wo)
        .collect();

    // Find queries where E6 helped most
    let mut improvements: Vec<(usize, f64)> = per_query_deltas
        .iter()
        .enumerate()
        .map(|(i, &d)| (i, d))
        .collect();
    improvements.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let top_improvement_queries: Vec<String> = improvements
        .iter()
        .take(5)
        .filter(|(_, d)| *d > 0.0)
        .filter_map(|(i, _)| query_descriptions.get(*i).cloned())
        .collect();

    let hurt_queries: Vec<String> = improvements
        .iter()
        .rev()
        .take(5)
        .filter(|(_, d)| *d < 0.0)
        .filter_map(|(i, _)| query_descriptions.get(*i).cloned())
        .collect();

    E6AblationMetrics {
        score_with_e6,
        score_without_e6,
        delta,
        impact_percent,
        is_essential: impact_percent > 5.0,
        per_query_deltas,
        top_improvement_queries,
        hurt_queries,
    }
}

/// Compute all E6 sparse metrics.
pub fn compute_all_sparse_metrics(
    keyword_results: &[KeywordQueryResult],
    sparse_vectors: &[SparseVectorStats],
    with_e6_scores: &[f64],
    without_e6_scores: &[f64],
    query_descriptions: &[String],
    k_values: &[usize],
) -> E6SparseMetrics {
    let keyword_precision = compute_keyword_precision_metrics(keyword_results, k_values);
    let retrieval_quality = compute_retrieval_quality_metrics(keyword_results, k_values);
    let sparsity_analysis = compute_sparsity_analysis_metrics(sparse_vectors);
    let ablation =
        compute_e6_ablation_metrics(with_e6_scores, without_e6_scores, query_descriptions);

    E6SparseMetrics {
        keyword_precision,
        retrieval_quality,
        sparsity_analysis,
        ablation,
        query_count: keyword_results.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_precision_perfect() {
        let results = vec![KeywordQueryResult {
            query_id: uuid::Uuid::new_v4(),
            keyword: "HNSW".to_string(),
            domain: "TechnicalAcronyms".to_string(),
            ranked_docs: vec![uuid::Uuid::new_v4()],
            relevant_docs: vec![],
            anti_relevant_docs: vec![],
            scores: vec![0.95],
        }];

        // Set up so first doc is relevant
        let mut results = results;
        results[0].relevant_docs = vec![results[0].ranked_docs[0]];

        let metrics = compute_keyword_precision_metrics(&results, &[1, 5, 10]);

        assert!((metrics.exact_match_accuracy - 1.0).abs() < 0.01);
        println!("[VERIFIED] Perfect exact match produces accuracy = 1.0");
    }

    #[test]
    fn test_sparsity_analysis() {
        let vectors = vec![
            SparseVectorStats {
                active_terms: 1500,
                total_dimension: 30000,
                weights: vec![0.5; 1500],
            },
            SparseVectorStats {
                active_terms: 1200,
                total_dimension: 30000,
                weights: vec![0.6; 1200],
            },
        ];

        let metrics = compute_sparsity_analysis_metrics(&vectors);

        assert!((metrics.avg_active_terms - 1350.0).abs() < 1.0);
        assert!(metrics.sparsity_ratio > 0.95);

        println!("[VERIFIED] Sparsity analysis computes correctly");
        println!("  Avg active terms: {}", metrics.avg_active_terms);
        println!("  Sparsity ratio: {}", metrics.sparsity_ratio);
    }

    #[test]
    fn test_ablation_metrics() {
        let with_e6 = vec![0.8, 0.9, 0.7, 0.85];
        let without_e6 = vec![0.6, 0.65, 0.5, 0.6];
        let descriptions = vec![
            "query1".to_string(),
            "query2".to_string(),
            "query3".to_string(),
            "query4".to_string(),
        ];

        let metrics = compute_e6_ablation_metrics(&with_e6, &without_e6, &descriptions);

        assert!(metrics.delta > 0.0);
        assert!(metrics.is_essential);

        println!("[VERIFIED] Ablation metrics compute correctly");
        println!("  Delta: {}", metrics.delta);
        println!("  Impact: {}%", metrics.impact_percent);
    }

    #[test]
    fn test_retrieval_quality() {
        let doc1 = uuid::Uuid::new_v4();
        let doc2 = uuid::Uuid::new_v4();
        let doc3 = uuid::Uuid::new_v4();

        let results = vec![KeywordQueryResult {
            query_id: uuid::Uuid::new_v4(),
            keyword: "test".to_string(),
            domain: "test".to_string(),
            ranked_docs: vec![doc1, doc2, doc3],
            relevant_docs: vec![doc1],
            anti_relevant_docs: vec![],
            scores: vec![0.9, 0.8, 0.7],
        }];

        let metrics = compute_retrieval_quality_metrics(&results, &[1, 5, 10]);

        // MRR should be 1.0 since relevant doc is at position 0
        assert!((metrics.mrr - 1.0).abs() < 0.01);
        // P@1 should be 1.0
        assert!((metrics.precision_at_k.get(&1).copied().unwrap_or(0.0) - 1.0).abs() < 0.01);

        println!("[VERIFIED] Retrieval quality metrics compute correctly");
    }
}
