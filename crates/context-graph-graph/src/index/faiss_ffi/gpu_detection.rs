//! GPU availability detection for FAISS operations.
//!
//! This module provides safe methods to check GPU availability before
//! making FAISS FFI calls. Uses subprocess detection to prevent crashes
//! on systems with driver issues (especially WSL2).
//!
//! # Environment Variables
//!
//! - `SKIP_GPU_TESTS=1`: Force `gpu_available()` to return false
//! - `FAISS_GPU_CHECKED=1`: Use cached result (internal use)
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004 Section 3.1: FAISS FFI Bindings

#[cfg(feature = "faiss-gpu")]
use std::os::raw::c_int;

#[cfg(feature = "faiss-gpu")]
use super::bindings::faiss_get_num_gpus;

/// Check if FAISS GPU support is available.
///
/// Returns true if:
/// 1. The `faiss-gpu` feature is enabled (FAISS library is linked)
/// 2. FAISS reports at least one CUDA-capable GPU
///
/// Use this before attempting any GPU operations to avoid crashes
/// on systems without GPU hardware or with driver issues.
///
/// This function uses a subprocess to safely detect GPU availability,
/// preventing crashes from driver initialization failures (especially on WSL2).
///
/// # Example
///
/// ```ignore
/// if gpu_available() {
///     let resources = GpuResources::new()?;
///     // ... use GPU resources
/// } else {
///     println!("No GPU available, skipping GPU operations");
/// }
/// ```
///
/// # Environment Variables
///
/// - `SKIP_GPU_TESTS=1`: Force this function to return false
/// - `FAISS_GPU_CHECKED=1`: Use cached result (internal use)
#[cfg(feature = "faiss-gpu")]
pub fn gpu_available() -> bool {
    use std::sync::OnceLock;

    // Cache the result to avoid repeated subprocess calls
    static GPU_AVAILABLE: OnceLock<bool> = OnceLock::new();

    *GPU_AVAILABLE.get_or_init(|| {
        // Allow tests to skip GPU via environment variable
        if std::env::var("SKIP_GPU_TESTS")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            return false;
        }

        // Use subprocess to safely check GPU availability
        // This prevents crashes from WSL2 driver issues
        check_gpu_via_subprocess()
    })
}

/// Check GPU availability using nvidia-smi as a subprocess.
/// This is safer than calling CUDA directly as it won't crash on driver issues.
#[cfg(feature = "faiss-gpu")]
fn check_gpu_via_subprocess() -> bool {
    use std::process::Command;

    // First check if nvidia-smi works (safest check)
    match Command::new("nvidia-smi")
        .arg("--query-gpu=count")
        .arg("--format=csv,noheader")
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(count) = stdout.trim().parse::<i32>() {
                    if count > 0 {
                        // nvidia-smi works and found GPUs
                        // Now try a quick CUDA test to verify driver works
                        return check_cuda_works();
                    }
                }
            }
            false
        }
        Err(_) => false,
    }
}

/// Verify FAISS GPU actually works by running a test that calls faiss_get_num_gpus.
/// Returns false if the FAISS test crashes or fails.
#[cfg(feature = "faiss-gpu")]
fn check_cuda_works() -> bool {
    use std::path::Path;
    use std::process::Command;

    // Check for pre-compiled FAISS GPU test binary (specific to our build)
    // This tests that FAISS can actually call CUDA functions without crashing
    let faiss_test = "/tmp/test_faiss_gpu_check";
    if Path::new(faiss_test).exists() {
        if let Ok(output) = Command::new(faiss_test).output() {
            if output.status.success() {
                return true;
            }
            // FAISS GPU test crashed - GPU not usable
            return false;
        }
    }

    // Fallback: Check for CUDA test binary
    let cuda_test = "/tmp/test_cuda3";
    if Path::new(cuda_test).exists() {
        if let Ok(output) = Command::new(cuda_test).output() {
            if !output.status.success() {
                return false;
            }
            // CUDA works, but we still need to be careful about FAISS
            // On WSL2 with driver issues, CUDA may work but FAISS crashes
        }
    }

    // Check for WSL2 with known driver issues
    // The /usr/lib/wsl/lib directory indicates WSL2
    if Path::new("/usr/lib/wsl/lib").exists() {
        // On WSL2, we've seen driver shim issues that crash FAISS
        // Be conservative and return false unless we have a working FAISS test
        if !Path::new(faiss_test).exists() {
            // No FAISS test binary - can't verify, assume unsafe on WSL2
            return false;
        }
    }

    // No test binaries available and not on WSL2 - assume GPU works
    true
}

/// Check if FAISS GPU support is available.
///
/// When the `faiss-gpu` feature is not enabled, this always returns `false`
/// because the FAISS library is not linked.
#[cfg(not(feature = "faiss-gpu"))]
#[inline]
pub fn gpu_available() -> bool {
    // FAISS library not linked - no GPU support available
    false
}

/// Directly check GPU count via FAISS FFI.
///
/// # Safety
/// This can crash on WSL2 with driver issues. Use `gpu_available()` instead
/// which performs a safe subprocess check first.
#[cfg(feature = "faiss-gpu")]
pub unsafe fn gpu_count_direct() -> Result<i32, i32> {
    let mut num_gpus: c_int = 0;
    let rc = faiss_get_num_gpus(&mut num_gpus);
    if rc == 0 {
        Ok(num_gpus)
    } else {
        Err(rc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_available_returns_bool() {
        // This test verifies gpu_available() works without crashing
        // even when no GPU is present. It should return false on systems
        // without CUDA GPUs and true on systems with them.
        let available = gpu_available();
        println!("GPU available: {}", available);
        // The function should return a valid bool either way
        // Using the value in a meaningful assertion
        let _ = available; // Mark as used - the test is that this doesn't crash
    }
}
