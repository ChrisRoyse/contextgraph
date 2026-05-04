# AgentOps Research Demo Current State

## Implemented Command

```bash
scripts/blackboxfix_gemma_tool_demo.py \
  --root /tmp/blackboxfix-agentops-demo-manual-03 \
  --force \
  --model gemma4:e4b \
  --blackbox-bin target/debug/context-graph \
  --timeout-seconds 300 \
  --dynamic-timeout-seconds 900
```

## What It Does

The harness uses local `gemma4:e4b` through Ollama native `/api/chat` tool
calling. Gemma must call exactly one allowlisted tool:

```text
run_dynamicjepa_blackboxfix_pipeline
```

The tool executes the real `counter_world` DynamicJEPA verification path and
then records the action in BlackBoxFix.

Implemented capabilities:

- real data ingest into DynamicJEPA RocksDB;
- latent panel materialization;
- trajectory compilation;
- action-conditioned train/val/test dataset shard compilation;
- tiny CUDA training;
- model artifact registry and file-hash verification;
- prediction;
- planning;
- guard decisions;
- surprise recording;
- action-conditioned DynamicJEPA episode export;
- W3C PROV-style provenance artifact;
- BlackBoxFix SDK ingest;
- expected delta persistence;
- independent source-of-truth verifier readback;
- deliberate failed verifier and incident creation;
- scorecard;
- evidence/replay/episode/OTEL/dashboard exports;
- FSV certification.

## Durable Sources Of Truth

```text
BlackBoxFix ledger:
/tmp/blackboxfix-agentops-demo-manual-03/blackbox/blackbox.sqlite

DynamicJEPA RocksDB:
/tmp/blackboxfix-agentops-demo-manual-03/dynamicjepa/dbs/agentops_20260504T043844Z_dfc5a3cd_counter_world_rocksdb

DynamicJEPA episodes:
/tmp/blackboxfix-agentops-demo-manual-03/dynamicjepa/exports/dynamicjepa-episodes.jsonl

Provenance:
/tmp/blackboxfix-agentops-demo-manual-03/provenance/artifact-provenance.json

FSV package:
/tmp/blackboxfix-agentops-demo-manual-03/blackbox/exports/fsv-certification/manifest.json
```

## Current Capability Proof

DynamicJEPA physical readback:

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

BlackBoxFix physical readback:

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

Training/export readback:

```text
train rows=41
val rows=12
test rows=6
DynamicJEPA episode records=4
plan episodes=1
edge-case episodes=3
```

Planning/surprise readback:

```text
plan_trace_id=9fd9a36d-b1c6-457b-af0c-28343e1d51a5
candidate_count=5
prediction_count=5
guard_count=5
surprise_event_id=bb5c53f4-3e16-4031-bd59-4c1f327a68f7
```

Artifact provenance readback:

```text
format=blackboxfix_agentops_provenance_json_v1
agents=3
entities=6
activities=3
relations=3
```

## Operational Notes

RocksDB readbacks must be serial in the current local CLI setup. Parallel
manual reads can fail with `LOCK: Resource temporarily unavailable`. This is a
source-of-truth access constraint, not a model or verifier success condition.
The harness verifier script reads RocksDB sequentially.

The demo generated about 28 MB in `/tmp/blackboxfix-agentops-demo-manual-03`.
It does not download large datasets.

## Research Basis Used

- Ollama tool calling: <https://docs.ollama.com/capabilities/tool-calling>
- Ollama chat API: <https://docs.ollama.com/api/chat>
- Gemma function calling: <https://ai.google.dev/gemma/docs/capabilities/function-calling>
- OpenTelemetry GenAI semantic conventions:
  <https://opentelemetry.io/docs/specs/semconv/gen-ai/>
- W3C PROV-DM: <https://www.w3.org/TR/prov-dm/>
