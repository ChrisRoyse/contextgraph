# Problem And Signal Reality Packets

This document records the current implementation that separates four realities in the Rust SWE-bench loop:

1. problem reality: what the benchmark task actually is;
2. attempt reality: what the model actually did;
3. reward reality: what the verified outcome scored.
4. edit-submission reality: whether the model submitted the candidate patch through the governed editor tool.

## Implementation

Source: `crates/context-graph-cli/src/bin/reality-loop.rs`.

The loop now writes `<run_root>/problem-reality-packet.json` immediately after cloning/resetting the real repository and before attempt 1. It fails closed if the repo HEAD is not the manifest `base_commit` or if the workspace is dirty.

The packet includes:

- SWE-bench dataset, instance id, repo, base commit, and manifest source;
- FAIL_TO_PASS tests and PASS_TO_PASS count;
- problem-statement hash, terms, symbols, and mentioned paths;
- git file inventory and selected production candidate files;
- explicit source/test boundary and non-claims;
- a root-cause discovery checklist the model must use before editing.

Each attempt now writes `<attempt_root>/reality-signal-packet.json`. This packet is materialized as a BlackBoxFix tool call named `rust_materialize_reality_signal_packet`, then independently verified by a BlackBoxFix verifier row.

The packet includes:

- validation status and typed failure code;
- official SWE-bench progress when an official report exists;
- normalized failure class and first error location when test output contains one;
- patch hash, patch stats, changed files, and file-existence readbacks;
- tool grounding: which ContextGraph tools were called, which files were read, and whether changed files were read before editing;
- scorecard, reward/loss, feature vector, and SQLite ledger counts;
- reinjection directive for the next prompt.

The final candidate patch is now also a governed tool artifact. The live loop no longer asks the model to finish with raw JSON text. When the model knows the fix, it must call `apply_line_window_edit` or `apply_structured_edit`. The preferred path for the inner actor is `apply_line_window_edit`: the inner actor opens a real file window, copies the returned file SHA-256, and submits an inclusive line range plus replacement text. ContextGraph compiles those edits into a candidate patch, writes the patch to the attempt tool artifact directory, reads the patch file and SHA-256 back from disk, and only then allows patch validation or official SWE-bench evaluation to proceed.

## Root Cause Fixed

Before this hardening, the loop captured many artifacts, but a model failure such as "did not call a required governed tool" or "could not satisfy the raw final JSON patch contract" could abort the entire run before a per-attempt signal packet was created. That meant the next loop did not have a durable, compact, model-facing reality signal explaining what actually failed.

Now model-contract failures are attempt reality, not infrastructure failure. The system still fails closed for infrastructure and measurement boundary errors, but model behavior failures are persisted, scored, verified, and reinjected.

The specific root cause fixed in this pass was the final handoff boundary. Local models were able to inspect source reality through governed tools, but then had to leave the tool boundary and emit a raw JSON final patch response. That put the most fragile model behavior at the exact point where the system needed the strongest source-of-truth guarantees. The final patch handoff is now tool-mediated through `apply_structured_edit`.

A second implementation bug surfaced during FSV: the Rust process wrapper captured child stdout/stderr through pipes but did not drain them until process exit. `blackbox reward compile` can emit enough JSON to fill the pipe, which blocked the child in `pipe_write` and made the parent wait until timeout. The wrapper now writes child stdout/stderr to durable temporary files, waits with timeout, then reads those files back as source-of-truth process output. This preserves error evidence and prevents reward compilation from hanging on large verified payloads.

## Blindspots Now Made Explicit

The system now records these blindspots instead of hiding them:

- dynamic coverage is unknown until targeted or official tests run;
- problem-statement ambiguity is outside the official success claim unless additional human/product checks are added;
- SWE-bench oracle narrowness is visible through FAIL_TO_PASS and PASS_TO_PASS metadata;
- a no-patch attempt has no candidate patch source of truth, so patch paths are recorded as `exists=false`;
- a changed file that was not read through tools appears in `changed_files_without_tool_read`.
- a failed structured edit is persisted as `RUST_REALITY_FINAL_EDIT_TOOL_FAILED` with the exact tool result and remediation;
- a missing final edit submission is persisted as `RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED` with the required next tool.
- tool-use quality is now captured as reward-facing reality: problem reality read, repo navigation, source-window grounding, preferred edit tool use, patch artifact submission, official eval readback, and repeated-patch penalty.

The problem packet also lists the next reality signals that should be reduced before high confidence:

- pre-patch failing-test stdout/stderr and stack location;
- dynamic coverage or execution trace for touched source spans;
- local reproduction and official Docker dependency/runtime versions;
- a minimal counterexample that distinguishes the bug from neighboring behavior;
- negative invariants proving the patch did not only satisfy the metric while breaking intent;
- flakiness and retry reality for nondeterministic failures;
- performance, memory, security, and public-API side effects when the task surface implies them;
- similar verified episodes and failed patch patterns from ContextGraph memory.

## Full State Verification Evidence

Latest FSV run:

```text
/tmp/contextgraph-tool-mediated-fsv-10/rust_reality_1777970781993552749_69ff234d/
```

Source-of-truth files:

```text
summary: file:/tmp/contextgraph-tool-mediated-fsv-10/rust_reality_1777970781993552749_69ff234d/reality-loop-summary.json
problem: file:/tmp/contextgraph-tool-mediated-fsv-10/rust_reality_1777970781993552749_69ff234d/problem-reality-packet.json
ledger: sqlite:/tmp/contextgraph-tool-mediated-fsv-10/rust_reality_1777970781993552749_69ff234d/tasks/scikit-learn__scikit-learn-25638/qwen3.6-e4b/blackbox.sqlite#ledger_records
signal: file:/tmp/contextgraph-tool-mediated-fsv-10/rust_reality_1777970781993552749_69ff234d/tasks/scikit-learn__scikit-learn-25638/qwen3.6-e4b/attempts/attempt-000001/reality-signal-packet.json
tool_transcript: file:/tmp/contextgraph-tool-mediated-fsv-10/rust_reality_1777970781993552749_69ff234d/tasks/scikit-learn__scikit-learn-25638/qwen3.6-e4b/attempts/attempt-000001/agent-tools/agent-tool-transcript.json
validation: file:/tmp/contextgraph-tool-mediated-fsv-10/rust_reality_1777970781993552749_69ff234d/tasks/scikit-learn__scikit-learn-25638/qwen3.6-e4b/attempts/attempt-000001/model-parse-report.json
```

Verified state from the latest run:

```json
{
  "status": "unsolved",
  "task": "scikit-learn__scikit-learn-25638",
  "problem_reality": {
    "fail_to_pass_total": 10,
    "pass_to_pass_count": 194
  },
  "attempt": {
    "status": "failed",
    "error_code": "RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED",
    "failure_class": "model_final_edit_tool_missing",
    "required_next_tool": "apply_structured_edit with exact find/replace edits copied from opened source windows",
    "reward": -3.365,
    "loss": 3.365
  },
  "agent_tool_transcript": {
    "status": "blocked",
    "error_code": "RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED",
    "executed_tool_calls": 4,
    "tools_used": ["read_problem_reality", "inspect_symbol", "open_file_window"]
  },
  "blackbox_ledger_counts": {
    "agent_run": 1,
    "model_decision": 1,
    "tool_call": 6,
    "expected_delta": 6,
    "readback_snapshot": 6,
    "verifier_result": 6,
    "evidence_packet": 6,
    "incident_packet": 1,
    "scorecard": 1,
    "reward_compilation": 1
  }
}
```

## Edge Cases Verified

1. Missing PASS_TO_PASS metadata fails closed:

```text
/tmp/contextgraph-reality-signal-fsv-01/failures/failure-1777968728-RUST_REALITY_TASK_PASS_TO_PASS_MISSING.json
```

2. Wrong dataset fails closed:

```text
/tmp/contextgraph-reality-edge-bad-dataset/failures/failure-1777968990-RUST_REALITY_TASK_DATASET_MISMATCH.json
```

3. Official evaluation disabled fails closed:

```text
/tmp/contextgraph-reality-edge-no-official/failures/failure-1777968990-RUST_REALITY_OFFICIAL_EVAL_REQUIRED.json
```

4. Model missed the required tool call but the loop remained operational:

```text
/tmp/contextgraph-reality-signal-fsv-04/rust_reality_1777968923321426755_69ff234d/tasks/scikit-learn__scikit-learn-25638/qwen3.6-e4b/attempts/attempt-000001/inner-harness-response.json
```

The model miss was persisted as attempt reality, scored, written to the BlackBoxFix ledger, and converted into a reality-signal packet for the next prompt.

5. Model used governed tools but failed the final JSON patch contract:

```text
/tmp/contextgraph-reality-signal-fsv-05/rust_reality_1777969093757587401_69ff234d/tasks/scikit-learn__scikit-learn-25638/qwen3.6-e4b/attempts/attempt-000001/model-parse-report.json
```

This is the older blindspot that motivated the current fix. The final raw JSON patch contract has been removed from the live path.

6. Tool-mediated edit submission works in unit FSV:

```text
cargo test -p context-graph-cli --bin reality-loop -- --nocapture
```

The `tool_mediated_final_edit_submission_fsv` test creates a real temporary git repository, runs `apply_structured_edit`, reads the candidate patch artifact and SHA-256 back from disk, applies the patch, and reads the edited source file back. The source of truth changes from `return x + 1` to `return x + 2`.

7. Missing patch source of truth fails closed:

```text
RUST_REALITY_TOOL_SUBMITTED_PATCH_SOT_MISSING
```

8. Patch SHA mismatch fails closed:

```text
RUST_REALITY_TOOL_SUBMITTED_PATCH_SHA_MISMATCH
```

9. Empty patch readback fails closed:

```text
RUST_REALITY_TOOL_SUBMITTED_PATCH_READBACK_EMPTY
```

10. Ambiguous structured edit fails closed with a specific source span error:

```text
/tmp/contextgraph-tool-mediated-fsv-09/rust_reality_1777970590995615862_69ff234d/tasks/scikit-learn__scikit-learn-25638/qwen3.6-e4b/attempts/attempt-000001/agent-tools/round-04-00-apply_structured_edit/tool-result.json
```

That run proves the model called `apply_structured_edit`, but the find block matched three locations in `sklearn/utils/multiclass.py`. The persisted error was `RUST_REALITY_STRUCTURED_EDIT_FIND_AMBIGUOUS`, with remediation to open the exact source window and submit a unique find block.

11. Missing final `apply_structured_edit` is now captured as attempt reality:

```text
/tmp/contextgraph-tool-mediated-fsv-10/rust_reality_1777970781993552749_69ff234d/tasks/scikit-learn__scikit-learn-25638/qwen3.6-e4b/attempts/attempt-000001/reality-signal-packet.json
```

That run proves the inner actor read problem/source reality through governed tools, but never submitted the candidate patch through `apply_structured_edit`. The loop did not run official SWE-bench evaluation because there was no patch artifact to evaluate. It persisted the failure, score, reward, source-of-truth paths, BlackBoxFix evidence rows, and the exact next required tool.

12. the inner actor coding-agent harness smoke with tool-use reward signals:

```text
/tmp/contextgraph-inner-actor-agent-harness-fsv-01/rust_reality_1777971780544438052_69ff234d/
```

That run used real `qwen3.6:27b`, real governed tool calling, and the real `scikit-learn__scikit-learn-25638` task. The first request persisted the optimized options:

```json
{
  "context_tokens": 32768,
  "max_new_tokens": 4096,
  "temperature": 0.0,
  "top_p": 0.9
}
```

The tool surface included `list_files`, `apply_line_window_edit`, and `apply_structured_edit`. the inner actor used `read_problem_reality` and `inspect_symbol` but did not open a file window or submit a patch. The reality packet therefore scored:

```json
{
  "used_problem_reality": true,
  "used_navigation_or_search": true,
  "opened_source_window": false,
  "used_preferred_line_window_edit": false,
  "submitted_patch_artifact": false,
  "official_eval_ran": false,
  "validation_passed": false,
  "score_unbounded": 3.0
}
```

This is the desired optimization signal: the inner actor gets credit for early reality discovery, but the next loop is explicitly pushed toward `open_file_window` and `apply_line_window_edit`.

## Research Basis

- SWE-agent's ACI work emphasizes that software agents need concise repository browsing, editing, linting, and test feedback interfaces.
- SWE-bench's official harness report is the durable success source because it writes `resolved_ids`, `unresolved_ids`, `completed_ids`, and related report fields.
- Aider's edit-format docs support using structured search/replace style edits rather than asking local models to emit raw full patches.
- the inner actor's tool-calling flow requires the application to execute model-requested tools and return the tool result back to the model.
