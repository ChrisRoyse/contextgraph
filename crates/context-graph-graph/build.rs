//! Build script for context-graph-graph.
//!
//! Configures linker paths for FAISS GPU library.
//!
//! # FAISS C API Requirements
//!
//! - libfaiss_c.so must be installed in /usr/local/lib
//! - Built with: -DFAISS_ENABLE_GPU=ON -DFAISS_ENABLE_C_API=ON
//! - CUDA libraries must be available
//!
//! # WSL2 GPU Support
//!
//! On WSL2, the CUDA driver shim is in /usr/lib/wsl/lib. This path must be
//! prioritized in the runtime library search path to avoid segfaults from
//! conflicting libcuda.so versions.

fn main() {
    // Tell Cargo to rerun if build.rs changes
    println!("cargo:rerun-if-changed=build.rs");

    // WSL2 CUDA driver shim path - MUST come first in rpath to avoid segfaults
    // See: https://github.com/microsoft/WSL/issues/13773
    let wsl_lib = "/usr/lib/wsl/lib";
    if std::path::Path::new(wsl_lib).exists() {
        println!("cargo:rustc-link-search=native={}", wsl_lib);
        // Add to rpath FIRST so WSL CUDA shim is loaded before any other libcuda.so
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", wsl_lib);
    }

    // Add library search paths for FAISS
    // Standard installation location for FAISS built from source
    println!("cargo:rustc-link-search=native=/usr/local/lib");

    // CUDA library paths (for GPU support)
    // Check multiple possible CUDA installation locations
    let cuda_paths = [
        "/usr/local/cuda/lib64",
        "/usr/local/cuda-13.1/lib64",
        "/usr/local/cuda-13.1/targets/x86_64-linux/lib",
        "/usr/local/cuda-13/lib64",
    ];

    for path in cuda_paths {
        if std::path::Path::new(path).exists() {
            println!("cargo:rustc-link-search=native={}", path);
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", path);
        }
    }

    // Link against FAISS C API library
    // The FFI bindings use #[link(name = "faiss_c")] which handles the actual linking,
    // but we need to ensure the search path is set

    // Also link against the main FAISS library (dependency of faiss_c)
    // Note: faiss_c dynamically links to faiss, so we may need both
    println!("cargo:rustc-link-lib=dylib=faiss");

    // Runtime library path for FAISS (after WSL path)
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/local/lib");

    // Print build info for debugging
    if std::env::var("CARGO_FEATURE_VERBOSE").is_ok() {
        println!("cargo:warning=FAISS library search path: /usr/local/lib");
        if std::path::Path::new(wsl_lib).exists() {
            println!("cargo:warning=WSL2 CUDA shim path: {}", wsl_lib);
        }
    }
}
