# Session Report: Phase G Holdout Rotation

Date: 2026-05-13
Issue: #101 / TASK-G-005
Commit:

- 655fcb96 feat(mejepa): rotate holdout after promotions

## Summary

Implemented deterministic holdout rotation for ME-JEPA A/B/C promotions.

- `AbcPromoter` now tracks optional train/holdout split state and a `HoldoutRotationPolicy`.
- Every fourth successful promotion rotates deterministic 10% samples from train to holdout and 10% from holdout to train.
- Rotation events persist to `CF_MEJEPA_HEAL_REPORTS` under `holdout-rotation/...` keys with before/after counts, moved task IDs, full after-split IDs, seed, and successful-promotion counter.
- Rollback lookup and MCP heal status now skip only explicit holdout-rotation event keys while keeping ordinary heal-report decode failures fail-closed.
- Futurebuild docs now mark `TASK-G-005` complete and document the required rotation cadence in Phase G.

## Files Changed

- crates/context-graph-mejepa/src/heal/cf.rs
- crates/context-graph-mejepa/src/heal/promote.rs
- crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs
- crates/context-graph-mejepa/examples/phase_g_holdout_rotation_fsv.rs
- scripts/ci/phase_g_holdout_rotation_fsv.sh
- docs/futurebuild/00_INDEX.md
- docs/futurebuild/02_CURRENT_STATE_HONEST.md
- docs/futurebuild/12_IMPLEMENTATION_PHASES.md
- docs/futurebuild/13_TASK_INDEX.md

## Verification

- cargo test -p context-graph-mejepa --lib
- cargo check -p context-graph-mejepa --examples
- cargo check -p context-graph-mcp
- cargo clippy -p context-graph-mejepa --all-targets -- -D warnings
- scripts/ci/phase_g_holdout_rotation_fsv.sh

## FSV Evidence

Artifact:

- /mnt/d/contextgraph/fsv/phase-g-holdout-rotation-fsv/holdout_rotation_fsv.json

Post-commit FSV:

- all_passed: true
- build_release_sha: 655fcb96
- artifact_sha256: 78854175bb6fe6ad66c2e5f3314b206d438a7a374c066386e423122b71f250b1
- rotation_events: 1
- split_before: train 40, holdout 10
- split_after: train 37, holdout 13, rotation_index 1, total_successful_promotions 5, successful_promotions_since_rotation 1
- train_to_holdout: train-017, train-018, train-026, train-027
- holdout_to_train: holdout-002

Assertions:

- five_promotions_succeeded: true
- rotation_fired_once: true
- rotation_fired_on_fourth_promotion: true
- fifth_promotion_did_not_rotate_again: true
- train_rotated_10pct: true
- holdout_rotated_10pct: true
- new_split_readback_matches_memory: true
- rotation_persisted_to_heal_reports: true

Phase G remains non-green until the rolling 0.95 prediction-oracle correlation gate is met.
