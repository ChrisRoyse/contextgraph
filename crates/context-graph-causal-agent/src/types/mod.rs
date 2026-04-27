//! Type definitions for the Causal Discovery Agent.
//!
//! Contains structures for causal analysis results, candidate pairs,
//! and configuration options.
//!
//! # Causal Hint System
//!
//! The [`CausalHint`] and [`CausalDirectionHint`] types are re-exported from
//! `context_graph_core::traits` and enable LLM-enhanced E5 embeddings by
//! providing direction hints during memory storage.
//! See the Causal Discovery LLM + E5 Integration Plan for details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// CAUSAL HINTS FOR E5 EMBEDDING ENHANCEMENT
// ============================================================================

// Re-export CausalHint types from context-graph-core to ensure type compatibility
// with EmbeddingMetadata. This is the authoritative source for these types.
pub use context_graph_core::traits::{CausalDirectionHint, CausalHint};

// ============================================================================
// DIRECTIONAL EMBEDDINGS (Phase 2a)
// ============================================================================

/// Embeddings generated with LLM direction awareness.
///
/// Supports forward (A→B), backward (B→A), and bidirectional (A↔B) causal relationships.
/// For bidirectional relationships, both primary and secondary embeddings are populated.
#[derive(Debug, Clone)]
pub struct DirectionalEmbeddings {
    /// Primary cause embedding (from the detected cause).
    /// For ACausesB: embed_as_cause(A)
    /// For BCausesA: embed_as_cause(B)
    /// For Bidirectional: embed_as_cause(A)
    pub cause_primary: Vec<f32>,

    /// Primary effect embedding (from the detected effect).
    /// For ACausesB: embed_as_effect(B)
    /// For BCausesA: embed_as_effect(A)
    /// For Bidirectional: embed_as_effect(B)
    pub effect_primary: Vec<f32>,

    /// For bidirectional: reverse cause embedding (embed_as_cause(B)).
    pub cause_secondary: Option<Vec<f32>>,

    /// For bidirectional: reverse effect embedding (embed_as_effect(A)).
    pub effect_secondary: Option<Vec<f32>>,

    /// The detected causal direction.
    pub direction: CausalLinkDirection,
}

impl DirectionalEmbeddings {
    /// Create embeddings for a forward (A causes B) relationship.
    pub fn forward(cause_vec: Vec<f32>, effect_vec: Vec<f32>) -> Self {
        Self {
            cause_primary: cause_vec,
            effect_primary: effect_vec,
            cause_secondary: None,
            effect_secondary: None,
            direction: CausalLinkDirection::ACausesB,
        }
    }

    /// Create embeddings for a backward (B causes A) relationship.
    pub fn backward(cause_vec: Vec<f32>, effect_vec: Vec<f32>) -> Self {
        Self {
            cause_primary: cause_vec,
            effect_primary: effect_vec,
            cause_secondary: None,
            effect_secondary: None,
            direction: CausalLinkDirection::BCausesA,
        }
    }

    /// Create embeddings for a bidirectional (A ↔ B) relationship.
    ///
    /// # Arguments
    /// * `a_cause` - A embedded as cause
    /// * `a_effect` - A embedded as effect
    /// * `b_cause` - B embedded as cause
    /// * `b_effect` - B embedded as effect
    pub fn bidirectional(
        a_cause: Vec<f32>,
        a_effect: Vec<f32>,
        b_cause: Vec<f32>,
        b_effect: Vec<f32>,
    ) -> Self {
        Self {
            cause_primary: a_cause,
            effect_primary: b_effect,
            cause_secondary: Some(b_cause),
            effect_secondary: Some(a_effect),
            direction: CausalLinkDirection::Bidirectional,
        }
    }

    /// Check if this is a bidirectional relationship.
    pub fn is_bidirectional(&self) -> bool {
        matches!(self.direction, CausalLinkDirection::Bidirectional)
    }

    /// Get the embedding dimension (should be 768 for E5).
    pub fn dimension(&self) -> usize {
        self.cause_primary.len()
    }
}

/// Result of LLM causal relationship analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalAnalysisResult {
    /// Whether a causal link was detected.
    pub has_causal_link: bool,

    /// Direction of the causal relationship.
    pub direction: CausalLinkDirection,

    /// Confidence score [0.0, 1.0].
    pub confidence: f32,

    /// Description of the causal mechanism.
    pub mechanism: String,

    /// Type of causal mechanism (direct, mediated, feedback, temporal).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mechanism_type: Option<MechanismType>,

    /// Raw LLM response (for debugging).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_response: Option<String>,

    /// LLM provenance metadata (Phase 1.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_provenance: Option<context_graph_core::types::LLMProvenance>,
}

impl Default for CausalAnalysisResult {
    fn default() -> Self {
        Self {
            has_causal_link: false,
            direction: CausalLinkDirection::NoCausalLink,
            confidence: 0.0,
            mechanism: String::new(),
            mechanism_type: None,
            raw_response: None,
            llm_provenance: None,
        }
    }
}

/// Type of causal mechanism detected by the LLM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismType {
    /// A directly causes B without intermediaries.
    Direct,

    /// A causes X which causes B (indirect pathway).
    Mediated,

    /// A and B mutually reinforce each other (feedback loops).
    Feedback,

    /// A precedes B in a necessary sequence.
    Temporal,
}

impl MechanismType {
    /// Convert from string (for parsing LLM output).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "direct" => Some(Self::Direct),
            "mediated" => Some(Self::Mediated),
            "feedback" => Some(Self::Feedback),
            "temporal" => Some(Self::Temporal),
            _ => None,
        }
    }

    /// Convert to string for serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Mediated => "mediated",
            Self::Feedback => "feedback",
            Self::Temporal => "temporal",
        }
    }
}

// ============================================================================
// SOURCE SPAN TYPES (Provenance Tracking)
// ============================================================================

/// A span in the source text where a causal relationship was found.
///
/// Provides provenance tracking by recording exact character offsets
/// into the input text from which a causal relationship was extracted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Character offset from start of input text (0-based).
    pub start_char: usize,

    /// Character offset for end of span (exclusive).
    pub end_char: usize,

    /// The exact text excerpt (truncated to 200 chars).
    /// This MUST be copied exactly from the input, not paraphrased.
    pub text_excerpt: String,

    /// What this span represents: "cause", "effect", or "full".
    pub span_type: SpanType,
}

impl SourceSpan {
    /// Create a new source span with validation.
    pub fn new(
        start_char: usize,
        end_char: usize,
        text_excerpt: impl Into<String>,
        span_type: SpanType,
    ) -> Self {
        let excerpt = text_excerpt.into();
        // Truncate to 200 chars if needed
        let truncated = if excerpt.chars().count() > 200 {
            excerpt.chars().take(200).collect::<String>() + "..."
        } else {
            excerpt
        };
        Self {
            start_char,
            end_char,
            text_excerpt: truncated,
            span_type,
        }
    }

    /// Check if the offsets are within bounds of a given text length.
    pub fn is_valid_for(&self, text_len: usize) -> bool {
        self.start_char < self.end_char && self.end_char <= text_len
    }

    /// Validate that text_excerpt matches the source text at the given offsets.
    ///
    /// Returns false if offsets are out of bounds or cross unicode boundaries.
    pub fn matches_source(&self, source_text: &str) -> bool {
        // Use .get() to handle invalid byte boundaries gracefully
        let Some(actual) = source_text.get(self.start_char..self.end_char) else {
            return false;
        };
        // Allow for truncation - check if excerpt starts with actual or vice versa
        self.text_excerpt.trim() == actual.trim()
            || actual.starts_with(self.text_excerpt.trim_end_matches("..."))
    }
}

/// Type of span in source text.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpanType {
    /// Span containing the cause portion of the relationship.
    Cause,
    /// Span containing the effect portion of the relationship.
    Effect,
    /// Span containing the full relationship context.
    Full,
}

impl SpanType {
    /// Parse from string representation.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "cause" => Self::Cause,
            "effect" => Self::Effect,
            _ => Self::Full,
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cause => "cause",
            Self::Effect => "effect",
            Self::Full => "full",
        }
    }
}

// ============================================================================
// MULTI-RELATIONSHIP EXTRACTION TYPES
// ============================================================================

/// A single causal relationship extracted from content.
///
/// Unlike [`CausalHint`] which describes whether text IS causal,
/// this represents a specific cause-effect relationship found within the text.
/// Each relationship gets its own explanatory paragraph for E5 embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedCausalRelationship {
    /// Brief statement of the cause (e.g., "Chronic stress elevates cortisol")
    pub cause: String,

    /// Brief statement of the effect (e.g., "Elevated cortisol damages neurons")
    pub effect: String,

    /// 1-2 paragraph explanation of HOW and WHY this causal link exists.
    /// This is embedded for semantic search.
    pub explanation: String,

    /// Confidence score [0.0, 1.0] from the LLM.
    pub confidence: f32,

    /// Type of causal mechanism.
    pub mechanism_type: MechanismType,

    /// Source spans indicating where in the input text this relationship was found.
    /// Each relationship should have at least one span for provenance tracking.
    #[serde(default)]
    pub source_spans: Vec<SourceSpan>,
}

impl ExtractedCausalRelationship {
    /// Create a new extracted relationship with validation.
    pub fn new(
        cause: String,
        effect: String,
        explanation: String,
        confidence: f32,
        mechanism_type: MechanismType,
    ) -> Self {
        Self {
            cause,
            effect,
            explanation,
            confidence: confidence.clamp(0.0, 1.0),
            mechanism_type,
            source_spans: Vec::new(),
        }
    }

    /// Create a new extracted relationship with source spans.
    pub fn new_with_spans(
        cause: String,
        effect: String,
        explanation: String,
        confidence: f32,
        mechanism_type: MechanismType,
        source_spans: Vec<SourceSpan>,
    ) -> Self {
        Self {
            cause,
            effect,
            explanation,
            confidence: confidence.clamp(0.0, 1.0),
            mechanism_type,
            source_spans,
        }
    }

    /// Add source spans to this relationship.
    pub fn with_source_spans(mut self, spans: Vec<SourceSpan>) -> Self {
        self.source_spans = spans;
        self
    }

    /// Check if this relationship meets the minimum confidence threshold.
    pub fn is_confident(&self, threshold: f32) -> bool {
        self.confidence >= threshold
    }

    /// Check if this relationship has provenance tracking.
    pub fn has_provenance(&self) -> bool {
        !self.source_spans.is_empty()
    }

    /// Validate source spans against the original source text.
    pub fn validate_spans(&self, source_text: &str) -> bool {
        if self.source_spans.is_empty() {
            return false;
        }
        self.source_spans
            .iter()
            .all(|span| span.is_valid_for(source_text.len()) && span.matches_source(source_text))
    }
}

/// Result of multi-relationship extraction from a piece of content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiRelationshipResult {
    /// All causal relationships extracted from the content.
    pub relationships: Vec<ExtractedCausalRelationship>,

    /// Whether the content had any causal language at all.
    pub has_causal_content: bool,
}

impl MultiRelationshipResult {
    /// Create an empty result for non-causal content.
    pub fn not_causal() -> Self {
        Self {
            relationships: Vec::new(),
            has_causal_content: false,
        }
    }

    /// Check if any relationships were found.
    pub fn has_relationships(&self) -> bool {
        !self.relationships.is_empty()
    }

    /// Get only relationships above a confidence threshold.
    pub fn confident_relationships(&self, threshold: f32) -> Vec<&ExtractedCausalRelationship> {
        self.relationships
            .iter()
            .filter(|r| r.is_confident(threshold))
            .collect()
    }
}

/// Direction of a causal relationship between two memories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalLinkDirection {
    /// Memory A causes Memory B (A → B).
    ACausesB,

    /// Memory B causes Memory A (B → A).
    BCausesA,

    /// Bidirectional causation (A ↔ B).
    Bidirectional,

    /// No causal relationship detected.
    NoCausalLink,
}

impl CausalLinkDirection {
    /// Parse from LLM response string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.contains("a_causes_b") || lower.contains("a causes b") || lower == "forward" {
            Self::ACausesB
        } else if lower.contains("b_causes_a")
            || lower.contains("b causes a")
            || lower == "backward"
        {
            Self::BCausesA
        } else if lower.contains("bidirectional") || lower.contains("mutual") || lower == "both" {
            Self::Bidirectional
        } else {
            Self::NoCausalLink
        }
    }

    /// Whether this represents an actual causal link.
    pub fn is_causal(&self) -> bool {
        !matches!(self, Self::NoCausalLink)
    }
}

/// A candidate pair of memories for causal analysis.
#[derive(Debug, Clone)]
pub struct CausalCandidate {
    /// Potential cause memory ID.
    pub cause_memory_id: Uuid,

    /// Potential cause memory content.
    pub cause_content: String,

    /// Potential effect memory ID.
    pub effect_memory_id: Uuid,

    /// Potential effect memory content.
    pub effect_content: String,

    /// Initial score based on heuristics (causal markers, temporal order).
    pub initial_score: f32,

    /// Timestamp of the earlier memory.
    pub earlier_timestamp: DateTime<Utc>,

    /// Timestamp of the later memory.
    pub later_timestamp: DateTime<Utc>,
}

impl CausalCandidate {
    /// Create a new causal candidate.
    pub fn new(
        cause_id: Uuid,
        cause_content: String,
        effect_id: Uuid,
        effect_content: String,
        score: f32,
        earlier: DateTime<Utc>,
        later: DateTime<Utc>,
    ) -> Self {
        Self {
            cause_memory_id: cause_id,
            cause_content,
            effect_memory_id: effect_id,
            effect_content,
            initial_score: score,
            earlier_timestamp: earlier,
            later_timestamp: later,
        }
    }
}

/// Memory representation for causal analysis.
#[derive(Debug, Clone)]
pub struct MemoryForAnalysis {
    /// Memory UUID.
    pub id: Uuid,

    /// Text content.
    pub content: String,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Session ID (if available).
    pub session_id: Option<String>,

    /// E1 semantic embedding (for clustering).
    pub e1_embedding: Vec<f32>,
}

/// Causal markers used for initial scoring.
pub struct CausalMarkers;

impl CausalMarkers {
    /// Words/phrases that indicate causation.
    pub const CAUSE_MARKERS: &'static [&'static str] = &[
        "because",
        "due to",
        "caused by",
        "result of",
        "led to",
        "since",
        "as a result of",
        "owing to",
        "thanks to",
        "on account of",
        "stems from",
        "arises from",
        "originates from",
        "triggered by",
    ];

    /// Words/phrases that indicate effects.
    pub const EFFECT_MARKERS: &'static [&'static str] = &[
        "therefore",
        "consequently",
        "as a result",
        "thus",
        "hence",
        "so",
        "accordingly",
        "resulting in",
        "leading to",
        "causing",
        "produces",
        "yields",
        "generates",
        "brings about",
    ];

    /// Check if text contains any cause markers.
    pub fn has_cause_marker(text: &str) -> bool {
        let lower = text.to_lowercase();
        Self::CAUSE_MARKERS.iter().any(|m| lower.contains(m))
    }

    /// Check if text contains any effect markers.
    pub fn has_effect_marker(text: &str) -> bool {
        let lower = text.to_lowercase();
        Self::EFFECT_MARKERS.iter().any(|m| lower.contains(m))
    }

    /// Count causal markers in text.
    pub fn count_markers(text: &str) -> usize {
        let lower = text.to_lowercase();
        let cause_count = Self::CAUSE_MARKERS
            .iter()
            .filter(|m| lower.contains(*m))
            .count();
        let effect_count = Self::EFFECT_MARKERS
            .iter()
            .filter(|m| lower.contains(*m))
            .count();
        cause_count + effect_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_causal_direction_parsing() {
        assert_eq!(
            CausalLinkDirection::from_str("A_causes_B"),
            CausalLinkDirection::ACausesB
        );
        assert_eq!(
            CausalLinkDirection::from_str("B_causes_A"),
            CausalLinkDirection::BCausesA
        );
        assert_eq!(
            CausalLinkDirection::from_str("bidirectional"),
            CausalLinkDirection::Bidirectional
        );
        assert_eq!(
            CausalLinkDirection::from_str("none"),
            CausalLinkDirection::NoCausalLink
        );
    }

    #[test]
    fn test_causal_markers() {
        assert!(CausalMarkers::has_cause_marker(
            "This happened because of X"
        ));
        assert!(CausalMarkers::has_effect_marker("Therefore, Y occurred"));
        assert!(!CausalMarkers::has_cause_marker("Hello world"));
        assert_eq!(CausalMarkers::count_markers("Because of X, therefore Y"), 2);
    }

    // =========================================================================
    // CAUSAL HINT TESTS
    // =========================================================================

    #[test]
    fn test_causal_direction_hint_from_str() {
        assert_eq!(
            CausalDirectionHint::from_str("cause"),
            CausalDirectionHint::Cause
        );
        assert_eq!(
            CausalDirectionHint::from_str("effect"),
            CausalDirectionHint::Effect
        );
        assert_eq!(
            CausalDirectionHint::from_str("neutral"),
            CausalDirectionHint::Neutral
        );
        assert_eq!(
            CausalDirectionHint::from_str("unknown"),
            CausalDirectionHint::Neutral
        );
    }

    #[test]
    fn test_causal_direction_hint_bias_factors() {
        assert_eq!(CausalDirectionHint::Cause.bias_factors(), (1.3, 0.8));
        assert_eq!(CausalDirectionHint::Effect.bias_factors(), (0.8, 1.3));
        assert_eq!(CausalDirectionHint::Neutral.bias_factors(), (1.0, 1.0));
    }

    #[test]
    fn test_causal_hint_is_useful() {
        // High confidence causal hint is useful
        let useful_hint = CausalHint::new(
            true,
            CausalDirectionHint::Cause,
            0.8,
            vec!["causes".to_string()],
        );
        assert!(useful_hint.is_useful());

        // Low confidence causal hint is not useful
        let low_conf_hint = CausalHint::new(true, CausalDirectionHint::Cause, 0.3, vec![]);
        assert!(!low_conf_hint.is_useful());

        // Non-causal hint is not useful even with high confidence
        let not_causal = CausalHint::not_causal();
        assert!(!not_causal.is_useful());
    }

    #[test]
    fn test_causal_hint_bias_factors() {
        // Useful hint returns direction bias
        let cause_hint = CausalHint::new(true, CausalDirectionHint::Cause, 0.9, vec![]);
        assert_eq!(cause_hint.bias_factors(), (1.3, 0.8));

        // Non-useful hint returns neutral bias
        let low_conf = CausalHint::new(true, CausalDirectionHint::Cause, 0.2, vec![]);
        assert_eq!(low_conf.bias_factors(), (1.0, 1.0));
    }

    #[test]
    fn test_causal_hint_confidence_clamping() {
        let hint = CausalHint::new(true, CausalDirectionHint::Cause, 1.5, vec![]);
        assert_eq!(hint.confidence, 1.0);

        let hint2 = CausalHint::new(true, CausalDirectionHint::Cause, -0.5, vec![]);
        assert_eq!(hint2.confidence, 0.0);
    }
}
