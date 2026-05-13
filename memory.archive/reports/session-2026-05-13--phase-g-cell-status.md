# Phase G TASK-G-002 Cell Status Report

Date: 2026-05-13
Issue: #98
Commit: 40f8fcf3 (`feat(mejepa): add phase g cell status driver`)

## What Shipped

- Added `context-graph-cli mejepa cell-status --threshold 0.95`.
- Source of truth: latest `CF_MEJEPA_EVAL_REPORTS` row via `load_latest_report()`.
- Enumerates the required 80 Phase G cells: 8 mutation categories x 10 non-Python fixture languages.
- Per row output includes cell key, mutation category, language, correlation, report holdout count, distance to threshold, last update timestamp, and status.
- CLI exits `0` only when every required cell is present and at/above threshold; below-threshold and missing cells produce JSON output and exit `1`.
- Added synthetic source-of-truth FSV seeding real RocksDB eval reports for pass, below-threshold, and missing-cell scenarios.
- Updated futurebuild status docs to move `TASK-G-002` from the open Phase G range to complete.

## Evidence

- `cargo check -p context-graph-cli --examples` passed.
- `cargo clippy -p context-graph-cli --all-targets -- -D warnings` passed.
- `cargo test -p context-graph-cli --lib` passed: 276 tests.
- FSV script: `scripts/ci/phase_g_cell_status_fsv.sh`.
- FSV artifact: `/mnt/d/contextgraph/fsv/phase-g-cell-status-fsv/cell_status_fsv.json`.
- FSV SHA-256: `7b477763a7e47b69884bc6993076f49e9125c28ca488b876d9deb462f6217940`.
- FSV `build_release_sha`: `40f8fcf3`.
- FSV `all_passed`: `true`.

## FSV Assertions

- Seed reports read back from `CF_MEJEPA_EVAL_REPORTS`.
- Pass scenario lists all 80 cells, passes 80, fails 0, exits 0.
- Below-threshold scenario lists all 80 cells, marks `off_by_one::rust` as `below_threshold`, exits 1.
- Missing-cell scenario lists all 80 cells, marks `known_good::ruby` as `missing`, exits 1.
- Every CLI output names `CF_MEJEPA_EVAL_REPORTS` as source of truth.
