//! Single-embedder baseline using only E1 (1024D semantic embedding).
//!
//! This represents the traditional RAG approach where a single embedding model
//! (like e5-large-v2) is used for semantic similarity search.

use std::collections::HashMap;
use uuid::Uuid;

use context_graph_core::types::fingerprint::SemanticFingerprint;

use super::single_hnsw::SingleBruteForceIndex;
use crate::util::cosine_similarity as compute_cosine_similarity;

/// Configuration for single-embedder baseline.
#[derive(Debug, Clone)]
pub struct SingleEmbedderConfig {
    /// Dimension of embeddings (1024 for E1).
    pub dimension: usize,
    /// HNSW M parameter (connectivity).
    pub hnsw_m: usize,
    /// HNSW ef_construction parameter.
    pub hnsw_ef_construction: usize,
    /// HNSW ef_search parameter.
    pub hnsw_ef_search: usize,
}

impl Default for SingleEmbedderConfig {
    fn default() -> Self {
        Self {
            dimension: 1024, // E1 semantic embedding
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 100,
        }
    }
}

/// Single-embedder baseline that uses only E1 for similarity search.
///
/// This represents traditional RAG systems that use a single semantic embedding model.
/// The hypothesis is that this approach degrades in accuracy as corpus size grows
/// because semantic similarity becomes less discriminative without the orthogonal
/// signals from additional embedding spaces.
pub struct SingleEmbedderBaseline {
    /// HNSW index for the single embedding space.
    index: SingleBruteForceIndex,
    /// Stored embeddings for reference.
    embeddings: HashMap<Uuid, Vec<f32>>,
    /// Configuration.
    config: SingleEmbedderConfig,
    /// Topic assignments (for ground truth).
    topic_assignments: HashMap<Uuid, usize>,
}

impl SingleEmbedderBaseline {
    /// Create a new baseline with default config.
    pub fn new() -> Self {
        Self::with_config(SingleEmbedderConfig::default())
    }

    /// Create a new baseline with specific config.
    pub fn with_config(config: SingleEmbedderConfig) -> Self {
        let index = SingleBruteForceIndex::new(
            config.dimension,
            config.hnsw_m,
            config.hnsw_ef_construction,
            config.hnsw_ef_search,
        );

        Self {
            index,
            embeddings: HashMap::new(),
            config,
            topic_assignments: HashMap::new(),
        }
    }

    /// Build baseline from semantic fingerprints.
    ///
    /// Extracts only E1 (semantic) embeddings for the single-space baseline.
    pub fn from_fingerprints(fingerprints: &[(Uuid, &SemanticFingerprint)]) -> Self {
        let mut baseline = Self::new();

        for (id, fp) in fingerprints {
            baseline.insert(*id, &fp.e1_semantic);
        }

        baseline
    }

    /// Build baseline from fingerprints with topic assignments.
    pub fn from_fingerprints_with_topics(
        fingerprints: &[(Uuid, &SemanticFingerprint)],
        topics: &HashMap<Uuid, usize>,
    ) -> Self {
        let mut baseline = Self::from_fingerprints(fingerprints);
        baseline.topic_assignments = topics.clone();
        baseline
    }

    /// Insert a single embedding.
    pub fn insert(&mut self, id: Uuid, embedding: &[f32]) {
        if embedding.len() != self.config.dimension {
            panic!(
                "FAIL FAST: Embedding dimension mismatch: expected {}, got {}",
                self.config.dimension,
                embedding.len()
            );
        }

        self.index.insert(id, embedding);
        self.embeddings.insert(id, embedding.to_vec());
    }

    /// Insert with topic assignment.
    pub fn insert_with_topic(&mut self, id: Uuid, embedding: &[f32], topic: usize) {
        self.insert(id, embedding);
        self.topic_assignments.insert(id, topic);
    }

    /// Search for k nearest neighbors.
    ///
    /// Returns list of (id, similarity) pairs sorted by similarity descending.
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(Uuid, f32)> {
        self.index.search(query, k, None)
    }

    /// Search with distance threshold.
    pub fn search_with_threshold(
        &self,
        query: &[f32],
        k: usize,
        min_similarity: f32,
    ) -> Vec<(Uuid, f32)> {
        self.index.search(query, k, Some(min_similarity))
    }

    /// Get number of stored embeddings.
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Get embedding by ID.
    pub fn get_embedding(&self, id: &Uuid) -> Option<&[f32]> {
        self.embeddings.get(id).map(|v| v.as_slice())
    }

    /// Get topic assignment for an ID.
    pub fn get_topic(&self, id: &Uuid) -> Option<usize> {
        self.topic_assignments.get(id).copied()
    }

    /// Get all IDs.
    pub fn ids(&self) -> Vec<Uuid> {
        self.embeddings.keys().copied().collect()
    }

    /// Get configuration.
    pub fn config(&self) -> &SingleEmbedderConfig {
        &self.config
    }

    /// Compute cosine similarity between two embeddings.
    pub fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        compute_cosine_similarity(a, b)
    }

    /// Detect topics using k-means on the single embedding space.
    ///
    /// Returns predicted cluster labels for each ID.
    pub fn detect_topics_kmeans(
        &self,
        n_clusters: usize,
        max_iters: usize,
    ) -> HashMap<Uuid, usize> {
        if self.embeddings.is_empty() || n_clusters == 0 {
            return HashMap::new();
        }

        // Simple k-means implementation
        // Sort IDs for deterministic order (HashMap iteration is non-deterministic)
        let mut ids: Vec<Uuid> = self.embeddings.keys().copied().collect();
        ids.sort();
        let embeddings: Vec<&Vec<f32>> = ids.iter().map(|id| &self.embeddings[id]).collect();
        let dim = self.config.dimension;

        // Initialize centroids randomly (using first k points)
        let mut centroids: Vec<Vec<f32>> = embeddings
            .iter()
            .take(n_clusters)
            .map(|e| (*e).clone())
            .collect();

        // Pad with random if not enough points
        while centroids.len() < n_clusters {
            centroids.push(vec![0.0; dim]);
        }

        let mut assignments = vec![0usize; embeddings.len()];

        for _ in 0..max_iters {
            // Assign points to nearest centroid
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
                    .map(|(e, _)| *e)
                    .collect();

                if cluster_points.is_empty() {
                    continue;
                }

                for d in 0..dim {
                    centroid[d] = cluster_points.iter().map(|e| e[d]).sum::<f32>()
                        / cluster_points.len() as f32;
                }
            }
        }

        ids.into_iter().zip(assignments).collect()
    }
}

impl Default for SingleEmbedderBaseline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    fn random_embedding(seed: u64, dim: usize) -> Vec<f32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut embedding = vec![0.0f32; dim];
        let mut hasher = DefaultHasher::new();

        for (i, e) in embedding.iter_mut().enumerate() {
            (seed, i).hash(&mut hasher);
            let hash = hasher.finish();
            *e = (hash as f32 / u64::MAX as f32) * 2.0 - 1.0;
            hasher = DefaultHasher::new();
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        for e in &mut embedding {
            *e /= norm;
        }

        embedding
    }

    #[test]
    fn test_insert_and_search() {
        let mut baseline = SingleEmbedderBaseline::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let e1 = random_embedding(1, 1024);
        let e2 = random_embedding(2, 1024);

        baseline.insert(id1, &e1);
        baseline.insert(id2, &e2);

        assert_eq!(baseline.len(), 2);

        // Search for e1
        let results = baseline.search(&e1, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id1);
    }

    #[test]
    fn test_cosine_similarity() {
        let baseline = SingleEmbedderBaseline::new();

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];

        assert!((baseline.cosine_similarity(&a, &b) - 1.0).abs() < 0.01);
        assert!(baseline.cosine_similarity(&a, &c).abs() < 0.01);
    }
}
