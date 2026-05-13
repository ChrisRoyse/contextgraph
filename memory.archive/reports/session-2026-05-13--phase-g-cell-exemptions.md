---
namespace: reports
agent_id: codex
timestamp: 2026-05-13T09:20:10-05:00
status: active
tags: phase-g, task-g-007, mejepa, cell-exemptions, fsv
---

# Session Report: Phase G Cell Exemptions

Date: 2026-05-13
Issue: #103 / TASK-G-007
Commit:

- 1338c0db feat(mejepa): apply ship gate cell exemptions

## Summary

Implemented active per-cell exemptions in the MCP ship-gate status path.

- `mejepa_ship_gate_status` now loads `config/cell_exemptions.toml` or the `CONTEXTGRAPH_CELL_EXEMPTIONS` override path.
- `ShipGateStabilityStatus` now computes both raw latest correlation and effective ship-gate correlation after excluding active exempt cells.
- Exempt cell failures are filtered only when the failure text names the exempt cell; non-cell failures still block the window.
- The MCP response now reports `cellExemptions`, `cellExemptionsPath`, `perCellCorrelation`, `effectiveShipGateCorrelation`, and raw/effective latest-window status.
- `config/cell_exemptions.toml` remains empty by default, with comments making active exemptions operator-approved only.
- Futurebuild ledgers mark `TASK-G-007` complete while preserving the larger Phase G warning that the real 0.95 rolling gate is still not met.

## Files Changed

- config/cell_exemptions.toml
- crates/context-graph-mejepa/src/eval/ship_gate_stability.rs
- crates/context-graph-mejepa/src/eval/mod.rs
- crates/context-graph-mcp/src/handlers/tools/mejepa_eval_tools.rs
- crates/context-graph-mcp/src/tools/definitions/mejepa.rs
- crates/context-graph-mcp/examples/phase_g_cell_exemptions_fsv.rs
- scripts/ci/phase_g_cell_exemptions_fsv.sh
- docs/futurebuild/00_INDEX.md
- docs/futurebuild/02_CURRENT_STATE_HONEST.md
- docs/futurebuild/12_IMPLEMENTATION_PHASES.md
- docs/futurebuild/13_TASK_INDEX.md

## Verification

- cargo check -p context-graph-mcp --examples
- cargo test -p context-graph-mejepa --lib
- cargo clippy -p context-graph-mejepa --all-targets -- -D warnings
- cargo clippy -p context-graph-mcp --all-targets -- -D warnings
- scripts/ci/phase_g_cell_exemptions_fsv.sh
- scripts/ci/phase_g_ship_gate_stability_fsv.sh

## FSV Evidence

Primary artifact:

- /mnt/d/contextgraph/fsv/phase-g-cell-exemptions-fsv/cell_exemptions_fsv.json

Post-commit FSV:

- all_passed: true
- build_release_sha: 1338c0db
- artifact_sha256: 9008fa231ef863b83414facddaf835ab041dfb28753f7b0cd958378593824915
- active config: `shipGatePassed=true`, `consecutivePassingWindows=4`, `effectiveShipGateCorrelation=0.9599999785423279`, raw latest-window status false, effective latest-window status true, failures empty
- empty config: `shipGatePassed=false`, `consecutivePassingWindows=0`, failure includes `compile_error::python`
- expired config: `shipGatePassed=false`, `consecutivePassingWindows=0`, failure includes `compile_error::python`
- active readback count: `CF_MEJEPA_EVAL_REPORTS=4`

Assertions:

- `active_exempt_low_cell_does_not_block`: true
- `active_exempt_cell_still_reports_correlation`: true
- `active_exemption_reason_reported`: true
- `active_exempt_failure_filtered`: true
- `empty_config_low_cell_blocks`: true
- `expired_exemption_low_cell_blocks`: true
- `active_report_rows_reopened`: true

Regression artifact:

- /mnt/d/contextgraph/fsv/phase-g-ship-gate-stability-fsv/ship_gate_stability_fsv.json
- build_release_sha: 1338c0db
- artifact_sha256: 44afcbd2bba2582dcc227834fd52659225dda38f05b2be98547edb221000b1f0
- #102 assertions remain true after the exemption-aware response fields were added.

## Failures

- First FSV run failed because the JSON assertion compared an `f32` cell correlation to literal `0.10`; fixed with a numeric tolerance. Journal: `memory/journal/2026-05-13--failure-phase-g-cell-exemptions-float-json.md`
- First test rerun after fail-closed tightening exposed a bad unit fixture that still carried a non-exempt generic failure. Fixed by making the unit fixture contain only the exempt-cell failure. Journal: `memory/journal/2026-05-13--failure-phase-g-cell-exemption-unit-fixture.md`

## Resume Here Next Session

Continue Phase G from issue #104 or the highest-priority unclaimed Phase G issue. `TASK-G-007` is closed by commit 1338c0db and the FSV evidence above. Do not infer that the human-review replacement gate is green; the real rolling holdout remains below 0.95 until measured otherwise.
