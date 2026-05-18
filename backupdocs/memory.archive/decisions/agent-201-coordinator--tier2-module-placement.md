---
namespace: decisions
agent_id: agent-201-coordinator
timestamp: 2026-05-08T23:58:00Z
status: active
tags: tier2, module-placement, mincut, granger, certificates, clean-room
---

# Decision: Tier-2 Module Placement

## What

- **Stoer-Wagner mincut** lives in a NEW crate `crates/context-graph-mincut/`.
- **Granger causality test** lives in `crates/context-graph-solver/src/granger.rs`
  (new module within the existing solver crate).
- **Merkle tree + Stage 0 + Stage F certificates** live in
  `crates/context-graph-witness/src/cert.rs` (new module within the existing
  witness crate, gated under the same `ed25519` feature when signing matters
  for these certs — but Merkle alone needs no feature flag).

## Why

- Mincut is conceptually a graph algorithm but does not depend on the solver
  crate's PageRank/CG primitives. Putting it in the solver would conflate
  "linear-algebra solvers" with "graph cuts" — different algorithmic families.
  Future mincut variants (Karger, parallel, link-cut) will accumulate; a
  dedicated crate keeps the dependency graph clean.
- Granger is a statistical test (OLS + F-test) that operates on time-series
  inputs. It composes naturally with the solver crate's CSR + matrix machinery
  (the Granger F-test reduces to two least-squares problems we can use the
  same matrix code for). Putting it in `context-graph-solver` keeps the
  statistical primitives co-located with their numerical primitives.
- Merkle + certificates extend the existing witness chain with proof-of-
  membership semantics. The certificates structurally embed the chain
  primitives we already shipped in `context-graph-witness`. Putting them in
  the same crate keeps the cryptographic surface in one place. The pure
  Merkle tree has no ed25519 dependency; it is feature-gated only where it
  composes with signed certificates.

## How to apply

When adding new graph algorithms (Karger, link-cut, etc.), add them as
modules within `context-graph-mincut`. When adding new statistical tests
(Wald, t-test), add them as modules within `context-graph-solver`. When
adding new certificate kinds (TEE attestation, supersession), add them as
modules within `context-graph-witness`.

## Cross-references

- Supersedes the speculative `crates/context-graph-mincut/` placement in
  `docs/ruvectorfindings/06_RISKS_AND_NON_TRANSFERABLE.md` table only in that
  it CONFIRMS that location.
- Continuous with Tier-1 placement decision
  (`memory/decisions/agent-200-coordinator--tier1-module-placement.md`):
  related-functionality-stays-co-located.
