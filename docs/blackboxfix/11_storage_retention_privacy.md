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
- policy decisions (Phase 6);
- incident packets (Phase 4);
- export manifests (Phase 5).

### Evidence Artifact Store (Phase 4)

Large or sensitive artifacts:

- redacted logs;
- test reports;
- scanner reports;
- diffs;
- screenshots if UI automation is used;
- signed audit packets.

Artifact records store SHA-256 hashes, MIME types, sizes, and retention
policies. The ledger holds only the hash + locator.

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
`idx_ledger_records_kind`; richer indexes arrive when the dashboard lands.

## Hash Chaining

Two chains exist (both already enforced in Phase 1):

- **Run chain** — ordered records sharing one `run_id`, linked by
  `run_previous_record_sha256` → `run_chain_sha256`.
- **Tenant/global chain** — ordered records across the whole ledger, linked
  by `previous_record_sha256` → `record_sha256`.

This gives local investigation and enterprise audit a way to detect tampering.
A Phase 8 enhancement adds a daily checkpoint (e.g. signed Merkle root of the
day's `record_sha256` values) for cross-tenant attestation.

## Retention Classes (Phase 8)

| Class | Use case |
|---|---|
| `debug_short` | low-risk development traces |
| `standard` | team operational evidence |
| `regulated` | long-retention audit evidence |
| `incident` | high-priority security/compliance record |
| `training` | approved redacted export rows |

Retention is policy-driven and visible on each evidence packet.

## Privacy Controls

Already shipped (Phase 1):

- field-level redaction (`redact_v1`);
- deterministic hashing for stable identifiers (`sha256` over canonical JSON);
- `redaction_json.secret_digest_fields` preserves equality checks without the
  raw secret.

To add later:

- envelope encryption for sensitive artifacts;
- tenant-scoped keys;
- data classification tags;
- export approval workflow;
- deletion workflow for data subject requests where legally required;
- immutable audit record for deletion actions (an `evidence_deletion`
  record kind that is itself append-only).

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
- evidence artifact hash cannot be computed — Phase 4.
- redaction policy fails — Phase 1: `BBX_REDACTION_VIOLATION` (no row written).
- schema validation fails — Phase 1: `BBX_SCHEMA_VALIDATION_FAILED` /
  `BBX_INPUT_SCHEMA_INVALID` / `BBX_INPUT_JSON_INVALID`.
- hash chain cannot be extended — Phase 1: append re-runs
  `verify_ledger_integrity` inside the transaction; corruption refuses
  further writes.
- source-of-truth readback cannot be persisted — Phase 2+.

Fail-closed means **no fallback**. Do not silently switch to in-memory
storage or write to a degraded path; emit the typed error and exit non-zero.

## Enterprise Deployment (Phase 8)

Enterprise storage supports:

- private VPC deployment;
- customer-managed keys;
- region selection;
- backup and restore;
- retention lock;
- audit export;
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
