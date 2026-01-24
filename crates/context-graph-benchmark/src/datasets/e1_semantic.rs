//! E1 Semantic Embedder Benchmark Dataset.
//!
//! This module generates synthetic and real data datasets for benchmarking
//! the E1 semantic embedder (intfloat/e5-large-v2, 1024D).
//!
//! E1 is THE semantic foundation per ARCH-12. This benchmark validates:
//! - Retrieval quality (MRR, P@K, R@K, NDCG)
//! - Topic separation (intra vs inter topic similarity)
//! - Noise robustness (degradation under perturbation)
//! - Foundation role (E1 as baseline for other embedders)
//!
//! ## Query Types
//!
//! 1. Same-topic retrieval (exact match expected)
//! 2. Related-topic retrieval (partial match expected)
//! 3. Off-topic distractor filtering (no match expected)
//! 4. Cross-domain queries (test generalization)

use std::collections::{HashMap, HashSet};

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for E1 semantic benchmark dataset generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E1SemanticDatasetConfig {
    /// Number of documents to generate.
    pub num_documents: usize,
    /// Number of queries to generate.
    pub num_queries: usize,
    /// Semantic domains to include.
    pub semantic_domains: Vec<SemanticDomain>,
    /// Noise levels for robustness testing (0.0 to 1.0).
    pub noise_levels: Vec<f64>,
    /// Fraction of queries that should be cross-domain.
    pub cross_domain_ratio: f64,
    /// Fraction of queries that should be off-topic distractors.
    pub distractor_ratio: f64,
    /// Minimum documents per topic.
    pub min_docs_per_topic: usize,
    /// Maximum documents per topic.
    pub max_docs_per_topic: usize,
    /// Random seed for reproducibility.
    pub seed: u64,
}

impl Default for E1SemanticDatasetConfig {
    fn default() -> Self {
        Self {
            num_documents: 5000,
            num_queries: 500,
            semantic_domains: vec![
                SemanticDomain::Code,
                SemanticDomain::DataScience,
                SemanticDomain::Infrastructure,
                SemanticDomain::Documentation,
                SemanticDomain::General,
            ],
            noise_levels: vec![0.0, 0.1, 0.2, 0.3],
            cross_domain_ratio: 0.1,
            distractor_ratio: 0.05,
            min_docs_per_topic: 5,
            max_docs_per_topic: 50,
            seed: 42,
        }
    }
}

// ============================================================================
// Semantic Domains
// ============================================================================

/// Semantic domains for categorizing content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticDomain {
    /// Programming and code-related content.
    Code,
    /// Data science, ML, statistics.
    DataScience,
    /// Infrastructure, DevOps, cloud.
    Infrastructure,
    /// Documentation, READMEs, guides.
    Documentation,
    /// General purpose content.
    General,
}

impl SemanticDomain {
    /// Get all domains.
    pub fn all() -> Vec<Self> {
        vec![
            Self::Code,
            Self::DataScience,
            Self::Infrastructure,
            Self::Documentation,
            Self::General,
        ]
    }

    /// Get domain name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Code => "Code",
            Self::DataScience => "DataScience",
            Self::Infrastructure => "Infrastructure",
            Self::Documentation => "Documentation",
            Self::General => "General",
        }
    }

    /// Get sample topics for this domain.
    pub fn sample_topics(&self) -> Vec<&'static str> {
        match self {
            Self::Code => vec![
                "rust programming",
                "async await patterns",
                "error handling",
                "type system design",
                "memory management",
                "trait implementations",
                "macro metaprogramming",
                "testing strategies",
                "performance optimization",
                "API design",
            ],
            Self::DataScience => vec![
                "neural networks",
                "embeddings and vectors",
                "transformer architecture",
                "statistical analysis",
                "data preprocessing",
                "model evaluation",
                "feature engineering",
                "clustering algorithms",
                "dimensionality reduction",
                "time series analysis",
            ],
            Self::Infrastructure => vec![
                "kubernetes deployment",
                "docker containers",
                "CI/CD pipelines",
                "monitoring and logging",
                "database management",
                "load balancing",
                "service mesh",
                "infrastructure as code",
                "security hardening",
                "disaster recovery",
            ],
            Self::Documentation => vec![
                "API documentation",
                "user guides",
                "installation instructions",
                "troubleshooting guides",
                "architecture diagrams",
                "changelog formats",
                "contribution guidelines",
                "code comments",
                "README templates",
                "tutorial writing",
            ],
            Self::General => vec![
                "project planning",
                "team collaboration",
                "code review process",
                "technical debt",
                "agile methodology",
                "communication patterns",
                "knowledge sharing",
                "decision making",
                "problem solving",
                "learning resources",
            ],
        }
    }
}

// ============================================================================
// Query Types
// ============================================================================

/// Type of semantic query for benchmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticQueryType {
    /// Same-topic retrieval (exact match expected).
    SameTopic,
    /// Related-topic retrieval (partial match expected).
    RelatedTopic,
    /// Off-topic distractor (no match expected).
    OffTopic,
    /// Cross-domain query (test generalization).
    CrossDomain,
}

impl SemanticQueryType {
    /// Expected match quality for this query type.
    pub fn expected_precision(&self) -> f64 {
        match self {
            Self::SameTopic => 0.8,     // High precision expected
            Self::RelatedTopic => 0.5,  // Moderate precision
            Self::OffTopic => 0.0,      // No matches expected
            Self::CrossDomain => 0.3,   // Lower precision across domains
        }
    }
}

// ============================================================================
// Dataset Structures
// ============================================================================

/// A document in the E1 semantic benchmark dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticDocument {
    /// Unique document ID.
    pub id: Uuid,
    /// Document text content.
    pub text: String,
    /// Domain this document belongs to.
    pub domain: SemanticDomain,
    /// Topic within the domain.
    pub topic: String,
    /// Topic ID for clustering.
    pub topic_id: usize,
    /// Source dataset (for real data).
    pub source_dataset: Option<String>,
}

/// A query in the E1 semantic benchmark dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticQuery {
    /// Unique query ID.
    pub id: Uuid,
    /// Query text.
    pub text: String,
    /// Domain this query targets.
    pub target_domain: SemanticDomain,
    /// Topic this query targets.
    pub target_topic: String,
    /// Topic ID.
    pub target_topic_id: usize,
    /// Query type.
    pub query_type: SemanticQueryType,
    /// IDs of relevant documents (ground truth).
    pub relevant_docs: HashSet<Uuid>,
    /// Partial relevance scores (1.0 = fully relevant, 0.5 = partially).
    pub relevance_scores: HashMap<Uuid, f64>,
    /// Noise level applied to query (0.0 = no noise).
    pub noise_level: f64,
}

/// Ground truth for E1 semantic benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticGroundTruth {
    /// Topic assignments: doc_id -> topic_id.
    pub topic_assignments: HashMap<Uuid, usize>,
    /// Domain assignments: doc_id -> domain.
    pub domain_assignments: HashMap<Uuid, SemanticDomain>,
    /// Topic to domain mapping.
    pub topic_domains: HashMap<usize, SemanticDomain>,
    /// Topic names.
    pub topic_names: HashMap<usize, String>,
    /// Related topics (for RelatedTopic queries).
    pub related_topics: HashMap<usize, HashSet<usize>>,
}

/// Complete E1 semantic benchmark dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E1SemanticBenchmarkDataset {
    /// Documents in the dataset.
    pub documents: Vec<SemanticDocument>,
    /// Queries for evaluation.
    pub queries: Vec<SemanticQuery>,
    /// Ground truth information.
    pub ground_truth: SemanticGroundTruth,
    /// Configuration used to generate this dataset.
    pub config: E1SemanticDatasetConfig,
    /// Statistics about the dataset.
    pub stats: E1SemanticDatasetStats,
}

/// Statistics about the E1 semantic dataset.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E1SemanticDatasetStats {
    /// Total number of documents.
    pub num_documents: usize,
    /// Total number of queries.
    pub num_queries: usize,
    /// Number of topics.
    pub num_topics: usize,
    /// Number of domains.
    pub num_domains: usize,
    /// Queries by type.
    pub queries_by_type: HashMap<String, usize>,
    /// Documents per domain.
    pub docs_per_domain: HashMap<String, usize>,
    /// Average documents per topic.
    pub avg_docs_per_topic: f64,
}

impl E1SemanticBenchmarkDataset {
    /// Get document by ID.
    pub fn get_document(&self, id: &Uuid) -> Option<&SemanticDocument> {
        self.documents.iter().find(|d| &d.id == id)
    }

    /// Get documents for a topic.
    pub fn documents_for_topic(&self, topic_id: usize) -> Vec<&SemanticDocument> {
        self.documents
            .iter()
            .filter(|d| d.topic_id == topic_id)
            .collect()
    }

    /// Get documents for a domain.
    pub fn documents_for_domain(&self, domain: SemanticDomain) -> Vec<&SemanticDocument> {
        self.documents.iter().filter(|d| d.domain == domain).collect()
    }

    /// Get queries by type.
    pub fn queries_by_type(&self, query_type: SemanticQueryType) -> Vec<&SemanticQuery> {
        self.queries
            .iter()
            .filter(|q| q.query_type == query_type)
            .collect()
    }

    /// Validate dataset consistency.
    pub fn validate(&self) -> Result<(), String> {
        // Check all documents have topic assignments
        for doc in &self.documents {
            if !self.ground_truth.topic_assignments.contains_key(&doc.id) {
                return Err(format!("Missing topic assignment for doc {}", doc.id));
            }
        }

        // Check queries reference valid documents
        for query in &self.queries {
            for doc_id in &query.relevant_docs {
                if !self.documents.iter().any(|d| &d.id == doc_id) {
                    return Err(format!(
                        "Query {} references unknown doc {}",
                        query.id, doc_id
                    ));
                }
            }
        }

        Ok(())
    }
}

// ============================================================================
// Dataset Generator
// ============================================================================

/// Generator for E1 semantic benchmark datasets.
pub struct E1SemanticDatasetGenerator {
    config: E1SemanticDatasetConfig,
    rng: ChaCha8Rng,
}

impl E1SemanticDatasetGenerator {
    /// Create a new generator with config.
    pub fn new(config: E1SemanticDatasetConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        Self { config, rng }
    }

    /// Generate a synthetic dataset.
    pub fn generate(&mut self) -> E1SemanticBenchmarkDataset {
        let mut documents = Vec::new();
        let mut topic_assignments = HashMap::new();
        let mut domain_assignments = HashMap::new();
        let mut topic_domains = HashMap::new();
        let mut topic_names = HashMap::new();
        let mut related_topics: HashMap<usize, HashSet<usize>> = HashMap::new();

        let mut topic_id = 0;
        let docs_per_domain = self.config.num_documents / self.config.semantic_domains.len();

        // Clone semantic_domains to avoid borrow conflict with self.rng
        let semantic_domains = self.config.semantic_domains.clone();
        // Generate documents by domain
        for domain in &semantic_domains {
            let topics = domain.sample_topics();
            let docs_per_topic = docs_per_domain / topics.len().max(1);

            for topic in topics {
                topic_domains.insert(topic_id, *domain);
                topic_names.insert(topic_id, topic.to_string());

                // Generate documents for this topic
                let num_docs = self.rng.gen_range(
                    self.config.min_docs_per_topic..=self.config.max_docs_per_topic.min(docs_per_topic.max(5)),
                );

                for _ in 0..num_docs {
                    let doc = self.generate_document(*domain, topic, topic_id);
                    topic_assignments.insert(doc.id, topic_id);
                    domain_assignments.insert(doc.id, *domain);
                    documents.push(doc);
                }

                topic_id += 1;
            }
        }

        // Build related topics (topics in same domain are related)
        for (tid, domain) in &topic_domains {
            let related: HashSet<usize> = topic_domains
                .iter()
                .filter(|(other_tid, other_domain)| *other_tid != tid && *other_domain == domain)
                .map(|(other_tid, _)| *other_tid)
                .collect();
            related_topics.insert(*tid, related);
        }

        let ground_truth = SemanticGroundTruth {
            topic_assignments,
            domain_assignments,
            topic_domains,
            topic_names,
            related_topics,
        };

        // Generate queries
        let queries = self.generate_queries(&documents, &ground_truth);

        // Compute statistics
        let stats = self.compute_stats(&documents, &queries);

        E1SemanticBenchmarkDataset {
            documents,
            queries,
            ground_truth,
            config: self.config.clone(),
            stats,
        }
    }

    /// Generate a single document.
    fn generate_document(
        &mut self,
        domain: SemanticDomain,
        topic: &str,
        topic_id: usize,
    ) -> SemanticDocument {
        let text = self.generate_document_text(domain, topic);
        SemanticDocument {
            id: Uuid::new_v4(),
            text,
            domain,
            topic: topic.to_string(),
            topic_id,
            source_dataset: None,
        }
    }

    /// Generate document text for a domain/topic.
    fn generate_document_text(&mut self, domain: SemanticDomain, topic: &str) -> String {
        // Generate realistic-looking text for the domain
        let templates = match domain {
            SemanticDomain::Code => vec![
                format!("Implementation of {} in Rust with proper error handling.", topic),
                format!("The {} module provides essential functionality for the system.", topic),
                format!("This function demonstrates best practices for {} in modern Rust.", topic),
                format!("Key considerations when implementing {} include safety and performance.", topic),
                format!("The {} pattern is widely used in systems programming.", topic),
            ],
            SemanticDomain::DataScience => vec![
                format!("Understanding {} is crucial for machine learning applications.", topic),
                format!("The {} algorithm provides efficient solutions for data processing.", topic),
                format!("In this analysis of {}, we examine performance characteristics.", topic),
                format!("Key metrics for evaluating {} include accuracy and latency.", topic),
                format!("The {} technique has shown significant improvements in recent benchmarks.", topic),
            ],
            SemanticDomain::Infrastructure => vec![
                format!("Configuring {} for production environments requires careful planning.", topic),
                format!("The {} service handles distributed workloads efficiently.", topic),
                format!("Best practices for {} include monitoring and alerting.", topic),
                format!("Scaling {} horizontally provides better reliability.", topic),
                format!("Security considerations for {} are documented here.", topic),
            ],
            SemanticDomain::Documentation => vec![
                format!("This guide covers the basics of {} for new users.", topic),
                format!("The {} section explains common use cases and examples.", topic),
                format!("Troubleshooting {} issues involves checking configuration.", topic),
                format!("API reference for {} with detailed parameter descriptions.", topic),
                format!("Getting started with {} is straightforward with this tutorial.", topic),
            ],
            SemanticDomain::General => vec![
                format!("An overview of {} and its applications in software development.", topic),
                format!("The importance of {} in modern engineering practices.", topic),
                format!("Exploring {} reveals interesting patterns and solutions.", topic),
                format!("Key insights about {} from industry experience.", topic),
                format!("Understanding {} helps teams work more effectively.", topic),
            ],
        };

        let idx = self.rng.gen_range(0..templates.len());
        templates[idx].clone()
    }

    /// Generate queries for the dataset.
    fn generate_queries(
        &mut self,
        documents: &[SemanticDocument],
        ground_truth: &SemanticGroundTruth,
    ) -> Vec<SemanticQuery> {
        let mut queries = Vec::new();
        let num_queries = self.config.num_queries;

        // Distribute queries by type
        let num_distractor = (num_queries as f64 * self.config.distractor_ratio) as usize;
        let num_cross_domain = (num_queries as f64 * self.config.cross_domain_ratio) as usize;
        let num_related = num_queries / 5; // 20% related topic queries
        let num_same_topic = num_queries - num_distractor - num_cross_domain - num_related;

        // Generate same-topic queries
        for _ in 0..num_same_topic {
            if let Some(query) = self.generate_same_topic_query(documents, ground_truth) {
                queries.push(query);
            }
        }

        // Generate related-topic queries
        for _ in 0..num_related {
            if let Some(query) = self.generate_related_topic_query(documents, ground_truth) {
                queries.push(query);
            }
        }

        // Generate cross-domain queries
        for _ in 0..num_cross_domain {
            if let Some(query) = self.generate_cross_domain_query(documents, ground_truth) {
                queries.push(query);
            }
        }

        // Generate distractor queries
        for _ in 0..num_distractor {
            queries.push(self.generate_distractor_query());
        }

        // Shuffle queries
        queries.shuffle(&mut self.rng);
        queries
    }

    /// Generate a same-topic query.
    fn generate_same_topic_query(
        &mut self,
        documents: &[SemanticDocument],
        ground_truth: &SemanticGroundTruth,
    ) -> Option<SemanticQuery> {
        let doc = documents.choose(&mut self.rng)?;
        let topic_id = doc.topic_id;

        // Find all documents with same topic
        let relevant_docs: HashSet<Uuid> = documents
            .iter()
            .filter(|d| d.topic_id == topic_id)
            .map(|d| d.id)
            .collect();

        if relevant_docs.is_empty() {
            return None;
        }

        let relevance_scores: HashMap<Uuid, f64> =
            relevant_docs.iter().map(|id| (*id, 1.0)).collect();

        let query_text = self.generate_query_text(&doc.topic, doc.domain);

        Some(SemanticQuery {
            id: Uuid::new_v4(),
            text: query_text,
            target_domain: doc.domain,
            target_topic: doc.topic.clone(),
            target_topic_id: topic_id,
            query_type: SemanticQueryType::SameTopic,
            relevant_docs,
            relevance_scores,
            noise_level: 0.0,
        })
    }

    /// Generate a related-topic query.
    fn generate_related_topic_query(
        &mut self,
        documents: &[SemanticDocument],
        ground_truth: &SemanticGroundTruth,
    ) -> Option<SemanticQuery> {
        let doc = documents.choose(&mut self.rng)?;
        let topic_id = doc.topic_id;

        // Get related topics
        let related = ground_truth.related_topics.get(&topic_id)?;
        if related.is_empty() {
            return None;
        }

        // Find documents with related topics
        let mut relevant_docs = HashSet::new();
        let mut relevance_scores = HashMap::new();

        // Same topic = fully relevant
        for d in documents.iter().filter(|d| d.topic_id == topic_id) {
            relevant_docs.insert(d.id);
            relevance_scores.insert(d.id, 1.0);
        }

        // Related topics = partially relevant
        for d in documents.iter().filter(|d| related.contains(&d.topic_id)) {
            relevant_docs.insert(d.id);
            relevance_scores.insert(d.id, 0.5);
        }

        let query_text = self.generate_query_text(&doc.topic, doc.domain);

        Some(SemanticQuery {
            id: Uuid::new_v4(),
            text: query_text,
            target_domain: doc.domain,
            target_topic: doc.topic.clone(),
            target_topic_id: topic_id,
            query_type: SemanticQueryType::RelatedTopic,
            relevant_docs,
            relevance_scores,
            noise_level: 0.0,
        })
    }

    /// Generate a cross-domain query.
    fn generate_cross_domain_query(
        &mut self,
        documents: &[SemanticDocument],
        ground_truth: &SemanticGroundTruth,
    ) -> Option<SemanticQuery> {
        // Pick a generic concept that might appear across domains
        let cross_domain_concepts = [
            "performance optimization",
            "error handling",
            "configuration",
            "testing",
            "documentation",
            "best practices",
            "troubleshooting",
            "monitoring",
        ];

        let concept = cross_domain_concepts.choose(&mut self.rng)?;

        // Find documents that might match this concept (from any domain)
        // For synthetic data, we'll mark some random docs as relevant
        let num_relevant = self.rng.gen_range(5..20);
        let mut sampled: Vec<_> = documents.choose_multiple(&mut self.rng, num_relevant).collect();

        let relevant_docs: HashSet<Uuid> = sampled.iter().map(|d| d.id).collect();
        let relevance_scores: HashMap<Uuid, f64> =
            relevant_docs.iter().map(|id| (*id, 0.5)).collect();

        let target_doc = sampled.first()?;

        Some(SemanticQuery {
            id: Uuid::new_v4(),
            text: format!("How to implement {} in the system?", concept),
            target_domain: target_doc.domain,
            target_topic: concept.to_string(),
            target_topic_id: target_doc.topic_id,
            query_type: SemanticQueryType::CrossDomain,
            relevant_docs,
            relevance_scores,
            noise_level: 0.0,
        })
    }

    /// Generate an off-topic distractor query.
    fn generate_distractor_query(&mut self) -> SemanticQuery {
        let distractor_topics = [
            "cooking recipes for beginners",
            "ancient history timeline",
            "gardening tips for spring",
            "music theory fundamentals",
            "travel destinations in Europe",
            "fitness workout routines",
            "home decoration ideas",
            "photography techniques",
        ];

        let topic = distractor_topics.choose(&mut self.rng).unwrap();

        SemanticQuery {
            id: Uuid::new_v4(),
            text: format!("Information about {}", topic),
            target_domain: SemanticDomain::General,
            target_topic: topic.to_string(),
            target_topic_id: usize::MAX, // Invalid topic
            query_type: SemanticQueryType::OffTopic,
            relevant_docs: HashSet::new(),
            relevance_scores: HashMap::new(),
            noise_level: 0.0,
        }
    }

    /// Generate query text for a topic.
    fn generate_query_text(&mut self, topic: &str, domain: SemanticDomain) -> String {
        let templates = match domain {
            SemanticDomain::Code => vec![
                format!("How to implement {} in Rust?", topic),
                format!("Best practices for {} in code", topic),
                format!("Examples of {} implementation", topic),
            ],
            SemanticDomain::DataScience => vec![
                format!("How does {} work in machine learning?", topic),
                format!("Explain {} algorithm", topic),
                format!("Applications of {} in data science", topic),
            ],
            SemanticDomain::Infrastructure => vec![
                format!("How to configure {} in production?", topic),
                format!("Best practices for {} deployment", topic),
                format!("Troubleshooting {} issues", topic),
            ],
            SemanticDomain::Documentation => vec![
                format!("How to write documentation for {}?", topic),
                format!("Examples of {} documentation", topic),
                format!("Guide for {} setup", topic),
            ],
            SemanticDomain::General => vec![
                format!("What is {} and how does it work?", topic),
                format!("Overview of {}", topic),
                format!("Introduction to {}", topic),
            ],
        };

        let idx = self.rng.gen_range(0..templates.len());
        templates[idx].clone()
    }

    /// Compute statistics for the dataset.
    fn compute_stats(
        &self,
        documents: &[SemanticDocument],
        queries: &[SemanticQuery],
    ) -> E1SemanticDatasetStats {
        let mut queries_by_type = HashMap::new();
        for query in queries {
            let type_name = format!("{:?}", query.query_type);
            *queries_by_type.entry(type_name).or_insert(0) += 1;
        }

        let mut docs_per_domain = HashMap::new();
        for doc in documents {
            let domain_name = doc.domain.name().to_string();
            *docs_per_domain.entry(domain_name).or_insert(0) += 1;
        }

        let topics: HashSet<usize> = documents.iter().map(|d| d.topic_id).collect();
        let num_topics = topics.len();
        let avg_docs_per_topic = if num_topics > 0 {
            documents.len() as f64 / num_topics as f64
        } else {
            0.0
        };

        E1SemanticDatasetStats {
            num_documents: documents.len(),
            num_queries: queries.len(),
            num_topics,
            num_domains: self.config.semantic_domains.len(),
            queries_by_type,
            docs_per_domain,
            avg_docs_per_topic,
        }
    }
}

// ============================================================================
// Noise Injection
// ============================================================================

/// Apply noise to query text for robustness testing.
pub fn apply_query_noise(query: &SemanticQuery, noise_level: f64, rng: &mut impl Rng) -> SemanticQuery {
    if noise_level <= 0.0 {
        return query.clone();
    }

    let mut noisy_text = query.text.clone();

    // Apply character-level noise (working with char indices, not byte indices)
    let char_count = noisy_text.chars().count();
    let num_chars_to_modify = (char_count as f64 * noise_level * 0.1) as usize;
    for _ in 0..num_chars_to_modify {
        let current_char_count = noisy_text.chars().count();
        if current_char_count == 0 {
            break;
        }
        let char_pos = rng.gen_range(0..current_char_count);
        // Convert char position to byte position
        let byte_pos = noisy_text
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(noisy_text.len());
        // Simple noise: insert random character
        let noise_char = (rng.gen_range(b'a'..=b'z') as char).to_string();
        noisy_text.insert_str(byte_pos, &noise_char);
    }

    // Apply word-level noise (shuffle some words)
    if noise_level > 0.2 {
        let words: Vec<&str> = noisy_text.split_whitespace().collect();
        if words.len() > 3 {
            let mut word_vec: Vec<String> = words.iter().map(|s| s.to_string()).collect();
            // Swap random pairs
            let num_swaps = ((words.len() as f64) * noise_level * 0.2) as usize;
            for _ in 0..num_swaps {
                let i = rng.gen_range(0..word_vec.len());
                let j = rng.gen_range(0..word_vec.len());
                word_vec.swap(i, j);
            }
            noisy_text = word_vec.join(" ");
        }
    }

    SemanticQuery {
        text: noisy_text,
        noise_level,
        ..query.clone()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_generation() {
        let config = E1SemanticDatasetConfig {
            num_documents: 100,
            num_queries: 20,
            ..Default::default()
        };

        let mut generator = E1SemanticDatasetGenerator::new(config);
        let dataset = generator.generate();

        assert!(!dataset.documents.is_empty());
        assert!(!dataset.queries.is_empty());
        assert!(dataset.validate().is_ok());
    }

    #[test]
    fn test_query_types_distribution() {
        let config = E1SemanticDatasetConfig {
            num_documents: 200,
            num_queries: 100,
            distractor_ratio: 0.1,
            cross_domain_ratio: 0.1,
            ..Default::default()
        };

        let mut generator = E1SemanticDatasetGenerator::new(config);
        let dataset = generator.generate();

        let same_topic = dataset.queries_by_type(SemanticQueryType::SameTopic).len();
        let related = dataset.queries_by_type(SemanticQueryType::RelatedTopic).len();
        let off_topic = dataset.queries_by_type(SemanticQueryType::OffTopic).len();
        let cross_domain = dataset.queries_by_type(SemanticQueryType::CrossDomain).len();

        assert!(same_topic > 0);
        assert!(off_topic > 0);
        // Related and cross-domain might be 0 if generation failed
    }

    #[test]
    fn test_noise_application() {
        let query = SemanticQuery {
            id: Uuid::new_v4(),
            text: "How to implement error handling in Rust?".to_string(),
            target_domain: SemanticDomain::Code,
            target_topic: "error handling".to_string(),
            target_topic_id: 0,
            query_type: SemanticQueryType::SameTopic,
            relevant_docs: HashSet::new(),
            relevance_scores: HashMap::new(),
            noise_level: 0.0,
        };

        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let noisy = apply_query_noise(&query, 0.3, &mut rng);

        assert_ne!(noisy.text, query.text);
        assert!((noisy.noise_level - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_domain_sample_topics() {
        for domain in SemanticDomain::all() {
            let topics = domain.sample_topics();
            assert!(!topics.is_empty());
        }
    }
}
