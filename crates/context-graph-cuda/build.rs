//! Build script for CUDA kernel compilation.
//!
//! Compiles .cu files in kernels/ directory using nvcc.
//! Produces PTX for Driver API (knn.cu) or static libraries for Runtime API.
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

    // Compile poincare_distance.cu (still uses Runtime API)
    compile_kernel_to_lib(
        &nvcc,
        "kernels/poincare_distance.cu",
        "poincare_distance",
        &cuda_arch,
        &out_dir,
    );

    // Compile cone_check.cu (still uses Runtime API)
    compile_kernel_to_lib(
        &nvcc,
        "kernels/cone_check.cu",
        "cone_check",
        &cuda_arch,
        &out_dir,
    );

    // Compile knn.cu to PTX (uses Driver API to avoid WSL2 cudart bugs)
    compile_kernel_to_ptx(
        &nvcc,
        "kernels/knn.cu",
        "knn",
        &cuda_arch,
        &out_dir,
    );

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
        FAISS must be rebuilt with CUDA 13.1+ and sm_120 (RTX 5090) support.\n\
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
         Download from: https://developer.nvidia.com/cuda-downloads"
    );
}

/// Compile CUDA kernel to PTX for Driver API loading.
///
/// This avoids linking against cudart which has static initialization
/// bugs on WSL2 with CUDA 13.1.
#[cfg(feature = "cuda")]
fn compile_kernel_to_ptx(
    nvcc: &std::path::Path,
    source: &str,
    name: &str,
    arch: &str,
    out_dir: &std::path::Path,
) {
    let ptx_path = out_dir.join(format!("{}.ptx", name));

    // Get additional nvcc flags from environment
    let extra_flags: Vec<String> = env::var("NVCC_FLAGS")
        .map(|f| f.split_whitespace().map(String::from).collect())
        .unwrap_or_default();

    // Extract compute capability from arch (sm_120 -> compute_120)
    let compute = arch.replace("sm_", "compute_");

    // Compile to PTX
    // --ptx: Generate PTX assembly
    // -O3: Highest optimization level
    // -arch: Target virtual architecture for PTX
    let mut compile_cmd = Command::new(nvcc);
    compile_cmd
        .arg("--ptx")
        .arg("-O3")
        .args(["-arch", &compute])
        .arg("-DNDEBUG")
        .args(&extra_flags)
        .args(["-o", ptx_path.to_str().unwrap()])
        .arg(source);

    println!("cargo:warning=Running PTX compilation: {:?}", compile_cmd);

    let compile_status = compile_cmd
        .status()
        .unwrap_or_else(|e| panic!("Failed to run nvcc: {}\nCommand: {:?}", e, compile_cmd));

    if !compile_status.success() {
        panic!(
            "CUDA kernel PTX compilation failed for {}\n\
             nvcc exit code: {:?}\n\
             Source: {}\n\
             Target arch: {}",
            source,
            compile_status.code(),
            source,
            arch
        );
    }

    // Read PTX and generate Rust code to embed it
    let ptx_content = fs::read_to_string(&ptx_path)
        .unwrap_or_else(|e| panic!("Failed to read PTX file {}: {}", ptx_path.display(), e));

    // Generate Rust module with embedded PTX
    let rs_path = out_dir.join(format!("{}_ptx.rs", name));

    // Use a different approach to embed PTX: escape the content
    // This avoids raw string delimiter conflicts
    let escaped_ptx = ptx_content
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");

    let rs_content = format!(
        "// Auto-generated PTX for {} kernel.\n\
         // This file is generated by build.rs. Do not edit manually.\n\
         \n\
         /// PTX code for {} kernels.\n\
         /// Load with cuModuleLoadData.\n\
         pub const PTX: &str = \"{}\";\n",
        name, name, escaped_ptx
    );

    fs::write(&rs_path, rs_content)
        .unwrap_or_else(|e| panic!("Failed to write {}: {}", rs_path.display(), e));

    println!(
        "cargo:warning=Successfully compiled CUDA kernel to PTX: {} -> {}",
        source,
        ptx_path.display()
    );
}

/// Compile CUDA kernel to static library for Runtime API linking.
#[cfg(feature = "cuda")]
fn compile_kernel_to_lib(
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
             nvcc exit code: {:?}",
            source,
            compile_status.code()
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
        .expect("Failed to run ar");

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

    // Link CUDA runtime library (required for these kernels)
    println!("cargo:rustc-link-lib=cudart");

    // Also link math library for transcendental functions
    println!("cargo:rustc-link-lib=m");

    // Link C++ standard library for CUDA runtime symbols
    println!("cargo:rustc-link-lib=stdc++");

    println!(
        "cargo:warning=Successfully compiled CUDA kernel: {} -> {}",
        source,
        lib_path.display()
    );
}
