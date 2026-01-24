//! E11 Entity Embedder Benchmark Dataset.
//!
//! This module provides dataset loading and processing for benchmarking
//! the E11 entity embedder (all-MiniLM-L6-v2, 384D) and its MCP tools.
//!
//! E11 is a RELATIONAL_ENHANCER per ARCH-12. This benchmark validates:
//! - Entity extraction accuracy (precision, recall, F1)
//! - Entity canonicalization (postgres → postgresql)
//! - TransE relationship inference (valid vs invalid triples)
//! - Entity-based retrieval (E11 contribution over E1-only)
//!
//! ## Target Metrics (from E11 Design Document)
//!
//! - Entity extraction F1: >= 0.85
//! - TransE valid triple score: > -0.5
//! - TransE invalid triple score: < -1.5
//! - E11 contribution (vs E1-only): >= +5%
//! - Entity search latency: < 100ms

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_core::entity::{detect_entities, EntityLink, EntityMetadata, EntityType};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for E11 entity benchmark dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E11EntityDatasetConfig {
    /// Maximum chunks to load (0 = unlimited).
    pub max_chunks: usize,
    /// Random seed for reproducibility.
    pub seed: u64,
    /// Minimum entities per document for entity benchmarks.
    pub min_entities_per_doc: usize,
    /// Number of TransE valid triples to generate.
    pub num_valid_triples: usize,
    /// Number of TransE invalid triples to generate.
    pub num_invalid_triples: usize,
    /// Number of entity pairs for relationship inference testing.
    pub num_entity_pairs: usize,
}

impl Default for E11EntityDatasetConfig {
    fn default() -> Self {
        Self {
            max_chunks: 0,
            seed: 42,
            min_entities_per_doc: 1,
            num_valid_triples: 100,
            num_invalid_triples: 100,
            num_entity_pairs: 100,
        }
    }
}

// ============================================================================
// Entity Document
// ============================================================================

/// A document with extracted entities for E11 benchmarking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDocument {
    /// Unique document ID.
    pub id: Uuid,
    /// Document text content.
    pub text: String,
    /// Topic hint from source dataset.
    pub topic_hint: String,
    /// Source dataset (e.g., "arxiv", "stackoverflow").
    pub source_dataset: Option<String>,
    /// Extracted entities from this document.
    pub entities: Vec<EntityLinkSerializable>,
    /// Ground truth entity count (for extraction benchmarks).
    pub ground_truth_entity_count: usize,
}

/// Serializable version of EntityLink.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EntityLinkSerializable {
    /// Raw entity text as extracted from content.
    pub surface_form: String,
    /// Canonical entity identifier (lowercase, normalized).
    pub canonical_id: String,
    /// Entity type category.
    pub entity_type: String,
}

impl From<&EntityLink> for EntityLinkSerializable {
    fn from(link: &EntityLink) -> Self {
        Self {
            surface_form: link.surface_form.clone(),
            canonical_id: link.canonical_id.clone(),
            entity_type: entity_type_to_string(link.entity_type),
        }
    }
}

impl From<EntityLink> for EntityLinkSerializable {
    fn from(link: EntityLink) -> Self {
        Self {
            surface_form: link.surface_form,
            canonical_id: link.canonical_id,
            entity_type: entity_type_to_string(link.entity_type),
        }
    }
}

fn entity_type_to_string(et: EntityType) -> String {
    match et {
        EntityType::ProgrammingLanguage => "ProgrammingLanguage".to_string(),
        EntityType::Framework => "Framework".to_string(),
        EntityType::Database => "Database".to_string(),
        EntityType::Cloud => "Cloud".to_string(),
        EntityType::Company => "Company".to_string(),
        EntityType::TechnicalTerm => "TechnicalTerm".to_string(),
        EntityType::Unknown => "Unknown".to_string(),
    }
}

// ============================================================================
// Knowledge Triples
// ============================================================================

/// A knowledge triple (subject, predicate, object) for TransE validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeTriple {
    /// Subject entity (head).
    pub subject: String,
    /// Predicate (relation).
    pub predicate: String,
    /// Object entity (tail).
    pub object: String,
    /// Whether this triple is valid (ground truth).
    pub is_valid: bool,
    /// Source of this triple (e.g., "KB", "document").
    pub source: String,
}

impl KnowledgeTriple {
    /// Create a valid triple.
    pub fn valid(subject: impl Into<String>, predicate: impl Into<String>, object: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            is_valid: true,
            source: source.into(),
        }
    }

    /// Create an invalid triple.
    pub fn invalid(subject: impl Into<String>, predicate: impl Into<String>, object: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            is_valid: false,
            source: source.into(),
        }
    }
}

// ============================================================================
// Entity Pairs for Relationship Inference
// ============================================================================

/// An entity pair for relationship inference testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityPair {
    /// Head entity.
    pub head: String,
    /// Tail entity.
    pub tail: String,
    /// Expected relation (ground truth, if known).
    pub expected_relation: Option<String>,
    /// Confidence in expected relation (1.0 = certain).
    pub confidence: f32,
    /// Source document where these co-occur.
    pub source_doc_id: Option<Uuid>,
}

// ============================================================================
// Ground Truth
// ============================================================================

/// Ground truth for E11 entity benchmark.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityGroundTruth {
    /// All known entities from KB.
    pub known_entities: HashSet<String>,
    /// Entity type mapping: canonical_id -> entity_type.
    pub entity_types: HashMap<String, String>,
    /// Canonicalization mapping: surface_form -> canonical_id.
    pub canonicalization: HashMap<String, String>,
    /// Valid knowledge triples.
    pub valid_triples: Vec<KnowledgeTriple>,
    /// Invalid knowledge triples (for contrast).
    pub invalid_triples: Vec<KnowledgeTriple>,
    /// Entity pairs with known relationships.
    pub entity_pairs: Vec<EntityPair>,
    /// Documents by entity: canonical_id -> Vec<doc_id>.
    pub docs_by_entity: HashMap<String, Vec<Uuid>>,
}

// ============================================================================
// Dataset
// ============================================================================

/// Complete E11 entity benchmark dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E11EntityBenchmarkDataset {
    /// Documents with extracted entities.
    pub documents: Vec<EntityDocument>,
    /// Ground truth information.
    pub ground_truth: EntityGroundTruth,
    /// Configuration used.
    pub config: E11EntityDatasetConfig,
    /// Dataset statistics.
    pub stats: E11EntityDatasetStats,
}

/// Statistics about the E11 entity dataset.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E11EntityDatasetStats {
    /// Total documents.
    pub num_documents: usize,
    /// Documents with at least one entity.
    pub docs_with_entities: usize,
    /// Total entities extracted.
    pub total_entities: usize,
    /// Unique canonical entities.
    pub unique_entities: usize,
    /// Average entities per document.
    pub avg_entities_per_doc: f64,
    /// Entity type distribution.
    pub entity_type_counts: HashMap<String, usize>,
    /// Valid triples count.
    pub num_valid_triples: usize,
    /// Invalid triples count.
    pub num_invalid_triples: usize,
    /// Entity pairs count.
    pub num_entity_pairs: usize,
}

impl E11EntityBenchmarkDataset {
    /// Get document by ID.
    pub fn get_document(&self, id: &Uuid) -> Option<&EntityDocument> {
        self.documents.iter().find(|d| &d.id == id)
    }

    /// Get documents containing a specific entity.
    pub fn documents_with_entity(&self, canonical_id: &str) -> Vec<&EntityDocument> {
        self.documents
            .iter()
            .filter(|d| d.entities.iter().any(|e| e.canonical_id == canonical_id))
            .collect()
    }

    /// Get all unique entities across documents.
    pub fn unique_entities(&self) -> HashSet<String> {
        self.documents
            .iter()
            .flat_map(|d| d.entities.iter().map(|e| e.canonical_id.clone()))
            .collect()
    }

    /// Validate dataset consistency.
    pub fn validate(&self) -> Result<(), String> {
        if self.documents.is_empty() {
            return Err("Dataset has no documents".to_string());
        }

        if self.stats.docs_with_entities == 0 {
            return Err("No documents contain entities".to_string());
        }

        if self.ground_truth.valid_triples.is_empty() && self.ground_truth.invalid_triples.is_empty() {
            return Err("No knowledge triples for TransE validation".to_string());
        }

        Ok(())
    }
}

// ============================================================================
// Dataset Loader
// ============================================================================

/// Loader for E11 entity benchmark datasets from real data.
pub struct E11EntityDatasetLoader {
    config: E11EntityDatasetConfig,
}

impl E11EntityDatasetLoader {
    /// Create a new loader with config.
    pub fn new(config: E11EntityDatasetConfig) -> Self {
        Self { config }
    }

    /// Load dataset from ChunkRecords.
    pub fn load_from_chunks(&self, chunks: &[crate::realdata::ChunkRecord]) -> E11EntityBenchmarkDataset {
        use tracing::{debug, info};

        info!(
            num_chunks = chunks.len(),
            max_chunks = self.config.max_chunks,
            "E11EntityDatasetLoader: Loading dataset from chunks"
        );

        let mut documents = Vec::new();
        let mut ground_truth = EntityGroundTruth::default();
        let mut entity_type_counts: HashMap<String, usize> = HashMap::new();
        let mut total_entities = 0;
        let mut docs_with_entities = 0;

        // Build known entities from KB
        let kb = context_graph_core::entity::get_entity_kb();
        for (surface, (canonical, entity_type)) in kb.iter() {
            ground_truth.known_entities.insert(canonical.to_string());
            ground_truth.entity_types.insert(canonical.to_string(), entity_type_to_string(*entity_type));
            ground_truth.canonicalization.insert(surface.to_string(), canonical.to_string());
        }

        // Process chunks
        let chunks_to_process = if self.config.max_chunks > 0 {
            &chunks[..chunks.len().min(self.config.max_chunks)]
        } else {
            chunks
        };

        for chunk in chunks_to_process {
            let uuid = chunk.uuid();

            // Extract entities from text
            let entity_metadata: EntityMetadata = detect_entities(&chunk.text);
            let entities: Vec<EntityLinkSerializable> = entity_metadata
                .entities
                .iter()
                .map(EntityLinkSerializable::from)
                .collect();

            // Skip documents with too few entities if configured
            if entities.len() < self.config.min_entities_per_doc {
                continue;
            }

            // Update entity type counts
            for entity in &entities {
                *entity_type_counts.entry(entity.entity_type.clone()).or_insert(0) += 1;

                // Track docs by entity
                ground_truth
                    .docs_by_entity
                    .entry(entity.canonical_id.clone())
                    .or_default()
                    .push(uuid);
            }

            total_entities += entities.len();
            if !entities.is_empty() {
                docs_with_entities += 1;
            }

            documents.push(EntityDocument {
                id: uuid,
                text: chunk.text.clone(),
                topic_hint: chunk.topic_hint.clone(),
                source_dataset: chunk.source_dataset.clone(),
                ground_truth_entity_count: entities.len(),
                entities,
            });
        }

        debug!(
            documents = documents.len(),
            docs_with_entities = docs_with_entities,
            total_entities = total_entities,
            "E11EntityDatasetLoader: Extracted entities from documents"
        );

        // Generate knowledge triples
        self.generate_triples(&documents, &mut ground_truth);

        // Generate entity pairs for relationship inference
        self.generate_entity_pairs(&documents, &mut ground_truth);

        let unique_entities = documents
            .iter()
            .flat_map(|d| d.entities.iter().map(|e| e.canonical_id.clone()))
            .collect::<HashSet<_>>()
            .len();

        let avg_entities_per_doc = if !documents.is_empty() {
            total_entities as f64 / documents.len() as f64
        } else {
            0.0
        };

        let stats = E11EntityDatasetStats {
            num_documents: documents.len(),
            docs_with_entities,
            total_entities,
            unique_entities,
            avg_entities_per_doc,
            entity_type_counts,
            num_valid_triples: ground_truth.valid_triples.len(),
            num_invalid_triples: ground_truth.invalid_triples.len(),
            num_entity_pairs: ground_truth.entity_pairs.len(),
        };

        info!(
            num_documents = stats.num_documents,
            docs_with_entities = stats.docs_with_entities,
            total_entities = stats.total_entities,
            unique_entities = stats.unique_entities,
            valid_triples = stats.num_valid_triples,
            invalid_triples = stats.num_invalid_triples,
            entity_pairs = stats.num_entity_pairs,
            "E11EntityDatasetLoader: Dataset loaded successfully"
        );

        E11EntityBenchmarkDataset {
            documents,
            ground_truth,
            config: self.config.clone(),
            stats,
        }
    }

    /// Generate valid and invalid knowledge triples from documents and KB.
    fn generate_triples(&self, documents: &[EntityDocument], ground_truth: &mut EntityGroundTruth) {
        use rand::prelude::*;
        use rand_chacha::ChaCha8Rng;

        let mut rng = ChaCha8Rng::seed_from_u64(self.config.seed);

        // Known valid relations from technical domain
        let known_valid_triples = vec![
            // Programming language relations
            KnowledgeTriple::valid("Tokio", "depends_on", "Rust", "KB"),
            KnowledgeTriple::valid("Axum", "depends_on", "Tokio", "KB"),
            KnowledgeTriple::valid("Actix", "depends_on", "Tokio", "KB"),
            KnowledgeTriple::valid("Django", "uses", "Python", "KB"),
            KnowledgeTriple::valid("FastAPI", "uses", "Python", "KB"),
            KnowledgeTriple::valid("Flask", "uses", "Python", "KB"),
            KnowledgeTriple::valid("React", "uses", "JavaScript", "KB"),
            KnowledgeTriple::valid("Vue.js", "uses", "JavaScript", "KB"),
            KnowledgeTriple::valid("Angular", "uses", "TypeScript", "KB"),
            KnowledgeTriple::valid("Next.js", "extends", "React", "KB"),

            // Database relations
            KnowledgeTriple::valid("PostgreSQL", "is_type", "Database", "KB"),
            KnowledgeTriple::valid("MySQL", "is_type", "Database", "KB"),
            KnowledgeTriple::valid("MongoDB", "is_type", "Database", "KB"),
            KnowledgeTriple::valid("Redis", "is_type", "Database", "KB"),
            KnowledgeTriple::valid("RocksDB", "is_type", "Database", "KB"),

            // Cloud relations
            KnowledgeTriple::valid("S3", "part_of", "AWS", "KB"),
            KnowledgeTriple::valid("EC2", "part_of", "AWS", "KB"),
            KnowledgeTriple::valid("Lambda", "part_of", "AWS", "KB"),
            KnowledgeTriple::valid("BigQuery", "part_of", "GCP", "KB"),
            KnowledgeTriple::valid("Kubernetes", "alternative_to", "Docker", "KB"),

            // Company relations
            KnowledgeTriple::valid("Claude", "created_by", "Anthropic", "KB"),
            KnowledgeTriple::valid("GPT", "created_by", "OpenAI", "KB"),
            KnowledgeTriple::valid("GitHub", "owned_by", "Microsoft", "KB"),

            // Technical term relations
            KnowledgeTriple::valid("GraphQL", "alternative_to", "REST", "KB"),
            KnowledgeTriple::valid("gRPC", "alternative_to", "REST", "KB"),
            KnowledgeTriple::valid("ColBERT", "is_type", "TechnicalTerm", "KB"),
            KnowledgeTriple::valid("SPLADE", "is_type", "TechnicalTerm", "KB"),
            KnowledgeTriple::valid("HNSW", "is_type", "TechnicalTerm", "KB"),
        ];

        // Invalid relations (semantically incorrect)
        let known_invalid_triples = vec![
            KnowledgeTriple::invalid("PostgreSQL", "works_at", "Google", "synthetic"),
            KnowledgeTriple::invalid("Rust", "created_by", "JavaScript", "synthetic"),
            KnowledgeTriple::invalid("AWS", "extends", "Python", "synthetic"),
            KnowledgeTriple::invalid("Django", "depends_on", "React", "synthetic"),
            KnowledgeTriple::invalid("MongoDB", "uses", "PostgreSQL", "synthetic"),
            KnowledgeTriple::invalid("Redis", "alternative_to", "Rust", "synthetic"),
            KnowledgeTriple::invalid("Tokio", "part_of", "Python", "synthetic"),
            KnowledgeTriple::invalid("GPT", "created_by", "Anthropic", "synthetic"),
            KnowledgeTriple::invalid("Claude", "created_by", "OpenAI", "synthetic"),
            KnowledgeTriple::invalid("Kubernetes", "depends_on", "FastAPI", "synthetic"),
        ];

        ground_truth.valid_triples.extend(known_valid_triples);
        ground_truth.invalid_triples.extend(known_invalid_triples);

        // Generate additional triples from document co-occurrences
        let all_entities: Vec<String> = documents
            .iter()
            .flat_map(|d| d.entities.iter().map(|e| e.canonical_id.clone()))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        // Generate more invalid triples by random pairing
        let relations = vec![
            "depends_on", "uses", "extends", "part_of", "created_by",
            "alternative_to", "works_at", "implements",
        ];

        let mut invalid_count = ground_truth.invalid_triples.len();
        while invalid_count < self.config.num_invalid_triples && all_entities.len() >= 2 {
            let head_idx = rng.gen_range(0..all_entities.len());
            let tail_idx = rng.gen_range(0..all_entities.len());
            let rel_idx = rng.gen_range(0..relations.len());

            if head_idx != tail_idx {
                let triple = KnowledgeTriple::invalid(
                    &all_entities[head_idx],
                    relations[rel_idx],
                    &all_entities[tail_idx],
                    "random",
                );

                // Check it's not accidentally valid
                let is_duplicate = ground_truth.valid_triples.iter().any(|t| {
                    t.subject == triple.subject && t.predicate == triple.predicate && t.object == triple.object
                });

                if !is_duplicate {
                    ground_truth.invalid_triples.push(triple);
                    invalid_count += 1;
                }
            }
        }
    }

    /// Generate entity pairs for relationship inference testing.
    fn generate_entity_pairs(&self, documents: &[EntityDocument], ground_truth: &mut EntityGroundTruth) {
        use rand::prelude::*;
        use rand_chacha::ChaCha8Rng;

        let mut rng = ChaCha8Rng::seed_from_u64(self.config.seed + 1);

        // Find entity pairs that co-occur in documents
        for doc in documents {
            let entities = &doc.entities;
            if entities.len() < 2 {
                continue;
            }

            // Generate pairs from co-occurring entities
            for i in 0..entities.len() {
                for j in (i + 1)..entities.len() {
                    // Check if this pair has a known relation
                    let head = &entities[i].canonical_id;
                    let tail = &entities[j].canonical_id;

                    // Look for known relation
                    let known_relation = ground_truth.valid_triples.iter()
                        .find(|t| &t.subject.to_lowercase() == head && &t.object.to_lowercase() == tail)
                        .map(|t| t.predicate.clone());

                    if ground_truth.entity_pairs.len() < self.config.num_entity_pairs {
                        ground_truth.entity_pairs.push(EntityPair {
                            head: head.clone(),
                            tail: tail.clone(),
                            expected_relation: known_relation,
                            confidence: if ground_truth.valid_triples.iter().any(|t| {
                                t.subject.to_lowercase() == *head && t.object.to_lowercase() == *tail
                            }) { 1.0 } else { 0.5 },
                            source_doc_id: Some(doc.id),
                        });
                    }
                }
            }

            if ground_truth.entity_pairs.len() >= self.config.num_entity_pairs {
                break;
            }
        }

        // Shuffle for randomness
        ground_truth.entity_pairs.shuffle(&mut rng);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_chunk(id: &str, text: &str, topic: &str) -> crate::realdata::ChunkRecord {
        crate::realdata::ChunkRecord {
            id: id.to_string(),
            doc_id: format!("doc_{}", id),
            title: format!("Test {}", id),
            chunk_idx: 0,
            text: text.to_string(),
            word_count: text.split_whitespace().count(),
            start_word: 0,
            end_word: text.split_whitespace().count(),
            topic_hint: topic.to_string(),
            source_dataset: Some("test".to_string()),
        }
    }

    #[test]
    fn test_dataset_loading() {
        let chunks = vec![
            create_test_chunk(
                "00000000-0000-0000-0000-000000000001",
                "Using Rust and Tokio for async programming with PostgreSQL database",
                "programming",
            ),
            create_test_chunk(
                "00000000-0000-0000-0000-000000000002",
                "Deploying to AWS with Kubernetes and Docker containers",
                "infrastructure",
            ),
            create_test_chunk(
                "00000000-0000-0000-0000-000000000003",
                "Building React frontend with Next.js and TypeScript",
                "frontend",
            ),
        ];

        let config = E11EntityDatasetConfig {
            min_entities_per_doc: 0,
            num_valid_triples: 10,
            num_invalid_triples: 10,
            num_entity_pairs: 10,
            ..Default::default()
        };

        let loader = E11EntityDatasetLoader::new(config);
        let dataset = loader.load_from_chunks(&chunks);

        assert_eq!(dataset.documents.len(), 3);
        assert!(dataset.stats.total_entities > 0);
        assert!(dataset.stats.unique_entities > 0);
        assert!(!dataset.ground_truth.valid_triples.is_empty());
        assert!(!dataset.ground_truth.invalid_triples.is_empty());
    }

    #[test]
    fn test_entity_extraction() {
        let text = "Rust is a systems programming language. Tokio provides async runtime.";
        let entities = detect_entities(text);

        let canonical_ids: HashSet<_> = entities.canonical_ids().into_iter().collect();
        assert!(canonical_ids.contains("rust_language"), "Should detect Rust");
        assert!(canonical_ids.contains("tokio"), "Should detect Tokio");
    }

    #[test]
    fn test_knowledge_triple_creation() {
        let valid = KnowledgeTriple::valid("Tokio", "depends_on", "Rust", "KB");
        assert!(valid.is_valid);
        assert_eq!(valid.subject, "Tokio");
        assert_eq!(valid.predicate, "depends_on");
        assert_eq!(valid.object, "Rust");

        let invalid = KnowledgeTriple::invalid("PostgreSQL", "works_at", "Google", "synthetic");
        assert!(!invalid.is_valid);
    }

    #[test]
    fn test_dataset_validation() {
        let config = E11EntityDatasetConfig::default();
        let loader = E11EntityDatasetLoader::new(config);

        // Empty dataset should fail validation
        let empty_dataset = E11EntityBenchmarkDataset {
            documents: vec![],
            ground_truth: EntityGroundTruth::default(),
            config: E11EntityDatasetConfig::default(),
            stats: E11EntityDatasetStats::default(),
        };

        assert!(empty_dataset.validate().is_err());
    }

    #[test]
    fn test_entity_type_conversion() {
        assert_eq!(entity_type_to_string(EntityType::ProgrammingLanguage), "ProgrammingLanguage");
        assert_eq!(entity_type_to_string(EntityType::Database), "Database");
        assert_eq!(entity_type_to_string(EntityType::Framework), "Framework");
        assert_eq!(entity_type_to_string(EntityType::Unknown), "Unknown");
    }
}
