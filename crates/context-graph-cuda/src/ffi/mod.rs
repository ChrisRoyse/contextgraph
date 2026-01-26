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
//!
//! # FAISS GPU Support
//!
//! FAISS GPU is conditionally available via the `faiss-working` feature.
//! Without this feature, only the custom GPU k-NN implementation (knn.rs) is available.
//!
//! ## Enabling FAISS GPU
//!
//! 1. Run `./scripts/rebuild_faiss_gpu.sh` to build FAISS with CUDA 13.1 support
//! 2. Build with: `cargo build --features faiss-working`
//!
//! ## Why FAISS is Optional
//!
//! On WSL2 with CUDA 13.1, the standard FAISS build crashes during static
//! initialization due to cudart bugs. The custom k-NN in knn.rs uses the
//! CUDA Driver API which works correctly. FAISS GPU requires a custom build
//! with lazy CUDA initialization.

pub mod cuda_driver;
pub mod knn;

// FAISS module - only compiled when faiss-working feature is explicitly enabled.
// This feature requires FAISS to be rebuilt with:
//   - CUDA 13.1+
//   - sm_120 (RTX 5090) architecture support
//   - C API enabled (FAISS_ENABLE_C_API=ON)
//
// The build.rs will FAIL FAST if faiss-working is enabled but libfaiss_c.so is not found.
#[cfg(feature = "faiss-working")]
pub mod faiss;

pub use cuda_driver::*;
pub use knn::*;

// Re-export FAISS types when available
#[cfg(feature = "faiss-working")]
pub use faiss::{
    check_faiss_result, gpu_available as faiss_gpu_available, FaissGpuResourcesProvider,
    FaissIndex, FaissStandardGpuResources, GpuResources as FaissGpuResources, MetricType,
    FAISS_OK,
};

/// Check if FAISS GPU support is available.
///
/// Returns `true` only when:
/// 1. The `faiss-working` feature is enabled
/// 2. FAISS reports at least one GPU
/// 3. GPU actually works (verified at runtime)
///
/// When `faiss-working` is not enabled, this always returns `false` with
/// informative logging about how to enable it.
#[inline]
pub fn is_faiss_gpu_available() -> bool {
    #[cfg(feature = "faiss-working")]
    {
        faiss::gpu_available()
    }

    #[cfg(not(feature = "faiss-working"))]
    {
        // Log once about FAISS being disabled
        static LOGGED: std::sync::Once = std::sync::Once::new();
        LOGGED.call_once(|| {
            tracing::info!(
                target: "context_graph::cuda::faiss",
                "FAISS GPU is disabled (faiss-working feature not enabled). \
                 Using custom GPU k-NN implementation. \
                 To enable FAISS GPU: \
                 1. Run ./scripts/rebuild_faiss_gpu.sh \
                 2. Build with: cargo build --features faiss-working"
            );
        });
        false
    }
}

/// Get a human-readable status of FAISS GPU availability.
///
/// Returns a status string explaining:
/// - Whether FAISS is enabled/disabled
/// - If disabled, why and how to enable
/// - If enabled, whether GPU is working
pub fn faiss_status() -> &'static str {
    #[cfg(feature = "faiss-working")]
    {
        if faiss::gpu_available() {
            "FAISS GPU: ENABLED and WORKING"
        } else {
            "FAISS GPU: ENABLED but GPU unavailable (check CUDA installation)"
        }
    }

    #[cfg(not(feature = "faiss-working"))]
    {
        "FAISS GPU: DISABLED (faiss-working feature not enabled)"
    }
}
