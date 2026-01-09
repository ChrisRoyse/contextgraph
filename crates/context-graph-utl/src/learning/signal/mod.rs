//! Complete UTL computation output with all component values and derived decisions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{UtlError, UtlResult};
use crate::johari::{JohariQuadrant, SuggestedAction};
use crate::lifecycle::LifecycleLambdaWeights;

use super::intensity::LearningIntensity;

/// Complete UTL computation output with all component values and derived decisions.
///
/// This struct captures the full result of a UTL computation including:
/// - Raw component values (delta_s, delta_c, w_e, phi)
/// - Computed learning magnitude
/// - Johari classification and suggested action
/// - Storage/consolidation recommendations
/// - Performance metrics (latency)
///
/// # Constitution Reference
///
/// - ΔS: [0,1] entropy/novelty (constitution.yaml:154)
/// - ΔC: [0,1] coherence/understanding (constitution.yaml:155)
/// - wₑ: [0.5,1.5] emotional weight (constitution.yaml:156)
/// - φ: [0,π] phase sync (constitution.yaml:157)
///
/// # Example
///
/// ```
/// use context_graph_utl::{LearningSignal, JohariQuadrant, SuggestedAction};
///
/// let signal = LearningSignal::new(
///     0.7, 0.6, 0.8, 1.2, 0.5,
///     None,
///     JohariQuadrant::Open,
///     SuggestedAction::DirectRecall,
///     false, true, 1500,
/// ).expect("Valid signal");
///
/// assert!(signal.magnitude >= 0.0 && signal.magnitude <= 1.0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSignal {
    /// Learning magnitude L in [0, 1], computed from formula
    pub magnitude: f32,

    /// Surprise (entropy) value [0, 1]
    pub delta_s: f32,

    /// Coherence value [0, 1]
    pub delta_c: f32,

    /// Emotional weight [0.5, 1.5]
    pub w_e: f32,

    /// Phase angle [0, PI]
    pub phi: f32,

    /// Marblestone lambda weights if lifecycle-adjusted
    pub lambda_weights: Option<LifecycleLambdaWeights>,

    /// Classified Johari quadrant
    pub quadrant: JohariQuadrant,

    /// Recommended retrieval action based on quadrant
    pub suggested_action: SuggestedAction,

    /// Whether consolidation is recommended (magnitude > 0.6)
    pub should_consolidate: bool,

    /// Whether storage is recommended (magnitude > 0.3)
    pub should_store: bool,

    /// When computation was performed (UTC)
    pub timestamp: DateTime<Utc>,

    /// Computation time in microseconds for performance tracking
    pub latency_us: u64,
}

impl LearningSignal {
    /// Create a new LearningSignal with validation.
    ///
    /// # Arguments
    /// * `magnitude` - Pre-computed learning magnitude [0, 1]
    /// * `delta_s` - Surprise component [0, 1]
    /// * `delta_c` - Coherence component [0, 1]
    /// * `w_e` - Emotional weight [0.5, 1.5]
    /// * `phi` - Phase angle [0, π]
    /// * `lambda_weights` - Optional lifecycle weights
    /// * `quadrant` - Johari classification
    /// * `suggested_action` - Recommended action
    /// * `should_consolidate` - Consolidation flag
    /// * `should_store` - Storage flag
    /// * `latency_us` - Computation time in microseconds
    ///
    /// # Returns
    /// `Ok(LearningSignal)` if valid, `Err(UtlError)` if magnitude is NaN/Infinity
    ///
    /// # Errors
    /// Returns `UtlError::InvalidComputation` if magnitude is NaN or Infinity
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        magnitude: f32,
        delta_s: f32,
        delta_c: f32,
        w_e: f32,
        phi: f32,
        lambda_weights: Option<LifecycleLambdaWeights>,
        quadrant: JohariQuadrant,
        suggested_action: SuggestedAction,
        should_consolidate: bool,
        should_store: bool,
        latency_us: u64,
    ) -> UtlResult<Self> {
        let signal = Self {
            magnitude,
            delta_s,
            delta_c,
            w_e,
            phi,
            lambda_weights,
            quadrant,
            suggested_action,
            should_consolidate,
            should_store,
            timestamp: Utc::now(),
            latency_us,
        };

        signal.validate()?;
        Ok(signal)
    }

    /// Validate that magnitude and components are finite (not NaN or Infinity).
    ///
    /// # Returns
    /// `Ok(())` if valid, `Err(UtlError::InvalidComputation)` otherwise
    pub fn validate(&self) -> UtlResult<()> {
        if self.magnitude.is_nan() {
            return Err(UtlError::InvalidComputation {
                delta_s: self.delta_s,
                delta_c: self.delta_c,
                w_e: self.w_e,
                phi: self.phi,
                reason: "magnitude is NaN".to_string(),
            });
        }

        if self.magnitude.is_infinite() {
            return Err(UtlError::InvalidComputation {
                delta_s: self.delta_s,
                delta_c: self.delta_c,
                w_e: self.w_e,
                phi: self.phi,
                reason: "magnitude is Infinity".to_string(),
            });
        }

        // Also check component values for NaN (fail fast)
        if self.delta_s.is_nan() || self.delta_c.is_nan() || self.w_e.is_nan() || self.phi.is_nan()
        {
            return Err(UtlError::InvalidComputation {
                delta_s: self.delta_s,
                delta_c: self.delta_c,
                w_e: self.w_e,
                phi: self.phi,
                reason: "component value is NaN".to_string(),
            });
        }

        Ok(())
    }

    /// Check if this signal indicates high learning potential (> 0.7)
    #[inline]
    pub fn is_high_learning(&self) -> bool {
        self.magnitude > 0.7
    }

    /// Check if this signal indicates low learning potential (< 0.3)
    #[inline]
    pub fn is_low_learning(&self) -> bool {
        self.magnitude < 0.3
    }

    /// Get the learning intensity category
    #[inline]
    pub fn intensity_category(&self) -> LearningIntensity {
        LearningIntensity::from_magnitude(self.magnitude)
    }
}

#[cfg(test)]
mod tests;
