//! Per-Embedder Stress Test Corpus
//!
//! This module provides curated test corpora designed to stress-test specific
//! embedder capabilities. Each embedder has queries that require its unique
//! signal to answer correctly.
//!
//! # Research Insight
//!
//! Since E1 (semantic) is part of the 13-embedder pipeline, we measure what
//! *additional* information each of the 12 extra embedders provides beyond E1.
//!
//! # Embedder Stress Tests
//!
//! | Embedder | What It Captures | Stress Test Focus |
//! |----------|------------------|-------------------|
//! | E5 Causal | Cause-effect direction | Causal direction asymmetry |
//! | E6 Sparse | Exact keyword matching | Rare technical terms |
//! | E7 Code | Programming patterns | Code signatures vs NL |
//! | E8 Graph | Structural connectivity | Link relationships |
//! | E9 HDC | Noise-robust patterns | OCR errors, typos |
//! | E10 Multimodal | Cross-modal intent | Visual/diagram descriptions |
//! | E11 Entity | Named entities | Entity disambiguation |
//! | E12 Late | Token-level precision | Word order sensitivity |
//! | E13 SPLADE | Term expansion | Implicit vocabulary |

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use context_graph_storage::teleological::indexes::EmbedderIndex;

/// Configuration for a single embedder's stress test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderStressConfig {
    /// Target embedder for this stress test.
    pub embedder: EmbedderIndex,
    /// Human-readable name.
    pub name: &'static str,
    /// Short description of what this embedder captures.
    pub description: &'static str,
    /// Corpus entries with expected rankings.
    pub corpus: Vec<StressCorpusEntry>,
    /// Stress-test queries.
    pub queries: Vec<StressQuery>,
}

/// A corpus entry for stress testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressCorpusEntry {
    /// Document content.
    pub content: String,
    /// Document ID (assigned during indexing).
    pub doc_id: usize,
    /// Why E1 alone might fail on this document.
    pub e1_limitation: Option<String>,
    /// Metadata (links for E8, timestamps for E2-E4, etc.)
    pub metadata: Option<serde_json::Value>,
}

/// A stress-test query designed for a specific embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressQuery {
    /// Query text.
    pub query: String,
    /// Which embedder this query is designed to stress.
    pub target_embedder: EmbedderIndex,
    /// Expected top doc indices (in order of expected ranking).
    pub expected_top_docs: Vec<usize>,
    /// Docs that should NOT be ranked high (anti-examples).
    pub anti_expected_docs: Vec<usize>,
    /// Why E1 alone would likely fail this query.
    pub e1_failure_reason: String,
}

/// Results from running stress tests for a single embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderStressResults {
    /// Target embedder.
    pub embedder: EmbedderIndex,
    /// Human-readable name.
    pub name: String,
    /// Mean Reciprocal Rank on stress-test queries.
    pub stress_test_mrr: f32,
    /// Percentage of queries where expected doc is #1.
    pub stress_test_success_rate: f32,
    /// Score(all) - Score(all except this embedder).
    pub ablation_delta: f32,
    /// stress_mrr * ablation_delta.
    pub unique_contribution: f32,
    /// How well it avoids ranking wrong docs high.
    pub anti_ranking_score: f32,
    /// Number of queries that passed expectations.
    pub queries_passed: usize,
    /// Total number of queries.
    pub queries_total: usize,
}

// =============================================================================
// STRESS CORPUS BUILDERS
// =============================================================================

/// Build E5 Causal stress test corpus.
///
/// E5 captures cause-effect asymmetric relationships.
/// E1 alone would rank semantically similar docs equally regardless of causal direction.
pub fn build_e5_causal_corpus() -> EmbedderStressConfig {
    EmbedderStressConfig {
        embedder: EmbedderIndex::E5Causal,
        name: "E5 Causal",
        description: "Cause-effect relationships with asymmetric direction",
        corpus: vec![
            StressCorpusEntry {
                content: "Increased insulin causes blood glucose to decrease. This is the mechanism of action for managing diabetes.".into(),
                doc_id: 0,
                e1_limitation: Some("Semantically similar to reversed causal statement".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Decreased blood glucose triggers insulin release from the pancreas. This is the body's natural feedback mechanism.".into(),
                doc_id: 1,
                e1_limitation: Some("Same keywords but opposite causal direction".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Insulin and glucose are both hormones involved in metabolism. They interact in complex ways.".into(),
                doc_id: 2,
                e1_limitation: Some("Mentions same entities but no causality".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Memory leaks in Rust occur when reference cycles prevent Drop from being called. Using Weak references breaks cycles.".into(),
                doc_id: 3,
                e1_limitation: None,
                metadata: None,
            },
            StressCorpusEntry {
                content: "When Weak references are used, memory is freed. This happens because the reference count reaches zero.".into(),
                doc_id: 4,
                e1_limitation: Some("Reversed causal flow".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Connection timeouts occur because the server is overloaded. The solution is to increase server capacity.".into(),
                doc_id: 5,
                e1_limitation: None,
                metadata: None,
            },
            StressCorpusEntry {
                content: "Server capacity was increased, which caused fewer connection timeouts. Users reported improved experience.".into(),
                doc_id: 6,
                e1_limitation: Some("Effect mentioned before cause".into()),
                metadata: None,
            },
        ],
        queries: vec![
            StressQuery {
                query: "Why does blood sugar drop after eating?".into(),
                target_embedder: EmbedderIndex::E5Causal,
                expected_top_docs: vec![0], // Insulin -> glucose decrease
                anti_expected_docs: vec![1], // Wrong causal direction
                e1_failure_reason: "E1 would rank doc 1 highly due to word overlap".into(),
            },
            StressQuery {
                query: "What causes memory leaks in Rust?".into(),
                target_embedder: EmbedderIndex::E5Causal,
                expected_top_docs: vec![3],
                anti_expected_docs: vec![4],
                e1_failure_reason: "Both docs mention memory and Rust".into(),
            },
            StressQuery {
                query: "Why do connection timeouts happen?".into(),
                target_embedder: EmbedderIndex::E5Causal,
                expected_top_docs: vec![5],
                anti_expected_docs: vec![6],
                e1_failure_reason: "Both mention timeout and server".into(),
            },
        ],
    }
}

/// Build E6 Sparse stress test corpus.
///
/// E6 captures exact keyword matching for rare/technical terms.
/// E1 alone might miss rare terms or favor semantically similar but incorrect matches.
pub fn build_e6_sparse_corpus() -> EmbedderStressConfig {
    EmbedderStressConfig {
        embedder: EmbedderIndex::E6Sparse,
        name: "E6 Sparse",
        description: "Exact keyword matching for rare technical terms",
        corpus: vec![
            StressCorpusEntry {
                content: "The HNSW index uses hierarchical navigable small worlds for approximate nearest neighbor search. It provides logarithmic complexity.".into(),
                doc_id: 0,
                e1_limitation: Some("Rare acronym HNSW not in E1 vocabulary".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The vector database uses approximate nearest neighbor search for similarity queries. It's optimized for high dimensions.".into(),
                doc_id: 1,
                e1_limitation: Some("Semantically similar but missing HNSW".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "FAISS library implements IVF-PQ for similarity search. It's developed by Facebook Research.".into(),
                doc_id: 2,
                e1_limitation: Some("Different library, similar domain".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "UUID v7 timestamp encoding stores time in the first 48 bits. This enables time-ordered sorting.".into(),
                doc_id: 3,
                e1_limitation: Some("Specific version v7 matters".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Universal identifiers use random bits for uniqueness. Time can be optionally embedded.".into(),
                doc_id: 4,
                e1_limitation: Some("Generic UUID info, not v7 specific".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "RocksDB compaction strategy merges SSTables to reduce read amplification. Level compaction is the default.".into(),
                doc_id: 5,
                e1_limitation: Some("Specific to RocksDB".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Database storage engines use log-structured merge trees. Compaction is a key operation.".into(),
                doc_id: 6,
                e1_limitation: Some("Generic LSM-tree info".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "tokio::spawn semantics require the future to be Send. This enables work-stealing across threads.".into(),
                doc_id: 7,
                e1_limitation: Some("Exact Rust API matters".into()),
                metadata: None,
            },
        ],
        queries: vec![
            StressQuery {
                query: "HNSW implementation details".into(),
                target_embedder: EmbedderIndex::E6Sparse,
                expected_top_docs: vec![0],
                anti_expected_docs: vec![1, 2],
                e1_failure_reason: "E1 might prefer doc 1/2 due to ANN semantic similarity".into(),
            },
            StressQuery {
                query: "UUID v7 timestamp encoding".into(),
                target_embedder: EmbedderIndex::E6Sparse,
                expected_top_docs: vec![3],
                anti_expected_docs: vec![4],
                e1_failure_reason: "E1 doesn't distinguish v7 from generic UUID".into(),
            },
            StressQuery {
                query: "RocksDB compaction strategy".into(),
                target_embedder: EmbedderIndex::E6Sparse,
                expected_top_docs: vec![5],
                anti_expected_docs: vec![6],
                e1_failure_reason: "E1 sees both as compaction related".into(),
            },
            StressQuery {
                query: "tokio::spawn semantics".into(),
                target_embedder: EmbedderIndex::E6Sparse,
                expected_top_docs: vec![7],
                anti_expected_docs: vec![],
                e1_failure_reason: "E1 doesn't recognize exact API name".into(),
            },
        ],
    }
}

/// Build E7 Code stress test corpus.
///
/// E7 captures programming language patterns and code structure.
/// E1 alone would prefer natural language descriptions over actual code.
pub fn build_e7_code_corpus() -> EmbedderStressConfig {
    EmbedderStressConfig {
        embedder: EmbedderIndex::E7Code,
        name: "E7 Code",
        description: "Programming language patterns and code structure",
        corpus: vec![
            StressCorpusEntry {
                content: "fn process_batch<T: Send>(items: Vec<T>) -> Result<Vec<T>, Error> { items.into_par_iter().map(process).collect() }".into(),
                doc_id: 0,
                e1_limitation: Some("Code syntax not in E1 training".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The batch processing function handles items concurrently using parallel iterators for improved throughput.".into(),
                doc_id: 1,
                e1_limitation: Some("Natural language description".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "async fn handle_request(req: Request) -> Response { let body = req.body().await?; Response::new(body) }".into(),
                doc_id: 2,
                e1_limitation: Some("Different function signature".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "impl Iterator for Counter { type Item = u32; fn next(&mut self) -> Option<Self::Item> { self.count += 1; Some(self.count) } }".into(),
                doc_id: 3,
                e1_limitation: Some("Iterator trait implementation".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The iterator pattern allows sequential access to elements without exposing the underlying data structure.".into(),
                doc_id: 4,
                e1_limitation: Some("Concept explanation, not implementation".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "struct Config<'a> where T: 'a + Clone { data: &'a T, options: HashMap<String, Value> }".into(),
                doc_id: 5,
                e1_limitation: Some("Lifetime bounds in struct".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Configuration structs hold application settings. They often use references for borrowed data.".into(),
                doc_id: 6,
                e1_limitation: Some("Description without code".into()),
                metadata: None,
            },
        ],
        queries: vec![
            StressQuery {
                query: "generic batch processing function with error handling".into(),
                target_embedder: EmbedderIndex::E7Code,
                expected_top_docs: vec![0],
                anti_expected_docs: vec![1],
                e1_failure_reason: "E1 prefers natural language doc 1".into(),
            },
            StressQuery {
                query: "impl Iterator for".into(),
                target_embedder: EmbedderIndex::E7Code,
                expected_top_docs: vec![3],
                anti_expected_docs: vec![4],
                e1_failure_reason: "E1 doesn't understand Rust impl syntax".into(),
            },
            StressQuery {
                query: "struct with lifetime bounds".into(),
                target_embedder: EmbedderIndex::E7Code,
                expected_top_docs: vec![5],
                anti_expected_docs: vec![6],
                e1_failure_reason: "E1 sees both as config-related".into(),
            },
            StressQuery {
                query: "async fn returning Response".into(),
                target_embedder: EmbedderIndex::E7Code,
                expected_top_docs: vec![2],
                anti_expected_docs: vec![],
                e1_failure_reason: "E1 doesn't parse function signatures".into(),
            },
        ],
    }
}

/// Build E8 Graph stress test corpus.
///
/// E8 captures structural connectivity and link relationships.
/// E1 focuses on semantic similarity without understanding structural relationships.
pub fn build_e8_graph_corpus() -> EmbedderStressConfig {
    EmbedderStressConfig {
        embedder: EmbedderIndex::E8Graph,
        name: "E8 Graph",
        description: "Structural connectivity and link relationships with source/target asymmetry",
        corpus: vec![
            // Module dependency relationships
            StressCorpusEntry {
                content: "The AuthService module imports UserRepository and TokenValidator. It exports authenticate() and logout() functions.".into(),
                doc_id: 0,
                e1_limitation: Some("E1 misses import/export direction".into()),
                metadata: Some(serde_json::json!({
                    "imports": ["UserRepository", "TokenValidator"],
                    "exports": ["authenticate", "logout"],
                    "graph_direction": "source"
                })),
            },
            StressCorpusEntry {
                content: "The UserRepository is used by AuthService, OrderService, and ProfileService. It provides findById() and save() methods.".into(),
                doc_id: 1,
                e1_limitation: Some("E1 doesn't understand 'used by' as target relationship".into()),
                metadata: Some(serde_json::json!({
                    "used_by": ["AuthService", "OrderService", "ProfileService"],
                    "provides": ["findById", "save"],
                    "graph_direction": "target"
                })),
            },
            StressCorpusEntry {
                content: "Authentication handles user login and logout operations. Token validation ensures security.".into(),
                doc_id: 2,
                e1_limitation: Some("Semantic match without structural info".into()),
                metadata: None,
            },
            // Class inheritance relationships
            StressCorpusEntry {
                content: "The HttpHandler class extends BaseHandler and implements RequestHandler interface. It overrides handle() method.".into(),
                doc_id: 3,
                e1_limitation: Some("E1 misses extends/implements hierarchy".into()),
                metadata: Some(serde_json::json!({
                    "extends": "BaseHandler",
                    "implements": ["RequestHandler"],
                    "graph_direction": "source"
                })),
            },
            StressCorpusEntry {
                content: "BaseHandler is the parent class for HttpHandler, WebSocketHandler, and GrpcHandler. It defines abstract handle() method.".into(),
                doc_id: 4,
                e1_limitation: Some("E1 doesn't see 'parent class' as structural relationship".into()),
                metadata: Some(serde_json::json!({
                    "subclasses": ["HttpHandler", "WebSocketHandler", "GrpcHandler"],
                    "graph_direction": "target"
                })),
            },
            StressCorpusEntry {
                content: "Request handling processes incoming HTTP requests and returns responses.".into(),
                doc_id: 5,
                e1_limitation: Some("Generic HTTP description".into()),
                metadata: None,
            },
            // Database schema relationships
            StressCorpusEntry {
                content: "The orders table has a foreign key user_id that references the users table. Each order belongs to exactly one user.".into(),
                doc_id: 6,
                e1_limitation: Some("E1 misses FK relationship direction".into()),
                metadata: Some(serde_json::json!({
                    "references": "users",
                    "foreign_key": "user_id",
                    "graph_direction": "source"
                })),
            },
            StressCorpusEntry {
                content: "The users table is referenced by orders, reviews, and payments tables. It is the primary entity.".into(),
                doc_id: 7,
                e1_limitation: Some("E1 doesn't understand 'referenced by' as inbound links".into()),
                metadata: Some(serde_json::json!({
                    "referenced_by": ["orders", "reviews", "payments"],
                    "graph_direction": "target"
                })),
            },
            // API endpoint relationships
            StressCorpusEntry {
                content: "The /api/orders endpoint calls /api/users for user validation and /api/inventory for stock checking.".into(),
                doc_id: 8,
                e1_limitation: Some("E1 misses API call direction".into()),
                metadata: Some(serde_json::json!({
                    "calls": ["/api/users", "/api/inventory"],
                    "graph_direction": "source"
                })),
            },
            StressCorpusEntry {
                content: "The /api/users endpoint is called by /api/orders, /api/auth, and /api/profile for user data retrieval.".into(),
                doc_id: 9,
                e1_limitation: Some("E1 doesn't see 'called by' as inbound relationship".into()),
                metadata: Some(serde_json::json!({
                    "called_by": ["/api/orders", "/api/auth", "/api/profile"],
                    "graph_direction": "target"
                })),
            },
        ],
        queries: vec![
            // Module dependency queries
            StressQuery {
                query: "What modules does AuthService import?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![0], // AuthService imports UserRepository, TokenValidator
                anti_expected_docs: vec![1, 2], // UserRepository is target, doc 2 has no structure
                e1_failure_reason: "E1 matches all auth-related docs equally".into(),
            },
            StressQuery {
                query: "What modules use UserRepository?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![1], // UserRepository used_by list
                anti_expected_docs: vec![0], // AuthService is source, not answering this
                e1_failure_reason: "E1 doesn't understand 'use' direction".into(),
            },
            // Class hierarchy queries
            StressQuery {
                query: "What does HttpHandler extend?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![3], // HttpHandler extends BaseHandler
                anti_expected_docs: vec![4, 5],
                e1_failure_reason: "E1 matches all handler-related docs".into(),
            },
            StressQuery {
                query: "What classes inherit from BaseHandler?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![4], // BaseHandler subclasses list
                anti_expected_docs: vec![3], // HttpHandler is a source node
                e1_failure_reason: "E1 doesn't understand inheritance direction".into(),
            },
            // Database relationship queries
            StressQuery {
                query: "What table does orders reference?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![6], // orders references users
                anti_expected_docs: vec![7],
                e1_failure_reason: "E1 sees both as order/user related".into(),
            },
            StressQuery {
                query: "What tables have foreign keys to users?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![7], // users referenced_by list
                anti_expected_docs: vec![6],
                e1_failure_reason: "E1 doesn't understand FK direction".into(),
            },
            // API call graph queries
            StressQuery {
                query: "What APIs does /api/orders call?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![8], // orders calls users, inventory
                anti_expected_docs: vec![9],
                e1_failure_reason: "E1 matches API endpoints semantically".into(),
            },
            StressQuery {
                query: "What endpoints call /api/users?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![9], // users called_by list
                anti_expected_docs: vec![8],
                e1_failure_reason: "E1 doesn't understand call direction".into(),
            },
        ],
    }
}

/// Build E9 HDC stress test corpus.
///
/// E9 captures noise-robust patterns using hyperdimensional codes.
/// E1 would struggle with OCR errors and typos.
pub fn build_e9_hdc_corpus() -> EmbedderStressConfig {
    EmbedderStressConfig {
        embedder: EmbedderIndex::E9HDC,
        name: "E9 HDC",
        description: "Noise-robust patterns resilient to OCR errors and typos",
        corpus: vec![
            StressCorpusEntry {
                content: "authentication middleware for secure API access".into(),
                doc_id: 0,
                e1_limitation: Some("Clean text baseline".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "aut4entication middl3ware for s3cure API acc3ss".into(),
                doc_id: 1,
                e1_limitation: Some("OCR errors break E1 tokenization".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "authorization handler for protected endpoints".into(),
                doc_id: 2,
                e1_limitation: Some("Different meaning despite similar words".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The configuration file specifies database connection parameters.".into(),
                doc_id: 3,
                e1_limitation: Some("Clean text".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Th3 c0nfigurat1on f1le sp3cifies databas3 connection param3ters.".into(),
                doc_id: 4,
                e1_limitation: Some("Leet-speak OCR errors".into()),
                metadata: None,
            },
        ],
        queries: vec![
            StressQuery {
                query: "authent1cation m1ddleware".into(),
                target_embedder: EmbedderIndex::E9HDC,
                expected_top_docs: vec![0, 1], // Both should match
                anti_expected_docs: vec![2],
                e1_failure_reason: "E1 can't match misspelled tokens".into(),
            },
            StressQuery {
                query: "c0nf1gurat1on f1le".into(),
                target_embedder: EmbedderIndex::E9HDC,
                expected_top_docs: vec![3, 4],
                anti_expected_docs: vec![],
                e1_failure_reason: "E1 tokenizer fails on leet-speak".into(),
            },
        ],
    }
}

/// Build E10 Multimodal stress test corpus.
///
/// E10 captures cross-modal alignment between text and visual/intent concepts.
/// E1 focuses on text semantics without visual understanding.
pub fn build_e10_multimodal_corpus() -> EmbedderStressConfig {
    EmbedderStressConfig {
        embedder: EmbedderIndex::E10Multimodal,
        name: "E10 Multimodal",
        description: "Cross-modal alignment for visual/intent concepts",
        corpus: vec![
            StressCorpusEntry {
                content: "The flowchart shows user registration with email verification. Arrows indicate the flow from signup form to confirmation email to account activation.".into(),
                doc_id: 0,
                e1_limitation: Some("Visual diagram description".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "User registration requires email and password fields. The form validates input before submission.".into(),
                doc_id: 1,
                e1_limitation: Some("Text description without visual".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The diagram displays a circular workflow pattern where tasks cycle through planning, execution, and review phases.".into(),
                doc_id: 2,
                e1_limitation: Some("Different visual pattern".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The architecture diagram shows microservices connected through an API gateway. Each service has its own database.".into(),
                doc_id: 3,
                e1_limitation: Some("Architecture visualization".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Microservices communicate through APIs. Each service is independently deployable.".into(),
                doc_id: 4,
                e1_limitation: Some("Concept without visual".into()),
                metadata: None,
            },
        ],
        queries: vec![
            StressQuery {
                query: "registration flow diagram".into(),
                target_embedder: EmbedderIndex::E10Multimodal,
                expected_top_docs: vec![0],
                anti_expected_docs: vec![1],
                e1_failure_reason: "E1 doesn't understand 'flow diagram' as visual concept".into(),
            },
            StressQuery {
                query: "microservices architecture diagram".into(),
                target_embedder: EmbedderIndex::E10Multimodal,
                expected_top_docs: vec![3],
                anti_expected_docs: vec![4],
                e1_failure_reason: "E1 treats both as microservices text".into(),
            },
        ],
    }
}

/// Build E11 Entity stress test corpus.
///
/// E11 captures named entity references for precise entity matching.
/// E1 might confuse entities with similar names or contexts.
pub fn build_e11_entity_corpus() -> EmbedderStressConfig {
    EmbedderStressConfig {
        embedder: EmbedderIndex::E11Entity,
        name: "E11 Entity",
        description: "Named entity references for precise entity matching",
        corpus: vec![
            StressCorpusEntry {
                content: "PostgreSQL 16 introduces new JSON functions including json_exists and json_value for improved JSON handling.".into(),
                doc_id: 0,
                e1_limitation: Some("Specific database version".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The database added improved JSON support with new query operators and indexing capabilities.".into(),
                doc_id: 1,
                e1_limitation: Some("Generic database, not PostgreSQL".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "MySQL 8.0 has enhanced JSON features including multi-valued indexes and JSON schema validation.".into(),
                doc_id: 2,
                e1_limitation: Some("Different database entity".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Anthropic Claude excels at complex reasoning and following nuanced instructions.".into(),
                doc_id: 3,
                e1_limitation: Some("Specific AI entity".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "OpenAI GPT models are widely used for text generation and conversation.".into(),
                doc_id: 4,
                e1_limitation: Some("Different AI entity".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Large language models have revolutionized natural language processing.".into(),
                doc_id: 5,
                e1_limitation: Some("Generic LLM reference".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The Rust programming language provides memory safety without garbage collection through its ownership system.".into(),
                doc_id: 6,
                e1_limitation: Some("Rust language".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Rust is a popular survival video game where players gather resources and build bases.".into(),
                doc_id: 7,
                e1_limitation: Some("Rust game - entity disambiguation".into()),
                metadata: None,
            },
        ],
        queries: vec![
            StressQuery {
                query: "PostgreSQL JSON features".into(),
                target_embedder: EmbedderIndex::E11Entity,
                expected_top_docs: vec![0],
                anti_expected_docs: vec![1, 2],
                e1_failure_reason: "E1 sees all as database JSON related".into(),
            },
            StressQuery {
                query: "Anthropic Claude capabilities".into(),
                target_embedder: EmbedderIndex::E11Entity,
                expected_top_docs: vec![3],
                anti_expected_docs: vec![4, 5],
                e1_failure_reason: "E1 might conflate different LLMs".into(),
            },
            StressQuery {
                query: "Rust language ownership".into(),
                target_embedder: EmbedderIndex::E11Entity,
                expected_top_docs: vec![6],
                anti_expected_docs: vec![7],
                e1_failure_reason: "E1 might confuse Rust language with Rust game".into(),
            },
        ],
    }
}

/// Build E12 Late Interaction stress test corpus.
///
/// E12 captures token-level precision for word order and exact phrases.
/// E1 uses pooled embeddings that lose word order information.
pub fn build_e12_late_interaction_corpus() -> EmbedderStressConfig {
    EmbedderStressConfig {
        embedder: EmbedderIndex::E12LateInteraction,
        name: "E12 Late Interaction",
        description: "Token-level precision for word order and exact phrases",
        corpus: vec![
            StressCorpusEntry {
                content: "The cat sat on the mat and watched the birds outside.".into(),
                doc_id: 0,
                e1_limitation: Some("Correct word order".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The mat sat on the cat while birds watched outside.".into(),
                doc_id: 1,
                e1_limitation: Some("Wrong word order - semantic nonsense".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "A feline rested upon the rug and observed the avians.".into(),
                doc_id: 2,
                e1_limitation: Some("Paraphrase with different tokens".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "SELECT name, email FROM users WHERE active = true ORDER BY created_at DESC".into(),
                doc_id: 3,
                e1_limitation: Some("SQL clause order matters".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "FROM users SELECT email, name WHERE true = active DESC ORDER BY created_at".into(),
                doc_id: 4,
                e1_limitation: Some("Invalid SQL - wrong clause order".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "kill -9 process_id".into(),
                doc_id: 5,
                e1_limitation: Some("Command argument order".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "process_id -9 kill".into(),
                doc_id: 6,
                e1_limitation: Some("Invalid command order".into()),
                metadata: None,
            },
        ],
        queries: vec![
            StressQuery {
                query: "cat sitting on mat".into(),
                target_embedder: EmbedderIndex::E12LateInteraction,
                expected_top_docs: vec![0],
                anti_expected_docs: vec![1],
                e1_failure_reason: "E1 pooled embedding loses word order".into(),
            },
            StressQuery {
                query: "SELECT FROM WHERE ORDER BY".into(),
                target_embedder: EmbedderIndex::E12LateInteraction,
                expected_top_docs: vec![3],
                anti_expected_docs: vec![4],
                e1_failure_reason: "E1 treats both as SQL-related".into(),
            },
            StressQuery {
                query: "kill signal command".into(),
                target_embedder: EmbedderIndex::E12LateInteraction,
                expected_top_docs: vec![5],
                anti_expected_docs: vec![6],
                e1_failure_reason: "E1 sees same tokens regardless of order".into(),
            },
        ],
    }
}

/// Build E13 SPLADE stress test corpus.
///
/// E13 captures learned term expansion for implicit vocabulary.
/// E1 requires exact semantic overlap.
pub fn build_e13_splade_corpus() -> EmbedderStressConfig {
    EmbedderStressConfig {
        embedder: EmbedderIndex::E13Splade,
        name: "E13 SPLADE",
        description: "Learned term expansion for implicit vocabulary",
        corpus: vec![
            StressCorpusEntry {
                content: "The HTTP server handles incoming web requests using a thread pool for concurrent processing.".into(),
                doc_id: 0,
                e1_limitation: Some("SPLADE expands REST API to HTTP server".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The application receives network traffic and routes it to appropriate handlers.".into(),
                doc_id: 1,
                e1_limitation: Some("Generic network terms".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The service processes client connections using async I/O for scalability.".into(),
                doc_id: 2,
                e1_limitation: Some("Different terminology for same concept".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "The ML model predicts house prices based on square footage, location, and number of bedrooms.".into(),
                doc_id: 3,
                e1_limitation: Some("SPLADE expands real estate to house prices".into()),
                metadata: None,
            },
            StressCorpusEntry {
                content: "Property valuation uses multiple features including size and geographic data.".into(),
                doc_id: 4,
                e1_limitation: Some("Different vocabulary for same domain".into()),
                metadata: None,
            },
        ],
        queries: vec![
            StressQuery {
                query: "REST API endpoint".into(),
                target_embedder: EmbedderIndex::E13Splade,
                expected_top_docs: vec![0],
                anti_expected_docs: vec![],
                e1_failure_reason: "No REST or API in corpus but SPLADE expands to HTTP server".into(),
            },
            StressQuery {
                query: "real estate price prediction".into(),
                target_embedder: EmbedderIndex::E13Splade,
                expected_top_docs: vec![3],
                anti_expected_docs: vec![],
                e1_failure_reason: "SPLADE learns real estate maps to house prices".into(),
            },
        ],
    }
}

/// Get all embedder stress test configurations.
pub fn get_all_stress_configs() -> Vec<EmbedderStressConfig> {
    vec![
        build_e5_causal_corpus(),
        build_e6_sparse_corpus(),
        build_e7_code_corpus(),
        build_e8_graph_corpus(),
        build_e9_hdc_corpus(),
        build_e10_multimodal_corpus(),
        build_e11_entity_corpus(),
        build_e12_late_interaction_corpus(),
        build_e13_splade_corpus(),
    ]
}

/// Get stress config for a specific embedder.
pub fn get_stress_config(embedder: EmbedderIndex) -> Option<EmbedderStressConfig> {
    get_all_stress_configs()
        .into_iter()
        .find(|c| c.embedder == embedder)
}

/// Map from embedder index to stress config.
pub fn stress_config_map() -> HashMap<EmbedderIndex, EmbedderStressConfig> {
    get_all_stress_configs()
        .into_iter()
        .map(|c| (c.embedder, c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_configs_have_queries() {
        for config in get_all_stress_configs() {
            assert!(
                !config.queries.is_empty(),
                "Config for {:?} has no queries",
                config.embedder
            );
            assert!(
                !config.corpus.is_empty(),
                "Config for {:?} has no corpus",
                config.embedder
            );
            println!(
                "[VERIFIED] {:?} has {} corpus entries and {} queries",
                config.embedder,
                config.corpus.len(),
                config.queries.len()
            );
        }
    }

    #[test]
    fn test_expected_docs_exist_in_corpus() {
        for config in get_all_stress_configs() {
            let max_doc_id = config.corpus.len();
            for query in &config.queries {
                for &doc_id in &query.expected_top_docs {
                    assert!(
                        doc_id < max_doc_id,
                        "Query '{}' expects doc {} but corpus only has {} docs",
                        query.query,
                        doc_id,
                        max_doc_id
                    );
                }
                for &doc_id in &query.anti_expected_docs {
                    assert!(
                        doc_id < max_doc_id,
                        "Query '{}' anti-expects doc {} but corpus only has {} docs",
                        query.query,
                        doc_id,
                        max_doc_id
                    );
                }
            }
        }
        println!("[VERIFIED] All expected doc IDs exist in their corpus");
    }

    #[test]
    fn test_embedder_coverage() {
        let configs = stress_config_map();

        // These embedders should have stress tests
        let expected_embedders = [
            EmbedderIndex::E5Causal,
            EmbedderIndex::E6Sparse,
            EmbedderIndex::E7Code,
            EmbedderIndex::E8Graph,
            EmbedderIndex::E9HDC,
            EmbedderIndex::E10Multimodal,
            EmbedderIndex::E11Entity,
            EmbedderIndex::E12LateInteraction,
            EmbedderIndex::E13Splade,
        ];

        for embedder in expected_embedders {
            assert!(
                configs.contains_key(&embedder),
                "Missing stress test for {:?}",
                embedder
            );
        }
        println!("[VERIFIED] All expected embedders have stress tests");
    }
}
