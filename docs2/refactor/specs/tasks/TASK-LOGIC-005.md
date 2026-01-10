# TASK-LOGIC-005: Single Embedder Search

## Metadata

| Field | Value |
|-------|-------|
| **ID** | TASK-LOGIC-005 |
| **Status** | `done` |
| **Layer** | Logic |
| **Depends On** | TASK-LOGIC-004 (TeleologicalComparator) |
| **Completed** | 2026-01-09 |
| **Verified** | YES (47 tests pass, 1 integration test) |

---

## Summary

**IMPLEMENTED** - Single embedder HNSW search across 12 indexes.

**Files Created:**
- `crates/context-graph-storage/src/teleological/search/mod.rs`
- `crates/context-graph-storage/src/teleological/search/single.rs`
- `crates/context-graph-storage/src/teleological/search/result.rs`
- `crates/context-graph-storage/src/teleological/search/error.rs`

**Tests:** 47 unit tests + 1 integration test pass

---

## API Reference

### SingleEmbedderSearch

```rust
// File: crates/context-graph-storage/src/teleological/search/single.rs

pub struct SingleEmbedderSearch {
    registry: Arc<EmbedderIndexRegistry>,
    config: SingleEmbedderSearchConfig,
}

impl SingleEmbedderSearch {
    pub fn new(registry: Arc<EmbedderIndexRegistry>) -> Self;
    pub fn with_config(registry: Arc<EmbedderIndexRegistry>, config: SingleEmbedderSearchConfig) -> Self;

    /// Search a single embedder index.
    /// Returns SearchError for: E6/E12/E13, wrong dimension, NaN/Inf, empty query.
    pub fn search(
        &self,
        embedder: EmbedderIndex,
        query: &[f32],
        k: usize,
        threshold: Option<f32>,
    ) -> SearchResult<SingleEmbedderSearchResults>;

    pub fn search_default(&self, embedder: EmbedderIndex, query: &[f32]) -> SearchResult<SingleEmbedderSearchResults>;
    pub fn search_ids_above_threshold(&self, embedder: EmbedderIndex, query: &[f32], k: usize, min_similarity: f32) -> SearchResult<Vec<(Uuid, f32)>>;
}
```

### Result Types

```rust
// File: crates/context-graph-storage/src/teleological/search/result.rs

pub struct EmbedderSearchHit {
    pub id: Uuid,
    pub distance: f32,       // HNSW distance (lower = more similar)
    pub similarity: f32,     // [0.0, 1.0] = 1.0 - distance
    pub embedder: EmbedderIndex,
}

pub struct SingleEmbedderSearchResults {
    pub hits: Vec<EmbedderSearchHit>,  // Sorted by similarity descending
    pub embedder: EmbedderIndex,
    pub k: usize,
    pub threshold: Option<f32>,
    pub latency_us: u64,
}
```

### Error Types

```rust
// File: crates/context-graph-storage/src/teleological/search/error.rs

pub enum SearchError {
    DimensionMismatch { embedder, expected, actual },
    UnsupportedEmbedder { embedder },  // E6, E12, E13
    EmptyQuery { embedder },
    InvalidVector { embedder, message },  // NaN/Inf
    Index(IndexError),
    NotFound { id },
    Store(String),
}

pub type SearchResult<T> = Result<T, SearchError>;
```

---

## Supported Embedders (12 HNSW)

| Embedder | Dimension | Use Case |
|----------|-----------|----------|
| E1Semantic | 1024D | General meaning |
| E1Matryoshka128 | 128D | Stage 2 fast filter |
| E2TemporalRecent | 512D | Recency |
| E3TemporalPeriodic | 512D | Cycles |
| E4TemporalPositional | 512D | Who/what |
| E5Causal | 768D | Why/because |
| E7Code | 1536D | Code/tech |
| E8Graph | 384D | Sentiment |
| E9HDC | 1024D | Structure |
| E10Multimodal | 768D | Intent |
| E11Entity | 384D | Multi-modal |
| PurposeVector | 13D | Teleological |

**NOT supported (different algorithms):**
- E6Sparse → Inverted index
- E12LateInteraction → MaxSim token-level
- E13Splade → Inverted index

---

## Test Verification

```bash
# Run all search tests (47 pass)
cargo test -p context-graph-storage search

# Run with output
cargo test -p context-graph-storage search -- --nocapture

# Run integration test
cargo test -p context-graph-storage --test full_integration_real_data search
```

### Test Coverage

| Category | Tests | Status |
|----------|-------|--------|
| FAIL FAST validation | 8 | PASS |
| Empty index / k=0 | 3 | PASS |
| Search with data | 7 | PASS |
| All 12 HNSW embedders | 1 | PASS |
| Threshold filtering | 3 | PASS |
| Full state verification | 1 | PASS |
| Result/Error types | 20+ | PASS |

---

## Integration with Pipeline

```rust
use context_graph_storage::teleological::search::{
    SingleEmbedderSearch, SingleEmbedderSearchConfig,
};
use context_graph_storage::teleological::indexes::{
    EmbedderIndex, EmbedderIndexRegistry,
};
use std::sync::Arc;

// Create registry and search
let registry = Arc::new(EmbedderIndexRegistry::new());
let search = SingleEmbedderSearch::new(registry);

// Stage 2: Fast filter with E1Matryoshka128
let query_128d = vec![0.5f32; 128];
let candidates = search.search(
    EmbedderIndex::E1Matryoshka128,
    &query_128d,
    1000,
    Some(0.5),  // threshold
)?;

// Stage 3: Full E1Semantic reranking
let query_1024d = vec![0.5f32; 1024];
let reranked = search.search(
    EmbedderIndex::E1Semantic,
    &query_1024d,
    100,
    Some(0.7),
)?;
```

---

## Commit

```
feat(TASK-LOGIC-005): implement single embedder HNSW search

- Add SingleEmbedderSearch for 12 HNSW-capable indexes
- FAIL FAST validation: dimension, NaN/Inf, embedder type
- Distance-to-similarity conversion with clamping
- Threshold filtering and sorted results
- 47 tests + 1 integration test

Supports: Stage 2 (E1Matryoshka128) and Stage 3 (E1Semantic) of 5-stage pipeline
NOT supported: E6/E12/E13 (require different algorithms)

Refs: ARCH-04 (entry-point discovery)
```
