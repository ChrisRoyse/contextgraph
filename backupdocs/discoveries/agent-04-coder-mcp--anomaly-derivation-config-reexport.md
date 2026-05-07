---
namespace: discoveries
agent_id: agent-04-coder-mcp
timestamp: 2026-04-15T08:05:00Z
status: resolved
tags: imports, rust-modules, reexport
---

# `AnomalyDerivationConfig` is re-exported at `rocksdb_store::` not `anomaly_derivation::`

## Symptom
First build iteration failed with:

```
error[E0603]: module `anomaly_derivation` is private
  --> crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs:29:57
use context_graph_storage::teleological::rocksdb_store::anomaly_derivation::AnomalyDerivationConfig;
                                                        ^^^^^^^^^^^^^^^^^^ private module
```

## Cause
`crates/context-graph-storage/src/teleological/rocksdb_store/mod.rs:45`
declares `mod anomaly_derivation;` (private), and at line 74 re-exports
the types:

```rust
pub use anomaly_derivation::{
    classify_edge_as_anomaly, AnomalyDerivationConfig, AnomalyDerivationSummary,
};
```

So the correct import is:

```rust
use context_graph_storage::teleological::rocksdb_store::AnomalyDerivationConfig;
```

This matches the handoff's claim (agent-03 called it "`context_graph_storage::teleological::rocksdb_store`") but the handoff
example used the longer private-module path, which would have leaked the
private module boundary.

## Pattern for future agents
Always grep `pub use` before writing imports:

```
grep -n "pub use" crates/context-graph-storage/src/teleological/rocksdb_store/mod.rs
```

Then prefer the re-exported path — it's the public API surface.
