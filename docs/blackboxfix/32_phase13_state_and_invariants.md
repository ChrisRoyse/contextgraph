# Phase 13 State And Invariants

Phase 13 ships privacy retention and erasure-request decisions. It uses the
enterprise retention/RBAC state from Phase 8, but adds executable CLI workflows
and append-only privacy audit rows.

## Shipped Surface

Commands:

```bash
blackbox privacy evaluate-retention \
  --ledger PATH --tenant TENANT --run-id RUN_ID \
  --principal-id PRINCIPAL --retention-class CLASS \
  --data-subject SUBJECT --purpose PURPOSE --agent AGENT

blackbox privacy request-erasure \
  --ledger PATH --tenant TENANT --run-id RUN_ID \
  --principal-id PRINCIPAL --retention-class CLASS \
  --data-subject SUBJECT --reason REASON --agent AGENT
```

Connector:

- `blackbox_privacy_v1`

Ledger record kinds:

- `retention_decision`
- `evidence_deletion`

Phase 13 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 7`. The
physical SQLite table shape is unchanged, but old schema-version ledgers are
wiped on `blackbox init`; there is no migration shim. Phase 14 later bumps the
live schema to `LEDGER_SCHEMA_VERSION = 8` for release-gate rows, Phase 15
later bumped the live schema to `LEDGER_SCHEMA_VERSION = 9` for pilot-package
rows, and the reward compiler later bumps the current live schema to
`LEDGER_SCHEMA_VERSION = 14` for `reward_compilation` rows.

## Source Of Truth

The source of truth is the physical SQLite ledger:

```text
sqlite:PATH#ledger_records
```

Before a privacy row is appended, the command:

1. Opens the initialized SQLite ledger.
2. Verifies table shape, metadata, append-only triggers, global hash chain, and
   per-run hash chain.
3. Requires the parent `agent_run` row to exist for the requested tenant.
4. Reads the latest `enterprise_tenant_config` and
   `enterprise_directory_snapshot`.
5. Appends an `enterprise_access_decision` for `privacy.review` or
   `privacy.erase`.
6. Fails with `PRIVACY_RBAC_DENIED` if the principal lacks permission; no
   privacy decision row is written.
7. Resolves the requested retention class from
   `enterprise_tenant_config.retention.classes`.
8. Hashes `--data-subject` and persists only `data_subject_sha256`.
9. Appends `retention_decision` or `evidence_deletion`.
10. Reads the appended privacy row back by sequence, `privacy_decision_id`,
    `run_id`, `payload_sha256`, and `record_sha256`.

## Invariants

- Missing parent run fails as `PRIVACY_RUN_NOT_FOUND` before authorization or
  privacy rows are appended.
- Empty, whitespace-padded, oversized, or secret-like data subject values fail
  as `PRIVACY_REQUEST_INVALID` or `PRIVACY_SECRET_MATERIAL_REFUSED` before any
  new row is appended.
- Unauthorized principals produce a durable `enterprise_access_decision`
  denial and then fail as `PRIVACY_RBAC_DENIED`; no privacy row is appended.
- Unknown retention classes fail as `PRIVACY_RETENTION_CLASS_NOT_FOUND`; the
  authorization decision remains as audit evidence, but no privacy row is
  appended.
- Legal-hold erasure requests append `evidence_deletion` with
  `decision.status=denied` and `reason_code=LEGAL_HOLD_ACTIVE`.
- Incident-bearing runs append `evidence_deletion` with
  `decision.status=denied` and `reason_code=INCIDENT_EVIDENCE_LOCKED`.
- Approved erasure decisions never delete ledger rows. The append-only ledger
  keeps the audit decision and reports whether matching external artifacts may
  be deleted by their own source-of-truth store.

## Error Codes

| Code | Meaning |
|---|---|
| `PRIVACY_RUN_NOT_FOUND` | requested run has no physical `agent_run` row |
| `PRIVACY_RBAC_DENIED` | principal lacks `privacy.review` or `privacy.erase` |
| `PRIVACY_RETENTION_CLASS_NOT_FOUND` | class id is absent from tenant retention config |
| `PRIVACY_REQUEST_INVALID` | request field is empty, padded, or too large |
| `PRIVACY_SECRET_MATERIAL_REFUSED` | request field looks like credential material |
| `PRIVACY_TENANT_CONFIG_SCHEMA_INVALID` | persisted enterprise config has retention schema drift |
| `PRIVACY_RECORD_SCHEMA_DRIFT` | source ledger row is malformed |
| `PRIVACY_DECISION_READBACK_MISMATCH` | appended privacy row could not be read back by id/hash |

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase13_privacy_retention_fsv -- --nocapture
```

The test uses a real SQLite ledger, real enterprise config JSON, real SCIM
directory JSON, and real CLI commands. It verifies:

- retention review persists one `retention_decision` with status `retained`;
- standard erasure persists one approved `evidence_deletion`;
- legal-hold erasure persists one denied `evidence_deletion`;
- incident-bearing erasure persists one denied `evidence_deletion`;
- missing retention class creates no `retention_decision`;
- unauthorized principal creates no `evidence_deletion`;
- missing run creates no authorization or privacy row;
- empty data subject creates no new row;
- raw data subject text is absent from every `payload_json`, while the expected
  SHA-256 digest is present.

Final automated physical counts:

```text
agent_run=6
enterprise_access_decision=6
enterprise_directory_snapshot=1
enterprise_tenant_config=1
evidence_deletion=3
evidence_packet=1
expected_delta=1
incident_packet=1
readback_snapshot=1
retention_decision=1
tool_call=1
verifier_result=1
```

## Manual FSV

Manual FSV must print before/after source-of-truth state for the happy path and
at least three edge cases. The source-of-truth query is direct SQLite, not
command stdout.

Completed manual FSV:

- log: `/tmp/bbx-phase13-manual-fsv.ve11jv/fsv.log`
- log SHA-256:
  `91f0a2dd062ddc50d59eca23a719c0ca8590a378cb90a31b38182b86008350b7`
- final ledger integrity: `schema_version=7`, `record_count=24`,
  `status=passed`
- final physical counts: `agent_run=6`, `enterprise_access_decision=6`,
  `enterprise_directory_snapshot=1`, `enterprise_tenant_config=1`,
  `evidence_deletion=3`, `evidence_packet=1`, `expected_delta=1`,
  `incident_packet=1`, `readback_snapshot=1`, `retention_decision=1`,
  `tool_call=1`, `verifier_result=1`
- final privacy rows:

```text
5|retention_decision|retained|RETENTION_POLICY_APPLIED|sha256:9c4d263a27cb9ecaa9a0f05aa92ef9855d68f2b9eccc1b08ad327f063acd6527
8|evidence_deletion|approved|ERASURE_APPROVED_BY_POLICY|sha256:9c4d263a27cb9ecaa9a0f05aa92ef9855d68f2b9eccc1b08ad327f063acd6527
11|evidence_deletion|denied|LEGAL_HOLD_ACTIVE|sha256:9c4d263a27cb9ecaa9a0f05aa92ef9855d68f2b9eccc1b08ad327f063acd6527
24|evidence_deletion|denied|INCIDENT_EVIDENCE_LOCKED|sha256:9c4d263a27cb9ecaa9a0f05aa92ef9855d68f2b9eccc1b08ad327f063acd6527
```

The raw subject string `subject.phase13@example.test` was checked with direct
SQLite `payload_json LIKE` reads across all rows and returned only `0` values.
The expected digest was present in 4 privacy decision rows.

Required checks:

```bash
sqlite3 "$ledger" \
  "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind;"

sqlite3 "$ledger" \
  "SELECT sequence, record_kind, json_extract(payload_json,'$.decision.status'), json_extract(payload_json,'$.decision.reason_code') FROM ledger_records WHERE record_kind IN ('retention_decision','evidence_deletion') ORDER BY sequence;"

sqlite3 "$ledger" \
  "SELECT payload_json LIKE '%subject.phase13@example.test%' FROM ledger_records;"
```

The final query must return only `0` values. If the raw subject appears in the
ledger, Phase 13 has failed data minimization.
