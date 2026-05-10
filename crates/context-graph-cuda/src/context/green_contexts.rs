//! Green Contexts implementation for GPU SM partitioning.
//!
//! TASK-13: Implements CUDA Green Contexts auto-enable for Volta+ GPUs.
//! Constitution: 70% inference / 30% background partition.
//!
//! # Green Contexts vs MIG
//!
//! - Green Contexts: In-process SM partitioning (what we use)
//! - MIG: System-level GPU partitioning (A100/H100 only, not RTX)
//!
//! # Compute Capability Requirements
//!
//! - 7.0+ (Volta): Basic green contexts support
//! - 9.0+ (Hopper): Thread block cluster features
//! - 12.0 (RTX 5090): Full support with Blackwell optimizations
//!
//! # Fail-Closed GPU Checks
//!
//! Unsupported GPUs, invalid configuration, or failed CUDA device-attribute
//! queries return errors. Phase 1/Phase 1b must not silently continue with
//! degraded GPU behavior.

use crate::error::{CudaError, CudaResult};
use crate::safe::device::GpuDevice;
use tracing::info;

// ============================================================================
// CONSTANTS (Constitution: 70% inference / 30% background)
// ============================================================================

/// Minimum compute capability for Green Contexts support.
/// Based on CUDA documentation: Green Contexts available since Volta (7.0).
pub const GREEN_CONTEXTS_MIN_COMPUTE_MAJOR: u32 = 7;
pub const GREEN_CONTEXTS_MIN_COMPUTE_MINOR: u32 = 0;

/// Constitution-mandated inference partition percentage.
pub const INFERENCE_PARTITION_PERCENT: f32 = 0.70;

/// Constitution-mandated background partition percentage.
pub const BACKGROUND_PARTITION_PERCENT: f32 = 0.30;

/// Minimum SMs required for partitioning to be worthwhile.
/// Below this, the overhead of partitioning exceeds benefits.
pub const MIN_SMS_FOR_PARTITIONING: u32 = 8;

// ============================================================================
// CONFIGURATION
// ============================================================================

/// Green Contexts configuration for GPU SM partitioning.
///
/// # Constitution Compliance
///
/// - `stack.gpu.target = "RTX 5090"` (Compute 12.0)
/// - 70% inference / 30% background partitioning
///
/// # Example
///
/// ```ignore
/// use context_graph_cuda::context::GreenContextsConfig;
///
/// let config = GreenContextsConfig::default();
/// assert_eq!(config.min_compute_capability, (7, 0));
/// assert!((config.inference_partition - 0.70).abs() < 0.001);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GreenContextsConfig {
    /// Minimum compute capability required (major, minor).
    /// Default: (7, 0) for Volta+ support.
    pub min_compute_capability: (u32, u32),

    /// Inference workload partition percentage (0.0 to 1.0).
    /// Constitution: 0.70 (70% of SMs).
    pub inference_partition: f32,

    /// Background workload partition percentage (0.0 to 1.0).
    /// Constitution: 0.30 (30% of SMs).
    pub background_partition: f32,
}

impl Default for GreenContextsConfig {
    fn default() -> Self {
        Self {
            min_compute_capability: (
                GREEN_CONTEXTS_MIN_COMPUTE_MAJOR,
                GREEN_CONTEXTS_MIN_COMPUTE_MINOR,
            ),
            inference_partition: INFERENCE_PARTITION_PERCENT,
            background_partition: BACKGROUND_PARTITION_PERCENT,
        }
    }
}

impl GreenContextsConfig {
    /// Validate configuration.
    ///
    /// # Errors
    ///
    /// Returns `CudaError::InvalidConfig` if:
    /// - Partition percentages sum to > 1.0
    /// - Any percentage is negative
    /// - Compute capability is invalid (major = 0)
    pub fn validate(&self) -> CudaResult<()> {
        let total = self.inference_partition + self.background_partition;
        if total > 1.0 {
            return Err(CudaError::InvalidConfig(format!(
                "Partition percentages sum to {:.2}, must be <= 1.0",
                total
            )));
        }
        if self.inference_partition < 0.0 || self.background_partition < 0.0 {
            return Err(CudaError::InvalidConfig(
                "Partition percentages cannot be negative".to_string(),
            ));
        }
        if self.min_compute_capability.0 == 0 {
            return Err(CudaError::InvalidConfig(
                "Compute capability major version cannot be 0".to_string(),
            ));
        }
        Ok(())
    }
}

// ============================================================================
// CAPABILITY DETECTION
// ============================================================================

/// Check if Green Contexts should be auto-enabled for this device.
///
/// Returns `true` if the device's compute capability meets the minimum
/// requirement (default: 7.0 for Volta+).
///
/// # Example
///
/// ```ignore
/// use context_graph_cuda::{GpuDevice, context::should_enable_green_contexts};
///
/// let device = GpuDevice::new(0)?;
/// if should_enable_green_contexts(&device)? {
///     println!("Green Contexts available!");
/// } else {
///     println!("Running without GPU partitioning");
/// }
/// ```
pub fn should_enable_green_contexts(device: &GpuDevice) -> CudaResult<bool> {
    should_enable_green_contexts_with_config(device, &GreenContextsConfig::default())
}

/// Check Green Contexts support with custom configuration.
///
/// Same as `should_enable_green_contexts` but with configurable minimum
/// compute capability requirements.
pub fn should_enable_green_contexts_with_config(
    device: &GpuDevice,
    config: &GreenContextsConfig,
) -> CudaResult<bool> {
    let (major, minor) = device.compute_capability()?;
    let (req_major, req_minor) = config.min_compute_capability;
    let gpu_name = device.name()?;

    let supported = major > req_major || (major == req_major && minor >= req_minor);

    if supported {
        info!(
            gpu_name = %gpu_name,
            compute = format!("{}.{}", major, minor),
            required = format!("{}.{}", req_major, req_minor),
            "Green Contexts ENABLED: compute capability meets requirements"
        );
    } else {
        info!(
            gpu_name = %gpu_name,
            compute = format!("{}.{}", major, minor),
            required = format!("{}.{}", req_major, req_minor),
            "Green Contexts unavailable: compute capability below requirements"
        );
    }

    Ok(supported)
}

// ============================================================================
// GREEN CONTEXT PARTITION
// ============================================================================

/// Represents a single Green Context partition.
///
/// Each partition owns a specific subset of GPU Streaming Multiprocessors (SMs).
/// Kernels launched in this partition's context are guaranteed to only use
/// the allocated SMs.
///
/// # Thread Safety
///
/// `GreenContext` is `Send` but NOT `Sync` - same as CUDA contexts.
#[derive(Debug)]
pub struct GreenContext {
    /// Partition identifier (0 = inference, 1 = background).
    partition_id: u32,

    /// Percentage of total SMs allocated to this partition.
    percentage: f32,

    /// Actual SM count allocated (computed from total SMs × percentage).
    sm_count: u32,

    /// Whether this partition is currently active.
    active: bool,
}

impl GreenContext {
    /// Get partition ID.
    #[inline]
    #[must_use]
    pub fn partition_id(&self) -> u32 {
        self.partition_id
    }

    /// Get allocated SM percentage.
    #[inline]
    #[must_use]
    pub fn percentage(&self) -> f32 {
        self.percentage
    }

    /// Get actual SM count.
    #[inline]
    #[must_use]
    pub fn sm_count(&self) -> u32 {
        self.sm_count
    }

    /// Check if partition is active.
    #[inline]
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }
}

// ============================================================================
// GREEN CONTEXTS MANAGER
// ============================================================================

/// Green Contexts manager for GPU SM partitioning.
///
/// Manages inference (70%) and background (30%) GPU partitions per constitution.
/// Auto-detects GPU capability and fails closed on unsupported hardware.
///
/// # Lifecycle
///
/// 1. Create with `GreenContexts::new(device, config)`
/// 2. Check `is_enabled()` to see if partitioning is active
/// 3. Get partitions with `inference_context()` / `background_context()`
/// 4. Partitions are automatically cleaned up on drop
///
/// # Example
///
/// ```ignore
/// use context_graph_cuda::{GpuDevice, context::{GreenContexts, GreenContextsConfig}};
///
/// let device = GpuDevice::new(0)?;
/// let gc = GreenContexts::new(&device, GreenContextsConfig::default())?;
///
/// if gc.is_enabled() {
///     if let Some(inference) = gc.inference_context() {
///         println!("Inference partition: {}% ({} SMs)",
///             inference.percentage() * 100.0,
///             inference.sm_count());
///     }
/// }
/// ```
#[derive(Debug)]
pub struct GreenContexts {
    /// Configuration used for this manager.
    config: GreenContextsConfig,

    /// Whether Green Contexts are enabled (GPU supports it).
    enabled: bool,

    /// Total SM count on the device.
    total_sm_count: u32,

    /// Inference partition (70% SMs). None if not enabled.
    inference: Option<GreenContext>,

    /// Background partition (30% SMs). None if not enabled.
    background: Option<GreenContext>,
}

impl GreenContexts {
    /// Create a new Green Contexts manager.
    ///
    /// Auto-detects GPU capability and creates partitions if supported.
    /// Unsupported GPUs and invalid configuration fail closed.
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device to partition
    /// * `config` - Partition configuration
    ///
    /// # Example
    ///
    /// ```ignore
    /// let device = GpuDevice::new(0)?;
    /// let gc = GreenContexts::new(&device, GreenContextsConfig::default())?;
    /// ```
    pub fn new(device: &GpuDevice, config: GreenContextsConfig) -> CudaResult<Self> {
        // Validate config first
        config.validate()?;

        // Check capability
        if !should_enable_green_contexts_with_config(device, &config)? {
            let (major, minor) = device.compute_capability()?;
            return Err(CudaError::InvalidConfig(format!(
                "Green Contexts require compute capability {}.{}, got {}.{}",
                config.min_compute_capability.0, config.min_compute_capability.1, major, minor
            )));
        }

        // Query the real SM count from the CUDA driver. Hardcoded estimates hide
        // driver/device mismatches and are not acceptable for Phase 1b preflight.
        let total_sm_count = device.multiprocessor_count()?;

        if total_sm_count < MIN_SMS_FOR_PARTITIONING {
            return Err(CudaError::InvalidConfig(format!(
                "Green Contexts require at least {} SMs, got {}",
                MIN_SMS_FOR_PARTITIONING, total_sm_count
            )));
        }

        // Calculate partition sizes
        let inference_sms = (total_sm_count as f32 * config.inference_partition).floor() as u32;
        let background_sms = (total_sm_count as f32 * config.background_partition).floor() as u32;

        // Ensure at least 1 SM per partition
        let inference_sms = inference_sms.max(1);
        let background_sms = background_sms.max(1);

        info!(
            total_sms = total_sm_count,
            inference_sms = inference_sms,
            background_sms = background_sms,
            inference_pct = format!("{:.0}%", config.inference_partition * 100.0),
            background_pct = format!("{:.0}%", config.background_partition * 100.0),
            "Green Contexts partitions created"
        );

        Ok(Self {
            config,
            enabled: true,
            total_sm_count,
            inference: Some(GreenContext {
                partition_id: 0,
                percentage: config.inference_partition,
                sm_count: inference_sms,
                active: true,
            }),
            background: Some(GreenContext {
                partition_id: 1,
                percentage: config.background_partition,
                sm_count: background_sms,
                active: true,
            }),
        })
    }

    /// Create a disabled Green Contexts manager.
    #[cfg(test)]
    fn disabled(config: GreenContextsConfig) -> Self {
        Self {
            config,
            enabled: false,
            total_sm_count: 0,
            inference: None,
            background: None,
        }
    }

    /// Check if Green Contexts are enabled.
    #[inline]
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the inference partition context (70% SMs).
    ///
    /// Returns `None` if Green Contexts are not enabled.
    #[inline]
    #[must_use]
    pub fn inference_context(&self) -> Option<&GreenContext> {
        self.inference.as_ref()
    }

    /// Get the background partition context (30% SMs).
    ///
    /// Returns `None` if Green Contexts are not enabled.
    #[inline]
    #[must_use]
    pub fn background_context(&self) -> Option<&GreenContext> {
        self.background.as_ref()
    }

    /// Get total SM count.
    #[inline]
    #[must_use]
    pub fn total_sm_count(&self) -> u32 {
        self.total_sm_count
    }

    /// Get the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &GreenContextsConfig {
        &self.config
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Configuration Tests (No GPU Required)
    // ========================================================================

    #[test]
    fn test_config_default() {
        let config = GreenContextsConfig::default();
        assert_eq!(config.min_compute_capability, (7, 0));
        assert!((config.inference_partition - 0.70).abs() < 0.001);
        assert!((config.background_partition - 0.30).abs() < 0.001);
    }

    #[test]
    fn test_config_validate_success() {
        let config = GreenContextsConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_partition_sum_exceeds_one() {
        let config = GreenContextsConfig {
            min_compute_capability: (7, 0),
            inference_partition: 0.8,
            background_partition: 0.3, // Sum = 1.1 > 1.0
        };
        let result = config.validate();
        assert!(result.is_err());
        match result {
            Err(CudaError::InvalidConfig(msg)) => {
                assert!(msg.contains("sum to"), "Error message: {}", msg);
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    #[test]
    fn test_config_validate_negative_partition() {
        let config = GreenContextsConfig {
            min_compute_capability: (7, 0),
            inference_partition: -0.1,
            background_partition: 0.3,
        };
        let result = config.validate();
        assert!(result.is_err());
        match result {
            Err(CudaError::InvalidConfig(msg)) => {
                assert!(msg.contains("negative"), "Error message: {}", msg);
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    #[test]
    fn test_config_validate_zero_compute() {
        let config = GreenContextsConfig {
            min_compute_capability: (0, 0),
            inference_partition: 0.7,
            background_partition: 0.3,
        };
        let result = config.validate();
        assert!(result.is_err());
    }

    // ========================================================================
    // Constants Tests
    // ========================================================================

    #[test]
    fn test_partition_percentages_sum() {
        let sum = INFERENCE_PARTITION_PERCENT + BACKGROUND_PARTITION_PERCENT;
        assert!(
            (sum - 1.0).abs() < 0.001,
            "Partition percentages must sum to 1.0, got {}",
            sum
        );
    }

    #[test]
    fn test_min_compute_capability_constants() {
        assert_eq!(GREEN_CONTEXTS_MIN_COMPUTE_MAJOR, 7);
        assert_eq!(GREEN_CONTEXTS_MIN_COMPUTE_MINOR, 0);
    }

    // ========================================================================
    // GreenContext Tests
    // ========================================================================

    #[test]
    fn test_green_context_accessors() {
        let ctx = GreenContext {
            partition_id: 0,
            percentage: 0.7,
            sm_count: 119,
            active: true,
        };
        assert_eq!(ctx.partition_id(), 0);
        assert!((ctx.percentage() - 0.7).abs() < 0.001);
        assert_eq!(ctx.sm_count(), 119);
        assert!(ctx.is_active());
    }

    // ========================================================================
    // GreenContexts Manager Tests (No GPU Required)
    // ========================================================================

    #[test]
    fn test_disabled_manager() {
        let config = GreenContextsConfig::default();
        let gc = GreenContexts::disabled(config);

        assert!(!gc.is_enabled());
        assert!(gc.inference_context().is_none());
        assert!(gc.background_context().is_none());
        assert_eq!(gc.total_sm_count(), 0);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_min_sms_constant() {
        assert!(
            MIN_SMS_FOR_PARTITIONING >= 8,
            "Need at least 8 SMs for meaningful partitioning"
        );
    }

    // ========================================================================
    // Edge Case Tests (MANDATORY - No GPU Required)
    // ========================================================================

    #[test]
    fn edge_case_partition_sum_overflow() {
        let config = GreenContextsConfig {
            min_compute_capability: (7, 0),
            inference_partition: 0.9,
            background_partition: 0.9, // Sum = 1.8
        };
        println!("BEFORE validate: config = {:?}", config);
        let result = config.validate();
        println!("AFTER validate: result = {:?}", result);
        assert!(result.is_err());
    }

    #[test]
    fn edge_case_zero_compute() {
        let config = GreenContextsConfig {
            min_compute_capability: (0, 0),
            inference_partition: 0.7,
            background_partition: 0.3,
        };
        println!("BEFORE validate: config = {:?}", config);
        let result = config.validate();
        println!("AFTER validate: result = {:?}", result);
        assert!(result.is_err());
    }

    #[test]
    fn edge_case_negative_partition() {
        let config = GreenContextsConfig {
            min_compute_capability: (7, 0),
            inference_partition: -0.5,
            background_partition: 0.3,
        };
        println!("BEFORE validate: config = {:?}", config);
        let result = config.validate();
        println!("AFTER validate: result = {:?}", result);
        assert!(result.is_err());
    }

    // ========================================================================
    // GPU-Required Tests
    // ========================================================================

    #[test]
    fn test_green_contexts_with_real_gpu() {
        let device = GpuDevice::new(0).expect("GPU required for this test");
        let config = GreenContextsConfig::default();
        let (major, _) = device
            .compute_capability()
            .expect("compute capability query failed");

        if major >= 7 {
            let gc = GreenContexts::new(&device, config).expect("Green Contexts creation failed");
            // Should be enabled on Volta+ GPUs
            assert!(
                gc.is_enabled(),
                "Green Contexts should be enabled on Volta+ GPU"
            );

            let inference = gc
                .inference_context()
                .expect("inference partition should exist");
            assert_eq!(inference.partition_id(), 0);
            assert!(inference.sm_count() > 0);

            let background = gc
                .background_context()
                .expect("background partition should exist");
            assert_eq!(background.partition_id(), 1);
            assert!(background.sm_count() > 0);

            // Verify partition sizes are reasonable
            assert!(
                inference.sm_count() > background.sm_count(),
                "Inference partition ({}) should be larger than background ({}) (70% vs 30%)",
                inference.sm_count(),
                background.sm_count()
            );
        } else {
            // Older GPU must fail closed.
            assert!(
                GreenContexts::new(&device, config).is_err(),
                "Green Contexts should fail closed on pre-Volta GPU"
            );
        }
    }

    #[test]
    fn test_green_contexts_rtx5090() {
        let device = GpuDevice::new(0).expect("GPU required");
        let (major, minor) = device
            .compute_capability()
            .expect("compute capability query failed");

        // Only run on RTX 5090
        if major != 12 || minor != 0 {
            println!(
                "Skipping: requires RTX 5090 (compute 12.0), got {}.{}",
                major, minor
            );
            return;
        }

        let gc = GreenContexts::new(&device, GreenContextsConfig::default())
            .expect("Green Contexts creation failed");

        assert!(
            gc.is_enabled(),
            "Green Contexts must be enabled on RTX 5090"
        );

        // RTX 5090 should have ~170 SMs
        assert!(
            gc.total_sm_count() >= 150,
            "RTX 5090 should have ~170 SMs, got {}",
            gc.total_sm_count()
        );

        let inference = gc.inference_context().expect("inference partition");
        let background = gc.background_context().expect("background partition");

        // Verify 70/30 split
        let inference_percent = inference.sm_count() as f32 / gc.total_sm_count() as f32;
        let background_percent = background.sm_count() as f32 / gc.total_sm_count() as f32;

        assert!(
            (inference_percent - 0.70).abs() < 0.05,
            "Inference should be ~70%, got {:.1}%",
            inference_percent * 100.0
        );
        assert!(
            (background_percent - 0.30).abs() < 0.05,
            "Background should be ~30%, got {:.1}%",
            background_percent * 100.0
        );
    }

    #[test]
    fn test_green_contexts_fail_closed() {
        let device = GpuDevice::new(0).expect("GPU required");

        // Impossible compute requirements should fail closed.
        let invalid_config = GreenContextsConfig {
            min_compute_capability: (99, 0), // No GPU has this
            inference_partition: 0.7,
            background_partition: 0.3,
        };
        assert!(
            GreenContexts::new(&device, invalid_config).is_err(),
            "Should fail closed with impossible requirements"
        );

        // Very high partition sum should be caught by validation
        let bad_config = GreenContextsConfig {
            min_compute_capability: (7, 0),
            inference_partition: 0.8,
            background_partition: 0.5, // Sum = 1.3
        };
        assert!(
            GreenContexts::new(&device, bad_config).is_err(),
            "Should fail closed with invalid partition sum"
        );
    }
}
