# TASK-ARCH-004: Implement safe GpuDevice RAII wrapper

```xml
<task_spec id="TASK-ARCH-004" version="1.0">
<metadata>
  <title>Implement safe GpuDevice RAII wrapper</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements><requirement_ref>REQ-ARCH-004</requirement_ref></implements>
  <depends_on>TASK-02, TASK-03</depends_on>
  <estimated_hours>3</estimated_hours>
</metadata>

<context>
Raw FFI calls require manual resource management. RAII wrappers ensure automatic cleanup
via Drop trait, preventing resource leaks on error paths. This is critical for GPU resources
which can exhaust device memory if not properly freed.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-cuda/src/ffi/cuda_driver.rs (from TASK-ARCH-002)
- /home/cabdru/contextgraph/crates/context-graph-cuda/src/ffi/faiss.rs (from TASK-ARCH-003)
</input_context_files>

<scope>
<in_scope>
- Create src/safe/device.rs with GpuDevice struct
- Implement GpuDevice::new() with cuInit, cuDeviceGet
- Implement GpuDevice::compute_capability() returning (major, minor)
- Implement GpuDevice::name() returning String
- Implement GpuDevice::memory_info() returning (free, total)
- Implement Drop for GpuDevice (cleanup context)
- Create src/safe/faiss.rs with GpuResources struct
- Implement GpuResources::new() and Drop
- Implement GpuIndex RAII wrapper
</in_scope>
<out_of_scope>
- Green Contexts (TASK-EMBED-001)
- CI gate script (TASK-ARCH-005)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-cuda/src/safe/device.rs
use crate::error::GpuError;

pub struct GpuDevice {
    device: CUdevice,
    context: CUcontext,
    ordinal: i32,
}

impl GpuDevice {
    /// Create a new GPU device handle.
    ///
    /// # Arguments
    /// * `ordinal` - GPU device index (0-based)
    ///
    /// # Errors
    /// Returns error if device not available or CUDA init fails.
    pub fn new(ordinal: i32) -> Result<Self, GpuError>;

    /// Get compute capability (major, minor).
    pub fn compute_capability(&self) -> (u32, u32);

    /// Get device name.
    pub fn name(&self) -> String;

    /// Get memory info (free_bytes, total_bytes).
    pub fn memory_info(&self) -> Result<(usize, usize), GpuError>;

    /// Get device ordinal.
    pub fn ordinal(&self) -> i32;
}

impl Drop for GpuDevice {
    fn drop(&mut self);
}

// crates/context-graph-cuda/src/safe/faiss.rs
pub struct GpuResources {
    handle: FaissGpuResources,
}

impl GpuResources {
    pub fn new() -> Result<Self, GpuError>;
    pub fn handle(&self) -> FaissGpuResources;
}

impl Drop for GpuResources {
    fn drop(&mut self);
}

pub struct GpuIndex {
    index: FaissIndex,
    resources: Arc<GpuResources>,
}

impl GpuIndex {
    pub fn from_cpu(cpu_index: FaissIndex, resources: Arc<GpuResources>, device: i32) -> Result<Self, GpuError>;
    pub fn search(&self, vectors: &[f32], k: usize) -> Result<(Vec<f32>, Vec<i64>), GpuError>;
}

impl Drop for GpuIndex {
    fn drop(&mut self);
}
```
</signatures>
<constraints>
- GpuDevice MUST call cuCtxDestroy in Drop
- GpuResources MUST call faiss_gpu_resources_free in Drop
- GpuIndex MUST call faiss_index_free in Drop
- All constructors MUST return Result, never panic on GPU errors
- Drop implementations MUST NOT panic (log errors instead)
</constraints>
<verification>
```bash
cargo test -p context-graph-cuda
cargo clippy -p context-graph-cuda -- -D warnings
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-cuda/src/safe/device.rs
- crates/context-graph-cuda/src/safe/faiss.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-cuda/src/safe/mod.rs (add device, faiss modules)
- crates/context-graph-cuda/src/lib.rs (re-export safe types)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-cuda
cargo test -p context-graph-cuda gpu_device -- --ignored  # GPU tests
```
</test_commands>
</task_spec>
```

## Implementation Notes

### RAII Pattern

```rust
impl Drop for GpuDevice {
    fn drop(&mut self) {
        // MUST NOT panic - log error instead
        unsafe {
            let result = cuCtxDestroy(self.context);
            if result != CUDA_SUCCESS {
                tracing::error!(
                    "Failed to destroy CUDA context for device {}: error {}",
                    self.ordinal,
                    result
                );
            }
        }
    }
}
```

### Thread Safety

GpuDevice should be `Send` but NOT `Sync`:
- Can be moved between threads
- Should not be shared across threads (CUDA contexts are thread-bound)

```rust
unsafe impl Send for GpuDevice {}
// Explicitly NOT implementing Sync
```

### Error Handling

All FFI errors must be converted to GpuError:
```rust
fn check_cuda_result(result: CUresult, context: &str) -> Result<(), GpuError> {
    if result == CUDA_SUCCESS {
        Ok(())
    } else {
        Err(GpuError::CudaDriver(format!("{}: error {}", context, result)))
    }
}
```
