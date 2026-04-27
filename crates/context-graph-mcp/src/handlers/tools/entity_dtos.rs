//! DTOs for E11 Entity MCP tools.
//!
//! Per E11 Design Document, these DTOs support:
//! - extract_entities: Extract and canonicalize entities from text
//! - search_by_entities: Find memories containing specific entities
//! - infer_relationship: Infer relationship between two entities using TransE
//! - find_related_entities: Find entities with given relationship
//! - validate_knowledge: Score a (subject, predicate, object) triple
//! - get_entity_graph: Visualize entity relationships in memory
//!
//! ## KEPLER Model (E11)
//!
//! E11 uses KEPLER (RoBERTa-base + TransE on Wikidata5M, 768D) for entity embeddings.
//! KEPLER was trained with TransE objective on 4.8M entities and 20M triples, making
//! TransE operations (h + r ≈ t) semantically meaningful.
//!
//! TransE Score Ranges (KEPLER):
//! - Valid triples: typically score > -5.0
//! - Uncertain triples: -10.0 <= score <= -5.0
//! - Invalid triples: typically score < -10.0
//!
//! Constitution References:
//! - ARCH-12: E1 is the semantic foundation, E11 enhances
//! - ARCH-20: E11 SHOULD use entity linking for disambiguation
//! - E11 is RELATIONAL_ENHANCER with topic_weight 0.5
//! - Delta_S method: TransE ||h+r-t||

use context_graph_core::entity::{EntityLink, EntityType};
use context_graph_core::traits::SearchStrategy;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default topK for entity search results.
pub const DEFAULT_ENTITY_SEARCH_TOP_K: usize = 10;

/// Maximum topK for entity search results.
pub const MAX_ENTITY_SEARCH_TOP_K: usize = 50;

/// Default minimum score threshold for entity search results.
pub const DEFAULT_MIN_ENTITY_SCORE: f32 = 0.2;

/// Default boost for exact entity matches.
pub const DEFAULT_EXACT_MATCH_BOOST: f32 = 1.15;

/// TransE score threshold for "valid" triples (KEPLER calibrated).
/// KEPLER valid triples: typically score > -5.0
pub const VALID_THRESHOLD: f32 = -5.0;

/// TransE score threshold for "invalid" triples (KEPLER calibrated).
/// KEPLER invalid triples: typically score < -10.0
/// Score between -10.0 and -5.0 is uncertain.
pub const UNCERTAIN_THRESHOLD: f32 = -10.0;

// ============================================================================
// TRAIT IMPLS (parse_request helper)
// ============================================================================

impl super::validate::Validate for ExtractEntitiesRequest {
    fn validate(&self) -> Result<(), String> {
        self.validate()
    }
}

impl super::validate::Validate for SearchByEntitiesRequest {
    fn validate(&self) -> Result<(), String> {
        self.validate()
    }
}

impl super::validate::Validate for InferRelationshipRequest {
    fn validate(&self) -> Result<(), String> {
        self.validate()
    }
}

impl super::validate::Validate for FindRelatedEntitiesRequest {
    fn validate(&self) -> Result<(), String> {
        self.validate()
    }
}

impl super::validate::Validate for ValidateKnowledgeRequest {
    fn validate(&self) -> Result<(), String> {
        self.validate()
    }
}

impl super::validate::Validate for GetEntityGraphRequest {
    fn validate(&self) -> Result<(), String> {
        self.validate()
    }
}

// ============================================================================
// EXTRACT_ENTITIES DTOs
// ============================================================================

/// Request parameters for extract_entities tool.
///
/// Extracts and canonicalizes entities from text using pattern matching
/// and knowledge base lookup.
///
/// # Example JSON
/// ```json
/// {
///   "text": "Building a web server with Rust and PostgreSQL",
///   "includeUnknown": true,
///   "groupByType": false
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ExtractEntitiesRequest {
    /// Text to extract entities from (required).
    pub text: String,

    /// Whether to include entities not in knowledge base (default: true).
    #[serde(rename = "includeUnknown", default = "default_true")]
    pub include_unknown: bool,

    /// Whether to group results by entity type (default: false).
    #[serde(rename = "groupByType", default)]
    pub group_by_type: bool,
}

fn default_true() -> bool {
    true
}

impl ExtractEntitiesRequest {
    /// Validate the request parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.text.is_empty() {
            return Err("text is required and cannot be empty".to_string());
        }
        Ok(())
    }
}

/// Response for extract_entities tool.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractEntitiesResponse {
    /// All extracted entities with canonical links.
    pub entities: Vec<EntityLinkDto>,

    /// Entities grouped by type (if groupByType=true).
    #[serde(rename = "byType", skip_serializing_if = "Option::is_none")]
    pub by_type: Option<EntityByType>,

    /// Total number of entities extracted.
    #[serde(rename = "totalCount")]
    pub total_count: usize,
}

/// Entity link DTO for JSON serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityLinkDto {
    /// Raw entity text as found in content.
    #[serde(rename = "surfaceForm")]
    pub surface_form: String,

    /// Canonical entity identifier (normalized).
    #[serde(rename = "canonicalId")]
    pub canonical_id: String,

    /// Entity type category.
    #[serde(rename = "entityType")]
    pub entity_type: String,

    /// Confidence score (1.0 for KB matches, lower for heuristic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,

    /// How this entity was extracted: "knowledgeBase" or "heuristic".
    #[serde(rename = "extractionMethod", skip_serializing_if = "Option::is_none")]
    pub extraction_method: Option<String>,

    /// Human-readable explanation of the confidence score.
    #[serde(
        rename = "confidenceExplanation",
        skip_serializing_if = "Option::is_none"
    )]
    pub confidence_explanation: Option<String>,
}

impl EntityLinkDto {
    /// Create a fallback DTO when entity extraction yields no KB match.
    ///
    /// Used when the entity text doesn't match the knowledge base but we still
    /// need a valid DTO for the API response.
    pub fn fallback(surface_form: String) -> Self {
        Self {
            canonical_id: surface_form.to_lowercase().replace(' ', "_"),
            surface_form,
            entity_type: "Unknown".to_string(),
            confidence: Some(0.5),
            extraction_method: Some("heuristic".to_string()),
            confidence_explanation: Some("Entity not found in knowledge base".to_string()),
        }
    }
}

impl From<EntityLink> for EntityLinkDto {
    fn from(link: EntityLink) -> Self {
        let (method, explanation) = if link.entity_type != EntityType::Unknown {
            (
                "knowledgeBase".to_string(),
                "Matched against built-in knowledge base".to_string(),
            )
        } else {
            (
                "heuristic".to_string(),
                "Detected via capitalization/pattern heuristics".to_string(),
            )
        };
        Self {
            surface_form: link.surface_form,
            canonical_id: link.canonical_id,
            entity_type: entity_type_to_string(link.entity_type),
            confidence: Some(link.confidence),
            extraction_method: Some(method),
            confidence_explanation: Some(explanation),
        }
    }
}

impl From<&EntityLink> for EntityLinkDto {
    fn from(link: &EntityLink) -> Self {
        let (method, explanation) = if link.entity_type != EntityType::Unknown {
            (
                "knowledgeBase".to_string(),
                "Matched against built-in knowledge base".to_string(),
            )
        } else {
            (
                "heuristic".to_string(),
                "Detected via capitalization/pattern heuristics".to_string(),
            )
        };
        Self {
            surface_form: link.surface_form.clone(),
            canonical_id: link.canonical_id.clone(),
            entity_type: entity_type_to_string(link.entity_type),
            confidence: Some(link.confidence),
            extraction_method: Some(method),
            confidence_explanation: Some(explanation),
        }
    }
}

/// Convert EntityType enum to string for JSON serialization.
pub fn entity_type_to_string(entity_type: EntityType) -> String {
    match entity_type {
        EntityType::ProgrammingLanguage => "ProgrammingLanguage".to_string(),
        EntityType::Framework => "Framework".to_string(),
        EntityType::Database => "Database".to_string(),
        EntityType::Cloud => "Cloud".to_string(),
        EntityType::Company => "Company".to_string(),
        EntityType::TechnicalTerm => "TechnicalTerm".to_string(),
        EntityType::Unknown => "Unknown".to_string(),
    }
}

/// Entities grouped by type.
#[derive(Debug, Clone, Default, Serialize)]
pub struct EntityByType {
    #[serde(rename = "ProgrammingLanguage", skip_serializing_if = "Vec::is_empty")]
    pub programming_language: Vec<EntityLinkDto>,

    #[serde(rename = "Framework", skip_serializing_if = "Vec::is_empty")]
    pub framework: Vec<EntityLinkDto>,

    #[serde(rename = "Database", skip_serializing_if = "Vec::is_empty")]
    pub database: Vec<EntityLinkDto>,

    #[serde(rename = "Cloud", skip_serializing_if = "Vec::is_empty")]
    pub cloud: Vec<EntityLinkDto>,

    #[serde(rename = "Company", skip_serializing_if = "Vec::is_empty")]
    pub company: Vec<EntityLinkDto>,

    #[serde(rename = "TechnicalTerm", skip_serializing_if = "Vec::is_empty")]
    pub technical_term: Vec<EntityLinkDto>,

    #[serde(rename = "Unknown", skip_serializing_if = "Vec::is_empty")]
    pub unknown: Vec<EntityLinkDto>,
}

// ============================================================================
// SEARCH_BY_ENTITIES DTOs
// ============================================================================

/// Request parameters for search_by_entities tool.
///
/// Finds memories containing specific entities with entity-aware ranking.
///
/// # Example JSON
/// ```json
/// {
///   "entities": ["PostgreSQL", "Rust"],
///   "matchMode": "any",
///   "topK": 10,
///   "minScore": 0.2,
///   "includeContent": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct SearchByEntitiesRequest {
    /// Entity names to search for (required).
    pub entities: Vec<String>,

    /// Filter by entity types (optional, reserved for future use).
    #[serde(rename = "entityTypes")]
    pub _entity_types: Option<Vec<String>>,

    /// Match mode: "any" or "all" (default: "any").
    #[serde(rename = "matchMode", default = "default_match_mode")]
    pub match_mode: String,

    /// Maximum number of results (1-50, default: 10).
    #[serde(rename = "topK", default = "default_top_k")]
    pub top_k: usize,

    /// Minimum similarity score threshold (0-1, default: 0.2).
    #[serde(rename = "minScore", default = "default_min_score")]
    pub min_score: f32,

    /// Whether to include full content text (default: false).
    #[serde(rename = "includeContent", default)]
    pub include_content: bool,

    /// Boost for exact entity matches (default: 1.15).
    #[serde(rename = "boostExactMatch", default = "default_exact_match_boost")]
    pub boost_exact_match: f32,

    /// Search strategy for retrieval.
    /// - "multi_space": Default multi-embedder fusion
    /// - "pipeline": E13 SPLADE recall → E1 → E12 ColBERT rerank (maximum precision)
    ///
    /// When "pipeline" is selected, E12 reranking is automatically enabled.
    #[serde(default)]
    pub strategy: Option<String>,
}

impl SearchByEntitiesRequest {
    /// Parse the strategy parameter into SearchStrategy enum.
    /// Returns E1Only by default (for entity search we use E1+E11 union, not multi_space).
    /// Pipeline if "pipeline" is specified.
    pub fn parse_strategy(&self) -> SearchStrategy {
        match self.strategy.as_deref() {
            Some("pipeline") => SearchStrategy::Pipeline,
            Some("multi_space") => SearchStrategy::MultiSpace,
            _ => SearchStrategy::E1Only, // Default for entity search
        }
    }
}

fn default_match_mode() -> String {
    "any".to_string()
}

fn default_top_k() -> usize {
    DEFAULT_ENTITY_SEARCH_TOP_K
}

fn default_min_score() -> f32 {
    DEFAULT_MIN_ENTITY_SCORE
}

fn default_exact_match_boost() -> f32 {
    DEFAULT_EXACT_MATCH_BOOST
}

impl SearchByEntitiesRequest {
    /// Validate the request parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.entities.is_empty() {
            return Err("entities array is required and cannot be empty".to_string());
        }

        if self.top_k < 1 || self.top_k > MAX_ENTITY_SEARCH_TOP_K {
            return Err(format!(
                "topK must be between 1 and {}, got {}",
                MAX_ENTITY_SEARCH_TOP_K, self.top_k
            ));
        }

        if self.min_score.is_nan() || self.min_score.is_infinite() {
            return Err("minScore must be a finite number".to_string());
        }

        if self.min_score < 0.0 || self.min_score > 1.0 {
            return Err(format!(
                "minScore must be between 0.0 and 1.0, got {}",
                self.min_score
            ));
        }

        if self.match_mode != "any" && self.match_mode != "all" {
            return Err(format!(
                "matchMode must be 'any' or 'all', got '{}'",
                self.match_mode
            ));
        }

        if self.boost_exact_match < 1.0 || self.boost_exact_match > 3.0 {
            return Err(format!(
                "boostExactMatch must be between 1.0 and 3.0, got {}",
                self.boost_exact_match
            ));
        }

        // Validate strategy if provided
        if let Some(ref strat) = self.strategy {
            let valid = ["e1_only", "multi_space", "pipeline"];
            if !valid.contains(&strat.as_str()) {
                return Err(format!(
                    "strategy must be one of {:?}, got '{}'",
                    valid, strat
                ));
            }
        }

        Ok(())
    }
}

/// Response for search_by_entities tool.
#[derive(Debug, Clone, Serialize)]
pub struct SearchByEntitiesResponse {
    /// Search results with entity-aware scores.
    pub results: Vec<EntitySearchResult>,

    /// Entities detected from the query.
    #[serde(rename = "detectedQueryEntities")]
    pub detected_query_entities: Vec<EntityLinkDto>,

    /// Total candidates evaluated before filtering.
    #[serde(rename = "totalCandidates")]
    pub total_candidates: usize,

    /// Search time in milliseconds.
    #[serde(rename = "searchTimeMs")]
    pub search_time_ms: u64,
}

/// A single entity search result.
#[derive(Debug, Clone, Serialize)]
pub struct EntitySearchResult {
    /// UUID of the matched memory.
    #[serde(rename = "memoryId")]
    pub memory_id: Uuid,

    /// Combined score (E11 similarity + entity overlap).
    pub score: f32,

    /// E11 embedding cosine similarity.
    #[serde(rename = "e11Similarity")]
    pub e11_similarity: f32,

    /// Entity Jaccard overlap score.
    #[serde(rename = "entityOverlap")]
    pub entity_overlap: f32,

    /// Entities matched in this memory.
    #[serde(rename = "matchedEntities")]
    pub matched_entities: Vec<EntityLinkDto>,

    /// Full content text (if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

// ============================================================================
// INFER_RELATIONSHIP DTOs
// ============================================================================

/// Request parameters for infer_relationship tool.
///
/// Infers the relationship between two entities using TransE.
///
/// # Example JSON
/// ```json
/// {
///   "headEntity": "Tokio",
///   "tailEntity": "Rust",
///   "topK": 3,
///   "includeScore": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct InferRelationshipRequest {
    /// Subject/head entity (required).
    #[serde(rename = "headEntity")]
    pub head_entity: String,

    /// Object/tail entity (required).
    #[serde(rename = "tailEntity")]
    pub tail_entity: String,

    /// Optional type hint for head entity.
    #[serde(rename = "headType")]
    pub head_type: Option<String>,

    /// Optional type hint for tail entity.
    #[serde(rename = "tailType")]
    pub tail_type: Option<String>,

    /// Top-K relation candidates to return (default: 5).
    #[serde(rename = "topK", default = "default_relation_top_k")]
    pub top_k: usize,

    /// Whether to include raw TransE scores (default: true).
    #[serde(rename = "includeScore", default = "default_true")]
    pub include_score: bool,
}

fn default_relation_top_k() -> usize {
    5
}

impl InferRelationshipRequest {
    /// Validate the request parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.head_entity.is_empty() {
            return Err("headEntity is required and cannot be empty".to_string());
        }

        if self.tail_entity.is_empty() {
            return Err("tailEntity is required and cannot be empty".to_string());
        }

        if self.top_k < 1 || self.top_k > 20 {
            return Err(format!("topK must be between 1 and 20, got {}", self.top_k));
        }

        Ok(())
    }
}

/// Response for infer_relationship tool.
#[derive(Debug, Clone, Serialize)]
pub struct InferRelationshipResponse {
    /// Head entity with canonical link.
    pub head: EntityLinkDto,

    /// Tail entity with canonical link.
    pub tail: EntityLinkDto,

    /// Ranked relation candidates.
    #[serde(rename = "inferredRelations")]
    pub inferred_relations: Vec<RelationCandidate>,

    /// Raw predicted relation vector (if requested).
    #[serde(rename = "predictedVector", skip_serializing_if = "Option::is_none")]
    pub predicted_vector: Option<Vec<f32>>,
}

/// A relation candidate with TransE score.
#[derive(Debug, Clone, Serialize)]
pub struct RelationCandidate {
    /// Relation name (e.g., "depends_on", "implements").
    pub relation: String,

    /// Cosine similarity score (SRC-3 normalized [0,1], higher = better).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,

    /// Normalized confidence [0, 1].
    pub confidence: f32,
}

// ============================================================================
// FIND_RELATED_ENTITIES DTOs
// ============================================================================

/// Request parameters for find_related_entities tool.
///
/// Finds entities that have a given relationship to a source entity.
///
/// # Example JSON
/// ```json
/// {
///   "entity": "Tokio",
///   "relation": "depends_on",
///   "direction": "outgoing",
///   "topK": 10
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct FindRelatedEntitiesRequest {
    /// Source entity (required).
    pub entity: String,

    /// Relationship to find (required).
    pub relation: String,

    /// Direction: "outgoing" (h→t) or "incoming" (t←h) (default: "outgoing").
    #[serde(default = "default_direction")]
    pub direction: String,

    /// Filter result entity types (optional).
    #[serde(rename = "entityType")]
    pub entity_type: Option<String>,

    /// Maximum results (default: 10).
    #[serde(rename = "topK", default = "default_top_k")]
    pub top_k: usize,

    /// Minimum TransE score threshold (default: no threshold).
    #[serde(rename = "minScore")]
    pub min_score: Option<f32>,

    /// Whether to search stored memories (default: true).
    #[serde(rename = "searchMemories", default = "default_true")]
    pub search_memories: bool,
}

fn default_direction() -> String {
    "outgoing".to_string()
}

impl FindRelatedEntitiesRequest {
    /// Validate the request parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.entity.is_empty() {
            return Err("entity is required and cannot be empty".to_string());
        }

        if self.relation.is_empty() {
            return Err("relation is required and cannot be empty".to_string());
        }

        if self.direction != "outgoing" && self.direction != "incoming" {
            return Err(format!(
                "direction must be 'outgoing' or 'incoming', got '{}'",
                self.direction
            ));
        }

        if self.top_k < 1 || self.top_k > MAX_ENTITY_SEARCH_TOP_K {
            return Err(format!(
                "topK must be between 1 and {}, got {}",
                MAX_ENTITY_SEARCH_TOP_K, self.top_k
            ));
        }

        Ok(())
    }
}

/// Response for find_related_entities tool.
#[derive(Debug, Clone, Serialize)]
pub struct FindRelatedEntitiesResponse {
    /// Source entity with canonical link.
    #[serde(rename = "sourceEntity")]
    pub source_entity: EntityLinkDto,

    /// Relationship searched.
    pub relation: String,

    /// Direction searched.
    pub direction: String,

    /// Found related entities.
    #[serde(rename = "relatedEntities")]
    pub related_entities: Vec<RelatedEntity>,

    /// Search time in milliseconds.
    #[serde(rename = "searchTimeMs")]
    pub search_time_ms: u64,
}

/// A related entity result.
#[derive(Debug, Clone, Serialize)]
pub struct RelatedEntity {
    /// The related entity.
    pub entity: EntityLinkDto,

    /// TransE score for the relationship.
    #[serde(rename = "transeScore")]
    pub transe_score: f32,

    /// Whether this entity was found in stored memories.
    #[serde(rename = "foundInMemories")]
    pub found_in_memories: bool,

    /// Memory IDs where this entity appears (if found).
    #[serde(rename = "memoryIds", skip_serializing_if = "Option::is_none")]
    pub memory_ids: Option<Vec<Uuid>>,
}

// ============================================================================
// VALIDATE_KNOWLEDGE DTOs
// ============================================================================

/// Request parameters for validate_knowledge tool.
///
/// Scores whether a (subject, predicate, object) triple is valid using TransE.
///
/// # Example JSON
/// ```json
/// {
///   "subject": "Claude",
///   "predicate": "created_by",
///   "object": "Anthropic"
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ValidateKnowledgeRequest {
    /// Subject/head entity (required).
    pub subject: String,

    /// Predicate/relation (required).
    pub predicate: String,

    /// Object/tail entity (required).
    pub object: String,

    /// Optional type hint for subject.
    #[serde(rename = "subjectType")]
    pub subject_type: Option<String>,

    /// Optional type hint for object.
    #[serde(rename = "objectType")]
    pub object_type: Option<String>,
}

impl ValidateKnowledgeRequest {
    /// Validate the request parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.subject.is_empty() {
            return Err("subject is required and cannot be empty".to_string());
        }

        if self.predicate.is_empty() {
            return Err("predicate is required and cannot be empty".to_string());
        }

        if self.object.is_empty() {
            return Err("object is required and cannot be empty".to_string());
        }

        Ok(())
    }
}

/// Response for validate_knowledge tool.
#[derive(Debug, Clone, Serialize)]
pub struct ValidateKnowledgeResponse {
    /// The validated triple.
    pub triple: KnowledgeTripleDto,

    /// Raw TransE score (0 = perfect, more negative = worse).
    #[serde(rename = "transeScore")]
    pub transe_score: f32,

    /// Normalized confidence [0, 1].
    pub confidence: f32,

    /// Validation result: "valid", "uncertain", or "unlikely".
    pub validation: String,

    /// Memory IDs that support this triple.
    #[serde(rename = "supportingMemories", skip_serializing_if = "Option::is_none")]
    pub supporting_memories: Option<Vec<Uuid>>,

    /// Memory IDs that may contradict this triple.
    #[serde(
        rename = "contradictingMemories",
        skip_serializing_if = "Option::is_none"
    )]
    pub contradicting_memories: Option<Vec<Uuid>>,

    /// Whether confidence/validation were adjusted by memory evidence.
    #[serde(rename = "evidenceAdjusted", skip_serializing_if = "Option::is_none")]
    pub evidence_adjusted: Option<bool>,
}

/// A knowledge triple DTO.
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeTripleDto {
    /// Subject entity.
    pub subject: EntityLinkDto,

    /// Predicate/relation.
    pub predicate: String,

    /// Object entity.
    pub object: EntityLinkDto,
}

// ============================================================================
// GET_ENTITY_GRAPH DTOs
// ============================================================================

/// Request parameters for get_entity_graph tool.
///
/// Builds and visualizes entity relationships from stored memories.
///
/// # Example JSON
/// ```json
/// {
///   "centerEntity": "PostgreSQL",
///   "maxNodes": 50,
///   "minRelationScore": 0.3
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct GetEntityGraphRequest {
    /// Optional center entity to focus on.
    #[serde(rename = "centerEntity")]
    pub center_entity: Option<String>,

    /// Maximum number of nodes (default: 50).
    #[serde(rename = "maxNodes", default = "default_max_nodes")]
    pub max_nodes: usize,

    /// Filter node entity types (reserved for future use).
    #[serde(rename = "entityTypes")]
    pub _entity_types: Option<Vec<String>>,

    /// Minimum edge score threshold (default: 0.3).
    #[serde(rename = "minRelationScore", default = "default_min_relation_score")]
    pub min_relation_score: f32,

    /// Include memory counts per node (reserved for future use, default: true).
    #[serde(rename = "includeMemoryCounts", default = "default_true")]
    pub _include_memory_counts: bool,
}

fn default_max_nodes() -> usize {
    50
}

fn default_min_relation_score() -> f32 {
    0.3
}

impl GetEntityGraphRequest {
    /// Validate the request parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_nodes < 1 || self.max_nodes > 500 {
            return Err(format!(
                "maxNodes must be between 1 and 500, got {}",
                self.max_nodes
            ));
        }

        if self.min_relation_score < 0.0 || self.min_relation_score > 1.0 {
            return Err(format!(
                "minRelationScore must be between 0.0 and 1.0, got {}",
                self.min_relation_score
            ));
        }

        Ok(())
    }
}

/// Response for get_entity_graph tool.
#[derive(Debug, Clone, Serialize)]
pub struct GetEntityGraphResponse {
    /// Graph nodes (entities).
    pub nodes: Vec<EntityNode>,

    /// Graph edges (relationships).
    pub edges: Vec<EntityEdge>,

    /// Center entity if specified.
    #[serde(rename = "centerEntity", skip_serializing_if = "Option::is_none")]
    pub center_entity: Option<EntityLinkDto>,

    /// Total memories scanned.
    #[serde(rename = "totalMemoriesScanned")]
    pub total_memories_scanned: usize,
}

/// An entity node in the graph.
#[derive(Debug, Clone, Serialize)]
pub struct EntityNode {
    /// Canonical entity ID.
    pub id: String,

    /// Display label.
    pub label: String,

    /// Entity type.
    #[serde(rename = "entityType")]
    pub entity_type: String,

    /// Number of memories mentioning this entity.
    #[serde(rename = "memoryCount")]
    pub memory_count: usize,

    /// Importance based on frequency and centrality.
    pub importance: f32,
}

/// An edge in the entity graph.
///
/// Phase 3a: Extended with TransE scores and relationship discovery method
/// for full provenance of entity relationships.
#[derive(Debug, Clone, Serialize)]
pub struct EntityEdge {
    /// Source entity canonical ID.
    pub source: String,

    /// Target entity canonical ID.
    pub target: String,

    /// Inferred relationship.
    pub relation: String,

    /// Edge weight (TransE score normalized).
    pub weight: f32,

    /// Memory IDs supporting this edge.
    #[serde(rename = "memoryIds")]
    pub memory_ids: Vec<Uuid>,

    /// TransE score for this edge (closer to 0 = stronger relationship).
    /// Phase 3a: Persisted for provenance - was previously discarded.
    #[serde(rename = "transeScore", skip_serializing_if = "Option::is_none")]
    pub transe_score: Option<f32>,

    /// How this relationship was discovered.
    /// Phase 3a: Tracks provenance of relationship discovery.
    #[serde(rename = "discoveryMethod", skip_serializing_if = "Option::is_none")]
    pub discovery_method: Option<String>,
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Convert TransE score to confidence [0, 1].
///
/// KEPLER TransE scores have larger magnitude than generic sentence embedders:
/// - Valid triples: typically score > -5.0
/// - Invalid triples: typically score < -10.0
///
/// We use linear normalization: confidence = (score + 15.0) / 15.0, clamped to [0, 1]
/// This maps:
/// - Score 0 → confidence 1.0
/// - Score -5.0 → confidence 0.67 (valid threshold)
/// - Score -7.5 → confidence 0.5 (midpoint)
/// - Score -10.0 → confidence 0.33 (invalid threshold)
/// - Score -15.0 → confidence 0.0
pub fn transe_score_to_confidence(score: f32) -> f32 {
    let normalized = (score + 15.0) / 15.0;
    normalized.clamp(0.0, 1.0)
}

/// Determine validation result from TransE score.
///
/// KEPLER thresholds:
/// - valid: score > -5.0
/// - uncertain: -10.0 <= score <= -5.0
/// - unlikely: score < -10.0
pub fn validation_from_score(score: f32) -> &'static str {
    if score > VALID_THRESHOLD {
        "valid"
    } else if score >= UNCERTAIN_THRESHOLD {
        "uncertain"
    } else {
        "unlikely"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_entities_validation_success() {
        let req = ExtractEntitiesRequest {
            text: "Building with Rust and PostgreSQL".to_string(),
            include_unknown: true,
            group_by_type: false,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_extract_entities_empty_text() {
        let req = ExtractEntitiesRequest {
            text: String::new(),
            include_unknown: true,
            group_by_type: false,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_search_by_entities_validation_success() {
        let req = SearchByEntitiesRequest {
            entities: vec!["PostgreSQL".to_string()],
            _entity_types: None,
            match_mode: "any".to_string(),
            top_k: 10,
            min_score: 0.2,
            include_content: false,
            boost_exact_match: 1.15,
            strategy: Some("pipeline".to_string()),
        };
        assert!(req.validate().is_ok());
        assert_eq!(req.parse_strategy(), SearchStrategy::Pipeline);
    }

    #[test]
    fn test_search_by_entities_empty_entities() {
        let req = SearchByEntitiesRequest {
            entities: vec![],
            _entity_types: None,
            match_mode: "any".to_string(),
            top_k: 10,
            min_score: 0.2,
            include_content: false,
            boost_exact_match: 1.15,
            strategy: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_search_by_entities_invalid_match_mode() {
        let req = SearchByEntitiesRequest {
            entities: vec!["PostgreSQL".to_string()],
            _entity_types: None,
            match_mode: "some".to_string(),
            top_k: 10,
            min_score: 0.2,
            include_content: false,
            boost_exact_match: 1.15,
            strategy: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_search_by_entities_parse_strategy_default() {
        let req = SearchByEntitiesRequest {
            entities: vec!["PostgreSQL".to_string()],
            _entity_types: None,
            match_mode: "any".to_string(),
            top_k: 10,
            min_score: 0.2,
            include_content: false,
            boost_exact_match: 1.15,
            strategy: None,
        };
        // Default is E1Only for entity search
        assert_eq!(req.parse_strategy(), SearchStrategy::E1Only);
    }

    #[test]
    fn test_infer_relationship_validation_success() {
        let req = InferRelationshipRequest {
            head_entity: "Tokio".to_string(),
            tail_entity: "Rust".to_string(),
            head_type: None,
            tail_type: None,
            top_k: 5,
            include_score: true,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_infer_relationship_empty_head() {
        let req = InferRelationshipRequest {
            head_entity: String::new(),
            tail_entity: "Rust".to_string(),
            head_type: None,
            tail_type: None,
            top_k: 5,
            include_score: true,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_validate_knowledge_validation_success() {
        let req = ValidateKnowledgeRequest {
            subject: "Claude".to_string(),
            predicate: "created_by".to_string(),
            object: "Anthropic".to_string(),
            subject_type: None,
            object_type: None,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_transe_score_to_confidence() {
        // Perfect score (0) should give max confidence
        let conf_0 = transe_score_to_confidence(0.0);
        assert!(
            (conf_0 - 1.0).abs() < 0.01,
            "Score 0 should give confidence 1.0, got {}",
            conf_0
        );

        // Score -5.0 (valid threshold) should give ~0.67 confidence
        let conf_5 = transe_score_to_confidence(-5.0);
        assert!(
            (conf_5 - 0.67).abs() < 0.01,
            "Score -5 should give ~0.67 confidence, got {}",
            conf_5
        );

        // Score -7.5 (midpoint) should give ~0.5 confidence
        let conf_75 = transe_score_to_confidence(-7.5);
        assert!(
            (conf_75 - 0.5).abs() < 0.01,
            "Score -7.5 should give ~0.5 confidence, got {}",
            conf_75
        );

        // Score -10.0 (invalid threshold) should give ~0.33 confidence
        let conf_10 = transe_score_to_confidence(-10.0);
        assert!(
            (conf_10 - 0.33).abs() < 0.01,
            "Score -10 should give ~0.33 confidence, got {}",
            conf_10
        );

        // Very negative score should give 0 confidence
        let conf_20 = transe_score_to_confidence(-20.0);
        assert!(
            (conf_20 - 0.0).abs() < 0.01,
            "Score -20 should give 0 confidence, got {}",
            conf_20
        );
    }

    #[test]
    fn test_validation_from_score() {
        // KEPLER thresholds: valid > -5.0, uncertain >= -10.0, invalid < -10.0
        assert_eq!(validation_from_score(0.0), "valid");
        assert_eq!(validation_from_score(-3.0), "valid");
        assert_eq!(validation_from_score(-5.0), "uncertain"); // Boundary
        assert_eq!(validation_from_score(-7.0), "uncertain");
        assert_eq!(validation_from_score(-10.0), "uncertain"); // Boundary
        assert_eq!(validation_from_score(-10.1), "unlikely");
        assert_eq!(validation_from_score(-15.0), "unlikely");
    }

    #[test]
    fn test_entity_type_to_string() {
        assert_eq!(entity_type_to_string(EntityType::Database), "Database");
        assert_eq!(
            entity_type_to_string(EntityType::ProgrammingLanguage),
            "ProgrammingLanguage"
        );
        assert_eq!(entity_type_to_string(EntityType::Unknown), "Unknown");
    }
}
