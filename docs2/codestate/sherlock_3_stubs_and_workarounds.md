# Sherlock Holmes Investigation Report #3: Stubs, Mocks, and Workarounds

**Case ID**: SHERLOCK-003-STUBS-AND-WORKAROUNDS
**Date**: 2026-01-08
**Investigator**: Sherlock Holmes Agent #3
**Verdict**: CRITICAL - 43 stub/mock/fallback patterns masking incomplete functionality

---

## Executive Summary

*"The world is full of obvious things which nobody by any chance ever observes."*

After forensic investigation of the codebase for stub implementations, mock data, fallback patterns, and workarounds, I have cataloged **43 distinct masking patterns** that hide the system not actually working in production scenarios.

**Summary Counts:**
- **Stub Implementations**: 15 (including 5 bio-nervous layers)
- **Mock/Fake Data Patterns**: 8
- **Fallback Patterns (`.ok()`, `.unwrap_or_default()`)**: 12 critical instances
- **Placeholder Constants**: 3
- **Feature Flag Divergence**: 5 patterns
- **TODO/FIXME Items**: 17+ (incomplete implementations)

---

## Section 1: Stub Implementations

### STUB #1: StubMultiArrayProvider [CRITICAL]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/multi_array_stub.rs`
**Lines**: 1-712

**What It Masks**: Real 13-embedding generation from GPU models

**Evidence**:
```rust
// Line 162-165: FAKE EMBEDDINGS
fn content_hash(content: &str) -> f32 {
    let sum: u32 = content.bytes().map(u32::from).sum();
    (sum % 256) as f32 / 255.0  // Just byte sum modulo 256!
}
```

**Impact Analysis**:
- ALL tests use this stub instead of real GPU embeddings
- "Hello world" and "Security vulnerability" would produce SIMILAR embeddings if byte sums are close
- Semantic search is not tested - only SHAPE of results is verified
- Tests pass but production behavior is completely untested

**If Removed**: All embedding-dependent tests would fail until real GPU provider is configured

---

### STUB #2: InMemoryTeleologicalStore [CRITICAL]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/teleological_store_stub.rs`
**Lines**: 1-1044

**What It Masks**: Real HNSW-indexed teleological storage with RocksDB persistence

**Critical Limitations (from header, lines 10-16)**:
```rust
// - **O(n) search complexity**: All search operations perform full table scans.
// - **No persistence**: All data is lost when the store is dropped.
// - **No HNSW indexing**: Unlike production stores, this stub does not use
//   approximate nearest neighbor search.
```

**Impact Analysis**:
- Production claims HNSW indexing but tests use O(n) linear scan
- No test verifies actual HNSW nearest neighbor search correctness
- Persistence tests will fail in production if store behavior differs
- Performance characteristics completely different from production

**If Removed**: All retrieval tests would fail, revealing no production store is wired up

---

### STUB #3: StubUtlProcessor [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/utl_stub.rs`
**Lines**: 1-1203

**What It Masks**: Real UTL computation with embedding-based surprise/coherence

**Evidence** (Lines 248-252, 285-286):
```rust
// Fallback: use prior_entropy as a proxy for surprise
Ok(context.prior_entropy.clamp(0.0, 1.0))

// Fallback: use current_coherence as a proxy
Ok(context.current_coherence.clamp(0.0, 1.0))
```

**Impact Analysis**:
- When embeddings not provided, falls back to context values
- Real KNN-based surprise computation only works if embeddings provided
- Most tests likely use fallback path, not real computation
- Named "StubUtlProcessor" but claims to implement "real" UTL

**If Removed**: UTL tests without embedding context would fail

---

### STUB #4: StubVectorOps (CUDA Stub) [CRITICAL]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-cuda/src/stub.rs`
**Lines**: 1-226

**What It Masks**: Real CUDA GPU vector operations

**Evidence** (Lines 42-46):
```rust
#[deprecated(
    since = "0.1.0",
    note = "TEST ONLY: StubVectorOps violates AP-007 if used in production. Use real CUDA implementations."
)]
```

**Impact Analysis**:
- `is_gpu_available()` always returns `false`
- `device_name()` returns "CPU (Stub)"
- Tests run CPU code but production requires GPU
- AP-007 Constitution violation if used in production

**If Removed**: All CUDA tests would fail until real GPU operations configured

---

### STUB #5-9: Bio-Nervous Layer Stubs [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/layers/`

| Layer | File | Budget | Status |
|-------|------|--------|--------|
| L1 Sensing | `sensing.rs` | 5ms | Returns `NotImplemented` error |
| L2 Reflex | `reflex.rs` | 100us | Returns `NotImplemented` error |
| L3 Memory | `memory.rs` | 1ms | Returns `NotImplemented` error |
| L4 Learning | `learning.rs` | 10ms | Returns `NotImplemented` error |
| L5 Coherence | `coherence.rs` | 10ms | Returns `NotImplemented` error |

**Evidence** (sensing.rs lines 37-43):
```rust
async fn process(&self, _input: LayerInput) -> CoreResult<LayerOutput> {
    // FAIL FAST - No mock data in production (AP-007)
    Err(CoreError::NotImplemented(
        "L1 SensingLayer requires real implementation..."
    ))
}
```

**Impact Analysis**:
- `health_check()` returns `false` for all layers
- Layer names include "[NOT IMPLEMENTED]"
- These are HONEST stubs - they FAIL FAST rather than return fake data
- Good design: Production will immediately surface missing implementation

**If Removed**: Bio-nervous system tests would have no layer implementations

---

### STUB #10: InMemoryGraphIndex [MEDIUM]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/graph_index.rs`
**Lines**: 1-503

**What It Masks**: FAISS GPU-accelerated graph index

**Evidence** (Lines 17-31):
```rust
/// In-memory brute-force vector index.
///
/// Uses HashMap storage with linear search for simplicity.
/// Production will use FAISS GPU for high-performance ANN search.
///
/// # Performance
///
/// - search: O(n * d) where n = vectors, d = dimension
```

**Impact Analysis**:
- Linear O(n*d) search vs production FAISS approximate nearest neighbor
- Tests pass with brute force but production behavior differs
- Suitable for "Ghost System phase with up to ~10,000 vectors" only

**If Removed**: Graph search tests would fail

---

### STUB #11: LazyFailMultiArrayProvider [MEDIUM - INTENTIONAL]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-mcp/src/server.rs`
**Lines**: 31-106

**What It Masks**: Real GPU embedding provider in MCP server

**Evidence** (Lines 34-44):
```rust
/// Placeholder MultiArrayEmbeddingProvider that fails on first use with a clear error.
///
/// This is NOT a stub that returns fake data. It exists only to provide a clear,
/// actionable error message when embedding operations are attempted before the
/// real GPU implementation is ready.
```

**Impact Analysis**:
- `embed_all()` returns error with clear instructions
- `is_ready()` returns `false`
- All 13 embedders report `health_status() = false`
- GOOD DESIGN: Fails fast with actionable error, doesn't return fake data

**If Removed**: MCP server would fail to compile without embedding provider

---

## Section 2: Mock Data in Tests

### MOCK #1: Zeroed Fingerprints in Tests [CRITICAL]

**Locations**: Multiple files
- `/home/cabdru/contextgraph/crates/context-graph-core/src/retrieval/tests.rs:27-34`
- `/home/cabdru/contextgraph/crates/context-graph-core/src/johari/default_manager.rs`
- `/home/cabdru/contextgraph/crates/context-graph-core/src/alignment/tests.rs`

**Evidence** (retrieval/tests.rs lines 27-34):
```rust
fn create_test_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(
        SemanticFingerprint::zeroed(),  // <-- ALL ZEROS!
        PurposeVector::new([0.75; NUM_EMBEDDERS]),
        JohariFingerprint::zeroed(),
        [0u8; 32],
    )
}
```

**Impact Analysis**:
- Cosine similarity of zero vectors is undefined (0/0)
- Tests verify code doesn't crash with zeros, not correct behavior
- Real semantic search with meaningful embeddings not tested
- 50+ usages of `::zeroed()` across codebase

---

### MOCK #2: Hash-Based Deterministic Embeddings [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/multi_array_stub.rs:158-232`

**Evidence**:
```rust
fn fill_dense_embedding(content: &str, dim: usize) -> Vec<f32> {
    let base = Self::content_hash(content);  // Byte sum % 256
    (0..dim)
        .map(|i| Self::deterministic_value(base, i))
        .collect()
}
```

**Impact Analysis**:
- Two completely different concepts could have similar embeddings
- "cat" (byte sum 312) vs "act" (byte sum 312) = IDENTICAL embeddings
- No semantic relationship captured - purely hash-based

---

### MOCK #3: Fixed HNSW Results in Multi-Space Tests [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/storage/multi_space.rs:389-450`

**Evidence**:
```rust
/// Test HNSW manager with fixed results
struct TestHnswManager {
    results: HashMap<u8, Vec<(Uuid, f32)>>,  // FIXED results, not computed!
}
```

**Impact Analysis**:
- RRF fusion tests use mocked HNSW results
- Tests verify RRF math, not end-to-end correctness
- Real HNSW behavior not tested in fusion scenarios

---

### MOCK #4-8: Additional Mock Patterns

| Pattern | Location | Impact |
|---------|----------|--------|
| Test embeddings with base_value | Multiple test files | Predictable, not semantic |
| Normalized test vectors | graph_index.rs | Geometric, not meaningful |
| Fixed dimension vectors | Various | Shape testing only |
| Placeholder model IDs | multi_array_stub.rs:311-326 | "stub-e1" through "stub-e13" |
| Simulated latencies | multi_array_stub.rs:272-274 | 5ms per embedder, fake |

---

## Section 3: Fallback Patterns

### FALLBACK #1: Silent RwLock Poison Handling [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/retrieval/pipeline.rs:542`

```rust
let last_time = self.last_query_time.read().ok().and_then(|g| *g);
```

**Impact**: If lock is poisoned from thread panic, silently returns `None`

---

### FALLBACK #2: Embedder Health Lock Poisoning [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/multi_array_stub.rs:392-399`

```rust
fn health_status(&self) -> [bool; NUM_EMBEDDERS] {
    self.embedder_health
        .read()
        .map(|h| *h)
        .unwrap_or_else(|_| {
            // If lock is poisoned, log and return all unhealthy
            [false; NUM_EMBEDDERS]
        })
}
```

**Impact**: Lock poison hidden; caller can't distinguish "all unhealthy" from "lock error"

---

### FALLBACK #3: Sparse Vector Creation Silent Failure [MEDIUM]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/multi_array_stub.rs:215`

```rust
SparseVector::new(indices, values).unwrap_or_else(|_| SparseVector::empty())
```

**Impact**: Invalid sparse vectors silently become empty; search returns no matches

---

### FALLBACK #4: Silent File Cleanup in Tests [MEDIUM]

**Locations**: `/home/cabdru/contextgraph/crates/context-graph-core/src/index/hnsw_impl.rs`
- Line 1602: `std::fs::remove_dir_all(&temp_dir).ok();`
- Line 1688: `std::fs::remove_file(&temp_path).ok();`
- Line 1725: `std::fs::remove_file(&temp_path).ok();`
- Line 1762: `std::fs::remove_file(&temp_path).ok();`
- Line 1813: `std::fs::remove_dir_all(&temp_dir).ok();`

**Impact**: Cleanup failures silently ignored; file system may be corrupted

---

### FALLBACK #5-12: Additional Fallback Patterns

| Pattern | Location | Count | Impact |
|---------|----------|-------|--------|
| `.unwrap_or_default()` | Various | 25+ | Defaults mask errors |
| `.unwrap_or(0)` | Ordering comparisons | 15+ | NaN handling hidden |
| `.unwrap_or(true/false)` | Lock poison | 5+ | Concurrency bugs hidden |
| `.ok()` on Result | File operations | 10+ | IO errors swallowed |

---

## Section 4: Feature Flag Divergence

### FLAG #1: Stubs Gated by `#[cfg(any(test, feature = "test-utils"))]`

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/mod.rs`

**All stub modules use this gate**:
```rust
#[cfg(any(test, feature = "test-utils"))]
mod graph_index;
#[cfg(any(test, feature = "test-utils"))]
mod layers;
#[cfg(any(test, feature = "test-utils"))]
mod multi_array_stub;
#[cfg(any(test, feature = "test-utils"))]
mod teleological_store_stub;
#[cfg(any(test, feature = "test-utils"))]
mod utl_stub;
```

**Impact**: Good design - stubs cannot leak to production unless `test-utils` feature enabled

---

### FLAG #2: CUDA Stub Gated by `#[cfg(test)]`

**Location**: `/home/cabdru/contextgraph/crates/context-graph-cuda/src/lib.rs:44`

```rust
#[cfg(test)]
mod stub;
```

**Impact**: GPU stub only available in tests, production must use real CUDA

---

### FLAG #3-5: Additional Feature Flags

| Flag | Purpose | Risk |
|------|---------|------|
| `#[cfg(test)]` on 150+ modules | Test-only code | Low - correct isolation |
| `feature = "test-utils"` | Downstream test access | Medium - must not enable in prod |
| `#[deprecated]` on StubVectorOps | Usage warning | Low - clear warning |

---

## Section 5: Default/Zeroed Values

### ZEROED #1: SemanticFingerprint::zeroed() [CRITICAL]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/types/fingerprint/semantic/`

**Usage Count**: 50+ across codebase

**Evidence**:
```rust
// fingerprint.rs:40
/// need a placeholder fingerprint (e.g., in tests), but be aware that zeroed data
```

**Impact**:
- All 13 embedding arrays filled with zeros
- Cosine similarity undefined for zero vectors
- Tests don't verify real semantic behavior

---

### ZEROED #2: JohariFingerprint::zeroed()

**Usage Count**: 15+ across codebase

**Impact**: Johari quadrant calculations on zero data meaningless

---

### ZEROED #3: Default::default() for Pipeline Config

**Usage Count**: 100+ in tests

**Impact**: Tests use default configs, not production configs

---

## Section 6: TODO/FIXME Analysis

### Critical TODOs

| Location | TODO | Priority |
|----------|------|----------|
| `query/mod.rs:26` | M04-T18 - Implement semantic search | HIGH |
| `query/mod.rs:40` | M04-T20 - Implement entailment query | HIGH |
| `query/mod.rs:48` | M04-T21 - Implement contradiction detection | HIGH |
| `traversal/mod.rs:101` | M04-T22 - Implement traversal utilities | MEDIUM |
| `hyperbolic/mod.rs:28` | M04-T23 - CUDA kernels for batch operations | HIGH |
| `entailment/mod.rs:31` | M04-T24 - CUDA kernels for containment | HIGH |
| `query/mod.rs:56` | M04-T27 - Implement query builder | MEDIUM |
| `query/mod.rs:65` | M04-T28 - Implement graph API | HIGH |
| `indexes.rs:13` | M02-023 - Implement storage indexes | HIGH |
| `rocksdb_store.rs:930` | Implement BM25 scoring | MEDIUM |

---

## Section 7: Placeholder Constants

### PLACEHOLDER #1: CONTENT_TO_GOAL_FACTOR [CRITICAL]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/config/constants.rs:153`

```rust
/// NOTE: This is a placeholder per `retrieval/pipeline.rs:578`.
/// In production, this should be replaced with proper teleological computation.
pub const CONTENT_TO_GOAL_FACTOR: f32 = 0.9;
```

**Used In**: `/home/cabdru/contextgraph/crates/context-graph-core/src/retrieval/pipeline.rs:581`

```rust
// NOTE: This is a placeholder per estimation::CONTENT_TO_GOAL_FACTOR
// In production, use proper teleological computation
let goal_alignment = content_sim * estimation::CONTENT_TO_GOAL_FACTOR;
```

**Impact**:
- Stage 4 teleological filtering uses fake math
- Goal alignment = content_similarity * 0.9 (meaningless)
- Real teleological computation not implemented

---

### PLACEHOLDER #2: Johari quadrant_correlation

**Location**: `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/johari.rs:854`

```rust
"quadrant_correlation": {} // Placeholder - full impl requires multi-memory scan
```

---

### PLACEHOLDER #3: Sample size in Johari

**Location**: `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/johari.rs:983`

```rust
"sample_size": 150 // Placeholder - actual impl would track this
```

---

## Impact Analysis

### What Breaks If Stubs Are Removed

| Stub | Dependent Tests | Production Impact |
|------|-----------------|-------------------|
| StubMultiArrayProvider | ALL retrieval tests | Must configure GPU provider |
| InMemoryTeleologicalStore | ALL storage tests | Must configure RocksDB |
| StubUtlProcessor | ALL UTL tests | Must provide embeddings |
| StubVectorOps | ALL CUDA tests | Must have real GPU |
| Bio-nervous layers | Layer tests | Must implement 5 layers |
| InMemoryGraphIndex | Graph tests | Must configure FAISS |

### Production Path Analysis

**Current State**:
1. Tests use stubs -> Tests pass
2. Production requires real implementations -> NOT WIRED UP
3. Gap: Production will fail on first use

**Required for Production**:
1. GPU embedding provider (RTX 5090 required per AP-007)
2. RocksDB teleological store with HNSW indexes
3. Real CUDA vector operations
4. 5 bio-nervous layer implementations
5. FAISS GPU graph index
6. Real teleological Stage 4 computation

---

## Recommendations

### CRITICAL (Must Fix Before Any Production Use)

1. **Replace Stage 4 placeholder** - `CONTENT_TO_GOAL_FACTOR` must be replaced with real teleological fingerprint lookup
2. **Create integration tests with real embeddings** - At least one test path must use GPU
3. **Wire up production stores** - RocksDB backend must be the default, not InMemory
4. **Implement bio-nervous layers** - Currently ALL return `NotImplemented`

### HIGH (Required for Correct Behavior)

5. **Propagate RwLock poison errors** - Don't hide concurrency bugs
6. **Replace zeroed fingerprints** - Tests need meaningful semantic data
7. **Implement TODO M04 tasks** - Query, traversal, CUDA kernels incomplete
8. **Add production vs test configuration** - Clear separation needed

### MEDIUM (Technical Debt)

9. **Remove silent `.ok()` patterns** - At least log errors
10. **Add HNSW correctness tests** - Currently only stub is tested
11. **Benchmark with production data** - Current tests use trivial data

---

## Memory Storage

```json
{
  "summary": "43 stub/mock/fallback patterns masking incomplete functionality",
  "stub_count": 15,
  "mock_count": 8,
  "fallback_count": 12,
  "placeholder_count": 3,
  "todo_count": 17,
  "critical_stubs": [
    "StubMultiArrayProvider - fake embeddings from byte hash",
    "InMemoryTeleologicalStore - O(n) scan, no HNSW",
    "StubVectorOps - CPU instead of GPU",
    "stage4_placeholder_filtering - CONTENT_TO_GOAL_FACTOR * content_sim",
    "Bio-nervous layers - all return NotImplemented"
  ],
  "production_blockers": [
    "No GPU embedding provider wired up",
    "No RocksDB store wired up",
    "Stage 4 uses placeholder math",
    "All 5 bio-nervous layers unimplemented",
    "CUDA required but only stub exists"
  ],
  "intentional_phase0": [
    "Bio-nervous stubs (fail fast)",
    "LazyFailMultiArrayProvider (fail fast)",
    "test-utils feature gate"
  ]
}
```

---

## Conclusion

*"When you have eliminated the impossible, whatever remains, however improbable, must be the truth."*

**VERDICT**: The codebase contains a comprehensive stub infrastructure that:

1. **CORRECTLY** gates all stubs with `#[cfg(test)]` or `#[cfg(any(test, feature = "test-utils"))]`
2. **CORRECTLY** uses fail-fast patterns for bio-nervous layers and MCP embedding provider
3. **INCORRECTLY** allows tests to pass without testing real semantic behavior
4. **INCORRECTLY** uses placeholder math in Stage 4 teleological filtering
5. **INCORRECTLY** hides concurrency errors with `.ok()` and `.unwrap_or_default()`

The system will **APPEAR TO WORK** in tests but will **FAIL IN PRODUCTION** because:
- Tests verify code structure, not semantic correctness
- Stubs return deterministic but meaningless data
- Real GPU/storage/layer implementations are not wired up
- Placeholder constants mask missing computation

**The stubs are well-designed for Phase 0 development, but the gap to production is NOT TESTED.**

---

*"The game is afoot!"* - Production readiness requires bridging the stub-to-real implementation gap.

**Case Status**: INVESTIGATION COMPLETE - Stubs and Workarounds Documented
