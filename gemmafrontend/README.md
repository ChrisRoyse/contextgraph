# Gemma Frontend Evidence Console

This is an isolated proof-of-concept UI for demonstrating the whole
ContextGraph system:

```text
Gemma -> ContextGraph -> Evidence
```

Internally the right pane shows the harness boundary, DynamicJEPA durable
state, BlackBoxFix ledger/verifiers/evidence, correlation intelligence, and
reward/loss materialization.

## Run

```bash
python3 gemmafrontend/server.py --host 127.0.0.1 --port 8787
```

Then open:

```text
http://127.0.0.1:8787
```

Runtime snapshots are written outside the repo by default:

```text
/tmp/gemmafrontend-runs
```

Set `GEMMAFRONTEND_RUNTIME_ROOT` or pass `--runtime-root` to change that.

## Modes

- `Run Live With Gemma`: calls `scripts/blackboxfix_gemma_tool_demo.py` with
  the local Ollama model and then reads back all generated source-of-truth
  artifacts. This requires Ollama, the configured Gemma model, and
  `target/debug/context-graph`.
- `Load Latest Real Run`: scans `/tmp/blackboxfix-agentops-demo*`, loads the
  newest completed run, and verifies physical files plus
  `blackbox.sqlite#ledger_records`.

The inspect mode is not mock data. It reads completed real run artifacts and
persists a frontend system snapshot proving what was read.

## Source Of Truth

The frontend reads:

- `harness-report.json`
- `blackbox/blackbox.sqlite#ledger_records`
- `blackbox/exports/evidence.json`
- `blackbox/exports/scorecard.json`
- `dynamicjepa/pipeline-result.json`
- `dynamicjepa/exports/dynamicjepa-episodes.jsonl`
- `provenance/artifact-provenance.json`
- `intelligence/correlation-intelligence.json`

Frontend snapshots are then persisted to:

```text
/tmp/gemmafrontend-runs/frontend-runs/*/system-snapshot.json
/tmp/gemmafrontend-runs/frontend-run-index.jsonl
```

## Manual FSV

Run:

```bash
python3 gemmafrontend/manual_fsv.py --run-root /tmp/blackboxfix-agentops-demo-manual-03
```

The manual FSV starts the local server, loads the real run root, reads the
SQLite ledger directly, verifies the persisted frontend snapshot, and exercises
three fail-closed edge cases.

## Latest Local Proof

This side project was verified with a fresh live run on the current
schema-14 binary:

```text
source_root=/tmp/gemmafrontend-runs/live-runs/blackboxfix-agentops-demo-ui-1777905483-3702752a
blackbox_run_id=run_2e1bdb675c264d02a94eac339020c48b
frontend_snapshot=/tmp/gemmafrontend-runs/frontend-runs/frontend_1777905517_8c06e4e9/system-snapshot.json
manual_fsv_log=/tmp/gemmafrontend-fsv-85ivt53n/manual-fsv.log
```

Physical ledger readback confirmed:

```text
schema_version=14
reward_compilation=1
reward.raw=-3.365
reward.loss=3.365
training_sample.sample_kind=verified_outcome_reward_sample
```
