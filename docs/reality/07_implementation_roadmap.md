# Implementation Roadmap

## Phase 1: Rust-Only SWE-Bench Reality Retry Loop

Goal: make official Docker failure evidence drive attempt two and beyond through the Rust `reality-loop` binary only.

Tasks:

1. Use `crates/context-graph-cli/src/bin/reality-loop.rs` as the only ContextGraph SWE-bench loop implementation.
2. After each candidate patch, export attempt-specific predictions for the official harness.
3. Run the official SWE-bench Docker evaluator directly from the Rust loop.
4. Read the final report, instance `report.json`, and `test_output.txt`.
5. Persist compact `official-failure-evidence.json` or `official-success-evidence.json` beside the attempt.
6. Build the next prompt from official evidence, not local `pytest` blockage.
7. Stop only when official `resolved_ids` contains the instance id.
8. Persist source-of-truth paths and SHA-256 for every official artifact.

Full-state verification:

- read the official final report from disk;
- read the task BlackBoxFix SQLite ledger;
- read the attempt prompt used for the next attempt;
- prove the failed official test name appears in the next prompt;
- prove the ledger has the official verifier/evidence/incident rows.

## Phase 2: Keep Prompt Compilation In Rust

Implementation location:

```text
crates/context-graph-cli/src/bin/reality-loop.rs
```

Inputs:

- original task;
- repository context;
- failed official tests;
- assertion excerpts;
- previous patch hash;
- incident summary;
- reward components;
- model mistake history.

Outputs:

- prompt text;
- prompt manifest;
- included evidence refs;
- excluded evidence refs;
- no-repeat patch constraints.

The prompt compiler must be deterministic for the same inputs and must write/read back the prompt artifacts before a model call.

## Phase 2 Current Implementation Status

Implemented:

- Rust prompt compilation in `crates/context-graph-cli/src/bin/reality-loop.rs`;
- official failure prompt candidate compilation after SWE-bench Docker readback;
- local failure prompt candidate compilation after local patch/test readback;
- deterministic prompt templates: `swe_repair_initial_v2`, `swe_repair_local_failure_v2`, and `swe_repair_official_failure_v2`;
- compiler manifest persistence in both `<attempt_root>/prompt-manifest.json` and `<task_root>/prompt-candidates/attempt-NNN/prompt-manifest.json`;
- compiler quality gates with explicit `null` for non-applicable checks;
- source-of-truth refs for task manifest, repo context, official failure evidence, local attempt summary, previous prompt SHA, previous output SHA, and previous patch SHA;
- Rust artifacts that write and read back prompt, request, response, parse report, and quality metadata.

Root cause fixed:

Prompt construction used to be spread across separate helper functions. That made prompt versions, evidence inclusion, previous-output hashes, and quality scores difficult to audit or optimize. Phase 2 centralizes prompt generation into a deterministic compiler whose outputs are durable artifacts with quality checks and source-of-truth references.

Fail-closed behavior:

| Case | Error Code | Field |
|---|---|---|
| Missing task or repo context source of truth | Rust `RUST_REALITY_*` source-of-truth error | `repo_context.source_of_truth` |
| Unknown prompt kind | Rust prompt contract error | `prompt_kind` |
| Hidden solution/gold patch leakage | Rust prompt quality error | `prompt_quality` |
| Missing required task fields | `RUST_REALITY_TASK_*` manifest errors | `task.<field>` |

Latest verified Phase 2 smoke:

```text
/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777939166629468299_801dcad0/
```

Observed durable outputs:

- official run result: `unsolved:attempt_budget_exhausted`;
- official attempt count: `1`;
- official resolved attempts: `0`;
- root reality ledger counts: `agent_run=1`, `agent_step=16`;
- task BlackBoxFix ledger counts: `agent_run=1`, `model_decision=1`, `tool_call=2`, `verifier_result=2`, `evidence_packet=2`, `incident_packet=2`, `reward_compilation=2`, `scorecard=2`;
- second-attempt prompt candidate: `<run_root>/tasks/psf__requests-2317/qwen3-8b/prompt-candidates/attempt-002/`;
- second-attempt template: `swe_repair_official_failure_v2`;
- compiler version: Rust reality-loop prompt contract;
- prompt quality: `status=passed`, `score=1.0`;
- previous prompt SHA and previous output SHA persisted in compiler `input_refs`.

This does not claim the model solved the SWE-bench instance. It proves the loop read official durable failure evidence, recorded it, scored it, and compiled the next prompt from that evidence.

## Continuous Reality Loop Mode

Implemented only in `crates/context-graph-cli/src/bin/reality-loop.rs`:

```bash
cargo build -p context-graph-cli --bin reality-loop
target/debug/reality-loop \
  --model gemma4:e4b \
  --task-manifest /tmp/contextgraph-hard-swe-sklearn-25638-task-manifest.json \
  --official-eval-each-attempt \
  --run-until-resolved
```

Behavior:

- `--run-until-resolved` ignores the attempt budget for each selected model/task pair.
- The loop stops only when the official SWE-bench Docker report places the instance id in `resolved_ids`.
- `--run-until-resolved` requires `--official-eval-each-attempt`; local validation alone cannot terminate the loop as success.
- Model/candidate failures are not terminal. Malformed JSON, missing patch fields, patch apply failures, repeated patches, and failed tests become source-of-truth evidence for the next prompt.
- Infrastructure failures still fail closed. Ollama timeout/HTTP/JSON errors, official harness errors, missing Docker/SWE-bench artifacts, ledger errors, and compiler quality failures abort with typed errors.
- Attempt directory names and official run ids now preserve the full attempt number, including attempts above `999`.

The root cause fixed here was that the loop used a finite `for attempt in range(1, max_attempts + 1)` structure and could break on local blocked candidate state before reinjecting evidence. Continuous mode now uses a reality-driven `while` condition and treats candidate failure as feedback.

## Rust Port Current Implementation Status

Implemented in `crates/context-graph-cli/src/bin/reality-loop.rs`:

- Rust-owned SWE task loop for one local model at a time, starting with `gemma4:e4b`;
- real SWE-bench Lite task manifest input;
- real git clone/reset/clean workspace control;
- real Ollama/Gemma model calls;
- VRAM budget readbacks through `nvidia-smi`;
- prompt, request, response, parse report, validation, event, and summary artifacts with read-after-write verification;
- BlackBoxFix ledger/evidence/incident/score/reward integration;
- hard rejection of `--run-until-resolved` unless official SWE-bench readback is enabled;
- hard rejection of benchmark test or harness mutations;
- failure artifacts for startup/configuration errors.

Latest Rust smoke:

```text
/tmp/contextgraph-rust-reality-loop-smoke-v4/rust_reality_1777959982988939074_e525987d/
```

Observed result:

```text
status=unsolved
attempts_total=2
attempt_1_error=RUST_REALITY_PATCH_APPLY_FAILED
attempt_2_error=RUST_REALITY_MODEL_RESPONSE_JSON_INVALID
ledger: model_decision=2, tool_call=2, verifier_result=2, incident_packet=2, reward_compilation=2
```

The Rust loop is intentionally strict. It does not repair malformed model output by heuristic extraction. If the model violates the response contract, that violation becomes typed evidence for the next attempt.

## Phase 3: Add Experiment State

Create a durable experiment registry:

```text
<run_root>/reality-optimizer/
  experiment-registry.json
  champion-state.json
  attempts.jsonl
  prompt-compiler-runs.jsonl
  reward-series.jsonl
```

Source of truth:

- registry files;
- BlackBoxFix ledger;
- official harness output;
- artifact hashes.

## Phase 4: Add Model Comparison With Hard Tasks

Use the smoke suite first:

- `psf__requests-2317`
- `django__django-12908`

Run models one at a time:

- `gemma4:e4b`
- `qwen3:8b`
- `mistral-nemo:12b`

Metrics:

- patch parse rate;
- patch apply rate;
- official resolved rate;
- failed FAIL_TO_PASS count;
- repeated patch rate;
- incident rate;
- reward trajectory;
- tokens/time per attempt;
- source-of-truth coverage.

Do not use easy tool-choice prompts as the headline benchmark. Those prove the harness. SWE-bench proves reality repair.

## Phase 5: Add LoRA/Fine-Tune Export

Export verified episodes:

```text
<run_root>/training/
  positive.jsonl
  negative.jsonl
  preference_pairs.jsonl
  rejected_patches.jsonl
```

Positive sample:

- official resolved;
- source-of-truth coverage complete;
- no critical incidents.

Negative sample:

- official unresolved;
- failed tests known;
- patch malformed or repeated;
- verifier/incident evidence present.

Preference pair:

- same task;
- same model family or comparable context;
- higher reward attempt preferred.

## Phase 6: Add Reality Autoresearch

Create:

```text
gemmafrontend/reality_autoresearch_loop.py
```

The loop:

1. reads champion state;
2. compiles next prompt;
3. generates action;
4. executes through ContextGraph;
5. verifies source of truth;
6. compiles reward;
7. keeps/discards/repairs/escalates;
8. repeats until budget or promotion.

This is the ContextGraph equivalent of Karpathy AutoResearch, but for verified software actions instead of only `val_bpb`.

## Phase 7: Productize The Demo

Update `gemmafrontend/`:

- show the active optimization loop;
- show current champion;
- show last failed official test;
- show exact source-of-truth readback;
- show reward component deltas;
- show why the next prompt changed;
- show what the agent is forbidden to edit;
- show the promotion gate.

Investor-facing message:

> ContextGraph turns any LLM into a reality-verified optimization loop. It does not trust the model. It measures the world, scores the outcome, and feeds verified failure back into the next attempt.

## Edge Case Audit

Required edge cases:

| Case | Expected Result |
|---|---|
| Missing Docker socket | fail closed with typed error; no success row. |
| Empty predictions JSONL | fail closed before official eval. |
| Invalid prediction schema | fail closed before official eval. |
| Official unresolved | incident + negative reward + next prompt evidence. |
| Official resolved | promotion candidate only after ledger/readback verification. |
| Repeated unchanged patch | penalize and force new strategy. |
| Tampered official report | hash mismatch blocks promotion. |
| Reward spec hash mismatch | reward compilation blocked. |

## Immediate Next Action

Implement Phase 1 in code, then run one bounded smoke:

```bash
cargo build -p context-graph-cli --bin reality-loop
target/debug/reality-loop \
  --model gemma4:e4b \
  --task-manifest /tmp/contextgraph-hard-swe-sklearn-25638-task-manifest.json \
  --runtime-root /tmp/contextgraph-rust-reality-loop-smoke \
  --max-attempts 2 \
  --official-eval-each-attempt \
  --git-timeout 600 \
  --model-timeout 300
```

Success for this smoke is not solving the instance. Success is proving:

- attempt 1 official failure was read from disk;
- failure evidence was persisted;
- attempt 2 prompt includes that evidence;
- BlackBoxFix ledger contains both official verifier rows;
- the final summary reports official reality, not local model claims.

## Phase 1 Current Implementation Status

Implemented in `crates/context-graph-cli/src/bin/reality-loop.rs`:

- official per-attempt SWE-bench evaluation with `--official-eval-each-attempt`;
- governed Rust model tool gateway;
- read-only context tools such as repository search, source-window readback, official failure readback, and attempt comparison;
- mutating patch application only through the ContextGraph/BlackBoxFix verification path;
- root/meta reality ledger at `<run_root>/reality-loop.sqlite`;
- append-only root event stream at `<run_root>/reality-events.jsonl`;
- live root state at `<run_root>/reality-state.json`;
- prompt candidate artifacts at `<task_root>/prompt-candidates/attempt-NNN/`;
- multiple-reality branch registry at `<run_root>/reality-scoreboard/reality-branches.json`;
- operational scoreboard with agent tool counts and tool transcript refs.

The current root cause fixed in Phase 1 was an official SWE-bench edge case: if the candidate patch fails before the official harness emits an instance `report.json`, the loop must not crash. It now persists an `official-failure-evidence.json` with:

- `official_error_code`;
- missing official artifacts;
- official report SHA;
- official readback SHA;
- run-instance log SHA;
- patch diff SHA;
- source-of-truth refs for every physical artifact that exists.

Latest verified smoke output:

```text
/tmp/contextgraph-rust-reality-loop-smoke-v4/rust_reality_1777959982988939074_e525987d/
```

That run proves a local model submitted attempts through the Rust loop, failed without a false success claim, recorded the failures in BlackBoxFix, and persisted typed reality evidence for the next prompt.
