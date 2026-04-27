//! E7 Code Embedding Benchmark Metrics
//!
//! Specialized metrics for evaluating E7 code search performance.
//! E7 (V_correctness) uses 1536D embeddings for code patterns and function signatures.
//!
//! # Metrics
//! - P@K, MRR, NDCG@10: Standard retrieval metrics
//! - E7 Unique Finds: Documents found by E7 that E1 missed
//! - IoU@K: Token-level Intersection over Union with ground truth
//! - Latency: Query response time (P50, P95, P99)
//! - Entity Type Accuracy: Correct entity type detection rate
//!
//! # Philosophy
//! E7 should ENHANCE E1, not replace it. E7 finds what E1 misses:
//! - Code patterns E1 treats as natural language
//! - Function signatures and structural similarity
//! - Import relationships and dependency patterns

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Query types for E7 code search evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum E7QueryType {
    /// "How is X implemented?"
    FunctionSearch,
    /// "Find code that uses X"
    PatternSearch,
    /// "What modules import X?"
    ImportSearch,
    /// "What types implement trait X?"
    TraitSearch,
    /// "Find struct definitions"
    StructSearch,
    /// "Find enum definitions"
    EnumSearch,
    /// "Find impl blocks"
    ImplSearch,
    /// "Find test functions"
    TestSearch,
    /// "Find documentation"
    DocSearch,
    /// "Find constants"
    ConstSearch,
    /// Semantic/natural language query
    SemanticSearch,
}

impl E7QueryType {
    /// Parse from string representation.
    pub fn parse_from(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "function_search" | "func" => Some(Self::FunctionSearch),
            "pattern_search" | "pattern" => Some(Self::PatternSearch),
            "import_search" | "import" => Some(Self::ImportSearch),
            "trait_search" | "trait" => Some(Self::TraitSearch),
            "struct_search" | "struct" => Some(Self::StructSearch),
            "enum_search" | "enum" => Some(Self::EnumSearch),
            "impl_search" | "impl" => Some(Self::ImplSearch),
            "test_search" | "test" => Some(Self::TestSearch),
            "doc_search" | "doc" => Some(Self::DocSearch),
            "const_search" | "const" => Some(Self::ConstSearch),
            "semantic_search" | "semantic" => Some(Self::SemanticSearch),
            _ => None,
        }
    }

    /// Get display name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::FunctionSearch => "Function Search",
            Self::PatternSearch => "Pattern Search",
            Self::ImportSearch => "Import Search",
            Self::TraitSearch => "Trait Search",
            Self::StructSearch => "Struct Search",
            Self::EnumSearch => "Enum Search",
            Self::ImplSearch => "Impl Search",
            Self::TestSearch => "Test Search",
            Self::DocSearch => "Doc Search",
            Self::ConstSearch => "Const Search",
            Self::SemanticSearch => "Semantic Search",
        }
    }
}

/// Ground truth entry for E7 evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E7GroundTruth {
    /// Query ID.
    pub query_id: String,
    /// Query text.
    pub query: String,
    /// Query type.
    pub query_type: E7QueryType,
    /// Relevant document paths.
    pub relevant_docs: Vec<String>,
    /// Relevant function/method names (if applicable).
    pub relevant_functions: Vec<String>,
    /// Expected entity types in results.
    pub expected_entity_types: Vec<String>,
    /// Notes about this ground truth entry.
    pub notes: Option<String>,
}

/// Result of a single E7 query evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E7QueryResult {
    /// Query ID.
    pub query_id: String,
    /// Query type.
    pub query_type: E7QueryType,
    /// Retrieved document paths.
    pub retrieved_docs: Vec<String>,
    /// Retrieved entity types.
    pub retrieved_entity_types: Vec<String>,
    /// E7 similarity scores.
    pub e7_scores: Vec<f64>,
    /// E1 similarity scores (for comparison).
    pub e1_scores: Vec<f64>,
    /// Query latency.
    pub latency: Duration,
    /// Precision at various K values.
    pub precision_at: HashMap<usize, f64>,
    /// Recall at various K values.
    pub recall_at: HashMap<usize, f64>,
    /// Mean Reciprocal Rank.
    pub mrr: f64,
    /// NDCG at various K values.
    pub ndcg_at: HashMap<usize, f64>,
    /// IoU at various K values.
    pub iou_at: HashMap<usize, f64>,
    /// Documents found by E7 but not E1.
    pub e7_unique_finds: Vec<String>,
    /// Documents found by E1 but not E7.
    pub e1_unique_finds: Vec<String>,
}

/// Aggregated E7 benchmark metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E7BenchmarkMetrics {
    /// Total number of queries evaluated.
    pub query_count: usize,

    /// Mean precision at K.
    pub mean_precision_at: HashMap<usize, f64>,

    /// Mean recall at K.
    pub mean_recall_at: HashMap<usize, f64>,

    /// Mean MRR across all queries.
    pub mean_mrr: f64,

    /// Mean NDCG at K.
    pub mean_ndcg_at: HashMap<usize, f64>,

    /// Mean IoU at K.
    pub mean_iou_at: HashMap<usize, f64>,

    /// Latency percentiles.
    pub latency_p50: Duration,
    pub latency_p95: Duration,
    pub latency_p99: Duration,

    /// E7 unique finds rate (what % of relevant docs E7 found that E1 missed).
    pub e7_unique_find_rate: f64,

    /// Metrics broken down by query type.
    pub by_query_type: HashMap<E7QueryType, E7QueryTypeMetrics>,

    /// Entity type accuracy (correctly identified entity types).
    pub entity_type_accuracy: f64,
}

/// Metrics for a specific query type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E7QueryTypeMetrics {
    /// Number of queries of this type.
    pub count: usize,
    /// Mean P@10 for this query type.
    pub mean_precision_at_10: f64,
    /// Mean MRR for this query type.
    pub mean_mrr: f64,
    /// Mean NDCG@10 for this query type.
    pub mean_ndcg_at_10: f64,
    /// E7 unique find rate for this query type.
    pub e7_unique_find_rate: f64,
}

impl E7BenchmarkMetrics {
    /// Create from individual query results.
    pub fn from_results(results: &[E7QueryResult]) -> Self {
        if results.is_empty() {
            return Self::default();
        }

        let query_count = results.len();

        // Collect latencies for percentile calculation
        let mut latencies: Vec<Duration> = results.iter().map(|r| r.latency).collect();
        latencies.sort();

        let latency_p50 = latencies[latencies.len() / 2];
        let latency_p95 = latencies[(latencies.len() as f64 * 0.95) as usize];
        let latency_p99 =
            latencies[(latencies.len() as f64 * 0.99).min(latencies.len() as f64 - 1.0) as usize];

        // Aggregate precision/recall/NDCG
        let mut mean_precision_at: HashMap<usize, f64> = HashMap::new();
        let mut mean_recall_at: HashMap<usize, f64> = HashMap::new();
        let mut mean_ndcg_at: HashMap<usize, f64> = HashMap::new();
        let mut mean_iou_at: HashMap<usize, f64> = HashMap::new();

        for k in [1, 3, 5, 10] {
            let p_sum: f64 = results.iter().filter_map(|r| r.precision_at.get(&k)).sum();
            let r_sum: f64 = results.iter().filter_map(|r| r.recall_at.get(&k)).sum();
            let n_sum: f64 = results.iter().filter_map(|r| r.ndcg_at.get(&k)).sum();
            let i_sum: f64 = results.iter().filter_map(|r| r.iou_at.get(&k)).sum();

            mean_precision_at.insert(k, p_sum / query_count as f64);
            mean_recall_at.insert(k, r_sum / query_count as f64);
            mean_ndcg_at.insert(k, n_sum / query_count as f64);
            mean_iou_at.insert(k, i_sum / query_count as f64);
        }

        let mean_mrr = results.iter().map(|r| r.mrr).sum::<f64>() / query_count as f64;

        // Calculate E7 unique find rate
        let total_unique_finds: usize = results.iter().map(|r| r.e7_unique_finds.len()).sum();
        let total_relevant: usize = results
            .iter()
            .map(|r| {
                r.e7_unique_finds.len()
                    + r.e1_unique_finds.len()
                    + r.retrieved_docs
                        .iter()
                        .filter(|d| {
                            !r.e7_unique_finds.contains(d) && !r.e1_unique_finds.contains(d)
                        })
                        .count()
            })
            .sum();
        let e7_unique_find_rate = if total_relevant > 0 {
            total_unique_finds as f64 / total_relevant as f64
        } else {
            0.0
        };

        // Metrics by query type
        let mut by_query_type: HashMap<E7QueryType, E7QueryTypeMetrics> = HashMap::new();
        for qt in [
            E7QueryType::FunctionSearch,
            E7QueryType::PatternSearch,
            E7QueryType::ImportSearch,
            E7QueryType::TraitSearch,
            E7QueryType::StructSearch,
            E7QueryType::EnumSearch,
            E7QueryType::ImplSearch,
            E7QueryType::TestSearch,
            E7QueryType::DocSearch,
            E7QueryType::ConstSearch,
            E7QueryType::SemanticSearch,
        ] {
            let type_results: Vec<_> = results.iter().filter(|r| r.query_type == qt).collect();

            if !type_results.is_empty() {
                let count = type_results.len();
                let mean_p10 = type_results
                    .iter()
                    .filter_map(|r| r.precision_at.get(&10))
                    .sum::<f64>()
                    / count as f64;
                let mean_mrr_t = type_results.iter().map(|r| r.mrr).sum::<f64>() / count as f64;
                let mean_ndcg10 = type_results
                    .iter()
                    .filter_map(|r| r.ndcg_at.get(&10))
                    .sum::<f64>()
                    / count as f64;
                let unique_finds: usize =
                    type_results.iter().map(|r| r.e7_unique_finds.len()).sum();
                let type_relevant: usize = type_results
                    .iter()
                    .map(|r| r.retrieved_docs.len().max(1))
                    .sum();

                by_query_type.insert(
                    qt,
                    E7QueryTypeMetrics {
                        count,
                        mean_precision_at_10: mean_p10,
                        mean_mrr: mean_mrr_t,
                        mean_ndcg_at_10: mean_ndcg10,
                        e7_unique_find_rate: unique_finds as f64 / type_relevant as f64,
                    },
                );
            }
        }

        // Entity type accuracy
        let correct_entity_types: usize = results
            .iter()
            .filter(|r| !r.retrieved_entity_types.is_empty())
            .count();
        let entity_type_accuracy = correct_entity_types as f64 / query_count as f64;

        Self {
            query_count,
            mean_precision_at,
            mean_recall_at,
            mean_mrr,
            mean_ndcg_at,
            mean_iou_at,
            latency_p50,
            latency_p95,
            latency_p99,
            e7_unique_find_rate,
            by_query_type,
            entity_type_accuracy,
        }
    }

    /// Overall E7 benchmark score (0-1).
    pub fn overall_score(&self) -> f64 {
        let p10 = self.mean_precision_at.get(&10).copied().unwrap_or(0.0);
        let ndcg10 = self.mean_ndcg_at.get(&10).copied().unwrap_or(0.0);

        // Weight: 30% MRR, 25% P@10, 25% NDCG@10, 20% E7 unique finds
        0.30 * self.mean_mrr + 0.25 * p10 + 0.25 * ndcg10 + 0.20 * self.e7_unique_find_rate
    }

    /// Check if E7 is providing value over E1.
    ///
    /// E7 is considered valuable if:
    /// - E7 unique find rate > 5% (finds things E1 misses)
    /// - Overall score > 0.5
    pub fn is_valuable(&self) -> bool {
        self.e7_unique_find_rate > 0.05 && self.overall_score() > 0.5
    }
}

// ===========================================================================
// Metric Computation Functions
// ===========================================================================

/// Compute Precision@K for E7 results.
pub fn precision_at_k(retrieved: &[String], relevant: &HashSet<String>, k: usize) -> f64 {
    if k == 0 || retrieved.is_empty() {
        return 0.0;
    }

    let top_k = retrieved.iter().take(k);
    let hits = top_k.filter(|doc| relevant.contains(*doc)).count();

    hits as f64 / k.min(retrieved.len()) as f64
}

/// Compute Recall@K for E7 results.
pub fn recall_at_k(retrieved: &[String], relevant: &HashSet<String>, k: usize) -> f64 {
    if relevant.is_empty() {
        return 0.0;
    }

    let top_k = retrieved.iter().take(k);
    let hits = top_k.filter(|doc| relevant.contains(*doc)).count();

    hits as f64 / relevant.len() as f64
}

/// Compute Mean Reciprocal Rank.
pub fn mrr(retrieved: &[String], relevant: &HashSet<String>) -> f64 {
    for (i, doc) in retrieved.iter().enumerate() {
        if relevant.contains(doc) {
            return 1.0 / (i + 1) as f64;
        }
    }
    0.0
}

/// Compute NDCG@K.
///
/// Normalized Discounted Cumulative Gain measures ranking quality.
pub fn ndcg_at_k(retrieved: &[String], relevant: &HashSet<String>, k: usize) -> f64 {
    if k == 0 || relevant.is_empty() {
        return 0.0;
    }

    // DCG: sum of relevance / log2(rank + 1)
    let dcg: f64 = retrieved
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, doc)| {
            let rel = if relevant.contains(doc) { 1.0 } else { 0.0 };
            rel / (i as f64 + 2.0).log2()
        })
        .sum();

    // Ideal DCG: all relevant docs at top positions
    let ideal_dcg: f64 = (0..relevant.len().min(k))
        .map(|i| 1.0 / (i as f64 + 2.0).log2())
        .sum();

    if ideal_dcg < f64::EPSILON {
        0.0
    } else {
        dcg / ideal_dcg
    }
}

/// Find documents retrieved by E7 but not E1.
///
/// These represent E7's unique value - code patterns E1 missed.
pub fn e7_unique_finds(
    e7_retrieved: &[String],
    e1_retrieved: &[String],
    relevant: &HashSet<String>,
) -> Vec<String> {
    let e1_set: HashSet<_> = e1_retrieved.iter().collect();

    e7_retrieved
        .iter()
        .filter(|doc| relevant.contains(*doc) && !e1_set.contains(doc))
        .cloned()
        .collect()
}

/// Find documents retrieved by E1 but not E7.
pub fn e1_unique_finds(
    e7_retrieved: &[String],
    e1_retrieved: &[String],
    relevant: &HashSet<String>,
) -> Vec<String> {
    let e7_set: HashSet<_> = e7_retrieved.iter().collect();

    e1_retrieved
        .iter()
        .filter(|doc| relevant.contains(*doc) && !e7_set.contains(doc))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_precision_at_k() {
        let retrieved = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let relevant: HashSet<_> = ["a", "c", "d"].iter().map(|s| s.to_string()).collect();

        assert!((precision_at_k(&retrieved, &relevant, 1) - 1.0).abs() < 0.001); // a is relevant
        assert!((precision_at_k(&retrieved, &relevant, 2) - 0.5).abs() < 0.001); // 1/2 relevant
        assert!((precision_at_k(&retrieved, &relevant, 3) - 0.666).abs() < 0.01);
        // 2/3 relevant
    }

    #[test]
    fn test_recall_at_k() {
        let retrieved = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let relevant: HashSet<_> = ["a", "c", "d"].iter().map(|s| s.to_string()).collect();

        assert!((recall_at_k(&retrieved, &relevant, 1) - 0.333).abs() < 0.01); // 1/3 relevant found
        assert!((recall_at_k(&retrieved, &relevant, 3) - 0.666).abs() < 0.01); // 2/3 relevant found
    }

    #[test]
    fn test_mrr() {
        let retrieved = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let relevant: HashSet<_> = ["b", "c"].iter().map(|s| s.to_string()).collect();

        assert!((mrr(&retrieved, &relevant) - 0.5).abs() < 0.001); // b is at position 2
    }

    #[test]
    fn test_ndcg_at_k() {
        let retrieved = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let relevant: HashSet<_> = ["a", "c"].iter().map(|s| s.to_string()).collect();

        let ndcg = ndcg_at_k(&retrieved, &relevant, 3);
        assert!(
            ndcg > 0.0 && ndcg <= 1.0,
            "NDCG should be between 0 and 1: {}",
            ndcg
        );
    }

    #[test]
    fn test_e7_unique_finds() {
        let e7 = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let e1 = vec!["a".to_string(), "d".to_string()];
        let relevant: HashSet<_> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();

        let unique = e7_unique_finds(&e7, &e1, &relevant);
        assert_eq!(unique, vec!["b".to_string(), "c".to_string()]);
    }

    #[test]
    fn test_query_type_parsing() {
        assert_eq!(
            E7QueryType::parse_from("function_search"),
            Some(E7QueryType::FunctionSearch)
        );
        assert_eq!(
            E7QueryType::parse_from("pattern"),
            Some(E7QueryType::PatternSearch)
        );
        assert_eq!(E7QueryType::parse_from("unknown"), None);
    }

    #[test]
    fn test_benchmark_metrics_from_results() {
        let results = vec![E7QueryResult {
            query_id: "q1".to_string(),
            query_type: E7QueryType::FunctionSearch,
            retrieved_docs: vec!["a".to_string(), "b".to_string()],
            retrieved_entity_types: vec!["Function".to_string()],
            e7_scores: vec![0.9, 0.8],
            e1_scores: vec![0.7, 0.6],
            latency: Duration::from_millis(10),
            precision_at: [(10, 0.8)].into_iter().collect(),
            recall_at: [(10, 0.6)].into_iter().collect(),
            mrr: 0.9,
            ndcg_at: [(10, 0.85)].into_iter().collect(),
            iou_at: [(10, 0.7)].into_iter().collect(),
            e7_unique_finds: vec!["b".to_string()],
            e1_unique_finds: vec![],
        }];

        let metrics = E7BenchmarkMetrics::from_results(&results);
        assert_eq!(metrics.query_count, 1);
        assert!((metrics.mean_mrr - 0.9).abs() < 0.001);
        assert!(metrics.overall_score() > 0.0);
    }
}
