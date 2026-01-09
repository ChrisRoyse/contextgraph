//! Error types and exit codes for the warm model loading system.
//!
//! # Exit Code Mapping (REQ-WARM-008 through REQ-WARM-011)
//!
//! | Exit Code | Error Variant | Description |
//! |-----------|---------------|-------------|
//! | 101 | `CudaUnavailable`, `ModelFileMissing` | CUDA unavailable (AP-007) or model file not found |
//! | 102 | `ModelLoadFailed` | Model loading failed |
//! | 103 | `ModelValidationFailed` | Model validation failed |
//! | 104 | `VramInsufficientTotal` | Insufficient VRAM for models |
//! | 105 | `VramInsufficientHeadroom` | Insufficient VRAM headroom |
//! | 106 | `CudaInitFailed` | CUDA initialization failed |
//! | 107 | `CudaCapabilityInsufficient` | GPU compute capability too low |
//! | 108 | `CudaAllocFailed` | CUDA allocation failed |
//! | 109 | `FakeAllocationDetected` | Fake GPU memory allocation detected (AP-007) |
//! | 110 | `SinWaveOutputDetected` | Sin wave pattern in output (AP-007) |
//! | 111 | `WeightFileMissing` | SafeTensors weight file not found |
//! | 112 | `WeightFileCorrupted` | SafeTensors file failed to parse |
//! | 113 | `WeightChecksumMismatch` | SHA256 checksum verification failed |
//! | 114 | `InferenceInitFailed` | Inference initialization failed |
//! | 115 | `InferenceFailed` | Inference execution failed |
//! | 116 | `ModelDimensionMismatch` | Model output dimension mismatch |
//!
//! # Design Principles
//!
//! - **NO FALLBACKS**: Fatal errors terminate startup immediately
//! - **COMPREHENSIVE LOGGING**: All errors include diagnostic context
//! - **MACHINE READABLE**: Exit codes enable orchestration integration

use thiserror::Error;

/// Error type for warm loading operations.
///
/// Each variant includes diagnostic information for debugging.
/// Fatal startup errors (exit codes 101-116) prevent inference requests.
#[derive(Debug, Error)]
pub enum WarmError {
    // === Exit Code 101: Model File Missing ===
    /// Model weight file not found at expected path.
    #[error("Model file missing: {model_id} not found at {path}")]
    ModelFileMissing {
        /// The model identifier (e.g., "E1_Semantic").
        model_id: String,
        /// The filesystem path that was searched.
        path: String,
    },

    // === Exit Code 102: Model Load Failed ===
    /// Model loading failed during weight parsing or transfer.
    #[error("Model load failed for {model_id}: {reason}")]
    ModelLoadFailed {
        /// The model identifier.
        model_id: String,
        /// Human-readable failure reason.
        reason: String,
        /// Bytes successfully read before failure.
        bytes_read: usize,
        /// Expected total file size in bytes.
        file_size: usize,
    },

    // === Exit Code 103: Model Validation Failed ===
    /// Model validation failed after loading (NaN detection, inference test).
    #[error("Model validation failed for {model_id}: {reason}")]
    ModelValidationFailed {
        /// The model identifier.
        model_id: String,
        /// Description of validation failure.
        reason: String,
        /// Expected validation output (if applicable).
        expected_output: Option<String>,
        /// Actual validation output (if applicable).
        actual_output: Option<String>,
    },

    // === Exit Code 104: VRAM Insufficient Total ===
    /// Insufficient total VRAM for loading model weights.
    #[error("Insufficient VRAM: required {required_gb:.2}GB, available {available_gb:.2}GB")]
    VramInsufficientTotal {
        /// Total VRAM required for all models in bytes.
        required_bytes: usize,
        /// Available VRAM on GPU in bytes.
        available_bytes: usize,
        /// Required VRAM in gigabytes.
        required_gb: f64,
        /// Available VRAM in gigabytes.
        available_gb: f64,
        /// Per-model memory requirements: (model_id, bytes).
        model_breakdown: Vec<(String, usize)>,
    },

    // === Exit Code 105: VRAM Insufficient Headroom ===
    /// Insufficient VRAM headroom for working memory.
    #[error("Insufficient headroom: models {model_gb:.2}GB, available {available_gb:.2}GB, headroom required {headroom_gb:.2}GB")]
    VramInsufficientHeadroom {
        /// Total VRAM for model weights in bytes.
        model_bytes: usize,
        /// Total available VRAM in bytes.
        available_bytes: usize,
        /// Required headroom for working memory in bytes.
        headroom_required: usize,
        /// Model weights in gigabytes.
        model_gb: f64,
        /// Available VRAM in gigabytes.
        available_gb: f64,
        /// Required headroom in gigabytes.
        headroom_gb: f64,
    },

    // === Exit Code 106: CUDA Initialization Failed ===
    /// CUDA runtime initialization failed (driver issues, no GPU, exclusive mode).
    #[error("CUDA initialization failed: {cuda_error}")]
    CudaInitFailed {
        /// Raw CUDA error message.
        cuda_error: String,
        /// Detected driver version (may be empty).
        driver_version: String,
        /// Detected GPU name (may be empty).
        gpu_name: String,
    },

    // === Exit Code 101: CUDA Unavailable ===
    /// CUDA feature or real GPU is unavailable - Constitution AP-007 violation.
    ///
    /// # Constitution Compliance
    ///
    /// AP-007: Stub data in prod → use tests/fixtures/
    /// This error is returned when:
    /// - CUDA feature is not enabled at compile time
    /// - A simulated/stub GPU is detected at runtime
    ///
    /// # Error Code: EMB-E001
    #[error("[EMB-E001] CUDA_UNAVAILABLE: {message}")]
    CudaUnavailable {
        /// Human-readable message explaining why CUDA is unavailable.
        message: String,
    },

    // === Exit Code 107: CUDA Capability Insufficient ===
    /// GPU compute capability below required minimum (12.0 for RTX 5090).
    #[error("GPU compute capability {actual_cc} insufficient, required {required_cc}")]
    CudaCapabilityInsufficient {
        /// Actual compute capability (e.g., "8.6").
        actual_cc: String,
        /// Required compute capability (e.g., "12.0").
        required_cc: String,
        /// GPU model name.
        gpu_name: String,
    },

    // === Exit Code 108: CUDA Allocation Failed ===
    /// CUDA memory allocation failed (fragmentation, race condition).
    #[error("CUDA allocation failed: requested {requested_bytes} bytes")]
    CudaAllocFailed {
        /// Bytes requested in allocation.
        requested_bytes: usize,
        /// Raw CUDA error message.
        cuda_error: String,
        /// Free VRAM at failure (if query succeeded).
        vram_free: Option<usize>,
        /// Recent allocation history for debugging.
        allocation_history: Vec<String>,
    },

    // === Exit Code 109: Fake Allocation Detected ===
    /// Fake/mock GPU memory allocation detected - Constitution AP-007 violation.
    ///
    /// # Constitution Compliance
    ///
    /// AP-007: Mock/stub data is FORBIDDEN in production.
    /// Fake allocations (matching pattern 0x7f80_0000_0000) MUST cause immediate process exit.
    #[error("[EMB-E009] FAKE_ALLOCATION_DETECTED: Fake GPU memory pointer detected at 0x{detected_address:016x}, expected real CUDA allocation")]
    FakeAllocationDetected {
        /// The fake device pointer that was detected.
        detected_address: u64,
        /// Tensor name that received the fake allocation.
        tensor_name: String,
        /// Expected base address pattern for real allocations.
        expected_pattern: String,
    },

    // === Exit Code 110: Sin Wave Output Detected ===
    /// Sin wave pattern detected in model output - indicates mock/fake inference.
    ///
    /// # Constitution Compliance
    ///
    /// AP-007: Mock/stub data is FORBIDDEN in production.
    /// Sin wave outputs indicate fake inference and MUST cause immediate process exit.
    ///
    /// # Detection Algorithm
    ///
    /// FFT analysis shows >80% energy concentrated in single frequency band.
    #[error("[EMB-E010] SIN_WAVE_OUTPUT_DETECTED: Model output contains sin wave pattern (dominant_freq={dominant_frequency_hz:.2}Hz, energy_concentration={energy_concentration:.2}%)")]
    SinWaveOutputDetected {
        /// The model identifier.
        model_id: String,
        /// Dominant frequency detected in output.
        dominant_frequency_hz: f32,
        /// Percentage of energy in dominant frequency band.
        energy_concentration: f32,
        /// Number of output elements analyzed.
        output_size: usize,
    },

    // === Exit Code 116: Model Dimension Mismatch ===
    /// Model output dimension does not match expected value.
    ///
    /// Note: This was formerly exit code 110 but was reassigned to make room
    /// for SinWaveOutputDetected per TASK-EMB-017.
    #[error("Model dimension mismatch for {model_id}: expected {expected}, got {actual}")]
    ModelDimensionMismatch {
        /// The model identifier.
        model_id: String,
        /// Expected output dimension.
        expected: usize,
        /// Actual output dimension.
        actual: usize,
    },

    // === Exit Code 111: Weight File Missing ===
    /// SafeTensors weight file not found at expected path.
    /// Different from ModelFileMissing as this is specifically for weight file loading.
    #[error("Weight file missing for {model_id}: {path:?}")]
    WeightFileMissing {
        /// The model identifier (e.g., "E1_Semantic").
        model_id: String,
        /// The filesystem path that was searched.
        path: std::path::PathBuf,
    },

    // === Exit Code 112: Weight File Corrupted ===
    /// SafeTensors file failed to parse (invalid format, corrupted data).
    #[error("Weight file corrupted for {model_id} at {path:?}: {reason}")]
    WeightFileCorrupted {
        /// The model identifier.
        model_id: String,
        /// The filesystem path.
        path: std::path::PathBuf,
        /// Human-readable parse error.
        reason: String,
    },

    // === Exit Code 113: Weight Checksum Mismatch ===
    /// SHA256 checksum verification failed.
    #[error("Weight checksum mismatch for {model_id}: expected {expected}, got {actual}")]
    WeightChecksumMismatch {
        /// The model identifier.
        model_id: String,
        /// Expected SHA256 hex string.
        expected: String,
        /// Actual computed SHA256 hex string.
        actual: String,
    },

    // === Non-Fatal Errors (Exit Code 1) ===
    /// Model already registered (programming error).
    #[error("Model already registered: {model_id}")]
    ModelAlreadyRegistered { model_id: String },

    /// Model not found in registry.
    #[error("Model not registered: {model_id}")]
    ModelNotRegistered { model_id: String },

    /// Invalid configuration value.
    #[error("Invalid configuration: {field} - {reason}")]
    InvalidConfig { field: String, reason: String },

    /// Registry lock poisoned (thread panic while holding lock).
    #[error("Registry lock poisoned")]
    RegistryLockPoisoned,

    /// Working memory pool exhausted (non-fatal).
    #[error("Working memory exhausted: requested {requested_bytes}, available {available_bytes}")]
    WorkingMemoryExhausted {
        requested_bytes: usize,
        available_bytes: usize,
    },

    /// CUDA feature not enabled in build.
    #[error("CUDA not available (feature disabled)")]
    CudaNotAvailable,

    /// CUDA query operation failed (non-fatal).
    #[error("CUDA query failed: {error}")]
    CudaQueryFailed { error: String },

    /// Diagnostic dump write failed (non-fatal, best-effort).
    #[error("Diagnostic dump failed: {reason}")]
    DiagnosticDumpFailed { reason: String },

    /// Model loading timed out.
    #[error("Timeout loading model {model_id} after {timeout_ms}ms")]
    LoadTimeout { model_id: String, timeout_ms: u64 },

    /// VRAM allocation failed (generic, for pool isolation).
    #[error("VRAM allocation failed: requested {requested_bytes}, available {available_bytes} - {error}")]
    VramAllocationFailed {
        requested_bytes: usize,
        available_bytes: usize,
        error: String,
    },

    // === Exit Code 114: Inference Initialization Failed ===
    /// Inference initialization failed.
    /// Error code: EMB-E011
    #[error("[EMB-E011] Inference initialization failed for {model_id}: {reason}")]
    InferenceInitFailed {
        /// The model identifier.
        model_id: String,
        /// Human-readable failure reason.
        reason: String,
    },

    // === Exit Code 115: Inference Failed ===
    /// Inference execution failed.
    /// Error code: EMB-E011
    #[error(
        "[EMB-E011] Inference failed for {model_id}: {reason} (input_hash=0x{input_hash:016x})"
    )]
    InferenceFailed {
        /// The model identifier.
        model_id: String,
        /// Human-readable failure reason.
        reason: String,
        /// Hash of the input that caused the failure.
        input_hash: u64,
    },
}

impl WarmError {
    /// Get the process exit code for this error.
    ///
    /// Returns exit codes 101-116 for fatal errors, 1 for non-fatal.
    /// See module documentation for the complete exit code mapping.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::CudaUnavailable { .. } => 101, // EMB-E001: CUDA_UNAVAILABLE
            Self::ModelFileMissing { .. } => 101,
            Self::ModelLoadFailed { .. } => 102,
            Self::ModelValidationFailed { .. } => 103,
            Self::VramInsufficientTotal { .. } => 104,
            Self::VramInsufficientHeadroom { .. } => 105,
            Self::CudaInitFailed { .. } => 106,
            Self::CudaCapabilityInsufficient { .. } => 107,
            Self::CudaAllocFailed { .. } => 108,
            Self::FakeAllocationDetected { .. } => 109,
            Self::SinWaveOutputDetected { .. } => 110,
            Self::ModelDimensionMismatch { .. } => 116,
            Self::WeightFileMissing { .. } => 111,
            Self::WeightFileCorrupted { .. } => 112,
            Self::WeightChecksumMismatch { .. } => 113,
            Self::InferenceInitFailed { .. } => 114,
            Self::InferenceFailed { .. } => 115,
            _ => 1,
        }
    }

    /// Check if this is a fatal startup error.
    ///
    /// Fatal errors prevent warm loading from completing and should
    /// terminate the process with the appropriate exit code.
    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::CudaUnavailable { .. }
                | Self::ModelFileMissing { .. }
                | Self::ModelLoadFailed { .. }
                | Self::ModelValidationFailed { .. }
                | Self::VramInsufficientTotal { .. }
                | Self::VramInsufficientHeadroom { .. }
                | Self::CudaInitFailed { .. }
                | Self::CudaCapabilityInsufficient { .. }
                | Self::CudaAllocFailed { .. }
                | Self::FakeAllocationDetected { .. }
                | Self::SinWaveOutputDetected { .. }
                | Self::ModelDimensionMismatch { .. }
                | Self::WeightFileMissing { .. }
                | Self::WeightFileCorrupted { .. }
                | Self::WeightChecksumMismatch { .. }
                | Self::InferenceInitFailed { .. }
                | Self::InferenceFailed { .. }
        )
    }

    /// Get the error category name for logging.
    #[must_use]
    pub fn category(&self) -> &'static str {
        match self {
            Self::ModelFileMissing { .. } => "MODEL_FILE",
            Self::ModelLoadFailed { .. } => "MODEL_LOAD",
            Self::ModelValidationFailed { .. } | Self::ModelDimensionMismatch { .. } => {
                "MODEL_VALIDATION"
            }
            Self::VramInsufficientTotal { .. } | Self::VramInsufficientHeadroom { .. } => "VRAM",
            Self::CudaUnavailable { .. }
            | Self::CudaInitFailed { .. }
            | Self::CudaCapabilityInsufficient { .. }
            | Self::CudaAllocFailed { .. }
            | Self::CudaQueryFailed { .. }
            | Self::CudaNotAvailable => "CUDA",
            Self::FakeAllocationDetected { .. } => "FAKE_DETECTION",
            Self::SinWaveOutputDetected { .. } => "FAKE_DETECTION",
            Self::ModelAlreadyRegistered { .. }
            | Self::ModelNotRegistered { .. }
            | Self::RegistryLockPoisoned => "REGISTRY",
            Self::InvalidConfig { .. } => "CONFIG",
            Self::WorkingMemoryExhausted { .. } | Self::VramAllocationFailed { .. } => "MEMORY",
            Self::DiagnosticDumpFailed { .. } => "DIAGNOSTIC",
            Self::LoadTimeout { .. } => "TIMEOUT",
            Self::WeightFileMissing { .. }
            | Self::WeightFileCorrupted { .. }
            | Self::WeightChecksumMismatch { .. } => "WEIGHT_FILE",
            Self::InferenceInitFailed { .. } | Self::InferenceFailed { .. } => "INFERENCE",
        }
    }

    /// Get the structured error code for logging (e.g., "ERR-WARM-MODEL-MISSING").
    #[must_use]
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::CudaUnavailable { .. } => "EMB-E001",
            Self::ModelFileMissing { .. } => "ERR-WARM-MODEL-MISSING",
            Self::ModelLoadFailed { .. } => "ERR-WARM-MODEL-LOAD",
            Self::ModelValidationFailed { .. } => "ERR-WARM-MODEL-VALIDATION",
            Self::VramInsufficientTotal { .. } => "ERR-WARM-VRAM-INSUFFICIENT",
            Self::VramInsufficientHeadroom { .. } => "ERR-WARM-VRAM-HEADROOM",
            Self::CudaInitFailed { .. } => "ERR-WARM-CUDA-INIT",
            Self::CudaCapabilityInsufficient { .. } => "ERR-WARM-CUDA-CAPABILITY",
            Self::CudaAllocFailed { .. } => "ERR-WARM-CUDA-ALLOC",
            Self::FakeAllocationDetected { .. } => "ERR-WARM-FAKE-ALLOC",
            Self::SinWaveOutputDetected { .. } => "ERR-WARM-SINWAVE-OUTPUT",
            Self::ModelDimensionMismatch { .. } => "ERR-WARM-MODEL-DIMENSION",
            Self::ModelAlreadyRegistered { .. } => "ERR-WARM-REGISTRY-DUPLICATE",
            Self::ModelNotRegistered { .. } => "ERR-WARM-REGISTRY-MISSING",
            Self::InvalidConfig { .. } => "ERR-WARM-CONFIG-INVALID",
            Self::RegistryLockPoisoned => "ERR-WARM-REGISTRY-POISON",
            Self::WorkingMemoryExhausted { .. } => "ERR-WARM-MEMORY-WORKING",
            Self::CudaNotAvailable => "ERR-WARM-CUDA-UNAVAILABLE",
            Self::CudaQueryFailed { .. } => "ERR-WARM-CUDA-QUERY",
            Self::DiagnosticDumpFailed { .. } => "ERR-WARM-DIAGNOSTIC-DUMP",
            Self::LoadTimeout { .. } => "ERR-WARM-LOAD-TIMEOUT",
            Self::VramAllocationFailed { .. } => "ERR-WARM-VRAM-ALLOC",
            Self::WeightFileMissing { .. } => "ERR-WARM-WEIGHT-MISSING",
            Self::WeightFileCorrupted { .. } => "ERR-WARM-WEIGHT-CORRUPTED",
            Self::WeightChecksumMismatch { .. } => "ERR-WARM-WEIGHT-CHECKSUM",
            Self::InferenceInitFailed { .. } => "ERR-WARM-INFERENCE-INIT",
            Self::InferenceFailed { .. } => "ERR-WARM-INFERENCE-EXEC",
        }
    }
}

/// Result type alias for warm loading operations.
pub type WarmResult<T> = Result<T, WarmError>;
