---
namespace: reports
agent_id: agent-codex
created: 2026-05-13T13:20:00-05:00
updated: 2026-05-13T13:20:00-05:00
status: active
tags: phase-g,promotion,per-cell,fsv,github-112
---

# Session Report: 2026-05-13 — Phase G Per-Cell Promotion Gate

## Task: Complete TASK-G-016 / #112 per-cell A/B/C promotion gate
## Outcome: complete

## Summary
Implemented the live per-cell hold-or-improve gate in the Phase G A/B/C promotion path. `HoldoutEval` now carries per-cell correlations, `HoldoutExample` carries the source cell key, and `AbcPromoter::evaluate` plus the internal winner selection reject candidates whose per-cell correlation regresses more than `0.005` from Mode A.

The gate returns `MEJEPA_PROMOTION_CELL_REGRESSION` with cell-level before/after evidence, treats missing after-cells as fail-closed regressions, allows exact-tolerance cases, and can fall back to a non-regressing Mode C candidate when Mode B has the best aggregate score but regresses a cell. Committed as `2704ba3f`.

## Files Created or Modified in the Repo
- `crates/context-graph-mejepa/src/heal/promote.rs` — per-cell holdout metrics and promotion-gate enforcement.
- `crates/context-graph-mejepa/src/heal/errors.rs` — `MEJEPA_PROMOTION_CELL_REGRESSION` error payload.
- `crates/context-graph-mejepa/src/heal/pipeline.rs` — drift-window holdout examples now carry a global cell key.
- `crates/context-graph-mejepa/examples/phase_g_per_cell_promotion_gate_fsv.rs` — TASK-G-016 FSV harness.
- `scripts/ci/phase_g_per_cell_promotion_gate_fsv.sh` — FSV runner.
- `docs/futurebuild/{00_INDEX.md,02_CURRENT_STATE_HONEST.md,12_IMPLEMENTATION_PHASES.md,13_TASK_INDEX.md}` — Phase G ledger updates.

## Files Written or Updated in ./memory/
- `reports/session-2026-05-13--phase-g-per-cell-promotion-gate.md` — this report.

## Decisions Made
- **Approach chosen**: put per-cell correlation on `HoldoutEval` and gate the actual A/B/C winner path.
- **Considered alternatives**:
  1. Keep only the existing `heal::per_cell_promotion` reporting helper — rejected because it did not block `AbcPromoter` promotions.
  2. Gate only final persisted eval reports — rejected because promotion must fail before active weights flip.
- **Why this one wins**: fewest moving parts and the gate now runs in the same path that chooses promoted weights.
- **What changes the decision**: if holdout examples gain first-class `(mutation_category, language)` fields from drift samples, replace the global fallback cell with exact corpus cells.

## Discoveries
- Existing Phase E per-cell promotion code persisted a decision record, but `choose_winner` still used aggregate coverage/agreement/OOD only. The reporting module was not the promotion gate.

## Failures
- A progress comment on #112 initially used shell-interpreted backticks, so inline code identifiers were stripped before posting. Posted a correction comment with the exact symbols.

## Successes
- Post-commit FSV passed with `build_release_sha=2704ba3f`, `all_passed=true`, `readback_equal=true`, one happy path, and three boundary cases.

## Open Blockers
- None for #112.

## Tests / Build
- Tests added: `promotion_evaluate_blocks_one_cell_regression`, `promotion_evaluate_allows_exact_tolerance_floor`, `promotion_evaluate_falls_back_to_non_regressing_candidate`, plus FSV harness boundary cases.
- Tests passing: yes (`cargo test -p context-graph-mejepa`: 154 lib tests, 16 integration tests, 0 doc tests).
- Build status: passing (`cargo build --release -p context-graph-mejepa`).
- Clippy: clean (`cargo clippy -p context-graph-mejepa --no-deps --tests --examples -- -D warnings`).

## FSV evidence
- `/mnt/d/contextgraph/fsv/phase-g-per-cell-promotion-gate-fsv/per_cell_promotion_gate_fsv.json` — sha256:961bcd717d7431698b29141854e106abf4606edec12201f5ca54aa354e14975a

## GitHub issues touched
- #112 — claimed/commented/closed — final comment pending at report write time.

## Resume Here Next Session
- Start by reading: the open Phase G queue; #113 is the next sequential open task unless a higher-priority issue appears.
- Watch out for: current drift-window holdout construction uses `global_or_per_cell` because `DriftSample` has no first-class cell key yet.
- Priority: continue closing Phase G issues toward the 0.95 rolling-window convergence gate.
- Open question: whether future drift samples should carry exact mutation-category/language cells so live promotions always have true per-cell keys.

## Confidence: high
The promotion gate is enforced in the live winner path, not just in a reporting helper, and the FSV artifact proves block/allow/fallback behavior by file readback.
