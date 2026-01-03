//! GpuTensor wrapper for type-safe GPU tensor operations.
//!
//! # Design
//!
//! `GpuTensor` wraps `candle_core::Tensor` with additional tracking:
//! - Automatic device placement
//! - Memory usage tracking
//! - Easy conversion to/from CPU vectors
//!
//! # Usage
//!
//! ```rust,no_run
//! # use context_graph_embeddings::gpu::GpuTensor;
//! # use context_graph_embeddings::gpu::init_gpu;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! init_gpu()?;
//!
//! // Create from CPU vector
//! let vec = vec![1.0f32, 2.0, 3.0, 4.0];
//! let tensor = GpuTensor::from_vec(&vec)?;
//!
//! // Check tensor properties
//! assert_eq!(tensor.dim(), 4);
//! assert_eq!(tensor.shape(), &[4]);
//!
//! // Convert back to CPU
//! let result: Vec<f32> = tensor.to_vec()?;
//! assert_eq!(result.len(), 4);
//! # Ok(())
//! # }
//! ```

mod core;
mod ops;

pub use self::core::GpuTensor;
