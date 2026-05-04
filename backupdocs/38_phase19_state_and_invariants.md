# Phase 19 — Executable FSV Protocol State And Invariants

Phase 19 turns `19_full_state_verification_protocol.md` from a human-only
checklist into executable product state.

## Shipped Surface

- Command:
  `blackbox fsv certify --ledger PATH --tenant TENANT --phase N --evidence FILE --out DIR [--force] [--run-id RUN_ID] --agent AGENT`
- Connector:
  `blackbox_fsv_protocol_v1`
- Record kind:
  `full_state_verification`
- Schema:
  Phase 19 shipped `LEDGER_SCHEMA_VERSION = 13`; current ledgers run under
  `LEDGER_SCHEMA_VERSION = 14` after the reward compiler.

## Source Of Truth

- SQLite file at `--ledger PATH`, table `ledger_records`;
- SQLite metadata table `blackbox_metadata`;
- SQLite `PRAGMA integrity_check`;
- FSV evidence JSON file passed through `--evidence`;
- declared artifact files and their SHA-256/byte-count readbacks;
- generated package files:
  `fsv-report.json`, `provenance.json`, `edge-cases.jsonl`, `manifest.json`.

## Evidence Contract

The input evidence JSON must use:

```json
{
  "format": "blackbox_full_state_verification_evidence_json_v1",
  "phase": 19,
  "tenant_id": "tenant_...",
  "run_id": "run_...",
  "source_of_truth": "sqlite:/tmp/bbx.sqlite#ledger_records",
  "trigger": {"command": "...", "exit_code": 0},
  "pre_state": {"ledger_record_count": 0, "record_kind_counts": {}},
  "post_state": {"ledger_record_count": 2, "record_kind_counts": {"agent_run": 1, "tool_call": 1}, "ledger_tip_sha256": "sha256:..."},
  "expected_delta": {"ledger_record_count": 2, "record_kind_counts": {"agent_run": 1, "tool_call": 1}},
  "actual_delta": {"ledger_record_count": 2, "record_kind_counts": {"agent_run": 1, "tool_call": 1}},
  "read_queries": [{"source_of_truth": "...", "query": "...", "output": "..."}],
  "artifacts": [{"path": "/tmp/fsv.log", "sha256": "sha256:...", "bytes": 123}],
  "edge_cases": [
    {"category": "missing_target_state", "command": "...", "exit_code": 1, "error_code": "...", "pre_state": {}, "post_state": {}, "expected_delta": {}, "actual_delta": {}},
    {"category": "wrong_target_state", "command": "...", "exit_code": 1, "error_code": "...", "pre_state": {}, "post_state": {}, "expected_delta": {}, "actual_delta": {}},
    {"category": "permission_or_schema_failure", "command": "...", "exit_code": 1, "error_code": "...", "pre_state": {}, "post_state": {}, "expected_delta": {}, "actual_delta": {}}
  ]
}
```

The command rejects the evidence before writing package files or appending a
row when any required field is missing, `post_state` does not match the
physical SQLite readback, deltas do not match, an edge category is missing, or
an artifact hash/byte count differs from disk.

## Persisted Invariants

- One successful certification appends exactly one `full_state_verification`
  row.
- The row is appended only after package JSON/JSONL files are written and read
  back successfully.
- `manifest.json` contains only safe relative file paths.
- Every manifest file entry is verified by physical file hash and byte count
  after directory promotion.
- The appended row is read back by SQLite sequence, record kind,
  `full_state_verification_id`, `payload_sha256`, and `record_sha256`.
- The connector test reports `full_state_verification_records_total` from the
  physical ledger and includes SQLite `integrity_check=["ok"]`.

## Error Codes

- `FSV_EVIDENCE_JSON_INVALID`
- `FSV_EVIDENCE_SCHEMA_INVALID`
- `FSV_PHASE_INVALID`
- `FSV_PHASE_MISMATCH`
- `FSV_TENANT_MISMATCH`
- `FSV_RUN_MISMATCH`
- `FSV_SOURCE_OF_TRUTH_MISMATCH`
- `FSV_TRIGGER_FAILED`
- `FSV_DELTA_MISMATCH`
- `FSV_STATE_DELTA_MISMATCH`
- `FSV_POST_STATE_MISMATCH`
- `FSV_READ_QUERIES_MISSING`
- `FSV_EDGE_CASES_INCOMPLETE`
- `FSV_EDGE_CATEGORY_INVALID`
- `FSV_EDGE_ERROR_CODE_MISSING`
- `FSV_EDGE_COMMAND_MISSING`
- `FSV_EDGE_EXIT_CODE_INVALID`
- `FSV_ARTIFACTS_MISSING`
- `FSV_ARTIFACT_HASH_MISMATCH`
- `FSV_SQLITE_INTEGRITY_CHECK_FAILED`
- `FSV_JSON_READBACK_MISMATCH`
- `FSV_JSONL_SCHEMA_INVALID`
- `FSV_JSONL_READBACK_MISMATCH`
- `FSV_MANIFEST_READBACK_MISMATCH`
- `FSV_MANIFEST_PATH_INVALID`
- `FSV_FILE_HASH_MISMATCH`
- `FSV_CERTIFICATION_READBACK_MISMATCH`

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase19_fsv_protocol_fsv -- --nocapture
```

The test uses the real `context-graph` binary, real SQLite ledger files, real
artifact files, direct SQLite reads, and manifest hash checks. It proves:

- missing evidence file: no package, no `full_state_verification` row;
- wrong post-state: no package, no row;
- bad artifact hash: no package, no row;
- happy path: package files exist and one row is appended;
- connector readback reports the physical certification count and SQLite
  integrity check;
- output-exists edge: no additional row.

Expected happy final counts:

- `agent_run=1`
- `tool_call=1`
- `full_state_verification=1`

## Manual FSV Checklist

For manual verification:

1. Capture `SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind`
   before and after the trigger action.
2. Capture `SELECT value FROM blackbox_metadata WHERE key='ledger_tip_sha256'`
   before certification.
3. Record the trigger command, stdout/stderr hashes, and exit code.
4. Run at least three failing actions for the mandatory edge categories and
   capture typed error JSON plus before/after state.
5. Hash every artifact declared in `artifacts`.
6. Run `blackbox fsv certify`.
7. Read the persisted row:

```sql
SELECT sequence,
       json_extract(payload_json, '$.full_state_verification_id'),
       json_extract(payload_json, '$.phase'),
       json_extract(payload_json, '$.fsv_protocol.edge_case_count'),
       json_extract(payload_json, '$.fsv_protocol.sqlite_integrity_check[0]')
FROM ledger_records
WHERE record_kind='full_state_verification'
ORDER BY sequence DESC
LIMIT 1;
```

8. Hash every file listed in `manifest.json` and compare with the manifest.
