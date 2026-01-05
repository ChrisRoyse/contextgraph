# Task: TASK-F001 - Implement SemanticFingerprint Struct

## Metadata
- **ID**: TASK-F001
- **Layer**: Foundation
- **Priority**: P0 (Critical Path)
- **Estimated Effort**: M (Medium)
- **Dependencies**: None (Foundation start)
- **Traces To**: TS-101, FR-101, FR-102, FR-103, FR-104
- **Status**: COMPLETE (2026-01-05)
- **Verified By**: sherlock-holmes agent

---

## IMPLEMENTATION COMPLETE

**Files Created:**
- `crates/context-graph-core/src/types/fingerprint/mod.rs`
- `crates/context-graph-core/src/types/fingerprint/sparse.rs`
- `crates/context-graph-core/src/types/fingerprint/semantic.rs`

**Files Modified:**
- `crates/context-graph-core/src/types/mod.rs` - Added fingerprint exports

**Test Results:** 36 tests passed, 0 failed
**Clippy Status:** Zero warnings

---

## Description

Implement `SemanticFingerprint` struct that stores ALL 12 embedding vectors without fusion. This is the foundational data structure for the Teleological Vector Architecture.

**Key Principle**: NO FUSION. Each embedding space is preserved independently for:
1. Per-space similarity search (12x HNSW indexes)
2. Per-space Johari quadrant classification
3. Per-space teleological alignment computation
4. Full semantic information preservation (~95KB vs 6KB fused with 67% info loss)

---

## Acceptance Criteria (ALL MET)

- [x] `SemanticFingerprint` struct defined with exactly 12 embedding fields
- [x] `SparseVector` struct for E6 sparse embeddings with validation (indices/values pairs)
- [x] Correct dimensions matching specification
- [x] `storage_size()` method returns accurate byte count
- [x] `zeroed()` constructor for initialization
- [x] `get_embedding(idx: usize)` accessor returns `Option<EmbeddingSlice>` for index 0-11
- [x] All types derive `Debug`, `Clone`, `Serialize`, `Deserialize`
- [x] NO fusion logic, NO gating, NO single-vector output
- [x] Unit tests with REAL dimension validation (no mocks)
- [x] Integration test proving serialization round-trip with bincode

---

## ACTUAL IMPLEMENTATION (Source of Truth)

### Dimension Constants (semantic.rs)
```rust
pub const E1_DIM: usize = 1024;      // e5-large-v2
pub const E2_DIM: usize = 512;       // Temporal-Recent
pub const E3_DIM: usize = 512;       // Temporal-Periodic
pub const E4_DIM: usize = 512;       // Temporal-Positional
pub const E5_DIM: usize = 768;       // Causal (Longformer)
pub const E6_SPARSE_VOCAB: usize = 30_522; // SPLADE vocabulary
pub const E7_DIM: usize = 256;       // CodeT5p
pub const E8_DIM: usize = 384;       // MiniLM (Graph)
pub const E9_DIM: usize = 10_000;    // HDC
pub const E10_DIM: usize = 768;      // CLIP
pub const E11_DIM: usize = 384;      // MiniLM (Entity)
pub const E12_TOKEN_DIM: usize = 128; // ColBERT per-token

pub const TOTAL_DENSE_DIMS: usize = 15120; // Sum of dense (excluding E6, E12)
```

### SemanticFingerprint Struct
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFingerprint {
    pub e1_semantic: Vec<f32>,           // 1024D
    pub e2_temporal_recent: Vec<f32>,    // 512D
    pub e3_temporal_periodic: Vec<f32>,  // 512D
    pub e4_temporal_positional: Vec<f32>,// 512D
    pub e5_causal: Vec<f32>,             // 768D
    pub e6_sparse: SparseVector,         // ~1500 active of 30522 vocab
    pub e7_code: Vec<f32>,               // 256D
    pub e8_graph: Vec<f32>,              // 384D
    pub e9_hdc: Vec<f32>,                // 10000D
    pub e10_multimodal: Vec<f32>,        // 768D
    pub e11_entity: Vec<f32>,            // 384D
    pub e12_late_interaction: Vec<Vec<f32>>, // 128D per token
}
```

**Design Note:** Uses `Vec<f32>` instead of fixed-size `Box<[f32; N]>` to:
1. Enable serde serialization for large embeddings (E9 has 10000 dims)
2. Avoid stack overflow with large arrays
3. Maintain flexibility for future dimension changes

Dimension validation is performed via `validate()` method.

### SparseVector Struct
```rust
pub const SPARSE_VOCAB_SIZE: usize = 30_522;
pub const MAX_SPARSE_ACTIVE: usize = 1_526; // ~5% sparsity

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SparseVector {
    pub indices: Vec<u16>,  // Sorted ascending, < SPARSE_VOCAB_SIZE
    pub values: Vec<f32>,   // Same length as indices
}
```

### Key Methods
```rust
impl SemanticFingerprint {
    pub fn zeroed() -> Self;                    // All zeros
    pub fn get_embedding(&self, idx: usize) -> Option<EmbeddingSlice<'_>>;
    pub fn storage_size(&self) -> usize;        // Total heap bytes
    pub fn validate(&self) -> Result<(), String>; // Dimension validation
    pub fn token_count(&self) -> usize;         // E12 token count
    pub fn embedding_name(idx: usize) -> Option<&'static str>;
    pub fn embedding_dim(idx: usize) -> Option<usize>;
}

impl SparseVector {
    pub fn new(indices: Vec<u16>, values: Vec<f32>) -> Result<Self, SparseVectorError>;
    pub fn empty() -> Self;
    pub fn nnz(&self) -> usize;                 // Non-zero count
    pub fn dot(&self, other: &Self) -> f32;     // Sparse dot product
    pub fn memory_size(&self) -> usize;         // Heap bytes
}
```

---

## Verification Commands

```bash
# 1. Compile check
cargo check -p context-graph-core

# 2. Run unit tests
cargo test -p context-graph-core fingerprint -- --nocapture

# 3. Clippy (zero warnings required)
cargo clippy -p context-graph-core -- -D warnings

# 4. Verify public exports
cargo doc -p context-graph-core --no-deps
```

---

## Verification Results (2026-01-05)

### Test Output
```
running 36 tests
test types::fingerprint::semantic::tests::test_dimension_constants ... ok
test types::fingerprint::semantic::tests::test_embedding_dim ... ok
test types::fingerprint::semantic::tests::test_embedding_name ... ok
test types::fingerprint::semantic::tests::test_semantic_fingerprint_default ... ok
test types::fingerprint::semantic::tests::test_semantic_fingerprint_dimensions ... ok
test types::fingerprint::semantic::tests::test_semantic_fingerprint_get_embedding ... ok
test types::fingerprint::semantic::tests::test_semantic_fingerprint_get_embedding_types ... ok
test types::fingerprint::semantic::tests::test_semantic_fingerprint_partial_eq ... ok
test types::fingerprint::semantic::tests::test_semantic_fingerprint_storage_size_* ... ok (4 tests)
test types::fingerprint::semantic::tests::test_semantic_fingerprint_serialization_roundtrip ... ok
test types::fingerprint::semantic::tests::test_semantic_fingerprint_token_count ... ok
test types::fingerprint::semantic::tests::test_semantic_fingerprint_validate ... ok
test types::fingerprint::semantic::tests::test_semantic_fingerprint_validate_dimension_errors ... ok
test types::fingerprint::semantic::tests::test_semantic_fingerprint_zeroed ... ok
test types::fingerprint::sparse::tests::* ... ok (20 tests)

test result: ok. 36 passed; 0 failed; 0 ignored
```

### Clippy Output
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
(zero warnings)
```

---

## Constraints Applied

- **NO `unwrap()` in production code** - uses `expect()` with context or propagates errors
- **NO mock data in tests** - all tests use real dimensions and real data
- **NO backwards compatibility hacks** - fail fast if dimensions mismatch
- **Vec<f32> for dynamic safety** - runtime validation via `validate()`
- **All f32 values** - GPU compatibility requirement
- **SparseVector indices are u16** - max vocab 30522 fits in u16

---

## Related Files

| Path | Purpose |
|------|---------|
| `crates/context-graph-core/src/types/fingerprint/mod.rs` | Module exports |
| `crates/context-graph-core/src/types/fingerprint/sparse.rs` | SparseVector impl |
| `crates/context-graph-core/src/types/fingerprint/semantic.rs` | SemanticFingerprint impl |
| `crates/context-graph-core/src/types/mod.rs` | Root type exports |
| `crates/context-graph-core/Cargo.toml` | Added bincode dependency |

---

## Notes

- This task establishes the foundational data structure
- TASK-F002 (TeleologicalFingerprint) depends on this being complete
- TASK-F003 (JohariFingerprint) can start in parallel after F001
- TASK-F006 (Remove Fusion Files) is independent and can run in parallel

Reference: constitution.yaml Section `embeddings.paradigm`: "NO FUSION - Store all 12 embeddings as TeleologicalFingerprint array"
