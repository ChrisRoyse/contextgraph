# TASK-L007: Cross-Space Similarity Engine

```yaml
metadata:
  id: "TASK-L007"
  title: "Cross-Space Similarity Engine"
  layer: "logic"
  priority: "P0"
  estimated_hours: 8
  created: "2026-01-04"
  updated: "2026-01-05"
  status: "COMPLETED"
  commit: "pending" # Will be set after this commit
  dependencies:
    - "TASK-L001"  # Multi-Embedding Query Executor - COMPLETED (eb3a3e4)
    - "TASK-L002"  # Purpose Vector Computation - COMPLETED (5e57efe)
    - "TASK-L005"  # Per-Space HNSW Index Builder - COMPLETED (dcde2a4)
  blockers: []
```

## Implementation Summary

**STATUS: COMPLETED** - All 96 tests pass, implementation verified.

### Files Created

| File | Lines | Description |
|------|-------|-------------|
| `src/similarity/mod.rs` | 64 | Module exports, documentation |
| `src/similarity/config.rs` | ~280 | CrossSpaceConfig, WeightingStrategy, MissingSpaceHandling |
| `src/similarity/result.rs` | ~260 | CrossSpaceSimilarity struct with builder |
| `src/similarity/error.rs` | ~240 | SimilarityError enum with thiserror |
| `src/similarity/engine.rs` | ~145 | CrossSpaceSimilarityEngine trait |
| `src/similarity/default_engine.rs` | ~530 | DefaultCrossSpaceEngine implementation |
| `src/similarity/multi_utl.rs` | ~360 | MultiUtlParams, sigmoid, validation |
| `src/similarity/explanation.rs` | ~400 | SimilarityExplanation with per-space breakdown |
| `src/similarity/tests.rs` | ~480 | 20+ tests including edge cases |

**Total**: ~2,759 lines in `crates/context-graph-core/src/similarity/`

### Files Modified

| File | Change |
|------|--------|
| `src/lib.rs` | Added `pub mod similarity;` at line 35 |

---

## Verification Results

### Compilation
```
cargo check -p context-graph-core
✓ Compiles with 0 errors
⚠ 4 warnings (unrelated deprecated imports in other modules)
```

### Test Results
```
cargo test -p context-graph-core similarity -- --nocapture
test result: ok. 96 passed; 0 failed; 0 ignored
```

### Edge Cases Verified

1. **Empty Fingerprints** (`edge_case_empty_fingerprints`)
   - Input: Both fingerprints with 0 active embeddings
   - Output: `Err(SimilarityError::InsufficientSpaces { required: 1, found: 0 })`
   - Status: ✓ PASS

2. **Identical Vectors** (`edge_case_identical_vectors`)
   - Input: Fingerprint compared to itself
   - Output: `score=1.0`, `confidence=1.0`
   - Status: ✓ PASS

3. **RRF Single Item** (`edge_case_rrf_single_item`)
   - Input: Single item at rank 0, k=60
   - Output: `score = 1/(60+0+1) = 0.01639`
   - Status: ✓ PASS

### Performance Verified

| Metric | Required | Actual | Status |
|--------|----------|--------|--------|
| Pair similarity | <5ms | <1ms | ✓ |
| Batch 100 | <50ms | <5ms | ✓ |
| RRF fusion | <2ms/1000 | <0.5ms | ✓ |

---

## Architecture Reference

### Type Hierarchy (VERIFIED)

```
crates/context-graph-core/src/
├── types/fingerprint/
│   ├── semantic/
│   │   ├── constants.rs         # NUM_EMBEDDERS = 13
│   │   ├── fingerprint.rs       # SemanticFingerprint struct
│   │   └── slice.rs             # EmbeddingSlice enum
│   ├── teleological/types.rs    # TeleologicalFingerprint struct
│   └── purpose.rs               # PurposeVector struct
├── retrieval/
│   └── aggregation.rs           # AggregationStrategy::aggregate_rrf (REUSED)
└── similarity/                   # NEW MODULE (this task)
    └── *.rs
```

### Key Types (EXACT FIELD NAMES)

```rust
// TeleologicalFingerprint (src/types/fingerprint/teleological/types.rs)
pub struct TeleologicalFingerprint {
    pub id: Uuid,
    pub semantic: SemanticFingerprint,      // NOT "semantic_fingerprint"
    pub purpose_vector: PurposeVector,
    pub johari: JohariFingerprint,
    pub purpose_evolution: Vec<PurposeSnapshot>,
    pub theta_to_north_star: f32,
    pub content_hash: [u8; 32],
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub access_count: u64,
}

// PurposeVector (src/types/fingerprint/purpose.rs)
pub struct PurposeVector {
    pub alignments: [f32; NUM_EMBEDDERS],   // NOT "alignment" (singular)
    pub dominant_embedder: u8,
    pub coherence: f32,
    pub stability: f32,
}

// SemanticFingerprint method access
impl SemanticFingerprint {
    pub fn get_embedding(&self, index: usize) -> Option<EmbeddingSlice>;
}
```

### RRF Implementation (REUSED FROM L001)

```rust
// src/retrieval/aggregation.rs lines 138-149
// IMPORTED via: use crate::retrieval::AggregationStrategy;
pub fn aggregate_rrf(ranked_lists: &[(usize, Vec<Uuid>)], k: f32) -> HashMap<Uuid, f32>
```

---

## Public API

### Config
```rust
use context_graph_core::similarity::{
    CrossSpaceConfig,
    WeightingStrategy,
    MissingSpaceHandling,
};

let config = CrossSpaceConfig {
    weighting_strategy: WeightingStrategy::RRF { k: 60.0 },
    min_active_spaces: 1,
    use_purpose_weighting: false,
    missing_space_handling: MissingSpaceHandling::Skip,
    include_breakdown: false,
};
```

### Engine
```rust
use context_graph_core::similarity::{
    CrossSpaceSimilarityEngine,
    DefaultCrossSpaceEngine,
};

let engine = DefaultCrossSpaceEngine::new();

// Compute pair similarity
let result = engine.compute_similarity(&fp1, &fp2, &config).await?;

// Compute batch
let results = engine.compute_batch(&query, &candidates, &config).await?;

// Compute RRF from pre-ranked lists
let scores = engine.compute_rrf_from_ranks(&ranked_lists, 60.0);

// Compute Multi-UTL score
let utl = engine.compute_multi_utl(&params).await;

// Generate explanation
let explanation = engine.explain(&result);
```

### Result
```rust
use context_graph_core::similarity::CrossSpaceSimilarity;

// Access fields
let score = result.score;           // [0.0, 1.0]
let confidence = result.confidence; // [0.0, 1.0]
let active = result.active_count(); // 0-13
let breakdown = result.space_scores; // Option<[Option<f32>; 13]>
```

---

## Multi-UTL Formula

From constitution.yaml:
```
L_multi = sigmoid(2.0 · (Σᵢ τᵢλ_S·ΔSᵢ) · (Σⱼ τⱼλ_C·ΔCⱼ) · wₑ · cos φ)
```

Implementation:
```rust
use context_graph_core::similarity::{MultiUtlParams, sigmoid};

let params = MultiUtlParams::default()
    .with_semantic_deltas([0.1; 13])
    .with_coherence_deltas([0.1; 13])
    .with_tau_weights([1.0; 13])
    .with_lambda_s(1.0)
    .with_lambda_c(1.0)
    .with_w_e(1.0)
    .with_phi(0.0);

let score = engine.compute_multi_utl(&params).await;
```

---

## Traceability

| Requirement | Source | Implementation |
|-------------|--------|----------------|
| 13 embedding spaces | constitution.yaml | `[f32; NUM_EMBEDDERS]` where NUM_EMBEDDERS=13 |
| RRF fusion k=60 | constitution.yaml | Reuses `AggregationStrategy::aggregate_rrf` |
| Purpose weighting | constitution.yaml | `WeightingStrategy::PurposeAligned` |
| Multi-UTL formula | constitution.yaml | `compute_multi_utl()` with sigmoid |
| <5ms pair latency | constitution.yaml | Performance tests enforced |
| No fallbacks | constitution.yaml | All errors explicit via `SimilarityError` |

---

## Next Task: TASK-L008

The Cross-Space Similarity Engine is now available for TASK-L008 (5-Stage Teleological Retrieval Pipeline).

**Import path**:
```rust
use context_graph_core::similarity::{
    CrossSpaceSimilarityEngine,
    DefaultCrossSpaceEngine,
    CrossSpaceConfig,
    CrossSpaceSimilarity,
    WeightingStrategy,
    SimilarityError,
};
```

---

*Task completed: 2026-01-05*
*Verified: cargo check + cargo test = 0 errors, 96 tests pass*
*Implementation: ~2,759 lines across 9 files*
