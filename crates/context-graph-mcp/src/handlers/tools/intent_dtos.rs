//! DTOs for intent-aware MCP tools.
//!
//! Per PRD v6 E10 Intent/Context Enhancement, these DTOs support:
//! - search_by_intent: Find memories with similar intent using asymmetric E10
//! - find_contextual_matches: Find memories relevant to a context using E10
//!
//! Constitution References:
//! - ARCH-12: E1 is the semantic foundation, E10 enhances
//! - ARCH-15: Uses E5-base-v2's query/passage prefix-based asymmetry
//! - E10 ENHANCES E1 semantic search via blendWithSemantic parameter
//! - Direction modifiers set to 1.0 (neutral) - E5's prefix training provides natural asymmetry

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_core::traits::SearchStrategy;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default topK for intent search results.
pub const DEFAULT_INTENT_SEARCH_TOP_K: usize = 10;

/// Maximum topK for intent search results.
pub const MAX_INTENT_SEARCH_TOP_K: usize = 50;

/// Default minimum score threshold for intent search results.
pub const DEFAULT_MIN_INTENT_SCORE: f32 = 0.2;

/// Default blend weight for E10 vs E1 semantic (LEGACY - kept for backward compatibility).
/// 0.1 means 90% E1 semantic + 10% E10 intent/context.
/// Reduced from 0.3 based on benchmark findings showing E10 optimal at ~0.1.
/// NOTE: New multiplicative boost approach supersedes linear blending.
pub const DEFAULT_BLEND_WITH_SEMANTIC: f32 = 0.1;

// ============================================================================
// INTENT BOOST CONFIGURATION (ARCH-17 Compliant)
// ============================================================================

/// Configuration for E10 intent-based multiplicative boost.
///
/// Per ARCH-12 and ARCH-17: E1 is THE semantic foundation. E10 ENHANCES E1
/// via multiplicative boost, not linear blending (which competes with E1).
///
/// # Philosophy
///
/// The multiplicative boost model treats E10 as a **modifier** that adjusts
/// E1 scores up or down based on intent alignment:
/// - E10 sim > 0.5: Intent aligned → boost E1 score up
/// - E10 sim < 0.5: Intent misaligned → reduce E1 score
/// - E10 sim ≈ 0.5: Neutral → no change to E1
///
/// # ARCH-17 Adaptive Boost
///
/// Boost strength adapts to E1 quality:
/// - E1 > 0.8 (strong match): Light boost (refine results)
/// - E1 ∈ [0.4, 0.8] (moderate): Medium boost (enhance results)
/// - E1 < 0.4 (weak match): Strong boost (broaden results)
///
/// This ensures E10 helps most when E1 alone is insufficient.
#[derive(Debug, Clone, Copy)]
pub struct IntentBoostConfig {
    /// Boost strength when E1 is strong (> 0.8). Default: 0.05.
    /// Light touch - E1 already found good matches, just refine.
    pub strong_e1_boost: f32,

    /// Boost strength when E1 is moderate (0.4-0.8). Default: 0.10.
    /// Medium enhancement - E1 found decent matches, help distinguish.
    pub medium_e1_boost: f32,

    /// Boost strength when E1 is weak (< 0.4). Default: 0.15.
    /// Strong enhancement - E1 struggling, E10 can help broaden search.
    pub weak_e1_boost: f32,

    /// Maximum boost range (±). Default: 0.20.
    /// Final multiplier clamped to [1.0 - boost_range, 1.0 + boost_range].
    pub boost_range: f32,

    /// Neutral point for E10 similarity. Default: 0.5.
    /// E10 sim above this means intent aligned, below means misaligned.
    pub neutral_point: f32,

    /// Whether boost is enabled. Default: true.
    /// If false, returns E1 score unchanged.
    pub enabled: bool,
}

impl Default for IntentBoostConfig {
    fn default() -> Self {
        Self {
            strong_e1_boost: 0.05,  // Light refinement for strong E1 matches
            medium_e1_boost: 0.10,  // Medium enhancement for moderate E1 matches
            weak_e1_boost: 0.15,    // Strong enhancement when E1 is struggling
            boost_range: 0.20,      // Max ±20% adjustment
            neutral_point: 0.5,     // E10 sim of 0.5 is neutral
            enabled: true,
        }
    }
}

impl IntentBoostConfig {
    /// Create a new config with custom boost strengths.
    pub fn new(strong: f32, medium: f32, weak: f32) -> Self {
        Self {
            strong_e1_boost: strong,
            medium_e1_boost: medium,
            weak_e1_boost: weak,
            ..Default::default()
        }
    }

    /// Create a disabled config (passthrough E1 scores).
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a conservative config (minimal E10 influence).
    pub fn conservative() -> Self {
        Self {
            strong_e1_boost: 0.02,
            medium_e1_boost: 0.05,
            weak_e1_boost: 0.08,
            boost_range: 0.10,
            ..Default::default()
        }
    }

    /// Create an aggressive config (stronger E10 influence).
    pub fn aggressive() -> Self {
        Self {
            strong_e1_boost: 0.10,
            medium_e1_boost: 0.15,
            weak_e1_boost: 0.25,
            boost_range: 0.30,
            ..Default::default()
        }
    }

    /// Compute intent-enhanced score using multiplicative boost.
    ///
    /// # Algorithm
    ///
    /// 1. Determine boost strength based on E1 quality (ARCH-17)
    /// 2. Compute intent alignment factor from E10 similarity
    /// 3. Apply bounded multiplicative boost to E1 score
    ///
    /// # Arguments
    ///
    /// * `e1_sim` - E1 semantic similarity score (0.0-1.0)
    /// * `e10_sim` - E10 intent similarity score (0.0-1.0)
    ///
    /// # Returns
    ///
    /// Intent-enhanced score = E1 * (1 + boost), clamped to [0.0, 1.0]
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = IntentBoostConfig::default();
    ///
    /// // Strong E1 match, aligned intent → slight boost
    /// let score = config.compute_enhanced_score(0.85, 0.7);
    /// assert!(score > 0.85); // Boosted up
    ///
    /// // Weak E1 match, misaligned intent → reduced
    /// let score = config.compute_enhanced_score(0.3, 0.2);
    /// assert!(score < 0.3); // Reduced
    ///
    /// // Neutral E10 → no change
    /// let score = config.compute_enhanced_score(0.6, 0.5);
    /// assert!((score - 0.6).abs() < 0.01);
    /// ```
    pub fn compute_enhanced_score(&self, e1_sim: f32, e10_sim: f32) -> f32 {
        // If disabled, passthrough E1 score
        if !self.enabled {
            return e1_sim;
        }

        // 1. Determine boost strength based on E1 quality (ARCH-17)
        let boost_strength = if e1_sim > 0.8 {
            self.strong_e1_boost  // Refine strong matches
        } else if e1_sim > 0.4 {
            self.medium_e1_boost  // Enhance moderate matches
        } else {
            self.weak_e1_boost    // Broaden weak matches
        };

        // 2. Compute intent alignment factor: [-1, +1]
        // E10 > neutral_point → positive (aligned)
        // E10 < neutral_point → negative (misaligned)
        let intent_alignment = (e10_sim - self.neutral_point) * 2.0;

        // 3. Apply bounded multiplicative boost
        let boost = (boost_strength * intent_alignment).clamp(-self.boost_range, self.boost_range);
        let intent_multiplier = 1.0 + boost;

        // 4. Final score = E1 * multiplier, clamped to valid range
        (e1_sim * intent_multiplier).clamp(0.0, 1.0)
    }
}

/// Intent→Context direction modifier (NEUTRAL).
/// Set to 1.0 (no modification) - E5-base-v2's prefix-based training provides natural asymmetry.
/// Previously 1.2 when using random projection; now unnecessary with real E5 embeddings.
#[allow(dead_code)]
pub const INTENT_TO_CONTEXT_MODIFIER: f32 = 1.0;

/// Context→Intent direction modifier (NEUTRAL).
/// Set to 1.0 (no modification) - E5-base-v2's prefix-based training provides natural asymmetry.
/// Previously 0.8 when using random projection; now unnecessary with real E5 embeddings.
#[allow(dead_code)]
pub const CONTEXT_TO_INTENT_MODIFIER: f32 = 1.0;

/// Configurable direction modifiers for E10 asymmetric similarity.
///
/// NOTE: With E5-base-v2, direction modifiers are no longer needed - the model's
/// prefix-based training ("query:" vs "passage:") provides natural asymmetry.
/// Both values default to 1.0 (neutral). This struct is kept for backwards
/// compatibility and potential future experimentation.
///
/// Default values: intent_to_context = 1.0, context_to_intent = 1.0
/// Expected ratio: 1.0 (= 1.0 / 1.0) - symmetric, letting E5 handle asymmetry
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DirectionModifiers {
    /// Modifier for intent→context direction (default 1.0 - neutral)
    pub intent_to_context: f32,
    /// Modifier for context→intent direction (default 1.0 - neutral)
    pub context_to_intent: f32,
}

impl Default for DirectionModifiers {
    fn default() -> Self {
        Self {
            intent_to_context: INTENT_TO_CONTEXT_MODIFIER,
            context_to_intent: CONTEXT_TO_INTENT_MODIFIER,
        }
    }
}

#[allow(dead_code)]
impl DirectionModifiers {
    /// Create new direction modifiers with custom values.
    ///
    /// # Arguments
    /// * `intent_to_context` - Modifier for intent→context (default 1.0)
    /// * `context_to_intent` - Modifier for context→intent (default 1.0)
    pub fn new(intent_to_context: f32, context_to_intent: f32) -> Self {
        Self {
            intent_to_context,
            context_to_intent,
        }
    }

    /// Calculate the expected asymmetry ratio (intent_to_context / context_to_intent).
    pub fn expected_ratio(&self) -> f32 {
        if self.context_to_intent.abs() < f32::EPSILON {
            return f32::INFINITY;
        }
        self.intent_to_context / self.context_to_intent
    }

    /// Apply intent→context modifier to a raw similarity score.
    pub fn apply_intent_to_context(&self, raw_similarity: f32) -> f32 {
        (raw_similarity * self.intent_to_context).clamp(0.0, 1.0)
    }

    /// Apply context→intent modifier to a raw similarity score.
    pub fn apply_context_to_intent(&self, raw_similarity: f32) -> f32 {
        (raw_similarity * self.context_to_intent).clamp(0.0, 1.0)
    }

    /// Validate the modifiers are within reasonable bounds.
    ///
    /// # Returns
    /// Error message if validation fails.
    pub fn validate(&self) -> Result<(), String> {
        if self.intent_to_context < 0.5 || self.intent_to_context > 2.0 {
            return Err(format!(
                "intent_to_context must be in [0.5, 2.0], got {}",
                self.intent_to_context
            ));
        }
        if self.context_to_intent < 0.3 || self.context_to_intent > 1.5 {
            return Err(format!(
                "context_to_intent must be in [0.3, 1.5], got {}",
                self.context_to_intent
            ));
        }
        Ok(())
    }
}

// ============================================================================
// REQUEST DTOs
// ============================================================================

/// Request parameters for search_by_intent tool.
///
/// # Example JSON
/// ```json
/// {
///   "query": "Improve the performance of the database queries",
///   "topK": 10,
///   "minScore": 0.2,
///   "blendWithSemantic": 0.3,
///   "includeContent": true
/// }
/// ```
///
/// # Alternative: Context-based search
/// ```json
/// {
///   "context": "Working on database optimization for production system",
///   "weightProfile": "balanced",
///   "topK": 10
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct SearchByIntentRequest {
    /// The intent or goal to search for.
    /// Describe what you're trying to accomplish.
    /// Either `query` or `context` must be provided (not both).
    #[serde(default)]
    pub query: Option<String>,

    /// The context or situation to find relevant memories for.
    /// Alternative to `query` - describe the current situation.
    /// Either `query` or `context` must be provided (not both).
    #[serde(default)]
    pub context: Option<String>,

    /// Maximum number of results to return (1-50, default: 10).
    #[serde(rename = "topK", default = "default_top_k")]
    pub top_k: usize,

    /// Minimum similarity score threshold (0-1, default: 0.2).
    #[serde(rename = "minScore", default = "default_min_score")]
    pub min_score: f32,

    /// Blend weight for E10 intent vs E1 semantic (0-1, default: 0.3).
    /// 0.0 = pure E1 semantic, 1.0 = pure E10 intent.
    #[serde(rename = "blendWithSemantic", default = "default_blend")]
    pub blend_with_semantic: f32,

    /// Whether to include full content text in results (default: false).
    #[serde(rename = "includeContent", default)]
    pub include_content: bool,

    /// Search strategy for retrieval.
    /// - "multi_space": Default multi-embedder fusion
    /// - "pipeline": E13 SPLADE recall → E1 → E12 ColBERT rerank (maximum precision)
    ///
    /// When "pipeline" is selected, E12 reranking is automatically enabled.
    #[serde(default)]
    pub strategy: Option<String>,

    /// Weight profile for retrieval.
    /// - "intent_search": E10=0.25, E1=0.40 (default for query-based)
    /// - "balanced": Balanced weights across embedders (default for context-based)
    /// - Other profiles: semantic_search, code_search, causal_reasoning, etc.
    #[serde(rename = "weightProfile", default)]
    pub weight_profile: Option<String>,

    /// Custom weights for 13 embedders (overrides weightProfile).
    /// Map of embedder name (E1-E13) to weight value (0.0-1.0).
    /// Weights must sum to ~1.0 (±0.01).
    #[serde(rename = "customWeights", default)]
    pub custom_weights: Option<std::collections::HashMap<String, f64>>,

    /// Embedders to exclude from search (e.g., ["E7", "E8"]).
    /// Excluded embedder weights are zeroed and remaining weights renormalized.
    #[serde(rename = "excludeEmbedders", default)]
    pub exclude_embedders: Vec<String>,
}

fn default_top_k() -> usize {
    DEFAULT_INTENT_SEARCH_TOP_K
}

fn default_min_score() -> f32 {
    DEFAULT_MIN_INTENT_SCORE
}

fn default_blend() -> f32 {
    DEFAULT_BLEND_WITH_SEMANTIC
}

impl Default for SearchByIntentRequest {
    fn default() -> Self {
        Self {
            query: None,
            context: None,
            top_k: DEFAULT_INTENT_SEARCH_TOP_K,
            min_score: DEFAULT_MIN_INTENT_SCORE,
            blend_with_semantic: DEFAULT_BLEND_WITH_SEMANTIC,
            include_content: false,
            strategy: None,
            weight_profile: None,
            custom_weights: None,
            exclude_embedders: Vec::new(),
        }
    }
}

impl SearchByIntentRequest {
    /// Get the effective query text (from `query` or `context`).
    ///
    /// Returns the query text, preferring `query` over `context` if both are provided.
    pub fn get_query_text(&self) -> Option<&str> {
        self.query.as_deref().or(self.context.as_deref())
    }

    /// Check if this is a context-based search (using `context` parameter).
    pub fn is_context_based(&self) -> bool {
        self.query.is_none() && self.context.is_some()
    }

    /// Get the effective weight profile.
    ///
    /// Returns the user-specified profile, or a default based on query type:
    /// - "intent_search" for query-based searches
    /// - "balanced" for context-based searches
    pub fn get_weight_profile(&self) -> &str {
        self.weight_profile.as_deref().unwrap_or_else(|| {
            if self.is_context_based() {
                "balanced"
            } else {
                "intent_search"
            }
        })
    }

    /// Parse the strategy parameter into SearchStrategy enum.
    ///
    /// Strategy selection priority:
    /// 1. User-specified strategy takes precedence
    /// 2. Auto-upgrade to Pipeline if query is a precision query (quoted terms, keyword patterns)
    /// 3. Default to MultiSpace
    pub fn parse_strategy(&self) -> SearchStrategy {
        // User-specified strategy takes precedence
        match self.strategy.as_deref() {
            Some("pipeline") => SearchStrategy::Pipeline,
            Some("e1_only") => SearchStrategy::E1Only,
            _ => SearchStrategy::MultiSpace, // Default to multi-space for E10 enhancement
        }
    }

    /// Validate the request parameters.
    ///
    /// # Errors
    /// Returns an error message if:
    /// - neither query nor context is provided
    /// - both query and context are provided (ambiguous)
    /// - the provided query/context is empty
    /// - topK is outside [1, 50]
    /// - minScore is outside [0, 1] or NaN/infinite
    /// - blendWithSemantic is outside [0, 1] or NaN/infinite
    pub fn validate(&self) -> Result<(), String> {
        // Validate query/context - exactly one must be provided
        match (&self.query, &self.context) {
            (None, None) => {
                return Err("Either 'query' or 'context' must be provided".to_string());
            }
            (Some(q), None) if q.is_empty() => {
                return Err("query cannot be empty".to_string());
            }
            (None, Some(c)) if c.is_empty() => {
                return Err("context cannot be empty".to_string());
            }
            (Some(q), Some(c)) if !q.is_empty() && !c.is_empty() => {
                return Err("Provide either 'query' or 'context', not both".to_string());
            }
            _ => {} // Valid: one non-empty parameter provided
        }

        if self.top_k < 1 || self.top_k > MAX_INTENT_SEARCH_TOP_K {
            return Err(format!(
                "topK must be between 1 and {}, got {}",
                MAX_INTENT_SEARCH_TOP_K, self.top_k
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

        // Validate weight profile if provided
        if let Some(ref profile) = self.weight_profile {
            let valid = [
                "intent_search", "intent_enhanced", "balanced", "semantic_search",
                "code_search", "causal_reasoning", "fact_checking", "temporal_navigation",
                "category_weighted", "sequence_navigation", "conversation_history",
            ];
            if !valid.contains(&profile.as_str()) {
                return Err(format!(
                    "weightProfile must be one of {:?}, got '{}'",
                    valid, profile
                ));
            }
        }

        Ok(())
    }
}

// ============================================================================
// RESPONSE DTOs
// ============================================================================

/// A single search result for intent/context search.
#[derive(Debug, Clone, Serialize)]
pub struct IntentSearchResult {
    /// UUID of the matched memory.
    #[serde(rename = "memoryId")]
    pub memory_id: Uuid,

    /// Blended score (E1 semantic + E10 intent/context).
    pub score: f32,

    /// Raw E1 semantic similarity (before blending).
    #[serde(rename = "e1Similarity")]
    pub e1_similarity: f32,

    /// Raw E10 intent/context similarity (before blending).
    #[serde(rename = "e10Similarity")]
    pub e10_similarity: f32,

    /// Full content text (if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Source provenance information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceInfo>,
}

/// Source provenance information.
#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
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

/// Response metadata for intent/context search.
#[derive(Debug, Clone, Serialize)]
pub struct IntentSearchMetadata {
    /// Number of candidates evaluated before filtering.
    #[serde(rename = "candidatesEvaluated")]
    pub candidates_evaluated: usize,

    /// Number of results filtered by score threshold.
    #[serde(rename = "filteredByScore")]
    pub filtered_by_score: usize,

    /// E10 blend weight used.
    #[serde(rename = "blendWeight")]
    pub blend_weight: f32,

    /// E1 weight (1.0 - blendWeight).
    #[serde(rename = "e1Weight")]
    pub e1_weight: f32,

    /// Direction modifier applied.
    #[serde(rename = "directionModifier")]
    pub direction_modifier: f32,
}

/// Response for search_by_intent tool.
#[derive(Debug, Clone, Serialize)]
pub struct SearchByIntentResponse {
    /// Original query (or context if context-based search).
    pub query: String,

    /// Matched results with blended scores.
    pub results: Vec<IntentSearchResult>,

    /// Number of results returned.
    pub count: usize,

    /// Metadata about the search.
    pub metadata: IntentSearchMetadata,

    /// Whether this was a context-based search.
    #[serde(rename = "isContextBased", skip_serializing_if = "std::ops::Not::not")]
    pub is_context_based: bool,

    /// Weight profile used for search.
    #[serde(rename = "weightProfile")]
    pub weight_profile: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_by_intent_validation_success_with_query() {
        let req = SearchByIntentRequest {
            query: Some("Find performance optimizations".to_string()),
            ..Default::default()
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_search_by_intent_validation_success_with_context() {
        let req = SearchByIntentRequest {
            context: Some("Working on database optimization".to_string()),
            ..Default::default()
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_search_by_intent_empty_query_and_context() {
        let req = SearchByIntentRequest::default();
        let err = req.validate().unwrap_err();
        assert!(err.contains("query") || err.contains("context"));
    }

    #[test]
    fn test_search_by_intent_invalid_blend() {
        let req = SearchByIntentRequest {
            query: Some("test".to_string()),
            blend_with_semantic: 1.5,
            ..Default::default()
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_search_by_intent_both_query_and_context_error() {
        let req = SearchByIntentRequest {
            query: Some("find optimizations".to_string()),
            context: Some("working on database".to_string()),
            ..Default::default()
        };
        let err = req.validate().unwrap_err();
        assert!(err.contains("not both"));
    }

    #[test]
    fn test_search_by_intent_empty_query_string() {
        let req = SearchByIntentRequest {
            query: Some("".to_string()),
            ..Default::default()
        };
        let err = req.validate().unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn test_search_by_intent_get_query_text() {
        let req_query = SearchByIntentRequest {
            query: Some("my query".to_string()),
            ..Default::default()
        };
        assert_eq!(req_query.get_query_text(), Some("my query"));

        let req_context = SearchByIntentRequest {
            context: Some("my context".to_string()),
            ..Default::default()
        };
        assert_eq!(req_context.get_query_text(), Some("my context"));
    }

    #[test]
    fn test_search_by_intent_is_context_based() {
        let req_query = SearchByIntentRequest {
            query: Some("my query".to_string()),
            ..Default::default()
        };
        assert!(!req_query.is_context_based());

        let req_context = SearchByIntentRequest {
            context: Some("my context".to_string()),
            ..Default::default()
        };
        assert!(req_context.is_context_based());
    }

    #[test]
    fn test_search_by_intent_weight_profile_defaults() {
        // Query-based defaults to intent_search
        let req_query = SearchByIntentRequest {
            query: Some("my query".to_string()),
            ..Default::default()
        };
        assert_eq!(req_query.get_weight_profile(), "intent_search");

        // Context-based defaults to balanced
        let req_context = SearchByIntentRequest {
            context: Some("my context".to_string()),
            ..Default::default()
        };
        assert_eq!(req_context.get_weight_profile(), "balanced");

        // Explicit weight profile overrides default
        let req_custom = SearchByIntentRequest {
            query: Some("my query".to_string()),
            weight_profile: Some("code_search".to_string()),
            ..Default::default()
        };
        assert_eq!(req_custom.get_weight_profile(), "code_search");
    }

    #[test]
    fn test_direction_modifiers_neutral() {
        // Direction modifiers are now 1.0 (neutral) since E5-base-v2
        // handles asymmetry via prefix-based training
        assert!((INTENT_TO_CONTEXT_MODIFIER - 1.0).abs() < 0.001);
        assert!((CONTEXT_TO_INTENT_MODIFIER - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_default_blend_ensures_e1_dominant() {
        // Default 0.1 means 90% E1, 10% E10
        let e1_weight = 1.0 - DEFAULT_BLEND_WITH_SEMANTIC;
        assert!(e1_weight > DEFAULT_BLEND_WITH_SEMANTIC);
        assert!((DEFAULT_BLEND_WITH_SEMANTIC - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_direction_modifiers_struct_default() {
        let modifiers = DirectionModifiers::default();
        // Both default to 1.0 (neutral) for E5-base-v2
        assert!((modifiers.intent_to_context - 1.0).abs() < 0.001);
        assert!((modifiers.context_to_intent - 1.0).abs() < 0.001);
        assert!((modifiers.expected_ratio() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_direction_modifiers_custom() {
        // Custom modifiers can still be used for experimentation
        let modifiers = DirectionModifiers::new(1.4, 0.6);
        assert!((modifiers.expected_ratio() - 2.333).abs() < 0.01);
    }

    #[test]
    fn test_direction_modifiers_apply() {
        let modifiers = DirectionModifiers::default();

        // With default 1.0 modifiers, output equals input (neutral)
        let i2c = modifiers.apply_intent_to_context(0.5);
        assert!((i2c - 0.5).abs() < 0.001);

        let c2i = modifiers.apply_context_to_intent(0.5);
        assert!((c2i - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_direction_modifiers_clamping() {
        // Test with a custom modifier that would exceed 1.0
        let modifiers = DirectionModifiers::new(1.5, 0.5);

        // Should clamp to 1.0 max
        let high = modifiers.apply_intent_to_context(0.9);
        assert!(high <= 1.0);

        // Should clamp to 0.0 min
        let low = modifiers.apply_context_to_intent(-0.1);
        assert!(low >= 0.0);
    }

    #[test]
    fn test_direction_modifiers_validation() {
        let valid = DirectionModifiers::default();
        assert!(valid.validate().is_ok());

        let invalid_i2c = DirectionModifiers::new(2.5, 0.8);
        assert!(invalid_i2c.validate().is_err());

        let invalid_c2i = DirectionModifiers::new(1.2, 0.2);
        assert!(invalid_c2i.validate().is_err());
    }

    // =========================================================================
    // INTENT BOOST CONFIG TESTS (ARCH-17)
    // =========================================================================

    #[test]
    fn test_intent_boost_config_default() {
        let config = IntentBoostConfig::default();
        assert!(config.enabled);
        assert!((config.strong_e1_boost - 0.05).abs() < 0.001);
        assert!((config.medium_e1_boost - 0.10).abs() < 0.001);
        assert!((config.weak_e1_boost - 0.15).abs() < 0.001);
        assert!((config.boost_range - 0.20).abs() < 0.001);
        assert!((config.neutral_point - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_intent_boost_neutral_e10_no_change() {
        // E10 sim of 0.5 (neutral) should not change E1 score
        let config = IntentBoostConfig::default();

        let e1_sim = 0.6;
        let e10_sim = 0.5;  // Neutral
        let enhanced = config.compute_enhanced_score(e1_sim, e10_sim);

        // Should be essentially unchanged
        assert!((enhanced - e1_sim).abs() < 0.01,
            "Neutral E10 should not change E1. Got {} expected ~{}", enhanced, e1_sim);
    }

    #[test]
    fn test_intent_boost_aligned_intent_boosts_e1() {
        // E10 sim > 0.5 (aligned) should boost E1 score
        let config = IntentBoostConfig::default();

        let e1_sim = 0.6;
        let e10_sim = 0.8;  // Aligned intent
        let enhanced = config.compute_enhanced_score(e1_sim, e10_sim);

        // Should be boosted
        assert!(enhanced > e1_sim,
            "Aligned E10 should boost E1. Got {} expected > {}", enhanced, e1_sim);
    }

    #[test]
    fn test_intent_boost_misaligned_intent_reduces_e1() {
        // E10 sim < 0.5 (misaligned) should reduce E1 score
        let config = IntentBoostConfig::default();

        let e1_sim = 0.6;
        let e10_sim = 0.2;  // Misaligned intent
        let enhanced = config.compute_enhanced_score(e1_sim, e10_sim);

        // Should be reduced
        assert!(enhanced < e1_sim,
            "Misaligned E10 should reduce E1. Got {} expected < {}", enhanced, e1_sim);
    }

    #[test]
    fn test_intent_boost_arch17_adaptive_strength() {
        // ARCH-17: Boost strength adapts to E1 quality
        // The *relative* boost (multiplier) should be higher for weaker E1
        let config = IntentBoostConfig::default();
        let e10_sim = 0.8;  // Aligned intent

        // Strong E1 (> 0.8) → light boost (0.05 boost_strength)
        let strong_e1 = 0.85;
        let strong_enhanced = config.compute_enhanced_score(strong_e1, e10_sim);
        let strong_multiplier = strong_enhanced / strong_e1;

        // Medium E1 (0.4-0.8) → medium boost (0.10 boost_strength)
        let medium_e1 = 0.6;
        let medium_enhanced = config.compute_enhanced_score(medium_e1, e10_sim);
        let medium_multiplier = medium_enhanced / medium_e1;

        // Weak E1 (< 0.4) → strong boost (0.15 boost_strength)
        let weak_e1 = 0.3;
        let weak_enhanced = config.compute_enhanced_score(weak_e1, e10_sim);
        let weak_multiplier = weak_enhanced / weak_e1;

        // Weak E1 should get higher RELATIVE boost (multiplier) than strong E1
        // Because boost_strength is higher: 0.15 > 0.10 > 0.05
        assert!(weak_multiplier > medium_multiplier,
            "Weak E1 should get higher multiplier than medium. weak={:.4} medium={:.4}",
            weak_multiplier, medium_multiplier);
        assert!(medium_multiplier > strong_multiplier,
            "Medium E1 should get higher multiplier than strong. medium={:.4} strong={:.4}",
            medium_multiplier, strong_multiplier);

        // Verify boost strengths are correctly applied:
        // intent_alignment = (0.8 - 0.5) * 2 = 0.6
        // expected multipliers: 1 + (boost_strength * 0.6)
        let expected_strong_mult = 1.0 + 0.05 * 0.6;  // 1.03
        let expected_medium_mult = 1.0 + 0.10 * 0.6;  // 1.06
        let expected_weak_mult = 1.0 + 0.15 * 0.6;    // 1.09

        assert!((strong_multiplier - expected_strong_mult).abs() < 0.001,
            "Strong multiplier: got {:.4} expected {:.4}", strong_multiplier, expected_strong_mult);
        assert!((medium_multiplier - expected_medium_mult).abs() < 0.001,
            "Medium multiplier: got {:.4} expected {:.4}", medium_multiplier, expected_medium_mult);
        assert!((weak_multiplier - expected_weak_mult).abs() < 0.001,
            "Weak multiplier: got {:.4} expected {:.4}", weak_multiplier, expected_weak_mult);
    }

    #[test]
    fn test_intent_boost_disabled_passthrough() {
        // Disabled config should passthrough E1 unchanged
        let config = IntentBoostConfig::disabled();

        let e1_sim = 0.6;
        let e10_sim = 0.9;  // Strong alignment (would normally boost)
        let enhanced = config.compute_enhanced_score(e1_sim, e10_sim);

        assert!((enhanced - e1_sim).abs() < 0.001,
            "Disabled config should passthrough E1. Got {} expected {}", enhanced, e1_sim);
    }

    #[test]
    fn test_intent_boost_clamped_to_valid_range() {
        // Results should be clamped to [0.0, 1.0]
        let config = IntentBoostConfig::aggressive();

        // Very high E1 + strong alignment shouldn't exceed 1.0
        let high_enhanced = config.compute_enhanced_score(0.95, 0.95);
        assert!(high_enhanced <= 1.0, "Should be clamped to 1.0 max");

        // Very low E1 + strong misalignment shouldn't go below 0.0
        let low_enhanced = config.compute_enhanced_score(0.05, 0.05);
        assert!(low_enhanced >= 0.0, "Should be clamped to 0.0 min");
    }

    #[test]
    fn test_intent_boost_e1_foundation_principle() {
        // Verify E1 is THE foundation - E10 only modifies, doesn't replace
        let config = IntentBoostConfig::default();

        // Even with perfect E10 alignment, E1=0 should stay 0
        let zero_e1 = config.compute_enhanced_score(0.0, 1.0);
        assert!((zero_e1 - 0.0).abs() < 0.001,
            "E1=0 should stay 0 even with perfect E10. Got {}", zero_e1);

        // E10 modifies E1, so result should be proportional to E1
        let low_e1 = config.compute_enhanced_score(0.3, 0.8);
        let high_e1 = config.compute_enhanced_score(0.9, 0.8);

        assert!(high_e1 > low_e1,
            "Higher E1 should result in higher enhanced score. high={} low={}", high_e1, low_e1);
    }
}
