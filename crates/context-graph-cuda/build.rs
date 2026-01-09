//! Build script for CUDA kernel compilation.
//!
//! Compiles .cu files in kernels/ directory using nvcc.
//! Links resulting static library for FFI.
//!
//! # Target Hardware
//!
//! - RTX 5090 (Compute Capability 12.0)
//! - CUDA 13.1+
//!
//! # Constitution Reference
//!
//! - stack.gpu: RTX 5090, compute: "12.0"
//! - stack.lang.cuda: "13.1"
//!
//! # Environment Variables
//!
//! - `CUDA_PATH`: Path to CUDA toolkit (auto-detected if not set)
//! - `CUDA_ARCH`: Target architecture (default: sm_120 for RTX 5090)
//! - `NVCC_FLAGS`: Additional nvcc flags

#[cfg(feature = "cuda")]
use std::env;
#[cfg(feature = "cuda")]
use std::path::PathBuf;
#[cfg(feature = "cuda")]
use std::process::Command;

fn main() {
    // Always tell Cargo to re-run if build.rs or kernels change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=kernels/");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=CUDA_ARCH");
    println!("cargo:rerun-if-env-changed=NVCC_FLAGS");

    // Only compile CUDA kernels if cuda feature is enabled
    #[cfg(feature = "cuda")]
    {
        compile_cuda_kernels();
    }

    // CUDA is ALWAYS required - no stub implementations
    // RTX 5090 / Blackwell architecture mandated by constitution
    #[cfg(not(feature = "cuda"))]
    {
        panic!("CUDA feature is required. RTX 5090 GPU must be available. No fallback stubs.");
    }
}

#[cfg(feature = "cuda")]
fn compile_cuda_kernels() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set by cargo"));

    // Target architecture: RTX 5090 = Compute Capability 12.0 = sm_120
    let cuda_arch = env::var("CUDA_ARCH").unwrap_or_else(|_| "sm_120".to_string());

    // Find nvcc
    let nvcc = find_nvcc();

    // Compile poincare_distance.cu
    compile_kernel(
        &nvcc,
        "kernels/poincare_distance.cu",
        "poincare_distance",
        &cuda_arch,
        &out_dir,
    );

    // Compile cone_check.cu
    compile_kernel(
        &nvcc,
        "kernels/cone_check.cu",
        "cone_check",
        &cuda_arch,
        &out_dir,
    );
}

#[cfg(feature = "cuda")]
fn find_nvcc() -> PathBuf {
    // Check if nvcc is in PATH
    if let Ok(output) = Command::new("which").arg("nvcc").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return PathBuf::from(path);
        }
    }

    // Check CUDA_PATH environment variable
    if let Ok(cuda_path) = env::var("CUDA_PATH") {
        let nvcc_path = PathBuf::from(&cuda_path).join("bin").join("nvcc");
        if nvcc_path.exists() {
            return nvcc_path;
        }
    }

    // Check common CUDA installation paths
    let common_paths = [
        "/usr/local/cuda/bin/nvcc",
        "/usr/local/cuda-13.1/bin/nvcc",
        "/usr/local/cuda-13.0/bin/nvcc",
        "/usr/local/cuda-12.0/bin/nvcc",
        "/opt/cuda/bin/nvcc",
    ];

    for path in &common_paths {
        if PathBuf::from(path).exists() {
            return PathBuf::from(path);
        }
    }

    panic!(
        "nvcc not found. Please install CUDA Toolkit 13.1+ or set CUDA_PATH environment variable.\n\
         Download from: https://developer.nvidia.com/cuda-downloads\n\
         Or disable 'cuda' feature to use stub implementations."
    );
}

#[cfg(feature = "cuda")]
fn compile_kernel(
    nvcc: &std::path::Path,
    source: &str,
    name: &str,
    arch: &str,
    out_dir: &std::path::Path,
) {
    let obj_path = out_dir.join(format!("{}.o", name));
    let lib_path = out_dir.join(format!("lib{}.a", name));

    // Get additional nvcc flags from environment
    let extra_flags: Vec<String> = env::var("NVCC_FLAGS")
        .map(|f| f.split_whitespace().map(String::from).collect())
        .unwrap_or_default();

    // Compile to object file
    // -c: Compile only (no linking)
    // -O3: Highest optimization level
    // -arch: Target GPU architecture
    // --compiler-options: Pass flags to host compiler
    // -fPIC: Position-independent code (required for shared libraries)
    // --generate-line-info: Preserve line info for debugging
    // -DNDEBUG: Disable debug assertions in release
    let mut compile_cmd = Command::new(nvcc);
    compile_cmd
        .arg("-c")
        .arg("-O3")
        .args(["-arch", arch])
        .args(["--compiler-options", "-fPIC"])
        .arg("--generate-line-info")
        .arg("-DNDEBUG")
        .args(&extra_flags)
        .args(["-o", obj_path.to_str().unwrap()])
        .arg(source);

    println!("cargo:warning=Running: {:?}", compile_cmd);

    let compile_status = compile_cmd
        .status()
        .unwrap_or_else(|e| panic!("Failed to run nvcc: {}\nCommand: {:?}", e, compile_cmd));

    if !compile_status.success() {
        panic!(
            "CUDA kernel compilation failed for {}\n\
             nvcc exit code: {:?}\n\
             Source: {}\n\
             Target arch: {}\n\
             Please check that:\n\
             1. CUDA Toolkit 13.1+ is installed\n\
             2. Source file exists and is valid CUDA code\n\
             3. Target architecture matches your GPU",
            source,
            compile_status.code(),
            source,
            arch
        );
    }

    // Create static library using ar
    let ar_status = Command::new("ar")
        .args([
            "rcs",
            lib_path.to_str().unwrap(),
            obj_path.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run ar - is it installed?");

    if !ar_status.success() {
        panic!(
            "Failed to create static library: {}\n\
             ar exit code: {:?}",
            lib_path.display(),
            ar_status.code()
        );
    }

    // Tell Cargo where to find the library
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static={}", name);

    // Link CUDA driver library (libcuda.so)
    // Required for Driver API (cuInit, cuDeviceGetCount) - used instead of Runtime API
    // to avoid CUDA 13.1 WSL2 segfault bug with cudaGetDeviceCount
    println!("cargo:rustc-link-lib=cuda");

    // Link CUDA runtime library
    // cudart: CUDA runtime API (required for kernel launching)
    println!("cargo:rustc-link-lib=cudart");

    // Also link math library for transcendental functions
    println!("cargo:rustc-link-lib=m");

    // Link C++ standard library for CUDA runtime symbols
    // Required for __cxa_guard_acquire, __cxa_guard_release, __gxx_personality_v0
    println!("cargo:rustc-link-lib=stdc++");

    // Add WSL2 CUDA driver path (libcuda.so lives here, not in /usr/local/cuda)
    // This is critical for WSL2 where the driver is provided by Windows
    if PathBuf::from("/usr/lib/wsl/lib").exists() {
        println!("cargo:rustc-link-search=native=/usr/lib/wsl/lib");
    }

    // Add CUDA library path for cudart
    if let Ok(cuda_path) = env::var("CUDA_PATH") {
        let lib64_path = PathBuf::from(&cuda_path).join("lib64");
        if lib64_path.exists() {
            println!("cargo:rustc-link-search=native={}", lib64_path.display());
        }
        let lib_path = PathBuf::from(&cuda_path).join("lib");
        if lib_path.exists() {
            println!("cargo:rustc-link-search=native={}", lib_path.display());
        }
    } else {
        // Common CUDA library paths
        for path in &[
            "/usr/local/cuda/lib64",
            "/usr/local/cuda/lib",
            "/opt/cuda/lib64",
        ] {
            if PathBuf::from(path).exists() {
                println!("cargo:rustc-link-search=native={}", path);
            }
        }
    }

    println!(
        "cargo:warning=Successfully compiled CUDA kernel: {} -> {}",
        source,
        lib_path.display()
    );
}
