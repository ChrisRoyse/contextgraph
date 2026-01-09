//! External signal types for blind spot discovery.
//!
//! External signals represent observations from "others" (external systems, users,
//! dream layers) that may indicate blind spots in the Johari classification.
//!
//! From constitution.yaml: Memory can be Open(semantic) but Blind(causal).

use chrono::{DateTime, Utc};

use crate::types::JohariQuadrant;

/// Number of embedders in the system (E1-E13).
const NUM_EMBEDDERS: usize = 13;

/// Signal from external source referencing an embedder space.
///
/// External signals indicate that "others" have knowledge about a memory
/// dimension that the system may be unaware of (Blind spot).
///
/// # Invariants
/// - `embedder_idx` must be in range 0-12 (enforced by constructor)
/// - `strength` must be in range [0.0, 1.0] (enforced by constructor)
#[derive(Debug, Clone)]
pub struct ExternalSignal {
    /// Source identifier (e.g., "user_feedback", "dream_layer", "curator")
    pub source: String,

    /// Which embedder space this signal references (0-12 for E1-E13)
    pub embedder_idx: usize,

    /// Signal strength [0.0, 1.0]
    /// Higher values indicate stronger external awareness
    pub strength: f32,

    /// Optional description of what was observed
    pub description: Option<String>,

    /// When the signal was generated
    pub timestamp: DateTime<Utc>,
}

impl ExternalSignal {
    /// Create a new external signal with validation.
    ///
    /// # Arguments
    /// * `source` - Identifier of the signal source
    /// * `embedder_idx` - Index of the embedder space (0-12)
    /// * `strength` - Signal strength [0.0, 1.0]
    ///
    /// # Panics
    /// - Panics if `embedder_idx >= 13`
    /// - Panics if `strength` is not in [0.0, 1.0]
    pub fn new(source: impl Into<String>, embedder_idx: usize, strength: f32) -> Self {
        assert!(
            embedder_idx < NUM_EMBEDDERS,
            "embedder_idx must be 0-12, got {}",
            embedder_idx
        );
        assert!(
            (0.0..=1.0).contains(&strength),
            "strength must be [0,1], got {}",
            strength
        );

        Self {
            source: source.into(),
            embedder_idx,
            strength,
            description: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a signal with a description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Create a signal with a specific timestamp.
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Get the embedder name for this signal's embedder index.
    pub fn embedder_name(&self) -> &'static str {
        match self.embedder_idx {
            0 => "E1_Semantic",
            1 => "E2_Temporal_Recent",
            2 => "E3_Temporal_Periodic",
            3 => "E4_Temporal_Positional",
            4 => "E5_Causal",
            5 => "E6_Sparse_Lexical",
            6 => "E7_Code",
            7 => "E8_Graph",
            8 => "E9_HDC",
            9 => "E10_Multimodal",
            10 => "E11_Entity",
            11 => "E12_Late_Interaction",
            12 => "E13_SPLADE",
            _ => "Unknown",
        }
    }
}

/// Candidate blind spot discovered from external signals.
///
/// A blind spot is a dimension where the system is unaware of knowledge
/// that external sources have observed. This is a candidate for transition
/// to the Blind quadrant.
#[derive(Debug, Clone)]
pub struct BlindSpotCandidate {
    /// Which embedder space has the blind spot
    pub embedder_idx: usize,

    /// Current quadrant (will be Unknown or Hidden)
    pub current_quadrant: JohariQuadrant,

    /// Aggregate signal strength from external sources
    pub signal_strength: f32,

    /// Suggested transition target (always Blind for blind spots)
    pub suggested_transition: JohariQuadrant,

    /// Sources that contributed to this discovery
    pub sources: Vec<String>,
}

impl BlindSpotCandidate {
    /// Create a new blind spot candidate.
    ///
    /// # Arguments
    /// * `embedder_idx` - Index of the embedder space with the blind spot
    /// * `current_quadrant` - Current quadrant (should be Unknown or Hidden)
    /// * `signal_strength` - Aggregate signal strength from external sources
    /// * `sources` - List of source identifiers
    pub fn new(
        embedder_idx: usize,
        current_quadrant: JohariQuadrant,
        signal_strength: f32,
        sources: Vec<String>,
    ) -> Self {
        Self {
            embedder_idx,
            current_quadrant,
            signal_strength,
            suggested_transition: JohariQuadrant::Blind,
            sources,
        }
    }

    /// Get the embedder name for this candidate's embedder index.
    pub fn embedder_name(&self) -> &'static str {
        match self.embedder_idx {
            0 => "E1_Semantic",
            1 => "E2_Temporal_Recent",
            2 => "E3_Temporal_Periodic",
            3 => "E4_Temporal_Positional",
            4 => "E5_Causal",
            5 => "E6_Sparse_Lexical",
            6 => "E7_Code",
            7 => "E8_Graph",
            8 => "E9_HDC",
            9 => "E10_Multimodal",
            10 => "E11_Entity",
            11 => "E12_Late_Interaction",
            12 => "E13_SPLADE",
            _ => "Unknown",
        }
    }

    /// Check if the transition is valid from the current quadrant.
    ///
    /// Blind spots can only be discovered from Unknown or Hidden quadrants.
    pub fn is_valid_candidate(&self) -> bool {
        matches!(
            self.current_quadrant,
            JohariQuadrant::Unknown | JohariQuadrant::Hidden
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_signal_new() {
        let signal = ExternalSignal::new("user_feedback", 5, 0.7);
        assert_eq!(signal.source, "user_feedback");
        assert_eq!(signal.embedder_idx, 5);
        assert!((signal.strength - 0.7).abs() < f32::EPSILON);
        assert!(signal.description.is_none());

        println!("[VERIFIED] test_external_signal_new: Signal creation works correctly");
    }

    #[test]
    fn test_external_signal_with_description() {
        let signal =
            ExternalSignal::new("dream_layer", 4, 0.5).with_description("Causal pattern detected");

        assert_eq!(
            signal.description,
            Some("Causal pattern detected".to_string())
        );

        println!("[VERIFIED] test_external_signal_with_description: Description added correctly");
    }

    #[test]
    #[should_panic(expected = "embedder_idx must be 0-12")]
    fn test_external_signal_invalid_embedder() {
        ExternalSignal::new("test", 13, 0.5);
    }

    #[test]
    #[should_panic(expected = "strength must be [0,1]")]
    fn test_external_signal_invalid_strength_high() {
        ExternalSignal::new("test", 5, 1.5);
    }

    #[test]
    #[should_panic(expected = "strength must be [0,1]")]
    fn test_external_signal_invalid_strength_negative() {
        ExternalSignal::new("test", 5, -0.1);
    }

    #[test]
    fn test_external_signal_embedder_name() {
        let signal = ExternalSignal::new("test", 4, 0.5);
        assert_eq!(signal.embedder_name(), "E5_Causal");

        let signal = ExternalSignal::new("test", 0, 0.5);
        assert_eq!(signal.embedder_name(), "E1_Semantic");

        println!("[VERIFIED] test_external_signal_embedder_name: Embedder names correct");
    }

    #[test]
    fn test_blind_spot_candidate() {
        let candidate = BlindSpotCandidate::new(
            5,
            JohariQuadrant::Unknown,
            0.75,
            vec!["user".to_string(), "dream".to_string()],
        );

        assert_eq!(candidate.embedder_idx, 5);
        assert_eq!(candidate.current_quadrant, JohariQuadrant::Unknown);
        assert!((candidate.signal_strength - 0.75).abs() < f32::EPSILON);
        assert_eq!(candidate.suggested_transition, JohariQuadrant::Blind);
        assert_eq!(candidate.sources.len(), 2);
        assert!(candidate.is_valid_candidate());

        println!("[VERIFIED] test_blind_spot_candidate: Candidate creation works correctly");
    }

    #[test]
    fn test_blind_spot_candidate_invalid_quadrant() {
        let candidate =
            BlindSpotCandidate::new(5, JohariQuadrant::Open, 0.75, vec!["test".to_string()]);

        // Open quadrant is not a valid source for blind spot discovery
        assert!(!candidate.is_valid_candidate());

        println!(
            "[VERIFIED] test_blind_spot_candidate_invalid_quadrant: Validates quadrant correctly"
        );
    }
}
