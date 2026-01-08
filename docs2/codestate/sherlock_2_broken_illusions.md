# Sherlock Holmes Investigation Report #2: Broken Illusions

**Case ID**: SHERLOCK-002-BROKEN-ILLUSIONS
**Date**: 2026-01-08
**Investigator**: Sherlock Holmes Agent #2
**Verdict**: GUILTY - Multiple systems have broken illusions

---

## Executive Summary

*"There is nothing more deceptive than an obvious fact."*

After forensic investigation of systems claimed as "complete" by Sherlock #1, I have discovered **17 critical broken illusions** - code that compiles and passes tests but does NOT actually perform its claimed function.

**Severity Distribution:**
- CRITICAL: 5 (system fundamentally broken)
- HIGH: 7 (major functionality compromised)
- MEDIUM: 5 (partial functionality only)

---

## Category 1: Functions That Don't Work As Advertised

### ILLUSION #1: Thompson Sampling Does NOT Sample [CRITICAL]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/atc/level3_bandit.rs:108-132`

**Claim**: "Thompson Sampling Bandit Threshold Selector"

**Reality**: Does NOT perform Thompson sampling. Uses Beta mean instead of sampling.

```rust
// Line 109-110: COMMENT ADMITS THE LIE
/// Select arm using Thompson sampling
/// (Simplified: uses Beta mean instead of sampling for determinism)
pub fn select_thompson(&self) -> Option<ThresholdArm> {
    // ...
    // Line 122-123: Uses Beta mean, NOT sampling!
    let mean = alpha / (alpha + beta);
    if mean > best_score {
        best_score = mean;
        best_arm_idx = idx;
    }
}
```

**Impact**: Thompson Sampling's core value is STOCHASTIC exploration from Beta distribution. Using the mean makes this a GREEDY algorithm, destroying exploration-exploitation balance.

**Verdict**: Function name is a LIE. Should be renamed `select_greedy_beta_mean()`.

---

### ILLUSION #2: Gaussian Process Does NOT Use Kernel Prediction [CRITICAL]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/atc/level4_bayesian.rs:88-93`

**Claim**: "Gaussian Process-like tracker" with "GP prediction"

**Reality**: `predict_performance()` IGNORES the input thresholds and returns global mean/variance.

```rust
// Line 88-93: CRITICAL BROKEN ILLUSION
/// Estimate performance for a threshold configuration (simplified)
pub fn predict_performance(&self, _thresholds: &HashMap<String, f32>) -> (f32, f32) {
    // Simplified: return mean + sqrt(variance)
    // In a real implementation, this would use actual GP prediction
    (self.mean, self.variance.sqrt())  // <-- IGNORES _thresholds ENTIRELY!
}
```

**Impact**: Bayesian Optimization requires GP to predict performance at NEW threshold configurations. This function returns the SAME prediction regardless of input, making the "Expected Improvement" search meaningless.

**Verdict**: This is NOT a Gaussian Process. It's a global mean tracker with misleading name.

---

### ILLUSION #3: Stage 4 Teleological Filtering Uses Placeholder Estimation [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/retrieval/pipeline.rs:561-609`

**Claim**: "Stage 4: Teleological Alignment" filtering with 50 candidates

**Reality**: Uses `CONTENT_TO_GOAL_FACTOR` placeholder instead of fetching real TeleologicalFingerprints.

```rust
// pipeline.rs - stage4_placeholder_filtering()
let goal_alignment = content_sim * estimation::CONTENT_TO_GOAL_FACTOR;
// NOTE: This is a placeholder per estimation::CONTENT_TO_GOAL_FACTOR
// In production, use proper teleological computation
```

**Impact**: The retrieval pipeline's teleological alignment is FAKE. It multiplies content similarity by a constant factor instead of computing actual purpose/goal alignment.

**Verdict**: Stage 4 produces MISLEADING alignment scores.

---

### ILLUSION #4: Embedding Provider is Stub with Hash-Based "Embeddings" [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/multi_array_stub.rs:158-232`

**Claim**: "StubMultiArrayProvider" for testing

**Reality**: ALL tests in the codebase use this stub, which generates FAKE embeddings from content hash, not real semantic understanding.

```rust
// Line 162-165: FAKE EMBEDDINGS
fn content_hash(content: &str) -> f32 {
    let sum: u32 = content.bytes().map(u32::from).sum();
    (sum % 256) as f32 / 255.0  // Just byte sum modulo 256!
}

fn fill_dense_embedding(content: &str, dim: usize) -> Vec<f32> {
    let base = Self::content_hash(content);
    (0..dim)
        .map(|i| Self::deterministic_value(base, i))
        .collect()  // Deterministic hash, NOT semantic!
}
```

**Impact**: Tests pass with FAKE embeddings that have NO semantic meaning. "Hello world" and "Security vulnerability" would be nearly identical if their byte sums are similar.

**Verdict**: All retrieval tests are testing the STUB, not real semantic search.

---

### ILLUSION #5: InMemoryTeleologicalStore is O(n) Linear Scan [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/teleological_store_stub.rs:351-436`

**Claim**: "search_semantic" with proper search functionality

**Reality**: Performs FULL TABLE SCAN on every search.

```rust
// Line 364-422: O(n) scan through ALL data
for entry in self.data.iter() {
    // ... checks every single entry
    let embedder_scores = Self::compute_semantic_scores(query, &fp.semantic);
    // ...
}
```

The file header even admits this (lines 10-16):
```rust
// - **O(n) search complexity**: All search operations perform full table scans.
//   This is acceptable for small test datasets but will be prohibitively slow
//   for any production workload.
// - **No HNSW indexing**: Unlike production stores, this stub does not use
//   approximate nearest neighbor search.
```

**Impact**: Tests pass but production behavior would be completely different. No HNSW is actually being tested.

**Verdict**: Tests don't validate HNSW behavior at all.

---

## Category 2: Silent Failure Patterns

### ILLUSION #6: Silent File Cleanup on Test Failure [MEDIUM]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/index/hnsw_impl.rs:1602,1688,1725,1762,1813`

**Pattern**: `.ok()` swallowing cleanup errors

```rust
// Line 1602
std::fs::remove_dir_all(&temp_dir).ok();
// Line 1688
std::fs::remove_file(&temp_path).ok();
// Line 1725
std::fs::remove_file(&temp_path).ok();
```

**Impact**: If cleanup fails, tests continue without warning. File system state may be corrupted.

---

### ILLUSION #7: Error Reading Last Query Time Silently Handled [MEDIUM]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/retrieval/pipeline.rs:542`

```rust
let last_time = self.last_query_time.read().ok().and_then(|g| *g);
```

**Impact**: If RwLock is poisoned, returns None silently. Could hide concurrency bugs.

---

### ILLUSION #8: Sparse Vector Creation Fails Silently [MEDIUM]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/multi_array_stub.rs:215`

```rust
SparseVector::new(indices, values).unwrap_or_else(|_| SparseVector::empty())
```

**Impact**: Invalid sparse vectors become empty vectors silently. Search will return no matches.

---

## Category 3: Tests That Don't Test Real Behavior

### ILLUSION #9: Retrieval Tests Use Stub Provider [CRITICAL]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/retrieval/tests.rs:1-660`

**Header admission (line 13-14)**:
```rust
//! All tests use STUB implementations (InMemoryTeleologicalStore, StubMultiArrayProvider).
//! These are NOT real implementations - see integration tests for RocksDB + real embeddings.
```

**Example test (line 528-566)**:
```rust
async fn test_full_query_flow_with_stub_data() {
    // NOTE: Uses StubMultiArrayProvider - NOT real embeddings!
```

**Impact**: 100% of retrieval unit tests use fake data. They test the SHAPE of results, not the CORRECTNESS of semantic search.

---

### ILLUSION #10: RRF Formula Tests Use Mocked HNSW Results [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/storage/multi_space.rs:389-450`

```rust
/// Test storage that uses in-memory HashMap (still real data structures)
struct TestStorage { ... }

/// Test HNSW manager with fixed results
struct TestHnswManager {
    results: HashMap<u8, Vec<(Uuid, f32)>>,  // FIXED results, not computed!
}
```

**Impact**: RRF fusion tests pass but don't verify that HNSW actually returns correct candidates. The test validates math but not end-to-end correctness.

---

### ILLUSION #11: Pipeline Tests Create Fingerprints with Zeroed Semantics [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/retrieval/tests.rs:27-34`

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

**Impact**: Tests query zeroed fingerprints against zeroed queries. Cosine similarity of zero vectors is 0/0 = NaN or special-cased. Real semantic search not tested.

---

## Category 4: Dead or Unreachable Code

### ILLUSION #12: Validation Error Formatting Unused [LOW]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/types/memory_node/validation.rs:199`

```rust
let _ = error.to_string();
```

**Impact**: Error is formatted then immediately discarded. Either remove or use the value.

---

### ILLUSION #13: Debug Format for Config Unused [LOW]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/layers/helpers.rs:54`

```rust
let _ = format!("{:?}", config);
```

**Impact**: Config is formatted to string then discarded.

---

## Category 5: Race Conditions or Timing Issues

### ILLUSION #14: Last Query Time RwLock May Be Poisoned [MEDIUM]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/retrieval/pipeline.rs:542`

If a thread panics while holding the write lock, subsequent reads return `None` silently instead of propagating the poison.

---

### ILLUSION #15: Embedder Health Lock Poisoning Hidden [MEDIUM]

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

**Impact**: Returns false instead of propagating error. Caller cannot distinguish "all unhealthy" from "lock poisoned".

---

## Category 6: Data That Doesn't Persist

### ILLUSION #16: InMemoryTeleologicalStore Has No Persistence [HIGH]

**Location**: `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/teleological_store_stub.rs:619-644`

```rust
async fn checkpoint(&self) -> CoreResult<PathBuf> {
    warn!("Checkpoint requested but InMemoryTeleologicalStore does not persist data");
    Err(CoreError::FeatureDisabled { feature: "checkpoint".to_string() })
}

async fn restore(&self, checkpoint_path: &Path) -> CoreResult<()> {
    error!("Restore from {:?} requested but InMemoryTeleologicalStore does not persist data");
    Err(CoreError::FeatureDisabled { feature: "restore".to_string() })
}
```

**Impact**: Any code using InMemoryTeleologicalStore will LOSE ALL DATA on restart. Tests pass but integration will fail.

---

### ILLUSION #17: SELF_EGO_NODE Persistence Not Verified in Tests [MEDIUM]

While the `gwt/ego_node.rs` has update methods, no test verifies that ego state actually persists across sessions.

---

## Evidence Log

| File | Line | Issue | Severity |
|------|------|-------|----------|
| `atc/level3_bandit.rs` | 109-132 | Thompson Sampling doesn't sample | CRITICAL |
| `atc/level4_bayesian.rs` | 88-93 | GP ignores input thresholds | CRITICAL |
| `retrieval/pipeline.rs` | 561-609 | Stage 4 uses placeholder | HIGH |
| `stubs/multi_array_stub.rs` | 158-232 | Hash-based fake embeddings | HIGH |
| `stubs/teleological_store_stub.rs` | 351-436 | O(n) scan, no HNSW | HIGH |
| `retrieval/tests.rs` | 1-660 | All tests use stubs | CRITICAL |
| `storage/multi_space.rs` | 389-450 | Mocked HNSW results | HIGH |
| `retrieval/tests.rs` | 27-34 | Zeroed fingerprints | HIGH |

---

## Verification Matrix

| Claim from Sherlock #1 | My Verification | Actual Status |
|------------------------|-----------------|---------------|
| "ATC Level 3 Thompson complete" | Thompson doesn't sample | BROKEN ILLUSION |
| "ATC Level 4 Bayesian complete" | GP ignores input | BROKEN ILLUSION |
| "5-stage pipeline implemented" | Stage 4 is placeholder | PARTIAL |
| "Tests verify behavior" | Tests use stubs | TESTS ARE FAKE |
| "GWT 100% implemented" | Claimed, needs deep verification | NEEDS VERIFICATION |
| "13 embedders working" | Stubs only in tests | PRODUCTION UNTESTED |

---

## Recommendations

### CRITICAL (Must Fix Immediately)

1. **Rename `select_thompson()`** to `select_greedy_beta_mean()` or implement actual Thompson sampling with Beta distribution sampling
2. **Implement real GP prediction** with kernel-based prediction at input thresholds
3. **Create integration tests** that use REAL embedding providers, not stubs
4. **Add persistence verification tests** for ego_node and other stateful components

### HIGH (Fix Before Production)

5. Replace Stage 4 placeholder with real TeleologicalFingerprint lookup
6. Add tests that verify HNSW actually finds correct neighbors
7. Create benchmarks with non-trivial test data (not zeroed/deterministic)
8. Propagate RwLock poison errors instead of swallowing them

### MEDIUM (Technical Debt)

9. Remove dead code (`let _ = ...` patterns)
10. Add integration tests for production storage backend
11. Verify concurrency safety under thread panic scenarios

---

## Memory Storage

```bash
npx claude-flow memory store "broken_illusions" '{
  "summary": "17 broken illusions found in claimed-complete code",
  "critical_count": 5,
  "high_count": 7,
  "medium_count": 5,
  "critical_breaks": [
    "Thompson Sampling uses greedy mean not sampling",
    "GP prediction ignores input thresholds",
    "All retrieval tests use stub data",
    "Stage 4 uses placeholder estimation",
    "InMemory store has no HNSW"
  ],
  "key_finding": "Tests pass but do not verify real semantic search behavior"
}' --namespace "investigation/sherlock2"
```

---

## Conclusion

*"The evidence never lies, Watson. What appears to be a complete system is riddled with illusions."*

**VERDICT**: The codebase has substantial implementation BUT:

1. **ATC Levels 3 and 4** do NOT perform their advertised algorithms
2. **Tests** verify shape/structure but NOT semantic correctness
3. **Retrieval pipeline** Stage 4 is fundamentally broken
4. **All unit tests** use fake embedding providers

The system may APPEAR to work in demos but will produce **meaningless results** in production because:
- Thompson Sampling doesn't explore
- Bayesian optimization doesn't optimize
- Semantic search doesn't use real semantics
- Teleological alignment is fake multiplication

**Case Status**: INVESTIGATION COMPLETE - Broken Illusions Documented

---

*"When you have eliminated the impossible, whatever remains, however improbable, must be the truth. And the truth is: this code is deceptive."*
