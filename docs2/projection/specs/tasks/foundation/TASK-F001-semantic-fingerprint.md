# Task: TASK-F001 - Implement SemanticFingerprint Struct (13 Embedders)

## Metadata
- **ID**: TASK-F001
- **Layer**: Foundation
- **Priority**: P0 (Critical Path)
- **Estimated Effort**: M (Medium)
- **Dependencies**: None (Foundation start)
- **Traces To**: TS-101, FR-101, FR-102, FR-103, FR-104
- **Status**: VERIFIED COMPLETE
- **Last Audit**: 2026-01-05
- **Verified By**: sherlock-holmes forensic agent

---

## IMPLEMENTATION STATUS: COMPLETE

**All 12 verification checks PASS.** Implementation verified by sherlock-holmes forensic agent.

| Verification Check | Status |
|---|---|
| E13 field exists (`pub e13_splade: SparseVector`) | PASS |
| `NUM_EMBEDDERS = 13` | PASS |
| `E13_SPLADE_VOCAB = 30_522` | PASS |
| `get_embedding(0-12)` returns `Some(...)` | PASS |
| `get_embedding(13)` returns `None` | PASS |
| `storage_size()` includes E13 | PASS |
| `validate()` checks E13 bounds | PASS |
| `embedding_name(12)` returns `"E13_SPLADE"` | PASS |
| `embedding_dim(12)` returns `E13_SPLADE_VOCAB` | PASS |
| `PartialEq` compares `e13_splade` | PASS |
| `e13_splade_nnz()` method exists | PASS |
| `zeroed()` initializes E13 as empty | PASS |
| `mod.rs` exports all constants | PASS |
| All 87 tests pass | PASS |
| Clippy clean | PASS |

---

## Objective

Implement `SemanticFingerprint` struct that stores ALL 13 embedding vectors without fusion. This is the foundational data structure for the Teleological Vector Architecture.

**Key Principle**: NO FUSION. Each embedding space is preserved independently for:
1. Per-space similarity search (13x HNSW indexes)
2. Per-space Johari quadrant classification
3. Per-space teleological alignment computation
4. Full semantic information preservation
5. 5-stage retrieval pipeline support

---

## File Locations (VERIFIED)

| File | Purpose | Status |
|------|---------|--------|
| `crates/context-graph-core/src/types/fingerprint/semantic.rs` | SemanticFingerprint impl (1029 lines) | COMPLETE |
| `crates/context-graph-core/src/types/fingerprint/mod.rs` | Module exports (63 lines) | COMPLETE |
| `crates/context-graph-core/src/types/fingerprint/sparse.rs` | SparseVector (522 lines) | COMPLETE |

---

## Implementation Details

### Constants (semantic.rs:44-96)

```rust
pub const E1_DIM: usize = 1024;
pub const E2_DIM: usize = 512;
pub const E3_DIM: usize = 512;
pub const E4_DIM: usize = 512;
pub const E5_DIM: usize = 768;
pub const E6_SPARSE_VOCAB: usize = 30_522;
pub const E7_DIM: usize = 256;
pub const E8_DIM: usize = 384;
pub const E9_DIM: usize = 10_000;
pub const E10_DIM: usize = 768;
pub const E11_DIM: usize = 384;
pub const E12_TOKEN_DIM: usize = 128;
pub const E13_SPLADE_VOCAB: usize = 30_522;
pub const NUM_EMBEDDERS: usize = 13;
pub const TOTAL_DENSE_DIMS: usize = 15120;
```

### SemanticFingerprint Struct (semantic.rs:162-230)

```rust
pub struct SemanticFingerprint {
    pub e1_semantic: Vec<f32>,           // 1024D
    pub e2_temporal_recent: Vec<f32>,    // 512D
    pub e3_temporal_periodic: Vec<f32>,  // 512D
    pub e4_temporal_positional: Vec<f32>,// 512D
    pub e5_causal: Vec<f32>,             // 768D
    pub e6_sparse: SparseVector,         // ~1500/30522 sparse
    pub e7_code: Vec<f32>,               // 256D
    pub e8_graph: Vec<f32>,              // 384D
    pub e9_hdc: Vec<f32>,                // 10000D
    pub e10_multimodal: Vec<f32>,        // 768D
    pub e11_entity: Vec<f32>,            // 384D
    pub e12_late_interaction: Vec<Vec<f32>>, // 128D/token
    pub e13_splade: SparseVector,        // 30522 vocab sparse
}
```

### Key Methods

| Method | Location | Purpose |
|--------|----------|---------|
| `zeroed()` | semantic.rs:249-265 | Create zero-initialized fingerprint |
| `get_embedding(idx)` | semantic.rs:298-315 | Access embedding by index 0-12 |
| `storage_size()` | semantic.rs:341-368 | Calculate heap memory usage |
| `validate()` | semantic.rs:393-533 | Validate all dimensions and bounds |
| `token_count()` | semantic.rs:551-553 | E12 token count |
| `e13_splade_nnz()` | semantic.rs:572-575 | E13 non-zero count |
| `embedding_name(idx)` | semantic.rs:582-599 | Get name by index |
| `embedding_dim(idx)` | semantic.rs:608-625 | Get dimension by index |

---

## Verification Commands

```bash
# 1. Run all fingerprint tests
cargo test -p context-graph-core fingerprint -- --nocapture

# 2. Clippy (zero warnings required)
cargo clippy -p context-graph-core -- -D warnings

# 3. Count embedding fields (should output 13)
grep -cE "^\s+pub e[0-9]+_" crates/context-graph-core/src/types/fingerprint/semantic.rs

# 4. Verify NUM_EMBEDDERS constant
grep "NUM_EMBEDDERS" crates/context-graph-core/src/types/fingerprint/semantic.rs

# 5. Verify E13_SPLADE_VOCAB constant
grep "E13_SPLADE_VOCAB" crates/context-graph-core/src/types/fingerprint/semantic.rs
```

---

## Full State Verification Protocol

After any modifications, you MUST perform Full State Verification:

### 1. Define Source of Truth

The source of truth is the `SemanticFingerprint` struct in:
```
crates/context-graph-core/src/types/fingerprint/semantic.rs
```

### 2. Execute & Inspect

```bash
# Build and run tests
cargo test -p context-graph-core fingerprint -- --nocapture

# Verify the struct has 13 fields
grep -c "pub e[0-9]" crates/context-graph-core/src/types/fingerprint/semantic.rs
# Expected output: 13

# Verify NUM_EMBEDDERS constant
grep "NUM_EMBEDDERS" crates/context-graph-core/src/types/fingerprint/*.rs
# Expected: pub const NUM_EMBEDDERS: usize = 13;
```

### 3. Boundary & Edge Case Audit

You MUST test these 3 edge cases and print before/after state:

#### Edge Case 1: Empty E13
```rust
let fp = SemanticFingerprint::zeroed();
println!("BEFORE: e13_splade.nnz() = {}", fp.e13_splade.nnz());
// Expected: 0
```

#### Edge Case 2: Maximum Valid E13 Index
```rust
let mut fp = SemanticFingerprint::zeroed();
fp.e13_splade = SparseVector::new(vec![30521], vec![1.0]).unwrap();
println!("AFTER: e13_splade max index = {}", fp.e13_splade.indices[0]);
// Expected: 30521 (valid, within vocab)
assert!(fp.validate().is_ok());
```

#### Edge Case 3: Out-of-Bounds Index (Should Fail)
```rust
// SparseVector::new validates indices, so this tests the validation
let mut fp = SemanticFingerprint::zeroed();
// Manually create invalid sparse vector (bypass validation for testing)
fp.e13_splade.indices = vec![30522];
fp.e13_splade.values = vec![1.0];
let result = fp.validate();
println!("OUT OF BOUNDS: {:?}", result);
// Expected: Err("E13 splade index 30522 exceeds vocabulary size 30522")
assert!(result.is_err());
```

### 4. Evidence of Success Log

```
=== TASK-F001 VERIFICATION LOG ===
Timestamp: 2026-01-05

1. NUM_EMBEDDERS = 13 ✓
2. E13_SPLADE_VOCAB = 30522 ✓
3. get_embedding(12) returns Sparse ✓
4. get_embedding(13) returns None ✓
5. storage_size includes E13 ✓
6. validate() checks E13 bounds ✓
7. Serialization roundtrip preserves E13 ✓
8. All 87 tests pass ✓
9. Zero clippy warnings ✓

Edge Cases:
- Empty E13: nnz=0 ✓
- Max index 30521: validates ✓
- Index 30522: rejects ✓

VERIFICATION COMPLETE
```

---

## Test Requirements

All tests are implemented in `semantic.rs` lines 657-1028. Key tests:

| Test | Line | Purpose |
|------|------|---------|
| `test_semantic_fingerprint_zeroed` | 662 | Verifies all fields zero-init |
| `test_semantic_fingerprint_dimensions` | 683 | Verifies dimensions match constants |
| `test_semantic_fingerprint_get_embedding` | 708 | Verifies 0-12 accessible, 13+ None |
| `test_semantic_fingerprint_get_embedding_types` | 726 | Verifies correct types (Dense/Sparse/TokenLevel) |
| `test_semantic_fingerprint_storage_size_zeroed` | 765 | Verifies base storage calculation |
| `test_semantic_fingerprint_serialization_roundtrip` | 836 | Verifies serde works with all fields |
| `test_semantic_fingerprint_validate` | 890 | Verifies validation passes for valid data |
| `test_embedding_name` | 953 | Verifies name mapping |
| `test_embedding_dim` | 974 | Verifies dimension mapping |
| `test_dimension_constants` | 986 | Verifies all constant values |
| `test_e13_splade_nnz` | 1018 | Verifies E13 nnz method |

---

## Constraints

- **NO `unwrap()` in production code** - use `expect()` with context or propagate errors
- **NO mock data in tests** - all tests use real dimensions and real data
- **NO backwards compatibility hacks** - fail fast if dimensions mismatch
- **NO hardcoded secrets** - use environment variables (SEC-07)
- **Vec<f32> for dynamic safety** - runtime validation via `validate()`
- **All f32 values** - GPU compatibility requirement
- **SparseVector indices are u16** - max vocab 30522 fits in u16

---

## SPLADE v3 Model Installation (OPTIONAL - for embedding generation)

The SemanticFingerprint struct is data-only. Model installation is needed for TASK-F007 (EmbeddingProvider).

### Model Location
```
./models/splade-v3/
├── config.json
├── model.safetensors
├── tokenizer.json
├── tokenizer_config.json
├── special_tokens_map.json
└── vocab.txt
```

### Installation Commands

```bash
# 1. Create models directory
mkdir -p ./models/splade-v3

# 2. Set HuggingFace token as environment variable (NEVER hardcode)
export HF_TOKEN="${HF_TOKEN}"

# 3. Install huggingface-cli if not present
pip install huggingface_hub

# 4. Login to HuggingFace
huggingface-cli login --token "$HF_TOKEN"

# 5. Download SPLADE v3 model
huggingface-cli download naver/splade-v3 --local-dir ./models/splade-v3

# 6. Verify download
ls -la ./models/splade-v3/
```

---

## Related Tasks

| Task | Dependency | Status |
|------|------------|--------|
| TASK-F002 | Depends on F001 | Ready to start |
| TASK-F003 | Depends on F001 | Ready to start |
| TASK-F005 | Depends on F001 | Ready to start |
| TASK-F007 | Depends on F001 | Ready to start |

---

## References

- `constitution.yaml` Section `embeddings.models.E13_SPLADE`
- `constitution.yaml` Section `embeddings.paradigm`: "NO FUSION - Store all 13 embeddings"
- PRD Section 3: "13-MODEL EMBEDDING -> TELEOLOGICAL FINGERPRINT"
- PRD Section 18.1: "5-STAGE OPTIMIZED RETRIEVAL" (Stage 1 uses E13 SPLADE)
- `/home/cabdru/steveprog/docs3/learntheory.md`: UTL theory `L = f((ΔS × ΔC) · wₑ · cos φ)`

---

## sherlock-holmes Verification (MANDATORY)

After ANY modifications, spawn sherlock-holmes agent:

```
Task(
  "Verify TASK-F001 complete",
  "Forensically verify SemanticFingerprint has 13 embedders (E1-E13).
   Check: 1) e13_splade field exists, 2) NUM_EMBEDDERS=13,
   3) get_embedding(0-12) all return Some, 4) get_embedding(13) returns None,
   5) All tests pass, 6) Zero clippy warnings.
   Report any discrepancies with exact file:line references.",
  "sherlock-holmes"
)
```

The sherlock-holmes agent will:
1. Assume ALL CODE IS GUILTY until proven innocent
2. Verify every acceptance criterion
3. Check for regressions
4. Identify any remaining issues
5. Provide forensic evidence of completion

**DO NOT mark this task complete until sherlock-holmes verification passes.**

---

## Acceptance Criteria Checklist

- [x] `E13_SPLADE_VOCAB` constant defined as 30_522
- [x] `NUM_EMBEDDERS` constant updated to 13
- [x] `e13_splade: SparseVector` field added to SemanticFingerprint
- [x] `zeroed()` initializes E13 with `SparseVector::empty()`
- [x] `get_embedding(12)` returns `Some(EmbeddingSlice::Sparse(&self.e13_splade))`
- [x] `get_embedding(13)` returns `None` (out of bounds)
- [x] `storage_size()` includes E13 memory
- [x] `validate()` validates E13 indices < E13_SPLADE_VOCAB
- [x] `embedding_name(12)` returns `Some("E13_SPLADE")`
- [x] `embedding_dim(12)` returns `Some(E13_SPLADE_VOCAB)`
- [x] `PartialEq` compares E13
- [x] `e13_splade_nnz()` method implemented
- [x] All 13 embedders accessible via index 0-12
- [x] Serialization roundtrip includes E13
- [x] Zero clippy warnings
- [x] All 87 tests pass with real data (NO MOCKS)
