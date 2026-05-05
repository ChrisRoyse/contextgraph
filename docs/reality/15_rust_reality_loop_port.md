# Rust Reality Loop Port

## Purpose

The reality loop is now available as a Rust-owned binary:

```text
crates/context-graph-cli/src/bin/reality-loop.rs
target/debug/reality-loop
```

The port exists because the loop is infrastructure, not a notebook experiment. It should own process execution, filesystem writes, ledger verification, GPU budget checks, and failure logging with deterministic cleanup and typed errors.

## Rust-Owned Responsibilities

The binary currently owns:

- task manifest readback from a real SWE-bench Lite task manifest;
- workspace clone/reset/clean for the target repository;
- repository context selection from real git files and grep results;
- prompt writing with SHA-256 readback;
- Ollama/Gemma `/api/chat` request construction;
- GPU VRAM readback before and after model calls using `nvidia-smi`;
- model response parse reporting;
- candidate patch write/readback;
- `git apply` validation against the exact base commit;
- benchmark-integrity guard that rejects test, fixture, and harness mutations;
- BlackBoxFix run/model/tool/verifier/score/reward records;
- SQLite ledger readbacks for record-kind counts;
- append-only reality event stream with read-after-write verification;
- typed failure artifacts under `<runtime_root>/failures/`.

The official SWE-bench evaluator remains an external Python/Docker verifier because the official harness is Python. The Rust loop invokes it only as a subprocess and treats its report files as source-of-truth readbacks.

## Source Of Truth

For a Rust run, the core source-of-truth paths are:

```text
<run_root>/reality-loop-summary.json
<run_root>/reality-events.jsonl
<run_root>/task-manifest-readback.json
<task_root>/blackbox.sqlite
<attempt_root>/prompt.txt
<attempt_root>/ollama-request.json
<attempt_root>/ollama-response.json
<attempt_root>/model-response.txt
<attempt_root>/model-parse-report.json
<attempt_root>/validation.json
<attempt_root>/attempt-summary.json
```

When official evaluation is enabled, the additional source-of-truth files are:

```text
<attempt_root>/official-prediction.jsonl
<attempt_root>/official-eval-command.json
<run_root>/official-eval/**
<attempt_root>/official-success-evidence.json
<attempt_root>/official-failure-evidence.json
```

## Latest Verified Smoke

Latest bounded Gemma smoke:

```text
/tmp/contextgraph-rust-reality-loop-smoke-v4/rust_reality_1777959982988939074_e525987d/
```

Command shape:

```bash
target/debug/reality-loop \
  --task-manifest /tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777940680618314859_801dcad0/tasks/psf__requests-2317/qwen3-8b/task-manifest.json \
  --runtime-root /tmp/contextgraph-rust-reality-loop-smoke-v4 \
  --model gemma4:e4b \
  --max-attempts 2 \
  --gpu-min-free-mb 8192 \
  --context-graph-bin target/debug/context-graph
```

Observed durable state:

```text
summary_status=unsolved
attempts_total=2
attempt_1_error=RUST_REALITY_PATCH_APPLY_FAILED
attempt_2_error=RUST_REALITY_MODEL_RESPONSE_JSON_INVALID
agent_run=1
model_decision=2
tool_call=2
verifier_result=2
incident_packet=2
reward_compilation=2
```

This is the correct result for the smoke. The model struggled, the loop did not hide it, and the failures became durable evidence for further optimization.

## Edge Cases Verified

The Rust binary fails closed and writes failure artifacts for:

```text
RUST_REALITY_TASK_MANIFEST_MISSING
RUST_REALITY_CONTINUOUS_REQUIRES_OFFICIAL_EVAL
RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED
```

Failure artifacts are stored under:

```text
/tmp/contextgraph-rust-reality-loop-tests/failures/
```

Each artifact records the input args, typed error, remediation, and source-of-truth reference.

## Root Causes Fixed During Port

The port exposed and fixed these issues:

- reward spec path resolution now uses an absolute path before invoking BlackBoxFix from a task directory;
- final summary status now distinguishes `running`, `unsolved`, and `solved`;
- repository context selection now prioritizes files named in the issue, including `requests/sessions.py`;
- model parse failures now produce `model-parse-report.json` instead of being mislabeled as a missing patch;
- patches that modify benchmark tests or harness files are rejected before any success claim.

## Current Gap

Gemma still fails this smoke task at the patch-generation layer:

- it can produce malformed raw JSON;
- it can produce corrupt unified diffs;
- it may try to edit tests unless explicitly blocked.

That is useful signal. The next optimization target is a stronger action contract: either a structured patch tool with path/hunk arguments or a two-stage loop where the model first proposes a file/function intent and ContextGraph compiles/validates the patch before applying it.

## Run Until Resolved

Continuous mode is enabled only when official SWE-bench evaluation is also enabled:

```bash
target/debug/reality-loop \
  --model gemma4:e4b \
  --task-manifest <real-task-manifest.json> \
  --official-eval-each-attempt \
  --run-until-resolved
```

The binary refuses `--run-until-resolved` without official readback because local validation is not reality success.
