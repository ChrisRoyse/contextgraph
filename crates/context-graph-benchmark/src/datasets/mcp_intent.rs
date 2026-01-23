//! MCP Intent Tool Integration benchmark dataset generation.
//!
//! Generates datasets for benchmarking E10's role as an E1 ENHANCER
//! in MCP tools. The 13 embedders create FINGERPRINTS for memories,
//! with E1 as the semantic foundation and E10 adding intent/context understanding.
//!
//! ## Philosophy: E1-Foundation + E10-Enhancement
//!
//! - E1 is THE semantic foundation - all retrieval starts with E1
//! - E10 ENHANCES E1 - adds intent/context dimension, doesn't replace
//! - Blended retrieval - default 70% E1, 30% E10 (configurable)
//! - Asymmetric vectors - separate intent/context encodings with direction modifiers
//!
//! ## Dataset Types
//!
//! - **IntentMemory**: Memories with pre-computed E1/E10 embeddings
//! - **IntentToolQuery**: Queries for search_by_intent tool
//! - **ContextToolQuery**: Queries for find_contextual_matches tool
//! - **AsymmetricPair**: Pairs for validating direction modifiers
//! - **E1StrengthQuery**: Queries categorized by E1 match strength

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::multimodal::{IntentDirection, IntentDomain};

// ============================================================================
// CONFIGURATION
// ============================================================================

/// Configuration for MCP Intent benchmark dataset generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPIntentDatasetConfig {
    /// Number of memories to generate.
    pub num_memories: usize,

    /// Number of intent tool queries.
    pub num_intent_queries: usize,

    /// Number of context tool queries.
    pub num_context_queries: usize,

    /// Number of asymmetric validation pairs.
    pub num_asymmetric_pairs: usize,

    /// Number of E1-strong queries (E1 similarity > 0.8).
    pub num_e1_strong_queries: usize,

    /// Number of E1-weak queries (E1 similarity < 0.3).
    pub num_e1_weak_queries: usize,

    /// Random seed for reproducibility.
    pub seed: u64,

    /// Domains to include.
    pub domains: Vec<IntentDomain>,

    /// Embedding dimension for E1.
    pub e1_dim: usize,

    /// Embedding dimension for E10.
    pub e10_dim: usize,
}

impl Default for MCPIntentDatasetConfig {
    fn default() -> Self {
        Self {
            num_memories: 1000,
            num_intent_queries: 100,
            num_context_queries: 100,
            num_asymmetric_pairs: 50,
            num_e1_strong_queries: 30,
            num_e1_weak_queries: 30,
            seed: 42,
            domains: IntentDomain::all(),
            e1_dim: 1024,
            e10_dim: 768,
        }
    }
}

// ============================================================================
// DATASET TYPES
// ============================================================================

/// A memory with pre-computed embeddings for benchmarking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentMemory {
    /// Unique identifier.
    pub id: Uuid,

    /// Memory content text.
    pub content: String,

    /// Primary intent domain.
    pub domain: IntentDomain,

    /// Direction (intent or context).
    pub direction: IntentDirection,

    /// Pre-computed E1 semantic embedding.
    pub e1_embedding: Vec<f32>,

    /// Pre-computed E10 intent embedding.
    pub e10_intent_embedding: Vec<f32>,

    /// Pre-computed E10 context embedding.
    pub e10_context_embedding: Vec<f32>,

    /// Ground truth relevance score for queries.
    pub relevance_score: f32,
}

/// Query for search_by_intent tool benchmarking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentToolQuery {
    /// Query text.
    pub query: String,

    /// Query E1 embedding.
    pub e1_embedding: Vec<f32>,

    /// Query E10 intent embedding.
    pub e10_intent_embedding: Vec<f32>,

    /// Expected domain.
    pub expected_domain: IntentDomain,

    /// Ground truth relevant memory IDs (ordered by relevance).
    pub ground_truth_ids: Vec<Uuid>,

    /// Ground truth relevance scores.
    pub ground_truth_scores: Vec<f32>,

    /// Blend weight used for this query (for sweep analysis).
    pub blend_weight: f32,
}

/// Query for find_contextual_matches tool benchmarking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextToolQuery {
    /// Context text.
    pub context: String,

    /// Context E1 embedding.
    pub e1_embedding: Vec<f32>,

    /// Context E10 context embedding.
    pub e10_context_embedding: Vec<f32>,

    /// Expected domain.
    pub expected_domain: IntentDomain,

    /// Ground truth relevant memory IDs.
    pub ground_truth_ids: Vec<Uuid>,

    /// Ground truth relevance scores.
    pub ground_truth_scores: Vec<f32>,

    /// Blend weight used for this query.
    pub blend_weight: f32,
}

/// Pair for asymmetric retrieval validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsymmetricPair {
    /// Intent text.
    pub intent_text: String,

    /// Context text.
    pub context_text: String,

    /// Intent E10 embedding (from intent text).
    pub intent_e10_embedding: Vec<f32>,

    /// Context E10 embedding (from context text).
    pub context_e10_embedding: Vec<f32>,

    /// Base cosine similarity (without direction modifier).
    pub base_similarity: f32,

    /// Expected intent→context similarity (base × 1.2).
    pub expected_intent_to_context: f32,

    /// Expected context→intent similarity (base × 0.8).
    pub expected_context_to_intent: f32,

    /// Domain for context.
    pub domain: IntentDomain,
}

/// Query categorized by E1 match strength (for ARCH-17 validation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E1StrengthQuery {
    /// Query text.
    pub query: String,

    /// Query E1 embedding.
    pub e1_embedding: Vec<f32>,

    /// Query E10 intent embedding.
    pub e10_intent_embedding: Vec<f32>,

    /// E1 similarity strength category.
    pub strength: E1Strength,

    /// Ground truth E1-only MRR for this query.
    pub e1_only_mrr: f32,

    /// Ground truth E1+E10 blended MRR for this query.
    pub blended_mrr: f32,

    /// Expected behavior: refine (E1 strong) or broaden (E1 weak).
    pub expected_behavior: E10Behavior,

    /// Expected domain.
    pub expected_domain: IntentDomain,

    /// Ground truth relevant memory IDs.
    pub ground_truth_ids: Vec<Uuid>,
}

/// E1 match strength category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum E1Strength {
    /// E1 similarity > 0.8 (strong semantic match).
    Strong,
    /// E1 similarity < 0.3 (weak semantic match).
    Weak,
    /// E1 similarity in [0.3, 0.8] (medium).
    Medium,
}

/// Expected E10 behavior based on E1 strength (ARCH-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum E10Behavior {
    /// E10 should refine E1's strong results.
    Refine,
    /// E10 should broaden E1's weak results.
    Broaden,
}

// ============================================================================
// DATASET
// ============================================================================

/// Complete MCP Intent benchmark dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPIntentBenchmarkDataset {
    /// All memories in the corpus.
    pub memories: Vec<IntentMemory>,

    /// Queries for search_by_intent tool.
    pub intent_queries: Vec<IntentToolQuery>,

    /// Queries for find_contextual_matches tool.
    pub context_queries: Vec<ContextToolQuery>,

    /// Pairs for asymmetric validation.
    pub asymmetric_pairs: Vec<AsymmetricPair>,

    /// E1-strong queries (E1 > 0.8).
    pub e1_strong_queries: Vec<E1StrengthQuery>,

    /// E1-weak queries (E1 < 0.3).
    pub e1_weak_queries: Vec<E1StrengthQuery>,

    /// Random seed used.
    pub seed: u64,
}

/// Dataset statistics for reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPIntentDatasetStats {
    /// Total memories.
    pub total_memories: usize,

    /// Intent memories count.
    pub intent_memories: usize,

    /// Context memories count.
    pub context_memories: usize,

    /// Memories per domain.
    pub memories_per_domain: std::collections::HashMap<String, usize>,

    /// Total intent queries.
    pub intent_queries: usize,

    /// Total context queries.
    pub context_queries: usize,

    /// Total asymmetric pairs.
    pub asymmetric_pairs: usize,

    /// E1-strong queries.
    pub e1_strong_queries: usize,

    /// E1-weak queries.
    pub e1_weak_queries: usize,

    /// E1 embedding dimension.
    pub e1_dim: usize,

    /// E10 embedding dimension.
    pub e10_dim: usize,
}

impl MCPIntentBenchmarkDataset {
    /// Get dataset statistics.
    pub fn stats(&self) -> MCPIntentDatasetStats {
        let mut memories_per_domain = std::collections::HashMap::new();
        let mut intent_memories = 0;
        let mut context_memories = 0;

        for mem in &self.memories {
            *memories_per_domain
                .entry(mem.domain.display_name().to_string())
                .or_insert(0) += 1;

            match mem.direction {
                IntentDirection::Intent => intent_memories += 1,
                IntentDirection::Context => context_memories += 1,
                IntentDirection::Unknown => {}
            }
        }

        let e1_dim = self.memories.first().map(|m| m.e1_embedding.len()).unwrap_or(1024);
        let e10_dim = self.memories.first().map(|m| m.e10_intent_embedding.len()).unwrap_or(768);

        MCPIntentDatasetStats {
            total_memories: self.memories.len(),
            intent_memories,
            context_memories,
            memories_per_domain,
            intent_queries: self.intent_queries.len(),
            context_queries: self.context_queries.len(),
            asymmetric_pairs: self.asymmetric_pairs.len(),
            e1_strong_queries: self.e1_strong_queries.len(),
            e1_weak_queries: self.e1_weak_queries.len(),
            e1_dim,
            e10_dim,
        }
    }
}

// ============================================================================
// GENERATOR
// ============================================================================

/// Generator for MCP Intent benchmark datasets.
pub struct MCPIntentDatasetGenerator {
    config: MCPIntentDatasetConfig,
    rng: ChaCha8Rng,
}

impl MCPIntentDatasetGenerator {
    /// Create a new generator with the given config.
    pub fn new(config: MCPIntentDatasetConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        Self { config, rng }
    }

    /// Generate the complete benchmark dataset.
    pub fn generate(&mut self) -> MCPIntentBenchmarkDataset {
        // Generate memories
        let memories = self.generate_memories();

        // Generate intent queries
        let intent_queries = self.generate_intent_queries(&memories);

        // Generate context queries
        let context_queries = self.generate_context_queries(&memories);

        // Generate asymmetric pairs
        let asymmetric_pairs = self.generate_asymmetric_pairs();

        // Generate E1-strength queries
        let e1_strong_queries = self.generate_e1_strength_queries(&memories, E1Strength::Strong);
        let e1_weak_queries = self.generate_e1_strength_queries(&memories, E1Strength::Weak);

        MCPIntentBenchmarkDataset {
            memories,
            intent_queries,
            context_queries,
            asymmetric_pairs,
            e1_strong_queries,
            e1_weak_queries,
            seed: self.config.seed,
        }
    }

    /// Generate random unit vector of given dimension.
    fn random_unit_vector(&mut self, dim: usize) -> Vec<f32> {
        let mut vec: Vec<f32> = (0..dim).map(|_| self.rng.gen::<f32>() - 0.5).collect();
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for v in &mut vec {
                *v /= norm;
            }
        }
        vec
    }

    /// Generate a vector similar to target with given similarity.
    fn vector_with_similarity(&mut self, target: &[f32], target_sim: f32) -> Vec<f32> {
        let dim = target.len();
        let noise = self.random_unit_vector(dim);

        // Linear interpolation: result = target * sim + noise * sqrt(1 - sim^2)
        let noise_weight = (1.0 - target_sim * target_sim).sqrt().max(0.0);

        let mut result: Vec<f32> = target
            .iter()
            .zip(noise.iter())
            .map(|(t, n)| t * target_sim + n * noise_weight)
            .collect();

        // Normalize
        let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for v in &mut result {
                *v /= norm;
            }
        }

        result
    }

    /// Generate memories with embeddings.
    fn generate_memories(&mut self) -> Vec<IntentMemory> {
        let mut memories = Vec::with_capacity(self.config.num_memories);
        let half = self.config.num_memories / 2;
        let num_domains = self.config.domains.len();
        let e1_dim = self.config.e1_dim;
        let e10_dim = self.config.e10_dim;

        // Generate domain centroids for E1 and E10
        let mut domain_e1_centroids: Vec<Vec<f32>> = Vec::with_capacity(num_domains);
        let mut domain_e10_intent_centroids: Vec<Vec<f32>> = Vec::with_capacity(num_domains);
        let mut domain_e10_context_centroids: Vec<Vec<f32>> = Vec::with_capacity(num_domains);

        for _ in 0..num_domains {
            domain_e1_centroids.push(self.random_unit_vector(e1_dim));
            domain_e10_intent_centroids.push(self.random_unit_vector(e10_dim));
            domain_e10_context_centroids.push(self.random_unit_vector(e10_dim));
        }

        // Generate intent memories
        for i in 0..half {
            let domain_idx = i % num_domains;
            let domain = self.config.domains[domain_idx];

            let content = self.generate_memory_content(domain, IntentDirection::Intent);
            let relevance_score = self.rng.gen_range(0.5..1.0);

            // Generate embeddings with domain clustering
            let e1_sim = self.rng.gen_range(0.6..0.9);
            let e1_embedding = self.vector_with_similarity(&domain_e1_centroids[domain_idx], e1_sim);

            let e10_intent_sim = self.rng.gen_range(0.7..0.95);
            let e10_intent_embedding = self.vector_with_similarity(&domain_e10_intent_centroids[domain_idx], e10_intent_sim);

            let e10_context_sim = self.rng.gen_range(0.3..0.6); // Lower similarity for opposite direction
            let e10_context_embedding = self.vector_with_similarity(&domain_e10_context_centroids[domain_idx], e10_context_sim);

            memories.push(IntentMemory {
                id: Uuid::new_v4(),
                content,
                domain,
                direction: IntentDirection::Intent,
                e1_embedding,
                e10_intent_embedding,
                e10_context_embedding,
                relevance_score,
            });
        }

        // Generate context memories
        for i in 0..(self.config.num_memories - half) {
            let domain_idx = i % num_domains;
            let domain = self.config.domains[domain_idx];

            let content = self.generate_memory_content(domain, IntentDirection::Context);
            let relevance_score = self.rng.gen_range(0.5..1.0);

            let e1_sim = self.rng.gen_range(0.6..0.9);
            let e1_embedding = self.vector_with_similarity(&domain_e1_centroids[domain_idx], e1_sim);

            let e10_intent_sim = self.rng.gen_range(0.3..0.6); // Lower for opposite direction
            let e10_intent_embedding = self.vector_with_similarity(&domain_e10_intent_centroids[domain_idx], e10_intent_sim);

            let e10_context_sim = self.rng.gen_range(0.7..0.95);
            let e10_context_embedding = self.vector_with_similarity(&domain_e10_context_centroids[domain_idx], e10_context_sim);

            memories.push(IntentMemory {
                id: Uuid::new_v4(),
                content,
                domain,
                direction: IntentDirection::Context,
                e1_embedding,
                e10_intent_embedding,
                e10_context_embedding,
                relevance_score,
            });
        }

        // Shuffle
        memories.shuffle(&mut self.rng);
        memories
    }

    /// Generate memory content text.
    fn generate_memory_content(&mut self, domain: IntentDomain, direction: IntentDirection) -> String {
        let templates = match direction {
            IntentDirection::Intent => domain.intent_templates(),
            IntentDirection::Context => domain.context_templates(),
            IntentDirection::Unknown => domain.intent_templates(),
        };

        let template = *templates.choose(&mut self.rng).unwrap_or(&"Working on task");
        self.fill_template(template)
    }

    /// Fill template placeholders.
    fn fill_template(&mut self, template: &str) -> String {
        let placeholders = [
            ("{component}", &["AuthService", "CacheLayer", "DatabasePool", "APIHandler"][..]),
            ("{module}", &["auth", "cache", "database", "api"]),
            ("{operation}", &["batch processing", "query execution", "request handling"]),
            ("{service}", &["user service", "order service", "payment service"]),
            ("{feature}", &["search", "filtering", "pagination"]),
            ("{bug_type}", &["null pointer", "race condition", "memory leak"]),
            ("{vulnerability}", &["SQL injection", "XSS", "CSRF"]),
        ];

        let mut result = template.to_string();
        for (placeholder, values) in &placeholders {
            if result.contains(placeholder) {
                if let Some(value) = values.choose(&mut self.rng) {
                    result = result.replacen(placeholder, value, 1);
                }
            }
        }
        result
    }

    /// Generate intent tool queries.
    fn generate_intent_queries(&mut self, memories: &[IntentMemory]) -> Vec<IntentToolQuery> {
        let mut queries = Vec::with_capacity(self.config.num_intent_queries);

        for _ in 0..self.config.num_intent_queries {
            let domain = *self.config.domains.choose(&mut self.rng).unwrap();

            // Find relevant context memories (intent query → context memories)
            let relevant: Vec<_> = memories
                .iter()
                .filter(|m| m.domain == domain && matches!(m.direction, IntentDirection::Context))
                .take(5)
                .collect();

            if relevant.is_empty() {
                continue;
            }

            // Generate query embeddings similar to relevant memories
            let ref_memory = relevant.choose(&mut self.rng).unwrap();
            let e1_embedding = self.vector_with_similarity(&ref_memory.e1_embedding, 0.75);
            let e10_intent_embedding = self.vector_with_similarity(&ref_memory.e10_context_embedding, 0.8);

            let ground_truth_ids: Vec<_> = relevant.iter().map(|m| m.id).collect();
            let ground_truth_scores: Vec<_> = relevant.iter().map(|m| m.relevance_score).collect();

            queries.push(IntentToolQuery {
                query: format!("Find work related to {} optimization", domain.display_name()),
                e1_embedding,
                e10_intent_embedding,
                expected_domain: domain,
                ground_truth_ids,
                ground_truth_scores,
                blend_weight: 0.1,  // Updated: E10 enhances E1 at 10% weight
            });
        }

        queries
    }

    /// Generate context tool queries.
    fn generate_context_queries(&mut self, memories: &[IntentMemory]) -> Vec<ContextToolQuery> {
        let mut queries = Vec::with_capacity(self.config.num_context_queries);

        for _ in 0..self.config.num_context_queries {
            let domain = *self.config.domains.choose(&mut self.rng).unwrap();

            // Find relevant intent memories (context query → intent memories)
            let relevant: Vec<_> = memories
                .iter()
                .filter(|m| m.domain == domain && matches!(m.direction, IntentDirection::Intent))
                .take(5)
                .collect();

            if relevant.is_empty() {
                continue;
            }

            let ref_memory = relevant.choose(&mut self.rng).unwrap();
            let e1_embedding = self.vector_with_similarity(&ref_memory.e1_embedding, 0.75);
            let e10_context_embedding = self.vector_with_similarity(&ref_memory.e10_intent_embedding, 0.8);

            let ground_truth_ids: Vec<_> = relevant.iter().map(|m| m.id).collect();
            let ground_truth_scores: Vec<_> = relevant.iter().map(|m| m.relevance_score).collect();

            queries.push(ContextToolQuery {
                context: format!("Working on {} issues, need solutions", domain.display_name()),
                e1_embedding,
                e10_context_embedding,
                expected_domain: domain,
                ground_truth_ids,
                ground_truth_scores,
                blend_weight: 0.1,  // Updated: E10 enhances E1 at 10% weight
            });
        }

        queries
    }

    /// Generate asymmetric pairs for direction modifier validation.
    fn generate_asymmetric_pairs(&mut self) -> Vec<AsymmetricPair> {
        let mut pairs = Vec::with_capacity(self.config.num_asymmetric_pairs);

        for _ in 0..self.config.num_asymmetric_pairs {
            let domain = *self.config.domains.choose(&mut self.rng).unwrap();

            // Generate base embeddings
            let base = self.random_unit_vector(self.config.e10_dim);
            let intent_e10 = self.vector_with_similarity(&base, 0.9);
            let context_e10 = self.vector_with_similarity(&base, 0.9);

            // Compute base similarity
            let base_similarity = cosine_similarity(&intent_e10, &context_e10);

            pairs.push(AsymmetricPair {
                intent_text: format!("Optimize {} for better performance", domain.display_name()),
                context_text: format!("{} was slow under load", domain.display_name()),
                intent_e10_embedding: intent_e10,
                context_e10_embedding: context_e10,
                base_similarity,
                // E5-base-v2 handles asymmetry via prefixes - no artificial modifiers
                expected_intent_to_context: base_similarity * 1.0,
                expected_context_to_intent: base_similarity * 1.0,
                domain,
            });
        }

        pairs
    }

    /// Generate E1-strength categorized queries.
    fn generate_e1_strength_queries(
        &mut self,
        memories: &[IntentMemory],
        strength: E1Strength,
    ) -> Vec<E1StrengthQuery> {
        let count = match strength {
            E1Strength::Strong => self.config.num_e1_strong_queries,
            E1Strength::Weak => self.config.num_e1_weak_queries,
            E1Strength::Medium => 0,
        };

        let mut queries = Vec::with_capacity(count);

        for _ in 0..count {
            let domain = *self.config.domains.choose(&mut self.rng).unwrap();

            let relevant: Vec<_> = memories
                .iter()
                .filter(|m| m.domain == domain)
                .take(5)
                .collect();

            if relevant.is_empty() {
                continue;
            }

            let ref_memory = relevant.choose(&mut self.rng).unwrap();

            // Generate query with appropriate E1 similarity
            let (target_sim, behavior) = match strength {
                E1Strength::Strong => (0.85, E10Behavior::Refine),
                E1Strength::Weak => (0.25, E10Behavior::Broaden),
                E1Strength::Medium => (0.55, E10Behavior::Refine),
            };

            let e1_embedding = self.vector_with_similarity(&ref_memory.e1_embedding, target_sim);
            let e10_intent_embedding = self.vector_with_similarity(&ref_memory.e10_intent_embedding, 0.7);

            // Simulate MRR values
            let e1_only_mrr = match strength {
                E1Strength::Strong => self.rng.gen_range(0.6..0.9),
                E1Strength::Weak => self.rng.gen_range(0.1..0.3),
                E1Strength::Medium => self.rng.gen_range(0.3..0.6),
            };

            let blended_mrr = match strength {
                E1Strength::Strong => e1_only_mrr * self.rng.gen_range(1.0..1.1), // Refine
                E1Strength::Weak => e1_only_mrr + self.rng.gen_range(0.1..0.3),   // Broaden
                E1Strength::Medium => e1_only_mrr * self.rng.gen_range(1.0..1.15),
            };

            let ground_truth_ids: Vec<_> = relevant.iter().map(|m| m.id).collect();

            queries.push(E1StrengthQuery {
                query: format!("{} related work", domain.display_name()),
                e1_embedding,
                e10_intent_embedding,
                strength,
                e1_only_mrr,
                blended_mrr: blended_mrr.min(1.0),
                expected_behavior: behavior,
                expected_domain: domain,
                ground_truth_ids,
            });
        }

        queries
    }
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_generation() {
        let config = MCPIntentDatasetConfig {
            num_memories: 100,
            num_intent_queries: 20,
            num_context_queries: 20,
            num_asymmetric_pairs: 10,
            num_e1_strong_queries: 10,
            num_e1_weak_queries: 10,
            seed: 42,
            ..Default::default()
        };

        let mut generator = MCPIntentDatasetGenerator::new(config);
        let dataset = generator.generate();

        assert_eq!(dataset.memories.len(), 100);
        assert!(!dataset.intent_queries.is_empty());
        assert!(!dataset.context_queries.is_empty());
        assert_eq!(dataset.asymmetric_pairs.len(), 10);

        println!(
            "[VERIFIED] Dataset: {} memories, {} intent queries, {} context queries",
            dataset.memories.len(),
            dataset.intent_queries.len(),
            dataset.context_queries.len()
        );
    }

    #[test]
    fn test_embedding_dimensions() {
        let config = MCPIntentDatasetConfig {
            num_memories: 10,
            e1_dim: 1024,
            e10_dim: 768,
            ..Default::default()
        };

        let mut generator = MCPIntentDatasetGenerator::new(config);
        let dataset = generator.generate();

        for mem in &dataset.memories {
            assert_eq!(mem.e1_embedding.len(), 1024);
            assert_eq!(mem.e10_intent_embedding.len(), 768);
            assert_eq!(mem.e10_context_embedding.len(), 768);
        }

        println!("[VERIFIED] Embedding dimensions: E1=1024, E10=768");
    }

    #[test]
    fn test_asymmetric_pairs_have_correct_modifiers() {
        let config = MCPIntentDatasetConfig {
            num_asymmetric_pairs: 5,
            ..Default::default()
        };

        let mut generator = MCPIntentDatasetGenerator::new(config);
        let dataset = generator.generate();

        for pair in &dataset.asymmetric_pairs {
            let expected_ratio = pair.expected_intent_to_context / pair.expected_context_to_intent;
            assert!(
                (expected_ratio - 1.0).abs() < 0.01,
                "Asymmetric ratio should be 1.0 (neutral - E5-base-v2 handles via prefixes), got {}",
                expected_ratio
            );
        }

        println!("[VERIFIED] Asymmetric pairs have neutral 1.0/1.0 modifier ratio (E5-base-v2 handles asymmetry via prefixes)");
    }

    #[test]
    fn test_e1_strength_queries() {
        let config = MCPIntentDatasetConfig {
            num_memories: 100,
            num_e1_strong_queries: 10,
            num_e1_weak_queries: 10,
            ..Default::default()
        };

        let mut generator = MCPIntentDatasetGenerator::new(config);
        let dataset = generator.generate();

        for q in &dataset.e1_strong_queries {
            assert_eq!(q.strength, E1Strength::Strong);
            assert_eq!(q.expected_behavior, E10Behavior::Refine);
        }

        for q in &dataset.e1_weak_queries {
            assert_eq!(q.strength, E1Strength::Weak);
            assert_eq!(q.expected_behavior, E10Behavior::Broaden);
        }

        println!("[VERIFIED] E1 strength queries correctly categorized");
    }

    #[test]
    fn test_dataset_stats() {
        let config = MCPIntentDatasetConfig {
            num_memories: 50,
            ..Default::default()
        };

        let mut generator = MCPIntentDatasetGenerator::new(config);
        let dataset = generator.generate();
        let stats = dataset.stats();

        assert_eq!(stats.total_memories, 50);
        assert!(stats.intent_memories + stats.context_memories <= 50);
        assert_eq!(stats.e1_dim, 1024);
        assert_eq!(stats.e10_dim, 768);

        println!("[VERIFIED] Stats: {} memories, {} intent, {} context",
            stats.total_memories, stats.intent_memories, stats.context_memories);
    }
}
