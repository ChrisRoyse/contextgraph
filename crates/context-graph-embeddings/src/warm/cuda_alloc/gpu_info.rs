//! GPU device information for warm loading operations.
//!
//! Contains hardware properties needed for capacity planning and
//! capability verification during warm model loading.

use super::constants::{GB, MINIMUM_VRAM_BYTES, REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR};

/// GPU device information for warm loading operations.
///
/// Contains hardware properties needed for capacity planning and
/// capability verification during warm model loading.
///
/// # Fields
///
/// - `device_id`: CUDA device ordinal (typically 0 for single-GPU systems)
/// - `name`: Human-readable GPU name from driver
/// - `compute_capability`: (major, minor) tuple for capability checks
/// - `total_memory_bytes`: Total VRAM capacity
/// - `driver_version`: CUDA driver version string
///
/// # Example
///
/// ```rust,ignore
/// let allocator = WarmCudaAllocator::new(0)?;
/// let info = allocator.get_gpu_info()?;
///
/// println!("GPU: {} with {} VRAM", info.name, format_bytes(info.total_memory_bytes));
/// println!("Compute Capability: {}.{}", info.compute_capability.0, info.compute_capability.1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuInfo {
    /// CUDA device ordinal (0-indexed).
    pub device_id: u32,
    /// GPU model name (e.g., "NVIDIA GeForce RTX 5090").
    pub name: String,
    /// Compute capability as (major, minor) version tuple.
    ///
    /// RTX 5090 (Blackwell): (12, 0)
    /// RTX 4090 (Ada Lovelace): (8, 9)
    /// RTX 3090 (Ampere): (8, 6)
    pub compute_capability: (u32, u32),
    /// Total VRAM in bytes.
    pub total_memory_bytes: usize,
    /// CUDA driver version string (e.g., "13.2.0").
    pub driver_version: String,
}

impl GpuInfo {
    /// Create a new GpuInfo with the given parameters.
    #[must_use]
    pub fn new(
        device_id: u32,
        name: String,
        compute_capability: (u32, u32),
        total_memory_bytes: usize,
        driver_version: String,
    ) -> Self {
        Self {
            device_id,
            name,
            compute_capability,
            total_memory_bytes,
            driver_version,
        }
    }

    /// Get the compute capability as a formatted string (e.g., "12.0").
    #[must_use]
    pub fn compute_capability_string(&self) -> String {
        format!(
            "{}.{}",
            self.compute_capability.0, self.compute_capability.1
        )
    }

    /// Get total memory in gigabytes.
    #[must_use]
    pub fn total_memory_gb(&self) -> f64 {
        self.total_memory_bytes as f64 / GB as f64
    }

    /// Check if this GPU meets the minimum compute capability.
    #[must_use]
    pub fn meets_compute_requirement(&self, required_major: u32, required_minor: u32) -> bool {
        self.compute_capability.0 > required_major
            || (self.compute_capability.0 == required_major
                && self.compute_capability.1 >= required_minor)
    }

    /// Check if this GPU meets RTX 5090 requirements.
    #[must_use]
    pub fn meets_rtx_5090_requirements(&self) -> bool {
        self.meets_compute_requirement(REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR)
            && self.total_memory_bytes >= MINIMUM_VRAM_BYTES
    }
}

impl Default for GpuInfo {
    fn default() -> Self {
        Self {
            device_id: 0,
            name: "No GPU".to_string(),
            compute_capability: (0, 0),
            total_memory_bytes: 0,
            driver_version: "N/A".to_string(),
        }
    }
}
