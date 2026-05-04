# Demo FSV And Test Plan

This document defines manual and automated verification for the demo program.

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

