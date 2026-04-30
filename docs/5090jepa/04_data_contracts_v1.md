# 04 — Data Contracts (v1, demo subset)

The full DynamicJEPA contract is in `docs/dynamicjepa/implementationplan/03_core_data_contracts.md`. This file lists the *trimmed v1 subset* used by the 5090 demo. Every record below is added to `crates/context-graph-core/src/dynamicjepa/`. Module layout follows the full contract:

Current implementation status as of the Phase 4 double-check on 2026-04-30: records needed through `RawDomainEvent`, `AdapterRunRecord`, normalized state/action/outcome/transition, `InstrumentReading`, and `LatentPanel` are implemented and FSV-verified. Later records remain defined for schema completeness and are wired in Phase 5+.

```
crates/context-graph-core/src/dynamicjepa/
  mod.rs
  ids.rs
  error.rs
  record_header.rs
  domain_pack.rs
  schema.rs
  instrument.rs
  adapter.rs
  event.rs
  state_action_outcome.rs
  panel.rs
  binding.rs
  trajectory.rs
  dataset.rs
  predictor.rs
  artifact.rs
  planner.rs
  skill.rs
  guard.rs
  verification.rs
  validation.rs
  tests.rs
```

Records not used by the demo (`SkillPolicyRecord`, `SurpriseEventRecord` heavyweight fields) still get *defined* (so the storage schema is whole) but their write paths are wired in Phases 8/10 only. Definitions never use `Option<>` to paper over missing data — every required field is required.

## 1. Shared Record Header

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DjRecordHeader {
    pub record_id: Uuid,
    pub record_version: u8,           // must equal storage codec version byte
    pub domain_pack_id: DomainPackId,
    pub domain_pack_version: String,  // semver, e.g. "1.0.0"
    pub created_at_unix_ms: i64,
    pub source_run_id: Option<Uuid>,  // None only for ingest-time raw events
    pub content_hash: [u8; 32],       // SHA-256 over canonical bincode of payload, excluding header
}
```

Validation rules (in `validation.rs`, called by every record's `validate()`):

- `record_version` must match the constant for that record class.
- `domain_pack_id` must be a registered `DomainPackId` (existence checked at storage write time, not at construct time, but `validate()` enforces format).
- `domain_pack_version` must parse as semver.
- `content_hash` must be SHA-256 over the canonically serialized payload **excluding the header struct itself**. Decode path recomputes and rejects on mismatch.

Codec contract (storage side; see `05_storage_schema_v1.md`):

```
[record_version: u8][bincode(SerializableRecord)]
```

Empty bytes, wrong version, mismatched hash, or bincode failure all return `DynamicJepaError::Codec` with `expected_version`, `actual_version`, `payload_type`, and `remediation_hint`.

## 2. IDs (`ids.rs`)

Newtypes; never raw `String`/`Uuid` exchanges:

```rust
pub struct DomainPackId(pub String);
pub struct InstrumentId(pub String);
pub struct AdapterId(pub String);
pub struct EventId(pub Uuid);
pub struct StateId(pub Uuid);
pub struct ActionId(pub Uuid);
pub struct OutcomeId(pub Uuid);
pub struct TransitionId(pub Uuid);
pub struct PanelId(pub Uuid);
pub struct BindingId(pub Uuid);
pub struct TrajectoryId(pub Uuid);
pub struct DatasetShardId(pub Uuid);
pub struct DatasetId(pub Uuid);
pub struct TrainingRunId(pub Uuid);
pub struct ModelArtifactId(pub Uuid);
pub struct PredictionId(pub Uuid);
pub struct SkillId(pub Uuid);
pub struct PlanTraceId(pub Uuid);
pub struct GuardDecisionId(pub Uuid);
pub struct SurpriseEventId(pub Uuid);
pub struct VerificationRunId(pub Uuid);
```

Rules:

- string ids must match `^[a-z][a-z0-9_\\-.]{2,127}$` (ASCII, lowercase). Reject otherwise.
- UUID ids are generated once by the writer (`Uuid::new_v4()`), not derived deterministically — the only deterministic ids are the string newtypes (`DomainPackId`, `InstrumentId`, `AdapterId`).
- IDs implement `Display`, `FromStr`, `Hash`, `Eq`, `Ord` so they can be CLI args, map keys, and bincode‑serialized.

## 3. Domain Pack (`domain_pack.rs`)

```rust
pub struct DomainPack {
    pub header: DjRecordHeader,
    pub id: DomainPackId,
    pub version: String,
    pub title: String,
    pub schema_version: u8,                 // 1 for the demo
    pub state_schema: StateSchema,
    pub action_schema: ActionSchema,
    pub outcome_schema: OutcomeSchema,
    pub entity_schema: EntitySchema,        // may be empty for counter_world
    pub time_schema: TimeSchema,
    pub instrument_specs: Vec<InstrumentSpec>,
    pub adapter_specs: Vec<AdapterSpec>,    // ≥1 required
    pub objective_specs: Vec<ObjectiveSpec>,// ≥1 required
    pub invariants: Vec<InvariantSpec>,
    pub dataset_policy: DatasetPolicy,
    pub planner_policy: PlannerPolicy,
    pub verification_policy: VerificationPolicy,
    pub source_hash: [u8; 32],              // SHA-256 of the operator-supplied TOML file
}
```

Schemas (each is a typed list of fields, no free-form JSON):

```rust
pub struct StateSchema { pub fields: Vec<FieldSpec> }
pub struct ActionSchema { pub fields: Vec<FieldSpec> }
pub struct OutcomeSchema { pub fields: Vec<FieldSpec> }
pub struct EntitySchema { pub fields: Vec<FieldSpec> }
pub struct TimeSchema { pub field: FieldSpec }     // exactly one timestamp field

pub struct FieldSpec {
    pub name: String,
    pub kind: FieldKind,
    pub required: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

pub enum FieldKind {
    I64, F64, Bool, String,
    Categorical { variants: Vec<String> },
    Vector { dim: usize },
    UnixMs,
}
```

Validation rejects:

- empty `id` or `version`;
- duplicate field names within a schema;
- duplicate `instrument_id` across `instrument_specs`;
- duplicate `adapter_id` across `adapter_specs`;
- required instrument with no adapter producing its inputs;
- objective target field that does not exist in schemas;
- planner policy referencing an undeclared action field;
- non‑ASCII id;
- environment‑variable substitution (`${...}`) in any string field;
- TOML containing keys not in the strict schema (unknown keys = fail).

`DomainPack::canonical_hash()` recomputes the source hash deterministically; it must be reproducible across machines from the same TOML bytes.

## 4. Instruments & Readings (`instrument.rs`)

```rust
pub struct InstrumentSpec {
    pub instrument_id: InstrumentId,
    pub kind: InstrumentKind,             // demo: Scalar | Categorical | Onehot | Time
    pub input_fields: Vec<String>,        // dotted paths, e.g. "state.counter"
    pub output_shape: Vec<usize>,         // strict, no -1 wildcards
    pub normalization: Normalization,     // None | StandardScore { mean, std }
    pub required: bool,
    pub model_ref: Option<String>,        // None for v1 (no learned instruments)
    pub version: u8,
    pub validation: InstrumentValidation,
}

pub enum InstrumentKind {
    Scalar,
    Categorical { variants: Vec<String> },
    Onehot { dim: usize },
    Time { encoding: TimeEncoding },      // FractionalDayOfWeek | UnixSeconds
}

pub struct InstrumentValidation {
    pub require_finite: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub reject_nan: bool,
    pub reject_inf: bool,
}

pub struct InstrumentReading {
    pub header: DjRecordHeader,
    pub reading_id: Uuid,
    pub event_id: EventId,
    pub instrument_id: InstrumentId,
    pub instrument_hash: [u8; 16],        // BLAKE3-128 of (instrument_id, version, output_shape)
    pub input_hash: [u8; 32],             // SHA-256 over the input field bytes
    pub output_dense: Vec<f32>,           // shape per spec, flattened row-major
    pub status: ReadingStatus,            // Ok | Failed { error: String }
}
```

Rules:

- required instruments must succeed for the panel to materialize. Optional instruments with `Failed` status produce a slot mask but never substitute zeros.
- shape mismatch, NaN, infinity → `DynamicJepaError::InstrumentFailed` with field path and expected vs actual shape.
- reading is never written without a matching adapter run completion record.

## 5. Latent Panel (`panel.rs`)

```rust
pub struct LatentPanel {
    pub header: DjRecordHeader,
    pub panel_id: PanelId,
    pub event_id: EventId,
    pub state_id: StateId,
    pub action_id: ActionId,
    pub outcome_id: Option<OutcomeId>,    // None for the final event of an open trajectory
    pub instrument_reading_ids: Vec<Uuid>,
    pub ordered_slots: Vec<PanelSlot>,    // order = domain_pack.instrument_order()
    pub slot_vectors: Vec<Vec<f32>>,      // parallel to ordered_slots
    pub slot_masks: Vec<bool>,            // true = present, false = intentionally absent
    pub panel_hash: [u8; 32],
    pub materializer_version: u8,
}

pub struct PanelSlot {
    pub instrument_id: InstrumentId,
    pub dim: usize,
    pub kind: PanelSlotKind,              // State | Action | Outcome | Time
}
```

Validation:

- `slot_count == ordered_slots.len() == slot_vectors.len() == slot_masks.len()`;
- every required slot has `mask == true`;
- vector dim matches slot dim when `mask == true`;
- optional failed slots must have `mask == false` and an empty vector; substituting a zero vector is forbidden;
- no NaN/Inf;
- `panel_hash` reproducible from `(domain_pack_id, sorted instrument_reading_ids, ordered_slots, slot_vectors)`.

## 6. Universal Event / State / Action / Outcome / Transition (`event.rs`, `state_action_outcome.rs`)

```rust
pub struct RawDomainEvent {
    pub header: DjRecordHeader,
    pub event_id: EventId,
    pub domain_pack_id: DomainPackId,
    pub adapter_id: AdapterId,
    pub source_kind: SourceKind,           // JsonlFixture for the demo
    pub source_uri: String,                // file path for fixtures
    pub source_offset: u64,                // byte offset of the line in the JSONL
    pub payload_format: PayloadFormat,     // Json
    pub payload_bytes: Vec<u8>,            // exact raw line bytes
    pub payload_hash: [u8; 32],
    pub received_at_unix_ms: i64,
}

pub struct NormalizedState   { pub header: DjRecordHeader, pub state_id: StateId, pub fields: BTreeMap<String, FieldValue>, pub source_event_id: EventId }
pub struct NormalizedAction  { pub header: DjRecordHeader, pub action_id: ActionId, pub fields: BTreeMap<String, FieldValue>, pub source_event_id: EventId, pub action_origin: ActionOrigin }
pub struct NormalizedOutcome { pub header: DjRecordHeader, pub outcome_id: OutcomeId, pub fields: BTreeMap<String, FieldValue>, pub source_event_id: EventId }
pub struct StateTransition   { pub header: DjRecordHeader, pub transition_id: TransitionId, pub prior_state: StateId, pub action: ActionId, pub outcome: OutcomeId, pub next_state: StateId, pub timestamp_ms: i64 }

pub enum ActionOrigin { Observed, Hypothetical, Planned }   // demo: Observed for ingestion; Hypothetical when planner generates candidates

pub enum FieldValue {
    I64(i64), F64(f64), Bool(bool), String(String),
    Categorical { variant: String },
    Vector(Vec<f32>),
    UnixMs(i64),
}
```

Rules:

- a transition is complete only with prior + action + outcome + next state;
- normalized records must reference their source event; ingestion fails if not;
- adapter parse failure writes an `AdapterRunRecord` with `status = Failed` *if* the raw event is already persisted, otherwise no rows.

## 7. Binding, Trajectory, Dataset (`binding.rs`, `trajectory.rs`, `dataset.rs`)

Demo records are exactly per the full contract; key trimmed fields:

```rust
pub struct BindingRecord {
    pub header: DjRecordHeader,
    pub binding_id: BindingId,
    pub binding_kind: BindingKind,        // EntityToEntity | EventToTrajectory | StateToGoal | PanelToMemory
    pub left_ref: BindingRef,             // (CF, key bytes)
    pub right_ref: BindingRef,
    pub evidence_refs: Vec<BindingRef>,
    pub score: f32,                       // [0.0, 1.0]
    pub method: BindingMethod,            // demo: IdEquality | ExplicitMapping
    pub left_domain_pack_id: DomainPackId,
    pub right_domain_pack_id: DomainPackId,
    pub created_by_run_id: Uuid,
    pub version: u8,
}

pub struct TrajectoryRecord {
    pub header: DjRecordHeader,
    pub trajectory_id: TrajectoryId,
    pub segmentation_policy_id: String,   // demo: "by_domain_session"
    pub ordered_transition_ids: Vec<TransitionId>,
    pub ordered_panel_ids: Vec<PanelId>,
    pub start_time_unix_ms: i64,
    pub end_time_unix_ms: i64,
    pub entity_refs: Vec<String>,
    pub binding_refs: Vec<BindingId>,
    pub trajectory_hash: [u8; 32],
    pub record_count: u32,
}

pub struct DatasetShardRecord {
    pub header: DjRecordHeader,
    pub dataset_id: DatasetId,
    pub shard_id: DatasetShardId,
    pub source_trajectory_ids: Vec<TrajectoryId>,
    pub split_name: String,               // "train" | "val" | "test"
    pub row_count: u32,
    pub input_panel_ids: Vec<PanelId>,
    pub target_panel_ids: Vec<PanelId>,
    pub action_ids: Vec<ActionId>,
    pub negative_panel_ids: Vec<PanelId>, // parallel to action_ids; one negative per row
    pub objective_ids: Vec<String>,       // demo: ["latent_delta"]
    pub shape_summary: ShapeSummary,
    pub source_hashes: Vec<[u8; 32]>,
    pub leakage_report: LeakageReport,
    pub compiler_version: u8,
}
```

Validation: monotonic timestamps, no duplicate transitions, deterministic hashes, no future leakage in input panels, every negative reference must read back from `CF_DJ_LATENT_PANELS`.

## 8. Predictor / Artifact / Prediction (`predictor.rs`, `artifact.rs`)

```rust
pub struct TrainingRunRecord {
    pub header: DjRecordHeader,
    pub training_run_id: TrainingRunId,
    pub domain_pack_id: DomainPackId,
    pub dataset_id: DatasetId,
    pub started_at_unix_ms: i64,
    pub finished_at_unix_ms: Option<i64>,
    pub status: TrainingRunStatus,        // Started | Completed | Failed { error: String }
    pub training_config_hash: [u8; 32],
    pub objective_ids: Vec<String>,
    pub metrics: BTreeMap<String, f64>,
    pub artifact_ids: Vec<ModelArtifactId>,
}

pub struct ModelArtifactRecord {
    pub header: DjRecordHeader,
    pub artifact_id: ModelArtifactId,
    pub training_run_id: TrainingRunId,
    pub domain_pack_id: DomainPackId,
    pub domain_pack_version: String,
    pub dataset_id: DatasetId,
    pub artifact_root: PathBuf,           // canonical absolute path
    pub files: Vec<ArtifactFile>,         // each with relative_path, sha256, size_bytes
    pub model_config_hash: [u8; 32],
    pub evaluation_report_hash: [u8; 32],
    pub created_at_unix_ms: i64,
    pub status: ArtifactStatus,           // Active | Quarantined { reason: String }
}

pub struct PredictionRecord {
    pub header: DjRecordHeader,
    pub prediction_id: PredictionId,
    pub model_artifact_id: ModelArtifactId,
    pub model_artifact_hash_at_inference: [u8; 32],
    pub input_panel_id: PanelId,
    pub candidate_action_id: ActionId,
    pub predicted_next_panel_vec: Vec<f32>,
    pub uncertainty: f32,
    pub objective_scores: BTreeMap<String, f32>,
    pub created_at_unix_ms: i64,
}
```

Rule: a `PredictionRecord` is **never** written before the model artifact's on‑disk SHA‑256 is recomputed and compared to the registry value at inference time. Mismatch → typed error, no row.

## 9. Plan Trace, Guard Decision, Surprise (`planner.rs`, `guard.rs`)

Records exactly per the full contract; demo specifics:

- `SkillPolicyRecord` exists but the demo's only skill is `enumerate_declared_actions` which generates the candidate set from the domain pack's declared action schema (e.g. `up/down/left/right/noop` for gridworld, `delta ∈ {-2,-1,0,1,2}` for counter_world).
- `GuardDecisionRecord` for the demo only carries one guard: `bounds_check`, which rejects predicted next panels whose state field exits the declared `[min, max]` of the domain pack. Other guards left as enum variants for future use.
- `SurpriseEventRecord` is written only when `cosine(predicted_next_panel, observed_next_panel) < domain_pack.surprise_threshold`. Default threshold from domain pack TOML; demo uses 0.85.

## 10. Verification (`verification.rs`)

```rust
pub struct VerificationRunRecord {
    pub header: DjRecordHeader,
    pub verification_run_id: VerificationRunId,
    pub test_name: String,                // "verify-counter-world" | "verify-gridworld"
    pub db_path_hash: [u8; 32],           // SHA-256 of canonical absolute path
    pub fixture_hashes: Vec<[u8; 32]>,
    pub before_counts: BTreeMap<String, u64>,    // CF name -> count
    pub after_counts: BTreeMap<String, u64>,
    pub commands_executed: Vec<String>,
    pub artifact_hash_checks: Vec<ArtifactHashCheck>,
    pub decoded_record_excerpts: BTreeMap<String, DjJsonValue>,  // CF -> 1 sample; bincode-safe JSON enum
    pub expected_results: BTreeMap<String, DjJsonValue>,
    pub actual_results: BTreeMap<String, DjJsonValue>,
    pub status: VerificationStatus,       // Passed | Failed { failure_details: String }
    pub created_at_unix_ms: i64,
}
```

Every FSV harness ends by writing one of these. Operator evidence logs are derived from `decoded_record_excerpts` + `actual_results`.

Phase 1 correction: use a local bincode-safe `DjJsonValue` enum instead of raw
`serde_json::Value` inside persisted records. `serde_json::Value` requires
`deserialize_any`, which `bincode 1.3` does not support, so using it would make
`VerificationRunRecord` impossible to round-trip under the no-fallback codec
contract. CLI/MCP layers can convert `DjJsonValue` to JSON at the boundary.

## 11. Errors (`error.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum DynamicJepaError {
    #[error("validation failed: {message} (field={field})")]
    Validation { message: String, field: String, remediation: String },
    #[error("source-of-truth missing: {cf} key={key:?}")]
    SourceOfTruthMissing { cf: String, key: Vec<u8> },
    #[error("storage: {operation} cf={cf}: {message}")]
    Storage { operation: String, cf: String, message: String, remediation: String },
    #[error("codec: expected_version={expected} actual_version={actual} type={payload_type}")]
    Codec { expected: u8, actual: u8, payload_type: String, remediation: String },
    #[error("instrument {instrument_id} failed for event {event_id}: {message}")]
    InstrumentFailed { instrument_id: String, event_id: Uuid, message: String, field_path: String },
    #[error("required instrument {instrument_id} missing for transition {transition_id}")]
    RequiredInstrumentMissing { instrument_id: String, transition_id: Uuid },
    #[error("panel shape mismatch: domain_pack={domain_pack_id} expected={expected:?} actual={actual:?}")]
    PanelShapeMismatch { domain_pack_id: String, expected: Vec<usize>, actual: Vec<usize> },
    #[error("binding evidence missing: binding={binding_id} ref={evidence_ref}")]
    BindingEvidenceMissing { binding_id: Uuid, evidence_ref: String },
    #[error("trajectory invariant violation: {message}")]
    TrajectoryInvariantViolation { message: String, trajectory_id: Uuid },
    #[error("dataset leakage detected: {message}")]
    DatasetLeakageDetected { message: String, dataset_id: Uuid },
    #[error("training failed: run={training_run_id}: {message}")]
    TrainingFailed { training_run_id: Uuid, message: String, remediation: String },
    #[error("artifact hash mismatch: artifact={artifact_id} file={file}")]
    ArtifactHashMismatch { artifact_id: Uuid, file: String, expected: String, actual: String },
    #[error("prediction input missing: {target_id}")]
    PredictionInputMissing { target_id: String, cf: String },
    #[error("guard rejected action: guard={guard_id} reason={reason_code}")]
    GuardRejected { guard_id: String, reason_code: String, action_id: Uuid },
    #[error("verification failed: {test_name}: {message}")]
    VerificationFailed { test_name: String, message: String, evidence_path: String },
    #[error("storage invariant violation: {message}")]
    StorageInvariantViolation { message: String },
    #[error("domain pack not found: {id}")]
    DomainPackNotFound { id: String },
    #[error("domain pack version mismatch: id={id} expected={expected} actual={actual}")]
    DomainPackVersionMismatch { id: String, expected: String, actual: String },
    #[error("schema validation failed: {message} (field={field})")]
    SchemaValidationFailed { message: String, field: String, remediation: String },
    #[error("adapter failed: adapter={adapter_id} event={event_id}: {message}")]
    AdapterFailed { adapter_id: String, event_id: Uuid, message: String, field_path: String },
}

impl From<DynamicJepaError> for ContextGraphError { /* implement */ }
```

Every variant carries enough context to debug without running the operation again. Generic `String` errors are forbidden — always a typed variant.
