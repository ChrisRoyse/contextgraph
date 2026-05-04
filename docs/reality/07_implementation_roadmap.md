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
