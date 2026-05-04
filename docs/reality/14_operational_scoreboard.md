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
  reality-scoreboard.md
  source-of-truth-manifest.json
```

Each file must be read back and hash checked after writing.
