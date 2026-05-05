# Operational Scoreboard

The scoreboard tells whether the meta reality loop is actually making the inner loop better.

## Scoreboard Levels

### Level 1: Attempt Scoreboard

One row per attempt.

| Field | Meaning |
|---|---|
| `suite_id` | Which smoke suite was used. |
| `split` | dev, holdout, or promotion. |
| `model` | Local model tag. |
| `instance_id` | SWE-bench task. |
| `attempt` | Attempt number. |
| `patch_sha256` | Candidate patch identity. |
| `patch_parse_status` | Whether model output became a patch. |
| `patch_applied_official` | Official instance report patch status. |
| `official_resolved` | Official source-of-truth success. |
| `failed_fail_to_pass_count` | Remaining failing target tests. |
| `failed_pass_to_pass_count` | Regression count. |
| `reward` | Verified reward. |
| `loss` | Negative reward. |
| `incident_count` | BlackBoxFix incidents. |
| `agent_tool_rounds_total` | Governed tool rounds before final patch. |
| `agent_tool_result_count` | Physical tool-result files produced. |
| `agent_tool_names_used` | Registered tools the model actually used. |
| `agent_tool_transcript_sha256` | Hash of the durable tool transcript. |
| `prompt_template_id` | Prompt compiler version. |
| `source_of_truth_refs` | Report, ledger, and prompt manifest refs. |

### Level 2: Model-Task Scoreboard

One row per model/task pair.

| Field | Meaning |
|---|---|
| `best_attempt` | Highest reward or official resolved attempt. |
| `resolved_attempt` | First attempt that officially resolved. |
| `attempts_to_resolve` | Attempt count to success. |
| `best_reward` | Best verified reward. |
| `reward_delta` | Best minus first reward. |
| `failure_mode_initial` | First failure taxonomy. |
| `failure_mode_final` | Last failure taxonomy. |
| `repeated_patch_count` | Repeated patch attempts. |

### Level 3: Loop Artifact Scoreboard

One row per outer-loop artifact candidate.

| Field | Meaning |
|---|---|
| `artifact_kind` | prompt compiler, context selector, ACI, etc. |
| `artifact_version` | Candidate version. |
| `artifact_sha256` | File hash. |
| `baseline_version` | Previous champion. |
| `suite` | dev/holdout/promotion. |
| `resolved_delta` | Change in official solved count. |
| `reward_delta` | Change in aggregate reward. |
| `incident_delta` | Change in incident count. |
| `prompt_size_delta` | Token/char growth. |
| `decision` | keep/discard/escalate/promote. |

## Aggregate Metrics

For each suite:

- official solve rate;
- patch parse rate;
- official patch apply rate;
- average failed FAIL_TO_PASS count;
- PASS_TO_PASS regression rate;
- average reward;
- average loss;
- reward improvement per attempt;
- repeated patch rate;
- incident rate;
- agent tool result count;
- tool names used;
- source-of-truth coverage;
- prompt compiler quality score;
- wall-clock cost per verified attempt.

## Reality Score

Use a reality score for monitoring:

```text
reality_score =
  4.0 * official_resolve_rate
  + 1.0 * patch_apply_rate
  + 1.0 * fail_to_pass_reduction_rate
  + 1.0 * reward_improvement_rate
  - 1.5 * pass_to_pass_regression_rate
  - 1.0 * repeated_patch_rate
  - 1.0 * incident_rate
```

This is not the training reward. It is an operational dashboard metric for the loop builder.

## Source Of Truth

The scoreboard must be generated from:

- official final reports;
- official instance reports;
- official test outputs;
- BlackBoxFix SQLite ledgers;
- prompt manifests;
- artifact manifests;
- reward compilation rows.

Never hand-enter scoreboard rows.

## Minimum Winning Condition

For Stage 0:

```text
1 task
3 models
2 attempts
all attempts have official readback
attempt 2 prompt contains attempt 1 official failure
at least one model improves reward or failed test count
no source-of-truth coverage regression
```

For 100% smoke success:

```text
all selected model-task pairs official_resolved = true
source_of_truth_coverage = 100%
ledger evidence exists for every attempt
promotion split also passes
```

## Report Output

Generate:

```text
<run_root>/reality-scoreboard/
  attempt-scoreboard.jsonl
  model-task-scoreboard.json
  artifact-scoreboard.json
  reality-branches.json
  reality-scoreboard.md
  source-of-truth-manifest.json
```

Each file must be read back and hash checked after writing.

## Continuous Mode Fields

The root summary now records:

- `run_until_resolved`;
- `attempt_limit`;
- `official_eval.enabled_each_attempt`;
- root `reality_loop.ledger.physical_counts`;
- root `reality_loop.event_log`;
- per-task `terminal_reason`.

For `--run-until-resolved`, `attempt_limit` is `until_official_resolved`. A run is not fully successful until every selected model/task result is solved by source-of-truth readback.

Interpretation:

- `status=solved` on every selected task/model pair means the selected suite is complete.
- `terminal_reason=official_resolved` means the official SWE-bench `resolved_ids` contained the instance id.
- any `root_loop_failed` event means infrastructure/source-of-truth failure, not model failure.
- candidate failures should appear as future prompt evidence, not as loop termination.

## Current Scoreboard Source Of Truth

The Phase 1 smoke run produced the current shape here:

```text
/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777937558604930525_801dcad0/reality-scoreboard/
```

The latest official tool-enabled smoke run produced the complete official evidence here:

```text
/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777937357771207798_801dcad0/
```

The important observed values were:

- agent tool used: `search_repo`;
- official attempts: `1`;
- official resolved attempts: `0`;
- official error: `SWE_OFFICIAL_PATCH_APPLY_FAILED`;
- root reality ledger rows: `agent_run=1`, `agent_step=16`;
- task ledger rows include `tool_call=2`, `verifier_result=2`, `incident_packet=2`, `reward_compilation=2`;
- prompt-candidate branch exists for attempt 2.

## Current Rust Scoreboard Source Of Truth

The latest Rust-owned Gemma smoke produced:

```text
/tmp/contextgraph-rust-reality-loop-smoke-v4/rust_reality_1777959982988939074_e525987d/
```

Observed values from `reality-loop-summary.json` and the BlackBoxFix SQLite ledger:

- model: `gemma4:e4b`;
- instance: `psf__requests-2317`;
- status: `unsolved`;
- attempts: `2`;
- attempt 1 failure: `RUST_REALITY_PATCH_APPLY_FAILED`;
- attempt 2 failure: `RUST_REALITY_MODEL_RESPONSE_JSON_INVALID`;
- ledger rows: `agent_run=1`, `model_decision=2`, `tool_call=2`, `verifier_result=2`, `evidence_packet=2`, `incident_packet=2`, `scorecard=2`, `reward_compilation=2`;
- GPU budget current readback: `nvidia-smi:query-gpu+query-compute-apps`;
- summary source of truth: `file:/tmp/contextgraph-rust-reality-loop-smoke-v4/rust_reality_1777959982988939074_e525987d/reality-loop-summary.json`;
- ledger source of truth: `sqlite:/tmp/contextgraph-rust-reality-loop-smoke-v4/rust_reality_1777959982988939074_e525987d/tasks/psf__requests-2317/gemma4-e4b/blackbox.sqlite#ledger_records`.

This row should be treated as a real failed benchmark attempt, not a demo success. Its value is that it proves the loop catches invalid patch generation and carries failure evidence forward.
