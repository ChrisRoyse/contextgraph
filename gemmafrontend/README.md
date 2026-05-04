# Gemma Frontend Evidence Console

This is an isolated proof-of-concept UI for demonstrating the whole
ContextGraph system:

```text
Gemma -> ContextGraph -> Evidence
```

Internally the right pane shows the Gemma tool choice, ContextGraph gateway
validation, DynamicJEPA durable state, BlackBoxFix ledger/verifiers/evidence,
correlation intelligence, and reward/loss materialization.

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

- `Run Live With Gemma`: sends the operator prompt to Gemma with multiple
  ContextGraph tools. Gemma must select exactly one native tool call. The
  gateway validates the selected tool and arguments, executes the real
  source-of-truth operation, sends verified evidence back to Gemma, and then
  persists the final UI snapshot.
- `Load Latest Real Run`: scans `/tmp/blackboxfix-agentops-demo*`, loads the
  frontend live-run directory, loads the newest completed run, and verifies
  physical files plus `blackbox.sqlite#ledger_records`.

The inspect mode is not mock data. It reads completed real run artifacts and
persists a frontend system snapshot proving what was read.

## Exposed Gemma Tools

The current gateway exposes these validated tools:

- `run_proof_pipeline`
- `inspect_latest_run`
- `compile_reward_for_run`
- `explain_scorecard`
- `analyze_incident`
- `suggest_reward_improvements`
- `compare_recent_runs`
- `export_buyer_report`
- `evaluate_tool_policy`

The full proof tool still executes the internal harness function
`run_dynamicjepa_blackboxfix_pipeline`. The difference is that the frontend
now passes a persisted prevalidated Gemma tool-call artifact to the harness, so
the harness does not call Gemma a second time with a forced single-tool prompt.

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

Prevalidated tool-call artifacts and buyer reports are also persisted when
those tools run:

```text
/tmp/gemmafrontend-runs/prevalidated-tool-calls/*.json
/tmp/gemmafrontend-runs/buyer-reports/*.md
```

## Manual FSV

Run:

```bash
python3 gemmafrontend/manual_fsv.py --run-root /tmp/blackboxfix-agentops-demo-manual-03
```

The manual FSV starts the local server, loads the real run root, reads the
SQLite ledger directly, verifies the persisted frontend snapshot, runs one
read-only Gemma multi-tool turn, and exercises three fail-closed edge cases.

## Latest Local Proof

This side project was verified with a fresh live run on the current
schema-14 binary:

```text
source_root=/tmp/gemmafrontend-runs/live-runs/blackboxfix-agentops-demo-ui-1777908087-ee96a19a
blackbox_run_id=run_ff8a255b0afc4e98b4a869437a7c878d
frontend_snapshot=/tmp/gemmafrontend-runs/frontend-runs/frontend_1777908106_f5748fa9/system-snapshot.json
manual_fsv_log=/tmp/gemmafrontend-fsv-7zm6_llv/manual-fsv.log
ui_screenshot=gemmafrontend/screenshots/multitool-ui-after.png
prevalidated_tool_call=/tmp/gemmafrontend-runs/prevalidated-tool-calls/manual-proof-2.json
buyer_report=/tmp/gemmafrontend-runs/buyer-reports/buyer-report-run_ff8a255b0afc4e98b4a869437a7c878d-1777908324.md
```

Physical ledger readback confirmed:

```text
schema_version=14
agent_run=1
tool_call=1
verifier_result=2
incident_packet=1
policy_decision=2
scorecard=3
reward_compilation=3
latest_scorecard_source=sqlite ledger latest scorecard row
latest_score.policy_compliance=denied
latest_reward.raw=-2.165
latest_reward.loss=2.165
training_sample.sample_kind=verified_outcome_reward_sample
```
