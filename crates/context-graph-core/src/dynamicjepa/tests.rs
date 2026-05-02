use super::*;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

const TS0: i64 = 1_700_000_000_000;

fn root_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn uuid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn hash32(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn file_state(path: &Path) -> serde_json::Value {
    if !path.exists() {
        return json!({"exists": false, "size_bytes": 0, "sha256": null});
    }
    let bytes = fs::read(path).expect("read file state");
    json!({
        "exists": true,
        "size_bytes": bytes.len(),
        "sha256": hex32(&sha256(&bytes)),
    })
}

fn count_files_recursive(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }
    let mut count = 0;
    for entry in fs::read_dir(path).expect("read recursive file count dir") {
        let entry = entry.expect("read recursive file count entry");
        let file_type = entry.file_type().expect("read recursive file type");
        if file_type.is_file() {
            count += 1;
        } else if file_type.is_dir() {
            count += count_files_recursive(&entry.path());
        }
    }
    count
}

fn domain_id() -> DomainPackId {
    DomainPackId::new("counter_world").expect("counter_world id must validate")
}

fn header(record_id: Uuid, version: u8) -> DjRecordHeader {
    header_for_domain(record_id, version, domain_id())
}

fn header_for_domain(record_id: Uuid, version: u8, domain_pack_id: DomainPackId) -> DjRecordHeader {
    DjRecordHeader::new(
        record_id,
        version,
        domain_pack_id,
        "1.0.0",
        TS0,
        Some(uuid(900)),
    )
}

fn seal<R: DynamicJepaRecord>(mut record: R) -> R {
    record
        .refresh_content_hash()
        .expect("record content hash must compute");
    record
        .validate_record()
        .expect("sealed record must validate before persistence");
    record
}

fn write_unchecked<R: serde::Serialize>(record: &R, version: u8, path: &Path) {
    let mut body = bincode::serialize(record).expect("test record should bincode serialize");
    let mut bytes = Vec::with_capacity(body.len() + 1);
    bytes.push(version);
    bytes.append(&mut body);
    fs::write(path, bytes).expect("write corrupt source-of-truth bytes");
}

fn roundtrip_from_file<R>(dir: &Path, name: &str, record: &R) -> R
where
    R: DynamicJepaRecord + DeserializeOwned + PartialEq + Debug,
{
    let bytes = encode_versioned_record(record).expect("encode must validate and serialize");
    let path = dir.join(format!("{name}.bin"));
    fs::write(&path, &bytes).expect("write source-of-truth bincode file");

    let raw_from_sot = fs::read(&path).expect("read source-of-truth bincode file");
    assert_eq!(raw_from_sot, bytes, "source-of-truth bytes changed on disk");
    let decoded: R =
        decode_versioned_record(&raw_from_sot).expect("decode from source-of-truth bytes");
    assert_eq!(
        &decoded, record,
        "decoded record differs from persisted record"
    );
    decoded
}

#[derive(Debug, Deserialize)]
struct DomainPackToml {
    domain: DomainToml,
    time: TimeToml,
    state: SchemaToml,
    action: SchemaToml,
    outcome: SchemaToml,
    entity: SchemaToml,
    instruments: Vec<InstrumentToml>,
    adapters: Vec<AdapterToml>,
    objectives: Vec<ObjectiveToml>,
    invariants: Vec<InvariantToml>,
    dataset_policy: DatasetPolicyToml,
    planner_policy: PlannerPolicyToml,
    verification_policy: VerificationPolicyToml,
}

#[derive(Debug, Deserialize)]
struct DomainToml {
    id: String,
    version: String,
    title: String,
    schema_version: u8,
}

#[derive(Debug, Deserialize)]
struct TimeToml {
    field: FieldToml,
}

#[derive(Debug, Deserialize)]
struct SchemaToml {
    fields: Vec<FieldToml>,
}

#[derive(Debug, Deserialize)]
struct FieldToml {
    name: String,
    kind: TomlKind,
    required: bool,
    min: Option<f64>,
    max: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TomlKind {
    Name(String),
    Table {
        #[serde(rename = "type")]
        kind_type: String,
        variants: Option<Vec<String>>,
        dim: Option<usize>,
        encoding: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct InstrumentToml {
    id: String,
    kind: TomlKind,
    input_fields: Vec<String>,
    output_shape: Vec<usize>,
    normalization: NormalizationToml,
    required: bool,
    model_ref: Option<String>,
    pair_kinds: Vec<String>,
    version: u8,
    validation: InstrumentValidationToml,
}

#[derive(Debug, Deserialize)]
struct NormalizationToml {
    #[serde(rename = "type")]
    norm_type: String,
    mean: Option<f64>,
    std: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct InstrumentValidationToml {
    require_finite: bool,
    min: Option<f64>,
    max: Option<f64>,
    reject_nan: bool,
    reject_inf: bool,
}

#[derive(Debug, Deserialize)]
struct AdapterToml {
    id: String,
    kind: String,
    version: u8,
    mapping: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ObjectiveToml {
    id: String,
    kind: String,
    input_panel: String,
    target: String,
    loss_weight: f64,
    required_dataset_fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct InvariantToml {
    id: String,
    expression: String,
    severity: String,
}

#[derive(Debug, Deserialize)]
struct DatasetPolicyToml {
    default_split: String,
    split_key: Option<String>,
    split_buckets: Option<BTreeMap<String, f64>>,
    default_objective: String,
    negative_sampling: String,
    min_negatives_per_row: u32,
}

#[derive(Debug, Deserialize)]
struct PlannerPolicyToml {
    candidate_actions: CandidateActionsToml,
    guards: Vec<String>,
    surprise_threshold: f32,
}

#[derive(Debug, Deserialize)]
struct CandidateActionsToml {
    kind: String,
    deltas: Option<Vec<i64>>,
    moves: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct VerificationPolicyToml {
    required_cfs: Vec<String>,
    expected_event_count: Option<u64>,
    expected_panel_count: Option<u64>,
    expected_dataset_row_count: Option<u64>,
    expected_trajectory_count: Option<u64>,
    expected_train_rows_min: Option<u64>,
    expected_train_rows_max: Option<u64>,
    expected_val_rows_min: Option<u64>,
    expected_val_rows_max: Option<u64>,
    expected_test_rows_min: Option<u64>,
    expected_test_rows_max: Option<u64>,
}

fn validation_error(field: impl Into<String>, message: impl Into<String>) -> DynamicJepaError {
    DynamicJepaError::validation(
        field,
        message,
        "fix the Phase 0 TOML source of truth; Phase 1 does not substitute defaults",
    )
}

fn field_kind_from_toml(kind: TomlKind, field: &str) -> DynamicJepaResult<FieldKind> {
    match kind {
        TomlKind::Name(name) => match name.as_str() {
            "i64" => Ok(FieldKind::I64),
            "f64" => Ok(FieldKind::F64),
            "bool" => Ok(FieldKind::Bool),
            "string" => Ok(FieldKind::String),
            "unix_ms" => Ok(FieldKind::UnixMs),
            _ => Err(validation_error(
                field,
                format!("unsupported field kind name {name:?}"),
            )),
        },
        TomlKind::Table {
            kind_type,
            variants,
            dim,
            encoding: _,
        } => match kind_type.as_str() {
            "i64" => Ok(FieldKind::I64),
            "f64" => Ok(FieldKind::F64),
            "bool" => Ok(FieldKind::Bool),
            "string" => Ok(FieldKind::String),
            "unix_ms" => Ok(FieldKind::UnixMs),
            "categorical" => Ok(FieldKind::Categorical {
                variants: variants.ok_or_else(|| {
                    validation_error(field, "categorical field kind missing variants")
                })?,
            }),
            "vector" => Ok(FieldKind::Vector {
                dim: dim.ok_or_else(|| validation_error(field, "vector field kind missing dim"))?,
            }),
            _ => Err(validation_error(
                field,
                format!("unsupported field kind table type {kind_type:?}"),
            )),
        },
    }
}

fn instrument_kind_from_toml(kind: TomlKind, field: &str) -> DynamicJepaResult<InstrumentKind> {
    match kind {
        TomlKind::Name(name) => match name.as_str() {
            "scalar" => Ok(InstrumentKind::Scalar),
            _ => Err(validation_error(
                field,
                format!("unsupported instrument kind name {name:?}"),
            )),
        },
        TomlKind::Table {
            kind_type,
            variants,
            dim,
            encoding,
        } => match kind_type.as_str() {
            "scalar" => Ok(InstrumentKind::Scalar),
            "categorical" => Ok(InstrumentKind::Categorical {
                variants: variants.ok_or_else(|| {
                    validation_error(field, "categorical instrument kind missing variants")
                })?,
            }),
            "onehot" => Ok(InstrumentKind::Onehot {
                dim: dim
                    .ok_or_else(|| validation_error(field, "onehot instrument kind missing dim"))?,
            }),
            "time" => {
                let encoding = match encoding.as_deref() {
                    Some("fractional_day_of_week") => TimeEncoding::FractionalDayOfWeek,
                    Some("unix_seconds") => TimeEncoding::UnixSeconds,
                    other => {
                        return Err(validation_error(
                            field,
                            format!("unsupported time encoding {other:?}"),
                        ))
                    }
                };
                Ok(InstrumentKind::Time { encoding })
            }
            _ => Err(validation_error(
                field,
                format!("unsupported instrument kind table type {kind_type:?}"),
            )),
        },
    }
}

fn field_spec_from_toml(field: FieldToml, path: &str) -> DynamicJepaResult<FieldSpec> {
    Ok(FieldSpec {
        name: field.name,
        kind: field_kind_from_toml(field.kind, path)?,
        required: field.required,
        min: field.min,
        max: field.max,
    })
}

fn fields_from_toml(fields: Vec<FieldToml>, path: &str) -> DynamicJepaResult<Vec<FieldSpec>> {
    fields
        .into_iter()
        .enumerate()
        .map(|(idx, field)| field_spec_from_toml(field, &format!("{path}.fields[{idx}]")))
        .collect()
}

fn normalization_from_toml(
    value: NormalizationToml,
    field: &str,
) -> DynamicJepaResult<Normalization> {
    match value.norm_type.as_str() {
        "none" => Ok(Normalization::None),
        "standard_score" => Ok(Normalization::StandardScore {
            mean: value
                .mean
                .ok_or_else(|| validation_error(field, "standard_score missing mean"))?,
            std: value
                .std
                .ok_or_else(|| validation_error(field, "standard_score missing std"))?,
        }),
        other => Err(validation_error(
            field,
            format!("unsupported normalization type {other:?}"),
        )),
    }
}

fn domain_pack_from_phase0_toml(path: &Path, record_id: Uuid) -> DynamicJepaResult<DomainPack> {
    let bytes = fs::read(path).map_err(|err| {
        validation_error(
            path.display().to_string(),
            format!("failed to read Phase 0 TOML source of truth: {err}"),
        )
    })?;
    let source_hash = sha256(&bytes);
    let source_text = std::str::from_utf8(&bytes).map_err(|err| {
        validation_error(
            path.display().to_string(),
            format!("Phase 0 TOML source of truth is not UTF-8: {err}"),
        )
    })?;
    let parsed: DomainPackToml = toml::from_str(source_text).map_err(|err| {
        validation_error(
            path.display().to_string(),
            format!("failed to parse Phase 0 TOML source of truth: {err}"),
        )
    })?;
    let domain_id = DomainPackId::new(parsed.domain.id)?;
    let candidate_actions = match parsed.planner_policy.candidate_actions.kind.as_str() {
        "enumerated" => {
            if let Some(deltas) = parsed.planner_policy.candidate_actions.deltas {
                CandidateActionConfig::EnumeratedDeltas { deltas }
            } else if let Some(moves) = parsed.planner_policy.candidate_actions.moves {
                CandidateActionConfig::EnumeratedMoves { moves }
            } else {
                return Err(validation_error(
                    "planner_policy.candidate_actions",
                    "enumerated candidate actions require deltas or moves",
                ));
            }
        }
        other => {
            return Err(validation_error(
                "planner_policy.candidate_actions.kind",
                format!("unsupported candidate action kind {other:?}"),
            ))
        }
    };

    Ok(seal(DomainPack {
        header: header_for_domain(record_id, DOMAIN_PACK_RECORD_VERSION, domain_id.clone()),
        id: domain_id,
        version: parsed.domain.version,
        title: parsed.domain.title,
        schema_version: parsed.domain.schema_version,
        state_schema: StateSchema {
            fields: fields_from_toml(parsed.state.fields, "state")?,
        },
        action_schema: ActionSchema {
            fields: fields_from_toml(parsed.action.fields, "action")?,
        },
        outcome_schema: OutcomeSchema {
            fields: fields_from_toml(parsed.outcome.fields, "outcome")?,
        },
        entity_schema: EntitySchema {
            fields: fields_from_toml(parsed.entity.fields, "entity")?,
        },
        time_schema: TimeSchema {
            field: field_spec_from_toml(parsed.time.field, "time.field")?,
        },
        instrument_specs: parsed
            .instruments
            .into_iter()
            .enumerate()
            .map(|(idx, spec)| {
                Ok(InstrumentSpec {
                    instrument_id: InstrumentId::new(spec.id)?,
                    kind: instrument_kind_from_toml(
                        spec.kind,
                        &format!("instruments[{idx}].kind"),
                    )?,
                    input_fields: spec.input_fields,
                    output_shape: spec.output_shape,
                    normalization: normalization_from_toml(
                        spec.normalization,
                        &format!("instruments[{idx}].normalization"),
                    )?,
                    required: spec.required,
                    model_ref: spec.model_ref,
                    pair_kinds: spec
                        .pair_kinds
                        .into_iter()
                        .map(|kind| PairKindName::parse(&kind))
                        .collect::<DynamicJepaResult<Vec<_>>>()?,
                    version: spec.version,
                    validation: InstrumentValidation {
                        require_finite: spec.validation.require_finite,
                        min: spec.validation.min,
                        max: spec.validation.max,
                        reject_nan: spec.validation.reject_nan,
                        reject_inf: spec.validation.reject_inf,
                    },
                })
            })
            .collect::<DynamicJepaResult<Vec<_>>>()?,
        adapter_specs: parsed
            .adapters
            .into_iter()
            .map(|adapter| {
                Ok(AdapterSpec {
                    adapter_id: AdapterId::new(adapter.id)?,
                    kind: adapter.kind,
                    version: adapter.version,
                    mapping: adapter.mapping,
                })
            })
            .collect::<DynamicJepaResult<Vec<_>>>()?,
        objective_specs: parsed
            .objectives
            .into_iter()
            .map(|objective| ObjectiveSpec {
                id: objective.id,
                kind: objective.kind,
                input_panel: objective.input_panel,
                target: objective.target,
                loss_weight: objective.loss_weight,
                required_dataset_fields: objective.required_dataset_fields,
            })
            .collect(),
        invariants: parsed
            .invariants
            .into_iter()
            .map(|invariant| InvariantSpec {
                id: invariant.id,
                expression: invariant.expression,
                severity: invariant.severity,
            })
            .collect(),
        dataset_policy: DatasetPolicy {
            default_split: parsed.dataset_policy.default_split,
            split_key: parsed.dataset_policy.split_key,
            split_buckets: parsed.dataset_policy.split_buckets,
            default_objective: parsed.dataset_policy.default_objective,
            negative_sampling: parsed.dataset_policy.negative_sampling,
            min_negatives_per_row: parsed.dataset_policy.min_negatives_per_row,
        },
        planner_policy: PlannerPolicy {
            candidate_actions,
            guards: parsed.planner_policy.guards,
            surprise_threshold: parsed.planner_policy.surprise_threshold,
        },
        constellation: None,
        verification_policy: VerificationPolicy {
            required_cfs: parsed.verification_policy.required_cfs,
            expected_event_count: parsed.verification_policy.expected_event_count,
            expected_panel_count: parsed.verification_policy.expected_panel_count,
            expected_dataset_row_count: parsed.verification_policy.expected_dataset_row_count,
            expected_trajectory_count: parsed.verification_policy.expected_trajectory_count,
            expected_train_rows_min: parsed.verification_policy.expected_train_rows_min,
            expected_train_rows_max: parsed.verification_policy.expected_train_rows_max,
            expected_val_rows_min: parsed.verification_policy.expected_val_rows_min,
            expected_val_rows_max: parsed.verification_policy.expected_val_rows_max,
            expected_test_rows_min: parsed.verification_policy.expected_test_rows_min,
            expected_test_rows_max: parsed.verification_policy.expected_test_rows_max,
        },
        source_hash,
    }))
}

fn phase0_counter_domain_pack() -> DomainPack {
    domain_pack_from_phase0_toml(
        &root_dir().join("configs/dynamicjepa/domain_packs/counter_world.v1.toml"),
        uuid(1),
    )
    .expect("counter_world Phase 0 TOML must parse into a valid Phase 1 DomainPack")
}

fn phase0_gridworld_domain_pack() -> DomainPack {
    domain_pack_from_phase0_toml(
        &root_dir().join("configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml"),
        uuid(2),
    )
    .expect("gridworld_5x5 Phase 0 TOML must parse into a valid Phase 1 DomainPack")
}

fn sample_domain_pack() -> DomainPack {
    phase0_counter_domain_pack()
}

#[derive(Clone)]
struct Samples {
    raw_event: RawDomainEvent,
    adapter_run: AdapterRunRecord,
    state: NormalizedState,
    action: NormalizedAction,
    outcome: NormalizedOutcome,
    transition: StateTransition,
    reading_counter: InstrumentReading,
    reading_delta: InstrumentReading,
    panel: LatentPanel,
    binding: BindingRecord,
    trajectory: TrajectoryRecord,
    dataset: DatasetShardRecord,
    training: TrainingRunRecord,
    artifact: ModelArtifactRecord,
    prediction: PredictionRecord,
    skill: SkillPolicyRecord,
    guard: GuardDecisionRecord,
    plan: PlanTraceRecord,
    surprise: SurpriseEventRecord,
    verification: VerificationRunRecord,
}

fn sample_records() -> (DomainPack, Samples) {
    let domain = sample_domain_pack();
    let fixture_path =
        root_dir().join("configs/dynamicjepa/verification/counter_world_happy_path.jsonl");
    let fixture = fs::read_to_string(&fixture_path).expect("phase0 fixture must exist");
    let first_line = fixture
        .lines()
        .next()
        .expect("counter fixture must have first row")
        .as_bytes()
        .to_vec();
    let event_id = EventId(uuid(10));
    let state_id = StateId(uuid(11));
    let action_id = ActionId(uuid(12));
    let outcome_id = OutcomeId(uuid(13));
    let transition_id = TransitionId(uuid(14));
    let panel_id = PanelId(uuid(15));
    let reading_counter_id = uuid(16);
    let reading_delta_id = uuid(17);
    let binding_id = BindingId(uuid(18));
    let trajectory_id = TrajectoryId(uuid(19));
    let dataset_id = DatasetId(uuid(20));
    let shard_id = DatasetShardId(uuid(21));
    let training_run_id = TrainingRunId(uuid(22));
    let artifact_id = ModelArtifactId(uuid(23));
    let prediction_id = PredictionId(uuid(24));
    let skill_id = SkillId(uuid(25));
    let guard_id = GuardDecisionId(uuid(26));
    let plan_id = PlanTraceId(uuid(27));
    let surprise_id = SurpriseEventId(uuid(28));
    let verification_id = VerificationRunId(uuid(29));

    let raw_event = seal(RawDomainEvent {
        header: header(uuid(30), RAW_DOMAIN_EVENT_RECORD_VERSION),
        event_id,
        domain_pack_id: domain_id(),
        adapter_id: AdapterId::new("json_event").unwrap(),
        source_kind: SourceKind::JsonlFixture,
        source_uri: fixture_path.display().to_string(),
        source_offset: 0,
        payload_format: PayloadFormat::Json,
        payload_hash: sha256(&first_line),
        payload_bytes: first_line,
        received_at_unix_ms: TS0,
    });

    let mut state_fields = BTreeMap::new();
    state_fields.insert("counter".to_string(), FieldValue::I64(0));
    let state = seal(NormalizedState {
        header: header(uuid(31), NORMALIZED_STATE_RECORD_VERSION),
        state_id,
        fields: state_fields,
        source_event_id: event_id,
    });

    let mut action_fields = BTreeMap::new();
    action_fields.insert("delta".to_string(), FieldValue::I64(1));
    let action = seal(NormalizedAction {
        header: header(uuid(32), NORMALIZED_ACTION_RECORD_VERSION),
        action_id,
        fields: action_fields,
        source_event_id: event_id,
        action_origin: ActionOrigin::Observed,
    });

    let mut outcome_fields = BTreeMap::new();
    outcome_fields.insert("next_counter".to_string(), FieldValue::I64(1));
    let outcome = seal(NormalizedOutcome {
        header: header(uuid(33), NORMALIZED_OUTCOME_RECORD_VERSION),
        outcome_id,
        fields: outcome_fields,
        source_event_id: event_id,
    });

    let transition = seal(StateTransition {
        header: header(uuid(34), STATE_TRANSITION_RECORD_VERSION),
        transition_id,
        prior_state: state_id,
        action: action_id,
        outcome: outcome_id,
        next_state: StateId(uuid(35)),
        timestamp_ms: TS0,
    });

    let adapter_run = seal(AdapterRunRecord {
        header: header(uuid(36), ADAPTER_RUN_RECORD_VERSION),
        adapter_run_id: uuid(37),
        adapter_id: AdapterId::new("json_event").unwrap(),
        domain_pack_id: domain_id(),
        event_id,
        started_at_unix_ms: TS0,
        finished_at_unix_ms: Some(TS0 + 1),
        status: AdapterRunStatus::Completed,
        error_code: None,
        error_message: None,
        field_path: None,
        expected_kind: None,
        actual_kind: None,
        output_state_id: Some(state_id),
        output_action_id: Some(action_id),
        output_outcome_id: Some(outcome_id),
        output_transition_id: Some(transition_id),
    });

    let reading_counter = seal(InstrumentReading {
        header: header(uuid(38), INSTRUMENT_READING_RECORD_VERSION),
        reading_id: reading_counter_id,
        event_id,
        instrument_id: InstrumentId::new("counter_scalar").unwrap(),
        instrument_hash: [1; 16],
        input_hash: hash32(2),
        output_dense: vec![0.0],
        status: ReadingStatus::Ok,
    });
    let reading_delta = seal(InstrumentReading {
        header: header(uuid(39), INSTRUMENT_READING_RECORD_VERSION),
        reading_id: reading_delta_id,
        event_id,
        instrument_id: InstrumentId::new("delta_scalar").unwrap(),
        instrument_hash: [2; 16],
        input_hash: hash32(3),
        output_dense: vec![0.25],
        status: ReadingStatus::Ok,
    });
    let panel = seal(LatentPanel {
        header: header(uuid(40), LATENT_PANEL_RECORD_VERSION),
        panel_id,
        event_id,
        state_id,
        action_id,
        outcome_id: Some(outcome_id),
        instrument_reading_ids: vec![reading_counter_id, reading_delta_id],
        pairwise_reading_ids: Vec::new(),
        ordered_slots: vec![
            PanelSlot {
                instrument_id: InstrumentId::new("counter_scalar").unwrap(),
                dim: 1,
                kind: PanelSlotKind::State,
            },
            PanelSlot {
                instrument_id: InstrumentId::new("delta_scalar").unwrap(),
                dim: 1,
                kind: PanelSlotKind::Action,
            },
        ],
        slot_vectors: vec![vec![0.0], vec![0.25]],
        slot_masks: vec![true, true],
        panel_hash: hash32(4),
        materializer_version: 1,
    });

    let binding_ref = BindingRef {
        cf: "dj_raw_events".to_string(),
        key_bytes: event_id.into_bytes().to_vec(),
    };
    let binding = seal(BindingRecord {
        header: header(uuid(41), BINDING_RECORD_VERSION),
        binding_id,
        binding_kind: BindingKind::EventToTrajectory,
        left_ref: binding_ref.clone(),
        right_ref: BindingRef {
            cf: "dj_trajectories".to_string(),
            key_bytes: trajectory_id.into_bytes().to_vec(),
        },
        evidence_refs: vec![binding_ref],
        score: 1.0,
        method: BindingMethod::IdEquality,
        left_domain_pack_id: domain_id(),
        right_domain_pack_id: domain_id(),
        created_by_run_id: uuid(42),
        version: 1,
    });

    let trajectory = seal(TrajectoryRecord {
        header: header(uuid(43), TRAJECTORY_RECORD_VERSION),
        trajectory_id,
        segmentation_policy_id: "by_domain_session".to_string(),
        ordered_transition_ids: vec![transition_id],
        ordered_panel_ids: vec![panel_id],
        start_time_unix_ms: TS0,
        end_time_unix_ms: TS0,
        entity_refs: vec![],
        binding_refs: vec![binding_id],
        trajectory_hash: hash32(5),
        record_count: 1,
    });

    let dataset = seal(DatasetShardRecord {
        header: header(uuid(44), DATASET_SHARD_RECORD_VERSION),
        dataset_id,
        shard_id,
        source_trajectory_ids: vec![trajectory_id],
        split_name: "train".to_string(),
        row_count: 1,
        input_panel_ids: vec![panel_id],
        target_panel_ids: vec![PanelId(uuid(45))],
        action_ids: vec![action_id],
        negative_panel_ids: vec![PanelId(uuid(46))],
        objective_ids: vec!["predict_next_counter".to_string()],
        shape_summary: ShapeSummary {
            input_dim: 2,
            target_dim: 2,
            action_dim: 1,
            n_train_rows: 1,
            n_val_rows: 0,
            n_test_rows: 0,
        },
        source_hashes: vec![hash32(6), hash32(7)],
        leakage_report: LeakageReport {
            future_in_input_count: 0,
            same_panel_input_target_count: 0,
            negative_equals_target_count: 0,
            negative_feature_equals_target_count: 0,
            split_overlap_count: 0,
        },
        compiler_version: 2,
    });

    let artifact_file = ArtifactFile {
        relative_path: "model.safetensors".to_string(),
        sha256: hash32(8),
        size_bytes: 128,
    };
    let training = seal(TrainingRunRecord {
        header: header(uuid(47), TRAINING_RUN_RECORD_VERSION),
        training_run_id,
        domain_pack_id: domain_id(),
        dataset_id,
        started_at_unix_ms: TS0,
        finished_at_unix_ms: Some(TS0 + 10),
        status: TrainingRunStatus::Completed,
        training_config_hash: hash32(9),
        objective_ids: vec!["predict_next_counter".to_string()],
        metrics: BTreeMap::from([("val_latent_mse".to_string(), 0.001)]),
        artifact_ids: vec![artifact_id],
    });
    let artifact = seal(ModelArtifactRecord {
        header: header(uuid(48), MODEL_ARTIFACT_RECORD_VERSION),
        artifact_id,
        training_run_id,
        domain_pack_id: domain_id(),
        domain_pack_version: "1.0.0".to_string(),
        dataset_id,
        artifact_root: PathBuf::from("/tmp/5090jepa_artifacts/counter_world/sample"),
        files: vec![artifact_file.clone()],
        model_config_hash: hash32(10),
        evaluation_report_hash: hash32(11),
        created_at_unix_ms: TS0 + 11,
        status: ArtifactStatus::Active,
    });

    let prediction = seal(PredictionRecord {
        header: header(uuid(49), PREDICTION_RECORD_VERSION),
        prediction_id,
        model_artifact_id: artifact_id,
        model_artifact_hash_at_inference: artifact_file.sha256,
        input_panel_id: panel_id,
        candidate_action_id: action_id,
        predicted_next_panel_vec: vec![0.1, 0.2],
        uncertainty: 0.05,
        objective_scores: BTreeMap::from([("predict_next_counter".to_string(), 0.99)]),
        created_at_unix_ms: TS0 + 12,
    });

    let skill = seal(SkillPolicyRecord {
        header: header(uuid(50), SKILL_POLICY_RECORD_VERSION),
        skill_id,
        domain_pack_id: domain_id(),
        skill_name: "enumerate_declared_actions".to_string(),
        strategy: SkillStrategy::EnumerateDeclaredActions,
        version: 1,
    });

    let guard = seal(GuardDecisionRecord {
        header: header(uuid(51), GUARD_DECISION_RECORD_VERSION),
        guard_decision_id: guard_id,
        plan_trace_id: plan_id.0,
        guard_id: "bounds_check".to_string(),
        candidate_action_id: action_id,
        decision: GuardDecision::Allow,
        evidence_refs: vec!["state.counter <= 1024".to_string()],
        threshold_values: BTreeMap::from([("max_counter".to_string(), 1024.0)]),
        utility_score: None,
        utility_decision: None,
        gtau_decision: None,
        constellation_uuid: None,
        cosine_to_centroid_per_modality: None,
        tau_per_modality: None,
        gtau_failed_modalities: None,
        created_at_unix_ms: TS0 + 13,
    });

    let plan = seal(PlanTraceRecord {
        header: header(uuid(52), PLAN_TRACE_RECORD_VERSION),
        plan_trace_id: plan_id,
        domain_pack_id: domain_id(),
        current_panel_id: panel_id,
        model_artifact_id: artifact_id,
        model_artifact_hash_at_plan: artifact_file.sha256,
        skill_policy_id: skill_id,
        candidate_action_ids: vec![action_id],
        prediction_ids: vec![prediction_id],
        guard_decision_ids: vec![guard_id.0],
        utility_scores: vec![0.99],
        selected_action_id: Some(action_id),
        no_accepted_candidate: false,
        constellation_uuid_used: None,
        status: PlanTraceStatus::Selected,
        created_at_unix_ms: TS0 + 14,
    });

    let surprise = seal(SurpriseEventRecord {
        header: header(uuid(53), SURPRISE_EVENT_RECORD_VERSION),
        surprise_event_id: surprise_id,
        prediction_id,
        observed_outcome_id: outcome_id,
        observed_panel_id: panel_id,
        surprise_kind: SurpriseKind::UnexpectedOutcome,
        cosine: 0.2,
        threshold: 0.85,
        error_norm: 1.5,
        created_at_unix_ms: TS0 + 15,
    });

    let verification = seal(VerificationRunRecord {
        header: header(uuid(54), VERIFICATION_RUN_RECORD_VERSION),
        verification_run_id: verification_id,
        test_name: "dynamicjepa_phase1_core_fsv".to_string(),
        db_path_hash: hash32(12),
        fixture_hashes: vec![sha256(
            fs::read(
                root_dir().join("configs/dynamicjepa/verification/counter_world_happy_path.jsonl"),
            )
            .expect("phase0 fixture")
            .as_slice(),
        )],
        before_counts: BTreeMap::from([("phase1_sot_files".to_string(), 0)]),
        after_counts: BTreeMap::from([("phase1_sot_files".to_string(), 20)]),
        commands_executed: vec!["cargo test -p context-graph-core dynamicjepa".to_string()],
        artifact_hash_checks: vec![ArtifactHashCheck::from(artifact_file)],
        decoded_record_excerpts: BTreeMap::from([(
            "DomainPack".to_string(),
            DjJsonValue::from_json(json!({"id": "counter_world", "version": "1.0.0"})).unwrap(),
        )]),
        expected_results: BTreeMap::from([(
            "roundtrip_records".to_string(),
            DjJsonValue::from_json(json!(20)).unwrap(),
        )]),
        actual_results: BTreeMap::from([(
            "roundtrip_records".to_string(),
            DjJsonValue::from_json(json!(20)).unwrap(),
        )]),
        status: VerificationStatus::Passed,
        created_at_unix_ms: TS0 + 16,
    });

    (
        domain,
        Samples {
            raw_event,
            adapter_run,
            state,
            action,
            outcome,
            transition,
            reading_counter,
            reading_delta,
            panel,
            binding,
            trajectory,
            dataset,
            training,
            artifact,
            prediction,
            skill,
            guard,
            plan,
            surprise,
            verification,
        },
    )
}

#[test]
fn dynamicjepa_phase1_all_records_roundtrip_from_file_source_of_truth() {
    let temp = TempDir::new().expect("temp source-of-truth dir");
    let before_count = fs::read_dir(temp.path()).expect("read temp dir").count();
    let (domain, samples) = sample_records();
    let grid_domain = phase0_gridworld_domain_pack();

    let domain_back: DomainPack = roundtrip_from_file(temp.path(), "domain_pack", &domain);
    let grid_domain_back: DomainPack =
        roundtrip_from_file(temp.path(), "gridworld_domain_pack", &grid_domain);
    roundtrip_from_file::<RawDomainEvent>(temp.path(), "raw_event", &samples.raw_event);
    roundtrip_from_file::<AdapterRunRecord>(temp.path(), "adapter_run", &samples.adapter_run);
    roundtrip_from_file::<NormalizedState>(temp.path(), "state", &samples.state);
    roundtrip_from_file::<NormalizedAction>(temp.path(), "action", &samples.action);
    roundtrip_from_file::<NormalizedOutcome>(temp.path(), "outcome", &samples.outcome);
    roundtrip_from_file::<StateTransition>(temp.path(), "transition", &samples.transition);
    roundtrip_from_file::<InstrumentReading>(
        temp.path(),
        "reading_counter",
        &samples.reading_counter,
    );
    roundtrip_from_file::<InstrumentReading>(temp.path(), "reading_delta", &samples.reading_delta);
    roundtrip_from_file::<LatentPanel>(temp.path(), "panel", &samples.panel);
    roundtrip_from_file::<BindingRecord>(temp.path(), "binding", &samples.binding);
    roundtrip_from_file::<TrajectoryRecord>(temp.path(), "trajectory", &samples.trajectory);
    roundtrip_from_file::<DatasetShardRecord>(temp.path(), "dataset", &samples.dataset);
    roundtrip_from_file::<TrainingRunRecord>(temp.path(), "training", &samples.training);
    roundtrip_from_file::<ModelArtifactRecord>(temp.path(), "artifact", &samples.artifact);
    roundtrip_from_file::<PredictionRecord>(temp.path(), "prediction", &samples.prediction);
    roundtrip_from_file::<SkillPolicyRecord>(temp.path(), "skill", &samples.skill);
    roundtrip_from_file::<GuardDecisionRecord>(temp.path(), "guard", &samples.guard);
    roundtrip_from_file::<PlanTraceRecord>(temp.path(), "plan", &samples.plan);
    roundtrip_from_file::<SurpriseEventRecord>(temp.path(), "surprise", &samples.surprise);
    roundtrip_from_file::<VerificationRunRecord>(
        temp.path(),
        "verification",
        &samples.verification,
    );

    let after_count = fs::read_dir(temp.path()).expect("read temp dir").count();
    assert_eq!(before_count, 0);
    assert_eq!(after_count, 22);
    assert_eq!(domain_back.id.as_str(), "counter_world");
    assert_eq!(domain_back.instrument_specs.len(), 2);
    assert_eq!(grid_domain_back.id.as_str(), "gridworld_5x5");
    assert_eq!(grid_domain_back.instrument_specs.len(), 3);
}

#[test]
fn dynamicjepa_phase1_edge_cases_fail_from_source_of_truth_bytes() {
    let temp = TempDir::new().expect("temp edge source-of-truth dir");
    let (domain, samples) = sample_records();
    let before_count = fs::read_dir(temp.path()).expect("read temp dir").count();

    let empty_path = temp.path().join("edge_empty_payload.bin");
    fs::write(&empty_path, []).expect("write empty source-of-truth file");
    let empty_bytes = fs::read(&empty_path).expect("read empty source-of-truth file");
    let empty_err = decode_versioned_record::<DomainPack>(&empty_bytes)
        .expect_err("empty payload must fail decode");
    assert_eq!(empty_err.code(), "CODEC");

    let wrong_version_path = temp.path().join("edge_wrong_version.bin");
    let mut valid_bytes = encode_versioned_record(&domain).expect("domain encodes");
    valid_bytes[0] = 9;
    fs::write(&wrong_version_path, &valid_bytes).expect("write wrong version bytes");
    let wrong_version_err = decode_versioned_record::<DomainPack>(
        &fs::read(&wrong_version_path).expect("read wrong version bytes"),
    )
    .expect_err("wrong version must fail decode");
    assert!(wrong_version_err.to_string().contains("actual_version=9"));

    let invalid_id_path = temp.path().join("edge_invalid_id.bin");
    let mut invalid_domain = domain.clone();
    invalid_domain.id = DomainPackId("CounterWorld".to_string());
    invalid_domain.header.domain_pack_id = DomainPackId("CounterWorld".to_string());
    invalid_domain
        .refresh_content_hash()
        .expect("invalid id record still hashes for corrupt-source simulation");
    write_unchecked(
        &invalid_domain,
        DOMAIN_PACK_RECORD_VERSION,
        &invalid_id_path,
    );
    let invalid_id_err = decode_versioned_record::<DomainPack>(
        &fs::read(&invalid_id_path).expect("read invalid id bytes"),
    )
    .expect_err("invalid id must fail validation after decode");
    assert!(invalid_id_err.to_string().contains("lowercase ASCII"));

    let shape_path = temp.path().join("edge_panel_shape_mismatch.bin");
    let mut bad_panel = samples.panel.clone();
    bad_panel.slot_vectors[0] = vec![0.0, 1.0];
    bad_panel
        .refresh_content_hash()
        .expect("bad shape record still hashes for corrupt-source simulation");
    write_unchecked(&bad_panel, LATENT_PANEL_RECORD_VERSION, &shape_path);
    let shape_err = decode_versioned_record::<LatentPanel>(
        &fs::read(&shape_path).expect("read bad panel bytes"),
    )
    .expect_err("shape mismatch must fail validation after decode");
    assert_eq!(shape_err.code(), "PANEL_SHAPE_MISMATCH");

    let after_count = fs::read_dir(temp.path()).expect("read temp dir").count();
    assert_eq!(before_count, 0);
    assert_eq!(after_count, 4);
}

#[test]
fn dynamicjepa_phase1_full_state_verification_evidence_log() {
    let evidence_root = root_dir().join("tmp/5090jepa_evidence/phase1/core_record_fsv");
    if evidence_root.exists() {
        fs::remove_dir_all(&evidence_root).expect("clear stale phase1 evidence dir");
    }
    fs::create_dir_all(&evidence_root).expect("create phase1 evidence dir");
    let before_files = count_files_recursive(&evidence_root);

    let (domain, samples) = sample_records();
    let grid_domain = phase0_gridworld_domain_pack();
    roundtrip_from_file::<DomainPack>(&evidence_root, "domain_pack", &domain);
    roundtrip_from_file::<DomainPack>(&evidence_root, "gridworld_domain_pack", &grid_domain);
    roundtrip_from_file::<RawDomainEvent>(&evidence_root, "raw_event", &samples.raw_event);
    roundtrip_from_file::<LatentPanel>(&evidence_root, "latent_panel", &samples.panel);
    roundtrip_from_file::<VerificationRunRecord>(
        &evidence_root,
        "verification_run",
        &samples.verification,
    );

    let edge_dir = evidence_root.join("edge_cases");
    fs::create_dir_all(&edge_dir).expect("create edge case evidence dir");

    let empty_path = edge_dir.join("empty_payload.bin");
    let empty_before = file_state(&empty_path);
    fs::write(&empty_path, []).expect("write empty edge source-of-truth file");
    let empty_after = file_state(&empty_path);
    let empty_err =
        decode_versioned_record::<DomainPack>(&fs::read(&empty_path).expect("read empty edge"))
            .expect_err("empty edge must fail");

    let wrong_version_path = edge_dir.join("wrong_version.bin");
    let wrong_before = file_state(&wrong_version_path);
    let mut wrong_bytes = encode_versioned_record(&domain).expect("domain encodes");
    wrong_bytes[0] = 9;
    fs::write(&wrong_version_path, wrong_bytes).expect("write wrong version edge");
    let wrong_after = file_state(&wrong_version_path);
    let wrong_err = decode_versioned_record::<DomainPack>(
        &fs::read(&wrong_version_path).expect("read wrong version edge"),
    )
    .expect_err("wrong version edge must fail");

    let invalid_id_path = edge_dir.join("invalid_id_format.bin");
    let invalid_before = file_state(&invalid_id_path);
    let mut invalid_domain = domain.clone();
    invalid_domain.id = DomainPackId("CounterWorld".to_string());
    invalid_domain.header.domain_pack_id = DomainPackId("CounterWorld".to_string());
    invalid_domain
        .refresh_content_hash()
        .expect("invalid id corrupt record hashes");
    write_unchecked(
        &invalid_domain,
        DOMAIN_PACK_RECORD_VERSION,
        &invalid_id_path,
    );
    let invalid_after = file_state(&invalid_id_path);
    let invalid_err = decode_versioned_record::<DomainPack>(
        &fs::read(&invalid_id_path).expect("read invalid id edge"),
    )
    .expect_err("invalid id edge must fail");

    let shape_path = edge_dir.join("panel_shape_mismatch.bin");
    let shape_before = file_state(&shape_path);
    let mut bad_panel = samples.panel.clone();
    bad_panel.slot_vectors[0] = vec![0.0, 1.0];
    bad_panel
        .refresh_content_hash()
        .expect("bad panel corrupt record hashes");
    write_unchecked(&bad_panel, LATENT_PANEL_RECORD_VERSION, &shape_path);
    let shape_after = file_state(&shape_path);
    let shape_err =
        decode_versioned_record::<LatentPanel>(&fs::read(&shape_path).expect("read shape edge"))
            .expect_err("shape edge must fail");

    let after_files = count_files_recursive(&evidence_root);
    let evidence = json!({
        "operation": "dynamicjepa_phase1_core_record_fsv",
        "status": "ok",
        "trigger_event": "cargo test -p context-graph-core dynamicjepa_phase1_full_state_verification_evidence_log -- --nocapture",
        "source_of_truth": {
            "type": "versioned bincode files",
            "path": evidence_root,
            "records": [
                "domain_pack.bin",
                "gridworld_domain_pack.bin",
                "raw_event.bin",
                "latent_panel.bin",
                "verification_run.bin",
                "edge_cases/empty_payload.bin",
                "edge_cases/wrong_version.bin",
                "edge_cases/invalid_id_format.bin",
                "edge_cases/panel_shape_mismatch.bin"
            ]
        },
        "before_state": {"file_count": before_files},
        "after_state": {"file_count": after_files},
        "decoded_records": {
            "domain_pack": {
                "id": domain.id.as_str(),
                "version": domain.version,
                "instrument_count": domain.instrument_specs.len(),
                "adapter_count": domain.adapter_specs.len()
            },
            "gridworld_domain_pack": {
                "id": grid_domain.id.as_str(),
                "version": grid_domain.version,
                "instrument_count": grid_domain.instrument_specs.len(),
                "adapter_count": grid_domain.adapter_specs.len(),
                "expected_event_count": grid_domain.verification_policy.expected_event_count
            },
            "raw_event": {
                "event_id": samples.raw_event.event_id.to_string(),
                "source_offset": samples.raw_event.source_offset,
                "payload_hash": hex32(&samples.raw_event.payload_hash)
            },
            "latent_panel": {
                "panel_id": samples.panel.panel_id.to_string(),
                "slot_count": samples.panel.ordered_slots.len(),
                "slot_vectors": samples.panel.slot_vectors
            },
            "verification_run": {
                "test_name": samples.verification.test_name,
                "status": "Passed"
            }
        },
        "edge_case_audit": {
            "empty_payload": {
                "before_state": empty_before,
                "after_state": empty_after,
                "error_code": empty_err.code(),
                "error_message": empty_err.to_string()
            },
            "wrong_version": {
                "before_state": wrong_before,
                "after_state": wrong_after,
                "error_code": wrong_err.code(),
                "error_message": wrong_err.to_string()
            },
            "invalid_id_format": {
                "before_state": invalid_before,
                "after_state": invalid_after,
                "error_code": invalid_err.code(),
                "error_message": invalid_err.to_string()
            },
            "panel_shape_mismatch": {
                "before_state": shape_before,
                "after_state": shape_after,
                "error_code": shape_err.code(),
                "error_message": shape_err.to_string()
            }
        }
    });
    let evidence_path = root_dir().join("tmp/5090jepa_evidence/phase1/core_record_fsv.json");
    fs::write(
        &evidence_path,
        serde_json::to_string_pretty(&evidence).expect("serialize phase1 evidence"),
    )
    .expect("write phase1 evidence json");
    println!("{}", serde_json::to_string_pretty(&evidence).unwrap());

    assert_eq!(before_files, 0);
    assert_eq!(after_files, 9);
}
