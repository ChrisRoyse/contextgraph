# Heal domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory. **`ASSUME THE TASK IS WRONG`** — audit file paths, symbol names, and CF references against the live tree before implementing.

**Status snapshot (2026-05-12):** Phase A ships the scheduler skeleton and seven ticker structure. Phase B ships `AgentSurprise` queue priority/consumer behavior and core per-head UTML carriers. All tasks below remain open unless noted; they are the full production policy layer on top of the shipped skeleton.

## TASK-HEAL-001 — Self-optimization scheduler (full implementation)

- **Phase:** E · **Effort:** 1.5d · **Blocked by:** TASK-FLYWHEEL-002 · **Spec:** TECH-HEAL §3 + SPEC-HEAL REQ-HEAL-FB-01
- **Goal:** Replace the FLYWHEEL-002 skeleton with full implementation that calls all the heal/* modules per `09_SELF_HEALING_AUTONOMY.md §2` + `TECH-HEAL §3`.
- **Outputs:** `crates/context-graph-mejepa/src/heal/scheduler.rs` (full); `scheduler_state.rs` extensions.
- **DoD:** all 7 tickers fire and call expected heal modules; FSV verifies for each ticker that the right module was called.
- **FSV:** `/mnt/d/contextgraph/fsv/mejepa-heal-fsv/scheduler_full.json`

## TASK-HEAL-002 — Per-cell stratified drift detection

- **Phase:** E · **Effort:** 1d · **Blocked by:** TASK-HEAL-001 · **Spec:** SPEC-HEAL REQ-HEAL-FB-02 + TECH-HEAL §4
- **Goal:** Track drift per (mutation_category × language) cell separately. Per-cell soft drift triggers per-cell LoRA refresh.
- **Outputs:** `heal/drift_per_cell.rs` (NEW).
- **DoD:** synthetic per-cell drift fires per-cell intervention; global state unaffected.

## TASK-HEAL-003 — Dissipation detection

- **Phase:** E · **Effort:** 1d · **Blocked by:** TASK-HEAL-001, TASK-UTML-008
- **Goal:** UTML §VII.1 detection per `TECH-HEAL §6`. mean(L_step) > 0.3 + ΔK or ΔΞ degrading + ΔM/mean(L_step) < 0.2 → fire.
- **Outputs:** `heal/dissipation.rs` (NEW).
- **DoD:** synthetic dissipation pattern fires within expected window; intervention queued.

## TASK-HEAL-004 — Bayesian small-window drift posterior

- **Phase:** E · **Effort:** 0.5d · **Blocked by:** TASK-HEAL-002 · **Spec:** SPEC-HEAL REQ-HEAL-FB-03 + TECH-HEAL §5
- **Goal:** When per-cell N < 50, use Beta posterior. Don't fire on noise.
- **Outputs:** `heal/drift_bayesian.rs` (NEW).
- **DoD:** N=10 with 7/10 PASS → no fire; N=10 with 0/10 PASS → fire (catastrophic posterior).

## TASK-HEAL-005 — Constellation refresh trigger wired into scheduler

- **Phase:** E · **Effort:** 0.5d · **Blocked by:** TASK-HEAL-001
- **Goal:** Per `09 §11`. Refresh stale cells > 90 days OR > 100 new examples since last refresh OR τ_m drift > 3%.
- **Outputs:** `heal/scheduler_state.rs::tick_constellation_freshness` extension.
- **DoD:** stale cell refits within next tick; metric verified.

## TASK-HEAL-006 — `mejepa_promote_approval` MCP tool

- **Phase:** E · **Effort:** 0.5d · **Blocked by:** TASK-HEAL-001 · **Spec:** SPEC-HEAL REQ-HEAL-FB-08
- **Goal:** Operator-callable tool to approve / reject queued catastrophic A/B/C promotions.
- **Outputs:** `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs::handle_promote_approval`.
- **DoD:** queued catastrophic promotion blocks until approval; non-catastrophic auto-promotes; rejection path persists with reason.

## TASK-HEAL-007 — Emergency eviction tick

- **Phase:** E · **Effort:** 0.5d · **Blocked by:** none
- **Goal:** Per `SPEC-HEAL REQ-HEAL-FB-10`. 10-minute tick checks quota; if > 110 GB, emergency eviction.
- **Outputs:** `heal/emergency_eviction.rs` (NEW); scheduler tick wiring.
- **DoD:** synthetic 112 GB usage → eviction kicks in; quota recovers ≤ 110 GB.

## TASK-HEAL-008 — Drift attribution (D4)

- **Phase:** E · **Effort:** 0.5d · **Blocked by:** TASK-HEAL-002
- **Goal:** When drift fires, classify root cause: data shift / model staleness / spec change / per-cell distribution shift.
- **Outputs:** `heal/drift_attribution.rs` (NEW).
- **DoD:** synthetic drift events of each kind classify correctly; weekly report includes attribution histogram.
