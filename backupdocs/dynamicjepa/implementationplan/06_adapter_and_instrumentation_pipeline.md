# Adapter And Instrumentation Pipeline

Adapters convert raw domain events into normalized state/action/outcome records. Instruments convert normalized records into measured signals used by latent panels.

## Pipeline Stages

```text
raw input
  -> raw event record
  -> adapter run
  -> normalized state/action/outcome/transition
  -> instrument readings
  -> latent panel
```

Every stage must persist enough information to debug failures. A later stage must reference the exact source records from earlier stages.

## Raw Event Ingestion

`RawDomainEvent` fields:

- `event_id`
- `domain_pack_id`
- `adapter_id`
- `source_kind`
- `source_uri`
- `source_offset`
- `payload_format`
- `payload_bytes`
- `payload_hash`
- `received_at_unix_ms`

Rules:

- payload is stored exactly as received;
- payload hash is computed before parsing;
- oversized payloads fail before any write unless the domain pack declares a chunking adapter;
- raw event is not considered normalized until an adapter run completes.

## Adapter Runs

`AdapterRunRecord` fields:

- `adapter_run_id`
- `adapter_id`
- `domain_pack_id`
- `event_id`
- `started_at_unix_ms`
- `finished_at_unix_ms`
- `status`
- `error_code`
- `error_message`
- `output_state_id`
- `output_action_id`
- `output_outcome_id`
- `output_transition_id`

Rules:

- adapter parse failures write a failed adapter run record if the raw event was already persisted;
- adapter output must validate against the domain pack before normalized records are written;
- invalid output does not write state/action/outcome/transition records;
- all adapter errors include adapter id, event id, payload hash, and field path.

## Built-In Adapter Types

Start with:

- `json_event`: maps JSON fields to state/action/outcome fields through explicit paths.
- `memory_event`: maps existing Context Graph memory records and source metadata into DynamicJEPA observations.
- `graph_event`: maps graph/typed-edge records into entity/action/outcome transitions.
- `learner_event`: maps learner session observations and delta logs into learning-state transitions.

Custom adapter code can be added later, but v1 should prefer declarative adapters that are easy to verify.

## Instrument Execution

Instrument types:

- `scalar`: numeric field to normalized f32 vector.
- `categorical`: declared category to one-hot vector.
- `time`: timestamp-derived features.
- `text_embedding`: existing embedding service output.
- `sparse_terms`: sparse lexical features.
- `graph_neighborhood`: graph-neighborhood statistics from existing graph storage.
- `panel_join`: pulls an existing Context Graph fingerprint or learner vector into a panel slot.

Rules:

- instruments run in the order declared by the domain pack;
- required instrument failure aborts panel materialization;
- every reading records input field values or input hash;
- reading shape must match declared output shape;
- reading values must be finite;
- instrument version must be persisted.

## No Fallback Policy

Disallowed behavior:

- substituting zero vectors for failed instruments;
- using stale embeddings when the requested model version is missing;
- inferring a domain pack from the payload;
- silently skipping unknown fields;
- converting parse failures into null fields;
- defaulting missing action fields to zero.

Allowed behavior:

- a domain pack can declare an instrument optional;
- optional absence must be represented in `slot_masks` with an explicit reason and persisted reading status;
- optional absence must not satisfy required objectives.

## APIs

Service:

- `ingest_raw_event(domain_pack_ref, adapter_id, payload)`
- `run_adapter(event_id)`
- `materialize_instrument_readings(transition_id)`
- `materialize_panel(transition_id)`

Storage:

- `put_raw_event`
- `put_adapter_run`
- `put_normalized_transition_batch`
- `put_instrument_readings_and_panel_batch`
- `get_adapter_run`
- `list_readings_for_event`

CLI:

- `dynamicjepa ingest-event --domain counter_world --adapter json_event --file event.json`
- `dynamicjepa run-adapter --event-id ...`
- `dynamicjepa materialize-panel --transition-id ...`
- `dynamicjepa read-panel --panel-id ... --include-readings`

## Verification Fixtures

`counter_world_happy_path.jsonl` should include:

```json
{"state":{"counter":2},"action":{"delta":2},"outcome":{"next_counter":4},"ts":1000}
```

Expected persisted result:

- raw event count +1;
- adapter run count +1 with status `completed`;
- state count +1 with `counter=2`;
- action count +1 with `delta=2`;
- outcome count +1 with `next_counter=4`;
- transition count +1 linking all three;
- instrument reading count +1 for `counter_scalar`;
- latent panel count +1 with one required slot.

Manual verification must read and print each row.
