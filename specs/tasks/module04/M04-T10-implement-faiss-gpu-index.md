---
id: "M04-T10"
title: "Implement FaissGpuIndex Wrapper"
description: |
  Implement FaissGpuIndex struct wrapping FAISS GPU index.
  Methods: new(config), train(vectors), search(queries, k), add_with_ids(vectors, ids),
  ntotal(), save(path), load(path).
  Use NonNull for GPU pointer, Arc<GpuResources> for resource sharing.
  Performance: <5ms for k=10 search on 10M vectors.
layer: "logic"
status: "pending"
priority: "critical"
estimated_hours: 5
sequence: 14
depends_on:
  - "M04-T01"
  - "M04-T09"
spec_refs:
  - "TECH-GRAPH-004 Section 3.2"
  - "REQ-KG-001 through REQ-KG-008"
files_to_create:
  - path: "crates/context-graph-graph/src/index/gpu_index.rs"
    description: "FaissGpuIndex wrapper with full FAISS GPU operations"
files_to_modify:
  - path: "crates/context-graph-graph/src/index/mod.rs"
    description: "Add gpu_index module declaration and re-exports"
test_file: "crates/context-graph-graph/tests/gpu_index_tests.rs"
---

## Context

FaissGpuIndex provides a safe, high-level Rust interface to FAISS GPU-accelerated IVF-PQ index. This is the core component for semantic similarity search in the knowledge graph, supporting 10M+ vectors with sub-5ms query latency. The wrapper handles index lifecycle, training, vector addition, and k-NN search while ensuring proper GPU resource management.

## Scope

### In Scope
- FaissGpuIndex struct with RAII resource management
- Index creation using factory string from IndexConfig
- Training with minimum vector requirements
- k-NN search with configurable k
- Vector addition with IDs
- Index serialization to/from disk
- Thread-safe design with Arc<GpuResources>

### Out of Scope
- FFI bindings (see M04-T09)
- SearchResult processing (see M04-T11)
- Configuration types (see M04-T01)

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/index/gpu_index.rs

use std::ffi::CString;
use std::path::Path;
use std::ptr::NonNull;
use std::sync::Arc;

use crate::config::IndexConfig;
use crate::error::{GraphError, GraphResult};
use super::faiss_ffi::{
    FaissIndex, GpuResources, MetricType,
    faiss_index_factory, faiss_index_cpu_to_gpu, faiss_Index_train,
    faiss_Index_is_trained, faiss_Index_add_with_ids, faiss_Index_search,
    faiss_IndexIVF_nprobe_set, faiss_Index_ntotal, faiss_write_index,
    faiss_read_index, faiss_Index_free, check_faiss_result,
};

/// GPU-accelerated FAISS IVF-PQ index
pub struct FaissGpuIndex {
    /// Raw pointer to GPU index (NonNull for safety guarantees)
    index: NonNull<FaissIndex>,
    /// Shared GPU resources
    resources: Arc<GpuResources>,
    /// Index configuration
    config: IndexConfig,
    /// Whether the index has been trained
    is_trained: bool,
}

impl FaissGpuIndex {
    /// Create a new FAISS GPU index from configuration
    ///
    /// # Arguments
    /// * `config` - Index configuration with dimension, nlist, pq_segments, etc.
    ///
    /// # Returns
    /// * `GraphResult<Self>` - New index or error if creation fails
    pub fn new(config: IndexConfig) -> GraphResult<Self> {
        let resources = Arc::new(GpuResources::new()?);
        Self::with_resources(config, resources)
    }

    /// Create index with shared GPU resources
    ///
    /// # Arguments
    /// * `config` - Index configuration
    /// * `resources` - Shared GPU resources (for multi-index scenarios)
    pub fn with_resources(config: IndexConfig, resources: Arc<GpuResources>) -> GraphResult<Self> {
        // Create CPU index first
        let factory_string = CString::new(config.factory_string())
            .map_err(|e| GraphError::InvalidConfig(format!("Invalid factory string: {}", e)))?;

        let mut cpu_index: *mut FaissIndex = std::ptr::null_mut();
        check_faiss_result(
            unsafe {
                faiss_index_factory(
                    &mut cpu_index,
                    config.dimension as i32,
                    factory_string.as_ptr(),
                    MetricType::L2,
                )
            },
            "faiss_index_factory",
        )?;

        // Transfer to GPU
        let mut gpu_index: *mut FaissIndex = std::ptr::null_mut();
        check_faiss_result(
            unsafe {
                faiss_index_cpu_to_gpu(
                    resources.as_provider(),
                    config.gpu_id as i32,
                    cpu_index,
                    &mut gpu_index,
                )
            },
            "faiss_index_cpu_to_gpu",
        )?;

        // Free CPU index (GPU copy owns data now)
        unsafe { faiss_Index_free(cpu_index) };

        let index = NonNull::new(gpu_index)
            .ok_or_else(|| GraphError::GpuResourceAllocation(
                "GPU index pointer is null".to_string()
            ))?;

        Ok(Self {
            index,
            resources,
            config,
            is_trained: false,
        })
    }

    /// Train the index with vectors
    ///
    /// # Arguments
    /// * `vectors` - Training vectors (n_vectors * dimension f32 values)
    ///
    /// # Errors
    /// * `InsufficientTrainingData` if vectors.len() < config.min_train_vectors
    /// * `FaissTrainingFailed` if FAISS training fails
    pub fn train(&mut self, vectors: &[f32]) -> GraphResult<()> {
        let n_vectors = vectors.len() / self.config.dimension;

        if n_vectors < self.config.min_train_vectors {
            return Err(GraphError::InsufficientTrainingData {
                provided: n_vectors,
                required: self.config.min_train_vectors,
            });
        }

        check_faiss_result(
            unsafe {
                faiss_Index_train(
                    self.index.as_ptr(),
                    n_vectors as i64,
                    vectors.as_ptr(),
                )
            },
            "faiss_Index_train",
        ).map_err(|_| GraphError::FaissTrainingFailed(
            format!("Training failed with {} vectors", n_vectors)
        ))?;

        // Set nprobe after training
        check_faiss_result(
            unsafe {
                faiss_IndexIVF_nprobe_set(
                    self.index.as_ptr(),
                    self.config.nprobe as i64,
                )
            },
            "faiss_IndexIVF_nprobe_set",
        )?;

        self.is_trained = true;
        Ok(())
    }

    /// Check if the index is trained
    pub fn is_trained(&self) -> bool {
        self.is_trained
    }

    /// Search for k nearest neighbors
    ///
    /// # Arguments
    /// * `queries` - Query vectors (n_queries * dimension f32 values)
    /// * `k` - Number of neighbors to return
    ///
    /// # Returns
    /// * `SearchResult` with ids and distances for each query
    ///
    /// # Performance
    /// * Target: <5ms for k=10 on 10M vectors
    pub fn search(&self, queries: &[f32], k: usize) -> GraphResult<SearchResult> {
        if !self.is_trained {
            return Err(GraphError::IndexNotTrained);
        }

        let n_queries = queries.len() / self.config.dimension;
        let total_results = n_queries * k;

        let mut distances = vec![0.0f32; total_results];
        let mut ids = vec![-1i64; total_results];

        check_faiss_result(
            unsafe {
                faiss_Index_search(
                    self.index.as_ptr(),
                    n_queries as i64,
                    queries.as_ptr(),
                    k as i64,
                    distances.as_mut_ptr(),
                    ids.as_mut_ptr(),
                )
            },
            "faiss_Index_search",
        ).map_err(|_| GraphError::FaissSearchFailed(
            format!("Search failed for {} queries with k={}", n_queries, k)
        ))?;

        Ok(SearchResult {
            ids,
            distances,
            k,
            num_queries: n_queries,
        })
    }

    /// Add vectors with IDs to the index
    ///
    /// # Arguments
    /// * `vectors` - Vectors to add (n_vectors * dimension f32 values)
    /// * `ids` - Vector IDs (one per vector)
    ///
    /// # Note
    /// Index must be trained before adding vectors
    pub fn add_with_ids(&mut self, vectors: &[f32], ids: &[i64]) -> GraphResult<()> {
        if !self.is_trained {
            return Err(GraphError::IndexNotTrained);
        }

        let n_vectors = vectors.len() / self.config.dimension;

        if n_vectors != ids.len() {
            return Err(GraphError::InvalidConfig(
                format!("Vector count {} doesn't match ID count {}", n_vectors, ids.len())
            ));
        }

        check_faiss_result(
            unsafe {
                faiss_Index_add_with_ids(
                    self.index.as_ptr(),
                    n_vectors as i64,
                    vectors.as_ptr(),
                    ids.as_ptr(),
                )
            },
            "faiss_Index_add_with_ids",
        ).map_err(|_| GraphError::FaissAddFailed(
            format!("Failed to add {} vectors", n_vectors)
        ))?;

        Ok(())
    }

    /// Get total number of vectors in index
    pub fn ntotal(&self) -> usize {
        unsafe { faiss_Index_ntotal(self.index.as_ptr()) as usize }
    }

    /// Save index to file
    ///
    /// # Arguments
    /// * `path` - File path to save index
    pub fn save<P: AsRef<Path>>(&self, path: P) -> GraphResult<()> {
        let path_str = path.as_ref().to_string_lossy();
        let c_path = CString::new(path_str.as_ref())
            .map_err(|e| GraphError::InvalidConfig(format!("Invalid path: {}", e)))?;

        check_faiss_result(
            unsafe { faiss_write_index(self.index.as_ptr(), c_path.as_ptr()) },
            "faiss_write_index",
        )?;

        Ok(())
    }

    /// Load index from file
    ///
    /// # Arguments
    /// * `path` - File path to load index from
    /// * `config` - Index configuration (must match saved index)
    pub fn load<P: AsRef<Path>>(path: P, config: IndexConfig) -> GraphResult<Self> {
        let resources = Arc::new(GpuResources::new()?);
        Self::load_with_resources(path, config, resources)
    }

    /// Load index from file with shared resources
    pub fn load_with_resources<P: AsRef<Path>>(
        path: P,
        config: IndexConfig,
        resources: Arc<GpuResources>,
    ) -> GraphResult<Self> {
        let path_str = path.as_ref().to_string_lossy();
        let c_path = CString::new(path_str.as_ref())
            .map_err(|e| GraphError::InvalidConfig(format!("Invalid path: {}", e)))?;

        // Load CPU index
        let mut cpu_index: *mut FaissIndex = std::ptr::null_mut();
        check_faiss_result(
            unsafe { faiss_read_index(c_path.as_ptr(), 0, &mut cpu_index) },
            "faiss_read_index",
        )?;

        // Transfer to GPU
        let mut gpu_index: *mut FaissIndex = std::ptr::null_mut();
        check_faiss_result(
            unsafe {
                faiss_index_cpu_to_gpu(
                    resources.as_provider(),
                    config.gpu_id as i32,
                    cpu_index,
                    &mut gpu_index,
                )
            },
            "faiss_index_cpu_to_gpu",
        )?;

        // Free CPU index
        unsafe { faiss_Index_free(cpu_index) };

        let index = NonNull::new(gpu_index)
            .ok_or_else(|| GraphError::GpuResourceAllocation(
                "Loaded GPU index pointer is null".to_string()
            ))?;

        // Check if loaded index is trained
        let is_trained = unsafe { faiss_Index_is_trained(index.as_ptr()) } != 0;

        Ok(Self {
            index,
            resources,
            config,
            is_trained,
        })
    }

    /// Get the index configuration
    pub fn config(&self) -> &IndexConfig {
        &self.config
    }

    /// Get reference to shared GPU resources
    pub fn resources(&self) -> &Arc<GpuResources> {
        &self.resources
    }
}

impl Drop for FaissGpuIndex {
    fn drop(&mut self) {
        unsafe {
            faiss_Index_free(self.index.as_ptr());
        }
    }
}

// SAFETY: FaissGpuIndex owns its GPU resources via Arc<GpuResources>.
// FAISS GPU operations are internally synchronized.
// All mutable operations take &mut self ensuring exclusive access.
unsafe impl Send for FaissGpuIndex {}
unsafe impl Sync for FaissGpuIndex {}

impl std::fmt::Debug for FaissGpuIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaissGpuIndex")
            .field("ntotal", &self.ntotal())
            .field("is_trained", &self.is_trained)
            .field("config", &self.config)
            .finish()
    }
}
```

### Constraints
- Use NonNull for GPU pointer safety
- Arc<GpuResources> enables multi-index GPU sharing
- train() MUST validate min_train_vectors requirement
- train() MUST set nprobe after successful training
- search() MUST return GraphError::IndexNotTrained if not trained
- Drop MUST free GPU resources correctly
- Must be Send + Sync for multi-threaded use

### Acceptance Criteria
- [ ] new() creates IVF-PQ index and transfers to GPU
- [ ] train() requires min_train_vectors, sets nprobe after training
- [ ] search() returns SearchResult with ids and distances
- [ ] add_with_ids() adds vectors incrementally (no rebuild)
- [ ] Drop impl frees GPU resources correctly
- [ ] Send + Sync implemented (unsafe)
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. new(config):
   - Allocate GpuResources
   - Create CPU index with factory string
   - Transfer to GPU with faiss_index_cpu_to_gpu
   - Free CPU index (GPU owns data)
   - Return wrapped FaissGpuIndex

2. train(vectors):
   - Validate vector count >= min_train_vectors
   - Call faiss_Index_train
   - Set nprobe with faiss_IndexIVF_nprobe_set
   - Mark is_trained = true

3. search(queries, k):
   - Check is_trained
   - Allocate output buffers
   - Call faiss_Index_search
   - Return SearchResult

4. Drop:
   - Call faiss_Index_free (GpuResources freed via Arc)

### Edge Cases
- Zero vectors for training: Return InsufficientTrainingData
- Search on untrained index: Return IndexNotTrained
- Vector/ID count mismatch: Return InvalidConfig
- GPU allocation failure: Return GpuResourceAllocation
- File not found for load: Return appropriate error

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph gpu_index
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Index creation succeeds on GPU-enabled system
- [ ] Training with 4M+ vectors succeeds
- [ ] Search returns k results per query
- [ ] Save/load roundtrip preserves index
- [ ] Drop runs without segfault

### Performance Test
```rust
#[test]
#[cfg(feature = "gpu")]
fn test_search_performance() {
    // Create and train index with 10M vectors
    // Time search with k=10
    // Assert latency < 5ms
}
```
