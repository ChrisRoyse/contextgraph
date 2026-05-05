# SWE-Bench Reality Loop

## Objective

Use local models plus ContextGraph to improve on SWE-bench by turning every failed attempt into verified repair evidence for the next attempt.

This is not a claim that local models automatically reach 100%. The defensible claim is:

> ContextGraph can create the loop needed to repeatedly test against official durable reality, extract exact failures, and feed those failures back into model attempts until the official report resolves the instance or the budget is exhausted.

## Source Of Truth

For SWE-bench, the source of truth is the official Docker harness report:

```text
official_report.resolved_ids
official_report.unresolved_ids
official_report.error_ids
logs/run_evaluation/<run_id>/<model>/<instance>/report.json
logs/run_evaluation/<run_id>/<model>/<instance>/test_output.txt
```

Local validation is useful but secondary. A candidate is solved only when the official report places the instance id in `resolved_ids`.

## Current Project State

Current files:

- `crates/context-graph-cli/src/bin/reality-loop.rs`
- `gemmafrontend/swe_smoke_benchmark_suite.json`
- `docs/contextgraph_swe_bench_refinement_loop.md`

Verified current behavior:

- real SWE-bench Lite metadata is read;
- real GitHub repository is cloned;
- exact base commit is checked out;
- local model generates a patch;
- candidate patch is applied to a real workspace;
- official test patch is applied locally when possible;
- local validation outcome is persisted;
- prediction JSONL is exported;
- official Docker SWE-bench harness runs through Docker Desktop;
- official report/logs are read back from disk;
- official result is appended to the same BlackBoxFix ledger;
- scorecard and reward are recomputed after official evidence is added.

Rust-only status:

- the previous Python refinement loop, Python official-eval bridge, and Python prompt compiler were removed;
- the Rust loop is the only supported ContextGraph SWE-bench refinement path;
- the official SWE-bench harness is still an external Python/Docker evaluator because that is how SWE-bench is distributed;
- the Rust loop must keep compacting official per-instance report/test-output reality into the next prompt.

## Attempt Algorithm

```text
for task in selected_swe_bench_lite_tasks:
  for model in selected_models:
    initialize ledger and workspace
    attempt = 1
    champion = none

    while attempt <= max_attempts:
      reset workspace to base commit
      compile prompt from task + repository context + prior official evidence
      ask model for patch
      validate patch syntax and schema
      apply candidate patch
      export attempt prediction JSONL
      run official SWE-bench Docker eval
      read official report, instance report, and test output
      persist readback in BlackBoxFix
      compile reward

      if official report resolved instance:
        mark solved and stop

      if reward improved vs champion:
        store as best failed attempt

      compact failure evidence into next prompt
      attempt += 1
```

## Failure Evidence To Reinject

The next prompt should receive:

- instance id;
- original problem statement;
- exact base commit;
- previous patch SHA-256;
- previous patch diff, truncated only after hash capture;
- official `patch_successfully_applied`;
- official `resolved`;
- failed FAIL_TO_PASS tests;
- failed PASS_TO_PASS tests;
- concise traceback/assertion excerpts;
- relevant source snippets re-read from base checkout;
- reward components;
- incident packet summary;
- explicit instruction not to repeat an unchanged patch.

## Reward For SWE-Bench

Initial reward should be simple and reality-bound:

```text
reward =
  + 8.0 * official_resolved
  + 1.5 * patch_applied
  + 1.0 * fail_to_pass_pass_ratio
  + 0.5 * pass_to_pass_pass_ratio
  - 3.0 * incident_present
  - 2.0 * official_error
  - 1.0 * policy_missing
  - 0.5 * repeated_patch
```

Then:

```text
loss = -reward
```

The system should keep both scalar reward and component-level attribution.

## Benchmark Expansion Rules

Do not jump directly to a broad SWE-bench sweep.

Expand only when:

- Docker Desktop state is healthy;
- disk/cache is inspected;
- official readback exists for every run;
- failures are reinjected into prompts;
- no attempt is marked solved without official `resolved_ids`;
- repeated patch loops are detected;
- at least one instance shows reward movement across attempts.

## Why This Is Stronger Than A Normal Benchmark

Normal benchmark:

```text
model answer -> evaluator says pass/fail
```

ContextGraph benchmark:

```text
model action -> real repo mutation -> official Docker evaluation
  -> source-of-truth report -> evidence ledger -> reward/loss
  -> next attempt and training sample
```

The system measures consequences, not claims.
