# ContextGraph Model Evidence Console

This is an isolated proof-of-concept UI for demonstrating the whole
ContextGraph system:

```text
Model -> ContextGraph -> Evidence
```

Internally the right pane shows the selected model's tool choice, ContextGraph gateway
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

The UI has a model selector populated from Ollama `/api/tags`. Current local
benchmark models are `gemma4:e4b`, `qwen3:8b`, and `mistral-nemo:12b`.

The left pane intentionally does not contain preset agent prompts. It shows a
non-clickable prompt-hint guide sourced from `/api/tools`, which is generated
from the same backend tool definitions sent to the model. Type your own prompt
in the composer to test whether the selected model chooses the correct tool.

Runtime snapshots are written outside the repo by default:

```text
/tmp/gemmafrontend-runs
```

Set `GEMMAFRONTEND_RUNTIME_ROOT` or pass `--runtime-root` to change that.

## Modes

- `Run Live With Model`: sends the operator prompt to the selected local
  Ollama model with multiple ContextGraph tools. The model must select exactly
  one native tool call. The
  gateway validates the selected tool and arguments, executes the real
  source-of-truth operation, sends verified evidence back to the model, and
  then persists the final UI snapshot.
- `Load Latest Real Run`: scans `/tmp/blackboxfix-agentops-demo*`, loads the
  frontend live-run directory, loads the newest completed run, and verifies
  physical files plus `blackbox.sqlite#ledger_records`.
- `GET /api/tools`: returns the current tool contract for the hint panel. This
  is read-only and exists so the UI can show the real available tool surface
  without maintaining separate prompt presets.

The inspect mode is not mock data. It reads completed real run artifacts and
persists a frontend system snapshot proving what was read.

## Exposed Model Tools

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
- `build_investor_category_pack`

The full proof tool still executes the internal harness function
`run_dynamicjepa_blackboxfix_pipeline`. The difference is that the frontend
now passes a persisted prevalidated model tool-call artifact to the harness, so
the harness does not call the model a second time with a forced single-tool
prompt.

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
/tmp/gemmafrontend-runs/product-category/*/product-category-pack.json
```

## Product Category Pack

The frontend now includes an investor-facing product category layer:

```text
Verified Outcome Intelligence for Tool-Using AI Agents
```

Use the `Investor pack` prompt chip or `Build Investor Pack` button. The selected model
selects `build_investor_category_pack`, ContextGraph validates the arguments,
then the server builds a real pack from completed run roots.

Pack artifacts include:

- `model-comparison-arena.json`
- `verified-outcome-leaderboard.json`
- `failure-injection-studio.json`
- `any-llm-connector-contracts.json`
- `governance-evidence-map.json`
- `reward-compiler-dashboard.json`
- `self-improvement-loop-demo.json`
- `executive-category-view.json`
- `attestation-manifest.json`
- `signed-attestation.json`
- `attestation-tamper-test.json`

The pack builder signs the manifest with OpenSSL and verifies it immediately.
It also writes a modified manifest and proves the original signature rejects
the tampered file.

## Manual FSV

Run:

```bash
python3 gemmafrontend/manual_fsv.py --run-root /tmp/blackboxfix-agentops-demo-manual-03
```

The manual FSV starts the local server, loads the real run root, reads the
SQLite ledger directly, verifies the persisted frontend snapshot, runs one
read-only multi-tool turn, builds a product category pack, checks every
pack artifact hash, verifies the signed attestation, proves tamper detection,
and exercises fail-closed edge cases.

## Multi-Model Benchmark

The model benchmark runner executes the same verified outcome task against
multiple local Ollama models:

```bash
python3 gemmafrontend/model_benchmark.py \
  --base-url http://127.0.0.1:8787 \
  --model gemma4:e4b \
  --model qwen3:8b \
  --model mistral-nemo:12b
```

The benchmark is not a text-answer benchmark. Each model must emit a native
tool call, ContextGraph must run the real proof pipeline, and the script reads
the persisted snapshot plus `blackbox.sqlite#ledger_records` back from disk.

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
latest_product_pack=/tmp/gemmafrontend-runs/product-category/category_1777909648987202052_17b480eee5/product-category-pack.json
latest_product_screenshot=gemmafrontend/screenshots/product-category-ui-after.png
latest_product_fsv_log=/tmp/gemmafrontend-fsv-2igz1vz9/manual-fsv.log
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

Latest product category readback confirmed:

```text
pack_id=category_1777909648987202052_17b480eee5
input_runs=2
models_measured=1
actions_attempted=2
incidents_caught=2
evidence_packets=4
training_episodes=8
average_reward=-2.765
attestation_status=verified
tamper_status=tamper_detected
```

## Latest Multi-Model Benchmark

Source-of-truth summary:

```text
/tmp/gemmafrontend-runs/model-benchmarks/model_benchmark_1777911308159691625_e7e17dfb/model-benchmark-summary.json
```

Models installed and tested through Ollama:

```text
gemma4:e4b          8.0B   Q4_K_M   9.6 GB
qwen3:8b            8.2B   Q4_K_M   5.2 GB
mistral-nemo:12b    12.2B  Q4_0     7.1 GB
```

Full proof readback:

```text
gemma4:e4b          run_6678fc42a5ce4a34bcd12f6174496fa2  tool=run_proof_pipeline  reward=-3.365  loss=3.365
qwen3:8b            run_5e6c4b3f4fc64bd49c749812fc394727  tool=run_proof_pipeline  reward=-3.365  loss=3.365
mistral-nemo:12b    run_da781447ad334e39ae7c35e84ab92bf6  tool=run_proof_pipeline  reward=-3.365  loss=3.365
```

Each ledger read back as schema `14` with:

```text
agent_run=1
model_decision=1
tool_call=1
verifier_result=2
evidence_packet=2
incident_packet=1
scorecard=1
full_state_verification=1
reward_compilation=1
```

The identical reward is expected for this first benchmark because every model
ran the same deterministic proof scenario, including the intentional failed
verifier that proves incident detection. The useful first comparison is native
tool-call compliance and end-to-end verified execution; differentiated task
suites can now be added on top.
