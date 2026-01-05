# TASK-L001: Multi-Embedding Query Executor

```yaml
metadata:
  id: "TASK-L001"
  title: "Multi-Embedding Query Executor"
  layer: "logic"
  priority: "P0"
  created: "2026-01-04"
  updated: "2026-01-05"
  status: "COMPLETE"
  verified: "2026-01-05 - 71 tests pass"
  dependencies:
    - "TASK-F001"  # SemanticFingerprint struct (COMPLETE)
    - "TASK-F005"  # HNSW indexes (COMPLETE)
    - "TASK-F007"  # MultiArrayEmbeddingProvider trait (COMPLETE)
```

## Status: ✅ COMPLETE

**Implementation Location**: `crates/context-graph-core/src/retrieval/`

**Verification**:
```bash
cargo test -p context-graph-core retrieval -- --nocapture
# Result: 71 passed; 0 failed
```

---

## Summary

Implements a query executor that searches across all 13 embedding spaces in parallel, aggregating results with RRF (Reciprocal Rank Fusion), supporting the 5-stage retrieval pipeline with <60ms latency target at 1M memories.

---

## Files (ALL EXIST)

| File | Location | Status |
|------|----------|--------|
| mod.rs | `src/retrieval/mod.rs` | ✅ Module root with exports |
| query.rs | `src/retrieval/query.rs` | ✅ `MultiEmbeddingQuery`, `PipelineStageConfig`, `EmbeddingSpaceMask` |
| aggregation.rs | `src/retrieval/aggregation.rs` | ✅ `AggregationStrategy` with RRF |
| result.rs | `src/retrieval/result.rs` | ✅ `SpaceSearchResult`, `AggregatedMatch`, `PipelineStageTiming` |
| executor.rs | `src/retrieval/executor.rs` | ✅ `MultiEmbeddingQueryExecutor` trait, `SpaceInfo`, `IndexType` |
| in_memory_executor.rs | `src/retrieval/in_memory_executor.rs` | ✅ `InMemoryMultiEmbeddingExecutor` |
| tests.rs | `src/retrieval/tests.rs` | ✅ 71 tests with real data |

**Integration**: `lib.rs` contains `pub mod retrieval;`

---

## Public API

```rust
// All exports from crates/context-graph-core/src/retrieval/mod.rs
pub use aggregation::AggregationStrategy;
pub use executor::{IndexType, MultiEmbeddingQueryExecutor, SpaceInfo};
pub use in_memory_executor::InMemoryMultiEmbeddingExecutor;
pub use query::{EmbeddingSpaceMask, MultiEmbeddingQuery, PipelineStageConfig};
pub use result::{
    AggregatedMatch, MultiEmbeddingResult, PipelineStageTiming, ScoredMatch,
    SpaceContribution, SpaceSearchResult,
};
```

---

## Architecture (constitution.yaml)

### 5-Stage Pipeline

| Stage | Component | Target | Candidates |
|-------|-----------|--------|------------|
| 1 | SPLADE Sparse (E13) | <5ms | 1000 |
| 2 | Matryoshka 128D (E1[:128]) | <10ms | 200 |
| 3 | Full 13-space HNSW | <20ms | 100 |
| 4 | Teleological alignment | <10ms | 50 |
| 5 | Late Interaction (E12) | <15ms | Final |

**Total: <60ms @ 1M memories**

### 13 Embedding Spaces

```
Index 0:  E1_Semantic        (1024D) - Dense HNSW
Index 1:  E2_Temporal_Recent (512D)  - Dense HNSW
Index 2:  E3_Temporal_Periodic (512D) - Dense HNSW
Index 3:  E4_Temporal_Positional (512D) - Dense HNSW
Index 4:  E5_Causal          (768D)  - Dense HNSW
Index 5:  E6_Sparse          (var)   - Inverted
Index 6:  E7_Code            (256D)  - Dense HNSW
Index 7:  E8_Graph           (384D)  - Dense HNSW
Index 8:  E9_HDC             (10000D)- Dense HNSW
Index 9:  E10_Multimodal     (768D)  - Dense HNSW
Index 10: E11_Entity         (384D)  - Dense HNSW
Index 11: E12_Late_Interaction (128D per token) - Dense HNSW
Index 12: E13_SPLADE         (30522) - Inverted
```

### RRF Formula

```
RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1)
k = 60 (default)
```

---

## Usage

### Basic Query

```rust
use context_graph_core::retrieval::{
    MultiEmbeddingQuery, MultiEmbeddingQueryExecutor, EmbeddingSpaceMask,
    InMemoryMultiEmbeddingExecutor,
};
use context_graph_core::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider};

let store = InMemoryTeleologicalStore::new();
let provider = StubMultiArrayProvider::new();
let executor = InMemoryMultiEmbeddingExecutor::new(store, provider);

let query = MultiEmbeddingQuery {
    query_text: "How does memory consolidation work?".to_string(),
    active_spaces: EmbeddingSpaceMask::ALL,
    final_limit: 10,
    ..Default::default()
};

let result = executor.execute(query).await?;
assert!(result.within_latency_target()); // <60ms
```

### Pipeline Mode

```rust
let query = MultiEmbeddingQuery {
    query_text: "neural memory".to_string(),
    ..Default::default()
};

let result = executor.execute_pipeline(query).await?;
if let Some(timings) = result.stage_timings {
    println!("{}", timings.summary());
    // S1:35µs S2:2ms S3:10ms S4:70ns S5:943µs Total:13ms
}
```

### Space Masks

```rust
EmbeddingSpaceMask::ALL          // All 13 spaces (0x1FFF)
EmbeddingSpaceMask::ALL_DENSE    // 12 dense (0x0FFF)
EmbeddingSpaceMask::SEMANTIC_ONLY // E1 only (0x0001)
EmbeddingSpaceMask::HYBRID       // E1 + E13 (0x1001)
EmbeddingSpaceMask::CODE_FOCUSED // E1 + E7 + E13 (0x1041)
EmbeddingSpaceMask::TEXT_CORE    // E1 + E2 + E3 (0x0007)
EmbeddingSpaceMask::SPLADE_ONLY  // E13 only (0x1000)
```

---

## Tests Verified

```
[VERIFIED] EmbeddingSpaceMask::ALL has 13 active spaces
[VERIFIED] RRF aggregation formula is correct
[VERIFIED] PipelineStageConfig defaults match constitution.yaml
[VERIFIED] Full query flow: 10 results, 13 spaces searched, ~20ms total time
[VERIFIED] Pipeline execution returns stage timings: S1:35µs S2:2ms S3:10ms S4:70ns S5:943µs
[VERIFIED] Empty query text returns ValidationError
[VERIFIED] Zero active spaces returns ValidationError
[VERIFIED] SpaceSearchResult::success creates correct result
[VERIFIED] SpaceSearchResult::failure creates correct result
[VERIFIED] Sparse space (SPLADE) search executes correctly
[VERIFIED] execute_with_embeddings works correctly
[VERIFIED] warm_up succeeds for all spaces
... (71 total tests)
```

---

## Dependencies

### Internal (all exist)

- `crate::error::{CoreError, CoreResult}` - Error handling
- `crate::types::fingerprint::{SemanticFingerprint, PurposeVector, SparseVector, NUM_EMBEDDERS}`
- `crate::traits::{MultiArrayEmbeddingProvider, TeleologicalMemoryStore}`
- `crate::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider}`

### Crates (in Cargo.toml)

- `async-trait` - Async trait definitions
- `uuid` - Memory ID generation
- `tracing` - Logging

---

## Traceability

| Requirement | Source | Implementation | Test |
|-------------|--------|----------------|------|
| 13-space search | constitution.yaml | `execute()` + `EmbeddingSpaceMask::ALL` | `test_full_query_flow_with_real_data` |
| RRF fusion k=60 | constitution.yaml | `aggregate_rrf()` | `test_rrf_aggregation_formula` |
| <60ms latency | constitution.yaml | `within_latency_target()` | `test_multi_embedding_result_latency_check` |
| Graceful degradation | constitution.yaml | Continue on space failure | `test_space_search_result_failure` |
| SPLADE sparse | contextprd.md | `EmbeddingSpaceMask::SPLADE_ONLY` | `test_sparse_space_search` |
| Fail-fast validation | CLAUDE.md | `query.validate()` | `test_query_validation_*` |

---

## Future Work (Out of Scope)

| Task | Description |
|------|-------------|
| TASK-L003 | Goal alignment scoring |
| TASK-L005 | Index building (production HNSW) |
| TASK-L008 | Purpose-aware retrieval weighting |
| TBD | RocksDB production executor |

---

*Completed: 2026-01-05*
*Verified: 71 tests pass*
