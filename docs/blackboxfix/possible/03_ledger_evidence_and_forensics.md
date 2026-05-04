# Ledger, Evidence, And Forensics

The shipped system is centered on a tamper-evident SQLite ledger. Everything
important is either stored in `ledger_records`, derived from those rows, or
verified by reading an external physical source of truth.

## Ledger Store

The ledger is a SQLite database at `--ledger PATH`.

Physical tables:

- `ledger_records`
- `blackbox_metadata`

SQLite is configured for WAL mode, `synchronous=FULL`, foreign keys, and a
busy timeout. The `ledger_records` table is guarded by append-only triggers
that reject updates and deletes.

## Hash Chains

Two chains are maintained:

- per-record chain over every ledger row;
- per-run chain over records that share a `run_id`.

Every inspect/readback path can recompute these chains. A mismatch is a
hash-chain failure, not a warning.

## Redaction

The shipped `redact_v1` policy redacts sensitive field names and blocks
secret-like literals outside known sensitive fields. Redaction happens before
persistence.

The system can persist redaction proof:

- redacted field paths;
- digest for redacted secret values;
- redaction policy id.

This applies to normal tool inputs, model-decision inputs, and Phase 24 SDK
tool-call expected deltas.

## Record Families

The ledger can store:

- `agent_run`
- `tool_call`
- `expected_delta`
- `readback_snapshot`
- `verifier_result`
- `evidence_packet`
- `incident_packet`
- `policy_decision`
- `agent_step`
- `model_decision`
- `enterprise_tenant_config`
- `enterprise_directory_snapshot`
- `enterprise_access_decision`
- `scorecard`
- `retention_decision`
- `evidence_deletion`
- `release_gate`
- `pilot_package`
- `risk_assessment`
- `http_request_audit`
- `first_wedge_demo`
- `full_state_verification`

Phase 24 SDK ingestion maps external SDK events into existing
`agent_run`, `agent_step`, `model_decision`, and `tool_call` rows with
`payload.sdk_event` metadata.

## Evidence Packets

Evidence packets can tie together:

- expected delta;
- readback snapshot;
- verifier result;
- source-of-truth locator;
- hashes;
- run id;
- tool call id;
- incident context when verification fails.

The evidence export command can write a durable report file for one run.

## Incidents

Failed verification can produce incident packets. These are used by:

- replay;
- dashboard error views;
- scorecards;
- release gates;
- pilot packages;
- risk assessment;
- first-wedge demos;
- OpenTelemetry logs.

## Replay

`blackbox replay --ledger PATH --run-id RUN_ID` can reconstruct the run
timeline from immutable ledger rows. Replay is forensic: it reads what
happened and why, without rerunning the original action.

## Inspection

`blackbox inspect ledger` can:

- list recent rows;
- filter by run id;
- verify row count metadata;
- verify ledger tip hash;
- verify per-record and per-run hash chains;
- parse actor/payload/redaction JSON.

This makes the ledger itself inspectable as a source of truth.

## Atomic Batch Append

Phase 24 introduced a shared atomic batch append path. It allows multiple
records to be validated, hash-linked, inserted, and metadata-updated in one
transaction. SDK ingest uses this so a bad event in a batch appends zero rows.

## Forensic Value

The system can answer after the fact:

- which run created a tool call;
- which model/tool/capability was selected;
- what input was redacted and hashed;
- what state was expected;
- what state was actually read;
- whether verification passed, failed, or blocked;
- what evidence and incident packets were created;
- whether later governance decisions depended on that proof.
