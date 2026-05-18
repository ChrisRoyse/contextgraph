---
namespace: decisions
agent_id: agent-142-coordinator
timestamp: 2026-05-08T20:55:00Z
status: active
tags: ruvector, clean-room, solver, ppr, conjugate-gradient, tier1, fsv
---

# Decision — ContextGraph owns a clean-room solver foundation

## What shipped

ContextGraph now has `crates/context-graph-solver/`, a clean-room solver crate inspired by the read-only RuVector solver snapshot at HEAD `ef5274c2`.

Implemented:

- Weighted Forward-Push Personalized PageRank for influence ranking.
- Validated Conjugate Gradient for symmetric positive-definite systems.
- CSR matrix construction with hard validation of dimensions, finite values, sorted rows, non-negative adjacency weights, and SPD symmetry claims.
- Fail-closed structured errors with remediation text.
- Manual full-state verification example that writes evidence JSON, reads it back from disk, and proves happy-path plus edge-case behavior.

## Source of truth

The crate itself is source code under `crates/context-graph-solver/`.

Manual verification source of truth:

- `target/context-graph-solver-fsv/manual-evidence.json`

That evidence file records:

- PPR on a two-node graph with the closed-form expected estimate `[2/3, 1/3]`.
- CG on a 2x2 SPD system with closed-form solution `[1/11, 7/11]`.
- Edge cases for empty graph, zero PPR tolerance, and unsymmetric CG input.

## What did not ship

BMSSP/multigrid was intentionally not hidden behind a CG wrapper. That algorithm remains unimplemented until a real clean-room V-cycle implementation lands with its own FSV evidence.

The optimizer MCP tool `optimizer_compute_influence` is also still pending; this crate is the algorithmic foundation it should call.

## Why

The RuVector findings require solver-backed influence ranking and JEPA sampling without path dependencies or vendored code. This crate gives ContextGraph an owned implementation of the two safest primitives first and keeps the missing primitive explicit instead of introducing a misleading fallback.

## Verification run

Commands run:

```bash
cargo test -p context-graph-solver -- --nocapture
cargo run -p context-graph-solver --example solver_fsv -- target/context-graph-solver-fsv/manual-evidence.json
sed -n '1,260p' target/context-graph-solver-fsv/manual-evidence.json
cargo check
```

All passed on 2026-05-08. `cargo check` completed for the default workspace members; existing dead-code warnings remain in CLI/MCP path-policy modules and are unrelated to the solver crate.
