#![deny(deprecated)]

//! CUDA acceleration for Context Graph.
//!
//! This crate provides GPU-accelerated operations for:
//! - Vector similarity search (cosine, dot product)
//! - Neural attention mechanisms
//! - Modern Hopfield network computations
//!
//! # Constitution AP-007 Compliance
//!
//! **CUDA is ALWAYS required - no stub implementations in production.**
//!
//! The `StubVectorOps` type is available ONLY in test builds (`#[cfg(test)]`)
//! and must NOT be used in production code paths. All production code must
//! use real CUDA implementations.
//!
//! # Target Hardware
//!
//! - RTX 5090 (32GB GDDR7, 1.8 TB/s bandwidth)
//! - CUDA 13.1 with Compute Capability 12.0
//! - Blackwell architecture optimizations
//!
//! # Example (Test Only)
//!
//! ```ignore
//! // StubVectorOps is only available in #[cfg(test)] builds
//! #[cfg(test)]
//! use context_graph_cuda::{StubVectorOps, VectorOps};
//!
//! #[cfg(test)]
//! fn test_example() {
//!     let ops = StubVectorOps::new();
//!     assert!(!ops.is_gpu_available());
//! }
//! ```

pub mod cone;
pub mod error;
pub mod ops;
pub mod poincare;

// AP-007: StubVectorOps is TEST ONLY - not available in production builds
#[cfg(test)]
pub mod stub;

pub use cone::{
    cone_check_batch_cpu, cone_membership_score_cpu, get_cone_kernel_info, is_cone_gpu_available,
    ConeCudaConfig, ConeData, ConeKernelInfo, CONE_DATA_DIM, POINT_DIM,
};
#[cfg(feature = "cuda")]
pub use cone::{cone_check_batch_gpu, cone_check_single_gpu};
pub use error::{CudaError, CudaResult};
pub use ops::VectorOps;
pub use poincare::{poincare_distance_batch_cpu, poincare_distance_cpu, PoincareCudaConfig};
#[cfg(feature = "cuda")]
pub use poincare::{poincare_distance_batch_gpu, poincare_distance_single_gpu};
// AP-007: StubVectorOps export is gated to test-only builds
// Allow deprecated usage in tests - the deprecation warning is intentional for production
#[cfg(test)]
#[allow(deprecated)]
pub use stub::StubVectorOps;
