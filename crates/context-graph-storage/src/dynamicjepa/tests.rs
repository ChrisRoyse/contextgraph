use super::column_families::*;
use super::*;
use context_graph_core::dynamicjepa::*;
use rocksdb::DB;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
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
        "fix the Phase 0 TOML source of truth; Phase 2 storage does not substitute defaults",
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

fn phase0_counter_domain_pack() -> DomainPack {
    let path = root_dir().join("configs/dynamicjepa/domain_packs/counter_world.v1.toml");
    let bytes = fs::read(&path).expect("read counter_world Phase 0 TOML source of truth");
    let source_hash = sha256(&bytes);
    let source_text = std::str::from_utf8(&bytes).expect("Phase 0 TOML must be UTF-8");
    let parsed: DomainPackToml =
        toml::from_str(source_text).expect("Phase 0 TOML must parse into strict test schema");
    let domain_id = DomainPackId::new(parsed.domain.id).expect("Phase 0 domain id validates");
    let candidate_actions = match parsed.planner_policy.candidate_actions.kind.as_str() {
        "enumerated" => {
            if let Some(deltas) = parsed.planner_policy.candidate_actions.deltas {
                CandidateActionConfig::EnumeratedDeltas { deltas }
            } else if let Some(moves) = parsed.planner_policy.candidate_actions.moves {
                CandidateActionConfig::EnumeratedMoves { moves }
            } else {
                panic!("Phase 0 candidate_actions enumerated requires deltas or moves");
            }
        }
        other => panic!("unsupported Phase 0 candidate action kind {other:?}"),
    };

    seal(DomainPack {
        header: header_for_domain(uuid(1), DOMAIN_PACK_RECORD_VERSION, domain_id.clone()),
        id: domain_id,
        version: parsed.domain.version,
        title: parsed.domain.title,
        schema_version: parsed.domain.schema_version,
        state_schema: StateSchema {
            fields: fields_from_toml(parsed.state.fields, "state").unwrap(),
        },
        action_schema: ActionSchema {
            fields: fields_from_toml(parsed.action.fields, "action").unwrap(),
        },
        outcome_schema: OutcomeSchema {
            fields: fields_from_toml(parsed.outcome.fields, "outcome").unwrap(),
        },
        entity_schema: EntitySchema {
            fields: fields_from_toml(parsed.entity.fields, "entity").unwrap(),
        },
        time_schema: TimeSchema {
            field: field_spec_from_toml(parsed.time.field, "time.field").unwrap(),
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
            .collect::<DynamicJepaResult<Vec<_>>>()
            .unwrap(),
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
            .collect::<DynamicJepaResult<Vec<_>>>()
            .unwrap(),
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
    })
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
}

fn sample_records() -> (DomainPack, Samples) {
    let domain = phase0_counter_domain_pack();
    let fixture_path =
        root_dir().join("configs/dynamicjepa/verification/counter_world_happy_path.jsonl");
    let fixture = fs::read_to_string(&fixture_path).expect("Phase 0 fixture must exist");
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

    let state = seal(NormalizedState {
        header: header(uuid(31), NORMALIZED_STATE_RECORD_VERSION),
        state_id,
        fields: BTreeMap::from([("counter".to_string(), FieldValue::I64(0))]),
        source_event_id: event_id,
    });

    let action = seal(NormalizedAction {
        header: header(uuid(32), NORMALIZED_ACTION_RECORD_VERSION),
        action_id,
        fields: BTreeMap::from([("delta".to_string(), FieldValue::I64(1))]),
        source_event_id: event_id,
        action_origin: ActionOrigin::Observed,
    });

    let outcome = seal(NormalizedOutcome {
        header: header(uuid(33), NORMALIZED_OUTCOME_RECORD_VERSION),
        outcome_id,
        fields: BTreeMap::from([("next_counter".to_string(), FieldValue::I64(1))]),
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
        cf: CF_DJ_RAW_EVENTS.to_string(),
        key_bytes: event_id.into_bytes().to_vec(),
    };
    let binding = seal(BindingRecord {
        header: header(uuid(41), BINDING_RECORD_VERSION),
        binding_id,
        binding_kind: BindingKind::EventToTrajectory,
        left_ref: binding_ref.clone(),
        right_ref: BindingRef {
            cf: CF_DJ_TRAJECTORIES.to_string(),
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
        },
    )
}

fn expected_happy_counts() -> BTreeMap<String, u64> {
    BTreeMap::from([
        (CF_DJ_DOMAIN_PACKS.to_string(), 1),
        (CF_DJ_DOMAIN_PACK_BY_NAME_VERSION.to_string(), 1),
        (CF_DJ_INSTRUMENT_REGISTRY.to_string(), 2),
        (CF_DJ_ADAPTER_REGISTRY.to_string(), 1),
        (CF_DJ_RAW_EVENTS.to_string(), 1),
        (CF_DJ_NORMALIZED_STATES.to_string(), 1),
        (CF_DJ_ACTIONS.to_string(), 1),
        (CF_DJ_OUTCOMES.to_string(), 1),
        (CF_DJ_TRANSITIONS.to_string(), 1),
        (CF_DJ_ADAPTER_RUNS.to_string(), 1),
        (CF_DJ_INSTRUMENT_READINGS.to_string(), 2),
        (CF_DJ_LATENT_PANELS.to_string(), 1),
        (CF_DJ_PAIRWISE_READINGS.to_string(), 0),
        (CF_DJ_CONSTELLATIONS.to_string(), 0),
        (CF_DJ_THRESHOLD_CALIBRATIONS.to_string(), 0),
        (CF_DJ_BINDINGS.to_string(), 1),
        (CF_DJ_BINDINGS_BY_ENTITY.to_string(), 1),
        (CF_DJ_TRAJECTORIES.to_string(), 1),
        (CF_DJ_DATASET_SHARDS.to_string(), 1),
        (CF_DJ_TRAINING_RUNS.to_string(), 1),
        (CF_DJ_MODEL_ARTIFACTS.to_string(), 1),
        (CF_DJ_PREDICTIONS.to_string(), 1),
        (CF_DJ_SKILL_POLICIES.to_string(), 1),
        (CF_DJ_PLAN_TRACES.to_string(), 1),
        (CF_DJ_GUARD_DECISIONS.to_string(), 1),
        (CF_DJ_SURPRISE_EVENTS.to_string(), 1),
        (CF_DJ_VERIFICATION_RUNS.to_string(), 1),
        (CF_DJ_AUDIT_LOG.to_string(), 2),
        (CF_DJ_AUDIT_WITNESS_CHAIN.to_string(), 2),
    ])
}

fn flush_dj_cfs(db: &DB) {
    for cf_name in DJ_CF_NAMES {
        db.flush_cf(
            db.cf_handle(cf_name)
                .expect("DynamicJEPA CF must exist before flush"),
        )
        .expect("flush DynamicJEPA CF to physical RocksDB files");
    }
}

#[test]
fn dynamicjepa_audit_witness_chain_tamper_fsv() {
    let evidence_root = root_dir().join("tmp/5090jepa_evidence/phase2/audit_witness_tamper_fsv");
    if evidence_root.exists() {
        fs::remove_dir_all(&evidence_root).expect("clear stale audit witness tamper evidence");
    }
    fs::create_dir_all(&evidence_root).expect("create audit witness tamper evidence root");

    let db_path = evidence_root.join("rocksdb");
    let store =
        crate::teleological::RocksDbTeleologicalStore::open(&db_path).expect("open witness DB");
    let db = store.dynamicjepa_db();

    let empty_before = snapshot_dj_counts(db).expect("snapshot empty DB");
    let empty_verification =
        verify_audit_witness_chain(db).expect("empty audit witness chain verifies");
    assert_eq!(empty_verification.entries, 0);
    assert_eq!(empty_verification.audit_rows, 0);
    assert_eq!(empty_verification.witness_rows, 0);
    assert_eq!(
        empty_before,
        snapshot_dj_counts(db).expect("empty verification must not mutate DB")
    );

    let (domain, _) = sample_records();
    let domain_uuid = put_domain_pack(db, &domain).expect("put audited domain pack");
    let after_write = snapshot_dj_counts(db).expect("snapshot after audited write");
    let verified =
        verify_audit_witness_chain(db).expect("audit witness chain verifies after write");
    assert_eq!(verified.entries, 1);
    assert_eq!(verified.audit_rows, 1);
    assert_eq!(verified.witness_rows, 1);

    let witness_cf = db
        .cf_handle(CF_DJ_AUDIT_WITNESS_CHAIN)
        .expect("audit witness CF exists");
    let sequence_zero = 0u64.to_be_bytes();
    let mut witness_value = db
        .get_cf(witness_cf, sequence_zero)
        .expect("read persisted witness row")
        .expect("sequence zero witness row exists");
    assert_eq!(witness_value.len(), 97);
    witness_value[24 + 32] ^= 0x01;
    db.put_cf(witness_cf, sequence_zero, witness_value)
        .expect("tamper persisted witness row");
    flush_dj_cfs(db);

    let after_tamper = snapshot_dj_counts(db).expect("snapshot after tamper");
    assert_eq!(after_write, after_tamper);
    let err = verify_audit_witness_chain(db).expect_err("tampered witness row must fail closed");
    assert_eq!(err.code(), "STORAGE_INVARIANT_VIOLATION");

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "operation": "dynamicjepa_audit_witness_chain_tamper_fsv",
            "status": "ok",
            "source_of_truth": {
                "path": db_path,
                "column_families": [CF_DJ_AUDIT_LOG, CF_DJ_AUDIT_WITNESS_CHAIN],
                "domain_pack_storage_uuid": domain_uuid.to_string()
            },
            "empty_before_counts": empty_before,
            "empty_verification": empty_verification,
            "after_write_counts": after_write,
            "verified_after_write": verified,
            "after_tamper_counts": after_tamper,
            "tamper_error_code": err.code(),
            "tamper_error_message": err.to_string()
        }))
        .expect("serialize tamper FSV evidence")
    );
}

#[test]
fn dynamicjepa_phase1_meaning_compression_storage_fsv() {
    let evidence_root = root_dir().join("tmp/upgradeplancombine/phase1/storage_records_fsv");
    if evidence_root.exists() {
        fs::remove_dir_all(&evidence_root).expect("clear stale Phase 1 storage evidence");
    }
    fs::create_dir_all(&evidence_root).expect("create Phase 1 storage evidence root");

    let db_path = evidence_root.join("rocksdb");
    let store =
        crate::teleological::RocksDbTeleologicalStore::open(&db_path).expect("open Phase 1 DB");
    let db = store.dynamicjepa_db();
    let before_counts = snapshot_dj_counts(db).expect("snapshot before counts");

    let event_id = EventId(uuid(8001));
    let domain_uuid = uuid(8002);
    let pairwise = seal(PairwiseReading {
        header: header(uuid(8003), PAIRWISE_READING_RECORD_VERSION),
        pairwise_id: PairwiseReadingId(uuid(8004)),
        event_id,
        instrument_j: InstrumentId::new("alpha_sensor").unwrap(),
        instrument_k: InstrumentId::new("beta_sensor").unwrap(),
        instrument_j_artifact_hash: hash32(11),
        instrument_k_artifact_hash: hash32(12),
        kinds_emitted: PairKindBitset(PairKindBitset::COSINE_AGREEMENT),
        cosine_agreement: 0.5,
        rank_disagreement: None,
        modality_contradiction: None,
        sparse_dense_mismatch: None,
        temporal_surprise: None,
        causal_direction_disagreement: None,
        safety_proximity: None,
        created_at_unix_ms: TS0 as u64,
        validation_status: ReadingStatus::Ok,
    });
    put_pairwise_reading(db, event_id.as_uuid(), 1, 2, &pairwise).expect("put pairwise reading");

    let constellation = seal(ConstellationCentroid {
        header: header(uuid(8005), CONSTELLATION_CENTROID_RECORD_VERSION),
        constellation_id: ConstellationId(uuid(8006)),
        domain_pack_id: domain_id(),
        subject_id: "subject.alpha".to_string(),
        modality_id: InstrumentId::new("alpha_sensor").unwrap(),
        centroid: vec![1.0, 0.0],
        instrument_artifact_hash: hash32(13),
        reference_set_count: 5,
        kept_count: 5,
        dropped_zero_norm: 0,
        loo_stability: 0.9,
        calibration_percentile: 10,
        calibration_set_size: 5,
        built_at_unix_ms: TS0 as u64,
        built_by_run_id: Some(uuid(8007)),
    });
    put_constellation(db, domain_uuid, "subject.alpha", 1, &constellation)
        .expect("put constellation");

    let threshold = seal(ThresholdCalibration {
        header: header(uuid(8008), THRESHOLD_CALIBRATION_RECORD_VERSION),
        calibration_id: ThresholdCalibrationId(uuid(8009)),
        domain_pack_id: domain_id(),
        subject_id: "subject.alpha".to_string(),
        modality_id: InstrumentId::new("alpha_sensor").unwrap(),
        tau: 0.7,
        percentile: 10,
        calibration_set_count: 5,
        calibration_event_uuids_sample: vec![event_id],
        calibration_set_disjoint_proof: true,
        calibration_min: 0.1,
        calibration_max: 0.9,
        calibration_p10: 0.2,
        supersede_seq: 0,
        supersedes_uuid: None,
        reason: None,
        calibrated_at_unix_ms: TS0 as u64,
    });
    put_threshold_calibration(db, domain_uuid, "subject.alpha", 1, 0, &threshold)
        .expect("put threshold calibration");

    flush_dj_cfs(db);
    let after_counts = snapshot_dj_counts(db).expect("snapshot after counts");
    assert_eq!(after_counts[CF_DJ_PAIRWISE_READINGS], 1);
    assert_eq!(after_counts[CF_DJ_CONSTELLATIONS], 1);
    assert_eq!(after_counts[CF_DJ_THRESHOLD_CALIBRATIONS], 1);

    let pairwise_rows = inspect_cf(db, CF_DJ_PAIRWISE_READINGS, 10, 0).expect("inspect pairwise");
    let constellation_rows =
        inspect_cf(db, CF_DJ_CONSTELLATIONS, 10, 0).expect("inspect constellation");
    let threshold_rows =
        inspect_cf(db, CF_DJ_THRESHOLD_CALIBRATIONS, 10, 0).expect("inspect threshold");
    assert_eq!(pairwise_rows.len(), 1);
    assert_eq!(constellation_rows.len(), 1);
    assert_eq!(threshold_rows.len(), 1);
    assert_eq!(
        pairwise_rows[0]["decoded"]["pairwise_id"],
        pairwise.pairwise_id.to_string()
    );
    assert_eq!(
        constellation_rows[0]["decoded"]["constellation_id"],
        constellation.constellation_id.to_string()
    );
    assert_eq!(
        threshold_rows[0]["decoded"]["calibration_id"],
        threshold.calibration_id.to_string()
    );

    let evidence = json!({
        "operation": "dynamicjepa_phase1_meaning_compression_storage_fsv",
        "status": "ok",
        "source_of_truth": {
            "type": "physical RocksDB column families",
            "path": db_path.display().to_string(),
            "column_families": [
                CF_DJ_PAIRWISE_READINGS,
                CF_DJ_CONSTELLATIONS,
                CF_DJ_THRESHOLD_CALIBRATIONS,
            ],
        },
        "before_counts": before_counts,
        "after_counts": after_counts,
        "decoded_records": {
            "dj_pairwise_readings": pairwise_rows,
            "dj_constellations": constellation_rows,
            "dj_threshold_calibrations": threshold_rows,
        }
    });
    println!("{}", serde_json::to_string_pretty(&evidence).unwrap());
}

fn db_state(db: &DB) -> serde_json::Value {
    json!({
        "counts": snapshot_dj_counts(db).expect("snapshot DynamicJEPA counts"),
        "domain_pack_rows": count_domain_packs(db).expect("count domain packs"),
    })
}

fn inspect_all_cfs(db: &DB) -> serde_json::Value {
    let mut rows = BTreeMap::new();
    for cf_name in DJ_CF_NAMES {
        rows.insert(
            (*cf_name).to_string(),
            inspect_cf(db, cf_name, 3, 0).expect("inspect DynamicJEPA CF"),
        );
    }
    json!(rows)
}

fn write_corrupt_domain_row(db: &DB, key: Uuid, value: &[u8]) {
    db.put_cf(
        db.cf_handle(CF_DJ_DOMAIN_PACKS)
            .expect("domain pack CF exists"),
        key.as_bytes(),
        value,
    )
    .expect("write corrupt source-of-truth bytes");
    db.flush_cf(
        db.cf_handle(CF_DJ_DOMAIN_PACKS)
            .expect("domain pack CF exists"),
    )
    .expect("flush corrupt source-of-truth bytes");
}

fn edge_case(db: &DB, name: &str, key: Uuid, value: Vec<u8>) -> serde_json::Value {
    let before = db_state(db);
    write_corrupt_domain_row(db, key, &value);
    let err = get_domain_pack_by_storage_id(db, key).expect_err("corrupt row must fail decode");
    let after = db_state(db);
    json!({
        "name": name,
        "key": key.to_string(),
        "before_state": before,
        "after_state": after,
        "error_code": err.code(),
        "error_message": err.to_string(),
    })
}

fn verification_record(
    before_counts: BTreeMap<String, u64>,
    after_counts: BTreeMap<String, u64>,
    db_path: &Path,
    samples: &Samples,
) -> VerificationRunRecord {
    let fixture_path =
        root_dir().join("configs/dynamicjepa/verification/counter_world_happy_path.jsonl");
    let fixture_bytes = fs::read(&fixture_path).expect("read Phase 0 fixture for verification");
    let artifact_file = samples.artifact.files[0].clone();
    seal(VerificationRunRecord {
        header: header(uuid(54), VERIFICATION_RUN_RECORD_VERSION),
        verification_run_id: VerificationRunId(uuid(29)),
        test_name: "dynamicjepa_phase2_storage_fsv".to_string(),
        db_path_hash: sha256(db_path.display().to_string().as_bytes()),
        fixture_hashes: vec![sha256(&fixture_bytes)],
        before_counts,
        after_counts,
        commands_executed: vec![
            "CUDA_COMPUTE_CAP=120 cargo test -p context-graph-storage dynamicjepa --all-features -- --nocapture".to_string(),
        ],
        artifact_hash_checks: vec![ArtifactHashCheck::from(artifact_file)],
        decoded_record_excerpts: BTreeMap::from([(
            "latent_panel".to_string(),
            DjJsonValue::from_json(json!({
                "panel_id": samples.panel.panel_id.to_string(),
                "slot_vectors": samples.panel.slot_vectors,
            }))
            .unwrap(),
        )]),
        expected_results: BTreeMap::from([(
            "all_dynamicjepa_cfs_populated".to_string(),
            DjJsonValue::from_json(json!(true)).unwrap(),
        )]),
        actual_results: BTreeMap::from([(
            "all_dynamicjepa_cfs_populated".to_string(),
            DjJsonValue::from_json(json!(true)).unwrap(),
        )]),
        status: VerificationStatus::Passed,
        created_at_unix_ms: TS0 + 16,
    })
}

#[test]
fn dynamicjepa_phase2_storage_full_state_verification() {
    assert_eq!(DJ_CF_COUNT, 29);
    assert_eq!(DJ_CF_NAMES.iter().collect::<BTreeSet<_>>().len(), 29);

    let evidence_root = root_dir().join("tmp/5090jepa_evidence/phase2/storage_fsv");
    if evidence_root.exists() {
        fs::remove_dir_all(&evidence_root).expect("clear stale Phase 2 storage evidence");
    }
    fs::create_dir_all(&evidence_root).expect("create Phase 2 storage evidence root");

    let db_path = evidence_root.join("rocksdb_happy");
    let store =
        crate::teleological::RocksDbTeleologicalStore::open(&db_path).expect("open happy FSV DB");
    let db = store.dynamicjepa_db();
    for cf_name in DJ_CF_NAMES {
        assert!(db.cf_handle(cf_name).is_some(), "{cf_name} must be open");
    }

    let before_counts = snapshot_dj_counts(db).expect("snapshot fresh DB counts");
    assert!(
        before_counts.values().all(|count| *count == 0),
        "fresh DB should have zero DynamicJEPA rows: {before_counts:?}"
    );

    let (domain, samples) = sample_records();
    let domain_uuid = put_domain_pack(db, &domain).expect("put domain pack");
    put_raw_event(db, &samples.raw_event).expect("put raw event");
    put_adapter_run(db, &samples.adapter_run).expect("put adapter run");
    put_normalized_state(db, &samples.state).expect("put state");
    put_action(db, &samples.action).expect("put action");
    put_outcome(db, &samples.outcome).expect("put outcome");
    put_transition(db, &samples.transition).expect("put transition");
    put_instrument_reading(db, &samples.reading_counter).expect("put counter reading");
    put_instrument_reading(db, &samples.reading_delta).expect("put delta reading");
    put_latent_panel(db, &samples.panel).expect("put panel");
    put_binding(db, &samples.binding).expect("put binding");
    put_trajectory(db, &samples.trajectory).expect("put trajectory");
    put_dataset_shard(db, &samples.dataset).expect("put dataset shard");
    put_training_run(db, &samples.training).expect("put training run");
    put_model_artifact(db, &samples.artifact).expect("put artifact");
    put_prediction(db, &samples.prediction).expect("put prediction");
    put_skill_policy(db, &samples.skill).expect("put skill policy");
    put_guard_decision(db, &samples.guard).expect("put guard decision");
    put_plan_trace(db, &samples.plan).expect("put plan trace");
    put_surprise_event(db, &samples.surprise).expect("put surprise event");
    let manual_audit = DjAuditRecord {
        audit_id: uuid(90),
        timestamp_unix_nanos: (TS0 as u64) * 1_000_000 + 1,
        operation: "verification_run".to_string(),
        actor: "phase2_storage_fsv".to_string(),
        input_ids: vec![samples.surprise.surprise_event_id.to_string()],
        output_ids: vec![samples.surprise.surprise_event_id.to_string()],
        cfs_touched: vec![
            CF_DJ_AUDIT_LOG.to_string(),
            CF_DJ_AUDIT_WITNESS_CHAIN.to_string(),
        ],
        content_hashes: vec![samples.surprise.header.content_hash],
        status: AuditStatus::Ok,
        verification_run_id: None,
        signal_yield: 0,
    };
    put_audit_record(db, &manual_audit).expect("put manual audit record");

    let mut expected_after_counts = expected_happy_counts();
    expected_after_counts.insert(CF_DJ_VERIFICATION_RUNS.to_string(), 0);
    let counts_before_verification =
        snapshot_dj_counts(db).expect("snapshot before verification record");
    assert_eq!(counts_before_verification, expected_after_counts);

    let expected_final_counts = expected_happy_counts();
    let verification = verification_record(
        before_counts.clone(),
        expected_final_counts.clone(),
        &db_path,
        &samples,
    );
    put_verification_run(db, &verification).expect("put verification run");
    flush_dj_cfs(db);

    let after_counts = snapshot_dj_counts(db).expect("snapshot after all writes");
    assert_eq!(after_counts, expected_final_counts);

    assert_eq!(
        get_domain_pack_by_storage_id(db, domain_uuid)
            .unwrap()
            .unwrap(),
        domain
    );
    assert_eq!(
        get_domain_pack(db, &domain.id, &domain.version)
            .unwrap()
            .unwrap(),
        domain
    );
    assert_eq!(
        get_instrument_spec(db, domain_uuid, &domain.instrument_specs[0].instrument_id)
            .unwrap()
            .unwrap(),
        domain.instrument_specs[0]
    );
    assert_eq!(
        get_adapter_spec(db, domain_uuid, &domain.adapter_specs[0].adapter_id)
            .unwrap()
            .unwrap(),
        domain.adapter_specs[0]
    );
    assert_eq!(
        get_raw_event(db, samples.raw_event.event_id)
            .unwrap()
            .unwrap(),
        samples.raw_event
    );
    assert_eq!(
        get_normalized_state(db, samples.state.state_id)
            .unwrap()
            .unwrap(),
        samples.state
    );
    assert_eq!(
        get_action(db, samples.action.action_id).unwrap().unwrap(),
        samples.action
    );
    assert_eq!(
        get_outcome(db, samples.outcome.outcome_id)
            .unwrap()
            .unwrap(),
        samples.outcome
    );
    assert_eq!(
        get_transition(db, samples.transition.transition_id)
            .unwrap()
            .unwrap(),
        samples.transition
    );
    assert_eq!(
        get_adapter_run(db, samples.adapter_run.adapter_run_id)
            .unwrap()
            .unwrap(),
        samples.adapter_run
    );
    assert_eq!(
        get_instrument_reading(
            db,
            samples.reading_counter.event_id,
            &samples.reading_counter.instrument_id,
        )
        .unwrap()
        .unwrap(),
        samples.reading_counter
    );
    assert_eq!(
        list_instrument_readings_for_event(db, samples.raw_event.event_id)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        get_latent_panel(db, samples.panel.panel_id)
            .unwrap()
            .unwrap(),
        samples.panel
    );
    assert_eq!(
        get_binding(db, samples.binding.binding_id)
            .unwrap()
            .unwrap(),
        samples.binding
    );
    assert!(
        binding_entity_index_exists(db, &samples.binding.left_ref, samples.binding.binding_id)
            .unwrap()
    );
    assert_eq!(
        get_trajectory(db, samples.trajectory.trajectory_id)
            .unwrap()
            .unwrap(),
        samples.trajectory
    );
    assert_eq!(
        get_dataset_shard(db, samples.dataset.dataset_id, samples.dataset.shard_id)
            .unwrap()
            .unwrap(),
        samples.dataset
    );
    assert_eq!(
        get_training_run(db, samples.training.training_run_id)
            .unwrap()
            .unwrap(),
        samples.training
    );
    assert_eq!(
        get_model_artifact(db, samples.artifact.artifact_id)
            .unwrap()
            .unwrap(),
        samples.artifact
    );
    assert_eq!(
        get_prediction(db, samples.prediction.prediction_id)
            .unwrap()
            .unwrap(),
        samples.prediction
    );
    assert_eq!(
        get_skill_policy(db, samples.skill.skill_id)
            .unwrap()
            .unwrap(),
        samples.skill
    );
    assert_eq!(
        get_plan_trace(db, samples.plan.plan_trace_id)
            .unwrap()
            .unwrap(),
        samples.plan
    );
    assert_eq!(
        get_guard_decision(db, samples.guard.guard_decision_id)
            .unwrap()
            .unwrap(),
        samples.guard
    );
    assert_eq!(
        get_surprise_event(db, samples.surprise.surprise_event_id)
            .unwrap()
            .unwrap(),
        samples.surprise
    );
    assert_eq!(
        get_verification_run(db, verification.verification_run_id)
            .unwrap()
            .unwrap(),
        verification
    );

    assert_eq!(list_domain_packs(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_raw_events(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_normalized_states(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_actions(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_outcomes(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_transitions(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_adapter_runs(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_instrument_readings(db, 10, 0).unwrap().len(), 2);
    assert_eq!(list_latent_panels(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_bindings(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_binding_entity_index_keys(db).unwrap().len(), 1);
    assert_eq!(list_trajectories(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_dataset_shards(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_training_runs(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_model_artifacts(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_predictions(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_skill_policies(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_plan_traces(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_guard_decisions(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_surprise_events(db, 10, 0).unwrap().len(), 1);
    assert_eq!(list_verification_runs(db, 10, 0).unwrap().len(), 1);
    assert_eq!(
        get_audit_record(db, manual_audit.timestamp_unix_nanos, manual_audit.audit_id)
            .unwrap()
            .unwrap(),
        manual_audit
    );
    assert_eq!(list_audit_records(db, 10, 0).unwrap().len(), 2);

    let decoded_rows = inspect_all_cfs(db);

    let edge_db_path = evidence_root.join("rocksdb_edges");
    let edge_store =
        crate::teleological::RocksDbTeleologicalStore::open(&edge_db_path).expect("open edge DB");
    let edge_db = edge_store.dynamicjepa_db();
    let edge_before_counts = snapshot_dj_counts(edge_db).expect("fresh edge DB counts");
    assert!(edge_before_counts.values().all(|count| *count == 0));

    let empty_edge = edge_case(edge_db, "empty_payload", uuid(7001), vec![]);
    let mut wrong_version = encode_record(&domain).expect("encode valid domain");
    wrong_version[0] = 9;
    let wrong_version_edge = edge_case(edge_db, "wrong_version_byte", uuid(7002), wrong_version);
    let corrupt_body_edge = edge_case(
        edge_db,
        "corrupt_bincode_body",
        uuid(7003),
        vec![DOMAIN_PACK_RECORD_VERSION, 255, 255],
    );

    let edge_after_counts = snapshot_dj_counts(edge_db).expect("edge after counts");
    assert_eq!(*edge_after_counts.get(CF_DJ_DOMAIN_PACKS).unwrap(), 3);
    for edge in [&empty_edge, &wrong_version_edge, &corrupt_body_edge] {
        assert_eq!(edge["error_code"], "CODEC");
    }

    let evidence = json!({
        "operation": "dynamicjepa_phase2_storage_fsv",
        "status": "ok",
        "trigger_event": "cargo test -p context-graph-storage dynamicjepa_phase2_storage_full_state_verification -- --nocapture",
        "source_of_truth": {
            "happy_path": {
                "type": "physical RocksDB column families",
                "path": db_path.display().to_string(),
                "column_family_count": DJ_CF_COUNT,
                "column_families": DJ_CF_NAMES,
            },
            "edge_cases": {
                "type": "physical RocksDB column families with intentionally corrupt rows",
                "path": edge_db_path.display().to_string(),
            }
        },
        "fresh_open_state": {
            "dynamicjepa_cf_count": DJ_CF_COUNT,
            "before_counts": before_counts,
        },
        "happy_path": {
            "expected_counts": expected_final_counts,
            "actual_counts": after_counts,
            "domain_pack_storage_uuid": domain_uuid.to_string(),
            "domain_pack": {
                "id": domain.id.as_str(),
                "version": domain.version,
                "source_hash_sha256": hex32(&domain.source_hash),
                "instrument_count": domain.instrument_specs.len(),
                "adapter_count": domain.adapter_specs.len(),
            },
            "raw_event": {
                "event_id": samples.raw_event.event_id.to_string(),
                "payload_hash": hex32(&samples.raw_event.payload_hash),
                "payload_bytes": String::from_utf8(samples.raw_event.payload_bytes.clone()).unwrap(),
            },
            "panel": {
                "panel_id": samples.panel.panel_id.to_string(),
                "slot_vectors": samples.panel.slot_vectors,
            },
            "verification_run": {
                "verification_run_id": verification.verification_run_id.to_string(),
                "test_name": verification.test_name,
                "status": "Passed",
            },
            "decoded_rows_from_source_of_truth": decoded_rows,
        },
        "edge_case_audit": {
            "before_counts": edge_before_counts,
            "after_counts": edge_after_counts,
            "cases": [empty_edge, wrong_version_edge, corrupt_body_edge],
        }
    });

    let evidence_json = serde_json::to_string_pretty(&evidence).expect("serialize evidence");
    let evidence_path = root_dir().join("tmp/5090jepa_evidence/phase2/storage_fsv.json");
    fs::write(&evidence_path, &evidence_json).expect("write Phase 2 storage FSV evidence");
    println!("{evidence_json}");
}
