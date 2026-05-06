# Evidence Map

This file maps claims to physical artifacts.

## Fresh Hardening Pass

| Claim | Source Of Truth |
|-------|-----------------|
| Phase 0-8 hardening passed | `/tmp/ccreality-fullstack-phase0-8-hardening-1778022423.log` |
| Phase 0-8 physical FSV wrote recommendation | `/tmp/cgreality-phase0-8-fsv-1778022423/smoke-1778022449/cgreality-smoke-phase0-8-fsv/smoke-attempt-1/claude-code-optimizer/recommendation-turn-01.json` |
| Phase 0-8 physical FSV ledger rows exist | `/tmp/cgreality-phase0-8-fsv-1778022423/ledger.sqlite#ledger_records` |
| Phase 0-8 shift log exists | `/tmp/cgreality-phase0-8-fsv-1778022423/cgreality-shift-log/phase0-8-fsv-1778022423.jsonl` |
| Phase 9 smoke passed | `/tmp/cgreality-smoke-1778022643/smoke-report.json` |
| Smoke recommendation exists | `/tmp/cgreality-smoke-1778022643/layer5-skill-runtime/smoke-1778022681/cgreality-smoke-phase09-layer5/smoke-attempt-1/claude-code-optimizer/recommendation-turn-01.json` |
| Smoke ledger rows exist | `/tmp/cgreality-smoke-1778022643/layer5-skill-runtime/ledger.sqlite#ledger_records` |
| Full FSV passed 39/39 | `/tmp/cgreality-fullstack-fsv-1778022699/fsv-master.log` and `docs/ccreality/FSV_RESULTS_2026-05-05.md` |
| Full FSV case logs exist | `/tmp/cgreality-fullstack-fsv-1778022699/logs` |
| Full FSV state/source logs exist | `/tmp/cgreality-fullstack-fsv-1778022699/cases/<case>/state.log` and `source-of-truth.log` |
| Release binaries build | `target/release/reality-loop`, `target/release/context-graph`, `target/release/context-graph-cli`, `target/release/context-graph-mcp` |

## Production Phase 12 Evidence

| Claim | Source Of Truth |
|-------|-----------------|
| Real SWE-bench Lite task manifest | `tasks/swebench-lite/psf__requests-2317.json` |
| Production runtime root | `/tmp/ccreality-prod-1` |
| Hardened production run | `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d` |
| Attempt summary | `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/gemma4-e4b/attempts/attempt-000001/attempt-summary.json` |
| Python syntax probe | `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/gemma4-e4b/attempts/attempt-000001/agent-tools/round-05-00-apply_line_window_edit/line-window-edit-python-py-compile.json` |
| Workspace restore | `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/gemma4-e4b/attempts/attempt-000001/agent-tools/round-05-00-apply_line_window_edit/line-window-edit-workspace-restore.json` |
| Production recommendation | `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/gemma4-e4b/attempts/attempt-000001/claude-code-optimizer/recommendation-turn-01.json` |
| Production BlackBox ledger | `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/gemma4-e4b/blackbox.sqlite#ledger_records` |
| Phase 12 FSV log | `/tmp/ccreality-phase12-fsv-20260505T225350Z.log` |
| Phase 12 report | `docs/ccreality/PHASE12_RESULTS_2026-05-05.md` |

## Docs Updated

- `docs/ccreality/SMOKE_RESULTS_2026-05-05.md`
- `docs/ccreality/FSV_RESULTS_2026-05-05.md`
- `docs/report-analysis/*.md`

## Code Hardened

- `scripts/cc/tests/smoke.sh`
- `scripts/cc/tests/verify_phase0_8_hardening.sh`
- `crates/context-graph-cli/src/governed_edit.rs`

