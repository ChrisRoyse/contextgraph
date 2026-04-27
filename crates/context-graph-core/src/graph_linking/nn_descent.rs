//! NN-Descent algorithm for efficient K-NN graph construction.
//!
//! NN-Descent is an efficient algorithm for approximate K-NN graph construction
//! based on the principle: "a neighbor of my neighbor is likely my neighbor."
//!
//! # Algorithm Overview
//!
//! 1. Initialize with random neighbors for each node
//! 2. Iteratively improve by examining neighbors-of-neighbors
//! 3. Converge when few updates occur
//!
//! # Complexity
//!
//! - Naive K-NN: O(n²) comparisons
//! - NN-Descent: O(n × k × iterations) ≈ O(n × k × log(n))
//!
//! # Architecture Reference
//!
//! - ARCH-18: E5/E8 use asymmetric similarity (handled by caller)
//! - AP-77: E5 MUST NOT use symmetric cosine (handled by caller)
//!
//! # Reference
//!
//! [NN-Descent Paper](https://www.cs.princeton.edu/cass/papers/www11.pdf)

use rand::prelude::*;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::{DirectedRelation, EdgeResult, EmbedderEdge, KnnGraph};
use super::{KNN_K, MIN_KNN_SIMILARITY, NN_DESCENT_ITERATIONS, NN_DESCENT_SAMPLE_RATE};

/// A similarity function that computes similarity between two vectors.
pub type SimilarityFn<'a> = &'a dyn Fn(&[f32], &[f32]) -> f32;

/// Configuration for NN-Descent algorithm.
#[derive(Debug, Clone)]
pub struct NnDescentConfig {
    /// Number of neighbors per node (k).
    pub k: usize,
    /// Number of iterations.
    pub iterations: usize,
    /// Sampling rate (ρ) for neighbor sampling.
    pub sample_rate: f32,
    /// Minimum similarity threshold for edges.
    pub min_similarity: f32,
    /// Early termination: stop if updates < threshold.
    pub early_termination_threshold: f32,
    /// Random seed for reproducibility (None = random).
    pub seed: Option<u64>,
}

impl Default for NnDescentConfig {
    fn default() -> Self {
        Self {
            k: KNN_K,
            iterations: NN_DESCENT_ITERATIONS,
            sample_rate: NN_DESCENT_SAMPLE_RATE,
            min_similarity: MIN_KNN_SIMILARITY,
            early_termination_threshold: 0.001,
            seed: None,
        }
    }
}

impl NnDescentConfig {
    /// Create config with custom k value.
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Create config with custom iterations.
    pub fn with_iterations(mut self, iterations: usize) -> Self {
        self.iterations = iterations;
        self
    }

    /// Create config with custom sample rate.
    pub fn with_sample_rate(mut self, sample_rate: f32) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    /// Create config with custom seed for reproducibility.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Create config with custom minimum similarity threshold.
    pub fn with_min_similarity(mut self, min_similarity: f32) -> Self {
        self.min_similarity = min_similarity;
        self
    }
}

/// NN-Descent algorithm for K-NN graph construction.
///
/// # Type Parameters
///
/// - The algorithm operates on node IDs (UUIDs) and uses a provided similarity
///   function to compare embeddings.
///
/// # Example
///
/// ```ignore
/// use context_graph_core::graph_linking::nn_descent::{NnDescent, NnDescentConfig};
///
/// // Prepare data
/// let nodes: Vec<Uuid> = /* node IDs */;
/// let embeddings: HashMap<Uuid, Vec<f32>> = /* embeddings */;
///
/// // Similarity function
/// let similarity = |a: &[f32], b: &[f32]| -> f32 {
///     cosine_similarity(a, b)
/// };
///
/// // Run NN-Descent
/// let config = NnDescentConfig::default();
/// let nn_descent = NnDescent::new(0, &nodes, config);
/// let graph = nn_descent.build(|id| embeddings.get(&id).unwrap(), &similarity)?;
/// ```
pub struct NnDescent {
    /// Embedder ID for the graph being built.
    embedder_id: u8,
    /// Node IDs in the graph.
    nodes: Vec<Uuid>,
    /// Configuration.
    config: NnDescentConfig,
}

impl NnDescent {
    /// Create a new NN-Descent builder.
    ///
    /// # Arguments
    ///
    /// * `embedder_id` - Which embedder this graph is for (0-12)
    /// * `nodes` - Node IDs to include in the graph
    /// * `config` - Algorithm configuration
    pub fn new(embedder_id: u8, nodes: &[Uuid], config: NnDescentConfig) -> Self {
        Self {
            embedder_id,
            nodes: nodes.to_vec(),
            config,
        }
    }

    /// Build the K-NN graph using NN-Descent algorithm.
    ///
    /// # Arguments
    ///
    /// * `get_embedding` - Function to retrieve embedding for a node ID
    /// * `similarity` - Function to compute similarity between embeddings
    ///
    /// # Returns
    ///
    /// A K-NN graph with k nearest neighbors per node.
    pub fn build<F>(&self, get_embedding: F, similarity: SimilarityFn<'_>) -> EdgeResult<KnnGraph>
    where
        F: Fn(Uuid) -> Option<Vec<f32>>,
    {
        let n = self.nodes.len();
        if n == 0 {
            return Ok(KnnGraph::new(self.embedder_id, self.config.k));
        }

        // For small graphs, use brute force
        if n <= self.config.k * 2 {
            return self.brute_force_knn(&get_embedding, similarity);
        }

        // Create index mapping
        let id_to_idx: HashMap<Uuid, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();

        // Initialize RNG
        let mut rng = match self.config.seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_entropy(),
        };

        // Initialize with random neighbors
        let mut neighbors: Vec<Vec<(usize, f32)>> = self.initialize_random(&mut rng);

        // Cache embeddings for efficiency
        let embeddings: Vec<Option<Vec<f32>>> =
            self.nodes.iter().map(|id| get_embedding(*id)).collect();

        // Iteratively improve
        for _iteration in 0..self.config.iterations {
            let updates =
                self.nn_descent_iteration(&mut neighbors, &embeddings, similarity, &mut rng);

            let update_rate = updates as f32 / (n * self.config.k) as f32;

            // Early termination
            if update_rate < self.config.early_termination_threshold {
                break;
            }
        }

        // Convert to KnnGraph
        self.to_knn_graph(neighbors, &id_to_idx)
    }

    /// Brute-force K-NN for small graphs.
    fn brute_force_knn<F>(
        &self,
        get_embedding: F,
        similarity: SimilarityFn<'_>,
    ) -> EdgeResult<KnnGraph>
    where
        F: Fn(Uuid) -> Option<Vec<f32>>,
    {
        let mut graph = KnnGraph::new(self.embedder_id, self.config.k);

        // Cache embeddings
        let embeddings: Vec<(Uuid, Option<Vec<f32>>)> = self
            .nodes
            .iter()
            .map(|id| (*id, get_embedding(*id)))
            .collect();

        for (i, (id_i, emb_i)) in embeddings.iter().enumerate() {
            let Some(vec_i) = emb_i else { continue };

            let mut candidates: Vec<(Uuid, f32)> = Vec::new();

            for (j, (id_j, emb_j)) in embeddings.iter().enumerate() {
                if i == j {
                    continue;
                }

                let Some(vec_j) = emb_j else { continue };

                let sim = similarity(vec_i, vec_j);
                if sim >= self.config.min_similarity {
                    candidates.push((*id_j, sim));
                }
            }

            // Sort by similarity descending
            candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            // Take top k — use with_direction for asymmetric embedders (E5=4, E8=7)
            let is_asymmetric = self.embedder_id == 4 || self.embedder_id == 7;
            for (target, sim) in candidates.into_iter().take(self.config.k) {
                let edge = if is_asymmetric {
                    EmbedderEdge::with_direction(
                        *id_i,
                        target,
                        self.embedder_id,
                        sim,
                        DirectedRelation::Forward,
                    )?
                } else {
                    EmbedderEdge::new(*id_i, target, self.embedder_id, sim)?
                };
                graph.add_edge(edge);
            }
        }

        Ok(graph)
    }

    /// Initialize with random neighbors.
    fn initialize_random(&self, rng: &mut StdRng) -> Vec<Vec<(usize, f32)>> {
        let n = self.nodes.len();
        let k = self.config.k.min(n - 1);

        self.nodes
            .iter()
            .enumerate()
            .map(|(i, _)| {
                // Sample random neighbors
                let mut neighbor_indices: Vec<usize> = (0..n).filter(|&j| j != i).collect();
                neighbor_indices.shuffle(rng);
                neighbor_indices
                    .into_iter()
                    .take(k)
                    .map(|j| (j, 0.0)) // Start with similarity 0.0 (will be computed)
                    .collect()
            })
            .collect()
    }

    /// Single iteration of NN-Descent.
    ///
    /// Returns the number of updates made.
    fn nn_descent_iteration(
        &self,
        neighbors: &mut [Vec<(usize, f32)>],
        embeddings: &[Option<Vec<f32>>],
        similarity: SimilarityFn<'_>,
        rng: &mut StdRng,
    ) -> usize {
        let n = self.nodes.len();
        let sample_size = ((self.config.k as f32) * self.config.sample_rate).ceil() as usize;
        let mut updates = 0;

        // For each node, explore neighbors-of-neighbors
        for i in 0..n {
            let Some(emb_i) = &embeddings[i] else {
                continue;
            };

            // Collect candidates from neighbors-of-neighbors
            let mut candidates: HashSet<usize> = HashSet::new();

            // Sample from current neighbors
            let current_neighbors: Vec<usize> = neighbors[i].iter().map(|(j, _)| *j).collect();
            for &j in current_neighbors.iter().take(sample_size) {
                // Add j's neighbors as candidates
                for &(k, _) in &neighbors[j] {
                    if k != i && !current_neighbors.contains(&k) {
                        candidates.insert(k);
                    }
                }
            }

            // Also add some random candidates for exploration
            let random_count = (sample_size / 2).max(1);
            for _ in 0..random_count {
                let random_idx = rng.gen_range(0..n);
                if random_idx != i {
                    candidates.insert(random_idx);
                }
            }

            // Compute similarities for candidates
            let mut new_neighbors: Vec<(usize, f32)> = neighbors[i].clone();

            for j in candidates {
                let Some(emb_j) = &embeddings[j] else {
                    continue;
                };

                let sim = similarity(emb_i, emb_j);
                if sim >= self.config.min_similarity {
                    // Try to insert this neighbor
                    let update = self.try_insert(&mut new_neighbors, j, sim);
                    if update {
                        updates += 1;
                    }
                }
            }

            // Also recompute similarities for existing neighbors (in case they were 0.0)
            for neighbor in &mut new_neighbors {
                if neighbor.1 == 0.0 {
                    let Some(emb_j) = &embeddings[neighbor.0] else {
                        continue;
                    };
                    neighbor.1 = similarity(emb_i, emb_j);
                }
            }

            neighbors[i] = new_neighbors;
        }

        updates
    }

    /// Try to insert a neighbor into the neighbor list.
    ///
    /// Returns true if the neighbor was inserted (list changed).
    fn try_insert(&self, neighbors: &mut Vec<(usize, f32)>, idx: usize, similarity: f32) -> bool {
        // Check if already present
        if let Some(pos) = neighbors.iter().position(|(j, _)| *j == idx) {
            // Update similarity if higher
            if similarity > neighbors[pos].1 {
                neighbors[pos].1 = similarity;
                return true;
            }
            return false;
        }

        // If under capacity, add
        if neighbors.len() < self.config.k {
            neighbors.push((idx, similarity));
            return true;
        }

        // Find minimum similarity neighbor
        let (min_idx, min_sim) = neighbors
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.1.partial_cmp(&b.1).unwrap())
            .map(|(i, (_, s))| (i, *s))
            .unwrap();

        // Replace if new is better
        if similarity > min_sim {
            neighbors[min_idx] = (idx, similarity);
            return true;
        }

        false
    }

    /// Convert internal representation to KnnGraph.
    fn to_knn_graph(
        &self,
        neighbors: Vec<Vec<(usize, f32)>>,
        _id_to_idx: &HashMap<Uuid, usize>,
    ) -> EdgeResult<KnnGraph> {
        let mut graph = KnnGraph::with_capacity(self.embedder_id, self.config.k, self.nodes.len());

        // Asymmetric embedders (E5=4, E8=7) require with_direction() instead of new()
        let is_asymmetric = self.embedder_id == 4 || self.embedder_id == 7;

        for (i, node_neighbors) in neighbors.into_iter().enumerate() {
            let source = self.nodes[i];

            for (j, similarity) in node_neighbors {
                if similarity >= self.config.min_similarity {
                    let target = self.nodes[j];
                    let edge = if is_asymmetric {
                        EmbedderEdge::with_direction(
                            source,
                            target,
                            self.embedder_id,
                            similarity,
                            DirectedRelation::Forward,
                        )?
                    } else {
                        EmbedderEdge::new(source, target, self.embedder_id, similarity)?
                    };
                    graph.add_edge(edge);
                }
            }
        }

        Ok(graph)
    }
}

/// Build K-NN graph for asymmetric embedders (E5, E8).
///
/// For asymmetric embedders, we compute directional similarity:
/// - E5: cause_embedding × effect_embedding
/// - E8: source_embedding × target_embedding
///
/// # Arguments
///
/// * `embedder_id` - Which asymmetric embedder (must be 4 for E5 or 7 for E8)
/// * `nodes` - Node IDs
/// * `get_source_embedding` - Function to get "as source/cause" embedding
/// * `get_target_embedding` - Function to get "as target/effect" embedding
/// * `config` - Algorithm configuration
pub fn build_asymmetric_knn<F1, F2>(
    embedder_id: u8,
    nodes: &[Uuid],
    get_source_embedding: F1,
    get_target_embedding: F2,
    similarity: SimilarityFn<'_>,
    config: NnDescentConfig,
) -> EdgeResult<KnnGraph>
where
    F1: Fn(Uuid) -> Option<Vec<f32>>,
    F2: Fn(Uuid) -> Option<Vec<f32>>,
{
    let n = nodes.len();
    if n == 0 {
        return Ok(KnnGraph::new(embedder_id, config.k));
    }

    let mut graph = KnnGraph::with_capacity(embedder_id, config.k, n);

    // Cache embeddings
    let source_embeddings: Vec<(Uuid, Option<Vec<f32>>)> = nodes
        .iter()
        .map(|id| (*id, get_source_embedding(*id)))
        .collect();

    let target_embeddings: Vec<(Uuid, Option<Vec<f32>>)> = nodes
        .iter()
        .map(|id| (*id, get_target_embedding(*id)))
        .collect();

    // For asymmetric, we compute A.source × B.target
    // This creates directed edges: A --causes/imports--> B
    for (i, (id_i, source_emb_i)) in source_embeddings.iter().enumerate() {
        let Some(vec_i) = source_emb_i else { continue };

        let mut candidates: Vec<(Uuid, f32)> = Vec::new();

        for (j, (id_j, target_emb_j)) in target_embeddings.iter().enumerate() {
            if i == j {
                continue;
            }

            let Some(vec_j) = target_emb_j else { continue };

            // Asymmetric: source_i × target_j
            let sim = similarity(vec_i, vec_j);
            if sim >= config.min_similarity {
                candidates.push((*id_j, sim));
            }
        }

        // Sort by similarity descending
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Take top k and create directed edges
        for (target, sim) in candidates.into_iter().take(config.k) {
            let edge = EmbedderEdge::with_direction(
                *id_i,
                target,
                embedder_id,
                sim,
                DirectedRelation::Forward,
            )?;
            graph.add_edge(edge);
        }
    }

    Ok(graph)
}

/// Statistics from NN-Descent execution.
#[derive(Debug, Clone)]
pub struct NnDescentStats {
    /// Number of nodes processed.
    pub node_count: usize,
    /// Number of iterations executed.
    pub iterations_executed: usize,
    /// Final update rate (updates / total edges).
    pub final_update_rate: f32,
    /// Total edges in resulting graph.
    pub edge_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }

    #[test]
    fn test_empty_nodes() {
        let config = NnDescentConfig::default();
        let nn = NnDescent::new(0, &[], config);
        let graph = nn.build(|_| None, &cosine_similarity).unwrap();

        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_small_graph_brute_force() {
        let nodes: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
        let embeddings: HashMap<Uuid, Vec<f32>> = nodes
            .iter()
            .enumerate()
            .map(|(i, id)| {
                // Create simple orthogonal-ish embeddings
                let mut emb = vec![0.0; 10];
                emb[i % 10] = 1.0;
                emb[(i + 1) % 10] = 0.5;
                (*id, emb)
            })
            .collect();

        let config = NnDescentConfig::default().with_k(3).with_seed(42);

        let nn = NnDescent::new(0, &nodes, config);
        let graph = nn
            .build(|id| embeddings.get(&id).cloned(), &cosine_similarity)
            .unwrap();

        // Each node should have up to 3 neighbors (k=3, but only 4 other nodes)
        assert_eq!(graph.node_count(), 5);
        assert!(graph.edge_count() > 0);
    }

    #[test]
    fn test_larger_graph_nn_descent() {
        let nodes: Vec<Uuid> = (0..100).map(|_| Uuid::new_v4()).collect();
        let embeddings: HashMap<Uuid, Vec<f32>> = nodes
            .iter()
            .enumerate()
            .map(|(i, id)| {
                // Create embeddings that form clusters
                let cluster = i / 10;
                let mut emb = vec![0.0; 32];
                emb[cluster] = 1.0;
                emb[(cluster + 1) % 32] = 0.3;
                // Add some noise
                for j in 0..32 {
                    emb[j] += (i as f32 * 0.01) * ((j + i) as f32).sin();
                }
                (*id, emb)
            })
            .collect();

        let config = NnDescentConfig::default()
            .with_k(10)
            .with_iterations(5)
            .with_seed(42)
            .with_sample_rate(0.5);

        let nn = NnDescent::new(0, &nodes, config);
        let graph = nn
            .build(|id| embeddings.get(&id).cloned(), &cosine_similarity)
            .unwrap();

        assert_eq!(graph.node_count(), 100);
        // Most nodes should have k neighbors
        let stats = graph.stats();
        assert!(stats.avg_neighbors > 5.0);
    }

    #[test]
    fn test_config_builder() {
        let config = NnDescentConfig::default()
            .with_k(15)
            .with_iterations(10)
            .with_sample_rate(0.8)
            .with_seed(123);

        assert_eq!(config.k, 15);
        assert_eq!(config.iterations, 10);
        assert!((config.sample_rate - 0.8).abs() < 0.001);
        assert_eq!(config.seed, Some(123));
    }

    #[test]
    fn test_asymmetric_knn() {
        let nodes: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();

        // Create asymmetric embeddings where all nodes have neighbors
        // Source embedding: node i has components at positions i and (i+1)%10
        // Target embedding: node i has components at positions i and (i-1+10)%10
        // This ensures every source can find at least one matching target
        let source_embs: HashMap<Uuid, Vec<f32>> = nodes
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let mut emb = vec![0.0; 16];
                emb[i % 16] = 0.8;
                emb[(i + 1) % 16] = 0.6;
                (*id, emb)
            })
            .collect();

        let target_embs: HashMap<Uuid, Vec<f32>> = nodes
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let mut emb = vec![0.0; 16];
                emb[i % 16] = 0.7;
                emb[(i + 2) % 16] = 0.7; // Overlap with next source
                (*id, emb)
            })
            .collect();

        let config = NnDescentConfig::default()
            .with_k(3)
            .with_min_similarity(0.1) // Lower threshold
            .with_seed(42);

        let graph = build_asymmetric_knn(
            4, // E5 causal
            &nodes,
            |id| source_embs.get(&id).cloned(),
            |id| target_embs.get(&id).cloned(),
            &cosine_similarity,
            config,
        )
        .unwrap();

        // With overlapping embeddings, most nodes should have neighbors
        assert!(
            graph.node_count() >= 9,
            "Expected at least 9 nodes with edges, got {}",
            graph.node_count()
        );

        // Edges should have Forward direction
        for edge in graph.edges() {
            assert_eq!(edge.direction(), DirectedRelation::Forward);
        }
    }
}
