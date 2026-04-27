//! E7 Code Embedding Parameter Tuning
//!
//! Grid search and optimization for E7 code search parameters.
//! Tunes: e7_blend, min_score, fetch_multiplier
//!
//! # Algorithm
//!
//! 1. Load ground truth from data/e7_ground_truth/queries.jsonl
//! 2. For each parameter combination:
//!    - Simulate code search with parameters
//!    - Compute MRR, P@K, NDCG metrics
//! 3. Select optimal parameters based on composite score
//! 4. Report improvement over baseline
//!
//! # Usage
//!
//! ```rust,ignore
//! use context_graph_benchmark::tuning::e7_tuning::{E7TuningConfig, E7Tuner};
//!
//! let config = E7TuningConfig::default();
//! let tuner = E7Tuner::new(config);
//! let ground_truth = vec![]; // Load from data/e7_ground_truth/queries.jsonl
//! let score_provider = SimulatedScoreProvider::new(42);
//! let results = tuner.run_grid_search(&ground_truth, &score_provider);
//! println!("Best params: {:?}", results.best_params);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::metrics::e7_code::{E7GroundTruth, E7QueryType};

// ===========================================================================
// Configuration
// ===========================================================================

/// Configuration for E7 parameter tuning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E7TuningConfig {
    /// Ground truth dataset path.
    pub ground_truth_path: PathBuf,

    /// E7 blend weight search range [start, end, step].
    pub e7_blend_range: (f32, f32, f32),

    /// Minimum score threshold search range [start, end, step].
    pub min_score_range: (f32, f32, f32),

    /// Fetch multiplier search range.
    pub fetch_multiplier_values: Vec<usize>,

    /// Number of bootstrap samples for confidence intervals.
    pub bootstrap_samples: usize,

    /// Random seed for reproducibility.
    pub seed: u64,

    /// Weight for MRR in composite score.
    pub weight_mrr: f64,

    /// Weight for P@5 in composite score.
    pub weight_p5: f64,

    /// Weight for NDCG@10 in composite score.
    pub weight_ndcg: f64,
}

impl Default for E7TuningConfig {
    fn default() -> Self {
        Self {
            ground_truth_path: PathBuf::from("data/e7_ground_truth/queries.jsonl"),
            e7_blend_range: (0.2, 0.8, 0.1), // 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8
            min_score_range: (0.1, 0.4, 0.05), // 0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4
            fetch_multiplier_values: vec![2, 3, 4, 5],
            bootstrap_samples: 100,
            seed: 42,
            weight_mrr: 0.4,
            weight_p5: 0.3,
            weight_ndcg: 0.3,
        }
    }
}

// ===========================================================================
// Parameter Set
// ===========================================================================

/// A set of E7 parameters to evaluate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct E7Params {
    /// E7 blend weight (weight of E7 in E1+E7 blend).
    pub e7_blend: f32,

    /// Minimum blended score threshold.
    pub min_score: f32,

    /// Over-fetch multiplier for reranking.
    pub fetch_multiplier: usize,
}

impl Default for E7Params {
    fn default() -> Self {
        // Current defaults from code_tools.rs
        Self {
            e7_blend: 0.4,
            min_score: 0.2,
            fetch_multiplier: 3,
        }
    }
}

impl E7Params {
    /// Create a new parameter set.
    pub fn new(e7_blend: f32, min_score: f32, fetch_multiplier: usize) -> Self {
        Self {
            e7_blend,
            min_score,
            fetch_multiplier,
        }
    }
}

// ===========================================================================
// Internal Query Result (for tuning purposes)
// ===========================================================================

/// Internal query result for tuning evaluation.
/// This is separate from E7QueryResult to allow different fields.
#[derive(Debug, Clone)]
struct TuningQueryResult {
    /// Query ID.
    #[allow(dead_code)] // Stored for debug/reporting in bin targets
    query_id: String,
    /// Query type.
    #[allow(dead_code)] // Stored for debug/reporting in bin targets
    query_type: E7QueryType,
    /// Relevant document paths (ground truth).
    relevant_docs: Vec<String>,
    /// Retrieved document paths.
    retrieved_docs: Vec<String>,
    /// E1 similarity scores.
    e1_scores: Vec<f64>,
    /// E7 similarity scores.
    e7_scores: Vec<f64>,
    /// Blended scores.
    blended_scores: Vec<f64>,
}

// ===========================================================================
// Results
// ===========================================================================

/// Result for a single parameter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E7ParamResult {
    /// The parameter set tested.
    pub params: E7Params,

    /// Mean Reciprocal Rank.
    pub mrr: f64,

    /// MRR confidence interval (lower, upper).
    pub mrr_ci: (f64, f64),

    /// Precision at various K values.
    pub precision_at: HashMap<usize, f64>,

    /// Recall at various K values.
    pub recall_at: HashMap<usize, f64>,

    /// NDCG at 10.
    pub ndcg_at_10: f64,

    /// Composite score (weighted combination).
    pub composite_score: f64,

    /// Number of E7 unique finds (E7 found, E1 missed).
    pub e7_unique_finds: usize,

    /// Performance metrics by query type.
    pub by_query_type: HashMap<String, QueryTypeResult>,
}

/// Result for a specific query type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryTypeResult {
    /// Query count.
    pub count: usize,
    /// MRR for this query type.
    pub mrr: f64,
    /// P@5 for this query type.
    pub precision_at_5: f64,
}

/// Full tuning results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E7TuningResults {
    /// Timestamp of the tuning run.
    pub timestamp: String,

    /// Configuration used.
    pub config: E7TuningConfig,

    /// Number of ground truth queries.
    pub num_queries: usize,

    /// All tested parameter combinations.
    pub all_results: Vec<E7ParamResult>,

    /// Best parameter configuration.
    pub best_params: E7Params,

    /// Best result.
    pub best_result: E7ParamResult,

    /// Baseline (default params) result.
    pub baseline_result: E7ParamResult,

    /// Improvement over baseline (percentage).
    pub improvement_pct: f64,

    /// Summary statistics.
    pub summary: TuningSummary,
}

/// Summary of tuning results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuningSummary {
    /// Optimal e7_blend value.
    pub optimal_e7_blend: f32,

    /// Optimal min_score value.
    pub optimal_min_score: f32,

    /// Optimal fetch_multiplier value.
    pub optimal_fetch_multiplier: usize,

    /// Best composite score.
    pub best_score: f64,

    /// Baseline composite score.
    pub baseline_score: f64,

    /// Improvement percentage.
    pub improvement_pct: f64,

    /// Parameter sensitivity analysis.
    pub sensitivity: ParameterSensitivity,
}

/// Parameter sensitivity analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSensitivity {
    /// Impact of e7_blend on composite score (std dev across values).
    pub e7_blend_impact: f64,

    /// Impact of min_score on composite score.
    pub min_score_impact: f64,

    /// Impact of fetch_multiplier on composite score.
    pub fetch_multiplier_impact: f64,

    /// Most impactful parameter.
    pub most_impactful: String,
}

// ===========================================================================
// Tuner
// ===========================================================================

/// E7 parameter tuner using grid search.
pub struct E7Tuner {
    config: E7TuningConfig,
}

impl E7Tuner {
    /// Create a new tuner with the given configuration.
    pub fn new(config: E7TuningConfig) -> Self {
        Self { config }
    }

    /// Generate all parameter combinations for grid search.
    pub fn generate_param_grid(&self) -> Vec<E7Params> {
        let mut params = Vec::new();

        let (blend_start, blend_end, blend_step) = self.config.e7_blend_range;
        let (score_start, score_end, score_step) = self.config.min_score_range;

        let mut blend = blend_start;
        while blend <= blend_end + f32::EPSILON {
            let mut score = score_start;
            while score <= score_end + f32::EPSILON {
                for &fetch_mult in &self.config.fetch_multiplier_values {
                    params.push(E7Params::new(blend, score, fetch_mult));
                }
                score += score_step;
            }
            blend += blend_step;
        }

        params
    }

    /// Run grid search over all parameter combinations.
    pub fn run_grid_search(
        &self,
        ground_truth: &[E7GroundTruth],
        score_provider: &impl ScoreProvider,
    ) -> E7TuningResults {
        let param_grid = self.generate_param_grid();
        let total_combinations = param_grid.len();

        println!(
            "Starting E7 grid search: {} combinations",
            total_combinations
        );
        println!("  e7_blend: {:?}", self.config.e7_blend_range);
        println!("  min_score: {:?}", self.config.min_score_range);
        println!(
            "  fetch_multiplier: {:?}",
            self.config.fetch_multiplier_values
        );
        println!();

        let mut all_results = Vec::with_capacity(total_combinations);

        for (i, params) in param_grid.iter().enumerate() {
            let result = self.evaluate_params(*params, ground_truth, score_provider);
            all_results.push(result);

            if (i + 1) % 10 == 0 || i + 1 == total_combinations {
                println!(
                    "  Progress: {}/{} ({:.1}%)",
                    i + 1,
                    total_combinations,
                    (i + 1) as f64 / total_combinations as f64 * 100.0
                );
            }
        }

        // Find best result
        let best_idx = all_results
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.composite_score
                    .partial_cmp(&b.composite_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);

        let best_result = all_results[best_idx].clone();
        let best_params = best_result.params;

        // Find baseline result
        let baseline_params = E7Params::default();
        let baseline_result = self.evaluate_params(baseline_params, ground_truth, score_provider);

        // Calculate improvement
        let improvement_pct = if baseline_result.composite_score > 0.0 {
            (best_result.composite_score - baseline_result.composite_score)
                / baseline_result.composite_score
                * 100.0
        } else {
            0.0
        };

        // Parameter sensitivity analysis
        let sensitivity = self.analyze_sensitivity(&all_results);

        let summary = TuningSummary {
            optimal_e7_blend: best_params.e7_blend,
            optimal_min_score: best_params.min_score,
            optimal_fetch_multiplier: best_params.fetch_multiplier,
            best_score: best_result.composite_score,
            baseline_score: baseline_result.composite_score,
            improvement_pct,
            sensitivity,
        };

        E7TuningResults {
            timestamp: chrono::Utc::now().to_rfc3339(),
            config: self.config.clone(),
            num_queries: ground_truth.len(),
            all_results,
            best_params,
            best_result,
            baseline_result,
            improvement_pct,
            summary,
        }
    }

    /// Evaluate a single parameter configuration.
    fn evaluate_params(
        &self,
        params: E7Params,
        ground_truth: &[E7GroundTruth],
        score_provider: &impl ScoreProvider,
    ) -> E7ParamResult {
        let mut query_results = Vec::with_capacity(ground_truth.len());
        let mut by_query_type: HashMap<String, Vec<TuningQueryResult>> = HashMap::new();

        for gt in ground_truth {
            // Get simulated scores for this query
            let scores = score_provider.get_scores(&gt.query, &gt.relevant_docs, params);

            // Build internal query result
            let result = TuningQueryResult {
                query_id: gt.query_id.clone(),
                query_type: gt.query_type,
                relevant_docs: gt.relevant_docs.clone(),
                retrieved_docs: scores.iter().map(|(doc, _)| doc.clone()).collect(),
                e1_scores: scores.iter().map(|(_, (e1, _))| *e1 as f64).collect(),
                e7_scores: scores.iter().map(|(_, (_, e7))| *e7 as f64).collect(),
                blended_scores: scores
                    .iter()
                    .map(|(_, (e1, e7))| blend_scores(*e1, *e7, params.e7_blend) as f64)
                    .collect(),
            };

            // Group by query type
            let query_type_str = format!("{:?}", gt.query_type);
            by_query_type
                .entry(query_type_str)
                .or_default()
                .push(result.clone());

            query_results.push(result);
        }

        // Compute metrics
        let mrr = compute_mrr(&query_results, params.min_score as f64);
        let precision_at =
            compute_precision_at_k(&query_results, &[1, 5, 10], params.min_score as f64);
        let recall_at = compute_recall_at_k(&query_results, &[1, 5, 10], params.min_score as f64);
        let ndcg_at_10 = compute_ndcg(&query_results, 10, params.min_score as f64);

        // Compute composite score
        let p5 = *precision_at.get(&5).unwrap_or(&0.0);
        let composite_score = self.config.weight_mrr * mrr
            + self.config.weight_p5 * p5
            + self.config.weight_ndcg * ndcg_at_10;

        // E7 unique finds
        let e7_unique = count_e7_unique_finds(&query_results);

        // Per-query-type metrics
        let by_query_type_results: HashMap<String, QueryTypeResult> = by_query_type
            .iter()
            .map(|(qt, results)| {
                let qt_mrr = compute_mrr(results, params.min_score as f64);
                let qt_p5 = compute_precision_at_k(results, &[5], params.min_score as f64)
                    .get(&5)
                    .copied()
                    .unwrap_or(0.0);
                (
                    qt.clone(),
                    QueryTypeResult {
                        count: results.len(),
                        mrr: qt_mrr,
                        precision_at_5: qt_p5,
                    },
                )
            })
            .collect();

        E7ParamResult {
            params,
            mrr,
            mrr_ci: (mrr - 0.05, mrr + 0.05), // Simplified CI for now
            precision_at,
            recall_at,
            ndcg_at_10,
            composite_score,
            e7_unique_finds: e7_unique,
            by_query_type: by_query_type_results,
        }
    }

    /// Analyze parameter sensitivity.
    fn analyze_sensitivity(&self, results: &[E7ParamResult]) -> ParameterSensitivity {
        // Group results by each parameter
        let mut by_blend: HashMap<String, Vec<f64>> = HashMap::new();
        let mut by_min_score: HashMap<String, Vec<f64>> = HashMap::new();
        let mut by_fetch_mult: HashMap<String, Vec<f64>> = HashMap::new();

        for r in results {
            by_blend
                .entry(format!("{:.2}", r.params.e7_blend))
                .or_default()
                .push(r.composite_score);
            by_min_score
                .entry(format!("{:.2}", r.params.min_score))
                .or_default()
                .push(r.composite_score);
            by_fetch_mult
                .entry(format!("{}", r.params.fetch_multiplier))
                .or_default()
                .push(r.composite_score);
        }

        // Compute standard deviation of means across parameter values
        let blend_means: Vec<f64> = by_blend
            .values()
            .map(|v| v.iter().sum::<f64>() / v.len() as f64)
            .collect();
        let score_means: Vec<f64> = by_min_score
            .values()
            .map(|v| v.iter().sum::<f64>() / v.len() as f64)
            .collect();
        let fetch_means: Vec<f64> = by_fetch_mult
            .values()
            .map(|v| v.iter().sum::<f64>() / v.len() as f64)
            .collect();

        let blend_impact = std_dev(&blend_means);
        let min_score_impact = std_dev(&score_means);
        let fetch_mult_impact = std_dev(&fetch_means);

        let most_impactful =
            if blend_impact >= min_score_impact && blend_impact >= fetch_mult_impact {
                "e7_blend".to_string()
            } else if min_score_impact >= fetch_mult_impact {
                "min_score".to_string()
            } else {
                "fetch_multiplier".to_string()
            };

        ParameterSensitivity {
            e7_blend_impact: blend_impact,
            min_score_impact,
            fetch_multiplier_impact: fetch_mult_impact,
            most_impactful,
        }
    }
}

// ===========================================================================
// Score Provider Trait
// ===========================================================================

/// Trait for providing E1/E7 scores for a query.
pub trait ScoreProvider {
    /// Get E1 and E7 scores for a query against candidate documents.
    ///
    /// Returns: Vec<(doc_path, (e1_score, e7_score))> sorted by blended score descending.
    fn get_scores(
        &self,
        query: &str,
        relevant_docs: &[String],
        params: E7Params,
    ) -> Vec<(String, (f32, f32))>;
}

/// Simulated score provider for testing without real embeddings.
pub struct SimulatedScoreProvider {
    seed: u64,
}

impl SimulatedScoreProvider {
    /// Create a new simulated score provider.
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Hash a string to a deterministic float in [0, 1].
    fn hash_to_score(&self, s: &str, salt: u64) -> f32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        salt.hash(&mut hasher);
        self.seed.hash(&mut hasher);
        let h = hasher.finish();

        h as f32 / u64::MAX as f32
    }
}

impl ScoreProvider for SimulatedScoreProvider {
    fn get_scores(
        &self,
        query: &str,
        relevant_docs: &[String],
        params: E7Params,
    ) -> Vec<(String, (f32, f32))> {
        let mut scores = Vec::new();

        // Generate scores for relevant docs (higher scores)
        for doc in relevant_docs {
            let e1_base = self.hash_to_score(&format!("e1:{}{}", query, doc), 1);
            let e7_base = self.hash_to_score(&format!("e7:{}{}", query, doc), 2);

            // Relevant docs get boosted scores
            let e1 = 0.5 + e1_base * 0.5;
            let e7 = 0.5 + e7_base * 0.5;

            scores.push((doc.clone(), (e1, e7)));
        }

        // Generate some irrelevant docs with lower scores
        let num_irrelevant = (params.fetch_multiplier * 10).min(50);
        for i in 0..num_irrelevant {
            let doc = format!("irrelevant_doc_{}.rs", i);
            let e1 = self.hash_to_score(&format!("e1:{}{}", query, doc), 3) * 0.5;
            let e7 = self.hash_to_score(&format!("e7:{}{}", query, doc), 4) * 0.5;
            scores.push((doc, (e1, e7)));
        }

        // Sort by blended score
        scores.sort_by(|(_, (e1_a, e7_a)), (_, (e1_b, e7_b))| {
            let blend_a = blend_scores(*e1_a, *e7_a, params.e7_blend);
            let blend_b = blend_scores(*e1_b, *e7_b, params.e7_blend);
            blend_b
                .partial_cmp(&blend_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scores
    }
}

// ===========================================================================
// Helper Functions
// ===========================================================================

/// Blend E1 and E7 scores.
fn blend_scores(e1: f32, e7: f32, e7_weight: f32) -> f32 {
    (1.0 - e7_weight) * e1 + e7_weight * e7
}

/// Compute Mean Reciprocal Rank.
fn compute_mrr(results: &[TuningQueryResult], min_score: f64) -> f64 {
    if results.is_empty() {
        return 0.0;
    }

    let mut sum = 0.0;
    for r in results {
        for (rank, (doc, score)) in r
            .retrieved_docs
            .iter()
            .zip(r.blended_scores.iter())
            .enumerate()
        {
            if *score >= min_score && r.relevant_docs.contains(doc) {
                sum += 1.0 / (rank + 1) as f64;
                break;
            }
        }
    }

    sum / results.len() as f64
}

/// Compute Precision at K for multiple K values.
fn compute_precision_at_k(
    results: &[TuningQueryResult],
    k_values: &[usize],
    min_score: f64,
) -> HashMap<usize, f64> {
    let mut precision_map = HashMap::new();

    for &k in k_values {
        let mut sum = 0.0;
        for r in results {
            let top_k: Vec<_> = r
                .retrieved_docs
                .iter()
                .zip(r.blended_scores.iter())
                .take(k)
                .filter(|(_, score)| **score >= min_score)
                .collect();

            let relevant_count = top_k
                .iter()
                .filter(|(doc, _)| r.relevant_docs.contains(*doc))
                .count();

            sum += relevant_count as f64 / k as f64;
        }
        precision_map.insert(k, sum / results.len() as f64);
    }

    precision_map
}

/// Compute Recall at K for multiple K values.
fn compute_recall_at_k(
    results: &[TuningQueryResult],
    k_values: &[usize],
    min_score: f64,
) -> HashMap<usize, f64> {
    let mut recall_map = HashMap::new();

    for &k in k_values {
        let mut sum = 0.0;
        for r in results {
            if r.relevant_docs.is_empty() {
                continue;
            }

            let top_k: Vec<_> = r
                .retrieved_docs
                .iter()
                .zip(r.blended_scores.iter())
                .take(k)
                .filter(|(_, score)| **score >= min_score)
                .collect();

            let relevant_count = top_k
                .iter()
                .filter(|(doc, _)| r.relevant_docs.contains(*doc))
                .count();

            sum += relevant_count as f64 / r.relevant_docs.len() as f64;
        }
        recall_map.insert(k, sum / results.len().max(1) as f64);
    }

    recall_map
}

/// Compute NDCG at K.
fn compute_ndcg(results: &[TuningQueryResult], k: usize, min_score: f64) -> f64 {
    if results.is_empty() {
        return 0.0;
    }

    let mut sum = 0.0;
    for r in results {
        let mut dcg = 0.0;
        for (i, (doc, score)) in r
            .retrieved_docs
            .iter()
            .zip(r.blended_scores.iter())
            .take(k)
            .enumerate()
        {
            if *score >= min_score && r.relevant_docs.contains(doc) {
                dcg += 1.0 / (i + 2) as f64; // log2(i+2) approximation
            }
        }

        // Ideal DCG (all relevant docs at top)
        let mut idcg = 0.0;
        for i in 0..r.relevant_docs.len().min(k) {
            idcg += 1.0 / (i + 2) as f64;
        }

        if idcg > 0.0 {
            sum += dcg / idcg;
        }
    }

    sum / results.len() as f64
}

/// Count E7 unique finds (found by E7 with high score, missed by E1).
fn count_e7_unique_finds(results: &[TuningQueryResult]) -> usize {
    let mut count = 0;
    for r in results {
        for (i, doc) in r.retrieved_docs.iter().enumerate() {
            if r.relevant_docs.contains(doc)
                && r.e7_scores.get(i).copied().unwrap_or(0.0) > 0.5
                && r.e1_scores.get(i).copied().unwrap_or(0.0) < 0.3
            {
                count += 1;
            }
        }
    }
    count
}

/// Compute standard deviation.
fn std_dev(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    fn make_ground_truth() -> Vec<E7GroundTruth> {
        vec![
            E7GroundTruth {
                query_id: "q1".to_string(),
                query: "How is memory chunking implemented?".to_string(),
                query_type: E7QueryType::FunctionSearch,
                relevant_docs: vec!["chunker.rs".to_string()],
                relevant_functions: vec!["chunk_text".to_string()],
                expected_entity_types: vec!["Function".to_string()],
                notes: None,
            },
            E7GroundTruth {
                query_id: "q2".to_string(),
                query: "Find the Memory struct".to_string(),
                query_type: E7QueryType::StructSearch,
                relevant_docs: vec!["memory/mod.rs".to_string()],
                relevant_functions: vec![],
                expected_entity_types: vec!["Struct".to_string()],
                notes: None,
            },
        ]
    }

    #[test]
    fn test_param_grid_generation() {
        let config = E7TuningConfig {
            e7_blend_range: (0.2, 0.4, 0.1),
            min_score_range: (0.1, 0.2, 0.1),
            fetch_multiplier_values: vec![2, 3],
            ..Default::default()
        };

        let tuner = E7Tuner::new(config);
        let grid = tuner.generate_param_grid();

        // 3 blend values × 2 score values × 2 fetch multipliers = 12
        assert_eq!(grid.len(), 12);

        // Check first and last
        assert!((grid[0].e7_blend - 0.2).abs() < 0.001);
        assert!((grid[0].min_score - 0.1).abs() < 0.001);
        assert_eq!(grid[0].fetch_multiplier, 2);
    }

    #[test]
    fn test_tuning_with_simulated_scores() {
        let config = E7TuningConfig {
            e7_blend_range: (0.3, 0.5, 0.1),
            min_score_range: (0.1, 0.2, 0.1),
            fetch_multiplier_values: vec![3],
            ..Default::default()
        };

        let tuner = E7Tuner::new(config);
        let ground_truth = make_ground_truth();
        let score_provider = SimulatedScoreProvider::new(42);

        let results = tuner.run_grid_search(&ground_truth, &score_provider);

        assert_eq!(results.num_queries, 2);
        assert!(!results.all_results.is_empty());
        assert!(results.best_result.mrr >= 0.0);
        assert!(results.best_result.mrr <= 1.0);
    }

    #[test]
    fn test_blend_scores() {
        let e1 = 0.8;
        let e7 = 0.4;

        // 50/50 blend
        let blended = blend_scores(e1, e7, 0.5);
        assert!((blended - 0.6).abs() < 0.001);

        // E1 only
        let e1_only = blend_scores(e1, e7, 0.0);
        assert!((e1_only - 0.8).abs() < 0.001);

        // E7 only
        let e7_only = blend_scores(e1, e7, 1.0);
        assert!((e7_only - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_sensitivity_analysis() {
        let config = E7TuningConfig::default();
        let tuner = E7Tuner::new(config);

        // Create mock results with varying scores
        let results = vec![
            E7ParamResult {
                params: E7Params::new(0.2, 0.1, 2),
                mrr: 0.5,
                mrr_ci: (0.45, 0.55),
                precision_at: HashMap::new(),
                recall_at: HashMap::new(),
                ndcg_at_10: 0.5,
                composite_score: 0.5,
                e7_unique_finds: 0,
                by_query_type: HashMap::new(),
            },
            E7ParamResult {
                params: E7Params::new(0.8, 0.1, 2),
                mrr: 0.7,
                mrr_ci: (0.65, 0.75),
                precision_at: HashMap::new(),
                recall_at: HashMap::new(),
                ndcg_at_10: 0.7,
                composite_score: 0.7,
                e7_unique_finds: 2,
                by_query_type: HashMap::new(),
            },
        ];

        let sensitivity = tuner.analyze_sensitivity(&results);
        assert!(sensitivity.e7_blend_impact >= 0.0);
    }

    #[test]
    fn test_default_params() {
        let params = E7Params::default();
        assert!((params.e7_blend - 0.4).abs() < 0.001);
        assert!((params.min_score - 0.2).abs() < 0.001);
        assert_eq!(params.fetch_multiplier, 3);
    }

    #[test]
    fn test_mrr_computation() {
        let results = vec![TuningQueryResult {
            query_id: "q1".to_string(),
            query_type: E7QueryType::FunctionSearch,
            relevant_docs: vec!["doc1.rs".to_string()],
            retrieved_docs: vec![
                "other.rs".to_string(),
                "doc1.rs".to_string(),
                "another.rs".to_string(),
            ],
            e1_scores: vec![0.5, 0.4, 0.3],
            e7_scores: vec![0.5, 0.4, 0.3],
            blended_scores: vec![0.5, 0.4, 0.3],
        }];

        let mrr = compute_mrr(&results, 0.0);
        // Relevant doc at position 2 (1-indexed), so RR = 1/2 = 0.5
        assert!((mrr - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_precision_at_k() {
        let results = vec![TuningQueryResult {
            query_id: "q1".to_string(),
            query_type: E7QueryType::FunctionSearch,
            relevant_docs: vec!["doc1.rs".to_string(), "doc2.rs".to_string()],
            retrieved_docs: vec![
                "doc1.rs".to_string(),
                "other.rs".to_string(),
                "doc2.rs".to_string(),
            ],
            e1_scores: vec![0.9, 0.8, 0.7],
            e7_scores: vec![0.9, 0.8, 0.7],
            blended_scores: vec![0.9, 0.8, 0.7],
        }];

        let precision = compute_precision_at_k(&results, &[1, 5], 0.0);

        // P@1 = 1/1 = 1.0 (doc1 is relevant)
        assert!((precision.get(&1).copied().unwrap_or(0.0) - 1.0).abs() < 0.001);

        // P@5 (only 3 docs) = 2/5 = 0.4 (doc1 and doc2 are relevant)
        assert!((precision.get(&5).copied().unwrap_or(0.0) - 0.4).abs() < 0.001);
    }
}
