//! CognitivePulse implementation for MCP response headers.
//!
//! CognitivePulse provides real-time UTL cognitive state in every MCP response.
//! All fields are REQUIRED - no Option types.
//! Computation MUST complete in < 1ms (warning logged if exceeded).
//!
//! # Constitution Reference
//!
//! Johari quadrant to action mapping (constitution.yaml:159-163):
//! - Open: delta_s < 0.5, delta_c > 0.5 -> DirectRecall
//! - Blind: delta_s > 0.5, delta_c < 0.5 -> TriggerDream
//! - Hidden: delta_s < 0.5, delta_c < 0.5 -> GetNeighborhood
//! - Unknown: delta_s > 0.5, delta_c > 0.5 -> EpistemicAction

use std::str::FromStr;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error, warn};

use context_graph_core::traits::UtlProcessor;
use context_graph_core::types::JohariQuadrant;
use context_graph_utl::johari::{get_suggested_action, SuggestedAction};

/// CognitivePulse provides real-time UTL state in every MCP response.
///
/// All fields are REQUIRED - no Option types.
/// Computation MUST complete in < 1ms.
///
/// # Example Response
///
/// ```json
/// {
///   "content": [{"type": "text", "text": "..."}],
///   "isError": false,
///   "_cognitive_pulse": {
///     "entropy": 0.42,
///     "coherence": 0.78,
///     "learning_score": 0.55,
///     "quadrant": "Open",
///     "suggested_action": "DirectRecall"
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CognitivePulse {
    /// Current entropy (surprise) level in [0.0, 1.0]
    pub entropy: f32,

    /// Current coherence level in [0.0, 1.0]
    pub coherence: f32,

    /// Current learning magnitude in [0.0, 1.0]
    pub learning_score: f32,

    /// Johari quadrant classification
    pub quadrant: JohariQuadrant,

    /// Suggested action per constitution.yaml:159-163
    pub suggested_action: SuggestedAction,
}

/// Error type for CognitivePulse computation failures.
///
/// FAIL FAST philosophy - no fallbacks, no workarounds.
/// All failures are logged with full context.
#[derive(Debug, Clone, Error)]
#[allow(dead_code)] // Variants prepared for future UTL integration
pub enum CognitivePulseError {
    /// Generic computation failure
    #[error("Computation failed: {0}")]
    ComputationFailed(String),

    /// Specific field validation failure
    #[error("Invalid input for field '{field}': {reason}")]
    InvalidInput { field: String, reason: String },

    /// UTL processor-level failure
    #[error("UTL processor error: {0}")]
    UtlProcessorError(String),
}

impl CognitivePulse {
    /// Create pulse from UTL processor state.
    ///
    /// Extracts current UTL metrics from the processor and maps
    /// to CognitivePulse fields. All fields are required - if any
    /// field is missing, returns error.
    ///
    /// # Arguments
    ///
    /// * `processor` - Reference to UTL processor implementing get_status()
    ///
    /// # Returns
    ///
    /// * `Ok(CognitivePulse)` - Successfully computed pulse
    /// * `Err(CognitivePulseError)` - Failed to compute, with detailed error
    ///
    /// # Performance
    ///
    /// Target: < 1ms. Warning logged if exceeded, but request continues.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let processor = get_utl_processor();
    /// let pulse = CognitivePulse::from_processor(processor.as_ref())?;
    /// ```
    pub fn from_processor(processor: &dyn UtlProcessor) -> Result<Self, CognitivePulseError> {
        let start = Instant::now();

        // Get status JSON from processor
        let status = processor.get_status();

        // Extract entropy - REQUIRED, no fallback
        let entropy = status
            .get("entropy")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .ok_or_else(|| {
                error!("CognitivePulse: missing 'entropy' in UTL status");
                CognitivePulseError::ComputationFailed("missing entropy in status".into())
            })?;

        // Extract coherence - REQUIRED, no fallback
        let coherence = status
            .get("coherence")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .ok_or_else(|| {
                error!("CognitivePulse: missing 'coherence' in UTL status");
                CognitivePulseError::ComputationFailed("missing coherence in status".into())
            })?;

        // Extract learning_score - REQUIRED, no fallback
        let learning_score = status
            .get("learning_score")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .ok_or_else(|| {
                error!("CognitivePulse: missing 'learning_score' in UTL status");
                CognitivePulseError::ComputationFailed("missing learning_score in status".into())
            })?;

        // Extract johari_quadrant - REQUIRED, no fallback
        let quadrant_str = status
            .get("johari_quadrant")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                error!("CognitivePulse: missing 'johari_quadrant' in UTL status");
                CognitivePulseError::ComputationFailed("missing johari_quadrant in status".into())
            })?;

        // Parse quadrant string to enum
        let quadrant = JohariQuadrant::from_str(quadrant_str).map_err(|e| {
            error!(
                quadrant = quadrant_str,
                error = %e,
                "CognitivePulse: invalid johari_quadrant value"
            );
            CognitivePulseError::InvalidInput {
                field: "quadrant".into(),
                reason: format!("invalid quadrant '{}': {}", quadrant_str, e),
            }
        })?;

        // Get suggested action from quadrant using existing function
        // This implements constitution.yaml:159-163 mapping
        let suggested_action = get_suggested_action(quadrant);

        // Construct pulse
        let pulse = Self {
            entropy,
            coherence,
            learning_score,
            quadrant,
            suggested_action,
        };

        // Validate all values in range
        pulse.validate()?;

        // Log timing
        let elapsed = start.elapsed();
        if elapsed.as_micros() > 1000 {
            warn!(
                elapsed_us = elapsed.as_micros(),
                entropy = pulse.entropy,
                coherence = pulse.coherence,
                quadrant = ?pulse.quadrant,
                "CognitivePulse computation exceeded 1ms target"
            );
        } else {
            debug!(
                elapsed_us = elapsed.as_micros(),
                entropy = pulse.entropy,
                coherence = pulse.coherence,
                learning_score = pulse.learning_score,
                quadrant = ?pulse.quadrant,
                suggested_action = ?pulse.suggested_action,
                "CognitivePulse computed successfully"
            );
        }

        Ok(pulse)
    }

    /// Create pulse directly from values.
    ///
    /// Used when values are already available (e.g., in tests or
    /// when extracting from already-parsed status).
    ///
    /// # Arguments
    ///
    /// * `entropy` - Entropy value [0.0, 1.0]
    /// * `coherence` - Coherence value [0.0, 1.0]
    /// * `learning_score` - Learning score [0.0, 1.0]
    /// * `quadrant` - Johari quadrant classification
    ///
    /// # Returns
    ///
    /// * `Ok(CognitivePulse)` - Valid pulse
    /// * `Err(CognitivePulseError)` - Validation failed
    #[allow(dead_code)] // Prepared for test and direct construction use cases
    pub fn from_values(
        entropy: f32,
        coherence: f32,
        learning_score: f32,
        quadrant: JohariQuadrant,
    ) -> Result<Self, CognitivePulseError> {
        let suggested_action = get_suggested_action(quadrant);

        let pulse = Self {
            entropy,
            coherence,
            learning_score,
            quadrant,
            suggested_action,
        };

        pulse.validate()?;
        Ok(pulse)
    }

    /// Validate all pulse values are within expected ranges.
    ///
    /// # Validation Rules
    ///
    /// - entropy: [0.0, 1.0]
    /// - coherence: [0.0, 1.0]
    /// - learning_score: [0.0, 1.0]
    /// - quadrant: must be valid JohariQuadrant (enforced by type)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All values valid
    /// * `Err(CognitivePulseError::InvalidInput)` - Validation failed
    pub fn validate(&self) -> Result<(), CognitivePulseError> {
        // Validate entropy range
        if !(0.0..=1.0).contains(&self.entropy) {
            error!(
                entropy = self.entropy,
                "CognitivePulse: entropy out of range [0.0, 1.0]"
            );
            return Err(CognitivePulseError::InvalidInput {
                field: "entropy".into(),
                reason: format!("value {} not in [0.0, 1.0]", self.entropy),
            });
        }

        // Validate coherence range
        if !(0.0..=1.0).contains(&self.coherence) {
            error!(
                coherence = self.coherence,
                "CognitivePulse: coherence out of range [0.0, 1.0]"
            );
            return Err(CognitivePulseError::InvalidInput {
                field: "coherence".into(),
                reason: format!("value {} not in [0.0, 1.0]", self.coherence),
            });
        }

        // Validate learning_score range
        if !(0.0..=1.0).contains(&self.learning_score) {
            error!(
                learning_score = self.learning_score,
                "CognitivePulse: learning_score out of range [0.0, 1.0]"
            );
            return Err(CognitivePulseError::InvalidInput {
                field: "learning_score".into(),
                reason: format!("value {} not in [0.0, 1.0]", self.learning_score),
            });
        }

        // Validate NaN (fail fast)
        if self.entropy.is_nan() || self.coherence.is_nan() || self.learning_score.is_nan() {
            error!("CognitivePulse: NaN value detected");
            return Err(CognitivePulseError::InvalidInput {
                field: "multiple".into(),
                reason: "NaN value detected".into(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cognitive_pulse_from_values_valid() {
        let pulse =
            CognitivePulse::from_values(0.42, 0.78, 0.55, JohariQuadrant::Open).expect("valid");

        assert!((pulse.entropy - 0.42).abs() < 0.001);
        assert!((pulse.coherence - 0.78).abs() < 0.001);
        assert!((pulse.learning_score - 0.55).abs() < 0.001);
        assert_eq!(pulse.quadrant, JohariQuadrant::Open);
        assert_eq!(pulse.suggested_action, SuggestedAction::DirectRecall);
    }

    #[test]
    fn test_cognitive_pulse_johari_action_mapping() {
        // constitution.yaml:159-163 compliance
        let open = CognitivePulse::from_values(0.3, 0.7, 0.5, JohariQuadrant::Open).expect("valid");
        assert_eq!(open.suggested_action, SuggestedAction::DirectRecall);

        let blind =
            CognitivePulse::from_values(0.7, 0.3, 0.5, JohariQuadrant::Blind).expect("valid");
        assert_eq!(blind.suggested_action, SuggestedAction::EpistemicAction);

        let hidden =
            CognitivePulse::from_values(0.3, 0.3, 0.5, JohariQuadrant::Hidden).expect("valid");
        assert_eq!(hidden.suggested_action, SuggestedAction::GetNeighborhood);

        let unknown =
            CognitivePulse::from_values(0.7, 0.7, 0.5, JohariQuadrant::Unknown).expect("valid");
        assert_eq!(unknown.suggested_action, SuggestedAction::TriggerDream);
    }

    #[test]
    fn test_cognitive_pulse_validation_entropy_out_of_range() {
        let result = CognitivePulse::from_values(1.5, 0.5, 0.5, JohariQuadrant::Open);
        assert!(result.is_err());

        match result.unwrap_err() {
            CognitivePulseError::InvalidInput { field, .. } => {
                assert_eq!(field, "entropy");
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_cognitive_pulse_validation_coherence_out_of_range() {
        let result = CognitivePulse::from_values(0.5, -0.1, 0.5, JohariQuadrant::Open);
        assert!(result.is_err());

        match result.unwrap_err() {
            CognitivePulseError::InvalidInput { field, .. } => {
                assert_eq!(field, "coherence");
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_cognitive_pulse_validation_learning_score_out_of_range() {
        let result = CognitivePulse::from_values(0.5, 0.5, 2.0, JohariQuadrant::Open);
        assert!(result.is_err());

        match result.unwrap_err() {
            CognitivePulseError::InvalidInput { field, .. } => {
                assert_eq!(field, "learning_score");
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_cognitive_pulse_validation_nan() {
        let result = CognitivePulse::from_values(f32::NAN, 0.5, 0.5, JohariQuadrant::Open);
        assert!(result.is_err());
    }

    #[test]
    fn test_cognitive_pulse_serialization() {
        let pulse =
            CognitivePulse::from_values(0.42, 0.78, 0.55, JohariQuadrant::Open).expect("valid");

        let json = serde_json::to_string(&pulse).expect("serialize");
        assert!(json.contains("\"entropy\":0.42"));
        assert!(json.contains("\"coherence\":0.78"));
        assert!(json.contains("\"learning_score\":0.55"));

        let deserialized: CognitivePulse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(pulse, deserialized);
    }

    #[test]
    fn test_cognitive_pulse_boundary_values() {
        // Minimum values
        let min =
            CognitivePulse::from_values(0.0, 0.0, 0.0, JohariQuadrant::Hidden).expect("valid");
        assert_eq!(min.entropy, 0.0);
        assert_eq!(min.coherence, 0.0);
        assert_eq!(min.learning_score, 0.0);

        // Maximum values
        let max = CognitivePulse::from_values(1.0, 1.0, 1.0, JohariQuadrant::Open).expect("valid");
        assert_eq!(max.entropy, 1.0);
        assert_eq!(max.coherence, 1.0);
        assert_eq!(max.learning_score, 1.0);
    }

    #[test]
    fn test_cognitive_pulse_error_display() {
        let err = CognitivePulseError::ComputationFailed("test error".into());
        assert_eq!(format!("{}", err), "Computation failed: test error");

        let err = CognitivePulseError::InvalidInput {
            field: "entropy".into(),
            reason: "out of range".into(),
        };
        assert!(format!("{}", err).contains("entropy"));
        assert!(format!("{}", err).contains("out of range"));

        let err = CognitivePulseError::UtlProcessorError("processor down".into());
        assert_eq!(format!("{}", err), "UTL processor error: processor down");
    }
}
