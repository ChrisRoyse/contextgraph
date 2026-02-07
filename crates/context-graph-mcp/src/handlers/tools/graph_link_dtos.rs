//! DTOs for graph linking MCP tools.
//!
//! Per Knowledge Graph Linking Enhancements specification, these DTOs support:
//! - get_memory_neighbors: K-NN neighbors in specific embedder space
//! - get_typed_edges: Typed edges derived from embedder agreement patterns
//! - traverse_graph: Multi-hop graph traversal following typed edges
//!
//! Constitution References:
//! - ARCH-18: E5/E8 use asymmetric similarity (direction matters)
//! - AP-60: Temporal embedders (E2-E4) never count toward edge type detection
//! - AP-77: E5 MUST NOT use symmetric cosine

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default number of neighbors for K-NN queries.
pub const DEFAULT_NEIGHBOR_TOP_K: usize = 10;

/// Maximum neighbors for K-NN queries.
pub const MAX_NEIGHBOR_TOP_K: usize = 50;

/// Default minimum similarity for neighbor queries.
pub const DEFAULT_MIN_NEIGHBOR_SIMILARITY: f32 = 0.0;

/// Default minimum weight for typed edges.
pub const DEFAULT_MIN_EDGE_WEIGHT: f32 = 0.0;

/// Default max hops for graph traversal.
pub const DEFAULT_TRAVERSAL_MAX_HOPS: usize = 2;

/// Maximum hops for graph traversal.
pub const MAX_TRAVERSAL_HOPS: usize = 5;

/// Default minimum edge weight for traversal.
pub const DEFAULT_TRAVERSAL_MIN_WEIGHT: f32 = 0.3;

/// Default max results for traversal.
pub const DEFAULT_TRAVERSAL_MAX_RESULTS: usize = 20;

/// Maximum results for traversal.
pub const MAX_TRAVERSAL_RESULTS: usize = 100;

/// Valid embedder IDs (0-12 for E1-E13).
pub const MAX_EMBEDDER_ID: usize = 12;

/// Valid edge types for filtering.
pub const VALID_EDGE_TYPES: [&str; 8] = [
    "semantic_similar",
    "code_related",
    "entity_shared",
    "causal_chain",
    "graph_connected",
    "intent_aligned",
    "keyword_overlap",
    "multi_agreement",
];

// ============================================================================
// REQUEST DTOs
// ============================================================================

/// Request parameters for get_memory_neighbors tool.
///
/// # Example JSON
/// ```json
/// {
///   "memory_id": "550e8400-e29b-41d4-a716-446655440000",
///   "embedder_id": 0,
///   "top_k": 10,
///   "min_similarity": 0.3,
///   "include_content": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct GetMemoryNeighborsRequest {
    /// UUID of the memory to find neighbors for (required).
    pub memory_id: String,

    /// Embedder space to search (0-12 for E1-E13, default: 0=E1 semantic).
    #[serde(default)]
    pub embedder_id: usize,

    /// Number of neighbors to return (1-50, default: 10).
    #[serde(default = "default_neighbor_top_k")]
    pub top_k: usize,

    /// Minimum similarity threshold (0-1, default: 0.0).
    #[serde(default)]
    pub min_similarity: f32,

    /// Whether to include memory content in results (default: false).
    #[serde(default)]
    pub include_content: bool,
}

fn default_neighbor_top_k() -> usize {
    DEFAULT_NEIGHBOR_TOP_K
}

impl Default for GetMemoryNeighborsRequest {
    fn default() -> Self {
        Self {
            memory_id: String::new(),
            embedder_id: 0,
            top_k: DEFAULT_NEIGHBOR_TOP_K,
            min_similarity: DEFAULT_MIN_NEIGHBOR_SIMILARITY,
            include_content: false,
        }
    }
}

impl GetMemoryNeighborsRequest {
    /// Validate the request parameters.
    ///
    /// # Returns
    /// - Ok(Uuid) if valid
    /// - Err(String) with error message if invalid
    pub fn validate(&self) -> Result<Uuid, String> {
        // Validate memory UUID
        let memory_uuid = Uuid::parse_str(&self.memory_id).map_err(|e| {
            format!(
                "Invalid UUID format for memory_id '{}': {}",
                self.memory_id, e
            )
        })?;

        // Validate embedder ID
        if self.embedder_id > MAX_EMBEDDER_ID {
            return Err(format!(
                "embedder_id must be between 0 and {}, got {}",
                MAX_EMBEDDER_ID, self.embedder_id
            ));
        }

        // Validate top_k
        if self.top_k < 1 || self.top_k > MAX_NEIGHBOR_TOP_K {
            return Err(format!(
                "top_k must be between 1 and {}, got {}",
                MAX_NEIGHBOR_TOP_K, self.top_k
            ));
        }

        // Validate min_similarity
        if self.min_similarity.is_nan() || self.min_similarity.is_infinite() {
            return Err("min_similarity must be a finite number".to_string());
        }
        if self.min_similarity < 0.0 || self.min_similarity > 1.0 {
            return Err(format!(
                "min_similarity must be between 0.0 and 1.0, got {}",
                self.min_similarity
            ));
        }

        Ok(memory_uuid)
    }
}

/// Request parameters for get_typed_edges tool.
///
/// # Example JSON
/// ```json
/// {
///   "memory_id": "550e8400-e29b-41d4-a716-446655440000",
///   "edge_type": "semantic_similar",
///   "direction": "outgoing",
///   "min_weight": 0.5,
///   "include_content": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct GetTypedEdgesRequest {
    /// UUID of the memory to get edges from (required).
    pub memory_id: String,

    /// Filter by edge type (optional, returns all types if not specified).
    pub edge_type: Option<String>,

    /// Edge direction: "outgoing", "incoming", or "both" (default: "outgoing").
    #[serde(default = "default_edge_direction")]
    pub direction: String,

    /// Minimum edge weight threshold (0-1, default: 0.0).
    #[serde(default)]
    pub min_weight: f32,

    /// Whether to include memory content in results (default: false).
    #[serde(default)]
    pub include_content: bool,
}

fn default_edge_direction() -> String {
    "outgoing".to_string()
}

impl Default for GetTypedEdgesRequest {
    fn default() -> Self {
        Self {
            memory_id: String::new(),
            edge_type: None,
            direction: "outgoing".to_string(),
            min_weight: DEFAULT_MIN_EDGE_WEIGHT,
            include_content: false,
        }
    }
}

impl GetTypedEdgesRequest {
    /// Validate the request parameters.
    ///
    /// # Returns
    /// - Ok(Uuid) if valid
    /// - Err(String) with error message if invalid
    pub fn validate(&self) -> Result<Uuid, String> {
        // Validate memory UUID
        let memory_uuid = Uuid::parse_str(&self.memory_id).map_err(|e| {
            format!(
                "Invalid UUID format for memory_id '{}': {}",
                self.memory_id, e
            )
        })?;

        // Validate edge_type if provided
        if let Some(ref edge_type) = self.edge_type {
            if !VALID_EDGE_TYPES.contains(&edge_type.as_str()) {
                return Err(format!(
                    "edge_type must be one of {:?}, got '{}'",
                    VALID_EDGE_TYPES, edge_type
                ));
            }
        }

        // Validate direction
        let valid_directions = ["outgoing", "incoming", "both"];
        if !valid_directions.contains(&self.direction.as_str()) {
            return Err(format!(
                "direction must be one of {:?}, got '{}'",
                valid_directions, self.direction
            ));
        }

        // Validate min_weight
        if self.min_weight.is_nan() || self.min_weight.is_infinite() {
            return Err("min_weight must be a finite number".to_string());
        }
        if self.min_weight < 0.0 || self.min_weight > 1.0 {
            return Err(format!(
                "min_weight must be between 0.0 and 1.0, got {}",
                self.min_weight
            ));
        }

        Ok(memory_uuid)
    }

    /// Returns true if searching for outgoing edges.
    pub fn is_outgoing(&self) -> bool {
        self.direction == "outgoing" || self.direction == "both"
    }

    /// Returns true if searching for incoming edges.
    pub fn is_incoming(&self) -> bool {
        self.direction == "incoming" || self.direction == "both"
    }
}

/// Request parameters for traverse_graph tool.
///
/// # Example JSON
/// ```json
/// {
///   "start_memory_id": "550e8400-e29b-41d4-a716-446655440000",
///   "max_hops": 3,
///   "edge_type": "causal_chain",
///   "min_weight": 0.4,
///   "max_results": 20,
///   "include_content": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct TraverseGraphRequest {
    /// UUID of the starting memory (required).
    pub start_memory_id: String,

    /// Maximum traversal depth (1-5, default: 2).
    #[serde(default = "default_max_hops")]
    pub max_hops: usize,

    /// Filter traversal by edge type (optional).
    pub edge_type: Option<String>,

    /// Minimum edge weight to follow (0-1, default: 0.3).
    #[serde(default = "default_min_weight")]
    pub min_weight: f32,

    /// Maximum paths to return (1-100, default: 20).
    #[serde(default = "default_max_results")]
    pub max_results: usize,

    /// Whether to include memory content in results (default: false).
    #[serde(default)]
    pub include_content: bool,
}

fn default_max_hops() -> usize {
    DEFAULT_TRAVERSAL_MAX_HOPS
}

fn default_min_weight() -> f32 {
    DEFAULT_TRAVERSAL_MIN_WEIGHT
}

fn default_max_results() -> usize {
    DEFAULT_TRAVERSAL_MAX_RESULTS
}

impl Default for TraverseGraphRequest {
    fn default() -> Self {
        Self {
            start_memory_id: String::new(),
            max_hops: DEFAULT_TRAVERSAL_MAX_HOPS,
            edge_type: None,
            min_weight: DEFAULT_TRAVERSAL_MIN_WEIGHT,
            max_results: DEFAULT_TRAVERSAL_MAX_RESULTS,
            include_content: false,
        }
    }
}

impl TraverseGraphRequest {
    /// Validate the request parameters.
    ///
    /// # Returns
    /// - Ok(Uuid) if valid
    /// - Err(String) with error message if invalid
    pub fn validate(&self) -> Result<Uuid, String> {
        // Validate start memory UUID
        let start_uuid = Uuid::parse_str(&self.start_memory_id).map_err(|e| {
            format!(
                "Invalid UUID format for start_memory_id '{}': {}",
                self.start_memory_id, e
            )
        })?;

        // Validate max_hops
        if self.max_hops < 1 || self.max_hops > MAX_TRAVERSAL_HOPS {
            return Err(format!(
                "max_hops must be between 1 and {}, got {}",
                MAX_TRAVERSAL_HOPS, self.max_hops
            ));
        }

        // Validate edge_type if provided
        if let Some(ref edge_type) = self.edge_type {
            if !VALID_EDGE_TYPES.contains(&edge_type.as_str()) {
                return Err(format!(
                    "edge_type must be one of {:?}, got '{}'",
                    VALID_EDGE_TYPES, edge_type
                ));
            }
        }

        // Validate min_weight
        if self.min_weight.is_nan() || self.min_weight.is_infinite() {
            return Err("min_weight must be a finite number".to_string());
        }
        if self.min_weight < 0.0 || self.min_weight > 1.0 {
            return Err(format!(
                "min_weight must be between 0.0 and 1.0, got {}",
                self.min_weight
            ));
        }

        // Validate max_results
        if self.max_results < 1 || self.max_results > MAX_TRAVERSAL_RESULTS {
            return Err(format!(
                "max_results must be between 1 and {}, got {}",
                MAX_TRAVERSAL_RESULTS, self.max_results
            ));
        }

        Ok(start_uuid)
    }
}

// ============================================================================
// RESPONSE DTOs
// ============================================================================

/// A single neighbor result from get_memory_neighbors.
#[derive(Debug, Clone, Serialize)]
pub struct NeighborResult {
    /// UUID of the neighbor memory.
    pub neighbor_id: Uuid,

    /// Similarity score in the specified embedder space.
    pub similarity: f32,

    /// Full content text (only if include_content=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Source metadata for provenance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<NeighborSourceInfo>,
}

/// Source information for a neighbor result.
#[derive(Debug, Clone, Serialize)]
pub struct NeighborSourceInfo {
    /// Source type (MDFileChunk, HookDescription, etc.)
    #[serde(rename = "type")]
    pub source_type: String,

    /// File path for MDFileChunk sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

/// Response for get_memory_neighbors tool.
#[derive(Debug, Clone, Serialize)]
pub struct GetMemoryNeighborsResponse {
    /// UUID of the query memory.
    pub memory_id: Uuid,

    /// Embedder space searched (0-12).
    pub embedder_id: usize,

    /// Embedder name for readability.
    pub embedder_name: String,

    /// Ranked list of neighbors (highest similarity first).
    pub neighbors: Vec<NeighborResult>,

    /// Number of neighbors returned.
    pub count: usize,

    /// Metadata about the search.
    pub metadata: NeighborSearchMetadata,
}

/// Metadata about a neighbor search operation.
#[derive(Debug, Clone, Serialize)]
pub struct NeighborSearchMetadata {
    /// Number of candidates evaluated.
    pub candidates_evaluated: usize,

    /// Number filtered out by min_similarity.
    pub filtered_by_similarity: usize,

    /// Whether asymmetric similarity was used (E5 causal, E8 graph).
    pub used_asymmetric: bool,
}

impl GetMemoryNeighborsResponse {
    /// Create an empty response (no neighbors found).
    pub fn empty(memory_id: Uuid, embedder_id: usize, embedder_name: &str) -> Self {
        Self {
            memory_id,
            embedder_id,
            embedder_name: embedder_name.to_string(),
            neighbors: vec![],
            count: 0,
            metadata: NeighborSearchMetadata {
                candidates_evaluated: 0,
                filtered_by_similarity: 0,
                used_asymmetric: false,
            },
        }
    }
}

/// A single typed edge from get_typed_edges.
#[derive(Debug, Clone, Serialize)]
pub struct TypedEdgeResult {
    /// Target memory UUID.
    pub target_id: Uuid,

    /// Edge type (semantic_similar, code_related, etc.).
    pub edge_type: String,

    /// Edge weight (combined agreement score).
    pub weight: f32,

    /// Weighted agreement score from contributing embedders.
    pub weighted_agreement: f32,

    /// Direction of the edge.
    pub direction: String,

    /// Contributing embedders that agreed on this edge.
    pub contributing_embedders: Vec<String>,

    /// Full content text (only if include_content=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Response for get_typed_edges tool.
#[derive(Debug, Clone, Serialize)]
pub struct GetTypedEdgesResponse {
    /// UUID of the source memory.
    pub memory_id: Uuid,

    /// Direction filter used.
    pub direction: String,

    /// Edge type filter used (null if all types).
    pub edge_type_filter: Option<String>,

    /// List of typed edges.
    pub edges: Vec<TypedEdgeResult>,

    /// Number of edges returned.
    pub count: usize,

    /// Metadata about the query.
    pub metadata: TypedEdgeMetadata,
}

/// Metadata about a typed edge query.
#[derive(Debug, Clone, Serialize)]
pub struct TypedEdgeMetadata {
    /// Total edges for this memory (before filtering).
    pub total_edges: usize,

    /// Number filtered out by edge_type.
    pub filtered_by_type: usize,

    /// Number filtered out by min_weight.
    pub filtered_by_weight: usize,
}

impl GetTypedEdgesResponse {
    /// Create an empty response (no edges found).
    pub fn empty(memory_id: Uuid, direction: &str, edge_type_filter: Option<String>) -> Self {
        Self {
            memory_id,
            direction: direction.to_string(),
            edge_type_filter,
            edges: vec![],
            count: 0,
            metadata: TypedEdgeMetadata {
                total_edges: 0,
                filtered_by_type: 0,
                filtered_by_weight: 0,
            },
        }
    }
}

/// A single path from traverse_graph.
#[derive(Debug, Clone, Serialize)]
pub struct TraversalPath {
    /// Ordered list of memory IDs in this path.
    pub path: Vec<Uuid>,

    /// Total path weight (product of edge weights).
    pub total_weight: f32,

    /// Number of hops in this path.
    pub hop_count: usize,

    /// Edge types traversed.
    pub edge_types: Vec<String>,

    /// Edge weights at each hop.
    pub edge_weights: Vec<f32>,
}

/// A node in the traversal with details.
#[derive(Debug, Clone, Serialize)]
pub struct TraversalNode {
    /// Memory UUID.
    pub memory_id: Uuid,

    /// Hop level from start (0 = start node).
    pub hop_level: usize,

    /// Edge type from parent (null for start node).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_type_from_parent: Option<String>,

    /// Edge weight from parent (null for start node).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_weight_from_parent: Option<f32>,

    /// Cumulative path weight to this node.
    pub cumulative_weight: f32,

    /// Full content text (only if include_content=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Response for traverse_graph tool.
#[derive(Debug, Clone, Serialize)]
pub struct TraverseGraphResponse {
    /// UUID of the starting memory.
    pub start_memory_id: Uuid,

    /// Maximum hops used.
    pub max_hops: usize,

    /// Edge type filter used (null if all types).
    pub edge_type_filter: Option<String>,

    /// All visited nodes with their details.
    pub nodes: Vec<TraversalNode>,

    /// All discovered paths.
    pub paths: Vec<TraversalPath>,

    /// Number of unique nodes visited.
    pub unique_nodes_visited: usize,

    /// Number of paths discovered.
    pub path_count: usize,

    /// Metadata about the traversal.
    pub metadata: TraversalMetadata,
}

/// Metadata about a graph traversal operation.
#[derive(Debug, Clone, Serialize)]
pub struct TraversalMetadata {
    /// Minimum weight threshold used.
    pub min_weight: f32,

    /// Maximum results limit.
    pub max_results: usize,

    /// Whether results were truncated.
    pub truncated: bool,

    /// Total edges evaluated.
    pub edges_evaluated: usize,

    /// Edges filtered by weight.
    pub edges_filtered_by_weight: usize,
}

impl TraverseGraphResponse {
    /// Create an empty response (no traversal possible).
    pub fn empty(
        start_memory_id: Uuid,
        max_hops: usize,
        edge_type_filter: Option<String>,
        min_weight: f32,
        max_results: usize,
    ) -> Self {
        Self {
            start_memory_id,
            max_hops,
            edge_type_filter,
            nodes: vec![],
            paths: vec![],
            unique_nodes_visited: 0,
            path_count: 0,
            metadata: TraversalMetadata {
                min_weight,
                max_results,
                truncated: false,
                edges_evaluated: 0,
                edges_filtered_by_weight: 0,
            },
        }
    }
}

// ============================================================================
// EMBEDDER NAME HELPERS
// ============================================================================

/// Get embedder name from ID.
pub fn embedder_name(id: usize) -> &'static str {
    match id {
        0 => "E1 (V_meaning)",
        1 => "E2 (V_freshness)",
        2 => "E3 (V_periodicity)",
        3 => "E4 (V_ordering)",
        4 => "E5 (V_causality)",
        5 => "E6 (V_selectivity)",
        6 => "E7 (V_correctness)",
        7 => "E8 (V_connectivity)",
        8 => "E9 (V_robustness)",
        9 => "E10 (V_multimodality)",
        10 => "E11 (V_factuality)",
        11 => "E12 (V_precision)",
        12 => "E13 (V_keyword_precision)",
        _ => "Unknown",
    }
}

/// Check if embedder uses asymmetric similarity.
pub fn uses_asymmetric_similarity(embedder_id: usize) -> bool {
    // E5 (causal) and E8 (graph) use asymmetric similarity per ARCH-18
    embedder_id == 4 || embedder_id == 7
}

// ============================================================================
// UNIFIED NEIGHBORS (Weighted RRF across all 13 embedders)
// ============================================================================

/// Semantic embedder indices for RRF fusion.
/// Per AP-60: Temporal embedders (E2-E4) are EXCLUDED from semantic fusion.
pub const SEMANTIC_EMBEDDER_INDICES: [usize; 10] = [
    0,  // E1  - Semantic (foundation)
    4,  // E5  - Causal (cause-effect)
    5,  // E6  - Sparse (keywords)
    6,  // E7  - Code (patterns)
    7,  // E8  - Graph (structure)
    8,  // E9  - HDC (robustness)
    9,  // E10 - Intent (goals)
    10, // E11 - Entity (knowledge)
    11, // E12 - ColBERT (phrases)
    12, // E13 - SPLADE (expansion)
];

/// Temporal embedder indices (excluded from semantic fusion per AP-60).
pub const TEMPORAL_EMBEDDER_INDICES: [usize; 3] = [
    1, // E2 - Freshness
    2, // E3 - Periodicity
    3, // E4 - Ordering
];

/// Standard RRF constant (k=60).
pub const RRF_K: f32 = 60.0;

/// Request parameters for get_unified_neighbors tool.
///
/// # Example JSON
/// ```json
/// {
///   "memory_id": "550e8400-e29b-41d4-a716-446655440000",
///   "weight_profile": "semantic_search",
///   "top_k": 10,
///   "min_score": 0.0,
///   "include_content": false,
///   "include_embedder_breakdown": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct GetUnifiedNeighborsRequest {
    /// UUID of the memory to find neighbors for (required).
    pub memory_id: String,

    /// Weight profile for RRF fusion (default: "semantic_search").
    /// Available profiles: semantic_search, code_search, causal_reasoning, etc.
    #[serde(default = "default_weight_profile")]
    pub weight_profile: String,

    /// Number of neighbors to return (1-50, default: 10).
    #[serde(default = "default_neighbor_top_k")]
    pub top_k: usize,

    /// Minimum RRF score threshold (0-1, default: 0.0).
    #[serde(default)]
    pub min_score: f32,

    /// Whether to include memory content in results (default: false).
    #[serde(default)]
    pub include_content: bool,

    /// Whether to include per-embedder breakdown in results (default: true).
    #[serde(default = "default_include_embedder_breakdown")]
    pub include_embedder_breakdown: bool,

    /// Custom per-embedder weights (overrides weight_profile). Each value 0-1, sum ~1.0.
    #[serde(default)]
    pub custom_weights: Option<std::collections::HashMap<String, f64>>,

    /// Embedders to exclude from fusion (zeroed + renormalized).
    #[serde(default)]
    pub exclude_embedders: Vec<String>,
}

fn default_weight_profile() -> String {
    "semantic_search".to_string()
}

fn default_include_embedder_breakdown() -> bool {
    true
}

impl Default for GetUnifiedNeighborsRequest {
    fn default() -> Self {
        Self {
            memory_id: String::new(),
            weight_profile: "semantic_search".to_string(),
            top_k: DEFAULT_NEIGHBOR_TOP_K,
            min_score: 0.0,
            include_content: false,
            include_embedder_breakdown: true,
            custom_weights: None,
            exclude_embedders: Vec::new(),
        }
    }
}

impl GetUnifiedNeighborsRequest {
    /// Validate the request parameters.
    ///
    /// # Returns
    /// - Ok(Uuid) if valid
    /// - Err(String) with error message if invalid
    pub fn validate(&self) -> Result<Uuid, String> {
        // Validate memory UUID
        let memory_uuid = Uuid::parse_str(&self.memory_id).map_err(|e| {
            format!(
                "Invalid UUID format for memory_id '{}': {}",
                self.memory_id, e
            )
        })?;

        // Validate top_k
        if self.top_k < 1 || self.top_k > MAX_NEIGHBOR_TOP_K {
            return Err(format!(
                "top_k must be between 1 and {}, got {}",
                MAX_NEIGHBOR_TOP_K, self.top_k
            ));
        }

        // Validate min_score
        if self.min_score.is_nan() || self.min_score.is_infinite() {
            return Err("min_score must be a finite number".to_string());
        }
        if self.min_score < 0.0 || self.min_score > 1.0 {
            return Err(format!(
                "min_score must be between 0.0 and 1.0, got {}",
                self.min_score
            ));
        }

        Ok(memory_uuid)
    }
}

/// A single neighbor result with unified RRF score from get_unified_neighbors.
#[derive(Debug, Clone, Serialize)]
pub struct UnifiedNeighborResult {
    /// UUID of the neighbor memory.
    pub neighbor_id: Uuid,

    /// Weighted RRF score (fused across embedders).
    pub rrf_score: f32,

    /// Number of embedders that found this neighbor.
    pub embedder_count: usize,

    /// Names of embedders that contributed to this result.
    pub contributing_embedders: Vec<String>,

    /// Per-embedder similarity scores (0.0 if not found by that embedder).
    /// Array of 13 scores: [E1, E2, ..., E13]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedder_scores: Option<[f32; 13]>,

    /// Per-embedder ranks (0 if not found by that embedder).
    /// Array of 13 ranks: [E1, E2, ..., E13]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedder_ranks: Option<[usize; 13]>,

    /// Full content text (only if include_content=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Source metadata for provenance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<NeighborSourceInfo>,
}

/// Summary of embedder agreement across all neighbors.
#[derive(Debug, Clone, Serialize)]
pub struct AgreementSummary {
    /// Count of neighbors with strong agreement (>=6 embedders).
    pub strong_agreement: usize,

    /// Count of neighbors with moderate agreement (3-5 embedders).
    pub moderate_agreement: usize,

    /// Count of neighbors with weak agreement (1-2 embedders).
    pub weak_agreement: usize,

    /// Top contributing embedders ranked by contribution count.
    pub top_contributing_embedders: Vec<EmbedderContribution>,
}

/// Contribution statistics for an embedder.
#[derive(Debug, Clone, Serialize)]
pub struct EmbedderContribution {
    /// Embedder name (e.g., "E1 (V_meaning)").
    pub embedder_name: String,

    /// Number of neighbors this embedder found.
    pub contribution_count: usize,

    /// Weight used in RRF fusion.
    pub weight: f32,
}

/// Metadata about unified neighbor search operation.
#[derive(Debug, Clone, Serialize)]
pub struct UnifiedNeighborMetadata {
    /// Total candidates evaluated across all embedders.
    pub total_candidates_evaluated: usize,

    /// Unique candidates after deduplication.
    pub unique_candidates: usize,

    /// Number filtered out by min_score.
    pub filtered_by_score: usize,

    /// RRF constant used (k=60).
    pub rrf_k: f32,

    /// Embedders excluded from fusion (E2-E4 temporal).
    pub excluded_embedders: Vec<String>,

    /// Fusion strategy used.
    pub fusion_strategy: String,
}

/// Response for get_unified_neighbors tool.
#[derive(Debug, Clone, Serialize)]
pub struct GetUnifiedNeighborsResponse {
    /// UUID of the query memory.
    pub memory_id: Uuid,

    /// Weight profile used for fusion.
    pub weight_profile: String,

    /// Ranked list of neighbors (highest RRF score first).
    pub neighbors: Vec<UnifiedNeighborResult>,

    /// Number of neighbors returned.
    pub count: usize,

    /// Agreement summary across all neighbors.
    pub agreement_summary: AgreementSummary,

    /// Metadata about the search operation.
    pub metadata: UnifiedNeighborMetadata,
}

impl GetUnifiedNeighborsResponse {
    /// Create an empty response (no neighbors found).
    pub fn empty(memory_id: Uuid, weight_profile: &str) -> Self {
        Self {
            memory_id,
            weight_profile: weight_profile.to_string(),
            neighbors: vec![],
            count: 0,
            agreement_summary: AgreementSummary {
                strong_agreement: 0,
                moderate_agreement: 0,
                weak_agreement: 0,
                top_contributing_embedders: vec![],
            },
            metadata: UnifiedNeighborMetadata {
                total_candidates_evaluated: 0,
                unique_candidates: 0,
                filtered_by_score: 0,
                rrf_k: RRF_K,
                excluded_embedders: vec![
                    "E2 (V_freshness)".to_string(),
                    "E3 (V_periodicity)".to_string(),
                    "E4 (V_ordering)".to_string(),
                ],
                fusion_strategy: "weighted_rrf".to_string(),
            },
        }
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ===== GetMemoryNeighborsRequest Tests =====

    #[test]
    fn test_get_neighbors_request_defaults() {
        let json = r#"{"memory_id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: GetMemoryNeighborsRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.memory_id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(req.embedder_id, 0);
        assert_eq!(req.top_k, DEFAULT_NEIGHBOR_TOP_K);
        assert!((req.min_similarity - DEFAULT_MIN_NEIGHBOR_SIMILARITY).abs() < f32::EPSILON);
        assert!(!req.include_content);
        println!("[PASS] GetMemoryNeighborsRequest uses correct defaults");
    }

    #[test]
    fn test_get_neighbors_request_validation_valid() {
        let req = GetMemoryNeighborsRequest {
            memory_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            embedder_id: 6, // E7 code
            top_k: 20,
            min_similarity: 0.5,
            include_content: true,
        };

        assert!(req.validate().is_ok());
        println!("[PASS] GetMemoryNeighborsRequest validates correct input");
    }

    #[test]
    fn test_get_neighbors_request_validation_invalid_uuid() {
        let req = GetMemoryNeighborsRequest {
            memory_id: "not-a-uuid".to_string(),
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid UUID"));
        println!("[PASS] GetMemoryNeighborsRequest rejects invalid UUID");
    }

    #[test]
    fn test_get_neighbors_request_validation_invalid_embedder() {
        let req = GetMemoryNeighborsRequest {
            memory_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            embedder_id: 15, // Invalid
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("embedder_id"));
        println!("[PASS] GetMemoryNeighborsRequest rejects invalid embedder_id");
    }

    // ===== GetTypedEdgesRequest Tests =====

    #[test]
    fn test_get_edges_request_defaults() {
        let json = r#"{"memory_id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: GetTypedEdgesRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.memory_id, "550e8400-e29b-41d4-a716-446655440000");
        assert!(req.edge_type.is_none());
        assert_eq!(req.direction, "outgoing");
        assert!((req.min_weight - DEFAULT_MIN_EDGE_WEIGHT).abs() < f32::EPSILON);
        assert!(!req.include_content);
        println!("[PASS] GetTypedEdgesRequest uses correct defaults");
    }

    #[test]
    fn test_get_edges_request_validation_valid() {
        let req = GetTypedEdgesRequest {
            memory_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            edge_type: Some("causal_chain".to_string()),
            direction: "both".to_string(),
            min_weight: 0.5,
            include_content: true,
        };

        assert!(req.validate().is_ok());
        println!("[PASS] GetTypedEdgesRequest validates correct input");
    }

    #[test]
    fn test_get_edges_request_validation_invalid_edge_type() {
        let req = GetTypedEdgesRequest {
            memory_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            edge_type: Some("invalid_type".to_string()),
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("edge_type"));
        println!("[PASS] GetTypedEdgesRequest rejects invalid edge_type");
    }

    #[test]
    fn test_get_edges_request_direction_helpers() {
        let outgoing = GetTypedEdgesRequest {
            memory_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            direction: "outgoing".to_string(),
            ..Default::default()
        };
        assert!(outgoing.is_outgoing());
        assert!(!outgoing.is_incoming());

        let incoming = GetTypedEdgesRequest {
            memory_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            direction: "incoming".to_string(),
            ..Default::default()
        };
        assert!(!incoming.is_outgoing());
        assert!(incoming.is_incoming());

        let both = GetTypedEdgesRequest {
            memory_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            direction: "both".to_string(),
            ..Default::default()
        };
        assert!(both.is_outgoing());
        assert!(both.is_incoming());
        println!("[PASS] Direction helpers work correctly");
    }

    // ===== TraverseGraphRequest Tests =====

    #[test]
    fn test_traverse_request_defaults() {
        let json = r#"{"start_memory_id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: TraverseGraphRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.start_memory_id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(req.max_hops, DEFAULT_TRAVERSAL_MAX_HOPS);
        assert!(req.edge_type.is_none());
        assert!((req.min_weight - DEFAULT_TRAVERSAL_MIN_WEIGHT).abs() < f32::EPSILON);
        assert_eq!(req.max_results, DEFAULT_TRAVERSAL_MAX_RESULTS);
        assert!(!req.include_content);
        println!("[PASS] TraverseGraphRequest uses correct defaults");
    }

    #[test]
    fn test_traverse_request_validation_valid() {
        let req = TraverseGraphRequest {
            start_memory_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            max_hops: 3,
            edge_type: Some("semantic_similar".to_string()),
            min_weight: 0.4,
            max_results: 50,
            include_content: true,
        };

        assert!(req.validate().is_ok());
        println!("[PASS] TraverseGraphRequest validates correct input");
    }

    #[test]
    fn test_traverse_request_validation_max_hops_too_high() {
        let req = TraverseGraphRequest {
            start_memory_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            max_hops: 10, // > MAX_TRAVERSAL_HOPS (5)
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("max_hops"));
        println!("[PASS] TraverseGraphRequest rejects max_hops > 5");
    }

    // ===== Helper Tests =====

    #[test]
    fn test_embedder_names() {
        assert_eq!(embedder_name(0), "E1 (V_meaning)");
        assert_eq!(embedder_name(4), "E5 (V_causality)");
        assert_eq!(embedder_name(6), "E7 (V_correctness)");
        assert_eq!(embedder_name(10), "E11 (V_factuality)");
        assert_eq!(embedder_name(99), "Unknown");
        println!("[PASS] Embedder names correct");
    }

    #[test]
    fn test_asymmetric_embedders() {
        // E5 (id=4) and E8 (id=7) use asymmetric per ARCH-18
        assert!(uses_asymmetric_similarity(4));
        assert!(uses_asymmetric_similarity(7));
        assert!(!uses_asymmetric_similarity(0)); // E1
        assert!(!uses_asymmetric_similarity(6)); // E7
        println!("[PASS] Asymmetric embedder detection correct");
    }

    // ===== Response Tests =====

    #[test]
    fn test_empty_neighbors_response() {
        let response =
            GetMemoryNeighborsResponse::empty(Uuid::nil(), 0, "E1 (V_meaning)");

        assert_eq!(response.count, 0);
        assert!(response.neighbors.is_empty());
        println!("[PASS] Empty neighbors response created correctly");
    }

    #[test]
    fn test_empty_edges_response() {
        let response = GetTypedEdgesResponse::empty(Uuid::nil(), "outgoing", None);

        assert_eq!(response.count, 0);
        assert!(response.edges.is_empty());
        println!("[PASS] Empty edges response created correctly");
    }

    #[test]
    fn test_empty_traversal_response() {
        let response =
            TraverseGraphResponse::empty(Uuid::nil(), 2, None, 0.3, 20);

        assert_eq!(response.unique_nodes_visited, 0);
        assert_eq!(response.path_count, 0);
        assert!(!response.metadata.truncated);
        println!("[PASS] Empty traversal response created correctly");
    }

    #[test]
    fn test_response_serialization() {
        let response = GetMemoryNeighborsResponse {
            memory_id: Uuid::nil(),
            embedder_id: 0,
            embedder_name: "E1 (V_meaning)".to_string(),
            neighbors: vec![NeighborResult {
                neighbor_id: Uuid::nil(),
                similarity: 0.85,
                content: None,
                source: None,
            }],
            count: 1,
            metadata: NeighborSearchMetadata {
                candidates_evaluated: 100,
                filtered_by_similarity: 50,
                used_asymmetric: false,
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"embedder_id\":0"));
        assert!(json.contains("\"similarity\":0.85"));
        println!("[PASS] Response serializes correctly");
    }

    // ===== GetUnifiedNeighborsRequest Tests =====

    #[test]
    fn test_unified_neighbors_request_defaults() {
        let json = r#"{"memory_id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: GetUnifiedNeighborsRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.memory_id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(req.weight_profile, "semantic_search");
        assert_eq!(req.top_k, DEFAULT_NEIGHBOR_TOP_K);
        assert!((req.min_score - 0.0).abs() < f32::EPSILON);
        assert!(!req.include_content);
        assert!(req.include_embedder_breakdown);
        println!("[PASS] GetUnifiedNeighborsRequest uses correct defaults");
    }

    #[test]
    fn test_unified_neighbors_request_validation_valid() {
        let req = GetUnifiedNeighborsRequest {
            memory_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            weight_profile: "code_search".to_string(),
            top_k: 20,
            min_score: 0.1,
            include_content: true,
            include_embedder_breakdown: true,
            custom_weights: None,
            exclude_embedders: Vec::new(),
        };

        assert!(req.validate().is_ok());
        println!("[PASS] GetUnifiedNeighborsRequest validates correct input");
    }

    #[test]
    fn test_unified_neighbors_request_validation_invalid_uuid() {
        let req = GetUnifiedNeighborsRequest {
            memory_id: "not-a-uuid".to_string(),
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid UUID"));
        println!("[PASS] GetUnifiedNeighborsRequest rejects invalid UUID");
    }

    #[test]
    fn test_unified_neighbors_request_validation_invalid_top_k() {
        let req = GetUnifiedNeighborsRequest {
            memory_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            top_k: 100, // > MAX_NEIGHBOR_TOP_K (50)
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("top_k"));
        println!("[PASS] GetUnifiedNeighborsRequest rejects invalid top_k");
    }

    #[test]
    fn test_unified_neighbors_response_empty() {
        let response = GetUnifiedNeighborsResponse::empty(Uuid::nil(), "semantic_search");

        assert_eq!(response.count, 0);
        assert!(response.neighbors.is_empty());
        assert_eq!(response.agreement_summary.strong_agreement, 0);
        assert_eq!(response.metadata.excluded_embedders.len(), 3);
        assert_eq!(response.metadata.fusion_strategy, "weighted_rrf");
        println!("[PASS] Empty unified neighbors response created correctly");
    }

    #[test]
    fn test_semantic_embedder_indices_excludes_temporal() {
        // Verify E2-E4 are NOT in SEMANTIC_EMBEDDER_INDICES
        assert!(!SEMANTIC_EMBEDDER_INDICES.contains(&1)); // E2
        assert!(!SEMANTIC_EMBEDDER_INDICES.contains(&2)); // E3
        assert!(!SEMANTIC_EMBEDDER_INDICES.contains(&3)); // E4

        // Verify E1, E5-E13 ARE in SEMANTIC_EMBEDDER_INDICES
        assert!(SEMANTIC_EMBEDDER_INDICES.contains(&0));  // E1
        assert!(SEMANTIC_EMBEDDER_INDICES.contains(&4));  // E5
        assert!(SEMANTIC_EMBEDDER_INDICES.contains(&12)); // E13

        println!("[PASS] SEMANTIC_EMBEDDER_INDICES correctly excludes temporal per AP-60");
    }

    #[test]
    fn test_temporal_embedder_indices() {
        assert_eq!(TEMPORAL_EMBEDDER_INDICES.len(), 3);
        assert!(TEMPORAL_EMBEDDER_INDICES.contains(&1)); // E2
        assert!(TEMPORAL_EMBEDDER_INDICES.contains(&2)); // E3
        assert!(TEMPORAL_EMBEDDER_INDICES.contains(&3)); // E4
        println!("[PASS] TEMPORAL_EMBEDDER_INDICES correct");
    }

    #[test]
    fn test_rrf_k_constant() {
        assert!((RRF_K - 60.0).abs() < f32::EPSILON);
        println!("[PASS] RRF_K constant is 60.0");
    }
}
