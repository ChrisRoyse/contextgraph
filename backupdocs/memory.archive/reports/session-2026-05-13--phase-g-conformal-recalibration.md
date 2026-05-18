# Session Report: Phase G Conformal Recalibration Gate

Date: 2026-05-13
Issue: #100 / TASK-G-004
Commits:

- 35836fce feat(mejepa): recalibrate conformal promotion gate
- 0be45bfe refactor(mejepa): drop conformal_recalibration field from HealReport

## Summary

Implemented fail-closed conformal recalibration for ME-JEPA A/B/C promotion commits.

- `AbcPromoter::commit` now calls `recalibrate_conformal()` before the active weights pointer flips.
- Recalibration writes `CalibrationRecord` rows to `CF_MEJEPA_CALIBRATION_HISTORY` with `target_coverage = 0.90` and measured empirical coverage.
- Accepted promotions update `active_calibration` before `active_weights`.
- Promotions outside `[0.88, 0.92]` empirical coverage are blocked with `MEJEPA_CONFORMAL_COVERAGE_OUT_OF_BAND`.
- `CalibrationRecord` now carries explicit `target_coverage`; legacy rows without that field still decode as `target_coverage = 1 - alpha`.
- Follow-up kept the persisted `HealReport` schema stable for cross-crate callers; detailed recalibration evidence remains in `CF_MEJEPA_CALIBRATION_HISTORY` and `AbcFsvEvidence`.

## Files Changed

- crates/context-graph-mejepa/src/calibration.rs
- crates/context-graph-mejepa/src/calibration_types.rs
- crates/context-graph-mejepa/src/conformal.rs
- crates/context-graph-mejepa/src/heal/calibration.rs
- crates/context-graph-mejepa/src/heal/cf.rs
- crates/context-graph-mejepa/src/heal/errors.rs
- crates/context-graph-mejepa/src/heal/pipeline.rs
- crates/context-graph-mejepa/src/heal/promote.rs
- crates/context-graph-mejepa/examples/phase_g_conformal_recalibration_promotion_fsv.rs
- scripts/ci/phase_g_conformal_recalibration_fsv.sh
- docs/futurebuild/00_INDEX.md
- docs/futurebuild/02_CURRENT_STATE_HONEST.md
- docs/futurebuild/12_IMPLEMENTATION_PHASES.md
- docs/futurebuild/13_TASK_INDEX.md

## Verification

- cargo test -p context-graph-mejepa --lib
- cargo check -p context-graph-mejepa --examples
- cargo clippy -p context-graph-mejepa --all-targets -- -D warnings
- scripts/ci/phase_g_conformal_recalibration_fsv.sh

## FSV Evidence

Artifact:

- /mnt/d/contextgraph/fsv/phase-g-conformal-recalibration-fsv/conformal_recalibration_promotion_fsv.json

Post-commit FSV:

- all_passed: true
- build_release_sha: 0be45bfe
- artifact_sha256: 6b8ffea851c211a76a5688f8a9c213380c49a58fd3e701a0b3fee65173fe6f52
- calibration_history_count: 3

Cases:

- accepted_90pct: promoted, empirical_coverage 0.9000000357627869, active pointer changed
- accepted_91pct: promoted, empirical_coverage 0.9100000262260437, active pointer changed
- blocked_93pct: blocked with MEJEPA_CONFORMAL_COVERAGE_OUT_OF_BAND, empirical_coverage 0.9300000071525574, active pointer unchanged

Assertions:

- accepted_promotions_flipped_active_weights: true
- active_calibration_points_to_last_accepted_record: true
- active_weights_pointer_exists: true
- empirical_coverage_measured: true
- target_coverage_persisted_090: true
- third_blocked_before_active_weight_flip: true
- third_blocked_with_required_code: true
- three_calibration_results_persisted: true

Phase G remains non-green until the rolling 0.95 prediction-oracle correlation gate is met.
