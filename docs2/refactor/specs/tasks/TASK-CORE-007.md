# TASK-CORE-007: Per-Embedder HNSW Index Integration

```xml
<task_spec id="TASK-CORE-007" version="3.0">
<metadata>
  <title>Per-Embedder HNSW Index Integration</title>
  <status>DONE</status>
  <completed_at>2026-01-09</completed_at>
  <layer>foundation</layer>
  <sequence>7</sequence>
  <implements>
    <requirement_ref>ARCH-01: TeleologicalArray is atomic storage unit</requirement_ref>
    <requirement_ref>ARCH-02: Compare only compatible embedding types</requirement_ref>
    <requirement_ref>ARCH-05: All 13 embedders must be present</requirement_ref>
    <requirement_ref>REQ-LATENCY-01: 60ms search at 1M memories</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="COMPLETED">TASK-CORE-006: Storage Trait Alignment</task_ref>
    <task_ref status="COMPLETED">TASK-CORE-002: Embedder enum</task_ref>
    <task_ref status="COMPLETED">TASK-CORE-004: ComparisonValidationError</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
</metadata>

<critical_context>
## WHAT ALREADY EXISTS - DO NOT RECREATE

The codebase has SUBSTANTIAL per-embedder index infrastructure already built:

### 1. HNSW Configuration Module (FULLY IMPLEMENTED)
**Location**: `crates/context-graph-storage/src/teleological/indexes/`

```
indexes/
├── mod.rs                    # Re-exports all types
├── hnsw_config/
│   ├── mod.rs               # Module re-exports
│   ├── config.rs            # HnswConfig, InvertedIndexConfig structs
│   ├── constants.rs         # E1_DIM=1024, E2_DIM=512, etc. (13 embedders)
│   ├── distance.rs          # DistanceMetric enum (5 variants)
│   ├── embedder.rs          # EmbedderIndex enum (15 variants, 12 HNSW)
│   └── functions.rs         # get_hnsw_config(), all_hnsw_configs()
└── metrics.rs               # compute_distance(), cosine_similarity()
```

### 2. EmbedderIndex Enum (EXISTS - 15 variants)
**Location**: `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs`

```rust
pub enum EmbedderIndex {
    E1Semantic,           // 1024D dense, Cosine
    E1Matryoshka128,      // 128D truncated, Cosine (Stage 2)
    E2TemporalRecent,     // 512D dense
    E3TemporalPeriodic,   // 512D dense
    E4TemporalPositional, // 512D dense
    E5Causal,             // 768D dense, AsymmetricCosine
    E6Sparse,             // 30522 vocab, INVERTED INDEX (not HNSW)
    E7Code,               // 1536D dense
    E8Graph,              // 384D dense
    E9HDC,                // 1024D projected
    E10Multimodal,        // 768D dense
    E11Entity,            // 384D dense
    E12LateInteraction,   // 128D per-token, MaxSim (not HNSW)
    E13Splade,            // 30522 vocab, INVERTED INDEX (not HNSW)
    PurposeVector,        // 13D purpose alignment
}
```

### 3. HnswConfig Struct (EXISTS)
**Location**: `crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs`

```rust
pub struct HnswConfig {
    pub m: usize,              // 16 default
    pub ef_construction: usize, // 200 default
    pub ef_search: usize,       // 100 default
    pub metric: DistanceMetric,
    pub dimension: usize,
}
```

### 4. Core Embedder Enum (SINGLE SOURCE OF TRUTH)
**Location**: `crates/context-graph-core/src/teleological/embedder.rs`

```rust
pub enum Embedder {
    Semantic = 0,         // E1
    TemporalRecent = 1,   // E2
    TemporalPeriodic = 2, // E3
    TemporalPositional = 3, // E4
    Causal = 4,           // E5
    Sparse = 5,           // E6
    Code = 6,             // E7
    Graph = 7,            // E8
    Hdc = 8,              // E9
    Multimodal = 9,       // E10
    Entity = 10,          // E11
    LateInteraction = 11, // E12
    KeywordSplade = 12,   // E13
}
```

### 5. TeleologicalMemoryStore Trait (EXISTS - 702 lines)
**Location**: `crates/context-graph-core/src/traits/teleological_memory_store.rs`

Has methods: `store()`, `retrieve()`, `search_semantic()`, `search_sparse()`, `search_purpose()`

### 6. RocksDbTeleologicalStore (EXISTS - ~1600 lines)
**Location**: `crates/context-graph-storage/src/teleological/rocksdb_store.rs`

- 20 column families configured
- TeleologicalStoreError enum defined
- Full CRUD implementation

## WHAT DOES NOT EXIST (THIS TASK MUST CREATE)

1. **EmbedderIndexOps Trait** - A trait that wraps HNSW index operations per-embedder
2. **HnswEmbedderIndex Struct** - Concrete implementation using HNSW library
3. **Index Registry** - Registry/factory to get/create indexes by embedder
4. **Integration** - Wire HnswConfig to actual HNSW implementation
</critical_context>

<objective>
Create an `EmbedderIndexOps` trait that provides a unified interface for per-embedder approximate nearest neighbor (ANN) search. Implement this trait using the existing `HnswConfig` infrastructure and integrate it with `RocksDbTeleologicalStore`.

The goal is to enable Stage 2 (Matryoshka 128D) and Stage 3 (full embeddings) of the 5-stage retrieval pipeline.
</objective>

<rationale>
Per constitution.yaml:
- ARCH-02 mandates "apples-to-apples" comparison - can only compare same embedder type
- REQ-LATENCY-01 requires 60ms search at 1M memories - requires HNSW indexes
- 5-stage pipeline Stage 2 uses E1 Matryoshka128, Stage 3 uses 10 full embeddings

Currently `search_semantic()` does brute-force scan. This task enables ANN search.
</rationale>

<scope>
  <in_scope>
    <item>Define `EmbedderIndexOps` trait for per-embedder ANN search operations</item>
    <item>Implement `HnswEmbedderIndex` using existing HnswConfig + HNSW library</item>
    <item>Create `EmbedderIndexRegistry` to manage 12 HNSW indexes (E1-E5,E7-E11,Matryoshka,Purpose)</item>
    <item>Integrate with existing column family infrastructure</item>
    <item>Tests using real data, not mocks</item>
  </in_scope>
  <out_of_scope>
    <item>InvertedIndex for E6/E13 sparse (separate task)</item>
    <item>ColBERT MaxSim for E12 (separate task)</item>
    <item>RocksDB schema changes (TASK-CORE-008)</item>
    <item>Full 5-stage pipeline orchestration (TASK-LOGIC-008)</item>
  </out_of_scope>
</scope>

<input_context_files>
  <!-- EXISTING - READ FIRST -->
  <file purpose="hnsw_config" critical="true">crates/context-graph-storage/src/teleological/indexes/mod.rs</file>
  <file purpose="embedder_enum" critical="true">crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs</file>
  <file purpose="config_struct" critical="true">crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs</file>
  <file purpose="distance_metrics" critical="true">crates/context-graph-storage/src/teleological/indexes/metrics.rs</file>
  <file purpose="core_embedder" critical="true">crates/context-graph-core/src/teleological/embedder.rs</file>
  <file purpose="trait" critical="true">crates/context-graph-core/src/traits/teleological_memory_store.rs</file>
  <file purpose="rocksdb_impl" critical="true">crates/context-graph-storage/src/teleological/rocksdb_store.rs</file>
  <file purpose="column_families">crates/context-graph-storage/src/teleological/column_families.rs</file>
  <file purpose="constitution">docs2/constitution.yaml</file>
</input_context_files>

<definition_of_done>

## File 1: EmbedderIndexOps Trait
**Path**: `crates/context-graph-storage/src/teleological/indexes/embedder_index.rs`

```rust
//! Per-embedder ANN index trait.
//!
//! FAIL FAST: Invalid operations panic. No fallbacks.

use super::{DistanceMetric, EmbedderIndex, HnswConfig};
use uuid::Uuid;

/// Result type for index operations.
pub type IndexResult<T> = Result<T, IndexError>;

/// Errors from index operations. FAIL FAST - no recovery.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("Dimension mismatch: expected {expected}, got {actual} for {embedder:?}")]
    DimensionMismatch {
        embedder: EmbedderIndex,
        expected: usize,
        actual: usize,
    },

    #[error("Index not found for {embedder:?}")]
    IndexNotFound { embedder: EmbedderIndex },

    #[error("Index operation failed for {embedder:?}: {message}")]
    OperationFailed {
        embedder: EmbedderIndex,
        message: String,
    },

    #[error("Index is read-only, cannot insert")]
    ReadOnly,

    #[error("Invalid vector: {message}")]
    InvalidVector { message: String },
}

/// Trait for per-embedder approximate nearest neighbor index.
///
/// Each embedder (E1-E13) has its own index with embedder-specific
/// configuration (dimension, distance metric).
///
/// # FAIL FAST
///
/// All methods validate inputs and panic on invariant violations.
/// Use Result only for expected failure modes (not found, etc).
pub trait EmbedderIndexOps: Send + Sync {
    /// Get the embedder this index serves.
    fn embedder(&self) -> EmbedderIndex;

    /// Get the HNSW configuration for this index.
    fn config(&self) -> &HnswConfig;

    /// Number of vectors in the index.
    fn len(&self) -> usize;

    /// Check if empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Insert a vector with associated ID.
    ///
    /// # Panics
    /// - If vector dimension != config.dimension
    /// - If vector contains NaN or Inf
    fn insert(&self, id: Uuid, vector: &[f32]) -> IndexResult<()>;

    /// Remove a vector by ID.
    ///
    /// Returns true if removed, false if not found.
    fn remove(&self, id: Uuid) -> IndexResult<bool>;

    /// Search for k nearest neighbors.
    ///
    /// # Arguments
    /// * `query` - Query vector (must match dimension)
    /// * `k` - Number of neighbors to return
    /// * `ef_search` - Optional override for HNSW ef parameter
    ///
    /// # Returns
    /// Vector of (id, distance) pairs sorted by distance ascending.
    ///
    /// # Panics
    /// - If query dimension != config.dimension
    /// - If query contains NaN or Inf
    fn search(&self, query: &[f32], k: usize, ef_search: Option<usize>) -> IndexResult<Vec<(Uuid, f32)>>;

    /// Batch insert multiple vectors.
    ///
    /// More efficient than individual inserts for bulk loading.
    fn insert_batch(&self, items: &[(Uuid, Vec<f32>)]) -> IndexResult<usize>;

    /// Flush any pending writes to storage.
    fn flush(&self) -> IndexResult<()>;

    /// Get memory usage in bytes.
    fn memory_bytes(&self) -> usize;
}

/// Validation helper - FAIL FAST on invalid vectors.
#[inline]
pub fn validate_vector(vector: &[f32], expected_dim: usize, embedder: EmbedderIndex) -> IndexResult<()> {
    if vector.len() != expected_dim {
        return Err(IndexError::DimensionMismatch {
            embedder,
            expected: expected_dim,
            actual: vector.len(),
        });
    }

    for (i, &v) in vector.iter().enumerate() {
        if !v.is_finite() {
            return Err(IndexError::InvalidVector {
                message: format!("Non-finite value at index {}: {}", i, v),
            });
        }
    }

    Ok(())
}
```

## File 2: HNSW Implementation
**Path**: `crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs`

```rust
//! HNSW index implementation using instant-distance crate.
//!
//! Each HnswEmbedderIndex wraps an instant_distance::Hnsw with
//! configuration from HnswConfig.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use super::{EmbedderIndex, HnswConfig, DistanceMetric};
use super::embedder_index::{EmbedderIndexOps, IndexError, IndexResult, validate_vector};

/// HNSW index for a single embedder.
pub struct HnswEmbedderIndex {
    embedder: EmbedderIndex,
    config: HnswConfig,
    // Internal HNSW structure
    id_to_idx: RwLock<HashMap<Uuid, usize>>,
    idx_to_id: RwLock<Vec<Uuid>>,
    vectors: RwLock<Vec<Vec<f32>>>,
}

impl HnswEmbedderIndex {
    /// Create new index for specified embedder.
    ///
    /// # Panics
    /// If embedder has no HNSW config (E6, E12, E13)
    pub fn new(embedder: EmbedderIndex) -> Self {
        let config = super::get_hnsw_config(embedder)
            .unwrap_or_else(|| panic!(
                "FAIL FAST: No HNSW config for {:?}. Use InvertedIndex for E6/E13, MaxSim for E12.",
                embedder
            ));

        Self {
            embedder,
            config,
            id_to_idx: RwLock::new(HashMap::new()),
            idx_to_id: RwLock::new(Vec::new()),
            vectors: RwLock::new(Vec::new()),
        }
    }

    /// Create index with custom config (for testing).
    pub fn with_config(embedder: EmbedderIndex, config: HnswConfig) -> Self {
        Self {
            embedder,
            config,
            id_to_idx: RwLock::new(HashMap::new()),
            idx_to_id: RwLock::new(Vec::new()),
            vectors: RwLock::new(Vec::new()),
        }
    }
}

impl EmbedderIndexOps for HnswEmbedderIndex {
    fn embedder(&self) -> EmbedderIndex {
        self.embedder
    }

    fn config(&self) -> &HnswConfig {
        &self.config
    }

    fn len(&self) -> usize {
        self.idx_to_id.read().unwrap().len()
    }

    fn insert(&self, id: Uuid, vector: &[f32]) -> IndexResult<()> {
        validate_vector(vector, self.config.dimension, self.embedder)?;

        let mut id_to_idx = self.id_to_idx.write().unwrap();
        let mut idx_to_id = self.idx_to_id.write().unwrap();
        let mut vectors = self.vectors.write().unwrap();

        // Check for duplicate - update existing
        if id_to_idx.contains_key(&id) {
            let idx = id_to_idx[&id];
            vectors[idx] = vector.to_vec();
            return Ok(());
        }

        // Insert new
        let idx = idx_to_id.len();
        id_to_idx.insert(id, idx);
        idx_to_id.push(id);
        vectors.push(vector.to_vec());

        Ok(())
    }

    fn remove(&self, id: Uuid) -> IndexResult<bool> {
        let mut id_to_idx = self.id_to_idx.write().unwrap();

        if let Some(_idx) = id_to_idx.remove(&id) {
            // Note: HNSW doesn't support true deletion - mark as removed
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn search(&self, query: &[f32], k: usize, _ef_search: Option<usize>) -> IndexResult<Vec<(Uuid, f32)>> {
        validate_vector(query, self.config.dimension, self.embedder)?;

        let vectors = self.vectors.read().unwrap();
        let idx_to_id = self.idx_to_id.read().unwrap();

        if vectors.is_empty() {
            return Ok(Vec::new());
        }

        // Brute force placeholder until HNSW library integrated
        let mut distances: Vec<(usize, f32)> = vectors
            .iter()
            .enumerate()
            .map(|(idx, vec)| {
                let dist = super::compute_distance(query, vec, self.config.metric);
                (idx, dist)
            })
            .collect();

        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        distances.truncate(k);

        Ok(distances
            .into_iter()
            .map(|(idx, dist)| (idx_to_id[idx], dist))
            .collect())
    }

    fn insert_batch(&self, items: &[(Uuid, Vec<f32>)]) -> IndexResult<usize> {
        let mut count = 0;
        for (id, vec) in items {
            self.insert(*id, vec)?;
            count += 1;
        }
        Ok(count)
    }

    fn flush(&self) -> IndexResult<()> {
        Ok(())
    }

    fn memory_bytes(&self) -> usize {
        let vectors = self.vectors.read().unwrap();
        let overhead = std::mem::size_of::<Self>();
        let vector_bytes = vectors.iter().map(|v| v.len() * 4).sum::<usize>();
        let id_bytes = self.idx_to_id.read().unwrap().len() * 16;
        overhead + vector_bytes + id_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_index_e1_semantic() {
        println!("=== TEST: HNSW index for E1 Semantic (1024D) ===");

        let index = HnswEmbedderIndex::new(EmbedderIndex::E1Semantic);
        assert_eq!(index.config().dimension, 1024);
        assert_eq!(index.embedder(), EmbedderIndex::E1Semantic);
        assert_eq!(index.len(), 0);

        let id = Uuid::new_v4();
        let vector: Vec<f32> = (0..1024).map(|i| (i as f32) / 1024.0).collect();
        index.insert(id, &vector).unwrap();
        assert_eq!(index.len(), 1);

        let results = index.search(&vector, 1, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);
        assert!(results[0].1 < 0.001);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_dimension_mismatch_fails() {
        println!("=== TEST: Dimension mismatch FAIL FAST ===");

        let index = HnswEmbedderIndex::new(EmbedderIndex::E1Semantic);
        let wrong_vector = vec![1.0; 512];

        let result = index.insert(Uuid::new_v4(), &wrong_vector);
        assert!(result.is_err());

        match result.unwrap_err() {
            IndexError::DimensionMismatch { expected, actual, .. } => {
                assert_eq!(expected, 1024);
                assert_eq!(actual, 512);
            }
            _ => panic!("Wrong error type"),
        }

        println!("RESULT: PASS");
    }

    #[test]
    fn test_nan_vector_fails() {
        println!("=== TEST: NaN vector FAIL FAST ===");

        let index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
        let mut vector = vec![1.0; 384];
        vector[100] = f32::NAN;

        let result = index.insert(Uuid::new_v4(), &vector);
        assert!(result.is_err());

        match result.unwrap_err() {
            IndexError::InvalidVector { .. } => {}
            _ => panic!("Wrong error type"),
        }

        println!("RESULT: PASS");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_no_hnsw_config_panics() {
        println!("=== TEST: E6 sparse has no HNSW - panics ===");
        let _index = HnswEmbedderIndex::new(EmbedderIndex::E6Sparse);
    }

    #[test]
    fn test_batch_insert() {
        println!("=== TEST: Batch insert ===");

        let index = HnswEmbedderIndex::new(EmbedderIndex::E11Entity);
        let items: Vec<(Uuid, Vec<f32>)> = (0..100)
            .map(|i| {
                let id = Uuid::new_v4();
                let vector: Vec<f32> = (0..384).map(|j| ((i + j) as f32) / 1000.0).collect();
                (id, vector)
            })
            .collect();

        let count = index.insert_batch(&items).unwrap();
        assert_eq!(count, 100);
        assert_eq!(index.len(), 100);

        println!("RESULT: PASS");
    }
}
```

## File 3: Index Registry
**Path**: `crates/context-graph-storage/src/teleological/indexes/registry.rs`

```rust
//! Registry for managing per-embedder indexes.

use std::collections::HashMap;
use std::sync::Arc;

use super::{EmbedderIndex, HnswConfig};
use super::embedder_index::EmbedderIndexOps;
use super::hnsw_impl::HnswEmbedderIndex;

/// Registry of all per-embedder indexes.
///
/// Manages 12 HNSW indexes (E1-E5, E7-E11, Matryoshka128, PurposeVector).
/// E6, E12, E13 use different index types (inverted/MaxSim).
pub struct EmbedderIndexRegistry {
    indexes: HashMap<EmbedderIndex, Arc<dyn EmbedderIndexOps>>,
}

impl EmbedderIndexRegistry {
    /// Create registry with all HNSW indexes.
    pub fn new() -> Self {
        let mut indexes = HashMap::new();

        for embedder in EmbedderIndex::all_hnsw() {
            let index = HnswEmbedderIndex::new(embedder);
            indexes.insert(embedder, Arc::new(index) as Arc<dyn EmbedderIndexOps>);
        }

        Self { indexes }
    }

    /// Get index for embedder. Returns None for E6, E12, E13.
    pub fn get(&self, embedder: EmbedderIndex) -> Option<&Arc<dyn EmbedderIndexOps>> {
        self.indexes.get(&embedder)
    }

    /// Get index, panic if not found.
    pub fn get_or_panic(&self, embedder: EmbedderIndex) -> &Arc<dyn EmbedderIndexOps> {
        self.indexes.get(&embedder).unwrap_or_else(|| {
            panic!("FAIL FAST: No HNSW index for {:?}", embedder)
        })
    }

    /// Number of indexes in registry.
    pub fn len(&self) -> usize {
        self.indexes.len()
    }

    /// Iterate over all indexes.
    pub fn iter(&self) -> impl Iterator<Item = (&EmbedderIndex, &Arc<dyn EmbedderIndexOps>)> {
        self.indexes.iter()
    }

    /// Total memory used by all indexes.
    pub fn total_memory_bytes(&self) -> usize {
        self.indexes.values().map(|idx| idx.memory_bytes()).sum()
    }

    /// Total vectors across all indexes.
    pub fn total_vectors(&self) -> usize {
        self.indexes.values().map(|idx| idx.len()).sum()
    }
}

impl Default for EmbedderIndexRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_12_indexes() {
        println!("=== TEST: Registry has 12 HNSW indexes ===");

        let registry = EmbedderIndexRegistry::new();
        assert_eq!(registry.len(), 12);

        assert!(registry.get(EmbedderIndex::E1Semantic).is_some());
        assert!(registry.get(EmbedderIndex::E1Matryoshka128).is_some());
        assert!(registry.get(EmbedderIndex::E5Causal).is_some());
        assert!(registry.get(EmbedderIndex::E7Code).is_some());
        assert!(registry.get(EmbedderIndex::PurposeVector).is_some());

        // E6, E12, E13 should be None (not HNSW)
        assert!(registry.get(EmbedderIndex::E6Sparse).is_none());
        assert!(registry.get(EmbedderIndex::E12LateInteraction).is_none());
        assert!(registry.get(EmbedderIndex::E13Splade).is_none());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_registry_insert_and_search() {
        println!("=== TEST: Registry insert and search ===");

        let registry = EmbedderIndexRegistry::new();

        let e1_index = registry.get_or_panic(EmbedderIndex::E1Semantic);
        let id = uuid::Uuid::new_v4();
        let vector: Vec<f32> = (0..1024).map(|_| 0.5).collect();
        e1_index.insert(id, &vector).unwrap();

        let results = e1_index.search(&vector, 1, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);

        assert_eq!(registry.total_vectors(), 1);

        println!("RESULT: PASS");
    }
}
```

## File 4: Module Integration
**Path**: Update `crates/context-graph-storage/src/teleological/indexes/mod.rs`

Add these lines to existing file:
```rust
// ADD these new modules
pub mod embedder_index;
pub mod hnsw_impl;
pub mod registry;

// ADD these re-exports
pub use embedder_index::{EmbedderIndexOps, IndexError, IndexResult, validate_vector};
pub use hnsw_impl::HnswEmbedderIndex;
pub use registry::EmbedderIndexRegistry;
```

</definition_of_done>

<implementation_steps>

## Step 1: Create embedder_index.rs
1. Create file at `crates/context-graph-storage/src/teleological/indexes/embedder_index.rs`
2. Define `IndexError` enum with FAIL FAST variants
3. Define `EmbedderIndexOps` trait
4. Implement `validate_vector()` helper

## Step 2: Create hnsw_impl.rs
1. Create file at `crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs`
2. Implement `HnswEmbedderIndex` struct
3. Implement `EmbedderIndexOps` for `HnswEmbedderIndex`
4. Write unit tests with real data

## Step 3: Create registry.rs
1. Create file at `crates/context-graph-storage/src/teleological/indexes/registry.rs`
2. Implement `EmbedderIndexRegistry`
3. Write tests verifying 12 indexes created

## Step 4: Update mod.rs
1. Add module declarations and re-exports
2. Run `cargo check -p context-graph-storage`
3. Fix any compilation errors

## Step 5: Integration Test
1. Run all tests: `cargo test -p context-graph-storage indexes`
2. Verify all 12 HNSW indexes work
3. Verify E6/E12/E13 correctly rejected

</implementation_steps>

<full_state_verification>
## Source of Truth Definition

The source of truth for this task is:
1. **Compilation**: `cargo check -p context-graph-storage` must pass
2. **Tests**: `cargo test -p context-graph-storage indexes` must pass
3. **Index Count**: Registry must create exactly 12 indexes

## Execute & Inspect Protocol

After implementation, run these commands and record outputs:

```bash
# 1. Verify compilation - MUST SHOW 0 ERRORS
cargo check -p context-graph-storage 2>&1 | tee /tmp/check_output.txt
echo "Error count: $(grep -c 'error\[' /tmp/check_output.txt || echo 0)"
# EXPECTED OUTPUT: Error count: 0

# 2. Run all index tests - CAPTURE PASS/FAIL
cargo test -p context-graph-storage indexes -- --nocapture 2>&1 | tee /tmp/test_output.txt
grep -E "test.*ok|test.*FAILED|PASS|FAIL" /tmp/test_output.txt
# EXPECTED: All tests show "ok" or "PASS"

# 3. Verify new files exist with content
wc -l crates/context-graph-storage/src/teleological/indexes/embedder_index.rs
wc -l crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs
wc -l crates/context-graph-storage/src/teleological/indexes/registry.rs
# EXPECTED: Each file has > 50 lines

# 4. Verify 12 HNSW variants in EmbedderIndex
grep -c "E[0-9]*" crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs
# EXPECTED: 15 (12 HNSW + 3 non-HNSW)

# 5. Verify registry test shows 12 indexes
cargo test -p context-graph-storage registry_has_12 -- --nocapture 2>&1 | grep "registry.len()"
# EXPECTED: Shows assertion passes for len() == 12
```

## Boundary & Edge Case Audit

Execute these manual tests with synthetic data. Record state BEFORE and AFTER.

### Edge Case 1: Zero-dimension vector
```rust
// BEFORE: index.len() = 0
// ACTION: Insert empty vector
let index = HnswEmbedderIndex::new(EmbedderIndex::E1Semantic);
let empty: Vec<f32> = vec![];
let result = index.insert(Uuid::new_v4(), &empty);
// EXPECTED RESULT: Err(IndexError::DimensionMismatch { expected: 1024, actual: 0 })
// AFTER: index.len() = 0 (unchanged, insert rejected)
```

### Edge Case 2: Infinity in vector
```rust
// BEFORE: index.len() = 0
// ACTION: Insert vector with Inf
let index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
let mut vec = vec![1.0; 384];
vec[0] = f32::INFINITY;
let result = index.insert(Uuid::new_v4(), &vec);
// EXPECTED RESULT: Err(IndexError::InvalidVector { message: "Non-finite value at index 0: inf" })
// AFTER: index.len() = 0 (unchanged, insert rejected)
```

### Edge Case 3: Search empty index
```rust
// BEFORE: index.len() = 0
// ACTION: Search empty index
let index = HnswEmbedderIndex::new(EmbedderIndex::E1Semantic);
let query = vec![1.0; 1024];
let results = index.search(&query, 10, None).unwrap();
// EXPECTED RESULT: Empty vec (not panic)
// AFTER: index.len() = 0 (unchanged)
```

### Edge Case 4: Duplicate ID insert
```rust
// BEFORE: index.len() = 0
let index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
let id = Uuid::new_v4();
let vec1: Vec<f32> = vec![1.0; 384];
let vec2: Vec<f32> = vec![2.0; 384];

// ACTION: Insert same ID twice
index.insert(id, &vec1).unwrap();
// AFTER: index.len() = 1

index.insert(id, &vec2).unwrap();
// AFTER: index.len() = 1 (not 2, ID updated in place)

// VERIFY: Search returns vec2 values, not vec1
let results = index.search(&vec2, 1, None).unwrap();
assert_eq!(results[0].0, id);
assert!(results[0].1 < 0.001); // Should match vec2 exactly
```

## Evidence of Success

Provide logs showing:

1. **Test Output** - All tests pass:
```
running 7 tests
test indexes::embedder_index::tests::test_validate_vector ... ok
test indexes::hnsw_impl::tests::test_hnsw_index_e1_semantic ... ok
test indexes::hnsw_impl::tests::test_dimension_mismatch_fails ... ok
test indexes::hnsw_impl::tests::test_nan_vector_fails ... ok
test indexes::hnsw_impl::tests::test_no_hnsw_config_panics ... ok
test indexes::hnsw_impl::tests::test_batch_insert ... ok
test indexes::registry::tests::test_registry_has_12_indexes ... ok
test indexes::registry::tests::test_registry_insert_and_search ... ok
```

2. **Registry Verification**:
```
Registry has 12 HNSW indexes
E1Semantic: present
E1Matryoshka128: present
E2TemporalRecent: present
...
E6Sparse: None (correct - not HNSW)
E12LateInteraction: None (correct - not HNSW)
E13Splade: None (correct - not HNSW)
```

3. **Memory Check** - No leaks, reasonable usage:
```
Total vectors: 100
Total memory: 409600 bytes (~4KB per 1024D vector * 100)
```

</full_state_verification>

<test_commands>
  <command>cargo check -p context-graph-storage</command>
  <command>cargo test -p context-graph-storage indexes -- --nocapture</command>
  <command>cargo test -p context-graph-storage hnsw -- --nocapture</command>
  <command>cargo clippy -p context-graph-storage -- -D warnings</command>
</test_commands>

<files_to_create>
  <file path="crates/context-graph-storage/src/teleological/indexes/embedder_index.rs">
    EmbedderIndexOps trait and IndexError enum
  </file>
  <file path="crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs">
    HnswEmbedderIndex implementation
  </file>
  <file path="crates/context-graph-storage/src/teleological/indexes/registry.rs">
    EmbedderIndexRegistry for managing 12 indexes
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-storage/src/teleological/indexes/mod.rs" action="modify">
    Add: pub mod embedder_index; pub mod hnsw_impl; pub mod registry;
    Add re-exports for new types
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>EmbedderIndexOps trait is object-safe (dyn EmbedderIndexOps compiles)</criterion>
  <criterion>HnswEmbedderIndex implements EmbedderIndexOps</criterion>
  <criterion>EmbedderIndexRegistry creates exactly 12 indexes (not 13, not 15)</criterion>
  <criterion>Dimension mismatch returns IndexError::DimensionMismatch (not panic)</criterion>
  <criterion>NaN/Inf vectors return IndexError::InvalidVector (not panic)</criterion>
  <criterion>E6/E12/E13 panic on HnswEmbedderIndex::new() with clear message</criterion>
  <criterion>All tests use real vectors, not mocks</criterion>
  <criterion>No backwards compatibility shims</criterion>
</validation_criteria>

</task_spec>
```

## Quick Reference

| Item | Location |
|------|----------|
| Existing HnswConfig | `crates/context-graph-storage/src/teleological/indexes/hnsw_config/` |
| Existing EmbedderIndex enum | `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs` |
| Core Embedder enum | `crates/context-graph-core/src/teleological/embedder.rs` |
| New EmbedderIndexOps trait | `crates/context-graph-storage/src/teleological/indexes/embedder_index.rs` |
| New HnswEmbedderIndex | `crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs` |
| New registry | `crates/context-graph-storage/src/teleological/indexes/registry.rs` |

## Architecture Diagram

```
                    +------------------+
                    | EmbedderIndexOps |  <-- Trait (NEW)
                    +------------------+
                           ^
                           |
          +----------------+-----------------+
          |                                  |
+-------------------+            +---------------------+
| HnswEmbedderIndex |            | (future)            |
| (12 indexes)      |            | InvertedIndex (E6)  |
+-------------------+            | ColBERTIndex (E12)  |
          ^                      +---------------------+
          |
+---------------------+
| EmbedderIndexRegistry |  <-- Factory/Manager (NEW)
+---------------------+
          |
          v
+-------------------+
| Indexes: HashMap  |
| E1 -> HnswIndex   |
| E2 -> HnswIndex   |
| ...               |
| (12 total)        |
+-------------------+
```

## FAIL FAST Guarantees

| Scenario | Response |
|----------|----------|
| Wrong dimension | `IndexError::DimensionMismatch` |
| NaN/Inf in vector | `IndexError::InvalidVector` |
| E6/E12/E13 to HnswEmbedderIndex::new() | `panic!` with clear message |
| Search empty index | Empty results (not error) |
| Registry.get(E6) | `None` (not panic) |
| Registry.get_or_panic(E6) | `panic!` |

## NO BACKWARDS COMPATIBILITY

- Do NOT add deprecated wrappers
- Do NOT add compatibility shims
- Do NOT silently convert types
- If something breaks, it MUST error with clear message
- Tests MUST use real vectors, NOT mocks

---

## COMPLETION EVIDENCE (2026-01-09)

### Files Created

1. **`crates/context-graph-storage/src/teleological/indexes/embedder_index.rs`**
   - `EmbedderIndexOps` trait with 8 methods
   - `IndexError` enum with 5 variants
   - `IndexResult<T>` type alias
   - `validate_vector()` helper function
   - 8 unit tests

2. **`crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs`**
   - `HnswEmbedderIndex` struct with RwLock interior mutability
   - Full `EmbedderIndexOps` implementation
   - FAIL FAST panics for E6/E12/E13
   - 17 unit tests including panic tests

3. **`crates/context-graph-storage/src/teleological/indexes/registry.rs`**
   - `EmbedderIndexRegistry` managing 12 HNSW indexes
   - `get()` returns Option (None for E6/E12/E13)
   - `get_or_panic()` fails fast for non-HNSW
   - `get_dyn()` for trait object access
   - 17 unit tests

4. **Updated `crates/context-graph-storage/src/teleological/indexes/mod.rs`**
   - Added module declarations
   - Added re-exports
   - Added 5 comprehensive tests for TASK-CORE-007

### Test Results

```
test result: ok. 104 passed; 0 failed; 0 ignored
```

All acceptance criteria verified:
- AC1: EmbedderIndexOps trait with 8 methods ✓
- AC2: HnswEmbedderIndex for all 12 HNSW embedders ✓
- AC3: EmbedderIndexRegistry manages 12 indexes ✓
- AC4: FAIL FAST - E6/E12/E13 return None from registry ✓
- AC5: IndexError with 5 variants ✓
- AC6: validate_vector validates dimension and NaN/Inf ✓
- AC7: Thread-safe index operations with RwLock ✓
- AC8: Duplicate ID updates vector in place ✓

### Edge Cases Verified

- PurposeVector 13D index
- E7Code 1536D large vectors
- Batch insert 1000 vectors
- Search with k > len
- Remove and search exclusion
- Zero vector handling
- Subnormal float values
- Large but finite float values
- Registry aggregation across indexes

### Cargo Check

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.39s
```

No new compilation errors. Only pre-existing warnings from other modules.
