---
id: "M04-T01"
title: "Define IndexConfig for FAISS IVF-PQ"
description: |
  Implement IndexConfig struct for FAISS GPU index configuration.
  Fields: dimension (1536), nlist (16384), nprobe (128), pq_segments (64),
  pq_bits (8), gpu_id (0), use_float16 (true), min_train_vectors (4_194_304).
  Include factory_string() method returning "IVF{nlist},PQ{pq_segments}x{pq_bits}".
layer: "foundation"
status: "pending"
priority: "critical"
estimated_hours: 2
sequence: 2
depends_on:
  - "M04-T00"
spec_refs:
  - "TECH-GRAPH-004 Section 2"
  - "REQ-KG-001 through REQ-KG-005"
files_to_create:
  - path: "crates/context-graph-graph/src/config.rs"
    description: "IndexConfig struct implementation"
  - path: "crates/context-graph-graph/tests/config_tests.rs"
    description: "Unit tests for IndexConfig"
files_to_modify: []
test_file: "crates/context-graph-graph/tests/config_tests.rs"
---

## Context

IndexConfig defines the parameters for FAISS IVF-PQ (Inverted File with Product Quantization) GPU index. This configuration is critical for achieving the performance targets of <5ms search on 10M vectors with k=10. The IVF-PQ index partitions the vector space into nlist cells and uses product quantization to compress vectors.

## Scope

### In Scope
- Define IndexConfig struct with all 8 fields
- Implement Default trait with specified values
- Implement factory_string() method for FAISS index creation
- Add Serde serialization/deserialization
- Create unit tests

### Out of Scope
- FAISS FFI bindings (see M04-T09)
- Actual index creation (see M04-T10)

## Definition of Done

### Signatures

```rust
use serde::{Deserialize, Serialize};

/// Configuration for FAISS IVF-PQ GPU index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    /// Vector dimension (must match embedding dimension)
    pub dimension: usize,
    /// Number of inverted lists (clusters)
    pub nlist: usize,
    /// Number of clusters to probe during search
    pub nprobe: usize,
    /// Number of product quantization segments
    pub pq_segments: usize,
    /// Bits per quantization code
    pub pq_bits: u8,
    /// GPU device ID
    pub gpu_id: i32,
    /// Use float16 for reduced memory
    pub use_float16: bool,
    /// Minimum vectors required for training (256 * nlist)
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
    /// Generate FAISS factory string for index creation
    /// Returns format: "IVF{nlist},PQ{pq_segments}x{pq_bits}"
    pub fn factory_string(&self) -> String {
        format!("IVF{},PQ{}x{}", self.nlist, self.pq_segments, self.pq_bits)
    }

    /// Calculate minimum training vectors based on nlist
    pub fn calculate_min_train_vectors(&self) -> usize {
        256 * self.nlist
    }
}
```

### Constraints
- dimension MUST be 1536 (matching embedding dimension)
- nlist = 16384 provides good recall/speed tradeoff for 10M vectors
- nprobe = 128 balances accuracy vs search time
- min_train_vectors MUST equal 256 * nlist
- pq_bits MUST be in [4, 8, 12, 16]

### Acceptance Criteria
- [ ] IndexConfig struct compiles with all 8 fields
- [ ] Default returns nlist=16384, nprobe=128, pq_segments=64, pq_bits=8
- [ ] factory_string() returns "IVF16384,PQ64x8" for defaults
- [ ] min_train_vectors = 256 * nlist = 4,194,304
- [ ] Serde Serialize/Deserialize implemented
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Define struct with all 8 fields
2. Derive Debug, Clone, Serialize, Deserialize
3. Implement Default with spec values
4. Implement factory_string() using format!()
5. Add calculate_min_train_vectors() helper

### Edge Cases
- nlist = 0: Invalid, but validation handled elsewhere
- pq_bits outside valid range: Validation handled by FAISS

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph config
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Default values match specification
- [ ] factory_string() output matches expected format
- [ ] JSON serialization/deserialization roundtrips correctly
