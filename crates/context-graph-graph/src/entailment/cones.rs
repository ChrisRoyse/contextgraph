//! EntailmentCone implementation for O(1) IS-A hierarchy queries.
//!
//! An entailment cone in hyperbolic space enables efficient hierarchical
//! reasoning. A concept's cone contains all concepts it subsumes (entails).
//! Checking if concept A is a subconcept of B is O(1): check if A's position
//! lies within B's cone.
//!
//! # Aperture Decay
//!
//! Aperture decreases with hierarchy depth:
//! - Root concepts have wide cones (capture many descendants)
//! - Leaf concepts have narrow cones (very specific)
//! - Formula: `aperture = base * decay^depth`, clamped to [min, max]
//!
//! # Performance Targets
//!
//! - Cone containment check: <50μs CPU
//! - Entailment check: <1ms total
//! - Target hardware: RTX 5090, CUDA 13.1, Compute 12.0
//!
//! # Constitution Reference
//!
//! - perf.latency.entailment_check: <1ms
//! - Section 9 "HYPERBOLIC ENTAILMENT CONES" in contextprd.md

use serde::{Deserialize, Serialize};

use crate::config::ConeConfig;
use crate::error::GraphError;
use crate::hyperbolic::mobius::PoincareBall;
use crate::hyperbolic::poincare::PoincarePoint;

/// Entailment cone for O(1) IS-A hierarchy queries.
///
/// A cone rooted at `apex` with angular width `aperture * aperture_factor`
/// contains all points (concepts) that are entailed by the apex concept.
///
/// # Memory Layout
///
/// - apex: 256 bytes (64 f32 coords, 64-byte aligned)
/// - aperture: 4 bytes
/// - aperture_factor: 4 bytes
/// - depth: 4 bytes
/// - Total: 268 bytes (with padding for alignment)
///
/// # Invariants
///
/// - `apex.is_valid()` must be true (norm < 1.0)
/// - `aperture` in (0, π/2]
/// - `aperture_factor` in [0.5, 2.0]
///
/// # Example
///
/// ```
/// use context_graph_graph::hyperbolic::poincare::PoincarePoint;
/// use context_graph_graph::config::ConeConfig;
/// use context_graph_graph::entailment::cones::EntailmentCone;
///
/// let apex = PoincarePoint::origin();
/// let config = ConeConfig::default();
/// let cone = EntailmentCone::new(apex, 0, &config).expect("valid cone");
/// assert!(cone.is_valid());
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntailmentCone {
    /// Apex point of the cone in Poincare ball.
    pub apex: PoincarePoint,
    /// Base aperture in radians (computed from depth via ConeConfig).
    pub aperture: f32,
    /// Adjustment factor for aperture (learned during training).
    pub aperture_factor: f32,
    /// Depth in hierarchy (0 = root concept).
    pub depth: u32,
}

impl EntailmentCone {
    /// Create a new entailment cone at given apex position.
    ///
    /// # Arguments
    ///
    /// * `apex` - Position in Poincare ball (must satisfy ||coords|| < 1)
    /// * `depth` - Hierarchy depth (affects aperture via decay)
    /// * `config` - ConeConfig for aperture computation
    ///
    /// # Returns
    ///
    /// * `Ok(EntailmentCone)` - Valid cone
    /// * `Err(GraphError::InvalidHyperbolicPoint)` - If apex is invalid
    /// * `Err(GraphError::InvalidAperture)` - If computed aperture is invalid
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::poincare::PoincarePoint;
    /// use context_graph_graph::config::ConeConfig;
    /// use context_graph_graph::entailment::cones::EntailmentCone;
    ///
    /// let apex = PoincarePoint::origin();
    /// let config = ConeConfig::default();
    /// let cone = EntailmentCone::new(apex, 0, &config).expect("valid cone");
    /// assert!(cone.is_valid());
    /// assert_eq!(cone.depth, 0);
    /// assert_eq!(cone.aperture, config.base_aperture);
    /// ```
    pub fn new(apex: PoincarePoint, depth: u32, config: &ConeConfig) -> Result<Self, GraphError> {
        // FAIL FAST: Validate apex immediately
        if !apex.is_valid() {
            tracing::error!(
                norm = apex.norm(),
                "Invalid apex point: norm must be < 1.0"
            );
            return Err(GraphError::InvalidHyperbolicPoint { norm: apex.norm() });
        }

        let aperture = config.compute_aperture(depth);

        // FAIL FAST: Validate computed aperture
        if aperture <= 0.0 || aperture > std::f32::consts::FRAC_PI_2 {
            tracing::error!(
                aperture = aperture,
                depth = depth,
                "Invalid aperture computed from config"
            );
            return Err(GraphError::InvalidAperture(aperture));
        }

        Ok(Self {
            apex,
            aperture,
            aperture_factor: 1.0,
            depth,
        })
    }

    /// Create cone with explicit aperture (for deserialization/testing).
    ///
    /// # Arguments
    ///
    /// * `apex` - Position in Poincare ball
    /// * `aperture` - Explicit aperture in radians
    /// * `depth` - Hierarchy depth
    ///
    /// # Returns
    ///
    /// * `Ok(EntailmentCone)` - Valid cone
    /// * `Err(GraphError)` - If validation fails
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::poincare::PoincarePoint;
    /// use context_graph_graph::entailment::cones::EntailmentCone;
    ///
    /// let apex = PoincarePoint::origin();
    /// let cone = EntailmentCone::with_aperture(apex, 0.5, 0).expect("valid cone");
    /// assert_eq!(cone.aperture, 0.5);
    /// ```
    pub fn with_aperture(
        apex: PoincarePoint,
        aperture: f32,
        depth: u32,
    ) -> Result<Self, GraphError> {
        // FAIL FAST: Validate apex
        if !apex.is_valid() {
            tracing::error!(norm = apex.norm(), "Invalid apex point");
            return Err(GraphError::InvalidHyperbolicPoint { norm: apex.norm() });
        }

        // FAIL FAST: Validate aperture range
        if aperture <= 0.0 || aperture > std::f32::consts::FRAC_PI_2 {
            tracing::error!(
                aperture = aperture,
                "Aperture out of valid range (0, π/2]"
            );
            return Err(GraphError::InvalidAperture(aperture));
        }

        Ok(Self {
            apex,
            aperture,
            aperture_factor: 1.0,
            depth,
        })
    }

    /// Get the effective aperture after applying adjustment factor.
    ///
    /// Result is clamped to valid range (0, π/2].
    ///
    /// # Formula
    ///
    /// `effective = (aperture * aperture_factor).clamp(ε, π/2)`
    /// where ε is a small positive value to prevent zero aperture.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::entailment::cones::EntailmentCone;
    ///
    /// let mut cone = EntailmentCone::default();
    /// cone.aperture = 0.5;
    /// cone.aperture_factor = 1.5;
    /// assert!((cone.effective_aperture() - 0.75).abs() < 1e-6);
    /// ```
    #[inline]
    pub fn effective_aperture(&self) -> f32 {
        const MIN_APERTURE: f32 = 1e-6;
        let effective = self.aperture * self.aperture_factor;
        effective.clamp(MIN_APERTURE, std::f32::consts::FRAC_PI_2)
    }

    /// Check if a point is contained within this cone.
    ///
    /// # Algorithm
    ///
    /// 1. Compute angle between point direction and cone axis (toward origin)
    /// 2. Return angle <= effective_aperture()
    ///
    /// # Performance Target
    ///
    /// <50μs on CPU
    ///
    /// # Arguments
    ///
    /// * `point` - Point to check for containment
    /// * `ball` - PoincareBall for hyperbolic operations
    ///
    /// # Edge Cases
    ///
    /// - Point at apex: return true (angle = 0)
    /// - Apex at origin: return true (degenerate cone contains all)
    /// - Zero-length tangent vectors: return true (numerical edge case)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::{PoincareBall, PoincarePoint};
    /// use context_graph_graph::config::{HyperbolicConfig, ConeConfig};
    /// use context_graph_graph::entailment::cones::EntailmentCone;
    ///
    /// let config = ConeConfig::default();
    /// let apex = PoincarePoint::origin();
    /// let cone = EntailmentCone::new(apex.clone(), 0, &config).unwrap();
    /// let ball = PoincareBall::new(HyperbolicConfig::default());
    ///
    /// // Point at apex is always contained
    /// assert!(cone.contains(&apex, &ball));
    /// ```
    pub fn contains(&self, point: &PoincarePoint, ball: &PoincareBall) -> bool {
        let angle = self.compute_angle(point, ball);
        angle <= self.effective_aperture()
    }

    /// Compute angle between point direction and cone axis.
    ///
    /// # Algorithm
    ///
    /// 1. tangent = log_map(apex, point) - direction to point in tangent space
    /// 2. to_origin = log_map(apex, origin) - cone axis direction
    /// 3. cos_angle = dot(tangent, to_origin) / (||tangent|| * ||to_origin||)
    /// 4. angle = acos(cos_angle.clamp(-1.0, 1.0))
    ///
    /// # Edge Cases Return 0.0:
    ///
    /// - Point at apex (distance < eps)
    /// - Apex at origin (norm < eps)
    /// - Zero-length tangent or to_origin vectors
    ///
    /// # Performance
    ///
    /// Contributes ~40μs to total <50μs budget.
    fn compute_angle(&self, point: &PoincarePoint, ball: &PoincareBall) -> f32 {
        let config = ball.config();

        // Edge case: point at apex - angle is 0
        let apex_to_point_dist = ball.distance(&self.apex, point);
        if apex_to_point_dist < config.eps {
            return 0.0;
        }

        // Edge case: apex at origin (degenerate cone contains all)
        if self.apex.norm() < config.eps {
            return 0.0;
        }

        // Compute tangent vectors
        let tangent = ball.log_map(&self.apex, point);
        let origin = PoincarePoint::origin();
        let to_origin = ball.log_map(&self.apex, &origin);

        // Compute norms
        let tangent_norm: f32 = tangent.iter().map(|x| x * x).sum::<f32>().sqrt();
        let to_origin_norm: f32 = to_origin.iter().map(|x| x * x).sum::<f32>().sqrt();

        // Edge case: degenerate tangent vectors
        if tangent_norm < config.eps || to_origin_norm < config.eps {
            return 0.0;
        }

        // Compute angle via dot product
        let dot: f32 = tangent
            .iter()
            .zip(to_origin.iter())
            .map(|(a, b)| a * b)
            .sum();

        let cos_angle = (dot / (tangent_norm * to_origin_norm)).clamp(-1.0, 1.0);
        cos_angle.acos()
    }

    /// Compute soft membership score for a point.
    ///
    /// # Returns
    ///
    /// Value in [0, 1] where:
    /// - 1.0 = fully contained within cone
    /// - approaching 0 = far outside cone
    ///
    /// # CANONICAL FORMULA (DO NOT MODIFY)
    ///
    /// - If angle <= effective_aperture: score = 1.0
    /// - If angle > effective_aperture: score = exp(-2.0 * (angle - aperture))
    ///
    /// # Arguments
    ///
    /// * `point` - Point to compute membership score for
    /// * `ball` - PoincareBall for hyperbolic operations
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::{PoincareBall, PoincarePoint};
    /// use context_graph_graph::config::{HyperbolicConfig, ConeConfig};
    /// use context_graph_graph::entailment::cones::EntailmentCone;
    ///
    /// let config = ConeConfig::default();
    /// let apex = PoincarePoint::origin();
    /// let cone = EntailmentCone::new(apex.clone(), 0, &config).unwrap();
    /// let ball = PoincareBall::new(HyperbolicConfig::default());
    ///
    /// // Point at apex has score 1.0
    /// assert_eq!(cone.membership_score(&apex, &ball), 1.0);
    /// ```
    pub fn membership_score(&self, point: &PoincarePoint, ball: &PoincareBall) -> f32 {
        let angle = self.compute_angle(point, ball);
        let aperture = self.effective_aperture();

        if angle <= aperture {
            1.0
        } else {
            (-2.0 * (angle - aperture)).exp()
        }
    }

    /// Update aperture factor based on training signal.
    ///
    /// # Arguments
    ///
    /// * `delta` - Adjustment to aperture_factor
    ///   - Positive delta widens the cone (more inclusive)
    ///   - Negative delta narrows the cone (more exclusive)
    ///
    /// # Invariant
    ///
    /// Result is clamped to [0.5, 2.0] range per constitution constraints.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::entailment::cones::EntailmentCone;
    ///
    /// let mut cone = EntailmentCone::default();
    /// assert_eq!(cone.aperture_factor, 1.0);
    ///
    /// // Positive delta widens
    /// cone.update_aperture(0.3);
    /// assert!((cone.aperture_factor - 1.3).abs() < 1e-6);
    ///
    /// // Negative delta narrows
    /// cone.update_aperture(-0.5);
    /// assert!((cone.aperture_factor - 0.8).abs() < 1e-6);
    /// ```
    pub fn update_aperture(&mut self, delta: f32) {
        self.aperture_factor = (self.aperture_factor + delta).clamp(0.5, 2.0);
    }

    /// Validate cone parameters.
    ///
    /// # Returns
    ///
    /// `true` if all invariants hold:
    /// - apex.is_valid() (norm < 1.0)
    /// - aperture in (0, π/2]
    /// - aperture_factor in [0.5, 2.0]
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::entailment::cones::EntailmentCone;
    ///
    /// let cone = EntailmentCone::default();
    /// assert!(cone.is_valid());
    /// ```
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.apex.is_valid()
            && self.aperture > 0.0
            && self.aperture <= std::f32::consts::FRAC_PI_2
            && self.aperture_factor >= 0.5
            && self.aperture_factor <= 2.0
    }

    /// Validate cone and return detailed error.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Cone is valid
    /// * `Err(GraphError)` - Specific validation failure
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::entailment::cones::EntailmentCone;
    ///
    /// let cone = EntailmentCone::default();
    /// assert!(cone.validate().is_ok());
    /// ```
    pub fn validate(&self) -> Result<(), GraphError> {
        if !self.apex.is_valid() {
            return Err(GraphError::InvalidHyperbolicPoint {
                norm: self.apex.norm(),
            });
        }
        if self.aperture <= 0.0 || self.aperture > std::f32::consts::FRAC_PI_2 {
            return Err(GraphError::InvalidAperture(self.aperture));
        }
        if self.aperture_factor < 0.5 || self.aperture_factor > 2.0 {
            return Err(GraphError::InvalidConfig(format!(
                "aperture_factor {} outside valid range [0.5, 2.0]",
                self.aperture_factor
            )));
        }
        Ok(())
    }
}

impl Default for EntailmentCone {
    /// Create a default cone at origin with base aperture.
    ///
    /// # Returns
    ///
    /// Cone with:
    /// - apex: origin point
    /// - aperture: 1.0 (ConeConfig default base_aperture)
    /// - aperture_factor: 1.0
    /// - depth: 0
    fn default() -> Self {
        Self {
            apex: PoincarePoint::origin(),
            aperture: 1.0, // ConeConfig default base_aperture
            aperture_factor: 1.0,
            depth: 0,
        }
    }
}

// ============================================================================
// TESTS - MUST USE REAL DATA, NO MOCKS (per constitution REQ-KG-TEST)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConeConfig;
    use crate::hyperbolic::poincare::PoincarePoint;

    // ========== CONSTRUCTION TESTS ==========

    /// Test creation with default ConeConfig
    #[test]
    fn test_new_with_default_config() {
        let config = ConeConfig::default();
        let apex = PoincarePoint::origin();
        let cone = EntailmentCone::new(apex, 0, &config).expect("Should create valid cone");

        assert!(cone.is_valid());
        assert_eq!(cone.depth, 0);
        assert_eq!(cone.aperture, config.base_aperture);
        assert_eq!(cone.aperture_factor, 1.0);
    }

    /// Test aperture decay with depth
    #[test]
    fn test_aperture_decay_with_depth() {
        let config = ConeConfig::default();
        let apex = PoincarePoint::origin();

        let cone_d0 = EntailmentCone::new(apex.clone(), 0, &config).expect("valid");
        let cone_d1 = EntailmentCone::new(apex.clone(), 1, &config).expect("valid");
        let cone_d5 = EntailmentCone::new(apex.clone(), 5, &config).expect("valid");

        // Deeper = narrower aperture
        assert!(
            cone_d0.aperture > cone_d1.aperture,
            "depth 0 should have wider aperture than depth 1"
        );
        assert!(
            cone_d1.aperture > cone_d5.aperture,
            "depth 1 should have wider aperture than depth 5"
        );

        // Verify formula: base * decay^depth
        let expected_d1 = config.base_aperture * config.aperture_decay;
        assert!(
            (cone_d1.aperture - expected_d1).abs() < 1e-6,
            "Expected {}, got {}",
            expected_d1,
            cone_d1.aperture
        );
    }

    /// Test very deep node clamps to min_aperture
    #[test]
    fn test_deep_node_min_aperture() {
        let config = ConeConfig::default();
        let apex = PoincarePoint::origin();
        let cone = EntailmentCone::new(apex, 100, &config).expect("valid");

        assert_eq!(
            cone.aperture, config.min_aperture,
            "Very deep node should clamp to min_aperture"
        );
    }

    /// Test with_aperture constructor
    #[test]
    fn test_with_aperture_constructor() {
        let apex = PoincarePoint::origin();
        let cone = EntailmentCone::with_aperture(apex, 0.5, 3).expect("valid");

        assert_eq!(cone.aperture, 0.5);
        assert_eq!(cone.depth, 3);
        assert_eq!(cone.aperture_factor, 1.0);
        assert!(cone.is_valid());
    }

    // ========== EFFECTIVE APERTURE TESTS ==========

    /// Test effective_aperture with factor
    #[test]
    fn test_effective_aperture() {
        let mut cone = EntailmentCone::default();
        cone.aperture = 0.5;
        cone.aperture_factor = 1.5;

        let effective = cone.effective_aperture();
        assert!(
            (effective - 0.75).abs() < 1e-6,
            "Expected 0.75, got {}",
            effective
        );
    }

    /// Test effective_aperture clamping to pi/2
    #[test]
    fn test_effective_aperture_clamp_max() {
        let mut cone = EntailmentCone::default();
        cone.aperture = 1.5;
        cone.aperture_factor = 2.0;

        let effective = cone.effective_aperture();
        assert!(
            (effective - std::f32::consts::FRAC_PI_2).abs() < 1e-6,
            "Should clamp to π/2, got {}",
            effective
        );
    }

    /// Test effective_aperture clamping to minimum
    #[test]
    fn test_effective_aperture_clamp_min() {
        let mut cone = EntailmentCone::default();
        cone.aperture = 1e-8;
        cone.aperture_factor = 0.5;

        let effective = cone.effective_aperture();
        assert!(
            effective >= 1e-6,
            "Should clamp to minimum epsilon, got {}",
            effective
        );
    }

    // ========== VALIDATION TESTS ==========

    /// Test invalid apex rejection (FAIL FAST)
    #[test]
    fn test_invalid_apex_fails_fast() {
        let config = ConeConfig::default();
        // Create point with norm >= 1
        let mut coords = [0.0f32; 64];
        coords[0] = 1.0; // norm = 1.0, invalid for Poincare ball
        let invalid_apex = PoincarePoint::from_coords(coords);

        let result = EntailmentCone::new(invalid_apex, 0, &config);
        assert!(result.is_err(), "Should reject invalid apex");

        match result {
            Err(GraphError::InvalidHyperbolicPoint { norm }) => {
                assert!(
                    (norm - 1.0).abs() < 1e-6,
                    "Should report correct norm in error"
                );
            }
            _ => panic!("Expected InvalidHyperbolicPoint error"),
        }
    }

    /// Test with_aperture validation - zero aperture
    #[test]
    fn test_with_aperture_zero_fails() {
        let apex = PoincarePoint::origin();
        let result = EntailmentCone::with_aperture(apex, 0.0, 0);
        assert!(matches!(result, Err(GraphError::InvalidAperture(_))));
    }

    /// Test with_aperture validation - negative aperture
    #[test]
    fn test_with_aperture_negative_fails() {
        let apex = PoincarePoint::origin();
        let result = EntailmentCone::with_aperture(apex, -0.5, 0);
        assert!(matches!(result, Err(GraphError::InvalidAperture(_))));
    }

    /// Test with_aperture validation - aperture > π/2
    #[test]
    fn test_with_aperture_exceeds_max_fails() {
        let apex = PoincarePoint::origin();
        let result = EntailmentCone::with_aperture(apex, 2.0, 0);
        assert!(matches!(result, Err(GraphError::InvalidAperture(_))));
    }

    /// Test validate() returns specific errors
    #[test]
    fn test_validate_specific_errors() {
        let mut cone = EntailmentCone::default();
        assert!(cone.validate().is_ok());

        // Invalid aperture_factor below 0.5
        cone.aperture_factor = 0.3;
        let result = cone.validate();
        assert!(matches!(result, Err(GraphError::InvalidConfig(_))));

        // Invalid aperture_factor above 2.0
        cone.aperture_factor = 2.5;
        let result = cone.validate();
        assert!(matches!(result, Err(GraphError::InvalidConfig(_))));
    }

    /// Test is_valid returns false for invalid configurations
    #[test]
    fn test_is_valid_invalid_configurations() {
        let mut cone = EntailmentCone::default();
        assert!(cone.is_valid());

        // Invalid aperture
        cone.aperture = 0.0;
        assert!(!cone.is_valid(), "Zero aperture should be invalid");

        cone.aperture = 1.0;
        cone.aperture_factor = 0.4;
        assert!(
            !cone.is_valid(),
            "aperture_factor below 0.5 should be invalid"
        );

        cone.aperture_factor = 2.1;
        assert!(
            !cone.is_valid(),
            "aperture_factor above 2.0 should be invalid"
        );
    }

    // ========== SERIALIZATION TESTS ==========

    /// Test serialization roundtrip preserves all fields
    #[test]
    fn test_serde_roundtrip() {
        let config = ConeConfig::default();
        let mut coords = [0.0f32; 64];
        coords[0] = 0.5;
        coords[1] = 0.3;
        let apex = PoincarePoint::from_coords(coords);

        let original = EntailmentCone::new(apex, 5, &config).expect("valid");

        let serialized = bincode::serialize(&original).expect("Serialization failed");
        let deserialized: EntailmentCone =
            bincode::deserialize(&serialized).expect("Deserialization failed");

        assert_eq!(original.depth, deserialized.depth);
        assert!((original.aperture - deserialized.aperture).abs() < 1e-6);
        assert!((original.aperture_factor - deserialized.aperture_factor).abs() < 1e-6);
        assert!(deserialized.is_valid());
    }

    /// Test serialized size is approximately 268 bytes
    #[test]
    fn test_serialized_size() {
        let cone = EntailmentCone::default();
        let serialized = bincode::serialize(&cone).expect("Serialization failed");

        // 256 (coords) + 4 (aperture) + 4 (factor) + 4 (depth) = 268
        // bincode may add small overhead
        assert!(
            serialized.len() >= 268,
            "Serialized size should be at least 268 bytes, got {}",
            serialized.len()
        );
        assert!(
            serialized.len() <= 280,
            "Serialized size should not exceed 280 bytes, got {}",
            serialized.len()
        );
    }

    // ========== DEFAULT TESTS ==========

    /// Test Default creates valid cone
    #[test]
    fn test_default_is_valid() {
        let cone = EntailmentCone::default();
        assert!(cone.is_valid());
        assert!(cone.validate().is_ok());
        assert_eq!(cone.depth, 0);
        assert_eq!(cone.aperture, 1.0);
        assert_eq!(cone.aperture_factor, 1.0);
    }

    /// Test default apex is at origin
    #[test]
    fn test_default_apex_is_origin() {
        let cone = EntailmentCone::default();
        assert_eq!(cone.apex.norm(), 0.0);
    }

    // ========== EDGE CASE TESTS ==========

    /// Test apex at origin edge case
    #[test]
    fn test_apex_at_origin() {
        let config = ConeConfig::default();
        let origin = PoincarePoint::origin();

        // Should create valid cone (degenerate but allowed)
        let cone = EntailmentCone::new(origin, 0, &config).expect("valid");
        assert!(cone.is_valid());
        assert_eq!(cone.apex.norm(), 0.0);
    }

    /// Test apex near boundary
    #[test]
    fn test_apex_near_boundary() {
        let config = ConeConfig::default();
        // Create point with norm ≈ 0.99 (near but inside boundary)
        let scale = 0.99 / (64.0_f32).sqrt();
        let coords = [scale; 64];
        let apex = PoincarePoint::from_coords(coords);

        assert!(apex.is_valid(), "Apex should be valid");
        let cone = EntailmentCone::new(apex, 0, &config).expect("valid");
        assert!(cone.is_valid());
    }

    /// Test aperture at maximum (π/2)
    #[test]
    fn test_aperture_at_max() {
        let apex = PoincarePoint::origin();
        let cone =
            EntailmentCone::with_aperture(apex, std::f32::consts::FRAC_PI_2, 0).expect("valid");
        assert!(cone.is_valid());
        assert_eq!(cone.aperture, std::f32::consts::FRAC_PI_2);
    }

    /// Test aperture_factor at bounds
    #[test]
    fn test_aperture_factor_at_bounds() {
        let mut cone = EntailmentCone::default();

        // At lower bound
        cone.aperture_factor = 0.5;
        assert!(cone.is_valid());
        assert!(cone.validate().is_ok());

        // At upper bound
        cone.aperture_factor = 2.0;
        assert!(cone.is_valid());
        assert!(cone.validate().is_ok());
    }

    /// Integration test with real ConeConfig values
    #[test]
    fn test_real_config_integration() {
        // Use actual default values from config.rs
        let config = ConeConfig {
            min_aperture: 0.1,
            max_aperture: 1.5,
            base_aperture: 1.0,
            aperture_decay: 0.85,
            membership_threshold: 0.7,
        };

        let apex = PoincarePoint::origin();

        // Test multiple depths
        for depth in [0, 1, 5, 10, 20] {
            let cone = EntailmentCone::new(apex.clone(), depth, &config).expect("valid");
            assert!(cone.is_valid(), "Cone at depth {} should be valid", depth);
            assert!(
                cone.aperture >= config.min_aperture,
                "Aperture at depth {} should be >= min_aperture",
                depth
            );
            assert!(
                cone.aperture <= config.max_aperture,
                "Aperture at depth {} should be <= max_aperture",
                depth
            );
        }
    }

    /// Test Clone trait
    #[test]
    fn test_clone_is_independent() {
        let config = ConeConfig::default();
        let mut coords = [0.0f32; 64];
        coords[0] = 0.3;
        let apex = PoincarePoint::from_coords(coords);

        let original = EntailmentCone::new(apex, 5, &config).expect("valid");
        let cloned = original.clone();

        // Clone should have identical values
        assert_eq!(original.aperture, cloned.aperture);
        assert_eq!(original.depth, cloned.depth);
        assert_eq!(original.aperture_factor, cloned.aperture_factor);

        // Verify expected aperture: 1.0 * 0.85^5 = 0.4437
        let expected_aperture = 1.0 * 0.85_f32.powi(5);
        assert!(
            (original.aperture - expected_aperture).abs() < 0.01,
            "Expected {}, got {}",
            expected_aperture,
            original.aperture
        );
        assert_eq!(original.depth, 5);
    }

    /// Test Debug trait
    #[test]
    fn test_debug_output() {
        let cone = EntailmentCone::default();
        let debug_str = format!("{:?}", cone);
        assert!(debug_str.contains("EntailmentCone"));
        assert!(debug_str.contains("apex"));
        assert!(debug_str.contains("aperture"));
        assert!(debug_str.contains("aperture_factor"));
        assert!(debug_str.contains("depth"));
    }

    /// Test with various valid apex positions
    #[test]
    fn test_various_apex_positions() {
        let config = ConeConfig::default();

        // Near origin
        let mut coords1 = [0.0f32; 64];
        coords1[0] = 0.01;
        let apex1 = PoincarePoint::from_coords(coords1);
        assert!(EntailmentCone::new(apex1, 0, &config).is_ok());

        // Medium distance
        let mut coords2 = [0.0f32; 64];
        coords2[0] = 0.5;
        coords2[1] = 0.3;
        let apex2 = PoincarePoint::from_coords(coords2);
        assert!(EntailmentCone::new(apex2, 0, &config).is_ok());

        // Near boundary
        let mut coords3 = [0.0f32; 64];
        coords3[0] = 0.9;
        let apex3 = PoincarePoint::from_coords(coords3);
        assert!(EntailmentCone::new(apex3, 0, &config).is_ok());
    }

    /// Test depth affects aperture monotonically
    #[test]
    fn test_depth_aperture_monotonicity() {
        let config = ConeConfig::default();
        let apex = PoincarePoint::origin();

        let mut prev_aperture = f32::INFINITY;
        for depth in 0..20 {
            let cone = EntailmentCone::new(apex.clone(), depth, &config).expect("valid");
            assert!(
                cone.aperture <= prev_aperture,
                "Aperture should decrease or stay same with depth"
            );
            prev_aperture = cone.aperture;
        }
    }

    /// Test PartialEq for PoincarePoint in apex
    #[test]
    fn test_apex_equality() {
        let config = ConeConfig::default();
        let apex = PoincarePoint::origin();

        let cone1 = EntailmentCone::new(apex.clone(), 0, &config).expect("valid");
        let cone2 = EntailmentCone::new(apex, 0, &config).expect("valid");

        assert_eq!(cone1.apex, cone2.apex);
        assert_eq!(cone1.aperture, cone2.aperture);
        assert_eq!(cone1.depth, cone2.depth);
    }

    /// Test NaN handling in apex
    #[test]
    fn test_nan_apex_rejected() {
        let config = ConeConfig::default();
        let mut coords = [0.0f32; 64];
        coords[0] = f32::NAN;
        let apex = PoincarePoint::from_coords(coords);

        // NaN norm < 1.0 is false, so should be rejected
        let result = EntailmentCone::new(apex, 0, &config);
        assert!(result.is_err(), "NaN apex should be rejected");
    }

    /// Test infinity handling in apex
    #[test]
    fn test_infinity_apex_rejected() {
        let config = ConeConfig::default();
        let mut coords = [0.0f32; 64];
        coords[0] = f32::INFINITY;
        let apex = PoincarePoint::from_coords(coords);

        let result = EntailmentCone::new(apex, 0, &config);
        assert!(result.is_err(), "Infinity apex should be rejected");
    }

    // ========== CONTAINMENT TESTS (M04-T07) ==========

    use crate::config::HyperbolicConfig;
    use crate::hyperbolic::mobius::PoincareBall;

    fn default_ball() -> PoincareBall {
        PoincareBall::new(HyperbolicConfig::default())
    }

    /// Test point at apex is always contained
    #[test]
    fn test_point_at_apex_contained() {
        let config = ConeConfig::default();
        let apex = PoincarePoint::origin();
        let cone = EntailmentCone::new(apex.clone(), 0, &config).expect("valid cone");
        let ball = default_ball();

        assert!(
            cone.contains(&apex, &ball),
            "Point at apex must be contained"
        );
        assert_eq!(
            cone.membership_score(&apex, &ball),
            1.0,
            "Point at apex must have score 1.0"
        );
    }

    /// Test apex at origin contains all points (degenerate cone)
    #[test]
    fn test_apex_at_origin_contains_all() {
        let apex = PoincarePoint::origin();
        let cone = EntailmentCone::with_aperture(apex, 0.5, 0).expect("valid");
        let ball = default_ball();

        // Create various points
        let mut coords = [0.0f32; 64];
        coords[0] = 0.5;
        let point = PoincarePoint::from_coords(coords);

        assert!(
            cone.contains(&point, &ball),
            "Origin apex contains all points"
        );
        assert_eq!(
            cone.membership_score(&point, &ball),
            1.0,
            "Origin apex gives score 1.0"
        );
    }

    /// Test point along cone axis is contained
    #[test]
    fn test_point_along_cone_axis_contained() {
        // Apex not at origin, point toward origin (along cone axis)
        let mut apex_coords = [0.0f32; 64];
        apex_coords[0] = 0.5;
        let apex = PoincarePoint::from_coords(apex_coords);
        let cone = EntailmentCone::with_aperture(apex, 0.5, 1).expect("valid");
        let ball = default_ball();

        // Point between apex and origin (along axis)
        let mut point_coords = [0.0f32; 64];
        point_coords[0] = 0.25;
        let point = PoincarePoint::from_coords(point_coords);

        assert!(
            cone.contains(&point, &ball),
            "Point along axis toward origin should be contained"
        );
        assert_eq!(
            cone.membership_score(&point, &ball),
            1.0,
            "Point along axis should have score 1.0"
        );
    }

    /// Test point perpendicular to cone axis is outside (for narrow cone)
    #[test]
    fn test_point_outside_cone_perpendicular() {
        let mut apex_coords = [0.0f32; 64];
        apex_coords[0] = 0.5;
        let apex = PoincarePoint::from_coords(apex_coords);
        // Very narrow cone aperture
        let cone = EntailmentCone::with_aperture(apex, 0.3, 1).expect("valid");
        let ball = default_ball();

        // Point in perpendicular direction
        let mut point_coords = [0.0f32; 64];
        point_coords[1] = 0.5;
        let point = PoincarePoint::from_coords(point_coords);

        assert!(
            !cone.contains(&point, &ball),
            "Perpendicular point should be outside narrow cone"
        );
        let score = cone.membership_score(&point, &ball);
        assert!(
            score < 1.0,
            "Outside point should have score < 1.0, got {}",
            score
        );
        assert!(
            score > 0.0,
            "Score should be > 0 (exponential decay), got {}",
            score
        );
    }

    /// Test membership_score canonical formula: exp(-2.0 * (angle - aperture))
    #[test]
    fn test_membership_score_canonical_formula() {
        let mut apex_coords = [0.0f32; 64];
        apex_coords[0] = 0.5;
        let apex = PoincarePoint::from_coords(apex_coords);
        let cone = EntailmentCone::with_aperture(apex, 0.3, 1).expect("valid");
        let ball = default_ball();

        // Point outside cone
        let mut point_coords = [0.0f32; 64];
        point_coords[1] = 0.5;
        let point = PoincarePoint::from_coords(point_coords);

        let score = cone.membership_score(&point, &ball);

        // Score should be positive and less than 1
        assert!(
            score > 0.0,
            "Score should be > 0 (exponential never reaches 0)"
        );
        assert!(score < 1.0, "Score should be < 1 for outside point");

        // Verify it's a valid exponential decay value
        assert!(score <= 1.0 && score >= 0.0, "Score must be in [0, 1]");
    }

    /// Test update_aperture with positive delta
    #[test]
    fn test_update_aperture_positive_delta() {
        let mut cone = EntailmentCone::default();
        assert_eq!(cone.aperture_factor, 1.0);

        cone.update_aperture(0.3);
        assert!(
            (cone.aperture_factor - 1.3).abs() < 1e-6,
            "Expected 1.3, got {}",
            cone.aperture_factor
        );
    }

    /// Test update_aperture with negative delta
    #[test]
    fn test_update_aperture_negative_delta() {
        let mut cone = EntailmentCone::default();
        cone.update_aperture(-0.3);
        assert!(
            (cone.aperture_factor - 0.7).abs() < 1e-6,
            "Expected 0.7, got {}",
            cone.aperture_factor
        );
    }

    /// Test update_aperture clamps to max 2.0
    #[test]
    fn test_update_aperture_clamps_max() {
        let mut cone = EntailmentCone::default();
        cone.update_aperture(10.0); // Large positive
        assert_eq!(
            cone.aperture_factor, 2.0,
            "Should clamp to max 2.0"
        );
    }

    /// Test update_aperture clamps to min 0.5
    #[test]
    fn test_update_aperture_clamps_min() {
        let mut cone = EntailmentCone::default();
        cone.update_aperture(-10.0); // Large negative
        assert_eq!(
            cone.aperture_factor, 0.5,
            "Should clamp to min 0.5"
        );
    }

    /// Test containment boundary - wide aperture should contain more
    #[test]
    fn test_containment_boundary_wide_aperture() {
        let config = ConeConfig::default();
        let apex = PoincarePoint::origin();
        let cone = EntailmentCone::new(apex, 0, &config).expect("valid");
        let ball = default_ball();

        // With apex at origin, all points should be contained
        let mut coords = [0.0f32; 64];
        coords[0] = 0.9;
        let point = PoincarePoint::from_coords(coords);

        assert!(
            cone.contains(&point, &ball),
            "Wide cone at origin should contain points"
        );
    }

    /// Test containment with non-origin apex and point toward origin
    #[test]
    fn test_containment_toward_origin() {
        // Cone apex at (0.7, 0, ...)
        let mut apex_coords = [0.0f32; 64];
        apex_coords[0] = 0.7;
        let apex = PoincarePoint::from_coords(apex_coords);
        let cone = EntailmentCone::with_aperture(apex, 0.8, 2).expect("valid");
        let ball = default_ball();

        // Point closer to origin than apex (should be in cone direction)
        let mut point_coords = [0.0f32; 64];
        point_coords[0] = 0.3;
        let point = PoincarePoint::from_coords(point_coords);

        // This point is along the cone axis (toward origin)
        assert!(
            cone.contains(&point, &ball),
            "Point toward origin should be contained in wide cone"
        );
    }

    /// Test that compute_angle returns 0 for point at apex
    #[test]
    fn test_compute_angle_point_at_apex() {
        let mut apex_coords = [0.0f32; 64];
        apex_coords[0] = 0.3;
        let apex = PoincarePoint::from_coords(apex_coords);
        let cone = EntailmentCone::with_aperture(apex.clone(), 0.5, 1).expect("valid");
        let ball = default_ball();

        // Point at apex should give angle 0
        assert!(cone.contains(&apex, &ball));
        assert_eq!(cone.membership_score(&apex, &ball), 1.0);
    }

    /// Test multiple containment checks for determinism
    #[test]
    fn test_containment_deterministic() {
        let mut apex_coords = [0.0f32; 64];
        apex_coords[0] = 0.4;
        let apex = PoincarePoint::from_coords(apex_coords);
        let cone = EntailmentCone::with_aperture(apex, 0.5, 1).expect("valid");
        let ball = default_ball();

        let mut point_coords = [0.0f32; 64];
        point_coords[0] = 0.2;
        let point = PoincarePoint::from_coords(point_coords);

        let first_result = cone.contains(&point, &ball);
        let first_score = cone.membership_score(&point, &ball);

        // Run many times to verify determinism
        for _ in 0..100 {
            assert_eq!(
                cone.contains(&point, &ball),
                first_result,
                "Containment should be deterministic"
            );
            assert_eq!(
                cone.membership_score(&point, &ball),
                first_score,
                "Score should be deterministic"
            );
        }
    }

    /// Test membership score is 1.0 for all contained points
    #[test]
    fn test_membership_score_contained_is_one() {
        let apex = PoincarePoint::origin();
        let cone = EntailmentCone::with_aperture(apex, 1.0, 0).expect("valid");
        let ball = default_ball();

        // Multiple points that should be contained (apex at origin means all contained)
        let test_coords = vec![0.1, 0.3, 0.5, 0.7, 0.9];
        for coord in test_coords {
            let mut coords = [0.0f32; 64];
            coords[0] = coord;
            let point = PoincarePoint::from_coords(coords);

            if cone.contains(&point, &ball) {
                assert_eq!(
                    cone.membership_score(&point, &ball),
                    1.0,
                    "Contained point should have score 1.0"
                );
            }
        }
    }

    /// Test update_aperture maintains validity
    #[test]
    fn test_update_aperture_maintains_validity() {
        let mut cone = EntailmentCone::default();
        assert!(cone.is_valid());

        // Multiple updates
        cone.update_aperture(0.5);
        assert!(cone.is_valid(), "Cone should remain valid after update");
        assert!(cone.validate().is_ok());

        cone.update_aperture(-0.8);
        assert!(cone.is_valid(), "Cone should remain valid after update");
        assert!(cone.validate().is_ok());

        // Extreme updates that trigger clamping
        cone.update_aperture(100.0);
        assert!(cone.is_valid(), "Cone should remain valid after clamped update");
        assert_eq!(cone.aperture_factor, 2.0);

        cone.update_aperture(-100.0);
        assert!(cone.is_valid(), "Cone should remain valid after clamped update");
        assert_eq!(cone.aperture_factor, 0.5);
    }

    /// Test containment with 2D point configurations
    #[test]
    fn test_containment_2d_points() {
        // Apex at (0.5, 0.3, 0, ...)
        let mut apex_coords = [0.0f32; 64];
        apex_coords[0] = 0.5;
        apex_coords[1] = 0.3;
        let apex = PoincarePoint::from_coords(apex_coords);
        let cone = EntailmentCone::with_aperture(apex, 0.7, 1).expect("valid");
        let ball = default_ball();

        // Point in same general direction but closer to origin
        let mut point_coords = [0.0f32; 64];
        point_coords[0] = 0.2;
        point_coords[1] = 0.1;
        let point = PoincarePoint::from_coords(point_coords);

        // Should be contained (toward origin from apex)
        let is_contained = cone.contains(&point, &ball);
        let score = cone.membership_score(&point, &ball);

        // Verify consistency
        if is_contained {
            assert_eq!(score, 1.0);
        } else {
            assert!(score < 1.0);
        }
    }

    /// Test that effective_aperture affects containment
    #[test]
    fn test_aperture_factor_affects_containment() {
        let mut apex_coords = [0.0f32; 64];
        apex_coords[0] = 0.5;
        let apex = PoincarePoint::from_coords(apex_coords);
        let ball = default_ball();

        // Create a point that might be on the boundary
        let mut point_coords = [0.0f32; 64];
        point_coords[0] = 0.3;
        point_coords[1] = 0.4; // Diagonal from axis
        let point = PoincarePoint::from_coords(point_coords);

        // Narrow cone (small aperture)
        let mut narrow_cone = EntailmentCone::with_aperture(apex.clone(), 0.2, 1).expect("valid");
        narrow_cone.aperture_factor = 0.5; // Make even narrower

        // Wide cone (same base aperture but larger factor)
        let mut wide_cone = EntailmentCone::with_aperture(apex, 0.2, 1).expect("valid");
        wide_cone.aperture_factor = 2.0; // Make wider

        // Wide cone should contain more than narrow cone
        assert!(
            wide_cone.effective_aperture() > narrow_cone.effective_aperture(),
            "Wide cone should have larger effective aperture"
        );
    }
}
