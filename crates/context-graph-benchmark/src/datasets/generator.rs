//! Synthetic dataset generator for benchmarks.
//!
//! Generates controlled test data with known ground truth for evaluating
//! retrieval and clustering performance across different scales.

use std::collections::{HashMap, HashSet};

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rand_distr::Normal;
use uuid::Uuid;

use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E6_SPARSE_VOCAB,
};

#[cfg(test)]
use context_graph_core::types::fingerprint::{
    E10_DIM, E11_DIM, E1_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM, E7_DIM, E8_DIM, E9_DIM,
};

use super::topic_clusters::{TopicCluster, TopicGenerator};
use super::{BenchmarkDataset, QueryData};
use crate::config::TierConfig;

/// Configuration for dataset generation.
#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    /// Random seed.
    pub seed: u64,
    /// Noise standard deviation for intra-topic variation.
    pub noise_std: f32,
    /// Fraction of queries that should be divergent (out-of-topic).
    pub divergent_query_fraction: f32,
    /// Number of sparse entries per E6 embedding.
    pub sparse_entries_e6: usize,
    /// Number of sparse entries per E13 embedding.
    pub sparse_entries_e13: usize,
    /// Number of tokens for E12 late interaction.
    pub tokens_e12: usize,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            noise_std: 0.15,
            divergent_query_fraction: 0.1,
            sparse_entries_e6: 100,
            sparse_entries_e13: 80,
            tokens_e12: 32,
        }
    }
}

/// Dataset generator for creating synthetic benchmark data.
pub struct DatasetGenerator {
    config: GeneratorConfig,
    rng: ChaCha8Rng,
}

impl DatasetGenerator {
    /// Create a new generator with default config.
    pub fn new(seed: u64) -> Self {
        Self::with_config(GeneratorConfig {
            seed,
            ..Default::default()
        })
    }

    /// Create a new generator with specific config.
    pub fn with_config(config: GeneratorConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        Self { config, rng }
    }

    /// Generate a deterministic UUID using the seeded RNG.
    fn generate_deterministic_uuid(&mut self) -> Uuid {
        let mut bytes = [0u8; 16];
        self.rng.fill(&mut bytes);
        // Set version to 4 (random) and variant to RFC 4122
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes)
    }

    /// Generate a complete benchmark dataset for a tier.
    pub fn generate_dataset(&mut self, tier_config: &TierConfig) -> BenchmarkDataset {
        // Generate topics
        let mut topic_generator = TopicGenerator::new(self.config.seed);
        let topic_centroids = topic_generator.generate(tier_config.topic_count, self.config.noise_std);

        // Generate fingerprints
        let (fingerprints, topic_assignments) = self.generate_fingerprints(
            tier_config.memory_count,
            &topic_centroids,
            tier_config.min_memories_per_topic,
            tier_config.max_memories_per_topic,
        );

        // Generate queries
        let queries = self.generate_queries(
            tier_config.query_count,
            tier_config.relevant_docs_per_query,
            &topic_centroids,
            &topic_assignments,
            &fingerprints,
        );

        BenchmarkDataset {
            fingerprints,
            topic_assignments,
            topic_centroids,
            queries,
            config: tier_config.clone(),
            seed: self.config.seed,
        }
    }

    /// Generate fingerprints with topic assignments.
    fn generate_fingerprints(
        &mut self,
        count: usize,
        topics: &[TopicCluster],
        min_per_topic: usize,
        max_per_topic: usize,
    ) -> (Vec<(Uuid, SemanticFingerprint)>, HashMap<Uuid, usize>) {
        let mut fingerprints = Vec::with_capacity(count);
        let mut topic_assignments = HashMap::with_capacity(count);

        // Distribute documents across topics
        let mut remaining = count;
        let mut topic_counts = vec![0usize; topics.len()];

        // First ensure minimum per topic
        for i in 0..topics.len() {
            topic_counts[i] = min_per_topic.min(remaining);
            remaining = remaining.saturating_sub(min_per_topic);
        }

        // Distribute remaining documents
        while remaining > 0 {
            let topic_idx = self.rng.gen_range(0..topics.len());
            if topic_counts[topic_idx] < max_per_topic {
                topic_counts[topic_idx] += 1;
                remaining -= 1;
            }
        }

        // Generate documents for each topic
        for (topic_idx, &topic_count) in topic_counts.iter().enumerate() {
            let topic = &topics[topic_idx];

            for _ in 0..topic_count {
                let id = self.generate_deterministic_uuid();
                let fp = self.generate_fingerprint(topic);
                fingerprints.push((id, fp));
                topic_assignments.insert(id, topic_idx);
            }
        }

        // Shuffle to avoid topic ordering bias
        fingerprints.shuffle(&mut self.rng);

        (fingerprints, topic_assignments)
    }

    /// Generate a single fingerprint within a topic cluster.
    fn generate_fingerprint(&mut self, topic: &TopicCluster) -> SemanticFingerprint {
        self.generate_fingerprint_with_noise(topic, self.config.noise_std)
    }

    /// Generate a single fingerprint within a topic cluster with custom noise level.
    fn generate_fingerprint_with_noise(&mut self, topic: &TopicCluster, noise_std: f32) -> SemanticFingerprint {
        let embeddings = topic.sample_all(&mut self.rng, noise_std);

        SemanticFingerprint {
            e1_semantic: embeddings.e1,
            e2_temporal_recent: embeddings.e2,
            e3_temporal_periodic: embeddings.e3,
            e4_temporal_positional: embeddings.e4,
            e5_causal_as_cause: embeddings.e5.clone(),
            e5_causal_as_effect: embeddings.e5,
            e5_causal: Vec::new(), // Empty - using new dual format
            e6_sparse: self.generate_sparse(E6_SPARSE_VOCAB, self.config.sparse_entries_e6),
            e7_code: embeddings.e7,
            e8_graph_as_source: embeddings.e8.clone(),
            e8_graph_as_target: embeddings.e8,
            e8_graph: Vec::new(), // Empty - using new dual format
            e9_hdc: embeddings.e9,
            e10_multimodal: embeddings.e10,
            e11_entity: embeddings.e11,
            e12_late_interaction: self.generate_late_interaction(self.config.tokens_e12),
            e13_splade: self.generate_sparse(E13_SPLADE_VOCAB, self.config.sparse_entries_e13),
        }
    }

    /// Generate sparse embedding.
    fn generate_sparse(&mut self, vocab_size: usize, n_entries: usize) -> SparseVector {
        let mut indices: Vec<u16> = (0..vocab_size as u16)
            .collect::<Vec<_>>()
            .choose_multiple(&mut self.rng, n_entries)
            .copied()
            .collect();
        indices.sort_unstable();

        let normal = Normal::new(0.0, 1.0).unwrap();
        let values: Vec<f32> = (0..n_entries)
            .map(|_| (normal.sample(&mut self.rng) as f32).abs())
            .collect();

        SparseVector::new(indices, values).expect("Invalid sparse vector generation")
    }

    /// Generate late interaction embeddings.
    fn generate_late_interaction(&mut self, n_tokens: usize) -> Vec<Vec<f32>> {
        let normal = Normal::new(0.0, 0.5).unwrap();

        (0..n_tokens)
            .map(|_| {
                let mut token: Vec<f32> = (0..E12_TOKEN_DIM)
                    .map(|_| normal.sample(&mut self.rng) as f32)
                    .collect();

                // Normalize
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

    /// Generate queries with ground truth.
    fn generate_queries(
        &mut self,
        count: usize,
        relevant_per_query: usize,
        topics: &[TopicCluster],
        topic_assignments: &HashMap<Uuid, usize>,
        _fingerprints: &[(Uuid, SemanticFingerprint)],
    ) -> Vec<QueryData> {
        let n_divergent = (count as f32 * self.config.divergent_query_fraction) as usize;
        let n_normal = count - n_divergent;

        let mut queries = Vec::with_capacity(count);

        // Generate normal queries (within existing topics)
        for _ in 0..n_normal {
            let topic_idx = self.rng.gen_range(0..topics.len());
            let topic = &topics[topic_idx];

            // Generate full query fingerprint (sample from topic with reduced noise for queries)
            let query_fingerprint = self.generate_fingerprint_with_noise(topic, self.config.noise_std * 0.5);
            let embedding = query_fingerprint.e1_semantic.clone();

            // Find relevant documents (same topic)
            // Sort for deterministic order (HashMap iteration is non-deterministic)
            let mut topic_docs: Vec<Uuid> = topic_assignments
                .iter()
                .filter(|(_, &t)| t == topic_idx)
                .map(|(id, _)| *id)
                .collect();
            topic_docs.sort();

            let relevant_docs: HashSet<Uuid> = topic_docs
                .choose_multiple(&mut self.rng, relevant_per_query.min(topic_docs.len()))
                .copied()
                .collect();

            queries.push(QueryData {
                id: self.generate_deterministic_uuid(),
                embedding,
                fingerprint: query_fingerprint,
                topic: topic_idx,
                relevant_docs,
                is_divergent: false,
            });
        }

        // Generate divergent queries (new topics)
        let mut topic_generator = TopicGenerator::new(self.config.seed + 1000);
        for _ in 0..n_divergent {
            // Generate a divergent topic cluster for this query
            let divergent_topic = topic_generator.generate_divergent_topic(topics);

            // Generate full fingerprint from divergent topic
            let query_fingerprint = self.generate_fingerprint_with_noise(&divergent_topic, self.config.noise_std * 0.5);
            let embedding = query_fingerprint.e1_semantic.clone();

            queries.push(QueryData {
                id: self.generate_deterministic_uuid(),
                embedding,
                fingerprint: query_fingerprint,
                topic: topics.len(), // Indicates divergent
                relevant_docs: HashSet::new(), // No relevant docs for divergent queries
                is_divergent: true,
            });
        }

        // Shuffle queries
        queries.shuffle(&mut self.rng);
        queries
    }
}

/// Generate a random dense embedding.
#[allow(dead_code)]
fn random_dense_embedding(dim: usize, rng: &mut ChaCha8Rng) -> Vec<f32> {
    let normal = Normal::new(0.0, 0.5).unwrap();
    let mut embedding: Vec<f32> = (0..dim).map(|_| normal.sample(rng) as f32).collect();

    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in &mut embedding {
            *x /= norm;
        }
    }

    embedding
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Tier, TierConfig};

    #[test]
    fn test_generate_tier0_dataset() {
        let mut generator = DatasetGenerator::new(42);
        let config = TierConfig::for_tier(Tier::Tier0);
        let dataset = generator.generate_dataset(&config);

        assert_eq!(dataset.document_count(), config.memory_count);
        assert_eq!(dataset.topic_count(), config.topic_count);
        assert_eq!(dataset.query_count(), config.query_count);

        // Validate dataset
        dataset.validate().unwrap();
    }

    #[test]
    fn test_fingerprint_dimensions() {
        let mut generator = DatasetGenerator::new(42);
        let config = TierConfig::for_tier(Tier::Tier0);
        let dataset = generator.generate_dataset(&config);

        for (_, fp) in &dataset.fingerprints {
            assert_eq!(fp.e1_semantic.len(), E1_DIM);
            assert_eq!(fp.e2_temporal_recent.len(), E2_DIM);
            assert_eq!(fp.e3_temporal_periodic.len(), E3_DIM);
            assert_eq!(fp.e4_temporal_positional.len(), E4_DIM);
            // E5 uses dual vectors (cause + effect) for asymmetric causal similarity
            assert_eq!(fp.e5_causal_as_cause.len(), E5_DIM);
            assert_eq!(fp.e5_causal_as_effect.len(), E5_DIM);
            assert!(fp.e5_causal.is_empty(), "Legacy e5_causal should be empty");
            assert_eq!(fp.e7_code.len(), E7_DIM);
            assert_eq!(fp.e8_graph.len(), E8_DIM);
            assert_eq!(fp.e9_hdc.len(), E9_DIM);
            assert_eq!(fp.e10_multimodal.len(), E10_DIM);
            assert_eq!(fp.e11_entity.len(), E11_DIM);
        }
    }

    #[test]
    fn test_topic_assignment_coverage() {
        let mut generator = DatasetGenerator::new(42);
        let config = TierConfig::for_tier(Tier::Tier0);
        let dataset = generator.generate_dataset(&config);

        // All documents should have topic assignments
        for (id, _) in &dataset.fingerprints {
            assert!(dataset.topic_assignments.contains_key(id));
        }

        // All topics should have some documents
        for topic_idx in 0..config.topic_count {
            let count = dataset.documents_for_topic(topic_idx).len();
            assert!(count >= config.min_memories_per_topic);
        }
    }

    #[test]
    fn test_divergent_queries() {
        let config = GeneratorConfig {
            seed: 42,
            divergent_query_fraction: 0.2,
            ..Default::default()
        };
        let mut generator = DatasetGenerator::with_config(config);
        let tier_config = TierConfig::for_tier(Tier::Tier0);
        let dataset = generator.generate_dataset(&tier_config);

        let divergent_count = dataset.queries.iter().filter(|q| q.is_divergent).count();
        let expected = (tier_config.query_count as f32 * 0.2) as usize;

        // Allow some tolerance
        assert!((divergent_count as i32 - expected as i32).abs() <= 2);
    }
}
