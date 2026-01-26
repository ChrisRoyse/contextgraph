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

// FAISS module - requires FAISS rebuilt with lazy CUDA initialization.
// On WSL2 with CUDA 13.1, the standard FAISS build crashes during static
// initialization due to cudart bugs. The custom k-NN in knn.rs uses the
// CUDA Driver API which works correctly.
//
// To enable FAISS GPU:
// 1. Rebuild FAISS from source with: cmake -DFAISS_ENABLE_GPU=ON
//    -DCMAKE_CUDA_ARCHITECTURES=120 -DBUILD_SHARED_LIBS=ON
// 2. Ensure FAISS uses lazy CUDA initialization
// 3. Uncomment: pub mod faiss;
#[cfg(feature = "faiss-working")]
pub mod faiss;

pub use cuda_driver::*;
pub use knn::*;
