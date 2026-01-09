//! Configuration for Cone CUDA operations.

use super::constants::DEFAULT_CURVATURE;
use crate::error::{CudaError, CudaResult};

/// Configuration for Cone CUDA operations.
///
/// # Example
///
/// ```
/// use context_graph_cuda::cone::ConeCudaConfig;
///
/// let config = ConeCudaConfig::default();
/// assert!((config.curvature - (-1.0)).abs() < 1e-6);
/// assert!(config.validate().is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct ConeCudaConfig {
    /// Curvature (must be negative, default -1.0).
    pub curvature: f32,
}

impl Default for ConeCudaConfig {
    fn default() -> Self {
        Self {
            curvature: DEFAULT_CURVATURE,
        }
    }
}

impl ConeCudaConfig {
    /// Create config with custom curvature.
    ///
    /// # Errors
    ///
    /// Returns error if curvature is not negative or is NaN.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_cuda::cone::ConeCudaConfig;
    ///
    /// let config = ConeCudaConfig::with_curvature(-0.5).unwrap();
    /// assert!((config.curvature - (-0.5)).abs() < 1e-6);
    ///
    /// // Invalid curvature returns error
    /// assert!(ConeCudaConfig::with_curvature(0.5).is_err());
    /// ```
    pub fn with_curvature(curvature: f32) -> CudaResult<Self> {
        if curvature >= 0.0 {
            return Err(CudaError::InvalidConfig(
                "Cone curvature must be negative".to_string(),
            ));
        }
        if curvature.is_nan() {
            return Err(CudaError::InvalidConfig(
                "Cone curvature cannot be NaN".to_string(),
            ));
        }
        Ok(Self { curvature })
    }

    /// Validate configuration parameters.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Curvature is not negative
    /// - Curvature is NaN
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_cuda::cone::ConeCudaConfig;
    ///
    /// let config = ConeCudaConfig::default();
    /// assert!(config.validate().is_ok());
    /// ```
    pub fn validate(&self) -> CudaResult<()> {
        if self.curvature >= 0.0 {
            return Err(CudaError::InvalidConfig(
                "Cone curvature must be negative".to_string(),
            ));
        }
        if self.curvature.is_nan() {
            return Err(CudaError::InvalidConfig(
                "Cone curvature cannot be NaN".to_string(),
            ));
        }
        Ok(())
    }

    /// Get absolute value of curvature (always positive).
    #[inline]
    pub fn abs_curvature(&self) -> f32 {
        self.curvature.abs()
    }
}
