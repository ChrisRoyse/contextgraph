---
id: "M04-T09"
title: "Define FAISS FFI Bindings"
description: |
  Implement faiss_ffi module with C bindings to FAISS library.
  Bindings: faiss_index_factory, faiss_StandardGpuResources_new/free,
  faiss_index_cpu_to_gpu, faiss_Index_train, faiss_Index_is_trained,
  faiss_Index_add_with_ids, faiss_Index_search, faiss_IndexIVF_nprobe_set,
  faiss_Index_ntotal, faiss_write_index, faiss_read_index, faiss_Index_free.
  Include GpuResources RAII wrapper with Send + Sync.
layer: "logic"
status: "pending"
priority: "critical"
estimated_hours: 4
sequence: 13
depends_on:
  - "M04-T08a"
spec_refs:
  - "TECH-GRAPH-004 Section 3.1"
files_to_create:
  - path: "crates/context-graph-graph/src/index/faiss_ffi.rs"
    description: "FAISS C FFI bindings and GpuResources RAII wrapper"
files_to_modify:
  - path: "crates/context-graph-graph/src/index/mod.rs"
    description: "Add faiss_ffi module declaration"
test_file: "crates/context-graph-graph/tests/faiss_ffi_tests.rs"
---

## Context

FAISS (Facebook AI Similarity Search) provides GPU-accelerated vector similarity search. The FFI bindings expose the FAISS C API to Rust, enabling IVF-PQ index creation, training, and search operations. This is the foundation for all semantic similarity operations in the knowledge graph. The GpuResources wrapper ensures proper allocation and deallocation of GPU memory.

## Scope

### In Scope
- Define extern "C" declarations for FAISS C API
- Create GpuResources RAII wrapper struct
- Implement MetricType enum (InnerProduct=0, L2=1)
- Link directive for faiss_c library
- Ensure GpuResources is Send + Sync (via unsafe impl)

### Out of Scope
- High-level index wrapper (see M04-T10)
- Index configuration (see M04-T01)
- Search result processing (see M04-T11)

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/index/faiss_ffi.rs

use std::os::raw::{c_char, c_int, c_float, c_long};
use std::ptr::NonNull;

/// Metric type for distance computation
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    /// Inner product (higher = more similar)
    InnerProduct = 0,
    /// L2 (Euclidean) distance (lower = more similar)
    L2 = 1,
}

/// Opaque pointers to FAISS types
#[repr(C)]
pub struct FaissIndex {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FaissGpuResourcesProvider {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FaissStandardGpuResources {
    _private: [u8; 0],
}

// ========== FAISS C API Bindings ==========

#[link(name = "faiss_c")]
extern "C" {
    /// Create index from factory string
    pub fn faiss_index_factory(
        p_index: *mut *mut FaissIndex,
        d: c_int,
        description: *const c_char,
        metric: MetricType,
    ) -> c_int;

    /// Allocate GPU resources
    pub fn faiss_StandardGpuResources_new(
        p_res: *mut *mut FaissStandardGpuResources,
    ) -> c_int;

    /// Free GPU resources
    pub fn faiss_StandardGpuResources_free(
        res: *mut FaissStandardGpuResources,
    );

    /// Transfer index from CPU to GPU
    pub fn faiss_index_cpu_to_gpu(
        provider: *mut FaissGpuResourcesProvider,
        device: c_int,
        index: *const FaissIndex,
        p_out: *mut *mut FaissIndex,
    ) -> c_int;

    /// Cast standard GPU resources to provider
    pub fn faiss_StandardGpuResources_as_GpuResourcesProvider(
        res: *mut FaissStandardGpuResources,
    ) -> *mut FaissGpuResourcesProvider;

    /// Train the index with vectors
    pub fn faiss_Index_train(
        index: *mut FaissIndex,
        n: c_long,
        x: *const c_float,
    ) -> c_int;

    /// Check if index is trained
    pub fn faiss_Index_is_trained(
        index: *const FaissIndex,
    ) -> c_int;

    /// Add vectors with IDs
    pub fn faiss_Index_add_with_ids(
        index: *mut FaissIndex,
        n: c_long,
        x: *const c_float,
        xids: *const c_long,
    ) -> c_int;

    /// Search for k nearest neighbors
    pub fn faiss_Index_search(
        index: *const FaissIndex,
        n: c_long,
        x: *const c_float,
        k: c_long,
        distances: *mut c_float,
        labels: *mut c_long,
    ) -> c_int;

    /// Set nprobe parameter for IVF index
    pub fn faiss_IndexIVF_nprobe_set(
        index: *mut FaissIndex,
        nprobe: c_long,
    ) -> c_int;

    /// Get total number of vectors in index
    pub fn faiss_Index_ntotal(
        index: *const FaissIndex,
    ) -> c_long;

    /// Write index to file
    pub fn faiss_write_index(
        index: *const FaissIndex,
        fname: *const c_char,
    ) -> c_int;

    /// Read index from file
    pub fn faiss_read_index(
        fname: *const c_char,
        io_flags: c_int,
        p_out: *mut *mut FaissIndex,
    ) -> c_int;

    /// Free index
    pub fn faiss_Index_free(
        index: *mut FaissIndex,
    );
}

// ========== RAII Wrapper ==========

/// RAII wrapper for FAISS GPU resources
pub struct GpuResources {
    ptr: NonNull<FaissStandardGpuResources>,
}

impl GpuResources {
    /// Allocate new GPU resources
    pub fn new() -> Result<Self, crate::error::GraphError> {
        let mut res_ptr: *mut FaissStandardGpuResources = std::ptr::null_mut();
        let result = unsafe { faiss_StandardGpuResources_new(&mut res_ptr) };

        if result != 0 {
            return Err(crate::error::GraphError::GpuResourceAllocation(
                format!("Failed to allocate GPU resources, error code: {}", result)
            ));
        }

        NonNull::new(res_ptr)
            .map(|ptr| GpuResources { ptr })
            .ok_or_else(|| crate::error::GraphError::GpuResourceAllocation(
                "GPU resources pointer is null".to_string()
            ))
    }

    /// Get the raw pointer for FFI calls
    pub fn as_ptr(&self) -> *mut FaissStandardGpuResources {
        self.ptr.as_ptr()
    }

    /// Get as GpuResourcesProvider for cpu_to_gpu transfer
    pub fn as_provider(&self) -> *mut FaissGpuResourcesProvider {
        unsafe {
            faiss_StandardGpuResources_as_GpuResourcesProvider(self.ptr.as_ptr())
        }
    }
}

impl Drop for GpuResources {
    fn drop(&mut self) {
        unsafe {
            faiss_StandardGpuResources_free(self.ptr.as_ptr());
        }
    }
}

// SAFETY: GpuResources wraps a pointer to GPU resources allocated by FAISS.
// The underlying FAISS implementation handles thread-safety for GPU operations.
// We ensure single ownership via NonNull and RAII cleanup.
unsafe impl Send for GpuResources {}
unsafe impl Sync for GpuResources {}

impl Default for GpuResources {
    fn default() -> Self {
        Self::new().expect("Failed to allocate default GPU resources")
    }
}

impl std::fmt::Debug for GpuResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuResources")
            .field("ptr", &self.ptr)
            .finish()
    }
}

/// Helper to convert Rust result code to Result
#[inline]
pub fn check_faiss_result(code: c_int, operation: &str) -> Result<(), crate::error::GraphError> {
    if code == 0 {
        Ok(())
    } else {
        Err(crate::error::GraphError::FaissIndexCreation(
            format!("{} failed with error code: {}", operation, code)
        ))
    }
}
```

### Constraints
- All extern "C" declarations must exactly match FAISS C API
- Link directive: #[link(name = "faiss_c")]
- GpuResources must be Send + Sync for multi-threaded use
- Must handle null pointers gracefully
- No panics in FFI boundary code

### Acceptance Criteria
- [ ] All extern "C" declarations compile
- [ ] GpuResources wrapper handles allocation/deallocation
- [ ] GpuResources is Send + Sync
- [ ] MetricType enum with InnerProduct=0, L2=1
- [ ] Link directive: #[link(name = "faiss_c")]
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Define opaque struct types for FAISS pointers
2. Add extern "C" block with all FAISS function declarations
3. Implement GpuResources wrapper with:
   - new() that calls faiss_StandardGpuResources_new
   - Drop that calls faiss_StandardGpuResources_free
   - as_provider() for cpu_to_gpu operations
4. Unsafe impl Send + Sync with safety comment
5. Add helper function for error code checking

### Edge Cases
- Null pointer returned from FAISS: Return GraphError
- GPU not available: GpuResourceAllocation error
- Invalid metric type: Use repr(C) to ensure ABI compatibility
- FAISS library not found: Link error at build time

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph faiss_ffi
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] GpuResources::new() returns Ok on GPU-enabled system
- [ ] GpuResources Drop runs without segfault
- [ ] MetricType::L2 has value 1
- [ ] MetricType::InnerProduct has value 0
- [ ] FFI functions are correctly linked

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_type_values() {
        assert_eq!(MetricType::InnerProduct as i32, 0);
        assert_eq!(MetricType::L2 as i32, 1);
    }

    #[test]
    #[cfg(feature = "gpu")]
    fn test_gpu_resources_allocation() {
        let resources = GpuResources::new();
        assert!(resources.is_ok());
    }

    #[test]
    fn test_gpu_resources_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GpuResources>();
    }
}
```
