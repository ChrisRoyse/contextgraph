# Implementation Roadmap

## Phase 1: Finish The SWE-Bench Reality Retry Loop

Goal: make official Docker failure evidence drive attempt two and beyond.

Tasks:

1. Add an `--official-eval-each-attempt` mode to `gemmafrontend/swe_lite_refinement_loop.py`.
2. After each candidate patch, export an attempt-specific predictions JSONL.
3. Run `gemmafrontend/swe_lite_official_eval.py` for that attempt.
4. Read `official-eval-summary.json`, final report, instance `report.json`, and `test_output.txt`.
5. Persist a compact `official-failure-evidence.json` beside the attempt.
6. Build the next prompt from official evidence, not local `pytest` blockage.
7. Stop only when official `resolved_ids` contains the instance id.
8. Persist source-of-truth paths and SHA-256 for every official artifact.

Full-state verification:

- read the official final report from disk;
- read the task BlackBoxFix SQLite ledger;
- read the attempt prompt used for the next attempt;
- prove the failed official test name appears in the next prompt;
- prove the ledger has the official verifier/evidence/incident rows.

## Phase 2: Add A Prompt Compiler

Create:

```text
gemmafrontend/reality_prompt_compiler.py
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

The prompt compiler should be deterministic for the same inputs.

## Phase 2 Current Implementation Status

Implemented:

- `gemmafrontend/reality_prompt_compiler.py`;
- initial prompt compilation in `gemmafrontend/swe_lite_refinement_loop.py`;
- official failure prompt candidate compilation after SWE-bench Docker readback;
- local failure prompt candidate compilation after local patch/test readback;
- deterministic prompt templates: `swe_repair_initial_v2`, `swe_repair_local_failure_v2`, and `swe_repair_official_failure_v2`;
- compiler manifest persistence in both `<attempt_root>/prompt-manifest.json` and `<task_root>/prompt-candidates/attempt-NNN/prompt-manifest.json`;
- compiler quality gates with explicit `null` for non-applicable checks;
- source-of-truth refs for task manifest, repo context, official failure evidence, local attempt summary, previous prompt SHA, previous output SHA, and previous patch SHA;
- CLI mode that writes and reads back `prompt.txt`, `prompt-compiler-output.json`, and `prompt-quality.json`.

Root cause fixed:

Prompt construction used to be spread across separate helper functions. That made prompt versions, evidence inclusion, previous-output hashes, and quality scores difficult to audit or optimize. Phase 2 centralizes prompt generation into a deterministic compiler whose outputs are durable artifacts with quality checks and source-of-truth references.

Fail-closed behavior:

| Case | Error Code | Field |
|---|---|---|
| Missing task or repo context source of truth | `PROMPT_COMPILER_REPO_CONTEXT_SOURCE_MISSING` | `repo_context.source_of_truth` |
| Unknown prompt kind | `PROMPT_COMPILER_KIND_UNKNOWN` | `prompt_kind` |
| Hidden solution/gold patch leakage | `PROMPT_COMPILER_QUALITY_FAILED` | `prompt_quality` |
| Missing required task fields | `PROMPT_COMPILER_TASK_FIELD_MISSING` | `task.<field>` |

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
- compiler version: `reality_prompt_compiler_v1`;
- prompt quality: `status=passed`, `score=1.0`;
- previous prompt SHA and previous output SHA persisted in compiler `input_refs`.

This does not claim the model solved the SWE-bench instance. It proves the loop read official durable failure evidence, recorded it, scored it, and compiled the next prompt from that evidence.

## Continuous Reality Loop Mode

Implemented in `gemmafrontend/swe_lite_refinement_loop.py`:

```bash
python3 gemmafrontend/swe_lite_refinement_loop.py \
  --model qwen3:8b \
  --instance-id psf__requests-2317 \
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
python3 gemmafrontend/swe_lite_refinement_loop.py \
  --model qwen3:8b \
  --instance-id psf__requests-2317 \
  --max-tasks 1 \
  --max-attempts 2 \
  --official-eval-each-attempt \
  --git-timeout 600 \
  --pytest-timeout 120 \
  --model-timeout 300
```

Success for this smoke is not solving the instance. Success is proving:

- attempt 1 official failure was read from disk;
- failure evidence was persisted;
- attempt 2 prompt includes that evidence;
- BlackBoxFix ledger contains both official verifier rows;
- the final summary reports official reality, not local model claims.

## Phase 1 Current Implementation Status

Implemented in `gemmafrontend/swe_lite_refinement_loop.py`:

- official per-attempt SWE-bench evaluation with `--official-eval-each-attempt`;
- governed model tool gateway with `--agent-tool-rounds`;
- read-only tools: `list_files`, `read_file`, `search_repo`, `inspect_repository_context`, `read_failure_evidence`;
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
/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777937357771207798_801dcad0/
```

That run proves a local model used the governed `search_repo` tool, submitted a patch, failed official Docker evaluation with `SWE_OFFICIAL_PATCH_APPLY_FAILED`, recorded the failure in BlackBoxFix, and compiled the next prompt candidate from durable official evidence.
