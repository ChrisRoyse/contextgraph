//! Configuration for entailment cones in hyperbolic space.

use serde::{Deserialize, Serialize};

use crate::error::GraphError;

/// Configuration for entailment cones in hyperbolic space.
///
/// Entailment cones enable O(1) IS-A hierarchy queries. A concept's cone
/// contains all concepts it subsumes. Aperture narrows with depth,
/// creating increasingly specific cones for child concepts.
///
/// # Mathematics
///
/// - Aperture at depth d: `aperture(d) = base_aperture * decay^d`
/// - Result clamped to `[min_aperture, max_aperture]`
/// - Cone A contains point P iff angle(P - apex, axis) <= aperture
///
/// # Constitution Reference
///
/// - perf.latency.entailment_check: <1ms
/// - Section 9 "HYPERBOLIC ENTAILMENT CONES" in contextprd.md
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConeConfig {
    /// Minimum cone aperture in radians.
    /// Prevents cones from becoming too narrow at deep levels.
    /// Default: 0.1 rad (~5.7 degrees)
    pub min_aperture: f32,

    /// Maximum cone aperture in radians.
    /// Prevents cones from becoming too wide at root level.
    /// Default: 1.5 rad (~85.9 degrees)
    pub max_aperture: f32,

    /// Base aperture for depth 0 nodes (root concepts).
    /// This is the starting aperture before decay is applied.
    /// Default: 1.0 rad (~57.3 degrees)
    pub base_aperture: f32,

    /// Decay factor applied per hierarchy level.
    /// Must be in open interval (0, 1).
    /// Default: 0.85 (15% narrower per level)
    pub aperture_decay: f32,

    /// Threshold for soft membership scoring.
    /// Points with membership score >= threshold are considered contained.
    /// Must be in open interval (0, 1).
    /// Default: 0.7
    pub membership_threshold: f32,
}

impl Default for ConeConfig {
    fn default() -> Self {
        Self {
            min_aperture: 0.1,    // ~5.7 degrees
            max_aperture: 1.5,    // ~85.9 degrees
            base_aperture: 1.0,   // ~57.3 degrees
            aperture_decay: 0.85, // 15% narrower per level
            membership_threshold: 0.7,
        }
    }
}

impl ConeConfig {
    /// Compute aperture for a node at given depth.
    ///
    /// # Formula
    /// `aperture = base_aperture * aperture_decay^depth`
    /// Result is clamped to `[min_aperture, max_aperture]`.
    ///
    /// # Arguments
    /// * `depth` - Depth in hierarchy (0 = root)
    ///
    /// # Returns
    /// Aperture in radians, clamped to valid range.
    ///
    /// # Example
    /// ```
    /// use context_graph_graph::config::ConeConfig;
    ///
    /// let config = ConeConfig::default();
    /// assert_eq!(config.compute_aperture(0), 1.0);  // base at root
    /// assert!((config.compute_aperture(1) - 0.85).abs() < 1e-6);  // 1.0 * 0.85
    /// assert_eq!(config.compute_aperture(100), 0.1);  // clamped to min
    /// ```
    pub fn compute_aperture(&self, depth: u32) -> f32 {
        let raw = self.base_aperture * self.aperture_decay.powi(depth as i32);
        raw.clamp(self.min_aperture, self.max_aperture)
    }

    /// Validate configuration parameters.
    ///
    /// # Validation Rules
    /// - `min_aperture` > 0: Must be positive
    /// - `max_aperture` > `min_aperture`: Max must exceed min
    /// - `base_aperture` in [`min_aperture`, `max_aperture`]: Base must be in valid range
    /// - `aperture_decay` in (0, 1): Must be strictly between 0 and 1
    /// - `membership_threshold` in (0, 1): Must be strictly between 0 and 1
    ///
    /// # Errors
    /// Returns `GraphError::InvalidConfig` with descriptive message if any
    /// parameter is invalid. Fails fast on first error.
    ///
    /// # Example
    /// ```
    /// use context_graph_graph::config::ConeConfig;
    ///
    /// let valid = ConeConfig::default();
    /// assert!(valid.validate().is_ok());
    ///
    /// let mut invalid = ConeConfig::default();
    /// invalid.aperture_decay = 1.5;  // must be < 1
    /// assert!(invalid.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), GraphError> {
        // Check for NaN in min_aperture
        if self.min_aperture.is_nan() {
            return Err(GraphError::InvalidConfig(
                "min_aperture cannot be NaN".to_string(),
            ));
        }

        // Check min_aperture is positive
        if self.min_aperture <= 0.0 {
            return Err(GraphError::InvalidConfig(format!(
                "min_aperture must be positive (got {})",
                self.min_aperture
            )));
        }

        // Check for NaN in max_aperture
        if self.max_aperture.is_nan() {
            return Err(GraphError::InvalidConfig(
                "max_aperture cannot be NaN".to_string(),
            ));
        }

        // Check max_aperture > min_aperture
        if self.max_aperture <= self.min_aperture {
            return Err(GraphError::InvalidConfig(format!(
                "max_aperture ({}) must be greater than min_aperture ({})",
                self.max_aperture, self.min_aperture
            )));
        }

        // Check for NaN in base_aperture
        if self.base_aperture.is_nan() {
            return Err(GraphError::InvalidConfig(
                "base_aperture cannot be NaN".to_string(),
            ));
        }

        // Check base_aperture is in valid range
        if self.base_aperture < self.min_aperture || self.base_aperture > self.max_aperture {
            return Err(GraphError::InvalidConfig(format!(
                "base_aperture ({}) must be in range [{}, {}]",
                self.base_aperture, self.min_aperture, self.max_aperture
            )));
        }

        // Check for NaN in aperture_decay
        if self.aperture_decay.is_nan() {
            return Err(GraphError::InvalidConfig(
                "aperture_decay cannot be NaN".to_string(),
            ));
        }

        // Check aperture_decay in (0, 1)
        if self.aperture_decay <= 0.0 || self.aperture_decay >= 1.0 {
            return Err(GraphError::InvalidConfig(format!(
                "aperture_decay must be in open interval (0, 1), got {}",
                self.aperture_decay
            )));
        }

        // Check for NaN in membership_threshold
        if self.membership_threshold.is_nan() {
            return Err(GraphError::InvalidConfig(
                "membership_threshold cannot be NaN".to_string(),
            ));
        }

        // Check membership_threshold in (0, 1)
        if self.membership_threshold <= 0.0 || self.membership_threshold >= 1.0 {
            return Err(GraphError::InvalidConfig(format!(
                "membership_threshold must be in open interval (0, 1), got {}",
                self.membership_threshold
            )));
        }

        Ok(())
    }
}
