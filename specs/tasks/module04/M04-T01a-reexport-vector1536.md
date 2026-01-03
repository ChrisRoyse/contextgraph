---
id: "M04-T01a"
title: "Re-export Vector1536 from Embeddings Crate"
description: |
  Re-export the Vector1536 type from context-graph-embeddings for use in the graph crate.
  This ensures type consistency between embedding vectors and graph index vectors.
  Add to crates/context-graph-graph/src/lib.rs.
layer: "foundation"
status: "pending"
priority: "high"
estimated_hours: 1
sequence: 3
depends_on:
  - "M04-T01"
spec_refs:
  - "TECH-GRAPH-004 Section 2"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/lib.rs"
    description: "Add Vector1536 re-export"
  - path: "crates/context-graph-graph/Cargo.toml"
    description: "Ensure context-graph-embeddings dependency exists"
test_file: "crates/context-graph-graph/tests/config_tests.rs"
---

## Context

Vector1536 is the standard 1536-dimensional vector type used throughout the embedding pipeline. Re-exporting it from the graph crate ensures type consistency when vectors flow from embeddings to the FAISS index. This prevents type mismatches and eliminates the need for conversions.

## Scope

### In Scope
- Re-export Vector1536 from context-graph-embeddings
- Ensure Cargo.toml has the embeddings crate dependency
- Add a simple test verifying the re-export works

### Out of Scope
- Defining Vector1536 (already exists in embeddings crate)
- IndexConfig changes (handled in M04-T01)

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/lib.rs

// Re-export Vector1536 for type consistency with embeddings
pub use context_graph_embeddings::Vector1536;
```

```toml
# In Cargo.toml, ensure this dependency exists:
[dependencies]
context-graph-embeddings = { path = "../context-graph-embeddings" }
```

### Constraints
- Vector1536 MUST be accessible from crate root: `context_graph_graph::Vector1536`
- No wrapper types - use the exact same type
- Do not re-implement - re-export only

### Acceptance Criteria
- [ ] `pub use context_graph_embeddings::Vector1536;` compiles
- [ ] Vector1536 is accessible from crate root
- [ ] Cargo.toml has context-graph-embeddings dependency
- [ ] Compiles with `cargo build`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Open `src/lib.rs`
2. Add `pub use context_graph_embeddings::Vector1536;`
3. Verify Cargo.toml has dependency
4. Test compilation

### Edge Cases
- Vector1536 not found in embeddings crate: Verify embeddings crate exports it
- Circular dependency: Should not occur as embeddings doesn't depend on graph

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] `use context_graph_graph::Vector1536;` works in tests
- [ ] `cargo doc -p context-graph-graph` shows Vector1536 in documentation
