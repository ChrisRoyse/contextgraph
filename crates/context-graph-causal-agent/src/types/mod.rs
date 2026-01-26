//! Type definitions for the Causal Discovery Agent.
//!
//! Contains structures for causal analysis results, candidate pairs,
//! and configuration options.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub fn from_str(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.contains("a_causes_b") || lower.contains("a causes b") || lower == "forward" {
            Self::ACausesB
        } else if lower.contains("b_causes_a") || lower.contains("b causes a") || lower == "backward"
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
        assert!(CausalMarkers::has_cause_marker("This happened because of X"));
        assert!(CausalMarkers::has_effect_marker("Therefore, Y occurred"));
        assert!(!CausalMarkers::has_cause_marker("Hello world"));
        assert_eq!(
            CausalMarkers::count_markers("Because of X, therefore Y"),
            2
        );
    }
}
