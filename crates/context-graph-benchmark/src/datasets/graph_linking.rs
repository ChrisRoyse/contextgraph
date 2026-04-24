//! Graph linking benchmark dataset generation.
//!
//! Generates test data for benchmarking graph linking operations:
//! - NN-Descent K-NN graph construction
//! - EdgeBuilder edge creation
//! - BackgroundGraphBuilder batch processing
//! - Graph expansion in retrieval pipeline
//! - R-GCN GNN inference

use std::collections::HashMap;

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rand_distr::Normal;
use uuid::Uuid;

use context_graph_core::graph_linking::GraphLinkEdgeType;
use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E6_SPARSE_VOCAB,
};

use super::topic_clusters::{TopicCluster, TopicGenerator};

/// Configuration for graph linking dataset generation.
#[derive(Debug, Clone)]
pub struct GraphLinkingDatasetConfig {
    /// Random seed for reproducibility.
    pub seed: u64,
    /// Number of memories to generate.
    pub num_memories: usize,
    /// Number of topic clusters.
    pub num_topics: usize,
    /// Intra-topic similarity (0.0-1.0). Higher = more clustered.
    pub intra_topic_similarity: f32,
    /// Inter-topic similarity (0.0-1.0). Lower = more separated.
    pub inter_topic_similarity: f32,
    /// K for K-NN graph construction.
    pub k_neighbors: usize,
    /// Minimum weighted agreement for edge creation.
    pub min_edge_agreement: f32,
    /// Noise standard deviation for embeddings.
    pub noise_std: f32,
}

impl Default for GraphLinkingDatasetConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            num_memories: 1000,
            num_topics: 20,
            intra_topic_similarity: 0.85,
            inter_topic_similarity: 0.15,
            k_neighbors: 20,
            min_edge_agreement: 2.5,
            noise_std: 0.1,
        }
    }
}

impl GraphLinkingDatasetConfig {
    /// Create config for a specific benchmark tier.
    pub fn for_tier(tier: ScaleTier) -> Self {
        match tier {
            ScaleTier::Tier1_100 => Self {
                num_memories: 100,
                num_topics: 5,
                k_neighbors: 10,
                ..Default::default()
            },
            ScaleTier::Tier2_1K => Self {
                num_memories: 1_000,
                num_topics: 20,
                k_neighbors: 20,
                ..Default::default()
            },
            ScaleTier::Tier3_10K => Self {
                num_memories: 10_000,
                num_topics: 100,
                k_neighbors: 20,
                ..Default::default()
            },
            ScaleTier::Tier4_100K => Self {
                num_memories: 100_000,
                num_topics: 500,
                k_neighbors: 30,
                ..Default::default()
            },
            ScaleTier::Tier5_1M => Self {
                num_memories: 1_000_000,
                num_topics: 2000,
                k_neighbors: 50,
                ..Default::default()
            },
            ScaleTier::Tier6_10M => Self {
                num_memories: 10_000_000,
                num_topics: 10000,
                k_neighbors: 50,
                ..Default::default()
            },
        }
    }
}

/// Scale tiers for benchmarking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ScaleTier {
    /// 100 memories, 5 topics
    Tier1_100,
    /// 1K memories, 20 topics
    Tier2_1K,
    /// 10K memories, 100 topics
    Tier3_10K,
    /// 100K memories, 500 topics
    Tier4_100K,
    /// 1M memories, 2000 topics
    Tier5_1M,
    /// 10M memories, 10000 topics
    Tier6_10M,
}

impl ScaleTier {
    /// Get tier level (1-6).
    pub fn level(&self) -> u8 {
        match self {
            Self::Tier1_100 => 1,
            Self::Tier2_1K => 2,
            Self::Tier3_10K => 3,
            Self::Tier4_100K => 4,
            Self::Tier5_1M => 5,
            Self::Tier6_10M => 6,
        }
    }

    /// Get number of memories.
    pub fn size(&self) -> usize {
        match self {
            Self::Tier1_100 => 100,
            Self::Tier2_1K => 1_000,
            Self::Tier3_10K => 10_000,
            Self::Tier4_100K => 100_000,
            Self::Tier5_1M => 1_000_000,
            Self::Tier6_10M => 10_000_000,
        }
    }

    /// Get tier name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Tier1_100 => "100",
            Self::Tier2_1K => "1K",
            Self::Tier3_10K => "10K",
            Self::Tier4_100K => "100K",
            Self::Tier5_1M => "1M",
            Self::Tier6_10M => "10M",
        }
    }

    /// Create tier from level number.
    pub fn from_level(level: u8) -> anyhow::Result<Self> {
        match level {
            1 => Ok(Self::Tier1_100),
            2 => Ok(Self::Tier2_1K),
            3 => Ok(Self::Tier3_10K),
            4 => Ok(Self::Tier4_100K),
            5 => Ok(Self::Tier5_1M),
            6 => Ok(Self::Tier6_10M),
            _ => anyhow::bail!("Invalid tier level: {}. Expected 1-6.", level),
        }
    }
}

/// Memory data with all 13 embeddings.
#[derive(Debug, Clone)]
pub struct MemoryData {
    /// Memory ID.
    pub id: Uuid,
    /// Full semantic fingerprint.
    pub fingerprint: SemanticFingerprint,
    /// Topic cluster assignment.
    pub topic_id: usize,
}

/// Expected edge between two memories.
#[derive(Debug, Clone)]
pub struct ExpectedEdge {
    /// Source memory ID.
    pub source: Uuid,
    /// Target memory ID.
    pub target: Uuid,
    /// Expected edge type.
    pub edge_type: GraphLinkEdgeType,
    /// Minimum expected weight.
    pub min_weight: f32,
    /// Whether this edge is expected (based on topic membership).
    pub is_same_topic: bool,
}

/// Embedding set for a single memory (used for similarity computation).
#[derive(Debug, Clone)]
pub struct EmbeddingScores {
    /// Similarity scores from all 13 embedders.
    pub scores: [f32; 14],
}

/// Complete graph linking benchmark dataset.
#[derive(Debug)]
pub struct GraphLinkingDataset {
    /// Generated memories with fingerprints.
    pub memories: Vec<MemoryData>,
    /// Topic clusters used for generation.
    pub topic_clusters: Vec<TopicCluster>,
    /// Topic assignment map (id -> topic_id).
    pub topic_assignments: HashMap<Uuid, usize>,
    /// Expected edges based on topic membership.
    pub expected_edges: Vec<ExpectedEdge>,
    /// Configuration used to generate this dataset.
    pub config: GraphLinkingDatasetConfig,
}

impl GraphLinkingDataset {
    /// Generate dataset for a specific tier.
    pub fn for_tier(tier: ScaleTier) -> Self {
        Self::generate(GraphLinkingDatasetConfig::for_tier(tier))
    }

    /// Generate dataset with custom config.
    pub fn generate(config: GraphLinkingDatasetConfig) -> Self {
        let mut generator = GraphLinkingDatasetGenerator::new(config.clone());
        generator.generate()
    }

    /// Get number of memories.
    pub fn memory_count(&self) -> usize {
        self.memories.len()
    }

    /// Get number of topics.
    pub fn topic_count(&self) -> usize {
        self.topic_clusters.len()
    }

    /// Get memories in a specific topic.
    pub fn memories_in_topic(&self, topic_id: usize) -> Vec<&MemoryData> {
        self.memories
            .iter()
            .filter(|m| m.topic_id == topic_id)
            .collect()
    }

    /// Get sample pairs for edge building.
    pub fn sample_pairs(&self, count: usize) -> impl Iterator<Item = MemoryPair<'_>> + '_ {
        let mut rng = ChaCha8Rng::seed_from_u64(self.config.seed + 100);
        let memories = &self.memories;

        (0..count).filter_map(move |_| {
            if memories.len() < 2 {
                return None;
            }
            let i = rng.gen_range(0..memories.len());
            let j = rng.gen_range(0..memories.len());
            if i != j {
                Some(MemoryPair {
                    source: &memories[i],
                    target: &memories[j],
                })
            } else {
                None
            }
        })
    }

    /// Get sample embedder score vectors.
    pub fn sample_scores(&self, count: usize) -> impl Iterator<Item = [f32; 14]> {
        let mut rng = ChaCha8Rng::seed_from_u64(self.config.seed + 200);
        let intra_sim = self.config.intra_topic_similarity;
        let inter_sim = self.config.inter_topic_similarity;

        (0..count).map(move |_| {
            let is_same_topic = rng.gen_bool(0.7); // 70% same topic pairs
            let base = if is_same_topic { intra_sim } else { inter_sim };
            let noise = 0.1;

            let mut scores = [0.0f32; 14];
            for score in &mut scores {
                *score = (base + rng.gen_range(-noise..noise)).clamp(0.0, 1.0);
            }
            // Set temporal to 0 (per AP-60)
            scores[1] = 0.0; // E2
            scores[2] = 0.0; // E3
            scores[3] = 0.0; // E4
            scores
        })
    }

    /// Get sample subgraphs for GNN benchmarking.
    pub fn sample_subgraphs(&self, count: usize) -> impl Iterator<Item = SubgraphData> + '_ {
        let mut rng = ChaCha8Rng::seed_from_u64(self.config.seed + 300);
        let k = self.config.k_neighbors;

        (0..count).map(move |_| {
            let num_nodes = rng.gen_range(10..=200);
            let num_edges = rng.gen_range(num_nodes..num_nodes * k);

            SubgraphData {
                num_nodes,
                num_edges,
                features: vec![vec![0.0f32; 32]; num_nodes],
                edges: (0..num_edges)
                    .map(|_| (rng.gen_range(0..num_nodes), rng.gen_range(0..num_nodes)))
                    .collect(),
                edge_types: (0..num_edges).map(|_| rng.gen_range(0..8)).collect(),
            }
        })
    }

    /// Get sample candidate sets for graph expansion benchmarking.
    pub fn sample_candidate_sets(&self, count: usize) -> impl Iterator<Item = Vec<CandidateData>> + '_ {
        let mut rng = ChaCha8Rng::seed_from_u64(self.config.seed + 400);
        let memories = &self.memories;

        (0..count).map(move |_| {
            let set_size = rng.gen_range(10..=100);
            (0..set_size)
                .filter_map(|_| {
                    if memories.is_empty() {
                        return None;
                    }
                    let idx = rng.gen_range(0..memories.len());
                    Some(CandidateData {
                        id: memories[idx].id,
                        score: rng.gen_range(0.3..1.0),
                    })
                })
                .collect()
        })
    }

    /// Get average subgraph nodes.
    pub fn avg_subgraph_nodes(&self) -> usize {
        100 // Default average
    }

    /// Get average subgraph edges.
    pub fn avg_subgraph_edges(&self) -> usize {
        500 // Default average
    }
}

/// Memory pair for edge building.
pub struct MemoryPair<'a> {
    pub source: &'a MemoryData,
    pub target: &'a MemoryData,
}

/// Subgraph data for GNN benchmarking.
#[derive(Debug, Clone)]
pub struct SubgraphData {
    pub num_nodes: usize,
    pub num_edges: usize,
    pub features: Vec<Vec<f32>>,
    pub edges: Vec<(usize, usize)>,
    pub edge_types: Vec<u8>,
}

/// Candidate data for retrieval benchmarking.
#[derive(Debug, Clone)]
pub struct CandidateData {
    pub id: Uuid,
    pub score: f32,
}

/// Generator for graph linking benchmark datasets.
struct GraphLinkingDatasetGenerator {
    config: GraphLinkingDatasetConfig,
    rng: ChaCha8Rng,
}

impl GraphLinkingDatasetGenerator {
    fn new(config: GraphLinkingDatasetConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        Self { config, rng }
    }

    fn generate(&mut self) -> GraphLinkingDataset {
        // Generate topic clusters
        let mut topic_gen = TopicGenerator::new(self.config.seed);
        let topic_clusters = topic_gen.generate(
            self.config.num_topics,
            1.0 - self.config.intra_topic_similarity,
        );

        // Generate memories
        let memories = self.generate_memories(&topic_clusters);

        // Build topic assignment map
        let topic_assignments: HashMap<Uuid, usize> = memories
            .iter()
            .map(|m| (m.id, m.topic_id))
            .collect();

        // Generate expected edges
        let expected_edges = self.generate_expected_edges(&memories);

        GraphLinkingDataset {
            memories,
            topic_clusters,
            topic_assignments,
            expected_edges,
            config: self.config.clone(),
        }
    }

    fn generate_memories(&mut self, topic_clusters: &[TopicCluster]) -> Vec<MemoryData> {
        let mut memories = Vec::with_capacity(self.config.num_memories);

        // Distribute memories across topics
        let per_topic = self.config.num_memories / self.config.num_topics.max(1);
        let remainder = self.config.num_memories % self.config.num_topics.max(1);

        for (topic_id, topic) in topic_clusters.iter().enumerate() {
            let count = per_topic + if topic_id < remainder { 1 } else { 0 };

            for _ in 0..count {
                let id = self.generate_uuid();
                let fingerprint = self.generate_fingerprint(topic);

                memories.push(MemoryData {
                    id,
                    fingerprint,
                    topic_id,
                });
            }
        }

        // Shuffle memories
        memories.shuffle(&mut self.rng);
        memories
    }

    fn generate_fingerprint(&mut self, topic: &TopicCluster) -> SemanticFingerprint {
        let embeddings = topic.sample_all(&mut self.rng, self.config.noise_std);

        SemanticFingerprint {
            e1_semantic: embeddings.e1,
            e2_temporal_recent: embeddings.e2,
            e3_temporal_periodic: embeddings.e3,
            e4_temporal_positional: embeddings.e4,
            // Per ARCH-18, AP-77: E5 cause/effect are distinct vectors
            e5_causal_as_cause: embeddings.e5_cause,
            e5_causal_as_effect: embeddings.e5_effect,
            e5_causal: Vec::new(),
            e6_sparse: self.generate_sparse(E6_SPARSE_VOCAB, 100),
            e7_code: embeddings.e7,
            e8_graph_as_source: embeddings.e8.clone(),
            e8_graph_as_target: embeddings.e8,
            e8_graph: Vec::new(),
            e9_hdc: embeddings.e9,
            e10_multimodal_paraphrase: embeddings.e10.clone(),
            e10_multimodal_as_context: embeddings.e10,
            e11_entity: embeddings.e11,
            e12_late_interaction: self.generate_late_interaction(32),
            e13_splade: self.generate_sparse(E13_SPLADE_VOCAB, 80),
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

    fn generate_expected_edges(&mut self, memories: &[MemoryData]) -> Vec<ExpectedEdge> {
        let mut edges = Vec::new();

        // Build topic index
        let mut topic_members: HashMap<usize, Vec<usize>> = HashMap::new();
        for (idx, memory) in memories.iter().enumerate() {
            topic_members
                .entry(memory.topic_id)
                .or_default()
                .push(idx);
        }

        // Generate edges within topics (high probability)
        for members in topic_members.values() {
            for &i in members.iter().take(100) {
                // Limit per topic for performance
                for &j in members.iter().take(10) {
                    if i != j {
                        edges.push(ExpectedEdge {
                            source: memories[i].id,
                            target: memories[j].id,
                            edge_type: GraphLinkEdgeType::SemanticSimilar,
                            min_weight: self.config.min_edge_agreement / 8.5,
                            is_same_topic: true,
                        });
                    }
                }
            }
        }

        edges
    }

    fn generate_uuid(&mut self) -> Uuid {
        let mut bytes = [0u8; 16];
        self.rng.fill(&mut bytes);
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes)
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_generate_tier1_dataset() {
        let dataset = GraphLinkingDataset::for_tier(ScaleTier::Tier1_100);

        assert_eq!(dataset.memory_count(), 100);
        assert_eq!(dataset.topic_count(), 5);
        assert!(!dataset.memories.is_empty());
        assert!(!dataset.expected_edges.is_empty());
    }

    #[test]
    fn test_generate_tier2_dataset() {
        let dataset = GraphLinkingDataset::for_tier(ScaleTier::Tier2_1K);

        assert_eq!(dataset.memory_count(), 1000);
        assert_eq!(dataset.topic_count(), 20);
    }

    #[test]
    fn test_topic_distribution() {
        let dataset = GraphLinkingDataset::for_tier(ScaleTier::Tier1_100);

        // All memories should have valid topic assignments
        for memory in &dataset.memories {
            assert!(memory.topic_id < dataset.topic_count());
            assert!(dataset.topic_assignments.contains_key(&memory.id));
        }
    }

    #[test]
    fn test_sample_pairs() {
        let dataset = GraphLinkingDataset::for_tier(ScaleTier::Tier1_100);
        let pairs: Vec<_> = dataset.sample_pairs(10).collect();

        assert!(!pairs.is_empty());
        for pair in &pairs {
            assert_ne!(pair.source.id, pair.target.id);
        }
    }

    #[test]
    fn test_sample_scores() {
        let dataset = GraphLinkingDataset::for_tier(ScaleTier::Tier1_100);
        let scores: Vec<_> = dataset.sample_scores(10).collect();

        assert_eq!(scores.len(), 10);
        for s in &scores {
            assert_eq!(s.len(), 13);
            // Temporal should be 0
            assert_eq!(s[1], 0.0);
            assert_eq!(s[2], 0.0);
            assert_eq!(s[3], 0.0);
        }
    }

    #[test]
    fn test_scale_tier_from_level() {
        assert_eq!(ScaleTier::from_level(1).unwrap(), ScaleTier::Tier1_100);
        assert_eq!(ScaleTier::from_level(2).unwrap(), ScaleTier::Tier2_1K);
        assert_eq!(ScaleTier::from_level(6).unwrap(), ScaleTier::Tier6_10M);
        assert!(ScaleTier::from_level(7).is_err());
    }
}
