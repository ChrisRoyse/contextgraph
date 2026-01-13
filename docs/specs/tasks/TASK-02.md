# TASK-ARCH-002: Consolidate CUDA driver FFI bindings

```xml
<task_spec id="TASK-ARCH-002" version="1.0">
<metadata>
  <title>Consolidate CUDA driver FFI bindings</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements><requirement_ref>REQ-ARCH-002</requirement_ref></implements>
  <depends_on>TASK-01</depends_on>
  <estimated_hours>4</estimated_hours>
</metadata>

<context>
CUDA driver API calls (cuInit, cuDeviceGet, cuCtxCreate, etc.) are currently duplicated
in context-graph-embeddings/src/gpu/device/utils.rs and warm/cuda_alloc/allocator_cuda.rs.
This task moves all CUDA driver FFI to the consolidated crate.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-embeddings/src/gpu/device/utils.rs
- /home/cabdru/contextgraph/crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs
- /home/cabdru/contextgraph/crates/context-graph-cuda/src/ffi/mod.rs (from TASK-ARCH-001)
</input_context_files>

<scope>
<in_scope>
- Create src/ffi/cuda_driver.rs with all CUDA driver extern "C" declarations
- Move cuInit, cuDeviceGet, cuDeviceGetCount, cuCtxCreate, cuCtxDestroy
- Move cuMemAlloc, cuMemFree, cuMemcpy, cuMemGetInfo
- Move cuDeviceGetAttribute, cuDeviceGetName
- Add proper documentation for each FFI function
- Include link attributes for CUDA library
</in_scope>
<out_of_scope>
- FAISS FFI (TASK-ARCH-003)
- Safe wrappers (TASK-ARCH-004)
- Updating consumers to use new crate (separate migration task)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-cuda/src/ffi/cuda_driver.rs
#[link(name = "cuda")]
extern "C" {
    pub fn cuInit(flags: libc::c_uint) -> CUresult;
    pub fn cuDeviceGet(device: *mut CUdevice, ordinal: libc::c_int) -> CUresult;
    pub fn cuDeviceGetCount(count: *mut libc::c_int) -> CUresult;
    pub fn cuDeviceGetAttribute(
        pi: *mut libc::c_int,
        attrib: CUdevice_attribute,
        dev: CUdevice,
    ) -> CUresult;
    pub fn cuCtxCreate(
        pctx: *mut CUcontext,
        flags: libc::c_uint,
        dev: CUdevice,
    ) -> CUresult;
    pub fn cuCtxDestroy(ctx: CUcontext) -> CUresult;
    pub fn cuMemAlloc(dptr: *mut CUdeviceptr, bytesize: libc::size_t) -> CUresult;
    pub fn cuMemFree(dptr: CUdeviceptr) -> CUresult;
    pub fn cuMemGetInfo(free: *mut libc::size_t, total: *mut libc::size_t) -> CUresult;
}

pub type CUresult = libc::c_int;
pub type CUdevice = libc::c_int;
pub type CUcontext = *mut libc::c_void;
pub type CUdeviceptr = libc::c_ulonglong;
pub type CUdevice_attribute = libc::c_int;

// CUDA result codes
pub const CUDA_SUCCESS: CUresult = 0;
pub const CUDA_ERROR_NOT_INITIALIZED: CUresult = 3;
pub const CUDA_ERROR_INVALID_DEVICE: CUresult = 101;

// Device attributes
pub const CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR: CUdevice_attribute = 75;
pub const CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR: CUdevice_attribute = 76;
```
</signatures>
<constraints>
- All extern "C" blocks MUST have #[link(name = "cuda")] attribute
- All FFI functions MUST be marked unsafe implicitly (extern "C")
- Type aliases MUST use libc types for portability
- CUDA error codes MUST be documented with their meaning
</constraints>
<verification>
```bash
cargo check -p context-graph-cuda
# Verify no CUDA extern "C" outside this crate
grep -r 'extern "C"' crates/context-graph-embeddings --include="*.rs" | grep -v "context-graph-cuda" | grep -i cuda && exit 1 || echo "OK"
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-cuda/src/ffi/cuda_driver.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-cuda/src/ffi/mod.rs (add cuda_driver module)
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

### Type Safety

All CUDA types should be defined as type aliases to ensure:
1. Consistent sizes across platforms
2. Clear documentation of intent
3. Easy migration if CUDA changes types

### Link Attribute

The `#[link(name = "cuda")]` attribute tells the linker to link against libcuda.so/cuda.dll.
This is a runtime dependency - the crate will compile without CUDA installed but fail at runtime.

### Documentation Pattern

Each FFI function should have:
```rust
/// Initialize the CUDA driver.
///
/// # Safety
///
/// Must be called before any other CUDA function.
/// Thread-safe if called with same flags.
///
/// # Returns
///
/// CUDA_SUCCESS (0) on success, error code otherwise.
```
