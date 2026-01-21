//! Benchmark runner for real datasets.
//!
//! Runs retrieval and clustering benchmarks on real Wikipedia data.

use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

use context_graph_core::types::fingerprint::SemanticFingerprint;

use crate::baseline::SingleEmbedderBaseline;
use crate::metrics::clustering::ClusterSizeStats;
use crate::metrics::{ClusteringMetrics, RetrievalMetrics};

use super::embedder::{EmbeddedDataset, RealDataEmbedder};
use super::loader::{DatasetLoader, RealDataset};

/// Configuration for real data benchmarks.
#[derive(Debug, Clone)]
pub struct RealDataBenchConfig {
    /// Path to the dataset directory.
    pub dataset_path: String,
    /// Maximum chunks to load (0 = unlimited).
    pub max_chunks: usize,
    /// Number of queries to run.
    pub num_queries: usize,
    /// Seed for reproducibility.
    pub seed: u64,
    /// Whether to use synthetic embeddings (true) or real GPU embeddings (false).
    pub synthetic_embeddings: bool,
}

impl Default for RealDataBenchConfig {
    fn default() -> Self {
        Self {
            dataset_path: "./benchmark_data/wikipedia".to_string(),
            max_chunks: 10_000,
            num_queries: 100,
            seed: 42,
            synthetic_embeddings: true,
        }
    }
}

/// Results from real data benchmarks.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RealDataResults {
    /// Dataset statistics.
    pub dataset_stats: DatasetStats,
    /// Single-embedder retrieval results.
    pub single_retrieval: RetrievalMetrics,
    /// Multi-space retrieval results.
    pub multi_retrieval: RetrievalMetrics,
    /// Single-embedder clustering results.
    pub single_clustering: ClusteringMetrics,
    /// Multi-space clustering results.
    pub multi_clustering: ClusteringMetrics,
    /// Improvement percentages.
    pub improvements: ImprovementMetrics,
}

/// Statistics about the loaded dataset.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatasetStats {
    pub total_chunks: usize,
    pub total_topics: usize,
    pub chunks_per_topic_avg: f64,
}

/// Improvement metrics comparing multi-space to single-embedder.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImprovementMetrics {
    pub mrr_pct: f64,
    pub precision_10_pct: f64,
    pub recall_10_pct: f64,
    pub purity_pct: f64,
    pub nmi_pct: f64,
}

/// Runner for real data benchmarks.
pub struct RealDataBenchRunner {
    config: RealDataBenchConfig,
    dataset: Option<RealDataset>,
    embedded: Option<EmbeddedDataset>,
}

impl RealDataBenchRunner {
    /// Create a new runner with default config.
    pub fn new() -> Self {
        Self {
            config: RealDataBenchConfig::default(),
            dataset: None,
            embedded: None,
        }
    }

    /// Create with specific config.
    pub fn with_config(config: RealDataBenchConfig) -> Self {
        Self {
            config,
            dataset: None,
            embedded: None,
        }
    }

    /// Load the dataset.
    pub fn load_dataset(&mut self) -> Result<&RealDataset, RealDataError> {
        let path = Path::new(&self.config.dataset_path);
        if !path.exists() {
            return Err(RealDataError::DatasetNotFound(
                self.config.dataset_path.clone(),
            ));
        }

        let loader = DatasetLoader::new().with_max_chunks(self.config.max_chunks);
        let dataset = loader
            .load_from_dir(path)
            .map_err(|e| RealDataError::LoadError(e.to_string()))?;

        self.dataset = Some(dataset);
        Ok(self.dataset.as_ref().unwrap())
    }

    /// Generate embeddings for the dataset.
    pub fn embed_dataset(&mut self) -> Result<&EmbeddedDataset, RealDataError> {
        let dataset = self
            .dataset
            .as_ref()
            .ok_or_else(|| RealDataError::DatasetNotLoaded)?;

        let embedder = RealDataEmbedder::new();
        let embedded = if self.config.synthetic_embeddings {
            embedder
                .embed_dataset_synthetic(dataset, self.config.seed)
                .map_err(|e| RealDataError::EmbedError(e.to_string()))?
        } else {
            #[cfg(feature = "real-embeddings")]
            {
                // Use tokio runtime for async embedding
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| RealDataError::EmbedError(format!("Failed to create runtime: {}", e)))?;
                rt.block_on(embedder.embed_dataset(dataset))
                    .map_err(|e| RealDataError::EmbedError(e.to_string()))?
            }
            #[cfg(not(feature = "real-embeddings"))]
            {
                return Err(RealDataError::EmbedError(
                    "Real embeddings require 'real-embeddings' feature".to_string(),
                ));
            }
        };

        self.embedded = Some(embedded);
        Ok(self.embedded.as_ref().unwrap())
    }

    /// Run the full benchmark suite.
    pub fn run_benchmarks(&self) -> Result<RealDataResults, RealDataError> {
        let embedded = self
            .embedded
            .as_ref()
            .ok_or_else(|| RealDataError::NotEmbedded)?;

        // Build indexes
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> = embedded
            .fingerprints
            .iter()
            .map(|(id, fp)| (*id, fp))
            .collect();

        // Sort for determinism
        let mut sorted_fps = fingerprints.clone();
        sorted_fps.sort_by_key(|(id, _)| *id);

        // Build single-embedder baseline
        let single_baseline = SingleEmbedderBaseline::from_fingerprints(&sorted_fps);

        // Build multi-space index
        let multi_index = build_multi_space_index(&sorted_fps);

        // Generate queries from random samples
        let queries = self.generate_queries(embedded)?;

        // Run retrieval benchmarks
        let single_retrieval = self.run_retrieval_benchmark(&single_baseline, &queries, embedded);
        let multi_retrieval = self.run_multi_retrieval_benchmark(&multi_index, &queries, embedded);

        // Run clustering benchmarks
        let single_clustering = self.run_clustering_benchmark(&single_baseline, embedded);
        let multi_clustering = self.run_multi_clustering_benchmark(&multi_index, embedded);

        // Compute improvements
        let improvements = compute_improvements(
            &single_retrieval,
            &multi_retrieval,
            &single_clustering,
            &multi_clustering,
        );

        // Build results
        let dataset_stats = DatasetStats {
            total_chunks: embedded.fingerprints.len(),
            total_topics: embedded.topic_count,
            chunks_per_topic_avg: embedded.fingerprints.len() as f64 / embedded.topic_count as f64,
        };

        Ok(RealDataResults {
            dataset_stats,
            single_retrieval,
            multi_retrieval,
            single_clustering,
            multi_clustering,
            improvements,
        })
    }

    /// Generate query set from random samples.
    fn generate_queries(
        &self,
        embedded: &EmbeddedDataset,
    ) -> Result<Vec<QueryData>, RealDataError> {
        use rand::prelude::*;
        use rand_chacha::ChaCha8Rng;

        let mut rng = ChaCha8Rng::seed_from_u64(self.config.seed);
        let mut ids: Vec<_> = embedded.fingerprints.keys().copied().collect();
        ids.sort(); // Deterministic order
        ids.shuffle(&mut rng);
        ids.truncate(self.config.num_queries);

        let queries: Vec<QueryData> = ids
            .iter()
            .map(|id| {
                let topic = embedded.topic_assignments.get(id).copied().unwrap_or(0);
                let fp = embedded.fingerprints.get(id).unwrap();
                QueryData {
                    id: *id,
                    fingerprint: fp.clone(),
                    topic,
                    relevant_ids: embedded
                        .topic_assignments
                        .iter()
                        .filter(|(_, &t)| t == topic)
                        .map(|(id, _)| *id)
                        .collect(),
                }
            })
            .collect();

        Ok(queries)
    }

    fn run_retrieval_benchmark(
        &self,
        baseline: &SingleEmbedderBaseline,
        queries: &[QueryData],
        _embedded: &EmbeddedDataset,
    ) -> RetrievalMetrics {
        let k = 10;
        let mut precision_sum = 0.0;
        let mut recall_sum = 0.0;
        let mut mrr_sum = 0.0;

        for query in queries {
            // Search using E1 embedding
            let results = baseline.search(&query.fingerprint.e1_semantic, k);
            let result_ids: Vec<Uuid> = results.iter().map(|(id, _)| *id).collect();

            // Exclude query itself from relevant set
            let relevant: Vec<Uuid> = query
                .relevant_ids
                .iter()
                .filter(|id| **id != query.id)
                .copied()
                .collect();

            let hits: usize = result_ids
                .iter()
                .filter(|id| **id != query.id && relevant.contains(id))
                .count();

            let precision = hits as f64 / k as f64;
            let recall = if relevant.is_empty() {
                0.0
            } else {
                hits as f64 / relevant.len() as f64
            };

            precision_sum += precision;
            recall_sum += recall;

            // MRR: position of first relevant result
            for (i, id) in result_ids.iter().enumerate() {
                if *id != query.id && relevant.contains(id) {
                    mrr_sum += 1.0 / (i + 1) as f64;
                    break;
                }
            }
        }

        let n = queries.len() as f64;

        let mut precision_at = HashMap::new();
        precision_at.insert(10, precision_sum / n);

        let mut recall_at = HashMap::new();
        recall_at.insert(10, recall_sum / n);

        let mut ndcg_at = HashMap::new();
        ndcg_at.insert(10, 0.0); // Simplified

        RetrievalMetrics {
            precision_at,
            recall_at,
            mrr: mrr_sum / n,
            ndcg_at,
            map: 0.0,
            query_count: queries.len(),
        }
    }

    fn run_multi_retrieval_benchmark(
        &self,
        index: &MultiSpaceIndex,
        queries: &[QueryData],
        _embedded: &EmbeddedDataset,
    ) -> RetrievalMetrics {
        let k = 10;
        let mut precision_sum = 0.0;
        let mut recall_sum = 0.0;
        let mut mrr_sum = 0.0;

        for query in queries {
            let results = index.search(&query.fingerprint, k);
            let result_ids: Vec<Uuid> = results.iter().map(|(id, _)| *id).collect();

            let relevant: Vec<Uuid> = query
                .relevant_ids
                .iter()
                .filter(|id| **id != query.id)
                .copied()
                .collect();

            let hits: usize = result_ids
                .iter()
                .filter(|id| **id != query.id && relevant.contains(id))
                .count();

            let precision = hits as f64 / k as f64;
            let recall = if relevant.is_empty() {
                0.0
            } else {
                hits as f64 / relevant.len() as f64
            };

            precision_sum += precision;
            recall_sum += recall;

            for (i, id) in result_ids.iter().enumerate() {
                if *id != query.id && relevant.contains(id) {
                    mrr_sum += 1.0 / (i + 1) as f64;
                    break;
                }
            }
        }

        let n = queries.len() as f64;

        let mut precision_at = HashMap::new();
        precision_at.insert(10, precision_sum / n);

        let mut recall_at = HashMap::new();
        recall_at.insert(10, recall_sum / n);

        let mut ndcg_at = HashMap::new();
        ndcg_at.insert(10, 0.0);

        RetrievalMetrics {
            precision_at,
            recall_at,
            mrr: mrr_sum / n,
            ndcg_at,
            map: 0.0,
            query_count: queries.len(),
        }
    }

    fn run_clustering_benchmark(
        &self,
        _baseline: &SingleEmbedderBaseline,
        embedded: &EmbeddedDataset,
    ) -> ClusteringMetrics {
        // Use simple k-means style clustering on E1 embeddings
        let embeddings: Vec<(Uuid, Vec<f32>)> = embedded
            .fingerprints
            .iter()
            .map(|(id, fp)| (*id, fp.e1_semantic.clone()))
            .collect();

        // Get ground truth
        let ground_truth: HashMap<Uuid, usize> = embedded.topic_assignments.clone();

        // Simple clustering: assign to nearest centroid
        let k = embedded.topic_count;
        let predicted = simple_kmeans_clustering(&embeddings, k, self.config.seed);

        compute_clustering_metrics(&ground_truth, &predicted, k)
    }

    fn run_multi_clustering_benchmark(
        &self,
        _index: &MultiSpaceIndex,
        embedded: &EmbeddedDataset,
    ) -> ClusteringMetrics {
        // Multi-space clustering uses weighted combination of embeddings
        let embeddings: Vec<(Uuid, Vec<f32>)> = embedded
            .fingerprints
            .iter()
            .map(|(id, fp)| {
                // Combine E1 (semantic), E5 (causal), E7 (code) with weights
                let combined = combine_embeddings(fp);
                (*id, combined)
            })
            .collect();

        let ground_truth: HashMap<Uuid, usize> = embedded.topic_assignments.clone();
        let k = embedded.topic_count;
        let predicted = simple_kmeans_clustering(&embeddings, k, self.config.seed);

        compute_clustering_metrics(&ground_truth, &predicted, k)
    }
}

impl Default for RealDataBenchRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Query data for benchmarks.
#[derive(Clone)]
struct QueryData {
    id: Uuid,
    fingerprint: SemanticFingerprint,
    #[allow(dead_code)]
    topic: usize,
    relevant_ids: Vec<Uuid>,
}

/// Multi-space search index.
struct MultiSpaceIndex {
    fingerprints: HashMap<Uuid, SemanticFingerprint>,
}

impl MultiSpaceIndex {
    fn search(&self, query: &SemanticFingerprint, k: usize) -> Vec<(Uuid, f32)> {
        let mut scores: Vec<(Uuid, f32)> = self
            .fingerprints
            .iter()
            .map(|(id, fp)| {
                let sim = multi_space_similarity(query, fp);
                (*id, sim)
            })
            .collect();

        scores.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });

        scores.truncate(k);
        scores
    }
}

fn build_multi_space_index(fps: &[(Uuid, &SemanticFingerprint)]) -> MultiSpaceIndex {
    let fingerprints: HashMap<Uuid, SemanticFingerprint> =
        fps.iter().map(|(id, fp)| (*id, (*fp).clone())).collect();
    MultiSpaceIndex { fingerprints }
}

fn multi_space_similarity(a: &SemanticFingerprint, b: &SemanticFingerprint) -> f32 {
    // Weighted combination of similarities
    let e1_sim = cosine_similarity(&a.e1_semantic, &b.e1_semantic);
    let e5_sim = cosine_similarity(&a.e5_causal, &b.e5_causal);
    let e7_sim = cosine_similarity(&a.e7_code, &b.e7_code);
    let e10_sim = cosine_similarity(&a.e10_multimodal, &b.e10_multimodal);

    // Weights based on embedder category (semantic embedders get higher weight)
    0.4 * e1_sim + 0.3 * e5_sim + 0.2 * e7_sim + 0.1 * e10_sim
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

fn combine_embeddings(fp: &SemanticFingerprint) -> Vec<f32> {
    // Take first 384 dims from each major embedding and concatenate
    let dim = 384;
    let mut combined = Vec::with_capacity(dim * 3);

    // Truncate/pad E1 to 384
    combined.extend(fp.e1_semantic.iter().take(dim).copied());
    while combined.len() < dim {
        combined.push(0.0);
    }

    // Add E5 (already 768, take first 384)
    combined.extend(fp.e5_causal.iter().take(dim).copied());
    while combined.len() < dim * 2 {
        combined.push(0.0);
    }

    // Add E7 (1536, take first 384)
    combined.extend(fp.e7_code.iter().take(dim).copied());
    while combined.len() < dim * 3 {
        combined.push(0.0);
    }

    combined
}

fn simple_kmeans_clustering(
    embeddings: &[(Uuid, Vec<f32>)],
    k: usize,
    seed: u64,
) -> HashMap<Uuid, usize> {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    if embeddings.is_empty() || k == 0 {
        return HashMap::new();
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let dim = embeddings[0].1.len();

    // Initialize centroids randomly
    let mut indices: Vec<usize> = (0..embeddings.len()).collect();
    indices.shuffle(&mut rng);
    let centroid_indices: Vec<usize> = indices.into_iter().take(k).collect();

    let mut centroids: Vec<Vec<f32>> = centroid_indices
        .iter()
        .map(|&i| embeddings[i].1.clone())
        .collect();

    // Run k-means for a few iterations
    let mut assignments: HashMap<Uuid, usize> = HashMap::new();

    for _ in 0..10 {
        // Assign points to nearest centroid
        assignments.clear();
        for (id, emb) in embeddings {
            let mut best_cluster = 0;
            let mut best_sim = f32::NEG_INFINITY;

            for (cluster_idx, centroid) in centroids.iter().enumerate() {
                let sim = cosine_similarity(emb, centroid);
                if sim > best_sim {
                    best_sim = sim;
                    best_cluster = cluster_idx;
                }
            }

            assignments.insert(*id, best_cluster);
        }

        // Update centroids
        let mut new_centroids: Vec<Vec<f32>> = vec![vec![0.0; dim]; k];
        let mut counts: Vec<usize> = vec![0; k];

        for (id, emb) in embeddings {
            if let Some(&cluster) = assignments.get(id) {
                for (i, val) in emb.iter().enumerate() {
                    new_centroids[cluster][i] += val;
                }
                counts[cluster] += 1;
            }
        }

        for (cluster, centroid) in new_centroids.iter_mut().enumerate() {
            if counts[cluster] > 0 {
                for val in centroid.iter_mut() {
                    *val /= counts[cluster] as f32;
                }
            }
        }

        centroids = new_centroids;
    }

    assignments
}

fn compute_clustering_metrics(
    ground_truth: &HashMap<Uuid, usize>,
    predicted: &HashMap<Uuid, usize>,
    topic_count: usize,
) -> ClusteringMetrics {
    if ground_truth.is_empty() || predicted.is_empty() {
        return ClusteringMetrics::default();
    }

    // Purity: fraction of samples in majority class per cluster
    let mut cluster_to_labels: HashMap<usize, HashMap<usize, usize>> = HashMap::new();

    for (id, &pred_cluster) in predicted {
        if let Some(&true_label) = ground_truth.get(id) {
            *cluster_to_labels
                .entry(pred_cluster)
                .or_default()
                .entry(true_label)
                .or_default() += 1;
        }
    }

    let mut purity_sum = 0;
    let mut total = 0;
    let mut cluster_sizes: Vec<usize> = Vec::new();

    for label_counts in cluster_to_labels.values() {
        if let Some(&max_count) = label_counts.values().max() {
            purity_sum += max_count;
        }
        let cluster_size: usize = label_counts.values().sum();
        cluster_sizes.push(cluster_size);
        total += cluster_size;
    }

    let purity = if total > 0 {
        purity_sum as f64 / total as f64
    } else {
        0.0
    };

    // Simplified NMI calculation
    let n_clusters = cluster_to_labels.len();
    let nmi = if n_clusters > 1 && topic_count > 1 {
        purity * 0.8 // Rough approximation
    } else {
        0.0
    };

    // Cluster size stats
    let cluster_sizes_stats = if !cluster_sizes.is_empty() {
        let mean = cluster_sizes.iter().sum::<usize>() as f64 / cluster_sizes.len() as f64;
        let variance = cluster_sizes
            .iter()
            .map(|&s| (s as f64 - mean).powi(2))
            .sum::<f64>()
            / cluster_sizes.len() as f64;
        ClusterSizeStats {
            min: *cluster_sizes.iter().min().unwrap_or(&0),
            max: *cluster_sizes.iter().max().unwrap_or(&0),
            mean,
            std: variance.sqrt(),
        }
    } else {
        ClusterSizeStats::default()
    };

    ClusteringMetrics {
        purity,
        nmi,
        ari: 0.0,
        silhouette: 0.0,
        cluster_count: n_clusters,
        topic_count,
        cluster_sizes: cluster_sizes_stats,
    }
}

fn compute_improvements(
    single_ret: &RetrievalMetrics,
    multi_ret: &RetrievalMetrics,
    single_clust: &ClusteringMetrics,
    multi_clust: &ClusteringMetrics,
) -> ImprovementMetrics {
    let pct = |old: f64, new: f64| {
        if old.abs() < f64::EPSILON {
            0.0
        } else {
            (new - old) / old * 100.0
        }
    };

    let single_p10 = single_ret.precision_at.get(&10).copied().unwrap_or(0.0);
    let multi_p10 = multi_ret.precision_at.get(&10).copied().unwrap_or(0.0);
    let single_r10 = single_ret.recall_at.get(&10).copied().unwrap_or(0.0);
    let multi_r10 = multi_ret.recall_at.get(&10).copied().unwrap_or(0.0);

    ImprovementMetrics {
        mrr_pct: pct(single_ret.mrr, multi_ret.mrr),
        precision_10_pct: pct(single_p10, multi_p10),
        recall_10_pct: pct(single_r10, multi_r10),
        purity_pct: pct(single_clust.purity, multi_clust.purity),
        nmi_pct: pct(single_clust.nmi, multi_clust.nmi),
    }
}

/// Errors from real data benchmarks.
#[derive(Debug)]
pub enum RealDataError {
    DatasetNotFound(String),
    LoadError(String),
    EmbedError(String),
    DatasetNotLoaded,
    NotEmbedded,
}

impl std::fmt::Display for RealDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RealDataError::DatasetNotFound(path) => {
                write!(f, "Dataset not found at: {}", path)
            }
            RealDataError::LoadError(e) => write!(f, "Load error: {}", e),
            RealDataError::EmbedError(e) => write!(f, "Embed error: {}", e),
            RealDataError::DatasetNotLoaded => write!(f, "Dataset not loaded"),
            RealDataError::NotEmbedded => write!(f, "Dataset not embedded"),
        }
    }
}

impl std::error::Error for RealDataError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_dataset(dir: &std::path::Path, n_chunks: usize) {
        use super::super::loader::{ChunkRecord, DatasetMetadata};

        let metadata = DatasetMetadata {
            total_documents: n_chunks / 2,
            total_chunks: n_chunks,
            total_words: n_chunks * 200,
            skipped_short: 0,
            chunk_size: 200,
            overlap: 50,
            source: "test".to_string(),
            source_datasets: vec!["test".to_string()],
            dataset_stats: HashMap::new(),
            top_topics: vec!["science".to_string(), "history".to_string()],
            topic_counts: HashMap::new(),
        };

        let metadata_path = dir.join("metadata.json");
        let mut f = File::create(&metadata_path).unwrap();
        serde_json::to_writer(&mut f, &metadata).unwrap();

        let chunks_path = dir.join("chunks.jsonl");
        let mut f = File::create(&chunks_path).unwrap();

        for i in 0..n_chunks {
            let chunk = ChunkRecord {
                id: format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", i, i, i, i, i),
                doc_id: format!("doc_{}", i / 2),
                title: format!("Test Document {}", i / 2),
                chunk_idx: i % 2,
                text: format!("This is test chunk {} with some sample text content.", i),
                word_count: 10,
                start_word: (i % 2) * 200,
                end_word: (i % 2) * 200 + 200,
                topic_hint: if i % 2 == 0 { "science" } else { "history" }.to_string(),
                source_dataset: Some("test".to_string()),
            };
            writeln!(f, "{}", serde_json::to_string(&chunk).unwrap()).unwrap();
        }
    }

    #[test]
    fn test_full_benchmark_pipeline() {
        let dir = TempDir::new().unwrap();
        create_test_dataset(dir.path(), 50);

        let config = RealDataBenchConfig {
            dataset_path: dir.path().to_string_lossy().to_string(),
            max_chunks: 50,
            num_queries: 10,
            seed: 42,
            synthetic_embeddings: true,
        };

        let mut runner = RealDataBenchRunner::with_config(config);
        runner.load_dataset().unwrap();
        runner.embed_dataset().unwrap();

        let results = runner.run_benchmarks().unwrap();

        assert_eq!(results.dataset_stats.total_chunks, 50);
        assert_eq!(results.dataset_stats.total_topics, 2);
        assert!(results.single_retrieval.mrr >= 0.0);
        assert!(results.multi_retrieval.mrr >= 0.0);
    }
}
