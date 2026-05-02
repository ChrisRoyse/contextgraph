# Storage, Retention, And Privacy

BlackBoxFix stores high-value operational evidence. The storage layer must be
tamper-evident, queryable, privacy-aware, and exportable.

## Storage Layers

### Ledger Store

Durable append-only records:

- run records;
- step records;
- tool calls;
- expected deltas;
- readback snapshots;
- verifier results;
- policy decisions;
- incident packets;
- export manifests.

Initial implementation can use RocksDB or SQLite for local mode and Postgres
for team/enterprise mode. The schema should not assume one backend forever.

### Evidence Artifact Store

Large or sensitive artifacts:

- redacted logs;
- test reports;
- scanner reports;
- diffs;
- screenshots if UI automation is used;
- signed audit packets.

Artifact records store hashes, MIME types, sizes, and retention policies.

### Index Store

Query acceleration:

- run id;
- agent id;
- tool name;
- source-of-truth locator;
- verifier status;
- error code;
- risk tier;
- incident severity.

## Hash Chaining

Hash chains should exist at two levels:

- run chain: ordered records in one agent run;
- tenant chain: ordered committed ledger batches.

This gives local investigation and enterprise audit a way to detect tampering.

## Retention Classes

- `debug_short`: low-risk development traces.
- `standard`: team operational evidence.
- `regulated`: long-retention audit evidence.
- `incident`: high-priority security and compliance record.
- `training`: approved redacted export rows.

Retention should be policy-driven and visible on each evidence packet.

## Privacy Controls

- field-level redaction;
- deterministic hashing for stable identifiers;
- envelope encryption for sensitive artifacts;
- tenant-scoped keys;
- data classification tags;
- export approval workflow;
- deletion workflow for data subject requests where legally required;
- immutable audit record for deletion actions.

## Data Minimization

Persist the minimum data needed to verify, replay, audit, and train:

- store exact raw payload only when required and approved;
- otherwise store redacted payload plus digest;
- separate evidence artifact storage from searchable indexes;
- store model text with redaction and truncation policy;
- preserve source-of-truth identifiers and hashes.

## Failure Rules

The platform must fail closed if:

- ledger write fails;
- evidence artifact hash cannot be computed;
- redaction policy fails;
- schema validation fails;
- hash chain cannot be extended;
- source-of-truth readback cannot be persisted.

## Enterprise Deployment

Enterprise storage should support:

- private VPC deployment;
- customer-managed keys;
- region selection;
- backup and restore;
- retention lock;
- audit export;
- disaster recovery tests;
- tenant migration.

