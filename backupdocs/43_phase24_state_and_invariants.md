# Phase 24 — SDK CloudEvents Ingestion State And Invariants

Phase 24 ships provider-neutral SDK ingestion. Language SDKs or runtime
adapters emit strict CloudEvents JSON files, and `blackbox sdk ingest` maps
those events onto existing append-only ledger rows.

## Shipped Surface

- Command: `blackbox sdk schema [--ledger PATH]`.
- Command: `blackbox sdk ingest --ledger PATH --input CLOUDEVENTS_JSON`.
- Connector: `blackbox_sdk_ingest_v1`.
- Connector test:
  `blackbox connector test --connector blackbox_sdk_ingest_v1 --source CLOUDEVENTS_JSON`.
- Supported event types:
  - `dev.contextgraph.blackbox.agent_run.v1`
  - `dev.contextgraph.blackbox.agent_step.v1`
  - `dev.contextgraph.blackbox.model_decision.v1`
  - `dev.contextgraph.blackbox.tool_call.v1`

Phase 24 adds no new record kind and does not bump `LEDGER_SCHEMA_VERSION`;
current ledgers remain version `13`.

## Sources Of Truth

- SDK event file: `file:/path/to/events.json#sha256`.
- Ledger rows: `sqlite:/path/to/blackbox.sqlite#ledger_records`.

Return values are not evidence. Successful ingest requires a separate read of
the physical SDK event file and a separate SQLite read of every inserted row,
including `payload.sdk_event.source`, `payload.sdk_event.id`,
`payload.sdk_event.type`, and `payload.sdk_event.data_sha256`.

## Invariants

- Input must be a CloudEvents 1.0 JSON object or JSON array.
- Batch size must be 1 through 128 events.
- `datacontenttype` must resolve to `application/json`.
- `data` must be a JSON object.
- Only supported top-level CloudEvents attributes are accepted:
  `specversion`, `id`, `source`, `type`, `datacontenttype`, `subject`,
  `time`, `traceparent`, and `data`.
- Optional `traceparent` must follow the W3C shape with lowercase hexadecimal
  fields and non-zero trace/parent ids.
- CloudEvents `source` + `id` must be unique inside the batch and absent from
  existing `ledger_records`.
- Child events must reference an existing parent `agent_run` for the same
  tenant.
- Mutating tool-call events require `data.expected_delta`.
- Model/tool inputs and expected deltas are redacted before persistence.
- Batch append is atomic: a validation failure appends zero rows.
- Success includes a post-commit SQLite readback from `ledger_records`.

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase24_sdk_ingest_fsv -- --nocapture
```

The tests write real CloudEvents JSON files, initialize a real SQLite ledger,
run connector readback, ingest a run plus child events, query SQLite directly,
and verify redacted expected-delta state.

Expected happy-path ledger counts:

```text
agent_run=1
agent_step=1
model_decision=1
tool_call=1
```

Covered paths:

- happy run/step/model/tool ingestion and connector readback;
- duplicate CloudEvents source/id (`SDK_EVENT_DUPLICATE`);
- mutating tool call without expected delta (`SDK_EXPECTED_DELTA_REQUIRED`);
- empty batch (`SDK_EVENT_BATCH_EMPTY`);
- invalid W3C traceparent (`SDK_TRACEPARENT_INVALID`);
- all-or-nothing rollback when a later batch event is invalid.

## Manual FSV Checklist

1. Create a fresh ledger with `blackbox init`.
2. Write a CloudEvents `agent_run` JSON file with a known `source` and `id`.
3. Run `blackbox connector test --connector blackbox_sdk_ingest_v1 --source FILE`.
4. Run `blackbox sdk ingest --ledger PATH --input FILE`.
5. Query `ledger_records` directly for `json_extract(payload_json, '$.sdk_event.id')`.
6. Confirm the stored `run_id`, `payload.sdk_event.source`, `type`,
   `data_sha256`, and `runtime.framework`.
7. Write a child-event batch referencing the returned `run_id`.
8. Run `blackbox sdk ingest` on the batch.
9. Query `ledger_records` directly for expected `agent_step`,
   `model_decision`, and `tool_call` counts.
10. Read the `tool_call` payload and confirm `input_redacted` and
    `expected_delta_redacted` contain redaction markers for sensitive fields.
11. Run the edge cases and print before/after row counts:
    duplicate source/id, missing expected delta, empty batch, invalid
    traceparent.
