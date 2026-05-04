# Demo FSV And Test Plan

This document defines manual and automated verification for the demo program.

## Current Implemented Test Target

The current harness is:

```bash
scripts/blackboxfix_gemma_tool_demo.py
```

Latest successful manual run:

```text
/tmp/blackboxfix-agentops-demo-manual-03
```

Source of truth:

- BlackBoxFix ledger:
  `/tmp/blackboxfix-agentops-demo-manual-03/blackbox/blackbox.sqlite`
- DynamicJEPA RocksDB:
  `/tmp/blackboxfix-agentops-demo-manual-03/dynamicjepa/dbs/agentops_20260504T043844Z_dfc5a3cd_counter_world_rocksdb`
- DynamicJEPA episodes:
  `/tmp/blackboxfix-agentops-demo-manual-03/dynamicjepa/exports/dynamicjepa-episodes.jsonl`
- Provenance:
  `/tmp/blackboxfix-agentops-demo-manual-03/provenance/artifact-provenance.json`
- FSV package:
  `/tmp/blackboxfix-agentops-demo-manual-03/blackbox/exports/fsv-certification/manifest.json`

Manual readback rule: perform DynamicJEPA RocksDB reads serially. Parallel
RocksDB reads can collide on the local `LOCK` file.

## Current Happy Path Command

```bash
scripts/blackboxfix_gemma_tool_demo.py \
  --root /tmp/blackboxfix-agentops-demo-manual-03 \
  --force \
  --model gemma4:e4b \
  --blackbox-bin target/debug/context-graph \
  --timeout-seconds 300 \
  --dynamic-timeout-seconds 900
```

## Current Happy Path Readbacks

BlackBoxFix SQLite:

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

DynamicJEPA RocksDB:

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

Episode export:

```text
dynamicjepa-episodes.jsonl records=4
plan_episode_count=1
edge_case_episode_count=3
sha256=sha256:0f8fbaa44f2b3707f34f38f5f234296ab02c12a8c66b63d41d95c236712c3de5
```

Provenance:

```text
format=blackboxfix_agentops_provenance_json_v1
agents=3
entities=6
activities=3
relations=3
```

## Current Edge Case Audit

The latest run captured all three FSV-required categories:

```text
wrong_target_state:
  trigger: BlackBoxFix verifier expected dj_plan_traces=999
  error_code: CMD_EXIT_NON_ZERO
  expected_delta == actual_delta:
    expected_delta +1
    readback_snapshot +1
    verifier_result +1
    evidence_packet +1
    incident_packet +1

missing_target_state:
  trigger: require impossible Ollama model tag
  error_code: DEMO_MODEL_NOT_FOUND
  expected_delta == actual_delta == 0

permission_or_schema_failure:
  trigger: invalid Gemma tool scenario
  error_code: DEMO_TOOL_ARGUMENTS_INVALID
  expected_delta == actual_delta == 0
```

## Source Of Truth

For the flagship demo:

- SQLite ledger: `demo-root/blackbox.sqlite#ledger_records`
- Git repository: `demo-root/repo/.git`
- Deployment file: `demo-root/repo/deploy/service.yaml`
- Validator command:
  `python3 tools/validate_deployment.py deploy/service.yaml`
- Generated artifacts: `demo-root/artifacts/**`

## Happy Path Before State

Print:

```bash
sqlite3 blackbox.sqlite \
  "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind;"

git -C repo diff --no-ext-diff --no-color -- deploy/service.yaml

python3 repo/tools/validate_deployment.py repo/deploy/service.yaml

sha256sum repo/deploy/service.yaml
```

Expected:

- validator exits non-zero;
- Git diff is empty after baseline commit;
- file hash matches broken baseline.

## Trigger

```bash
scripts/blackboxfix_gemma_tool_demo.py \
  --demo deployment-fix \
  --root demo-root \
  --model gemma4:e4b \
  --ollama-url http://127.0.0.1:11434 \
  --blackbox-bin target/debug/context-graph
```

## Happy Path After State

Read separately:

```bash
sqlite3 blackbox.sqlite \
  "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind;"

sqlite3 blackbox.sqlite \
  "SELECT json_extract(payload_json,'$.status'), json_extract(payload_json,'$.verifier_id')
   FROM ledger_records WHERE record_kind='verifier_result' ORDER BY sequence;"

sqlite3 blackbox.sqlite \
  "SELECT COUNT(*) FROM ledger_records WHERE record_kind='incident_packet';"

git -C repo diff --no-ext-diff --no-color -- deploy/service.yaml

python3 repo/tools/validate_deployment.py repo/deploy/service.yaml

sha256sum repo/deploy/service.yaml
```

Expected:

- file changed on disk;
- validator exits `0`;
- Git diff matches expected patch;
- at least two passed verifier rows exist;
- intentional failure creates one incident row.

## Edge Case 1: Empty Prompt

Expected:

- non-zero exit;
- `DEMO_PROMPT_EMPTY`;
- SQLite before/after counts identical;
- file hash unchanged.

## Edge Case 2: Unsupported Tool

Synthetic tool call:

```json
{ "name": "run_shell", "arguments": { "command": "rm -rf /" } }
```

Expected:

- non-zero exit;
- `DEMO_TOOL_UNSUPPORTED`;
- no ledger mutation;
- no file mutation.

## Edge Case 3: Path Traversal

Synthetic tool call:

```json
{
  "name": "write_deployment_file",
  "arguments": {
    "path": "../outside.yaml",
    "content": "service: {}"
  }
}
```

Expected:

- non-zero exit;
- `DEMO_TOOL_PATH_REJECTED`;
- no outside file created;
- no ledger mutation.

## Edge Case 4: Invalid YAML

Synthetic tool call writes:

```text
service: [
```

Expected:

- non-zero exit before write promotion;
- `DEMO_DEPLOYMENT_YAML_INVALID`;
- original file hash unchanged;
- no ledger mutation for the rejected write.

## Edge Case 5: Oversized Content

Write content larger than the configured maximum.

Expected:

- non-zero exit;
- `DEMO_TOOL_CONTENT_TOO_LARGE`;
- no file mutation;
- no ledger mutation.

## Failure Demo Delta

The deliberate bad verifier is allowed to mutate the ledger because it proves
incident behavior.

Expected delta:

```text
expected_delta +1
readback_snapshot +1
verifier_result +1
evidence_packet +1
incident_packet +1
```

## Automated Checks

Run:

```bash
python3 -m py_compile scripts/blackboxfix_gemma_tool_demo.py
cargo test -p context-graph-cli --test blackbox_phase19_fsv_protocol_fsv -- --nocapture
cargo test -p context-graph-cli --test blackbox_phase24_sdk_ingest_fsv -- --nocapture
```

The demo harness should also run a real manual FSV path because mocked tests
would not prove the product claim.
