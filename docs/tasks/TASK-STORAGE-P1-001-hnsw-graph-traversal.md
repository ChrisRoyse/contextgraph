# TASK-STORAGE-P1-001: Replace HNSW Brute Force with Graph Traversal

```xml
<task_spec id="TASK-STORAGE-P1-001" version="1.0">
<metadata>
  <title>Replace HNSW Brute Force Linear Scan with Proper Graph Traversal</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>1</sequence>
  <priority>P1-CRITICAL</priority>
  <implements>
    <item>Sherlock-08: HNSW brute force replacement</item>
    <item>Performance target: <10ms search @ 1M vectors (vs current 1-5 seconds)</item>
    <item>O(log n) search complexity via graph traversal</item>
  </implements>
  <depends_on>
    <!-- No dependencies - this is a drop-in replacement -->
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_effort>2-3 days</estimated_effort>
</metadata>

<context>
The current HnswEmbedderIndex implementation in `hnsw_impl.rs` uses O(n) brute force
linear scan instead of actual HNSW graph traversal. The search method (lines 175-217)
iterates through ALL vectors and computes distances, which is a placeholder comment
explicitly states: "// Compute distances for all vectors (brute force - placeholder for real HNSW)".

At 1M memories, this results in 1-5 second search latency vs the target of <60ms.
This task replaces the brute force implementation with proper HNSW graph traversal
using the usearch crate (Rust bindings to USearch, fastest HNSW implementation).

The existing interface (EmbedderIndexOps trait) is preserved - only the internal
implementation changes.
</context>

<problem_analysis>
<current_state>
  <file path="crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs">
    - Lines 47-54: Uses flat storage (HashMap, Vec, Vec) instead of HNSW graph
    - Lines 191-206: Brute force O(n) linear scan computing distance to ALL vectors
    - Lines 219-226: Sequential batch insert (no HNSW graph construction)
    - No index persistence (rebuilds on startup)
  </file>
  <performance>
    - 1M vectors @ 384D: ~1-5 seconds per search
    - Target: <10ms per search (100x improvement required)
    - Memory overhead: O(n * d * 4) bytes (just vectors, no graph)
  </performance>
</current_state>

<root_cause>
  The implementation stores vectors in a flat Vec<Vec<f32>> and performs linear
  distance computation on every search. HNSW requires a hierarchical navigable
  small world graph structure with:
  1. Multi-layer skip-list-like graph
  2. Greedy graph traversal from entry point
  3. Neighborhood pruning (select_neighbors heuristic)
</root_cause>

<solution_approach>
  Use usearch-rs (USearch Rust bindings) which provides:
  1. Production-grade HNSW implementation (used by Qdrant, etc.)
  2. C++ core with Rust FFI bindings
  3. Support for all required distance metrics (cosine, dot, euclidean)
  4. Index persistence to disk
  5. Thread-safe operations

  Alternative considered: hnswlib-rs (Rust bindings to hnswlib)
  - Pro: Well-tested, original HNSW implementation
  - Con: Less active maintenance, harder to build

  Recommended: usearch crate (version 2.x)
  - MIT licensed, actively maintained
  - Better Rust API, cross-platform builds
  - Same performance characteristics as hnswlib
</solution_approach>
</problem_analysis>

<input_context_files>
  <file purpose="Current brute force implementation to replace">
    crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs
  </file>
  <file purpose="Trait interface that must be preserved">
    crates/context-graph-storage/src/teleological/indexes/embedder_index.rs
  </file>
  <file purpose="HNSW configuration (m, ef_construction, ef_search)">
    crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs
  </file>
  <file purpose="Distance metric implementations">
    crates/context-graph-storage/src/teleological/indexes/metrics.rs
  </file>
  <file purpose="Cargo dependencies to modify">
    crates/context-graph-storage/Cargo.toml
  </file>
  <file purpose="Module re-exports">
    crates/context-graph-storage/src/teleological/indexes/mod.rs
  </file>
</input_context_files>

<prerequisites>
  <check>usearch crate available on crates.io (verified: version 2.x available)</check>
  <check>All existing tests in hnsw_impl.rs pass (cargo test --package context-graph-storage)</check>
  <check>EmbedderIndexOps trait interface is stable and documented</check>
  <check>HnswConfig parameters (m, ef_construction, ef_search) are defined</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace internal storage from Vec<Vec<f32>> to usearch::Index
    - Implement proper HNSW graph construction in insert/insert_batch
    - Implement O(log n) graph traversal in search method
    - Map DistanceMetric enum to usearch::MetricKind
    - Preserve all EmbedderIndexOps trait semantics
    - Update Cargo.toml with usearch dependency
    - All existing tests must pass unchanged
    - Add benchmark test comparing old vs new implementation
  </in_scope>
  <out_of_scope>
    - Index persistence to disk (separate task: TASK-STORAGE-P1-002)
    - Concurrent index operations (already handled by RwLock)
    - Quantization/compression (separate task: TASK-STORAGE-P2-001)
    - E6/E12/E13 non-HNSW indexes (unchanged)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-storage/Cargo.toml">
      [dependencies]
      usearch = "2"  # Or latest 2.x version
    </signature>

    <signature file="crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs">
      // Storage changes from:
      // vectors: RwLock<Vec<Vec<f32>>>
      // To:
      use usearch::Index;

      pub struct HnswEmbedderIndex {
          embedder: EmbedderIndex,
          config: HnswConfig,
          index: RwLock<Index>,
          id_to_key: RwLock<HashMap<Uuid, u64>>,
          key_to_id: RwLock<HashMap<u64, Uuid>>,
          next_key: RwLock<u64>,
      }
    </signature>

    <signature file="crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs">
      impl HnswEmbedderIndex {
          pub fn new(embedder: EmbedderIndex) -> Self {
              // Creates usearch::Index with proper HNSW parameters
          }

          fn metric_to_usearch(metric: DistanceMetric) -> usearch::MetricKind {
              // Maps our DistanceMetric to usearch::MetricKind
          }
      }
    </signature>

    <signature file="crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs">
      impl EmbedderIndexOps for HnswEmbedderIndex {
          fn search(
              &self,
              query: &[f32],
              k: usize,
              ef_search: Option<usize>,
          ) -> IndexResult<Vec<(Uuid, f32)>> {
              // Uses index.search() with O(log n) graph traversal
              // NOT brute force linear scan
          }
      }
    </signature>
  </signatures>

  <constraints>
    - MUST use usearch crate (or hnswlib-rs as fallback)
    - MUST NOT change EmbedderIndexOps trait signature
    - MUST preserve all FAIL FAST error handling (dimension mismatch, NaN/Inf)
    - MUST preserve thread-safety (Send + Sync)
    - MUST preserve duplicate ID update behavior
    - Search complexity MUST be O(log n), not O(n)
    - Memory overhead MUST include graph structure (acceptable increase)
    - All 12 HNSW embedder configs MUST work correctly
  </constraints>

  <verification>
    - All 20 existing tests in hnsw_impl.rs pass unchanged
    - New benchmark test shows >10x speedup at 10K+ vectors
    - cargo clippy --package context-graph-storage passes
    - cargo doc --package context-graph-storage generates docs
    - Memory usage is within 2x of previous (graph overhead acceptable)
  </verification>
</definition_of_done>

<pseudo_code>
HnswEmbedderIndex::new(embedder) (hnsw_impl.rs):
  config = get_hnsw_config(embedder) or PANIC
  usearch_metric = metric_to_usearch(config.metric)
  usearch_options = IndexOptions {
    dimensions: config.dimension,
    metric: usearch_metric,
    connectivity: config.m,  // M parameter
    expansion_add: config.ef_construction,
    expansion_search: config.ef_search,
    quantization: ScalarKind::F32,
  }
  index = Index::new(&usearch_options)
  return Self { embedder, config, index: RwLock::new(index), ... }

metric_to_usearch(metric) (hnsw_impl.rs):
  match metric:
    Cosine -> MetricKind::Cos
    DotProduct -> MetricKind::IP (inner product)
    Euclidean -> MetricKind::L2sq
    AsymmetricCosine -> MetricKind::Cos (handled at query time)
    MaxSim -> PANIC (not used for HNSW)

insert(id, vector) (hnsw_impl.rs):
  validate_vector(vector, config.dimension, embedder)?

  lock id_to_key, key_to_id, index, next_key for write

  if id already exists:
    old_key = id_to_key[id]
    index.remove(old_key)  // Remove old vector
  else:
    key = next_key++
    id_to_key[id] = key
    key_to_id[key] = id

  key = id_to_key[id]
  index.add(key, vector)  // Adds to HNSW graph with proper construction

  return Ok(())

search(query, k, ef_search) (hnsw_impl.rs):
  validate_vector(query, config.dimension, embedder)?

  lock index, id_to_key for read

  if index.is_empty():
    return Ok([])

  // Temporarily override ef_search if provided
  effective_ef = ef_search.unwrap_or(config.ef_search)

  // O(log n) HNSW graph traversal - NOT brute force
  results = index.search(query, k)  // Returns (keys, distances)

  // Map keys back to UUIDs
  output = []
  for (key, distance) in results:
    if id = key_to_id.get(key):
      output.push((id, distance))

  return Ok(output)

insert_batch(items) (hnsw_impl.rs):
  // Use usearch batch add for better HNSW construction
  validate all vectors first (fail fast)

  lock all for write

  keys = []
  vectors = []
  for (id, vec) in items:
    key = next_key++
    id_to_key[id] = key
    key_to_id[key] = id
    keys.push(key)
    vectors.push(vec)

  // Single batch add builds better HNSW graph
  index.add_batch(&keys, &vectors)

  return Ok(items.len())

remove(id) (hnsw_impl.rs):
  lock id_to_key for write

  if key = id_to_key.remove(id):
    // Note: usearch may not support true deletion
    // Mark as removed in our mapping, skip in search results
    return Ok(true)
  else:
    return Ok(false)
</pseudo_code>

<files_to_modify>
  <file path="crates/context-graph-storage/Cargo.toml">
    Add usearch = "2" to [dependencies]
  </file>
  <file path="crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs">
    Replace entire implementation while preserving trait interface
  </file>
</files_to_modify>

<files_to_create>
  <file path="crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs">
    Complete rewrite with usearch-based HNSW (replaces existing)
  </file>
</files_to_create>

<validation_criteria>
  <criterion>cargo test --package context-graph-storage -- hnsw passes all 20 tests</criterion>
  <criterion>Benchmark at 10K vectors shows >10x speedup vs brute force</criterion>
  <criterion>Benchmark at 100K vectors shows >50x speedup vs brute force</criterion>
  <criterion>Search latency at 1M vectors is <50ms (measured, not estimated)</criterion>
  <criterion>cargo clippy --package context-graph-storage has no warnings</criterion>
  <criterion>All 12 HNSW embedder types work correctly (verified in test_all_hnsw_embedders)</criterion>
  <criterion>Memory usage at 100K vectors 384D is <500MB</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --package context-graph-storage</command>
  <command>cargo test --package context-graph-storage -- hnsw --nocapture</command>
  <command>cargo test --package context-graph-storage -- test_all_hnsw_embedders --nocapture</command>
  <command>cargo clippy --package context-graph-storage -- -D warnings</command>
  <command>cargo bench --package context-graph-storage -- hnsw_search (if benchmark exists)</command>
</test_commands>

<performance_targets>
  <target metric="search_latency_10k" value="<1ms" unit="milliseconds">
    Search latency with 10,000 vectors (384D, cosine)
  </target>
  <target metric="search_latency_100k" value="<5ms" unit="milliseconds">
    Search latency with 100,000 vectors (384D, cosine)
  </target>
  <target metric="search_latency_1m" value="<10ms" unit="milliseconds">
    Search latency with 1,000,000 vectors (384D, cosine)
  </target>
  <target metric="insert_throughput" value=">10000" unit="vectors/second">
    Batch insert throughput (384D vectors)
  </target>
  <target metric="memory_per_vector" value="<2KB" unit="bytes">
    Memory per vector including graph overhead (384D)
  </target>
</performance_targets>

<rollback_plan>
  If usearch integration fails:
  1. Keep existing brute force implementation (it works, just slow)
  2. Consider hnswlib-rs as alternative binding
  3. Consider pure-Rust hnsw implementation (slower but no FFI)
  4. File issue with performance impact assessment
</rollback_plan>

<references>
  <reference type="issue">Sherlock-08: HNSW brute force at line 191-206</reference>
  <reference type="crate">https://crates.io/crates/usearch</reference>
  <reference type="paper">Malkov & Yashunin, "Efficient and robust approximate nearest neighbor search using HNSW graphs"</reference>
  <reference type="code">https://github.com/unum-cloud/usearch</reference>
</references>
</task_spec>
```

## Summary

| Attribute | Value |
|-----------|-------|
| Task ID | TASK-STORAGE-P1-001 |
| Title | Replace HNSW Brute Force with Graph Traversal |
| Layer | logic |
| Priority | P1-CRITICAL |
| Complexity | High |
| Effort | 2-3 days |
| Dependencies | None |

## Problem Statement

The current `HnswEmbedderIndex::search()` method performs **O(n) brute force linear scan** instead of actual HNSW graph traversal:

```rust
// Current implementation (hnsw_impl.rs lines 191-206)
// Compute distances for all vectors (brute force - placeholder for real HNSW)
let mut distances: Vec<(usize, f32)> = vectors
    .iter()
    .enumerate()
    .filter(...)
    .map(|(idx, vec)| {
        let dist = compute_distance(query, vec, self.config.metric);
        (idx, dist)
    })
    .collect();
```

### Performance Impact

| Vector Count | Current Latency | Target Latency | Improvement Needed |
|--------------|-----------------|----------------|-------------------|
| 10K | ~10-50ms | <1ms | 10-50x |
| 100K | ~100-500ms | <5ms | 20-100x |
| 1M | **1-5 seconds** | <10ms | **100-500x** |

## Solution

Replace the brute force implementation with **usearch** (USearch Rust bindings), providing:

1. **True HNSW graph structure** with hierarchical layers
2. **O(log n) search complexity** via greedy graph traversal
3. **Configurable parameters** (M, ef_construction, ef_search already defined)
4. **Thread-safe operations** (maintained via RwLock wrapper)

## Files to Modify

1. **`crates/context-graph-storage/Cargo.toml`** - Add `usearch = "2"` dependency
2. **`crates/context-graph-storage/src/teleological/indexes/hnsw_impl.rs`** - Replace implementation

## Acceptance Criteria

- [ ] All 20 existing tests pass unchanged
- [ ] Search latency <10ms at 1M vectors (384D)
- [ ] Insert throughput >10,000 vectors/second
- [ ] Memory per vector <2KB (including graph overhead)
- [ ] No changes to `EmbedderIndexOps` trait interface
- [ ] cargo clippy passes with no warnings
