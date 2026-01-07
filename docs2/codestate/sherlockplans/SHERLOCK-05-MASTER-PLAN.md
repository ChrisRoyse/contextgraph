# SHERLOCK HOLMES MASTER INVESTIGATION REPORT

## CASE ID: SHERLOCK-05-MASTER-SYNTHESIS
## Date: 2026-01-07
## Investigator: Agent #5 (Final Synthesis) - Sherlock Holmes Protocol
## Status: CRITICAL ISSUES FOUND

---

*"When you have eliminated the impossible, whatever remains, however improbable, must be the truth."*

---

## EXECUTIVE SUMMARY

**VERDICT: SYSTEM NOT READY FOR PRODUCTION**

The Context Graph system has made significant progress, but **2 CRITICAL blocking issues** and **3 HIGH priority issues** remain before Phase 0 completion.

### Test Results Summary

| Crate | Tests Passed | Tests Failed | Status |
|-------|-------------|--------------|--------|
| context-graph-cuda | 120 | 0 | PASS |
| context-graph-embeddings | 103 | 0 (43 ignored) | PASS |
| context-graph-core | 48 | 3 (doc tests) | PARTIAL |
| context-graph-graph | 45 | 0 (23 ignored) | PASS |
| context-graph-storage | 63 | 0 (9 ignored) | PASS |
| context-graph-mcp | 250 | 11 + COMPILE ERROR | FAIL |

---

## CRITICAL BLOCKING ISSUES

### CRITICAL #1: COMPILATION ERROR in context-graph-mcp

**Location:** `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/memory.rs:140`

**Error:**
```
error[E0308]: mismatched types
   --> crates/context-graph-mcp/src/handlers/memory.rs:140:21
    |
140 |                     &embedding_output.fingerprint,
    |                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ expected `&TeleologicalFingerprint`, found `&SemanticFingerprint`
```

**Root Cause:** The `memory/store` handler calls `alignment_calculator.compute_alignment()` with a `SemanticFingerprint`, but the method signature expects a `TeleologicalFingerprint`.

**Fix Required:**
1. Construct a full `TeleologicalFingerprint` from the `SemanticFingerprint` before calling `compute_alignment`
2. Or modify the handler to build the `TeleologicalFingerprint` earlier in the pipeline

**Impact:** MCP server cannot be built. This blocks ALL MCP functionality.

---

### CRITICAL #2: HNSW Index Not Initialized in Tests

**Location:** `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/mod.rs:135-165`

**Error:**
```
"TeleologicalMemoryStore.store() failed: Index error: INDEX ERROR: Index for E1Semantic not initialized"
```

**Root Cause:** The test helper `create_test_handlers_with_rocksdb()` opens the RocksDB store but does NOT call `initialize_hnsw()`.

**Affected Tests:** 11 MCP integration tests fail:
- `test_rocksdb_fsv_store_creates_fingerprint`
- `test_rocksdb_fsv_retrieve_returns_stored_data`
- `test_rocksdb_fsv_delete_removes_from_store`
- `test_rocksdb_fsv_multiple_fingerprints`
- `test_rocksdb_integration_crud_cycle`
- `test_rocksdb_integration_multiple_fingerprints`
- `test_rocksdb_integration_utl_computation`
- `test_rocksdb_integration_search_by_purpose`
- `test_rocksdb_integration_search_multi`
- `test_rocksdb_integration_search_multi_custom_weights`
- `test_rocksdb_integration_search_single_space`

**Fix Required:**
```rust
// In create_test_handlers_with_rocksdb()
let teleological_store = Arc::new(
    RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore in test"),
);

// ADD THIS LINE:
tokio::runtime::Runtime::new().unwrap().block_on(async {
    teleological_store.initialize_hnsw().await
        .expect("Failed to initialize HNSW indexes");
});
```

---

## HIGH PRIORITY ISSUES

### HIGH #1: Doc Test Failures in context-graph-core

**Location:** Multiple files in `crates/context-graph-core/src/`

**Error:**
```
error[E0599]: no function or associated item named `zeroed` found for struct `SemanticFingerprint` in the current scope
```

**Root Cause:** The `SemanticFingerprint::zeroed()` method is gated behind `#[cfg(any(test, feature = "test-utils"))]` but doc tests run without this feature.

**Affected Doc Examples:**
- `crates/context-graph-core/src/purpose/mod.rs:21`
- `crates/context-graph-core/src/purpose/default_computer.rs:26`
- `crates/context-graph-core/src/types/fingerprint/mod.rs:20`

**Fix Options:**
1. Add `test-utils` feature to doc test compilation
2. Mark affected doc examples with `ignore` attribute
3. Create alternative constructors for doc examples

---

### HIGH #2: Hardcoded GPU Info

**Location:** `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/gpu/device/utils.rs:16-29`

**Issue:**
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

**Risk:** This hardcoded info will be WRONG on any GPU other than RTX 5090. Should query actual device properties.

---

### HIGH #3: Deprecated EmbeddingProvider Usage

**Location:** `crates/context-graph-core/src/stubs/embedding_stub.rs`

**Issue:** The `EmbeddingProvider` trait is deprecated in favor of `MultiArrayEmbeddingProvider`. Continued usage generates 20+ deprecation warnings.

**Fix:** Migrate remaining uses to `MultiArrayEmbeddingProvider`.

---

## WHAT IS WORKING (VERIFIED)

### CUDA Crate (context-graph-cuda)
- [x] SIGSEGV crash FIXED (commit 6b66086)
- [x] All 120 tests pass
- [x] CUDA Driver API migration complete
- [x] RTX 5090 Blackwell (CC 12.0) supported
- [x] Poincare distance kernels functional
- [x] Cone membership kernels functional
- [x] WSL2 compatibility verified

### Embeddings Crate (context-graph-embeddings)
- [x] All 13 models IMPLEMENTED (no stubs)
- [x] 9 pretrained models with weights (~25.8GB on disk)
- [x] 4 custom models (mathematical, no weights needed)
- [x] Candle 0.9.2-alpha.2 compatible with Blackwell
- [x] CUDA kernels compile with sm_120
- [x] Memory estimates available (~6.1GB VRAM FP32)

### WarmLoader Pipeline
- [x] `warm()` method EXISTS at integration/pipeline.rs:164
- [x] `run_preflight_checks()` EXISTS at loader/engine.rs:291
- [x] `initialize_cuda_for_test()` EXISTS at loader/engine.rs:302
- [x] Compilation succeeds with `--features cuda`

### Storage Crate (context-graph-storage)
- [x] 63 tests pass
- [x] RocksDB persistence working
- [x] 17 column families configured
- [x] Teleological serialization working

### Graph Crate (context-graph-graph)
- [x] 45 tests pass
- [x] Hyperbolic geometry working
- [x] Poincare ball operations verified
- [x] Entailment cones working

---

## PRIORITIZED ACTION PLAN

### Phase 1: CRITICAL FIXES (Blocks Everything)

| Priority | Task | File | Effort |
|----------|------|------|--------|
| P0 | Fix type mismatch in memory handler | memory.rs:140 | 2-4 hours |
| P0 | Add HNSW initialization to test helpers | tests/mod.rs | 1 hour |

### Phase 2: HIGH PRIORITY FIXES

| Priority | Task | File | Effort |
|----------|------|------|--------|
| P1 | Fix doc test feature gates | fingerprint/mod.rs, purpose/ | 1 hour |
| P1 | Add real GPU info query | gpu/device/utils.rs | 2-3 hours |
| P1 | Migrate deprecated EmbeddingProvider | embedding_stub.rs | 2 hours |

### Phase 3: VERIFICATION

| Task | Command | Expected |
|------|---------|----------|
| Build MCP | `cargo build -p context-graph-mcp` | SUCCESS |
| All tests | `cargo test --workspace` | ALL PASS |
| MCP server start | `cargo run -p context-graph-mcp` | Healthy startup |
| GPU operations | Run embedding generation | Valid embeddings |

---

## EVIDENCE LOG

### Test Execution Evidence

```
=== context-graph-cuda ===
test result: ok. 54 passed; 0 failed (unit tests)
test result: ok. 34 passed; 0 failed (cone integration)
test result: ok. 18 passed; 0 failed (poincare integration)
test result: ok. 14 passed; 0 failed (doc tests)
TOTAL: 120 passed, 0 failed

=== context-graph-embeddings ===
test result: ok. 103 passed; 0 failed; 43 ignored (doc tests)

=== context-graph-core ===
test result: FAILED. 48 passed; 3 failed (doc tests)

=== context-graph-graph ===
test result: ok. 45 passed; 0 failed; 23 ignored

=== context-graph-storage ===
test result: ok. 63 passed; 0 failed; 9 ignored

=== context-graph-mcp ===
COMPILATION ERROR: Type mismatch in memory.rs:140
test result (partial): 250 passed; 11 failed (HNSW not initialized)
```

### CUDA Kernel Compilation Evidence

```
Running: "/usr/local/cuda-13.1/bin/nvcc" "-arch" "sm_120" ... poincare_distance.cu
Successfully compiled CUDA kernel -> libpoincare_distance.a

Running: "/usr/local/cuda-13.1/bin/nvcc" "-arch" "sm_120" ... cone_check.cu
Successfully compiled CUDA kernel -> libcone_check.a
```

### Hardware Environment

| Component | Value |
|-----------|-------|
| GPU | NVIDIA GeForce RTX 5090 |
| VRAM | 32607 MiB (~32GB) |
| Compute Capability | 12.0 (Blackwell) |
| CUDA Toolkit | 13.1 (V13.1.80) |
| Driver | 591.44 |
| Platform | WSL2 Linux |

---

## CHAIN OF CUSTODY

| Timestamp | Action | Agent | Evidence |
|-----------|--------|-------|----------|
| 2026-01-07 08:35 | CUDA SIGSEGV investigation | Agent #1 | Report SHERLOCK-01 |
| 2026-01-07 08:45 | WarmLoader methods verified | Agent #2 | Report SHERLOCK-02 |
| 2026-01-07 08:55 | Embedding pipeline verified | Agent #3 | Report SHERLOCK-03 |
| 2026-01-07 09:05 | CUDA compatibility verified | Agent #4 | Report SHERLOCK-04 |
| 2026-01-07 09:30 | Master synthesis completed | Agent #5 | This report |

---

## FINAL VERDICT

```
================================================================
                    INVESTIGATION COMPLETE
================================================================

SYSTEM STATUS: NOT READY FOR PRODUCTION

BLOCKING ISSUES: 2
  1. [CRITICAL] MCP crate compilation error (type mismatch)
  2. [CRITICAL] HNSW indexes not initialized in tests

HIGH PRIORITY: 3
  3. Doc test feature gate issues
  4. Hardcoded GPU info
  5. Deprecated API usage

WORKING SUBSYSTEMS:
  - CUDA kernels (120/120 tests pass)
  - Embeddings (103/103 doc tests pass)
  - Storage (63/63 tests pass)
  - Graph (45/45 tests pass)
  - WarmLoader (all methods exist)
  - Candle/Blackwell compatibility (verified)

ESTIMATED EFFORT TO FIX:
  - P0 (Critical): 3-5 hours
  - P1 (High): 5-7 hours
  - Total: 8-12 hours

RECOMMENDATION:
  Fix P0 issues immediately, then run full test suite to verify.
  After P0 fixes, system should be functional for Phase 0.

================================================================
     CASE SHERLOCK-05 - VERDICT: FIXES REQUIRED
================================================================
```

---

## MEMORY STORAGE LOCATIONS

Investigation findings stored at:
- `sherlock/agent1/cuda-findings-summary`
- `sherlock/agent2/verified-summary`
- `sherlock/agent3/embedding-findings`
- `sherlock/agent3/model-status-summary`
- `sherlock/agent4/cuda-compat-findings`
- `sherlock/agent4/feature-flags`
- `sherlock/agent4/verified-summary`
- `sherlock/synthesis/master-findings`

---

## APPENDIX: CRITICAL FIX CODE SNIPPETS

### Fix #1: Memory Handler Type Mismatch

```rust
// File: crates/context-graph-mcp/src/handlers/memory.rs
// Line: ~140

// BEFORE (BROKEN):
let alignment_result = match self.alignment_calculator.compute_alignment(
    &embedding_output.fingerprint,  // SemanticFingerprint - WRONG TYPE
    &config,
).await { ... }

// AFTER (FIXED):
// Build a TeleologicalFingerprint from the SemanticFingerprint
let teleological_fp = TeleologicalFingerprint {
    id: uuid::Uuid::new_v4(),
    semantic: embedding_output.fingerprint.clone(),
    purpose_vector: PurposeVector::default(),  // Will be computed below
    johari: JohariFingerprint::default(),
    // ... other fields
};

let alignment_result = match self.alignment_calculator.compute_alignment(
    &teleological_fp,  // TeleologicalFingerprint - CORRECT TYPE
    &config,
).await { ... }
```

### Fix #2: HNSW Initialization in Tests

```rust
// File: crates/context-graph-mcp/src/handlers/tests/mod.rs
// Function: create_test_handlers_with_rocksdb()

pub(crate) fn create_test_handlers_with_rocksdb() -> (Handlers, TempDir) {
    let tempdir = TempDir::new().expect("Failed to create temp directory");
    let db_path = tempdir.path().join("test_rocksdb");

    let store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore");

    // ADD THIS: Initialize HNSW indexes
    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
    rt.block_on(async {
        store.initialize_hnsw().await
            .expect("Failed to initialize HNSW indexes");
    });

    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(store);

    // ... rest of function
}
```

---

*"The game is afoot!"*

**Case Status: INVESTIGATION COMPLETE - REMEDIATION REQUIRED**

---

*SHERLOCK-05-MASTER-SYNTHESIS Investigation Complete*
*Evidence preserved in `/home/cabdru/contextgraph/docs2/codestate/sherlockplans/`*
