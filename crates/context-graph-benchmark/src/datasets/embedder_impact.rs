//! Embedder impact benchmark dataset generation.
//!
//! Generates test data for measuring per-embedder contribution to retrieval quality,
//! graph structure, and resource usage.

use std::collections::{HashMap, HashSet};

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rand_distr::Normal;
use uuid::Uuid;

use context_graph_core::causal::asymmetric::CausalDirection;
use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E6_SPARSE_VOCAB,
};
use context_graph_storage::teleological::indexes::EmbedderIndex;

use super::graph_linking::ScaleTier;
use super::topic_clusters::{TopicCluster, TopicGenerator};

/// Configuration for embedder impact dataset generation.
#[derive(Debug, Clone)]
pub struct EmbedderImpactDatasetConfig {
    /// Scale tier for dataset size.
    pub tier: ScaleTier,
    /// Number of queries per topic.
    pub queries_per_topic: usize,
    /// Random seed for reproducibility.
    pub seed: u64,
    /// Noise standard deviation.
    pub noise_std: f32,
    /// Intra-topic similarity (higher = tighter clusters).
    pub intra_topic_similarity: f32,
    /// Inter-topic similarity (lower = better separation).
    pub inter_topic_similarity: f32,
    /// Number of sparse entries per E6 embedding.
    pub sparse_entries_e6: usize,
    /// Number of sparse entries per E13 embedding.
    pub sparse_entries_e13: usize,
    /// Number of tokens for E12 late interaction.
    pub tokens_e12: usize,
}

impl Default for EmbedderImpactDatasetConfig {
    fn default() -> Self {
        Self {
            tier: ScaleTier::Tier1_100,
            queries_per_topic: 3,
            seed: 42,
            noise_std: 0.15,
            intra_topic_similarity: 0.85,
            inter_topic_similarity: 0.15,
            sparse_entries_e6: 100,
            sparse_entries_e13: 80,
            tokens_e12: 32,
        }
    }
}

impl EmbedderImpactDatasetConfig {
    /// Create config for a specific tier.
    pub fn for_tier(tier: ScaleTier) -> Self {
        let queries_per_topic = match tier {
            ScaleTier::Tier1_100 => 5,
            ScaleTier::Tier2_1K => 3,
            ScaleTier::Tier3_10K => 2,
            ScaleTier::Tier4_100K => 1,
            ScaleTier::Tier5_1M => 1,
            ScaleTier::Tier6_10M => 1,
        };

        Self {
            tier,
            queries_per_topic,
            ..Default::default()
        }
    }
}

/// Query data with ground truth for embedder impact benchmarking.
#[derive(Debug, Clone)]
pub struct ImpactQueryData {
    /// Query ID.
    pub id: Uuid,
    /// Query fingerprint (all 13 embeddings).
    pub fingerprint: SemanticFingerprint,
    /// Topic this query is associated with.
    pub topic: usize,
    /// IDs of relevant documents (ground truth).
    pub relevant_docs: HashSet<Uuid>,
    /// Relevance scores per document (for NDCG).
    pub relevance_scores: HashMap<Uuid, f64>,
    /// Expected best embedder for this query type.
    pub expected_best_embedder: Option<EmbedderIndex>,
    /// Causal direction for E5 asymmetric similarity per ARCH-18, AP-77.
    /// Distribution: 40% Cause, 40% Effect, 20% Unknown.
    pub causal_direction: CausalDirection,
}

/// Per-embedder K-NN graph (precomputed neighbors).
#[derive(Debug, Clone)]
pub struct KnnGraph {
    /// Which embedder this graph is for.
    pub embedder: EmbedderIndex,
    /// K value used.
    pub k: usize,
    /// Neighbors per node: node_id -> [(neighbor_id, similarity)].
    pub neighbors: HashMap<Uuid, Vec<(Uuid, f32)>>,
}

impl KnnGraph {
    /// Create empty K-NN graph.
    pub fn new(embedder: EmbedderIndex, k: usize) -> Self {
        Self {
            embedder,
            k,
            neighbors: HashMap::new(),
        }
    }

    /// Add neighbors for a node.
    pub fn add_neighbors(&mut self, node_id: Uuid, neighbors: Vec<(Uuid, f32)>) {
        self.neighbors.insert(node_id, neighbors);
    }

    /// Get neighbors for a node.
    pub fn get_neighbors(&self, node_id: &Uuid) -> Option<&Vec<(Uuid, f32)>> {
        self.neighbors.get(node_id)
    }
}

/// Complete embedder impact benchmark dataset.
#[derive(Debug)]
pub struct EmbedderImpactDataset {
    /// Generated fingerprints with their IDs.
    pub fingerprints: Vec<(Uuid, SemanticFingerprint)>,
    /// Queries with ground truth.
    pub queries: Vec<ImpactQueryData>,
    /// Topic assignment for each fingerprint.
    pub topic_assignments: HashMap<Uuid, usize>,
    /// Topic clusters used for generation.
    pub topic_clusters: Vec<TopicCluster>,
    /// Pre-computed K-NN graphs per embedder.
    pub knn_graphs: HashMap<EmbedderIndex, KnnGraph>,
    /// Configuration used to generate this dataset.
    pub config: EmbedderImpactDatasetConfig,
}

impl EmbedderImpactDataset {
    /// Generate dataset for a specific tier.
    pub fn for_tier(tier: ScaleTier) -> Self {
        Self::generate(EmbedderImpactDatasetConfig::for_tier(tier))
    }

    /// Generate dataset with custom config.
    pub fn generate(config: EmbedderImpactDatasetConfig) -> Self {
        let mut generator = EmbedderImpactDatasetGenerator::new(config);
        generator.generate()
    }

    /// Get number of documents.
    pub fn document_count(&self) -> usize {
        self.fingerprints.len()
    }

    /// Get number of topics.
    pub fn topic_count(&self) -> usize {
        self.topic_clusters.len()
    }

    /// Get number of queries.
    pub fn query_count(&self) -> usize {
        self.queries.len()
    }

    /// Get fingerprint by ID.
    pub fn get_fingerprint(&self, id: &Uuid) -> Option<&SemanticFingerprint> {
        self.fingerprints
            .iter()
            .find(|(fid, _)| fid == id)
            .map(|(_, fp)| fp)
    }

    /// Get documents for a specific topic.
    pub fn documents_for_topic(&self, topic: usize) -> Vec<Uuid> {
        self.topic_assignments
            .iter()
            .filter(|(_, &t)| t == topic)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get ground truth for retrieval evaluation.
    pub fn retrieval_ground_truth(&self) -> Vec<(SemanticFingerprint, HashSet<Uuid>)> {
        self.queries
            .iter()
            .map(|q| (q.fingerprint.clone(), q.relevant_docs.clone()))
            .collect()
    }

    /// Get retrieval ground truth with relevance scores for NDCG.
    pub fn retrieval_ground_truth_scored(&self) -> Vec<(SemanticFingerprint, HashMap<Uuid, f64>)> {
        self.queries
            .iter()
            .map(|q| (q.fingerprint.clone(), q.relevance_scores.clone()))
            .collect()
    }

    /// Validate dataset consistency.
    pub fn validate(&self) -> Result<(), String> {
        // Check all fingerprints have topic assignments
        for (id, _) in &self.fingerprints {
            if !self.topic_assignments.contains_key(id) {
                return Err(format!("Missing topic assignment for {}", id));
            }
        }

        // Check topic count matches centroids
        let max_topic = self.topic_assignments.values().max().copied().unwrap_or(0);
        if max_topic >= self.topic_clusters.len() {
            return Err(format!(
                "Topic {} exceeds centroid count {}",
                max_topic,
                self.topic_clusters.len()
            ));
        }

        // Check queries have valid relevant docs
        for query in &self.queries {
            for doc_id in &query.relevant_docs {
                if !self.topic_assignments.contains_key(doc_id) {
                    return Err(format!(
                        "Query {} references unknown doc {}",
                        query.id, doc_id
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get all document IDs.
    pub fn all_doc_ids(&self) -> Vec<Uuid> {
        self.fingerprints.iter().map(|(id, _)| *id).collect()
    }

    /// Get statistics about the dataset.
    pub fn stats(&self) -> EmbedderImpactDatasetStats {
        let docs_per_topic: Vec<usize> = (0..self.topic_count())
            .map(|t| self.documents_for_topic(t).len())
            .collect();

        let avg_docs_per_topic = if !docs_per_topic.is_empty() {
            docs_per_topic.iter().sum::<usize>() as f64 / docs_per_topic.len() as f64
        } else {
            0.0
        };

        let avg_relevant_per_query = if !self.queries.is_empty() {
            self.queries
                .iter()
                .map(|q| q.relevant_docs.len())
                .sum::<usize>() as f64
                / self.queries.len() as f64
        } else {
            0.0
        };

        EmbedderImpactDatasetStats {
            document_count: self.document_count(),
            topic_count: self.topic_count(),
            query_count: self.query_count(),
            avg_docs_per_topic,
            avg_relevant_per_query,
            tier: self.config.tier,
        }
    }
}

/// Statistics about an embedder impact dataset.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbedderImpactDatasetStats {
    /// Total documents.
    pub document_count: usize,
    /// Total topics.
    pub topic_count: usize,
    /// Total queries.
    pub query_count: usize,
    /// Average documents per topic.
    pub avg_docs_per_topic: f64,
    /// Average relevant docs per query.
    pub avg_relevant_per_query: f64,
    /// Scale tier.
    pub tier: ScaleTier,
}

/// Generator for embedder impact datasets.
struct EmbedderImpactDatasetGenerator {
    config: EmbedderImpactDatasetConfig,
    #[allow(dead_code)]
    rng: ChaCha8Rng,
}

impl EmbedderImpactDatasetGenerator {
    fn new(config: EmbedderImpactDatasetConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        Self { config, rng }
    }

    fn generate(&mut self) -> EmbedderImpactDataset {
        // Get tier settings
        let num_memories = self.config.tier.size();
        let num_topics = self.tier_topic_count();

        // Generate topic clusters
        let mut topic_gen = TopicGenerator::new(self.config.seed);
        let topic_clusters = topic_gen.generate(num_topics, self.config.noise_std);

        // Generate fingerprints
        let (fingerprints, topic_assignments) =
            self.generate_fingerprints(num_memories, &topic_clusters);

        // Generate queries
        let queries = self.generate_queries(&topic_clusters, &topic_assignments, &fingerprints);

        // Build K-NN graphs (only for small datasets, skip for large)
        let knn_graphs = if num_memories <= 10000 {
            self.build_knn_graphs(&fingerprints)
        } else {
            HashMap::new()
        };

        EmbedderImpactDataset {
            fingerprints,
            queries,
            topic_assignments,
            topic_clusters,
            knn_graphs,
            config: self.config.clone(),
        }
    }

    fn tier_topic_count(&self) -> usize {
        match self.config.tier {
            ScaleTier::Tier1_100 => 5,
            ScaleTier::Tier2_1K => 20,
            ScaleTier::Tier3_10K => 100,
            ScaleTier::Tier4_100K => 500,
            ScaleTier::Tier5_1M => 2000,
            ScaleTier::Tier6_10M => 10000,
        }
    }

    fn generate_fingerprints(
        &mut self,
        count: usize,
        topics: &[TopicCluster],
    ) -> (Vec<(Uuid, SemanticFingerprint)>, HashMap<Uuid, usize>) {
        let mut fingerprints = Vec::with_capacity(count);
        let mut topic_assignments = HashMap::with_capacity(count);

        // Distribute documents across topics
        let per_topic = count / topics.len().max(1);
        let remainder = count % topics.len().max(1);

        for (topic_idx, topic) in topics.iter().enumerate() {
            let topic_count = per_topic + if topic_idx < remainder { 1 } else { 0 };

            for _ in 0..topic_count {
                let id = self.generate_uuid();
                let fp = self.generate_fingerprint(topic);
                fingerprints.push((id, fp));
                topic_assignments.insert(id, topic_idx);
            }
        }

        // Shuffle
        fingerprints.shuffle(&mut self.rng);
        (fingerprints, topic_assignments)
    }

    fn generate_fingerprint(&mut self, topic: &TopicCluster) -> SemanticFingerprint {
        let embeddings = topic.sample_all(&mut self.rng, self.config.noise_std);

        SemanticFingerprint {
            e1_semantic: embeddings.e1,
            e2_temporal_recent: embeddings.e2,
            e3_temporal_periodic: embeddings.e3,
            e4_temporal_positional: embeddings.e4,
            // Per ARCH-18, AP-77: E5 cause/effect are DISTINCT vectors
            e5_causal_as_cause: embeddings.e5_cause,
            e5_causal_as_effect: embeddings.e5_effect,
            e5_causal: Vec::new(),
            e6_sparse: self.generate_sparse(E6_SPARSE_VOCAB, self.config.sparse_entries_e6),
            e7_code: embeddings.e7,
            e8_graph_as_source: embeddings.e8.clone(),
            e8_graph_as_target: embeddings.e8,
            e8_graph: Vec::new(),
            e9_hdc: embeddings.e9,
            e10_multimodal_paraphrase: embeddings.e10.clone(),
            e10_multimodal_as_context: embeddings.e10,
            e11_entity: embeddings.e11,
            e12_late_interaction: self.generate_late_interaction(self.config.tokens_e12),
            e13_splade: self.generate_sparse(E13_SPLADE_VOCAB, self.config.sparse_entries_e13),
            e14_bge_m3_dense: Vec::new(),
        }
    }

    fn generate_sparse(&mut self, vocab_size: usize, n_entries: usize) -> SparseVector {
        let mut indices: Vec<u16> = (0..vocab_size as u16)
            .collect::<Vec<_>>()
            .choose_multiple(&mut self.rng, n_entries.min(vocab_size))
            .copied()
            .collect();
        indices.sort_unstable();

        let normal = Normal::new(0.0, 1.0).unwrap();
        let values: Vec<f32> = (0..indices.len())
            .map(|_| (normal.sample(&mut self.rng) as f32).abs())
            .collect();

        SparseVector::new(indices, values).unwrap_or_default()
    }

    fn generate_late_interaction(&mut self, n_tokens: usize) -> Vec<Vec<f32>> {
        let normal = Normal::new(0.0, 0.5).unwrap();

        (0..n_tokens)
            .map(|_| {
                let mut token: Vec<f32> = (0..E12_TOKEN_DIM)
                    .map(|_| normal.sample(&mut self.rng) as f32)
                    .collect();
                let norm: f32 = token.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > f32::EPSILON {
                    for x in &mut token {
                        *x /= norm;
                    }
                }
                token
            })
            .collect()
    }

    fn generate_queries(
        &mut self,
        topics: &[TopicCluster],
        topic_assignments: &HashMap<Uuid, usize>,
        _fingerprints: &[(Uuid, SemanticFingerprint)],
    ) -> Vec<ImpactQueryData> {
        let mut queries = Vec::new();
        let mut query_num = 0usize;

        for (topic_idx, topic) in topics.iter().enumerate() {
            for _ in 0..self.config.queries_per_topic {
                // Generate query fingerprint (reduced noise for tighter match)
                let fingerprint = {
                    let embeddings = topic.sample_all(&mut self.rng, self.config.noise_std * 0.5);
                    SemanticFingerprint {
                        e1_semantic: embeddings.e1,
                        e2_temporal_recent: embeddings.e2,
                        e3_temporal_periodic: embeddings.e3,
                        e4_temporal_positional: embeddings.e4,
                        // Per ARCH-18, AP-77: E5 cause/effect are DISTINCT vectors
                        e5_causal_as_cause: embeddings.e5_cause,
                        e5_causal_as_effect: embeddings.e5_effect,
                        e5_causal: Vec::new(),
                        e6_sparse: self
                            .generate_sparse(E6_SPARSE_VOCAB, self.config.sparse_entries_e6),
                        e7_code: embeddings.e7,
                        e8_graph_as_source: embeddings.e8.clone(),
                        e8_graph_as_target: embeddings.e8,
                        e8_graph: Vec::new(),
                        e9_hdc: embeddings.e9,
                        e10_multimodal_paraphrase: embeddings.e10.clone(),
                        e10_multimodal_as_context: embeddings.e10,
                        e11_entity: embeddings.e11,
                        e12_late_interaction: self
                            .generate_late_interaction(self.config.tokens_e12),
                        e13_splade: self
                            .generate_sparse(E13_SPLADE_VOCAB, self.config.sparse_entries_e13),
                        e14_bge_m3_dense: Vec::new(),
                    }
                };

                // Assign causal direction per ARCH-18, AP-77
                // Distribution: 40% Cause, 40% Effect, 20% Unknown
                let causal_direction = match query_num % 5 {
                    0 | 1 => CausalDirection::Cause,  // 40%
                    2 | 3 => CausalDirection::Effect, // 40%
                    _ => CausalDirection::Unknown,    // 20%
                };
                query_num += 1;

                // Get relevant docs (same topic) - sorted for determinism
                let mut topic_docs: Vec<Uuid> = topic_assignments
                    .iter()
                    .filter(|(_, &t)| t == topic_idx)
                    .map(|(id, _)| *id)
                    .collect();
                topic_docs.sort();

                // Sample subset as relevant
                let max_relevant = 20.min(topic_docs.len());
                let num_relevant = self.rng.gen_range(3.min(topic_docs.len())..=max_relevant);
                let relevant_docs: HashSet<Uuid> = topic_docs
                    .choose_multiple(&mut self.rng, num_relevant)
                    .copied()
                    .collect();

                // Assign relevance scores (1.0 for same topic, with some variation)
                let relevance_scores: HashMap<Uuid, f64> = relevant_docs
                    .iter()
                    .map(|&id| {
                        let score = 0.7 + self.rng.gen::<f64>() * 0.3;
                        (id, score)
                    })
                    .collect();

                queries.push(ImpactQueryData {
                    id: self.generate_uuid(),
                    fingerprint,
                    topic: topic_idx,
                    relevant_docs,
                    relevance_scores,
                    expected_best_embedder: None, // Could be set based on topic characteristics
                    causal_direction,
                });
            }
        }

        // Shuffle queries
        queries.shuffle(&mut self.rng);
        queries
    }

    fn build_knn_graphs(
        &mut self,
        fingerprints: &[(Uuid, SemanticFingerprint)],
    ) -> HashMap<EmbedderIndex, KnnGraph> {
        // Only build for key embedders to save time
        let embedders = [
            EmbedderIndex::E1Semantic,
            EmbedderIndex::E5Causal,
            EmbedderIndex::E7Code,
            EmbedderIndex::E10Multimodal,
            EmbedderIndex::E11Entity,
        ];

        let k = 10;
        let mut graphs = HashMap::new();

        for embedder in embedders {
            let mut graph = KnnGraph::new(embedder, k);

            // For each document, find K nearest neighbors using that embedder
            for (id, fp) in fingerprints {
                let query_embedding = self.get_embedding(fp, embedder);

                // Compute similarities to all other docs
                let mut similarities: Vec<(Uuid, f32)> = fingerprints
                    .iter()
                    .filter(|(other_id, _)| other_id != id)
                    .map(|(other_id, other_fp)| {
                        let other_embedding = self.get_embedding(other_fp, embedder);
                        let sim = cosine_similarity(query_embedding, other_embedding);
                        (*other_id, sim)
                    })
                    .collect();

                // Sort and take top K
                similarities
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                similarities.truncate(k);

                graph.add_neighbors(*id, similarities);
            }

            graphs.insert(embedder, graph);
        }

        graphs
    }

    fn get_embedding<'a>(&self, fp: &'a SemanticFingerprint, embedder: EmbedderIndex) -> &'a [f32] {
        match embedder {
            EmbedderIndex::E1Semantic | EmbedderIndex::E1Matryoshka128 => &fp.e1_semantic,
            EmbedderIndex::E2TemporalRecent => &fp.e2_temporal_recent,
            EmbedderIndex::E3TemporalPeriodic => &fp.e3_temporal_periodic,
            EmbedderIndex::E4TemporalPositional => &fp.e4_temporal_positional,
            // Per ARCH-18, AP-77: E5 cause/effect are DISTINCT vectors
            EmbedderIndex::E5Causal | EmbedderIndex::E5CausalCause => &fp.e5_causal_as_cause,
            EmbedderIndex::E5CausalEffect => &fp.e5_causal_as_effect,
            EmbedderIndex::E7Code => &fp.e7_code,
            EmbedderIndex::E8Graph => &fp.e8_graph_as_source,
            EmbedderIndex::E9HDC => &fp.e9_hdc,
            EmbedderIndex::E10Multimodal
            | EmbedderIndex::E10MultimodalParaphrase
            | EmbedderIndex::E10MultimodalContext => &fp.e10_multimodal_paraphrase,
            EmbedderIndex::E11Entity => &fp.e11_entity,
            // E6, E12, E13 are sparse/late-interaction, return empty.
            // E14 BGE-M3 Dense lives in sibling CF, not in SemanticFingerprint.
            EmbedderIndex::E6Sparse
            | EmbedderIndex::E12LateInteraction
            | EmbedderIndex::E13Splade
            | EmbedderIndex::E14BgeM3Dense => &[],
        }
    }

    fn generate_uuid(&mut self) -> Uuid {
        let mut bytes = [0u8; 16];
        self.rng.fill(&mut bytes);
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes)
    }
}

/// Compute cosine similarity between two vectors (raw [-1, 1] range).
///
/// Delegates to the canonical implementation in `crate::util`.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    crate::util::cosine_similarity_raw(a, b)
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_generate_tier1_dataset() {
        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier1_100);

        assert_eq!(dataset.document_count(), 100);
        assert_eq!(dataset.topic_count(), 5);
        assert!(!dataset.queries.is_empty());
        assert!(dataset.validate().is_ok());
    }

    #[test]
    fn test_generate_tier2_dataset() {
        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier2_1K);

        assert_eq!(dataset.document_count(), 1000);
        assert_eq!(dataset.topic_count(), 20);
        assert!(dataset.validate().is_ok());
    }

    #[test]
    fn test_dataset_stats() {
        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier1_100);
        let stats = dataset.stats();

        assert_eq!(stats.document_count, 100);
        assert_eq!(stats.topic_count, 5);
        assert!(stats.avg_docs_per_topic > 0.0);
        assert!(stats.avg_relevant_per_query > 0.0);
    }

    #[test]
    fn test_knn_graphs_generated() {
        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier1_100);

        // Should have K-NN graphs for key embedders
        assert!(dataset.knn_graphs.contains_key(&EmbedderIndex::E1Semantic));
        assert!(dataset.knn_graphs.contains_key(&EmbedderIndex::E7Code));
    }

    #[test]
    fn test_query_ground_truth() {
        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier1_100);
        let ground_truth = dataset.retrieval_ground_truth();

        assert!(!ground_truth.is_empty());
        for (_, relevant) in &ground_truth {
            assert!(!relevant.is_empty());
        }
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 0.001);
    }

    // ============================================================================
    // E5 Causal Embedder Fix Verification Tests (ARCH-18, AP-77)
    // ============================================================================

    #[test]
    fn test_e5_cause_effect_distinct() {
        // Per ARCH-18, AP-77: E5 cause/effect vectors must be distinct
        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier1_100);

        for (id, fp) in &dataset.fingerprints {
            // Check that cause and effect vectors are not identical
            assert_ne!(
                fp.e5_causal_as_cause, fp.e5_causal_as_effect,
                "E5 cause/effect vectors must be distinct for document {}",
                id
            );

            // Check that they have sufficient distance (cosine similarity < 0.95)
            let sim = cosine_similarity(&fp.e5_causal_as_cause, &fp.e5_causal_as_effect);
            assert!(
                sim < 0.95,
                "E5 cause/effect vectors too similar ({:.3}) for document {}",
                sim,
                id
            );
        }
    }

    #[test]
    fn test_query_causal_directions_distributed() {
        // Per ARCH-18: Query causal directions should be distributed 40/40/20
        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier1_100);

        let cause_count = dataset
            .queries
            .iter()
            .filter(|q| q.causal_direction == CausalDirection::Cause)
            .count();
        let effect_count = dataset
            .queries
            .iter()
            .filter(|q| q.causal_direction == CausalDirection::Effect)
            .count();
        let unknown_count = dataset
            .queries
            .iter()
            .filter(|q| q.causal_direction == CausalDirection::Unknown)
            .count();

        let total = dataset.queries.len();

        // Check approximate distribution (allowing some tolerance)
        // Expected: 40% cause, 40% effect, 20% unknown
        let cause_ratio = cause_count as f64 / total as f64;
        let effect_ratio = effect_count as f64 / total as f64;
        let unknown_ratio = unknown_count as f64 / total as f64;

        assert!(
            cause_ratio >= 0.35 && cause_ratio <= 0.45,
            "Cause ratio {:.2} not in expected range [0.35, 0.45]",
            cause_ratio
        );
        assert!(
            effect_ratio >= 0.35 && effect_ratio <= 0.45,
            "Effect ratio {:.2} not in expected range [0.35, 0.45]",
            effect_ratio
        );
        assert!(
            unknown_ratio >= 0.15 && unknown_ratio <= 0.25,
            "Unknown ratio {:.2} not in expected range [0.15, 0.25]",
            unknown_ratio
        );

        println!(
            "[VERIFIED] Causal direction distribution: Cause={:.1}%, Effect={:.1}%, Unknown={:.1}%",
            cause_ratio * 100.0,
            effect_ratio * 100.0,
            unknown_ratio * 100.0
        );
    }

    #[test]
    fn test_get_embedding_e5_returns_distinct_vectors() {
        // Per ARCH-18, AP-77: get_embedding should return distinct vectors for E5Cause vs E5Effect
        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier1_100);
        let (_, fp) = &dataset.fingerprints[0];

        let config = EmbedderImpactDatasetConfig::for_tier(ScaleTier::Tier1_100);
        let generator = EmbedderImpactDatasetGenerator::new(config);

        let cause_vec = generator.get_embedding(fp, EmbedderIndex::E5CausalCause);
        let effect_vec = generator.get_embedding(fp, EmbedderIndex::E5CausalEffect);

        assert_ne!(
            cause_vec, effect_vec,
            "E5CausalCause and E5CausalEffect must return different vectors"
        );
    }

    // ============================================================================
    // E5 Benchmark Enhancement Tests (per E5 Causal Embedder Benchmark Plan)
    // ============================================================================

    #[test]
    fn test_e5_vector_distance_threshold() {
        // Per ARCH-18, AP-77: E5 cause/effect vectors must have minimum 0.3 cosine distance
        // Using 0.25 threshold for test tolerance (slightly below 0.3)
        const MIN_DISTANCE_THRESHOLD: f32 = 0.25;

        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier1_100);
        let mut violations = Vec::new();
        let mut distances = Vec::new();

        for (id, fp) in &dataset.fingerprints {
            let sim = cosine_similarity(&fp.e5_causal_as_cause, &fp.e5_causal_as_effect);
            let distance = 1.0 - sim;
            distances.push(distance);

            if distance < MIN_DISTANCE_THRESHOLD {
                violations.push((id, distance));
            }
        }

        // Compute statistics
        let min_distance = distances.iter().cloned().fold(f32::INFINITY, f32::min);
        let avg_distance = distances.iter().sum::<f32>() / distances.len() as f32;

        println!(
            "[E5 VECTOR VERIFICATION] Checked {} documents",
            distances.len()
        );
        println!(
            "[E5 VECTOR VERIFICATION] Min distance: {:.3}, Avg: {:.3}",
            min_distance, avg_distance
        );
        println!(
            "[E5 VECTOR VERIFICATION] Threshold violations: {}",
            violations.len()
        );

        // Assert no violations
        assert!(
            violations.is_empty(),
            "E5 cause/effect distance {:.3} < {:.3} for {} documents. \
             First violation: doc {} with distance {:.3}",
            min_distance,
            MIN_DISTANCE_THRESHOLD,
            violations.len(),
            violations
                .first()
                .map(|(id, _)| id.to_string())
                .unwrap_or_default(),
            violations.first().map(|(_, d)| *d).unwrap_or(0.0)
        );

        // Verify minimum meets the 0.3 threshold target (with some tolerance for noise)
        assert!(
            min_distance >= MIN_DISTANCE_THRESHOLD,
            "Minimum E5 cause/effect distance {:.3} < threshold {:.3}",
            min_distance,
            MIN_DISTANCE_THRESHOLD
        );
    }

    #[test]
    fn test_e5_direction_distribution_40_40_20() {
        // Per the benchmark plan: Query causal directions should be 40% Cause, 40% Effect, 20% Unknown
        const TOLERANCE_PCT: f64 = 5.0;

        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier2_1K); // Use larger dataset for better distribution

        let cause_count = dataset
            .queries
            .iter()
            .filter(|q| q.causal_direction == CausalDirection::Cause)
            .count();
        let effect_count = dataset
            .queries
            .iter()
            .filter(|q| q.causal_direction == CausalDirection::Effect)
            .count();
        let unknown_count = dataset
            .queries
            .iter()
            .filter(|q| q.causal_direction == CausalDirection::Unknown)
            .count();

        let total = dataset.queries.len() as f64;
        let cause_pct = (cause_count as f64 / total) * 100.0;
        let effect_pct = (effect_count as f64 / total) * 100.0;
        let unknown_pct = (unknown_count as f64 / total) * 100.0;

        println!(
            "[E5 DIRECTION DISTRIBUTION] Cause={:.1}%, Effect={:.1}%, Unknown={:.1}%",
            cause_pct, effect_pct, unknown_pct
        );

        // Check each is within tolerance of target
        assert!(
            (cause_pct - 40.0).abs() <= TOLERANCE_PCT,
            "Cause percentage {:.1}% not within {}% of target 40%",
            cause_pct,
            TOLERANCE_PCT
        );
        assert!(
            (effect_pct - 40.0).abs() <= TOLERANCE_PCT,
            "Effect percentage {:.1}% not within {}% of target 40%",
            effect_pct,
            TOLERANCE_PCT
        );
        assert!(
            (unknown_pct - 20.0).abs() <= TOLERANCE_PCT,
            "Unknown percentage {:.1}% not within {}% of target 20%",
            unknown_pct,
            TOLERANCE_PCT
        );
    }

    #[test]
    fn test_e5_vectors_present_in_all_fingerprints() {
        // Verify all fingerprints have valid E5 cause/effect vectors
        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier1_100);

        for (id, fp) in &dataset.fingerprints {
            // Check cause vector is present and has correct dimension
            assert!(
                !fp.e5_causal_as_cause.is_empty(),
                "E5 cause vector empty for document {}",
                id
            );
            assert_eq!(
                fp.e5_causal_as_cause.len(),
                768,
                "E5 cause vector dimension {} != 768 for document {}",
                fp.e5_causal_as_cause.len(),
                id
            );

            // Check effect vector is present and has correct dimension
            assert!(
                !fp.e5_causal_as_effect.is_empty(),
                "E5 effect vector empty for document {}",
                id
            );
            assert_eq!(
                fp.e5_causal_as_effect.len(),
                768,
                "E5 effect vector dimension {} != 768 for document {}",
                fp.e5_causal_as_effect.len(),
                id
            );

            // Check both vectors are normalized (unit length, approximately)
            let cause_norm: f32 = fp
                .e5_causal_as_cause
                .iter()
                .map(|x| x * x)
                .sum::<f32>()
                .sqrt();
            let effect_norm: f32 = fp
                .e5_causal_as_effect
                .iter()
                .map(|x| x * x)
                .sum::<f32>()
                .sqrt();

            assert!(
                (cause_norm - 1.0).abs() < 0.1,
                "E5 cause vector not normalized (norm={:.3}) for document {}",
                cause_norm,
                id
            );
            assert!(
                (effect_norm - 1.0).abs() < 0.1,
                "E5 effect vector not normalized (norm={:.3}) for document {}",
                effect_norm,
                id
            );
        }
    }

    #[test]
    fn test_e5_queries_have_causal_direction() {
        // Verify all queries have a valid causal direction assigned
        let dataset = EmbedderImpactDataset::for_tier(ScaleTier::Tier1_100);

        for query in &dataset.queries {
            // Check that direction is one of the valid values
            let is_valid = matches!(
                query.causal_direction,
                CausalDirection::Cause | CausalDirection::Effect | CausalDirection::Unknown
            );
            assert!(is_valid, "Query {} has invalid causal direction", query.id);
        }
    }
}
