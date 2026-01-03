---
id: "M04-T01"
title: "Complete IndexConfig for FAISS IVF-PQ"
description: |
  Complete the existing IndexConfig struct by adding missing fields (gpu_id, use_float16,
  min_train_vectors), fixing pq_bits type (usize→u8), and implementing factory_string()
  and calculate_min_train_vectors() methods.
layer: "foundation"
status: "completed"
priority: "critical"
estimated_hours: 2
sequence: 2
depends_on: []
spec_refs:
  - "TECH-GRAPH-004 Section 2"
  - "REQ-KG-001 through REQ-KG-006"
  - "constitution.yaml: perf.latency.faiss_1M_k100: <2ms"
files_to_modify:
  - path: "crates/context-graph-graph/src/config.rs"
    description: "Add missing fields and methods to IndexConfig"
files_to_create: []
test_file: "crates/context-graph-graph/src/config.rs (inline #[cfg(test)])"
---

## CRITICAL: Current State Analysis (Audited 2026-01-03)

### CRATE EXISTS - M04-T00 IS COMPLETE

```bash
$ ls crates/context-graph-graph/
Cargo.toml  benches/  src/  tests/

$ cargo build -p context-graph-graph
# Compiles successfully
```

### IndexConfig ALREADY EXISTS but is INCOMPLETE

**File**: `crates/context-graph-graph/src/config.rs` (lines 26-50)

**Current Implementation**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    pub dimension: usize,      // ✓ EXISTS
    pub nlist: usize,          // ✓ EXISTS
    pub nprobe: usize,         // ✓ EXISTS
    pub pq_segments: usize,    // ✓ EXISTS
    pub pq_bits: usize,        // ✗ WRONG TYPE (should be u8)
    // MISSING: gpu_id: i32
    // MISSING: use_float16: bool
    // MISSING: min_train_vectors: usize
}
```

**Current Default Implementation**:
```rust
impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            dimension: 1536,     // ✓ CORRECT
            nlist: 16384,        // ✓ CORRECT
            nprobe: 128,         // ✓ CORRECT
            pq_segments: 64,     // ✓ CORRECT
            pq_bits: 8,          // ✗ WRONG TYPE (should be u8)
            // MISSING: gpu_id: 0
            // MISSING: use_float16: true
            // MISSING: min_train_vectors: 4_194_304
        }
    }
}
```

**Missing Methods**:
- `factory_string()` → must return `"IVF{nlist},PQ{pq_segments}x{pq_bits}"`
- `calculate_min_train_vectors()` → must return `256 * self.nlist`

### Existing Tests (lines 112-182)
The file has inline tests but they do NOT cover:
- `factory_string()` output format
- `min_train_vectors` calculation
- `gpu_id` and `use_float16` fields
- Validation of pq_bits range [4, 8, 12, 16]

---

## Context

IndexConfig defines parameters for FAISS IVF-PQ (Inverted File with Product Quantization) GPU index. This configuration is critical for achieving:
- **Performance**: <5ms search on 10M vectors with k=10
- **Memory**: 8GB VRAM for 10M 1536D vectors using PQ64x8
- **Recall**: nprobe=128 balances accuracy vs speed

### Constitution References
- `stack.deps`: faiss@0.12+gpu
- `stack.gpu`: RTX 5090, VRAM 32GB, CUDA 13.1
- `perf.latency.faiss_1M_k100`: <2ms
- `perf.throughput.search_batch_100`: <5ms

---

## Scope

### In Scope
1. Add missing fields to IndexConfig: `gpu_id`, `use_float16`, `min_train_vectors`
2. Change `pq_bits` type from `usize` to `u8`
3. Implement `factory_string()` method
4. Implement `calculate_min_train_vectors()` method
5. Update Default impl with new fields
6. Add comprehensive tests for all fields and methods
7. Verify Serde serialization/deserialization works with new fields

### Out of Scope
- FAISS FFI bindings (see M04-T09)
- Actual index creation (see M04-T10)
- Validation logic beyond basic type constraints

---

## Definition of Done

### Target Signature

The EXACT code that must exist in `crates/context-graph-graph/src/config.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Configuration for FAISS IVF-PQ GPU index.
///
/// Configures the FAISS GPU index for 10M+ vector search with <5ms latency.
///
/// # Performance Targets
/// - 10M vectors, k=10: <5ms latency
/// - 10M vectors, k=100: <10ms latency
/// - Memory: ~8GB VRAM for 10M 1536D vectors with PQ64x8
///
/// # Constitution Reference
/// - perf.latency.faiss_1M_k100: <2ms
/// - stack.deps: faiss@0.12+gpu
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexConfig {
    /// Vector dimension (must match embedding dimension).
    /// Default: 1536 per constitution embeddings.models.E7_Code
    pub dimension: usize,

    /// Number of inverted lists (clusters).
    /// Default: 16384 = 4 * sqrt(10M) for optimal recall/speed tradeoff
    pub nlist: usize,

    /// Number of clusters to probe during search.
    /// Default: 128 balances accuracy vs search time
    pub nprobe: usize,

    /// Number of product quantization segments.
    /// Must evenly divide dimension. Default: 64 (1536/64 = 24 bytes per segment)
    pub pq_segments: usize,

    /// Bits per quantization code.
    /// Valid values: 4, 8, 12, 16. Default: 8
    pub pq_bits: u8,

    /// GPU device ID.
    /// Default: 0 (primary GPU)
    pub gpu_id: i32,

    /// Use float16 for reduced memory.
    /// Default: true (halves VRAM usage)
    pub use_float16: bool,

    /// Minimum vectors required for training (256 * nlist).
    /// Default: 4,194,304 (256 * 16384)
    pub min_train_vectors: usize,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            dimension: 1536,
            nlist: 16384,
            nprobe: 128,
            pq_segments: 64,
            pq_bits: 8,
            gpu_id: 0,
            use_float16: true,
            min_train_vectors: 4_194_304, // 256 * 16384
        }
    }
}

impl IndexConfig {
    /// Generate FAISS factory string for index creation.
    ///
    /// Returns format: "IVF{nlist},PQ{pq_segments}x{pq_bits}"
    ///
    /// # Example
    /// ```
    /// use context_graph_graph::config::IndexConfig;
    /// let config = IndexConfig::default();
    /// assert_eq!(config.factory_string(), "IVF16384,PQ64x8");
    /// ```
    pub fn factory_string(&self) -> String {
        format!("IVF{},PQ{}x{}", self.nlist, self.pq_segments, self.pq_bits)
    }

    /// Calculate minimum training vectors based on nlist.
    ///
    /// FAISS requires at least 256 vectors per cluster for quality training.
    ///
    /// # Returns
    /// 256 * nlist
    ///
    /// # Example
    /// ```
    /// use context_graph_graph::config::IndexConfig;
    /// let config = IndexConfig::default();
    /// assert_eq!(config.calculate_min_train_vectors(), 4_194_304);
    /// ```
    pub fn calculate_min_train_vectors(&self) -> usize {
        256 * self.nlist
    }
}
```

### Constraints
- `dimension` MUST be 1536 (matching embedding dimension from constitution)
- `nlist` = 16384 provides good recall/speed tradeoff for 10M vectors
- `nprobe` = 128 balances accuracy vs search time
- `pq_segments` MUST evenly divide dimension (1536 % 64 = 0)
- `pq_bits` MUST be one of [4, 8, 12, 16]
- `min_train_vectors` MUST equal 256 * nlist
- `gpu_id` MUST be i32 (CUDA device ordinal type)
- `use_float16` defaults to true for RTX 5090 VRAM optimization

---

## Implementation Steps

### Step 1: Read Current File
```bash
cat crates/context-graph-graph/src/config.rs
```

### Step 2: Modify IndexConfig Struct
Replace the existing IndexConfig struct (lines 26-38) with the target signature above.

### Step 3: Update Default Implementation
Replace the existing Default impl (lines 40-50) with the target Default impl above.

### Step 4: Add Methods
Add `factory_string()` and `calculate_min_train_vectors()` methods after Default impl.

### Step 5: Update Tests
Replace existing tests (lines 112-125) with comprehensive tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_config_default_values() {
        let config = IndexConfig::default();
        assert_eq!(config.dimension, 1536);
        assert_eq!(config.nlist, 16384);
        assert_eq!(config.nprobe, 128);
        assert_eq!(config.pq_segments, 64);
        assert_eq!(config.pq_bits, 8);
        assert_eq!(config.gpu_id, 0);
        assert!(config.use_float16);
        assert_eq!(config.min_train_vectors, 4_194_304);
    }

    #[test]
    fn test_index_config_pq_segments_divides_dimension() {
        let config = IndexConfig::default();
        assert_eq!(
            config.dimension % config.pq_segments,
            0,
            "PQ segments must divide dimension evenly"
        );
    }

    #[test]
    fn test_index_config_min_train_vectors_formula() {
        let config = IndexConfig::default();
        assert_eq!(
            config.min_train_vectors,
            256 * config.nlist,
            "min_train_vectors must equal 256 * nlist"
        );
    }

    #[test]
    fn test_factory_string_default() {
        let config = IndexConfig::default();
        assert_eq!(config.factory_string(), "IVF16384,PQ64x8");
    }

    #[test]
    fn test_factory_string_custom() {
        let config = IndexConfig {
            dimension: 768,
            nlist: 4096,
            nprobe: 64,
            pq_segments: 32,
            pq_bits: 4,
            gpu_id: 1,
            use_float16: false,
            min_train_vectors: 256 * 4096,
        };
        assert_eq!(config.factory_string(), "IVF4096,PQ32x4");
    }

    #[test]
    fn test_calculate_min_train_vectors() {
        let config = IndexConfig::default();
        assert_eq!(config.calculate_min_train_vectors(), 4_194_304);

        let custom = IndexConfig {
            nlist: 1024,
            ..Default::default()
        };
        assert_eq!(custom.calculate_min_train_vectors(), 256 * 1024);
    }

    #[test]
    fn test_index_config_serialization_roundtrip() {
        let config = IndexConfig::default();
        let json = serde_json::to_string(&config).expect("Serialization failed");
        let deserialized: IndexConfig =
            serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_index_config_json_format() {
        let config = IndexConfig::default();
        let json = serde_json::to_string_pretty(&config).expect("Serialization failed");
        assert!(json.contains("\"dimension\": 1536"));
        assert!(json.contains("\"nlist\": 16384"));
        assert!(json.contains("\"nprobe\": 128"));
        assert!(json.contains("\"pq_segments\": 64"));
        assert!(json.contains("\"pq_bits\": 8"));
        assert!(json.contains("\"gpu_id\": 0"));
        assert!(json.contains("\"use_float16\": true"));
        assert!(json.contains("\"min_train_vectors\": 4194304"));
    }

    #[test]
    fn test_pq_bits_type_is_u8() {
        let config = IndexConfig::default();
        // This is a compile-time check - if pq_bits is not u8, this won't compile
        let _: u8 = config.pq_bits;
    }
}
```

### Step 6: Verify Build
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph config
cargo clippy -p context-graph-graph -- -D warnings
```

---

## Verification

### Test Commands (Source of Truth)
```bash
# Build verification
cargo build -p context-graph-graph 2>&1

# Run all config tests
cargo test -p context-graph-graph config -- --nocapture 2>&1

# Clippy verification
cargo clippy -p context-graph-graph -- -D warnings 2>&1

# Check documentation compiles
cargo doc -p context-graph-graph --no-deps 2>&1
```

### Expected Test Output
```
running 10 tests
test config::tests::test_index_config_default_values ... ok
test config::tests::test_index_config_pq_segments_divides_dimension ... ok
test config::tests::test_index_config_min_train_vectors_formula ... ok
test config::tests::test_factory_string_default ... ok
test config::tests::test_factory_string_custom ... ok
test config::tests::test_calculate_min_train_vectors ... ok
test config::tests::test_index_config_serialization_roundtrip ... ok
test config::tests::test_index_config_json_format ... ok
test config::tests::test_pq_bits_type_is_u8 ... ok
test config::tests::test_hyperbolic_config_default ... ok
test config::tests::test_cone_config_default ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## Full State Verification Protocol

### 1. Source of Truth Definition
The source of truth for this task is:
- **File**: `crates/context-graph-graph/src/config.rs`
- **Struct**: `IndexConfig` with all 8 fields
- **Methods**: `factory_string()` and `calculate_min_train_vectors()`
- **Tests**: All tests pass with `cargo test`

### 2. Execute & Inspect

After implementation, run these commands and verify output:

```bash
# Verify IndexConfig has all 8 fields
grep -A 30 "pub struct IndexConfig" crates/context-graph-graph/src/config.rs

# Verify factory_string method exists
grep -A 5 "fn factory_string" crates/context-graph-graph/src/config.rs

# Verify calculate_min_train_vectors method exists
grep -A 5 "fn calculate_min_train_vectors" crates/context-graph-graph/src/config.rs

# Run tests and capture output
cargo test -p context-graph-graph config -- --nocapture 2>&1 | tee /tmp/m04-t01-test-output.txt

# Verify specific test exists and passes
grep "test_factory_string_default" /tmp/m04-t01-test-output.txt
grep "ok" /tmp/m04-t01-test-output.txt | wc -l  # Should show at least 10 OKs
```

### 3. Boundary & Edge Case Audit

**Edge Case 1: pq_bits type verification**
```bash
# Before: pq_bits was usize
grep "pq_bits: usize" crates/context-graph-graph/src/config.rs
# Should return NOTHING after fix

# After: pq_bits is u8
grep "pq_bits: u8" crates/context-graph-graph/src/config.rs
# Should return the field definition
```

**Edge Case 2: Empty factory string (nlist=0)**
```rust
// This is a logic edge case - with nlist=0:
// factory_string() would return "IVF0,PQ64x8"
// This is valid string output (validation is separate concern)
let config = IndexConfig { nlist: 0, ..Default::default() };
assert_eq!(config.factory_string(), "IVF0,PQ64x8");
```

**Edge Case 3: min_train_vectors calculation overflow**
```rust
// With nlist at max usize, 256 * nlist would overflow
// For practical purposes, nlist <= 1M is reasonable
// Testing with very large nlist:
let config = IndexConfig { nlist: 1_000_000, ..Default::default() };
assert_eq!(config.calculate_min_train_vectors(), 256_000_000);
```

### 4. Evidence of Success

After all verification, provide this log:

```
=== M04-T01 VERIFICATION LOG ===
Timestamp: [ISO timestamp]

FIELD VERIFICATION:
- dimension: usize ✓
- nlist: usize ✓
- nprobe: usize ✓
- pq_segments: usize ✓
- pq_bits: u8 ✓ (CHANGED from usize)
- gpu_id: i32 ✓ (NEW)
- use_float16: bool ✓ (NEW)
- min_train_vectors: usize ✓ (NEW)

METHOD VERIFICATION:
- factory_string() returns "IVF16384,PQ64x8" for defaults ✓
- calculate_min_train_vectors() returns 4194304 for defaults ✓

TEST OUTPUT:
[paste cargo test output showing all tests pass]

CLIPPY OUTPUT:
[paste cargo clippy output showing no warnings]

RESULT: PASS/FAIL
```

---

## IMPORTANT: Manual Output Verification

After running tests, you MUST verify the actual values by inspecting:

1. **Struct in compiled output** - Run `cargo expand -p context-graph-graph` or inspect the source
2. **JSON serialization** - The test `test_index_config_json_format` validates JSON keys exist
3. **Method output** - The tests `test_factory_string_*` validate exact string format

**DO NOT rely only on return values**. Verify the source file contains exact code.

---

## Final Verification: Sherlock-Holmes Agent

**MANDATORY**: After completing all implementation and verification steps, spawn a `sherlock-holmes` subagent with this prompt:

```
Forensically verify M04-T01 (IndexConfig completion) is 100% complete:

1. Read crates/context-graph-graph/src/config.rs
2. Verify IndexConfig has EXACTLY 8 fields:
   - dimension: usize
   - nlist: usize
   - nprobe: usize
   - pq_segments: usize
   - pq_bits: u8 (NOT usize)
   - gpu_id: i32
   - use_float16: bool
   - min_train_vectors: usize

3. Verify Default impl sets:
   - min_train_vectors = 4_194_304
   - use_float16 = true
   - gpu_id = 0

4. Verify methods exist:
   - factory_string() -> String
   - calculate_min_train_vectors() -> usize

5. Run: cargo test -p context-graph-graph config -- --nocapture
   Verify ALL tests pass

6. Run: cargo clippy -p context-graph-graph -- -D warnings
   Verify NO warnings

7. Check factory_string() output format matches "IVF{nlist},PQ{pq_segments}x{pq_bits}"

Report any discrepancies. The task is NOT complete until Sherlock confirms ALL criteria pass.
```

If Sherlock identifies ANY issues, fix them BEFORE marking this task complete.

---

## Acceptance Criteria

- [ ] IndexConfig has all 8 fields with correct types
- [ ] `pq_bits` is `u8` (not `usize`)
- [ ] `gpu_id` is `i32`
- [ ] `use_float16` is `bool`
- [ ] `min_train_vectors` is `usize`
- [ ] Default returns all spec values including `min_train_vectors: 4_194_304`
- [ ] `factory_string()` returns `"IVF16384,PQ64x8"` for defaults
- [ ] `calculate_min_train_vectors()` returns `256 * nlist`
- [ ] Serde Serialize/Deserialize implemented with PartialEq
- [ ] All tests pass with `cargo test -p context-graph-graph config`
- [ ] No clippy warnings
- [ ] Sherlock-Holmes verification passed

---

## Notes for AI Agent

1. **FILE EXISTS**: Do NOT create new file. MODIFY existing `crates/context-graph-graph/src/config.rs`
2. **PRESERVE OTHER CONFIGS**: HyperbolicConfig and ConeConfig exist in same file - do not remove them
3. **NO BACKWARDS COMPATIBILITY**: Change `pq_bits` type directly. If downstream breaks, fix downstream.
4. **FAIL FAST**: If tests fail, do not mark complete. Fix first.
5. **VERIFY OUTPUT**: After tests pass, manually grep for the exact struct definition to confirm.

---

*Audited: 2026-01-03*
*Codebase State: commit 9e6cc2d*
*Module: 04 - Knowledge Graph*
*Task: M04-T01*
*Dependencies: None (M04-T00 is COMPLETE)*
