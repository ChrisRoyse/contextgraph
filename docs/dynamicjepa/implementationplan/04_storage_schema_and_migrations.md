# Storage Schema And Migrations

RocksDB is the primary source of truth for DynamicJEPA runtime state. Model artifact files are secondary physical artifacts whose existence and hash must be registered in RocksDB.

## Source Of Truth

Primary source of truth:

- the existing Context Graph RocksDB database opened by `RocksDbTeleologicalStore`;
- DynamicJEPA column families inside that database;
- decoded DynamicJEPA records read through explicit inspection APIs.

Secondary source of truth:

- artifact files under a configured artifact root;
- event fixture files under `configs/dynamicjepa/verification/`;
- domain pack source files under `configs/dynamicjepa/domain_packs/`.

Secondary files are valid only when the RocksDB registry contains their path, SHA-256 hash, size, and registration timestamp.

## New Column Families

Add DynamicJEPA CF constants in `crates/context-graph-storage/src/dynamicjepa/column_families.rs`:

```text
CF_DJ_DOMAIN_PACKS
CF_DJ_DOMAIN_PACK_BY_NAME_VERSION
CF_DJ_INSTRUMENT_REGISTRY
CF_DJ_ADAPTER_REGISTRY
CF_DJ_RAW_EVENTS
CF_DJ_NORMALIZED_STATES
CF_DJ_ACTIONS
CF_DJ_OUTCOMES
CF_DJ_TRANSITIONS
CF_DJ_ADAPTER_RUNS
CF_DJ_INSTRUMENT_READINGS
CF_DJ_LATENT_PANELS
CF_DJ_BINDINGS
CF_DJ_BINDINGS_BY_ENTITY
CF_DJ_TRAJECTORIES
CF_DJ_DATASET_SHARDS
CF_DJ_TRAINING_RUNS
CF_DJ_MODEL_ARTIFACTS
CF_DJ_PREDICTIONS
CF_DJ_SKILL_POLICIES
CF_DJ_PLAN_TRACES
CF_DJ_GUARD_DECISIONS
CF_DJ_SURPRISE_EVENTS
CF_DJ_VERIFICATION_RUNS
CF_DJ_AUDIT_LOG
```

Add `DYNAMICJEPA_CF_COUNT` and tests that count the list exactly.

## Key Formats

Use explicit byte layouts:

```text
domain pack primary: [domain_pack_uuid:16]
domain pack by name/version: [name_hash:16][version_hash:16] -> domain_pack_uuid
instrument registry: [domain_pack_uuid:16][instrument_hash:16]
adapter registry: [domain_pack_uuid:16][adapter_hash:16]
raw event: [event_uuid:16]
normalized state: [state_uuid:16]
action: [action_uuid:16]
outcome: [outcome_uuid:16]
transition: [transition_uuid:16]
adapter run: [adapter_run_uuid:16]
instrument reading: [event_uuid:16][instrument_hash:16]
latent panel: [panel_uuid:16]
binding primary: [binding_uuid:16]
binding by entity: [entity_hash:16][binding_uuid:16] -> empty
trajectory: [trajectory_uuid:16]
dataset shard: [dataset_uuid:16][shard_uuid:16]
training run: [training_run_uuid:16]
model artifact: [artifact_uuid:16]
prediction: [prediction_uuid:16]
skill policy: [skill_uuid:16]
plan trace: [plan_trace_uuid:16]
guard decision: [guard_decision_uuid:16]
surprise event: [surprise_uuid:16]
verification run: [verification_run_uuid:16]
audit log: [timestamp_nanos_be:8][audit_uuid:16]
```

Rules:

- never use variable-length prefixes when fixed hashes can support deterministic prefix iteration;
- document every key helper in `keys.rs`;
- test key helpers against exact byte lengths and round trips;
- no key helper may allocate hidden random ids.

## Value Format

Every DynamicJEPA value:

```text
[record_version: u8][bincode payload]
```

Codec rules:

- reject empty payloads;
- reject unexpected version bytes;
- include expected version, actual version, payload type, and remediation hint in the error;
- round-trip test every persisted record type;
- no automatic migration in decode paths.

## Column Family Options

Recommended options:

- UUID-keyed small records: reuse a `uuid_prefixed_cf_options(cache, 16, 2)` style helper.
- large panel/dataset records: 64KB blocks, LZ4, 8MB write buffer.
- secondary indexes: 16-byte prefix extractors, 2MB write buffer.
- audit logs: 8-byte timestamp prefix extractor, LZ4, append-oriented compaction.

Do not copy an option builder blindly. Each CF should document value size, access pattern, prefix shape, and write pattern.

## Atomic Write Batches

Required batch boundaries:

- registering a domain pack writes primary pack, name/version index, instrument registry rows, adapter registry rows, and audit row in one batch;
- ingesting a raw event writes raw event and adapter run start row in one batch;
- successful adapter output writes state/action/outcome/transition records and adapter run completion in one batch;
- panel materialization writes readings, panel, and audit row in one batch;
- binding writes primary binding, secondary indexes, and audit row in one batch;
- dataset compilation writes shard row, training run input reference if needed, and audit row in one batch;
- prediction writes prediction row and audit row in one batch;
- plan execution writes plan trace, guard decisions, prediction refs, and audit row in one batch.

Failed operations must write a failed run record only when the failure itself is part of the operation contract. For validation failures before a run id exists, no domain data should be written.

## Migration Plan

Phase 1 migration:

- add CF definitions;
- add DB open descriptors;
- add CF count tests;
- add inspection count/list/read functions;
- create a fresh DB test proving all CFs open and count zero.

Existing databases:

- RocksDB must open with all known CFs.
- If a DynamicJEPA CF is missing in an existing DB, the open path must create it only when `create_if_missing` is true and log the exact CF name.
- If creation fails, DB open fails.

Record migrations:

- none in v1.
- wrong version fails with a remediation hint such as "re-run dynamicjepa export_dataset for this shard".

## Physical Verification Requirements

Every storage method must have an independent read method:

- `put_domain_pack` -> `get_domain_pack` and `get_domain_pack_by_name_version`;
- `record_raw_event` -> `get_raw_event` and `count_raw_events`;
- `materialize_panel` -> `get_latent_panel`, `list_instrument_readings_for_event`, and `count_latent_panels`;
- `write_binding` -> `get_binding` and `list_bindings_for_entity`;
- `compile_dataset_shard` -> `get_dataset_shard` and `count_dataset_shards`;
- `register_artifact` -> `get_model_artifact`, filesystem hash read, and hash equality check;
- `record_prediction` -> `get_prediction`;
- `record_plan_trace` -> `get_plan_trace` and linked guard decision reads.
