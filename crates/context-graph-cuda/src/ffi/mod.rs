//! CUDA FFI bindings - SINGLE SOURCE OF TRUTH.
//!
//! ALL CUDA extern "C" declarations MUST be in this module.
//! No other crate may declare CUDA FFI bindings.
//!
//! # Constitution Compliance
//!
//! - ARCH-06: CUDA FFI only in context-graph-cuda
//! - AP-08: No sync I/O in async context (these are blocking calls)
//!
//! # Safety
//!
//! All functions in this module are unsafe FFI. Callers must ensure:
//! - cuInit() called before any other function
//! - Valid device ordinals passed to device functions
//! - Sufficient buffer sizes for string outputs

pub mod cuda_driver;
pub mod knn;

// FAISS module disabled - crashes on WSL2 with CUDA 13.1 due to
// static initialization bugs. Using custom GPU k-NN kernel instead.
// pub mod faiss;

pub use cuda_driver::*;
pub use knn::*;

// pub use faiss::*;
