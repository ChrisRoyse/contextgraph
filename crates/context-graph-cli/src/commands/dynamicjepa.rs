//! DynamicJEPA CLI: domain-pack registry, physical inspection, adapter/panel
//! materialization, binding, trajectory, and dataset compilation.
//!
//! Mutating commands write to the real RocksDB DynamicJEPA column families, then
//! immediately read the source-of-truth rows back through separate storage
//! inspection paths.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, Subcommand};
use context_graph_core::dynamicjepa::{
    ActionId, ActionOrigin, ActionSchema, AdapterId, AdapterRunRecord, AdapterRunStatus,
    AdapterSpec, ArtifactHashCheck, BindingId, BindingKind, BindingMethod, BindingRecord,
    BindingRef, CandidateActionConfig, DatasetId, DatasetPolicy, DatasetShardId,
    DatasetShardRecord, DjJsonValue, DjRecordHeader, DomainPack, DomainPackId, DynamicJepaError,
    DynamicJepaRecord, DynamicJepaResult, EntitySchema, EventId, FieldKind, FieldSpec, FieldValue,
    InstrumentId, InstrumentKind, InstrumentReading, InstrumentSpec, InstrumentValidation,
    InvariantSpec, LatentPanel, LeakageReport, ModelArtifactId, Normalization, NormalizedAction,
    NormalizedOutcome, NormalizedState, ObjectiveSpec, OutcomeId, OutcomeSchema, PanelId,
    PanelSlot, PanelSlotKind, PayloadFormat, PlanTraceId, PlannerPolicy, PredictionId,
    RawDomainEvent, ReadingStatus, ShapeSummary, SkillId, SourceKind, StateId, StateSchema,
    StateTransition, SurpriseEventId, TimeEncoding, TimeSchema, TrainingRunId, TrajectoryId,
    TrajectoryRecord, TransitionId, Validate, VerificationPolicy, VerificationRunId,
    VerificationRunRecord, VerificationStatus, ADAPTER_RUN_RECORD_VERSION, BINDING_RECORD_VERSION,
    DATASET_SHARD_RECORD_VERSION, DOMAIN_PACK_RECORD_VERSION, INSTRUMENT_READING_RECORD_VERSION,
    LATENT_PANEL_RECORD_VERSION, NORMALIZED_ACTION_RECORD_VERSION,
    NORMALIZED_OUTCOME_RECORD_VERSION, NORMALIZED_STATE_RECORD_VERSION,
    RAW_DOMAIN_EVENT_RECORD_VERSION, STATE_TRANSITION_RECORD_VERSION, TRAJECTORY_RECORD_VERSION,
};
use context_graph_dynamicjepa::{
    plan_next_action, predict_next_panel, record_surprise, train_dataset, verify_artifact_files,
    TrainConfig,
};
use context_graph_storage::dynamicjepa::column_families::{
    CF_DJ_ACTIONS, CF_DJ_ADAPTER_REGISTRY, CF_DJ_ADAPTER_RUNS, CF_DJ_AUDIT_LOG, CF_DJ_BINDINGS,
    CF_DJ_BINDINGS_BY_ENTITY, CF_DJ_DATASET_SHARDS, CF_DJ_DOMAIN_PACKS,
    CF_DJ_DOMAIN_PACK_BY_NAME_VERSION, CF_DJ_GUARD_DECISIONS, CF_DJ_INSTRUMENT_READINGS,
    CF_DJ_INSTRUMENT_REGISTRY, CF_DJ_LATENT_PANELS, CF_DJ_MODEL_ARTIFACTS, CF_DJ_NORMALIZED_STATES,
    CF_DJ_OUTCOMES, CF_DJ_PLAN_TRACES, CF_DJ_PREDICTIONS, CF_DJ_RAW_EVENTS, CF_DJ_SKILL_POLICIES,
    CF_DJ_SURPRISE_EVENTS, CF_DJ_TRAINING_RUNS, CF_DJ_TRAJECTORIES, CF_DJ_TRANSITIONS,
    CF_DJ_VERIFICATION_RUNS, DJ_CF_NAMES,
};
use context_graph_storage::dynamicjepa::keys::{
    adapter_registry_key, instrument_reading_key, instrument_registry_key, name_version_key,
    uuid_key,
};
use context_graph_storage::dynamicjepa::{
    binding_entity_index_exists, get_action, get_adapter_run, get_adapter_spec, get_binding,
    get_dataset_shard, get_domain_pack, get_domain_pack_by_storage_id, get_guard_decision,
    get_instrument_reading, get_instrument_spec, get_latent_panel, get_model_artifact,
    get_normalized_state, get_outcome, get_plan_trace, get_prediction, get_raw_event,
    get_surprise_event, get_training_run, get_trajectory, get_transition, get_verification_run,
    inspect_cf, inspect_cf_key, list_adapter_runs, list_bindings, list_dataset_shards,
    list_domain_packs, list_instrument_readings_for_event, list_latent_panels, list_trajectories,
    list_transitions, put_adapter_run_failure_batch, put_adapter_run_success_batch,
    put_audit_record, put_binding_with_audit_batch, put_dataset_shard_with_audit_batch,
    put_domain_pack, put_panel_with_readings_batch, put_raw_event_and_adapter_run_started,
    put_trajectory_with_audit_batch, put_verification_run_with_audit_batch, snapshot_dj_counts,
};
use context_graph_storage::dynamicjepa::{AuditStatus, DjAuditRecord};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Subcommand, Debug)]
pub enum DynamicJepaCommands {
    /// Register a strict TOML domain pack into DynamicJEPA RocksDB CFs.
    RegisterDomain(RegisterDomainArgs),
    /// List registered domain packs from the physical DB.
    ListDomains(ListDomainsArgs),
    /// Get one registered domain pack by id and version.
    GetDomain(GetDomainArgs),
    /// Count every DynamicJEPA column family in the physical DB.
    InspectCounts(InspectCountsArgs),
    /// Decode rows from one DynamicJEPA column family.
    InspectCf(InspectCfArgs),
    /// Ingest a JSONL fixture and run its registered adapter.
    IngestEvent(IngestEventArgs),
    /// Run the registered adapter for one already persisted raw event.
    RunAdapter(RunAdapterArgs),
    /// Materialize one latent panel or every pending panel for a domain.
    MaterializePanel(MaterializePanelArgs),
    /// Read one latent panel, optionally with its instrument readings.
    GetPanel(GetPanelArgs),
    /// List instrument readings for one event.
    ListInstrumentReadings(ListInstrumentReadingsArgs),
    /// Create a deterministic binding between two decoded source-of-truth rows.
    Bind(BindArgs),
    /// List persisted bindings.
    ListBindings(ListBindingsArgs),
    /// Compile latent panels and transitions into deterministic trajectories.
    CompileTrajectories(CompileTrajectoriesArgs),
    /// Read one compiled trajectory.
    GetTrajectory(GetTrajectoryArgs),
    /// List compiled trajectories for a domain.
    ListTrajectories(ListTrajectoriesArgs),
    /// Compile one-step trajectory rows into a persisted dataset shard.
    CompileDataset(CompileDatasetArgs),
    /// Read one persisted dataset shard.
    GetDatasetShard(GetDatasetShardArgs),
    /// Inspect one decoded dataset row and its linked source-of-truth records.
    InspectDataset(InspectDatasetArgs),
    /// Train a tiny DynamicJEPA model from persisted dataset shards and register an artifact.
    Train(TrainArgs),
    /// Read a persisted DynamicJEPA training run.
    GetTrainingRun(GetTrainingRunArgs),
    /// Read a persisted DynamicJEPA model artifact and optionally verify files.
    GetArtifact(GetArtifactArgs),
    /// Run a hash-verified DynamicJEPA artifact prediction and persist the row.
    Predict(PredictArgs),
    /// Enumerate candidate actions, predict them, evaluate guards, and persist a plan trace.
    Plan(PlanArgs),
    /// Compare a prediction with an observed outcome/panel and persist surprise when threshold is breached.
    RecordSurprise(RecordSurpriseArgs),
    /// Read a persisted DynamicJEPA prediction.
    GetPrediction(GetPredictionArgs),
    /// Read a persisted DynamicJEPA plan trace.
    GetPlanTrace(GetPlanTraceArgs),
    /// Read a persisted DynamicJEPA surprise event.
    GetSurprise(GetSurpriseArgs),
    /// Run bundled counter_world end-to-end full-state verification.
    VerifyCounterWorld(VerifyDomainArgs),
    /// Run bundled gridworld_5x5 end-to-end full-state verification.
    VerifyGridworld(VerifyDomainArgs),
}

impl DynamicJepaCommands {
    fn operation(&self) -> &'static str {
        match self {
            Self::RegisterDomain(_) => "dynamicjepa_register_domain",
            Self::ListDomains(_) => "dynamicjepa_list_domains",
            Self::GetDomain(_) => "dynamicjepa_get_domain",
            Self::InspectCounts(_) => "dynamicjepa_inspect_counts",
            Self::InspectCf(_) => "dynamicjepa_inspect_cf",
            Self::IngestEvent(_) => "dynamicjepa_ingest_event",
            Self::RunAdapter(_) => "dynamicjepa_run_adapter",
            Self::MaterializePanel(_) => "dynamicjepa_materialize_panel",
            Self::GetPanel(_) => "dynamicjepa_get_panel",
            Self::ListInstrumentReadings(_) => "dynamicjepa_list_instrument_readings",
            Self::Bind(_) => "dynamicjepa_bind",
            Self::ListBindings(_) => "dynamicjepa_list_bindings",
            Self::CompileTrajectories(_) => "dynamicjepa_compile_trajectories",
            Self::GetTrajectory(_) => "dynamicjepa_get_trajectory",
            Self::ListTrajectories(_) => "dynamicjepa_list_trajectories",
            Self::CompileDataset(_) => "dynamicjepa_compile_dataset",
            Self::GetDatasetShard(_) => "dynamicjepa_get_dataset_shard",
            Self::InspectDataset(_) => "dynamicjepa_inspect_dataset",
            Self::Train(_) => "dynamicjepa_train",
            Self::GetTrainingRun(_) => "dynamicjepa_get_training_run",
            Self::GetArtifact(_) => "dynamicjepa_get_artifact",
            Self::Predict(_) => "dynamicjepa_predict",
            Self::Plan(_) => "dynamicjepa_plan",
            Self::RecordSurprise(_) => "dynamicjepa_record_surprise",
            Self::GetPrediction(_) => "dynamicjepa_get_prediction",
            Self::GetPlanTrace(_) => "dynamicjepa_get_plan_trace",
            Self::GetSurprise(_) => "dynamicjepa_get_surprise",
            Self::VerifyCounterWorld(_) => "dynamicjepa_verify_counter_world",
            Self::VerifyGridworld(_) => "dynamicjepa_verify_gridworld",
        }
    }
}

#[derive(Args, Debug)]
pub struct RegisterDomainArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Strict DynamicJEPA domain-pack TOML.
    #[arg(long)]
    pub file: PathBuf,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ListDomainsArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Maximum decoded domain-pack records to return.
    #[arg(long, default_value_t = 100)]
    pub limit: usize,

    /// Number of decoded domain-pack records to skip.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetDomainArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Domain pack id, for example counter_world.
    #[arg(long)]
    pub id: String,

    /// Domain pack version. Phase 0 fixtures use 1.0.0.
    #[arg(long = "domain-version", default_value = "1.0.0")]
    pub domain_version: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct InspectCountsArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct InspectCfArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// DynamicJEPA CF name, for example dj_domain_packs.
    #[arg(long)]
    pub cf: String,

    /// Maximum rows to decode.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,

    /// Number of rows to skip.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct IngestEventArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Registered domain pack id, for example counter_world.
    #[arg(long)]
    pub domain: String,

    /// Registered adapter id. Phase 4 supports json_event.
    #[arg(long)]
    pub adapter: String,

    /// JSONL fixture file to ingest.
    #[arg(long)]
    pub file: PathBuf,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RunAdapterArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Raw event id whose started adapter run should be completed.
    #[arg(long = "event-id")]
    pub event_id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct MaterializePanelArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Materialize a single transition id.
    #[arg(long = "transition-id", conflicts_with = "all_pending")]
    pub transition_id: Option<String>,

    /// Materialize every transition without a panel for the domain.
    #[arg(long = "all-pending", conflicts_with = "transition_id")]
    pub all_pending: bool,

    /// Domain id required with --all-pending.
    #[arg(long)]
    pub domain: Option<String>,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetPanelArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Panel id to read.
    #[arg(long = "panel-id")]
    pub panel_id: String,

    /// Include instrument reading rows linked by the panel.
    #[arg(long = "include-readings")]
    pub include_readings: bool,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ListInstrumentReadingsArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Event id whose readings should be read from dj_instrument_readings.
    #[arg(long = "event-id")]
    pub event_id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct BindArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Left source-of-truth DynamicJEPA CF name.
    #[arg(long = "left-cf")]
    pub left_cf: String,

    /// Left source-of-truth key as lowercase or uppercase hex bytes.
    #[arg(long = "left-key")]
    pub left_key: String,

    /// Right source-of-truth DynamicJEPA CF name.
    #[arg(long = "right-cf")]
    pub right_cf: String,

    /// Right source-of-truth key as lowercase or uppercase hex bytes.
    #[arg(long = "right-key")]
    pub right_key: String,

    /// Deterministic binding method: id_equality or explicit_mapping.
    #[arg(long, default_value = "explicit_mapping")]
    pub method: String,

    /// Binding kind: event_to_trajectory, entity_to_entity, state_to_goal, or panel_to_memory.
    #[arg(long, default_value = "event_to_trajectory")]
    pub kind: String,

    /// Calibrated score in [0,1].
    #[arg(long, default_value_t = 1.0)]
    pub score: f32,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ListBindingsArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Optional filter matching a binding CF name or key hex.
    #[arg(long)]
    pub entity: Option<String>,

    /// Maximum decoded bindings to return.
    #[arg(long, default_value_t = 100)]
    pub limit: usize,

    /// Number of decoded bindings to skip.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct CompileTrajectoriesArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Registered domain pack id.
    #[arg(long)]
    pub domain: String,

    /// Segmentation policy id. Phase 5 supports by_domain_session.
    #[arg(long, default_value = "by_domain_session")]
    pub policy: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetTrajectoryArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Trajectory id to read.
    #[arg(long)]
    pub id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ListTrajectoriesArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Registered domain pack id.
    #[arg(long)]
    pub domain: String,

    /// Maximum decoded trajectories to return.
    #[arg(long, default_value_t = 100)]
    pub limit: usize,

    /// Number of decoded trajectories to skip.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct CompileDatasetArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Registered domain pack id.
    #[arg(long)]
    pub domain: String,

    /// Dataset policy id. Phase 6 supports one_step.
    #[arg(long, default_value = "one_step")]
    pub policy: String,

    /// Split to compile: train, val, or test.
    #[arg(long, default_value = "train")]
    pub split: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetDatasetShardArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Dataset id returned by compile-dataset.
    #[arg(long = "dataset-id")]
    pub dataset_id: String,

    /// Dataset shard id returned by compile-dataset.
    #[arg(long = "shard-id")]
    pub shard_id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct InspectDatasetArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Dataset id returned by compile-dataset.
    #[arg(long = "dataset-id")]
    pub dataset_id: String,

    /// Dataset shard id returned by compile-dataset.
    #[arg(long = "shard-id")]
    pub shard_id: String,

    /// Zero-based row index to decode.
    #[arg(long)]
    pub row: usize,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct TrainArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Dataset id returned by compile-dataset.
    #[arg(long = "dataset-id")]
    pub dataset_id: String,

    /// Strict tiny training config JSON.
    #[arg(long)]
    pub config: PathBuf,

    /// Artifact root directory. The run writes <root>/<domain>/<training_run_id>/.
    #[arg(long = "artifact-root")]
    pub artifact_root: PathBuf,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetTrainingRunArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Training run id to read.
    #[arg(long)]
    pub id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetArtifactArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Model artifact id to read.
    #[arg(long)]
    pub id: String,

    /// Recompute file hashes from disk and compare to the registry row.
    #[arg(long = "verify-files")]
    pub verify_files: bool,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct PredictArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Model artifact id to verify and run.
    #[arg(long = "artifact-id")]
    pub artifact_id: String,

    /// Input latent panel id.
    #[arg(long = "panel-id")]
    pub panel_id: String,

    /// Candidate action id.
    #[arg(long = "action-id")]
    pub action_id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct PlanArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Model artifact id to verify and run.
    #[arg(long = "artifact-id")]
    pub artifact_id: String,

    /// Current latent panel id.
    #[arg(long = "panel-id")]
    pub panel_id: String,

    /// Skill policy id. If missing, plan writes enumerate_declared_actions under this id.
    #[arg(long = "skill-id")]
    pub skill_id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RecordSurpriseArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Prediction id to compare against the observed rows.
    #[arg(long = "prediction-id")]
    pub prediction_id: String,

    /// Observed outcome id.
    #[arg(long = "observed-outcome-id")]
    pub observed_outcome_id: String,

    /// Observed panel id.
    #[arg(long = "observed-panel-id")]
    pub observed_panel_id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetPredictionArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Prediction id to read.
    #[arg(long)]
    pub id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetPlanTraceArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Plan trace id to read.
    #[arg(long)]
    pub id: String,

    /// Include decoded prediction rows referenced by the trace.
    #[arg(long = "include-predictions")]
    pub include_predictions: bool,

    /// Include decoded guard decision rows referenced by the trace.
    #[arg(long = "include-guards")]
    pub include_guards: bool,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetSurpriseArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Surprise event id to read.
    #[arg(long)]
    pub id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct VerifyDomainArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Root containing domain_packs/ and verification/ fixtures.
    #[arg(long = "fixtures-root", default_value = "configs/dynamicjepa")]
    pub fixtures_root: PathBuf,

    /// Artifact root directory. The run writes <root>/<domain>/<training_run_id>/.
    #[arg(long = "artifact-root")]
    pub artifact_root: PathBuf,

    /// Evidence root. The harness writes <root>/<verification_run_uuid>/.
    #[arg(long = "evidence-root")]
    pub evidence_root: Option<PathBuf>,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

pub async fn handle_dynamicjepa_command(action: DynamicJepaCommands) -> i32 {
    let operation = action.operation();
    let outcome = run(action).await;
    match outcome {
        Ok(outcome) => {
            if let Err(err) = print_json(&outcome.value) {
                error!(
                    operation,
                    error_code = "CLI_JSON_OUTPUT",
                    error_message = %err,
                    "DynamicJEPA CLI could not serialize JSON output"
                );
                eprintln!("dynamicjepa command FAILED: JSON output serialization failed: {err}");
                return 1;
            }
            if outcome.exit_code == 0 {
                info!(
                    operation,
                    status = "ok",
                    "DynamicJEPA CLI command completed"
                );
            } else {
                error!(
                    operation,
                    status = "error",
                    "DynamicJEPA CLI command returned a structured error"
                );
            }
            outcome.exit_code
        }
        Err(err) => {
            let value = error_value(operation, None, &[], None, None, &err);
            let _ = print_json(&value);
            error!(
                operation,
                error_code = err.code(),
                error_message = %err,
                "DynamicJEPA CLI command failed before source-of-truth counts could be read"
            );
            1
        }
    }
}

struct CommandOutcome {
    value: Value,
    exit_code: i32,
}

#[derive(Debug)]
pub struct DynamicJepaCommandOutcome {
    pub value: Value,
    pub exit_code: i32,
}

pub async fn run_dynamicjepa_command(
    action: DynamicJepaCommands,
) -> DynamicJepaResult<DynamicJepaCommandOutcome> {
    run(action).await.map(|outcome| DynamicJepaCommandOutcome {
        value: outcome.value,
        exit_code: outcome.exit_code,
    })
}

async fn run(action: DynamicJepaCommands) -> DynamicJepaResult<CommandOutcome> {
    match action {
        DynamicJepaCommands::RegisterDomain(args) => register_domain(args),
        DynamicJepaCommands::ListDomains(args) => list_domains(args),
        DynamicJepaCommands::GetDomain(args) => get_domain(args),
        DynamicJepaCommands::InspectCounts(args) => inspect_counts(args),
        DynamicJepaCommands::InspectCf(args) => inspect_column_family(args),
        DynamicJepaCommands::IngestEvent(args) => ingest_event(args),
        DynamicJepaCommands::RunAdapter(args) => run_adapter_command(args),
        DynamicJepaCommands::MaterializePanel(args) => materialize_panel_command(args),
        DynamicJepaCommands::GetPanel(args) => get_panel_command(args),
        DynamicJepaCommands::ListInstrumentReadings(args) => list_instrument_readings_command(args),
        DynamicJepaCommands::Bind(args) => bind_command(args),
        DynamicJepaCommands::ListBindings(args) => list_bindings_command(args),
        DynamicJepaCommands::CompileTrajectories(args) => compile_trajectories_command(args),
        DynamicJepaCommands::GetTrajectory(args) => get_trajectory_command(args),
        DynamicJepaCommands::ListTrajectories(args) => list_trajectories_command(args),
        DynamicJepaCommands::CompileDataset(args) => compile_dataset_command(args),
        DynamicJepaCommands::GetDatasetShard(args) => get_dataset_shard_command(args),
        DynamicJepaCommands::InspectDataset(args) => inspect_dataset_command(args),
        DynamicJepaCommands::Train(args) => train_command(args),
        DynamicJepaCommands::GetTrainingRun(args) => get_training_run_command(args),
        DynamicJepaCommands::GetArtifact(args) => get_artifact_command(args),
        DynamicJepaCommands::Predict(args) => predict_command(args),
        DynamicJepaCommands::Plan(args) => plan_command(args),
        DynamicJepaCommands::RecordSurprise(args) => record_surprise_command(args),
        DynamicJepaCommands::GetPrediction(args) => get_prediction_command(args),
        DynamicJepaCommands::GetPlanTrace(args) => get_plan_trace_command(args),
        DynamicJepaCommands::GetSurprise(args) => get_surprise_command(args),
        DynamicJepaCommands::VerifyCounterWorld(args) => verify_counter_world_command(args),
        DynamicJepaCommands::VerifyGridworld(args) => verify_gridworld_command(args),
    }
}

fn register_domain(args: RegisterDomainArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_DOMAIN_PACKS,
        CF_DJ_DOMAIN_PACK_BY_NAME_VERSION,
        CF_DJ_INSTRUMENT_REGISTRY,
        CF_DJ_ADAPTER_REGISTRY,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "register-domain.db")?;
    let file_path = canonical_existing_path(&args.file, "register-domain.file")?;
    let before_counts = snapshot_dj_counts(db)?;

    let result = (|| -> DynamicJepaResult<Value> {
        let domain_pack = parse_domain_pack_file(&args.file)?;
        if get_domain_pack(db, &domain_pack.id, &domain_pack.version)?.is_some() {
            return Err(DynamicJepaError::validation(
                "register-domain.id_version",
                format!(
                    "domain pack {} version {} already exists",
                    domain_pack.id, domain_pack.version
                ),
                "bump the domain pack version or wipe the pre-production demo DB before registering again",
            ));
        }

        let domain_pack_uuid = put_domain_pack(db, &domain_pack)?;
        let primary = get_domain_pack_by_storage_id(db, domain_pack_uuid)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_DOMAIN_PACKS.to_string(),
                key: uuid_key(domain_pack_uuid).to_vec(),
            }
        })?;
        let by_name_version = get_domain_pack(db, &domain_pack.id, &domain_pack.version)?
            .ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_DOMAIN_PACK_BY_NAME_VERSION.to_string(),
                key: name_version_key(&domain_pack.id, &domain_pack.version).to_vec(),
            })?;

        if primary != domain_pack || by_name_version != domain_pack {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "registered domain pack readback mismatch for {}:{}",
                    domain_pack.id, domain_pack.version
                ),
            });
        }

        let mut instrument_records = Vec::with_capacity(domain_pack.instrument_specs.len());
        for spec in &domain_pack.instrument_specs {
            let stored = get_instrument_spec(db, domain_pack_uuid, &spec.instrument_id)?
                .ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_INSTRUMENT_REGISTRY.to_string(),
                    key: instrument_registry_key(domain_pack_uuid, &spec.instrument_id).to_vec(),
                })?;
            if stored != *spec {
                return Err(DynamicJepaError::StorageInvariantViolation {
                    message: format!(
                        "instrument registry readback mismatch for {} in domain {}",
                        spec.instrument_id, domain_pack.id
                    ),
                });
            }
            instrument_records.push(serde_json::to_value(&stored).map_err(|err| {
                DynamicJepaError::validation(
                    "register-domain.instrument_json",
                    format!("failed to serialize instrument readback: {err}"),
                    "inspect the InstrumentSpec serde implementation",
                )
            })?);
        }

        let mut adapter_records = Vec::with_capacity(domain_pack.adapter_specs.len());
        for spec in &domain_pack.adapter_specs {
            let stored =
                get_adapter_spec(db, domain_pack_uuid, &spec.adapter_id)?.ok_or_else(|| {
                    DynamicJepaError::SourceOfTruthMissing {
                        cf: CF_DJ_ADAPTER_REGISTRY.to_string(),
                        key: adapter_registry_key(domain_pack_uuid, &spec.adapter_id).to_vec(),
                    }
                })?;
            if stored != *spec {
                return Err(DynamicJepaError::StorageInvariantViolation {
                    message: format!(
                        "adapter registry readback mismatch for {} in domain {}",
                        spec.adapter_id, domain_pack.id
                    ),
                });
            }
            adapter_records.push(serde_json::to_value(&stored).map_err(|err| {
                DynamicJepaError::validation(
                    "register-domain.adapter_json",
                    format!("failed to serialize adapter readback: {err}"),
                    "inspect the AdapterSpec serde implementation",
                )
            })?);
        }

        let cf_samples = json!({
            CF_DJ_DOMAIN_PACKS: inspect_cf(db, CF_DJ_DOMAIN_PACKS, 10, 0)?,
            CF_DJ_DOMAIN_PACK_BY_NAME_VERSION: inspect_cf(db, CF_DJ_DOMAIN_PACK_BY_NAME_VERSION, 10, 0)?,
            CF_DJ_INSTRUMENT_REGISTRY: inspect_cf(db, CF_DJ_INSTRUMENT_REGISTRY, 20, 0)?,
            CF_DJ_ADAPTER_REGISTRY: inspect_cf(db, CF_DJ_ADAPTER_REGISTRY, 20, 0)?,
            CF_DJ_AUDIT_LOG: inspect_cf(db, CF_DJ_AUDIT_LOG, 20, 0)?,
        });

        let created_ids = json!({
            "domain_pack_uuid": domain_pack_uuid.to_string(),
            "domain_pack_id": domain_pack.id.to_string(),
            "domain_pack_version": domain_pack.version,
            "instrument_ids": domain_pack
                .instrument_specs
                .iter()
                .map(|spec| spec.instrument_id.to_string())
                .collect::<Vec<_>>(),
            "adapter_ids": domain_pack
                .adapter_specs
                .iter()
                .map(|spec| spec.adapter_id.to_string())
                .collect::<Vec<_>>(),
        });

        Ok(json!({
            "created_ids": created_ids,
            "decoded_records": {
                "domain_pack": primary,
                "domain_pack_by_name_version": by_name_version,
                "instrument_registry": instrument_records,
                "adapter_registry": adapter_records,
                "cf_samples": cf_samples,
                "source_hash_sha256": hex_bytes(&domain_pack.source_hash),
            }
        }))
    })();

    let after_counts = match snapshot_dj_counts(db) {
        Ok(counts) => counts,
        Err(count_err) => {
            if let Some(primary) = result.as_ref().err() {
                return Err(DynamicJepaError::StorageInvariantViolation {
                    message: format!(
                        "register-domain failed with {primary}; after-count source-of-truth inspection also failed with {count_err}"
                    ),
                });
            }
            return Err(count_err);
        }
    };
    match result {
        Ok(decoded_records) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_register_domain",
                "status": "ok",
                "db_path": db_path,
                "input_file": file_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": decoded_records["created_ids"].clone(),
                "decoded_records": decoded_records["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => Ok(CommandOutcome {
            value: error_value(
                "dynamicjepa_register_domain",
                Some(db_path),
                &source_cfs,
                Some(before_counts),
                Some(after_counts),
                &err,
            ),
            exit_code: 1,
        }),
    }
}

fn list_domains(args: ListDomainsArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    if args.limit == 0 {
        return Err(DynamicJepaError::validation(
            "list-domains.limit",
            "--limit must be greater than zero",
            "request at least one row so the source-of-truth read has observable output",
        ));
    }
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "list-domains.db")?;
    let counts = snapshot_dj_counts(db)?;
    let domains = list_domain_packs(db, args.limit, args.offset)?;
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_list_domains",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": [CF_DJ_DOMAIN_PACKS],
            "counts": counts,
            "limit": args.limit,
            "offset": args.offset,
            "decoded_records": {
                "domains": domains,
            },
        }),
        exit_code: 0,
    })
}

fn get_domain(args: GetDomainArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-domain.db")?;
    let counts = snapshot_dj_counts(db)?;
    let id = DomainPackId::new(args.id.clone())?;
    let record = get_domain_pack(db, &id, &args.domain_version)?;
    match record {
        Some(record) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_domain",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_DOMAIN_PACKS, CF_DJ_DOMAIN_PACK_BY_NAME_VERSION],
                "counts": counts,
                "decoded_records": {
                    "domain_pack": record,
                },
            }),
            exit_code: 0,
        }),
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_domain",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_DOMAIN_PACKS, CF_DJ_DOMAIN_PACK_BY_NAME_VERSION],
                "counts": counts,
                "queried": {
                    "id": args.id,
                    "version": args.domain_version,
                },
                "remediation": "register the domain pack with dynamicjepa register-domain before reading it",
            }),
            exit_code: 0,
        }),
    }
}

fn inspect_counts(args: InspectCountsArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "inspect-counts.db")?;
    let counts = snapshot_dj_counts(db)?;
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_inspect_counts",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": DJ_CF_NAMES,
            "counts": counts,
            "after_counts": counts,
        }),
        exit_code: 0,
    })
}

fn inspect_column_family(args: InspectCfArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    if args.limit == 0 {
        return Err(DynamicJepaError::validation(
            "inspect-cf.limit",
            "--limit must be greater than zero",
            "request at least one row so the source-of-truth read has observable output",
        ));
    }
    let cf_name = resolve_dynamic_cf_name(&args.cf)?;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "inspect-cf.db")?;
    let counts = snapshot_dj_counts(db)?;
    let rows = inspect_cf(db, cf_name, args.limit, args.offset)?;
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_inspect_cf",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": [cf_name],
            "counts": counts,
            "cf": cf_name,
            "limit": args.limit,
            "offset": args.offset,
            "decoded_records": {
                "rows": rows,
            },
        }),
        exit_code: 0,
    })
}

fn ingest_event(args: IngestEventArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = phase4_ingest_cfs();
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "ingest-event.db")?;
    let file_path = canonical_existing_path(&args.file, "ingest-event.file")?;
    let before_counts = snapshot_dj_counts(db)?;
    let domain_id = DomainPackId::new(args.domain.clone())?;
    let adapter_id = AdapterId::new(args.adapter.clone())?;
    let mut created = IngestCreatedIds::default();

    let result = (|| -> DynamicJepaResult<Value> {
        let domain_pack = get_domain_pack(db, &domain_id, "1.0.0")?.ok_or_else(|| {
            DynamicJepaError::DomainPackNotFound {
                id: domain_id.to_string(),
            }
        })?;
        let adapter = domain_pack
            .adapter_specs
            .iter()
            .find(|spec| spec.adapter_id == adapter_id)
            .ok_or_else(|| {
                DynamicJepaError::validation(
                    "ingest-event.adapter",
                    format!(
                        "adapter {} is not registered for domain {}",
                        adapter_id, domain_pack.id
                    ),
                    "register the domain pack with the requested adapter before ingestion",
                )
            })?
            .clone();
        ingest_jsonl_fixture(
            db,
            &domain_pack,
            &adapter,
            &args.file,
            &file_path,
            &mut created,
        )?;
        verify_ingest_created_ids(db, &created)?;
        Ok(json!({
            "created_ids": created,
            "decoded_records": phase4_ingest_decoded_records(db)?,
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_ingest_event",
                "status": "ok",
                "db_path": db_path,
                "input_file": file_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => {
            let mut value = error_value(
                "dynamicjepa_ingest_event",
                Some(db_path),
                &source_cfs,
                Some(before_counts.clone()),
                Some(after_counts.clone()),
                &err,
            );
            enrich_phase4_error_value(
                &mut value,
                &before_counts,
                &after_counts,
                json!(created),
                phase4_ingest_decoded_records(db)?,
            )?;
            Ok(CommandOutcome {
                value,
                exit_code: 1,
            })
        }
    }
}

fn run_adapter_command(args: RunAdapterArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = phase4_ingest_cfs();
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "run-adapter.db")?;
    let before_counts = snapshot_dj_counts(db)?;
    let event_id = parse_event_id(&args.event_id, "run-adapter.event_id")?;
    let mut created = IngestCreatedIds::default();

    let result = (|| -> DynamicJepaResult<Value> {
        run_adapter_for_event(db, event_id, &mut created)?;
        verify_ingest_created_ids(db, &created)?;
        Ok(json!({
            "created_ids": created,
            "decoded_records": phase4_ingest_decoded_records(db)?,
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_run_adapter",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => {
            let mut value = error_value(
                "dynamicjepa_run_adapter",
                Some(db_path),
                &source_cfs,
                Some(before_counts.clone()),
                Some(after_counts.clone()),
                &err,
            );
            enrich_phase4_error_value(
                &mut value,
                &before_counts,
                &after_counts,
                json!(created),
                phase4_ingest_decoded_records(db)?,
            )?;
            Ok(CommandOutcome {
                value,
                exit_code: 1,
            })
        }
    }
}

fn materialize_panel_command(args: MaterializePanelArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_TRANSITIONS,
        CF_DJ_NORMALIZED_STATES,
        CF_DJ_ACTIONS,
        CF_DJ_OUTCOMES,
        CF_DJ_INSTRUMENT_READINGS,
        CF_DJ_LATENT_PANELS,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "materialize-panel.db")?;
    let before_counts = snapshot_dj_counts(db)?;
    let mut created = PanelCreatedIds::default();

    let result = (|| -> DynamicJepaResult<Value> {
        if args.all_pending {
            let domain_raw = args.domain.as_ref().ok_or_else(|| {
                DynamicJepaError::validation(
                    "materialize-panel.domain",
                    "--domain is required with --all-pending",
                    "provide the registered domain id whose pending transitions should be scanned",
                )
            })?;
            let domain_id = DomainPackId::new(domain_raw.clone())?;
            let domain_pack = get_domain_pack(db, &domain_id, "1.0.0")?.ok_or_else(|| {
                DynamicJepaError::DomainPackNotFound {
                    id: domain_id.to_string(),
                }
            })?;
            let transitions = list_transitions(db, 1_000_000, 0)?;
            let mut existing_panel_keys = list_latent_panels(db, 1_000_000, 0)?
                .into_iter()
                .filter(|panel| panel.header.domain_pack_id == domain_pack.id)
                .filter_map(|panel| {
                    panel
                        .outcome_id
                        .map(|outcome_id| (panel.state_id, panel.action_id, outcome_id))
                })
                .collect::<BTreeSet<_>>();
            let mut skipped = 0u64;
            for transition in transitions
                .into_iter()
                .filter(|transition| transition.header.domain_pack_id == domain_pack.id)
            {
                let panel_key = (
                    transition.prior_state,
                    transition.action,
                    transition.outcome,
                );
                if existing_panel_keys.contains(&panel_key) {
                    skipped += 1;
                    continue;
                }
                let evidence = materialize_one_panel(db, &domain_pack, transition)?;
                existing_panel_keys.insert(panel_key);
                created.push(evidence);
            }
            Ok(json!({
                "created_ids": created,
                "summary": {
                    "materialized": created.panel_ids.len(),
                    "skipped_existing": skipped,
                },
                "decoded_records": phase4_panel_decoded_records(db)?,
            }))
        } else {
            let transition_raw = args.transition_id.as_ref().ok_or_else(|| {
                DynamicJepaError::validation(
                    "materialize-panel.transition_id",
                    "either --transition-id or --all-pending must be provided",
                    "choose one materialization mode explicitly",
                )
            })?;
            let transition_id =
                parse_transition_id(transition_raw, "materialize-panel.transition_id")?;
            let transition = get_transition(db, transition_id)?.ok_or_else(|| {
                DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_TRANSITIONS.to_string(),
                    key: transition_id.into_bytes().to_vec(),
                }
            })?;
            if panel_for_transition(db, &transition)?.is_some() {
                return Err(DynamicJepaError::validation(
                    "materialize-panel.transition_id",
                    format!("transition {transition_id} already has a latent panel"),
                    "use --all-pending to skip existing panels or wipe the demo DB before rebuilding",
                ));
            }
            let domain_pack = get_domain_pack(
                db,
                &transition.header.domain_pack_id,
                &transition.header.domain_pack_version,
            )?
            .ok_or_else(|| DynamicJepaError::DomainPackNotFound {
                id: transition.header.domain_pack_id.to_string(),
            })?;
            let evidence = materialize_one_panel(db, &domain_pack, transition)?;
            created.push(evidence);
            Ok(json!({
                "created_ids": created,
                "summary": {
                    "materialized": created.panel_ids.len(),
                    "skipped_existing": 0,
                },
                "decoded_records": phase4_panel_decoded_records(db)?,
            }))
        }
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_materialize_panel",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "summary": value["summary"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => {
            let mut value = error_value(
                "dynamicjepa_materialize_panel",
                Some(db_path),
                &source_cfs,
                Some(before_counts.clone()),
                Some(after_counts.clone()),
                &err,
            );
            enrich_phase4_error_value(
                &mut value,
                &before_counts,
                &after_counts,
                json!(created),
                phase4_panel_decoded_records(db)?,
            )?;
            Ok(CommandOutcome {
                value,
                exit_code: 1,
            })
        }
    }
}

fn get_panel_command(args: GetPanelArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-panel.db")?;
    let counts = snapshot_dj_counts(db)?;
    let panel_id = parse_panel_id(&args.panel_id, "get-panel.panel_id")?;
    let panel = get_latent_panel(db, panel_id)?;
    match panel {
        Some(panel) => {
            let readings = if args.include_readings {
                list_instrument_readings_for_event(db, panel.event_id)?
            } else {
                Vec::new()
            };
            Ok(CommandOutcome {
                value: json!({
                    "operation": "dynamicjepa_get_panel",
                    "status": "ok",
                    "db_path": db_path,
                    "source_of_truth": [CF_DJ_LATENT_PANELS, CF_DJ_INSTRUMENT_READINGS],
                    "counts": counts,
                    "decoded_records": {
                        "panel": panel,
                        "instrument_readings": readings,
                    },
                }),
                exit_code: 0,
            })
        }
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_panel",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_LATENT_PANELS],
                "counts": counts,
                "queried": {"panel_id": args.panel_id},
                "remediation": "materialize the panel from a persisted transition before reading it",
            }),
            exit_code: 0,
        }),
    }
}

fn list_instrument_readings_command(
    args: ListInstrumentReadingsArgs,
) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "list-instrument-readings.db")?;
    let counts = snapshot_dj_counts(db)?;
    let event_id = parse_event_id(&args.event_id, "list-instrument-readings.event_id")?;
    let readings = list_instrument_readings_for_event(db, event_id)?;
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_list_instrument_readings",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": [CF_DJ_INSTRUMENT_READINGS],
            "counts": counts,
            "queried": {"event_id": args.event_id},
            "decoded_records": {
                "instrument_readings": readings,
            },
        }),
        exit_code: 0,
    })
}

fn bind_command(args: BindArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [CF_DJ_BINDINGS, CF_DJ_BINDINGS_BY_ENTITY, CF_DJ_AUDIT_LOG];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "bind.db")?;
    let before_counts = snapshot_dj_counts(db)?;

    let result = (|| -> DynamicJepaResult<Value> {
        let left_cf = resolve_bindable_cf_name(&args.left_cf, "bind.left_cf")?;
        let right_cf = resolve_bindable_cf_name(&args.right_cf, "bind.right_cf")?;
        let left_key = parse_hex_key(&args.left_key, "bind.left_key")?;
        let right_key = parse_hex_key(&args.right_key, "bind.right_key")?;
        let method = parse_binding_method(&args.method)?;
        let binding_kind = parse_binding_kind(&args.kind)?;
        if matches!(method, BindingMethod::IdEquality) && left_key != right_key {
            return Err(DynamicJepaError::validation(
                "bind.method",
                "id_equality requires left-key and right-key to be byte-identical",
                "use --method explicit_mapping when binding distinct source-of-truth rows",
            ));
        }

        let binding_id = BindingId::new_v4();
        let left_decoded = inspect_cf_key(db, left_cf, &left_key)?.ok_or_else(|| {
            DynamicJepaError::BindingEvidenceMissing {
                binding_id: binding_id.0,
                evidence_ref: format!("{}:{}", left_cf, hex_bytes(&left_key)),
            }
        })?;
        let right_decoded = inspect_cf_key(db, right_cf, &right_key)?.ok_or_else(|| {
            DynamicJepaError::BindingEvidenceMissing {
                binding_id: binding_id.0,
                evidence_ref: format!("{}:{}", right_cf, hex_bytes(&right_key)),
            }
        })?;
        let (left_domain, left_version) = decoded_domain_identity(&left_decoded, "bind.left_ref")?;
        let (right_domain, _right_version) =
            decoded_domain_identity(&right_decoded, "bind.right_ref")?;
        if left_version.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                "bind.left_ref.header.domain_pack_version",
                "left reference decoded without a domain version",
                "bind only records with a DynamicJEPA record header",
            ));
        }

        let left_ref = BindingRef {
            cf: left_cf.to_string(),
            key_bytes: left_key,
        };
        let right_ref = BindingRef {
            cf: right_cf.to_string(),
            key_bytes: right_key,
        };
        let now = now_unix_ms()?;
        let run_id = Uuid::new_v4();
        let mut record = BindingRecord {
            header: DjRecordHeader::new(
                binding_id.0,
                BINDING_RECORD_VERSION,
                left_domain.clone(),
                left_version,
                now,
                Some(run_id),
            ),
            binding_id,
            binding_kind,
            left_ref: left_ref.clone(),
            right_ref: right_ref.clone(),
            evidence_refs: vec![left_ref.clone(), right_ref.clone()],
            score: args.score,
            method,
            left_domain_pack_id: left_domain,
            right_domain_pack_id: right_domain,
            created_by_run_id: run_id,
            version: 1,
        };
        record.refresh_content_hash()?;
        let audit = build_audit(
            "create_binding",
            AuditStatus::Ok,
            vec![
                format!(
                    "{}:{}",
                    record.left_ref.cf,
                    hex_bytes(&record.left_ref.key_bytes)
                ),
                format!(
                    "{}:{}",
                    record.right_ref.cf,
                    hex_bytes(&record.right_ref.key_bytes)
                ),
            ],
            vec![binding_id.to_string()],
            vec![
                CF_DJ_BINDINGS.to_string(),
                CF_DJ_BINDINGS_BY_ENTITY.to_string(),
                CF_DJ_AUDIT_LOG.to_string(),
            ],
            vec![record.header.content_hash],
        )?;
        put_binding_with_audit_batch(db, &record, &audit)?;

        let stored =
            get_binding(db, binding_id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_BINDINGS.to_string(),
                key: binding_id.into_bytes().to_vec(),
            })?;
        if stored != record {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!("binding readback mismatch for binding {binding_id}"),
            });
        }
        if !binding_entity_index_exists(db, &record.left_ref, binding_id)? {
            return Err(DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_BINDINGS_BY_ENTITY.to_string(),
                key: context_graph_storage::dynamicjepa::binding_entity_index_key(
                    &record.left_ref,
                    binding_id,
                )?
                .to_vec(),
            });
        }

        Ok(json!({
            "created_ids": {
                "binding_id": binding_id.to_string(),
                "audit_id": audit.audit_id.to_string(),
            },
            "decoded_records": {
                "binding": stored,
                "left_ref": left_decoded,
                "right_ref": right_decoded,
                "bindings_cf": inspect_cf(db, CF_DJ_BINDINGS, 20, 0)?,
                "bindings_by_entity_cf": inspect_cf(db, CF_DJ_BINDINGS_BY_ENTITY, 20, 0)?,
                "audit_log": inspect_cf(db, CF_DJ_AUDIT_LOG, 80, 0)?,
            }
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_bind",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => Ok(CommandOutcome {
            value: error_value(
                "dynamicjepa_bind",
                Some(db_path),
                &source_cfs,
                Some(before_counts),
                Some(after_counts),
                &err,
            ),
            exit_code: 1,
        }),
    }
}

fn list_bindings_command(args: ListBindingsArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    if args.limit == 0 {
        return Err(DynamicJepaError::validation(
            "list-bindings.limit",
            "--limit must be greater than zero",
            "request at least one row so the source-of-truth read has observable output",
        ));
    }
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "list-bindings.db")?;
    let counts = snapshot_dj_counts(db)?;
    let mut bindings = list_bindings(db, 1_000_000, 0)?;
    if let Some(entity) = &args.entity {
        let needle = entity.to_ascii_lowercase();
        bindings.retain(|binding| binding_matches_entity_filter(binding, &needle));
    }
    let total_after_filter = bindings.len();
    let bindings = bindings
        .into_iter()
        .skip(args.offset)
        .take(args.limit)
        .collect::<Vec<_>>();
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_list_bindings",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": [CF_DJ_BINDINGS, CF_DJ_BINDINGS_BY_ENTITY],
            "counts": counts,
            "limit": args.limit,
            "offset": args.offset,
            "filter": args.entity,
            "decoded_records": {
                "total_after_filter": total_after_filter,
                "bindings": bindings,
            },
        }),
        exit_code: 0,
    })
}

fn compile_trajectories_command(
    args: CompileTrajectoriesArgs,
) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_TRANSITIONS,
        CF_DJ_LATENT_PANELS,
        CF_DJ_TRAJECTORIES,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "compile-trajectories.db")?;
    let before_counts = snapshot_dj_counts(db)?;

    let result = (|| -> DynamicJepaResult<Value> {
        if args.policy != "by_domain_session" {
            return Err(DynamicJepaError::validation(
                "compile-trajectories.policy",
                format!("unsupported segmentation policy {:?}", args.policy),
                "Phase 5 supports exactly by_domain_session",
            ));
        }
        let domain_id = DomainPackId::new(args.domain.clone())?;
        let domain_pack = get_domain_pack(db, &domain_id, "1.0.0")?.ok_or_else(|| {
            DynamicJepaError::DomainPackNotFound {
                id: domain_id.to_string(),
            }
        })?;
        let existing = list_trajectories(db, 1_000_000, 0)?
            .into_iter()
            .filter(|trajectory| trajectory.header.domain_pack_id == domain_pack.id)
            .collect::<Vec<_>>();
        if !existing.is_empty() {
            return Err(DynamicJepaError::validation(
                "compile-trajectories.domain",
                format!(
                    "domain {} already has {} compiled trajectories",
                    domain_pack.id,
                    existing.len()
                ),
                "wipe the pre-production demo DB before recompiling trajectories; Phase 5 does not delete or merge existing source-of-truth rows",
            ));
        }

        let transitions = list_transitions(db, 1_000_000, 0)?
            .into_iter()
            .filter(|transition| transition.header.domain_pack_id == domain_pack.id)
            .collect::<Vec<_>>();
        if transitions.is_empty() {
            return Err(DynamicJepaError::validation(
                "compile-trajectories.transitions",
                format!("domain {} has no transitions to compile", domain_pack.id),
                "run ingest-event successfully before compiling trajectories",
            ));
        }
        let panels = list_latent_panels(db, 1_000_000, 0)?
            .into_iter()
            .filter(|panel| panel.header.domain_pack_id == domain_pack.id)
            .collect::<Vec<_>>();
        let panel_by_transition = panel_index_by_transition(panels)?;
        let mut groups: BTreeMap<String, TrajectoryGroup> = BTreeMap::new();
        for transition in transitions {
            let panel_key = (
                transition.prior_state.0,
                transition.action.0,
                transition.outcome.0,
            );
            let panel = panel_by_transition
                .get(&panel_key)
                .cloned()
                .ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_LATENT_PANELS.to_string(),
                    key: transition.transition_id.into_bytes().to_vec(),
                })?;
            let (group_key, entity_refs) = trajectory_group_key(db, &domain_pack, &transition)?;
            groups
                .entry(group_key)
                .or_insert_with(|| TrajectoryGroup {
                    entity_refs,
                    members: Vec::new(),
                })
                .members
                .push(TrajectoryMember { transition, panel });
        }
        if groups.is_empty() {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "domain {} had transitions but no trajectory groups were produced",
                    domain_pack.id
                ),
            });
        }

        let run_id = Uuid::new_v4();
        let mut created_ids = Vec::new();
        let mut decoded = Vec::new();
        for (group_key, group) in groups {
            let record =
                build_trajectory_record(&domain_pack, &args.policy, &group_key, group, run_id)?;
            let audit = build_audit(
                "compile_trajectory",
                AuditStatus::Ok,
                record
                    .ordered_transition_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                vec![record.trajectory_id.to_string()],
                vec![CF_DJ_TRAJECTORIES.to_string(), CF_DJ_AUDIT_LOG.to_string()],
                vec![record.header.content_hash],
            )?;
            put_trajectory_with_audit_batch(db, &record, &audit)?;
            let stored = get_trajectory(db, record.trajectory_id)?.ok_or_else(|| {
                DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_TRAJECTORIES.to_string(),
                    key: record.trajectory_id.into_bytes().to_vec(),
                }
            })?;
            if stored != record {
                return Err(DynamicJepaError::StorageInvariantViolation {
                    message: format!(
                        "trajectory readback mismatch for trajectory {}",
                        record.trajectory_id
                    ),
                });
            }
            verify_trajectory_sources(db, &stored)?;
            created_ids.push(stored.trajectory_id.to_string());
            decoded.push(stored);
        }

        Ok(json!({
            "created_ids": {
                "trajectory_ids": created_ids,
                "run_id": run_id.to_string(),
            },
            "summary": {
                "trajectory_count": decoded.len(),
            },
            "decoded_records": {
                "trajectories": decoded,
                "trajectories_cf": inspect_cf(db, CF_DJ_TRAJECTORIES, 20, 0)?,
                "audit_log": inspect_cf(db, CF_DJ_AUDIT_LOG, 120, 0)?,
            }
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_compile_trajectories",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "summary": value["summary"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => Ok(CommandOutcome {
            value: error_value(
                "dynamicjepa_compile_trajectories",
                Some(db_path),
                &source_cfs,
                Some(before_counts),
                Some(after_counts),
                &err,
            ),
            exit_code: 1,
        }),
    }
}

fn get_trajectory_command(args: GetTrajectoryArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-trajectory.db")?;
    let counts = snapshot_dj_counts(db)?;
    let trajectory_id = parse_trajectory_id(&args.id, "get-trajectory.id")?;
    let record = get_trajectory(db, trajectory_id)?;
    match record {
        Some(record) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_trajectory",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_TRAJECTORIES],
                "counts": counts,
                "decoded_records": {
                    "trajectory": record,
                },
            }),
            exit_code: 0,
        }),
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_trajectory",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_TRAJECTORIES],
                "counts": counts,
                "queried": {"trajectory_id": args.id},
                "remediation": "compile trajectories before reading a trajectory id",
            }),
            exit_code: 0,
        }),
    }
}

fn list_trajectories_command(args: ListTrajectoriesArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    if args.limit == 0 {
        return Err(DynamicJepaError::validation(
            "list-trajectories.limit",
            "--limit must be greater than zero",
            "request at least one row so the source-of-truth read has observable output",
        ));
    }
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "list-trajectories.db")?;
    let counts = snapshot_dj_counts(db)?;
    let domain_id = DomainPackId::new(args.domain.clone())?;
    let mut trajectories = list_trajectories(db, 1_000_000, 0)?
        .into_iter()
        .filter(|trajectory| trajectory.header.domain_pack_id == domain_id)
        .collect::<Vec<_>>();
    trajectories.sort_by(|left, right| {
        left.start_time_unix_ms
            .cmp(&right.start_time_unix_ms)
            .then_with(|| left.trajectory_id.cmp(&right.trajectory_id))
    });
    let total = trajectories.len();
    let trajectories = trajectories
        .into_iter()
        .skip(args.offset)
        .take(args.limit)
        .collect::<Vec<_>>();
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_list_trajectories",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": [CF_DJ_TRAJECTORIES],
            "counts": counts,
            "queried": {"domain": args.domain},
            "limit": args.limit,
            "offset": args.offset,
            "decoded_records": {
                "total_for_domain": total,
                "trajectories": trajectories,
            },
        }),
        exit_code: 0,
    })
}

fn compile_dataset_command(args: CompileDatasetArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_TRAJECTORIES,
        CF_DJ_LATENT_PANELS,
        CF_DJ_ACTIONS,
        CF_DJ_OUTCOMES,
        CF_DJ_DATASET_SHARDS,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "compile-dataset.db")?;
    let before_counts = snapshot_dj_counts(db)?;

    let result = (|| -> DynamicJepaResult<Value> {
        if args.policy != "one_step" {
            return Err(DynamicJepaError::validation(
                "compile-dataset.policy",
                format!("unsupported dataset policy {:?}", args.policy),
                "Phase 6 supports exactly one_step",
            ));
        }
        if !matches!(args.split.as_str(), "train" | "val" | "test") {
            return Err(DynamicJepaError::validation(
                "compile-dataset.split",
                format!("unsupported split {:?}", args.split),
                "use train, val, or test",
            ));
        }

        let domain_id = DomainPackId::new(args.domain.clone())?;
        let domain_pack = get_domain_pack(db, &domain_id, "1.0.0")?.ok_or_else(|| {
            DynamicJepaError::DomainPackNotFound {
                id: domain_id.to_string(),
            }
        })?;
        if domain_pack.dataset_policy.negative_sampling != "same_domain_different_outcome" {
            return Err(DynamicJepaError::validation(
                "compile-dataset.negative_sampling",
                format!(
                    "unsupported negative sampling policy {:?}",
                    domain_pack.dataset_policy.negative_sampling
                ),
                "Phase 6 supports same_domain_different_outcome",
            ));
        }

        let existing_shards = list_dataset_shards(db, 1_000_000, 0)?
            .into_iter()
            .filter(|shard| shard.header.domain_pack_id == domain_pack.id)
            .collect::<Vec<_>>();
        if existing_shards
            .iter()
            .any(|shard| shard.split_name == args.split)
        {
            return Err(DynamicJepaError::validation(
                "compile-dataset.split",
                format!(
                    "domain {} already has a dataset shard for split {}",
                    domain_pack.id, args.split
                ),
                "wipe the pre-production demo DB before recompiling dataset shards; Phase 6 does not delete or merge existing source-of-truth rows",
            ));
        }
        let dataset_id = existing_shards
            .first()
            .map(|shard| shard.dataset_id)
            .unwrap_or_else(DatasetId::new_v4);

        let trajectories = list_trajectories(db, 1_000_000, 0)?
            .into_iter()
            .filter(|trajectory| trajectory.header.domain_pack_id == domain_pack.id)
            .collect::<Vec<_>>();
        if trajectories.is_empty() {
            return Err(DynamicJepaError::validation(
                "compile-dataset.trajectories",
                format!("domain {} has no trajectories to compile", domain_pack.id),
                "run compile-trajectories successfully before compiling a dataset shard",
            ));
        }

        let all_rows = collect_dataset_rows(db, &domain_pack, &trajectories)?;
        if all_rows.is_empty() {
            return Err(DynamicJepaError::validation(
                "compile-dataset.rows",
                format!(
                    "domain {} trajectories produced zero one-step rows",
                    domain_pack.id
                ),
                "compile trajectories with at least two panels each before compiling one_step datasets",
            ));
        }
        let split_overlap_count = split_overlap_count(&domain_pack, &all_rows)?;
        let split_counts = split_row_counts(&all_rows);
        let selected_rows = all_rows
            .iter()
            .filter(|row| row.split_name == args.split)
            .cloned()
            .collect::<Vec<_>>();
        if selected_rows.is_empty() {
            return Err(DynamicJepaError::validation(
                "compile-dataset.split",
                format!("split bucket {} received 0 rows", args.split),
                "use a fixture/corpus with enough trajectories for the requested deterministic split",
            ));
        }

        let domain_panels = list_latent_panels(db, 1_000_000, 0)?
            .into_iter()
            .filter(|panel| panel.header.domain_pack_id == domain_pack.id)
            .collect::<Vec<_>>();
        let negative_pool = build_negative_pool(db, domain_panels)?;
        let compiled = build_dataset_record(
            &domain_pack,
            &args.split,
            &selected_rows,
            &negative_pool,
            &split_counts,
            split_overlap_count,
            dataset_id,
        )?;
        verify_dataset_shard_sources(db, &compiled)?;

        let audit = build_audit(
            "compile_dataset_shard",
            AuditStatus::Ok,
            compiled
                .source_trajectory_ids
                .iter()
                .map(ToString::to_string)
                .collect(),
            vec![
                compiled.dataset_id.to_string(),
                compiled.shard_id.to_string(),
            ],
            vec![
                CF_DJ_DATASET_SHARDS.to_string(),
                CF_DJ_AUDIT_LOG.to_string(),
            ],
            vec![compiled.header.content_hash],
        )?;
        put_dataset_shard_with_audit_batch(db, &compiled, &audit)?;
        let stored =
            get_dataset_shard(db, compiled.dataset_id, compiled.shard_id)?.ok_or_else(|| {
                DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_DATASET_SHARDS.to_string(),
                    key: context_graph_storage::dynamicjepa::keys::dataset_shard_key(
                        compiled.dataset_id,
                        compiled.shard_id,
                    )
                    .to_vec(),
                }
            })?;
        if stored != compiled {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "dataset shard readback mismatch for shard {}",
                    stored.shard_id
                ),
            });
        }
        verify_dataset_shard_sources(db, &stored)?;
        let first_row = dataset_row_snapshot(db, &stored, 0)?;

        Ok(json!({
            "created_ids": {
                "dataset_id": stored.dataset_id.to_string(),
                "shard_id": stored.shard_id.to_string(),
                "audit_id": audit.audit_id.to_string(),
                "source_trajectory_ids": stored
                    .source_trajectory_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            },
            "summary": {
                "split": stored.split_name,
                "row_count": stored.row_count,
                "all_split_row_counts": split_counts,
                "input_dim": stored.shape_summary.input_dim,
                "target_dim": stored.shape_summary.target_dim,
                "action_dim": stored.shape_summary.action_dim,
            },
            "decoded_records": {
                "dataset_shard": stored,
                "row_0": first_row,
                "dataset_shards_cf": inspect_cf(db, CF_DJ_DATASET_SHARDS, 20, 0)?,
                "audit_log": inspect_cf(db, CF_DJ_AUDIT_LOG, 160, 0)?,
            }
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_compile_dataset",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "summary": value["summary"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => Ok(CommandOutcome {
            value: error_value(
                "dynamicjepa_compile_dataset",
                Some(db_path),
                &source_cfs,
                Some(before_counts),
                Some(after_counts),
                &err,
            ),
            exit_code: 1,
        }),
    }
}

fn get_dataset_shard_command(args: GetDatasetShardArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-dataset-shard.db")?;
    let counts = snapshot_dj_counts(db)?;
    let dataset_id = parse_dataset_id(&args.dataset_id, "get-dataset-shard.dataset_id")?;
    let shard_id = parse_dataset_shard_id(&args.shard_id, "get-dataset-shard.shard_id")?;
    let record = get_dataset_shard(db, dataset_id, shard_id)?;
    match record {
        Some(record) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_dataset_shard",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_DATASET_SHARDS],
                "counts": counts,
                "decoded_records": {
                    "dataset_shard": record,
                },
            }),
            exit_code: 0,
        }),
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_dataset_shard",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_DATASET_SHARDS],
                "counts": counts,
                "queried": {
                    "dataset_id": args.dataset_id,
                    "shard_id": args.shard_id,
                },
                "remediation": "compile a dataset shard before reading it",
            }),
            exit_code: 0,
        }),
    }
}

fn inspect_dataset_command(args: InspectDatasetArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "inspect-dataset.db")?;
    let counts = snapshot_dj_counts(db)?;
    let dataset_id = parse_dataset_id(&args.dataset_id, "inspect-dataset.dataset_id")?;
    let shard_id = parse_dataset_shard_id(&args.shard_id, "inspect-dataset.shard_id")?;
    let record = get_dataset_shard(db, dataset_id, shard_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_DATASET_SHARDS.to_string(),
            key: context_graph_storage::dynamicjepa::keys::dataset_shard_key(dataset_id, shard_id)
                .to_vec(),
        }
    })?;
    let row = dataset_row_snapshot(db, &record, args.row)?;
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_inspect_dataset",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": [
                CF_DJ_DATASET_SHARDS,
                CF_DJ_LATENT_PANELS,
                CF_DJ_ACTIONS,
                CF_DJ_NORMALIZED_STATES,
                CF_DJ_OUTCOMES
            ],
            "counts": counts,
            "queried": {
                "dataset_id": args.dataset_id,
                "shard_id": args.shard_id,
                "row": args.row,
            },
            "decoded_records": {
                "dataset_shard": record,
                "row": row,
            },
        }),
        exit_code: 0,
    })
}

fn train_command(args: TrainArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_DATASET_SHARDS,
        CF_DJ_LATENT_PANELS,
        CF_DJ_ACTIONS,
        CF_DJ_TRAINING_RUNS,
        CF_DJ_MODEL_ARTIFACTS,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "train.db")?;
    let config_path = canonical_existing_path(&args.config, "train.config")?;
    let before_counts = snapshot_dj_counts(db)?;
    let dataset_id = parse_dataset_id(&args.dataset_id, "train.dataset_id")?;

    let result = (|| -> DynamicJepaResult<Value> {
        let config = TrainConfig::from_path(&args.config)?;
        let outcome = train_dataset(db, dataset_id, &config, &args.artifact_root)?;
        let training_run = get_training_run(db, outcome.training_run_id)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_TRAINING_RUNS.to_string(),
                key: outcome.training_run_id.into_bytes().to_vec(),
            }
        })?;
        let artifact = get_model_artifact(db, outcome.artifact_id)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_MODEL_ARTIFACTS.to_string(),
                key: outcome.artifact_id.into_bytes().to_vec(),
            }
        })?;
        Ok(json!({
            "created_ids": {
                "training_run_id": outcome.training_run_id.to_string(),
                "artifact_id": outcome.artifact_id.to_string(),
            },
            "summary": {
                "artifact_root": outcome.artifact_root,
                "row_count_by_split": outcome.row_count_by_split,
                "metrics": outcome.metrics,
            },
            "decoded_records": {
                "training_run": training_run,
                "artifact": artifact,
                "training_runs_cf": inspect_cf(db, CF_DJ_TRAINING_RUNS, 20, 0)?,
                "model_artifacts_cf": inspect_cf(db, CF_DJ_MODEL_ARTIFACTS, 20, 0)?,
                "audit_log": inspect_cf(db, CF_DJ_AUDIT_LOG, 200, 0)?,
            },
            "artifact_hashes": outcome.artifact_hashes,
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_train",
                "status": "ok",
                "db_path": db_path,
                "config_path": config_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "summary": value["summary"].clone(),
                "decoded_records": value["decoded_records"].clone(),
                "artifact_hashes": value["artifact_hashes"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => Ok(CommandOutcome {
            value: error_value(
                "dynamicjepa_train",
                Some(db_path),
                &source_cfs,
                Some(before_counts),
                Some(after_counts),
                &err,
            ),
            exit_code: 1,
        }),
    }
}

fn get_training_run_command(args: GetTrainingRunArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-training-run.db")?;
    let counts = snapshot_dj_counts(db)?;
    let id = parse_training_run_id(&args.id, "get-training-run.id")?;
    let record = get_training_run(db, id)?;
    match record {
        Some(record) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_training_run",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_TRAINING_RUNS],
                "counts": counts,
                "decoded_records": {
                    "training_run": record,
                },
            }),
            exit_code: 0,
        }),
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_training_run",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_TRAINING_RUNS],
                "counts": counts,
                "queried": {"training_run_id": args.id},
                "remediation": "run dynamicjepa train before reading a training run",
            }),
            exit_code: 0,
        }),
    }
}

fn get_artifact_command(args: GetArtifactArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-artifact.db")?;
    let counts = snapshot_dj_counts(db)?;
    let id = parse_model_artifact_id(&args.id, "get-artifact.id")?;
    let record = get_model_artifact(db, id)?;
    match record {
        Some(record) => {
            let artifact_hashes = if args.verify_files {
                let checks = verify_artifact_files(&record)?;
                if let Some(mismatch) = checks.iter().find(|check| !check.equal) {
                    return Err(DynamicJepaError::ArtifactHashMismatch {
                        artifact_id: record.artifact_id.0,
                        file: mismatch.relative_path.clone(),
                        expected: mismatch.registry_sha256.clone(),
                        actual: mismatch.recomputed_sha256.clone(),
                    });
                }
                json!(checks)
            } else {
                json!({
                    "verified": false,
                    "warning": "files not verified - pass --verify-files for FSV evidence",
                })
            };
            Ok(CommandOutcome {
                value: json!({
                    "operation": "dynamicjepa_get_artifact",
                    "status": "ok",
                    "db_path": db_path,
                    "source_of_truth": [CF_DJ_MODEL_ARTIFACTS],
                    "counts": counts,
                    "decoded_records": {
                        "artifact": record,
                    },
                    "artifact_hashes": artifact_hashes,
                }),
                exit_code: 0,
            })
        }
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_artifact",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_MODEL_ARTIFACTS],
                "counts": counts,
                "queried": {"artifact_id": args.id},
                "remediation": "run dynamicjepa train before reading a model artifact",
            }),
            exit_code: 0,
        }),
    }
}

fn predict_command(args: PredictArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_MODEL_ARTIFACTS,
        CF_DJ_LATENT_PANELS,
        CF_DJ_ACTIONS,
        CF_DJ_PREDICTIONS,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "predict.db")?;
    let before_counts = snapshot_dj_counts(db)?;
    let artifact_id = parse_model_artifact_id(&args.artifact_id, "predict.artifact_id")?;
    let panel_id = parse_panel_id(&args.panel_id, "predict.panel_id")?;
    let action_id = parse_action_id(&args.action_id, "predict.action_id")?;

    let result = (|| -> DynamicJepaResult<Value> {
        let outcome = predict_next_panel(db, artifact_id, panel_id, action_id)?;
        Ok(json!({
            "created_ids": {
                "prediction_id": outcome.prediction_id.to_string(),
            },
            "summary": {
                "predicted_dim": outcome.prediction.predicted_next_panel_vec.len(),
                "uncertainty": outcome.prediction.uncertainty,
                "model_artifact_hash_at_inference": hex_bytes(&outcome.prediction.model_artifact_hash_at_inference),
            },
            "decoded_records": {
                "prediction": outcome.prediction,
                "artifact": outcome.artifact,
                "predictions_cf": inspect_cf(db, CF_DJ_PREDICTIONS, 20, 0)?,
                "audit_log": inspect_cf(db, CF_DJ_AUDIT_LOG, 200, 0)?,
            }
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_predict",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "summary": value["summary"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => Ok(CommandOutcome {
            value: error_value(
                "dynamicjepa_predict",
                Some(db_path),
                &source_cfs,
                Some(before_counts),
                Some(after_counts),
                &err,
            ),
            exit_code: 1,
        }),
    }
}

fn plan_command(args: PlanArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_MODEL_ARTIFACTS,
        CF_DJ_LATENT_PANELS,
        CF_DJ_ACTIONS,
        CF_DJ_PREDICTIONS,
        CF_DJ_SKILL_POLICIES,
        CF_DJ_GUARD_DECISIONS,
        CF_DJ_PLAN_TRACES,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "plan.db")?;
    let before_counts = snapshot_dj_counts(db)?;
    let artifact_id = parse_model_artifact_id(&args.artifact_id, "plan.artifact_id")?;
    let panel_id = parse_panel_id(&args.panel_id, "plan.panel_id")?;
    let skill_id = parse_skill_id(&args.skill_id, "plan.skill_id")?;

    let result = (|| -> DynamicJepaResult<Value> {
        let outcome = plan_next_action(db, artifact_id, panel_id, skill_id)?;
        Ok(json!({
            "created_ids": {
                "plan_trace_id": outcome.plan_trace_id.to_string(),
                "candidate_action_ids": outcome.candidate_actions.iter().map(|action| action.action_id.to_string()).collect::<Vec<_>>(),
                "prediction_ids": outcome.predictions.iter().map(|prediction| prediction.prediction_id.to_string()).collect::<Vec<_>>(),
                "guard_decision_ids": outcome.guard_decisions.iter().map(|guard| guard.guard_decision_id.to_string()).collect::<Vec<_>>(),
                "selected_action_id": outcome.plan_trace.selected_action_id.map(|id| id.to_string()),
                "skill_id": outcome.skill_policy.skill_id.to_string(),
            },
            "summary": {
                "candidate_count": outcome.candidate_actions.len(),
                "prediction_count": outcome.predictions.len(),
                "guard_decision_count": outcome.guard_decisions.len(),
                "status": outcome.plan_trace.status,
                "utility_scores": outcome.plan_trace.utility_scores,
            },
            "decoded_records": {
                "plan_trace": outcome.plan_trace,
                "skill_policy": outcome.skill_policy,
                "candidate_actions": outcome.candidate_actions,
                "predictions": outcome.predictions,
                "guard_decisions": outcome.guard_decisions,
                "actions_cf": inspect_cf(db, CF_DJ_ACTIONS, 100, 0)?,
                "predictions_cf": inspect_cf(db, CF_DJ_PREDICTIONS, 100, 0)?,
                "guard_decisions_cf": inspect_cf(db, CF_DJ_GUARD_DECISIONS, 100, 0)?,
                "plan_traces_cf": inspect_cf(db, CF_DJ_PLAN_TRACES, 20, 0)?,
                "audit_log": inspect_cf(db, CF_DJ_AUDIT_LOG, 300, 0)?,
            }
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_plan",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "summary": value["summary"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => Ok(CommandOutcome {
            value: error_value(
                "dynamicjepa_plan",
                Some(db_path),
                &source_cfs,
                Some(before_counts),
                Some(after_counts),
                &err,
            ),
            exit_code: 1,
        }),
    }
}

fn record_surprise_command(args: RecordSurpriseArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_MODEL_ARTIFACTS,
        CF_DJ_PREDICTIONS,
        CF_DJ_OUTCOMES,
        CF_DJ_LATENT_PANELS,
        CF_DJ_SURPRISE_EVENTS,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "record-surprise.db")?;
    let before_counts = snapshot_dj_counts(db)?;
    let prediction_id = parse_prediction_id(&args.prediction_id, "record-surprise.prediction_id")?;
    let observed_outcome_id = parse_outcome_id(
        &args.observed_outcome_id,
        "record-surprise.observed_outcome_id",
    )?;
    let observed_panel_id =
        parse_panel_id(&args.observed_panel_id, "record-surprise.observed_panel_id")?;

    let result = (|| -> DynamicJepaResult<Value> {
        let outcome = record_surprise(db, prediction_id, observed_outcome_id, observed_panel_id)?;
        Ok(json!({
            "created_ids": {
                "surprise_event_id": outcome.surprise_event_id.map(|id| id.to_string()),
            },
            "summary": {
                "wrote_surprise": outcome.wrote_surprise,
                "cosine": outcome.cosine,
                "threshold": outcome.threshold,
                "error_norm": outcome.error_norm,
            },
            "decoded_records": {
                "surprise_event": outcome.surprise_event,
                "surprise_events_cf": inspect_cf(db, CF_DJ_SURPRISE_EVENTS, 20, 0)?,
                "audit_log": inspect_cf(db, CF_DJ_AUDIT_LOG, 300, 0)?,
            }
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_record_surprise",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "summary": value["summary"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => Ok(CommandOutcome {
            value: error_value(
                "dynamicjepa_record_surprise",
                Some(db_path),
                &source_cfs,
                Some(before_counts),
                Some(after_counts),
                &err,
            ),
            exit_code: 1,
        }),
    }
}

fn get_prediction_command(args: GetPredictionArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-prediction.db")?;
    let counts = snapshot_dj_counts(db)?;
    let id = parse_prediction_id(&args.id, "get-prediction.id")?;
    match get_prediction(db, id)? {
        Some(record) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_prediction",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_PREDICTIONS],
                "counts": counts,
                "decoded_records": {"prediction": record},
            }),
            exit_code: 0,
        }),
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_prediction",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_PREDICTIONS],
                "counts": counts,
                "queried": {"prediction_id": args.id},
                "remediation": "run dynamicjepa predict or plan before reading a prediction",
            }),
            exit_code: 0,
        }),
    }
}

fn get_plan_trace_command(args: GetPlanTraceArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-plan-trace.db")?;
    let counts = snapshot_dj_counts(db)?;
    let id = parse_plan_trace_id(&args.id, "get-plan-trace.id")?;
    match get_plan_trace(db, id)? {
        Some(record) => {
            let mut decoded = Map::new();
            decoded.insert("plan_trace".to_string(), json!(record));
            if args.include_predictions {
                let records = record
                    .prediction_ids
                    .iter()
                    .map(|prediction_id| {
                        get_prediction(db, *prediction_id)?.ok_or_else(|| {
                            DynamicJepaError::SourceOfTruthMissing {
                                cf: CF_DJ_PREDICTIONS.to_string(),
                                key: prediction_id.into_bytes().to_vec(),
                            }
                        })
                    })
                    .collect::<DynamicJepaResult<Vec<_>>>()?;
                decoded.insert("predictions".to_string(), json!(records));
            }
            if args.include_guards {
                let records = record
                    .guard_decision_ids
                    .iter()
                    .map(|guard_id| {
                        get_guard_decision(
                            db,
                            context_graph_core::dynamicjepa::GuardDecisionId(*guard_id),
                        )?
                        .ok_or_else(|| {
                            DynamicJepaError::SourceOfTruthMissing {
                                cf: CF_DJ_GUARD_DECISIONS.to_string(),
                                key: guard_id.as_bytes().to_vec(),
                            }
                        })
                    })
                    .collect::<DynamicJepaResult<Vec<_>>>()?;
                decoded.insert("guard_decisions".to_string(), json!(records));
            }
            Ok(CommandOutcome {
                value: json!({
                    "operation": "dynamicjepa_get_plan_trace",
                    "status": "ok",
                    "db_path": db_path,
                    "source_of_truth": [CF_DJ_PLAN_TRACES, CF_DJ_PREDICTIONS, CF_DJ_GUARD_DECISIONS],
                    "counts": counts,
                    "decoded_records": decoded,
                }),
                exit_code: 0,
            })
        }
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_plan_trace",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_PLAN_TRACES],
                "counts": counts,
                "queried": {"plan_trace_id": args.id},
                "remediation": "run dynamicjepa plan before reading a plan trace",
            }),
            exit_code: 0,
        }),
    }
}

fn get_surprise_command(args: GetSurpriseArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-surprise.db")?;
    let counts = snapshot_dj_counts(db)?;
    let id = parse_surprise_event_id(&args.id, "get-surprise.id")?;
    match get_surprise_event(db, id)? {
        Some(record) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_surprise",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_SURPRISE_EVENTS],
                "counts": counts,
                "decoded_records": {"surprise_event": record},
            }),
            exit_code: 0,
        }),
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_surprise",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_SURPRISE_EVENTS],
                "counts": counts,
                "queried": {"surprise_event_id": args.id},
                "remediation": "run dynamicjepa record-surprise before reading a surprise event",
            }),
            exit_code: 0,
        }),
    }
}

fn verify_counter_world_command(args: VerifyDomainArgs) -> DynamicJepaResult<CommandOutcome> {
    verify_domain_command(VerificationDomain::CounterWorld, args)
}

fn verify_gridworld_command(args: VerifyDomainArgs) -> DynamicJepaResult<CommandOutcome> {
    verify_domain_command(VerificationDomain::Gridworld, args)
}

#[derive(Debug, Clone, Copy)]
enum VerificationDomain {
    CounterWorld,
    Gridworld,
}

impl VerificationDomain {
    fn id(self) -> &'static str {
        match self {
            Self::CounterWorld => "counter_world",
            Self::Gridworld => "gridworld_5x5",
        }
    }

    fn operation(self) -> &'static str {
        match self {
            Self::CounterWorld => "dynamicjepa_verify_counter_world",
            Self::Gridworld => "dynamicjepa_verify_gridworld",
        }
    }

    fn test_name(self) -> &'static str {
        match self {
            Self::CounterWorld => "phase10_verify_counter_world",
            Self::Gridworld => "phase10_verify_gridworld",
        }
    }

    fn default_evidence_root(self) -> PathBuf {
        PathBuf::from("tmp/5090jepa_evidence").join(self.id())
    }

    fn domain_pack_path(self, fixtures_root: &Path) -> PathBuf {
        fixtures_root
            .join("domain_packs")
            .join(format!("{}.v1.toml", self.id()))
    }

    fn train_config_path(self, fixtures_root: &Path) -> PathBuf {
        match self {
            Self::CounterWorld => fixtures_root.join("verification/tiny_train_config.json"),
            Self::Gridworld => fixtures_root.join("verification/gridworld_5x5_train_config.json"),
        }
    }

    fn corpus_path(self, fixtures_root: &Path, evidence_dir: &Path) -> DynamicJepaResult<PathBuf> {
        match self {
            Self::CounterWorld => write_generated_counter_verification_fixture(evidence_dir),
            Self::Gridworld => Ok(fixtures_root.join("verification/gridworld_5x5_corpus.jsonl")),
        }
    }

    fn edge_fixtures(self, fixtures_root: &Path) -> [(&'static str, PathBuf, EdgeExpectation); 3] {
        match self {
            Self::CounterWorld => [
                (
                    "empty_input",
                    fixtures_root.join("verification/counter_world_edge_empty_input.jsonl"),
                    EdgeExpectation::empty_input(),
                ),
                (
                    "invalid_action",
                    fixtures_root.join("verification/counter_world_edge_invalid_action.jsonl"),
                    EdgeExpectation::failed_adapter(),
                ),
                (
                    "max_panel",
                    fixtures_root.join("verification/counter_world_edge_max_panel.jsonl"),
                    EdgeExpectation::failed_adapter(),
                ),
            ],
            Self::Gridworld => [
                (
                    "empty_input",
                    fixtures_root.join("verification/gridworld_5x5_edge_empty.jsonl"),
                    EdgeExpectation::empty_input(),
                ),
                (
                    "invalid_action",
                    fixtures_root.join("verification/gridworld_5x5_edge_invalid_action.jsonl"),
                    EdgeExpectation::failed_adapter(),
                ),
                (
                    "oob_state",
                    fixtures_root.join("verification/gridworld_5x5_edge_oob_state.jsonl"),
                    EdgeExpectation::failed_adapter(),
                ),
            ],
        }
    }

    fn expected_happy_counts(self, surprise_count: u64) -> BTreeMap<String, u64> {
        let mut counts = BTreeMap::from([
            (CF_DJ_DOMAIN_PACKS.to_string(), 1),
            (CF_DJ_DOMAIN_PACK_BY_NAME_VERSION.to_string(), 1),
            (CF_DJ_ADAPTER_REGISTRY.to_string(), 1),
            (CF_DJ_DATASET_SHARDS.to_string(), 3),
            (CF_DJ_MODEL_ARTIFACTS.to_string(), 1),
            (CF_DJ_PLAN_TRACES.to_string(), 1),
            (CF_DJ_PREDICTIONS.to_string(), 6),
            (CF_DJ_SKILL_POLICIES.to_string(), 1),
            (CF_DJ_GUARD_DECISIONS.to_string(), 5),
            (CF_DJ_SURPRISE_EVENTS.to_string(), surprise_count),
            (CF_DJ_TRAINING_RUNS.to_string(), 1),
            (CF_DJ_VERIFICATION_RUNS.to_string(), 1),
        ]);
        match self {
            Self::CounterWorld => {
                counts.insert(CF_DJ_INSTRUMENT_REGISTRY.to_string(), 2);
                counts.insert(CF_DJ_RAW_EVENTS.to_string(), 62);
                counts.insert(CF_DJ_ADAPTER_RUNS.to_string(), 62);
                counts.insert(CF_DJ_NORMALIZED_STATES.to_string(), 60);
                counts.insert(CF_DJ_ACTIONS.to_string(), 65);
                counts.insert(CF_DJ_OUTCOMES.to_string(), 60);
                counts.insert(CF_DJ_TRANSITIONS.to_string(), 60);
                counts.insert(CF_DJ_INSTRUMENT_READINGS.to_string(), 120);
                counts.insert(CF_DJ_LATENT_PANELS.to_string(), 60);
                counts.insert(CF_DJ_TRAJECTORIES.to_string(), 1);
            }
            Self::Gridworld => {
                counts.insert(CF_DJ_INSTRUMENT_REGISTRY.to_string(), 3);
                counts.insert(CF_DJ_RAW_EVENTS.to_string(), 8002);
                counts.insert(CF_DJ_ADAPTER_RUNS.to_string(), 8002);
                counts.insert(CF_DJ_NORMALIZED_STATES.to_string(), 8000);
                counts.insert(CF_DJ_ACTIONS.to_string(), 8005);
                counts.insert(CF_DJ_OUTCOMES.to_string(), 8000);
                counts.insert(CF_DJ_TRANSITIONS.to_string(), 8000);
                counts.insert(CF_DJ_INSTRUMENT_READINGS.to_string(), 24000);
                counts.insert(CF_DJ_LATENT_PANELS.to_string(), 8000);
                counts.insert(CF_DJ_TRAJECTORIES.to_string(), 1000);
            }
        }
        counts
    }
}

#[derive(Debug, Clone, Copy)]
struct EdgeExpectation {
    raw_delta: i64,
    adapter_delta: i64,
    normalized_delta: i64,
    transition_delta: i64,
    audit_delta: i64,
}

impl EdgeExpectation {
    fn empty_input() -> Self {
        Self {
            raw_delta: 0,
            adapter_delta: 0,
            normalized_delta: 0,
            transition_delta: 0,
            audit_delta: 1,
        }
    }

    fn failed_adapter() -> Self {
        Self {
            raw_delta: 1,
            adapter_delta: 1,
            normalized_delta: 0,
            transition_delta: 0,
            audit_delta: 2,
        }
    }
}

struct VerificationContext {
    domain: VerificationDomain,
    db: PathBuf,
    fixtures_root: PathBuf,
    artifact_root: PathBuf,
    evidence_dir: PathBuf,
    verification_run_id: VerificationRunId,
    before_counts: BTreeMap<String, u64>,
    commands: Vec<String>,
    fixture_hashes: Vec<[u8; 32]>,
    artifact_hash_checks: Vec<ArtifactHashCheck>,
    decoded: BTreeMap<String, Value>,
    expected: BTreeMap<String, Value>,
    actual: BTreeMap<String, Value>,
    edges: Vec<Value>,
    step_summaries: Vec<Value>,
}

fn verify_domain_command(
    domain: VerificationDomain,
    args: VerifyDomainArgs,
) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let mut ctx = VerificationContext::new(domain, args)?;
    let result = run_phase10_domain_verification(&mut ctx);
    match result {
        Ok(summary) => ctx.finish(VerificationStatus::Passed, summary, 0),
        Err(err) => {
            let failure = err.to_string();
            let summary = json!({
                "status": "error",
                "error_code": err.code(),
                "error_message": failure,
                "evidence_dir": ctx.evidence_dir,
            });
            ctx.finish(
                VerificationStatus::Failed {
                    failure_details: failure,
                },
                summary,
                1,
            )
        }
    }
}

impl VerificationContext {
    fn new(domain: VerificationDomain, args: VerifyDomainArgs) -> DynamicJepaResult<Self> {
        let verification_run_id = VerificationRunId::new_v4();
        let evidence_root = args
            .evidence_root
            .unwrap_or_else(|| domain.default_evidence_root());
        let evidence_dir = evidence_root.join(verification_run_id.to_string());
        fs::create_dir_all(evidence_dir.join("decoded")).map_err(|err| {
            DynamicJepaError::Storage {
                operation: "verify.create_evidence_dir".to_string(),
                cf: "filesystem".to_string(),
                message: format!("failed to create {}: {err}", evidence_dir.display()),
                remediation: "use a writable --evidence-root".to_string(),
            }
        })?;
        let store = open_store(&args.db)?;
        let before_counts = snapshot_dj_counts(store.dynamicjepa_db())?;
        drop(store);
        Ok(Self {
            domain,
            db: args.db,
            fixtures_root: args.fixtures_root,
            artifact_root: args.artifact_root,
            evidence_dir,
            verification_run_id,
            before_counts,
            commands: Vec::new(),
            fixture_hashes: Vec::new(),
            artifact_hash_checks: Vec::new(),
            decoded: BTreeMap::new(),
            expected: BTreeMap::new(),
            actual: BTreeMap::new(),
            edges: Vec::new(),
            step_summaries: Vec::new(),
        })
    }

    fn record_command(&mut self, command: String, label: &str, value: &Value) {
        self.commands.push(command);
        self.step_summaries.push(json!({
            "label": label,
            "status": value.get("status").cloned().unwrap_or(Value::Null),
            "created_ids": value.get("created_ids").cloned().unwrap_or(Value::Null),
            "summary": value.get("summary").cloned().unwrap_or(Value::Null),
            "error_code": value.get("error_code").cloned().unwrap_or(Value::Null),
            "counts": value
                .get("counts")
                .or_else(|| value.get("after_counts"))
                .cloned()
                .unwrap_or(Value::Null),
        }));
    }

    fn add_fixture_hash(&mut self, path: &Path) -> DynamicJepaResult<()> {
        self.fixture_hashes.push(file_sha256(path)?);
        Ok(())
    }

    fn fail(&self, message: impl Into<String>) -> DynamicJepaError {
        DynamicJepaError::VerificationFailed {
            test_name: self.domain.test_name().to_string(),
            message: message.into(),
            evidence_path: self.evidence_dir.display().to_string(),
        }
    }

    fn finish(
        mut self,
        status: VerificationStatus,
        mut summary: Value,
        exit_code: i32,
    ) -> DynamicJepaResult<CommandOutcome> {
        let store = open_store(&self.db)?;
        let db = store.dynamicjepa_db();
        let pre_record_counts = snapshot_dj_counts(db)?;
        let mut after_counts = pre_record_counts.clone();
        *after_counts
            .entry(CF_DJ_VERIFICATION_RUNS.to_string())
            .or_default() += 1;
        *after_counts.entry(CF_DJ_AUDIT_LOG.to_string()).or_default() += 1;
        self.actual
            .insert("final_counts".to_string(), json!(after_counts.clone()));

        let mut record = VerificationRunRecord {
            header: DjRecordHeader::new(
                self.verification_run_id.0,
                context_graph_core::dynamicjepa::VERIFICATION_RUN_RECORD_VERSION,
                DomainPackId::new(self.domain.id().to_string())?,
                "1.0.0".to_string(),
                now_unix_ms()?,
                Some(self.verification_run_id.0),
            ),
            verification_run_id: self.verification_run_id,
            test_name: self.domain.test_name().to_string(),
            db_path_hash: sha256(self.db.display().to_string().as_bytes()),
            fixture_hashes: self.fixture_hashes.clone(),
            before_counts: self.before_counts.clone(),
            after_counts: after_counts.clone(),
            commands_executed: self.commands.clone(),
            artifact_hash_checks: self.artifact_hash_checks.clone(),
            decoded_record_excerpts: values_to_dj_json_map(self.decoded.clone())?,
            expected_results: values_to_dj_json_map(self.expected.clone())?,
            actual_results: values_to_dj_json_map(self.actual.clone())?,
            status,
            created_at_unix_ms: now_unix_ms()?,
        };
        record.refresh_content_hash()?;
        let audit_status = match &record.status {
            VerificationStatus::Passed => AuditStatus::Ok,
            VerificationStatus::Failed { .. } => AuditStatus::Failed {
                error_code: "VERIFICATION_FAILED".to_string(),
            },
        };
        let audit = build_audit(
            self.domain.operation(),
            audit_status,
            self.commands.clone(),
            vec![self.verification_run_id.to_string()],
            vec![
                CF_DJ_VERIFICATION_RUNS.to_string(),
                CF_DJ_AUDIT_LOG.to_string(),
            ],
            vec![record.header.content_hash],
        )?;
        put_verification_run_with_audit_batch(db, &record, &audit)?;
        let stored = get_verification_run(db, self.verification_run_id)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_VERIFICATION_RUNS.to_string(),
                key: self.verification_run_id.into_bytes().to_vec(),
            }
        })?;
        if stored != record {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "verification run readback mismatch for {}",
                    self.verification_run_id
                ),
            });
        }
        let final_counts = snapshot_dj_counts(db)?;
        if final_counts != after_counts {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "verification final counts mismatch expected={after_counts:?} actual={final_counts:?}"
                ),
            });
        }
        drop(store);

        write_json_file(
            &self.evidence_dir.join("verification_run_record.json"),
            &record,
        )?;
        write_json_file(
            &self.evidence_dir.join("before_counts.json"),
            &self.before_counts,
        )?;
        write_json_file(&self.evidence_dir.join("after_counts.json"), &final_counts)?;
        write_json_file(
            &self
                .evidence_dir
                .join("artifact_files_recomputed_hashes.json"),
            &self.artifact_hash_checks,
        )?;
        write_json_file(
            &self.evidence_dir.join("summary.json"),
            &self.step_summaries,
        )?;
        for (idx, edge) in self.edges.iter().enumerate() {
            write_json_file(&self.evidence_dir.join(format!("edge_{idx:02}.json")), edge)?;
        }
        for (name, value) in &self.decoded {
            write_json_file(
                &self
                    .evidence_dir
                    .join("decoded")
                    .join(format!("{name}.json")),
                value,
            )?;
        }
        fs::write(
            self.evidence_dir.join("command_log.txt"),
            self.commands.join("\n") + "\n",
        )
        .map_err(|err| DynamicJepaError::Storage {
            operation: "verify.write_command_log".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to write command_log.txt: {err}"),
            remediation: "use a writable --evidence-root".to_string(),
        })?;

        if let Some(object) = summary.as_object_mut() {
            object.insert(
                "verification_run_id".to_string(),
                json!(self.verification_run_id.to_string()),
            );
            object.insert("evidence_dir".to_string(), json!(self.evidence_dir));
            object.insert("after_counts".to_string(), json!(final_counts));
            object.insert(
                "source_of_truth".to_string(),
                json!([CF_DJ_VERIFICATION_RUNS, CF_DJ_AUDIT_LOG]),
            );
            object.insert(
                "decoded_records".to_string(),
                json!({
                    "verification_run": {
                        "verification_run_id": stored.verification_run_id.to_string(),
                        "test_name": stored.test_name,
                        "status": stored.status,
                        "before_counts": stored.before_counts,
                        "after_counts": stored.after_counts,
                        "artifact_hash_checks": stored.artifact_hash_checks,
                    }
                }),
            );
        }
        Ok(CommandOutcome {
            value: summary,
            exit_code,
        })
    }
}

fn run_phase10_domain_verification(ctx: &mut VerificationContext) -> DynamicJepaResult<Value> {
    let domain_path = ctx.domain.domain_pack_path(&ctx.fixtures_root);
    let corpus_path = ctx
        .domain
        .corpus_path(&ctx.fixtures_root, &ctx.evidence_dir)?;
    let train_config = ctx.domain.train_config_path(&ctx.fixtures_root);
    ctx.add_fixture_hash(&domain_path)?;
    ctx.add_fixture_hash(&corpus_path)?;
    ctx.add_fixture_hash(&train_config)?;
    for (_, path, _) in ctx.domain.edge_fixtures(&ctx.fixtures_root) {
        ctx.add_fixture_hash(&path)?;
    }

    let initial = run_ok_step(
        "initial_counts",
        format!(
            "context-graph dynamicjepa inspect-counts --db {} --json",
            ctx.db.display()
        ),
        inspect_counts(InspectCountsArgs {
            db: ctx.db.clone(),
            json: true,
        }),
        ctx,
    )?;
    ctx.decoded.insert(
        "initial_counts".to_string(),
        compact_command_output(&initial),
    );

    let registered = run_ok_step(
        "register_domain",
        format!(
            "context-graph dynamicjepa register-domain --db {} --file {} --json",
            ctx.db.display(),
            domain_path.display()
        ),
        register_domain(RegisterDomainArgs {
            db: ctx.db.clone(),
            file: domain_path.clone(),
            json: true,
        }),
        ctx,
    )?;
    ctx.decoded.insert(
        "registered_domain".to_string(),
        compact_command_output(&registered),
    );

    run_adapter_edge_cases(ctx)?;

    let ingest = run_ok_step(
        "ingest_corpus",
        format!(
            "context-graph dynamicjepa ingest-event --db {} --domain {} --adapter json_event --file {} --json",
            ctx.db.display(),
            ctx.domain.id(),
            corpus_path.display()
        ),
        ingest_event(IngestEventArgs {
            db: ctx.db.clone(),
            domain: ctx.domain.id().to_string(),
            adapter: "json_event".to_string(),
            file: corpus_path.clone(),
            json: true,
        }),
        ctx,
    )?;
    ctx.decoded
        .insert("corpus_ingest".to_string(), compact_command_output(&ingest));

    let panels = run_ok_step(
        "materialize_panels",
        format!(
            "context-graph dynamicjepa materialize-panel --db {} --all-pending --domain {} --json",
            ctx.db.display(),
            ctx.domain.id()
        ),
        materialize_panel_command(MaterializePanelArgs {
            db: ctx.db.clone(),
            transition_id: None,
            all_pending: true,
            domain: Some(ctx.domain.id().to_string()),
            json: true,
        }),
        ctx,
    )?;
    let expected_panel_count = match ctx.domain {
        VerificationDomain::CounterWorld => 60,
        VerificationDomain::Gridworld => 8000,
    };
    assert_json_u64(
        ctx,
        &panels["summary"]["materialized"],
        expected_panel_count,
        "materialized panel count",
    )?;

    let trajectories = run_ok_step(
        "compile_trajectories",
        format!(
            "context-graph dynamicjepa compile-trajectories --db {} --domain {} --policy by_domain_session --json",
            ctx.db.display(),
            ctx.domain.id()
        ),
        compile_trajectories_command(CompileTrajectoriesArgs {
            db: ctx.db.clone(),
            domain: ctx.domain.id().to_string(),
            policy: "by_domain_session".to_string(),
            json: true,
        }),
        ctx,
    )?;
    let expected_trajectory_count = match ctx.domain {
        VerificationDomain::CounterWorld => 1,
        VerificationDomain::Gridworld => 1000,
    };
    assert_json_u64(
        ctx,
        &trajectories["summary"]["trajectory_count"],
        expected_trajectory_count,
        "trajectory count",
    )?;
    ctx.decoded.insert(
        "compiled_trajectories".to_string(),
        compact_command_output(&trajectories),
    );

    let (dataset_id, train_shard_id, val_shard_id, split_counts) =
        compile_all_splits_for_verification(ctx)?;
    assert_split_counts(ctx, &split_counts)?;

    let train = run_ok_step(
        "train",
        format!(
            "context-graph dynamicjepa train --db {} --dataset-id {} --config {} --artifact-root {} --json",
            ctx.db.display(),
            dataset_id,
            train_config.display(),
            ctx.artifact_root.display()
        ),
        train_command(TrainArgs {
            db: ctx.db.clone(),
            dataset_id: dataset_id.clone(),
            config: train_config.clone(),
            artifact_root: ctx.artifact_root.clone(),
            json: true,
        }),
        ctx,
    )?;
    let artifact_id = required_str(&train, &["created_ids", "artifact_id"], ctx)?;
    let training_run_id = required_str(&train, &["created_ids", "training_run_id"], ctx)?;
    assert_training_metrics(ctx, &train)?;
    ctx.decoded.insert(
        "training_summary".to_string(),
        compact_training_summary(&train),
    );

    let artifact = run_ok_step(
        "get_artifact_verify_files",
        format!(
            "context-graph dynamicjepa get-artifact --db {} --id {} --verify-files --json",
            ctx.db.display(),
            artifact_id
        ),
        get_artifact_command(GetArtifactArgs {
            db: ctx.db.clone(),
            id: artifact_id.clone(),
            verify_files: true,
            json: true,
        }),
        ctx,
    )?;
    ctx.artifact_hash_checks = artifact_checks_from_json(ctx, &artifact)?;
    ctx.decoded
        .insert("artifact".to_string(), compact_command_output(&artifact));

    let training_readback = run_ok_step(
        "get_training_run",
        format!(
            "context-graph dynamicjepa get-training-run --db {} --id {} --json",
            ctx.db.display(),
            training_run_id
        ),
        get_training_run_command(GetTrainingRunArgs {
            db: ctx.db.clone(),
            id: training_run_id.clone(),
            json: true,
        }),
        ctx,
    )?;
    ctx.decoded.insert(
        "training_run".to_string(),
        compact_command_output(&training_readback),
    );

    let row0 = run_ok_step(
        "inspect_train_row0",
        format!(
            "context-graph dynamicjepa inspect-dataset --db {} --dataset-id {} --shard-id {} --row 0 --json",
            ctx.db.display(),
            dataset_id,
            train_shard_id
        ),
        inspect_dataset_command(InspectDatasetArgs {
            db: ctx.db.clone(),
            dataset_id: dataset_id.clone(),
            shard_id: train_shard_id.clone(),
            row: 0,
            json: true,
        }),
        ctx,
    )?;
    let input_panel_id = required_str(&row0, &["decoded_records", "row", "input_panel_id"], ctx)?;
    let action_id = required_str(&row0, &["decoded_records", "row", "input_action_id"], ctx)?;
    let target_panel_id = required_str(&row0, &["decoded_records", "row", "target_panel_id"], ctx)?;
    ctx.decoded.insert("dataset_row0".to_string(), row0);

    let target_panel = get_panel_for_verification(ctx, &target_panel_id, "target_panel")?;
    let target_outcome_id = required_str(
        &target_panel,
        &["decoded_records", "panel", "outcome_id"],
        ctx,
    )?;

    let predict = run_ok_step(
        "predict",
        format!(
            "context-graph dynamicjepa predict --db {} --artifact-id {} --panel-id {} --action-id {} --json",
            ctx.db.display(),
            artifact_id,
            input_panel_id,
            action_id
        ),
        predict_command(PredictArgs {
            db: ctx.db.clone(),
            artifact_id: artifact_id.clone(),
            panel_id: input_panel_id.clone(),
            action_id: action_id.clone(),
            json: true,
        }),
        ctx,
    )?;
    let prediction_id = required_str(&predict, &["created_ids", "prediction_id"], ctx)?;
    ctx.decoded
        .insert("prediction".to_string(), predict.clone());

    let plan = run_ok_step(
        "plan",
        format!(
            "context-graph dynamicjepa plan --db {} --artifact-id {} --panel-id {} --skill-id 10000000-0000-0000-0000-000000000010 --json",
            ctx.db.display(),
            artifact_id,
            input_panel_id
        ),
        plan_command(PlanArgs {
            db: ctx.db.clone(),
            artifact_id: artifact_id.clone(),
            panel_id: input_panel_id.clone(),
            skill_id: "10000000-0000-0000-0000-000000000010".to_string(),
            json: true,
        }),
        ctx,
    )?;
    assert_json_u64(
        ctx,
        &plan["summary"]["candidate_count"],
        5,
        "candidate count",
    )?;
    assert_json_u64(
        ctx,
        &plan["summary"]["prediction_count"],
        5,
        "plan prediction count",
    )?;
    assert_json_u64(
        ctx,
        &plan["summary"]["guard_decision_count"],
        5,
        "guard decision count",
    )?;
    let plan_trace_id = required_str(&plan, &["created_ids", "plan_trace_id"], ctx)?;
    ctx.decoded.insert("plan".to_string(), plan);

    let plan_readback = run_ok_step(
        "get_plan_trace",
        format!(
            "context-graph dynamicjepa get-plan-trace --db {} --id {} --include-predictions --include-guards --json",
            ctx.db.display(),
            plan_trace_id
        ),
        get_plan_trace_command(GetPlanTraceArgs {
            db: ctx.db.clone(),
            id: plan_trace_id.clone(),
            include_predictions: true,
            include_guards: true,
            json: true,
        }),
        ctx,
    )?;
    ctx.decoded
        .insert("plan_trace_readback".to_string(), plan_readback);

    let surprise_none = run_ok_step(
        "record_surprise_expected_none",
        format!(
            "context-graph dynamicjepa record-surprise --db {} --prediction-id {} --observed-outcome-id {} --observed-panel-id {} --json",
            ctx.db.display(),
            prediction_id,
            target_outcome_id,
            target_panel_id
        ),
        record_surprise_command(RecordSurpriseArgs {
            db: ctx.db.clone(),
            prediction_id: prediction_id.clone(),
            observed_outcome_id: target_outcome_id.clone(),
            observed_panel_id: target_panel_id.clone(),
            json: true,
        }),
        ctx,
    )?;
    if surprise_none["summary"]["wrote_surprise"].as_bool() != Some(false) {
        return Err(ctx.fail("record-surprise wrote a surprise row for the known target row"));
    }
    ctx.decoded
        .insert("record_surprise_expected_none".to_string(), surprise_none);

    let surprise_count = if matches!(ctx.domain, VerificationDomain::CounterWorld) {
        let val_row = run_ok_step(
            "inspect_val_row0",
            format!(
                "context-graph dynamicjepa inspect-dataset --db {} --dataset-id {} --shard-id {} --row 0 --json",
                ctx.db.display(),
                dataset_id,
                val_shard_id
            ),
            inspect_dataset_command(InspectDatasetArgs {
                db: ctx.db.clone(),
                dataset_id: dataset_id.clone(),
                shard_id: val_shard_id.clone(),
                row: 0,
                json: true,
            }),
            ctx,
        )?;
        let far_panel_id = required_str(
            &val_row,
            &["decoded_records", "row", "target_panel_id"],
            ctx,
        )?;
        let far_panel = get_panel_for_verification(ctx, &far_panel_id, "forced_surprise_panel")?;
        let far_outcome_id =
            required_str(&far_panel, &["decoded_records", "panel", "outcome_id"], ctx)?;
        let surprise = run_ok_step(
            "record_surprise_forced",
            format!(
                "context-graph dynamicjepa record-surprise --db {} --prediction-id {} --observed-outcome-id {} --observed-panel-id {} --json",
                ctx.db.display(),
                prediction_id,
                far_outcome_id,
                far_panel_id
            ),
            record_surprise_command(RecordSurpriseArgs {
                db: ctx.db.clone(),
                prediction_id: prediction_id.clone(),
                observed_outcome_id: far_outcome_id.clone(),
                observed_panel_id: far_panel_id.clone(),
                json: true,
            }),
            ctx,
        )?;
        if surprise["summary"]["wrote_surprise"].as_bool() != Some(true) {
            return Err(ctx.fail("counter_world forced surprise did not write a surprise row"));
        }
        let surprise_id = required_str(&surprise, &["created_ids", "surprise_event_id"], ctx)?;
        ctx.decoded
            .insert("record_surprise_forced".to_string(), surprise);
        let surprise_readback = run_ok_step(
            "get_surprise",
            format!(
                "context-graph dynamicjepa get-surprise --db {} --id {} --json",
                ctx.db.display(),
                surprise_id
            ),
            get_surprise_command(GetSurpriseArgs {
                db: ctx.db.clone(),
                id: surprise_id.clone(),
                json: true,
            }),
            ctx,
        )?;
        ctx.decoded
            .insert("surprise".to_string(), surprise_readback);
        1
    } else {
        0
    };

    let pre_verification_counts = run_ok_step(
        "pre_verification_counts",
        format!(
            "context-graph dynamicjepa inspect-counts --db {} --json",
            ctx.db.display()
        ),
        inspect_counts(InspectCountsArgs {
            db: ctx.db.clone(),
            json: true,
        }),
        ctx,
    )?;
    let mut expected_counts = ctx.domain.expected_happy_counts(surprise_count);
    expected_counts.insert(
        CF_DJ_AUDIT_LOG.to_string(),
        pre_verification_counts["counts"][CF_DJ_AUDIT_LOG]
            .as_u64()
            .unwrap_or(0)
            + 1,
    );
    assert_counts_before_verification(ctx, &pre_verification_counts, &expected_counts)?;
    ctx.expected
        .insert("final_counts".to_string(), json!(expected_counts));
    ctx.expected.insert(
        "split_counts".to_string(),
        json!(expected_split_counts(ctx.domain)),
    );
    ctx.actual
        .insert("split_counts".to_string(), json!(split_counts));

    Ok(json!({
        "operation": ctx.domain.operation(),
        "status": "ok",
        "test_name": ctx.domain.test_name(),
        "db_path": ctx.db,
        "artifact_root": ctx.artifact_root,
        "evidence_dir": ctx.evidence_dir,
        "summary": {
            "domain": ctx.domain.id(),
            "dataset_id": dataset_id,
            "training_run_id": training_run_id,
            "artifact_id": artifact_id,
            "prediction_id": prediction_id,
            "plan_trace_id": plan_trace_id,
            "surprise_count": surprise_count,
            "split_counts": split_counts,
        }
    }))
}

fn run_adapter_edge_cases(ctx: &mut VerificationContext) -> DynamicJepaResult<()> {
    for (name, path, expectation) in ctx.domain.edge_fixtures(&ctx.fixtures_root) {
        let before = inspect_counts(InspectCountsArgs {
            db: ctx.db.clone(),
            json: true,
        })?
        .value;
        let command = format!(
            "context-graph dynamicjepa ingest-event --db {} --domain {} --adapter json_event --file {} --json",
            ctx.db.display(),
            ctx.domain.id(),
            path.display()
        );
        let outcome = ingest_event(IngestEventArgs {
            db: ctx.db.clone(),
            domain: ctx.domain.id().to_string(),
            adapter: "json_event".to_string(),
            file: path.clone(),
            json: true,
        })?;
        ctx.record_command(command, &format!("edge_{name}"), &outcome.value);
        if outcome.exit_code == 0 {
            return Err(ctx.fail(format!(
                "edge case {name} succeeded but failure was expected"
            )));
        }
        if outcome.value["error_code"].as_str() != Some("ADAPTER_FAILED") {
            return Err(ctx.fail(format!(
                "edge case {name} returned error_code {:?}, expected ADAPTER_FAILED",
                outcome.value["error_code"].as_str()
            )));
        }
        let after = inspect_counts(InspectCountsArgs {
            db: ctx.db.clone(),
            json: true,
        })?
        .value;
        assert_delta(
            ctx,
            &before,
            &after,
            CF_DJ_RAW_EVENTS,
            expectation.raw_delta,
            name,
        )?;
        assert_delta(
            ctx,
            &before,
            &after,
            CF_DJ_ADAPTER_RUNS,
            expectation.adapter_delta,
            name,
        )?;
        assert_delta(
            ctx,
            &before,
            &after,
            CF_DJ_NORMALIZED_STATES,
            expectation.normalized_delta,
            name,
        )?;
        assert_delta(
            ctx,
            &before,
            &after,
            CF_DJ_TRANSITIONS,
            expectation.transition_delta,
            name,
        )?;
        assert_delta(
            ctx,
            &before,
            &after,
            CF_DJ_AUDIT_LOG,
            expectation.audit_delta,
            name,
        )?;
        ctx.edges.push(json!({
            "name": name,
            "fixture": path,
            "before_counts": before["counts"],
            "error": outcome.value,
            "after_counts": after["counts"],
            "expected_delta": {
                CF_DJ_RAW_EVENTS: expectation.raw_delta,
                CF_DJ_ADAPTER_RUNS: expectation.adapter_delta,
                CF_DJ_NORMALIZED_STATES: expectation.normalized_delta,
                CF_DJ_TRANSITIONS: expectation.transition_delta,
                CF_DJ_AUDIT_LOG: expectation.audit_delta,
            }
        }));
    }
    Ok(())
}

fn compile_all_splits_for_verification(
    ctx: &mut VerificationContext,
) -> DynamicJepaResult<(String, String, String, BTreeMap<String, u64>)> {
    let mut dataset_id = String::new();
    let mut train_shard_id = String::new();
    let mut val_shard_id = String::new();
    let mut split_counts = BTreeMap::new();
    for split in ["train", "val", "test"] {
        let output = run_ok_step(
            &format!("compile_dataset_{split}"),
            format!(
                "context-graph dynamicjepa compile-dataset --db {} --domain {} --policy one_step --split {} --json",
                ctx.db.display(),
                ctx.domain.id(),
                split
            ),
            compile_dataset_command(CompileDatasetArgs {
                db: ctx.db.clone(),
                domain: ctx.domain.id().to_string(),
                policy: "one_step".to_string(),
                split: split.to_string(),
                json: true,
            }),
            ctx,
        )?;
        let this_dataset_id = required_str(&output, &["created_ids", "dataset_id"], ctx)?;
        let shard_id = required_str(&output, &["created_ids", "shard_id"], ctx)?;
        if dataset_id.is_empty() {
            dataset_id = this_dataset_id;
        } else if dataset_id != this_dataset_id {
            return Err(ctx.fail(format!(
                "dataset id drift: expected {dataset_id}, got {this_dataset_id}"
            )));
        }
        if split == "train" {
            train_shard_id = shard_id;
        } else if split == "val" {
            val_shard_id = shard_id;
        }
        split_counts.insert(
            split.to_string(),
            output["summary"]["row_count"]
                .as_u64()
                .ok_or_else(|| ctx.fail(format!("compile-dataset {split} missing row_count")))?,
        );
        ctx.decoded
            .insert(format!("dataset_{split}"), compact_dataset_summary(&output));
    }
    Ok((dataset_id, train_shard_id, val_shard_id, split_counts))
}

fn get_panel_for_verification(
    ctx: &mut VerificationContext,
    panel_id: &str,
    label: &str,
) -> DynamicJepaResult<Value> {
    let output = run_ok_step(
        label,
        format!(
            "context-graph dynamicjepa get-panel --db {} --panel-id {} --include-readings --json",
            ctx.db.display(),
            panel_id
        ),
        get_panel_command(GetPanelArgs {
            db: ctx.db.clone(),
            panel_id: panel_id.to_string(),
            include_readings: true,
            json: true,
        }),
        ctx,
    )?;
    ctx.decoded.insert(label.to_string(), output.clone());
    Ok(output)
}

fn run_ok_step(
    label: &str,
    command: String,
    outcome: DynamicJepaResult<CommandOutcome>,
    ctx: &mut VerificationContext,
) -> DynamicJepaResult<Value> {
    let outcome = outcome?;
    ctx.record_command(command, label, &outcome.value);
    if outcome.exit_code != 0 || outcome.value["status"].as_str() != Some("ok") {
        return Err(ctx.fail(format!(
            "step {label} failed: {}",
            serde_json::to_string(&outcome.value)
                .unwrap_or_else(|_| "<unserializable>".to_string())
        )));
    }
    Ok(outcome.value)
}

fn assert_split_counts(
    ctx: &VerificationContext,
    split_counts: &BTreeMap<String, u64>,
) -> DynamicJepaResult<()> {
    let expected = expected_split_counts(ctx.domain);
    if split_counts != &expected {
        return Err(ctx.fail(format!(
            "split counts mismatch expected={expected:?} actual={split_counts:?}"
        )));
    }
    Ok(())
}

fn expected_split_counts(domain: VerificationDomain) -> BTreeMap<String, u64> {
    match domain {
        VerificationDomain::CounterWorld => BTreeMap::from([
            ("train".to_string(), 41),
            ("val".to_string(), 12),
            ("test".to_string(), 6),
        ]),
        VerificationDomain::Gridworld => BTreeMap::from([
            ("train".to_string(), 4977),
            ("val".to_string(), 1330),
            ("test".to_string(), 693),
        ]),
    }
}

fn assert_training_metrics(ctx: &VerificationContext, train: &Value) -> DynamicJepaResult<()> {
    let metrics = &train["summary"]["metrics"];
    let val_mse = metrics["val_latent_mse"]
        .as_f64()
        .ok_or_else(|| ctx.fail("training metrics missing val_latent_mse"))?;
    let val_cosine = metrics["val_latent_cosine"]
        .as_f64()
        .ok_or_else(|| ctx.fail("training metrics missing val_latent_cosine"))?;
    let action_contrast = metrics["val_action_contrast_acc"].as_f64().unwrap_or(1.0);
    match ctx.domain {
        VerificationDomain::CounterWorld => {
            if val_mse > 0.005 || val_cosine < 0.99 {
                return Err(ctx.fail(format!(
                    "counter_world metrics outside gate: val_latent_mse={val_mse} val_latent_cosine={val_cosine}"
                )));
            }
        }
        VerificationDomain::Gridworld => {
            if val_mse > 0.04 || val_cosine < 0.95 || action_contrast < 0.95 {
                return Err(ctx.fail(format!(
                    "gridworld metrics outside gate: val_latent_mse={val_mse} val_latent_cosine={val_cosine} val_action_contrast_acc={action_contrast}"
                )));
            }
        }
    }
    Ok(())
}

fn assert_counts_before_verification(
    ctx: &VerificationContext,
    counts_value: &Value,
    expected_after_verification: &BTreeMap<String, u64>,
) -> DynamicJepaResult<()> {
    let counts = counts_value["counts"]
        .as_object()
        .ok_or_else(|| ctx.fail("pre-verification inspect-counts missing counts object"))?;
    for (cf, expected_final) in expected_after_verification {
        let mut expected_before = *expected_final;
        if cf == CF_DJ_VERIFICATION_RUNS || cf == CF_DJ_AUDIT_LOG {
            expected_before = expected_before.saturating_sub(1);
        }
        let actual = counts
            .get(cf)
            .and_then(Value::as_u64)
            .ok_or_else(|| ctx.fail(format!("missing count for {cf}")))?;
        if actual != expected_before {
            return Err(ctx.fail(format!(
                "pre-verification count mismatch for {cf}: expected {expected_before}, actual {actual}"
            )));
        }
    }
    Ok(())
}

fn assert_delta(
    ctx: &VerificationContext,
    before: &Value,
    after: &Value,
    cf: &str,
    expected: i64,
    edge_name: &str,
) -> DynamicJepaResult<()> {
    let before_count = before["counts"][cf]
        .as_i64()
        .ok_or_else(|| ctx.fail(format!("edge {edge_name} before missing {cf}")))?;
    let after_count = after["counts"][cf]
        .as_i64()
        .ok_or_else(|| ctx.fail(format!("edge {edge_name} after missing {cf}")))?;
    let actual = after_count - before_count;
    if actual != expected {
        return Err(ctx.fail(format!(
            "edge {edge_name} {cf} delta mismatch: expected {expected}, actual {actual}"
        )));
    }
    Ok(())
}

fn assert_json_u64(
    ctx: &VerificationContext,
    value: &Value,
    expected: u64,
    field: &str,
) -> DynamicJepaResult<()> {
    let actual = value
        .as_u64()
        .ok_or_else(|| ctx.fail(format!("{field} missing or not u64")))?;
    if actual != expected {
        return Err(ctx.fail(format!(
            "{field} mismatch: expected {expected}, actual {actual}"
        )));
    }
    Ok(())
}

fn required_str(
    value: &Value,
    path: &[&str],
    ctx: &VerificationContext,
) -> DynamicJepaResult<String> {
    let mut current = value;
    for segment in path {
        current = current
            .get(*segment)
            .ok_or_else(|| ctx.fail(format!("missing JSON path {}", path.join("."))))?;
    }
    current
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ctx.fail(format!("JSON path {} is not a string", path.join("."))))
}

fn artifact_checks_from_json(
    ctx: &VerificationContext,
    artifact: &Value,
) -> DynamicJepaResult<Vec<ArtifactHashCheck>> {
    artifact["artifact_hashes"]
        .as_array()
        .ok_or_else(|| ctx.fail("get-artifact output missing artifact_hashes"))?
        .iter()
        .map(|item| {
            let registry = item["registry_sha256"]
                .as_str()
                .ok_or_else(|| ctx.fail("artifact hash check missing registry_sha256"))?;
            let recomputed = item["recomputed_sha256"]
                .as_str()
                .ok_or_else(|| ctx.fail("artifact hash check missing recomputed_sha256"))?;
            let check = ArtifactHashCheck {
                relative_path: item["relative_path"]
                    .as_str()
                    .ok_or_else(|| ctx.fail("artifact hash check missing relative_path"))?
                    .to_string(),
                registry_sha256: parse_sha256_hex(registry, "registry_sha256")?,
                recomputed_sha256: parse_sha256_hex(recomputed, "recomputed_sha256")?,
                size_bytes: item["registry_size_bytes"]
                    .as_u64()
                    .ok_or_else(|| ctx.fail("artifact hash check missing registry_size_bytes"))?,
                equal: item["equal"].as_bool() == Some(true),
            };
            check.validate()?;
            Ok(check)
        })
        .collect()
}

fn compact_training_summary(value: &Value) -> Value {
    json!({
        "created_ids": value["created_ids"],
        "summary": value["summary"],
        "artifact_hashes": value["artifact_hashes"],
        "count_delta": value["count_delta"],
    })
}

fn compact_command_output(value: &Value) -> Value {
    json!({
        "operation": value["operation"],
        "status": value["status"],
        "source_of_truth": value["source_of_truth"],
        "counts": value.get("counts").or_else(|| value.get("after_counts")).cloned().unwrap_or(Value::Null),
        "created_ids": value.get("created_ids").cloned().unwrap_or(Value::Null),
        "summary": value.get("summary").cloned().unwrap_or(Value::Null),
        "count_delta": value.get("count_delta").cloned().unwrap_or(Value::Null),
        "artifact_hashes": value.get("artifact_hashes").cloned().unwrap_or(Value::Null),
    })
}

fn compact_dataset_summary(value: &Value) -> Value {
    json!({
        "created_ids": value["created_ids"],
        "summary": value["summary"],
        "shape_summary": value["decoded_records"]["dataset_shard"]["shape_summary"],
        "leakage_report": value["decoded_records"]["dataset_shard"]["leakage_report"],
    })
}

fn values_to_dj_json_map(
    values: BTreeMap<String, Value>,
) -> DynamicJepaResult<BTreeMap<String, DjJsonValue>> {
    values
        .into_iter()
        .map(|(key, value)| Ok((key, DjJsonValue::from_json(value)?)))
        .collect()
}

fn write_generated_counter_verification_fixture(evidence_dir: &Path) -> DynamicJepaResult<PathBuf> {
    let mut lines = Vec::new();
    for counter in 0..60 {
        lines.push(format!(
            r#"{{"state":{{"counter":{counter}}},"action":{{"delta":1}},"outcome":{{"next_counter":{}}},"ts":{}}}"#,
            counter + 1,
            1_700_000_000_000i64 + counter as i64 * 1_000
        ));
    }
    let path = evidence_dir.join("counter_world_phase10_generated_60.jsonl");
    fs::write(&path, lines.join("\n") + "\n").map_err(|err| DynamicJepaError::Storage {
        operation: "verify.write_counter_fixture".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to write {}: {err}", path.display()),
        remediation: "use a writable --evidence-root".to_string(),
    })?;
    Ok(path)
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> DynamicJepaResult<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|err| {
        DynamicJepaError::validation(
            "verification_evidence_json",
            format!("failed to serialize {}: {err}", path.display()),
            "verification evidence must be JSON serializable",
        )
    })?;
    fs::write(path, bytes).map_err(|err| DynamicJepaError::Storage {
        operation: "verify.write_json_file".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to write {}: {err}", path.display()),
        remediation: "use a writable --evidence-root".to_string(),
    })
}

fn file_sha256(path: &Path) -> DynamicJepaResult<[u8; 32]> {
    let bytes = fs::read(path).map_err(|err| DynamicJepaError::Storage {
        operation: "verify.file_sha256".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to read {}: {err}", path.display()),
        remediation: "verify --fixtures-root points at the checked-in DynamicJEPA fixtures"
            .to_string(),
    })?;
    Ok(sha256(&bytes))
}

fn parse_sha256_hex(value: &str, field: &str) -> DynamicJepaResult<[u8; 32]> {
    if value.len() != 64 {
        return Err(DynamicJepaError::validation(
            field,
            format!("expected 64 hex chars, got {}", value.len()),
            "artifact hash evidence must contain SHA-256 hex",
        ));
    }
    let mut out = [0u8; 32];
    for (idx, chunk) in value.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|err| {
            DynamicJepaError::validation(
                field,
                format!("hash hex was not UTF-8: {err}"),
                "artifact hash evidence must contain ASCII hex",
            )
        })?;
        out[idx] = u8::from_str_radix(text, 16).map_err(|err| {
            DynamicJepaError::validation(
                field,
                format!("invalid SHA-256 hex byte {text:?}: {err}"),
                "artifact hash evidence must contain lowercase or uppercase hex",
            )
        })?;
    }
    Ok(out)
}

#[derive(Debug, Default, Clone, Serialize)]
struct IngestCreatedIds {
    raw_event_ids: Vec<String>,
    adapter_run_ids: Vec<String>,
    state_ids: Vec<String>,
    action_ids: Vec<String>,
    outcome_ids: Vec<String>,
    transition_ids: Vec<String>,
    failed_adapter_run_ids: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
struct PanelCreatedIds {
    panel_ids: Vec<String>,
    event_ids: Vec<String>,
    transition_ids: Vec<String>,
    instrument_reading_ids: Vec<String>,
}

impl PanelCreatedIds {
    fn push(&mut self, evidence: MaterializedPanelEvidence) {
        self.panel_ids.push(evidence.panel_id);
        self.event_ids.push(evidence.event_id);
        self.transition_ids.push(evidence.transition_id);
        self.instrument_reading_ids
            .extend(evidence.instrument_reading_ids);
    }
}

struct MaterializedPanelEvidence {
    panel_id: String,
    event_id: String,
    transition_id: String,
    instrument_reading_ids: Vec<String>,
}

#[derive(Clone)]
struct TrajectoryMember {
    transition: StateTransition,
    panel: LatentPanel,
}

struct TrajectoryGroup {
    entity_refs: Vec<String>,
    members: Vec<TrajectoryMember>,
}

#[derive(Clone)]
struct DatasetCandidateRow {
    trajectory_id: TrajectoryId,
    trajectory_entity_refs: Vec<String>,
    row_index_in_trajectory: usize,
    global_row_index: usize,
    split_name: String,
    input_panel: LatentPanel,
    target_panel: LatentPanel,
    action_id: ActionId,
}

#[derive(Clone)]
struct NegativePoolEntry {
    panel: LatentPanel,
    outcome_signature: String,
}

#[derive(Debug)]
struct AdapterLineError {
    message: String,
    field_path: String,
    expected_kind: String,
    actual_kind: String,
}

#[derive(Debug)]
struct AdaptedLine {
    state_fields: BTreeMap<String, FieldValue>,
    action_fields: BTreeMap<String, FieldValue>,
    outcome_fields: BTreeMap<String, FieldValue>,
    time_fields: BTreeMap<String, FieldValue>,
}

fn phase4_ingest_cfs() -> [&'static str; 7] {
    [
        CF_DJ_RAW_EVENTS,
        CF_DJ_ADAPTER_RUNS,
        CF_DJ_NORMALIZED_STATES,
        CF_DJ_ACTIONS,
        CF_DJ_OUTCOMES,
        CF_DJ_TRANSITIONS,
        CF_DJ_AUDIT_LOG,
    ]
}

fn resolve_bindable_cf_name(name: &str, field: &str) -> DynamicJepaResult<&'static str> {
    let cf_name = resolve_dynamic_cf_name(name)?;
    if matches!(
        cf_name,
        CF_DJ_DOMAIN_PACK_BY_NAME_VERSION | CF_DJ_BINDINGS_BY_ENTITY
    ) {
        return Err(DynamicJepaError::validation(
            field,
            format!("column family {cf_name} is a secondary index, not a concrete record CF"),
            "bind concrete decoded source-of-truth records such as dj_raw_events, dj_latent_panels, or dj_trajectories",
        ));
    }
    Ok(cf_name)
}

fn parse_binding_method(value: &str) -> DynamicJepaResult<BindingMethod> {
    match value {
        "id_equality" => Ok(BindingMethod::IdEquality),
        "explicit_mapping" => Ok(BindingMethod::ExplicitMapping),
        other => Err(DynamicJepaError::validation(
            "bind.method",
            format!("unsupported binding method {other:?}"),
            "use id_equality or explicit_mapping",
        )),
    }
}

fn parse_binding_kind(value: &str) -> DynamicJepaResult<BindingKind> {
    match value {
        "entity_to_entity" => Ok(BindingKind::EntityToEntity),
        "event_to_trajectory" => Ok(BindingKind::EventToTrajectory),
        "state_to_goal" => Ok(BindingKind::StateToGoal),
        "panel_to_memory" => Ok(BindingKind::PanelToMemory),
        other => Err(DynamicJepaError::validation(
            "bind.kind",
            format!("unsupported binding kind {other:?}"),
            "use entity_to_entity, event_to_trajectory, state_to_goal, or panel_to_memory",
        )),
    }
}

fn decoded_domain_identity(
    inspected: &Value,
    field: &str,
) -> DynamicJepaResult<(DomainPackId, String)> {
    let header = inspected
        .get("decoded")
        .and_then(|decoded| decoded.get("header"))
        .ok_or_else(|| {
            DynamicJepaError::validation(
                field,
                "decoded binding reference does not contain a DynamicJEPA record header",
                "bind only concrete DynamicJEPA records with DjRecordHeader",
            )
        })?;
    let domain = header
        .get("domain_pack_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                format!("{field}.header.domain_pack_id"),
                "decoded binding reference is missing domain_pack_id",
                "bind only records decoded from DynamicJEPA source-of-truth CFs",
            )
        })?;
    let version = header
        .get("domain_pack_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                format!("{field}.header.domain_pack_version"),
                "decoded binding reference is missing domain_pack_version",
                "bind only records decoded from DynamicJEPA source-of-truth CFs",
            )
        })?;
    Ok((DomainPackId::new(domain.to_string())?, version.to_string()))
}

fn binding_matches_entity_filter(binding: &BindingRecord, needle: &str) -> bool {
    let left_key = hex_bytes(&binding.left_ref.key_bytes).to_ascii_lowercase();
    let right_key = hex_bytes(&binding.right_ref.key_bytes).to_ascii_lowercase();
    binding.left_ref.cf.to_ascii_lowercase().contains(needle)
        || binding.right_ref.cf.to_ascii_lowercase().contains(needle)
        || left_key == needle
        || right_key == needle
        || binding
            .evidence_refs
            .iter()
            .any(|evidence| hex_bytes(&evidence.key_bytes).to_ascii_lowercase() == needle)
}

fn panel_index_by_transition(
    panels: Vec<LatentPanel>,
) -> DynamicJepaResult<BTreeMap<(Uuid, Uuid, Uuid), LatentPanel>> {
    let mut out = BTreeMap::new();
    for panel in panels {
        let outcome_id =
            panel
                .outcome_id
                .ok_or_else(|| DynamicJepaError::StorageInvariantViolation {
                    message: format!("panel {} has no outcome id", panel.panel_id),
                })?;
        let key = (panel.state_id.0, panel.action_id.0, outcome_id.0);
        if out.insert(key, panel).is_some() {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: "multiple latent panels matched one transition key".to_string(),
            });
        }
    }
    Ok(out)
}

fn trajectory_group_key(
    db: &rocksdb::DB,
    domain_pack: &DomainPack,
    transition: &StateTransition,
) -> DynamicJepaResult<(String, Vec<String>)> {
    if domain_pack.entity_schema.fields.is_empty() {
        return Ok((format!("domain:{}", domain_pack.id), Vec::new()));
    }
    let state = get_normalized_state(db, transition.prior_state)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_NORMALIZED_STATES.to_string(),
            key: transition.prior_state.into_bytes().to_vec(),
        }
    })?;
    let raw_event = get_raw_event(db, state.source_event_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_RAW_EVENTS.to_string(),
            key: state.source_event_id.into_bytes().to_vec(),
        }
    })?;
    let adapter = domain_pack.adapter_specs.first().ok_or_else(|| {
        DynamicJepaError::validation(
            "compile-trajectories.adapter",
            format!("domain {} has no adapter specs", domain_pack.id),
            "register a domain pack with a json_event adapter before compiling trajectories",
        )
    })?;
    let root: Value = serde_json::from_slice(trim_jsonl_newline(&raw_event.payload_bytes))
        .map_err(|err| DynamicJepaError::AdapterFailed {
            adapter_id: adapter.adapter_id.to_string(),
            event_id: raw_event.event_id.0,
            message: format!("raw event payload could not be re-parsed for entity grouping: {err}"),
            field_path: "$".to_string(),
        })?;

    let mut refs = Vec::new();
    for field in &domain_pack.entity_schema.fields {
        let target_path = format!("entity.{}", field.name);
        let source_path = adapter.mapping.get(&target_path).ok_or_else(|| {
            DynamicJepaError::validation(
                format!("compile-trajectories.{target_path}"),
                format!("adapter {} does not map {target_path}", adapter.adapter_id),
                "add the entity mapping to the domain pack and re-register it",
            )
        })?;
        let raw_value = resolve_json_path(&root, source_path).ok_or_else(|| {
            DynamicJepaError::AdapterFailed {
                adapter_id: adapter.adapter_id.to_string(),
                event_id: raw_event.event_id.0,
                message: format!("JSON path {source_path:?} did not resolve for {target_path}"),
                field_path: target_path.clone(),
            }
        })?;
        let coerced = coerce_json_value(raw_value, field, &target_path).map_err(|err| {
            DynamicJepaError::AdapterFailed {
                adapter_id: adapter.adapter_id.to_string(),
                event_id: raw_event.event_id.0,
                message: err.message,
                field_path: err.field_path,
            }
        })?;
        refs.push(format!(
            "{}={}",
            field.name,
            entity_field_value_to_string(&coerced)
        ));
    }
    if refs.is_empty() {
        return Err(DynamicJepaError::validation(
            "compile-trajectories.entity_refs",
            "entity schema was non-empty but no entity refs were produced",
            "inspect the domain pack entity schema and adapter mapping",
        ));
    }
    Ok((format!("entity:{}", refs.join("|")), refs))
}

fn entity_field_value_to_string(value: &FieldValue) -> String {
    match value {
        FieldValue::I64(value) | FieldValue::UnixMs(value) => value.to_string(),
        FieldValue::F64(value) => value.to_string(),
        FieldValue::Bool(value) => value.to_string(),
        FieldValue::String(value) => value.clone(),
        FieldValue::Categorical { variant } => variant.clone(),
        FieldValue::Vector(values) => values
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(","),
    }
}

fn build_trajectory_record(
    domain_pack: &DomainPack,
    policy: &str,
    group_key: &str,
    mut group: TrajectoryGroup,
    run_id: Uuid,
) -> DynamicJepaResult<TrajectoryRecord> {
    group.members.sort_by(|left, right| {
        left.transition
            .timestamp_ms
            .cmp(&right.transition.timestamp_ms)
            .then_with(|| {
                left.transition
                    .transition_id
                    .cmp(&right.transition.transition_id)
            })
    });
    for pair in group.members.windows(2) {
        if pair[0].transition.timestamp_ms >= pair[1].transition.timestamp_ms {
            return Err(DynamicJepaError::TrajectoryInvariantViolation {
                message: format!(
                    "trajectory group {group_key} has non-strict timestamps {} then {}",
                    pair[0].transition.timestamp_ms, pair[1].transition.timestamp_ms
                ),
                trajectory_id: Uuid::new_v4(),
            });
        }
    }
    let first = group.members.first().ok_or_else(|| {
        DynamicJepaError::validation(
            "compile-trajectories.group",
            format!("trajectory group {group_key} was empty"),
            "compile only groups with at least one transition and panel",
        )
    })?;
    let last = group.members.last().expect("first checked");
    let record_count = u32::try_from(group.members.len()).map_err(|_| {
        DynamicJepaError::validation(
            "compile-trajectories.record_count",
            format!("trajectory group {group_key} has too many records"),
            "split huge groups before compiling trajectories",
        )
    })?;
    let ordered_transition_ids = group
        .members
        .iter()
        .map(|member| member.transition.transition_id)
        .collect::<Vec<_>>();
    let ordered_panel_ids = group
        .members
        .iter()
        .map(|member| member.panel.panel_id)
        .collect::<Vec<_>>();
    let trajectory_id = TrajectoryId::new_v4();
    let trajectory_hash = trajectory_hash(
        &domain_pack.id,
        policy,
        group_key,
        &ordered_transition_ids,
        &ordered_panel_ids,
    )?;
    let mut record = TrajectoryRecord {
        header: DjRecordHeader::new(
            trajectory_id.0,
            TRAJECTORY_RECORD_VERSION,
            domain_pack.id.clone(),
            domain_pack.version.clone(),
            now_unix_ms()?,
            Some(run_id),
        ),
        trajectory_id,
        segmentation_policy_id: policy.to_string(),
        ordered_transition_ids,
        ordered_panel_ids,
        start_time_unix_ms: first.transition.timestamp_ms,
        end_time_unix_ms: last.transition.timestamp_ms,
        entity_refs: group.entity_refs,
        binding_refs: Vec::new(),
        trajectory_hash,
        record_count,
    };
    record.refresh_content_hash()?;
    Ok(record)
}

fn trajectory_hash(
    domain_pack_id: &DomainPackId,
    policy: &str,
    group_key: &str,
    transition_ids: &[TransitionId],
    panel_ids: &[PanelId],
) -> DynamicJepaResult<[u8; 32]> {
    let bytes = serde_json::to_vec(&json!({
        "domain_pack_id": domain_pack_id.to_string(),
        "segmentation_policy_id": policy,
        "group_key": group_key,
        "ordered_transition_ids": transition_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "ordered_panel_ids": panel_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
    }))
    .map_err(|err| {
        DynamicJepaError::validation(
            "trajectory_hash",
            format!("failed to serialize trajectory hash payload: {err}"),
            "trajectory hash payload must remain JSON serializable",
        )
    })?;
    Ok(sha256(&bytes))
}

fn verify_trajectory_sources(db: &rocksdb::DB, record: &TrajectoryRecord) -> DynamicJepaResult<()> {
    let mut previous_timestamp = None;
    for transition_id in &record.ordered_transition_ids {
        let transition = get_transition(db, *transition_id)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_TRANSITIONS.to_string(),
                key: transition_id.into_bytes().to_vec(),
            }
        })?;
        if let Some(previous) = previous_timestamp {
            if previous >= transition.timestamp_ms {
                return Err(DynamicJepaError::TrajectoryInvariantViolation {
                    message: format!(
                        "trajectory {} timestamps are not strictly ascending",
                        record.trajectory_id
                    ),
                    trajectory_id: record.trajectory_id.0,
                });
            }
        }
        previous_timestamp = Some(transition.timestamp_ms);
    }
    for panel_id in &record.ordered_panel_ids {
        get_latent_panel(db, *panel_id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_LATENT_PANELS.to_string(),
            key: panel_id.into_bytes().to_vec(),
        })?;
    }
    Ok(())
}

fn collect_dataset_rows(
    db: &rocksdb::DB,
    domain_pack: &DomainPack,
    trajectories: &[TrajectoryRecord],
) -> DynamicJepaResult<Vec<DatasetCandidateRow>> {
    let mut trajectories = trajectories.to_vec();
    trajectories.sort_by(|left, right| {
        left.start_time_unix_ms
            .cmp(&right.start_time_unix_ms)
            .then_with(|| left.trajectory_id.cmp(&right.trajectory_id))
    });

    let mut rows = Vec::new();
    for trajectory in trajectories {
        verify_trajectory_sources(db, &trajectory)?;
        if trajectory.ordered_panel_ids.len() < 2 {
            continue;
        }
        for row_index in 0..(trajectory.ordered_panel_ids.len() - 1) {
            let transition_id = trajectory.ordered_transition_ids[row_index];
            let transition = get_transition(db, transition_id)?.ok_or_else(|| {
                DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_TRANSITIONS.to_string(),
                    key: transition_id.into_bytes().to_vec(),
                }
            })?;
            let input_panel_id = trajectory.ordered_panel_ids[row_index];
            let target_panel_id = trajectory.ordered_panel_ids[row_index + 1];
            let input_panel = get_latent_panel(db, input_panel_id)?.ok_or_else(|| {
                DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_LATENT_PANELS.to_string(),
                    key: input_panel_id.into_bytes().to_vec(),
                }
            })?;
            let target_panel = get_latent_panel(db, target_panel_id)?.ok_or_else(|| {
                DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_LATENT_PANELS.to_string(),
                    key: target_panel_id.into_bytes().to_vec(),
                }
            })?;
            if input_panel.action_id != transition.action {
                return Err(DynamicJepaError::StorageInvariantViolation {
                    message: format!(
                        "trajectory {} row {} input panel action {} did not match transition action {}",
                        trajectory.trajectory_id,
                        row_index,
                        input_panel.action_id,
                        transition.action
                    ),
                });
            }
            let global_row_index = rows.len();
            rows.push(DatasetCandidateRow {
                trajectory_id: trajectory.trajectory_id,
                trajectory_entity_refs: trajectory.entity_refs.clone(),
                row_index_in_trajectory: row_index,
                global_row_index,
                split_name: String::new(),
                input_panel,
                target_panel,
                action_id: transition.action,
            });
        }
    }

    let total_rows = rows.len();
    for row in &mut rows {
        row.split_name = split_for_dataset_row(domain_pack, row, total_rows)?;
    }
    Ok(rows)
}

fn split_for_dataset_row(
    domain_pack: &DomainPack,
    row: &DatasetCandidateRow,
    total_rows: usize,
) -> DynamicJepaResult<String> {
    match domain_pack.dataset_policy.default_split.as_str() {
        "by_time" => Ok(by_time_split(row.global_row_index, total_rows).to_string()),
        "by_domain_key_hash" => by_domain_key_hash_split(domain_pack, row),
        other => Err(DynamicJepaError::validation(
            "compile-dataset.default_split",
            format!("unsupported deterministic split policy {other:?}"),
            "use by_time or by_domain_key_hash in the registered domain pack",
        )),
    }
}

fn by_time_split(row_index: usize, total_rows: usize) -> &'static str {
    if total_rows <= 1 {
        return "train";
    }
    if total_rows == 2 {
        return if row_index == 0 { "train" } else { "val" };
    }

    let train_count = ((total_rows * 7) / 10).max(1);
    let remaining_after_train = total_rows.saturating_sub(train_count);
    let val_count = if remaining_after_train == 0 {
        0
    } else {
        ((total_rows * 2).div_ceil(10))
            .max(1)
            .min(remaining_after_train)
    };
    if row_index < train_count {
        "train"
    } else if row_index < train_count + val_count {
        "val"
    } else {
        "test"
    }
}

fn by_domain_key_hash_split(
    domain_pack: &DomainPack,
    row: &DatasetCandidateRow,
) -> DynamicJepaResult<String> {
    let split_key = domain_pack
        .dataset_policy
        .split_key
        .as_deref()
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "compile-dataset.split_key",
                "by_domain_key_hash requires dataset_policy.split_key",
                "set split_key = \"entity.<field>\" in the domain pack",
            )
        })?;
    let entity_field = split_key.strip_prefix("entity.").ok_or_else(|| {
        DynamicJepaError::validation(
            "compile-dataset.split_key",
            format!("split key {split_key:?} is not an entity field"),
            "Phase 6 by_domain_key_hash supports entity.<field> keys",
        )
    })?;
    let prefix = format!("{entity_field}=");
    let entity_value = row
        .trajectory_entity_refs
        .iter()
        .find_map(|reference| reference.strip_prefix(&prefix))
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "compile-dataset.entity_refs",
                format!(
                    "trajectory {} does not carry split key {}",
                    row.trajectory_id, split_key
                ),
                "compile trajectories from fixtures that include the declared entity split key",
            )
        })?;
    let buckets = domain_pack
        .dataset_policy
        .split_buckets
        .as_ref()
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "compile-dataset.split_buckets",
                "by_domain_key_hash requires train/val/test split buckets",
                "declare split_buckets in the domain pack dataset policy",
            )
        })?;
    let train = split_bucket_fraction(buckets, "train")?;
    let val = split_bucket_fraction(buckets, "val")?;
    let test = split_bucket_fraction(buckets, "test")?;
    if (train + val + test - 1.0).abs() > 1e-6 {
        return Err(DynamicJepaError::validation(
            "compile-dataset.split_buckets",
            format!(
                "split bucket fractions must sum to 1.0, got {}",
                train + val + test
            ),
            "fix the domain pack split_buckets before compiling datasets",
        ));
    }

    let train_end = fraction_to_bucket_boundary(train, "train")?;
    let val_end = train_end + fraction_to_bucket_boundary(val, "val")?;
    let hash = context_graph_storage::dynamicjepa::keys::blake3_128(entity_value.as_bytes());
    let bucket = u128::from_be_bytes(hash) % 1000;
    if bucket < train_end as u128 {
        Ok("train".to_string())
    } else if bucket < val_end as u128 {
        Ok("val".to_string())
    } else {
        Ok("test".to_string())
    }
}

fn split_bucket_fraction(
    buckets: &BTreeMap<String, f64>,
    name: &'static str,
) -> DynamicJepaResult<f64> {
    let value = buckets.get(name).copied().ok_or_else(|| {
        DynamicJepaError::validation(
            "compile-dataset.split_buckets",
            format!("missing {name} split bucket"),
            "declare train, val, and test bucket fractions",
        )
    })?;
    if value < 0.0 || !value.is_finite() {
        return Err(DynamicJepaError::validation(
            "compile-dataset.split_buckets",
            format!("{name} split bucket must be finite and non-negative, got {value}"),
            "fix the domain pack split_buckets before compiling datasets",
        ));
    }
    Ok(value)
}

fn fraction_to_bucket_boundary(value: f64, name: &'static str) -> DynamicJepaResult<u64> {
    let boundary = (value * 1000.0).round();
    if !(0.0..=1000.0).contains(&boundary) {
        return Err(DynamicJepaError::validation(
            "compile-dataset.split_buckets",
            format!("{name} bucket fraction {value} produced invalid boundary {boundary}"),
            "declare split buckets between 0.0 and 1.0",
        ));
    }
    Ok(boundary as u64)
}

fn split_row_counts(rows: &[DatasetCandidateRow]) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::from([
        ("train".to_string(), 0u32),
        ("val".to_string(), 0u32),
        ("test".to_string(), 0u32),
    ]);
    for row in rows {
        if let Some(count) = counts.get_mut(&row.split_name) {
            *count += 1;
        }
    }
    counts
}

fn split_overlap_count(
    domain_pack: &DomainPack,
    rows: &[DatasetCandidateRow],
) -> DynamicJepaResult<u64> {
    if domain_pack.dataset_policy.default_split != "by_domain_key_hash" {
        return Ok(0);
    }
    let mut by_entity: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in rows {
        for entity_ref in &row.trajectory_entity_refs {
            by_entity
                .entry(entity_ref.clone())
                .or_default()
                .insert(row.split_name.clone());
        }
    }
    Ok(by_entity.values().filter(|splits| splits.len() > 1).count() as u64)
}

fn build_negative_pool(
    db: &rocksdb::DB,
    panels: Vec<LatentPanel>,
) -> DynamicJepaResult<Vec<NegativePoolEntry>> {
    let mut pool = panels
        .into_iter()
        .map(|panel| {
            let outcome_signature = panel_outcome_signature(db, &panel)?;
            Ok(NegativePoolEntry {
                panel,
                outcome_signature,
            })
        })
        .collect::<DynamicJepaResult<Vec<_>>>()?;
    pool.sort_by(|left, right| left.panel.panel_id.cmp(&right.panel.panel_id));
    Ok(pool)
}

fn build_dataset_record(
    domain_pack: &DomainPack,
    split_name: &str,
    selected_rows: &[DatasetCandidateRow],
    negative_pool: &[NegativePoolEntry],
    split_counts: &BTreeMap<String, u32>,
    split_overlap_count: u64,
    dataset_id: DatasetId,
) -> DynamicJepaResult<DatasetShardRecord> {
    let row_count = u32::try_from(selected_rows.len()).map_err(|_| {
        DynamicJepaError::validation(
            "compile-dataset.row_count",
            format!("dataset split {split_name} has too many rows"),
            "split huge datasets into multiple shards before compiling",
        )
    })?;
    let objective_id = domain_pack.dataset_policy.default_objective.clone();
    if !domain_pack
        .objective_specs
        .iter()
        .any(|objective| objective.id == objective_id)
    {
        return Err(DynamicJepaError::validation(
            "compile-dataset.default_objective",
            format!("default objective {objective_id:?} is not declared in objective_specs"),
            "fix the domain pack objective list before compiling datasets",
        ));
    }

    let mut source_trajectory_ids = Vec::new();
    let mut seen_trajectories = BTreeSet::new();
    let mut input_panel_ids = Vec::with_capacity(selected_rows.len());
    let mut target_panel_ids = Vec::with_capacity(selected_rows.len());
    let mut action_ids = Vec::with_capacity(selected_rows.len());
    let mut negative_panel_ids = Vec::with_capacity(selected_rows.len());
    let mut source_hashes = Vec::with_capacity(selected_rows.len() * 2);
    let mut leakage_report = LeakageReport {
        future_in_input_count: 0,
        same_panel_input_target_count: 0,
        negative_equals_target_count: 0,
        split_overlap_count,
    };

    let input_dim = panel_dense_dim(&selected_rows[0].input_panel)?;
    let target_dim = panel_dense_dim(&selected_rows[0].target_panel)?;
    let action_dim = panel_action_dim(&selected_rows[0].input_panel)?;
    for row in selected_rows {
        if seen_trajectories.insert(row.trajectory_id.0) {
            source_trajectory_ids.push(row.trajectory_id);
        }
        let row_input_dim = panel_dense_dim(&row.input_panel)?;
        let row_target_dim = panel_dense_dim(&row.target_panel)?;
        let row_action_dim = panel_action_dim(&row.input_panel)?;
        if row_input_dim != input_dim
            || row_target_dim != target_dim
            || row_action_dim != action_dim
        {
            return Err(DynamicJepaError::PanelShapeMismatch {
                domain_pack_id: domain_pack.id.to_string(),
                expected: vec![input_dim, target_dim, action_dim],
                actual: vec![row_input_dim, row_target_dim, row_action_dim],
            });
        }

        if row.input_panel.panel_id == row.target_panel.panel_id {
            leakage_report.same_panel_input_target_count += 1;
        }
        let negative_panel_id = select_negative_panel_id(
            domain_pack,
            row,
            negative_pool,
            domain_pack.dataset_policy.min_negatives_per_row,
        )?;
        if negative_panel_id == row.target_panel.panel_id {
            leakage_report.negative_equals_target_count += 1;
        }

        input_panel_ids.push(row.input_panel.panel_id);
        target_panel_ids.push(row.target_panel.panel_id);
        action_ids.push(row.action_id);
        negative_panel_ids.push(negative_panel_id);
        source_hashes.push(row.input_panel.header.content_hash);
        source_hashes.push(row.target_panel.header.content_hash);
    }
    leakage_report.validate()?;

    let shard_id = DatasetShardId::new_v4();
    let mut record = DatasetShardRecord {
        header: DjRecordHeader::new(
            shard_id.0,
            DATASET_SHARD_RECORD_VERSION,
            domain_pack.id.clone(),
            domain_pack.version.clone(),
            now_unix_ms()?,
            Some(Uuid::new_v4()),
        ),
        dataset_id,
        shard_id,
        source_trajectory_ids,
        split_name: split_name.to_string(),
        row_count,
        input_panel_ids,
        target_panel_ids,
        action_ids,
        negative_panel_ids,
        objective_ids: vec![objective_id],
        shape_summary: ShapeSummary {
            input_dim,
            target_dim,
            action_dim,
            n_train_rows: *split_counts.get("train").unwrap_or(&0),
            n_val_rows: *split_counts.get("val").unwrap_or(&0),
            n_test_rows: *split_counts.get("test").unwrap_or(&0),
        },
        source_hashes,
        leakage_report,
        compiler_version: 1,
    };
    record.refresh_content_hash()?;
    Ok(record)
}

fn select_negative_panel_id(
    domain_pack: &DomainPack,
    row: &DatasetCandidateRow,
    negative_pool: &[NegativePoolEntry],
    required: u32,
) -> DynamicJepaResult<PanelId> {
    let target_signature = negative_pool
        .iter()
        .find(|entry| entry.panel.panel_id == row.target_panel.panel_id)
        .map(|entry| entry.outcome_signature.as_str())
        .ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_LATENT_PANELS.to_string(),
            key: row.target_panel.panel_id.into_bytes().to_vec(),
        })?;
    let candidates = negative_pool
        .iter()
        .filter(|entry| {
            entry.panel.panel_id != row.target_panel.panel_id
                && entry.outcome_signature != target_signature
        })
        .collect::<Vec<_>>();
    if candidates.len() < required as usize {
        return Err(DynamicJepaError::DatasetLeakageDetected {
            message: format!(
                "insufficient negatives for domain {} trajectory {} row {}: required={} found={}",
                domain_pack.id,
                row.trajectory_id,
                row.row_index_in_trajectory,
                required,
                candidates.len()
            ),
            dataset_id: Uuid::nil(),
        });
    }
    let seed = format!(
        "{}:{}:{}:{}",
        row.trajectory_id,
        row.row_index_in_trajectory,
        row.input_panel.panel_id,
        row.target_panel.panel_id
    );
    let hash = context_graph_storage::dynamicjepa::keys::blake3_128(seed.as_bytes());
    let idx = (u128::from_be_bytes(hash) % candidates.len() as u128) as usize;
    Ok(candidates[idx].panel.panel_id)
}

fn verify_dataset_shard_sources(
    db: &rocksdb::DB,
    record: &DatasetShardRecord,
) -> DynamicJepaResult<()> {
    let row_count = record.row_count as usize;
    if record.source_hashes.len() != row_count * 2 {
        return Err(DynamicJepaError::DatasetLeakageDetected {
            message: format!(
                "source_hashes length {} does not equal row_count*2 ({})",
                record.source_hashes.len(),
                row_count * 2
            ),
            dataset_id: record.dataset_id.0,
        });
    }
    for row_idx in 0..row_count {
        let input_panel_id = record.input_panel_ids[row_idx];
        let target_panel_id = record.target_panel_ids[row_idx];
        let action_id = record.action_ids[row_idx];
        let negative_panel_id = record.negative_panel_ids[row_idx];
        let input_panel = get_latent_panel(db, input_panel_id)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_LATENT_PANELS.to_string(),
                key: input_panel_id.into_bytes().to_vec(),
            }
        })?;
        let target_panel = get_latent_panel(db, target_panel_id)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_LATENT_PANELS.to_string(),
                key: target_panel_id.into_bytes().to_vec(),
            }
        })?;
        get_action(db, action_id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_ACTIONS.to_string(),
            key: action_id.into_bytes().to_vec(),
        })?;
        get_latent_panel(db, negative_panel_id)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_LATENT_PANELS.to_string(),
                key: negative_panel_id.into_bytes().to_vec(),
            }
        })?;
        if record.source_hashes[row_idx * 2] != input_panel.header.content_hash {
            return Err(DynamicJepaError::DatasetLeakageDetected {
                message: format!(
                    "shard source hashes diverged from live input panel {} at row {}",
                    input_panel_id, row_idx
                ),
                dataset_id: record.dataset_id.0,
            });
        }
        if record.source_hashes[row_idx * 2 + 1] != target_panel.header.content_hash {
            return Err(DynamicJepaError::DatasetLeakageDetected {
                message: format!(
                    "shard source hashes diverged from live target panel {} at row {}",
                    target_panel_id, row_idx
                ),
                dataset_id: record.dataset_id.0,
            });
        }
    }
    Ok(())
}

fn dataset_row_snapshot(
    db: &rocksdb::DB,
    record: &DatasetShardRecord,
    row_index: usize,
) -> DynamicJepaResult<Value> {
    let row_count = record.row_count as usize;
    if row_index >= row_count {
        return Err(DynamicJepaError::validation(
            "inspect-dataset.row",
            format!("row index {row_index} is outside dataset row_count {row_count}"),
            "request a row index in 0..row_count",
        ));
    }
    let input_panel_id = record.input_panel_ids[row_index];
    let target_panel_id = record.target_panel_ids[row_index];
    let negative_panel_id = record.negative_panel_ids[row_index];
    let action_id = record.action_ids[row_index];

    let input_panel = get_latent_panel(db, input_panel_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_LATENT_PANELS.to_string(),
            key: input_panel_id.into_bytes().to_vec(),
        }
    })?;
    let target_panel = get_latent_panel(db, target_panel_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_LATENT_PANELS.to_string(),
            key: target_panel_id.into_bytes().to_vec(),
        }
    })?;
    let negative_panel = get_latent_panel(db, negative_panel_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_LATENT_PANELS.to_string(),
            key: negative_panel_id.into_bytes().to_vec(),
        }
    })?;
    let action =
        get_action(db, action_id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_ACTIONS.to_string(),
            key: action_id.into_bytes().to_vec(),
        })?;
    let input_state = get_normalized_state(db, input_panel.state_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_NORMALIZED_STATES.to_string(),
            key: input_panel.state_id.into_bytes().to_vec(),
        }
    })?;
    let target_state = get_normalized_state(db, target_panel.state_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_NORMALIZED_STATES.to_string(),
            key: target_panel.state_id.into_bytes().to_vec(),
        }
    })?;
    let negative_state = get_normalized_state(db, negative_panel.state_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_NORMALIZED_STATES.to_string(),
            key: negative_panel.state_id.into_bytes().to_vec(),
        }
    })?;
    let target_outcome = panel_outcome_record(db, &target_panel)?;
    let negative_outcome = panel_outcome_record(db, &negative_panel)?;
    let expected_input_hash = record.source_hashes.get(row_index * 2).ok_or_else(|| {
        DynamicJepaError::DatasetLeakageDetected {
            message: format!("missing input source hash for row {row_index}"),
            dataset_id: record.dataset_id.0,
        }
    })?;
    let expected_target_hash = record.source_hashes.get(row_index * 2 + 1).ok_or_else(|| {
        DynamicJepaError::DatasetLeakageDetected {
            message: format!("missing target source hash for row {row_index}"),
            dataset_id: record.dataset_id.0,
        }
    })?;

    Ok(json!({
        "row_index": row_index,
        "input_panel_id": input_panel_id.to_string(),
        "input_panel_dense_norm": panel_dense_norm(&input_panel),
        "input_state_fields": input_state.fields,
        "input_action_id": action_id.to_string(),
        "input_action_fields": action.fields,
        "target_panel_id": target_panel_id.to_string(),
        "target_panel_dense_norm": panel_dense_norm(&target_panel),
        "target_state_fields": target_state.fields,
        "target_outcome_fields": target_outcome.fields,
        "negative_panel_id": negative_panel_id.to_string(),
        "negative_state_fields": negative_state.fields,
        "negative_outcome_fields": negative_outcome.fields,
        "source_hash_checks": {
            "input_equal": *expected_input_hash == input_panel.header.content_hash,
            "target_equal": *expected_target_hash == target_panel.header.content_hash,
        }
    }))
}

fn panel_outcome_record(
    db: &rocksdb::DB,
    panel: &LatentPanel,
) -> DynamicJepaResult<NormalizedOutcome> {
    let outcome_id = panel
        .outcome_id
        .ok_or_else(|| DynamicJepaError::DatasetLeakageDetected {
            message: format!("panel {} has no outcome_id", panel.panel_id),
            dataset_id: Uuid::nil(),
        })?;
    get_outcome(db, outcome_id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
        cf: CF_DJ_OUTCOMES.to_string(),
        key: outcome_id.into_bytes().to_vec(),
    })
}

fn panel_outcome_signature(db: &rocksdb::DB, panel: &LatentPanel) -> DynamicJepaResult<String> {
    let outcome = panel_outcome_record(db, panel)?;
    serde_json::to_string(&outcome.fields).map_err(|err| {
        DynamicJepaError::validation(
            "compile-dataset.outcome_signature",
            format!("failed to serialize outcome fields: {err}"),
            "outcome fields must remain JSON serializable for deterministic negative sampling",
        )
    })
}

fn panel_dense_dim(panel: &LatentPanel) -> DynamicJepaResult<usize> {
    let dim = panel
        .ordered_slots
        .iter()
        .map(|slot| slot.dim)
        .sum::<usize>();
    if dim == 0 {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: panel.header.domain_pack_id.to_string(),
            expected: vec![1],
            actual: vec![0],
        });
    }
    Ok(dim)
}

fn panel_action_dim(panel: &LatentPanel) -> DynamicJepaResult<usize> {
    let dim = panel
        .ordered_slots
        .iter()
        .filter(|slot| matches!(slot.kind, PanelSlotKind::Action))
        .map(|slot| slot.dim)
        .sum::<usize>();
    if dim == 0 {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: panel.header.domain_pack_id.to_string(),
            expected: vec![1],
            actual: vec![0],
        });
    }
    Ok(dim)
}

fn panel_dense_norm(panel: &LatentPanel) -> f64 {
    panel
        .slot_vectors
        .iter()
        .flatten()
        .map(|value| {
            let value = *value as f64;
            value * value
        })
        .sum::<f64>()
        .sqrt()
}

fn ingest_jsonl_fixture(
    db: &rocksdb::DB,
    domain_pack: &DomainPack,
    adapter: &AdapterSpec,
    source_file: &Path,
    source_uri: &str,
    created: &mut IngestCreatedIds,
) -> DynamicJepaResult<()> {
    if adapter.kind != "json_event" {
        return Err(DynamicJepaError::validation(
            "ingest-event.adapter.kind",
            format!("unsupported adapter kind {:?}", adapter.kind),
            "Phase 4 supports json_event only",
        ));
    }
    let bytes = fs::read(source_file).map_err(|err| {
        DynamicJepaError::validation(
            "ingest-event.file",
            format!(
                "failed to read JSONL fixture {}: {err}",
                source_file.display()
            ),
            "provide an existing JSONL fixture file",
        )
    })?;
    if bytes.is_empty() {
        let err = DynamicJepaError::AdapterFailed {
            adapter_id: adapter.adapter_id.to_string(),
            event_id: Uuid::nil(),
            message: "empty input file".to_string(),
            field_path: String::new(),
        };
        let audit = build_audit(
            "ingest_event",
            AuditStatus::Failed {
                error_code: err.code().to_string(),
            },
            vec![source_uri.to_string()],
            Vec::new(),
            vec![CF_DJ_AUDIT_LOG.to_string()],
            Vec::new(),
        )?;
        put_audit_record(db, &audit)?;
        return Err(err);
    }

    let run_id = Uuid::new_v4();
    let mut offset = 0u64;
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        let line_offset = offset;
        offset += line.len() as u64;
        process_fixture_line(
            db,
            domain_pack,
            adapter,
            source_uri,
            line_offset,
            line,
            run_id,
            created,
        )?;
    }
    Ok(())
}

fn process_fixture_line(
    db: &rocksdb::DB,
    domain_pack: &DomainPack,
    adapter: &AdapterSpec,
    source_uri: &str,
    source_offset: u64,
    line_bytes: &[u8],
    run_id: Uuid,
    created: &mut IngestCreatedIds,
) -> DynamicJepaResult<()> {
    let now = now_unix_ms()?;
    let event_id = EventId::new_v4();
    let adapter_run_id = Uuid::new_v4();
    let mut raw_event = RawDomainEvent {
        header: DjRecordHeader::new(
            event_id.0,
            RAW_DOMAIN_EVENT_RECORD_VERSION,
            domain_pack.id.clone(),
            domain_pack.version.clone(),
            now,
            None,
        ),
        event_id,
        domain_pack_id: domain_pack.id.clone(),
        adapter_id: adapter.adapter_id.clone(),
        source_kind: SourceKind::JsonlFixture,
        source_uri: source_uri.to_string(),
        source_offset,
        payload_format: PayloadFormat::Json,
        payload_bytes: line_bytes.to_vec(),
        payload_hash: sha256(line_bytes),
        received_at_unix_ms: now,
    };
    raw_event.refresh_content_hash()?;
    let mut started_run = AdapterRunRecord {
        header: DjRecordHeader::new(
            adapter_run_id,
            ADAPTER_RUN_RECORD_VERSION,
            domain_pack.id.clone(),
            domain_pack.version.clone(),
            now,
            Some(run_id),
        ),
        adapter_run_id,
        adapter_id: adapter.adapter_id.clone(),
        domain_pack_id: domain_pack.id.clone(),
        event_id,
        started_at_unix_ms: now,
        finished_at_unix_ms: None,
        status: AdapterRunStatus::Started,
        error_code: None,
        error_message: None,
        field_path: None,
        expected_kind: None,
        actual_kind: None,
        output_state_id: None,
        output_action_id: None,
        output_outcome_id: None,
        output_transition_id: None,
    };
    started_run.refresh_content_hash()?;
    let ingest_audit = build_audit(
        "ingest_raw_event",
        AuditStatus::Ok,
        vec![source_uri.to_string(), source_offset.to_string()],
        vec![event_id.to_string(), adapter_run_id.to_string()],
        vec![
            CF_DJ_RAW_EVENTS.to_string(),
            CF_DJ_ADAPTER_RUNS.to_string(),
            CF_DJ_AUDIT_LOG.to_string(),
        ],
        vec![
            raw_event.header.content_hash,
            started_run.header.content_hash,
        ],
    )?;
    put_raw_event_and_adapter_run_started(db, &raw_event, &started_run, &ingest_audit)?;
    created.raw_event_ids.push(event_id.to_string());
    created.adapter_run_ids.push(adapter_run_id.to_string());

    match adapt_payload(domain_pack, adapter, line_bytes) {
        Ok(adapted) => {
            let state_id = StateId::new_v4();
            let action_id = ActionId::new_v4();
            let outcome_id = OutcomeId::new_v4();
            let transition_id = TransitionId::new_v4();
            let timestamp_ms = adapted_timestamp_ms(domain_pack, &adapted)?;
            let mut state = NormalizedState {
                header: DjRecordHeader::new(
                    state_id.0,
                    NORMALIZED_STATE_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                state_id,
                fields: adapted.state_fields,
                source_event_id: event_id,
            };
            state.refresh_content_hash()?;
            let mut action = NormalizedAction {
                header: DjRecordHeader::new(
                    action_id.0,
                    NORMALIZED_ACTION_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                action_id,
                fields: adapted.action_fields,
                source_event_id: event_id,
                action_origin: ActionOrigin::Observed,
            };
            action.refresh_content_hash()?;
            let mut outcome = NormalizedOutcome {
                header: DjRecordHeader::new(
                    outcome_id.0,
                    NORMALIZED_OUTCOME_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                outcome_id,
                fields: adapted.outcome_fields,
                source_event_id: event_id,
            };
            outcome.refresh_content_hash()?;
            let mut transition = StateTransition {
                header: DjRecordHeader::new(
                    transition_id.0,
                    STATE_TRANSITION_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                transition_id,
                prior_state: state_id,
                action: action_id,
                outcome: outcome_id,
                next_state: state_id,
                timestamp_ms,
            };
            transition.refresh_content_hash()?;
            let mut completed_run = started_run.clone();
            completed_run.finished_at_unix_ms = Some(adapter_run_terminal_timestamp_ms(
                started_run.started_at_unix_ms,
            )?);
            completed_run.status = AdapterRunStatus::Completed;
            completed_run.output_state_id = Some(state_id);
            completed_run.output_action_id = Some(action_id);
            completed_run.output_outcome_id = Some(outcome_id);
            completed_run.output_transition_id = Some(transition_id);
            completed_run.refresh_content_hash()?;
            let audit = build_audit(
                "complete_adapter_run",
                AuditStatus::Ok,
                vec![event_id.to_string(), adapter_run_id.to_string()],
                vec![
                    state_id.to_string(),
                    action_id.to_string(),
                    outcome_id.to_string(),
                    transition_id.to_string(),
                ],
                vec![
                    CF_DJ_ADAPTER_RUNS.to_string(),
                    CF_DJ_NORMALIZED_STATES.to_string(),
                    CF_DJ_ACTIONS.to_string(),
                    CF_DJ_OUTCOMES.to_string(),
                    CF_DJ_TRANSITIONS.to_string(),
                    CF_DJ_AUDIT_LOG.to_string(),
                ],
                vec![
                    completed_run.header.content_hash,
                    state.header.content_hash,
                    action.header.content_hash,
                    outcome.header.content_hash,
                    transition.header.content_hash,
                ],
            )?;
            put_adapter_run_success_batch(
                db,
                &completed_run,
                &state,
                &action,
                &outcome,
                &transition,
                &audit,
            )?;
            created.state_ids.push(state_id.to_string());
            created.action_ids.push(action_id.to_string());
            created.outcome_ids.push(outcome_id.to_string());
            created.transition_ids.push(transition_id.to_string());
            Ok(())
        }
        Err(line_err) => {
            let mut failed_run = started_run;
            failed_run.finished_at_unix_ms = Some(adapter_run_terminal_timestamp_ms(
                failed_run.started_at_unix_ms,
            )?);
            failed_run.status = AdapterRunStatus::Failed;
            failed_run.error_code = Some("ADAPTER_FAILED".to_string());
            failed_run.error_message = Some(line_err.message.clone());
            failed_run.field_path = Some(line_err.field_path.clone());
            failed_run.expected_kind = Some(line_err.expected_kind.clone());
            failed_run.actual_kind = Some(line_err.actual_kind.clone());
            failed_run.refresh_content_hash()?;
            let audit = build_audit(
                "complete_adapter_run",
                AuditStatus::Failed {
                    error_code: "ADAPTER_FAILED".to_string(),
                },
                vec![event_id.to_string(), adapter_run_id.to_string()],
                vec![adapter_run_id.to_string()],
                vec![CF_DJ_ADAPTER_RUNS.to_string(), CF_DJ_AUDIT_LOG.to_string()],
                vec![failed_run.header.content_hash],
            )?;
            put_adapter_run_failure_batch(db, &failed_run, &audit)?;
            created
                .failed_adapter_run_ids
                .push(adapter_run_id.to_string());
            Err(DynamicJepaError::AdapterFailed {
                adapter_id: adapter.adapter_id.to_string(),
                event_id: event_id.0,
                message: line_err.message,
                field_path: line_err.field_path,
            })
        }
    }
}

fn run_adapter_for_event(
    db: &rocksdb::DB,
    event_id: EventId,
    created: &mut IngestCreatedIds,
) -> DynamicJepaResult<()> {
    let raw_event =
        get_raw_event(db, event_id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_RAW_EVENTS.to_string(),
            key: event_id.into_bytes().to_vec(),
        })?;
    let domain_pack = get_domain_pack(
        db,
        &raw_event.header.domain_pack_id,
        &raw_event.header.domain_pack_version,
    )?
    .ok_or_else(|| DynamicJepaError::DomainPackNotFound {
        id: raw_event.header.domain_pack_id.to_string(),
    })?;
    let adapter = domain_pack
        .adapter_specs
        .iter()
        .find(|spec| spec.adapter_id == raw_event.adapter_id)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "run-adapter.adapter",
                format!(
                    "adapter {} is not registered for domain {}",
                    raw_event.adapter_id, domain_pack.id
                ),
                "inspect the domain pack adapter registry before running the adapter",
            )
        })?
        .clone();
    let runs = list_adapter_runs(db, 1_000_000, 0)?;
    let matching: Vec<_> = runs
        .into_iter()
        .filter(|run| run.event_id == event_id)
        .collect();
    if matching.len() != 1 {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "expected exactly one adapter run for event {event_id}, found {}",
                matching.len()
            ),
        });
    }
    let started_run = matching.into_iter().next().expect("len checked");
    if started_run.status != AdapterRunStatus::Started {
        return Err(DynamicJepaError::validation(
            "run-adapter.event_id",
            format!(
                "adapter run {} for event {} is already {:?}",
                started_run.adapter_run_id, event_id, started_run.status
            ),
            "run-adapter only completes a persisted Started row; use inspect-cf to read terminal rows",
        ));
    }
    let run_id = started_run
        .header
        .source_run_id
        .unwrap_or_else(Uuid::new_v4);
    let source_uri = raw_event.source_uri.clone();
    process_existing_raw_event(
        db,
        &domain_pack,
        &adapter,
        &raw_event,
        started_run,
        run_id,
        &source_uri,
        created,
    )
}

fn process_existing_raw_event(
    db: &rocksdb::DB,
    domain_pack: &DomainPack,
    adapter: &AdapterSpec,
    raw_event: &RawDomainEvent,
    started_run: AdapterRunRecord,
    run_id: Uuid,
    _source_uri: &str,
    created: &mut IngestCreatedIds,
) -> DynamicJepaResult<()> {
    match adapt_payload(domain_pack, adapter, &raw_event.payload_bytes) {
        Ok(adapted) => {
            let state_id = StateId::new_v4();
            let action_id = ActionId::new_v4();
            let outcome_id = OutcomeId::new_v4();
            let transition_id = TransitionId::new_v4();
            let timestamp_ms = adapted_timestamp_ms(domain_pack, &adapted)?;
            let mut state = NormalizedState {
                header: DjRecordHeader::new(
                    state_id.0,
                    NORMALIZED_STATE_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                state_id,
                fields: adapted.state_fields,
                source_event_id: raw_event.event_id,
            };
            state.refresh_content_hash()?;
            let mut action = NormalizedAction {
                header: DjRecordHeader::new(
                    action_id.0,
                    NORMALIZED_ACTION_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                action_id,
                fields: adapted.action_fields,
                source_event_id: raw_event.event_id,
                action_origin: ActionOrigin::Observed,
            };
            action.refresh_content_hash()?;
            let mut outcome = NormalizedOutcome {
                header: DjRecordHeader::new(
                    outcome_id.0,
                    NORMALIZED_OUTCOME_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                outcome_id,
                fields: adapted.outcome_fields,
                source_event_id: raw_event.event_id,
            };
            outcome.refresh_content_hash()?;
            let mut transition = StateTransition {
                header: DjRecordHeader::new(
                    transition_id.0,
                    STATE_TRANSITION_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                transition_id,
                prior_state: state_id,
                action: action_id,
                outcome: outcome_id,
                next_state: state_id,
                timestamp_ms,
            };
            transition.refresh_content_hash()?;
            let mut completed_run = started_run;
            completed_run.finished_at_unix_ms = Some(adapter_run_terminal_timestamp_ms(
                completed_run.started_at_unix_ms,
            )?);
            completed_run.status = AdapterRunStatus::Completed;
            completed_run.output_state_id = Some(state_id);
            completed_run.output_action_id = Some(action_id);
            completed_run.output_outcome_id = Some(outcome_id);
            completed_run.output_transition_id = Some(transition_id);
            completed_run.refresh_content_hash()?;
            let audit = build_audit(
                "complete_adapter_run",
                AuditStatus::Ok,
                vec![raw_event.event_id.to_string()],
                vec![
                    state_id.to_string(),
                    action_id.to_string(),
                    outcome_id.to_string(),
                    transition_id.to_string(),
                ],
                vec![
                    CF_DJ_ADAPTER_RUNS.to_string(),
                    CF_DJ_NORMALIZED_STATES.to_string(),
                    CF_DJ_ACTIONS.to_string(),
                    CF_DJ_OUTCOMES.to_string(),
                    CF_DJ_TRANSITIONS.to_string(),
                    CF_DJ_AUDIT_LOG.to_string(),
                ],
                vec![
                    completed_run.header.content_hash,
                    state.header.content_hash,
                    action.header.content_hash,
                    outcome.header.content_hash,
                    transition.header.content_hash,
                ],
            )?;
            put_adapter_run_success_batch(
                db,
                &completed_run,
                &state,
                &action,
                &outcome,
                &transition,
                &audit,
            )?;
            created.state_ids.push(state_id.to_string());
            created.action_ids.push(action_id.to_string());
            created.outcome_ids.push(outcome_id.to_string());
            created.transition_ids.push(transition_id.to_string());
            Ok(())
        }
        Err(line_err) => {
            let mut failed_run = started_run;
            failed_run.finished_at_unix_ms = Some(adapter_run_terminal_timestamp_ms(
                failed_run.started_at_unix_ms,
            )?);
            failed_run.status = AdapterRunStatus::Failed;
            failed_run.error_code = Some("ADAPTER_FAILED".to_string());
            failed_run.error_message = Some(line_err.message.clone());
            failed_run.field_path = Some(line_err.field_path.clone());
            failed_run.expected_kind = Some(line_err.expected_kind.clone());
            failed_run.actual_kind = Some(line_err.actual_kind.clone());
            failed_run.refresh_content_hash()?;
            let audit = build_audit(
                "complete_adapter_run",
                AuditStatus::Failed {
                    error_code: "ADAPTER_FAILED".to_string(),
                },
                vec![raw_event.event_id.to_string()],
                vec![failed_run.adapter_run_id.to_string()],
                vec![CF_DJ_ADAPTER_RUNS.to_string(), CF_DJ_AUDIT_LOG.to_string()],
                vec![failed_run.header.content_hash],
            )?;
            put_adapter_run_failure_batch(db, &failed_run, &audit)?;
            created
                .failed_adapter_run_ids
                .push(failed_run.adapter_run_id.to_string());
            Err(DynamicJepaError::AdapterFailed {
                adapter_id: adapter.adapter_id.to_string(),
                event_id: raw_event.event_id.0,
                message: line_err.message,
                field_path: line_err.field_path,
            })
        }
    }
}

fn adapt_payload(
    domain_pack: &DomainPack,
    adapter: &AdapterSpec,
    payload_bytes: &[u8],
) -> Result<AdaptedLine, AdapterLineError> {
    let trimmed = trim_jsonl_newline(payload_bytes);
    let root: Value = serde_json::from_slice(trimmed).map_err(|err| AdapterLineError {
        message: format!("json parse failed: {err}"),
        field_path: String::new(),
        expected_kind: "valid JSON object".to_string(),
        actual_kind: "invalid_json".to_string(),
    })?;
    if !root.is_object() {
        return Err(AdapterLineError {
            message: "json_event payload must be a JSON object".to_string(),
            field_path: "$".to_string(),
            expected_kind: "object".to_string(),
            actual_kind: actual_json_kind(&root),
        });
    }
    reject_extra_json_fields(&root, adapter)?;

    let mut state_fields = BTreeMap::new();
    let mut action_fields = BTreeMap::new();
    let mut outcome_fields = BTreeMap::new();
    let mut time_fields = BTreeMap::new();

    for target_path in ordered_adapter_targets(domain_pack, adapter) {
        let source_path = adapter
            .mapping
            .get(&target_path)
            .ok_or_else(|| AdapterLineError {
                message: format!("adapter target path {target_path:?} disappeared during ordering"),
                field_path: target_path.clone(),
                expected_kind: "registered adapter mapping".to_string(),
                actual_kind: "missing".to_string(),
            })?;
        let field_spec =
            schema_field_for_path(domain_pack, &target_path).ok_or_else(|| AdapterLineError {
                message: format!("adapter target path {target_path:?} is not declared in schemas"),
                field_path: target_path.clone(),
                expected_kind: "declared schema field".to_string(),
                actual_kind: "undeclared".to_string(),
            })?;
        let value = resolve_json_path(&root, source_path).ok_or_else(|| AdapterLineError {
            message: format!("JSON path {source_path:?} did not resolve for {target_path}"),
            field_path: target_path.clone(),
            expected_kind: expected_kind_name(&field_spec.kind),
            actual_kind: "missing".to_string(),
        })?;
        let coerced = coerce_json_value(value, field_spec, &target_path)?;
        let (scope, field_name) = target_path
            .split_once('.')
            .ok_or_else(|| AdapterLineError {
                message: format!("adapter target path {target_path:?} is not dotted"),
                field_path: target_path.clone(),
                expected_kind: "scope.field".to_string(),
                actual_kind: "invalid_path".to_string(),
            })?;
        match scope {
            "state" => {
                state_fields.insert(field_name.to_string(), coerced);
            }
            "action" => {
                action_fields.insert(field_name.to_string(), coerced);
            }
            "outcome" => {
                outcome_fields.insert(field_name.to_string(), coerced);
            }
            "time" => {
                time_fields.insert(field_name.to_string(), coerced);
            }
            "entity" => {}
            _ => {
                return Err(AdapterLineError {
                    message: format!("unsupported adapter target scope {scope:?}"),
                    field_path: target_path.clone(),
                    expected_kind: "state/action/outcome/time/entity".to_string(),
                    actual_kind: scope.to_string(),
                });
            }
        }
    }

    ensure_required_schema_fields(domain_pack, "state", &state_fields)?;
    ensure_required_schema_fields(domain_pack, "action", &action_fields)?;
    ensure_required_schema_fields(domain_pack, "outcome", &outcome_fields)?;
    ensure_time_field(domain_pack, &time_fields)?;

    Ok(AdaptedLine {
        state_fields,
        action_fields,
        outcome_fields,
        time_fields,
    })
}

fn ordered_adapter_targets(domain_pack: &DomainPack, adapter: &AdapterSpec) -> Vec<String> {
    let mut ordered = Vec::new();
    for field in &domain_pack.state_schema.fields {
        let path = format!("state.{}", field.name);
        if adapter.mapping.contains_key(&path) {
            ordered.push(path);
        }
    }
    for field in &domain_pack.action_schema.fields {
        let path = format!("action.{}", field.name);
        if adapter.mapping.contains_key(&path) {
            ordered.push(path);
        }
    }
    for field in &domain_pack.outcome_schema.fields {
        let path = format!("outcome.{}", field.name);
        if adapter.mapping.contains_key(&path) {
            ordered.push(path);
        }
    }
    let time_path = format!("time.{}", domain_pack.time_schema.field.name);
    if adapter.mapping.contains_key(&time_path) {
        ordered.push(time_path);
    }
    for field in &domain_pack.entity_schema.fields {
        let path = format!("entity.{}", field.name);
        if adapter.mapping.contains_key(&path) {
            ordered.push(path);
        }
    }
    for key in adapter.mapping.keys() {
        if !ordered.iter().any(|path| path == key) {
            ordered.push(key.clone());
        }
    }
    ordered
}

fn trim_jsonl_newline(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 && matches!(bytes[end - 1], b'\n' | b'\r') {
        end -= 1;
    }
    &bytes[..end]
}

fn reject_extra_json_fields(root: &Value, adapter: &AdapterSpec) -> Result<(), AdapterLineError> {
    let expected: BTreeSet<String> = adapter.mapping.values().cloned().collect();
    let mut observed = Vec::new();
    collect_json_leaf_paths(root, "$", &mut observed);
    for path in observed {
        if !expected.contains(&path) {
            return Err(AdapterLineError {
                message: format!("unexpected JSON field {path} is not declared in adapter mapping"),
                field_path: path,
                expected_kind: "only declared adapter mapping JSON paths".to_string(),
                actual_kind: "extra_field".to_string(),
            });
        }
    }
    Ok(())
}

fn collect_json_leaf_paths(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                out.push(path.to_string());
            }
            for (key, child) in map {
                collect_json_leaf_paths(child, &format!("{path}.{key}"), out);
            }
        }
        Value::Array(_) => out.push(path.to_string()),
        _ => out.push(path.to_string()),
    }
}

fn resolve_json_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if !path.starts_with("$.") || path.contains("[") || path.contains("..") {
        return None;
    }
    let mut current = root;
    for segment in path.trim_start_matches("$.").split('.') {
        if segment.is_empty() {
            return None;
        }
        current = current.get(segment)?;
    }
    Some(current)
}

fn schema_field_for_path<'a>(domain_pack: &'a DomainPack, path: &str) -> Option<&'a FieldSpec> {
    let (scope, name) = path.split_once('.')?;
    match scope {
        "state" => domain_pack
            .state_schema
            .fields
            .iter()
            .find(|field| field.name == name),
        "action" => domain_pack
            .action_schema
            .fields
            .iter()
            .find(|field| field.name == name),
        "outcome" => domain_pack
            .outcome_schema
            .fields
            .iter()
            .find(|field| field.name == name),
        "entity" => domain_pack
            .entity_schema
            .fields
            .iter()
            .find(|field| field.name == name),
        "time" if domain_pack.time_schema.field.name == name => {
            Some(&domain_pack.time_schema.field)
        }
        _ => None,
    }
}

fn coerce_json_value(
    value: &Value,
    field: &FieldSpec,
    field_path: &str,
) -> Result<FieldValue, AdapterLineError> {
    let coerced = match &field.kind {
        FieldKind::I64 => {
            let value = value
                .as_i64()
                .ok_or_else(|| type_error(value, field, field_path))?;
            validate_numeric_bounds(value as f64, field, field_path)?;
            FieldValue::I64(value)
        }
        FieldKind::UnixMs => {
            let value = value
                .as_i64()
                .ok_or_else(|| type_error(value, field, field_path))?;
            validate_numeric_bounds(value as f64, field, field_path)?;
            FieldValue::UnixMs(value)
        }
        FieldKind::F64 => {
            let value = value
                .as_f64()
                .ok_or_else(|| type_error(value, field, field_path))?;
            validate_numeric_bounds(value, field, field_path)?;
            FieldValue::F64(value)
        }
        FieldKind::Bool => {
            let value = value
                .as_bool()
                .ok_or_else(|| type_error(value, field, field_path))?;
            FieldValue::Bool(value)
        }
        FieldKind::String => {
            let value = value
                .as_str()
                .ok_or_else(|| type_error(value, field, field_path))?;
            FieldValue::String(value.to_string())
        }
        FieldKind::Categorical { variants } => {
            let value = value
                .as_str()
                .ok_or_else(|| type_error(value, field, field_path))?;
            if !variants.iter().any(|variant| variant == value) {
                return Err(AdapterLineError {
                    message: format!("value {value:?} is not a declared variant of {field_path}"),
                    field_path: field_path.to_string(),
                    expected_kind: expected_kind_name(&field.kind),
                    actual_kind: actual_json_kind(&Value::String(value.to_string())),
                });
            }
            FieldValue::Categorical {
                variant: value.to_string(),
            }
        }
        FieldKind::Vector { dim } => {
            let array = value
                .as_array()
                .ok_or_else(|| type_error(value, field, field_path))?;
            if array.len() != *dim {
                return Err(AdapterLineError {
                    message: format!(
                        "vector field {field_path} expected dim {dim}, got {}",
                        array.len()
                    ),
                    field_path: field_path.to_string(),
                    expected_kind: format!("vector[{dim}]"),
                    actual_kind: format!("vector[{}]", array.len()),
                });
            }
            let mut out = Vec::with_capacity(*dim);
            for (idx, item) in array.iter().enumerate() {
                let value = item.as_f64().ok_or_else(|| AdapterLineError {
                    message: format!("vector field {field_path}[{idx}] is not numeric"),
                    field_path: format!("{field_path}[{idx}]"),
                    expected_kind: "f64".to_string(),
                    actual_kind: actual_json_kind(item),
                })?;
                out.push(value as f32);
            }
            FieldValue::Vector(out)
        }
    };
    coerced
        .validate(field_path)
        .map_err(|err| AdapterLineError {
            message: err.to_string(),
            field_path: field_path.to_string(),
            expected_kind: expected_kind_name(&field.kind),
            actual_kind: field_value_kind(&coerced).to_string(),
        })?;
    Ok(coerced)
}

fn type_error(value: &Value, field: &FieldSpec, field_path: &str) -> AdapterLineError {
    AdapterLineError {
        message: format!(
            "value at {field_path} has kind {}, expected {}",
            actual_json_kind(value),
            expected_kind_name(&field.kind)
        ),
        field_path: field_path.to_string(),
        expected_kind: expected_kind_name(&field.kind),
        actual_kind: actual_json_kind(value),
    }
}

fn validate_numeric_bounds(
    value: f64,
    field: &FieldSpec,
    field_path: &str,
) -> Result<(), AdapterLineError> {
    if let Some(min) = field.min {
        if value < min {
            return Err(AdapterLineError {
                message: format!("value {value} is below min {min} for {field_path}"),
                field_path: field_path.to_string(),
                expected_kind: format!(">= {min}"),
                actual_kind: "number".to_string(),
            });
        }
    }
    if let Some(max) = field.max {
        if value > max {
            return Err(AdapterLineError {
                message: format!("value {value} exceeds max {max} for {field_path}"),
                field_path: field_path.to_string(),
                expected_kind: format!("<= {max}"),
                actual_kind: "number".to_string(),
            });
        }
    }
    Ok(())
}

fn ensure_required_schema_fields(
    domain_pack: &DomainPack,
    scope: &str,
    fields: &BTreeMap<String, FieldValue>,
) -> Result<(), AdapterLineError> {
    let schema_fields = match scope {
        "state" => &domain_pack.state_schema.fields,
        "action" => &domain_pack.action_schema.fields,
        "outcome" => &domain_pack.outcome_schema.fields,
        _ => return Ok(()),
    };
    for field in schema_fields.iter().filter(|field| field.required) {
        if !fields.contains_key(&field.name) {
            return Err(AdapterLineError {
                message: format!("required field {scope}.{} was not produced", field.name),
                field_path: format!("{scope}.{}", field.name),
                expected_kind: expected_kind_name(&field.kind),
                actual_kind: "missing".to_string(),
            });
        }
    }
    Ok(())
}

fn ensure_time_field(
    domain_pack: &DomainPack,
    fields: &BTreeMap<String, FieldValue>,
) -> Result<(), AdapterLineError> {
    let field = &domain_pack.time_schema.field;
    if field.required && !fields.contains_key(&field.name) {
        return Err(AdapterLineError {
            message: format!("required field time.{} was not produced", field.name),
            field_path: format!("time.{}", field.name),
            expected_kind: expected_kind_name(&field.kind),
            actual_kind: "missing".to_string(),
        });
    }
    Ok(())
}

fn adapted_timestamp_ms(domain_pack: &DomainPack, adapted: &AdaptedLine) -> DynamicJepaResult<i64> {
    let field = &domain_pack.time_schema.field;
    let value =
        adapted
            .time_fields
            .get(&field.name)
            .ok_or_else(|| DynamicJepaError::AdapterFailed {
                adapter_id: "json_event".to_string(),
                event_id: Uuid::nil(),
                message: format!("time field {} missing after adapter success", field.name),
                field_path: format!("time.{}", field.name),
            })?;
    match value {
        FieldValue::I64(value) | FieldValue::UnixMs(value) => Ok(*value),
        other => Err(DynamicJepaError::AdapterFailed {
            adapter_id: "json_event".to_string(),
            event_id: Uuid::nil(),
            message: format!(
                "time field {} must be i64/unix_ms, got {}",
                field.name,
                field_value_kind(other)
            ),
            field_path: format!("time.{}", field.name),
        }),
    }
}

fn actual_json_kind(value: &Value) -> String {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(number) if number.as_i64().is_some() => "i64",
        Value::Number(_) => "f64",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
    .to_string()
}

fn expected_kind_name(kind: &FieldKind) -> String {
    match kind {
        FieldKind::I64 => "i64".to_string(),
        FieldKind::F64 => "f64".to_string(),
        FieldKind::Bool => "bool".to_string(),
        FieldKind::String => "string".to_string(),
        FieldKind::UnixMs => "unix_ms".to_string(),
        FieldKind::Categorical { variants } => format!("categorical{{{}}}", variants.join(",")),
        FieldKind::Vector { dim } => format!("vector[{dim}]"),
    }
}

fn field_value_kind(value: &FieldValue) -> &'static str {
    match value {
        FieldValue::I64(_) => "i64",
        FieldValue::F64(_) => "f64",
        FieldValue::Bool(_) => "bool",
        FieldValue::String(_) => "string",
        FieldValue::Categorical { .. } => "categorical",
        FieldValue::Vector(_) => "vector",
        FieldValue::UnixMs(_) => "unix_ms",
    }
}

fn verify_ingest_created_ids(
    db: &rocksdb::DB,
    created: &IngestCreatedIds,
) -> DynamicJepaResult<()> {
    for id in &created.raw_event_ids {
        let id = parse_event_id(id, "verify_ingest.raw_event_id")?;
        get_raw_event(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_RAW_EVENTS.to_string(),
            key: id.into_bytes().to_vec(),
        })?;
    }
    for id in &created.adapter_run_ids {
        let id = parse_uuid(id, "verify_ingest.adapter_run_id")?;
        get_adapter_run(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_ADAPTER_RUNS.to_string(),
            key: id.as_bytes().to_vec(),
        })?;
    }
    for id in &created.failed_adapter_run_ids {
        let id = parse_uuid(id, "verify_ingest.failed_adapter_run_id")?;
        let run =
            get_adapter_run(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_ADAPTER_RUNS.to_string(),
                key: id.as_bytes().to_vec(),
            })?;
        if run.status != AdapterRunStatus::Failed {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!("adapter run {id} was expected Failed, got {:?}", run.status),
            });
        }
    }
    for id in &created.state_ids {
        let id = parse_state_id(id, "verify_ingest.state_id")?;
        get_normalized_state(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_NORMALIZED_STATES.to_string(),
            key: id.into_bytes().to_vec(),
        })?;
    }
    for id in &created.action_ids {
        let id = parse_action_id(id, "verify_ingest.action_id")?;
        get_action(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_ACTIONS.to_string(),
            key: id.into_bytes().to_vec(),
        })?;
    }
    for id in &created.outcome_ids {
        let id = parse_outcome_id(id, "verify_ingest.outcome_id")?;
        get_outcome(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_OUTCOMES.to_string(),
            key: id.into_bytes().to_vec(),
        })?;
    }
    for id in &created.transition_ids {
        let id = parse_transition_id(id, "verify_ingest.transition_id")?;
        get_transition(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_TRANSITIONS.to_string(),
            key: id.into_bytes().to_vec(),
        })?;
    }
    Ok(())
}

fn materialize_one_panel(
    db: &rocksdb::DB,
    domain_pack: &DomainPack,
    transition: StateTransition,
) -> DynamicJepaResult<MaterializedPanelEvidence> {
    let state = get_normalized_state(db, transition.prior_state)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_NORMALIZED_STATES.to_string(),
            key: transition.prior_state.into_bytes().to_vec(),
        }
    })?;
    let action = get_action(db, transition.action)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_ACTIONS.to_string(),
            key: transition.action.into_bytes().to_vec(),
        }
    })?;
    let outcome = get_outcome(db, transition.outcome)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_OUTCOMES.to_string(),
            key: transition.outcome.into_bytes().to_vec(),
        }
    })?;

    let mut readings = Vec::with_capacity(domain_pack.instrument_specs.len());
    let mut reading_ids = Vec::with_capacity(domain_pack.instrument_specs.len());
    let mut slots = Vec::with_capacity(domain_pack.instrument_specs.len());
    let mut vectors = Vec::with_capacity(domain_pack.instrument_specs.len());
    let mut masks = Vec::with_capacity(domain_pack.instrument_specs.len());
    for spec in &domain_pack.instrument_specs {
        let slot = panel_slot_for_spec(spec)?;
        match execute_instrument(domain_pack, spec, &state, &action, &outcome, &transition) {
            Ok(output) => {
                let reading_id = Uuid::new_v4();
                let mut reading = InstrumentReading {
                    header: DjRecordHeader::new(
                        reading_id,
                        INSTRUMENT_READING_RECORD_VERSION,
                        domain_pack.id.clone(),
                        domain_pack.version.clone(),
                        now_unix_ms()?,
                        transition.header.source_run_id,
                    ),
                    reading_id,
                    event_id: state.source_event_id,
                    instrument_id: spec.instrument_id.clone(),
                    instrument_hash: instrument_hash(spec),
                    input_hash: output.input_hash,
                    output_dense: output.values.clone(),
                    status: ReadingStatus::Ok,
                };
                reading.refresh_content_hash()?;
                reading_ids.push(reading_id);
                slots.push(slot);
                vectors.push(output.values);
                masks.push(true);
                readings.push(reading);
            }
            Err(err) if !spec.required => {
                let reading_id = Uuid::new_v4();
                let error_message = err.to_string();
                let failure_hash = sha256(
                    format!(
                        "{}:{}:{error_message}",
                        spec.instrument_id, state.source_event_id
                    )
                    .as_bytes(),
                );
                let mut reading = InstrumentReading {
                    header: DjRecordHeader::new(
                        reading_id,
                        INSTRUMENT_READING_RECORD_VERSION,
                        domain_pack.id.clone(),
                        domain_pack.version.clone(),
                        now_unix_ms()?,
                        transition.header.source_run_id,
                    ),
                    reading_id,
                    event_id: state.source_event_id,
                    instrument_id: spec.instrument_id.clone(),
                    instrument_hash: instrument_hash(spec),
                    input_hash: failure_hash,
                    output_dense: Vec::new(),
                    status: ReadingStatus::Failed {
                        error: error_message,
                    },
                };
                reading.refresh_content_hash()?;
                reading_ids.push(reading_id);
                slots.push(slot);
                vectors.push(Vec::new());
                masks.push(false);
                readings.push(reading);
            }
            Err(err) => {
                let audit = build_audit(
                    "materialize_panel",
                    AuditStatus::Failed {
                        error_code: err.code().to_string(),
                    },
                    vec![transition.transition_id.to_string()],
                    Vec::new(),
                    vec![CF_DJ_AUDIT_LOG.to_string()],
                    Vec::new(),
                )?;
                put_audit_record(db, &audit)?;
                return Err(err);
            }
        }
    }

    let panel_id = PanelId::new_v4();
    let panel_hash = panel_hash(domain_pack, &reading_ids, &slots, &vectors)?;
    let mut panel = LatentPanel {
        header: DjRecordHeader::new(
            panel_id.0,
            LATENT_PANEL_RECORD_VERSION,
            domain_pack.id.clone(),
            domain_pack.version.clone(),
            now_unix_ms()?,
            transition.header.source_run_id,
        ),
        panel_id,
        event_id: state.source_event_id,
        state_id: state.state_id,
        action_id: action.action_id,
        outcome_id: Some(outcome.outcome_id),
        instrument_reading_ids: reading_ids.clone(),
        ordered_slots: slots,
        slot_vectors: vectors,
        slot_masks: masks,
        panel_hash,
        materializer_version: 1,
    };
    panel.refresh_content_hash()?;
    let audit = build_audit(
        "materialize_panel",
        AuditStatus::Ok,
        vec![transition.transition_id.to_string()],
        std::iter::once(panel_id.to_string())
            .chain(reading_ids.iter().map(Uuid::to_string))
            .collect(),
        vec![
            CF_DJ_INSTRUMENT_READINGS.to_string(),
            CF_DJ_LATENT_PANELS.to_string(),
            CF_DJ_AUDIT_LOG.to_string(),
        ],
        std::iter::once(panel.header.content_hash)
            .chain(readings.iter().map(|reading| reading.header.content_hash))
            .collect(),
    )?;
    put_panel_with_readings_batch(db, &readings, &panel, &audit)?;

    let stored_panel =
        get_latent_panel(db, panel_id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_LATENT_PANELS.to_string(),
            key: panel_id.into_bytes().to_vec(),
        })?;
    if stored_panel != panel {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!("latent panel readback mismatch for panel {panel_id}"),
        });
    }
    for reading in &readings {
        let stored_reading = get_instrument_reading(db, reading.event_id, &reading.instrument_id)?
            .ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_INSTRUMENT_READINGS.to_string(),
                key: instrument_reading_key(reading.event_id, &reading.instrument_id).to_vec(),
            })?;
        if stored_reading != *reading {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "instrument reading readback mismatch for event {} instrument {}",
                    reading.event_id, reading.instrument_id
                ),
            });
        }
    }
    Ok(MaterializedPanelEvidence {
        panel_id: panel_id.to_string(),
        event_id: state.source_event_id.to_string(),
        transition_id: transition.transition_id.to_string(),
        instrument_reading_ids: reading_ids.iter().map(Uuid::to_string).collect(),
    })
}

struct InstrumentOutput {
    values: Vec<f32>,
    input_hash: [u8; 32],
}

fn execute_instrument(
    domain_pack: &DomainPack,
    spec: &InstrumentSpec,
    state: &NormalizedState,
    action: &NormalizedAction,
    outcome: &NormalizedOutcome,
    transition: &StateTransition,
) -> DynamicJepaResult<InstrumentOutput> {
    let inputs = spec
        .input_fields
        .iter()
        .map(|path| {
            resolve_field_value(domain_pack, path, state, action, outcome, transition)
                .map(|value| (path.clone(), value))
        })
        .collect::<DynamicJepaResult<Vec<_>>>()?;
    let input_hash = sha256(&serde_json::to_vec(&inputs).map_err(|err| {
        DynamicJepaError::validation(
            "instrument.input_hash",
            format!("failed to serialize instrument inputs: {err}"),
            "instrument inputs must be JSON serializable for deterministic hashing",
        )
    })?);
    let values = match &spec.kind {
        InstrumentKind::Scalar => {
            if inputs.len() != 1 {
                return Err(instrument_failed(
                    spec,
                    state.source_event_id,
                    "scalar instrument requires exactly one input field",
                    spec.input_fields.join(","),
                ));
            }
            let raw = numeric_field_value(&inputs[0].1).ok_or_else(|| {
                instrument_failed(
                    spec,
                    state.source_event_id,
                    format!(
                        "scalar input {} is {} not numeric",
                        inputs[0].0,
                        field_value_kind(&inputs[0].1)
                    ),
                    inputs[0].0.clone(),
                )
            })?;
            validate_instrument_numeric(spec, raw, &inputs[0].0, state.source_event_id)?;
            vec![normalize_value(raw, &spec.normalization)? as f32]
        }
        InstrumentKind::Categorical { variants } => {
            if inputs.len() != 1 {
                return Err(instrument_failed(
                    spec,
                    state.source_event_id,
                    "categorical instrument requires exactly one input field",
                    spec.input_fields.join(","),
                ));
            }
            let variant = match &inputs[0].1 {
                FieldValue::Categorical { variant } | FieldValue::String(variant) => variant,
                other => {
                    return Err(instrument_failed(
                        spec,
                        state.source_event_id,
                        format!(
                            "categorical input {} is {} not categorical/string",
                            inputs[0].0,
                            field_value_kind(other)
                        ),
                        inputs[0].0.clone(),
                    ));
                }
            };
            let idx = variants
                .iter()
                .position(|item| item == variant)
                .ok_or_else(|| {
                    instrument_failed(
                        spec,
                        state.source_event_id,
                        format!("variant {variant:?} is not declared"),
                        inputs[0].0.clone(),
                    )
                })?;
            onehot(variants.len(), idx)?
        }
        InstrumentKind::Onehot { dim } => {
            let idx = compound_onehot_index(*dim, &inputs, spec, state.source_event_id)?;
            onehot(*dim, idx)?
        }
        InstrumentKind::Time { encoding } => {
            if inputs.len() != 1 {
                return Err(instrument_failed(
                    spec,
                    state.source_event_id,
                    "time instrument requires exactly one input field",
                    spec.input_fields.join(","),
                ));
            }
            let raw = numeric_field_value(&inputs[0].1).ok_or_else(|| {
                instrument_failed(
                    spec,
                    state.source_event_id,
                    format!(
                        "time input {} is {} not numeric",
                        inputs[0].0,
                        field_value_kind(&inputs[0].1)
                    ),
                    inputs[0].0.clone(),
                )
            })?;
            validate_instrument_numeric(spec, raw, &inputs[0].0, state.source_event_id)?;
            match encoding {
                TimeEncoding::UnixSeconds => vec![(raw / 1000.0) as f32],
                TimeEncoding::FractionalDayOfWeek => {
                    let seconds_in_week = 7.0 * 24.0 * 3600.0;
                    vec![((raw / 1000.0) % seconds_in_week / seconds_in_week) as f32]
                }
            }
        }
    };
    let expected_dim = spec.output_shape.iter().product::<usize>();
    if values.len() != expected_dim {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: domain_pack.id.to_string(),
            expected: vec![expected_dim],
            actual: vec![values.len()],
        });
    }
    for (idx, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(instrument_failed(
                spec,
                state.source_event_id,
                format!("output value {idx} is not finite: {value}"),
                spec.input_fields.join(","),
            ));
        }
    }
    Ok(InstrumentOutput { values, input_hash })
}

fn resolve_field_value(
    domain_pack: &DomainPack,
    path: &str,
    state: &NormalizedState,
    action: &NormalizedAction,
    outcome: &NormalizedOutcome,
    transition: &StateTransition,
) -> DynamicJepaResult<FieldValue> {
    let (scope, name) = path.split_once('.').ok_or_else(|| {
        DynamicJepaError::validation(
            "instrument.input_fields",
            format!("input field {path:?} is not dotted"),
            "use state/action/outcome/time dotted instrument input fields",
        )
    })?;
    let value = match scope {
        "state" => state.fields.get(name).cloned(),
        "action" => action.fields.get(name).cloned(),
        "outcome" => outcome.fields.get(name).cloned(),
        "time" if domain_pack.time_schema.field.name == name => {
            match domain_pack.time_schema.field.kind {
                FieldKind::UnixMs => Some(FieldValue::UnixMs(transition.timestamp_ms)),
                _ => Some(FieldValue::I64(transition.timestamp_ms)),
            }
        }
        _ => None,
    };
    value.ok_or_else(|| DynamicJepaError::InstrumentFailed {
        instrument_id: "unknown".to_string(),
        event_id: state.source_event_id.0,
        message: format!("input field {path:?} missing from normalized records"),
        field_path: path.to_string(),
    })
}

fn numeric_field_value(value: &FieldValue) -> Option<f64> {
    match value {
        FieldValue::I64(value) | FieldValue::UnixMs(value) => Some(*value as f64),
        FieldValue::F64(value) => Some(*value),
        _ => None,
    }
}

fn validate_instrument_numeric(
    spec: &InstrumentSpec,
    value: f64,
    field_path: &str,
    event_id: EventId,
) -> DynamicJepaResult<()> {
    if spec.validation.require_finite && !value.is_finite() {
        return Err(instrument_failed(
            spec,
            event_id,
            format!("numeric input {field_path} is not finite: {value}"),
            field_path.to_string(),
        ));
    }
    if spec.validation.reject_nan && value.is_nan() {
        return Err(instrument_failed(
            spec,
            event_id,
            format!("numeric input {field_path} is NaN"),
            field_path.to_string(),
        ));
    }
    if spec.validation.reject_inf && value.is_infinite() {
        return Err(instrument_failed(
            spec,
            event_id,
            format!("numeric input {field_path} is infinite"),
            field_path.to_string(),
        ));
    }
    if let Some(min) = spec.validation.min {
        if value < min {
            return Err(instrument_failed(
                spec,
                event_id,
                format!("numeric input {field_path}={value} is below min {min}"),
                field_path.to_string(),
            ));
        }
    }
    if let Some(max) = spec.validation.max {
        if value > max {
            return Err(instrument_failed(
                spec,
                event_id,
                format!("numeric input {field_path}={value} exceeds max {max}"),
                field_path.to_string(),
            ));
        }
    }
    Ok(())
}

fn normalize_value(value: f64, normalization: &Normalization) -> DynamicJepaResult<f64> {
    match normalization {
        Normalization::None => Ok(value),
        Normalization::StandardScore { mean, std } => {
            if *std <= 0.0 || !std.is_finite() {
                return Err(DynamicJepaError::validation(
                    "instrument.normalization.std",
                    format!("std must be finite and > 0, got {std}"),
                    "fix the registered domain pack normalization",
                ));
            }
            Ok((value - *mean) / *std)
        }
    }
}

fn compound_onehot_index(
    dim: usize,
    inputs: &[(String, FieldValue)],
    spec: &InstrumentSpec,
    event_id: EventId,
) -> DynamicJepaResult<usize> {
    if inputs.len() == 1 {
        let value = numeric_field_value(&inputs[0].1).ok_or_else(|| {
            instrument_failed(
                spec,
                event_id,
                format!("onehot input {} is not numeric", inputs[0].0),
                inputs[0].0.clone(),
            )
        })?;
        let idx = value as i64;
        if idx < 0 || idx as usize >= dim || (idx as f64 - value).abs() > f64::EPSILON {
            return Err(instrument_failed(
                spec,
                event_id,
                format!("onehot index {value} is outside 0..{}", dim - 1),
                inputs[0].0.clone(),
            ));
        }
        return Ok(idx as usize);
    }
    if inputs.len() == 2 {
        let side = (dim as f64).sqrt() as usize;
        if side * side != dim {
            return Err(instrument_failed(
                spec,
                event_id,
                format!("compound two-field onehot dim {dim} is not a square"),
                spec.input_fields.join(","),
            ));
        }
        let x = numeric_field_value(&inputs[0].1).ok_or_else(|| {
            instrument_failed(
                spec,
                event_id,
                format!("onehot input {} is not numeric", inputs[0].0),
                inputs[0].0.clone(),
            )
        })? as i64;
        let y = numeric_field_value(&inputs[1].1).ok_or_else(|| {
            instrument_failed(
                spec,
                event_id,
                format!("onehot input {} is not numeric", inputs[1].0),
                inputs[1].0.clone(),
            )
        })? as i64;
        if x < 0 || y < 0 || x as usize >= side || y as usize >= side {
            return Err(instrument_failed(
                spec,
                event_id,
                format!(
                    "compound onehot coordinate ({x},{y}) is outside 0..{}",
                    side - 1
                ),
                spec.input_fields.join(","),
            ));
        }
        return Ok(x as usize * side + y as usize);
    }
    Err(instrument_failed(
        spec,
        event_id,
        format!(
            "onehot instrument supports one index input or two coordinate inputs, got {}",
            inputs.len()
        ),
        spec.input_fields.join(","),
    ))
}

fn onehot(dim: usize, idx: usize) -> DynamicJepaResult<Vec<f32>> {
    if idx >= dim {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: "onehot".to_string(),
            expected: vec![dim],
            actual: vec![idx + 1],
        });
    }
    let mut values = vec![0.0f32; dim];
    values[idx] = 1.0;
    Ok(values)
}

fn instrument_failed(
    spec: &InstrumentSpec,
    event_id: EventId,
    message: impl Into<String>,
    field_path: impl Into<String>,
) -> DynamicJepaError {
    DynamicJepaError::InstrumentFailed {
        instrument_id: spec.instrument_id.to_string(),
        event_id: event_id.0,
        message: message.into(),
        field_path: field_path.into(),
    }
}

fn panel_slot_for_spec(spec: &InstrumentSpec) -> DynamicJepaResult<PanelSlot> {
    let first = spec.input_fields.first().ok_or_else(|| {
        DynamicJepaError::validation(
            "InstrumentSpec.input_fields",
            "instrument must have at least one input field",
            "fix the registered domain pack instrument spec",
        )
    })?;
    let kind = if first.starts_with("state.") {
        PanelSlotKind::State
    } else if first.starts_with("action.") {
        PanelSlotKind::Action
    } else if first.starts_with("outcome.") {
        PanelSlotKind::Outcome
    } else if first.starts_with("time.") {
        PanelSlotKind::Time
    } else {
        return Err(DynamicJepaError::validation(
            "InstrumentSpec.input_fields",
            format!("unsupported panel slot input prefix in {first:?}"),
            "use state/action/outcome/time input fields for Phase 4 panels",
        ));
    };
    Ok(PanelSlot {
        instrument_id: spec.instrument_id.clone(),
        dim: spec.output_shape.iter().product(),
        kind,
    })
}

fn instrument_hash(spec: &InstrumentSpec) -> [u8; 16] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(spec.instrument_id.as_str().as_bytes());
    bytes.push(0);
    bytes.push(spec.version);
    bytes.push(0);
    bytes.extend_from_slice(format!("{:?}", spec.output_shape).as_bytes());
    context_graph_storage::dynamicjepa::keys::blake3_128(&bytes)
}

fn panel_hash(
    domain_pack: &DomainPack,
    reading_ids: &[Uuid],
    slots: &[PanelSlot],
    vectors: &[Vec<f32>],
) -> DynamicJepaResult<[u8; 32]> {
    let mut sorted_reading_ids = reading_ids.iter().map(Uuid::to_string).collect::<Vec<_>>();
    sorted_reading_ids.sort();
    let bytes = serde_json::to_vec(&json!({
        "domain_pack_id": domain_pack.id.to_string(),
        "reading_ids": sorted_reading_ids,
        "ordered_slots": slots,
        "slot_vectors": vectors,
    }))
    .map_err(|err| {
        DynamicJepaError::validation(
            "panel_hash",
            format!("failed to serialize panel hash payload: {err}"),
            "panel hash payload must remain JSON serializable",
        )
    })?;
    Ok(sha256(&bytes))
}

fn panel_for_transition(
    db: &rocksdb::DB,
    transition: &StateTransition,
) -> DynamicJepaResult<Option<LatentPanel>> {
    Ok(list_latent_panels(db, 1_000_000, 0)?
        .into_iter()
        .find(|panel| {
            panel.state_id == transition.prior_state
                && panel.action_id == transition.action
                && panel.outcome_id == Some(transition.outcome)
        }))
}

fn phase4_ingest_decoded_records(db: &rocksdb::DB) -> DynamicJepaResult<Value> {
    Ok(json!({
        CF_DJ_RAW_EVENTS: inspect_cf(db, CF_DJ_RAW_EVENTS, 20, 0)?,
        CF_DJ_ADAPTER_RUNS: inspect_cf(db, CF_DJ_ADAPTER_RUNS, 20, 0)?,
        CF_DJ_NORMALIZED_STATES: inspect_cf(db, CF_DJ_NORMALIZED_STATES, 20, 0)?,
        CF_DJ_ACTIONS: inspect_cf(db, CF_DJ_ACTIONS, 20, 0)?,
        CF_DJ_OUTCOMES: inspect_cf(db, CF_DJ_OUTCOMES, 20, 0)?,
        CF_DJ_TRANSITIONS: inspect_cf(db, CF_DJ_TRANSITIONS, 20, 0)?,
        CF_DJ_AUDIT_LOG: inspect_cf(db, CF_DJ_AUDIT_LOG, 40, 0)?,
    }))
}

fn phase4_panel_decoded_records(db: &rocksdb::DB) -> DynamicJepaResult<Value> {
    Ok(json!({
        CF_DJ_INSTRUMENT_READINGS: inspect_cf(db, CF_DJ_INSTRUMENT_READINGS, 40, 0)?,
        CF_DJ_LATENT_PANELS: inspect_cf(db, CF_DJ_LATENT_PANELS, 40, 0)?,
        CF_DJ_AUDIT_LOG: inspect_cf(db, CF_DJ_AUDIT_LOG, 80, 0)?,
    }))
}

fn enrich_phase4_error_value(
    value: &mut Value,
    before_counts: &BTreeMap<String, u64>,
    after_counts: &BTreeMap<String, u64>,
    created_ids: Value,
    decoded_records: Value,
) -> DynamicJepaResult<()> {
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "count_delta".to_string(),
            json!(count_delta(before_counts, after_counts)?),
        );
        object.insert("created_ids".to_string(), created_ids);
        object.insert("decoded_records".to_string(), decoded_records);
    }
    Ok(())
}

fn build_audit(
    operation: impl Into<String>,
    status: AuditStatus,
    input_ids: Vec<String>,
    output_ids: Vec<String>,
    cfs_touched: Vec<String>,
    content_hashes: Vec<[u8; 32]>,
) -> DynamicJepaResult<DjAuditRecord> {
    let timestamp_unix_nanos = now_unix_ms()? as u64 * 1_000_000;
    let record = DjAuditRecord {
        audit_id: Uuid::new_v4(),
        timestamp_unix_nanos,
        operation: operation.into(),
        actor: "cli".to_string(),
        input_ids,
        output_ids,
        cfs_touched,
        content_hashes,
        status,
        verification_run_id: None,
    };
    record.validate()?;
    Ok(record)
}

fn parse_uuid(value: &str, field: &str) -> DynamicJepaResult<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        DynamicJepaError::validation(
            field,
            format!("invalid UUID {value:?}: {err}"),
            "pass an id returned by a prior DynamicJEPA command or decoded source-of-truth row",
        )
    })
}

fn parse_event_id(value: &str, field: &str) -> DynamicJepaResult<EventId> {
    Ok(EventId(parse_uuid(value, field)?))
}

fn parse_state_id(value: &str, field: &str) -> DynamicJepaResult<StateId> {
    Ok(StateId(parse_uuid(value, field)?))
}

fn parse_action_id(value: &str, field: &str) -> DynamicJepaResult<ActionId> {
    Ok(ActionId(parse_uuid(value, field)?))
}

fn parse_outcome_id(value: &str, field: &str) -> DynamicJepaResult<OutcomeId> {
    Ok(OutcomeId(parse_uuid(value, field)?))
}

fn parse_transition_id(value: &str, field: &str) -> DynamicJepaResult<TransitionId> {
    Ok(TransitionId(parse_uuid(value, field)?))
}

fn parse_panel_id(value: &str, field: &str) -> DynamicJepaResult<PanelId> {
    Ok(PanelId(parse_uuid(value, field)?))
}

fn parse_trajectory_id(value: &str, field: &str) -> DynamicJepaResult<TrajectoryId> {
    Ok(TrajectoryId(parse_uuid(value, field)?))
}

fn parse_dataset_id(value: &str, field: &str) -> DynamicJepaResult<DatasetId> {
    Ok(DatasetId(parse_uuid(value, field)?))
}

fn parse_dataset_shard_id(value: &str, field: &str) -> DynamicJepaResult<DatasetShardId> {
    Ok(DatasetShardId(parse_uuid(value, field)?))
}

fn parse_training_run_id(value: &str, field: &str) -> DynamicJepaResult<TrainingRunId> {
    Ok(TrainingRunId(parse_uuid(value, field)?))
}

fn parse_model_artifact_id(value: &str, field: &str) -> DynamicJepaResult<ModelArtifactId> {
    Ok(ModelArtifactId(parse_uuid(value, field)?))
}

fn parse_prediction_id(value: &str, field: &str) -> DynamicJepaResult<PredictionId> {
    Ok(PredictionId(parse_uuid(value, field)?))
}

fn parse_skill_id(value: &str, field: &str) -> DynamicJepaResult<SkillId> {
    Ok(SkillId(parse_uuid(value, field)?))
}

fn parse_plan_trace_id(value: &str, field: &str) -> DynamicJepaResult<PlanTraceId> {
    Ok(PlanTraceId(parse_uuid(value, field)?))
}

fn parse_surprise_event_id(value: &str, field: &str) -> DynamicJepaResult<SurpriseEventId> {
    Ok(SurpriseEventId(parse_uuid(value, field)?))
}

fn parse_hex_key(value: &str, field: &str) -> DynamicJepaResult<Vec<u8>> {
    let text = value.trim();
    if text.is_empty() || text.len() % 2 != 0 {
        return Err(DynamicJepaError::validation(
            field,
            format!("hex key must have a non-empty even number of characters, got {text:?}"),
            "copy the key_hex value from dynamicjepa inspect-cf",
        ));
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    let chars = text.as_bytes();
    for idx in (0..chars.len()).step_by(2) {
        let high = hex_nibble(chars[idx]).ok_or_else(|| {
            DynamicJepaError::validation(
                field,
                format!("invalid hex byte at offset {idx} in {text:?}"),
                "copy the key_hex value from dynamicjepa inspect-cf",
            )
        })?;
        let low = hex_nibble(chars[idx + 1]).ok_or_else(|| {
            DynamicJepaError::validation(
                field,
                format!("invalid hex byte at offset {} in {text:?}", idx + 1),
                "copy the key_hex value from dynamicjepa inspect-cf",
            )
        })?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_domain_pack_file(path: &Path) -> DynamicJepaResult<DomainPack> {
    let bytes = fs::read(path).map_err(|err| {
        DynamicJepaError::validation(
            "domain_pack.file",
            format!("failed to read domain pack TOML {}: {err}", path.display()),
            "provide an existing TOML file under configs/dynamicjepa/domain_packs",
        )
    })?;
    let source_hash = sha256(&bytes);
    let text = std::str::from_utf8(&bytes).map_err(|err| {
        DynamicJepaError::validation(
            "domain_pack.file",
            format!("domain pack TOML is not UTF-8: {err}"),
            "save the domain pack TOML as UTF-8 text",
        )
    })?;
    let value: toml::Value = toml::from_str(text).map_err(|err| {
        DynamicJepaError::validation(
            "domain_pack.toml",
            format!("TOML parse failed: {err}"),
            "fix the TOML syntax before registration; no partial registry rows are written",
        )
    })?;
    reject_env_substitution(&value, "domain_pack")?;

    let parsed: DomainPackToml = toml::from_str(text).map_err(|err| {
        DynamicJepaError::validation(
            "domain_pack.toml",
            format!("strict TOML schema parse failed: {err}"),
            "remove unknown keys or extend the parser intentionally before registering this pack",
        )
    })?;
    let domain_id = DomainPackId::new(parsed.domain.id)?;
    let version = parsed.domain.version;
    let header = DjRecordHeader::new(
        Uuid::new_v4(),
        DOMAIN_PACK_RECORD_VERSION,
        domain_id.clone(),
        version.clone(),
        now_unix_ms()?,
        Some(Uuid::new_v4()),
    );
    let candidate_actions = candidate_actions_from_toml(parsed.planner_policy.candidate_actions)?;
    let mut pack = DomainPack {
        header,
        id: domain_id,
        version,
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
            .map(|(idx, spec)| instrument_from_toml(spec, idx))
            .collect::<DynamicJepaResult<Vec<_>>>()?,
        adapter_specs: parsed
            .adapters
            .into_iter()
            .map(adapter_from_toml)
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
    };
    pack.refresh_content_hash()?;
    pack.validate_record()?;
    Ok(pack)
}

fn instrument_from_toml(spec: InstrumentToml, idx: usize) -> DynamicJepaResult<InstrumentSpec> {
    Ok(InstrumentSpec {
        instrument_id: InstrumentId::new(spec.id)?,
        kind: instrument_kind_from_toml(spec.kind, &format!("instruments[{idx}].kind"))?,
        input_fields: spec.input_fields,
        output_shape: spec.output_shape,
        normalization: normalization_from_toml(
            spec.normalization,
            &format!("instruments[{idx}].normalization"),
        )?,
        required: spec.required,
        model_ref: spec.model_ref,
        version: spec.version,
        validation: InstrumentValidation {
            require_finite: spec.validation.require_finite,
            min: spec.validation.min,
            max: spec.validation.max,
            reject_nan: spec.validation.reject_nan,
            reject_inf: spec.validation.reject_inf,
        },
    })
}

fn adapter_from_toml(adapter: AdapterToml) -> DynamicJepaResult<AdapterSpec> {
    Ok(AdapterSpec {
        adapter_id: AdapterId::new(adapter.id)?,
        kind: adapter.kind,
        version: adapter.version,
        mapping: adapter.mapping,
    })
}

fn field_spec_from_toml(field: FieldToml, path: &str) -> DynamicJepaResult<FieldSpec> {
    Ok(FieldSpec {
        name: field.name,
        kind: field_kind_from_toml(field.kind, &format!("{path}.kind"))?,
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

fn field_kind_from_toml(kind: TomlKind, field: &str) -> DynamicJepaResult<FieldKind> {
    match kind {
        TomlKind::Name(name) => match name.as_str() {
            "i64" => Ok(FieldKind::I64),
            "f64" => Ok(FieldKind::F64),
            "bool" => Ok(FieldKind::Bool),
            "string" => Ok(FieldKind::String),
            "unix_ms" => Ok(FieldKind::UnixMs),
            other => Err(toml_validation_error(
                field,
                format!("unsupported field kind name {other:?}"),
            )),
        },
        TomlKind::Table(table) => match table.kind_type.as_str() {
            "i64" => Ok(FieldKind::I64),
            "f64" => Ok(FieldKind::F64),
            "bool" => Ok(FieldKind::Bool),
            "string" => Ok(FieldKind::String),
            "unix_ms" => Ok(FieldKind::UnixMs),
            "categorical" => Ok(FieldKind::Categorical {
                variants: table.variants.ok_or_else(|| {
                    toml_validation_error(field, "categorical field kind missing variants")
                })?,
            }),
            "vector" => Ok(FieldKind::Vector {
                dim: table
                    .dim
                    .ok_or_else(|| toml_validation_error(field, "vector field kind missing dim"))?,
            }),
            other => Err(toml_validation_error(
                field,
                format!("unsupported field kind table type {other:?}"),
            )),
        },
    }
}

fn instrument_kind_from_toml(kind: TomlKind, field: &str) -> DynamicJepaResult<InstrumentKind> {
    match kind {
        TomlKind::Name(name) => match name.as_str() {
            "scalar" => Ok(InstrumentKind::Scalar),
            other => Err(toml_validation_error(
                field,
                format!("unsupported instrument kind name {other:?}"),
            )),
        },
        TomlKind::Table(table) => match table.kind_type.as_str() {
            "scalar" => Ok(InstrumentKind::Scalar),
            "categorical" => Ok(InstrumentKind::Categorical {
                variants: table.variants.ok_or_else(|| {
                    toml_validation_error(field, "categorical instrument kind missing variants")
                })?,
            }),
            "onehot" => Ok(InstrumentKind::Onehot {
                dim: table.dim.ok_or_else(|| {
                    toml_validation_error(field, "onehot instrument kind missing dim")
                })?,
            }),
            "time" => {
                let encoding = match table.encoding.as_deref() {
                    Some("fractional_day_of_week") => TimeEncoding::FractionalDayOfWeek,
                    Some("unix_seconds") => TimeEncoding::UnixSeconds,
                    other => {
                        return Err(toml_validation_error(
                            field,
                            format!("unsupported time encoding {other:?}"),
                        ));
                    }
                };
                Ok(InstrumentKind::Time { encoding })
            }
            other => Err(toml_validation_error(
                field,
                format!("unsupported instrument kind table type {other:?}"),
            )),
        },
    }
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
                .ok_or_else(|| toml_validation_error(field, "standard_score missing mean"))?,
            std: value
                .std
                .ok_or_else(|| toml_validation_error(field, "standard_score missing std"))?,
        }),
        other => Err(toml_validation_error(
            field,
            format!("unsupported normalization type {other:?}"),
        )),
    }
}

fn candidate_actions_from_toml(
    value: CandidateActionsToml,
) -> DynamicJepaResult<CandidateActionConfig> {
    if value.kind != "enumerated" {
        return Err(toml_validation_error(
            "planner_policy.candidate_actions.kind",
            format!("unsupported candidate action kind {:?}", value.kind),
        ));
    }
    match (value.deltas, value.moves) {
        (Some(deltas), None) => Ok(CandidateActionConfig::EnumeratedDeltas { deltas }),
        (None, Some(moves)) => Ok(CandidateActionConfig::EnumeratedMoves { moves }),
        (Some(_), Some(_)) => Err(toml_validation_error(
            "planner_policy.candidate_actions",
            "declare deltas or moves, not both",
        )),
        (None, None) => Err(toml_validation_error(
            "planner_policy.candidate_actions",
            "enumerated candidate actions require deltas or moves",
        )),
    }
}

fn reject_env_substitution(value: &toml::Value, path: &str) -> DynamicJepaResult<()> {
    match value {
        toml::Value::String(text) if text.contains("${") => Err(DynamicJepaError::validation(
            path,
            "environment-variable substitution is forbidden in domain pack string fields",
            "write literal deterministic values and bump the domain pack version if values change",
        )),
        toml::Value::Array(items) => {
            for (idx, item) in items.iter().enumerate() {
                reject_env_substitution(item, &format!("{path}[{idx}]"))?;
            }
            Ok(())
        }
        toml::Value::Table(table) => {
            for (key, item) in table {
                reject_env_substitution(item, &format!("{path}.{key}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn open_store(path: &Path) -> DynamicJepaResult<RocksDbTeleologicalStore> {
    RocksDbTeleologicalStore::open_with_config(path, TeleologicalStoreConfig::default()).map_err(
        |err| DynamicJepaError::Storage {
            operation: "open_dynamicjepa_db".to_string(),
            cf: "rocksdb".to_string(),
            message: format!("failed to open {}: {err}", path.display()),
            remediation: "verify the DB path is a writable demo path and no active process owns the RocksDB lock".to_string(),
        },
    )
}

fn canonical_existing_path(path: &Path, field: &str) -> DynamicJepaResult<String> {
    fs::canonicalize(path)
        .map_err(|err| {
            DynamicJepaError::validation(
                field,
                format!("failed to canonicalize {}: {err}", path.display()),
                "use an existing path; DB paths are created when opened, input files must already exist",
            )
        })
        .map(|path| path.display().to_string())
}

fn resolve_dynamic_cf_name(name: &str) -> DynamicJepaResult<&'static str> {
    DJ_CF_NAMES
        .iter()
        .copied()
        .find(|candidate| *candidate == name)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "inspect-cf.cf",
                format!("unknown DynamicJEPA column family {name:?}"),
                format!("use one of: {}", DJ_CF_NAMES.join(", ")),
            )
        })
}

fn count_delta(
    before: &BTreeMap<String, u64>,
    after: &BTreeMap<String, u64>,
) -> DynamicJepaResult<BTreeMap<String, i64>> {
    let mut delta = BTreeMap::new();
    for (cf, before_count) in before {
        let after_count =
            after
                .get(cf)
                .copied()
                .ok_or_else(|| DynamicJepaError::StorageInvariantViolation {
                    message: format!("after-count snapshot missing DynamicJEPA column family {cf}"),
                })?;
        delta.insert(cf.clone(), after_count as i64 - *before_count as i64);
    }
    for cf in after.keys() {
        if !before.contains_key(cf) {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "after-count snapshot contained unexpected DynamicJEPA column family {cf}"
                ),
            });
        }
    }
    Ok(delta)
}

fn error_value(
    operation: &'static str,
    db_path: Option<String>,
    source_cfs: &[&'static str],
    before_counts: Option<BTreeMap<String, u64>>,
    after_counts: Option<BTreeMap<String, u64>>,
    err: &DynamicJepaError,
) -> Value {
    let mut object = Map::new();
    object.insert("operation".to_string(), json!(operation));
    object.insert("status".to_string(), json!("error"));
    if let Some(db_path) = db_path {
        object.insert("db_path".to_string(), json!(db_path));
    }
    if !source_cfs.is_empty() {
        object.insert("source_of_truth".to_string(), json!(source_cfs));
    }
    if let Some(before_counts) = before_counts {
        object.insert("before_counts".to_string(), json!(before_counts));
    }
    if let Some(after_counts) = after_counts {
        object.insert("after_counts".to_string(), json!(after_counts));
    }
    object.insert("error_code".to_string(), json!(err.code()));
    object.insert("error_message".to_string(), json!(err.to_string()));
    if let Some(remediation) = remediation(err) {
        object.insert("remediation".to_string(), json!(remediation));
    }
    match err {
        DynamicJepaError::Validation { field, .. }
        | DynamicJepaError::SchemaValidationFailed { field, .. } => {
            object.insert("field".to_string(), json!(field));
        }
        DynamicJepaError::Storage { operation, cf, .. } => {
            object.insert("storage_operation".to_string(), json!(operation));
            object.insert("cf".to_string(), json!(cf));
        }
        DynamicJepaError::Codec {
            expected,
            actual,
            payload_type,
            ..
        } => {
            object.insert("expected_version".to_string(), json!(expected));
            object.insert("actual_version".to_string(), json!(actual));
            object.insert("payload_type".to_string(), json!(payload_type));
        }
        DynamicJepaError::SourceOfTruthMissing { cf, key } => {
            object.insert("cf".to_string(), json!(cf));
            object.insert("key_hex".to_string(), json!(hex_bytes(key)));
        }
        DynamicJepaError::AdapterFailed {
            adapter_id,
            event_id,
            field_path,
            ..
        } => {
            object.insert("adapter_id".to_string(), json!(adapter_id));
            object.insert("event_id".to_string(), json!(event_id.to_string()));
            object.insert("field_path".to_string(), json!(field_path));
        }
        DynamicJepaError::InstrumentFailed {
            instrument_id,
            event_id,
            field_path,
            ..
        } => {
            object.insert("instrument_id".to_string(), json!(instrument_id));
            object.insert("event_id".to_string(), json!(event_id.to_string()));
            object.insert("field_path".to_string(), json!(field_path));
        }
        DynamicJepaError::BindingEvidenceMissing {
            binding_id,
            evidence_ref,
        } => {
            object.insert("binding_id".to_string(), json!(binding_id.to_string()));
            object.insert("evidence_ref".to_string(), json!(evidence_ref));
        }
        DynamicJepaError::TrajectoryInvariantViolation { trajectory_id, .. } => {
            object.insert(
                "trajectory_id".to_string(),
                json!(trajectory_id.to_string()),
            );
        }
        DynamicJepaError::DatasetLeakageDetected { dataset_id, .. } => {
            object.insert("dataset_id".to_string(), json!(dataset_id.to_string()));
        }
        DynamicJepaError::TrainingFailed {
            training_run_id, ..
        } => {
            object.insert(
                "training_run_id".to_string(),
                json!(training_run_id.to_string()),
            );
        }
        DynamicJepaError::ArtifactHashMismatch {
            artifact_id,
            file,
            expected,
            actual,
        } => {
            object.insert("artifact_id".to_string(), json!(artifact_id.to_string()));
            object.insert("file".to_string(), json!(file));
            object.insert("expected".to_string(), json!(expected));
            object.insert("actual".to_string(), json!(actual));
            object.insert("wrote_prediction".to_string(), json!(false));
            object.insert("wrote_plan_trace".to_string(), json!(false));
            object.insert("wrote_surprise".to_string(), json!(false));
        }
        DynamicJepaError::PredictionInputMissing { target_id, cf } => {
            object.insert("target_id".to_string(), json!(target_id));
            object.insert("cf".to_string(), json!(cf));
            object.insert("wrote_prediction".to_string(), json!(false));
            object.insert("wrote_plan_trace".to_string(), json!(false));
        }
        DynamicJepaError::GuardRejected {
            guard_id,
            reason_code,
            action_id,
        } => {
            object.insert("guard_id".to_string(), json!(guard_id));
            object.insert("reason_code".to_string(), json!(reason_code));
            object.insert("action_id".to_string(), json!(action_id.to_string()));
        }
        DynamicJepaError::DomainPackVersionMismatch {
            id,
            expected,
            actual,
        } => {
            object.insert("id".to_string(), json!(id));
            object.insert("expected".to_string(), json!(expected));
            object.insert("actual".to_string(), json!(actual));
        }
        _ => {}
    }
    Value::Object(object)
}

fn remediation(err: &DynamicJepaError) -> Option<String> {
    match err {
        DynamicJepaError::Validation { remediation, .. }
        | DynamicJepaError::SchemaValidationFailed { remediation, .. }
        | DynamicJepaError::Storage { remediation, .. }
        | DynamicJepaError::Codec { remediation, .. }
        | DynamicJepaError::TrainingFailed { remediation, .. } => Some(remediation.clone()),
        DynamicJepaError::SourceOfTruthMissing { .. } => Some(
            "inspect DynamicJEPA counts and the referenced CF; rerun the prior command from a fresh demo DB if the row was never written"
                .to_string(),
        ),
        DynamicJepaError::StorageInvariantViolation { .. } => Some(
            "inspect the decoded source-of-truth rows and wipe/rebuild the pre-production demo DB if the invariant cannot be reconciled"
                .to_string(),
        ),
        DynamicJepaError::DomainPackNotFound { .. } => {
            Some("register the domain pack before running this command".to_string())
        }
        DynamicJepaError::BindingEvidenceMissing { .. } => Some(
            "read the referenced source-of-truth CF/key with inspect-cf before binding; no binding rows are written when evidence is missing"
                .to_string(),
        ),
        DynamicJepaError::TrajectoryInvariantViolation { .. } => Some(
            "inspect dj_transitions and dj_latent_panels; trajectory compilation requires strict ordering and one decoded panel per transition"
                .to_string(),
        ),
        DynamicJepaError::DatasetLeakageDetected { .. } => Some(
            "inspect the dataset shard inputs, target panels, negative panels, and split evidence; no dataset shard row is written when leakage or insufficient negatives is detected"
                .to_string(),
        ),
        DynamicJepaError::ArtifactHashMismatch { .. } => Some(
            "recompute the artifact files from the registry path; rerun training from a fresh artifact root if any file differs"
                .to_string(),
        ),
        DynamicJepaError::PredictionInputMissing { .. } => Some(
            "inspect the referenced artifact, panel, and action source-of-truth rows before prediction or planning"
                .to_string(),
        ),
        DynamicJepaError::GuardRejected { .. } => Some(
            "inspect dj_guard_decisions for the rejected action and choose a candidate within the registered bounds"
                .to_string(),
        ),
        _ => None,
    }
}

fn print_json(value: &Value) -> Result<(), serde_json::Error> {
    let text = serde_json::to_string_pretty(value)?;
    println!("{text}");
    Ok(())
}

fn now_unix_ms() -> DynamicJepaResult<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            DynamicJepaError::validation(
                "system_clock",
                format!("system clock is before Unix epoch: {err}"),
                "fix host clock before writing timestamped DynamicJEPA records",
            )
        })?;
    Ok(duration.as_millis() as i64)
}

fn adapter_run_terminal_timestamp_ms(started_at_unix_ms: i64) -> DynamicJepaResult<i64> {
    let finished_at_unix_ms = now_unix_ms()?;
    Ok(finished_at_unix_ms.max(started_at_unix_ms))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn toml_validation_error(field: impl Into<String>, message: impl Into<String>) -> DynamicJepaError {
    DynamicJepaError::validation(
        field,
        message,
        "fix the strict domain pack TOML source of truth; Phase 3 registration does not substitute defaults",
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
struct DomainToml {
    id: String,
    version: String,
    title: String,
    schema_version: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimeToml {
    field: FieldToml,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaToml {
    fields: Vec<FieldToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
    Table(KindTableToml),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KindTableToml {
    #[serde(rename = "type")]
    kind_type: String,
    variants: Option<Vec<String>>,
    dim: Option<usize>,
    encoding: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstrumentToml {
    id: String,
    kind: TomlKind,
    input_fields: Vec<String>,
    output_shape: Vec<usize>,
    normalization: NormalizationToml,
    required: bool,
    model_ref: Option<String>,
    version: u8,
    validation: InstrumentValidationToml,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NormalizationToml {
    #[serde(rename = "type")]
    norm_type: String,
    mean: Option<f64>,
    std: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstrumentValidationToml {
    require_finite: bool,
    min: Option<f64>,
    max: Option<f64>,
    reject_nan: bool,
    reject_inf: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterToml {
    id: String,
    kind: String,
    version: u8,
    mapping: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObjectiveToml {
    id: String,
    kind: String,
    input_panel: String,
    target: String,
    loss_weight: f64,
    required_dataset_fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvariantToml {
    id: String,
    expression: String,
    severity: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatasetPolicyToml {
    default_split: String,
    split_key: Option<String>,
    split_buckets: Option<BTreeMap<String, f64>>,
    default_objective: String,
    negative_sampling: String,
    min_negatives_per_row: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlannerPolicyToml {
    candidate_actions: CandidateActionsToml,
    guards: Vec<String>,
    surprise_threshold: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateActionsToml {
    kind: String,
    deltas: Option<Vec<i64>>,
    moves: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
