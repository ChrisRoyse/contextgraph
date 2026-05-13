---
namespace: decisions
agent_id: agent-200-coordinator
timestamp: 2026-05-08T22:05:00Z
status: active
tags: tier1, module-placement, mmr, conformal, ed25519, clean-room
---

# Decision: Tier-1 Module Placement

## What

- **Generic MMR diversity reranker** lives in `crates/context-graph-solver/src/mmr.rs`.
  Re-exported from the crate root.
- **Conformal prediction** lives in `crates/context-graph-solver/src/conformal.rs`.
  Re-exported from the crate root.
- **Ed25519 segment signing** lives in `crates/context-graph-witness/src/sign.rs`,
  gated behind a non-default Cargo feature `ed25519`.

## Why

- `context-graph-solver`'s description is "Clean-room graph influence and sparse
  linear solver primitives for ContextGraph." MMR (greedy max-marginal relevance)
  and conformal prediction are statistical / retrieval primitives that compose
  with PPR over the same graphs. Putting them in `context-graph-solver` keeps
  the dependency graph flat and avoids creating a new crate just for two ~300-LOC
  modules. Doc 06's recommendation said "new module under existing retrieval
  crate" but ContextGraph has no `retrieval` crate; `context-graph-solver` is
  the closest and the spirit of the rule (no new crate just for one module) is
  honored. Future moves (e.g., to a `context-graph-retrieval` crate) require
  only re-exports — public API stays stable.
- `context-graph-witness` is already the canonical home for the chain primitive,
  per `docs/ruvectorfindings/03 §2`. Ed25519 segment signing is an additive
  feature on the same chain; gating it behind the `ed25519` Cargo feature keeps
  builds lean for callers that only want SHAKE-256 verification.

## How to apply

When adding new sub-primitives to MMR or conformal, prefer extending the existing
modules over a new crate. When adding signing schemes (ML-DSA-65, SLH-DSA-128s),
use the established `sign.rs` pattern with new feature flags — `mldsa`, `slhdsa`.

## Cross-references

- `docs/ruvectorfindings/06_RISKS_AND_NON_TRANSFERABLE.md §C` — the "where to put
  the new crates" table; `context-graph-retrieval` was speculative, never landed.
- This decision supersedes that table for MMR + conformal placement until a
  separate retrieval crate is justified.
