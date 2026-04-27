//! Retrieval benchmark runner.
//!
//! Evaluates retrieval quality using P@K, R@K, MRR, NDCG, and MAP metrics.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::baseline::SingleEmbedderBaseline;
use crate::datasets::BenchmarkDataset;
use crate::metrics::performance::{LatencyTracker, MemoryStats, PerformanceMetrics};
use crate::metrics::retrieval::{self, RetrievalMetrics};
use crate::util::{cosine_similarity, similarity_sort_desc};

/// Retrieval benchmark runner.
pub struct RetrievalRunner {
    k_values: Vec<usize>,
    warmup_iterations: usize,
}

impl RetrievalRunner {
    /// Create a new runner with default K values.
    pub fn new() -> Self {
        Self {
            k_values: vec![1, 5, 10, 20, 50],
            warmup_iterations: 5,
        }
    }

    /// Set K values for P@K and R@K.
    pub fn with_k_values(mut self, k_values: Vec<usize>) -> Self {
        self.k_values = k_values;
        self
    }

    /// Set warmup iterations.
    pub fn with_warmup(mut self, iterations: usize) -> Self {
        self.warmup_iterations = iterations;
        self
    }

    /// Run retrieval benchmark on single-embedder baseline.
    pub fn run_single_embedder(
        &self,
        baseline: &SingleEmbedderBaseline,
        dataset: &BenchmarkDataset,
    ) -> RetrievalBenchmarkResults {
        let max_k = *self.k_values.iter().max().unwrap_or(&10);
        let mut latency_tracker = LatencyTracker::with_capacity(dataset.queries.len());

        // Warmup
        for query in dataset.queries.iter().take(self.warmup_iterations) {
            let _ = baseline.search(&query.embedding, max_k);
        }

        // Collect results
        let start = Instant::now();
        let mut query_results: Vec<(Vec<Uuid>, HashSet<Uuid>)> = Vec::new();

        for query in &dataset.queries {
            let query_start = Instant::now();
            let results = baseline.search(&query.embedding, max_k);
            latency_tracker.record_duration(query_start.elapsed());

            let retrieved_ids: Vec<Uuid> = results.iter().map(|(id, _)| *id).collect();
            query_results.push((retrieved_ids, query.relevant_docs.clone()));
        }

        let total_duration = start.elapsed();

        // Compute metrics
        let metrics = retrieval::compute_all_metrics(&query_results, &self.k_values);

        let performance = PerformanceMetrics::from_measurements(
            latency_tracker.measurements(),
            total_duration,
            MemoryStats::default(),
            Duration::ZERO,
        );

        RetrievalBenchmarkResults {
            metrics,
            performance,
            query_count: dataset.queries.len(),
        }
    }

    /// Run retrieval benchmark on multi-space system.
    ///
    /// Uses 4 embedding spaces (E1, E5, E7, E10) for simplified multi-space search.
    pub fn run_multi_space(&self, dataset: &BenchmarkDataset) -> RetrievalBenchmarkResults {
        let max_k = *self.k_values.iter().max().unwrap_or(&10);
        let mut latency_tracker = LatencyTracker::with_capacity(dataset.queries.len());

        // Build multi-space search index (simplified for benchmark)
        // In production, this would use the full TeleologicalMemoryStore
        let multi_space_index = MultiSpaceIndex::from_dataset(dataset);

        // Warmup
        for _query in dataset.queries.iter().take(self.warmup_iterations) {
            let fp = dataset.get_fingerprint(&dataset.fingerprints[0].0).unwrap();
            let _ = multi_space_index.search(fp, max_k);
        }

        // Collect results
        let start = Instant::now();
        let mut query_results: Vec<(Vec<Uuid>, HashSet<Uuid>)> = Vec::new();

        for query in &dataset.queries {
            let query_start = Instant::now();

            // Use full multi-space search with query fingerprint
            // This searches across E1, E5, E7, E10 with weighted combination
            let results = multi_space_index.search(&query.fingerprint, max_k);
            latency_tracker.record_duration(query_start.elapsed());

            let retrieved_ids: Vec<Uuid> = results.iter().map(|(id, _)| *id).collect();
            query_results.push((retrieved_ids, query.relevant_docs.clone()));
        }

        let total_duration = start.elapsed();

        // Compute metrics
        let metrics = retrieval::compute_all_metrics(&query_results, &self.k_values);

        let performance = PerformanceMetrics::from_measurements(
            latency_tracker.measurements(),
            total_duration,
            MemoryStats::default(),
            Duration::ZERO,
        );

        RetrievalBenchmarkResults {
            metrics,
            performance,
            query_count: dataset.queries.len(),
        }
    }
}

impl Default for RetrievalRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Results from a retrieval benchmark run.
#[derive(Debug, Clone)]
pub struct RetrievalBenchmarkResults {
    /// Retrieval quality metrics.
    pub metrics: RetrievalMetrics,
    /// Performance metrics.
    pub performance: PerformanceMetrics,
    /// Number of queries evaluated.
    pub query_count: usize,
}

/// Simplified multi-space index for benchmarking.
///
/// This implements a weighted combination of multiple embedding spaces
/// without the full complexity of the production system.
struct MultiSpaceIndex {
    /// E1 (semantic) index.
    e1_index: Vec<(Uuid, Vec<f32>)>,
    /// E5 (causal) index.
    e5_index: Vec<(Uuid, Vec<f32>)>,
    /// E7 (code) index.
    e7_index: Vec<(Uuid, Vec<f32>)>,
    /// E10 (multimodal) index.
    e10_index: Vec<(Uuid, Vec<f32>)>,
}

impl MultiSpaceIndex {
    /// Build index from dataset.
    fn from_dataset(dataset: &BenchmarkDataset) -> Self {
        let mut e1_index = Vec::with_capacity(dataset.fingerprints.len());
        let mut e5_index = Vec::with_capacity(dataset.fingerprints.len());
        let mut e7_index = Vec::with_capacity(dataset.fingerprints.len());
        let mut e10_index = Vec::with_capacity(dataset.fingerprints.len());

        for (id, fp) in &dataset.fingerprints {
            e1_index.push((*id, fp.e1_semantic.clone()));
            e5_index.push((*id, fp.e5_active_vector().to_vec()));
            e7_index.push((*id, fp.e7_code.clone()));
            e10_index.push((*id, fp.e10_multimodal_paraphrase.clone()));
        }

        Self {
            e1_index,
            e5_index,
            e7_index,
            e10_index,
        }
    }

    /// Search using multiple spaces (for fingerprint queries).
    ///
    /// Computes weighted similarity across E1, E5, E7, and E10 embedding spaces.
    fn search(
        &self,
        query_fp: &context_graph_core::types::fingerprint::SemanticFingerprint,
        k: usize,
    ) -> Vec<(Uuid, f32)> {
        let mut scores: HashMap<Uuid, f32> = HashMap::new();

        // Accumulate weighted similarities from each space
        #[allow(clippy::type_complexity)]
        let spaces: [(&[(Uuid, Vec<f32>)], &[f32]); 4] = [
            (&self.e1_index, &query_fp.e1_semantic),
            (&self.e5_index, query_fp.e5_active_vector()),
            (&self.e7_index, &query_fp.e7_code),
            (&self.e10_index, &query_fp.e10_multimodal_paraphrase),
        ];

        for (index, query_vec) in spaces {
            for (id, vec) in index {
                let sim = cosine_similarity(query_vec, vec);
                *scores.entry(*id).or_insert(0.0) += sim;
            }
        }

        // Normalize by number of spaces
        let num_spaces = spaces.len() as f32;
        let mut results: Vec<(Uuid, f32)> = scores
            .into_iter()
            .map(|(id, score)| (id, score / num_spaces))
            .collect();

        results.sort_by(similarity_sort_desc);
        results.truncate(k);
        results
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;
    use crate::config::{Tier, TierConfig};
    use crate::datasets::DatasetGenerator;

    #[test]
    fn test_retrieval_runner() {
        let mut generator = DatasetGenerator::new(42);
        let config = TierConfig::for_tier(Tier::Tier0);
        let dataset = generator.generate_dataset(&config);

        // Build baseline
        let mut baseline = SingleEmbedderBaseline::new();
        for (id, fp) in &dataset.fingerprints {
            baseline.insert(*id, &fp.e1_semantic);
        }

        let runner = RetrievalRunner::new();
        let results = runner.run_single_embedder(&baseline, &dataset);

        // Should have computed meaningful metrics (exact match retrieval should achieve MRR > 0.1)
        assert!(
            results.metrics.mrr >= 0.1,
            "MRR too low: {} — expected at least 0.1 for exact-match queries",
            results.metrics.mrr
        );
        assert!(results.performance.operation_count > 0);
    }
}
