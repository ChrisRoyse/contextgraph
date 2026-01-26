//! DTOs for E6 keyword search MCP tools.
//!
//! Per PRD v6 and CLAUDE.md, E6 (V_selectivity) provides:
//! - Exact keyword matches via sparse vector representation
//! - Term-level precision that E1 may dilute through semantic averaging
//!
//! # Constitution Compliance
//!
//! - ARCH-12: E1 is the semantic foundation, E6 enhances with keyword precision
//! - E6 finds: "Exact keyword matches" that E1 misses by "Diluting via averaging"
//! - Use E6 for: "Keyword queries (exact terms, jargon)"
//! - FAIL FAST: All errors propagate immediately with logging

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_core::traits::SearchStrategy;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default topK for keyword search results.
pub const DEFAULT_KEYWORD_SEARCH_TOP_K: usize = 10;

/// Maximum topK for keyword search results.
pub const MAX_KEYWORD_SEARCH_TOP_K: usize = 50;

/// Default minimum score threshold for keyword search results.
pub const DEFAULT_MIN_KEYWORD_SCORE: f32 = 0.1;

/// Default blend weight for E6 vs E1 semantic.
/// 0.3 means 70% E1 semantic + 30% E6 keyword.
/// E6 needs higher weight than E10 because it's finding fundamentally different matches.
pub const DEFAULT_KEYWORD_BLEND: f32 = 0.3;

/// Whether to use E13 SPLADE expansion by default.
pub const DEFAULT_USE_SPLADE_EXPANSION: bool = true;

// ============================================================================
// REQUEST DTOs
// ============================================================================

/// Request parameters for search_by_keywords tool.
///
/// # Example JSON
/// ```json
/// {
///   "query": "RocksDB compaction tuning parameters",
///   "topK": 10,
///   "minScore": 0.1,
///   "blendWithSemantic": 0.3,
///   "useSpladeExpansion": true,
///   "includeContent": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct SearchByKeywordsRequest {
    /// The keyword query to search for (required).
    /// Can be a phrase or multiple keywords.
    pub query: String,

    /// Maximum number of results to return (1-50, default: 10).
    #[serde(rename = "topK", default = "default_top_k")]
    pub top_k: usize,

    /// Minimum score threshold (0-1, default: 0.1).
    #[serde(rename = "minScore", default = "default_min_score")]
    pub min_score: f32,

    /// Blend weight for E6 keyword vs E1 semantic (0-1, default: 0.3).
    /// 0.0 = pure E1 semantic, 1.0 = pure E6 keyword.
    /// E6 needs higher weight than E10 for keyword-specific queries.
    #[serde(rename = "blendWithSemantic", default = "default_blend")]
    pub blend_with_semantic: f32,

    /// Whether to use E13 SPLADE for term expansion (default: true).
    /// When true, expands query terms to related terms (fast→quick).
    #[serde(rename = "useSpladeExpansion", default = "default_use_splade")]
    pub use_splade_expansion: bool,

    /// Whether to include full content text in results (default: false).
    #[serde(rename = "includeContent", default)]
    pub include_content: bool,

    /// Search strategy for retrieval.
    /// - "multi_space": Default multi-embedder fusion (backward compatible)
    /// - "pipeline": E13 SPLADE recall → E1 → E12 ColBERT rerank (maximum precision)
    ///
    /// When "pipeline" is selected, E12 reranking is automatically enabled.
    #[serde(default)]
    pub strategy: Option<String>,
}

fn default_top_k() -> usize {
    DEFAULT_KEYWORD_SEARCH_TOP_K
}

fn default_min_score() -> f32 {
    DEFAULT_MIN_KEYWORD_SCORE
}

fn default_blend() -> f32 {
    DEFAULT_KEYWORD_BLEND
}

fn default_use_splade() -> bool {
    DEFAULT_USE_SPLADE_EXPANSION
}

impl Default for SearchByKeywordsRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            top_k: DEFAULT_KEYWORD_SEARCH_TOP_K,
            min_score: DEFAULT_MIN_KEYWORD_SCORE,
            blend_with_semantic: DEFAULT_KEYWORD_BLEND,
            use_splade_expansion: DEFAULT_USE_SPLADE_EXPANSION,
            include_content: false,
            strategy: None,
        }
    }
}

impl SearchByKeywordsRequest {
    /// Parse the strategy parameter into SearchStrategy enum.
    ///
    /// Strategy selection priority:
    /// 1. User-specified strategy takes precedence
    /// 2. Auto-upgrade to Pipeline if query is a precision query (quoted terms, keyword patterns)
    /// 3. Default to MultiSpace
    ///
    /// Note: Keyword search already benefits from E6/E13, but Pipeline adds E12 reranking
    /// for even better precision on exact matches.
    pub fn parse_strategy(&self) -> SearchStrategy {
        // User-specified strategy takes precedence
        match self.strategy.as_deref() {
            Some("pipeline") => SearchStrategy::Pipeline,
            Some("e1_only") => SearchStrategy::E1Only,
            _ => SearchStrategy::MultiSpace, // Default to multi-space for E6 enhancement
        }
    }

    /// Validate the request parameters.
    ///
    /// # Errors
    /// Returns an error message if:
    /// - query is empty
    /// - topK is outside [1, 50]
    /// - minScore is outside [0, 1] or NaN/infinite
    /// - blendWithSemantic is outside [0, 1] or NaN/infinite
    pub fn validate(&self) -> Result<(), String> {
        if self.query.is_empty() {
            return Err("query is required and cannot be empty".to_string());
        }

        if self.top_k < 1 || self.top_k > MAX_KEYWORD_SEARCH_TOP_K {
            return Err(format!(
                "topK must be between 1 and {}, got {}",
                MAX_KEYWORD_SEARCH_TOP_K, self.top_k
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

        if self.blend_with_semantic.is_nan() || self.blend_with_semantic.is_infinite() {
            return Err("blendWithSemantic must be a finite number".to_string());
        }

        if self.blend_with_semantic < 0.0 || self.blend_with_semantic > 1.0 {
            return Err(format!(
                "blendWithSemantic must be between 0.0 and 1.0, got {}",
                self.blend_with_semantic
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

/// A single search result for keyword search.
#[derive(Debug, Clone, Serialize)]
pub struct KeywordSearchResult {
    /// UUID of the matched memory.
    #[serde(rename = "memoryId")]
    pub memory_id: Uuid,

    /// Blended score (E1 semantic + E6 keyword).
    pub score: f32,

    /// Raw E1 semantic similarity (before blending).
    #[serde(rename = "e1Similarity")]
    pub e1_similarity: f32,

    /// Raw E6 keyword match score (before blending).
    #[serde(rename = "e6KeywordScore")]
    pub e6_keyword_score: f32,

    /// Number of keywords that matched in this result.
    #[serde(rename = "matchingKeywords")]
    pub matching_keywords: u32,

    /// Full content text (if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Source provenance information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<KeywordSourceInfo>,
}

/// Source provenance information.
#[derive(Debug, Clone, Serialize)]
pub struct KeywordSourceInfo {
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

/// Response metadata for keyword search.
#[derive(Debug, Clone, Serialize)]
pub struct KeywordSearchMetadata {
    /// Number of candidates evaluated before filtering.
    #[serde(rename = "candidatesEvaluated")]
    pub candidates_evaluated: usize,

    /// Number of results filtered by score threshold.
    #[serde(rename = "filteredByScore")]
    pub filtered_by_score: usize,

    /// E6 blend weight used.
    #[serde(rename = "e6BlendWeight")]
    pub e6_blend_weight: f32,

    /// E1 weight (1.0 - e6BlendWeight).
    #[serde(rename = "e1Weight")]
    pub e1_weight: f32,

    /// Whether SPLADE expansion was used.
    #[serde(rename = "usedSpladeExpansion")]
    pub used_splade_expansion: bool,

    /// Keywords extracted from query.
    #[serde(rename = "extractedKeywords")]
    pub extracted_keywords: Vec<String>,
}

/// Response for search_by_keywords tool.
#[derive(Debug, Clone, Serialize)]
pub struct SearchByKeywordsResponse {
    /// Original query.
    pub query: String,

    /// Matched results with blended scores.
    pub results: Vec<KeywordSearchResult>,

    /// Number of results returned.
    pub count: usize,

    /// Metadata about the search.
    pub metadata: KeywordSearchMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_by_keywords_validation_success() {
        let req = SearchByKeywordsRequest {
            query: "RocksDB compaction".to_string(),
            ..Default::default()
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_search_by_keywords_empty_query() {
        let req = SearchByKeywordsRequest::default();
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_search_by_keywords_invalid_blend() {
        let req = SearchByKeywordsRequest {
            query: "test".to_string(),
            blend_with_semantic: 1.5,
            ..Default::default()
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_search_by_keywords_invalid_top_k() {
        let req = SearchByKeywordsRequest {
            query: "test".to_string(),
            top_k: 100,
            ..Default::default()
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_default_blend_weight() {
        // E6 needs higher blend weight than E10 for keyword-specific queries
        assert!((DEFAULT_KEYWORD_BLEND - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_default_splade_expansion() {
        assert!(DEFAULT_USE_SPLADE_EXPANSION);
    }
}
