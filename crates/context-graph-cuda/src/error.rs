//! Error types for CUDA operations.

use thiserror::Error;

/// CUDA-specific errors.
#[derive(Debug, Error)]
pub enum CudaError {
    /// CUDA device initialization failed.
    #[error("Failed to initialize CUDA device: {0}")]
    DeviceInitError(String),

    /// Memory allocation failed.
    #[error("CUDA memory allocation failed: {0}")]
    MemoryError(String),

    /// Kernel execution failed.
    #[error("CUDA kernel execution failed: {0}")]
    KernelError(String),

    /// Dimension mismatch.
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Device not available.
    #[error("No CUDA device available")]
    NoDevice,

    /// Feature not implemented.
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    /// Invalid configuration parameter.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// FAISS operation failed.
    #[error("FAISS {operation} failed with error code {code}")]
    FaissError {
        /// Operation that failed
        operation: String,
        /// FAISS error code
        code: i32,
    },

    /// CUDA runtime API error.
    #[error("CUDA runtime error in {operation}: code {code}")]
    CudaRuntimeError {
        /// Operation that failed
        operation: String,
        /// CUDA error code
        code: i32,
    },

    /// Invalid argument.
    #[error("Invalid argument '{argument}': {reason}")]
    InvalidArgument {
        /// Argument name
        argument: String,
        /// Reason why it's invalid
        reason: String,
    },

    /// Tensor operation failed (candle-core).
    #[error("Tensor operation failed: {0}")]
    TensorError(String),

    /// Batch size exceeds GPU memory budget.
    #[error("Batch size {size} exceeds maximum {max}")]
    BatchTooLarge {
        /// Requested batch size
        size: usize,
        /// Maximum allowed batch size
        max: usize,
    },
}

/// Result type for CUDA operations.
pub type CudaResult<T> = Result<T, CudaError>;
