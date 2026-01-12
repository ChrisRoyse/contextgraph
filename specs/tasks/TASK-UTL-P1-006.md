# TASK-UTL-P1-006: Implement MaxSimTokenEntropy for E12 (LateInteraction/ColBERT)

**Priority:** P1
**Status:** ✅ COMPLETED (2026-01-12)
**Spec Reference:** SPEC-UTL-003
**Estimated Effort:** 3-4 hours
**Implements:** REQ-UTL-003-07, REQ-UTL-003-08
**Constitution Reference:** `utl.delta_methods.ΔS E12: "Token KNN"`

---

## Critical Constraints (MUST READ FIRST)

1. **NO BACKWARDS COMPATIBILITY** - System must work or fail fast with robust error logging
2. **NO MOCK DATA IN TESTS** - Use real data structures and verify actual outputs
3. **NO FALLBACKS OR WORKAROUNDS** - If something doesn't work, error out immediately
4. **FAIL FAST** - All errors are fatal with descriptive messages

---

## Current Codebase State (Audited 2026-01-12)

### Verified File Paths

| File | Status | Purpose |
|------|--------|---------|
| `crates/context-graph-utl/src/surprise/embedder_entropy/mod.rs` | EXISTS | `EmbedderEntropy` trait definition |
| `crates/context-graph-utl/src/surprise/embedder_entropy/factory.rs` | EXISTS | Factory routing - **MUST UPDATE** |
| `crates/context-graph-utl/src/surprise/embedder_entropy/transe.rs` | EXISTS | Reference implementation pattern |
| `crates/context-graph-utl/src/config/surprise.rs` | EXISTS | `SurpriseConfig` - **MUST ADD CONFIG FIELDS** |
| `crates/context-graph-utl/src/error.rs` | EXISTS | `UtlError`, `UtlResult` types |
| `crates/context-graph-storage/src/teleological/search/maxsim.rs` | EXISTS | Reference: `E12_TOKEN_DIM=128`, `cosine_similarity_128d` |
| `crates/context-graph-storage/src/teleological/indexes/hnsw_config/constants.rs` | EXISTS | `E12_TOKEN_DIM = 128` constant |
| `crates/context-graph-core/src/teleological/embedder.rs` | EXISTS | `Embedder::LateInteraction = 11` (index 11 in enum) |

### Current Factory Routing (Line 85-93 of factory.rs)

```rust
// E2-E4, E6, E8, E12: Default KNN-based entropy
Embedder::TemporalRecent
| Embedder::TemporalPeriodic
| Embedder::TemporalPositional
| Embedder::Sparse
| Embedder::Graph
| Embedder::LateInteraction => {  // <-- THIS MUST CHANGE
    Box::new(DefaultKnnEntropy::from_config(embedder, config))
}
```

### Existing Module Structure

```
crates/context-graph-utl/src/surprise/embedder_entropy/
├── asymmetric_knn.rs     (E5)
├── cross_modal.rs        (E10) - ~27KB, good reference
├── default_knn.rs        (fallback)
├── factory.rs            (routing)
├── gmm_mahalanobis.rs    (E1)
├── hamming_prototype.rs  (E9)
├── hybrid_gmm_knn.rs     (E7) - ~28KB, good reference
├── jaccard_active.rs     (E13)
├── mod.rs                (exports)
└── transe.rs             (E11) - ~29KB, BEST reference pattern
```

---

## Implementation Requirements

### 1. Create New File: `maxsim_token.rs`

**Path:** `crates/context-graph-utl/src/surprise/embedder_entropy/maxsim_token.rs`

**Required Imports:**
```rust
use super::EmbedderEntropy;
use crate::config::SurpriseConfig;
use crate::error::{UtlError, UtlResult};
use context_graph_core::teleological::Embedder;
```

**Struct Definition:**
```rust
/// E12 (LateInteraction) entropy using ColBERT-style MaxSim aggregation.
///
/// Late interaction embeddings are variable-length sequences of per-token
/// vectors (128D each). MaxSim finds, for each query token, its maximum
/// cosine similarity with any document token, then averages these scores.
///
/// # Algorithm
///
/// 1. Reshape current embedding into tokens (chunks of 128D)
/// 2. For each history embedding:
///    a. Reshape into tokens
///    b. For each current token, find max similarity to any history token
///    c. Average the per-token max similarities = MaxSim score
/// 3. ΔS = 1 - avg(top-k MaxSim scores), clamped to [0, 1]
///
/// # Token Representation
///
/// Embeddings are stored as flattened Vec<f32> with length = num_tokens * 128.
/// - 128 elements = 1 token
/// - 256 elements = 2 tokens
/// - Invalid: 257 elements -> not evenly divisible -> return empty
///
/// # Constitution Reference
/// E12: "Token KNN" (constitution.yaml delta_methods.ΔS)
#[derive(Debug, Clone)]
pub struct MaxSimTokenEntropy {
    /// Per-token embedding dimension. MUST be 128 (ColBERT standard).
    token_dim: usize,
    /// Minimum token count to consider valid. Default: 1.
    min_tokens: usize,
    /// Running mean for score normalization.
    running_mean: f32,
    /// Running variance for score normalization.
    running_variance: f32,
    /// Sample count for statistics.
    sample_count: usize,
    /// k neighbors for top-k averaging.
    k_neighbors: usize,
}
```

**Required Methods:**

```rust
impl MaxSimTokenEntropy {
    /// Create with constitution defaults (token_dim=128, min_tokens=1, k=5).
    pub fn new() -> Self;

    /// Create with specific token dimension.
    pub fn with_token_dim(token_dim: usize) -> Self;

    /// Create from SurpriseConfig.
    pub fn from_config(config: &SurpriseConfig) -> Self;

    /// Builder: set minimum token count.
    #[must_use]
    pub fn with_min_tokens(self, min_tokens: usize) -> Self;

    /// Builder: set k neighbors.
    #[must_use]
    pub fn with_k_neighbors(self, k: usize) -> Self;

    /// Reshape flat embedding into token slices.
    /// Returns empty Vec if length not divisible by token_dim.
    fn tokenize<'a>(&self, embedding: &'a [f32]) -> Vec<&'a [f32]>;

    /// Compute cosine similarity between two token vectors.
    fn token_similarity(&self, a: &[f32], b: &[f32]) -> f32;

    /// Compute MaxSim score between two tokenized embeddings.
    /// MaxSim(Q, D) = (1/|Q|) × Σᵢ max_j cos(qᵢ, dⱼ)
    fn compute_maxsim(&self, query_tokens: &[&[f32]], doc_tokens: &[&[f32]]) -> f32;

    /// Sigmoid normalization.
    #[inline]
    fn sigmoid(x: f32) -> f32;
}

impl Default for MaxSimTokenEntropy {
    fn default() -> Self { Self::new() }
}

impl EmbedderEntropy for MaxSimTokenEntropy {
    fn compute_delta_s(
        &self,
        current: &[f32],
        history: &[Vec<f32>],
        k: usize,
    ) -> UtlResult<f32>;

    fn embedder_type(&self) -> Embedder { Embedder::LateInteraction }

    fn reset(&mut self);
}
```

### 2. Update SurpriseConfig

**File:** `crates/context-graph-utl/src/config/surprise.rs`

Add these fields to `SurpriseConfig` struct (after line 150):
```rust
// --- MaxSim Token (E12 LateInteraction) ---

/// Token dimension for E12 late interaction embeddings.
/// Constitution: 128D per token (ColBERT standard)
/// Range: `[64, 256]`
pub late_interaction_token_dim: usize,

/// Minimum tokens required for valid E12 embedding.
/// Range: `[1, 10]`
pub late_interaction_min_tokens: usize,

/// k neighbors for MaxSim entropy averaging.
/// Range: `[1, 20]`
pub late_interaction_k_neighbors: usize,
```

Add to `Default` impl (after line 191):
```rust
// MaxSim Token (E12 LateInteraction) - per constitution.yaml delta_methods.ΔS E12
late_interaction_token_dim: 128,
late_interaction_min_tokens: 1,
late_interaction_k_neighbors: 5,
```

### 3. Update Module Exports

**File:** `crates/context-graph-utl/src/surprise/embedder_entropy/mod.rs`

After line 43 (`mod transe;`), add:
```rust
mod maxsim_token;
```

After line 53 (`pub use transe::TransEEntropy;`), add:
```rust
pub use maxsim_token::MaxSimTokenEntropy;
```

### 4. Update Factory Routing

**File:** `crates/context-graph-utl/src/surprise/embedder_entropy/factory.rs`

**Step 1:** Update imports (line 17-21):
```rust
use super::{
    AsymmetricKnnEntropy, CrossModalEntropy, DefaultKnnEntropy, EmbedderEntropy,
    GmmMahalanobisEntropy, HammingPrototypeEntropy, HybridGmmKnnEntropy, JaccardActiveEntropy,
    MaxSimTokenEntropy, TransEEntropy,  // ADD MaxSimTokenEntropy
};
```

**Step 2:** Change factory routing (lines 85-93):

FROM:
```rust
// E2-E4, E6, E8, E12: Default KNN-based entropy
Embedder::TemporalRecent
| Embedder::TemporalPeriodic
| Embedder::TemporalPositional
| Embedder::Sparse
| Embedder::Graph
| Embedder::LateInteraction => {
    Box::new(DefaultKnnEntropy::from_config(embedder, config))
}
```

TO:
```rust
// E12 (LateInteraction): MaxSim token-level entropy per constitution.yaml
Embedder::LateInteraction => Box::new(MaxSimTokenEntropy::from_config(config)),

// E2-E4, E6, E8: Default KNN-based entropy
Embedder::TemporalRecent
| Embedder::TemporalPeriodic
| Embedder::TemporalPositional
| Embedder::Sparse
| Embedder::Graph => {
    Box::new(DefaultKnnEntropy::from_config(embedder, config))
}
```

**Step 3:** Update factory doc comment (lines 42-48):
```rust
/// # Routing
/// - E1 (Semantic) → GmmMahalanobisEntropy
/// - E5 (Causal) → AsymmetricKnnEntropy
/// - E7 (Code) → HybridGmmKnnEntropy
/// - E9 (Hdc) → HammingPrototypeEntropy
/// - E10 (Multimodal) → CrossModalEntropy
/// - E11 (Entity) → TransEEntropy
/// - E12 (LateInteraction) → MaxSimTokenEntropy  // ADD THIS LINE
/// - E13 (KeywordSplade) → JaccardActiveEntropy
/// - All others → DefaultKnnEntropy
```

---

## Algorithm Specification

### MaxSim Formula (ColBERT)

```
MaxSim(Q, D) = (1/|Q|) × Σᵢ max_j cos(qᵢ, dⱼ)
```

Where:
- Q = query tokens (current embedding tokenized)
- D = document tokens (history embedding tokenized)
- qᵢ = i-th query token (128D vector)
- dⱼ = j-th document token (128D vector)
- cos(a,b) = cosine similarity

### Entropy Calculation

```
ΔS = 1 - avg(top-k MaxSim scores)
```

- High MaxSim = similar content = low surprise (ΔS near 0)
- Low MaxSim = novel content = high surprise (ΔS near 1)
- Empty history = maximum surprise (ΔS = 1.0)

### Tokenization Logic

```rust
fn tokenize<'a>(&self, embedding: &'a [f32]) -> Vec<&'a [f32]> {
    let token_dim = self.token_dim;
    if embedding.len() % token_dim != 0 {
        return vec![]; // FAIL FAST: invalid length
    }
    embedding.chunks_exact(token_dim).collect()
}
```

---

## Required Tests (in maxsim_token.rs)

| Test Name | Description | Expected Output |
|-----------|-------------|-----------------|
| `test_maxsim_empty_history_returns_one` | Empty history | `Ok(1.0)` |
| `test_maxsim_identical_returns_near_zero` | Same tokens | `Ok(delta_s < 0.5)` |
| `test_maxsim_orthogonal_returns_near_one` | Orthogonal tokens | `Ok(delta_s > 0.5)` |
| `test_maxsim_partial_overlap` | Some matching tokens | `Ok(delta_s ∈ (0.3, 0.7))` |
| `test_maxsim_empty_input_error` | Empty current | `Err(UtlError::EmptyInput)` |
| `test_maxsim_embedder_type` | Check embedder type | `Embedder::LateInteraction` |
| `test_maxsim_valid_range` | All outputs | `∈ [0.0, 1.0]` |
| `test_maxsim_no_nan_infinity` | No invalid values | `!is_nan() && !is_infinite()` |
| `test_maxsim_variable_length_query` | 2 vs 5 tokens | Valid score |
| `test_maxsim_variable_length_doc` | 5 vs 2 tokens | Valid score |
| `test_maxsim_single_token` | 1 token each | Degenerates to cosine |
| `test_maxsim_tokenize_valid` | 256 elements | 2 tokens |
| `test_maxsim_tokenize_invalid` | 257 elements | Empty vec |
| `test_maxsim_from_config` | Config values | Applied correctly |
| `test_maxsim_reset` | Reset state | Cleared properly |
| `test_maxsim_nan_input_error` | NaN in current | `Err(UtlError::EntropyError)` |
| `test_maxsim_infinity_input_error` | Inf in current | `Err(UtlError::EntropyError)` |

---

## Full State Verification Requirements

### 1. Source of Truth

The source of truth is:
- Factory routing: `EmbedderEntropyFactory::create(Embedder::LateInteraction, config)` returns `MaxSimTokenEntropy`
- Trait implementation: `MaxSimTokenEntropy.embedder_type()` returns `Embedder::LateInteraction`
- Computation: `compute_delta_s()` returns `UtlResult<f32>` in `[0.0, 1.0]`

### 2. Execute & Inspect Protocol

After implementation, run these verification commands:

```bash
# Build verification
cargo build -p context-graph-utl 2>&1 | grep -E "(error|warning)"

# Run all MaxSim tests
cargo test -p context-graph-utl maxsim_token -- --nocapture 2>&1

# Verify factory routing
cargo test -p context-graph-utl test_factory_creates_correct_types -- --nocapture 2>&1
cargo test -p context-graph-utl test_factory_create_all -- --nocapture 2>&1

# Run clippy
cargo clippy -p context-graph-utl -- -D warnings 2>&1 | head -20
```

### 3. Boundary & Edge Case Audit (Print State Before/After)

**Edge Case 1: Empty History**
```rust
#[test]
fn test_edge_case_empty_history() {
    let calc = MaxSimTokenEntropy::new();
    let current = vec![0.5f32; 128]; // 1 token
    let history: Vec<Vec<f32>> = vec![];

    println!("BEFORE: history.len() = {}", history.len());
    let result = calc.compute_delta_s(&current, &history, 5);
    println!("AFTER: result = {:?}", result);

    assert_eq!(result.unwrap(), 1.0);
}
```

**Edge Case 2: Maximum Token Count**
```rust
#[test]
fn test_edge_case_max_tokens() {
    let calc = MaxSimTokenEntropy::new();
    let current = vec![0.5f32; 128 * 100]; // 100 tokens
    let history = vec![vec![0.5f32; 128 * 100]; 10];

    println!("BEFORE: current_tokens=100, history_embeddings=10");
    let result = calc.compute_delta_s(&current, &history, 5);
    println!("AFTER: delta_s = {:?}", result);

    assert!(result.is_ok());
    let delta_s = result.unwrap();
    assert!((0.0..=1.0).contains(&delta_s));
}
```

**Edge Case 3: Invalid Token Length (Not Divisible by 128)**
```rust
#[test]
fn test_edge_case_invalid_token_length() {
    let calc = MaxSimTokenEntropy::new();
    let current = vec![0.5f32; 257]; // NOT divisible by 128
    let history = vec![vec![0.5f32; 128]];

    println!("BEFORE: current.len()={} (invalid)", current.len());
    let result = calc.compute_delta_s(&current, &history, 5);
    println!("AFTER: result = {:?}", result);

    // Should either return error or handle gracefully
    // Implementation choice: return max surprise (1.0) or error
}
```

### 4. Evidence of Success Log

After all tests pass, the test output must show:
```
[PASS] test_maxsim_empty_history_returns_one - delta_s = 1.0
[PASS] test_maxsim_identical_returns_near_zero - delta_s < 0.5
[PASS] test_maxsim_embedder_type - Embedder::LateInteraction
[PASS] test_maxsim_valid_range - All outputs in [0.0, 1.0]
[PASS] test_maxsim_no_nan_infinity - AP-10 compliant
[PASS] test_maxsim_from_config - Config values applied
[PASS] test_maxsim_reset - State cleared
```

---

## Manual Verification Checklist

After implementation, manually verify:

- [ ] `cargo build -p context-graph-utl` succeeds with no errors
- [ ] `cargo test -p context-graph-utl maxsim_token` - ALL tests pass
- [ ] `cargo clippy -p context-graph-utl -- -D warnings` - no warnings
- [ ] Factory routes `Embedder::LateInteraction` to `MaxSimTokenEntropy`
- [ ] `compute_delta_s()` returns values in `[0.0, 1.0]`
- [ ] No NaN/Infinity outputs (AP-10 compliance)
- [ ] `embedder_type()` returns `Embedder::LateInteraction`
- [ ] `reset()` clears running statistics

---

## Synthetic Test Data

Use these known inputs to verify expected outputs:

**Identical Tokens (Expected: ΔS ≈ 0)**
```rust
let current = vec![0.5f32; 128]; // 1 token, all 0.5
let history = vec![vec![0.5f32; 128]; 10]; // 10 identical embeddings
// MaxSim = 1.0, ΔS = 1 - 1.0 = 0.0
```

**Orthogonal Tokens (Expected: ΔS ≈ 1)**
```rust
let mut current = vec![0.0f32; 128];
current[0..64].fill(1.0); // First half non-zero

let mut hist_item = vec![0.0f32; 128];
hist_item[64..128].fill(1.0); // Second half non-zero
let history = vec![hist_item];
// cos(orthogonal) ≈ 0, MaxSim ≈ 0, ΔS ≈ 1.0
```

**Multi-token with Partial Match (Expected: ΔS ∈ (0.3, 0.7))**
```rust
let current = vec![0.5f32; 256]; // 2 tokens
let mut history_item = vec![0.5f32; 256];
history_item[128..256].fill(0.1); // Second token differs
let history = vec![history_item];
// First token matches perfectly (sim=1), second doesn't
```

---

## Rollback Plan

If implementation causes issues:
1. Revert factory routing to `DefaultKnnEntropy` for `Embedder::LateInteraction`
2. Remove `maxsim_token.rs` from module exports
3. Delete `maxsim_token.rs` file
4. Remove config fields from `SurpriseConfig`

---

## Related Tasks

- **TASK-UTL-P1-003**: HybridGmmKnnEntropy for E7 - COMPLETED
- **TASK-UTL-P1-004**: CrossModalEntropy for E10 - COMPLETED
- **TASK-UTL-P1-005**: TransEEntropy for E11 - Ready (similar pattern)

---

## Constitution Compliance

| Rule | Requirement | How Addressed |
|------|-------------|---------------|
| AP-10 | No NaN/Infinity in UTL | Clamp output to [0,1], validate inputs |
| ARCH-02 | Apples-to-apples comparison | E12↔E12 only within calculator |
| UTL-003 | Embedder uses specified ΔS method | "Token KNN" via MaxSim |

---

## Quick Start for Implementation

1. Read `transe.rs` as reference pattern (same structure, different algorithm)
2. Copy structure, change:
   - Struct name: `TransEEntropy` → `MaxSimTokenEntropy`
   - Algorithm: TransE distance → MaxSim scoring
   - Embedder type: `Entity` → `LateInteraction`
3. Add tokenize logic (chunk by 128)
4. Update factory, config, module exports
5. Run tests, verify all pass
6. Manual verification of edge cases

---

## Completion Notes (2026-01-12)

### Files Modified/Created

| File | Action | Description |
|------|--------|-------------|
| `crates/context-graph-utl/src/surprise/embedder_entropy/maxsim_token.rs` | CREATED | MaxSimTokenEntropy implementation (~400 lines, 33 tests) |
| `crates/context-graph-utl/src/surprise/embedder_entropy/mod.rs` | MODIFIED | Added module declaration and export |
| `crates/context-graph-utl/src/surprise/embedder_entropy/factory.rs` | MODIFIED | Updated routing + factory test for E12 |
| `crates/context-graph-utl/src/config/surprise.rs` | MODIFIED | Added E12 config fields |

### Test Results

```
cargo test -p context-graph-utl maxsim_token: 33 passed, 0 failed
cargo test -p context-graph-utl test_factory_routes_late_interaction_to_maxsim: 1 passed
cargo clippy -p context-graph-utl -- -D warnings: PASS (no warnings in UTL crate)
```

### Verification Checklist (All Passed)

- [x] `cargo build -p context-graph-utl` succeeds with no errors
- [x] `cargo test -p context-graph-utl maxsim_token` - ALL 33 tests pass
- [x] `cargo clippy -p context-graph-utl -- -D warnings` - no warnings
- [x] Factory routes `Embedder::LateInteraction` to `MaxSimTokenEntropy`
- [x] `compute_delta_s()` returns values in `[0.0, 1.0]`
- [x] No NaN/Infinity outputs (AP-10 compliance)
- [x] `embedder_type()` returns `Embedder::LateInteraction`
- [x] `reset()` clears running statistics

### Algorithm Implementation

Implemented ColBERT-style MaxSim scoring:
- `MaxSim(Q, D) = (1/|Q|) × Σᵢ max_j cos(qᵢ, dⱼ)`
- `ΔS = 1 - avg(top-k MaxSim scores)`
- Cosine similarity shifted from [-1,1] to [0,1] via `(sim + 1) / 2`
- Token dimension: 128D (ColBERT standard)

### Constitution Compliance

| Rule | Status |
|------|--------|
| AP-10 (No NaN/Infinity) | ✅ Validated |
| ARCH-02 (Apples-to-apples) | ✅ E12↔E12 only |
| UTL-003 (ΔS method) | ✅ Token KNN via MaxSim |
