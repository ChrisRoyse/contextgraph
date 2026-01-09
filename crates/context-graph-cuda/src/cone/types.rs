//! Data types for cone operations.

use super::constants::CONE_DATA_DIM;
use crate::error::{CudaError, CudaResult};

/// Cone data packed for GPU transfer.
///
/// Contains 64 apex coordinates plus 1 aperture value.
///
/// # Example
///
/// ```
/// use context_graph_cuda::cone::ConeData;
///
/// let apex = [0.1f32; 64];
/// let cone = ConeData::new(apex, 0.5).unwrap();
/// assert!((cone.aperture - 0.5).abs() < 1e-6);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ConeData {
    /// Apex point in Poincare ball (64 floats).
    pub apex: [f32; 64],
    /// Effective aperture angle in radians.
    pub aperture: f32,
}

impl ConeData {
    /// Create new cone data.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Apex norm >= 1.0 (outside Poincare ball)
    /// - Aperture is negative
    /// - Aperture is NaN
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_cuda::cone::ConeData;
    ///
    /// let apex = [0.1f32; 64];
    /// let cone = ConeData::new(apex, 0.5).unwrap();
    /// assert!((cone.aperture - 0.5).abs() < 1e-6);
    ///
    /// // Invalid apex (outside ball) returns error
    /// let bad_apex = [1.0f32; 64]; // norm > 1
    /// assert!(ConeData::new(bad_apex, 0.5).is_err());
    /// ```
    pub fn new(apex: [f32; 64], aperture: f32) -> CudaResult<Self> {
        // Validate apex is inside Poincare ball
        let norm_sq: f32 = apex.iter().map(|x| x * x).sum();
        if norm_sq >= 1.0 {
            return Err(CudaError::InvalidConfig(format!(
                "Apex norm {} >= 1.0, must be inside Poincare ball",
                norm_sq.sqrt()
            )));
        }
        if aperture < 0.0 {
            return Err(CudaError::InvalidConfig(
                "Aperture must be non-negative".to_string(),
            ));
        }
        if aperture.is_nan() {
            return Err(CudaError::InvalidConfig(
                "Aperture cannot be NaN".to_string(),
            ));
        }
        Ok(Self { apex, aperture })
    }

    /// Create cone data from raw components (unchecked).
    ///
    /// Use this when you've already validated the data.
    #[inline]
    pub fn from_raw(apex: [f32; 64], aperture: f32) -> Self {
        Self { apex, aperture }
    }

    /// Pack to GPU format [apex_0..apex_63, aperture].
    pub fn to_gpu_format(&self) -> [f32; CONE_DATA_DIM] {
        let mut data = [0.0f32; CONE_DATA_DIM];
        data[..64].copy_from_slice(&self.apex);
        data[64] = self.aperture;
        data
    }

    /// Unpack from GPU format.
    pub fn from_gpu_format(data: &[f32; CONE_DATA_DIM]) -> Self {
        let mut apex = [0.0f32; 64];
        apex.copy_from_slice(&data[..64]);
        Self {
            apex,
            aperture: data[64],
        }
    }
}

/// Information about the CUDA kernel configuration.
///
/// Useful for debugging and performance tuning.
#[derive(Debug, Clone, Copy)]
pub struct ConeKernelInfo {
    /// Block dimension X (warp-aligned, typically 32).
    pub block_dim_x: i32,
    /// Block dimension Y (cones per block, typically 8).
    pub block_dim_y: i32,
    /// Point dimension (must be 64).
    pub point_dim: i32,
    /// Cone data dimension (65 = 64 apex + 1 aperture).
    pub cone_data_dim: i32,
    /// Shared memory per block in bytes.
    pub shared_mem_bytes: i32,
}

// NOTE: get_cone_kernel_info is defined in gpu.rs, not here.
// This avoids duplicate function definitions.
