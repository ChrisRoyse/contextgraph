//! Constructor methods for EntailmentCone.
//!
//! This module contains the `new` and `with_aperture` constructors.

use crate::config::ConeConfig;
use crate::error::GraphError;
use crate::hyperbolic::poincare::PoincarePoint;

use super::EntailmentCone;

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
            tracing::error!(norm = apex.norm(), "Invalid apex point: norm must be < 1.0");
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
            tracing::error!(aperture = aperture, "Aperture out of valid range (0, π/2]");
            return Err(GraphError::InvalidAperture(aperture));
        }

        Ok(Self {
            apex,
            aperture,
            aperture_factor: 1.0,
            depth,
        })
    }
}
