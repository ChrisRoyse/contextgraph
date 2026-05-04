# Latest Correlation Intelligence Report

Source run:

```text
/tmp/blackboxfix-agentops-demo-manual-03
```

Generated artifacts:

```text
/tmp/blackboxfix-agentops-demo-manual-03/intelligence/correlation-intelligence.json
/tmp/blackboxfix-agentops-demo-manual-03/intelligence/correlation-intelligence.md
```

Generator:

```text
scripts/blackboxfix_demo_intelligence_report.py
```

## Identity Graph

```text
run_id=run_ab2eb49499cb438cbd62654a2cef7e4e
trace_id=091ade44709da5b288f0d8aead86e8f7
model=gemma4:e4b
prompt_sha256=sha256:eb6fd2adc7d21f5e7ffd117eaf68386cc952be23ace8cdc44ce0695ece4d4837
model_decision_id=decision_5aabfa5f0f9e49e8bbda11df594dd8ae
tool_call_id=tool_f1ac116a79194e1bb80a1f89ab19780d
tool_name=run_dynamicjepa_blackboxfix_pipeline
dynamic_run_id=agentops_20260504T043844Z_dfc5a3cd
plan_trace_id=9fd9a36d-b1c6-457b-af0c-28343e1d51a5
artifact_id=0542048b-0c8c-412d-b423-86ec241e1b45
training_run_id=762dd48c-c623-4af9-b0d1-6a7b46c1bbe7
surprise_event_id=bb5c53f4-3e16-4031-bd59-4c1f327a68f7
```

## Correlation Edges

```text
prompt_sha256 -> model_decision
model_decision -> tool_call
tool_call -> DynamicJEPA RocksDB state
expected_delta -> actual_readback
plan_trace -> training episode
artifact registry -> provenance graph
failed verifier -> incident packet -> scorecard
```

These edges mean the run can be followed from prompt identity all the way to
durable state, verification, incident handling, and training data.

## Calculated Intelligence

```text
source_of_truth_coverage=1.0
readback_ratio=1.0
verifier_pass_rate=0.5
verifier_failure_rate=0.5
incident_rate_per_run=1
evidence_hash_verification=passed
```

The pass/failure split is intentional because the demo includes one happy-path
verifier and one deliberately wrong expected state.

Planning:

```text
candidate_count=5
prediction_count=5
guard_decision_count=5
guard_allow_count=5
guard_allow_rate=1.0
selected_utility=2.0
max_utility=2.0
selected_highest_utility_action=true
```

Training and artifact integrity:

```text
dataset_rows_total=59
train=41
val=12
test=6
train_ratio=0.694915
val_ratio=0.20339
test_ratio=0.101695
training_runs=1
model_artifacts=1
artifact_hash_integrity_ratio=1.0
```

Dynamic state yield:

```text
raw_events=62
latent_panels=60
transitions=60
audit_log_records=316
latent_panel_yield_per_raw_event=0.967742
transition_yield_per_raw_event=0.967742
```

Surprise:

```text
surprise_events=1
surprise_rate_per_plan=1.0
latest_kind=UnexpectedOutcome
latest_error_norm=4.989360332489014
latest_cosine=0.5738434195518494
latest_threshold=0.8500000238418579
```

Training/eval yield:

```text
dynamicjepa_episode_count=4
plan_episode_count=1
edge_case_episode_count=3
episodes_sha256=sha256:0f8fbaa44f2b3707f34f38f5f234296ab02c12a8c66b63d41d95c236712c3de5
```

Governance:

```text
trust_level=incident
policy_compliance=policy_missing
risk=1.0
training_value=0.8
verification_confidence=0.9175
```

## What This Means

The system is extracting useful intelligence already:

- it can prove whether the action persisted;
- it can detect and preserve source-of-truth mismatch;
- it can lower trust when a verifier fails;
- it can connect verified operational outcomes to training/eval episodes;
- it can show planning quality, guard behavior, surprise, and artifact
  integrity from durable state.

The current run is a single example, so it proves the data model and correlation
path. Statistical claims require multiple runs across the five-demo suite.

## Current Gaps

### First-Class Model I/O Record

The ledger currently stores prompt identity as `prompt_sha256` and records the
structured tool call. It does not yet store a first-class `model_io` record with
prompt/output hashes, redacted previews, token counts, model parameters, and
optional encrypted raw content pointers.

### Policy Decision Record

The scorecard reports `policy_missing` because this run did not emit an explicit
policy decision before the tool call. The fix is to add a `policy_decision`
record with allow/deny, policy ids, governed mode, and approval context.

### Multi-Run Statistics

The current run is enough for FSV proof, but not enough for rates by prompt,
tool, scenario, connector, policy, or verifier. Run the five-scenario suite and
aggregate by the correlation keys in `23_correlation_intelligence_design.md`.
