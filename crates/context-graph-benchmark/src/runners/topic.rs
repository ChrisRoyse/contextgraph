//! Topic detection benchmark runner.
//!
//! Evaluates clustering quality using Purity, NMI, ARI, and Silhouette metrics.

use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

use crate::baseline::SingleEmbedderBaseline;
use crate::datasets::BenchmarkDataset;
use crate::metrics::clustering::{self, ClusteringMetrics};
use crate::metrics::performance::{MemoryStats, PerformanceMetrics};

/// Topic detection benchmark runner.
pub struct TopicRunner {
    /// Number of expected topics (from ground truth).
    expected_topics: Option<usize>,
    /// Maximum k-means iterations.
    max_iterations: usize,
}

impl TopicRunner {
    /// Create a new runner.
    pub fn new() -> Self {
        Self {
            expected_topics: None,
            max_iterations: 100,
        }
    }

    /// Set expected number of topics.
    pub fn with_expected_topics(mut self, n: usize) -> Self {
        self.expected_topics = Some(n);
        self
    }

    /// Set maximum iterations for clustering.
    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    /// Run topic detection on single-embedder baseline.
    pub fn run_single_embedder(
        &self,
        baseline: &SingleEmbedderBaseline,
        dataset: &BenchmarkDataset,
    ) -> TopicBenchmarkResults {
        let n_clusters = self.expected_topics.unwrap_or(dataset.topic_count());

        let start = Instant::now();

        // Run k-means clustering
        let predicted_labels = baseline.detect_topics_kmeans(n_clusters, self.max_iterations);

        let clustering_time = start.elapsed();

        // Convert to vectors for metric computation
        let ids: Vec<Uuid> = dataset.fingerprints.iter().map(|(id, _)| *id).collect();
        let predicted: Vec<usize> = ids
            .iter()
            .map(|id| predicted_labels.get(id).copied().unwrap_or(0))
            .collect();
        let true_labels: Vec<usize> = ids
            .iter()
            .map(|id| dataset.topic_assignments.get(id).copied().unwrap_or(0))
            .collect();

        // Compute distance matrix for silhouette (using E1 embeddings)
        let distance_matrix = compute_distance_matrix(baseline, &ids);

        // Compute metrics
        let metrics =
            clustering::compute_all_metrics(&predicted, &true_labels, Some(&distance_matrix));

        let performance = PerformanceMetrics {
            latency_us: crate::metrics::performance::LatencyStats::from_measurements(&[
                clustering_time.as_micros() as u64,
            ]),
            throughput_ops_sec: 1.0 / clustering_time.as_secs_f64(),
            memory: MemoryStats::default(),
            index_build_time_us: clustering_time.as_micros() as u64,
            operation_count: 1,
        };

        TopicBenchmarkResults {
            metrics,
            performance,
            predicted_clusters: n_clusters,
            true_clusters: dataset.topic_count(),
        }
    }

    /// Run topic detection on multi-space system.
    pub fn run_multi_space(&self, dataset: &BenchmarkDataset) -> TopicBenchmarkResults {
        let n_clusters = self.expected_topics.unwrap_or(dataset.topic_count());

        let start = Instant::now();

        // Use multi-space clustering (weighted combination of semantic embedders)
        let predicted_labels = multi_space_clustering(dataset, n_clusters, self.max_iterations);

        let clustering_time = start.elapsed();

        // Convert to vectors for metric computation
        let ids: Vec<Uuid> = dataset.fingerprints.iter().map(|(id, _)| *id).collect();
        let predicted: Vec<usize> = ids
            .iter()
            .map(|id| predicted_labels.get(id).copied().unwrap_or(0))
            .collect();
        let true_labels: Vec<usize> = ids
            .iter()
            .map(|id| dataset.topic_assignments.get(id).copied().unwrap_or(0))
            .collect();

        // Compute distance matrix for silhouette (using multi-space distance)
        let distance_matrix = compute_multispace_distance_matrix(dataset, &ids);

        // Compute metrics
        let metrics =
            clustering::compute_all_metrics(&predicted, &true_labels, Some(&distance_matrix));

        let performance = PerformanceMetrics {
            latency_us: crate::metrics::performance::LatencyStats::from_measurements(&[
                clustering_time.as_micros() as u64,
            ]),
            throughput_ops_sec: 1.0 / clustering_time.as_secs_f64(),
            memory: MemoryStats::default(),
            index_build_time_us: clustering_time.as_micros() as u64,
            operation_count: 1,
        };

        TopicBenchmarkResults {
            metrics,
            performance,
            predicted_clusters: n_clusters,
            true_clusters: dataset.topic_count(),
        }
    }
}

impl Default for TopicRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Results from a topic detection benchmark run.
#[derive(Debug, Clone)]
pub struct TopicBenchmarkResults {
    /// Clustering quality metrics.
    pub metrics: ClusteringMetrics,
    /// Performance metrics.
    pub performance: PerformanceMetrics,
    /// Number of predicted clusters.
    pub predicted_clusters: usize,
    /// Number of true clusters (ground truth).
    pub true_clusters: usize,
}

/// Compute distance matrix using E1 embeddings.
fn compute_distance_matrix(baseline: &SingleEmbedderBaseline, ids: &[Uuid]) -> Vec<Vec<f64>> {
    let n = ids.len();
    let mut matrix = vec![vec![0.0; n]; n];

    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }

            if let (Some(a), Some(b)) = (
                baseline.get_embedding(&ids[i]),
                baseline.get_embedding(&ids[j]),
            ) {
                // Distance = 1 - cosine_similarity
                let sim = cosine_similarity(a, b);
                matrix[i][j] = (1.0 - sim) as f64;
            }
        }
    }

    matrix
}

/// Compute multi-space distance matrix.
fn compute_multispace_distance_matrix(dataset: &BenchmarkDataset, ids: &[Uuid]) -> Vec<Vec<f64>> {
    let n = ids.len();
    let mut matrix = vec![vec![0.0; n]; n];

    // Get fingerprints by ID
    let id_to_fp: HashMap<Uuid, &context_graph_core::types::fingerprint::SemanticFingerprint> =
        dataset
            .fingerprints
            .iter()
            .map(|(id, fp)| (*id, fp))
            .collect();

    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }

            if let (Some(fp_a), Some(fp_b)) = (id_to_fp.get(&ids[i]), id_to_fp.get(&ids[j])) {
                // Weighted combination of semantic embedders
                let sim_e1 = cosine_similarity(&fp_a.e1_semantic, &fp_b.e1_semantic);
                let sim_e5 = cosine_similarity(fp_a.e5_active_vector(), fp_b.e5_active_vector());
                let sim_e7 = cosine_similarity(&fp_a.e7_code, &fp_b.e7_code);
                let sim_e10 = cosine_similarity(
                    &fp_a.e10_multimodal_paraphrase,
                    &fp_b.e10_multimodal_paraphrase,
                );

                // Equal weights for semantic embedders
                let avg_sim = (sim_e1 + sim_e5 + sim_e7 + sim_e10) / 4.0;
                matrix[i][j] = (1.0 - avg_sim) as f64;
            }
        }
    }

    matrix
}

/// Multi-space k-means clustering.
fn multi_space_clustering(
    dataset: &BenchmarkDataset,
    n_clusters: usize,
    max_iters: usize,
) -> HashMap<Uuid, usize> {
    if dataset.fingerprints.is_empty() || n_clusters == 0 {
        return HashMap::new();
    }

    let ids: Vec<Uuid> = dataset.fingerprints.iter().map(|(id, _)| *id).collect();

    // Concatenate semantic embeddings for clustering
    // E1 (1024) + E5 (768) + E7 (1536) + E10 (768) = 4096 dims
    let embeddings: Vec<Vec<f32>> = dataset
        .fingerprints
        .iter()
        .map(|(_, fp)| {
            let mut concat = fp.e1_semantic.clone();
            concat.extend_from_slice(fp.e5_active_vector());
            concat.extend_from_slice(&fp.e7_code);
            concat.extend_from_slice(&fp.e10_multimodal_paraphrase);
            concat
        })
        .collect();

    let dim = embeddings.first().map(|e| e.len()).unwrap_or(0);

    // Initialize centroids
    let mut centroids: Vec<Vec<f32>> = embeddings.iter().take(n_clusters).cloned().collect();

    while centroids.len() < n_clusters {
        centroids.push(vec![0.0; dim]);
    }

    let mut assignments = vec![0usize; embeddings.len()];

    for _ in 0..max_iters {
        // Assign to nearest centroid
        let mut changed = false;
        for (i, embedding) in embeddings.iter().enumerate() {
            let mut best_cluster = 0;
            let mut best_dist = f32::MAX;

            for (j, centroid) in centroids.iter().enumerate() {
                let dist: f32 = embedding
                    .iter()
                    .zip(centroid.iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum();

                if dist < best_dist {
                    best_dist = dist;
                    best_cluster = j;
                }
            }

            if assignments[i] != best_cluster {
                assignments[i] = best_cluster;
                changed = true;
            }
        }

        if !changed {
            break;
        }

        // Update centroids
        for (j, centroid) in centroids.iter_mut().enumerate() {
            let cluster_points: Vec<&Vec<f32>> = embeddings
                .iter()
                .zip(assignments.iter())
                .filter(|(_, &a)| a == j)
                .map(|(e, _)| e)
                .collect();

            if cluster_points.is_empty() {
                continue;
            }

            for d in 0..dim {
                centroid[d] =
                    cluster_points.iter().map(|e| e[d]).sum::<f32>() / cluster_points.len() as f32;
            }
        }
    }

    ids.into_iter().zip(assignments).collect()
}

/// Compute cosine similarity (raw [-1, 1] range).
///
/// Delegates to the canonical implementation in `crate::util`.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    crate::util::cosine_similarity_raw(a, b)
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;
    use crate::config::{Tier, TierConfig};
    use crate::datasets::DatasetGenerator;

    #[test]
    fn test_topic_runner() {
        let mut generator = DatasetGenerator::new(42);
        let config = TierConfig::for_tier(Tier::Tier0);
        let dataset = generator.generate_dataset(&config);

        // Build baseline
        let mut baseline = SingleEmbedderBaseline::new();
        for (id, fp) in &dataset.fingerprints {
            baseline.insert(*id, &fp.e1_semantic);
        }

        let runner = TopicRunner::new().with_expected_topics(config.topic_count);
        let results = runner.run_single_embedder(&baseline, &dataset);

        // Should have computed metrics
        assert!(results.metrics.purity >= 0.0);
        assert_eq!(results.predicted_clusters, config.topic_count);
    }
}
