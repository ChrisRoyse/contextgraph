//! PoincarePoint operations.
//!
//! Contains all methods for creating, manipulating, and validating
//! points in the Poincare ball model.

use super::types::PoincarePoint;
use crate::config::HyperbolicConfig;

impl PoincarePoint {
    /// Creates the origin point (all zeros).
    ///
    /// The origin is the center of the Poincare ball, representing the most
    /// general/root concept in hierarchical embeddings.
    ///
    /// # Returns
    ///
    /// Point with all 64 coordinates set to 0.0
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::PoincarePoint;
    ///
    /// let origin = PoincarePoint::origin();
    /// assert_eq!(origin.norm(), 0.0);
    /// assert!(origin.coords.iter().all(|&x| x == 0.0));
    /// ```
    #[inline]
    pub fn origin() -> Self {
        Self { coords: [0.0; 64] }
    }

    /// Creates a point from coordinates.
    ///
    /// # Warning
    ///
    /// Does NOT validate norm. Call `project()` after if norm may exceed 1.0.
    /// For validated construction, use `from_coords_projected()`.
    ///
    /// # Arguments
    ///
    /// * `coords` - 64-element array of f32 coordinates
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::PoincarePoint;
    ///
    /// let coords = [0.1f32; 64];
    /// let point = PoincarePoint::from_coords(coords);
    /// assert!(point.is_valid()); // norm = sqrt(64 * 0.01) = 0.8
    /// ```
    #[inline]
    pub fn from_coords(coords: [f32; 64]) -> Self {
        Self { coords }
    }

    /// Creates a validated point, projecting if necessary.
    ///
    /// Unlike `from_coords()`, this ensures the resulting point is valid
    /// by projecting to `max_norm` if the input norm is too large.
    ///
    /// # Arguments
    ///
    /// * `coords` - 64-element array of f32 coordinates
    /// * `config` - HyperbolicConfig containing max_norm
    ///
    /// # Returns
    ///
    /// Point guaranteed to have norm < max_norm
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::PoincarePoint;
    /// use context_graph_graph::config::HyperbolicConfig;
    ///
    /// let config = HyperbolicConfig::default();
    /// let coords = [1.0f32; 64]; // norm = 8.0, way outside ball
    /// let point = PoincarePoint::from_coords_projected(coords, &config);
    /// assert!(point.is_valid());
    /// // Floating point tolerance for norm after projection
    /// assert!(point.norm() <= config.max_norm + 1e-6);
    /// ```
    pub fn from_coords_projected(coords: [f32; 64], config: &HyperbolicConfig) -> Self {
        let mut point = Self { coords };
        point.project(config);
        point
    }

    /// Computes squared Euclidean norm of coordinates.
    ///
    /// More efficient than `norm()` when comparing magnitudes since it avoids
    /// the sqrt operation. Use this for validation checks.
    ///
    /// # Returns
    ///
    /// Sum of squared coordinates: sum(coords[i]^2)
    ///
    /// # Performance
    ///
    /// O(64) additions and multiplications. Compiler will auto-vectorize with
    /// SIMD when available.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::PoincarePoint;
    ///
    /// let point = PoincarePoint::origin();
    /// assert_eq!(point.norm_squared(), 0.0);
    ///
    /// let mut coords = [0.0f32; 64];
    /// coords[0] = 0.5;
    /// let point = PoincarePoint::from_coords(coords);
    /// assert_eq!(point.norm_squared(), 0.25);
    /// ```
    #[inline]
    pub fn norm_squared(&self) -> f32 {
        self.coords.iter().map(|&x| x * x).sum()
    }

    /// Computes Euclidean norm of coordinates.
    ///
    /// # Returns
    ///
    /// sqrt(sum(coords[i]^2))
    ///
    /// # Performance
    ///
    /// O(64) with one sqrt at the end. Use `norm_squared()` if you only need
    /// to compare magnitudes.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::PoincarePoint;
    ///
    /// let mut coords = [0.0f32; 64];
    /// coords[0] = 0.6;
    /// coords[1] = 0.8;
    /// let point = PoincarePoint::from_coords(coords);
    /// assert!((point.norm() - 1.0).abs() < 1e-6); // 0.36 + 0.64 = 1.0
    /// ```
    #[inline]
    pub fn norm(&self) -> f32 {
        self.norm_squared().sqrt()
    }

    /// Projects point to stay strictly inside the Poincare ball.
    ///
    /// If `||coords|| >= max_norm`, rescales all coordinates so that
    /// `||coords|| = max_norm`. This prevents numerical instability that
    /// occurs when points approach or exceed the unit ball boundary.
    ///
    /// # Arguments
    ///
    /// * `config` - HyperbolicConfig containing max_norm (default: 0.99999)
    ///
    /// # Algorithm
    ///
    /// 1. Compute current norm
    /// 2. If norm >= max_norm: scale = max_norm / norm
    /// 3. Multiply all coordinates by scale
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::PoincarePoint;
    /// use context_graph_graph::config::HyperbolicConfig;
    ///
    /// let config = HyperbolicConfig::default();
    /// let mut point = PoincarePoint::from_coords([0.2f32; 64]);
    /// // norm = sqrt(64 * 0.04) = 1.6, exceeds max_norm
    /// point.project(&config);
    /// assert!(point.norm() < 1.0);
    /// ```
    pub fn project(&mut self, config: &HyperbolicConfig) {
        let norm = self.norm();
        if norm >= config.max_norm {
            let scale = config.max_norm / norm;
            for c in &mut self.coords {
                *c *= scale;
            }
        }
    }

    /// Creates a projected copy without modifying self.
    ///
    /// # Arguments
    ///
    /// * `config` - HyperbolicConfig containing max_norm
    ///
    /// # Returns
    ///
    /// New PoincarePoint with norm <= max_norm
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::PoincarePoint;
    /// use context_graph_graph::config::HyperbolicConfig;
    ///
    /// let config = HyperbolicConfig::default();
    /// let original = PoincarePoint::from_coords([0.2f32; 64]);
    /// let projected = original.projected(&config);
    /// // Floating point tolerance for norm after projection
    /// assert!(projected.norm() <= config.max_norm + 1e-6);
    /// // original unchanged
    /// assert!((original.norm() - projected.norm()).abs() > 0.5);
    /// ```
    pub fn projected(&self, config: &HyperbolicConfig) -> Self {
        let mut result = self.clone();
        result.project(config);
        result
    }

    /// Checks if point is valid (strictly inside unit ball).
    ///
    /// # Returns
    ///
    /// `true` if `||coords|| < 1.0`, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::PoincarePoint;
    ///
    /// let origin = PoincarePoint::origin();
    /// assert!(origin.is_valid());
    ///
    /// let boundary = PoincarePoint::from_coords([0.125f32; 64]);
    /// // norm = sqrt(64 * 0.015625) = 1.0, ON boundary = invalid
    /// assert!(!boundary.is_valid());
    /// ```
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.norm_squared() < 1.0
    }

    /// Checks if point is valid with given config's max_norm.
    ///
    /// Stricter than `is_valid()` - checks against config's max_norm
    /// rather than 1.0.
    ///
    /// # Arguments
    ///
    /// * `config` - HyperbolicConfig containing max_norm
    ///
    /// # Returns
    ///
    /// `true` if `||coords|| < config.max_norm`
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_graph::hyperbolic::PoincarePoint;
    /// use context_graph_graph::config::HyperbolicConfig;
    ///
    /// let config = HyperbolicConfig::default(); // max_norm = 0.99999
    /// let point = PoincarePoint::from_coords([0.12f32; 64]);
    /// // norm approx 0.96, valid for both
    /// assert!(point.is_valid());
    /// assert!(point.is_valid_for_config(&config));
    /// ```
    #[inline]
    pub fn is_valid_for_config(&self, config: &HyperbolicConfig) -> bool {
        self.norm() < config.max_norm
    }
}
