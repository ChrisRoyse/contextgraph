//! Warm Loading Data Types for GPU Weight Management.
//!
//! This module defines the data structures used for loading model weights into GPU memory
//! during warm startup. All types enforce fail-fast validation per Constitution AP-007.
//!
//! # Constitution Alignment
//!
//! - **AP-007**: No Stub Data in Production - All fields contain REAL validated data
//! - **REQ-WARM-003**: Non-evictable VRAM allocation
//! - **REQ-WARM-005**: Weight integrity verification via SHA256 checksums
//!
//! # Design Philosophy
//!
//! **FAIL FAST. NO FALLBACKS.**
//!
//! - All constructors panic on invalid data (null pointers, zero checksums, empty collections)
//! - No silent defaults or fallback values
//! - Validation happens at construction time, not runtime
//!
//! # Critical: No Simulation
//!
//! These types are designed to hold REAL data from actual GPU operations:
//! - `gpu_ptr` must be a real cudaMalloc pointer (never 0x0)
//! - `checksum` must be a real SHA256 hash (never all zeros)
//! - `tensors` must contain real GpuTensor instances backed by GPU memory

use crate::gpu::GpuTensor;
use candle_core::DType;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// =============================================================================
// CRITICAL: NO SIMULATION - ALL DATA MUST BE REAL
// Constitution AP-007: "No Stub Data in Production"
// =============================================================================

/// Metadata extracted from SafeTensors file header.
///
/// Contains the shape and type information for all tensors in a model weight file.
/// This metadata is parsed from the SafeTensors header before actual weight loading.
///
/// # Constitution Alignment
///
/// - AP-007: No stub data - shapes must reflect actual SafeTensors content
///
/// # CRITICAL: No Simulation
///
/// All fields must reflect actual SafeTensors header content. This struct will
/// PANIC if constructed with invalid data.
///
/// # Example
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// use candle_core::DType;
/// use context_graph_embeddings::warm::loader::types::TensorMetadata;
///
/// let mut shapes = HashMap::new();
/// shapes.insert("embeddings.weight".to_string(), vec![30522, 768]);
/// shapes.insert("encoder.layer.0.attention.self.query.weight".to_string(), vec![768, 768]);
///
/// let metadata = TensorMetadata::new(shapes, DType::F32, 24_030_504);
/// assert!(metadata.verify_params());
/// ```
#[derive(Debug, Clone)]
pub struct TensorMetadata {
    /// Tensor name -> shape mapping.
    ///
    /// Example: `{"embeddings.weight": [30522, 768], "layer.0.weight": [768, 768]}`
    ///
    /// # Invariant
    /// Must be non-empty. Empty shapes indicates corrupted or incomplete SafeTensors file.
    pub shapes: HashMap<String, Vec<usize>>,

    /// Data type of tensors (from candle_core).
    ///
    /// Common values: DType::F32, DType::F16, DType::BF16
    pub dtype: DType,

    /// Total number of parameters across all tensors.
    ///
    /// # Invariant
    /// MUST be > 0 for valid models. Zero parameters indicates empty/corrupted model.
    pub total_params: usize,
}

impl TensorMetadata {
    /// Create new TensorMetadata with validation.
    ///
    /// # Arguments
    ///
    /// * `shapes` - HashMap mapping tensor names to their shapes
    /// * `dtype` - Data type of the tensors
    /// * `total_params` - Total parameter count (should match sum of shape products)
    ///
    /// # Panics
    ///
    /// - If `shapes` is empty (no tensors in SafeTensors file)
    /// - If `total_params` is 0 (empty model)
    ///
    /// # Constitution: Fail-Fast
    ///
    /// Per AP-007, we panic immediately on invalid data rather than
    /// propagating corruption through the system.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let shapes: HashMap<String, Vec<usize>> = [
    ///     ("layer.weight".to_string(), vec![768, 768])
    /// ].into_iter().collect();
    ///
    /// let metadata = TensorMetadata::new(shapes, DType::F32, 589_824);
    /// ```
    #[must_use]
    pub fn new(shapes: HashMap<String, Vec<usize>>, dtype: DType, total_params: usize) -> Self {
        // FAIL-FAST: Empty shapes means corrupted SafeTensors
        assert!(
            !shapes.is_empty(),
            "CONSTITUTION VIOLATION AP-007: shapes is empty. \
             SafeTensors must contain at least one tensor. \
             This indicates corrupted or incomplete weight file."
        );

        // FAIL-FAST: Zero params means empty model
        assert!(
            total_params > 0,
            "CONSTITUTION VIOLATION AP-007: total_params is 0. \
             Model must have parameters. \
             This indicates corrupted or empty weight file."
        );

        Self {
            shapes,
            dtype,
            total_params,
        }
    }

    /// Calculate total parameters from shapes (for verification).
    ///
    /// Computes the sum of all shape products to verify against stored total_params.
    ///
    /// # Returns
    ///
    /// Sum of element counts across all tensors.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // For shapes: {"a": [100, 768], "b": [768, 768]}
    /// // Returns: 100*768 + 768*768 = 76800 + 589824 = 666624
    /// let calculated = metadata.calculate_total_params();
    /// ```
    #[must_use]
    pub fn calculate_total_params(&self) -> usize {
        self.shapes
            .values()
            .map(|shape| shape.iter().product::<usize>())
            .sum()
    }

    /// Verify total_params matches calculated value.
    ///
    /// # Returns
    ///
    /// `true` if stored total_params equals the sum of all shape products.
    ///
    /// # Use Case
    ///
    /// Call this after parsing SafeTensors header to verify consistency.
    #[must_use]
    pub fn verify_params(&self) -> bool {
        self.total_params == self.calculate_total_params()
    }

    /// Get the number of tensors in this metadata.
    #[must_use]
    pub fn tensor_count(&self) -> usize {
        self.shapes.len()
    }

    /// Get shape for a specific tensor by name.
    #[must_use]
    pub fn get_shape(&self, name: &str) -> Option<&Vec<usize>> {
        self.shapes.get(name)
    }
}

/// Result of loading a model's weights into GPU memory.
///
/// Contains the GPU pointer, checksum, and metadata from a successful weight load operation.
/// This is the primary type returned by warm loading operations.
///
/// # Constitution Alignment
///
/// - REQ-WARM-003: Non-evictable VRAM allocation (gpu_ptr points to pinned memory)
/// - REQ-WARM-005: Weight integrity verification (checksum for validation)
/// - AP-007: No stub data in production (all fields are real, validated data)
///
/// # CRITICAL: No Simulation
///
/// All fields contain REAL data from actual loading operations.
/// This struct will PANIC if constructed with invalid data.
///
/// # Example
///
/// ```rust,ignore
/// use std::time::Duration;
/// use std::collections::HashMap;
/// use candle_core::DType;
/// use context_graph_embeddings::warm::loader::types::{WarmLoadResult, TensorMetadata};
///
/// // After real cudaMalloc and SHA256 computation:
/// let metadata = TensorMetadata::new(
///     [("weight".to_string(), vec![768, 768])].into_iter().collect(),
///     DType::F32,
///     589_824,
/// );
///
/// let result = WarmLoadResult::new(
///     0x7fff_dead_beef,      // Real GPU pointer from cudaMalloc
///     [0xAB; 32],            // Real SHA256 checksum
///     2_359_296,             // 589824 * 4 bytes
///     Duration::from_millis(150),
///     metadata,
/// );
///
/// assert!(result.verify_checksum(&[0xAB; 32]));
/// ```
#[derive(Debug)]
pub struct WarmLoadResult {
    /// Real GPU device pointer from cudaMalloc.
    ///
    /// # Invariant
    /// MUST be non-zero. Zero pointer = PANIC.
    /// This must be an actual pointer returned by CUDA memory allocation.
    pub gpu_ptr: u64,

    /// Real SHA256 checksum of the weight file.
    ///
    /// # Invariant
    /// MUST be non-zero. All-zero checksum = PANIC.
    /// This must be computed from actual file content.
    pub checksum: [u8; 32],

    /// Actual size of weights in GPU memory (bytes).
    ///
    /// # Invariant
    /// MUST be > 0. Zero size = PANIC.
    /// This is the actual cudaMalloc allocation size.
    pub size_bytes: usize,

    /// Loading duration for performance monitoring.
    ///
    /// Measures wall-clock time from start of load to completion.
    pub load_duration: Duration,

    /// Tensor metadata from SafeTensors header.
    ///
    /// Contains shapes, dtype, and total parameter count.
    pub tensor_metadata: TensorMetadata,
}

impl WarmLoadResult {
    /// Create a new WarmLoadResult with validation.
    ///
    /// # Arguments
    ///
    /// * `gpu_ptr` - Real CUDA device pointer (must be non-zero)
    /// * `checksum` - Real SHA256 checksum (must be non-zero)
    /// * `size_bytes` - Allocation size in bytes (must be > 0)
    /// * `load_duration` - Time taken to load
    /// * `tensor_metadata` - Parsed SafeTensors metadata
    ///
    /// # Panics
    ///
    /// - If `gpu_ptr` is 0 (null pointer)
    /// - If `checksum` is all zeros (invalid checksum)
    /// - If `size_bytes` is 0 (empty allocation)
    /// - If `tensor_metadata.total_params` is 0 (empty model)
    ///
    /// # Constitution: Fail-Fast
    ///
    /// Per AP-007, we panic immediately on invalid data rather than
    /// propagating corruption through the system.
    #[must_use]
    pub fn new(
        gpu_ptr: u64,
        checksum: [u8; 32],
        size_bytes: usize,
        load_duration: Duration,
        tensor_metadata: TensorMetadata,
    ) -> Self {
        // FAIL-FAST: Null GPU pointer
        assert!(
            gpu_ptr != 0,
            "CONSTITUTION VIOLATION AP-007: gpu_ptr is null (0x0). \
             Real cudaMalloc pointer required. \
             This indicates CUDA allocation failure or simulated data."
        );

        // FAIL-FAST: Zero checksum (impossible for real SHA256)
        assert!(
            checksum != [0u8; 32],
            "CONSTITUTION VIOLATION AP-007: checksum is all zeros. \
             Real SHA256 checksum required. \
             This indicates simulated data or computation failure."
        );

        // FAIL-FAST: Zero size
        assert!(
            size_bytes > 0,
            "CONSTITUTION VIOLATION AP-007: size_bytes is 0. \
             Real allocation size required. \
             This indicates allocation failure or simulated data."
        );

        // FAIL-FAST: Empty model (validated in TensorMetadata, but double-check)
        assert!(
            tensor_metadata.total_params > 0,
            "CONSTITUTION VIOLATION AP-007: total_params is 0. \
             Real model weights required. \
             This indicates corrupted or empty weight file."
        );

        Self {
            gpu_ptr,
            checksum,
            size_bytes,
            load_duration,
            tensor_metadata,
        }
    }

    /// Verify checksum matches expected value.
    ///
    /// # Arguments
    ///
    /// * `expected` - Expected SHA256 checksum to compare against
    ///
    /// # Returns
    ///
    /// `true` if checksums match exactly.
    #[must_use]
    pub fn verify_checksum(&self, expected: &[u8; 32]) -> bool {
        self.checksum == *expected
    }

    /// Get checksum as hex string for display/logging.
    #[must_use]
    pub fn checksum_hex(&self) -> String {
        self.checksum.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Get the bytes per parameter based on dtype.
    #[must_use]
    pub fn bytes_per_param(&self) -> usize {
        self.tensor_metadata.dtype.size_in_bytes()
    }
}

/// Complete set of weights for a model loaded into GPU memory.
///
/// Represents a fully loaded model with all tensors resident in GPU VRAM.
/// This is the primary output of the warm loading pipeline.
///
/// # Constitution Alignment
///
/// - REQ-WARM-003: Non-evictable VRAM allocation (tensors are pinned)
/// - REQ-WARM-005: Weight integrity verification (file_checksum)
/// - AP-007: No stub data in production (all tensors are real GPU data)
///
/// # CRITICAL: No Simulation
///
/// This represents REAL weights loaded into REAL GPU memory.
/// All GpuTensor instances must be backed by actual CUDA allocations.
///
/// # Example
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// use context_graph_embeddings::gpu::GpuTensor;
/// use context_graph_embeddings::warm::loader::types::LoadedModelWeights;
///
/// // After loading real tensors to GPU:
/// let mut tensors = HashMap::new();
/// tensors.insert("embeddings.weight".to_string(), gpu_tensor_1);
/// tensors.insert("encoder.weight".to_string(), gpu_tensor_2);
///
/// let weights = LoadedModelWeights::new(
///     "E1_Semantic".to_string(),
///     tensors,
///     [0xAB; 32],           // Real SHA256
///     100_000_000,          // ~100MB GPU memory
///     0,                    // CUDA device 0
/// );
///
/// assert!(weights.has_tensor("embeddings.weight"));
/// ```
#[derive(Debug)]
pub struct LoadedModelWeights {
    /// Model identifier (e.g., "E1_Semantic", "E2_Code").
    ///
    /// # Invariant
    /// MUST be non-empty. Empty identifier = PANIC.
    pub model_id: String,

    /// Named tensors loaded to GPU.
    ///
    /// Uses existing GpuTensor from crate::gpu module.
    /// Key is the tensor name from SafeTensors (e.g., "encoder.layer.0.weight").
    ///
    /// # Invariant
    /// MUST be non-empty. Model must have at least one tensor.
    pub tensors: HashMap<String, GpuTensor>,

    /// SHA256 checksum of source weight file.
    ///
    /// # Invariant
    /// MUST be non-zero. All-zero checksum = PANIC.
    /// Used to verify weight file integrity.
    pub file_checksum: [u8; 32],

    /// Total GPU memory used (bytes).
    ///
    /// # Invariant
    /// MUST be > 0. Sum of all tensor memory allocations.
    pub total_gpu_bytes: usize,

    /// CUDA device where weights are loaded.
    ///
    /// 0 = first GPU, 1 = second GPU, etc.
    pub device_id: u32,

    /// Timestamp when weights were loaded.
    ///
    /// Used for performance monitoring and cache management.
    pub loaded_at: Instant,
}

impl LoadedModelWeights {
    /// Create new LoadedModelWeights with validation.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Model identifier (must be non-empty)
    /// * `tensors` - HashMap of named GpuTensors (must be non-empty)
    /// * `file_checksum` - SHA256 of source file (must be non-zero)
    /// * `total_gpu_bytes` - Total GPU memory used (must be > 0)
    /// * `device_id` - CUDA device index
    ///
    /// # Panics
    ///
    /// - If `model_id` is empty
    /// - If `tensors` is empty
    /// - If `file_checksum` is all zeros
    /// - If `total_gpu_bytes` is 0
    ///
    /// # Constitution: Fail-Fast
    ///
    /// Per AP-007, we panic immediately on invalid data rather than
    /// propagating corruption through the system.
    #[must_use]
    pub fn new(
        model_id: String,
        tensors: HashMap<String, GpuTensor>,
        file_checksum: [u8; 32],
        total_gpu_bytes: usize,
        device_id: u32,
    ) -> Self {
        // FAIL-FAST: Empty model ID
        assert!(
            !model_id.is_empty(),
            "CONSTITUTION VIOLATION AP-007: model_id is empty. \
             Model must have a valid identifier."
        );

        // FAIL-FAST: No tensors
        assert!(
            !tensors.is_empty(),
            "CONSTITUTION VIOLATION AP-007: tensors is empty. \
             Model must have at least one tensor. \
             This indicates loading failure or corrupted weight file."
        );

        // FAIL-FAST: Zero checksum
        assert!(
            file_checksum != [0u8; 32],
            "CONSTITUTION VIOLATION AP-007: file_checksum is all zeros. \
             Real SHA256 checksum required for weight integrity."
        );

        // FAIL-FAST: Zero GPU bytes
        assert!(
            total_gpu_bytes > 0,
            "CONSTITUTION VIOLATION AP-007: total_gpu_bytes is 0. \
             Loaded model must occupy GPU memory."
        );

        Self {
            model_id,
            tensors,
            file_checksum,
            total_gpu_bytes,
            device_id,
            loaded_at: Instant::now(),
        }
    }

    /// Get a specific tensor by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Tensor name (e.g., "encoder.layer.0.weight")
    ///
    /// # Returns
    ///
    /// Reference to the GpuTensor if found, None otherwise.
    #[must_use]
    pub fn get_tensor(&self, name: &str) -> Option<&GpuTensor> {
        self.tensors.get(name)
    }

    /// Check if a tensor exists by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Tensor name to check
    ///
    /// # Returns
    ///
    /// `true` if tensor exists in this model's weights.
    #[must_use]
    pub fn has_tensor(&self, name: &str) -> bool {
        self.tensors.contains_key(name)
    }

    /// Get all tensor names.
    ///
    /// # Returns
    ///
    /// Iterator over tensor name strings.
    pub fn tensor_names(&self) -> impl Iterator<Item = &str> {
        self.tensors.keys().map(|s| s.as_str())
    }

    /// Get the number of tensors in this model.
    #[must_use]
    pub fn tensor_count(&self) -> usize {
        self.tensors.len()
    }

    /// Verify checksum matches expected value.
    ///
    /// # Arguments
    ///
    /// * `expected` - Expected SHA256 checksum
    ///
    /// # Returns
    ///
    /// `true` if checksums match exactly.
    #[must_use]
    pub fn verify_checksum(&self, expected: &[u8; 32]) -> bool {
        self.file_checksum == *expected
    }

    /// Get time since loading completed.
    #[must_use]
    pub fn age(&self) -> Duration {
        self.loaded_at.elapsed()
    }

    /// Get checksum as hex string for display/logging.
    #[must_use]
    pub fn checksum_hex(&self) -> String {
        self.file_checksum
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}

// =============================================================================
// VRAM ALLOCATION TRACKING (TASK-EMB-016)
// =============================================================================

/// GPU VRAM allocation tracking with real/fake detection.
///
/// # Constitution Alignment
///
/// - AP-007: All values MUST come from real CUDA API calls
/// - REQ-WARM-003: Non-evictable VRAM allocation tracking
///
/// # CRITICAL: No Simulation
///
/// Fake values are FORBIDDEN. The `is_real()` method detects known fake patterns:
/// - Fake pointer `0x7f80_0000_0000`
/// - VRAM delta mismatches (claims 1GB delta for 1KB allocation)
/// - Zero-size allocations
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_embeddings::warm::loader::types::VramAllocationTracking;
///
/// // From real CUDA calls:
/// let tracking = VramAllocationTracking::new(
///     0x7fff_0000_1000,  // Real cudaMalloc pointer
///     104_857_600,       // 100MB allocation
///     5000,              // 5GB VRAM before
///     5100,              // 5.1GB VRAM after (100MB delta)
/// );
///
/// assert!(tracking.is_real());
/// tracking.assert_real(); // Panics on fake data
/// ```
#[derive(Debug, Clone)]
pub struct VramAllocationTracking {
    /// Base pointer on GPU (from cudaMalloc).
    ///
    /// # Invariant
    /// MUST NOT be 0 (null) or 0x7f80_0000_0000 (known fake value).
    pub base_ptr: u64,

    /// Total bytes allocated.
    ///
    /// # Invariant
    /// MUST be > 0. Zero allocation indicates failed or fake allocation.
    pub size_bytes: usize,

    /// VRAM used before loading (from cudaMemGetInfo), in MB.
    pub vram_before_mb: u64,

    /// VRAM used after loading (from cudaMemGetInfo), in MB.
    pub vram_after_mb: u64,

    /// Actual delta: vram_after_mb - vram_before_mb.
    ///
    /// Calculated automatically in `new()`.
    pub vram_delta_mb: u64,
}

impl VramAllocationTracking {
    /// Known fake GPU pointer value used in simulations.
    ///
    /// Constitution AP-007 forbids using this value.
    pub const FAKE_POINTER: u64 = 0x7f80_0000_0000u64;

    /// Maximum allowed delta mismatch in MB (50MB tolerance for GPU overhead).
    pub const DELTA_TOLERANCE_MB: i64 = 50;

    /// Create new VramAllocationTracking with fail-fast validation.
    ///
    /// # Arguments
    ///
    /// * `base_ptr` - Real CUDA device pointer (must be non-zero)
    /// * `size_bytes` - Allocation size in bytes (must be > 0)
    /// * `vram_before_mb` - VRAM usage before allocation in MB
    /// * `vram_after_mb` - VRAM usage after allocation in MB
    ///
    /// # Panics
    ///
    /// - If `base_ptr` is 0 (null pointer)
    /// - If `size_bytes` is 0 (empty allocation)
    ///
    /// # Constitution: Fail-Fast
    ///
    /// Per AP-007, we panic immediately on invalid data rather than
    /// propagating corruption through the system.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tracking = VramAllocationTracking::new(
    ///     0x7fff_0000_1000,  // Real pointer
    ///     104_857_600,       // 100MB
    ///     5000,              // 5GB before
    ///     5100,              // 5.1GB after
    /// );
    /// ```
    #[must_use]
    pub fn new(base_ptr: u64, size_bytes: usize, vram_before_mb: u64, vram_after_mb: u64) -> Self {
        assert!(
            base_ptr != 0,
            "CONSTITUTION VIOLATION AP-007: base_ptr is null. \
             Real cudaMalloc pointer required."
        );
        assert!(
            size_bytes > 0,
            "CONSTITUTION VIOLATION AP-007: size_bytes is 0. \
             Real allocation size required."
        );

        let vram_delta_mb = vram_after_mb.saturating_sub(vram_before_mb);

        Self {
            base_ptr,
            size_bytes,
            vram_before_mb,
            vram_after_mb,
            vram_delta_mb,
        }
    }

    /// Check if allocation looks real (not simulated).
    ///
    /// Returns `false` if any known fake pattern is detected:
    /// - Fake pointer (0x7f80_0000_0000)
    /// - Zero-size allocation
    /// - VRAM delta doesn't match allocation size (within 50MB tolerance)
    ///
    /// # Returns
    ///
    /// `true` if allocation appears to be from real CUDA operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Real allocation: 100MB, delta matches
    /// let real = VramAllocationTracking::new(0x7fff_0000_1000, 104_857_600, 5000, 5100);
    /// assert!(real.is_real());
    ///
    /// // Fake: 1KB allocation with 1GB delta
    /// let fake = VramAllocationTracking {
    ///     base_ptr: 0x7fff_0000_1000,
    ///     size_bytes: 1024,
    ///     vram_before_mb: 1000,
    ///     vram_after_mb: 2000,
    ///     vram_delta_mb: 1000,
    /// };
    /// assert!(!fake.is_real());
    /// ```
    #[must_use]
    pub fn is_real(&self) -> bool {
        // Fake pointer check (common simulation value)
        if self.base_ptr == Self::FAKE_POINTER {
            return false;
        }

        // Zero allocation is suspicious
        if self.size_bytes == 0 {
            return false;
        }

        // VRAM delta should roughly match size_bytes
        // Convert size_bytes to MB for comparison
        let expected_delta_mb = (self.size_bytes / (1024 * 1024)) as i64;
        let actual_delta_mb = self.vram_delta_mb as i64;
        let delta_diff = (actual_delta_mb - expected_delta_mb).abs();

        // Allow tolerance for GPU overhead (rounding, fragmentation, etc.)
        delta_diff < Self::DELTA_TOLERANCE_MB
    }

    /// Panic if allocation appears simulated.
    ///
    /// # Panics
    ///
    /// Constitution AP-007 violation with error code EMB-E010 if `is_real()` returns false.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tracking = VramAllocationTracking::new(0x7fff_0000_1000, 104_857_600, 5000, 5100);
    /// tracking.assert_real(); // OK for real data
    ///
    /// // Would panic with "[EMB-E010] SIMULATION_DETECTED: ..."
    /// let fake = VramAllocationTracking {
    ///     base_ptr: 0x7f80_0000_0000, // FAKE POINTER
    ///     ..tracking
    /// };
    /// fake.assert_real(); // PANIC!
    /// ```
    pub fn assert_real(&self) {
        if !self.is_real() {
            panic!(
                "[EMB-E010] SIMULATION_DETECTED: VramAllocationTracking contains fake data. \
                 base_ptr=0x{:x}, size={}, delta={}MB. Constitution AP-007 violation.",
                self.base_ptr, self.size_bytes, self.vram_delta_mb
            );
        }
    }

    /// Get VRAM delta as human-readable string.
    ///
    /// # Returns
    ///
    /// Formatted string showing before/after/delta VRAM usage.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tracking = VramAllocationTracking::new(0x7fff_0000_1000, 104_857_600, 5000, 5100);
    /// assert_eq!(tracking.delta_display(), "100 MB (5000 -> 5100 MB)");
    /// ```
    #[must_use]
    pub fn delta_display(&self) -> String {
        format!(
            "{} MB ({} -> {} MB)",
            self.vram_delta_mb, self.vram_before_mb, self.vram_after_mb
        )
    }

    /// Get allocation size in megabytes.
    #[must_use]
    pub fn size_mb(&self) -> f64 {
        self.size_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get allocation size in gigabytes.
    #[must_use]
    pub fn size_gb(&self) -> f64 {
        self.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

// =============================================================================
// INFERENCE VALIDATION (TASK-EMB-016)
// =============================================================================

/// Inference validation result with golden reference comparison.
///
/// # Constitution Alignment
///
/// - AP-007: Output MUST NOT be sin wave or all zeros
/// - Validates model produces meaningful real output
///
/// # CRITICAL: No Simulation
///
/// The `is_real()` method detects fake inference patterns:
/// - Sin wave patterns: `(i * 0.001).sin()`
/// - All-zero outputs
/// - Low golden similarity (<0.95)
///
/// # Example
///
/// ```rust,ignore
/// use std::time::Duration;
/// use context_graph_embeddings::warm::loader::types::InferenceValidation;
///
/// // Real inference output (not a sin wave, not zeros)
/// let output: Vec<f32> = (0..768)
///     .map(|i| ((i * 17 + 42) % 1000) as f32 / 1000.0 - 0.5)
///     .collect();
///
/// let validation = InferenceValidation::new(
///     "The quick brown fox".to_string(),
///     output,
///     1.0,
///     Duration::from_millis(50),
///     true,
///     0.98,  // High golden similarity
/// );
///
/// assert!(validation.is_real());
/// validation.assert_real(); // Panics on fake patterns
/// ```
#[derive(Debug, Clone)]
pub struct InferenceValidation {
    /// Sample input used for validation (e.g., "The quick brown fox").
    ///
    /// # Invariant
    /// MUST NOT be empty. Real inference requires input.
    pub sample_input: String,

    /// Sample output (embedding vector).
    ///
    /// # Invariant
    /// MUST NOT be empty. MUST NOT be sin wave pattern or all zeros.
    pub sample_output: Vec<f32>,

    /// L2 norm of output (should be ~1.0 for normalized embeddings).
    pub output_norm: f32,

    /// Inference latency.
    pub latency: Duration,

    /// Whether output matches golden reference within tolerance.
    pub matches_golden: bool,

    /// Cosine similarity to golden reference (0.0 to 1.0).
    ///
    /// Must be > 0.95 for real inference to pass `is_real()`.
    pub golden_similarity: f32,
}

impl InferenceValidation {
    /// Minimum golden similarity required for `is_real()` to pass.
    pub const MIN_GOLDEN_SIMILARITY: f32 = 0.95;

    /// Maximum variance for sin wave detection (suspiciously smooth).
    pub const SIN_WAVE_VARIANCE_THRESHOLD: f32 = 0.0001;

    /// Minimum absolute value sum for non-zero detection.
    pub const ZERO_THRESHOLD: f32 = 1e-6;

    /// Create new InferenceValidation with fail-fast validation.
    ///
    /// # Arguments
    ///
    /// * `sample_input` - Text input used for validation (must be non-empty)
    /// * `sample_output` - Embedding output vector (must be non-empty)
    /// * `output_norm` - L2 norm of the output
    /// * `latency` - Inference duration
    /// * `matches_golden` - Whether output matches golden reference
    /// * `golden_similarity` - Cosine similarity to golden reference
    ///
    /// # Panics
    ///
    /// - If `sample_input` is empty
    /// - If `sample_output` is empty
    ///
    /// # Constitution: Fail-Fast
    ///
    /// Per AP-007, we panic immediately on invalid data.
    #[must_use]
    pub fn new(
        sample_input: String,
        sample_output: Vec<f32>,
        output_norm: f32,
        latency: Duration,
        matches_golden: bool,
        golden_similarity: f32,
    ) -> Self {
        assert!(
            !sample_input.is_empty(),
            "CONSTITUTION VIOLATION AP-007: sample_input is empty. \
             Real test input required."
        );
        assert!(
            !sample_output.is_empty(),
            "CONSTITUTION VIOLATION AP-007: sample_output is empty. \
             Real inference output required."
        );

        Self {
            sample_input,
            sample_output,
            output_norm,
            latency,
            matches_golden,
            golden_similarity,
        }
    }

    /// Check if output looks like real inference (not fake pattern).
    ///
    /// Detects:
    /// - Sin wave patterns (suspiciously smooth consecutive differences)
    /// - All-zero outputs
    /// - Low golden similarity (<0.95)
    ///
    /// # Returns
    ///
    /// `true` if output appears to be from real GPU inference.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Real output with varied values
    /// let real_output: Vec<f32> = (0..768)
    ///     .map(|i| ((i * 17 + 42) % 1000) as f32 / 1000.0 - 0.5)
    ///     .collect();
    /// let validation = InferenceValidation::new(
    ///     "test".to_string(), real_output, 1.0, Duration::from_millis(10), true, 0.99
    /// );
    /// assert!(validation.is_real());
    ///
    /// // Fake: sin wave pattern
    /// let sin_wave: Vec<f32> = (0..768).map(|i| (i as f32 * 0.001).sin()).collect();
    /// let fake = InferenceValidation::new(
    ///     "test".to_string(), sin_wave, 1.0, Duration::from_millis(10), true, 0.99
    /// );
    /// assert!(!fake.is_real()); // Detected as sin wave
    /// ```
    #[must_use]
    pub fn is_real(&self) -> bool {
        // Check 1: All zeros detection
        let is_zeros = self
            .sample_output
            .iter()
            .all(|&v| v.abs() < Self::ZERO_THRESHOLD);
        if is_zeros {
            return false;
        }

        // Check 2: Sin wave pattern detection
        // A sin wave has suspiciously smooth consecutive differences
        if self.detect_sin_wave_pattern() {
            return false;
        }

        // Check 3: Golden similarity must be high for real model
        if self.golden_similarity < Self::MIN_GOLDEN_SIMILARITY {
            return false;
        }

        true
    }

    /// Detect sin wave fake pattern.
    ///
    /// Sin waves have very low variance in their consecutive differences
    /// because the derivative of sin is cos, which is also smooth.
    fn detect_sin_wave_pattern(&self) -> bool {
        if self.sample_output.len() < 10 {
            return false;
        }

        // Check all windows of 10 elements for suspiciously smooth differences
        self.sample_output.windows(10).all(|w| {
            // Calculate consecutive differences
            let diffs: Vec<f32> = w.windows(2).map(|p| (p[1] - p[0]).abs()).collect();

            // Calculate variance of differences
            let mean: f32 = diffs.iter().sum::<f32>() / diffs.len() as f32;
            let variance: f32 =
                diffs.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / diffs.len() as f32;

            // Suspiciously smooth if variance is too low
            variance < Self::SIN_WAVE_VARIANCE_THRESHOLD
        })
    }

    /// Panic if output looks fake.
    ///
    /// # Panics
    ///
    /// Constitution AP-007 violation with error code EMB-E011 if `is_real()` returns false.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Real inference - OK
    /// let real = InferenceValidation::new(...);
    /// real.assert_real();
    ///
    /// // Fake sin wave - PANIC!
    /// let fake_sin = InferenceValidation { sample_output: sin_wave, .. };
    /// fake_sin.assert_real(); // PANIC: "[EMB-E011] FAKE_INFERENCE: ..."
    /// ```
    pub fn assert_real(&self) {
        if !self.is_real() {
            panic!(
                "[EMB-E011] FAKE_INFERENCE: Output pattern indicates simulation. \
                 Golden similarity: {:.4}, output_len: {}. Constitution AP-007 violation.",
                self.golden_similarity,
                self.sample_output.len()
            );
        }
    }

    /// Calculate L2 norm of sample_output for verification.
    ///
    /// # Returns
    ///
    /// L2 norm (Euclidean length) of the output vector.
    #[must_use]
    pub fn calculate_norm(&self) -> f32 {
        self.sample_output.iter().map(|v| v * v).sum::<f32>().sqrt()
    }

    /// Verify that stored output_norm matches calculated norm.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Maximum allowed difference between stored and calculated norm
    ///
    /// # Returns
    ///
    /// `true` if stored norm matches calculated norm within tolerance.
    #[must_use]
    pub fn verify_norm(&self, tolerance: f32) -> bool {
        let calculated = self.calculate_norm();
        (self.output_norm - calculated).abs() < tolerance
    }

    /// Get output dimension.
    #[must_use]
    pub fn output_dimension(&self) -> usize {
        self.sample_output.len()
    }
}

// =============================================================================
// COMPILE-TIME ASSERTIONS
// =============================================================================

/// Compile-time check: Checksum size must be 32 bytes (SHA256)
const _: () = assert!(
    std::mem::size_of::<[u8; 32]>() == 32,
    "Checksum must be exactly 32 bytes for SHA256"
);

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // HELPER FUNCTIONS
    // =========================================================================

    fn create_test_metadata() -> TensorMetadata {
        let mut shapes = HashMap::new();
        shapes.insert("embeddings.weight".to_string(), vec![30522, 768]);
        shapes.insert("encoder.layer.0.weight".to_string(), vec![768, 768]);
        // Total: 30522*768 + 768*768 = 23_440_896 + 589_824 = 24_030_720
        TensorMetadata::new(shapes, DType::F32, 24_030_720)
    }

    // =========================================================================
    // FAIL-FAST VALIDATION TESTS (TensorMetadata)
    // =========================================================================

    #[test]
    #[should_panic(expected = "CONSTITUTION VIOLATION AP-007: shapes is empty")]
    fn test_tensor_metadata_rejects_empty_shapes() {
        let _ = TensorMetadata::new(
            HashMap::new(), // EMPTY - MUST PANIC
            DType::F32,
            100,
        );
    }

    #[test]
    #[should_panic(expected = "CONSTITUTION VIOLATION AP-007: total_params is 0")]
    fn test_tensor_metadata_rejects_zero_params() {
        let _ = TensorMetadata::new(
            [("test".to_string(), vec![100, 768])].into_iter().collect(),
            DType::F32,
            0, // ZERO PARAMS - MUST PANIC
        );
    }

    // =========================================================================
    // FAIL-FAST VALIDATION TESTS (WarmLoadResult)
    // =========================================================================

    #[test]
    #[should_panic(expected = "CONSTITUTION VIOLATION AP-007: gpu_ptr is null")]
    fn test_warm_load_result_rejects_null_pointer() {
        let metadata = create_test_metadata();
        let _ = WarmLoadResult::new(
            0, // NULL POINTER - MUST PANIC
            [1u8; 32],
            1024,
            Duration::from_millis(100),
            metadata,
        );
    }

    #[test]
    #[should_panic(expected = "CONSTITUTION VIOLATION AP-007: checksum is all zeros")]
    fn test_warm_load_result_rejects_zero_checksum() {
        let metadata = create_test_metadata();
        let _ = WarmLoadResult::new(
            0x7fff_0000_1000,
            [0u8; 32], // ZERO CHECKSUM - MUST PANIC
            1024,
            Duration::from_millis(100),
            metadata,
        );
    }

    #[test]
    #[should_panic(expected = "CONSTITUTION VIOLATION AP-007: size_bytes is 0")]
    fn test_warm_load_result_rejects_zero_size() {
        let metadata = create_test_metadata();
        let _ = WarmLoadResult::new(
            0x7fff_0000_1000,
            [1u8; 32],
            0, // ZERO SIZE - MUST PANIC
            Duration::from_millis(100),
            metadata,
        );
    }

    // =========================================================================
    // FAIL-FAST VALIDATION TESTS (LoadedModelWeights)
    // =========================================================================

    #[test]
    #[should_panic(expected = "CONSTITUTION VIOLATION AP-007: model_id is empty")]
    fn test_loaded_model_weights_rejects_empty_model_id() {
        // Create a real GpuTensor for testing (requires GPU, so we skip in unit tests)
        // In practice, this test would be an integration test with GPU access
        // For unit testing, we verify the panic message is correct

        // Since we can't easily create a GpuTensor without GPU,
        // we test that the first assertion (model_id) fires before tensors check
        let tensors: HashMap<String, GpuTensor> = HashMap::new();

        let _ = LoadedModelWeights::new(
            "".to_string(), // EMPTY - MUST PANIC
            tensors,        // Empty too, but model_id check comes first
            [1u8; 32],
            1024,
            0,
        );
    }

    #[test]
    #[should_panic(expected = "CONSTITUTION VIOLATION AP-007: tensors is empty")]
    fn test_loaded_model_weights_rejects_empty_tensors() {
        let _ = LoadedModelWeights::new(
            "E1_Semantic".to_string(),
            HashMap::new(), // EMPTY - MUST PANIC
            [1u8; 32],
            1024,
            0,
        );
    }

    // NOTE: test_loaded_model_weights_rejects_zero_checksum cannot be tested in unit tests
    // because we cannot create a GpuTensor without GPU hardware. The tensors.is_empty()
    // check fires before the checksum check. This validation is documented and would
    // fire correctly in integration tests with actual GPU tensors.
    //
    // The expected panic message is: "CONSTITUTION VIOLATION AP-007: file_checksum is all zeros"
    // This will be verified in integration tests (TASK-EMB-006-integration).

    // =========================================================================
    // VALID DATA TESTS
    // =========================================================================

    #[test]
    fn test_tensor_metadata_accepts_valid_data() {
        let metadata = create_test_metadata();

        assert_eq!(metadata.tensor_count(), 2);
        assert!(metadata.total_params > 0);
        assert_eq!(metadata.dtype, DType::F32);
    }

    #[test]
    fn test_tensor_metadata_calculates_params() {
        let metadata = TensorMetadata::new(
            [
                ("layer1".to_string(), vec![768, 768]),  // 589,824
                ("layer2".to_string(), vec![768, 3072]), // 2,359,296
            ]
            .into_iter()
            .collect(),
            DType::F32,
            2_949_120, // Sum of above
        );

        assert!(metadata.verify_params());
        assert_eq!(metadata.calculate_total_params(), 2_949_120);
    }

    #[test]
    fn test_tensor_metadata_calculates_params_mismatch() {
        let metadata = TensorMetadata::new(
            [
                ("layer1".to_string(), vec![768, 768]), // 589,824
            ]
            .into_iter()
            .collect(),
            DType::F32,
            1_000_000, // Wrong value
        );

        assert!(!metadata.verify_params());
        assert_eq!(metadata.calculate_total_params(), 589_824);
    }

    #[test]
    fn test_warm_load_result_accepts_valid_data() {
        let metadata = TensorMetadata::new(
            [("embeddings.weight".to_string(), vec![30522, 768])]
                .into_iter()
                .collect(),
            DType::F32,
            23_440_896, // 30522 * 768
        );

        let result = WarmLoadResult::new(
            0x7fff_0000_1000, // Real-looking pointer
            [0xAB; 32],       // Non-zero checksum
            93_763_584,       // 23M params * 4 bytes
            Duration::from_millis(150),
            metadata,
        );

        assert!(result.gpu_ptr != 0);
        assert!(result.size_bytes > 0);
        assert_eq!(result.checksum, [0xAB; 32]);
    }

    #[test]
    fn test_checksum_verification() {
        let expected = [0xAB; 32];
        let metadata = TensorMetadata::new(
            [("test".to_string(), vec![100])].into_iter().collect(),
            DType::F32,
            100,
        );

        let result = WarmLoadResult::new(
            0x7fff_0000_1000,
            expected,
            400,
            Duration::from_millis(10),
            metadata,
        );

        assert!(result.verify_checksum(&expected));
        assert!(!result.verify_checksum(&[0xCD; 32]));
    }

    #[test]
    fn test_checksum_hex_conversion() {
        let checksum = [
            0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC,
            0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC,
            0xDD, 0xEE, 0xFF, 0x00,
        ];

        let metadata = TensorMetadata::new(
            [("test".to_string(), vec![100])].into_iter().collect(),
            DType::F32,
            100,
        );

        let result = WarmLoadResult::new(
            0x7fff_0000_1000,
            checksum,
            400,
            Duration::from_millis(10),
            metadata,
        );

        let hex = result.checksum_hex();
        assert_eq!(hex.len(), 64); // 32 bytes * 2 chars per byte
        assert!(hex.starts_with("deadbeef"));
    }

    #[test]
    fn test_tensor_metadata_get_shape() {
        let metadata = create_test_metadata();

        let shape = metadata.get_shape("embeddings.weight");
        assert!(shape.is_some());
        assert_eq!(shape.unwrap(), &vec![30522, 768]);

        let missing = metadata.get_shape("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_bytes_per_param() {
        let metadata = TensorMetadata::new(
            [("test".to_string(), vec![100])].into_iter().collect(),
            DType::F32,
            100,
        );

        let result = WarmLoadResult::new(
            0x7fff_0000_1000,
            [1u8; 32],
            400,
            Duration::from_millis(10),
            metadata,
        );

        assert_eq!(result.bytes_per_param(), 4); // F32 = 4 bytes

        // Test with F16
        let metadata_f16 = TensorMetadata::new(
            [("test".to_string(), vec![100])].into_iter().collect(),
            DType::F16,
            100,
        );

        let result_f16 = WarmLoadResult::new(
            0x7fff_0000_1000,
            [1u8; 32],
            200,
            Duration::from_millis(10),
            metadata_f16,
        );

        assert_eq!(result_f16.bytes_per_param(), 2); // F16 = 2 bytes
    }

    // =========================================================================
    // COMPILE-TIME ASSERTION VERIFICATION
    // =========================================================================

    #[test]
    fn test_checksum_size() {
        // Verify at runtime that checksum is correct size (compile-time assertion above)
        assert_eq!(std::mem::size_of::<[u8; 32]>(), 32);
    }

    // =========================================================================
    // VRAM ALLOCATION TRACKING TESTS (TASK-EMB-016)
    // =========================================================================

    /// Edge Case 1: VRAM Delta Mismatch
    ///
    /// Scenario: GPU reports delta that differs from allocation size by >50MB
    /// Expected: `VramAllocationTracking::is_real()` returns `false`
    #[test]
    fn test_vram_allocation_detects_delta_mismatch() {
        // Create allocation with massive mismatch: 1KB allocation claims 1000MB delta
        let alloc = VramAllocationTracking {
            base_ptr: 0x7fff_0000_1000, // Real-looking pointer
            size_bytes: 1024,           // 1KB allocated
            vram_before_mb: 1000,
            vram_after_mb: 2000, // Claims 1000MB delta for 1KB!
            vram_delta_mb: 1000,
        };

        assert!(
            !alloc.is_real(),
            "Should detect VRAM delta mismatch: 1KB allocation cannot cause 1000MB delta"
        );

        // Verify the detection is due to delta mismatch, not pointer
        assert_ne!(alloc.base_ptr, VramAllocationTracking::FAKE_POINTER);
    }

    #[test]
    fn test_vram_allocation_detects_fake_pointer() {
        // Use the known fake pointer value
        let alloc = VramAllocationTracking {
            base_ptr: VramAllocationTracking::FAKE_POINTER, // 0x7f80_0000_0000
            size_bytes: 104_857_600,                        // 100MB
            vram_before_mb: 5000,
            vram_after_mb: 5100,
            vram_delta_mb: 100,
        };

        assert!(
            !alloc.is_real(),
            "Should detect fake pointer 0x7f80_0000_0000"
        );
    }

    #[test]
    fn test_vram_allocation_accepts_valid_delta() {
        // Create allocation with matching delta: 100MB allocation with 100MB delta
        let alloc = VramAllocationTracking::new(
            0x7fff_0000_1000, // Real pointer
            104_857_600,      // 100MB
            5000,             // 5GB before
            5100,             // 5.1GB after (100MB delta)
        );

        assert!(
            alloc.is_real(),
            "Should accept valid VRAM allocation with matching delta"
        );
        assert_eq!(alloc.vram_delta_mb, 100);
    }

    #[test]
    fn test_vram_allocation_tolerates_small_overhead() {
        // Create allocation with small overhead (within 50MB tolerance)
        let alloc = VramAllocationTracking::new(
            0x7fff_0000_1000,
            104_857_600, // 100MB allocation
            5000,
            5130, // 130MB delta (30MB overhead)
        );

        // 100MB allocation with 130MB delta = 30MB difference, within 50MB tolerance
        assert!(
            alloc.is_real(),
            "Should accept allocation with small GPU overhead within 50MB tolerance"
        );
    }

    #[test]
    fn test_vram_allocation_rejects_excessive_overhead() {
        // Create allocation with excessive overhead (>50MB tolerance)
        let alloc = VramAllocationTracking {
            base_ptr: 0x7fff_0000_1000,
            size_bytes: 104_857_600, // 100MB allocation
            vram_before_mb: 5000,
            vram_after_mb: 5200, // 200MB delta (100MB overhead)
            vram_delta_mb: 200,
        };

        // 100MB allocation with 200MB delta = 100MB difference, exceeds 50MB tolerance
        assert!(
            !alloc.is_real(),
            "Should reject allocation with excessive overhead >50MB tolerance"
        );
    }

    #[test]
    fn test_vram_allocation_delta_display() {
        let alloc = VramAllocationTracking::new(0x7fff_0000_1000, 104_857_600, 5000, 5100);

        let display = alloc.delta_display();
        assert_eq!(display, "100 MB (5000 -> 5100 MB)");
    }

    #[test]
    fn test_vram_allocation_size_conversions() {
        let alloc = VramAllocationTracking::new(
            0x7fff_0000_1000,
            1_073_741_824, // 1GB
            5000,
            6024, // ~1GB delta
        );

        // Verify size_mb
        let size_mb = alloc.size_mb();
        assert!((size_mb - 1024.0).abs() < 0.1, "1GB should be ~1024MB");

        // Verify size_gb
        let size_gb = alloc.size_gb();
        assert!((size_gb - 1.0).abs() < 0.01, "1GB should be ~1.0GB");
    }

    // =========================================================================
    // VRAM ALLOCATION FAIL-FAST TESTS
    // =========================================================================

    #[test]
    #[should_panic(expected = "CONSTITUTION VIOLATION AP-007: base_ptr is null")]
    fn test_vram_allocation_rejects_null_ptr() {
        let _ = VramAllocationTracking::new(0, 1024, 1000, 1100);
    }

    #[test]
    #[should_panic(expected = "CONSTITUTION VIOLATION AP-007: size_bytes is 0")]
    fn test_vram_allocation_rejects_zero_size() {
        let _ = VramAllocationTracking::new(0x7fff_0000_1000, 0, 1000, 1100);
    }

    #[test]
    #[should_panic(expected = "[EMB-E010] SIMULATION_DETECTED")]
    fn test_vram_allocation_assert_real_panics_on_fake_pointer() {
        let fake_alloc = VramAllocationTracking {
            base_ptr: VramAllocationTracking::FAKE_POINTER, // KNOWN FAKE POINTER
            size_bytes: 1024,
            vram_before_mb: 1000,
            vram_after_mb: 1001,
            vram_delta_mb: 1,
        };

        fake_alloc.assert_real();
    }

    #[test]
    #[should_panic(expected = "[EMB-E010] SIMULATION_DETECTED")]
    fn test_vram_allocation_assert_real_panics_on_delta_mismatch() {
        let fake_alloc = VramAllocationTracking {
            base_ptr: 0x7fff_0000_1000, // Valid pointer
            size_bytes: 1024,           // 1KB
            vram_before_mb: 1000,
            vram_after_mb: 2000, // 1000MB delta for 1KB!
            vram_delta_mb: 1000,
        };

        fake_alloc.assert_real();
    }

    // =========================================================================
    // INFERENCE VALIDATION TESTS (TASK-EMB-016)
    // =========================================================================

    /// Edge Case 2: Sin Wave Pattern Detection
    ///
    /// Scenario: Inference output follows perfect mathematical pattern (sin wave)
    /// Expected: `InferenceValidation::is_real()` returns `false`
    #[test]
    fn test_inference_validation_detects_sin_wave() {
        // Generate sin wave output: (i * 0.001).sin()
        let sin_wave_output: Vec<f32> = (0..768).map(|i| (i as f32 * 0.001).sin()).collect();

        let validation = InferenceValidation {
            sample_input: "test input".to_string(),
            sample_output: sin_wave_output,
            output_norm: 1.0,
            latency: Duration::from_millis(10),
            matches_golden: true,
            golden_similarity: 0.99, // High similarity, but sin wave pattern
        };

        assert!(!validation.is_real(), "Should detect sin wave fake pattern");
    }

    /// Edge Case 3: Low Golden Similarity
    ///
    /// Scenario: Inference output has low cosine similarity to golden reference (<0.95)
    /// Expected: `InferenceValidation::is_real()` returns `false`
    #[test]
    fn test_inference_validation_rejects_low_golden() {
        // Generate realistic (non-sin-wave) output
        let output: Vec<f32> = (0..10)
            .map(|i| ((i * 17 + 42) % 1000) as f32 / 1000.0 - 0.5)
            .collect();

        let validation = InferenceValidation::new(
            "The quick brown fox".to_string(),
            output,
            1.0,
            Duration::from_millis(50),
            false,
            0.50, // LOW golden similarity - REJECT
        );

        assert!(
            !validation.is_real(),
            "Should reject low golden similarity (0.50 < 0.95)"
        );
    }

    #[test]
    fn test_inference_validation_detects_all_zeros() {
        let validation = InferenceValidation {
            sample_input: "test".to_string(),
            sample_output: vec![0.0; 768], // ALL ZEROS
            output_norm: 0.0,
            latency: Duration::from_millis(10),
            matches_golden: false,
            golden_similarity: 0.99, // High similarity but zeros
        };

        assert!(
            !validation.is_real(),
            "Should detect all-zero output as fake"
        );
    }

    #[test]
    fn test_inference_validation_accepts_high_golden() {
        // Generate non-sin-wave realistic output with high variance
        let output: Vec<f32> = (0..768)
            .map(|i| ((i * 17 + 42) % 1000) as f32 / 1000.0 - 0.5)
            .collect();

        let validation = InferenceValidation::new(
            "The quick brown fox".to_string(),
            output,
            1.0,
            Duration::from_millis(50),
            true,
            0.98, // HIGH golden similarity - ACCEPT
        );

        assert!(
            validation.is_real(),
            "Should accept high golden similarity with non-sin-wave output"
        );
    }

    #[test]
    fn test_inference_validation_accepts_borderline_golden() {
        // Generate realistic output
        let output: Vec<f32> = (0..768)
            .map(|i| ((i * 31 + 7) % 1000) as f32 / 1000.0 - 0.5)
            .collect();

        let validation = InferenceValidation::new(
            "Test borderline".to_string(),
            output,
            1.0,
            Duration::from_millis(50),
            true,
            0.96, // Just above 0.95 threshold
        );

        assert!(
            validation.is_real(),
            "Should accept golden similarity at 0.96 (just above 0.95 threshold)"
        );
    }

    #[test]
    fn test_inference_validation_rejects_borderline_golden() {
        // Generate realistic output
        let output: Vec<f32> = (0..768)
            .map(|i| ((i * 31 + 7) % 1000) as f32 / 1000.0 - 0.5)
            .collect();

        let validation = InferenceValidation::new(
            "Test borderline".to_string(),
            output,
            1.0,
            Duration::from_millis(50),
            false,
            0.94, // Just below 0.95 threshold
        );

        assert!(
            !validation.is_real(),
            "Should reject golden similarity at 0.94 (just below 0.95 threshold)"
        );
    }

    #[test]
    fn test_inference_validation_calculate_norm() {
        let output = vec![3.0, 4.0]; // L2 norm = 5.0

        let validation = InferenceValidation::new(
            "test".to_string(),
            output,
            5.0,
            Duration::from_millis(10),
            true,
            0.99,
        );

        let calculated = validation.calculate_norm();
        assert!(
            (calculated - 5.0).abs() < 0.001,
            "L2 norm of [3, 4] should be 5"
        );
    }

    #[test]
    fn test_inference_validation_verify_norm() {
        let output = vec![3.0, 4.0]; // L2 norm = 5.0

        let validation = InferenceValidation::new(
            "test".to_string(),
            output,
            5.0, // Correct norm
            Duration::from_millis(10),
            true,
            0.99,
        );

        assert!(
            validation.verify_norm(0.01),
            "Stored norm should match calculated"
        );

        // Test with wrong stored norm
        let validation_wrong = InferenceValidation {
            output_norm: 10.0, // Wrong!
            ..validation.clone()
        };

        assert!(
            !validation_wrong.verify_norm(0.01),
            "Wrong stored norm should not match"
        );
    }

    #[test]
    fn test_inference_validation_output_dimension() {
        let output = vec![0.1; 768];

        let validation = InferenceValidation::new(
            "test".to_string(),
            output,
            1.0,
            Duration::from_millis(10),
            true,
            0.99,
        );

        assert_eq!(validation.output_dimension(), 768);
    }

    // =========================================================================
    // INFERENCE VALIDATION FAIL-FAST TESTS
    // =========================================================================

    #[test]
    #[should_panic(expected = "CONSTITUTION VIOLATION AP-007: sample_input is empty")]
    fn test_inference_validation_rejects_empty_input() {
        let _ = InferenceValidation::new(
            "".to_string(),
            vec![0.1, 0.2, 0.3],
            1.0,
            Duration::from_millis(10),
            true,
            0.99,
        );
    }

    #[test]
    #[should_panic(expected = "CONSTITUTION VIOLATION AP-007: sample_output is empty")]
    fn test_inference_validation_rejects_empty_output() {
        let _ = InferenceValidation::new(
            "test".to_string(),
            vec![],
            0.0,
            Duration::from_millis(10),
            false,
            0.0,
        );
    }

    #[test]
    #[should_panic(expected = "[EMB-E011] FAKE_INFERENCE")]
    fn test_inference_validation_assert_real_panics_on_zeros() {
        let fake_validation = InferenceValidation {
            sample_input: "test".to_string(),
            sample_output: vec![0.0; 768], // ALL ZEROS
            output_norm: 0.0,
            latency: Duration::from_millis(10),
            matches_golden: false,
            golden_similarity: 0.1, // LOW
        };

        fake_validation.assert_real();
    }

    #[test]
    #[should_panic(expected = "[EMB-E011] FAKE_INFERENCE")]
    fn test_inference_validation_assert_real_panics_on_sin_wave() {
        let sin_wave: Vec<f32> = (0..768).map(|i| (i as f32 * 0.001).sin()).collect();

        let fake_validation = InferenceValidation {
            sample_input: "test".to_string(),
            sample_output: sin_wave,
            output_norm: 1.0,
            latency: Duration::from_millis(10),
            matches_golden: true,
            golden_similarity: 0.99,
        };

        fake_validation.assert_real();
    }

    #[test]
    #[should_panic(expected = "[EMB-E011] FAKE_INFERENCE")]
    fn test_inference_validation_assert_real_panics_on_low_golden() {
        let output: Vec<f32> = (0..768)
            .map(|i| ((i * 17 + 42) % 1000) as f32 / 1000.0 - 0.5)
            .collect();

        let fake_validation = InferenceValidation {
            sample_input: "test".to_string(),
            sample_output: output,
            output_norm: 1.0,
            latency: Duration::from_millis(10),
            matches_golden: false,
            golden_similarity: 0.50, // LOW
        };

        fake_validation.assert_real();
    }

    // =========================================================================
    // REAL DATA PATTERN ACCEPTANCE TESTS
    // =========================================================================

    #[test]
    fn test_inference_validation_accepts_noisy_realistic_output() {
        // Generate highly varied output that simulates real model output
        use std::f32::consts::PI;

        let output: Vec<f32> = (0..768)
            .map(|i| {
                // Complex formula that produces realistic variation
                let base = ((i * 17) % 1000) as f32 / 1000.0;
                let noise = ((i * 31 + 7) % 100) as f32 / 1000.0;
                let periodic = (i as f32 * PI / 50.0).sin() * 0.1;
                base + noise + periodic - 0.5
            })
            .collect();

        let validation = InferenceValidation::new(
            "The quick brown fox jumps over the lazy dog".to_string(),
            output,
            1.0,
            Duration::from_millis(45),
            true,
            0.97,
        );

        assert!(
            validation.is_real(),
            "Should accept realistic noisy output with high golden similarity"
        );
    }

    #[test]
    fn test_vram_allocation_realistic_model_sizes() {
        // Test with realistic model sizes
        let sizes = [
            (384 * 1024 * 1024, 384),       // 384MB - small model
            (1024 * 1024 * 1024, 1024),     // 1GB - medium model
            (4 * 1024 * 1024 * 1024, 4096), // 4GB - large model
        ];

        for (size_bytes, expected_delta_mb) in sizes {
            let alloc = VramAllocationTracking::new(
                0x7fff_0000_1000 + size_bytes as u64, // Vary pointer
                size_bytes,
                5000,
                5000 + expected_delta_mb,
            );

            assert!(
                alloc.is_real(),
                "Should accept realistic model size: {} bytes",
                size_bytes
            );
        }
    }
}
