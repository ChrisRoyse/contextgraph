# TASK-L005: Per-Space HNSW Index Builder

```yaml
metadata:
  id: "TASK-L005"
  title: "Per-Space HNSW Index Builder"
  layer: "logic"
  priority: "P1"
  estimated_hours: 10
  created: "2026-01-04"
  updated: "2026-01-05"
  status: "IN_PROGRESS"
  implementation_complete: true
  verification_pending: true
  dependencies:
    - "TASK-F001"  # SemanticFingerprint struct - COMPLETE
    - "TASK-F005"  # HNSW index configuration - COMPLETE
    - "TASK-F008"  # TeleologicalMemoryStore trait - COMPLETE
  spec_refs:
    - "constitution.yaml:storage:layer2c_per_embedder"
    - "constitution.yaml:embeddings:retrieval_pipeline"
```

## CRITICAL: IMPLEMENTATION STATUS

**The index module has been implemented.** All core files exist in `crates/context-graph-core/src/index/`. The remaining work is:

1. **Run tests** to verify the implementation
2. **Update lib.rs** to export the module correctly (already done)
3. **Verify Cargo.toml** has required dependencies

---

## NO BACKWARDS COMPATIBILITY - FAIL FAST SEMANTICS

- **NO FALLBACKS** - If index operations fail, they error immediately
- **NO MOCK DATA** - Tests use real data and verify actual outcomes
- **FAIL FAST** - Invalid configurations panic with context

---

## Current Codebase State (VERIFIED 2026-01-05)

### What Already Exists - COMPLETE

| Component | Location | Status |
|-----------|----------|--------|
| `EmbedderIndex` enum (15 variants) | `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs` | COMPLETE |
| `HnswConfig` struct | `crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs` | COMPLETE |
| `DistanceMetric` enum (5 variants) | `crates/context-graph-storage/src/teleological/indexes/hnsw_config/distance.rs` | COMPLETE |
| `InvertedIndexConfig` struct | `crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs` | COMPLETE |
| `get_hnsw_config()` function | `crates/context-graph-storage/src/teleological/indexes/hnsw_config/functions.rs` | COMPLETE |
| `all_hnsw_configs()` function | `crates/context-graph-storage/src/teleological/indexes/hnsw_config/functions.rs` | COMPLETE |
| Dimension constants (E1-E13) | `crates/context-graph-storage/src/teleological/indexes/hnsw_config/constants.rs` | COMPLETE |
| `compute_distance()`, `cosine_similarity()` | `crates/context-graph-storage/src/teleological/indexes/metrics.rs` | COMPLETE |
| `TeleologicalMemoryStore` trait | `crates/context-graph-core/src/traits/teleological_memory_store.rs` | COMPLETE |
| `MultiEmbeddingQueryExecutor` trait | `crates/context-graph-core/src/retrieval/executor.rs` | COMPLETE |
| `SemanticFingerprint` struct | `crates/context-graph-core/src/types/fingerprint/semantic/` | COMPLETE |
| `JohariTransitionManager` trait | `crates/context-graph-core/src/johari/manager.rs` | COMPLETE |

### Index Module - IMPLEMENTED (Verification Required)

| Component | Location | Status |
|-----------|----------|--------|
| `MultiSpaceIndexManager` trait | `crates/context-graph-core/src/index/manager.rs` | IMPLEMENTED |
| `HnswMultiSpaceIndex` struct | `crates/context-graph-core/src/index/hnsw_impl.rs` | IMPLEMENTED |
| `SimpleHnswIndex` struct | `crates/context-graph-core/src/index/hnsw_impl.rs` | IMPLEMENTED |
| `SpladeInvertedIndex` struct | `crates/context-graph-core/src/index/splade_impl.rs` | IMPLEMENTED |
| `IndexStatus` / `IndexHealth` | `crates/context-graph-core/src/index/status.rs` | IMPLEMENTED |
| `IndexError` enum | `crates/context-graph-core/src/index/error.rs` | IMPLEMENTED |
| `HnswConfig` (local copy) | `crates/context-graph-core/src/index/config.rs` | IMPLEMENTED |
| `mod.rs` exports | `crates/context-graph-core/src/index/mod.rs` | IMPLEMENTED |

### lib.rs Module Export - VERIFIED

```rust
// crates/context-graph-core/src/lib.rs already includes:
pub mod index;
```

---

## Architecture Overview

### 5-Stage Retrieval Pipeline

| Stage | Index Type | Embedder | Purpose | Target Latency |
|-------|------------|----------|---------|----------------|
| 1 | Inverted | E13 SPLADE | Sparse recall (10K candidates) | <5ms |
| 2 | HNSW | E1 Matryoshka 128D | Fast ANN filter (1K candidates) | <10ms |
| 3 | HNSW × 10 | E1-E5, E7-E11 | Full precision (100 candidates) | <20ms |
| 4 | HNSW | PurposeVector 13D | Teleological filter (50 candidates) | <10ms |
| 5 | MaxSim | E12 ColBERT | Final rerank (10 results) | <15ms |

### Index Counts

- **12 HNSW indexes**: E1-E5, E7-E11, E1Matryoshka128, PurposeVector
- **2 Inverted indexes**: E6Sparse (legacy), E13Splade (Stage 1)
- **1 MaxSim**: E12LateInteraction (Stage 5, future)

---

## Dimension Constants (DO NOT REDEFINE)

```rust
// Use from context_graph_core::index::config or context_graph_storage
pub const E1_DIM: usize = 1024;
pub const E1_MATRYOSHKA_DIM: usize = 128;
pub const E2_DIM: usize = 512;
pub const E3_DIM: usize = 512;
pub const E4_DIM: usize = 512;
pub const E5_DIM: usize = 768;
pub const E6_SPARSE_VOCAB: usize = 30_522;
pub const E7_DIM: usize = 256;
pub const E8_DIM: usize = 384;
pub const E9_DIM: usize = 10_000;
pub const E10_DIM: usize = 768;
pub const E11_DIM: usize = 384;
pub const E12_TOKEN_DIM: usize = 128;
pub const E13_SPLADE_VOCAB: usize = 30_522;
pub const PURPOSE_VECTOR_DIM: usize = 13;
pub const NUM_EMBEDDERS: usize = 13;
```

---

## Key APIs (Already Implemented)

### MultiSpaceIndexManager Trait

```rust
// crates/context-graph-core/src/index/manager.rs
#[async_trait]
pub trait MultiSpaceIndexManager: Send + Sync {
    async fn initialize(&mut self) -> IndexResult<()>;
    async fn add_vector(&mut self, embedder: EmbedderIndex, memory_id: Uuid, vector: &[f32]) -> IndexResult<()>;
    async fn add_fingerprint(&mut self, memory_id: Uuid, fingerprint: &SemanticFingerprint) -> IndexResult<()>;
    async fn add_purpose_vector(&mut self, memory_id: Uuid, purpose: &[f32]) -> IndexResult<()>;
    async fn add_splade(&mut self, memory_id: Uuid, sparse: &[(usize, f32)]) -> IndexResult<()>;
    async fn search(&self, embedder: EmbedderIndex, query: &[f32], k: usize) -> IndexResult<Vec<(Uuid, f32)>>;
    async fn search_splade(&self, sparse_query: &[(usize, f32)], k: usize) -> IndexResult<Vec<(Uuid, f32)>>;
    async fn search_matryoshka(&self, query_128d: &[f32], k: usize) -> IndexResult<Vec<(Uuid, f32)>>;
    async fn search_purpose(&self, purpose_query: &[f32], k: usize) -> IndexResult<Vec<(Uuid, f32)>>;
    async fn remove(&mut self, memory_id: Uuid) -> IndexResult<()>;
    fn status(&self) -> Vec<IndexStatus>;
    async fn persist(&self, path: &Path) -> IndexResult<()>;
    async fn load(&mut self, path: &Path) -> IndexResult<()>;
}
```

### IndexError Enum (Fail-Fast)

```rust
// crates/context-graph-core/src/index/error.rs
#[derive(Error, Debug)]
pub enum IndexError {
    DimensionMismatch { embedder: EmbedderIndex, expected: usize, actual: usize },
    InvalidEmbedder { embedder: EmbedderIndex },
    NotInitialized { embedder: EmbedderIndex },
    InvalidTermId { term_id: usize, vocab_size: usize },
    ZeroNormVector { memory_id: Uuid },
    NotFound { memory_id: Uuid },
    StorageError { context: String, message: String },
    CorruptedIndex { path: String },
    IoError { context: String, message: String },
    SerializationError { context: String, message: String },
}
```

### IndexHealth (NO DEGRADED MODE)

```rust
// crates/context-graph-core/src/index/status.rs
pub enum IndexHealth {
    Healthy,     // Normal operation
    Failed,      // Must rebuild - NO fallbacks
    Rebuilding,  // Being reconstructed
}
```

---

## Verification Requirements

### Step 1: Build Verification

```bash
cd /home/cabdru/contextgraph
cargo build -p context-graph-core 2>&1 | head -50
```

**Expected**: Compiles without errors.

### Step 2: Unit Test Execution

```bash
cargo test -p context-graph-core index:: --nocapture 2>&1
```

**Expected Output Pattern**:
```
[VERIFIED] IndexHealth defaults to Healthy
[VERIFIED] Health operational state checks
[VERIFIED] Health write capability checks
[VERIFIED] IndexStatus::new_empty creates healthy empty index
[VERIFIED] update_count calculates memory correctly
[VERIFIED] mark_failed sets Failed state
[VERIFIED] mark_rebuilding sets Rebuilding state
[VERIFIED] mark_healthy sets Healthy state
[VERIFIED] MultiIndexHealth aggregation correct
[VERIFIED] DimensionMismatch error format: ...
[VERIFIED] InvalidEmbedder error format: ...
[VERIFIED] InvalidTermId error format: ...
[VERIFIED] ZeroNormVector error format: ...
[VERIFIED] StorageError helper: ...
[VERIFIED] EmbedderIndex::uses_hnsw() works correctly
[VERIFIED] all_hnsw() returns 12 embedders
[VERIFIED] EmbedderIndex::dimension() works correctly
[VERIFIED] HnswConfig::new() creates valid config
[VERIFIED] InvertedIndexConfig::e13_splade() config correct
```

### Step 3: Edge Case Audit (MANDATORY)

These edge cases MUST pass:

1. **Empty index search**: Returns empty Vec, not error
2. **Dimension mismatch**: Returns `IndexError::DimensionMismatch`
3. **Invalid term_id**: Returns `IndexError::InvalidTermId`
4. **Zero-norm vector**: Returns `IndexError::ZeroNormVector`
5. **Non-HNSW embedder**: Returns `IndexError::InvalidEmbedder`

---

## Full State Verification Protocol (MANDATORY)

### 1. Source of Truth Identification

| Operation | Source of Truth | Verification Method |
|-----------|-----------------|---------------------|
| Initialize | `status()` returns 12 HNSW statuses | Count where `uses_hnsw() == true` |
| Add vector | `IndexStatus.element_count` | Check status after add, verify increment |
| Search | `Vec<(Uuid, f32)>` result | Verify returned UUID matches added ID |
| Persist | Files exist at path | `std::fs::metadata(path)` returns Ok |
| Load | Search returns persisted data | Compare results pre/post load |

### 2. Execute & Inspect Protocol

For EVERY test:
```rust
println!("[BEFORE] {}", describe_current_state());
let result = operation().await;
println!("[AFTER] {}", describe_current_state());
assert!(expected_condition);
println!("[VERIFIED] {}", what_was_verified);
```

### 3. Evidence of Success Log

After verification, tests must print:
```
[VERIFIED] 12 HNSW indexes initialized
[VERIFIED] Dimension validation: 1024 expected, 512 got -> ERROR
[VERIFIED] Vector added and searchable with similarity ~1.0
[VERIFIED] SPLADE search returns BM25-scored candidates
[VERIFIED] Persist/load roundtrip preserves vectors
[VERIFIED] Empty index returns empty Vec
[VERIFIED] Non-HNSW embedder rejected with InvalidEmbedder
[VERIFIED] Invalid term_id rejected
```

---

## Integration Points

### Used By

- **TASK-L006** (Purpose Pattern Index): Uses `search_purpose()`
- **TASK-L007** (Cross-Space Similarity): Uses `search()` across spaces
- **TASK-L008** (5-Stage Pipeline): Orchestrates all index operations

### Depends On

- **TASK-F001**: `SemanticFingerprint` struct for `add_fingerprint()`
- **TASK-F005**: HNSW configuration (dimensions, M, ef values)
- **TASK-F008**: `TeleologicalMemoryStore` for storage integration

---

## Files Summary

```
crates/context-graph-core/src/index/
├── mod.rs           # Module exports (HnswMultiSpaceIndex, SpladeInvertedIndex, etc.)
├── config.rs        # Local copies of HnswConfig, EmbedderIndex, constants
├── error.rs         # IndexError enum with fail-fast semantics
├── status.rs        # IndexStatus, IndexHealth, MultiIndexHealth
├── manager.rs       # MultiSpaceIndexManager trait definition
├── hnsw_impl.rs     # HnswMultiSpaceIndex and SimpleHnswIndex implementations
└── splade_impl.rs   # SpladeInvertedIndex with BM25 scoring
```

---

## Success Criteria Checklist

- [x] `mod.rs` exports all public types
- [x] `config.rs` defines dimension constants and EmbedderIndex
- [x] `error.rs` defines IndexError with descriptive messages
- [x] `status.rs` defines IndexHealth (no degraded mode)
- [x] `manager.rs` defines MultiSpaceIndexManager trait
- [x] `hnsw_impl.rs` implements HnswMultiSpaceIndex
- [x] `splade_impl.rs` implements SpladeInvertedIndex
- [x] `lib.rs` exports `pub mod index`
- [ ] `cargo build` succeeds
- [ ] All unit tests pass
- [ ] Edge cases print [VERIFIED] logs
- [ ] Persist/load roundtrip works

---

## Verification Commands

```bash
# 1. Verify build
cargo build -p context-graph-core

# 2. Run all index tests
cargo test -p context-graph-core index:: --nocapture

# 3. Check file structure
ls -la crates/context-graph-core/src/index/

# 4. Count test output verifications
cargo test -p context-graph-core index:: --nocapture 2>&1 | grep -c "\[VERIFIED\]"
```

---

## Traceability Matrix

| Requirement | Source | Implementation |
|-------------|--------|----------------|
| 12 HNSW indexes | constitution.yaml:layer2c | `EmbedderIndex::all_hnsw()` returns 12 |
| E1Matryoshka128 128D | constitution.yaml:layer2b | `search_matryoshka()` |
| E13 SPLADE inverted | constitution.yaml:layer2a | `SpladeInvertedIndex` |
| Purpose vector 13D | constitution.yaml:layer2d | `search_purpose()` |
| M=16 default | constitution.yaml | `HnswConfig::default_for_dimension()` |
| <10ms search target | constitution.yaml:perf | Async implementation |
| BM25+SPLADE hybrid | constitution.yaml:stage_1 | `SpladeInvertedIndex::search()` |
| Fail-fast errors | constitution.yaml:errors | `IndexError` variants |

---

*Task updated: 2026-01-05*
*Status: Implementation complete, verification pending*
*Layer: Logic*
*Priority: P1 - Core indexing infrastructure*
