//! DTOs for E9 robustness/blind-spot detection MCP tools.
//!
//! Per Constitution v6.5 and the 13-embedder philosophy:
//! - E1 is the semantic foundation
//! - E9 (HDC) finds what E1 MISSES due to character-level issues
//! - E9 doesn't compete with E1; it discovers E1's blind spots
//!
//! # The Blind Spot Pattern
//!
//! E9 finds memories where:
//! - E9 score is HIGH (strong character-level match)
//! - E1 score is LOW (semantic match failed)
//! - These are E1's blind spots that E9 discovered
//!
//! # Examples
//!
//! - Query "authetication" finds "authentication" (typo tolerance)
//! - Query "parseConfig" finds "parse_config" (code identifier matching)
//!
//! # Constitution Compliance
//!
//! - ARCH-12: E1 remains foundation; E9 enhances by finding blind spots
//! - Philosophy: E9 finds what E1 misses, doesn't compete with E1
//! - FAIL FAST: All errors propagate immediately with logging

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_core::traits::SearchStrategy;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default topK for robust search results.
pub const DEFAULT_ROBUST_SEARCH_TOP_K: usize = 10;

/// Maximum topK for robust search results.
pub const MAX_ROBUST_SEARCH_TOP_K: usize = 50;

/// Default minimum score threshold for results.
pub const DEFAULT_MIN_ROBUST_SCORE: f32 = 0.1;

/// Minimum E9 score for a result to be considered an "E9 discovery".
/// Results with E9 score >= this AND E1 score < E1_WEAKNESS_THRESHOLD
/// are marked as blind spots E9 found.
///
/// NOTE: The projected E9 vectors (1024D) have lower cosine similarity than
/// native Hamming similarity (10K bits). A native similarity of 0.58 maps to
/// projected cosine of ~0.16. This threshold is calibrated for projected vectors.
pub const E9_DISCOVERY_THRESHOLD: f32 = 0.15;

/// Maximum E1 score for a result to be considered "missed" by E1.
/// If E1 would have found it (score >= this), it's not a blind spot.
pub const E1_WEAKNESS_THRESHOLD: f32 = 0.5;

/// Minimum query length for E9 trigram encoding.
pub const MIN_QUERY_LENGTH: usize = 3;

// ============================================================================
// REQUEST DTOs
// ============================================================================

/// Request parameters for search_robust tool.
///
/// # Example JSON
/// ```json
/// {
///   "query": "authetication failed",
///   "topK": 10,
///   "minScore": 0.1,
///   "includeContent": true,
///   "includeE9Score": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct SearchRobustRequest {
    /// The query text (typos are OK - E9 is noise-tolerant).
    /// Minimum 3 characters for trigram encoding.
    pub query: String,

    /// Maximum number of results to return (1-50, default: 10).
    #[serde(rename = "topK", default = "default_top_k")]
    pub top_k: usize,

    /// Minimum score threshold (0-1, default: 0.1).
    #[serde(rename = "minScore", default = "default_min_score")]
    pub min_score: f32,

    /// Whether to include full content text in results (default: false).
    #[serde(rename = "includeContent", default)]
    pub include_content: bool,

    /// Whether to include separate E9 and E1 scores (default: true).
    /// Useful for understanding where E9 found blind spots.
    #[serde(rename = "includeE9Score", default = "default_include_e9_score")]
    pub include_e9_score: bool,

    /// E9 discovery threshold override (default: 0.15).
    /// Minimum E9 score for a result to be marked as "E9 discovery".
    /// Note: Projected E9 vectors have lower cosine similarity than native Hamming.
    #[serde(rename = "e9DiscoveryThreshold", default = "default_e9_threshold")]
    pub e9_discovery_threshold: f32,

    /// E1 weakness threshold override (default: 0.5).
    /// Maximum E1 score for a result to be considered "missed" by E1.
    #[serde(rename = "e1WeaknessThreshold", default = "default_e1_threshold")]
    pub e1_weakness_threshold: f32,

    /// Search strategy for retrieval.
    /// - "multi_space": Default multi-embedder fusion
    /// - "pipeline": E13 SPLADE recall → E1 → E12 ColBERT rerank (maximum precision)
    ///
    /// When "pipeline" is selected, E12 reranking is automatically enabled.
    #[serde(default)]
    pub strategy: Option<String>,
}

fn default_top_k() -> usize {
    DEFAULT_ROBUST_SEARCH_TOP_K
}

fn default_min_score() -> f32 {
    DEFAULT_MIN_ROBUST_SCORE
}

fn default_include_e9_score() -> bool {
    true
}

fn default_e9_threshold() -> f32 {
    E9_DISCOVERY_THRESHOLD
}

fn default_e1_threshold() -> f32 {
    E1_WEAKNESS_THRESHOLD
}

impl Default for SearchRobustRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            top_k: DEFAULT_ROBUST_SEARCH_TOP_K,
            min_score: DEFAULT_MIN_ROBUST_SCORE,
            include_content: false,
            include_e9_score: true,
            e9_discovery_threshold: E9_DISCOVERY_THRESHOLD,
            e1_weakness_threshold: E1_WEAKNESS_THRESHOLD,
            strategy: None,
        }
    }
}

impl SearchRobustRequest {
    /// Parse the strategy parameter into SearchStrategy enum.
    ///
    /// Strategy selection priority:
    /// 1. User-specified strategy takes precedence
    /// 2. Auto-upgrade to Pipeline if query is a precision query (quoted terms, keyword patterns)
    /// 2. Default to MultiSpace for E9 enhancement (ARCH-21)
    pub fn parse_strategy(&self) -> SearchStrategy {
        // User-specified strategy takes precedence
        match self.strategy.as_deref() {
            Some("pipeline") => SearchStrategy::Pipeline,
            Some("e1_only") => SearchStrategy::E1Only,
            _ => SearchStrategy::MultiSpace, // Default to multi-space for E9 enhancement
        }
    }

    /// Validate the request parameters.
    ///
    /// # Errors
    /// Returns an error message if:
    /// - query is too short (< 3 chars for trigrams)
    /// - topK is outside [1, 50]
    /// - thresholds are outside [0, 1] or invalid
    pub fn validate(&self) -> Result<(), String> {
        if self.query.len() < MIN_QUERY_LENGTH {
            return Err(format!(
                "query must be at least {} characters for E9 trigram encoding, got {}",
                MIN_QUERY_LENGTH,
                self.query.len()
            ));
        }

        if self.top_k < 1 || self.top_k > MAX_ROBUST_SEARCH_TOP_K {
            return Err(format!(
                "topK must be between 1 and {}, got {}",
                MAX_ROBUST_SEARCH_TOP_K, self.top_k
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

        if self.e9_discovery_threshold < 0.0 || self.e9_discovery_threshold > 1.0 {
            return Err(format!(
                "e9DiscoveryThreshold must be between 0.0 and 1.0, got {}",
                self.e9_discovery_threshold
            ));
        }

        if self.e1_weakness_threshold < 0.0 || self.e1_weakness_threshold > 1.0 {
            return Err(format!(
                "e1WeaknessThreshold must be between 0.0 and 1.0, got {}",
                self.e1_weakness_threshold
            ));
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

        Ok(())
    }
}

// ============================================================================
// RESPONSE DTOs
// ============================================================================

/// Source of a search result - either E1 semantic or E9 discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResultSource {
    /// Found by E1 semantic search (normal result).
    E1,
    /// Discovered by E9 - E1's blind spot (high E9, low E1).
    E9Discovery,
}

/// A single search result from search_robust.
#[derive(Debug, Clone, Serialize)]
pub struct RobustSearchResult {
    /// UUID of the matched memory.
    #[serde(rename = "memoryId")]
    pub memory_id: Uuid,

    /// Final score for ranking (E1 score for E1 results, E9 score for discoveries).
    pub score: f32,

    /// Source of this result.
    pub source: ResultSource,

    /// E9 similarity score (character-level structural match).
    /// Only included if includeE9Score=true.
    #[serde(rename = "e9Score", skip_serializing_if = "Option::is_none")]
    pub e9_score: Option<f32>,

    /// E1 similarity score (semantic match).
    /// Only included if includeE9Score=true.
    #[serde(rename = "e1Score", skip_serializing_if = "Option::is_none")]
    pub e1_score: Option<f32>,

    /// Divergence score (E9 - E1). Higher = more of a blind spot.
    /// Only included for E9_DISCOVERY results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub divergence: Option<f32>,

    /// Why this was marked as an E9 discovery.
    /// Only included for E9_DISCOVERY results.
    #[serde(rename = "discoveryReason", skip_serializing_if = "Option::is_none")]
    pub discovery_reason: Option<String>,

    /// Full content text (if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Source provenance information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<RobustSourceInfo>,
}

/// Source provenance information.
#[derive(Debug, Clone, Serialize)]
pub struct RobustSourceInfo {
    /// Type of source (HookDescription, ClaudeResponse, MDFileChunk).
    #[serde(rename = "sourceType")]
    pub source_type: String,

    /// File path if from file source.
    #[serde(skip_serializing_if = "Option::is_none", rename = "filePath")]
    pub file_path: Option<String>,

    /// Hook type if from hook source.
    #[serde(skip_serializing_if = "Option::is_none", rename = "hookType")]
    pub hook_type: Option<String>,

    /// Tool name if from tool use.
    #[serde(skip_serializing_if = "Option::is_none", rename = "toolName")]
    pub tool_name: Option<String>,
}

/// Response metadata for search_robust.
#[derive(Debug, Clone, Serialize)]
pub struct RobustSearchMetadata {
    /// Number of results from E1 semantic search.
    #[serde(rename = "e1ResultsCount")]
    pub e1_results_count: usize,

    /// Number of blind spots E9 discovered that E1 missed.
    #[serde(rename = "e9DiscoveriesCount")]
    pub e9_discoveries_count: usize,

    /// UUIDs of memories E9 discovered (blind spots).
    #[serde(rename = "blindSpotsFound")]
    pub blind_spots_found: Vec<Uuid>,

    /// Total candidates evaluated from E1 search.
    #[serde(rename = "e1CandidatesEvaluated")]
    pub e1_candidates_evaluated: usize,

    /// Total candidates evaluated from E9 search.
    #[serde(rename = "e9CandidatesEvaluated")]
    pub e9_candidates_evaluated: usize,

    /// E9 discovery threshold used.
    #[serde(rename = "e9DiscoveryThreshold")]
    pub e9_discovery_threshold: f32,

    /// E1 weakness threshold used.
    #[serde(rename = "e1WeaknessThreshold")]
    pub e1_weakness_threshold: f32,
}

/// Response for search_robust tool.
#[derive(Debug, Clone, Serialize)]
pub struct SearchRobustResponse {
    /// Original query.
    pub query: String,

    /// Combined results: E1 results + E9 discoveries.
    pub results: Vec<RobustSearchResult>,

    /// Total number of results returned.
    pub count: usize,

    /// Metadata about the search.
    pub metadata: RobustSearchMetadata,
}

// ============================================================================
// INTERNAL TYPES
// ============================================================================

/// Intermediate struct for blind spot detection.
#[derive(Debug, Clone)]
pub struct BlindSpotCandidate {
    pub memory_id: Uuid,
    pub e9_score: f32,
    pub e1_score: f32,
    pub divergence: f32,
}

impl BlindSpotCandidate {
    /// Check if this candidate qualifies as an E9 discovery.
    pub fn is_discovery(&self, e9_threshold: f32, e1_threshold: f32) -> bool {
        // A discovery is when E9 found something (score >= threshold)
        // but E1 missed it (score < weakness threshold).
        //
        // NOTE: With projected E9 vectors, E9 scores are much lower than E1 scores
        // in absolute terms, so divergence (E9-E1) is often negative. We don't
        // require positive divergence; the E9/E1 thresholds capture the semantics.
        self.e9_score >= e9_threshold && self.e1_score < e1_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_robust_validation_success() {
        let req = SearchRobustRequest {
            query: "authentication".to_string(),
            ..Default::default()
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_search_robust_query_too_short() {
        let req = SearchRobustRequest {
            query: "ab".to_string(),
            ..Default::default()
        };
        let err = req.validate().unwrap_err();
        assert!(err.contains("at least 3 characters"));
    }

    #[test]
    fn test_search_robust_invalid_top_k() {
        let req = SearchRobustRequest {
            query: "test query".to_string(),
            top_k: 100,
            ..Default::default()
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_search_robust_invalid_threshold() {
        let req = SearchRobustRequest {
            query: "test query".to_string(),
            e9_discovery_threshold: 1.5,
            ..Default::default()
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_blind_spot_candidate_is_discovery() {
        let candidate = BlindSpotCandidate {
            memory_id: Uuid::new_v4(),
            e9_score: 0.20,  // Above 0.15 threshold for projected vectors
            e1_score: 0.30,  // Below 0.5 E1 weakness threshold
            divergence: -0.10,
        };

        // Should be a discovery: E9=0.20 >= 0.15, E1=0.30 < 0.5
        assert!(candidate.is_discovery(E9_DISCOVERY_THRESHOLD, E1_WEAKNESS_THRESHOLD));
    }

    #[test]
    fn test_blind_spot_candidate_not_discovery_e1_too_high() {
        let candidate = BlindSpotCandidate {
            memory_id: Uuid::new_v4(),
            e9_score: 0.20,  // Above E9 threshold
            e1_score: 0.75,  // E1 found it too (above 0.5)
            divergence: -0.55,
        };

        // Should NOT be a discovery: E1=0.75 >= 0.5 means E1 would have found it
        assert!(!candidate.is_discovery(E9_DISCOVERY_THRESHOLD, E1_WEAKNESS_THRESHOLD));
    }

    #[test]
    fn test_blind_spot_candidate_not_discovery_e9_too_low() {
        let candidate = BlindSpotCandidate {
            memory_id: Uuid::new_v4(),
            e9_score: 0.10,  // Below 0.15 threshold for projected vectors
            e1_score: 0.30,
            divergence: -0.20,
        };

        // Should NOT be a discovery: E9=0.10 < 0.15 means not a strong E9 match
        assert!(!candidate.is_discovery(E9_DISCOVERY_THRESHOLD, E1_WEAKNESS_THRESHOLD));
    }

    #[test]
    fn test_default_thresholds() {
        // E9 threshold calibrated for projected vectors (cosine similarity much lower than native Hamming)
        assert!((E9_DISCOVERY_THRESHOLD - 0.15).abs() < 0.001);
        assert!((E1_WEAKNESS_THRESHOLD - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_result_source_serialization() {
        let e1_json = serde_json::to_string(&ResultSource::E1).unwrap();
        assert_eq!(e1_json, "\"E1\"");

        let e9_json = serde_json::to_string(&ResultSource::E9Discovery).unwrap();
        assert_eq!(e9_json, "\"E9_DISCOVERY\"");
    }
}
