//! Sparse dataset generation for benchmarking E6 embedder.
//!
//! This module generates synthetic datasets with known ground truth for
//! evaluating E6's keyword precision, sparsity characteristics, and
//! exact term matching capabilities.
//!
//! ## Dataset Types
//!
//! - **Keyword Documents**: Documents with rare technical terms that dense embedders miss
//! - **Keyword Queries**: Queries targeting exact term matching
//! - **Anti-Examples**: Semantically similar documents that lack key terms
//!
//! ## Keyword Domains
//!
//! - **TechnicalAcronyms**: HNSW, UUID, RocksDB, FAISS
//! - **ApiPaths**: tokio::spawn, std::fs, serde_json
//! - **VersionSpecific**: v7, v4, 2.0.0
//! - **ProductNames**: PostgreSQL vs "database"

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Configuration for sparse dataset generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E6SparseDatasetConfig {
    /// Number of documents to generate.
    pub num_documents: usize,

    /// Number of queries to generate.
    pub num_queries: usize,

    /// Keyword domains to include.
    pub keyword_domains: Vec<KeywordDomain>,

    /// Random seed for reproducibility.
    pub seed: u64,

    /// Ratio of anti-example documents (semantically similar but missing key terms).
    pub anti_example_ratio: f64,
}

impl Default for E6SparseDatasetConfig {
    fn default() -> Self {
        Self {
            num_documents: 100,
            num_queries: 20,
            keyword_domains: vec![
                KeywordDomain::TechnicalAcronyms,
                KeywordDomain::ApiPaths,
                KeywordDomain::VersionSpecific,
                KeywordDomain::ProductNames,
            ],
            seed: 42,
            anti_example_ratio: 0.3,
        }
    }
}

/// Domain enum for keyword categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeywordDomain {
    /// Technical acronyms: HNSW, UUID, FAISS, LSM, SSTable
    TechnicalAcronyms,
    /// API paths: tokio::spawn, std::fs, serde_json::from_str
    ApiPaths,
    /// Version-specific: v7, v4, 2.0, 3.0
    VersionSpecific,
    /// Product names: PostgreSQL, RocksDB, TensorFlow
    ProductNames,
}

impl KeywordDomain {
    /// Get all domains.
    pub fn all() -> Vec<Self> {
        vec![
            Self::TechnicalAcronyms,
            Self::ApiPaths,
            Self::VersionSpecific,
            Self::ProductNames,
        ]
    }

    /// Get keyword-document pairs for this domain.
    /// Returns (keyword, document_template, anti_document_template).
    pub fn keyword_templates(&self) -> Vec<(&'static str, &'static str, &'static str)> {
        match self {
            Self::TechnicalAcronyms => vec![
                (
                    "HNSW",
                    "The HNSW index uses hierarchical navigable small worlds for approximate nearest neighbor search. It provides logarithmic complexity for high-dimensional vectors.",
                    "The vector index uses hierarchical structures for approximate nearest neighbor search. It provides efficient complexity for high-dimensional vectors."
                ),
                (
                    "FAISS",
                    "FAISS library implements IVF-PQ for similarity search. It's developed by Meta AI Research for billion-scale vector search.",
                    "The vector library implements inverted file product quantization for similarity search. It's developed for large-scale vector operations."
                ),
                (
                    "LSM-tree",
                    "LSM-tree storage engines use log-structured merge trees for write-optimized workloads. Compaction merges sorted runs.",
                    "Storage engines use tree-based structures for write-optimized workloads. Background processes merge sorted data files."
                ),
                (
                    "SSTable",
                    "SSTable files are immutable sorted string tables used in LSM storage. Each SSTable contains key-value pairs sorted by key.",
                    "Data files are immutable sorted tables used in log-structured storage. Each file contains sorted key-value pairs."
                ),
                (
                    "B-tree",
                    "B-tree indexes provide O(log n) lookup with high fanout nodes. They're optimal for read-heavy database workloads.",
                    "Tree indexes provide logarithmic lookup with high fanout. They're optimal for read-heavy database workloads."
                ),
                (
                    "MVCC",
                    "MVCC provides snapshot isolation through multi-version concurrency control. Each transaction sees a consistent snapshot.",
                    "Multi-version systems provide snapshot isolation through concurrency control. Each transaction sees a consistent view."
                ),
            ],
            Self::ApiPaths => vec![
                (
                    "tokio::spawn",
                    "tokio::spawn semantics require the future to be Send. This enables work-stealing across threads in the async runtime.",
                    "The async spawn function requires the future to be Send. This enables work-stealing across threads in the runtime."
                ),
                (
                    "std::fs::read",
                    "std::fs::read reads entire file contents into a Vec<u8>. For large files, consider std::fs::read_to_string or buffered reading.",
                    "The file system read function loads entire file contents into memory. For large files, consider streaming or buffered reading."
                ),
                (
                    "serde_json::from_str",
                    "serde_json::from_str deserializes JSON from a string slice. It returns a Result with the deserialized value or parse error.",
                    "The JSON parse function deserializes from a string. It returns a result with the deserialized value or parse error."
                ),
                (
                    "Arc::new",
                    "Arc::new creates a new atomic reference counted pointer. Use Arc when sharing data across threads safely.",
                    "Creating a new reference counted pointer enables shared ownership. Use atomic references when sharing data across threads."
                ),
                (
                    "HashMap::get",
                    "HashMap::get returns an Option containing a reference to the value. For owned values, use HashMap::remove instead.",
                    "The hash map lookup returns an optional reference to the value. For owned values, use remove instead."
                ),
                (
                    "Vec::with_capacity",
                    "Vec::with_capacity pre-allocates memory for the expected number of elements. This avoids reallocations during push operations.",
                    "Pre-allocating vector capacity reserves memory for expected elements. This avoids reallocations during insertions."
                ),
            ],
            Self::VersionSpecific => vec![
                (
                    "UUID v7",
                    "UUID v7 timestamp encoding stores time in the first 48 bits. This enables time-ordered sorting while maintaining uniqueness.",
                    "Universal identifiers with timestamps store time in the leading bits. This enables time-ordered sorting while maintaining uniqueness."
                ),
                (
                    "UUID v4",
                    "UUID v4 uses random bits for all positions except version. It provides 122 bits of randomness for collision resistance.",
                    "Universal identifiers use random bits for uniqueness. They provide high entropy for collision resistance."
                ),
                (
                    "HTTP/2",
                    "HTTP/2 multiplexes streams over a single TCP connection. This eliminates head-of-line blocking at the application layer.",
                    "The new HTTP protocol multiplexes streams over a single connection. This improves performance by eliminating blocking."
                ),
                (
                    "TLS 1.3",
                    "TLS 1.3 reduces handshake latency to 1-RTT. It removes legacy cipher suites and simplifies the state machine.",
                    "The latest transport security reduces handshake latency. It removes legacy ciphers and simplifies the protocol."
                ),
                (
                    "Python 3.10",
                    "Python 3.10 introduces structural pattern matching with match/case syntax. This enables elegant destructuring of complex data.",
                    "Recent Python versions introduce pattern matching syntax. This enables elegant destructuring of complex data structures."
                ),
                (
                    "ECMAScript 2022",
                    "ECMAScript 2022 adds top-level await in modules. It also includes Object.hasOwn() and error cause chaining.",
                    "Recent JavaScript standards add await at module level. They also include new object methods and error handling features."
                ),
            ],
            Self::ProductNames => vec![
                (
                    "PostgreSQL",
                    "PostgreSQL supports advanced features like JSONB, window functions, and CTEs. It's known for ACID compliance and extensibility.",
                    "The relational database supports advanced features like JSON, analytics, and recursive queries. It's known for reliability."
                ),
                (
                    "RocksDB",
                    "RocksDB compaction strategy merges SSTables to reduce read amplification. Level compaction is the default for balanced workloads.",
                    "The key-value store's compaction strategy merges data files. Tiered compaction is available for write-heavy workloads."
                ),
                (
                    "TensorFlow",
                    "TensorFlow eager execution evaluates operations immediately. Use tf.function for graph compilation and optimization.",
                    "The machine learning framework supports immediate operation evaluation. Use decorators for graph compilation."
                ),
                (
                    "Kubernetes",
                    "Kubernetes pod scheduling considers resource requests and limits. Node affinity rules control placement decisions.",
                    "The container orchestration system schedules workloads based on resources. Affinity rules control placement."
                ),
                (
                    "Redis",
                    "Redis pub/sub provides real-time messaging with channel subscriptions. XREAD enables consumer groups for stream processing.",
                    "The in-memory store provides real-time messaging with subscriptions. Stream commands enable group-based processing."
                ),
                (
                    "Elasticsearch",
                    "Elasticsearch inverted index stores term-to-document mappings. BM25 scoring ranks documents by term frequency and rarity.",
                    "The search engine's inverted index stores term mappings. Relevance scoring ranks documents by frequency and rarity."
                ),
            ],
        }
    }

    /// Get query templates for this domain.
    pub fn query_templates(&self) -> Vec<&'static str> {
        match self {
            Self::TechnicalAcronyms => vec![
                "{keyword} implementation details",
                "how does {keyword} work",
                "{keyword} algorithm explained",
                "{keyword} data structure",
                "understanding {keyword}",
            ],
            Self::ApiPaths => vec![
                "{keyword} usage example",
                "how to use {keyword}",
                "{keyword} documentation",
                "{keyword} best practices",
                "when to use {keyword}",
            ],
            Self::VersionSpecific => vec![
                "{keyword} features",
                "what's new in {keyword}",
                "{keyword} specification",
                "{keyword} differences",
                "{keyword} requirements",
            ],
            Self::ProductNames => vec![
                "{keyword} configuration",
                "{keyword} performance tuning",
                "{keyword} architecture",
                "using {keyword}",
                "{keyword} internals",
            ],
        }
    }
}

impl std::fmt::Display for KeywordDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TechnicalAcronyms => write!(f, "technical_acronyms"),
            Self::ApiPaths => write!(f, "api_paths"),
            Self::VersionSpecific => write!(f, "version_specific"),
            Self::ProductNames => write!(f, "product_names"),
        }
    }
}

/// A document in the sparse benchmark dataset.
#[derive(Debug, Clone)]
pub struct SparseDocument {
    /// Unique ID.
    pub id: Uuid,

    /// Document content.
    pub content: String,

    /// Primary keywords expected to be active in E6.
    pub keywords: Vec<String>,

    /// Domain of this document.
    pub domain: KeywordDomain,

    /// Whether this is an anti-example (missing key terms).
    pub is_anti_example: bool,

    /// Why E1 alone might fail on this document.
    pub e1_limitation: Option<String>,
}

/// A query for keyword precision testing.
#[derive(Debug, Clone)]
pub struct SparseQuery {
    /// Unique ID.
    pub id: Uuid,

    /// Query text.
    pub query: String,

    /// Expected top document IDs (in order).
    pub expected_top: Vec<Uuid>,

    /// Documents that should NOT be ranked high.
    pub anti_expected: Vec<Uuid>,

    /// Why E1 alone would likely fail this query.
    pub e1_limitation: String,

    /// Domain of the query.
    pub domain: KeywordDomain,

    /// Primary keyword being tested.
    pub keyword: String,
}

/// Complete sparse benchmark dataset.
#[derive(Debug)]
pub struct E6SparseBenchmarkDataset {
    /// All documents.
    pub documents: Vec<SparseDocument>,

    /// Queries for evaluation.
    pub queries: Vec<SparseQuery>,

    /// Ground truth mapping: query_id -> ranked doc_ids
    pub ground_truth: HashMap<Uuid, Vec<Uuid>>,

    /// Configuration used.
    pub config: E6SparseDatasetConfig,
}

/// Generator for sparse benchmark datasets.
pub struct E6SparseDatasetGenerator {
    config: E6SparseDatasetConfig,
    rng: ChaCha8Rng,
}

impl E6SparseDatasetGenerator {
    /// Create a new generator with the given configuration.
    pub fn new(config: E6SparseDatasetConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        Self { config, rng }
    }

    /// Generate a deterministic UUID from the seeded RNG.
    fn next_uuid(&mut self) -> Uuid {
        let mut bytes = [0u8; 16];
        self.rng.fill_bytes(&mut bytes);
        bytes[6] = (bytes[6] & 0x0f) | 0x40; // Version 4
        bytes[8] = (bytes[8] & 0x3f) | 0x80; // Variant 1
        Uuid::from_bytes(bytes)
    }

    /// Generate a complete sparse benchmark dataset.
    pub fn generate(&mut self) -> E6SparseBenchmarkDataset {
        let mut documents = Vec::new();
        let mut queries = Vec::new();
        let mut ground_truth = HashMap::new();

        // Track keyword to document mappings
        let mut keyword_docs: HashMap<String, Vec<Uuid>> = HashMap::new();
        let mut keyword_anti_docs: HashMap<String, Vec<Uuid>> = HashMap::new();

        // Clone config values to avoid borrow checker issues
        let keyword_domains = self.config.keyword_domains.clone();
        let anti_example_ratio = self.config.anti_example_ratio;
        let num_documents = self.config.num_documents;
        let num_queries = self.config.num_queries;

        // Generate documents per domain
        for domain in &keyword_domains {
            let templates = domain.keyword_templates();

            for (keyword, doc_template, anti_template) in &templates {
                // Create the primary document with the keyword
                let doc_id = self.next_uuid();
                documents.push(SparseDocument {
                    id: doc_id,
                    content: doc_template.to_string(),
                    keywords: vec![keyword.to_string()],
                    domain: *domain,
                    is_anti_example: false,
                    e1_limitation: Some(format!(
                        "E1 may not distinguish exact '{}' from semantic equivalents",
                        keyword
                    )),
                });
                keyword_docs
                    .entry(keyword.to_string())
                    .or_default()
                    .push(doc_id);

                // Create anti-example document (semantically similar, missing keyword)
                if self.rng.gen_bool(anti_example_ratio) {
                    let anti_id = self.next_uuid();
                    documents.push(SparseDocument {
                        id: anti_id,
                        content: anti_template.to_string(),
                        keywords: vec![], // No target keywords
                        domain: *domain,
                        is_anti_example: true,
                        e1_limitation: Some(format!(
                            "Semantically similar to '{}' docs but missing exact term",
                            keyword
                        )),
                    });
                    keyword_anti_docs
                        .entry(keyword.to_string())
                        .or_default()
                        .push(anti_id);
                }
            }
        }

        // Pad with additional documents if needed
        while documents.len() < num_documents {
            let domain = keyword_domains[self.rng.gen_range(0..keyword_domains.len())];
            let templates = domain.keyword_templates();
            let (keyword, doc_template, _) = templates[self.rng.gen_range(0..templates.len())];

            let doc_id = self.next_uuid();
            documents.push(SparseDocument {
                id: doc_id,
                content: format!("{} Additional context for testing.", doc_template),
                keywords: vec![keyword.to_string()],
                domain,
                is_anti_example: false,
                e1_limitation: None,
            });
            keyword_docs
                .entry(keyword.to_string())
                .or_default()
                .push(doc_id);
        }

        // Generate queries
        let queries_per_domain = num_queries / keyword_domains.len().max(1);

        for domain in &keyword_domains {
            let query_templates = domain.query_templates();
            let keyword_templates = domain.keyword_templates();

            for _ in 0..queries_per_domain {
                let (keyword, _, _) =
                    keyword_templates[self.rng.gen_range(0..keyword_templates.len())];
                let query_template = query_templates[self.rng.gen_range(0..query_templates.len())];
                let query_text = query_template.replace("{keyword}", keyword);

                let query_id = self.next_uuid();
                let expected = keyword_docs.get(keyword).cloned().unwrap_or_default();
                let anti = keyword_anti_docs.get(keyword).cloned().unwrap_or_default();

                queries.push(SparseQuery {
                    id: query_id,
                    query: query_text,
                    expected_top: expected.clone(),
                    anti_expected: anti,
                    e1_limitation: format!(
                        "E1 may rank semantically similar docs without '{}' equally",
                        keyword
                    ),
                    domain: *domain,
                    keyword: keyword.to_string(),
                });

                ground_truth.insert(query_id, expected);
            }
        }

        // Shuffle documents and queries for fairness
        documents.shuffle(&mut self.rng);
        queries.shuffle(&mut self.rng);

        E6SparseBenchmarkDataset {
            documents,
            queries,
            ground_truth,
            config: self.config.clone(),
        }
    }
}

impl E6SparseBenchmarkDataset {
    /// Get a document by ID.
    pub fn get_document(&self, id: &Uuid) -> Option<&SparseDocument> {
        self.documents.iter().find(|d| &d.id == id)
    }

    /// Get documents by keyword.
    pub fn documents_with_keyword(&self, keyword: &str) -> Vec<&SparseDocument> {
        self.documents
            .iter()
            .filter(|d| d.keywords.iter().any(|k| k == keyword))
            .collect()
    }

    /// Get anti-example documents.
    pub fn anti_example_documents(&self) -> Vec<&SparseDocument> {
        self.documents
            .iter()
            .filter(|d| d.is_anti_example)
            .collect()
    }

    /// Validate dataset consistency.
    pub fn validate(&self) -> Result<(), String> {
        // Check queries reference valid documents
        for query in &self.queries {
            for doc_id in &query.expected_top {
                if self.get_document(doc_id).is_none() {
                    return Err(format!(
                        "Query {} references unknown document {}",
                        query.id, doc_id
                    ));
                }
            }
            for doc_id in &query.anti_expected {
                if self.get_document(doc_id).is_none() {
                    return Err(format!(
                        "Query {} references unknown anti-document {}",
                        query.id, doc_id
                    ));
                }
            }
        }

        // Check ground truth consistency
        for (query_id, doc_ids) in &self.ground_truth {
            let query = self.queries.iter().find(|q| &q.id == query_id);
            if query.is_none() {
                return Err(format!(
                    "Ground truth references unknown query {}",
                    query_id
                ));
            }
            for doc_id in doc_ids {
                if self.get_document(doc_id).is_none() {
                    return Err(format!(
                        "Ground truth for query {} references unknown document {}",
                        query_id, doc_id
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get dataset statistics.
    pub fn stats(&self) -> E6SparseDatasetStats {
        let mut docs_by_domain: HashMap<KeywordDomain, usize> = HashMap::new();
        let mut queries_by_domain: HashMap<KeywordDomain, usize> = HashMap::new();
        let mut keywords: std::collections::HashSet<String> = std::collections::HashSet::new();

        for doc in &self.documents {
            *docs_by_domain.entry(doc.domain).or_default() += 1;
            for kw in &doc.keywords {
                keywords.insert(kw.clone());
            }
        }

        for query in &self.queries {
            *queries_by_domain.entry(query.domain).or_default() += 1;
        }

        let anti_example_count = self.documents.iter().filter(|d| d.is_anti_example).count();

        E6SparseDatasetStats {
            total_documents: self.documents.len(),
            total_queries: self.queries.len(),
            anti_example_count,
            unique_keywords: keywords.len(),
            docs_by_domain,
            queries_by_domain,
            avg_keywords_per_doc: self
                .documents
                .iter()
                .map(|d| d.keywords.len())
                .sum::<usize>() as f64
                / self.documents.len().max(1) as f64,
        }
    }
}

/// Statistics about a sparse dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E6SparseDatasetStats {
    pub total_documents: usize,
    pub total_queries: usize,
    pub anti_example_count: usize,
    pub unique_keywords: usize,
    pub docs_by_domain: HashMap<KeywordDomain, usize>,
    pub queries_by_domain: HashMap<KeywordDomain, usize>,
    pub avg_keywords_per_doc: f64,
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_generation() {
        let config = E6SparseDatasetConfig::default();
        let mut generator = E6SparseDatasetGenerator::new(config);
        let dataset = generator.generate();

        assert!(!dataset.documents.is_empty());
        assert!(!dataset.queries.is_empty());

        println!("[VERIFIED] Sparse dataset generation works");
        println!("  Documents: {}", dataset.documents.len());
        println!("  Queries: {}", dataset.queries.len());
    }

    #[test]
    fn test_dataset_validation() {
        let config = E6SparseDatasetConfig {
            num_documents: 50,
            num_queries: 10,
            seed: 42,
            ..Default::default()
        };

        let mut generator = E6SparseDatasetGenerator::new(config);
        let dataset = generator.generate();

        let result = dataset.validate();
        assert!(result.is_ok(), "Validation failed: {:?}", result);

        println!("[VERIFIED] Generated dataset passes validation");
    }

    #[test]
    fn test_domain_coverage() {
        let config = E6SparseDatasetConfig {
            num_documents: 100,
            num_queries: 20,
            keyword_domains: KeywordDomain::all(),
            seed: 42,
            ..Default::default()
        };

        let mut generator = E6SparseDatasetGenerator::new(config);
        let dataset = generator.generate();
        let stats = dataset.stats();

        // Each domain should have some documents
        for domain in KeywordDomain::all() {
            let count = stats.docs_by_domain.get(&domain).copied().unwrap_or(0);
            assert!(count > 0, "Domain {:?} has no documents", domain);
        }

        println!("[VERIFIED] All domains have coverage");
        for (domain, count) in &stats.docs_by_domain {
            println!("  {:?}: {}", domain, count);
        }
    }

    #[test]
    fn test_anti_example_generation() {
        let config = E6SparseDatasetConfig {
            num_documents: 100,
            anti_example_ratio: 0.5,
            seed: 42,
            ..Default::default()
        };

        let mut generator = E6SparseDatasetGenerator::new(config);
        let dataset = generator.generate();

        let anti_count = dataset.anti_example_documents().len();
        assert!(anti_count > 0, "No anti-examples generated");

        println!("[VERIFIED] Anti-examples generated: {}", anti_count);
    }

    #[test]
    fn test_reproducibility() {
        let config = E6SparseDatasetConfig {
            num_documents: 20,
            num_queries: 5,
            seed: 12345,
            ..Default::default()
        };

        let mut gen1 = E6SparseDatasetGenerator::new(config.clone());
        let dataset1 = gen1.generate();

        let mut gen2 = E6SparseDatasetGenerator::new(config);
        let dataset2 = gen2.generate();

        // Same seed should produce same IDs
        assert_eq!(dataset1.documents.len(), dataset2.documents.len());
        for (d1, d2) in dataset1.documents.iter().zip(dataset2.documents.iter()) {
            assert_eq!(d1.id, d2.id, "Document IDs should match with same seed");
            assert_eq!(d1.content, d2.content);
        }

        println!("[VERIFIED] Dataset generation is reproducible with same seed");
    }

    #[test]
    fn test_query_keyword_matching() {
        let config = E6SparseDatasetConfig {
            num_documents: 50,
            num_queries: 10,
            seed: 42,
            ..Default::default()
        };

        let mut generator = E6SparseDatasetGenerator::new(config);
        let dataset = generator.generate();

        // Each query should have at least one expected document
        for query in &dataset.queries {
            assert!(
                !query.expected_top.is_empty(),
                "Query {} has no expected documents",
                query.id
            );
            assert!(
                !query.keyword.is_empty(),
                "Query {} has no keyword",
                query.id
            );
        }

        println!("[VERIFIED] All queries have expected documents and keywords");
    }
}
