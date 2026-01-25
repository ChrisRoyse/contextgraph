//! DTOs for E7 code search MCP tools.
//!
//! Per PRD v6 and CLAUDE.md, E7 (V_correctness) provides:
//! - Code patterns and function signatures via 1536D dense embeddings
//! - Code-specific understanding that E1 misses by treating code as natural language
//!
//! # Constitution Compliance
//!
//! - ARCH-12: E1 is the semantic foundation, E7 enhances with code understanding
//! - ARCH-13: Supports multiple strategies: E1Only, MultiSpace, Pipeline
//! - E7 finds: "Code patterns, function signatures" that E1 misses by "Treating code as NL"
//! - Use E7 for: "Code queries (implementations, functions)"
//! - FAIL FAST: All errors propagate immediately with logging

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::fmt;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default topK for code search results.
pub const DEFAULT_CODE_SEARCH_TOP_K: usize = 10;

/// Maximum topK for code search results.
pub const MAX_CODE_SEARCH_TOP_K: usize = 50;

/// Default minimum score threshold for code search results.
pub const DEFAULT_MIN_CODE_SCORE: f32 = 0.2;

/// Default blend weight for E7 vs E1 semantic.
/// 0.4 means 60% E1 semantic + 40% E7 code.
/// E7 needs significant weight for code-specific queries.
pub const DEFAULT_CODE_BLEND: f32 = 0.4;

// ============================================================================
// CODE SEARCH MODE
// ============================================================================

/// Code search mode for controlling E1/E7 scoring strategy.
///
/// Per ARCH-12: E1 is the semantic foundation, E7 enhances with code understanding.
/// All modes produce scores in [0, 1] range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodeSearchMode {
    /// Blend E1 semantic and E7 code scores.
    /// Score = (1-blend)*E1 + blend*E7
    /// Best for: balanced code search with semantic understanding.
    #[default]
    Hybrid,

    /// Pure E7 code search (ignores E1 semantic).
    /// Best for: function signatures, impl blocks, struct/enum definitions.
    E7Only,

    /// E1 primary (90%) with E7 tiebreaker (10%).
    /// Best for: natural language queries about code functionality.
    E1WithE7Rerank,

    /// Currently equivalent to Hybrid.
    /// Reserved for future full pipeline: E13 sparse -> E1 dense -> E7 code -> E12 rerank.
    Pipeline,
}

impl fmt::Display for CodeSearchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodeSearchMode::Hybrid => write!(f, "hybrid"),
            CodeSearchMode::E7Only => write!(f, "e7_only"),
            CodeSearchMode::E1WithE7Rerank => write!(f, "e1_with_e7_rerank"),
            CodeSearchMode::Pipeline => write!(f, "pipeline"),
        }
    }
}

// ============================================================================
// DETECTED LANGUAGE
// ============================================================================

/// Detected programming language in query.
#[derive(Debug, Clone, Serialize)]
pub struct DetectedLanguageInfo {
    /// Primary language detected (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_language: Option<String>,

    /// Confidence score for language detection (0-1).
    pub confidence: f32,

    /// Indicators that led to detection.
    pub indicators: Vec<String>,
}

// ============================================================================
// REQUEST DTOs
// ============================================================================

/// Request parameters for search_code tool.
///
/// # Example JSON
/// ```json
/// {
///   "query": "async function that handles HTTP requests",
///   "topK": 10,
///   "minScore": 0.2,
///   "blendWithSemantic": 0.4,
///   "searchMode": "hybrid",
///   "languageHint": "rust",
///   "includeContent": true,
///   "includeAstContext": false
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct SearchCodeRequest {
    /// The code query to search for (required).
    /// Can describe functionality, patterns, or specific code constructs.
    pub query: String,

    /// Maximum number of results to return (1-50, default: 10).
    #[serde(rename = "topK", default = "default_top_k")]
    pub top_k: usize,

    /// Minimum score threshold (0-1, default: 0.2).
    #[serde(rename = "minScore", default = "default_min_score")]
    pub min_score: f32,

    /// Blend weight for E7 code vs E1 semantic (0-1, default: 0.4).
    /// 0.0 = pure E1 semantic, 1.0 = pure E7 code.
    /// Only used in Hybrid mode.
    #[serde(rename = "blendWithSemantic", default = "default_blend")]
    pub blend_with_semantic: f32,

    /// Search mode controlling E1/E7 strategy (default: "hybrid").
    /// - "hybrid": Blend E1 and E7 scores
    /// - "e7Only": Pure E7 code search
    /// - "e1WithE7Rerank": E1 retrieval with E7 reranking
    /// - "pipeline": Full E13→E1→E7→E12 pipeline
    #[serde(rename = "searchMode", default)]
    pub search_mode: CodeSearchMode,

    /// Optional language hint to boost language-specific results.
    /// Supports: rust, python, javascript, typescript, go, java, cpp, sql
    #[serde(rename = "languageHint", default)]
    pub language_hint: Option<String>,

    /// Whether to include full content text in results (default: false).
    #[serde(rename = "includeContent", default)]
    pub include_content: bool,

    /// Whether to include AST context (scope chain, entity type) if available.
    /// Only affects chunks created by AST chunker.
    #[serde(rename = "includeAstContext", default)]
    pub include_ast_context: bool,
}

fn default_top_k() -> usize {
    DEFAULT_CODE_SEARCH_TOP_K
}

fn default_min_score() -> f32 {
    DEFAULT_MIN_CODE_SCORE
}

fn default_blend() -> f32 {
    DEFAULT_CODE_BLEND
}

impl Default for SearchCodeRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            top_k: DEFAULT_CODE_SEARCH_TOP_K,
            min_score: DEFAULT_MIN_CODE_SCORE,
            blend_with_semantic: DEFAULT_CODE_BLEND,
            search_mode: CodeSearchMode::Hybrid,
            language_hint: None,
            include_content: false,
            include_ast_context: false,
        }
    }
}

impl SearchCodeRequest {
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

        if self.top_k < 1 || self.top_k > MAX_CODE_SEARCH_TOP_K {
            return Err(format!(
                "topK must be between 1 and {}, got {}",
                MAX_CODE_SEARCH_TOP_K, self.top_k
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

        Ok(())
    }
}

// ============================================================================
// RESPONSE DTOs
// ============================================================================

/// A single search result for code search.
#[derive(Debug, Clone, Serialize)]
pub struct CodeSearchResult {
    /// UUID of the matched memory.
    #[serde(rename = "memoryId")]
    pub memory_id: Uuid,

    /// Blended score (E1 semantic + E7 code).
    pub score: f32,

    /// Raw E1 semantic similarity (before blending).
    #[serde(rename = "e1Similarity")]
    pub e1_similarity: f32,

    /// Raw E7 code similarity (before blending).
    #[serde(rename = "e7CodeScore")]
    pub e7_code_score: f32,

    /// Full content text (if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Source provenance information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<CodeSourceInfo>,
}

/// Source provenance information.
#[derive(Debug, Clone, Serialize)]
pub struct CodeSourceInfo {
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

/// Response metadata for code search.
#[derive(Debug, Clone, Serialize)]
pub struct CodeSearchMetadata {
    /// Number of candidates evaluated before filtering.
    #[serde(rename = "candidatesEvaluated")]
    pub candidates_evaluated: usize,

    /// Number of results filtered by score threshold.
    #[serde(rename = "filteredByScore")]
    pub filtered_by_score: usize,

    /// Search mode used.
    #[serde(rename = "searchMode")]
    pub search_mode: CodeSearchMode,

    /// E7 blend weight used (only for Hybrid mode).
    #[serde(rename = "e7BlendWeight")]
    pub e7_blend_weight: f32,

    /// E1 weight (1.0 - e7BlendWeight, only for Hybrid mode).
    #[serde(rename = "e1Weight")]
    pub e1_weight: f32,

    /// Language hint provided (if any).
    #[serde(skip_serializing_if = "Option::is_none", rename = "languageHint")]
    pub language_hint: Option<String>,

    /// Detected language info from query.
    #[serde(rename = "detectedLanguage")]
    pub detected_language: DetectedLanguageInfo,
}

/// Response for search_code tool.
#[derive(Debug, Clone, Serialize)]
pub struct SearchCodeResponse {
    /// Original query.
    pub query: String,

    /// Matched results with blended scores.
    pub results: Vec<CodeSearchResult>,

    /// Number of results returned.
    pub count: usize,

    /// Metadata about the search.
    pub metadata: CodeSearchMetadata,

    /// Code entity results from CodeStore (if code pipeline is enabled).
    /// E7-WIRING: Added for direct code search results.
    #[serde(skip_serializing_if = "Option::is_none", rename = "codeEntities")]
    pub code_entities: Option<Vec<CodeEntityResult>>,
}

// ============================================================================
// CODE ENTITY RESULTS (E7-WIRING)
// ============================================================================

/// A code entity search result from CodeStore.
///
/// E7-WIRING: Added for direct code search via E7 embeddings.
#[derive(Debug, Clone, Serialize)]
pub struct CodeEntityResult {
    /// Entity UUID.
    pub id: String,

    /// Entity name (function, struct, etc.).
    pub name: String,

    /// Entity type (Function, Struct, Trait, etc.).
    #[serde(rename = "entityType")]
    pub entity_type: String,

    /// E7 similarity score.
    pub score: f32,

    /// File path where entity is defined.
    #[serde(rename = "filePath")]
    pub file_path: String,

    /// Line number where entity starts.
    #[serde(skip_serializing_if = "Option::is_none", rename = "startLine")]
    pub start_line: Option<usize>,

    /// Line number where entity ends.
    #[serde(skip_serializing_if = "Option::is_none", rename = "endLine")]
    pub end_line: Option<usize>,

    /// Code content (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Parent scope chain (e.g., ["mod foo", "impl Bar"]).
    #[serde(skip_serializing_if = "Option::is_none", rename = "scopeChain")]
    pub scope_chain: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_code_validation_success() {
        let req = SearchCodeRequest {
            query: "async function HTTP handler".to_string(),
            ..Default::default()
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_search_code_empty_query() {
        let req = SearchCodeRequest::default();
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_search_code_invalid_blend() {
        let req = SearchCodeRequest {
            query: "test".to_string(),
            blend_with_semantic: 1.5,
            ..Default::default()
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_search_code_invalid_top_k() {
        let req = SearchCodeRequest {
            query: "test".to_string(),
            top_k: 100,
            ..Default::default()
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_default_blend_weight() {
        // E7 needs significant weight (0.4) for code queries
        assert!((DEFAULT_CODE_BLEND - 0.4).abs() < 0.001);
    }
}
