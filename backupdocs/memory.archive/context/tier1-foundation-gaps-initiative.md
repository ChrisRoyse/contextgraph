---
namespace: context
agent_id: agent-200-coordinator
timestamp: 2026-05-08T22:00:00Z
status: active
tags: tier1, ruvector-findings, clean-room, mejepa, mmr, conformal, ed25519
---

# Tier-1 Foundation Gaps Initiative (2026-05-08)

## What this initiative is

Per `docs/ruvectorfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` Tier-1, several
clean-room foundation primitives remain pending. The operator-stated end goal
(`docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`) is **ME-JEPA-Code**
— a reality compiler that makes line-by-line code review obsolete.

The Tier-1 work that's already SHIPPED:

- `crates/context-graph-witness/` — 73-byte SHAKE-256 chain core, format manifest, repair, atomic IO.
- `crates/context-graph-solver/` — weighted Forward-Push PPR + Conjugate Gradient + CSR validation.
- MCP `optimizer_compute_influence` (Forward-Push PPR over attempt-influence graph).
- MCP `optimizer_recall_recommendations` MMR rerank (text-Jaccard, optimizer-private).
- Witness-chain preflight on every write.

The Tier-1 work this initiative ships in this turn:

1. **Generic MMR diversity reranker** — extract from optimizer-private text-Jaccard form
   into a generic `solver::mmr` module that takes any `relevance: &[f64]` plus a similarity
   closure `Fn(usize, usize) -> f64`. Reusable for any retrieval pipeline.

2. **Conformal prediction module** — clean-room implementation per
   `02_CAPABILITIES_INVENTORY.md A.3` with `Distance`, `InverseRank`, `NormalizedDistance`
   nonconformity measures, calibration, prediction-set construction, statistical-coverage
   guarantee. Real synthetic-data tests demonstrating empirical coverage matches stated alpha.

3. **Ed25519 segment signing for context-graph-witness** — per `03 §2d`. New
   `WitnessSignature` struct, `sign_segment` / `verify_segment` functions, key
   derivation from operator-supplied seed (never written to disk), fail-closed
   verification path. Real keypair, tampered-byte rejection test.

The Tier-1 work explicitly DEFERRED to a future turn:

- BMSSP V-cycle multigrid (substantial; deserves its own focused turn — no fake
  "wrap CG and call it multigrid" allowed).
- 8/7/5/3-bit tier compression (substantial; bounded but needs many quantizer
  tests, RMSE per tier, decay-policy state machine).

## Clean-room rule (load-bearing)

Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`
every new file MUST start with the provenance header citing the inspirational
RuVector source file + commit. No code copied, no path-dep, no npm-depend.

## Coordination policy for this initiative

The implementation work itself is done by the coordinator (Claude Code, opus)
directly — bounded scope (3 small modules), tight integration, verification
must be personal. Memory writes happen incrementally under
`./memory/{discoveries,patterns,decisions,reports,journal}/`.

**Subagent use:**
- `code-simplifier` is invoked at the end to review all three modules; it
  reads the memory entries listed in `./memory/handoff/agent-200-coordinator--tier1-handoff.md`
  to know exactly what to review.

## FSV protocol for this initiative

For each module:

1. Compile clean (release): `cargo build --release -p context-graph-solver`,
   `cargo build --release -p context-graph-witness --features ed25519`.
2. Real tests pass (no mocks; synthetic data with known answers): `cargo test -p ...`.
3. Manual FSV: write a tiny standalone example demonstrating the module's
   public API on a known-answer input; print the output; verify the output
   matches what the math demands.
4. Tampered-byte rejection (where applicable: ed25519 verifies-then-rejects;
   conformal returns wide set under tampered calibration; MMR returns
   relevance-only ordering at λ=1).

## Cross-references

- `docs/ruvectorfindings/02_CAPABILITIES_INVENTORY.md` — capability spec.
- `docs/ruvectorfindings/03_REALITY_LOOP_APPLICATIONS.md §2d, §4a, §4d` — ccreality use cases.
- `docs/ruvectorfindings/04_JEPA_APPLICATIONS.md §1, §2, §5` — JEPA use cases.
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §4.2, §5.5` — conformal calibration role.
