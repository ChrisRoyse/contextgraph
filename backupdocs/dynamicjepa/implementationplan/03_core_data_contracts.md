# Core Data Contracts

The core contract must be strict enough that storage, MCP, CLI, and model training all agree on the same records. Every record should be serializable with `serde`, round-trip through bincode, and validate itself without touching storage.

## Shared Record Header

Every persisted DynamicJEPA record should include:

```text
record_id: Uuid
record_version: u8
domain_pack_id: String
domain_pack_version: String
created_at_unix_ms: i64
source_run_id: Option<Uuid>
content_hash: [u8; 32]
```

Rules:

- `record_version` must match the storage codec version byte.
- `domain_pack_id` and `domain_pack_version` must point to a persisted domain pack.
- `content_hash` is SHA-256 over the canonical serialized logical payload, excluding the hash field itself.
- Records with mismatched hash are corrupt and must fail decode or validation.

## IDs

Define newtypes in `ids.rs`:

- `DomainPackId(String)`
- `InstrumentId(String)`
- `AdapterId(String)`
- `EventId(Uuid)`
- `StateId(Uuid)`
- `ActionId(Uuid)`
- `OutcomeId(Uuid)`
- `PanelId(Uuid)`
- `BindingId(Uuid)`
- `TrajectoryId(Uuid)`
- `DatasetShardId(Uuid)`
- `TrainingRunId(Uuid)`
- `ModelArtifactId(Uuid)`
- `PredictionId(Uuid)`
- `SkillId(Uuid)`
- `PlanTraceId(Uuid)`
- `GuardDecisionId(Uuid)`
- `VerificationRunId(Uuid)`

Rules:

- string ids must be ASCII, lowercase, and match `[a-z][a-z0-9_\\-.]{2,127}`;
- UUID ids must be generated once by the writer and then treated as immutable keys;
- deterministic ids may be used only where the domain pack declares an idempotency key.

## Domain Pack

`DomainPack` describes a domain without code changes:

```text
id
version
title
schema_version
state_schema
action_schema
outcome_schema
entity_schema
time_schema
instrument_specs
adapter_specs
objective_specs
invariants
dataset_policy
planner_policy
verification_policy
```

Validation must reject:

- duplicate instrument ids;
- references to undeclared state/action/outcome fields;
- required instruments without an adapter path;
- objective specs with missing targets;
- unsupported schema versions;
- empty state/action/outcome schemas;
- non-deterministic defaults.

## Universal Event Records

DynamicJEPA separates raw event capture from normalized state/action/outcome records.

Records:

- `RawDomainEvent`: original input payload, source metadata, ingestion id, adapter id, and content hash.
- `NormalizedState`: canonical typed state fields before an action or observation.
- `NormalizedAction`: typed action fields, actor, target, parameters, and precondition references.
- `NormalizedOutcome`: typed outcome fields, reward/utility signals if present, and observed next-state references.
- `StateTransition`: links prior state, action, outcome, next state, and timestamp.

Rules:

- raw event storage must happen before adapter output storage;
- normalized records must carry source raw event ids;
- invalid adapter output must not write normalized records;
- a transition is complete only when it has prior state, action, outcome, and next-state references unless the domain declares observation-only transitions.

## Instruments

`InstrumentSpec` declares how a piece of state/action/outcome evidence becomes a measurable signal.

Fields:

- `instrument_id`
- `kind`: scalar, categorical, text_embedding, sparse_terms, graph_neighborhood, temporal, multimodal, custom
- `input_fields`
- `output_shape`
- `normalization`
- `required`
- `model_ref`
- `version`
- `validation`

`InstrumentReading` records the actual output:

- `reading_id`
- `event_id`
- `instrument_id`
- `input_hash`
- `output_vector`
- `output_sparse`
- `output_json`
- `shape`
- `status`
- `error`

Rules:

- required instruments must succeed or the panel materialization fails;
- optional instruments are allowed only when declared optional by the domain pack;
- shape mismatches are fatal;
- NaN and infinity are fatal;
- output values must not be silently clipped unless clipping is explicitly declared and logged.

## Latent Panel

`LatentPanel` is the multi-instrument representation used by JEPA.

Fields:

- `panel_id`
- `event_id`
- `state_id`
- `action_id`
- `outcome_id`
- `domain_pack_id`
- `instrument_reading_ids`
- `ordered_slots`
- `slot_vectors`
- `slot_masks`
- `panel_hash`
- `materializer_version`

Rules:

- slot order is defined by the persisted domain pack, not by hash map iteration;
- every required slot must have a reading;
- `slot_masks` represent intentionally modeled absence, not hidden failure;
- panel hash must be reproducible from sorted readings and slot order.

## Binding Records

`BindingRecord` connects entities, concepts, actions, goals, and latent slots across events or domains.

Fields:

- `binding_id`
- `binding_kind`
- `left_ref`
- `right_ref`
- `evidence_refs`
- `score`
- `method`
- `version`

Rules:

- binding method must be registered;
- evidence refs must read back from source-of-truth storage;
- scores must be finite and within `[0.0, 1.0]`;
- cross-domain bindings must include both domain pack ids.

## Trajectories And Dataset Shards

`TrajectoryRecord`:

- ordered transition ids;
- panel ids;
- binding ids;
- time span;
- entity/session ids;
- segmentation policy id;
- trajectory hash.

`DatasetShardRecord`:

- dataset id;
- shard id;
- source trajectory ids;
- row count;
- panel shapes;
- split name;
- leakage policy;
- negative sampling policy;
- source hashes.

Rules:

- split assignment must be deterministic;
- dataset rows must reference persisted panel ids;
- no row may include a future outcome in its input fields unless explicitly training a retrospective diagnostic objective;
- every compiled shard must be readable and countable from RocksDB.

## Predictor, Artifact, And Planning Records

Define:

- `TrainingRunRecord`
- `ModelArtifactRecord`
- `PredictionRecord`
- `SkillPolicyRecord`
- `PlanTraceRecord`
- `GuardDecisionRecord`
- `SurpriseEventRecord`
- `VerificationRunRecord`

Rules:

- a prediction must reference a registered model artifact and persisted input panel;
- a plan trace must reference predictions and guard decisions used to choose an action;
- surprise events must reference prediction id, observed outcome id, error metrics, and threshold policy;
- verification run records must include before/after counts and decoded record excerpts.

## Error Contract

Add `DynamicJepaError` in core and implement conversion to `ContextGraphError`.

Required variants:

- `DomainPackNotFound`
- `DomainPackVersionMismatch`
- `SchemaValidationFailed`
- `AdapterFailed`
- `InstrumentFailed`
- `RequiredInstrumentMissing`
- `PanelShapeMismatch`
- `BindingEvidenceMissing`
- `TrajectoryInvariantViolation`
- `DatasetLeakageDetected`
- `TrainingFailed`
- `ArtifactHashMismatch`
- `PredictionInputMissing`
- `GuardRejected`
- `VerificationFailed`
- `StorageInvariantViolation`

Every error should include operation, id, domain pack id, and remediation hint where possible.
