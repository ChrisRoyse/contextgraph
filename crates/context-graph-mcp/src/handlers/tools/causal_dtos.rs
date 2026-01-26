//! DTOs for causal reasoning MCP tools.
//!
//! Per PRD v6 E5 Causal Enhancement, these DTOs support:
//! - search_causes: Abductive reasoning to find likely causes of observed effects
//! - get_causal_chain: Build and visualize transitive causal chains
//!
//! Constitution References:
//! - ARCH-15: Uses asymmetric E5 with separate cause/effect encodings
//! - AP-77: Direction modifiers: cause→effect=1.2, effect→cause=0.8

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_core::causal::asymmetric::CausalDirection;
use context_graph_core::causal::chain::HOP_ATTENUATION;
use context_graph_core::traits::SearchStrategy;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default topK for cause search results.
pub const DEFAULT_CAUSE_SEARCH_TOP_K: usize = 10;

/// Maximum topK for cause search results.
pub const MAX_CAUSE_SEARCH_TOP_K: usize = 50;

/// Minimum score threshold for cause search results.
pub const MIN_CAUSE_SCORE: f32 = 0.1;

/// Default max hops for causal chain traversal.
pub const DEFAULT_MAX_HOPS: usize = 5;

/// Maximum hops for causal chain traversal.
pub const MAX_HOPS: usize = 10;

/// Default minimum similarity for chain traversal.
pub const DEFAULT_MIN_CHAIN_SIMILARITY: f32 = 0.3;

/// Abductive dampening factor (effect→cause direction modifier).
/// Per AP-77: effect→cause = 0.8
pub const ABDUCTIVE_DAMPENING: f32 = 0.8;

// ============================================================================
// REQUEST DTOs
// ============================================================================

/// Request parameters for search_causes tool.
///
/// # Example JSON
/// ```json
/// {
///   "query": "The application crashed with a null pointer exception",
///   "topK": 10,
///   "minScore": 0.2,
///   "includeContent": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct SearchCausesRequest {
    /// The observed effect to find causes for (required).
    /// This should describe what happened that you want to explain.
    pub query: String,

    /// Maximum number of causes to return (1-50, default: 10).
    #[serde(rename = "topK", default = "default_top_k")]
    pub top_k: usize,

    /// Minimum abductive score threshold (0-1, default: 0.1).
    /// Results with scores below this are filtered out.
    #[serde(rename = "minScore", default = "default_min_score")]
    pub min_score: f32,

    /// Whether to include full content text in results (default: false).
    #[serde(rename = "includeContent", default)]
    pub include_content: bool,

    /// Optional filter for causal direction of results.
    /// - "cause": Only return memories marked as causes
    /// - "effect": Only return memories marked as effects (rare use case)
    /// - None: No filtering (default)
    #[serde(rename = "filterCausalDirection")]
    pub filter_causal_direction: Option<String>,

    /// Search strategy for retrieval.
    /// - "multi_space": Default multi-embedder fusion (backward compatible)
    /// - "pipeline": E13 SPLADE recall → E1 → E12 ColBERT rerank (maximum precision)
    ///
    /// When "pipeline" is selected, E12 reranking is automatically enabled.
    #[serde(default)]
    pub strategy: Option<String>,

    /// E12 rerank weight for blending with fusion score (0.0-1.0, default: 0.4).
    ///
    /// Controls how much weight E12 MaxSim scores have in the final ranking:
    /// - 0.0: Pure fusion score (E12 has no effect)
    /// - 0.4: Default - 60% fusion + 40% E12 MaxSim
    /// - 1.0: Pure E12 MaxSim score (ignores fusion)
    ///
    /// Only used when strategy="pipeline" (which auto-enables E12 reranking).
    #[serde(rename = "rerankWeight", default = "default_rerank_weight")]
    pub rerank_weight: f32,
}

/// Default E12 rerank weight (40% E12, 60% fusion).
fn default_rerank_weight() -> f32 {
    0.4
}

fn default_top_k() -> usize {
    DEFAULT_CAUSE_SEARCH_TOP_K
}

fn default_min_score() -> f32 {
    MIN_CAUSE_SCORE
}

impl Default for SearchCausesRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            top_k: DEFAULT_CAUSE_SEARCH_TOP_K,
            min_score: MIN_CAUSE_SCORE,
            include_content: false,
            filter_causal_direction: None,
            strategy: None,
            rerank_weight: default_rerank_weight(),
        }
    }
}

impl SearchCausesRequest {
    /// Parse the strategy parameter into SearchStrategy enum.
    ///
    /// Strategy selection priority:
    /// 1. User-specified strategy takes precedence
    /// 2. Default to MultiSpace for optimal blind spot detection (ARCH-21)
    pub fn parse_strategy(&self) -> SearchStrategy {
        // User-specified strategy takes precedence
        match self.strategy.as_deref() {
            Some("pipeline") => SearchStrategy::Pipeline,
            Some("e1_only") => SearchStrategy::E1Only,
            _ => SearchStrategy::MultiSpace, // Default to multi-space for optimal blind spot detection
        }
    }

    /// Validate the request parameters.
    ///
    /// # Errors
    /// Returns an error message if:
    /// - query is empty
    /// - topK is outside [1, 50]
    /// - minScore is outside [0, 1] or NaN/infinite
    pub fn validate(&self) -> Result<(), String> {
        if self.query.is_empty() {
            return Err("query is required and cannot be empty".to_string());
        }

        if self.top_k < 1 || self.top_k > MAX_CAUSE_SEARCH_TOP_K {
            return Err(format!(
                "topK must be between 1 and {}, got {}",
                MAX_CAUSE_SEARCH_TOP_K, self.top_k
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

        // Validate filter_causal_direction if provided
        if let Some(ref dir) = self.filter_causal_direction {
            let valid = ["cause", "effect", "unknown"];
            if !valid.contains(&dir.as_str()) {
                return Err(format!(
                    "filterCausalDirection must be one of {:?}, got '{}'",
                    valid, dir
                ));
            }
        }

        // Validate strategy if provided
        if let Some(ref strat) = self.strategy {
            let valid = ["multi_space", "pipeline"];
            if !valid.contains(&strat.as_str()) {
                return Err(format!(
                    "strategy must be one of {:?}, got '{}'",
                    valid, strat
                ));
            }
        }

        // Validate rerank_weight
        if self.rerank_weight.is_nan() || self.rerank_weight.is_infinite() {
            return Err("rerankWeight must be a finite number".to_string());
        }

        if self.rerank_weight < 0.0 || self.rerank_weight > 1.0 {
            return Err(format!(
                "rerankWeight must be between 0.0 and 1.0, got {}",
                self.rerank_weight
            ));
        }

        Ok(())
    }
}

/// Request parameters for get_causal_chain tool.
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
pub struct GetCausalChainRequest {
    /// UUID of the starting memory (anchor point) - required.
    #[serde(rename = "anchorId")]
    pub anchor_id: String,

    /// Direction to traverse the chain (default: "forward").
    /// - "forward": From cause to effects (A → B → C)
    /// - "backward": From effect to causes (C → B → A)
    #[serde(default = "default_direction")]
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

fn default_direction() -> String {
    "forward".to_string()
}

fn default_max_hops() -> usize {
    DEFAULT_MAX_HOPS
}

fn default_min_similarity() -> f32 {
    DEFAULT_MIN_CHAIN_SIMILARITY
}

impl Default for GetCausalChainRequest {
    fn default() -> Self {
        Self {
            anchor_id: String::new(),
            direction: "forward".to_string(),
            max_hops: DEFAULT_MAX_HOPS,
            min_similarity: DEFAULT_MIN_CHAIN_SIMILARITY,
            include_content: false,
        }
    }
}

impl GetCausalChainRequest {
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

    /// Returns true if traversing forward (cause → effect).
    pub fn is_forward(&self) -> bool {
        self.direction == "forward"
    }
}

// ============================================================================
// RESPONSE DTOs
// ============================================================================

/// A single cause result from search_causes.
#[derive(Debug, Clone, Serialize)]
pub struct CauseSearchResult {
    /// UUID of the candidate cause memory.
    pub cause_id: Uuid,

    /// Abductive score (with 0.8x dampening applied).
    /// Higher scores indicate more likely causes.
    pub score: f32,

    /// Raw similarity before dampening.
    pub raw_similarity: f32,

    /// Causal direction of this memory (if persisted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causal_direction: Option<String>,

    /// Full content text (only if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Source metadata for provenance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceInfo>,
}

/// Source information for a search result.
#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
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

/// Response for search_causes tool.
#[derive(Debug, Clone, Serialize)]
pub struct SearchCausesResponse {
    /// The effect query that was analyzed.
    pub query: String,

    /// Ranked list of candidate causes (highest score first).
    pub causes: Vec<CauseSearchResult>,

    /// Number of causes returned.
    pub count: usize,

    /// Metadata about the search.
    pub metadata: CauseSearchMetadata,
}

/// Metadata about a cause search operation.
#[derive(Debug, Clone, Serialize)]
pub struct CauseSearchMetadata {
    /// Number of candidates evaluated.
    pub candidates_evaluated: usize,

    /// Number filtered out by minScore.
    pub filtered_by_score: usize,

    /// The abductive dampening factor applied (0.8).
    pub abductive_dampening: f32,
}

/// A single hop in a causal chain.
#[derive(Debug, Clone, Serialize)]
pub struct CausalChainHop {
    /// UUID of the memory at this hop.
    pub memory_id: Uuid,

    /// 0-based index of this hop in the chain.
    pub hop_index: usize,

    /// Base similarity for this hop (before asymmetric adjustment).
    pub base_similarity: f32,

    /// Asymmetric E5 similarity for this hop.
    pub asymmetric_similarity: f32,

    /// Cumulative chain strength up to this hop.
    /// Computed as: product of all prior hop scores × attenuation^hop
    pub cumulative_strength: f32,

    /// Causal direction of this memory.
    pub causal_direction: String,

    /// Full content text (only if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

impl CausalChainHop {
    /// Create a new hop with computed cumulative strength.
    ///
    /// # Arguments
    /// * `memory_id` - UUID of the memory
    /// * `hop_index` - 0-based index
    /// * `base_similarity` - Raw cosine similarity
    /// * `asymmetric_similarity` - E5 asymmetric similarity
    /// * `prior_strength` - Cumulative strength from prior hops (1.0 for first hop)
    /// * `causal_direction` - Direction of this memory
    pub fn new(
        memory_id: Uuid,
        hop_index: usize,
        base_similarity: f32,
        asymmetric_similarity: f32,
        prior_strength: f32,
        causal_direction: CausalDirection,
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
            causal_direction: format!("{}", causal_direction),
            content: None,
        }
    }

    /// Add content to this hop.
    pub fn with_content(mut self, content: String) -> Self {
        self.content = Some(content);
        self
    }
}

/// Response for get_causal_chain tool.
#[derive(Debug, Clone, Serialize)]
pub struct GetCausalChainResponse {
    /// UUID of the anchor (starting) memory.
    pub anchor_id: Uuid,

    /// Direction of traversal ("forward" or "backward").
    pub direction: String,

    /// The hops in the causal chain.
    pub chain: Vec<CausalChainHop>,

    /// Total chain score (product of all hop scores with attenuation).
    pub total_chain_score: f32,

    /// Number of hops in the chain.
    pub hop_count: usize,

    /// Whether the chain was truncated (hit maxHops limit).
    pub truncated: bool,

    /// Metadata about the chain traversal.
    pub metadata: CausalChainMetadata,
}

/// Metadata about a causal chain traversal.
#[derive(Debug, Clone, Serialize)]
pub struct CausalChainMetadata {
    /// Max hops limit used.
    pub max_hops: usize,

    /// Minimum similarity threshold used.
    pub min_similarity: f32,

    /// Hop attenuation factor (0.9).
    pub hop_attenuation: f32,

    /// Number of candidates evaluated across all hops.
    pub total_candidates_evaluated: usize,
}

impl GetCausalChainResponse {
    /// Create an empty response (no chain found).
    pub fn empty(anchor_id: Uuid, direction: &str, max_hops: usize, min_similarity: f32) -> Self {
        Self {
            anchor_id,
            direction: direction.to_string(),
            chain: vec![],
            total_chain_score: 0.0,
            hop_count: 0,
            truncated: false,
            metadata: CausalChainMetadata {
                max_hops,
                min_similarity,
                hop_attenuation: HOP_ATTENUATION,
                total_candidates_evaluated: 0,
            },
        }
    }

    /// Compute total chain score from hops.
    pub fn compute_total_score(&self) -> f32 {
        if self.chain.is_empty() {
            return 0.0;
        }
        // The last hop's cumulative_strength is the total score
        self.chain.last().map(|h| h.cumulative_strength).unwrap_or(0.0)
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ===== SearchCausesRequest Tests =====

    #[test]
    fn test_search_causes_request_defaults() {
        let json = r#"{"query": "application crashed"}"#;
        let req: SearchCausesRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.query, "application crashed");
        assert_eq!(req.top_k, DEFAULT_CAUSE_SEARCH_TOP_K);
        assert!((req.min_score - MIN_CAUSE_SCORE).abs() < f32::EPSILON);
        assert!(!req.include_content);
        assert!(req.filter_causal_direction.is_none());
        println!("[PASS] SearchCausesRequest uses correct defaults");
    }

    #[test]
    fn test_search_causes_request_validation_valid() {
        let req = SearchCausesRequest {
            query: "test query".to_string(),
            top_k: 20,
            min_score: 0.5,
            include_content: true,
            filter_causal_direction: Some("cause".to_string()),
            strategy: Some("pipeline".to_string()),
            rerank_weight: 0.6, // Custom E12 weight
        };

        assert!(req.validate().is_ok());
        assert_eq!(req.parse_strategy(), SearchStrategy::Pipeline);
        assert!((req.rerank_weight - 0.6).abs() < f32::EPSILON);
        println!("[PASS] SearchCausesRequest validates correct input with pipeline strategy");
    }

    #[test]
    fn test_search_causes_request_validation_empty_query() {
        let req = SearchCausesRequest {
            query: "".to_string(),
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("required"));
        println!("[PASS] SearchCausesRequest rejects empty query");
    }

    #[test]
    fn test_search_causes_request_validation_topk_too_high() {
        let req = SearchCausesRequest {
            query: "test".to_string(),
            top_k: 100,
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("topK"));
        println!("[PASS] SearchCausesRequest rejects topK > 50");
    }

    #[test]
    fn test_search_causes_request_validation_invalid_direction() {
        let req = SearchCausesRequest {
            query: "test".to_string(),
            filter_causal_direction: Some("invalid".to_string()),
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("filterCausalDirection"));
        println!("[PASS] SearchCausesRequest rejects invalid causal direction filter");
    }

    #[test]
    fn test_search_causes_request_validation_invalid_strategy() {
        let req = SearchCausesRequest {
            query: "test".to_string(),
            strategy: Some("invalid_strategy".to_string()),
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("strategy"));
        println!("[PASS] SearchCausesRequest rejects invalid strategy");
    }

    #[test]
    fn test_parse_strategy_default() {
        let req = SearchCausesRequest::default();
        assert_eq!(req.parse_strategy(), SearchStrategy::MultiSpace);
        println!("[PASS] parse_strategy returns MultiSpace by default");
    }

    #[test]
    fn test_parse_strategy_no_strategy_defaults_to_multispace() {
        // Without explicit strategy, defaults to MultiSpace for optimal blind spot detection
        let req = SearchCausesRequest {
            query: "find \"ConnectionRefused\" error".to_string(),
            strategy: None, // No explicit strategy - defaults to MultiSpace
            ..Default::default()
        };
        assert_eq!(req.parse_strategy(), SearchStrategy::MultiSpace);
        println!("[PASS] parse_strategy defaults to MultiSpace for blind spot detection");
    }

    #[test]
    fn test_parse_strategy_user_specified_multispace() {
        // User explicitly specifying multi_space should be respected even with precision query
        let req = SearchCausesRequest {
            query: "find \"ConnectionRefused\" error".to_string(),
            strategy: Some("multi_space".to_string()), // Explicit multi_space
            ..Default::default()
        };
        assert_eq!(req.parse_strategy(), SearchStrategy::MultiSpace);
        println!("[PASS] parse_strategy respects user-specified multi_space");
    }

    #[test]
    fn test_parse_strategy_user_specified_pipeline() {
        // User explicitly specifying pipeline should work
        let req = SearchCausesRequest {
            query: "generic query".to_string(),
            strategy: Some("pipeline".to_string()), // Explicit pipeline
            ..Default::default()
        };
        assert_eq!(req.parse_strategy(), SearchStrategy::Pipeline);
        println!("[PASS] parse_strategy respects user-specified pipeline");
    }

    // ===== GetCausalChainRequest Tests =====

    #[test]
    fn test_get_causal_chain_request_defaults() {
        let json = r#"{"anchorId": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: GetCausalChainRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.anchor_id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(req.direction, "forward");
        assert_eq!(req.max_hops, DEFAULT_MAX_HOPS);
        assert!((req.min_similarity - DEFAULT_MIN_CHAIN_SIMILARITY).abs() < f32::EPSILON);
        assert!(!req.include_content);
        println!("[PASS] GetCausalChainRequest uses correct defaults");
    }

    #[test]
    fn test_get_causal_chain_request_validation_valid() {
        let req = GetCausalChainRequest {
            anchor_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            direction: "backward".to_string(),
            max_hops: 3,
            min_similarity: 0.5,
            include_content: true,
        };

        let result = req.validate();
        assert!(result.is_ok());
        println!("[PASS] GetCausalChainRequest validates correct input");
    }

    #[test]
    fn test_get_causal_chain_request_validation_invalid_uuid() {
        let req = GetCausalChainRequest {
            anchor_id: "not-a-uuid".to_string(),
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid UUID"));
        println!("[PASS] GetCausalChainRequest rejects invalid UUID");
    }

    #[test]
    fn test_get_causal_chain_request_validation_invalid_direction() {
        let req = GetCausalChainRequest {
            anchor_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            direction: "sideways".to_string(),
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("direction"));
        println!("[PASS] GetCausalChainRequest rejects invalid direction");
    }

    #[test]
    fn test_get_causal_chain_request_validation_max_hops_too_high() {
        let req = GetCausalChainRequest {
            anchor_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            max_hops: 20,
            ..Default::default()
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("maxHops"));
        println!("[PASS] GetCausalChainRequest rejects maxHops > 10");
    }

    #[test]
    fn test_is_forward() {
        let forward = GetCausalChainRequest {
            direction: "forward".to_string(),
            ..Default::default()
        };
        assert!(forward.is_forward());

        let backward = GetCausalChainRequest {
            direction: "backward".to_string(),
            ..Default::default()
        };
        assert!(!backward.is_forward());
        println!("[PASS] is_forward() works correctly");
    }

    // ===== CausalChainHop Tests =====

    #[test]
    fn test_causal_chain_hop_first_hop() {
        let hop = CausalChainHop::new(
            Uuid::nil(),
            0, // First hop
            0.8,
            0.85,
            1.0, // Prior strength is 1.0 for first hop
            CausalDirection::Cause,
        );

        // First hop: cumulative = 0.85 * 0.9^0 * 1.0 = 0.85
        assert!((hop.cumulative_strength - 0.85).abs() < 0.01);
        assert_eq!(hop.hop_index, 0);
        assert_eq!(hop.causal_direction, "cause");
        println!("[PASS] First hop computed correctly");
    }

    #[test]
    fn test_causal_chain_hop_attenuation() {
        // Second hop
        let hop = CausalChainHop::new(
            Uuid::nil(),
            1, // Second hop
            0.8,
            0.8,
            0.85, // Prior strength from first hop
            CausalDirection::Effect,
        );

        // Second hop: cumulative = 0.85 * 0.8 * 0.9^1 = 0.85 * 0.72 = 0.612
        let expected = 0.85 * 0.8 * 0.9;
        assert!((hop.cumulative_strength - expected).abs() < 0.01);
        println!("[PASS] Hop attenuation applied correctly: {}", hop.cumulative_strength);
    }

    #[test]
    fn test_causal_chain_hop_with_content() {
        let hop = CausalChainHop::new(
            Uuid::nil(),
            0,
            0.8,
            0.85,
            1.0,
            CausalDirection::Unknown,
        );

        let hop_with_content = hop.with_content("Test content".to_string());
        assert_eq!(hop_with_content.content, Some("Test content".to_string()));
        println!("[PASS] with_content() works");
    }

    // ===== Response Serialization Tests =====

    #[test]
    fn test_search_causes_response_serialization() {
        let response = SearchCausesResponse {
            query: "test query".to_string(),
            causes: vec![CauseSearchResult {
                cause_id: Uuid::nil(),
                score: 0.64,
                raw_similarity: 0.8,
                causal_direction: Some("cause".to_string()),
                content: None,
                source: None,
            }],
            count: 1,
            metadata: CauseSearchMetadata {
                candidates_evaluated: 100,
                filtered_by_score: 90,
                abductive_dampening: ABDUCTIVE_DAMPENING,
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"query\":\"test query\""));
        assert!(json.contains("\"abductive_dampening\":0.8"));
        println!("[PASS] SearchCausesResponse serializes correctly");
    }

    #[test]
    fn test_get_causal_chain_response_empty() {
        let anchor_id = Uuid::new_v4();
        let response = GetCausalChainResponse::empty(anchor_id, "forward", 5, 0.3);

        assert_eq!(response.hop_count, 0);
        assert!(!response.truncated);
        assert_eq!(response.compute_total_score(), 0.0);
        println!("[PASS] Empty chain response correct");
    }

    // ===== Constitution Compliance Tests =====

    #[test]
    fn test_abductive_dampening_matches_constitution() {
        // AP-77: effect→cause = 0.8
        assert!((ABDUCTIVE_DAMPENING - 0.8).abs() < f32::EPSILON);
        println!("[PASS] Abductive dampening matches Constitution (0.8)");
    }

    #[test]
    fn test_hop_attenuation_from_core() {
        // Verify we're using the same constant as core
        assert!((HOP_ATTENUATION - 0.9).abs() < f32::EPSILON);
        println!("[PASS] HOP_ATTENUATION matches core (0.9)");
    }
}
