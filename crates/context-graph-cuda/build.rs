//! Build script for CUDA kernel compilation.
//!
//! Compiles .cu files in kernels/ directory using nvcc.
//! Produces native cubin for Driver API loading (knn.cu).
//!
//! # Target Hardware
//!
//! - RTX 5090 (Compute Capability 12.0)
//! - CUDA 13.2 (validated at build time)
//!
//! # Constitution Reference
//!
//! - stack.gpu: RTX 5090, compute: "12.0"
//! - stack.lang.cuda: "13.x"
//!
//! # Environment Variables
//!
//! - `CUDA_PATH`: Path to CUDA 13.2 toolkit (validated if set)
//! - `CUDA_HOME`: Alternate CUDA toolkit root used by Cargo/CMake environments
//! - `CUDA_ARCH`: Target architecture (must be sm_120a for RTX 5090)
//! - `NVCC_FLAGS`: Additional nvcc flags

#[cfg(feature = "cuda")]
use std::env;
#[cfg(feature = "cuda")]
use std::fs;
#[cfg(feature = "cuda")]
use std::path::PathBuf;
#[cfg(feature = "cuda")]
use std::process::Command;

fn main() {
    // Always tell Cargo to re-run if build.rs or kernels change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=kernels/");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=CUDA_HOME");
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

    // Target architecture: RTX 5090 = Compute Capability 12.0 = sm_120a.
    // sm_120a unlocks the Blackwell consumer family-specific instruction path
    // and avoids PTX JIT at first launch.
    let cuda_arch = env::var("CUDA_ARCH").unwrap_or_else(|_| "sm_120a".to_string());
    if cuda_arch != "sm_120a" {
        panic!(
            "Unsupported CUDA_ARCH={}. Context Graph is configured for RTX 5090 \
             native cubin builds and requires CUDA_ARCH=sm_120a.",
            cuda_arch
        );
    }

    // Find nvcc
    let nvcc = find_nvcc();

    // Compile native cubins (uses Driver API to avoid WSL2 cudart bugs).
    compile_kernel_to_cubin(&nvcc, "kernels/knn.cu", "knn", &cuda_arch, &out_dir);
    compile_exact_kernel_to_cubin(&nvcc, "kernels/hdc.cu", "hdc", &cuda_arch, &out_dir);

    // Link CUDA driver library (libcuda.so)
    // Required for Driver API (cuInit, cuDeviceGetCount, cuLaunchKernel)
    println!("cargo:rustc-link-lib=cuda");

    // Add WSL2 CUDA driver path (libcuda.so lives here, not in /usr/local/cuda)
    if PathBuf::from("/usr/lib/wsl/lib").exists() {
        println!("cargo:rustc-link-search=native=/usr/lib/wsl/lib");
    }

    // Add CUDA library path for driver library
    if let Ok(cuda_path) = env::var("CUDA_PATH") {
        let lib64_path = PathBuf::from(&cuda_path).join("lib64");
        if lib64_path.exists() {
            println!("cargo:rustc-link-search=native={}", lib64_path.display());
        }
    } else {
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

    // Link FAISS C library when faiss-working feature is enabled
    #[cfg(feature = "faiss-working")]
    {
        link_faiss_library();
    }
}

/// Link FAISS C library (libfaiss_c.so).
///
/// Searches common installation paths and validates library exists before linking.
/// Fails fast with clear error message if FAISS is not installed.
#[cfg(feature = "faiss-working")]
fn link_faiss_library() {
    let home = env::var("HOME").unwrap_or_else(|_| "/home".to_string());

    // Search paths in priority order
    let search_paths = [
        format!("{}/.local/lib", home),
        "/usr/local/lib".to_string(),
        "/usr/lib".to_string(),
        "/usr/lib/x86_64-linux-gnu".to_string(),
    ];

    for path in &search_paths {
        let lib_path = PathBuf::from(path).join("libfaiss_c.so");
        if lib_path.exists() {
            println!("cargo:rustc-link-search=native={}", path);
            println!("cargo:rustc-link-lib=dylib=faiss_c");
            println!("cargo:rustc-link-lib=dylib=faiss");
            println!(
                "cargo:warning=FAISS GPU enabled: linking against {}",
                lib_path.display()
            );
            return;
        }
    }

    // FAIL FAST - FAISS library not found
    let searched = search_paths
        .iter()
        .map(|p| format!("  - {}/libfaiss_c.so", p))
        .collect::<Vec<_>>()
        .join("\n");

    panic!(
        "\n\
        FAISS LIBRARY NOT FOUND - BUILD FAILED\n\
        \n\
        The 'faiss-working' feature is enabled but libfaiss_c.so was not found.\n\
        FAISS must be rebuilt with CUDA 13.2+ and sm_120 (RTX 5090) support.\n\
        \n\
        To fix this, run:\n\
          ./scripts/rebuild_faiss_gpu.sh\n\
        \n\
        Searched paths:\n\
        {}\n\
        \n\
        If FAISS is installed elsewhere, set LIBRARY_PATH:\n\
          export LIBRARY_PATH=/path/to/faiss/lib:$LIBRARY_PATH\n",
        searched
    );
}

#[cfg(feature = "cuda")]
fn find_nvcc() -> PathBuf {
    // Prefer explicit Cargo/CMake environment roots over PATH so builds use
    // the toolkit selected in .cargo/config.toml.
    for var in ["CUDA_PATH", "CUDA_HOME"] {
        if let Ok(cuda_path) = env::var(var) {
            let nvcc_path = PathBuf::from(&cuda_path).join("bin").join("nvcc");
            if !nvcc_path.exists() {
                panic!(
                    "{} is set to {:?}, but {:?} does not exist. Refusing to \
                     continue with another CUDA toolkit path; fix {} or unset it.",
                    var, cuda_path, nvcc_path, var
                );
            }
            validate_nvcc_cuda_13_2(&nvcc_path);
            return nvcc_path;
        }
    }

    // Check CUDA 13.2 installation paths only. Older toolkits are rejected
    // below by validate_nvcc_cuda_13_2 instead of being used as fallbacks.
    let common_paths = ["/usr/local/cuda-13.2/bin/nvcc", "/usr/local/cuda/bin/nvcc"];

    for path in &common_paths {
        if PathBuf::from(path).exists() {
            let nvcc = PathBuf::from(path);
            validate_nvcc_cuda_13_2(&nvcc);
            return nvcc;
        }
    }

    // PATH is allowed only when it resolves to CUDA 13.2.
    if let Ok(output) = Command::new("which").arg("nvcc").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let nvcc = PathBuf::from(path);
            validate_nvcc_cuda_13_2(&nvcc);
            return nvcc;
        }
    }

    panic!(
        "nvcc not found. Please install CUDA Toolkit 13.2+ or set CUDA_PATH/CUDA_HOME environment variable.\n\
         Download from: https://developer.nvidia.com/cuda-downloads"
    );
}

#[cfg(feature = "cuda")]
fn validate_nvcc_cuda_13_2(nvcc: &std::path::Path) {
    let output = Command::new(nvcc)
        .arg("--version")
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {:?} --version: {}", nvcc, e));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let version_text = format!("{}\n{}", stdout, stderr);

    if !output.status.success() || !version_text.contains("release 13.2") {
        panic!(
            "Unsupported nvcc at {:?}. Context Graph requires CUDA 13.2 for \
             RTX 5090 native sm_120a cubin builds.\n\
             nvcc --version output:\n{}",
            nvcc, version_text
        );
    }
}

/// Compile CUDA kernel to native cubin for Driver API loading.
///
/// This avoids linking against cudart which has static initialization
/// bugs on WSL2 with CUDA 13.x.
#[cfg(feature = "cuda")]
fn compile_kernel_to_cubin(
    nvcc: &std::path::Path,
    source: &str,
    name: &str,
    arch: &str,
    out_dir: &std::path::Path,
) {
    compile_kernel_to_cubin_inner(nvcc, source, name, arch, out_dir, true);
}

/// Compile CUDA kernel to native cubin without fast-math.
///
/// HDC output is hashed into durable ME-JEPA cache records, so approximate
/// arithmetic is not acceptable. The HDC kernel keeps exact integer bit logic
/// on-device and normalizes on the Rust side with the same f32 reduction used
/// by the CPU implementation.
#[cfg(feature = "cuda")]
fn compile_exact_kernel_to_cubin(
    nvcc: &std::path::Path,
    source: &str,
    name: &str,
    arch: &str,
    out_dir: &std::path::Path,
) {
    compile_kernel_to_cubin_inner(nvcc, source, name, arch, out_dir, false);
}

#[cfg(feature = "cuda")]
fn compile_kernel_to_cubin_inner(
    nvcc: &std::path::Path,
    source: &str,
    name: &str,
    arch: &str,
    out_dir: &std::path::Path,
    use_fast_math: bool,
) {
    let cubin_path = out_dir.join(format!("{}.cubin", name));

    // Get additional nvcc flags from environment
    let extra_flags: Vec<String> = env::var("NVCC_FLAGS")
        .map(|f| f.split_whitespace().map(String::from).collect())
        .unwrap_or_default();

    // Compile to native Blackwell cubin with tuned flags.
    // --cubin         : Generate native cubin (Driver API loads it via cuModuleLoadData)
    // -O3             : Highest nvcc optimization level
    // -arch sm_120a   : Target RTX 5090 Blackwell SASS, no PTX JIT fallback
    // -Xptxas -O3     : PTX assembler at max optimization during build
    // --use_fast_math : Acceptable for embedding workloads (cosines, L2 norms, dots —
    //                   1-ULP tolerance suffices). Enables fast division / sin/cos.
    // --restrict      : Treat __restrict__ pointers strictly (better aliasing analysis)
    // -DNDEBUG        : Strip asserts
    let mut compile_cmd = Command::new(nvcc);
    compile_cmd
        .arg("--cubin")
        .arg("-O3")
        .args(["-arch", arch])
        .args(["-Xptxas", "-O3"])
        .arg("--restrict")
        .arg("-DNDEBUG")
        .args(&extra_flags)
        .args(["-o", cubin_path.to_str().unwrap()]);
    if use_fast_math {
        compile_cmd.arg("--use_fast_math");
    }
    compile_cmd.arg(source);

    println!("cargo:warning=Running cubin compilation: {:?}", compile_cmd);

    let compile_status = compile_cmd
        .status()
        .unwrap_or_else(|e| panic!("Failed to run nvcc: {}\nCommand: {:?}", e, compile_cmd));

    if !compile_status.success() {
        panic!(
            "CUDA kernel cubin compilation failed for {}\n\
             nvcc exit code: {:?}\n\
             Source: {}\n\
             Target arch: {}",
            source,
            compile_status.code(),
            source,
            arch
        );
    }

    let metadata = fs::metadata(&cubin_path)
        .unwrap_or_else(|e| panic!("Failed to stat cubin file {}: {}", cubin_path.display(), e));
    if metadata.len() == 0 {
        panic!("CUDA cubin file is empty: {}", cubin_path.display());
    }

    // Generate Rust module with embedded cubin image.
    let rs_path = out_dir.join(format!("{}_image.rs", name));
    let cubin_path_literal = cubin_path
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");

    let rs_content = format!(
        "// Auto-generated CUDA cubin image for {} kernel.\n\
         // This file is generated by build.rs. Do not edit manually.\n\
         \n\
         /// Native sm_120a cubin image for {} kernels.\n\
         /// Load with cuModuleLoadData.\n\
         pub const IMAGE: &[u8] = include_bytes!(\"{}\");\n",
        name, name, cubin_path_literal
    );

    fs::write(&rs_path, rs_content)
        .unwrap_or_else(|e| panic!("Failed to write {}: {}", rs_path.display(), e));

    println!(
        "cargo:warning=Successfully compiled CUDA kernel to cubin: {} -> {} ({} bytes)",
        source,
        cubin_path.display(),
        metadata.len()
    );
}
