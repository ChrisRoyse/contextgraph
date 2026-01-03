//! GPU device management for RTX 5090 acceleration.
//!
//! # GPU-ONLY Architecture
//!
//! This module is **strictly GPU-only** with NO CPU fallback. If a CUDA-capable
//! GPU is not available, initialization will fail with a clear error.
//!
//! # Requirements
//!
//! - **Hardware**: NVIDIA CUDA-capable GPU (target: RTX 5090 / Blackwell GB202)
//! - **Driver**: CUDA 13.1+ with compatible NVIDIA drivers
//! - **Memory**: Minimum 16GB VRAM recommended (32GB for RTX 5090)
//!
//! # Singleton Pattern
//!
//! The GPU device is initialized once and shared globally. This ensures:
//! - Single CUDA context for optimal memory management
//! - Consistent device placement across all operations
//! - Automatic cleanup on process exit
//!
//! # Usage
//!
//! ```
//! # use context_graph_embeddings::gpu::{init_gpu, device};
//! # use candle_core::{Tensor, DType};
//! # fn main() -> candle_core::Result<()> {
//! // Initialize at startup - WILL FAIL if no GPU available
//! init_gpu()?;
//!
//! // Get device for tensor operations
//! let dev = device();
//! let tensor = Tensor::zeros((1024,), DType::F32, dev)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Error Handling
//!
//! - [`init_gpu`] returns an error if CUDA is unavailable
//! - [`device`] panics if called before initialization
//! - NO silent fallback to CPU - errors are explicit and actionable
//!
//! # Module Structure
//!
//! - [`core`]: Singleton initialization and static globals
//! - [`accessors`]: Device accessor functions
//! - [`utils`]: Internal utility functions

mod accessors;
mod core;
mod utils;

// Re-export all public APIs for backwards compatibility
pub use accessors::{default_dtype, device, get_gpu_info, is_gpu_available, require_gpu};
pub use core::init_gpu;
