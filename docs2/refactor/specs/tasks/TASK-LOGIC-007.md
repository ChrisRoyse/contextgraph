# TASK-LOGIC-007: Matrix Strategy Search

```xml
<task_spec id="TASK-LOGIC-007" version="3.0">
<metadata>
  <title>Matrix Strategy Search</title>
  <status>COMPLETED</status>
  <completed_date>2026-01-09</completed_date>
  <layer>logic</layer>
  <sequence>21</sequence>
  <implements>
    <requirement_ref>REQ-SEARCH-MATRIX-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="done">TASK-LOGIC-004</task_ref>
    <task_ref status="done">TASK-LOGIC-006</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <actual_days>0.5</actual_days>
</metadata>

<completion_summary>
## VERIFIED COMPLETE (2026-01-09)

**Test Results**: 30 tests pass
**Command**: `cargo test -p context-graph-storage matrix -- --nocapture`

### Implemented Types
| Type | File | Lines | Purpose |
|------|------|-------|---------|
| SearchMatrix | matrix.rs:58-215 | 158 | 13x13 weight matrix with predefined patterns |
| MatrixAnalysis | matrix.rs:218-228 | 11 | Optimization hints from matrix structure |
| CorrelationAnalysis | matrix.rs:232-237 | 6 | 13x13 Pearson correlations + patterns |
| CorrelationPattern | matrix.rs:240-250 | 11 | ConsensusHigh, TemporalSemanticAlign, etc. |
| MatrixSearchResults | matrix.rs:254-301 | 48 | Results with correlation metadata |
| MatrixStrategySearch | matrix.rs:305-453 | 149 | Main search orchestrator |
| MatrixSearchBuilder | matrix.rs:457-505 | 49 | Fluent builder API |

### Re-exports in mod.rs
```rust
pub use matrix::{
    SearchMatrix, MatrixAnalysis, CorrelationAnalysis,
    CorrelationPattern, MatrixSearchResults, MatrixStrategySearch,
    MatrixSearchBuilder, pearson_correlation_matched,
};
```

### Test Coverage by Category
| Category | Test Count | Status |
|----------|------------|--------|
| SearchMatrix constructors | 8 | PASS |
| Predefined matrices | 5 | PASS |
| Bounds checking (panic) | 2 | PASS |
| MatrixAnalysis | 4 | PASS |
| Pearson correlation | 3 | PASS |
| FAIL FAST validation | 2 | PASS |
| Weight application | 2 | PASS |
| Builder pattern | 2 | PASS |
| Verification logging | 2 | PASS |
| **TOTAL** | **30** | **PASS** |
</completion_summary>

<source_of_truth>
## Codebase Location (VERIFIED)
- **Primary file**: `crates/context-graph-storage/src/teleological/search/matrix.rs` (~520 lines)
- **Module export**: `crates/context-graph-storage/src/teleological/search/mod.rs`
- **Dependencies**: `multi.rs` (MultiEmbedderSearch), `error.rs` (SearchError)
- **EmbedderIndex**: `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs`
</source_of_truth>

<embedder_index_mapping>
## Correct Index Mapping (from EmbedderIndex::to_index())
| Index | Variant            | Dim   | Algorithm      | HNSW? |
|-------|---------------------|-------|----------------|-------|
| 0     | E1Semantic          | 1024  | HNSW           | YES   |
| 1     | E2TemporalRecent    | 512   | HNSW           | YES   |
| 2     | E3TemporalPeriodic  | 512   | HNSW           | YES   |
| 3     | E4TemporalPositional| 512   | HNSW           | YES   |
| 4     | E5Causal            | 768   | HNSW           | YES   |
| 5     | E6Sparse            | ~30K  | InvertedIndex  | NO    |
| 6     | E7Code              | 1536  | HNSW           | YES   |
| 7     | E8Graph             | 384   | HNSW           | YES   |
| 8     | E9HDC               | 1024  | HNSW           | YES   |
| 9     | E10Multimodal       | 768   | HNSW           | YES   |
| 10    | E11Entity           | 384   | HNSW           | YES   |
| 11    | E12LateInteraction  | 128/t | ColBERT MaxSim | NO    |
| 12    | E13Splade           | ~30K  | InvertedIndex  | NO    |

**Non-HNSW embedders (5, 11, 12)**: Matrix operations skip these by default in predefined matrices.
</embedder_index_mapping>

<predefined_matrices>
## Predefined SearchMatrix Patterns (IMPLEMENTED)

### semantic_focused()
- E1Semantic: 1.0 (diagonal)
- E5Causal: 0.3 (diagonal)
- E1-E5 cross: 0.2 (off-diagonal)
- Use case: Natural language queries

### code_heavy()
- E7Code: 1.0 (diagonal)
- E1Semantic: 0.3 (diagonal)
- E1-E7 cross: 0.2 (off-diagonal)
- Use case: Technical/code queries

### temporal_aware()
- E2,E3,E4: 0.8 each (diagonal)
- E1Semantic: 0.5 (diagonal)
- Temporal cross-correlations: 0.1 each
- Use case: Time-sensitive queries

### balanced()
- All 10 HNSW embedders: 0.1 each
- Skips E6, E12, E13 (non-HNSW)
- Use case: General purpose (Default)

### entity_focused()
- E11Entity: 1.0 (diagonal)
- E1Semantic: 0.4 (diagonal)
- E8Graph: 0.3 (diagonal)
- Use case: Entity relationship queries
</predefined_matrices>

<design_philosophy>
## FAIL FAST. NO FALLBACKS.

All errors are fatal. No recovery attempts. This ensures:
- Bugs are caught early in development
- Data integrity is preserved
- Clear error messages for debugging
- Matches existing multi.rs patterns exactly

**Panic conditions**:
- `matrix.get(i, j)` where i >= 13 or j >= 13
- `matrix.set(i, j, v)` where i >= 13 or j >= 13

**Error conditions** (return SearchError):
- Empty queries HashMap → `SearchError::Store("FAIL FAST: queries map is empty")`
- Query for E6Sparse/E12LateInteraction/E13Splade → `SearchError::UnsupportedEmbedder`
- Query dimension mismatch → `SearchError::DimensionMismatch`
</design_philosophy>

<verification_commands>
## Run These to Verify Implementation

```bash
# All matrix tests (30 tests)
cargo test -p context-graph-storage matrix -- --nocapture

# Specific verification tests
cargo test -p context-graph-storage test_verification_log -- --nocapture
cargo test -p context-graph-storage test_empty_queries_fails_fast -- --nocapture
cargo test -p context-graph-storage test_unsupported_embedder_in_queries_fails_fast -- --nocapture

# Ensure all builds
cargo check -p context-graph-storage
```
</verification_commands>

<full_state_verification>
## Source of Truth
- `EmbedderIndexRegistry` holds all HNSW indexes - query it directly
- `MultiEmbedderSearchResults.per_embedder` contains raw hits before aggregation
- `SearchMatrix.weights[i][j]` defines the correlation weight

## Test Evidence (from test output)
```
=== MATRIX.RS VERIFICATION LOG ===

Type Verification:
  - SearchMatrix: 13x13 weight matrix
    - zeros(), identity(), uniform()
    - get(), set(), diagonal()
    - is_diagonal(), has_cross_correlations()
    - sparsity(), active_embedders()
  - Predefined matrices:
    - semantic_focused()
    - code_heavy()
    - temporal_aware()
    - balanced()
    - entity_focused()
  - MatrixAnalysis: optimization hints
  - CorrelationAnalysis: 13x13 Pearson correlations
  - CorrelationPattern: ConsensusHigh, TemporalSemanticAlign, etc.
  - MatrixSearchResults: hits + correlation + metadata
  - MatrixStrategySearch: wraps MultiEmbedderSearch
  - MatrixSearchBuilder: fluent API

Fail Fast Verification:
  - Matrix index bounds: PANIC on >= 13
  - Empty queries: SearchError::Store

VERIFICATION COMPLETE
```

## Edge Cases Tested
| Case | Before State | Action | After State | Evidence |
|------|--------------|--------|-------------|----------|
| empty_queries | queries.is_empty() == true | search() | Err(Store) | Error message contains "empty" |
| identity_matrix | identity has diagonal=1.0 | compare to multi | Same results | Score diff < 1e-6 |
| zero_weight_skipped | E1 weight = 0.0 | search with E1 query | E1 not in results | active_embedders excludes 0 |
| cross_correlation_boosts | E1-E7 cross = 0.5 | shared ID in both | Higher score | Geometric mean applied |
| unsupported_embedder | queries contains E6Sparse | search() | Err(UnsupportedEmbedder) | Match on error variant |
</full_state_verification>

<api_reference>
## Public API Summary

```rust
// === SEARCH MATRIX ===
impl SearchMatrix {
    pub fn zeros() -> Self;
    pub fn identity() -> Self;
    pub fn uniform() -> Self;
    pub fn get(&self, i: usize, j: usize) -> f32;  // PANICS if >= 13
    pub fn set(&mut self, i: usize, j: usize, weight: f32);  // PANICS if >= 13
    pub fn diagonal(&self, embedder: EmbedderIndex) -> f32;
    pub fn is_diagonal(&self) -> bool;
    pub fn has_cross_correlations(&self) -> bool;
    pub fn sparsity(&self) -> f32;
    pub fn active_embedders(&self) -> Vec<usize>;

    // Predefined patterns
    pub fn semantic_focused() -> Self;
    pub fn code_heavy() -> Self;
    pub fn temporal_aware() -> Self;
    pub fn balanced() -> Self;  // Default
    pub fn entity_focused() -> Self;
}

// === SEARCH ===
impl MatrixStrategySearch {
    pub fn new(registry: Arc<EmbedderIndexRegistry>) -> Self;

    pub fn search(
        &self,
        queries: HashMap<EmbedderIndex, Vec<f32>>,
        matrix: SearchMatrix,
        k: usize,
        threshold: Option<f32>,
    ) -> SearchResult<MatrixSearchResults>;
}

// === BUILDER ===
impl MatrixSearchBuilder {
    pub fn new(queries: HashMap<EmbedderIndex, Vec<f32>>) -> Self;
    pub fn matrix(self, matrix: SearchMatrix) -> Self;
    pub fn k(self, k: usize) -> Self;
    pub fn threshold(self, threshold: f32) -> Self;
    pub fn execute(self, search: &MatrixStrategySearch) -> SearchResult<MatrixSearchResults>;
}

// === RESULTS ===
impl MatrixSearchResults {
    pub fn is_empty(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn top(&self) -> Option<&AggregatedHit>;
    pub fn top_n(&self, n: usize) -> &[AggregatedHit];
    pub fn ids(&self) -> Vec<Uuid>;
}
```
</api_reference>

<usage_examples>
## Usage Examples

### Basic Search with Predefined Matrix
```rust
use context_graph_storage::teleological::search::{
    MatrixStrategySearch, SearchMatrix,
};
use context_graph_storage::teleological::indexes::{EmbedderIndex, EmbedderIndexRegistry};
use std::collections::HashMap;
use std::sync::Arc;

let registry = Arc::new(EmbedderIndexRegistry::new());
let search = MatrixStrategySearch::new(Arc::clone(&registry));

let mut queries = HashMap::new();
queries.insert(EmbedderIndex::E1Semantic, vec![0.5f32; 1024]);
queries.insert(EmbedderIndex::E7Code, vec![0.5f32; 1536]);

// Use code-focused matrix
let results = search.search(
    queries,
    SearchMatrix::code_heavy(),
    10,
    None,
)?;

println!("Found {} results", results.len());
for hit in results.top_n(5) {
    println!("  ID={} score={:.4}", hit.id, hit.aggregated_score);
}
```

### Builder Pattern
```rust
let results = MatrixSearchBuilder::new(queries)
    .matrix(SearchMatrix::semantic_focused())
    .k(20)
    .threshold(0.5)
    .execute(&search)?;
```

### Custom Matrix
```rust
let mut custom = SearchMatrix::zeros();
custom.set(0, 0, 1.0);   // E1Semantic full weight
custom.set(6, 6, 0.8);   // E7Code high weight
custom.set(0, 6, 0.3);   // E1-E7 cross-correlation
custom.set(6, 0, 0.3);   // Symmetric

let results = search.search(queries, custom, 10, None)?;
```

### Analyzing Matrix Properties
```rust
let matrix = SearchMatrix::temporal_aware();

println!("Is diagonal: {}", matrix.is_diagonal());
println!("Has cross-correlations: {}", matrix.has_cross_correlations());
println!("Sparsity: {:.4}", matrix.sparsity());
println!("Active embedders: {:?}", matrix.active_embedders());
```
</usage_examples>

<dependencies>
## Dependencies (from Cargo.toml)

Already present in workspace - no new dependencies added:
- `rayon = "1.10"` (for parallel search in multi.rs)
- `uuid = "1.6"` (for hit IDs)
- Standard library collections
</dependencies>

<next_steps>
## Downstream Tasks That Can Now Proceed

| Task | Status | Dependency Satisfied |
|------|--------|---------------------|
| TASK-LOGIC-008 (5-Stage Pipeline) | Ready | LOGIC-005, LOGIC-006, LOGIC-007 ✓ |
| TASK-LOGIC-011 (RRF Fusion) | Ready | Can build on matrix aggregation |
| TASK-LOGIC-012 (Entry-Point Selection) | Ready | Uses matrix weights for routing |

## Integration Notes for TASK-LOGIC-008
- MatrixStrategySearch provides cross-embedder correlation analysis
- Use `SearchMatrix::balanced()` for Stage 3 (RRF across 13 spaces)
- Use `SearchMatrix::semantic_focused()` for Stage 2 (Matryoshka ANN)
- CorrelationAnalysis.coherence can inform result quality scoring
</next_steps>
</task_spec>
```
