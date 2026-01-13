# TASK-ARCH-003: Consolidate FAISS FFI bindings

```xml
<task_spec id="TASK-ARCH-003" version="1.0">
<metadata>
  <title>Consolidate FAISS FFI bindings</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements><requirement_ref>REQ-ARCH-003</requirement_ref></implements>
  <depends_on>TASK-01</depends_on>
  <estimated_hours>4</estimated_hours>
</metadata>

<context>
FAISS FFI bindings are currently in context-graph-graph/src/index/faiss_ffi/bindings.rs.
This task moves all FAISS FFI to the consolidated context-graph-cuda crate for
centralized unsafe code management and security auditing.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-graph/src/index/faiss_ffi/bindings.rs
- /home/cabdru/contextgraph/crates/context-graph-cuda/src/ffi/mod.rs (from TASK-ARCH-001)
</input_context_files>

<scope>
<in_scope>
- Create src/ffi/faiss.rs with all FAISS extern "C" declarations
- Move faiss_index_factory, faiss_index_add, faiss_index_search
- Move faiss_gpu_resources_new, faiss_gpu_resources_free
- Move faiss_gpu_index_new, faiss_gpu_index_free
- Move faiss_index_ntotal, faiss_index_d
- Add proper documentation for each FFI function
- Include link attributes for FAISS library
</in_scope>
<out_of_scope>
- CUDA driver FFI (TASK-ARCH-002)
- Safe wrappers (TASK-ARCH-004)
- Updating consumers to use new crate
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-cuda/src/ffi/faiss.rs
use libc::{c_char, c_int, c_void, size_t};

pub type FaissIndex = *mut c_void;
pub type FaissGpuResources = *mut c_void;
pub type idx_t = i64;

#[link(name = "faiss")]
extern "C" {
    /// Create an index from a factory string (e.g., "Flat", "IVF1024,Flat")
    pub fn faiss_index_factory(
        p_index: *mut FaissIndex,
        d: c_int,
        description: *const c_char,
        metric: c_int,
    ) -> c_int;

    /// Free an index
    pub fn faiss_index_free(index: FaissIndex) -> c_int;

    /// Add vectors to an index
    pub fn faiss_index_add(
        index: FaissIndex,
        n: idx_t,
        x: *const f32,
    ) -> c_int;

    /// Search for nearest neighbors
    pub fn faiss_index_search(
        index: FaissIndex,
        n: idx_t,
        x: *const f32,
        k: idx_t,
        distances: *mut f32,
        labels: *mut idx_t,
    ) -> c_int;

    /// Get total vectors in index
    pub fn faiss_index_ntotal(index: FaissIndex) -> idx_t;

    /// Get index dimension
    pub fn faiss_index_d(index: FaissIndex) -> c_int;

    /// Create GPU resources
    pub fn faiss_gpu_resources_new(res: *mut FaissGpuResources) -> c_int;

    /// Free GPU resources
    pub fn faiss_gpu_resources_free(res: FaissGpuResources) -> c_int;

    /// Copy index to GPU
    pub fn faiss_index_cpu_to_gpu(
        res: FaissGpuResources,
        device: c_int,
        index: FaissIndex,
        p_out: *mut FaissIndex,
    ) -> c_int;

    /// Copy index from GPU to CPU
    pub fn faiss_index_gpu_to_cpu(
        index: FaissIndex,
        p_out: *mut FaissIndex,
    ) -> c_int;
}

// FAISS metric types
pub const METRIC_L2: c_int = 1;
pub const METRIC_INNER_PRODUCT: c_int = 0;

// FAISS error codes
pub const FAISS_OK: c_int = 0;
```
</signatures>
<constraints>
- All extern "C" blocks MUST have #[link(name = "faiss")] attribute
- idx_t MUST be i64 (FAISS convention)
- Metric constants MUST match FAISS headers
- All pointers in FFI MUST be properly documented for ownership
</constraints>
<verification>
```bash
cargo check -p context-graph-cuda
# Verify no FAISS extern "C" outside this crate
grep -r 'extern "C"' crates/context-graph-graph --include="*.rs" | grep -i faiss && exit 1 || echo "OK"
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-cuda/src/ffi/faiss.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-cuda/src/ffi/mod.rs (add faiss module)
</files_to_modify>

<test_commands>
```bash
cargo check -p context-graph-cuda
cargo doc -p context-graph-cuda --no-deps
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Ownership Semantics

FAISS FFI has specific ownership rules:
- `faiss_index_factory` allocates, caller must call `faiss_index_free`
- `faiss_gpu_resources_new` allocates, caller must call `faiss_gpu_resources_free`
- `faiss_index_cpu_to_gpu` creates new GPU index, original CPU index still valid

### Thread Safety

FAISS indices are NOT thread-safe. Documentation should note:
- Multiple readers allowed
- Single writer required
- GPU operations serialize internally

### Error Handling

FAISS returns error codes. The safe wrapper (TASK-ARCH-004) will convert to Result types.
```rust
// In safe wrapper (TASK-ARCH-004):
fn check_faiss_result(code: c_int) -> Result<(), GpuError> {
    if code == FAISS_OK { Ok(()) } else { Err(GpuError::Faiss(format!("code {}", code))) }
}
```
