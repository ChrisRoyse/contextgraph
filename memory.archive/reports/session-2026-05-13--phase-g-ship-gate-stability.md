---
namespace: reports
agent_id: codex
timestamp: 2026-05-13T09:01:26-05:00
status: active
tags: phase-g, task-g-006, mejepa, ship-gate, fsv
---

# Session Report: Phase G Ship-Gate Stability

Date: 2026-05-13
Issue: #102 / TASK-G-006
Commit:

- 4909aa42 feat(mejepa): require stable ship gate windows

## Summary

Implemented the four-consecutive-window stability predicate for the ME-JEPA ship gate.

- `ship_gate_stability_status` now reads `CF_MEJEPA_EVAL_REPORTS` chronologically and computes `consecutive_passing_windows` against the 0.95 correlation threshold.
- Model-promotion rows in `CF_MEJEPA_MODEL_PROMOTIONS` reset the streak to zero, so a promoted model must establish a fresh four-window stability run.
- `mejepa_ship_gate_status` now reports `consecutivePassingWindows`, `requiredConsecutivePassingWindows`, latest-window status, reset reason, model-promotion reset count, and only returns `shipGatePassed=true` when the stability streak reaches four.
- Futurebuild ledgers mark `TASK-G-006` complete while preserving the larger Phase G warning that the human-review-replacement claim remains blocked until the real rolling 0.95 gate is met.

## Files Changed

- crates/context-graph-mejepa/src/eval/ship_gate_stability.rs
- crates/context-graph-mejepa/src/eval/mod.rs
- crates/context-graph-mcp/src/handlers/tools/mejepa_eval_tools.rs
- crates/context-graph-mcp/src/handlers/tools/mod.rs
- crates/context-graph-mcp/src/tools/definitions/mejepa.rs
- crates/context-graph-mcp/examples/phase_g_ship_gate_stability_fsv.rs
- scripts/ci/phase_g_ship_gate_stability_fsv.sh
- crates/context-graph-mcp/src/daemon.rs
- crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs
- docs/futurebuild/00_INDEX.md
- docs/futurebuild/02_CURRENT_STATE_HONEST.md
- docs/futurebuild/12_IMPLEMENTATION_PHASES.md
- docs/futurebuild/13_TASK_INDEX.md

## Supporting Fixes

- Boxed the startup toolchain gate report inside `StartupToolchainGateError` so `cargo clippy -p context-graph-mcp --all-targets -- -D warnings` stays clean.
- Updated the existing heal-status test fixture to match the current promotion recalibration contract and to assert the required `active_weights` pointer instead of an exact active-pointer count.

## Verification

- cargo check -p context-graph-mcp --examples
- cargo test -p context-graph-mejepa --lib
- cargo clippy -p context-graph-mejepa --all-targets -- -D warnings
- cargo clippy -p context-graph-mcp --all-targets -- -D warnings
- scripts/ci/phase_g_ship_gate_stability_fsv.sh

## FSV Evidence

Artifact:

- /mnt/d/contextgraph/fsv/phase-g-ship-gate-stability-fsv/ship_gate_stability_fsv.json

Post-commit FSV:

- all_passed: true
- build_release_sha: 4909aa42
- artifact_sha256: 21c8780c1b338fb0428920336f8e26a5cbe1e290cadcd94b79b721e67f41d988
- after_three: `consecutivePassingWindows=3`, `shipGatePassed=false`, blocker `MEJEPA_EVAL_SHIP_GATE_STABILITY_PENDING: consecutive_passing_windows=3/4`
- after_four: `consecutivePassingWindows=4`, `shipGatePassed=true`, no failures
- after_failure: `consecutivePassingWindows=0`, `shipGatePassed=false`, reset `failing_window:2026-05-17`
- after_promotion_reset: `consecutivePassingWindows=0`, `shipGatePassed=false`, `modelPromotionResetCount=1`, reset `model_promotion:phase_e/per-cell-promotion/00000000000000008000`
- reopened_counts: `CF_MEJEPA_EVAL_REPORTS=7`, `CF_MEJEPA_MODEL_PROMOTIONS=1`

Assertions:

- `mcp_response_includes_consecutive_passing_windows`: true
- `three_passing_not_ready`: true
- `fourth_passing_ready`: true
- `failing_window_resets`: true
- `stability_blocker_returned_before_four`: true
- `model_promotion_resets`: true
- `reopened_report_rows`: true
- `reopened_model_promotion_rows`: true

## Resume Here Next Session

Continue Phase G from issue #103 or the highest-priority unclaimed Phase G issue. `TASK-G-006` is closed by commit 4909aa42 and FSV evidence above. The real system still has not met the production claim because the latest documented holdout remains below 0.95; future work must keep the distinction between implementation completion and real convergence.
