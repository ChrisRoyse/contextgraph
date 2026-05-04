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
8. Run the exported prediction through the official Docker-based SWE-bench harness.
9. Persist the official report/log readback into the same BlackBoxFix ledger.
10. If validation fails, reprompt with the verified failure evidence and fresh source-of-truth repository context.

## Current Implementation

Script: `gemmafrontend/swe_lite_refinement_loop.py`

The loop records each attempt in a BlackBoxFix SQLite ledger and exports SWE-bench-compatible predictions:

- `predictions/latest-predictions.jsonl`
- `predictions/all-attempts-predictions.jsonl`
- `predictions/predictions-manifest.json`

The official evaluation command is written into the prediction manifest.

Official Docker bridge:

- `gemmafrontend/swe_lite_official_eval.py`

The bridge requires Docker Desktop for Windows through WSL integration. It does not use a native Docker daemon inside the Ubuntu WSL distro. It verifies the Docker Desktop socket and CLI, runs `swebench.harness.run_evaluation`, reads the official report/logs from disk, and appends the official result into the task/model BlackBoxFix ledger.

## Current Environment State

Verified on 2026-05-04:

- WSL native Docker packages were removed.
- Docker Desktop is reachable from WSL through `/mnt/wsl/docker-desktop/cli-tools/usr/bin/docker`.
- `/home/cabdru/.local/bin/docker` is only a symlink to Windows Docker Desktop at `/mnt/c/Program Files/Docker/Docker/resources/bin/docker.exe`.
- Native `/usr/bin/docker` and `/usr/bin/dockerd` are absent.
- Docker socket source of truth: `/mnt/wsl/docker-desktop-bind-mounts/Ubuntu-24.04/docker.sock`.
- Docker Desktop server readback: Docker Desktop 4.71.0, Engine 29.4.1.
- Free disk after cleanup: about 233 GB on `/`, about 458 GB on `/mnt/c`.

This is enough for the small SWE-bench Lite start point. It is still not enough to assume a full broad benchmark sweep without watching Docker image/cache growth.

## Latest Verified Run: Official Docker Evidence

Smoke benchmark suite:

- Suite definition: `gemmafrontend/swe_smoke_benchmark_suite.json`
- Scope: 2 SWE-bench Lite instances, 1 model per batch, 1 worker, 1 attempt per model/task run until manually expanded.

Selected seed instances:

| Instance | Repo | Why |
|---|---|---|
| `psf__requests-2317` | `psf/requests` | Already produces a verified official failure where the patch applies but the task remains unresolved. |
| `django__django-12908` | `django/django` | Small different-repo task with 2 FAIL_TO_PASS tests and a short gold/test patch, useful for checking generalization without a large corpus. |

Three-model smoke run:

`/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777920727174268169_e7e17dfb`

Observed model outcomes on `psf__requests-2317`:

| Model | Attempt Status | Reality Readback |
|---|---|---|
| `gemma4:e4b` | `failed` | Candidate patch did not apply to the base checkout. |
| `qwen3:8b` | `blocked` | Candidate patch applied and the official test patch applied; local validation blocked because `pytest` is not installed. |
| `mistral-nemo:12b` | `failed` | Candidate patch was syntactically corrupt and did not apply. |

This is the first useful model-separating signal: the system is no longer measuring only deterministic ContextGraph behavior. It can now distinguish malformed patches, applicable patches, and environment-blocked validation.

Clean single-model Qwen proof run with official SWE-bench Docker readback:

Run root:

`/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0`

Task:

`psf__requests-2317`

Model:

`qwen3:8b`

Observed state:

- Candidate patch was generated.
- Candidate patch applied to the real checkout.
- Official SWE-bench test patch applied.
- Local validation blocked because `/usr/bin/python3` has no `pytest` installed.
- The local blocked state was persisted as `SWE_PYTEST_UNAVAILABLE`, not treated as success.
- Official Docker SWE-bench evaluation ran through Docker Desktop and completed.
- Official result: unresolved, 0 resolved, 1 unresolved, 0 errors.
- The model patch applied cleanly in the official container.
- Official FAIL_TO_PASS result: 7 passed, 1 failed.
- Failed FAIL_TO_PASS test: `test_requests.py::TestTimeout::test_encoded_methods`.
- Official test output also showed `test_requests.py::RequestsTestCase::test_conflicting_post_params` failing during full test execution.
- `FAIL_TO_PASS` now normalizes from SWE-bench JSON text into 8 real test identifiers.
- BlackBoxFix ledger contains durable rows for the model decision, local validation verifier, official Docker verifier, evidence packets, incident packets, scorecards, and reward compilations.

Source-of-truth files:

- Local summary: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/refinement-loop-summary.json`
- Ledger: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/tasks/psf__requests-2317/qwen3-8b/blackbox.sqlite`
- Local validation: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/tasks/psf__requests-2317/qwen3-8b/attempts/attempt-001/validation.json`
- Prediction export: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/predictions/latest-predictions.jsonl`
- Official eval summary: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/official-eval/contextgraph_qwen_psf_requests_2317_003/official-eval-summary.json`
- Official final report: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/official-eval/contextgraph_qwen_psf_requests_2317_003/qwen3:8b.contextgraph_qwen_psf_requests_2317_003.json`
- Official instance report: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/official-eval/contextgraph_qwen_psf_requests_2317_003/logs/run_evaluation/contextgraph_qwen_psf_requests_2317_003/qwen3:8b/psf__requests-2317/report.json`
- Official test output: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/official-eval/contextgraph_qwen_psf_requests_2317_003/logs/run_evaluation/contextgraph_qwen_psf_requests_2317_003/qwen3:8b/psf__requests-2317/test_output.txt`
- Official readback copied into task evidence: `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/tasks/psf__requests-2317/qwen3-8b/official-eval/contextgraph_qwen_psf_requests_2317_003/official-readback.json`

Ledger readback after official ingest:

| Record Kind | Count |
|---|---:|
| `agent_run` | 1 |
| `model_decision` | 1 |
| `tool_call` | 2 |
| `expected_delta` | 2 |
| `readback_snapshot` | 2 |
| `verifier_result` | 2 |
| `evidence_packet` | 2 |
| `incident_packet` | 2 |
| `scorecard` | 2 |
| `reward_compilation` | 2 |

Final score/reward readback:

- `correctness`: 0.0
- `trust_level`: `incident`
- `readback_ratio`: 1.0
- `verifier_failure_rate`: 1.0
- `verifier_blocked_rate`: 0.0
- `reward`: -3.515
- `loss`: 3.515

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

## Official SWE-bench Bridge

The local Python environment is not enough for a leaderboard claim. SWE-bench's official harness uses Docker containers to create reproducible repository environments and runs `swebench.harness.run_evaluation` against a predictions JSONL file.

Run local refinement:

```bash
python3 gemmafrontend/swe_lite_refinement_loop.py \
  --model qwen3:8b \
  --instance-id psf__requests-2317 \
  --max-tasks 1 \
  --max-attempts 1 \
  --git-timeout 600 \
  --pytest-timeout 120 \
  --model-timeout 300
```

Run official Docker evaluation and ingest the readback:

```bash
python3 gemmafrontend/swe_lite_official_eval.py \
  --run-root /tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0 \
  --run-id contextgraph_qwen_psf_requests_2317_003 \
  --max-workers 1 \
  --timeout 900
```

The bridge verifies Docker Desktop before execution and writes typed errors instead of silently skipping official evaluation.

## Edge Case Audit

Three fail-closed cases were manually simulated:

| Edge Case | Trigger | Persisted Error |
|---|---|---|
| Missing Docker Desktop socket | `--docker-socket /tmp/contextgraph-swe-lite-refinement/edgecases/not-a-docker.sock` | `SWE_OFFICIAL_DOCKER_DESKTOP_SOCKET_MISSING` |
| Empty predictions JSONL | `--predictions-path /tmp/contextgraph-swe-lite-refinement/edgecases/empty-predictions.jsonl` | `SWE_OFFICIAL_PREDICTIONS_EMPTY` |
| Invalid prediction schema | JSONL row missing `model_patch` | `SWE_OFFICIAL_PREDICTION_FIELD_MISSING` |

Source-of-truth error files:

- `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/official-eval/edge_missing_socket/official-eval-error.json`
- `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/official-eval/edge_empty_predictions/official-eval-error.json`
- `/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777929796591742919_801dcad0/official-eval/edge_invalid_prediction/official-eval-error.json`

## Next Engineering Step

The next step is to use the official Docker failure evidence, not the local `pytest` block, to build attempt 2:

1. Parse official instance report and test output into compact failure evidence.
2. Re-read the base repository and relevant files.
3. Build the next prompt from the failed official test, not from model text.
4. Generate a new patch.
5. Re-run official Docker evaluation.
6. Stop only when the official report puts the instance id in `resolved_ids`.

## Research Basis

- SWE-bench Lite: https://www.swebench.com/lite.html
- SWE-bench evaluation guide: https://www.swebench.com/SWE-bench/guides/evaluation/
- SWE-bench harness reference: https://www.swebench.com/SWE-bench/reference/harness/
