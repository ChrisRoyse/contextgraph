# ContextGraph SWE-Bench Reality Loop

## Current Owner

The SWE-bench refinement loop is now Rust-only.

Canonical binary:

```text
crates/context-graph-cli/src/bin/reality-loop.rs
```

Deleted legacy Python entry points:

```text
gemmafrontend/swe_lite_refinement_loop.py
gemmafrontend/swe_lite_official_eval.py
gemmafrontend/reality_prompt_compiler.py
```

Do not run or recreate the old Python refinement loop. The only supported SWE-bench loop path is `reality-loop`.

## Goal

Use a local model through ContextGraph to solve SWE-bench tasks by iterating on verified durable reality:

1. Read a real SWE-bench Lite task manifest.
2. Clone the real repository and check out the exact base commit.
3. Compile the prompt inside the Rust loop from task, repository context, and verified failure evidence.
4. Ask the configured local Ollama model for a structured edit or patch.
5. Apply the candidate to the real checkout.
6. Reject test, fixture, harness, verifier, reward-spec, and benchmark mutations.
7. Run the official Docker-based SWE-bench harness.
8. Read the official report, per-instance report, and test output from disk.
9. Persist prompt, model response, patch, repository context, validation, verifier, scorecard, reward, and incident evidence into BlackBoxFix.
10. Reinject compact official failure reality into the next attempt.

Success is declared only when the official report places the task instance id in `resolved_ids`.

## Current Environment State

Verified on 2026-05-04:

- WSL native Docker packages were removed.
- Docker Desktop for Windows is the Docker provider.
- Native `/usr/bin/docker` and `/usr/bin/dockerd` are absent.
- Docker Desktop is reachable from WSL through the Docker Desktop socket.
- Free disk after cleanup was enough for small SWE-bench Lite smoke runs.

The Rust loop uses the official SWE-bench Python package as an external evaluator because SWE-bench itself is Python/Docker based. That external evaluator is not the ContextGraph refinement loop.

## Rust Command

```bash
cargo build -p context-graph-cli --bin reality-loop
target/debug/reality-loop \
  --model gemma4:e4b \
  --task-manifest /tmp/contextgraph-hard-swe-sklearn-25638-task-manifest.json \
  --runtime-root /tmp/contextgraph-rust-reality-loop \
  --official-eval-each-attempt \
  --run-until-resolved
```

`--run-until-resolved` requires `--official-eval-each-attempt`. Without official Docker readback, the loop refuses to claim or chase success.

## Source Of Truth

Primary success source:

```text
official_report.resolved_ids contains <instance_id>
```

Physical readbacks:

```text
<run_root>/measurement-contract.json
<run_root>/reality-loop-summary.json
<run_root>/reality-events.jsonl
<run_root>/blackbox/blackbox.sqlite
<attempt_root>/candidate.patch
<attempt_root>/official-success-evidence.json
<attempt_root>/official-failure-evidence.json
<official_eval_root>/logs/run_evaluation/.../<instance_id>/report.json
<official_eval_root>/logs/run_evaluation/.../<instance_id>/test_output.txt
```

## What This Proves

The important claim is not that a model solved every selected task. The important claim is that ContextGraph records the exact point where reality blocks progress:

- malformed model output;
- patch hunk mismatch;
- repeated patch;
- forbidden benchmark/test mutation;
- official harness infrastructure error;
- official unresolved instance;
- failed FAIL_TO_PASS test;
- PASS_TO_PASS regression;
- verified official pass.

That evidence becomes the next prompt and the reward sample. The loop improves against durable reality instead of model confidence.

## Edge Case Audit

Required Rust fail-closed cases:

| Edge Case | Expected Result |
|---|---|
| Missing task manifest | `RUST_REALITY_TASK_MANIFEST_MISSING`; no loop success row. |
| `--run-until-resolved` without official eval | `RUST_REALITY_CONTINUOUS_REQUIRES_OFFICIAL_EVAL`; loop refuses to start. |
| Invalid task dataset or source-of-truth binding | typed manifest error; no model call. |
| Dirty checkout after reset | `RUST_REALITY_WORKSPACE_NOT_CLEAN_AFTER_RESET`; no official claim. |
| Patch touches tests or harness | forbidden mutation incident; no promotion. |
| Official unresolved | incident + negative reward + next prompt evidence. |
| Official resolved | promotion candidate only after report and ledger readback. |

## Next Engineering Step

The next implementation work belongs in `crates/context-graph-cli/src/bin/reality-loop.rs`:

1. Keep expanding the Rust governed tool surface.
2. Ensure official per-instance test status and test-output excerpts are compacted into every failed next prompt.
3. Persist the tool transcript and compact failure reality in the Rust run root.
4. Continue using BlackBoxFix as the durable evidence ledger.
