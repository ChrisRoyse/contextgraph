# ContextGraph SWE-bench Refinement Loop

## Goal

Use local models through ContextGraph to solve SWE-bench tasks by iterating on verified durable reality:

1. Read real SWE-bench Lite task metadata.
2. Clone the real repository and check out the exact base commit.
3. Ask a local Ollama model for a unified diff.
4. Apply the candidate patch to the real checkout.
5. Apply the official SWE-bench test patch.
6. Run validation.
7. Persist prompt, model response, patch, repository context, validation, verifier, scorecard, reward, and prediction export.
8. If validation fails, reprompt with the verified failure evidence and fresh source-of-truth repository context.

## Current Implementation

Script: `gemmafrontend/swe_lite_refinement_loop.py`

The loop records each attempt in a BlackBoxFix SQLite ledger and exports SWE-bench-compatible predictions:

- `predictions/latest-predictions.jsonl`
- `predictions/all-attempts-predictions.jsonl`
- `predictions/predictions-manifest.json`

The official evaluation command is written into the prediction manifest.

## Latest Verified Run

Three-model smoke run:

`/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777920727174268169_e7e17dfb`

Observed model outcomes on `psf__requests-2317`:

| Model | Attempt Status | Reality Readback |
|---|---|---|
| `gemma4:e4b` | `failed` | Candidate patch did not apply to the base checkout. |
| `qwen3:8b` | `blocked` | Candidate patch applied and the official test patch applied; local validation blocked because `pytest` is not installed. |
| `mistral-nemo:12b` | `failed` | Candidate patch was syntactically corrupt and did not apply. |

This is the first useful model-separating signal: the system is no longer measuring only deterministic ContextGraph behavior. It can now distinguish malformed patches, applicable patches, and environment-blocked validation.

Single-model Qwen proof run:

Run root:

`/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777920644744218513_801dcad0`

Task:

`psf__requests-2317`

Model:

`qwen3:8b`

Observed state:

- Candidate patch was generated.
- Candidate patch applied to the real checkout.
- Official SWE-bench test patch applied.
- Validation blocked because `/usr/bin/python3` has no `pytest` installed.
- The blocked state was persisted as `SWE_PYTEST_UNAVAILABLE`, not treated as success.
- `FAIL_TO_PASS` now normalizes from SWE-bench JSON text into 8 real test identifiers.
- BlackBoxFix ledger contains durable rows for run, model decision, tool call, verifier result, evidence packet, incident packet, scorecard, and reward compilation.

Source-of-truth files:

- Summary: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777920644744218513_801dcad0/refinement-loop-summary.json`
- Ledger: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777920644744218513_801dcad0/tasks/psf__requests-2317/qwen3-8b/blackbox.sqlite`
- Validation: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777920644744218513_801dcad0/tasks/psf__requests-2317/qwen3-8b/attempts/attempt-001/validation.json`
- Prediction export: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777920644744218513_801dcad0/predictions/latest-predictions.jsonl`

## Why This Matters

The important change is not that the first model solved the task. It did not.

The important change is that ContextGraph now records the exact point where reality blocks progress:

- bad patch syntax,
- patch hunk mismatch,
- official test patch mismatch,
- missing test runtime,
- test failure,
- or verified pass.

That evidence becomes the next prompt. This is the feedback loop needed for local models to improve against durable reality rather than textual plausibility.

## Official SWE-bench Requirement

The local Python environment is not enough for a leaderboard claim. SWE-bench's official harness uses Docker containers to create reproducible repository environments and runs `swebench.harness.run_evaluation` against a predictions JSONL file.

The current machine has about 8 GB free on `/`, while SWE-bench documentation describes Docker image/cache requirements that can be far larger than that. Before claiming a SWE-bench score, run the exported prediction file through the official harness on a machine or disk layout with enough space.

Command shape:

```bash
python -m swebench.harness.run_evaluation \
  --dataset_name princeton-nlp/SWE-bench_Lite \
  --predictions_path /tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777920644744218513_801dcad0/predictions/latest-predictions.jsonl \
  --max_workers 1 \
  --run_id swe_lite_refine_1777920644744218513_801dcad0
```

## Next Engineering Step

Add an official-harness validation mode that:

1. Runs the exported prediction through `swebench.harness.run_evaluation`.
2. Reads the official report/log files as the source of truth.
3. Records official pass/fail/block status in the BlackBoxFix ledger.
4. Reprompts from official Docker logs when the patch fails.

That is the path from local proof loop to a real SWE-bench scoring engine.

## Research Basis

- SWE-bench Lite: https://www.swebench.com/lite.html
- SWE-bench evaluation guide: https://www.swebench.com/SWE-bench/guides/evaluation/
- SWE-bench harness reference: https://www.swebench.com/SWE-bench/reference/harness/
