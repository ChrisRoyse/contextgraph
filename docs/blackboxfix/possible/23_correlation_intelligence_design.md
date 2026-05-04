# Correlation Intelligence Design

This document captures what the demo data can teach us beyond "the command
ran." The goal is to turn verified operation records into intelligence about
model behavior, tool behavior, source-of-truth reliability, risk, and training
value.

## Implemented Artifact

The current generator is:

```text
scripts/blackboxfix_demo_intelligence_report.py
```

Latest generated outputs:

```text
/tmp/blackboxfix-agentops-demo-manual-03/intelligence/correlation-intelligence.json
/tmp/blackboxfix-agentops-demo-manual-03/intelligence/correlation-intelligence.md
```

The report reads real artifacts from the run directory:

- BlackBoxFix SQLite ledger;
- BlackBoxFix evidence, dashboard, and scorecard exports;
- DynamicJEPA episode export and manifest;
- DynamicJEPA run summary and RocksDB readback data from the harness report;
- artifact provenance JSON.

It fails closed with typed errors if required artifacts or fields are missing.

## Current Correlation Keys

Use these ids as first-class join keys:

```text
run_id
trace_id / traceparent
step_id
model_decision_id
tool_call_id
expected_delta_id
readback_snapshot_id
verifier_result_id
evidence_packet_id
incident_packet_id
dynamic_run_id
plan_trace_id
prediction_id
guard_decision_id
artifact_id
episode_id
prompt_sha256
output_sha256
```

The latest run already has most of these. The important gap is that prompt and
model text output are represented by digest/tool-call data, not by a first-class
model I/O record with optional encrypted raw content pointers.

## Should Prompts Be Attached To Outputs?

Yes, but attach them by digest, policy, and controlled object pointer.

Do not copy raw prompts into every ledger, evidence, dashboard, and training
export. That creates privacy and secret-retention risk. The safer default is:

```json
{
  "prompt_sha256": "sha256:...",
  "prompt_template_id": "deployment_fix_v1",
  "prompt_version": 3,
  "redacted_preview": "Fix the broken deployment health check...",
  "input_classification": "internal",
  "token_count": 312,
  "raw_prompt_ref": {
    "capture_mode": "restricted_encrypted",
    "object_uri": "bbx-object://...",
    "kms_key_id": "local-dev-key",
    "retention_policy_id": "agentops_30d"
  }
}
```

For model output:

```json
{
  "output_sha256": "sha256:...",
  "finish_reason": "tool_calls",
  "tool_calls": ["run_dynamicjepa_blackboxfix_pipeline"],
  "redacted_preview": "Calling the verified DynamicJEPA pipeline...",
  "token_count": 91,
  "raw_output_ref": {
    "capture_mode": "restricted_encrypted",
    "object_uri": "bbx-object://...",
    "retention_policy_id": "agentops_30d"
  }
}
```

The normal ledger should contain hashes, structured tool calls, classification,
and redacted previews. Raw prompt/output capture should be an explicit governed
mode, not the default.

## What Data We Have Now

From the latest run:

```text
prompt_sha256=sha256:eb6fd2adc7d21f5e7ffd117eaf68386cc952be23ace8cdc44ce0695ece4d4837
model=gemma4:e4b
tool=run_dynamicjepa_blackboxfix_pipeline
tool_call_id=tool_f1ac116a79194e1bb80a1f89ab19780d
dynamic_run_id=agentops_20260504T043844Z_dfc5a3cd
plan_trace_id=9fd9a36d-b1c6-457b-af0c-28343e1d51a5
artifact_id=0542048b-0c8c-412d-b423-86ec241e1b45
surprise_event_id=bb5c53f4-3e16-4031-bd59-4c1f327a68f7
```

Observed durable data:

```text
raw_events=62
latent_panels=60
transitions=60
dataset_rows_total=59
training_runs=1
model_artifacts=1
predictions=6
guard_decisions=5
plan_traces=1
surprise_events=1
audit_log_records=316
```

BlackBoxFix ledger:

```text
agent_run=1
agent_step=1
model_decision=1
tool_call=1
expected_delta=2
readback_snapshot=2
verifier_result=2
evidence_packet=2
incident_packet=1
scorecard=1
full_state_verification=1
```

## Calculations To Extract

The report now calculates:

- source-of-truth coverage;
- readback ratio;
- verifier pass/failure rate;
- incident rate;
- artifact hash integrity ratio;
- raw-event to panel yield;
- raw-event to transition yield;
- train/val/test ratios;
- guard allow rate;
- selected action utility versus maximum utility;
- surprise rate per plan;
- training episode yield;
- trust level, risk, policy compliance, and verification confidence.

Across multiple runs, add:

- failure rate by prompt template;
- failure rate by raw prompt digest;
- failure rate by tool and capability;
- verifier failure by connector;
- policy denial rate by scenario;
- mean surprise by action type;
- artifact integrity drift by model artifact;
- time from tool execution to verifier result;
- time from failed verifier to incident packet;
- repair success rate after incident;
- training episode yield by scenario;
- prompt/output patterns that correlate with incorrect or risky actions.

## Correlation Edges

The current report emits these edges:

```text
prompt_sha256 -> model_decision
model_decision -> tool_call
tool_call -> DynamicJEPA RocksDB state
expected_delta -> actual_readback
plan_trace -> training episode
artifact registry -> provenance graph
failed verifier -> incident packet -> scorecard
```

This is the core intelligence graph. Once there are multiple runs, it becomes a
queryable operational memory.

## Meaning Of The Latest Results

The system is operating as intended:

- Gemma produced the expected native tool call.
- The harness validated the scenario and capabilities.
- DynamicJEPA persisted real state.
- BlackBoxFix independently verified source-of-truth outputs.
- The happy-path verifier passed.
- The intentional wrong-state verifier failed and created an incident.
- The scorecard lowered trust to `incident`.
- Verified records became training/eval episodes.

The two important gaps are:

- no first-class `model_io` record yet for prompt/output capture policy;
- no explicit policy decision record yet, so the scorecard correctly reports
  `policy_missing`.

## Research Basis

This design follows:

- OpenTelemetry GenAI conventions for correlating model spans, inputs, outputs,
  and tool calls while making content capture configurable rather than always on:
  https://opentelemetry.io/docs/specs/semconv/gen-ai/
- W3C PROV's entity/activity/agent model for artifact lineage:
  https://www.w3.org/TR/prov-dm/
- NIST AI RMF's emphasis on measuring, monitoring, and managing AI risk across
  system lifecycle:
  https://www.nist.gov/itl/ai-risk-management-framework
