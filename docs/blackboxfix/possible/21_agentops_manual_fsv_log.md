# AgentOps Manual FSV Log

This log records the latest successful manual Full State Verification run for
the Gemma-triggered DynamicJEPA + BlackBoxFix demo.

## Run

```text
root=/tmp/blackboxfix-agentops-demo-manual-03
dynamic_run_id=agentops_20260504T043844Z_dfc5a3cd
blackbox_run_id=run_ab2eb49499cb438cbd62654a2cef7e4e
tool_call_id=tool_f1ac116a79194e1bb80a1f89ab19780d
model=gemma4:e4b
```

## Source Of Truth

```text
sqlite:/tmp/blackboxfix-agentops-demo-manual-03/blackbox/blackbox.sqlite#ledger_records
rocksdb:/tmp/blackboxfix-agentops-demo-manual-03/dynamicjepa/dbs/agentops_20260504T043844Z_dfc5a3cd_counter_world_rocksdb#dynamicjepa
file:/tmp/blackboxfix-agentops-demo-manual-03/dynamicjepa/exports/dynamicjepa-episodes.jsonl
file:/tmp/blackboxfix-agentops-demo-manual-03/provenance/artifact-provenance.json
```

## Execute And Inspect

BlackBoxFix ledger read:

```text
agent_run|1
agent_step|1
evidence_packet|2
expected_delta|2
full_state_verification|1
incident_packet|1
model_decision|1
readback_snapshot|2
scorecard|1
tool_call|1
verifier_result|2
```

DynamicJEPA RocksDB read:

```text
dj_raw_events=62
dj_latent_panels=60
dj_dataset_shards=3
dj_training_runs=1
dj_model_artifacts=1
dj_predictions=6
dj_guard_decisions=5
dj_plan_traces=1
dj_surprise_events=1
dj_verification_runs=1
dj_audit_log=316
```

Artifact registry read:

```text
artifact_id=0542048b-0c8c-412d-b423-86ec241e1b45
artifact_status=Active
file_count=5
all_hashes_equal=true
```

Plan trace read:

```text
plan_trace_id=9fd9a36d-b1c6-457b-af0c-28343e1d51a5
candidate_count=5
prediction_count=5
guard_count=5
selected_action_id=c135888d-894c-467b-89cc-86ac65c0e9dd
```

Surprise event read:

```text
surprise_event_id=bb5c53f4-3e16-4031-bd59-4c1f327a68f7
prediction_id=8e88bd95-b04c-40b3-b256-3fe880df38a9
observed_panel_id=4965dbbe-17f6-466b-802c-35da8750df68
```

Verifier script readback:

```text
status=ok
episode_count=4
plan_trace_id=9fd9a36d-b1c6-457b-af0c-28343e1d51a5
surprise_event_id=bb5c53f4-3e16-4031-bd59-4c1f327a68f7
provenance_sha256=sha256:87b848cc37f1ba169bd8ef116f249c804dc3a3a97b32b72c088b8ba150c1608f
```

File hash readback:

```text
dynamicjepa-episodes.jsonl:
sha256:0f8fbaa44f2b3707f34f38f5f234296ab02c12a8c66b63d41d95c236712c3de5

artifact-provenance.json:
sha256:87b848cc37f1ba169bd8ef116f249c804dc3a3a97b32b72c088b8ba150c1608f

fsv-certification/manifest.json:
sha256:651c79307cd7ce854b1baeee4111246d748d45977354e697220191cdf7e82721
```

## Boundary And Edge Case Audit

### Edge 1: Wrong Target State

Trigger:

```text
BlackBoxFix verifier expected dj_plan_traces=999
```

Result:

```text
error_code=CMD_EXIT_NON_ZERO
exit_code=1
```

Before:

```text
ledger_record_count=8
agent_run=1
agent_step=1
model_decision=1
tool_call=1
expected_delta=1
readback_snapshot=1
verifier_result=1
evidence_packet=1
```

After:

```text
ledger_record_count=13
agent_run=1
agent_step=1
model_decision=1
tool_call=1
expected_delta=2
readback_snapshot=2
verifier_result=2
evidence_packet=2
incident_packet=1
```

Expected delta matched actual delta:

```text
expected_delta +1
readback_snapshot +1
verifier_result +1
evidence_packet +1
incident_packet +1
```

### Edge 2: Missing Target State

Trigger:

```text
Require impossible Ollama model tag.
```

Result:

```text
error_code=DEMO_MODEL_NOT_FOUND
exit_code=2
```

Before and after were identical:

```text
ledger_record_count=13
actual_delta=0
```

### Edge 3: Permission Or Schema Failure

Trigger:

```text
Validate Gemma tool arguments with scenario=not_allowlisted.
```

Result:

```text
error_code=DEMO_TOOL_ARGUMENTS_INVALID
exit_code=2
```

Before and after were identical:

```text
ledger_record_count=13
actual_delta=0
```

## Regression Commands

These focused tests passed after the implementation:

```bash
cargo test -p context-graph-cli \
  --test blackbox_phase19_fsv_protocol_fsv \
  --test blackbox_phase24_sdk_ingest_fsv \
  --test dynamicjepa_bundle_export_fsv \
  -- --nocapture
```

Result:

```text
blackbox_phase19_fsv_protocol_fsv: 1 passed
blackbox_phase24_sdk_ingest_fsv: 4 passed
dynamicjepa_bundle_export_fsv: 1 passed
```
