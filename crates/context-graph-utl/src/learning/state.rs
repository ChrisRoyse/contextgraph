//! Compact UTL state for storage in MemoryNode.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{UtlError, UtlResult};
use crate::johari::JohariQuadrant;

use super::signal::LearningSignal;

/// Compact UTL state for storage in MemoryNode.
///
/// This is the persistent representation stored with each memory node,
/// containing only the essential values needed for retrieval decisions.
/// Use `UtlState::from_signal()` to convert from a full `LearningSignal`.
///
/// # Example
///
/// ```
/// use context_graph_utl::{LearningSignal, UtlState, JohariQuadrant, SuggestedAction};
///
/// let signal = LearningSignal::new(
///     0.7, 0.6, 0.8, 1.2, 0.5, None,
///     JohariQuadrant::Blind, SuggestedAction::TriggerDream,
///     true, true, 2000,
/// ).unwrap();
///
/// let state = UtlState::from_signal(&signal);
/// assert_eq!(state.learning_magnitude, 0.7);
/// assert_eq!(state.quadrant, JohariQuadrant::Blind);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtlState {
    /// Last computed surprise value [0, 1]
    pub delta_s: f32,

    /// Last computed coherence value [0, 1]
    pub delta_c: f32,

    /// Last computed emotional weight [0.5, 1.5]
    pub w_e: f32,

    /// Phase angle at computation [0, PI]
    pub phi: f32,

    /// Computed learning magnitude [0, 1]
    pub learning_magnitude: f32,

    /// Classified Johari quadrant
    pub quadrant: JohariQuadrant,

    /// When state was last updated (UTC)
    pub last_computed: DateTime<Utc>,
}

impl UtlState {
    /// Create a new UtlState from a LearningSignal.
    ///
    /// Extracts the essential values for persistent storage.
    pub fn from_signal(signal: &LearningSignal) -> Self {
        Self {
            delta_s: signal.delta_s,
            delta_c: signal.delta_c,
            w_e: signal.w_e,
            phi: signal.phi,
            learning_magnitude: signal.magnitude,
            quadrant: signal.quadrant,
            last_computed: signal.timestamp,
        }
    }

    /// Create a default/empty UtlState for new nodes.
    ///
    /// Uses neutral defaults:
    /// - delta_s = 0.0 (no surprise)
    /// - delta_c = 0.0 (no coherence established)
    /// - w_e = 1.0 (neutral emotional state)
    /// - phi = 0.0 (synchronized)
    /// - learning_magnitude = 0.5 (medium baseline)
    /// - quadrant = Hidden (low surprise, low coherence)
    pub fn empty() -> Self {
        Self {
            delta_s: 0.0,
            delta_c: 0.0,
            w_e: 1.0,
            phi: 0.0,
            learning_magnitude: 0.5,
            quadrant: JohariQuadrant::Hidden,
            last_computed: Utc::now(),
        }
    }

    /// Validate that all values are finite (not NaN or Infinity).
    ///
    /// # Returns
    /// `Ok(())` if all values are finite, `Err(UtlError::InvalidComputation)` otherwise
    pub fn validate(&self) -> UtlResult<()> {
        let values = [
            ("delta_s", self.delta_s),
            ("delta_c", self.delta_c),
            ("w_e", self.w_e),
            ("phi", self.phi),
            ("learning_magnitude", self.learning_magnitude),
        ];

        for (name, value) in values {
            if value.is_nan() || value.is_infinite() {
                return Err(UtlError::InvalidComputation {
                    delta_s: self.delta_s,
                    delta_c: self.delta_c,
                    w_e: self.w_e,
                    phi: self.phi,
                    reason: format!("{} is NaN or Infinity", name),
                });
            }
        }

        Ok(())
    }

    /// Check if this state is stale (not computed recently).
    ///
    /// # Arguments
    /// * `max_age_seconds` - Maximum age in seconds before considered stale
    ///
    /// # Returns
    /// `true` if state is older than `max_age_seconds`
    pub fn is_stale(&self, max_age_seconds: i64) -> bool {
        let age = Utc::now() - self.last_computed;
        age.num_seconds() > max_age_seconds
    }

    /// Get the age of this state in seconds.
    pub fn age_seconds(&self) -> i64 {
        (Utc::now() - self.last_computed).num_seconds()
    }
}

impl Default for UtlState {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::johari::SuggestedAction;

    #[test]
    fn test_utl_state_from_signal() {
        let signal = LearningSignal::new(
            0.7,
            0.6,
            0.8,
            1.2,
            0.5,
            None,
            JohariQuadrant::Blind,
            SuggestedAction::TriggerDream,
            true,
            true,
            2000,
        )
        .unwrap();

        let state = UtlState::from_signal(&signal);

        assert_eq!(state.delta_s, 0.6);
        assert_eq!(state.delta_c, 0.8);
        assert_eq!(state.w_e, 1.2);
        assert_eq!(state.phi, 0.5);
        assert_eq!(state.learning_magnitude, 0.7);
        assert_eq!(state.quadrant, JohariQuadrant::Blind);
        assert_eq!(state.last_computed, signal.timestamp);
    }

    #[test]
    fn test_utl_state_empty() {
        let state = UtlState::empty();

        assert_eq!(state.delta_s, 0.0);
        assert_eq!(state.delta_c, 0.0);
        assert_eq!(state.w_e, 1.0);
        assert_eq!(state.phi, 0.0);
        assert_eq!(state.learning_magnitude, 0.5);
        assert_eq!(state.quadrant, JohariQuadrant::Hidden);
    }

    #[test]
    fn test_utl_state_default_matches_empty() {
        let default_state = UtlState::default();
        let empty_state = UtlState::empty();

        assert_eq!(default_state.delta_s, empty_state.delta_s);
        assert_eq!(default_state.delta_c, empty_state.delta_c);
        assert_eq!(default_state.w_e, empty_state.w_e);
        assert_eq!(default_state.phi, empty_state.phi);
        assert_eq!(
            default_state.learning_magnitude,
            empty_state.learning_magnitude
        );
        assert_eq!(default_state.quadrant, empty_state.quadrant);
    }

    #[test]
    fn test_utl_state_staleness() {
        // Fresh state should not be stale
        let state = UtlState::empty();
        assert!(!state.is_stale(60)); // Not stale within 60 seconds

        // age_seconds() should be very small (just created)
        let age = state.age_seconds();
        assert!((0..2).contains(&age)); // Should be 0 or 1 second old
    }

    #[test]
    fn test_utl_state_validation_success() {
        let state = UtlState::empty();
        assert!(state.validate().is_ok());
    }

    #[test]
    fn test_utl_state_validation_nan() {
        let state = UtlState {
            delta_s: f32::NAN,
            delta_c: 0.5,
            w_e: 1.0,
            phi: 0.0,
            learning_magnitude: 0.5,
            quadrant: JohariQuadrant::Hidden,
            last_computed: Utc::now(),
        };
        assert!(state.validate().is_err());
    }

    #[test]
    fn test_utl_state_validation_infinity() {
        let state = UtlState {
            delta_s: 0.5,
            delta_c: f32::INFINITY,
            w_e: 1.0,
            phi: 0.0,
            learning_magnitude: 0.5,
            quadrant: JohariQuadrant::Hidden,
            last_computed: Utc::now(),
        };
        assert!(state.validate().is_err());
    }

    #[test]
    fn test_utl_state_serialization_roundtrip() {
        let original = UtlState {
            delta_s: 0.6,
            delta_c: 0.8,
            w_e: 1.2,
            phi: 0.5,
            learning_magnitude: 0.7,
            quadrant: JohariQuadrant::Blind,
            last_computed: Utc::now(),
        };

        let json = serde_json::to_string(&original).expect("Serialization failed");
        let deserialized: UtlState = serde_json::from_str(&json).expect("Deserialization failed");

        assert_eq!(deserialized.delta_s, original.delta_s);
        assert_eq!(deserialized.delta_c, original.delta_c);
        assert_eq!(deserialized.w_e, original.w_e);
        assert_eq!(deserialized.phi, original.phi);
        assert_eq!(deserialized.learning_magnitude, original.learning_magnitude);
        assert_eq!(deserialized.quadrant, original.quadrant);
    }
}
