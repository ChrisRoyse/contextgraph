# Phase 16 — Cross-Model Inner Loop (gemma4:e4b + qwen3:8b + mistral-nemo:12b)

**Goal:** the same harness drives all three local models, with a shared verified-failure memory that improves prompts without leaking unverified model claims.
**Effort:** 1–2 days.
**Risk:** low — mostly plumbing.

## Why three models

Per `docs/reality/13_path_to_100_percent_swe.md`, optimizing only one model risks overfitting the harness to that model's quirks. Three models with the same harness expose what's *generic* (good) vs what's *gemma-specific* (less good) in the optimizer's interventions.

Verified facts learned from one model also generalize: if Qwen tried patch sha256:abc and the official evaluator failed it, that is true for any model on the same task and can populate Gemma's `no-repeat-patch` list.

## Engine changes

`reality-loop attempt` accepts a `--model` CSV:
```
target/debug/reality-loop attempt \
  --task-manifest …/psf__requests-2317.json \
  --model gemma4:e4b,qwen3:8b,mistral-nemo:12b \
  --official-eval-each-attempt
```

Behavior:
- One attempt per (model, task) per CLI invocation.
- Models run **sequentially** (not in parallel) on the same GPU to keep VRAM budget enforcement simple. Future versions can parallelize across GPUs.
- Each model gets its own `<task_root>/<model>/attempt-NNN/` subtree, independently scoring and persisting.
- The engine writes one summary `<run_root>/<task>/cross-model-summary.json` aggregating per-model outcomes.

## Cross-model verified-failure memory

A new memory namespace `reality_loop_cross_model` is created in the ContextGraph memory graph.

Store rules (engine-enforced):
- A memory is created **only after** official Docker readback proves the failure.
- The memory text is structured: `{instance_id, model, attempt, patch_sha256, failed_test_names, error_classification, source_of_truth}`.
- Never store a model's reasoning or claims — only the verified failure facts.

Read rules (prompt compiler):
- When compiling a prompt for `qwen3:8b` on `psf__requests-2317`, the compiler retrieves all memories in `reality_loop_cross_model` matching `instance_id: psf__requests-2317`.
- The retrieved memories are formatted as a `CROSS_MODEL_VERIFIED_FAILURES_JSON` block in the prompt:
  ```
  {
    "previous_attempts": [
      {"model": "gemma4:e4b", "attempt": 17, "patch_sha256": "sha256:abc", "failed": ["test_redirect"], "error": "patch_apply_failure"},
      ...
    ]
  }
  ```
- The compiler also adds `cross_model.no_repeat_patch_sha256` covering all model patch attempts on this instance.

## Prompt template additions

`swe_repair_initial_v2` and `swe_repair_official_failure_v2` get an optional `<<<CROSS_MODEL_VERIFIED_FAILURES …>>>` block. Quality check: when at least one cross-model failure exists for this instance and the block is empty, fail the prompt quality gate.

## Operator changes

The operator can now run:
```bash
claude --model opus --permission-mode acceptEdits
> /loop 30m
```

…with cross-model attempts running underneath. Each /loop iteration sees the latest attempt across the model matrix. If the latest is from `qwen3:8b` and the diagnosis points at the prompt template, the harness edit applies to all three models. The recommendation must cite which models the edit affects.

## New MCP tools

| Tool | Purpose |
|------|---------|
| `cross_model_failures_for_task` | list verified failures across all models for an instance |
| `cross_model_summary` | aggregate (resolved counts, patch parse rate) per (instance × model) |

Both are read-only.

## Failure mode plan

- If only one model resolves the instance: the others receive that fact (without the patch text — only the verified outcome) in their next prompt as a "another model resolved this; here are the failed tests they removed" hint.
- If all three models repeatedly produce the same wrong patch: the harness needs a tool-surface improvement, not more reprompting. Surface to operator.

## AI-verifiable end-state for Phase 16

```bash
target/debug/reality-loop attempt \
  --task-manifest tasks/psf__requests-2317.json \
  --model gemma4:e4b,qwen3:8b,mistral-nemo:12b \
  --official-eval-each-attempt --max-attempts 1

test -s <run_root>/<task>/gemma4-e4b/attempts/attempt-000001/attempt-summary.json
test -s <run_root>/<task>/qwen3-8b/attempts/attempt-000001/attempt-summary.json
test -s <run_root>/<task>/mistral-nemo-12b/attempts/attempt-000001/attempt-summary.json
test -s <run_root>/<task>/cross-model-summary.json

mcp__cgreality__cross_model_failures_for_task instance_id=psf__requests-2317
# expected: at least one verified failure record with model field set per failed model
```

## Out of scope for Phase 16

- Parallel multi-GPU runs (later optimization).
- Training data export across models (Phase 18).
- Model-specific prompt template variants — start with one shared template; only specialize when cross-model data shows the same template hurts one model.
