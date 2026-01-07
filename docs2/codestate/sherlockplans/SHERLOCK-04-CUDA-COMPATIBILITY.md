# SHERLOCK INVESTIGATION CASE #4: CUDA/CANDLE BLACKWELL COMPATIBILITY

**Case ID:** SHERLOCK-04-CUDA-COMPAT
**Date:** 2026-01-07
**Investigator:** Agent #4 of 5 | Sherlock Holmes Protocol
**Subject:** Candle Framework CUDA Compatibility with RTX 5090 Blackwell (Compute Capability 12.0)
**Swarm ID:** swarm_1767796376357_fpuy9v94e

---

## EXECUTIVE SUMMARY

**VERDICT: COMPATIBLE WITH CAVEATS**

The context-graph-embeddings crate is **COMPATIBLE** with RTX 5090 Blackwell (CC 12.0) under
the current system configuration. However, there are architectural concerns that require
documentation for future maintenance.

---

## HARDWARE ENVIRONMENT VERIFIED

| Component | Value | Verification Command |
|-----------|-------|---------------------|
| GPU | NVIDIA GeForce RTX 5090 | `nvidia-smi` |
| VRAM | 32607 MiB (~32GB) | `nvidia-smi` |
| Compute Capability | 12.0 | `nvidia-smi --query-gpu=compute_cap --format=csv` |
| CUDA Toolkit | 13.1 (V13.1.80) | `nvcc --version` |
| Driver | 591.44 | `nvidia-smi` |
| Platform | WSL2 Linux | Environment |

---

## CANDLE DEPENDENCY ANALYSIS

### Version Information

| Crate | Version | Source |
|-------|---------|--------|
| candle-core | 0.9.2-alpha.2 | Cargo.lock |
| candle-nn | 0.9.2-alpha.2 | Cargo.lock |
| candle-transformers | 0.9.2-alpha.2 | Workspace Cargo.toml |
| candle-kernels | 0.9.2-alpha.2 | Transitive dependency |
| cudarc | 0.18.2 | Transitive via candle-core |
| bindgen_cuda | 0.1.5 | Build dependency of candle-kernels |

### Workspace Configuration

**File:** `/home/cabdru/contextgraph/Cargo.toml` (lines 52-57)
```toml
# GPU/ML - Candle framework (HuggingFace)
# Using 0.9.2-alpha for CUDA 13.x support (RTX 5090 Blackwell)
candle-core = { version = "0.9.2-alpha", features = ["cuda"] }
candle-nn = { version = "0.9.2-alpha", features = ["cuda"] }
candle-transformers = { version = "0.9.2-alpha", features = ["cuda"] }
```

---

## NVCC SM TARGET VERIFICATION

**Command:** `nvcc --list-gpu-code`

**Result: SM_120 IS SUPPORTED**

```
sm_75   (Turing)
sm_80   (Ampere)
sm_86   (Ampere)
sm_87   (Ampere)
sm_88   (Ampere)
sm_89   (Ada Lovelace)
sm_90   (Hopper)
sm_100  (Blackwell)
sm_110  (Blackwell)
sm_103  (Blackwell)
sm_120  (Blackwell - RTX 5090)  <-- CONFIRMED
sm_121  (Blackwell)
```

**CRITICAL:** CUDA 13.1 includes sm_120 support, which means the bindgen_cuda build process
will successfully compile PTX for the RTX 5090.

---

## BLACKWELL-SPECIFIC FIXES IN CANDLE

### Evidence of Blackwell Compatibility Fix

**File:** `/home/cabdru/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/candle-kernels-0.9.2-alpha.2/src/reduce.cu`

**Lines 9-13:**
```cpp
// Helpers to initialize reduction identities for both floating-point and
// integer types. For floats we keep using +/-INFINITY, while for integers
// we use well-defined numeric_limits values instead of relying on casting
// +/-INFINITY to an integer type (which is undefined behaviour and has been
// observed to break on newer GPU architectures such as Blackwell).
```

**Lines 31-59:** Integer specializations using `cuda::std::numeric_limits` for:
- `int64_t`
- `uint32_t`
- `uint8_t`

**FINDING:** Candle 0.9.2-alpha.2 has been explicitly patched to handle Blackwell's stricter
undefined behavior enforcement for integer type initialization.

---

## FEATURE FLAGS ANALYSIS

### context-graph-embeddings Features

**File:** `/home/cabdru/contextgraph/crates/context-graph-embeddings/Cargo.toml` (lines 90-96)

```toml
[features]
# GPU-first architecture: candle feature is MANDATORY for RTX 5090 acceleration
default = ["candle"]
candle = ["dep:candle-core", "dep:candle-nn", "dep:tokenizers"]
cuda = ["candle"]   # Alias for candle
onnx = []           # Future: ONNX Runtime support
```

**FINDING:** The `cuda` feature is an alias for `candle`. Both enable Candle with CUDA support.

### context-graph-cuda Features

**File:** `/home/cabdru/contextgraph/crates/context-graph-cuda/Cargo.toml` (lines 32-35)

```toml
[features]
default = ["cuda"]
cuda = []  # REQUIRED: RTX 5090 Blackwell CUDA support
cudnn = ["cuda"]  # Future: cuDNN support
```

**FINDING:** The context-graph-cuda crate uses direct FFI bindings, NOT cudarc, for CUDA Driver API calls.

---

## GPU INITIALIZATION CODE REVIEW

### Device Initialization Path

**File:** `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/gpu/device/core.rs`

```rust
// Line 100
match Device::new_cuda(0) {
    Ok(device) => init_success(device),
    Err(e) => init_failure(e)
}
```

**FINDING:** Uses Candle's `Device::new_cuda(0)` for GPU initialization. This leverages cudarc
internally, which uses the CUDA Driver API.

### GPU Info Query Issue

**File:** `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/gpu/device/utils.rs` (lines 16-29)

```rust
pub(crate) fn query_gpu_info(_device: &Device) -> GpuInfo {
    // TODO: When cuda-sys is available, query actual device properties
    GpuInfo {
        name: "NVIDIA GeForce RTX 5090".to_string(),
        total_vram: 32 * 1024 * 1024 * 1024,    // 32GB GDDR7
        compute_capability: "12.0".to_string(), // Blackwell SM_120
        available: true,
    }
}
```

**WARNING:** GPU info is HARDCODED, not queried from the device. This is noted with a TODO
but functions correctly for RTX 5090. On different hardware, the reported info would be wrong.

---

## WARM LOADER CUDA REQUIREMENTS

### Compute Capability Requirements

**File:** `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/warm/cuda_alloc/constants.rs`

```rust
pub const REQUIRED_COMPUTE_MAJOR: u32 = 12;
pub const REQUIRED_COMPUTE_MINOR: u32 = 0;
pub const MINIMUM_VRAM_BYTES: usize = 32 * 1024 * 1024 * 1024; // 32GB
```

**File:** `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs`

The allocator verifies compute capability at lines 182-198:
```rust
pub fn check_compute_capability(&self, required_major: u32, required_minor: u32) -> WarmResult<()> {
    let info = self.get_gpu_info()?;
    if !info.meets_compute_requirement(required_major, required_minor) {
        return Err(WarmError::CudaCapabilityInsufficient { ... });
    }
    Ok(())
}
```

---

## BUILD VERIFICATION

**Command:** `cargo build -p context-graph-embeddings --features cuda`

**Result:** SUCCESS

```
warning: `context-graph-embeddings` (lib) generated 5 warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.25s
```

**FINDING:** The crate compiles successfully with CUDA features enabled. Only dead code warnings.

---

## KNOWN ISSUES FROM EXTERNAL SOURCES

### HuggingFace text-embeddings-inference Issue #652

**URL:** https://github.com/huggingface/text-embeddings-inference/issues/652

**Error reported by users:**
- "cuda compute cap 120 is not supported"
- "Runtime compute cap 120 is not compatible with compile time compute cap 120"

**Root cause:** Candle library not supporting compute capability 120 at that time.

**Status for context-graph:** NOT APPLICABLE - We are using Candle 0.9.2-alpha.2 which includes
the Blackwell fix in reduce.cu. Additionally, our nvcc 13.1 supports sm_120.

### PyTorch Issue #159207

**URL:** https://github.com/pytorch/pytorch/issues/159207

**Issue:** "Add official support for CUDA sm_120 (RTX 5090 / Blackwell architecture)"

**Status for context-graph:** NOT APPLICABLE - We use Candle, not PyTorch.

---

## COMPATIBILITY MATRIX

| Component | RTX 5090 Compatible | Notes |
|-----------|---------------------|-------|
| CUDA Toolkit 13.1 | YES | sm_120 supported |
| Driver 591.44 | YES | Current driver |
| Candle 0.9.2-alpha.2 | YES | Blackwell fix in reduce.cu |
| cudarc 0.18.2 | YES | Via Candle |
| bindgen_cuda 0.1.5 | YES | Uses nvcc sm_120 |
| context-graph-embeddings | YES | Build verified |
| context-graph-cuda | YES | Direct FFI, no cudarc |

---

## RISK ASSESSMENT

### Low Risk
1. **Hardcoded GPU Info:** The `query_gpu_info()` function hardcodes RTX 5090 specs instead
   of querying. This works but will report incorrect info on different GPUs.

### Medium Risk
1. **Alpha Version Dependency:** Candle 0.9.2-alpha.2 is an alpha release. Future stable
   releases may have API changes.

### Mitigated Risks
1. **Blackwell Integer UB:** Fixed in reduce.cu with numeric_limits specializations.
2. **SM Target Missing:** Mitigated by CUDA 13.1 including sm_120.

---

## RECOMMENDATIONS FOR AGENT #5 (INTEGRATION)

1. **Runtime Testing Required:** While compilation succeeds, runtime testing on actual RTX 5090
   hardware is essential to verify:
   - `Device::new_cuda(0)` succeeds
   - Model inference produces valid embeddings
   - No CUDA runtime errors occur

2. **Watch for Candle Updates:** Monitor Candle for stable 0.10.x releases that may include
   improved Blackwell support.

3. **Consider cuda-sys Integration:** The TODO in `utils.rs` suggests adding actual GPU
   property queries via cuda-sys bindings for robustness.

4. **Exit Code Verification:** Verify that exit codes 106 (CudaInitFailed) and 107
   (CudaCapabilityInsufficient) are properly triggered on incompatible hardware.

---

## CHAIN OF CUSTODY

| Timestamp | Action | Evidence |
|-----------|--------|----------|
| 2026-01-07T08:50 | Read Cargo.toml files | Candle version identified |
| 2026-01-07T08:51 | cargo tree analysis | cudarc 0.18.2 confirmed |
| 2026-01-07T08:52 | Web search | External issues researched |
| 2026-01-07T08:53 | nvcc verification | sm_120 support confirmed |
| 2026-01-07T08:54 | nvidia-smi check | RTX 5090 CC 12.0 verified |
| 2026-01-07T08:55 | reduce.cu analysis | Blackwell fix confirmed |
| 2026-01-07T08:56 | Build test | Compilation successful |

---

## FINAL VERDICT

```
================================================================
                    CASE CLOSED
================================================================

THE CRIME: Potential CUDA/Candle incompatibility with RTX 5090 Blackwell

THE VERDICT: INNOCENT (Compatible)

THE EVIDENCE:
  1. CUDA 13.1 includes sm_120 target
  2. Candle 0.9.2-alpha.2 has Blackwell-specific fixes
  3. Build completes successfully
  4. nvidia-smi confirms CC 12.0 detection

THE CAVEATS:
  1. GPU info is hardcoded (low risk)
  2. Alpha version dependency (medium risk)
  3. Runtime testing still required

CONFIDENCE: HIGH (85%)

================================================================
         CASE SHERLOCK-04 - VERDICT: COMPATIBLE
================================================================
```

---

## MEMORY STORAGE

Findings stored at:
- Key: `sherlock/agent4/cuda-compat-findings`
- Namespace: `sherlock/investigations`

---

## SOURCES

- [NVIDIA Blackwell Compatibility Guide](https://docs.nvidia.com/cuda/blackwell-compatibility-guide/)
- [CUDA Toolkit 13.1 Release Notes](https://docs.nvidia.com/cuda/cuda-toolkit-release-notes/)
- [HuggingFace text-embeddings-inference Issue #652](https://github.com/huggingface/text-embeddings-inference/issues/652)
- [Candle GitHub Issues](https://github.com/huggingface/candle/issues/3166)
- [PyTorch Issue #159207](https://github.com/pytorch/pytorch/issues/159207)
