# Full Stack FSV Report

**Date:** 2026-05-05  
**Branch:** `ccreality-cutover`  
**Commit verified before this report:** `0f12ca530868412ab0ffd1d81e5b802072dbed87`

## Source Of Truth Definition

For this verification pass, the source of truth is not command return values. It is the data written to and read back from:

- Runtime roots under `/tmp/cgreality-*` and `/tmp/ccreality-prod-1`.
- Attempt artifacts: `attempt-summary.json`, `reality-signal-packet.json`, `problem-reality-packet.json`, `model-parse-report.json`.
- BlackBox SQLite ledgers: `ledger_records` rows.
- Shift logs: `cgreality-shift-log/*.jsonl`.
- Hook logs: `cgreality-hook-events.jsonl`.
- Hook pre-edit state: `cgreality-hook-state/preedit/**/*.json`.
- Recommendation artifacts: `claude-code-optimizer/recommendation-turn-*.json`.
- Verification reports: `docs/ccreality/SMOKE_RESULTS_2026-05-05.md`, `docs/ccreality/FSV_RESULTS_2026-05-05.md`, `docs/ccreality/PHASE12_RESULTS_2026-05-05.md`.

## Verification Commands Run

```bash
bash -n scripts/cc/tests/smoke.sh
bash -n scripts/cc/tests/verify_phase0_8_hardening.sh
cargo test -p context-graph-cli --bin reality-loop -- --skip ignored --test-threads=1 --nocapture
RUN_STAMP=1778022423 bash scripts/cc/tests/verify_phase0_8_hardening.sh
CGREALITY_SMOKE_RUN_STAMP=1778022643 bash scripts/cc/tests/smoke.sh
FSV_RUN_ROOT=/tmp/cgreality-fullstack-fsv-1778022699 bash scripts/cc/tests/fsv.sh
cargo build --release -p context-graph-cli --bin reality-loop -p context-graph-cli --bin context-graph -p context-graph-cli --bin context-graph-cli -p context-graph-mcp --bin context-graph-mcp
git diff --check
```

## Results

| Layer | Result | Evidence |
|-------|--------|----------|
| Phase 0-8 hardening | PASS | `/tmp/ccreality-fullstack-phase0-8-hardening-1778022423.log` |
| Phase 0-8 physical FSV | PASS | `/tmp/cgreality-phase0-8-fsv-1778022423` |
| Phase 9 smoke | PASS | `/tmp/cgreality-smoke-1778022643/smoke-report.json` |
| Phase 10 full FSV | PASS, 39/39 | `/tmp/cgreality-fullstack-fsv-1778022699/fsv-master.log` |
| Release build | PASS | release binaries in `target/release/` |
| Diff whitespace check | PASS | `git diff --check` returned 0 |

## Fresh Physical Readbacks

Phase 0-8 hardening read back:

```text
recommendation_path=/tmp/cgreality-phase0-8-fsv-1778022423/smoke-1778022449/cgreality-smoke-phase0-8-fsv/smoke-attempt-1/claude-code-optimizer/recommendation-turn-01.json
ledger=/tmp/cgreality-phase0-8-fsv-1778022423/ledger.sqlite
shift_log=/tmp/cgreality-phase0-8-fsv-1778022423/cgreality-shift-log/phase0-8-fsv-1778022423.jsonl
hook_log=/tmp/cgreality-phase0-8-fsv-1778022423/cgreality-hook-events.jsonl
ledger_counts=agent_run|1, tool_call|1
```

Smoke read back:

```text
smoke_root=/tmp/cgreality-smoke-1778022643
runtime_root=/tmp/cgreality-smoke-1778022643/layer5-skill-runtime
run_id=smoke-1778022681
task_id=cgreality-smoke-phase09-layer5
recommendation=/tmp/cgreality-smoke-1778022643/layer5-skill-runtime/smoke-1778022681/cgreality-smoke-phase09-layer5/smoke-attempt-1/claude-code-optimizer/recommendation-turn-01.json
ledger=/tmp/cgreality-smoke-1778022643/layer5-skill-runtime/ledger.sqlite
ledger_counts=agent_run|1, tool_call|3
```

Full FSV read back:

```text
run_root=/tmp/cgreality-fullstack-fsv-1778022699
case_logs=/tmp/cgreality-fullstack-fsv-1778022699/logs
state_logs=/tmp/cgreality-fullstack-fsv-1778022699/cases/<case>/state.log
source_logs=/tmp/cgreality-fullstack-fsv-1778022699/cases/<case>/source-of-truth.log
PASS=39
FAIL=0
```

Phase 12 production source remains:

```text
runtime_root=/tmp/ccreality-prod-1
hardened_run=/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d
production_recommendation=/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/gemma4-e4b/attempts/attempt-000001/claude-code-optimizer/recommendation-turn-01.json
phase12_fsv=/tmp/ccreality-phase12-fsv-20260505T225350Z.log
```

## Boundary And Edge Cases Verified

The fresh full FSV suite includes:

- Stale SHA edit rejected with file unchanged: `C2_stale_sha_fails`.
- Denied `.env` path rejected without mutation: `C3_denied_path_fails`.
- Shift log entry created and read back: `C4_shift_log_entry`.
- External drift detected: `C5_external_drift_detected`.
- Malformed recommendation rejected without durable write: `C7_malformed_recommendation_rejected`.
- Command metachar bypass denied: `C10_metachar_bypass_denied`.
- Parallel edit lock enforced: `C11_parallel_mcp_lock`.
- Stale lock recovered: `C12_stale_lock_recovered`.
- Stop hook blocks without recommendation and allows after recommendation: `D8`, `D9`.
- Large artifact returns pointer instead of inline payload: `F3_large_artifact_pointer`.

## Verdict

The stack is functioning as intended after the hardening fixes. The most important property verified is fail-closed behavior: invalid tool calls, stale source views, denied paths, malformed recommendations, and invalid Python edits do not silently pass and leave physical evidence explaining what failed.

