# Storage, Retention, And Privacy

BlackBoxFix stores high-value operational evidence. The storage layer must be
tamper-evident, queryable, privacy-aware, and exportable.

## Storage Layers

### Ledger Store (Phase 1: SQLite)

Phase 1 uses a single SQLite file with WAL journaling and `synchronous=FULL`
(see `04_event_ledger_schema.md`). The schema is intentionally backend-
agnostic: the same record envelope can move to Postgres for Phase 7 without
changing the producer code, and to RocksDB column families if storage volume
demands it.

The ledger holds:

- run records (`agent_run`);
- tool calls (`tool_call`);
- expected deltas (Phase 2);
- readback snapshots (Phase 2);
- verifier results (Phase 2);
- evidence packets (Phase 4);
- policy decisions (Phase 6);
- incident packets (Phase 4);
- export manifests (Phase 5);
- scorecards, privacy decisions, release gates, and pilot package rows
  (Phases 12-15).

### Evidence Artifact Store (Phase 4)

Phase 4 ships local JSON evidence export files. Larger or sensitive artifacts
are reserved for later connector/export phases:

- redacted logs;
- test reports;
- scanner reports;
- diffs;
- screenshots if UI automation is used;
- signed audit packets.
- pilot package reports, usage-meter JSONL, eval-candidate JSONL, and
  manifests.

Artifact records must store SHA-256 hashes, MIME types, sizes, and retention
policies. The ledger should hold only the hash + locator. Phase 4 already
applies this discipline to exported evidence reports by writing the file,
reading it back, byte-comparing it, parsing the JSON, and returning its SHA-256.

### Index Store (Phase 7)

Query acceleration:

- run id;
- agent id;
- tool name;
- source-of-truth locator;
- verifier status;
- error code;
- risk tier;
- incident severity.

Phase 1 already provides `idx_ledger_records_run_id` and
`idx_ledger_records_kind`. Phase 7 dashboard read models are built from a
verified full ledger scan in local mode; richer persisted query indexes remain
for the hosted/shared-ledger deployment.

## Hash Chaining

Two chains exist (both already enforced in Phase 1):

- **Run chain** — ordered records sharing one `run_id`, linked by
  `run_previous_record_sha256` → `run_chain_sha256`.
- **Tenant/global chain** — ordered records across the whole ledger, linked
  by `previous_record_sha256` → `record_sha256`.

This gives local investigation and enterprise audit a way to detect tampering.
Phase 8 adds tenant enterprise-control rows and audit/SIEM exports. A later
enhancement can add a daily checkpoint (e.g. signed Merkle root of the day's
`record_sha256` values) for cross-tenant attestation.

## Retention Classes (Phase 8)

| Class | Use case |
|---|---|
| `debug_short` | low-risk development traces |
| `standard` | team operational evidence |
| `regulated` | long-retention audit evidence |
| `incident` | high-priority security/compliance record |
| `training` | approved redacted export rows |

Retention is policy-driven. Phase 8 persists tenant retention policy in
`enterprise_tenant_config.retention` with a named default class and explicit
`retain_days` / `legal_hold` fields. Evidence packets remain immutable; later
artifact-tier storage can stamp the selected retention class on each artifact.
Phase 13 applies those configured classes through authorized
`retention_decision` and `evidence_deletion` ledger rows.

## Privacy Controls

Already shipped (Phase 1):

- field-level redaction (`redact_v1`);
- deterministic hashing for stable identifiers (`sha256` over canonical JSON);
- `redaction_json.secret_digest_fields` preserves equality checks without the
  raw secret.

Phase 8 shipped:

- tenant-scoped customer-managed key ids in
  `enterprise_tenant_config.tenant_key_store`;
- raw secret/key/certificate material is refused in enterprise-control input;
- audit exports include `controls.json` so security review can inspect the
  tenant key id, data residency, and retention policy used for the export.

Phase 13 shipped:

- RBAC-gated privacy review via `privacy.review` and erasure request review via
  `privacy.erase`;
- `retention_decision` rows that apply a named tenant retention class to a
  run after verifying the physical ledger;
- `evidence_deletion` rows that approve or deny erasure requests without
  deleting append-only ledger evidence;
- data-subject minimization: the raw subject is accepted only as CLI input and
  persisted as `data_subject_sha256`;
- fail-closed legal-hold and incident-evidence denials with typed reason codes
  (`LEGAL_HOLD_ACTIVE`, `INCIDENT_EVIDENCE_LOCKED`).

To add later:

- envelope encryption for sensitive artifacts;
- data classification tags;
- export approval workflow;
- artifact-store deletion execution for data subject requests where legally
  required. Phase 13 records the decision and audit trail; a future artifact
  store must perform and verify the external delete.

## Data Minimization

Persist the minimum data needed to verify, replay, audit, and train:

- store exact raw payload only when required and approved;
- otherwise store redacted payload plus digest;
- separate evidence artifact storage from searchable indexes;
- store model text with redaction and truncation policy;
- preserve source-of-truth identifiers and hashes.

## Failure Rules

The platform fails closed if:

- ledger write fails — Phase 1 already aborts the SQLite transaction and
  surfaces `BBX_SQLITE_ERROR`.
- evidence export hash/readback cannot be computed — Phase 4.
- redaction policy fails — Phase 1: `BBX_REDACTION_VIOLATION` (no row written).
- schema validation fails — Phase 1: `BBX_SCHEMA_VALIDATION_FAILED` /
  `BBX_INPUT_SCHEMA_INVALID` / `BBX_INPUT_JSON_INVALID`.
- hash chain cannot be extended — Phase 1: append re-runs
  `verify_ledger_integrity` inside the transaction; corruption refuses
  further writes.
- source-of-truth readback cannot be persisted — Phase 2+.
- privacy decision cannot be authorized, classified, appended, or read back —
  Phase 13 emits `PRIVACY_*` errors and writes no privacy decision row.
- pilot package files or rows cannot be written/read back — Phase 15 emits
  `PILOT_PACKAGE_*` errors and does not substitute a partial package.
- risk decision registers or risk rows cannot be validated/appended/read back
  — Phase 16 emits `RISK_*` errors and does not synthesize fallback closure.

Phase 14 consumes these privacy rows in release gates when
`blackbox gate evaluate --require-privacy` is used. The gate reads the
physical `retention_decision` and `evidence_deletion` rows from
`ledger_records`; a retained privacy decision is required, while approved
erasure decisions block promotion because the run's evidence is no longer
eligible for release use.

Fail-closed means **no fallback**. Do not silently switch to in-memory
storage or write to a degraded path; emit the typed error and exit non-zero.

## Enterprise Deployment (Phase 8)

Enterprise storage supports:

- private VPC deployment (`deployment.mode=private_vpc`);
- customer-managed keys (`tenant_key_store.customer_managed=true`);
- region selection and data residency in tenant config;
- backup and restore;
- retention lock;
- audit export (`audit.jsonl`, `siem-rfc5424.log`, `controls.json`,
  `manifest.json`);
- disaster recovery tests;
- tenant migration.

## FSV Of Storage Invariants

After every change to the storage layer, run:

```bash
ledger=/tmp/bbx-storage-fsv.sqlite
context-graph blackbox init --ledger "$ledger"
# Confirm WAL mode and append-only triggers
sqlite3 "$ledger" 'PRAGMA journal_mode;'              # expect: wal
sqlite3 "$ledger" "SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name;"
# expect: ledger_records_no_delete, ledger_records_no_update

# Append rows, then attempt UPDATE/DELETE — both must fail
context-graph blackbox record run --ledger "$ledger" --task t --agent a
sqlite3 "$ledger" "UPDATE ledger_records SET record_kind='x' WHERE sequence=1;" \
  && echo "FAIL: update should have aborted" || echo "OK: update aborted"
sqlite3 "$ledger" "DELETE FROM ledger_records WHERE sequence=1;" \
  && echo "FAIL: delete should have aborted" || echo "OK: delete aborted"
```

Both attempts must report "OK". If either succeeds, the storage layer has
regressed and must be rolled back (no backwards compat, no migration shim —
delete the change, fix root cause, re-land).
