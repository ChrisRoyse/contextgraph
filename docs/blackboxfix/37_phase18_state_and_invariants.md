# Phase 18 State And Invariants

Phase 18 turns the first-wedge vulnerability-remediation demo into an
executable, source-of-truth-verified package.

## Shipped Surface

Command:

```bash
blackbox demo first-wedge \
  --ledger PATH \
  --tenant TENANT \
  --run-id RUN_ID \
  --out DIR \
  [--force] \
  --agent AGENT
```

Connector:

- `blackbox_first_wedge_demo_v1`

Ledger record kind:

- `first_wedge_demo`

Package files:

- `demo-report.json`
- `evidence-index.json`
- `demo-script.jsonl`
- `manifest.json`

Phase 18 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 12`. The
physical SQLite table shape is unchanged, but old schema-version ledgers are
wiped on `blackbox init`; there is no migration shim.

## Source Of Truth

Primary source of truth:

- SQLite `ledger_records` rows in the user-provided ledger;
- SQLite `blackbox_metadata` count/tip rows;
- a real local Git diff readback from `git_file_diff_matches_patch_v1`;
- real OSV JSON scanner artifact readbacks;
- live GitHub Actions/GitHub Checks/GitLab CI readbacks;
- live GitHub issue evidence-link readbacks;
- package files on disk, verified by `manifest.json`.

## Required Proof Categories

The package command fails closed unless all categories exist as physical
`verifier_result` plus `evidence_packet` rows for the selected run:

| Proof | Required state |
|---|---|
| `git_delta` | passed `git_file_diff_matches_patch_v1` |
| `ci_success` | passed hosted CI verifier |
| `scanner_absence` | passed `scanner_vulnerability_absent_v1` |
| `ticket_evidence` | passed `github_issue_evidence_link_v1` |
| `ci_failure_demo` | failed hosted CI verifier with a shipped CI error code |
| `scanner_failure_demo` | failed `SCANNER_VULNERABILITY_STILL_PRESENT` |
| `permission_failure_demo` | blocked `CONNECTOR_PERMISSION_DENIED` |

## Invariants

- Missing run ids fail before any file or row is created.
- Incomplete proof coverage fails before any file or row is created.
- Existing output directories fail unless `--force` is explicit.
- Every JSON file is written, read back, and compared to the intended value.
- Every JSONL line must be a nonblank valid JSON value with format
  `blackbox_first_wedge_demo_step_json_v1`.
- `manifest.json` records SHA-256 and byte count for every package file.
- The command appends exactly one `first_wedge_demo` row only after package
  file readback succeeds.
- The appended row is read back from SQLite by sequence, demo id,
  `payload_sha256`, and `record_sha256`.

## Error Codes

| Code | Meaning |
|---|---|
| `FIRST_WEDGE_RUN_NOT_FOUND` | selected run is absent |
| `FIRST_WEDGE_DEMO_INCOMPLETE` | one or more required proof categories are missing |
| `FIRST_WEDGE_EVIDENCE_PACKET_MISSING` | a selected verifier result has no evidence packet |
| `FIRST_WEDGE_RECORD_SCHEMA_DRIFT` | required source row is malformed |
| `FIRST_WEDGE_JSON_READBACK_MISMATCH` | package JSON readback changed |
| `FIRST_WEDGE_JSONL_READBACK_MISMATCH` | package JSONL readback changed or is invalid |
| `FIRST_WEDGE_MANIFEST_READBACK_MISMATCH` | final manifest readback is malformed |
| `FIRST_WEDGE_FILE_HASH_MISMATCH` | final file hash/byte count differs from manifest |
| `FIRST_WEDGE_DEMO_READBACK_MISMATCH` | appended SQLite row cannot be read back exactly |

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase18_first_wedge_demo_fsv -- --nocapture
```

The test uses real sources:

- a local Git repository with a known `Cargo.lock` dependency diff;
- physical clean and vulnerable OSV JSON reports;
- live GitHub Actions run state;
- live GitHub issue evidence-link state;
- an invalid GitHub token to prove `CONNECTOR_PERMISSION_DENIED`;
- direct SQLite reads for row counts and latest `first_wedge_demo` payload;
- direct file reads and SHA-256 checks for the package manifest.

Happy-path expected final state after one complete package:

- `agent_run=1`
- `tool_call=1`
- `expected_delta=7`
- `readback_snapshot=6`
- `verifier_result=7`
- `evidence_packet=7`
- `incident_packet=2`
- `first_wedge_demo=1`

Edge cases covered:

- missing run id: no files, no `first_wedge_demo` row;
- incomplete proof coverage: no files, no `first_wedge_demo` row;
- output directory already exists without `--force`: no additional row.

## Manual FSV Checklist

Print direct SQLite state before and after each trigger:

```sql
SELECT record_kind, COUNT(*)
FROM ledger_records
GROUP BY record_kind
ORDER BY record_kind;
```

Inspect the demo row:

```sql
SELECT sequence,
       json_extract(payload_json, '$.first_wedge_demo_id'),
       json_extract(payload_json, '$.demo_success.status'),
       json_extract(payload_json, '$.required_proofs.permission_failure_demo.error_code')
FROM ledger_records
WHERE record_kind='first_wedge_demo'
ORDER BY sequence DESC
LIMIT 1;
```

Inspect files:

```bash
jq .demo_success first-wedge-demo/demo-report.json
jq .files first-wedge-demo/manifest.json
sha256sum first-wedge-demo/demo-report.json
wc -l first-wedge-demo/demo-script.jsonl
```
