# ContextGraph Three-Model Verified Outcome Benchmark Report

Generated: 2026-05-04

This report summarizes the real 3-model x 10-prompt benchmark run executed through the local ContextGraph/Gemma frontend. The benchmark does not score models by textual plausibility. It scores whether the model selected the correct governed ContextGraph tool and whether the resulting action produced durable, inspectable evidence.

## Source Of Truth

Primary benchmark source:

`/tmp/gemmafrontend-runs/prompt-matrix/prompt_matrix_1777917797017780011_e7e17dfb/prompt-matrix-summary.json`

Generated companion report:

`/tmp/gemmafrontend-runs/prompt-matrix/prompt_matrix_1777917797017780011_e7e17dfb/prompt-matrix-summary.md`

Per-row evidence sources:

- `snapshot_path`: persisted frontend snapshot JSON for each model/prompt request.
- `ledger_path`: physical BlackBoxFix SQLite ledger for the underlying run.
- `snapshot_sha256`: SHA-256 of the persisted snapshot file.
- `ledger_sha256`: SHA-256 of the SQLite ledger at the time the row was recorded.
- `blackbox_run_id`: run id used to correlate the model request, tool call, ledger, verifier rows, incident packet, reward compilation, and evidence exports.

The matrix runner verified each result by reading the persisted snapshot file and then separately reading the SQLite ledger named by the snapshot. Edge cases verified that rejected requests did not mutate the runtime index.

## Local Models Tested

The model inventory came from Ollama at `http://127.0.0.1:11434/api/tags`.

| Model | Family | Parameters | Quantization | Local Size Bytes | Digest Prefix |
|---|---:|---:|---:|---:|---|
| `gemma4:e4b` | gemma4 | 8.0B | Q4_K_M | 9608350718 | `c6eb396d...` |
| `qwen3:8b` | qwen3 | 8.2B | Q4_K_M | 5225388164 | `500a1f06...` |
| `mistral-nemo:12b` | llama | 12.2B | Q4_0 | 7071713227 | `e7e06d10...` |

## Prompt Set

Each model received the same 10 operator intents. The expected result was a native tool call to one declared ContextGraph tool.

| Prompt Id | Expected Tool | Capability Tested |
|---|---|---|
| `run_full_proof` | `run_proof_pipeline` | Create a fresh proof run with DynamicJEPA state, BlackBoxFix evidence, incidents, reward, and artifacts. |
| `inspect_latest_evidence` | `inspect_latest_run` | Read physical run files and SQLite ledger rows for the latest complete run. |
| `compile_reward` | `compile_reward_for_run` | Compile/read reward and loss from persisted scorecard evidence. |
| `explain_scorecard` | `explain_scorecard` | Explain scorecard values from source-of-truth evidence. |
| `analyze_incident` | `analyze_incident` | Explain the verifier failure and incident packet. |
| `reward_improvements` | `suggest_reward_improvements` | Derive concrete reward-improvement actions from reward components. |
| `compare_runs` | `compare_recent_runs` | Compare two real runs using persisted evidence. |
| `export_buyer_report` | `export_buyer_report` | Create buyer-facing evidence summary from durable artifacts. |
| `evaluate_policy_observe` | `evaluate_tool_policy` | Persist and read back a real `policy_decision` ledger row. |
| `build_category_pack` | `build_investor_category_pack` | Build the investor/product-category evidence pack. |

## Headline Results

| Model | Tool Choice Accuracy | Correct / Total | Average Reward | Average Latency Ms | Median Latency Ms | Min Ms | Max Ms |
|---|---:|---:|---:|---:|---:|---:|---:|
| `qwen3:8b` | 100.00% | 10 / 10 | -3.165 | 4204 | 1618.0 | 1518 | 26470 |
| `gemma4:e4b` | 100.00% | 10 / 10 | -3.165 | 5643 | 1824.5 | 1717 | 38792 |
| `mistral-nemo:12b` | 100.00% | 10 / 10 | -3.165 | 10528 | 4486.0 | 1952 | 30925 |

Derived ranking from this benchmark:

1. Tool selection: all three models tied at 100%.
2. Average latency: `qwen3:8b` was fastest, then `gemma4:e4b`, then `mistral-nemo:12b`.
3. Median latency: `qwen3:8b` was fastest, then `gemma4:e4b`, then `mistral-nemo:12b`.
4. Verified outcome reward: all three tied, because they all drove the same deterministic ContextGraph proof task and produced the same intentional incident outcome.

## What The Benchmark Actually Proves

This benchmark proves the following from persisted evidence:

- Each model can select one correct native ContextGraph tool from a multi-tool surface.
- ContextGraph rejects unsupported arguments and requires declared schemas.
- Each successful model request produces a persisted snapshot JSON file.
- Each successful model request can be correlated to a BlackBoxFix SQLite ledger.
- Each model's full proof run created durable DynamicJEPA/BlackBoxFix artifacts.
- The benchmark can detect policy evidence changing the reward: after `evaluate_tool_policy`, reward improved from `-3.365` to `-2.365`.
- Every run stayed in `incident` trust state because the demo intentionally includes one bad verifier expectation. This is expected: the system is proving that it catches and preserves mismatch evidence.

This benchmark does not prove that one model produced a better natural-language explanation than another. The current scoring target is governed tool selection plus verified system outcome.

## Reward And Loss Interpretation

Two reward/loss states appeared for every model:

| State | Count Per Model | Reward | Loss | Meaning |
|---|---:|---:|---:|---|
| Before policy evidence | 8 | -3.365 | 3.365 | The run had source-of-truth evidence and an incident, but policy evidence was still missing. |
| After policy evidence | 2 | -2.365 | 2.365 | A real `policy_decision` row was persisted and read back as `allowed`, reducing the missing-policy penalty. |

In this reward compiler, `loss = -reward` by design. Reward is the value to maximize. Loss is the equivalent scalar to minimize during downstream optimization.

## Governance And Incident Findings

For each model:

| Model | Policy Allowed Rows | Policy Missing Rows | Trust Incident Rows | Proof Incident-Caught Rows |
|---|---:|---:|---:|---:|
| `gemma4:e4b` | 2 | 8 | 10 | 10 |
| `qwen3:8b` | 2 | 8 | 10 | 10 |
| `mistral-nemo:12b` | 2 | 8 | 10 | 10 |

Interpretation:

- The policy status changed only after tools that explicitly created or consumed policy/product-category evidence.
- The incident state is not a hidden failure. It is the expected proof case: one verifier passes and one intentional verifier mismatch creates an incident packet.
- Because all three models routed the correct tools, the final outcome is mostly determined by the same deterministic ContextGraph proof harness.

## Full Prompt Result Table

| Model | Prompt | Expected Tool | Selected Tool | Correct | Latency Ms | Trust | Policy | Reward | Loss | Run Id |
|---|---|---|---|---:|---:|---|---|---:|---:|---|
| `gemma4:e4b` | `run_full_proof` | `run_proof_pipeline` | `run_proof_pipeline` | true | 38792 | incident | policy_missing | -3.365 | 3.365 | `run_ae12c3e172ac4fa09f0f53d306ddb671` |
| `gemma4:e4b` | `inspect_latest_evidence` | `inspect_latest_run` | `inspect_latest_run` | true | 1717 | incident | policy_missing | -3.365 | 3.365 | `run_ae12c3e172ac4fa09f0f53d306ddb671` |
| `gemma4:e4b` | `compile_reward` | `compile_reward_for_run` | `compile_reward_for_run` | true | 1737 | incident | policy_missing | -3.365 | 3.365 | `run_ae12c3e172ac4fa09f0f53d306ddb671` |
| `gemma4:e4b` | `explain_scorecard` | `explain_scorecard` | `explain_scorecard` | true | 1793 | incident | policy_missing | -3.365 | 3.365 | `run_ae12c3e172ac4fa09f0f53d306ddb671` |
| `gemma4:e4b` | `analyze_incident` | `analyze_incident` | `analyze_incident` | true | 2091 | incident | policy_missing | -3.365 | 3.365 | `run_ae12c3e172ac4fa09f0f53d306ddb671` |
| `gemma4:e4b` | `reward_improvements` | `suggest_reward_improvements` | `suggest_reward_improvements` | true | 1793 | incident | policy_missing | -3.365 | 3.365 | `run_ae12c3e172ac4fa09f0f53d306ddb671` |
| `gemma4:e4b` | `compare_runs` | `compare_recent_runs` | `compare_recent_runs` | true | 1856 | incident | policy_missing | -3.365 | 3.365 | `run_ae12c3e172ac4fa09f0f53d306ddb671` |
| `gemma4:e4b` | `export_buyer_report` | `export_buyer_report` | `export_buyer_report` | true | 1759 | incident | policy_missing | -3.365 | 3.365 | `run_ae12c3e172ac4fa09f0f53d306ddb671` |
| `gemma4:e4b` | `evaluate_policy_observe` | `evaluate_tool_policy` | `evaluate_tool_policy` | true | 2360 | incident | allowed | -2.365 | 2.365 | `run_ae12c3e172ac4fa09f0f53d306ddb671` |
| `gemma4:e4b` | `build_category_pack` | `build_investor_category_pack` | `build_investor_category_pack` | true | 2534 | incident | allowed | -2.365 | 2.365 | `run_ae12c3e172ac4fa09f0f53d306ddb671` |
| `qwen3:8b` | `run_full_proof` | `run_proof_pipeline` | `run_proof_pipeline` | true | 26470 | incident | policy_missing | -3.365 | 3.365 | `run_8f19cd628d53448183a6523f23b3704c` |
| `qwen3:8b` | `inspect_latest_evidence` | `inspect_latest_run` | `inspect_latest_run` | true | 1518 | incident | policy_missing | -3.365 | 3.365 | `run_8f19cd628d53448183a6523f23b3704c` |
| `qwen3:8b` | `compile_reward` | `compile_reward_for_run` | `compile_reward_for_run` | true | 1632 | incident | policy_missing | -3.365 | 3.365 | `run_8f19cd628d53448183a6523f23b3704c` |
| `qwen3:8b` | `explain_scorecard` | `explain_scorecard` | `explain_scorecard` | true | 1600 | incident | policy_missing | -3.365 | 3.365 | `run_8f19cd628d53448183a6523f23b3704c` |
| `qwen3:8b` | `analyze_incident` | `analyze_incident` | `analyze_incident` | true | 1930 | incident | policy_missing | -3.365 | 3.365 | `run_8f19cd628d53448183a6523f23b3704c` |
| `qwen3:8b` | `reward_improvements` | `suggest_reward_improvements` | `suggest_reward_improvements` | true | 1558 | incident | policy_missing | -3.365 | 3.365 | `run_8f19cd628d53448183a6523f23b3704c` |
| `qwen3:8b` | `compare_runs` | `compare_recent_runs` | `compare_recent_runs` | true | 1604 | incident | policy_missing | -3.365 | 3.365 | `run_8f19cd628d53448183a6523f23b3704c` |
| `qwen3:8b` | `export_buyer_report` | `export_buyer_report` | `export_buyer_report` | true | 1532 | incident | policy_missing | -3.365 | 3.365 | `run_8f19cd628d53448183a6523f23b3704c` |
| `qwen3:8b` | `evaluate_policy_observe` | `evaluate_tool_policy` | `evaluate_tool_policy` | true | 2050 | incident | allowed | -2.365 | 2.365 | `run_8f19cd628d53448183a6523f23b3704c` |
| `qwen3:8b` | `build_category_pack` | `build_investor_category_pack` | `build_investor_category_pack` | true | 2150 | incident | allowed | -2.365 | 2.365 | `run_8f19cd628d53448183a6523f23b3704c` |
| `mistral-nemo:12b` | `run_full_proof` | `run_proof_pipeline` | `run_proof_pipeline` | true | 30925 | incident | policy_missing | -3.365 | 3.365 | `run_3e231c4aed8549298dd1ef79240a0120` |
| `mistral-nemo:12b` | `inspect_latest_evidence` | `inspect_latest_run` | `inspect_latest_run` | true | 6080 | incident | policy_missing | -3.365 | 3.365 | `run_3e231c4aed8549298dd1ef79240a0120` |
| `mistral-nemo:12b` | `compile_reward` | `compile_reward_for_run` | `compile_reward_for_run` | true | 18214 | incident | policy_missing | -3.365 | 3.365 | `run_3e231c4aed8549298dd1ef79240a0120` |
| `mistral-nemo:12b` | `explain_scorecard` | `explain_scorecard` | `explain_scorecard` | true | 2399 | incident | policy_missing | -3.365 | 3.365 | `run_3e231c4aed8549298dd1ef79240a0120` |
| `mistral-nemo:12b` | `analyze_incident` | `analyze_incident` | `analyze_incident` | true | 2625 | incident | policy_missing | -3.365 | 3.365 | `run_3e231c4aed8549298dd1ef79240a0120` |
| `mistral-nemo:12b` | `reward_improvements` | `suggest_reward_improvements` | `suggest_reward_improvements` | true | 2479 | incident | policy_missing | -3.365 | 3.365 | `run_3e231c4aed8549298dd1ef79240a0120` |
| `mistral-nemo:12b` | `compare_runs` | `compare_recent_runs` | `compare_recent_runs` | true | 1952 | incident | policy_missing | -3.365 | 3.365 | `run_3e231c4aed8549298dd1ef79240a0120` |
| `mistral-nemo:12b` | `export_buyer_report` | `export_buyer_report` | `export_buyer_report` | true | 9151 | incident | policy_missing | -3.365 | 3.365 | `run_3e231c4aed8549298dd1ef79240a0120` |
| `mistral-nemo:12b` | `evaluate_policy_observe` | `evaluate_tool_policy` | `evaluate_tool_policy` | true | 2892 | incident | allowed | -2.365 | 2.365 | `run_3e231c4aed8549298dd1ef79240a0120` |
| `mistral-nemo:12b` | `build_category_pack` | `build_investor_category_pack` | `build_investor_category_pack` | true | 28568 | incident | allowed | -2.365 | 2.365 | `run_3e231c4aed8549298dd1ef79240a0120` |

## Full-State Verification Edge Cases

The matrix runner executed three negative cases and inspected runtime state before and after each case.

| Case | Expected Error | Actual Error | HTTP Status | Runtime State Unchanged |
|---|---|---|---:|---:|
| Empty prompt | `GEMMAFRONTEND_PROMPT_EMPTY` | `GEMMAFRONTEND_PROMPT_EMPTY` | 400 | true |
| Overlong prompt | `GEMMAFRONTEND_PROMPT_TOO_LONG` | `GEMMAFRONTEND_PROMPT_TOO_LONG` | 400 | true |
| Missing model | `GEMMAFRONTEND_OLLAMA_MODEL_MISSING` | `GEMMAFRONTEND_OLLAMA_MODEL_MISSING` | 503 | true |

Before and after all three edge cases:

- `complete_run_count`: 29
- `frontend_index_lines`: 193
- `run_count`: 30
- Runtime index source: `file:/tmp/gemmafrontend-runs/frontend-run-index.jsonl`

## Issues Found During Benchmarking

### Fixed: negative latency values

An earlier prompt matrix showed impossible negative latency values for two rows. Root cause: the matrix runner used wall-clock time, which can move backward if the system clock adjusts. The runner now uses monotonic time via `time.perf_counter()`.

Validation after fix:

- Fresh matrix rerun completed with status `ok`.
- No row had `elapsed_ms < 0`.
- `python3 -m py_compile gemmafrontend/model_prompt_matrix.py` passed.

### Already fixed: policy capability registration

Earlier runs showed `POLICY_CAPABILITY_UNKNOWN` for the demo capability. The demo capability is now registered and fresh policy-evaluation rows read back as `allowed`.

Validation evidence:

- Each model has 2 rows with `policy_compliance=allowed`.
- Each model has reward improvement from `-3.365` to `-2.365` after policy evidence exists.

## What Can Be Derived

### Model behavior

- All three local models can use the current ContextGraph native tool surface.
- All three selected the correct tool for every tested operator intent.
- No model emitted unsupported tools or unsupported tool arguments in the clean rerun.
- Qwen was fastest on this tool-routing benchmark.
- Mistral-Nemo was slowest on average and median latency in this local setup.

### System behavior

- ContextGraph is successfully acting as the action gateway between model output and durable evidence.
- The benchmark can correlate model choice to snapshots, SQLite ledgers, reward rows, policy rows, incidents, and product-category outputs.
- The same intentional incident appears across all models, which is correct for this demo: the verifier mismatch is part of the proof mechanism.
- Reward changes are driven by durable evidence changes, not by model prose.

### Benchmarking implication

The current benchmark is a verified outcome benchmark for tool-using agents. It is stronger than a text-answer benchmark for this product because the output is not just "the model said X." The output is:

`model tool choice -> ContextGraph execution -> source-of-truth readback -> evidence row -> scorecard -> reward/loss`

## Current Limits Of This Benchmark

- It does not yet grade free-form explanation quality.
- It does not yet run diverse real-world external connectors such as GitHub, Kubernetes, Postgres, Jira, or Slack.
- It does not yet distinguish model quality after the correct tool is selected, because the deterministic ContextGraph harness controls the actual proof outcome.
- It currently has one dominant task family: AgentOps proof/evidence/reward workflows.

## Recommended Next Benchmark Expansion

1. Add held-out prompts that are semantically different but map to the same tools.
2. Add deliberately ambiguous prompts and measure whether models ask for the safest verified action.
3. Add external source-of-truth connectors.
4. Score explanation faithfulness by comparing model explanations against ledger facts.
5. Add cost and token accounting per model.
6. Add a clean-success run without the intentional incident so reward can compare clean verified outcomes against incident outcomes.
7. Add model retry behavior: wrong tool choice should fail closed, then a second attempt can be scored separately.
