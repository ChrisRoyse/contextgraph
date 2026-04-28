# CUDA 13.2 / RTX 5090 Optimization Notes

## Official Research Snapshot

Official NVIDIA docs checked on 2026-04-27:

- CUDA Toolkit 13.2 Update 1 release notes: https://docs.nvidia.com/cuda/cuda-toolkit-release-notes/index.html
- CUDA Toolkit 13.1 archive: https://docs.nvidia.com/cuda/archive/13.1.0/cuda-toolkit-release-notes/index.html
- CUDA GPU compute capability list: https://developer.nvidia.com/cuda/gpus?lng=en-US

Relevant findings:

- CUDA 13.2 Update 1 is the current CUDA 13.2 release line in the live NVIDIA docs.
- CUDA 13.2 Update 1 lists Linux driver `>=595.58.03`.
- CUDA 13.x minor-version compatibility requires driver `>=580`.
- CUDA 13.1 GA listed Linux driver `590.44.01`.
- NVIDIA lists GeForce RTX 5090 under compute capability 12.0.

Local state:

```text
GPU: NVIDIA GeForce RTX 5090
Driver: 596.21
CUDA toolkit: 13.2
nvcc: 13.2.51
Compute capability: 12.0
VRAM: 32607 MiB
```

This local driver is above both the CUDA 13.2 Update 1 toolkit driver requirement and the broader CUDA 13.x minor-compatibility threshold.

## Active Local Build Settings

Current local `.cargo/config.toml` environment:

```toml
CUDA_HOME = "/usr/local/cuda-13.2"
CUDA_PATH = "/usr/local/cuda-13.2"
CMAKE_CUDA_COMPILER = "/usr/local/cuda-13.2/bin/nvcc"
CUDAToolkit_ROOT = "/usr/local/cuda-13.2"
CUDACXX = "/usr/local/cuda-13.2/bin/nvcc"
CMAKE_CUDA_ARCHITECTURES = "120a-real"
CUDARC_CUDA_VERSION = "13000"
NVCC_FLAGS = "-Xptxas -O3 --use_fast_math --restrict"
```

Build evidence:

```text
"/usr/local/cuda-13.2/bin/nvcc" "--ptx" "-O3" "-arch" "compute_120a"
```

## Why `compute_120a`

RTX 5090 is Blackwell consumer-class hardware reported by NVIDIA as compute capability 12.0. The CUDA build now targets `compute_120a`, matching the local RTX 5090 path used by the existing CUDA crate.

## Cudarc Constraint

The workspace currently has cudarc versions that do not yet expose a CUDA 13.2 ABI selector. `CUDARC_CUDA_VERSION` remains `13000` so cudarc users compile. This is separate from the native CUDA crate, which physically invokes CUDA 13.2 `nvcc`.

Do not set `CUDARC_CUDA_VERSION=13020` until all cudarc consumers in the dependency graph support that selector.

## Recommended Next Optimizations

1. Keep CUDA native kernels on CUDA 13.2 and `compute_120a`.
2. Promote `.cargo/config.toml` into tracked config or a documented setup script if the team wants reproducible CUDA 13.2 builds.
3. Add a CI/manual guard that fails if CUDA builds fall back to `/usr/local/cuda-13.1/bin/nvcc`.
4. Track cudarc releases and remove the `13000` compatibility pin only after 13.2 is officially supported by all cudarc versions in the workspace.
5. Consider capturing Nsight Compute baselines for E14 BGE-M3 dense forward pass after the pipeline has a stable representative workload.

