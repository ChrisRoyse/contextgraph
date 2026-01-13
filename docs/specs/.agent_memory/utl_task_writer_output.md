# UTL Task Spec Writer Agent Output

## Session: 2026-01-12
## Agent: UTL Task Spec Writer
## Status: COMPLETE

---

## Files Created

| File | Absolute Path | Purpose |
|------|---------------|---------|
| TASK-UTL-P2-001.md | `/home/cabdru/contextgraph/docs/specs/tasks/TASK-UTL-P2-001.md` | sklearn reference validation tests for silhouette algorithm |
| TASK-UTL-P2-002.md | `/home/cabdru/contextgraph/docs/specs/tasks/TASK-UTL-P2-002.md` | ClusterFit benchmark with <2ms p95 target |
| Agent Memory | `/home/cabdru/contextgraph/docs/specs/.agent_memory/utl_task_writer_output.md` | This file - coordination with next agent |

---

## Summary of Tasks Created

### TASK-UTL-P2-001: sklearn Reference Validation Tests

**Purpose**: Validate numerical correctness of silhouette coefficient implementation against sklearn/scipy reference values.

**Key Details**:
- Adds 6 test functions to `tests.rs`
- Pre-computed reference values from sklearn.metrics.silhouette_score
- Validates euclidean_distance, cosine_distance, manhattan_distance
- Tolerance: 1e-5 for f32 vs f64 precision differences
- Includes Python reference generation script documentation

**Files to Modify**:
- `crates/context-graph-utl/src/coherence/cluster_fit/tests.rs`

**Constitution Rules**:
- UTL-002: Ensures delta_C ClusterFit term is numerically correct
- AP-10: Verifies no NaN/Infinity in outputs

---

### TASK-UTL-P2-002: ClusterFit Benchmark

**Purpose**: Add criterion benchmarks with performance regression detection.

**Key Details**:
- Target: < 2ms p95 for 1000 embeddings at 1536 dimensions (Cosine distance)
- Scaling benchmarks for cluster sizes (10, 50, 100, 500, 1000)
- Scaling benchmarks for dimensions (384, 768, 1536, 3072)
- Distance metric comparison (Cosine vs Euclidean vs Manhattan)
- Sampling threshold impact benchmarks

**Files to Create**:
- `crates/context-graph-utl/benches/cluster_fit_bench.rs`

**Files to Modify**:
- `crates/context-graph-utl/Cargo.toml` (add [[bench]] entry)

**Constitution Rules**:
- UTL-002: ClusterFit performance matters for delta_C hot path

---

## Source Files Analyzed

| File | Path | Analysis |
|------|------|----------|
| compute.rs | `crates/context-graph-utl/src/coherence/cluster_fit/compute.rs` | 141 lines, silhouette formula implementation |
| distance.rs | `crates/context-graph-utl/src/coherence/cluster_fit/distance.rs` | 171 lines, distance metrics |
| types.rs | `crates/context-graph-utl/src/coherence/cluster_fit/types.rs` | 156 lines, ClusterFitConfig, ClusterContext |
| tests.rs | `crates/context-graph-utl/src/coherence/cluster_fit/tests.rs` | 729 lines, existing test suite |
| utl_bench.rs | `crates/context-graph-utl/benches/utl_bench.rs` | 399 lines, benchmark patterns |
| Cargo.toml | `crates/context-graph-utl/Cargo.toml` | Benchmark configuration |

---

## Constitution Rules Verified

| Rule ID | Rule Text | Task Coverage |
|---------|-----------|---------------|
| UTL-001 | compute_delta_sc MCP tool MUST exist | Out of scope (MCP layer) |
| UTL-002 | delta_C = 0.4*Connectivity + 0.4*ClusterFit + 0.2*Consistency | Both tasks |
| UTL-003 | Each embedder uses constitution-specified delta_S method | Out of scope |
| AP-10 | No NaN/Infinity in outputs | TASK-UTL-P2-001 |
| AP-33 | delta_C MUST include ClusterFit | Both tasks |

---

## Key Implementation Details

### Silhouette Coefficient Formula
```
s = (b - a) / max(a, b)
```
Where:
- `a` = mean intra-cluster distance (to same-cluster members)
- `b` = mean nearest-cluster distance (to nearest other cluster)
- Output range: [-1, 1]
- Normalized to [0, 1] for UTL formula: `score = (s + 1) / 2`

### Distance Metrics Supported
1. **Cosine** (default): `1 - cosine_similarity`, range [0, 2]
2. **Euclidean**: L2 norm, range [0, inf)
3. **Manhattan**: L1 norm, range [0, inf)

### Performance Characteristics
- Sampling enabled for clusters > 1000 members (configurable via max_sample_size)
- O(n) complexity with sampling, O(n) without for small clusters
- Current implementation is synchronous (no async overhead)

---

## Critical Rules for Implementation

### NO Workarounds
- Tests MUST fail if reference values don't match within tolerance
- NO mock data - use REAL pre-computed expected values
- NO fallback logic when errors occur

### NO Backwards Compatibility Hacks
- Benchmark targets are firm: < 2ms p95
- If performance regresses, fix the algorithm - don't adjust targets

### Reference Values
- All sklearn reference values MUST be documented with Python generation code
- Tolerance is 1e-5 (accounting for f32 vs f64 precision)

---

## Dependencies

### TASK-UTL-P2-001 Prerequisites
- cluster_fit module exists (verified)
- tests.rs exists (verified)
- All distance functions exist (verified)

### TASK-UTL-P2-002 Prerequisites
- criterion dev-dependency exists (verified)
- [[bench]] setup pattern exists (verified)
- TASK-UTL-P2-001 should complete first (sklearn tests validate correctness before benchmarking)

---

## Recommended Execution Order

1. **TASK-UTL-P2-001**: Add sklearn validation tests first
   - Validates numerical correctness before benchmarking
   - Low risk, additive tests

2. **TASK-UTL-P2-002**: Add benchmarks second
   - Depends on correctness being validated
   - Creates baseline for performance regression detection

---

## Test Commands Summary

### TASK-UTL-P2-001
```bash
cargo test -p context-graph-utl --lib coherence::cluster_fit::tests::test_silhouette_sklearn -- --nocapture
cargo test -p context-graph-utl --lib coherence::cluster_fit::tests::test_euclidean_distance_scipy -- --nocapture
cargo test -p context-graph-utl --lib coherence::cluster_fit -- --nocapture
```

### TASK-UTL-P2-002
```bash
cargo build -p context-graph-utl --benches
cargo bench -p context-graph-utl --bench cluster_fit_bench -- --noplot
cargo bench -p context-graph-utl --bench cluster_fit_bench cluster_fit_1000 -- --noplot
```

---

*UTL Task Spec Writer Agent Complete*
