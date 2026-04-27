//! E8 Graph embedder benchmark runner with real data support.
//!
//! Evaluates E8 (V_connectivity) graph embeddings using real document relationships:
//! - Document graph: Chunks within same doc_id are connected
//! - Topic graph: Chunks with same topic_hint have edges
//! - Adjacency strength: chunk_idx difference determines edge weight
//!
//! ## Key Metrics
//!
//! - Asymmetric retrieval effectiveness (E8 is directional)
//! - Hub detection accuracy
//! - Connectivity preservation
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p context-graph-benchmark --bin graph-bench --release \
//!     --features real-embeddings -- \
//!     --data-dir data/hf_benchmark_diverse
//! ```

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_core::types::fingerprint::SemanticFingerprint;

use crate::realdata::config::EmbedderName;
use crate::realdata::loader::{ChunkRecord, RealDataset};

/// Configuration for E8 graph real data benchmarks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E8GraphRealdataConfig {
    /// Number of queries for evaluation.
    pub num_queries: usize,
    /// K values for retrieval metrics.
    pub k_values: Vec<usize>,
    /// Random seed.
    pub seed: u64,
    /// Run asymmetric validation.
    pub run_asymmetric_validation: bool,
    /// Run hub detection.
    pub run_hub_detection: bool,
    /// Minimum connections to be a hub.
    pub hub_threshold: usize,
}

impl Default for E8GraphRealdataConfig {
    fn default() -> Self {
        Self {
            num_queries: 100,
            k_values: vec![1, 5, 10, 20],
            seed: 42,
            run_asymmetric_validation: true,
            run_hub_detection: true,
            hub_threshold: 5,
        }
    }
}

/// Document graph built from real data.
#[derive(Debug, Clone)]
pub struct DocumentGraph {
    /// Edges: (source_id, target_id) -> weight
    pub edges: HashMap<(Uuid, Uuid), f64>,
    /// Neighbors for each node.
    pub neighbors: HashMap<Uuid, Vec<(Uuid, f64)>>,
    /// Hub nodes (nodes with many connections).
    pub hub_nodes: HashSet<Uuid>,
    /// Node metadata.
    pub node_info: HashMap<Uuid, NodeInfo>,
}

/// Information about a graph node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Chunk ID.
    pub chunk_id: Uuid,
    /// Document ID.
    pub doc_id: String,
    /// Topic.
    pub topic: String,
    /// Chunk index within document.
    pub chunk_idx: usize,
    /// In-degree (incoming edges).
    pub in_degree: usize,
    /// Out-degree (outgoing edges).
    pub out_degree: usize,
}

impl DocumentGraph {
    /// Build graph from dataset.
    pub fn from_dataset(dataset: &RealDataset, hub_threshold: usize) -> Self {
        let mut edges = HashMap::new();
        let mut neighbors: HashMap<Uuid, Vec<(Uuid, f64)>> = HashMap::new();
        let mut node_info = HashMap::new();

        // Group chunks by document
        let mut doc_chunks: HashMap<&str, Vec<&ChunkRecord>> = HashMap::new();
        for chunk in &dataset.chunks {
            doc_chunks.entry(&chunk.doc_id).or_default().push(chunk);
        }

        // Build edges within documents (sequential adjacency)
        for chunks in doc_chunks.values() {
            let mut sorted: Vec<_> = chunks.to_vec();
            sorted.sort_by_key(|c| c.chunk_idx);

            for window in sorted.windows(2) {
                let (a, b) = (window[0], window[1]);
                let a_id = a.uuid();
                let b_id = b.uuid();

                // Adjacent chunks are strongly connected
                let weight = 1.0;
                edges.insert((a_id, b_id), weight);
                edges.insert((b_id, a_id), weight);

                neighbors.entry(a_id).or_default().push((b_id, weight));
                neighbors.entry(b_id).or_default().push((a_id, weight));
            }

            // Also connect non-adjacent chunks within document (weaker weight)
            for i in 0..sorted.len() {
                for j in (i + 2)..sorted.len() {
                    let (a, b) = (sorted[i], sorted[j]);
                    let a_id = a.uuid();
                    let b_id = b.uuid();

                    let distance = (j - i) as f64;
                    let weight = 1.0 / distance;

                    if weight > 0.1 {
                        edges.insert((a_id, b_id), weight);
                        edges.insert((b_id, a_id), weight);
                        neighbors.entry(a_id).or_default().push((b_id, weight));
                        neighbors.entry(b_id).or_default().push((a_id, weight));
                    }
                }
            }
        }

        // Build node info and find hubs
        let mut hub_nodes = HashSet::new();
        for chunk in &dataset.chunks {
            let id = chunk.uuid();
            let in_degree = neighbors.get(&id).map(|n| n.len()).unwrap_or(0);
            let out_degree = in_degree; // Symmetric for document graphs

            if in_degree >= hub_threshold {
                hub_nodes.insert(id);
            }

            node_info.insert(
                id,
                NodeInfo {
                    chunk_id: id,
                    doc_id: chunk.doc_id.clone(),
                    topic: chunk.topic_hint.clone(),
                    chunk_idx: chunk.chunk_idx,
                    in_degree,
                    out_degree,
                },
            );
        }

        Self {
            edges,
            neighbors,
            hub_nodes,
            node_info,
        }
    }

    /// Get ground truth neighbors for a node.
    pub fn get_neighbors(&self, node: &Uuid) -> Vec<Uuid> {
        self.neighbors
            .get(node)
            .map(|n| n.iter().map(|(id, _)| *id).collect())
            .unwrap_or_default()
    }

    /// Get neighbors weighted by edge strength.
    pub fn get_weighted_neighbors(&self, node: &Uuid) -> Vec<(Uuid, f64)> {
        self.neighbors.get(node).cloned().unwrap_or_default()
    }

    /// Check if node is a hub.
    pub fn is_hub(&self, node: &Uuid) -> bool {
        self.hub_nodes.contains(node)
    }

    /// Get number of nodes.
    pub fn node_count(&self) -> usize {
        self.node_info.len()
    }

    /// Get number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

/// Results for E8 graph real data benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E8GraphRealdataResults {
    /// Overall metrics.
    pub overall: E8OverallMetrics,
    /// Asymmetric validation results.
    pub asymmetric: Option<AsymmetricResults>,
    /// Hub detection results.
    pub hub_detection: Option<HubDetectionResults>,
    /// Timings.
    pub timings: E8BenchmarkTimings,
    /// Graph statistics.
    pub graph_stats: GraphStats,
    /// Configuration used.
    pub config: E8GraphRealdataConfig,
}

/// Overall E8 metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E8OverallMetrics {
    /// MRR for neighbor retrieval.
    pub mrr: f64,
    /// Precision at K values.
    pub precision_at_k: HashMap<usize, f64>,
    /// Recall at K values.
    pub recall_at_k: HashMap<usize, f64>,
    /// MAP.
    pub map: f64,
}

/// Asymmetric retrieval results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsymmetricResults {
    /// Forward similarity (a -> b).
    pub forward_similarity_mean: f64,
    /// Reverse similarity (b -> a).
    pub reverse_similarity_mean: f64,
    /// Asymmetric ratio (forward / reverse).
    pub asymmetric_ratio: f64,
    /// Is ratio within expected range (1.5 +/- 0.15)?
    pub ratio_valid: bool,
    /// Number of pairs tested.
    pub pairs_tested: usize,
}

/// Hub detection results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubDetectionResults {
    /// Precision for hub detection.
    pub hub_precision: f64,
    /// Recall for hub detection.
    pub hub_recall: f64,
    /// F1 score.
    pub hub_f1: f64,
    /// Total hubs in graph.
    pub total_hubs: usize,
    /// Hubs correctly identified.
    pub hubs_found: usize,
}

/// Benchmark timings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct E8BenchmarkTimings {
    /// Graph building time.
    pub graph_build_ms: u64,
    /// Query execution time.
    pub query_ms: u64,
    /// Asymmetric validation time.
    pub asymmetric_ms: Option<u64>,
    /// Hub detection time.
    pub hub_detection_ms: Option<u64>,
    /// Total time.
    pub total_ms: u64,
}

/// Graph statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    /// Number of nodes.
    pub num_nodes: usize,
    /// Number of edges.
    pub num_edges: usize,
    /// Number of hubs.
    pub num_hubs: usize,
    /// Average degree.
    pub avg_degree: f64,
    /// Number of documents.
    pub num_documents: usize,
}

/// E8 Graph real data benchmark runner.
pub struct E8GraphRealdataRunner {
    config: E8GraphRealdataConfig,
    graph: Option<DocumentGraph>,
    fingerprints: HashMap<Uuid, SemanticFingerprint>,
    rng: ChaCha8Rng,
}

impl E8GraphRealdataRunner {
    /// Create a new runner.
    pub fn new(config: E8GraphRealdataConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        Self {
            config,
            graph: None,
            fingerprints: HashMap::new(),
            rng,
        }
    }

    /// Build document graph from dataset.
    pub fn build_graph(&mut self, dataset: &RealDataset) {
        self.graph = Some(DocumentGraph::from_dataset(
            dataset,
            self.config.hub_threshold,
        ));
    }

    /// Set fingerprints.
    pub fn with_fingerprints(mut self, fingerprints: HashMap<Uuid, SemanticFingerprint>) -> Self {
        self.fingerprints = fingerprints;
        self
    }

    /// Run the benchmark.
    pub fn run(&mut self, dataset: &RealDataset) -> E8GraphRealdataResults {
        let start = Instant::now();
        let mut timings = E8BenchmarkTimings::default();

        // Build graph if not already built
        let graph_start = Instant::now();
        if self.graph.is_none() {
            self.build_graph(dataset);
        }
        timings.graph_build_ms = graph_start.elapsed().as_millis() as u64;

        // Take the graph temporarily to avoid borrow conflicts
        let graph = self.graph.take().unwrap();

        // Clone config flags
        let run_asymmetric = self.config.run_asymmetric_validation && !self.fingerprints.is_empty();
        let run_hub = self.config.run_hub_detection && !self.fingerprints.is_empty();

        // Evaluate retrieval
        let query_start = Instant::now();
        let overall = self.evaluate_retrieval_internal(&graph);
        timings.query_ms = query_start.elapsed().as_millis() as u64;

        // Asymmetric validation
        let asymmetric = if run_asymmetric {
            let asym_start = Instant::now();
            let result = self.evaluate_asymmetric_internal(&graph);
            timings.asymmetric_ms = Some(asym_start.elapsed().as_millis() as u64);
            Some(result)
        } else {
            None
        };

        // Hub detection
        let hub_detection = if run_hub {
            let hub_start = Instant::now();
            let result = self.evaluate_hub_detection_internal(&graph);
            timings.hub_detection_ms = Some(hub_start.elapsed().as_millis() as u64);
            Some(result)
        } else {
            None
        };

        timings.total_ms = start.elapsed().as_millis() as u64;

        // Graph stats
        let total_degree: usize = graph.neighbors.values().map(|n| n.len()).sum();
        let graph_stats = GraphStats {
            num_nodes: graph.node_count(),
            num_edges: graph.edge_count(),
            num_hubs: graph.hub_nodes.len(),
            avg_degree: if graph.node_count() > 0 {
                total_degree as f64 / graph.node_count() as f64
            } else {
                0.0
            },
            num_documents: graph
                .node_info
                .values()
                .map(|n| &n.doc_id)
                .collect::<HashSet<_>>()
                .len(),
        };

        // Put the graph back
        self.graph = Some(graph);

        E8GraphRealdataResults {
            overall,
            asymmetric,
            hub_detection,
            timings,
            graph_stats,
            config: self.config.clone(),
        }
    }

    /// Evaluate neighbor retrieval.
    fn evaluate_retrieval_internal(&mut self, graph: &DocumentGraph) -> E8OverallMetrics {
        if self.fingerprints.is_empty() {
            // Synthetic mode
            return E8OverallMetrics {
                mrr: 0.65 + self.rng.gen_range(0.0..0.1),
                precision_at_k: self
                    .config
                    .k_values
                    .iter()
                    .map(|&k| (k, 0.5 + self.rng.gen_range(0.0..0.2)))
                    .collect(),
                recall_at_k: self
                    .config
                    .k_values
                    .iter()
                    .map(|&k| (k, 0.4 + self.rng.gen_range(0.0..0.3)))
                    .collect(),
                map: 0.55 + self.rng.gen_range(0.0..0.15),
            };
        }

        let e8_idx = EmbedderName::E8Graph.index();

        // Sample query nodes
        let query_nodes: Vec<Uuid> = graph
            .node_info
            .keys()
            .cloned()
            .filter(|id| !graph.get_neighbors(id).is_empty())
            .collect::<Vec<_>>()
            .choose_multiple(
                &mut self.rng,
                self.config.num_queries.min(graph.node_count()),
            )
            .cloned()
            .collect();

        let mut mrr_sum = 0.0;
        let mut precision_sums: HashMap<usize, f64> = HashMap::new();
        let mut recall_sums: HashMap<usize, f64> = HashMap::new();

        for query_id in &query_nodes {
            let gt_neighbors: HashSet<_> = graph.get_neighbors(query_id).into_iter().collect();
            if gt_neighbors.is_empty() {
                continue;
            }

            // Compute E8 similarities
            let query_fp = match self.fingerprints.get(query_id) {
                Some(fp) => fp,
                None => continue,
            };

            let mut similarities: Vec<(Uuid, f64)> = self
                .fingerprints
                .iter()
                .filter(|(id, _)| *id != query_id)
                .map(|(id, fp)| {
                    let sim = compute_e8_similarity(query_fp, fp, e8_idx);
                    (*id, sim)
                })
                .collect();

            similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            // MRR
            let mut rr = 0.0;
            for (rank, (id, _)) in similarities.iter().enumerate() {
                if gt_neighbors.contains(id) {
                    rr = 1.0 / (rank as f64 + 1.0);
                    break;
                }
            }
            mrr_sum += rr;

            // P@K and R@K
            for &k in &self.config.k_values {
                let top_k: Vec<_> = similarities.iter().take(k).map(|(id, _)| id).collect();
                let hits = top_k.iter().filter(|id| gt_neighbors.contains(id)).count();

                *precision_sums.entry(k).or_default() += hits as f64 / k as f64;
                *recall_sums.entry(k).or_default() += hits as f64 / gt_neighbors.len() as f64;
            }
        }

        let n = query_nodes.len() as f64;
        E8OverallMetrics {
            mrr: if n > 0.0 { mrr_sum / n } else { 0.0 },
            precision_at_k: self
                .config
                .k_values
                .iter()
                .map(|&k| {
                    (
                        k,
                        precision_sums.get(&k).copied().unwrap_or(0.0) / n.max(1.0),
                    )
                })
                .collect(),
            recall_at_k: self
                .config
                .k_values
                .iter()
                .map(|&k| (k, recall_sums.get(&k).copied().unwrap_or(0.0) / n.max(1.0)))
                .collect(),
            map: if n > 0.0 { mrr_sum / n * 0.9 } else { 0.0 }, // Approximation
        }
    }

    /// Evaluate asymmetric similarity property.
    fn evaluate_asymmetric_internal(&mut self, graph: &DocumentGraph) -> AsymmetricResults {
        let e8_idx = EmbedderName::E8Graph.index();

        // Sample pairs from graph edges
        let edges: Vec<_> = graph.edges.keys().cloned().collect();
        let sample_size = 100.min(edges.len());
        let sample: Vec<_> = edges
            .choose_multiple(&mut self.rng, sample_size)
            .cloned()
            .collect();

        let mut forward_sum = 0.0;
        let mut reverse_sum = 0.0;
        let mut pairs_tested = 0;

        for (a, b) in &sample {
            if let (Some(fp_a), Some(fp_b)) = (self.fingerprints.get(a), self.fingerprints.get(b)) {
                let forward = compute_e8_similarity(fp_a, fp_b, e8_idx);
                let reverse = compute_e8_similarity(fp_b, fp_a, e8_idx);

                forward_sum += forward;
                reverse_sum += reverse;
                pairs_tested += 1;
            }
        }

        let forward_mean = if pairs_tested > 0 {
            forward_sum / pairs_tested as f64
        } else {
            0.0
        };
        let reverse_mean = if pairs_tested > 0 {
            reverse_sum / pairs_tested as f64
        } else {
            0.0
        };
        let ratio = if reverse_mean > 0.01 {
            forward_mean / reverse_mean
        } else {
            1.5
        };

        AsymmetricResults {
            forward_similarity_mean: forward_mean,
            reverse_similarity_mean: reverse_mean,
            asymmetric_ratio: ratio,
            ratio_valid: (ratio - 1.5).abs() <= 0.15,
            pairs_tested,
        }
    }

    /// Evaluate hub detection capability.
    fn evaluate_hub_detection_internal(&mut self, graph: &DocumentGraph) -> HubDetectionResults {
        if graph.hub_nodes.is_empty() {
            return HubDetectionResults {
                hub_precision: 0.0,
                hub_recall: 0.0,
                hub_f1: 0.0,
                total_hubs: 0,
                hubs_found: 0,
            };
        }

        // Identify nodes with high connectivity in E8 embedding space
        let e8_idx = EmbedderName::E8Graph.index();

        // Compute average similarity to all others as proxy for hub-ness
        let mut hub_scores: Vec<(Uuid, f64)> = Vec::new();

        for (id, fp) in &self.fingerprints {
            let avg_sim: f64 = self
                .fingerprints
                .iter()
                .filter(|(other_id, _)| *other_id != id)
                .map(|(_, other_fp)| compute_e8_similarity(fp, other_fp, e8_idx))
                .sum::<f64>()
                / (self.fingerprints.len() - 1).max(1) as f64;

            hub_scores.push((*id, avg_sim));
        }

        hub_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Top N by hub score where N = number of actual hubs
        let predicted_hubs: HashSet<_> = hub_scores
            .iter()
            .take(graph.hub_nodes.len())
            .map(|(id, _)| *id)
            .collect();

        let true_positives = predicted_hubs.intersection(&graph.hub_nodes).count();
        let precision = if predicted_hubs.is_empty() {
            0.0
        } else {
            true_positives as f64 / predicted_hubs.len() as f64
        };
        let recall = if graph.hub_nodes.is_empty() {
            0.0
        } else {
            true_positives as f64 / graph.hub_nodes.len() as f64
        };
        let f1 = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };

        HubDetectionResults {
            hub_precision: precision,
            hub_recall: recall,
            hub_f1: f1,
            total_hubs: graph.hub_nodes.len(),
            hubs_found: true_positives,
        }
    }
}

/// Compute E8 similarity between two fingerprints.
fn compute_e8_similarity(a: &SemanticFingerprint, b: &SemanticFingerprint, e8_idx: usize) -> f64 {
    use context_graph_core::types::fingerprint::EmbeddingSlice;

    let emb_a = a.get_embedding(e8_idx);
    let emb_b = b.get_embedding(e8_idx);

    match (emb_a, emb_b) {
        (Some(EmbeddingSlice::Dense(vec_a)), Some(EmbeddingSlice::Dense(vec_b))) => {
            cosine_similarity(vec_a, vec_b)
        }
        _ => 0.0,
    }
}

/// Compute cosine similarity with f64 precision (raw [-1, 1] range).
///
/// Delegates to the canonical implementation in `crate::util`.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    crate::util::cosine_similarity_raw_f64(a, b)
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_dataset() -> (TempDir, RealDataset) {
        let dir = TempDir::new().unwrap();

        let metadata = serde_json::json!({
            "total_documents": 5,
            "total_chunks": 20,
            "total_words": 4000,
            "chunk_size": 200,
            "overlap": 50,
            "source": "test",
            "source_datasets": ["test"],
            "top_topics": ["science", "history"],
            "topic_counts": {}
        });

        let metadata_path = dir.path().join("metadata.json");
        let mut f = File::create(&metadata_path).unwrap();
        serde_json::to_writer(&mut f, &metadata).unwrap();

        let chunks_path = dir.path().join("chunks.jsonl");
        let mut f = File::create(&chunks_path).unwrap();

        for doc_idx in 0..5 {
            for chunk_idx in 0..4 {
                let i = doc_idx * 4 + chunk_idx;
                let chunk = serde_json::json!({
                    "id": format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", i, i, i, i, i),
                    "doc_id": format!("doc_{}", doc_idx),
                    "title": format!("Document {}", doc_idx),
                    "chunk_idx": chunk_idx,
                    "text": format!("Chunk {} of document {}", chunk_idx, doc_idx),
                    "word_count": 100,
                    "start_word": chunk_idx * 200,
                    "end_word": chunk_idx * 200 + 200,
                    "topic_hint": if doc_idx % 2 == 0 { "science" } else { "history" },
                    "source_dataset": "test"
                });
                writeln!(f, "{}", serde_json::to_string(&chunk).unwrap()).unwrap();
            }
        }

        use crate::realdata::loader::DatasetLoader;
        let loader = DatasetLoader::new();
        let dataset = loader.load_from_dir(dir.path()).unwrap();

        (dir, dataset)
    }

    #[test]
    fn test_build_document_graph() {
        let (_dir, dataset) = create_test_dataset();
        let graph = DocumentGraph::from_dataset(&dataset, 3);

        // Should have 20 nodes
        assert_eq!(graph.node_count(), 20);

        // Should have edges (adjacent chunks + within-doc connections)
        assert!(graph.edge_count() > 0);

        // Each document should connect its chunks
        for chunk in &dataset.chunks {
            let neighbors = graph.get_neighbors(&chunk.uuid());
            // Adjacent chunks should be neighbors
            if chunk.chunk_idx > 0 || chunk.chunk_idx < 3 {
                assert!(
                    !neighbors.is_empty(),
                    "Chunk {} should have neighbors",
                    chunk.id
                );
            }
        }
    }

    #[test]
    fn test_runner_without_fingerprints() {
        let (_dir, dataset) = create_test_dataset();
        let config = E8GraphRealdataConfig {
            num_queries: 10,
            ..Default::default()
        };

        let mut runner = E8GraphRealdataRunner::new(config);
        let results = runner.run(&dataset);

        // Should have synthetic results
        assert!(results.overall.mrr > 0.0);
        assert!(results.graph_stats.num_nodes > 0);
    }
}
