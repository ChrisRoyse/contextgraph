//! DTOs for graph reasoning MCP tools.
//!
//! Per E8 upgrade specification (Phase 4), these DTOs support:
//! - search_connections: Find memories connected to a given concept
//! - get_graph_path: Multi-hop graph traversal between memories
//!
//! Constitution References:
//! - ARCH-15: Uses asymmetric E8 with separate source/target encodings
//! - AP-77: Direction modifiers: source→target=1.2, target→source=0.8

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_core::graph::asymmetric::GraphDirection;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default topK for connection search results.
pub const DEFAULT_CONNECTION_SEARCH_TOP_K: usize = 10;

/// Maximum topK for connection search results.
pub const MAX_CONNECTION_SEARCH_TOP_K: usize = 50;

/// Minimum score threshold for connection search results.
pub const MIN_CONNECTION_SCORE: f32 = 0.1;

/// Default max hops for graph path traversal.
pub const DEFAULT_MAX_HOPS: usize = 5;

/// Maximum hops for graph path traversal.
pub const MAX_HOPS: usize = 10;

/// Default minimum similarity for path traversal.
pub const DEFAULT_MIN_PATH_SIMILARITY: f32 = 0.3;

/// Hop attenuation factor for graph paths.
pub const HOP_ATTENUATION: f32 = 0.9;

/// Source direction modifier (per E8 Constitution spec).
pub const SOURCE_DIRECTION_MODIFIER: f32 = 1.2;

/// Target direction modifier (per E8 Constitution spec).
pub const TARGET_DIRECTION_MODIFIER: f32 = 0.8;

// ============================================================================
// REQUEST DTOs
// ============================================================================

/// Request parameters for search_connections tool.
///
/// # Example JSON
/// ```json
/// {
///   "query": "What modules import utils?",
///   "direction": "source",
///   "topK": 10,
///   "minScore": 0.2,
///   "includeContent": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct SearchConnectionsRequest {
    /// The query to find connections for (required).
    /// Can be a concept name or a structural query like "what imports X".
    pub query: String,

    /// Connection direction to search (default: "both").
    /// - "source": Find memories that point TO the query concept
    /// - "target": Find memories that the query concept points TO
    /// - "both": Find both incoming and outgoing connections
    #[serde(default = "default_direction")]
    pub direction: String,

    /// Maximum number of connections to return (1-50, default: 10).
    #[serde(rename = "topK", default = "default_top_k")]
    pub top_k: usize,

    /// Minimum connection score threshold (0-1, default: 0.1).
    /// Results with scores below this are filtered out.
    #[serde(rename = "minScore", default = "default_min_score")]
    pub min_score: f32,

    /// Whether to include full content text in results (default: false).
    #[serde(rename = "includeContent", default)]
    pub include_content: bool,

    /// Optional filter for graph direction of results.
    /// - "source": Only return memories that act as sources
    /// - "target": Only return memories that act as targets
    /// - None: No filtering (default)
    #[serde(rename = "filterGraphDirection")]
    pub filter_graph_direction: Option<String>,
}

fn default_direction() -> String {
    "both".to_string()
}

fn default_top_k() -> usize {
    DEFAULT_CONNECTION_SEARCH_TOP_K
}

fn default_min_score() -> f32 {
    MIN_CONNECTION_SCORE
}

impl Default for SearchConnectionsRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            direction: "both".to_string(),
            top_k: DEFAULT_CONNECTION_SEARCH_TOP_K,
            min_score: MIN_CONNECTION_SCORE,
            include_content: false,
            filter_graph_direction: None,
        }
    }
}

impl SearchConnectionsRequest {
    /// Validate the request parameters.
    ///
    /// # Errors
    /// Returns an error message if:
    /// - query is empty
    /// - direction is not "source", "target", or "both"
    /// - topK is outside [1, 50]
    /// - minScore is outside [0, 1] or NaN/infinite
    pub fn validate(&self) -> Result<(), String> {
        if self.query.is_empty() {
            return Err("query is required and cannot be empty".to_string());
        }

        let valid_directions = ["source", "target", "both"];
        if !valid_directions.contains(&self.direction.as_str()) {
            return Err(format!(
                "direction must be one of {:?}, got '{}'",
                valid_directions, self.direction
            ));
        }

        if self.top_k < 1 || self.top_k > MAX_CONNECTION_SEARCH_TOP_K {
            return Err(format!(
                "topK must be between 1 and {}, got {}",
                MAX_CONNECTION_SEARCH_TOP_K, self.top_k
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

        // Validate filter_graph_direction if provided
        if let Some(ref dir) = self.filter_graph_direction {
            let valid = ["source", "target", "unknown"];
            if !valid.contains(&dir.as_str()) {
                return Err(format!(
                    "filterGraphDirection must be one of {:?}, got '{}'",
                    valid, dir
                ));
            }
        }

        Ok(())
    }

    /// Returns true if searching for sources (incoming connections).
    pub fn is_source(&self) -> bool {
        self.direction == "source" || self.direction == "both"
    }

    /// Returns true if searching for targets (outgoing connections).
    pub fn is_target(&self) -> bool {
        self.direction == "target" || self.direction == "both"
    }
}

/// Request parameters for get_graph_path tool.
///
/// # Example JSON
/// ```json
/// {
///   "anchorId": "550e8400-e29b-41d4-a716-446655440000",
///   "direction": "forward",
///   "maxHops": 5,
///   "minSimilarity": 0.3,
///   "includeContent": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct GetGraphPathRequest {
    /// UUID of the starting memory (anchor point) - required.
    #[serde(rename = "anchorId")]
    pub anchor_id: String,

    /// Direction to traverse the path (default: "forward").
    /// - "forward": From source to targets (A → B → C)
    /// - "backward": From target to sources (C → B → A)
    #[serde(default = "default_path_direction")]
    pub direction: String,

    /// Maximum number of hops to traverse (1-10, default: 5).
    #[serde(rename = "maxHops", default = "default_max_hops")]
    pub max_hops: usize,

    /// Minimum similarity threshold for each hop (0-1, default: 0.3).
    #[serde(rename = "minSimilarity", default = "default_min_similarity")]
    pub min_similarity: f32,

    /// Whether to include full content text in results (default: false).
    #[serde(rename = "includeContent", default)]
    pub include_content: bool,
}

fn default_path_direction() -> String {
    "forward".to_string()
}

fn default_max_hops() -> usize {
    DEFAULT_MAX_HOPS
}

fn default_min_similarity() -> f32 {
    DEFAULT_MIN_PATH_SIMILARITY
}

impl Default for GetGraphPathRequest {
    fn default() -> Self {
        Self {
            anchor_id: String::new(),
            direction: "forward".to_string(),
            max_hops: DEFAULT_MAX_HOPS,
            min_similarity: DEFAULT_MIN_PATH_SIMILARITY,
            include_content: false,
        }
    }
}

impl GetGraphPathRequest {
    /// Validate the request parameters.
    ///
    /// # Errors
    /// Returns an error message if:
    /// - anchorId is not a valid UUID
    /// - direction is not "forward" or "backward"
    /// - maxHops is outside [1, 10]
    /// - minSimilarity is outside [0, 1] or NaN/infinite
    pub fn validate(&self) -> Result<Uuid, String> {
        // Validate anchor UUID
        let anchor_uuid = Uuid::parse_str(&self.anchor_id)
            .map_err(|e| format!("Invalid UUID format for anchorId '{}': {}", self.anchor_id, e))?;

        // Validate direction
        let valid_directions = ["forward", "backward"];
        if !valid_directions.contains(&self.direction.as_str()) {
            return Err(format!(
                "direction must be one of {:?}, got '{}'",
                valid_directions, self.direction
            ));
        }

        // Validate maxHops
        if self.max_hops < 1 || self.max_hops > MAX_HOPS {
            return Err(format!(
                "maxHops must be between 1 and {}, got {}",
                MAX_HOPS, self.max_hops
            ));
        }

        // Validate minSimilarity
        if self.min_similarity.is_nan() || self.min_similarity.is_infinite() {
            return Err("minSimilarity must be a finite number".to_string());
        }

        if self.min_similarity < 0.0 || self.min_similarity > 1.0 {
            return Err(format!(
                "minSimilarity must be between 0.0 and 1.0, got {}",
                self.min_similarity
            ));
        }

        Ok(anchor_uuid)
    }

    /// Returns true if traversing forward (source → target).
    pub fn is_forward(&self) -> bool {
        self.direction == "forward"
    }
}

// ============================================================================
// RESPONSE DTOs
// ============================================================================

/// A single connection result from search_connections.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionSearchResult {
    /// UUID of the connected memory.
    pub connection_id: Uuid,

    /// Connection score (with direction modifier applied).
    /// Higher scores indicate stronger connections.
    pub score: f32,

    /// Raw similarity before direction modifier.
    pub raw_similarity: f32,

    /// Graph direction of this memory (if persisted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_direction: Option<String>,

    /// Full content text (only if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Source metadata for provenance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<GraphSourceInfo>,
}

/// Source information for a graph search result.
#[derive(Debug, Clone, Serialize)]
pub struct GraphSourceInfo {
    /// Source type (MDFileChunk, HookDescription, etc.)
    #[serde(rename = "type")]
    pub source_type: String,

    /// File path for MDFileChunk sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,

    /// Hook type for HookDescription sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_type: Option<String>,

    /// Tool name for tool-related hook sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

/// Response for search_connections tool.
#[derive(Debug, Clone, Serialize)]
pub struct SearchConnectionsResponse {
    /// The query that was analyzed.
    pub query: String,

    /// Search direction used.
    pub direction: String,

    /// Ranked list of connected memories (highest score first).
    pub connections: Vec<ConnectionSearchResult>,

    /// Number of connections returned.
    pub count: usize,

    /// Metadata about the search.
    pub metadata: ConnectionSearchMetadata,
}

/// Metadata about a connection search operation.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionSearchMetadata {
    /// Number of candidates evaluated.
    pub candidates_evaluated: usize,

    /// Number filtered out by minScore.
    pub filtered_by_score: usize,

    /// Direction modifier applied (1.2 for source→target, 0.8 for target→source).
    pub direction_modifier: f32,
}

/// A single hop in a graph path.
#[derive(Debug, Clone, Serialize)]
pub struct GraphPathHop {
    /// UUID of the memory at this hop.
    pub memory_id: Uuid,

    /// 0-based index of this hop in the path.
    pub hop_index: usize,

    /// Base similarity for this hop (before asymmetric adjustment).
    pub base_similarity: f32,

    /// Asymmetric E8 similarity for this hop.
    pub asymmetric_similarity: f32,

    /// Cumulative path strength up to this hop.
    /// Computed as: product of all prior hop scores × attenuation^hop
    pub cumulative_strength: f32,

    /// Graph direction of this memory.
    pub graph_direction: String,

    /// Full content text (only if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

impl GraphPathHop {
    /// Create a new hop with computed cumulative strength.
    ///
    /// # Arguments
    /// * `memory_id` - UUID of the memory
    /// * `hop_index` - 0-based index
    /// * `base_similarity` - Raw cosine similarity
    /// * `asymmetric_similarity` - E8 asymmetric similarity
    /// * `prior_strength` - Cumulative strength from prior hops (1.0 for first hop)
    /// * `graph_direction` - Direction of this memory
    pub fn new(
        memory_id: Uuid,
        hop_index: usize,
        base_similarity: f32,
        asymmetric_similarity: f32,
        prior_strength: f32,
        graph_direction: GraphDirection,
    ) -> Self {
        // Apply hop attenuation: strength × 0.9^hop
        let attenuation = HOP_ATTENUATION.powi(hop_index as i32);
        let hop_contribution = asymmetric_similarity * attenuation;
        let cumulative_strength = prior_strength * hop_contribution;

        Self {
            memory_id,
            hop_index,
            base_similarity,
            asymmetric_similarity,
            cumulative_strength,
            graph_direction: format!("{}", graph_direction),
            content: None,
        }
    }

    /// Add content to this hop.
    pub fn with_content(mut self, content: String) -> Self {
        self.content = Some(content);
        self
    }
}

/// Response for get_graph_path tool.
#[derive(Debug, Clone, Serialize)]
pub struct GetGraphPathResponse {
    /// UUID of the anchor (starting) memory.
    pub anchor_id: Uuid,

    /// Direction of traversal ("forward" or "backward").
    pub direction: String,

    /// The hops in the graph path.
    pub path: Vec<GraphPathHop>,

    /// Total path score (product of all hop scores with attenuation).
    pub total_path_score: f32,

    /// Number of hops in the path.
    pub hop_count: usize,

    /// Whether the path was truncated (hit maxHops limit).
    pub truncated: bool,

    /// Metadata about the path traversal.
    pub metadata: GraphPathMetadata,
}

/// Metadata about a graph path traversal.
#[derive(Debug, Clone, Serialize)]
pub struct GraphPathMetadata {
    /// Max hops limit used.
    pub max_hops: usize,

    /// Minimum similarity threshold used.
    pub min_similarity: f32,

    /// Hop attenuation factor (0.9).
    pub hop_attenuation: f32,

    /// Number of candidates evaluated across all hops.
    pub total_candidates_evaluated: usize,
}

impl GetGraphPathResponse {
    /// Create an empty response (no path found).
    pub fn empty(anchor_id: Uuid, direction: &str, max_hops: usize, min_similarity: f32) -> Self {
        Self {
            anchor_id,
            direction: direction.to_string(),
            path: vec![],
            total_path_score: 0.0,
            hop_count: 0,
            truncated: false,
            metadata: GraphPathMetadata {
                max_hops,
                min_similarity,
                hop_attenuation: HOP_ATTENUATION,
                total_candidates_evaluated: 0,
            },
        }
    }

    /// Compute total path score from hops.
    pub fn compute_total_score(&self) -> f32 {
        if self.path.is_empty() {
            return 0.0;
        }
        // The last hop's cumulative_strength is the total score
        self.path.last().map(|h| h.cumulative_strength).unwrap_or(0.0)
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ===== SearchConnectionsRequest Tests =====

    #[test]
    fn test_search_connections_request_defaults() {
        let json = r#"{"query": "what imports utils"}"#;
        let req: SearchConnectionsRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.query, "what imports utils");
        assert_eq!(req.direction, "both");
        assert_eq!(req.top_k, DEFAULT_CONNECTION_SEARCH_TOP_K);
        assert!((req.min_score - MIN_CONNECTION_SCORE).abs() < f32::EPSILON);
        assert!(!req.include_content);
        assert!(req.filter_graph_direction.is_none());
        println!("[PASS] SearchConnectionsRequest uses correct defaults");
    }

    #[test]
    fn test_search_connections_request_validation_valid() {
        let req = SearchConnectionsRequest {
            query: "test query".to_string(),
            direction: "source".to_string(),
            top_k: 20,
            min_score: 0.5,
            include_content: true,
            filter_graph_direction: Some("source".to_string()),
        };

        assert!(req.validate().is_ok());
        println!("[PASS] SearchConnectionsRequest validates correct input");
    }

    #[test]
    fn test_search_connections_request_validation_empty_query() {
        let req = SearchConnectionsRequest {
            query: "".to_string(),
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("required"));
        println!("[PASS] SearchConnectionsRequest rejects empty query");
    }

    #[test]
    fn test_search_connections_request_validation_invalid_direction() {
        let req = SearchConnectionsRequest {
            query: "test".to_string(),
            direction: "sideways".to_string(),
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("direction"));
        println!("[PASS] SearchConnectionsRequest rejects invalid direction");
    }

    #[test]
    fn test_search_connections_request_validation_topk_too_high() {
        let req = SearchConnectionsRequest {
            query: "test".to_string(),
            top_k: 100,
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("topK"));
        println!("[PASS] SearchConnectionsRequest rejects topK > 50");
    }

    #[test]
    fn test_is_source_and_is_target() {
        let source = SearchConnectionsRequest {
            query: "test".to_string(),
            direction: "source".to_string(),
            ..Default::default()
        };
        assert!(source.is_source());
        assert!(!source.is_target());

        let target = SearchConnectionsRequest {
            query: "test".to_string(),
            direction: "target".to_string(),
            ..Default::default()
        };
        assert!(!target.is_source());
        assert!(target.is_target());

        let both = SearchConnectionsRequest {
            query: "test".to_string(),
            direction: "both".to_string(),
            ..Default::default()
        };
        assert!(both.is_source());
        assert!(both.is_target());

        println!("[PASS] is_source and is_target work correctly");
    }

    // ===== GetGraphPathRequest Tests =====

    #[test]
    fn test_get_graph_path_request_defaults() {
        let json = r#"{"anchorId": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: GetGraphPathRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.anchor_id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(req.direction, "forward");
        assert_eq!(req.max_hops, DEFAULT_MAX_HOPS);
        assert!((req.min_similarity - DEFAULT_MIN_PATH_SIMILARITY).abs() < f32::EPSILON);
        assert!(!req.include_content);
        println!("[PASS] GetGraphPathRequest uses correct defaults");
    }

    #[test]
    fn test_get_graph_path_request_validation_valid() {
        let req = GetGraphPathRequest {
            anchor_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            direction: "backward".to_string(),
            max_hops: 3,
            min_similarity: 0.5,
            include_content: true,
        };

        let result = req.validate();
        assert!(result.is_ok());
        println!("[PASS] GetGraphPathRequest validates correct input");
    }

    #[test]
    fn test_get_graph_path_request_validation_invalid_uuid() {
        let req = GetGraphPathRequest {
            anchor_id: "not-a-uuid".to_string(),
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid UUID"));
        println!("[PASS] GetGraphPathRequest rejects invalid UUID");
    }

    #[test]
    fn test_get_graph_path_request_validation_max_hops_too_high() {
        let req = GetGraphPathRequest {
            anchor_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            max_hops: 20,
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("maxHops"));
        println!("[PASS] GetGraphPathRequest rejects maxHops > 10");
    }

    #[test]
    fn test_is_forward() {
        let forward = GetGraphPathRequest {
            direction: "forward".to_string(),
            ..Default::default()
        };
        assert!(forward.is_forward());

        let backward = GetGraphPathRequest {
            direction: "backward".to_string(),
            ..Default::default()
        };
        assert!(!backward.is_forward());
        println!("[PASS] is_forward() works correctly");
    }

    // ===== GraphPathHop Tests =====

    #[test]
    fn test_graph_path_hop_first_hop() {
        let hop = GraphPathHop::new(
            Uuid::nil(),
            0, // First hop
            0.8,
            0.85,
            1.0, // Prior strength is 1.0 for first hop
            GraphDirection::Source,
        );

        // First hop: cumulative = 0.85 * 0.9^0 * 1.0 = 0.85
        assert!((hop.cumulative_strength - 0.85).abs() < 0.01);
        assert_eq!(hop.hop_index, 0);
        assert_eq!(hop.graph_direction, "source");
        println!("[PASS] First hop computed correctly");
    }

    #[test]
    fn test_graph_path_hop_attenuation() {
        // Second hop
        let hop = GraphPathHop::new(
            Uuid::nil(),
            1, // Second hop
            0.8,
            0.8,
            0.85, // Prior strength from first hop
            GraphDirection::Target,
        );

        // Second hop: cumulative = 0.85 * 0.8 * 0.9^1 = 0.85 * 0.72 = 0.612
        let expected = 0.85 * 0.8 * 0.9;
        assert!((hop.cumulative_strength - expected).abs() < 0.01);
        println!("[PASS] Hop attenuation applied correctly: {}", hop.cumulative_strength);
    }

    #[test]
    fn test_graph_path_hop_with_content() {
        let hop = GraphPathHop::new(
            Uuid::nil(),
            0,
            0.8,
            0.85,
            1.0,
            GraphDirection::Unknown,
        );

        let hop_with_content = hop.with_content("Test content".to_string());
        assert_eq!(hop_with_content.content, Some("Test content".to_string()));
        println!("[PASS] with_content() works");
    }

    // ===== Response Tests =====

    #[test]
    fn test_search_connections_response_serialization() {
        let response = SearchConnectionsResponse {
            query: "test query".to_string(),
            direction: "both".to_string(),
            connections: vec![ConnectionSearchResult {
                connection_id: Uuid::nil(),
                score: 0.72,
                raw_similarity: 0.8,
                graph_direction: Some("source".to_string()),
                content: None,
                source: None,
            }],
            count: 1,
            metadata: ConnectionSearchMetadata {
                candidates_evaluated: 100,
                filtered_by_score: 90,
                direction_modifier: SOURCE_DIRECTION_MODIFIER,
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"query\":\"test query\""));
        assert!(json.contains("\"direction_modifier\":1.2"));
        println!("[PASS] SearchConnectionsResponse serializes correctly");
    }

    #[test]
    fn test_get_graph_path_response_empty() {
        let anchor_id = Uuid::new_v4();
        let response = GetGraphPathResponse::empty(anchor_id, "forward", 5, 0.3);

        assert_eq!(response.hop_count, 0);
        assert!(!response.truncated);
        assert_eq!(response.compute_total_score(), 0.0);
        println!("[PASS] Empty path response correct");
    }

    // ===== Constitution Compliance Tests =====

    #[test]
    fn test_direction_modifiers_match_constitution() {
        // AP-77: source→target = 1.2, target→source = 0.8
        assert!((SOURCE_DIRECTION_MODIFIER - 1.2).abs() < f32::EPSILON);
        assert!((TARGET_DIRECTION_MODIFIER - 0.8).abs() < f32::EPSILON);
        println!("[PASS] Direction modifiers match Constitution (1.2/0.8)");
    }

    #[test]
    fn test_hop_attenuation_value() {
        assert!((HOP_ATTENUATION - 0.9).abs() < f32::EPSILON);
        println!("[PASS] HOP_ATTENUATION is 0.9");
    }
}
