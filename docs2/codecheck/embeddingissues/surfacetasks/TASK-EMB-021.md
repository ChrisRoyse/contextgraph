# TASK-EMB-021: Integrate ProjectionMatrix into SparseModel

<task_spec id="TASK-EMB-021" version="2.0">

## Metadata

| Field | Value |
|-------|-------|
| **Task ID** | TASK-EMB-021 |
| **Title** | Integrate Learned Projection into SparseModel |
| **Status** | COMPLETE |
| **Layer** | surface |
| **Sequence** | 21 |
| **Implements** | REQ-EMB-001 (Learned Sparse Projection Matrix) |
| **Depends On** | TASK-EMB-012 (COMPLETE), TASK-EMB-008 (COMPLETE) |
| **Estimated Complexity** | medium |
| **Created** | 2026-01-06 |
| **Updated** | 2026-01-06 |
| **Constitution Reference** | v4.0.0 |

---

## CRITICAL: Current Codebase State

**READ THIS SECTION FIRST. This is the actual state of the code, verified 2026-01-06.**

### What Already Exists (DO NOT RECREATE)

1. **`projection.rs` is FULLY IMPLEMENTED** - 1267 lines with:
   - `ProjectionMatrix::load(&Path)` - Loads from SafeTensors, validates shape [30522, 1536]
   - `ProjectionMatrix::project(&SparseVector)` - GPU matmul with L2 normalization
   - `ProjectionMatrix::project_batch(&[SparseVector])` - Batched projection
   - `ProjectionMatrix::checksum()` - Returns SHA256 checksum as `&[u8; 32]`
   - `ProjectionError` enum with 5 variants (EMB-E001, E004, E005, E006, E008)
   - Full CUDA validation - no CPU fallback allowed (AP-007 compliant)

2. **`mod.rs` already exports projection** (line 61):
   ```rust
   pub use projection::{ProjectionError, ProjectionMatrix, PROJECTION_TENSOR_NAME, PROJECTION_WEIGHT_FILE};
   ```

3. **`types.rs` has all constants** (lines 32-41):
   - `SPARSE_PROJECTED_DIMENSION = 1536`
   - `SPARSE_VOCAB_SIZE = 30522`
   - Compile-time assertion verifying 1536

4. **`SparseVector::to_dense_projected()` was REMOVED** - lines 151-166 in types.rs explain why

### What Currently PANICS (This is what you're fixing)

**File: `model.rs` lines 286-303**
```rust
pub async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
    // ...validation...

    // CRITICAL: to_dense_projected() has been removed (Constitution AP-007)
    // ProjectionMatrix integration is required (TASK-EMB-012)
    panic!(
        "[EMB-MIGRATION] SparseModel::embed() is temporarily unavailable.\n\
         The hash-based projection was removed (violated AP-007).\n\
         Waiting for ProjectionMatrix integration (TASK-EMB-012).\n\
         For sparse output, use embed_sparse() directly."
    );
}
```

---

## Exact Changes Required

### Change 1: Add projection to ModelState enum

**File:** `crates/context-graph-embeddings/src/models/pretrained/sparse/types.rs`

**Location:** Lines 197-211 (ModelState enum)

**Current Code:**
```rust
pub(crate) enum ModelState {
    /// Unloaded - no weights in memory.
    Unloaded,

    /// Loaded with candle model and tokenizer (GPU-accelerated).
    Loaded {
        /// BERT model weights on GPU (boxed to reduce enum size).
        weights: Box<BertWeights>,
        /// HuggingFace tokenizer for text encoding (boxed to reduce enum size).
        tokenizer: Box<Tokenizer>,
        /// MLM head weights for vocabulary projection.
        mlm_head: MlmHeadWeights,
    },
}
```

**New Code:**
```rust
use super::projection::ProjectionMatrix;

pub(crate) enum ModelState {
    /// Unloaded - no weights in memory.
    Unloaded,

    /// Loaded with candle model and tokenizer (GPU-accelerated).
    Loaded {
        /// BERT model weights on GPU (boxed to reduce enum size).
        weights: Box<BertWeights>,
        /// HuggingFace tokenizer for text encoding (boxed to reduce enum size).
        tokenizer: Box<Tokenizer>,
        /// MLM head weights for vocabulary projection.
        mlm_head: MlmHeadWeights,
        /// Learned projection matrix for sparse-to-dense conversion.
        /// CRITICAL: Uses neural projection, NOT hash modulo (AP-007).
        projection: Box<ProjectionMatrix>,
    },
}
```

### Change 2: Load projection in SparseModel::load()

**File:** `crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs`

**Location:** Lines 143-205 (load method)

**Add after line 182** (after `let mlm_head = load_mlm_head(...)`:
```rust
// Load projection matrix (REQUIRED - no fallback)
let projection = super::projection::ProjectionMatrix::load(&self.model_path)
    .map_err(|e| EmbeddingError::ModelLoadError {
        model_id: self.model_id,
        source: Box::new(std::io::Error::other(format!(
            "ProjectionMatrix load failed: {}",
            e
        ))),
    })?;

tracing::info!(
    "ProjectionMatrix loaded: shape [{}, {}], checksum {:02x}{:02x}{:02x}{:02x}...",
    super::types::SPARSE_VOCAB_SIZE,
    super::types::SPARSE_PROJECTED_DIMENSION,
    projection.checksum()[0],
    projection.checksum()[1],
    projection.checksum()[2],
    projection.checksum()[3]
);
```

**Modify line 198** (ModelState::Loaded construction):
```rust
*state = ModelState::Loaded {
    weights: Box::new(weights),
    tokenizer: Box::new(tokenizer),
    mlm_head,
    projection: Box::new(projection),  // ADD THIS LINE
};
```

### Change 3: Replace panic in embed() with real projection

**File:** `crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs`

**Location:** Lines 276-303 (embed method)

**Replace entire method with:**
```rust
/// Embed input to dense 1536D vector (for multi-array storage compatibility).
/// Per Constitution E6_Sparse: "~30K 5%active" projects to 1536D.
///
/// # Algorithm
/// 1. Validate input and check model loaded
/// 2. Get sparse vector via SPLADE (embed_sparse)
/// 3. Project using learned matrix: sparse @ weights^T
/// 4. L2 normalize output
/// 5. Return 1536D ModelEmbedding
///
/// # Output
/// - Dimension: 1536 (SPARSE_PROJECTED_DIMENSION)
/// - Normalization: L2 unit vector
/// - Format: Vec<f32> wrapped in ModelEmbedding
pub async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
    if !self.is_initialized() {
        return Err(EmbeddingError::NotInitialized {
            model_id: self.model_id(),
        });
    }

    self.validate_input(input)?;

    let start = std::time::Instant::now();

    // Step 1: Get sparse vector (30522D with ~5% active)
    let sparse = self.embed_sparse(input).await?;

    // Step 2: Get projection matrix from state
    let state = self
        .model_state
        .read()
        .map_err(|e| EmbeddingError::InternalError {
            message: format!("Failed to acquire read lock: {}", e),
        })?;

    let projection = match &*state {
        ModelState::Loaded { projection, .. } => projection,
        _ => {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }
    };

    // Step 3: Project using learned weights (NOT hash modulo!)
    let vector = projection.project(&sparse).map_err(|e| {
        tracing::error!("[EMB-E001] Projection failed: {}", e);
        EmbeddingError::InternalError {
            message: format!("Sparse projection failed: {}", e),
        }
    })?;

    // Step 4: Verify output dimension
    debug_assert_eq!(
        vector.len(),
        super::types::SPARSE_PROJECTED_DIMENSION,
        "Output dimension mismatch: expected {}, got {}",
        super::types::SPARSE_PROJECTED_DIMENSION,
        vector.len()
    );

    let latency_us = start.elapsed().as_micros() as u64;

    tracing::debug!(
        "SparseModel::embed completed: {}D -> {}D in {}us",
        sparse.nnz(),
        vector.len(),
        latency_us
    );

    Ok(ModelEmbedding::new(self.model_id, vector, latency_us))
}
```

### Change 4: Add ModelState import to model.rs

**File:** `crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs`

**Location:** Line 18 (imports)

**Current:**
```rust
use super::types::{ModelState, SparseVector};
```

**No change needed** - ModelState is already imported.

---

## Files to Modify (Exact List)

| File | Line Range | Change |
|------|------------|--------|
| `types.rs` | 197-211 | Add `projection: Box<ProjectionMatrix>` to ModelState::Loaded |
| `types.rs` | 8 (imports) | Add `use super::projection::ProjectionMatrix;` |
| `model.rs` | 182-189 | Add ProjectionMatrix::load() call after mlm_head load |
| `model.rs` | 198 | Add `projection: Box::new(projection)` to state construction |
| `model.rs` | 276-303 | Replace panic with real embed() implementation |

---

## Verification Commands

Run these commands after implementation:

```bash
cd /home/cabdru/contextgraph

# Step 1: Compile check
cargo check -p context-graph-embeddings --features cuda

# Step 2: Run sparse model tests
cargo test -p context-graph-embeddings sparse::model -- --nocapture

# Step 3: Verify no forbidden patterns
# MUST return NO matches:
grep -rn "to_dense_projected\|idx % " crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs

# Step 4: Verify panic is REMOVED
# MUST return NO matches:
grep -n "EMB-MIGRATION" crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs

# Step 5: Verify projection is loaded
# MUST return matches in load() method:
grep -n "ProjectionMatrix::load" crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs
```

---

## Full State Verification Requirements

### Source of Truth Verification

Before marking complete, verify:

1. **ModelState has projection field:**
   ```bash
   grep -A 15 "enum ModelState" crates/context-graph-embeddings/src/models/pretrained/sparse/types.rs | grep "projection"
   # MUST show: projection: Box<ProjectionMatrix>
   ```

2. **load() loads projection:**
   ```bash
   grep -B 2 -A 5 "ProjectionMatrix::load" crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs
   # MUST show load call with error mapping
   ```

3. **embed() uses projection.project():**
   ```bash
   grep -n "projection.project" crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs
   # MUST return at least 1 match
   ```

4. **No panic in embed():**
   ```bash
   grep -n "panic!" crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs
   # MUST return 0 matches in embed() method
   ```

### Edge Case Verification

The following edge cases are handled by ProjectionMatrix (already tested in projection.rs):

| Edge Case | Expected Behavior | Test Location |
|-----------|-------------------|---------------|
| Empty sparse vector | Returns `vec![0.0; 1536]` | projection.rs:324-328 |
| Max index (30521) | Normal projection | projection.rs:1169-1183 |
| Out-of-bounds index (>=30522) | Returns `DimensionMismatch` error | projection.rs:334-345 |
| Missing weight file | Returns `MatrixMissing` error | projection.rs:884-916 |
| No CUDA device | Returns `GpuError` | projection.rs:239-250 |

### Physical Output Verification

After full integration, manually verify:

```bash
# 1. Model loads without panic
cargo run --example sparse_embed --features cuda -- "test input"

# 2. Output is 1536 dimensions (not 768, not 30522)
# Check the ModelEmbedding.vector.len() in output

# 3. Related terms have high similarity
# Run similarity test between "machine learning" and "artificial intelligence"
# Expected: similarity > 0.7
```

---

## Anti-Patterns (Constitution AP-007)

| Pattern | Why Forbidden | Detection |
|---------|---------------|-----------|
| `idx % 1536` | Hash destroys semantics | grep for `% 1536` or `% SPARSE_PROJECTED` |
| `idx % projected_dim` | Same as above | grep for `% projected` |
| Fallback to hash | Silent degradation | grep for `to_dense_projected` |
| CPU fallback | Must use GPU | projection.rs already enforces |
| Skip checksum log | Can't verify weights | Must log checksum on load |

---

## Error Handling Requirements

**All errors must fail fast. No silent degradation.**

| Error Condition | Expected Behavior |
|-----------------|-------------------|
| Projection file missing | Return `EmbeddingError::ModelLoadError` |
| CUDA unavailable | Return `EmbeddingError::GpuError` |
| Wrong shape (not [30522, 1536]) | Return `EmbeddingError::ModelLoadError` |
| Projection not loaded | Return `EmbeddingError::NotInitialized` |
| GPU matmul failure | Return `EmbeddingError::InternalError` |

---

## Success Criteria Checklist

**TASK COMPLETED: 2026-01-06**

All criteria verified:

- [x] `cargo check -p context-graph-embeddings --features cuda` passes ✓
- [x] `cargo test -p context-graph-embeddings sparse::model` passes (24 tests) ✓
- [x] ModelState::Loaded has `projection: Box<ProjectionMatrix>` field (types.rs:214) ✓
- [x] SparseModel::load() calls `ProjectionMatrix::load()` and logs checksum (model.rs:186) ✓
- [x] SparseModel::embed() returns 1536D vector using `projection.project()` (model.rs:350) ✓
- [x] No panic! macro in SparseModel::embed() method ✓
- [x] No reference to `to_dense_projected()` anywhere in model.rs ✓
- [x] No reference to `idx %` anywhere in model.rs ✓
- [x] System errors if projection file missing (no fallback) - returns EMB-E006 ✓
- [x] Edge cases verified: empty vector, max index, GPU unavailable ✓

---

## Memory Key

Store completion status:
```
contextgraph/embedding-issues/task-emb-021-complete
```

</task_spec>
