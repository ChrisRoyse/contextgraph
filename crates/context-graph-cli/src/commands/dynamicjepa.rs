//! DynamicJEPA CLI: domain-pack registry, physical inspection, adapter/panel
//! materialization, binding, trajectory, and dataset compilation.
//!
//! Mutating commands write to the real RocksDB DynamicJEPA column families, then
//! immediately read the source-of-truth rows back through separate storage
//! inspection paths.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use clap::{Args, Subcommand};
use context_graph_core::dynamicjepa::{
    ActionId, ActionOrigin, ActionSchema, AdapterId, AdapterRunRecord, AdapterRunStatus,
    AdapterSpec, ArtifactHashCheck, BindingId, BindingKind, BindingMethod, BindingRecord,
    BindingRef, CandidateActionConfig, ConstellationCentroid, ConstellationConfig,
    ConstellationGlobalSpec, ConstellationId, ConstellationModalities, ConstellationSubjectSpec,
    DatasetId, DatasetPolicy, DatasetShardId, DatasetShardRecord, DjJsonValue, DjRecordHeader,
    DomainPack, DomainPackId, DynamicJepaError, DynamicJepaRecord, DynamicJepaResult,
    EntitySchema, EventId, FieldKind, FieldSpec, FieldValue, InstrumentId, InstrumentKind,
    InstrumentReading, InstrumentSpec, InstrumentValidation, InvariantSpec, LatentPanel,
    LeakageReport, ModelArtifactId, Normalization, NormalizedAction, NormalizedOutcome,
    NormalizedState, ObjectiveSpec, OutcomeId, OutcomeSchema, PairKindBitset, PairKindName,
    PairwiseReading, PairwiseReadingId, PanelId, PanelSlot, PanelSlotKind, PayloadFormat,
    PlanTraceId, PlannerPolicy, PredictionId, RawDomainEvent, ReadingStatus, ShapeSummary,
    SkillId, SourceKind, StateId, StateSchema, StateTransition, SurpriseEventId,
    ThresholdCalibration, ThresholdCalibrationId, TimeEncoding, TimeSchema, TrainingRunId,
    TrajectoryId, TrajectoryRecord, TransitionId, Validate, VerificationPolicy,
    VerificationRunId, VerificationRunRecord, VerificationStatus, ADAPTER_RUN_RECORD_VERSION,
    BINDING_RECORD_VERSION, CONSTELLATION_CENTROID_RECORD_VERSION, DATASET_SHARD_RECORD_VERSION,
    DOMAIN_PACK_RECORD_VERSION, INSTRUMENT_READING_RECORD_VERSION, LATENT_PANEL_RECORD_VERSION,
    NORMALIZED_ACTION_RECORD_VERSION, NORMALIZED_OUTCOME_RECORD_VERSION,
    NORMALIZED_STATE_RECORD_VERSION, PAIRWISE_READING_RECORD_VERSION,
    RAW_DOMAIN_EVENT_RECORD_VERSION, STATE_TRANSITION_RECORD_VERSION,
    THRESHOLD_CALIBRATION_RECORD_VERSION, TRAJECTORY_RECORD_VERSION,
};
use context_graph_dynamicjepa::{
    plan_next_action, predict_next_panel, record_surprise, train_dataset, verify_artifact_files,
    TrainConfig,
};
use context_graph_storage::dynamicjepa::column_families::{
    CF_DJ_ACTIONS, CF_DJ_ADAPTER_REGISTRY, CF_DJ_ADAPTER_RUNS, CF_DJ_AUDIT_LOG, CF_DJ_BINDINGS,
    CF_DJ_BINDINGS_BY_ENTITY, CF_DJ_CONSTELLATIONS, CF_DJ_DATASET_SHARDS, CF_DJ_DOMAIN_PACKS,
    CF_DJ_DOMAIN_PACK_BY_NAME_VERSION, CF_DJ_GUARD_DECISIONS, CF_DJ_INSTRUMENT_READINGS,
    CF_DJ_INSTRUMENT_REGISTRY, CF_DJ_LATENT_PANELS, CF_DJ_MODEL_ARTIFACTS,
    CF_DJ_NORMALIZED_STATES, CF_DJ_OUTCOMES, CF_DJ_PAIRWISE_READINGS, CF_DJ_PLAN_TRACES,
    CF_DJ_PREDICTIONS, CF_DJ_RAW_EVENTS, CF_DJ_SKILL_POLICIES, CF_DJ_SURPRISE_EVENTS,
    CF_DJ_THRESHOLD_CALIBRATIONS, CF_DJ_TRAINING_RUNS, CF_DJ_TRAJECTORIES, CF_DJ_TRANSITIONS,
    CF_DJ_VERIFICATION_RUNS,
    DJ_CF_NAMES,
};
use context_graph_storage::dynamicjepa::keys::{
    adapter_registry_key, domain_pack_storage_uuid, instrument_reading_key,
    instrument_registry_key, name_version_key, uuid_key,
};
use context_graph_storage::dynamicjepa::{
    binding_entity_index_exists, get_action, get_adapter_run, get_adapter_spec, get_binding,
    get_constellation, get_dataset_shard, get_domain_pack, get_domain_pack_by_storage_id,
    get_guard_decision, get_instrument_reading, get_instrument_spec, get_latent_panel,
    get_model_artifact, get_normalized_state, get_outcome, get_plan_trace, get_prediction,
    get_raw_event, get_surprise_event, get_threshold_calibration, get_training_run,
    get_trajectory, get_transition, get_verification_run, inspect_cf, inspect_cf_key,
    list_adapter_runs, list_audit_records, list_bindings, list_constellations, list_dataset_shards,
    list_domain_packs, list_instrument_readings_for_event, list_latent_panels,
    list_normalized_states, list_pairwise_readings_for_event, list_threshold_calibrations,
    list_trajectories, list_transitions, put_adapter_run_failure_batch,
    put_adapter_run_success_batch, put_audit_record, put_binding_with_audit_batch,
    put_constellations_with_audit_batch, put_dataset_shard_with_audit_batch, put_domain_pack,
    put_panel_with_readings_and_pairwise_batch, put_raw_event_and_adapter_run_started,
    put_threshold_calibrations_with_audit_batch, put_trajectory_with_audit_batch,
    put_verification_run_with_audit_batch, snapshot_dj_counts,
};
use context_graph_storage::dynamicjepa::{
    signal_yield_for_operation, AuditStatus, DjAuditRecord, SignalYieldDimensions,
};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tracing::{error, info};
use uuid::Uuid;
use zip::ZipArchive;

const DYNAMICJEPA_BUNDLE_SCHEMA_VERSION: u32 = 2;
const BUNDLE_SCHEMA_MANIFESTS: &[&str] = &["run_manifest.json", "aggregate_manifest.json"];

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
    /// Build immutable constellation centroids from persisted instrument readings.
    BuildConstellation(BuildConstellationArgs),
    /// List persisted constellation centroids.
    ListConstellations(ListConstellationsArgs),
    /// Read one persisted constellation centroid.
    GetConstellation(GetConstellationArgs),
    /// Calibrate G_tau thresholds from a disjoint held-out event set.
    CalibrateThreshold(CalibrateThresholdArgs),
    /// Supersede an existing G_tau threshold calibration.
    RecalibrateThreshold(RecalibrateThresholdArgs),
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
    /// Run bundled career_taxonomy end-to-end research-smoke full-state verification.
    VerifyCareerTaxonomy(VerifyDomainArgs),
    /// Build one immutable 5090 ME-JEPA research-smoke bundle from real verifier runs.
    ResearchSmoke(ResearchSmokeArgs),
    /// Run immutable research-smoke cells across one hyperparameter axis and verify the sweep bundle.
    HpSweep(HpSweepArgs),
    /// Aggregate immutable paper-small research-smoke bundles across real training seeds.
    MultiseedAggregate(MultiseedAggregateArgs),
    /// Capture a checked-in CI reference from an existing research-smoke bundle.
    CaptureReference(CaptureReferenceArgs),
    /// Compare a research-smoke bundle against a checked-in CI reference.
    CheckReference(CheckReferenceArgs),
    /// Compare a multi-seed release aggregate against the checked-in release reference.
    CheckReleaseAggregateReference(CheckReleaseAggregateReferenceArgs),
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
            Self::BuildConstellation(_) => "dynamicjepa_build_constellation",
            Self::ListConstellations(_) => "dynamicjepa_list_constellations",
            Self::GetConstellation(_) => "dynamicjepa_get_constellation",
            Self::CalibrateThreshold(_) => "dynamicjepa_calibrate_threshold",
            Self::RecalibrateThreshold(_) => "dynamicjepa_recalibrate_threshold",
            Self::GetPrediction(_) => "dynamicjepa_get_prediction",
            Self::GetPlanTrace(_) => "dynamicjepa_get_plan_trace",
            Self::GetSurprise(_) => "dynamicjepa_get_surprise",
            Self::VerifyCounterWorld(_) => "dynamicjepa_verify_counter_world",
            Self::VerifyGridworld(_) => "dynamicjepa_verify_gridworld",
            Self::VerifyCareerTaxonomy(_) => "dynamicjepa_verify_career_taxonomy",
            Self::ResearchSmoke(_) => "dynamicjepa_research_smoke",
            Self::HpSweep(_) => "dynamicjepa_hp_sweep",
            Self::MultiseedAggregate(_) => "dynamicjepa_multiseed_aggregate",
            Self::CaptureReference(_) => "dynamicjepa_capture_reference",
            Self::CheckReference(_) => "dynamicjepa_check_reference",
            Self::CheckReleaseAggregateReference(_) => {
                "dynamicjepa_check_release_aggregate_reference"
            }
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

    /// Return an explicit zero-count before-state when the DB path has not been created yet.
    #[arg(long = "allow-missing", hide = true)]
    pub allow_missing: bool,

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
pub struct BuildConstellationArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Registered domain pack id.
    #[arg(long)]
    pub domain: String,

    /// Registered domain pack version.
    #[arg(long = "domain-version", default_value = "1.0.0")]
    pub domain_version: String,

    /// Subject id from [[constellation.subjects]], or global.
    #[arg(long, default_value = "global")]
    pub subject: String,

    /// Event selector for the reference set, for example first:30 or last:50.
    #[arg(long = "source-event-selector")]
    pub source_event_selector: Option<String>,

    /// Optional verification/build run UUID to stamp on centroid rows.
    #[arg(long = "built-by-run-id")]
    pub built_by_run_id: Option<String>,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ListConstellationsArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Optional domain pack id filter.
    #[arg(long)]
    pub domain: Option<String>,

    /// Optional subject id filter.
    #[arg(long)]
    pub subject: Option<String>,

    /// Maximum decoded rows to return.
    #[arg(long, default_value_t = 100)]
    pub limit: usize,

    /// Number of decoded rows to skip after filtering.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetConstellationArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Registered domain pack id.
    #[arg(long)]
    pub domain: String,

    /// Registered domain pack version.
    #[arg(long = "domain-version", default_value = "1.0.0")]
    pub domain_version: String,

    /// Subject id from [[constellation.subjects]], or global.
    #[arg(long, default_value = "global")]
    pub subject: String,

    /// Instrument id or 1-based modality ordinal.
    #[arg(long)]
    pub modality: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct CalibrateThresholdArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Registered domain pack id.
    #[arg(long)]
    pub domain: String,

    /// Registered domain pack version.
    #[arg(long = "domain-version", default_value = "1.0.0")]
    pub domain_version: String,

    /// Subject id from [[constellation.subjects]], or global.
    #[arg(long, default_value = "global")]
    pub subject: String,

    /// Instrument id, 1-based modality ordinal, or all configured modalities.
    #[arg(long, default_value = "all")]
    pub modality: String,

    /// Held-out event selector, for example last:30.
    #[arg(long = "calibration-event-selector")]
    pub calibration_event_selector: Option<String>,

    /// Override percentile. When omitted, the domain pack constellation spec is used.
    #[arg(long)]
    pub percentile: Option<u8>,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RecalibrateThresholdArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Registered domain pack id.
    #[arg(long)]
    pub domain: String,

    /// Registered domain pack version.
    #[arg(long = "domain-version", default_value = "1.0.0")]
    pub domain_version: String,

    /// Subject id from [[constellation.subjects]], or global.
    #[arg(long, default_value = "global")]
    pub subject: String,

    /// Instrument id, 1-based modality ordinal, or all configured modalities.
    #[arg(long, default_value = "all")]
    pub modality: String,

    /// Held-out event selector, for example last:30.
    #[arg(long = "calibration-event-selector")]
    pub calibration_event_selector: Option<String>,

    /// Calibration UUID being superseded. Required for recalibration.
    #[arg(long)]
    pub supersedes: Option<String>,

    /// Operator-visible recalibration reason. Required for recalibration.
    #[arg(long)]
    pub reason: Option<String>,

    /// Override percentile. When omitted, the prior/domain spec percentile is used.
    #[arg(long)]
    pub percentile: Option<u8>,

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

#[derive(Args, Debug, Clone)]
pub struct ResearchSmokeArgs {
    /// Stable run id. Must contain only ASCII letters, digits, dot, dash, or underscore.
    #[arg(long = "run-id")]
    pub run_id: Option<String>,

    /// Root under which the immutable research bundle directory is created.
    #[arg(long = "run-root", default_value = "tmp/mejepa_research_runs")]
    pub run_root: PathBuf,

    /// Root containing domain_packs/ and verification/ fixtures.
    #[arg(long = "fixtures-root", default_value = "configs/dynamicjepa")]
    pub fixtures_root: PathBuf,

    /// Optional RocksDB root. Defaults to <run_dir>/dbs for a self-contained smoke bundle.
    #[arg(long = "db-root")]
    pub db_root: Option<PathBuf>,

    /// Optional artifact root. Defaults to <run_dir>/artifacts.
    #[arg(long = "artifact-root")]
    pub artifact_root: Option<PathBuf>,

    /// Research run purpose: smoke, paper_small, or paper_full.
    #[arg(long, default_value = "smoke")]
    pub purpose: String,

    /// Optional O*NET 30.2 text ZIP. When present, the career benchmark fixture
    /// is generated from this real public taxonomy source inside the bundle.
    #[arg(long = "onet-text-zip")]
    pub onet_text_zip: Option<PathBuf>,

    /// Number of O*NET-derived career events to generate when --onet-text-zip is supplied.
    #[arg(long = "career-event-count")]
    pub career_event_count: Option<u64>,

    /// Training seed to write into generated O*NET career train configs.
    #[arg(long = "career-training-seed")]
    pub career_training_seed: Option<u64>,

    /// Training seed to write into generated career ablation train configs.
    /// When omitted, ablations use the checked-in career train-config seed as a fixed control.
    #[arg(long = "career-ablation-training-seed")]
    pub career_ablation_training_seed: Option<u64>,

    /// Override generated O*NET career training learning rate. Used by hp-sweep; never silently ignored.
    #[arg(long = "hp-learning-rate", hide = true)]
    pub hp_learning_rate: Option<f64>,

    /// Override generated O*NET career latent dimension. Used by hp-sweep; never silently ignored.
    #[arg(long = "hp-latent-dim", hide = true)]
    pub hp_latent_dim: Option<u64>,

    /// Override generated O*NET career training batch size. Used by hp-sweep; never silently ignored.
    #[arg(long = "hp-batch-size", hide = true)]
    pub hp_batch_size: Option<u64>,

    /// Override generated O*NET career training epochs. Used by hp-sweep; never silently ignored.
    #[arg(long = "hp-epochs", hide = true)]
    pub hp_epochs: Option<u64>,

    /// Record the upstream sweep or promotion source for generated hyperparameters.
    #[arg(long = "hyperparameter-origin", hide = true)]
    pub hyperparameter_origin: Option<String>,

    /// Local system specification document to hash into the research bundle.
    #[arg(long = "system-specs", default_value = "docs2/systemspecs.md")]
    pub system_specs: PathBuf,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct HpSweepArgs {
    /// Strict training config used as the centerpoint for non-varied hyperparameters.
    #[arg(long = "base-config")]
    pub base_config: PathBuf,

    /// Hyperparameter axis to vary: lr, latent_dim, or batch_size.
    #[arg(long = "sweep-axis")]
    pub sweep_axis: String,

    /// Comma-delimited values for the selected axis.
    #[arg(long = "sweep-values", value_delimiter = ',')]
    pub sweep_values: Vec<String>,

    /// Fresh sweep bundle directory. Existing paths are rejected.
    #[arg(long = "output-root")]
    pub output_root: PathBuf,

    /// Root containing domain_packs/ and verification/ fixtures.
    #[arg(long = "fixtures-root", default_value = "configs/dynamicjepa")]
    pub fixtures_root: PathBuf,

    /// Research run purpose for each cell: smoke, paper_small, or paper_full.
    #[arg(long, default_value = "paper_small")]
    pub purpose: String,

    /// Real O*NET 30.2 text ZIP. HP sweeps do not run against mock data.
    #[arg(long = "onet-text-zip")]
    pub onet_text_zip: PathBuf,

    /// Number of O*NET-derived career events to generate in each cell.
    #[arg(long = "career-event-count")]
    pub career_event_count: Option<u64>,

    /// One training seed for each sweep value.
    #[arg(long = "career-training-seed")]
    pub career_training_seed: Option<u64>,

    /// Comma-delimited training seeds. Produces values x seeds immutable cells.
    #[arg(long = "career-training-seeds", value_delimiter = ',')]
    pub career_training_seeds: Vec<u64>,

    /// Local system specification document to hash into each child research bundle.
    #[arg(long = "system-specs", default_value = "docs2/systemspecs.md")]
    pub system_specs: PathBuf,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct MultiseedAggregateArgs {
    /// Root containing immutable child research-smoke bundle directories.
    #[arg(long = "bundle-root", default_value = "tmp/mejepa_research_runs")]
    pub bundle_root: PathBuf,

    /// Shell-style child bundle name pattern under --bundle-root. Supports '*' only.
    #[arg(long = "bundle-glob")]
    pub bundle_glob: Option<String>,

    /// Explicit child bundle directory. May be repeated; relative paths resolve from cwd.
    #[arg(long = "bundle")]
    pub bundles: Vec<PathBuf>,

    /// Fresh aggregate bundle directory to create. Existing paths are rejected.
    #[arg(long = "output-root")]
    pub output_root: PathBuf,

    /// Required number of child bundles matched before aggregation.
    #[arg(long = "expected-bundle-count")]
    pub expected_bundle_count: Option<usize>,

    /// Minimum number of passed child bundles required for aggregate statistics.
    #[arg(long = "min-passed", default_value_t = 3)]
    pub min_passed: usize,

    /// Expected research-smoke purpose for every child bundle.
    #[arg(long = "expected-purpose", default_value = "paper_small")]
    pub expected_purpose: String,

    /// Expected O*NET-derived career event count for every child bundle.
    #[arg(long = "expected-career-event-count")]
    pub expected_career_event_count: Option<u64>,

    /// Required career training seed set, comma-delimited. Failed bundles count as attempted seeds.
    #[arg(long = "required-career-training-seeds", value_delimiter = ',')]
    pub required_career_training_seeds: Vec<u64>,

    /// BCa bootstrap iteration count for confidence intervals.
    #[arg(long = "bootstrap-iters", default_value_t = 10000)]
    pub bootstrap_iters: u64,

    /// Deterministic seed for aggregate resampling.
    #[arg(long = "bootstrap-seed", default_value_t = 20260430)]
    pub bootstrap_seed: u64,

    /// Confidence level for aggregate intervals.
    #[arg(long = "confidence", default_value_t = 0.95)]
    pub confidence: f64,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CaptureReferenceArgs {
    /// Existing research-smoke bundle directory to capture as a CI reference.
    #[arg(long = "bundle")]
    pub bundle: PathBuf,

    /// Output JSON reference path. Parent directories are created.
    #[arg(long = "output")]
    pub output: PathBuf,

    /// Stable reference identifier written into the JSON.
    #[arg(long = "reference-id")]
    pub reference_id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CheckReferenceArgs {
    /// Research-smoke bundle directory to compare.
    #[arg(long = "bundle")]
    pub bundle: PathBuf,

    /// Checked-in JSON reference file.
    #[arg(long = "reference")]
    pub reference: PathBuf,

    /// Checked-in tolerance JSON file.
    #[arg(long = "tolerances")]
    pub tolerances: PathBuf,

    /// Require artifact registry hashes to match the reference exactly.
    #[arg(long = "require-artifact-hash-match")]
    pub require_artifact_hash_match: bool,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CheckReleaseAggregateReferenceArgs {
    /// Multi-seed aggregate bundle directory to compare.
    #[arg(long = "aggregate")]
    pub aggregate: PathBuf,

    /// Checked-in release aggregate reference JSON file.
    #[arg(long = "reference")]
    pub reference: PathBuf,

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
        DynamicJepaCommands::BuildConstellation(args) => build_constellation_command(args),
        DynamicJepaCommands::ListConstellations(args) => list_constellations_command(args),
        DynamicJepaCommands::GetConstellation(args) => get_constellation_command(args),
        DynamicJepaCommands::CalibrateThreshold(args) => calibrate_threshold_command(args, false),
        DynamicJepaCommands::RecalibrateThreshold(args) => recalibrate_threshold_command(args),
        DynamicJepaCommands::GetPrediction(args) => get_prediction_command(args),
        DynamicJepaCommands::GetPlanTrace(args) => get_plan_trace_command(args),
        DynamicJepaCommands::GetSurprise(args) => get_surprise_command(args),
        DynamicJepaCommands::VerifyCounterWorld(args) => verify_counter_world_command(args),
        DynamicJepaCommands::VerifyGridworld(args) => verify_gridworld_command(args),
        DynamicJepaCommands::VerifyCareerTaxonomy(args) => verify_career_taxonomy_command(args),
        DynamicJepaCommands::ResearchSmoke(args) => research_smoke_command(args),
        DynamicJepaCommands::HpSweep(args) => hp_sweep_command(args),
        DynamicJepaCommands::MultiseedAggregate(args) => multiseed_aggregate_command(args),
        DynamicJepaCommands::CaptureReference(args) => capture_reference_command(args),
        DynamicJepaCommands::CheckReference(args) => check_reference_command(args),
        DynamicJepaCommands::CheckReleaseAggregateReference(args) => {
            check_release_aggregate_reference_command(args)
        }
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
    let store = open_existing_store(&args.db, "dynamicjepa_list_domains")?;
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
    if args.allow_missing && !args.db.exists() {
        let counts = zero_dj_counts();
        return Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_inspect_counts",
                "status": "ok",
                "db_path": args.db,
                "db_exists": false,
                "source_of_truth": DJ_CF_NAMES,
                "counts": counts,
                "after_counts": counts,
            }),
            exit_code: 0,
        });
    }
    let store = open_existing_store(&args.db, "dynamicjepa_inspect_counts")?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "inspect-counts.db")?;
    let counts = snapshot_dj_counts(db)?;
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_inspect_counts",
            "status": "ok",
            "db_path": db_path,
            "db_exists": true,
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
    let store = open_existing_store(&args.db, "dynamicjepa_inspect_cf")?;
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
        CF_DJ_PAIRWISE_READINGS,
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
                    "source_of_truth": [CF_DJ_LATENT_PANELS, CF_DJ_INSTRUMENT_READINGS, CF_DJ_PAIRWISE_READINGS],
                    "counts": counts,
                    "decoded_records": {
                        "panel": panel,
                        "instrument_readings": readings,
                        "pairwise_readings": list_pairwise_readings_for_event(db, panel.event_id)?,
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

fn build_constellation_command(args: BuildConstellationArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_DOMAIN_PACKS,
        CF_DJ_NORMALIZED_STATES,
        CF_DJ_INSTRUMENT_READINGS,
        CF_DJ_CONSTELLATIONS,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "build-constellation.db")?;
    let before_counts = snapshot_dj_counts(db)?;

    let result = (|| -> DynamicJepaResult<Value> {
        let domain_id = DomainPackId::new(args.domain.clone())?;
        let domain = get_domain_pack(db, &domain_id, &args.domain_version)?.ok_or_else(|| {
            DynamicJepaError::DomainPackNotFound {
                id: domain_id.to_string(),
            }
        })?;
        let target = resolve_constellation_target(&domain, &args.subject)?;
        let all_matching = matching_constellation_states(db, &domain, &target)?;
        let selector = match args.source_event_selector.as_deref() {
            Some(raw) => parse_event_selector(raw, "build-constellation.source_event_selector")?,
            None => default_reference_selector(all_matching.len(), target.calibration_set_size),
        };
        let reference_states = apply_event_selector(all_matching, selector);
        if reference_states.len() < 5 {
            return Err(DynamicJepaError::ConstellationReferenceSetTooSmall {
                subject_id: target.subject_id.clone(),
                actual: reference_states.len(),
                minimum: 5,
            });
        }
        let built_by_run_id = args
            .built_by_run_id
            .as_deref()
            .map(|raw| parse_uuid(raw, "build-constellation.built_by_run_id"))
            .transpose()?;
        let now = now_unix_ms()?;
        let domain_uuid = domain_pack_storage_uuid(&domain.id, &domain.version);
        let mut writes = Vec::new();
        for modality in &target.modalities {
            let stats =
                compute_constellation_stats(db, &reference_states, &modality.spec, &target)?;
            if stats.loo_stability < target.require_loo_min {
                return Err(DynamicJepaError::ConstellationLooBelowThreshold {
                    subject_id: target.subject_id.clone(),
                    modality_id: modality.spec.instrument_id.to_string(),
                    loo_stability: stats.loo_stability,
                    required: target.require_loo_min,
                });
            }
            let constellation_id = ConstellationId::new_v4();
            let mut record = ConstellationCentroid {
                header: DjRecordHeader::new(
                    constellation_id.0,
                    CONSTELLATION_CENTROID_RECORD_VERSION,
                    domain.id.clone(),
                    domain.version.clone(),
                    now,
                    built_by_run_id,
                ),
                constellation_id,
                domain_pack_id: domain.id.clone(),
                subject_id: target.subject_id.clone(),
                modality_id: modality.spec.instrument_id.clone(),
                centroid: stats.centroid,
                instrument_artifact_hash: instrument_artifact_hash32(&modality.spec)?,
                reference_set_count: reference_states.len() as u32,
                kept_count: stats.kept_count,
                dropped_zero_norm: stats.dropped_zero_norm,
                loo_stability: stats.loo_stability,
                calibration_percentile: target.calibration_percentile,
                calibration_set_size: target.calibration_set_size,
                built_at_unix_ms: now as u64,
                built_by_run_id,
            };
            record.refresh_content_hash()?;
            writes.push((
                domain_uuid,
                target.subject_id.clone(),
                modality.ordinal,
                record,
            ));
        }

        let mut audit = build_audit(
            "build_constellation",
            AuditStatus::Ok,
            reference_states
                .iter()
                .map(|state| format!("reference_event:{}", state.source_event_id))
                .collect(),
            writes
                .iter()
                .map(|(_, _, _, record)| record.constellation_id.to_string())
                .collect(),
            vec![CF_DJ_CONSTELLATIONS.to_string(), CF_DJ_AUDIT_LOG.to_string()],
            writes
                .iter()
                .map(|(_, _, _, record)| record.header.content_hash)
                .collect(),
        )?;
        let signal_yield = signal_yield_for_operation(
            "build_constellation",
            SignalYieldDimensions {
                n_panel_slots: target.modalities.len() as u32,
                n_active_modalities: target.modalities.len() as u32,
                k_candidates: 0,
            },
        )?;
        set_audit_signal_yield(&mut audit, signal_yield)?;
        put_constellations_with_audit_batch(db, &writes, &audit)?;

        let mut stored = Vec::new();
        for (domain_uuid, subject_id, ordinal, record) in &writes {
            let row = get_constellation(db, *domain_uuid, subject_id, *ordinal)?.ok_or_else(|| {
                DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_CONSTELLATIONS.to_string(),
                    key: Vec::new(),
                }
            })?;
            if row != *record {
                return Err(DynamicJepaError::StorageInvariantViolation {
                    message: format!(
                        "constellation readback mismatch for {}",
                        record.constellation_id
                    ),
                });
            }
            stored.push(row);
        }

        Ok(json!({
            "created_ids": {
                "constellation_ids": stored.iter().map(|row| row.constellation_id.to_string()).collect::<Vec<_>>(),
                "audit_id": audit.audit_id.to_string(),
            },
            "summary": {
                "subject_id": target.subject_id,
                "selector": selector.to_string(),
                "reference_set_count": reference_states.len(),
                "modalities": stored.iter().map(|row| row.modality_id.to_string()).collect::<Vec<_>>(),
                "loo_stability": stored.iter().map(|row| (row.modality_id.to_string(), row.loo_stability)).collect::<BTreeMap<_, _>>(),
            },
            "decoded_records": {
                "constellations": stored,
                "constellations_cf": inspect_cf(db, CF_DJ_CONSTELLATIONS, 100, 0)?,
                "audit_log": inspect_cf(db, CF_DJ_AUDIT_LOG, 400, 0)?,
            }
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_build_constellation",
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
                "dynamicjepa_build_constellation",
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

fn list_constellations_command(args: ListConstellationsArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    if args.limit == 0 {
        return Err(DynamicJepaError::validation(
            "list-constellations.limit",
            "--limit must be greater than zero",
            "request at least one decoded row",
        ));
    }
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "list-constellations.db")?;
    let counts = snapshot_dj_counts(db)?;
    let domain_filter = args
        .domain
        .as_deref()
        .map(DomainPackId::new)
        .transpose()?;
    let rows = list_constellations(db, 1_000_000, 0)?
        .into_iter()
        .filter(|row| {
            domain_filter
                .as_ref()
                .map_or(true, |domain_id| &row.header.domain_pack_id == domain_id)
        })
        .filter(|row| {
            args.subject
                .as_ref()
                .map_or(true, |subject| row.subject_id == *subject)
        })
        .skip(args.offset)
        .take(args.limit)
        .collect::<Vec<_>>();
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_list_constellations",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": [CF_DJ_CONSTELLATIONS],
            "counts": counts,
            "limit": args.limit,
            "offset": args.offset,
            "decoded_records": {"constellations": rows},
        }),
        exit_code: 0,
    })
}

fn get_constellation_command(args: GetConstellationArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-constellation.db")?;
    let counts = snapshot_dj_counts(db)?;
    let domain_id = DomainPackId::new(args.domain.clone())?;
    let domain = get_domain_pack(db, &domain_id, &args.domain_version)?.ok_or_else(|| {
        DynamicJepaError::DomainPackNotFound {
            id: domain_id.to_string(),
        }
    })?;
    let modality = resolve_modality_ordinal(&domain, &args.modality, "get-constellation.modality")?;
    let domain_uuid = domain_pack_storage_uuid(&domain.id, &domain.version);
    let row = get_constellation(db, domain_uuid, &args.subject, modality.ordinal)?;
    match row {
        Some(record) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_constellation",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_CONSTELLATIONS],
                "counts": counts,
                "decoded_records": {"constellation": record},
            }),
            exit_code: 0,
        }),
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_constellation",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_CONSTELLATIONS],
                "counts": counts,
                "queried": {"domain": args.domain, "subject": args.subject, "modality": args.modality},
                "remediation": "run dynamicjepa build-constellation for this subject and modality",
            }),
            exit_code: 0,
        }),
    }
}

fn calibrate_threshold_command(
    args: CalibrateThresholdArgs,
    recalibrate: bool,
) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_DOMAIN_PACKS,
        CF_DJ_NORMALIZED_STATES,
        CF_DJ_INSTRUMENT_READINGS,
        CF_DJ_CONSTELLATIONS,
        CF_DJ_THRESHOLD_CALIBRATIONS,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "calibrate-threshold.db")?;
    let before_counts = snapshot_dj_counts(db)?;

    let result = (|| -> DynamicJepaResult<Value> {
        let domain_id = DomainPackId::new(args.domain.clone())?;
        let domain = get_domain_pack(db, &domain_id, &args.domain_version)?.ok_or_else(|| {
            DynamicJepaError::DomainPackNotFound {
                id: domain_id.to_string(),
            }
        })?;
        let target = resolve_constellation_target(&domain, &args.subject)?;
        let modalities = filter_target_modalities(
            &domain,
            &target,
            &args.modality,
            "calibrate-threshold.modality",
        )?;
        let all_matching = matching_constellation_states(db, &domain, &target)?;
        let selector = match args.calibration_event_selector.as_deref() {
            Some(raw) => parse_event_selector(raw, "calibrate-threshold.calibration_event_selector")?,
            None => EventSelector::Last(target.calibration_set_size as usize),
        };
        let calibration_states = apply_event_selector(all_matching, selector);
        if calibration_states.is_empty() {
            return Err(DynamicJepaError::validation(
                "calibrate-threshold.calibration_event_selector",
                "calibration selector returned no states",
                "select a non-empty held-out event range",
            ));
        }
        let domain_uuid = domain_pack_storage_uuid(&domain.id, &domain.version);
        let mut writes = Vec::new();
        let mut superseded = Vec::new();
        for modality in modalities {
            let constellation = get_constellation(
                db,
                domain_uuid,
                &target.subject_id,
                modality.ordinal,
            )?
            .ok_or_else(|| DynamicJepaError::GuardConstellationMissing {
                domain_id: domain.id.to_string(),
                subject_id: target.subject_id.clone(),
                modality_id: modality.spec.instrument_id.to_string(),
            })?;
            if constellation.instrument_artifact_hash != instrument_artifact_hash32(&modality.spec)?
            {
                return Err(DynamicJepaError::ConstellationInstrumentHashMismatch {
                    constellation_id: constellation.constellation_id.0,
                    modality_id: modality.spec.instrument_id.to_string(),
                });
            }
            let reference_events = reference_event_ids_for_constellation(db, constellation.constellation_id)?;
            let calibration_events = calibration_states
                .iter()
                .map(|state| state.source_event_id.0)
                .collect::<BTreeSet<_>>();
            let overlap_count = calibration_events.intersection(&reference_events).count();
            if overlap_count > 0 {
                return Err(DynamicJepaError::CalibrationSetNotDisjoint {
                    subject_id: target.subject_id.clone(),
                    overlap_count,
                });
            }
            let existing_zero =
                get_threshold_calibration(db, domain_uuid, &target.subject_id, modality.ordinal, 0)?;
            if existing_zero.is_some() && !recalibrate {
                return Err(DynamicJepaError::CalibrationSupersessionRequired {
                    subject_id: target.subject_id.clone(),
                    modality_id: modality.spec.instrument_id.to_string(),
                });
            }
            let (supersede_seq, supersedes_uuid, reason) = if recalibrate {
                return Err(DynamicJepaError::CalibrationSupersessionRequired {
                    subject_id: target.subject_id.clone(),
                    modality_id: modality.spec.instrument_id.to_string(),
                });
            } else {
                (0, None, None)
            };
            let percentile = args.percentile.unwrap_or(target.calibration_percentile);
            let record = build_threshold_calibration_record(
                db,
                &domain,
                &target,
                &modality,
                &constellation,
                &calibration_states,
                percentile,
                supersede_seq,
                supersedes_uuid,
                reason,
            )?;
            superseded.push(supersedes_uuid.map(|id| id.to_string()));
            writes.push((
                domain_uuid,
                target.subject_id.clone(),
                modality.ordinal,
                supersede_seq,
                record,
            ));
        }
        write_threshold_calibration_rows(
            db,
            "calibrate_threshold",
            &target,
            selector,
            &calibration_states,
            writes,
            superseded,
            &source_cfs,
        )
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_calibrate_threshold",
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
                "dynamicjepa_calibrate_threshold",
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

fn recalibrate_threshold_command(args: RecalibrateThresholdArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_DOMAIN_PACKS,
        CF_DJ_NORMALIZED_STATES,
        CF_DJ_INSTRUMENT_READINGS,
        CF_DJ_CONSTELLATIONS,
        CF_DJ_THRESHOLD_CALIBRATIONS,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "recalibrate-threshold.db")?;
    let before_counts = snapshot_dj_counts(db)?;

    let result = (|| -> DynamicJepaResult<Value> {
        let supersedes_raw = args.supersedes.as_deref().ok_or_else(|| {
            DynamicJepaError::CalibrationSupersessionRequired {
                subject_id: args.subject.clone(),
                modality_id: args.modality.clone(),
            }
        })?;
        let supersedes = parse_threshold_calibration_id(
            supersedes_raw,
            "recalibrate-threshold.supersedes",
        )?;
        let reason = args.reason.clone().filter(|value| !value.trim().is_empty()).ok_or_else(|| {
            DynamicJepaError::CalibrationSupersessionRequired {
                subject_id: args.subject.clone(),
                modality_id: args.modality.clone(),
            }
        })?;
        let domain_id = DomainPackId::new(args.domain.clone())?;
        let domain = get_domain_pack(db, &domain_id, &args.domain_version)?.ok_or_else(|| {
            DynamicJepaError::DomainPackNotFound {
                id: domain_id.to_string(),
            }
        })?;
        let target = resolve_constellation_target(&domain, &args.subject)?;
        let modalities = filter_target_modalities(
            &domain,
            &target,
            &args.modality,
            "recalibrate-threshold.modality",
        )?;
        let all_matching = matching_constellation_states(db, &domain, &target)?;
        let selector = match args.calibration_event_selector.as_deref() {
            Some(raw) => parse_event_selector(raw, "recalibrate-threshold.calibration_event_selector")?,
            None => EventSelector::Last(target.calibration_set_size as usize),
        };
        let calibration_states = apply_event_selector(all_matching, selector);
        if calibration_states.is_empty() {
            return Err(DynamicJepaError::validation(
                "recalibrate-threshold.calibration_event_selector",
                "calibration selector returned no states",
                "select a non-empty held-out event range",
            ));
        }
        let domain_uuid = domain_pack_storage_uuid(&domain.id, &domain.version);
        let mut writes = Vec::new();
        let mut superseded = Vec::new();
        for modality in modalities {
            let constellation =
                get_constellation(db, domain_uuid, &target.subject_id, modality.ordinal)?
                    .ok_or_else(|| DynamicJepaError::GuardConstellationMissing {
                        domain_id: domain.id.to_string(),
                        subject_id: target.subject_id.clone(),
                        modality_id: modality.spec.instrument_id.to_string(),
                    })?;
            if constellation.instrument_artifact_hash != instrument_artifact_hash32(&modality.spec)?
            {
                return Err(DynamicJepaError::ConstellationInstrumentHashMismatch {
                    constellation_id: constellation.constellation_id.0,
                    modality_id: modality.spec.instrument_id.to_string(),
                });
            }
            let reference_events = reference_event_ids_for_constellation(db, constellation.constellation_id)?;
            let calibration_events = calibration_states
                .iter()
                .map(|state| state.source_event_id.0)
                .collect::<BTreeSet<_>>();
            let overlap_count = calibration_events.intersection(&reference_events).count();
            if overlap_count > 0 {
                return Err(DynamicJepaError::CalibrationSetNotDisjoint {
                    subject_id: target.subject_id.clone(),
                    overlap_count,
                });
            }
            let prior_rows = list_threshold_calibrations(db, 1_000_000, 0)?
                .into_iter()
                .filter(|row| {
                    row.header.domain_pack_id == domain.id
                        && row.subject_id == target.subject_id
                        && row.modality_id == modality.spec.instrument_id
                })
                .collect::<Vec<_>>();
            let latest = prior_rows
                .iter()
                .max_by_key(|row| row.supersede_seq)
                .ok_or_else(|| DynamicJepaError::GuardThresholdMissing {
                    domain_id: domain.id.to_string(),
                    subject_id: target.subject_id.clone(),
                    modality_id: modality.spec.instrument_id.to_string(),
                })?;
            if latest.calibration_id != supersedes {
                return Err(DynamicJepaError::CalibrationSupersessionRequired {
                    subject_id: target.subject_id.clone(),
                    modality_id: modality.spec.instrument_id.to_string(),
                });
            }
            let percentile = args.percentile.unwrap_or(latest.percentile);
            let supersede_seq = latest.supersede_seq.checked_add(1).ok_or_else(|| {
                DynamicJepaError::validation(
                    "recalibrate-threshold.supersede_seq",
                    "supersede_seq overflowed u32",
                    "start a fresh domain pack version before recalibrating again",
                )
            })?;
            let record = build_threshold_calibration_record(
                db,
                &domain,
                &target,
                &modality,
                &constellation,
                &calibration_states,
                percentile,
                supersede_seq,
                Some(supersedes),
                Some(reason.clone()),
            )?;
            superseded.push(Some(supersedes.to_string()));
            writes.push((
                domain_uuid,
                target.subject_id.clone(),
                modality.ordinal,
                supersede_seq,
                record,
            ));
        }
        write_threshold_calibration_rows(
            db,
            "recalibrate_threshold",
            &target,
            selector,
            &calibration_states,
            writes,
            superseded,
            &source_cfs,
        )
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_recalibrate_threshold",
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
                "dynamicjepa_recalibrate_threshold",
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

fn verify_career_taxonomy_command(args: VerifyDomainArgs) -> DynamicJepaResult<CommandOutcome> {
    verify_domain_command(VerificationDomain::CareerTaxonomy, args)
}

fn research_smoke_command(args: ResearchSmokeArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    validate_research_purpose(&args.purpose)?;
    if matches!(args.purpose.as_str(), "paper_small" | "paper_full") && args.onet_text_zip.is_none()
    {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet_text_zip",
            format!(
                "purpose={} requires --onet-text-zip with the O*NET 30.2 text database",
                args.purpose
            ),
            "download https://www.onetcenter.org/dl_files/database/db_30_2_text.zip and pass its local path",
        ));
    }
    let _validated_career_event_count = research_career_event_count(
        &args.purpose,
        args.career_event_count,
        args.onet_text_zip.is_some(),
    )?;
    validate_research_career_training_seed(
        args.career_training_seed,
        args.career_ablation_training_seed,
        args.onet_text_zip.is_some(),
    )?;
    let hp_overrides = ResearchHyperparameterOverrides::from_research_args(&args)?;
    let run_id = args
        .run_id
        .clone()
        .unwrap_or_else(|| format!("{}_{}", now_unix_ms().unwrap_or(0), Uuid::new_v4()));
    validate_research_run_id(&run_id)?;
    let mut ctx = ResearchSmokeContext::new(run_id, args, hp_overrides)?;
    match run_research_smoke(&mut ctx) {
        Ok(summary) => ctx.finish_passed(summary),
        Err(err) => ctx.finish_failed(err),
    }
}

const HP_SWEEP_SCHEMA_VERSION: u64 = 1;
const HP_SWEEP_CHILD_REQUIRED_FILES: &[&str] = &[
    "run_manifest.json",
    "fsv_report.json",
    "training_metrics.json",
    "artifact_hashes.json",
    "paper_tables/table_prediction_metrics.csv",
    "paper_tables/table_ablation.csv",
    "paper_tables/table_collapse_diagnostics.csv",
    "plots/plot_manifest.json",
];
const HP_SWEEP_OUTPUT_REQUIRED_FILES: &[&str] = &[
    "sweep_manifest.json",
    "paper_tables/table_hp_sensitivity.csv",
    "paper_tables/table_collapse_diagnostics.csv",
    "plots/hp_sensitivity.svg",
    "plots/collapse_diagnostic_f5.svg",
    "plots/plot_manifest.json",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HpSweepAxis {
    LearningRate,
    LatentDim,
    BatchSize,
}

impl HpSweepAxis {
    fn parse(value: &str) -> DynamicJepaResult<Self> {
        match value {
            "lr" => Ok(Self::LearningRate),
            "latent_dim" => Ok(Self::LatentDim),
            "batch_size" => Ok(Self::BatchSize),
            other => Err(DynamicJepaError::validation(
                "hp_sweep.sweep_axis",
                format!("unknown HP sweep axis {other:?}"),
                "use one of: lr, latent_dim, batch_size",
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::LearningRate => "lr",
            Self::LatentDim => "latent_dim",
            Self::BatchSize => "batch_size",
        }
    }
}

#[derive(Debug, Clone)]
struct HpSweepValue {
    raw: String,
    normalized: String,
    safe: String,
}

#[derive(Debug, Clone, Copy)]
struct HpSweepHyperparameters {
    lr: f64,
    latent_dim: u64,
    batch_size: u64,
    epochs: u64,
}

impl HpSweepHyperparameters {
    fn from_train_config(config: &TrainConfig) -> Self {
        Self {
            lr: config.optim.lr,
            latent_dim: config.model.encoder.out_dim as u64,
            batch_size: config.schedule.batch_size as u64,
            epochs: config.schedule.epochs as u64,
        }
    }

    fn with_swept_value(
        self,
        axis: HpSweepAxis,
        value: &HpSweepValue,
        career_event_count: u64,
    ) -> DynamicJepaResult<Self> {
        let mut next = self;
        match axis {
            HpSweepAxis::LearningRate => {
                next.lr = value.normalized.parse::<f64>().map_err(|err| {
                    DynamicJepaError::validation(
                        "hp_sweep.sweep_values",
                        format!(
                            "failed to parse normalized learning rate {:?}: {err}",
                            value.normalized
                        ),
                        "rerun with finite positive learning-rate values",
                    )
                })?;
            }
            HpSweepAxis::LatentDim => {
                next.latent_dim = value.normalized.parse::<u64>().map_err(|err| {
                    DynamicJepaError::validation(
                        "hp_sweep.sweep_values",
                        format!(
                            "failed to parse normalized latent_dim {:?}: {err}",
                            value.normalized
                        ),
                        "rerun with positive integer latent dimensions",
                    )
                })?;
            }
            HpSweepAxis::BatchSize => {
                next.batch_size = value.normalized.parse::<u64>().map_err(|err| {
                    DynamicJepaError::validation(
                        "hp_sweep.sweep_values",
                        format!(
                            "failed to parse normalized batch_size {:?}: {err}",
                            value.normalized
                        ),
                        "rerun with positive integer batch sizes",
                    )
                })?;
                next.epochs = scaled_epochs_for_batch_sweep(
                    self.epochs,
                    self.batch_size,
                    next.batch_size,
                    career_event_count,
                )?;
            }
        }
        Ok(next)
    }

    fn manifest_value(self) -> Value {
        json!({
            "lr": self.lr,
            "latent_dim": self.latent_dim,
            "batch_size": self.batch_size,
            "epochs": self.epochs,
        })
    }
}

fn scaled_epochs_for_batch_sweep(
    base_epochs: u64,
    base_batch_size: u64,
    swept_batch_size: u64,
    career_event_count: u64,
) -> DynamicJepaResult<u64> {
    if base_epochs == 0 || base_batch_size == 0 || swept_batch_size == 0 {
        return Err(DynamicJepaError::validation(
            "hp_sweep.training_budget",
            format!(
                "cannot scale epochs with base_epochs={base_epochs}, base_batch_size={base_batch_size}, swept_batch_size={swept_batch_size}"
            ),
            "use positive base epochs and batch-size sweep values",
        ));
    }
    let split_counts = split_counts_for_row_count(career_event_count);
    let train_rows = split_counts.get("train").copied().unwrap_or(0).max(1);
    let base_steps_per_epoch = train_rows.div_ceil(base_batch_size);
    let swept_steps_per_epoch = train_rows.div_ceil(swept_batch_size);
    let target_steps = (base_epochs as u128)
        .checked_mul(base_steps_per_epoch as u128)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "hp_sweep.training_budget",
                "optimizer-step budget overflowed while scaling batch-size sweep epochs",
                "use a smaller base epoch count or narrower batch-size sweep values",
            )
        })?;
    let scaled_epochs = target_steps.div_ceil(swept_steps_per_epoch as u128);
    let scaled_epochs = u64::try_from(scaled_epochs).map_err(|_| {
        DynamicJepaError::validation(
            "hp_sweep.training_budget",
            "scaled epoch count does not fit in u64",
            "use a smaller base epoch count or narrower batch-size sweep values",
        )
    })?;
    Ok(scaled_epochs.max(1))
}

fn hp_sweep_command(args: HpSweepArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let operation = "dynamicjepa_hp_sweep";
    let before_state = json!({
        "base_config": ci_path_state(&args.base_config)?,
        "onet_text_zip": ci_path_state(&args.onet_text_zip)?,
        "output_root": ci_path_state(&args.output_root)?,
    });
    validate_research_purpose(&args.purpose)?;
    if args.output_root.exists() {
        return Err(DynamicJepaError::validation(
            "hp_sweep.output_root",
            format!("output root already exists: {}", args.output_root.display()),
            "choose a fresh sweep bundle path; HP sweeps never overwrite prior evidence",
        ));
    }
    if !args.onet_text_zip.exists() {
        return Err(DynamicJepaError::validation(
            "hp_sweep.onet_text_zip",
            format!("O*NET text ZIP does not exist: {}", args.onet_text_zip.display()),
            "download the O*NET 30.2 text database ZIP locally and pass --onet-text-zip; HP sweeps do not use mock data",
        ));
    }
    if !args.system_specs.exists() {
        return Err(DynamicJepaError::validation(
            "hp_sweep.system_specs",
            format!(
                "system specs file does not exist: {}",
                args.system_specs.display()
            ),
            "pass --system-specs with a local hardware specification document",
        ));
    }
    let base_config = TrainConfig::from_path(&args.base_config)?;
    let base_hp = HpSweepHyperparameters::from_train_config(&base_config);
    let axis = HpSweepAxis::parse(&args.sweep_axis)?;
    let values = parse_hp_sweep_values(axis, &args.sweep_values)?;
    let seeds = parse_hp_sweep_seeds(&args)?;
    let career_event_count =
        research_career_event_count(&args.purpose, args.career_event_count, true)?;

    fs::create_dir_all(args.output_root.join("cells")).map_err(|err| {
        DynamicJepaError::Storage {
            operation: "hp_sweep.create_cells_dir".to_string(),
            cf: "filesystem".to_string(),
            message: format!(
                "failed to create {}: {err}",
                args.output_root.join("cells").display()
            ),
            remediation: "choose a writable fresh --output-root".to_string(),
        }
    })?;
    fs::create_dir_all(args.output_root.join("paper_tables")).map_err(|err| {
        DynamicJepaError::Storage {
            operation: "hp_sweep.create_paper_tables".to_string(),
            cf: "filesystem".to_string(),
            message: format!(
                "failed to create {}: {err}",
                args.output_root.join("paper_tables").display()
            ),
            remediation: "choose a writable fresh --output-root".to_string(),
        }
    })?;
    fs::create_dir_all(args.output_root.join("plots")).map_err(|err| {
        DynamicJepaError::Storage {
            operation: "hp_sweep.create_plots".to_string(),
            cf: "filesystem".to_string(),
            message: format!(
                "failed to create {}: {err}",
                args.output_root.join("plots").display()
            ),
            remediation: "choose a writable fresh --output-root".to_string(),
        }
    })?;

    let mut cells = Vec::new();
    for seed in &seeds {
        for value in &values {
            let hp = base_hp.with_swept_value(axis, value, career_event_count)?;
            let cell_id = format!("{}_{}_seed{}", axis.as_str(), value.safe, seed);
            validate_research_run_id(&cell_id)?;
            let cell_args = ResearchSmokeArgs {
                run_id: Some(cell_id.clone()),
                run_root: args.output_root.join("cells"),
                fixtures_root: args.fixtures_root.clone(),
                db_root: None,
                artifact_root: None,
                purpose: args.purpose.clone(),
                onet_text_zip: Some(args.onet_text_zip.clone()),
                career_event_count: Some(career_event_count),
                career_training_seed: Some(*seed),
                career_ablation_training_seed: None,
                hp_learning_rate: Some(hp.lr),
                hp_latent_dim: Some(hp.latent_dim),
                hp_batch_size: Some(hp.batch_size),
                hp_epochs: Some(hp.epochs),
                hyperparameter_origin: Some(format!(
                    "hp_sweep:{}:{}:{}",
                    args.output_root.display(),
                    axis.as_str(),
                    value.raw
                )),
                system_specs: args.system_specs.clone(),
                json: true,
            };
            let outcome = research_smoke_command(cell_args)?;
            if outcome.exit_code != 0 {
                let failure = json!({
                    "cell_id": cell_id,
                    "seed": seed,
                    "sweep_axis": axis.as_str(),
                    "sweep_value": value.raw,
                    "cell_outcome": outcome.value,
                });
                write_hp_sweep_failure_bundle(&args, &before_state, &cells, failure.clone())?;
                let after_state = hp_sweep_after_state(&args.output_root)?;
                return Ok(CommandOutcome {
                    value: json!({
                        "operation": operation,
                        "status": "error",
                        "source_of_truth": args.output_root,
                        "before_state": before_state,
                        "after_state": after_state,
                        "failure_details": failure,
                    }),
                    exit_code: 1,
                });
            }
            let cell_dir = args.output_root.join("cells").join(&cell_id);
            let cell = read_hp_sweep_cell(&cell_dir, &cell_id, axis, value, *seed, hp)?;
            cells.push(cell);
        }
    }

    write_hp_sweep_tables(&args.output_root, &cells)?;
    write_hp_sensitivity_svg(
        &args.output_root.join("plots/hp_sensitivity.svg"),
        axis,
        &cells,
    )?;
    write_hp_collapse_f5_svg(
        &args.output_root.join("plots/collapse_diagnostic_f5.svg"),
        &cells,
    )?;
    write_hp_sweep_plot_manifest(&args.output_root)?;

    let manifest = json!({
        "schema_version": HP_SWEEP_SCHEMA_VERSION,
        "status": "passed",
        "created_at_unix_ms": now_unix_ms()?,
        "operation": operation,
        "base_config": args.base_config,
        "base_hyperparameters": base_hp.manifest_value(),
        "purpose": args.purpose,
        "career_event_count": career_event_count,
        "sweep_axis": axis.as_str(),
        "sweep_values": values.iter().map(|value| value.raw.clone()).collect::<Vec<_>>(),
        "career_training_seeds": seeds,
        "onet_text_zip": args.onet_text_zip,
        "cell_count": cells.len(),
        "cells": cells,
    });
    write_json_file(&args.output_root.join("sweep_manifest.json"), &manifest)?;
    let required_file_states =
        hp_sweep_relative_file_states(&args.output_root, HP_SWEEP_OUTPUT_REQUIRED_FILES)?;
    let fsv_report = json!({
        "schema_version": HP_SWEEP_SCHEMA_VERSION,
        "status": "passed",
        "source_of_truth": {
            "sweep_bundle": args.output_root,
            "child_bundles": args.output_root.join("cells"),
            "paper_tables": args.output_root.join("paper_tables"),
            "plots": args.output_root.join("plots"),
        },
        "created_at_unix_ms": now_unix_ms()?,
        "required_file_states": required_file_states,
        "readback_checks": {
            "cell_count": manifest["cell_count"],
            "hp_sensitivity_rows": read_ci_csv_table(
                &args.output_root.join("paper_tables/table_hp_sensitivity.csv"),
                "table_hp_sensitivity",
            )?.rows.len(),
            "collapse_rows": read_ci_csv_table(
                &args.output_root.join("paper_tables/table_collapse_diagnostics.csv"),
                "table_collapse_diagnostics",
            )?.rows.len(),
        },
    });
    write_json_file(&args.output_root.join("sweep_fsv_report.json"), &fsv_report)?;

    let manifest_readback = read_json_file(&args.output_root.join("sweep_manifest.json"))?;
    let fsv_readback = read_json_file(&args.output_root.join("sweep_fsv_report.json"))?;
    let sensitivity_readback = read_ci_csv_table(
        &args
            .output_root
            .join("paper_tables/table_hp_sensitivity.csv"),
        "table_hp_sensitivity",
    )?;
    let collapse_readback = read_ci_csv_table(
        &args
            .output_root
            .join("paper_tables/table_collapse_diagnostics.csv"),
        "table_collapse_diagnostics",
    )?;
    let after_state = hp_sweep_after_state(&args.output_root)?;
    Ok(CommandOutcome {
        value: json!({
            "operation": operation,
            "status": "ok",
            "source_of_truth": args.output_root,
            "before_state": before_state,
            "after_state": after_state,
            "sweep_manifest": manifest_readback,
            "sweep_fsv_report": fsv_readback,
            "readback_row_counts": {
                "table_hp_sensitivity": sensitivity_readback.rows.len(),
                "table_collapse_diagnostics": collapse_readback.rows.len(),
            },
        }),
        exit_code: 0,
    })
}

fn parse_hp_sweep_values(
    axis: HpSweepAxis,
    values: &[String],
) -> DynamicJepaResult<Vec<HpSweepValue>> {
    if values.is_empty() {
        return Err(DynamicJepaError::validation(
            "hp_sweep.sweep_values",
            "--sweep-values must contain at least one value",
            "pass a comma-delimited value list such as 1e-4,3e-4 or 16,32",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut parsed = Vec::new();
    for raw in values {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(DynamicJepaError::validation(
                "hp_sweep.sweep_values",
                "empty sweep value was supplied",
                "remove empty elements from --sweep-values",
            ));
        }
        let normalized = match axis {
            HpSweepAxis::LearningRate => {
                let lr = trimmed.parse::<f64>().map_err(|err| {
                    DynamicJepaError::validation(
                        "hp_sweep.sweep_values",
                        format!("learning-rate value {trimmed:?} is not numeric: {err}"),
                        "use finite positive floating point values for lr",
                    )
                })?;
                if !lr.is_finite() || lr <= 0.0 {
                    return Err(DynamicJepaError::validation(
                        "hp_sweep.sweep_values",
                        format!("learning-rate value must be finite and positive, got {trimmed:?}"),
                        "use finite positive floating point values for lr",
                    ));
                }
                format!("{lr:.12e}")
            }
            HpSweepAxis::LatentDim | HpSweepAxis::BatchSize => {
                let value = trimmed.parse::<u64>().map_err(|err| {
                    DynamicJepaError::validation(
                        "hp_sweep.sweep_values",
                        format!("integer sweep value {trimmed:?} is invalid: {err}"),
                        "use positive integer values for latent_dim and batch_size",
                    )
                })?;
                if value == 0 {
                    return Err(DynamicJepaError::validation(
                        "hp_sweep.sweep_values",
                        format!("integer sweep value must be positive, got {trimmed:?}"),
                        "use positive integer values for latent_dim and batch_size",
                    ));
                }
                value.to_string()
            }
        };
        if !seen.insert(normalized.clone()) {
            return Err(DynamicJepaError::validation(
                "hp_sweep.sweep_values",
                format!("duplicate sweep value after normalization: {trimmed:?}"),
                "deduplicate --sweep-values before running an immutable sweep",
            ));
        }
        parsed.push(HpSweepValue {
            raw: trimmed.to_string(),
            normalized,
            safe: safe_filename(trimmed),
        });
    }
    Ok(parsed)
}

fn parse_hp_sweep_seeds(args: &HpSweepArgs) -> DynamicJepaResult<Vec<u64>> {
    if args.career_training_seed.is_some() && !args.career_training_seeds.is_empty() {
        return Err(DynamicJepaError::validation(
            "hp_sweep.career_training_seed",
            "provide either --career-training-seed or --career-training-seeds, not both",
            "use one deterministic seed selection for the sweep bundle",
        ));
    }
    let seeds = if args.career_training_seeds.is_empty() {
        vec![args.career_training_seed.unwrap_or(42)]
    } else {
        args.career_training_seeds.clone()
    };
    let mut seen = BTreeSet::new();
    for seed in &seeds {
        if !seen.insert(*seed) {
            return Err(DynamicJepaError::validation(
                "hp_sweep.career_training_seeds",
                format!("duplicate career training seed: {seed}"),
                "deduplicate the seed list before running an immutable sweep",
            ));
        }
    }
    Ok(seeds)
}

fn read_hp_sweep_cell(
    cell_dir: &Path,
    cell_id: &str,
    axis: HpSweepAxis,
    value: &HpSweepValue,
    seed: u64,
    expected_hp: HpSweepHyperparameters,
) -> DynamicJepaResult<Value> {
    let manifest_path = cell_dir.join("run_manifest.json");
    let fsv_path = cell_dir.join("fsv_report.json");
    let manifest = read_json_file(&manifest_path)?;
    let fsv = read_json_file(&fsv_path)?;
    require_json_status(&manifest, "passed", "hp_sweep cell run_manifest.json")?;
    require_json_status(&fsv, "passed", "hp_sweep cell fsv_report.json")?;
    let run_id = required_json_string(&manifest, "run_id", "run_manifest.json")?;
    let training_rows = read_json_file(&cell_dir.join("training_metrics.json"))?;
    let career = training_rows
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["domain"] == "career_taxonomy"))
        .cloned()
        .ok_or_else(|| DynamicJepaError::VerificationFailed {
            test_name: "hp_sweep.cell_training_metrics".to_string(),
            message: format!(
                "cell {cell_id} did not contain a career_taxonomy training_metrics row"
            ),
            evidence_path: cell_dir.display().to_string(),
        })?;
    verify_hp_sweep_cell_hyperparameters(cell_id, &career["hyperparameters"], expected_hp)?;
    let career_db = path_value(&career["db_path"], "hp_sweep.cell.db_path")?;
    let store = open_existing_store(&career_db, "hp_sweep.read_cell_career_db")?;
    let db_counts = snapshot_dj_counts(store.dynamicjepa_db())?;
    if db_counts.get(CF_DJ_MODEL_ARTIFACTS).copied().unwrap_or(0) == 0 {
        return Err(DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_MODEL_ARTIFACTS.to_string(),
            key: Vec::new(),
        });
    }
    let child_required_files =
        hp_sweep_relative_file_states(cell_dir, HP_SWEEP_CHILD_REQUIRED_FILES)?;
    let collapse_table = read_ci_csv_table(
        &cell_dir.join("paper_tables/table_collapse_diagnostics.csv"),
        "cell_table_collapse_diagnostics",
    )?;
    let career_collapse_rows = collapse_table
        .rows
        .iter()
        .filter(|row| row.get("domain").map(String::as_str) == Some("career_taxonomy"))
        .count();
    if career_collapse_rows == 0 {
        return Err(DynamicJepaError::VerificationFailed {
            test_name: "hp_sweep.cell_collapse_table".to_string(),
            message: format!("cell {cell_id} collapse table has no career_taxonomy row"),
            evidence_path: cell_dir.display().to_string(),
        });
    }
    let metrics = &career["metrics"];
    Ok(json!({
        "cell_id": cell_id,
        "run_id": run_id,
        "run_dir": cell_dir,
        "seed": seed,
        "sweep_axis": axis.as_str(),
        "sweep_value": value.raw,
        "hyperparameters": expected_hp.manifest_value(),
        "status": manifest["status"],
        "fsv_status": fsv["status"],
        "career_db_path": career_db,
        "career_db_counts": db_counts,
        "career_verification_run_id": career["verification_run_id"],
        "metrics": {
            "val_latent_cosine": required_metric_f64(metrics, "val_latent_cosine")?,
            "val_latent_mse": required_metric_f64(metrics, "val_latent_mse")?,
            "test_latent_cosine": required_metric_f64(metrics, "test_latent_cosine")?,
            "test_latent_mse": required_metric_f64(metrics, "test_latent_mse")?,
            "collapse_val_var_min": metric_json_value(metrics, "collapse_val_var_min")?,
            "collapse_val_var_p25": metric_json_value(metrics, "collapse_val_var_p25")?,
            "collapse_val_var_median": metric_json_value(metrics, "collapse_val_var_median")?,
            "collapse_val_var_p75": metric_json_value(metrics, "collapse_val_var_p75")?,
            "collapse_val_var_max": metric_json_value(metrics, "collapse_val_var_max")?,
            "collapse_covar_frob": metric_json_value(metrics, "collapse_covar_frob")?,
            "collapse_eff_rank": metric_json_value(metrics, "collapse_eff_rank")?,
            "collapse_eff_rank_ratio": metric_json_value(metrics, "collapse_eff_rank_ratio")?,
            "collapse_alignment": metric_json_value(metrics, "collapse_alignment")?,
            "collapse_uniformity": metric_json_value(metrics, "collapse_uniformity")?,
            "collapse_sigreg_kl": metric_json_value(metrics, "collapse_sigreg_kl")?,
            "collapse_sigreg_kl_clamped": metric_json_value(metrics, "collapse_sigreg_kl_clamped")?,
            "collapse_latent_collapsed": metric_json_value(metrics, "collapse_latent_collapsed")?,
        },
        "run_manifest_sha256": hex_bytes(&file_sha256(&manifest_path)?),
        "fsv_report_sha256": hex_bytes(&file_sha256(&fsv_path)?),
        "required_file_states": child_required_files,
        "collapse_table_career_rows": career_collapse_rows,
    }))
}

fn verify_hp_sweep_cell_hyperparameters(
    cell_id: &str,
    hp: &Value,
    expected: HpSweepHyperparameters,
) -> DynamicJepaResult<()> {
    let actual_lr = hp["lr"].as_f64().ok_or_else(|| {
        DynamicJepaError::schema(
            "hp_sweep.cell.hyperparameters.lr",
            format!("cell {cell_id} is missing numeric lr hyperparameter"),
            "inspect the child training artifact config.json",
        )
    })?;
    let actual_latent = hp["latent_dim"].as_u64().ok_or_else(|| {
        DynamicJepaError::schema(
            "hp_sweep.cell.hyperparameters.latent_dim",
            format!("cell {cell_id} is missing numeric latent_dim hyperparameter"),
            "inspect the child training artifact config.json",
        )
    })?;
    let actual_batch = hp["batch_size"].as_u64().ok_or_else(|| {
        DynamicJepaError::schema(
            "hp_sweep.cell.hyperparameters.batch_size",
            format!("cell {cell_id} is missing numeric batch_size hyperparameter"),
            "inspect the child training artifact config.json",
        )
    })?;
    let actual_epochs = hp["epochs"].as_u64().ok_or_else(|| {
        DynamicJepaError::schema(
            "hp_sweep.cell.hyperparameters.epochs",
            format!("cell {cell_id} is missing numeric epochs hyperparameter"),
            "inspect the child training artifact config.json",
        )
    })?;
    if (actual_lr - expected.lr).abs() > expected.lr.abs().max(1.0) * 1.0e-12
        || actual_latent != expected.latent_dim
        || actual_batch != expected.batch_size
        || actual_epochs != expected.epochs
    {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "cell {cell_id} persisted HPs do not match requested HPs: actual=({actual_lr},{actual_latent},{actual_batch},{actual_epochs}) expected=({},{},{},{})",
                expected.lr, expected.latent_dim, expected.batch_size, expected.epochs
            ),
        });
    }
    Ok(())
}

fn metric_json_value(metrics: &Value, key: &str) -> DynamicJepaResult<Value> {
    let raw = metrics.get(key).ok_or_else(|| {
        DynamicJepaError::schema(
            format!("metrics.{key}"),
            format!("missing training metric {key}"),
            "inspect training artifact metrics.json; HP sweep tables require collapse diagnostics",
        )
    })?;
    if raw.is_null() {
        return Ok(json!("nan"));
    }
    let value = raw.as_f64().ok_or_else(|| {
        DynamicJepaError::schema(
            format!("metrics.{key}"),
            format!("training metric {key} is not numeric or null: {raw}"),
            "inspect training artifact metrics.json; HP sweep tables require numeric metrics",
        )
    })?;
    Ok(if value.is_finite() {
        json!(value)
    } else {
        json!("nan")
    })
}

fn write_hp_sweep_tables(output_root: &Path, cells: &[Value]) -> DynamicJepaResult<()> {
    let mut sensitivity = "cell_id,run_id,seed,sweep_axis,sweep_value,lr,latent_dim,batch_size,epochs,val_latent_cosine,val_latent_mse,test_latent_cosine,test_latent_mse,eff_rank_ratio,alignment,uniformity,latent_collapsed,career_db_path,run_dir\n".to_string();
    let mut collapse = "cell_id,run_id,seed,sweep_axis,sweep_value,lr,latent_dim,batch_size,epochs,val_var_min,val_var_p25,val_var_median,val_var_p75,val_var_max,covar_frob,eff_rank,eff_rank_ratio,alignment,uniformity,sigreg_kl,sigreg_kl_clamped,latent_collapsed,val_latent_cosine,val_latent_mse\n".to_string();
    for cell in cells {
        sensitivity.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(cell["cell_id"].as_str().unwrap_or("")),
            csv_escape(cell["run_id"].as_str().unwrap_or("")),
            cell["seed"].as_u64().unwrap_or(0),
            csv_escape(cell["sweep_axis"].as_str().unwrap_or("")),
            csv_escape(cell["sweep_value"].as_str().unwrap_or("")),
            hp_cell_f64(cell, "lr")?,
            hp_cell_u64(cell, "latent_dim")?,
            hp_cell_u64(cell, "batch_size")?,
            hp_cell_u64(cell, "epochs")?,
            metric_cell_from_cell(cell, "val_latent_cosine")?,
            metric_cell_from_cell(cell, "val_latent_mse")?,
            metric_cell_from_cell(cell, "test_latent_cosine")?,
            metric_cell_from_cell(cell, "test_latent_mse")?,
            metric_cell_from_cell(cell, "collapse_eff_rank_ratio")?,
            metric_cell_from_cell(cell, "collapse_alignment")?,
            metric_cell_from_cell(cell, "collapse_uniformity")?,
            metric_cell_from_cell(cell, "collapse_latent_collapsed")?,
            csv_escape(cell["career_db_path"].as_str().unwrap_or("")),
            csv_escape(cell["run_dir"].as_str().unwrap_or("")),
        ));
        collapse.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(cell["cell_id"].as_str().unwrap_or("")),
            csv_escape(cell["run_id"].as_str().unwrap_or("")),
            cell["seed"].as_u64().unwrap_or(0),
            csv_escape(cell["sweep_axis"].as_str().unwrap_or("")),
            csv_escape(cell["sweep_value"].as_str().unwrap_or("")),
            hp_cell_f64(cell, "lr")?,
            hp_cell_u64(cell, "latent_dim")?,
            hp_cell_u64(cell, "batch_size")?,
            hp_cell_u64(cell, "epochs")?,
            metric_cell_from_cell(cell, "collapse_val_var_min")?,
            metric_cell_from_cell(cell, "collapse_val_var_p25")?,
            metric_cell_from_cell(cell, "collapse_val_var_median")?,
            metric_cell_from_cell(cell, "collapse_val_var_p75")?,
            metric_cell_from_cell(cell, "collapse_val_var_max")?,
            metric_cell_from_cell(cell, "collapse_covar_frob")?,
            metric_cell_from_cell(cell, "collapse_eff_rank")?,
            metric_cell_from_cell(cell, "collapse_eff_rank_ratio")?,
            metric_cell_from_cell(cell, "collapse_alignment")?,
            metric_cell_from_cell(cell, "collapse_uniformity")?,
            metric_cell_from_cell(cell, "collapse_sigreg_kl")?,
            metric_cell_from_cell(cell, "collapse_sigreg_kl_clamped")?,
            metric_cell_from_cell(cell, "collapse_latent_collapsed")?,
            metric_cell_from_cell(cell, "val_latent_cosine")?,
            metric_cell_from_cell(cell, "val_latent_mse")?,
        ));
    }
    write_text_file(
        &output_root.join("paper_tables/table_hp_sensitivity.csv"),
        &sensitivity,
    )?;
    write_text_file(
        &output_root.join("paper_tables/table_collapse_diagnostics.csv"),
        &collapse,
    )
}

fn hp_cell_f64(cell: &Value, key: &str) -> DynamicJepaResult<String> {
    required_json_f64_cell(&cell["hyperparameters"], key)
}

fn hp_cell_u64(cell: &Value, key: &str) -> DynamicJepaResult<String> {
    required_json_u64_cell(&cell["hyperparameters"], key)
}

fn metric_cell_from_cell(cell: &Value, key: &str) -> DynamicJepaResult<String> {
    let raw = cell["metrics"].get(key).ok_or_else(|| {
        DynamicJepaError::schema(
            format!("hp_sweep.cell.metrics.{key}"),
            format!("missing cell metric {key}"),
            "inspect the child training_metrics.json and sweep_manifest.json",
        )
    })?;
    value_to_f64_cell(raw, &format!("hp_sweep.cell.metrics.{key}"))
}

fn write_hp_sensitivity_svg(
    path: &Path,
    axis: HpSweepAxis,
    cells: &[Value],
) -> DynamicJepaResult<()> {
    let rows = cells
        .iter()
        .map(|cell| {
            Ok((
                format!(
                    "{} s{}",
                    cell["sweep_value"].as_str().unwrap_or(""),
                    cell["seed"].as_u64().unwrap_or(0)
                ),
                cell["metrics"]["val_latent_cosine"]
                    .as_f64()
                    .ok_or_else(|| {
                        DynamicJepaError::schema(
                            "hp_sweep.plot.val_latent_cosine",
                            "cell metric val_latent_cosine is missing or not numeric",
                            "inspect sweep_manifest.json before plotting",
                        )
                    })?,
            ))
        })
        .collect::<DynamicJepaResult<Vec<_>>>()?;
    write_metric_bar_svg(
        path,
        &format!("{} sensitivity: validation latent cosine", axis.as_str()),
        rows,
    )
}

fn write_hp_collapse_f5_svg(path: &Path, cells: &[Value]) -> DynamicJepaResult<()> {
    if cells.is_empty() {
        return Err(DynamicJepaError::validation(
            "hp_sweep.f5_plot",
            "cannot render F5 collapse plot with zero cells",
            "run at least one HP sweep cell before rendering plots",
        ));
    }
    let width = 960.0;
    let height = 640.0;
    let mut marks = String::new();
    let panels = [
        ("collapse_eff_rank_ratio", 70.0, 70.0),
        ("collapse_covar_frob", 370.0, 70.0),
        ("val_latent_cosine", 670.0, 70.0),
        ("collapse_alignment", 70.0, 360.0),
        ("collapse_uniformity", 370.0, 360.0),
        ("collapse_val_var_min", 670.0, 360.0),
    ];
    for (metric, x0, y0) in panels {
        let values = cells
            .iter()
            .map(|cell| cell["metrics"][metric].as_f64().unwrap_or(0.0))
            .collect::<Vec<_>>();
        let max_value = values.iter().copied().fold(0.0f64, f64::max).max(1.0e-9);
        marks.push_str(&format!(
            r##"<text x="{x0:.0}" y="{:.0}" font-size="13" font-family="Arial, sans-serif" font-weight="700" fill="#111827">{}</text>
<rect x="{x0:.0}" y="{:.0}" width="230" height="190" fill="#ffffff" stroke="#d1d5db"/>
"##,
            y0 - 18.0,
            xml_escape(metric),
            y0,
        ));
        let slot = 230.0 / cells.len() as f64;
        for (idx, value) in values.iter().enumerate() {
            let bar_height = 160.0 * (*value / max_value);
            let x = x0 + idx as f64 * slot + slot * 0.2;
            let y = y0 + 178.0 - bar_height;
            marks.push_str(&format!(
                r##"<rect x="{x:.2}" y="{y:.2}" width="{:.2}" height="{bar_height:.2}" fill="#0f766e"/>
"##,
                (slot * 0.6).max(4.0),
            ));
        }
    }
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}" role="img" aria-label="F5 collapse diagnostics">
<rect width="100%" height="100%" fill="#ffffff"/>
<text x="32" y="34" font-size="18" font-family="Arial, sans-serif" font-weight="700" fill="#111827">F5 collapse diagnostics across HP sweep cells</text>
{marks}</svg>
"##
    );
    write_text_file(path, &svg)
}

fn write_hp_sweep_plot_manifest(output_root: &Path) -> DynamicJepaResult<()> {
    write_json_file(
        &output_root.join("plots/plot_manifest.json"),
        &json!({
            "status": "rendered",
            "plots": [
                {
                    "id": "hp_sensitivity",
                    "source_table": output_root.join("paper_tables/table_hp_sensitivity.csv"),
                    "output": output_root.join("plots/hp_sensitivity.svg"),
                },
                {
                    "id": "collapse_diagnostic_f5",
                    "source_table": output_root.join("paper_tables/table_collapse_diagnostics.csv"),
                    "output": output_root.join("plots/collapse_diagnostic_f5.svg"),
                }
            ]
        }),
    )
}

fn write_hp_sweep_failure_bundle(
    args: &HpSweepArgs,
    before_state: &Value,
    completed_cells: &[Value],
    failure: Value,
) -> DynamicJepaResult<()> {
    write_json_file(
        &args.output_root.join("sweep_manifest.json"),
        &json!({
            "schema_version": HP_SWEEP_SCHEMA_VERSION,
            "status": "failed",
            "created_at_unix_ms": now_unix_ms()?,
            "operation": "dynamicjepa_hp_sweep",
            "source_of_truth": args.output_root,
            "before_state": before_state,
            "completed_cells": completed_cells,
            "failure_details": failure,
        }),
    )?;
    write_json_file(
        &args.output_root.join("sweep_fsv_report.json"),
        &json!({
            "schema_version": HP_SWEEP_SCHEMA_VERSION,
            "status": "failed",
            "source_of_truth": args.output_root,
            "created_at_unix_ms": now_unix_ms()?,
            "completed_cell_count": completed_cells.len(),
            "failure_details": failure,
        }),
    )
}

fn hp_sweep_after_state(output_root: &Path) -> DynamicJepaResult<Value> {
    Ok(json!({
        "output_root": ci_path_state(output_root)?,
        "sweep_manifest": ci_path_state(&output_root.join("sweep_manifest.json"))?,
        "sweep_fsv_report": ci_path_state(&output_root.join("sweep_fsv_report.json"))?,
        "hp_sensitivity_table": ci_path_state(&output_root.join("paper_tables/table_hp_sensitivity.csv"))?,
        "collapse_table": ci_path_state(&output_root.join("paper_tables/table_collapse_diagnostics.csv"))?,
        "plots": [
            ci_path_state(&output_root.join("plots/hp_sensitivity.svg"))?,
            ci_path_state(&output_root.join("plots/collapse_diagnostic_f5.svg"))?,
            ci_path_state(&output_root.join("plots/plot_manifest.json"))?,
        ],
    }))
}

fn hp_sweep_relative_file_states(root: &Path, rel_paths: &[&str]) -> DynamicJepaResult<Vec<Value>> {
    let mut states = Vec::new();
    for rel_path in rel_paths {
        let path = root.join(rel_path);
        let metadata = fs::metadata(&path).map_err(|err| DynamicJepaError::Storage {
            operation: "hp_sweep.required_file".to_string(),
            cf: "filesystem".to_string(),
            message: format!("missing HP sweep file {}: {err}", path.display()),
            remediation:
                "fix the HP sweep writer; all declared manifests, tables, and plots must exist"
                    .to_string(),
        })?;
        if !metadata.is_file() {
            return Err(DynamicJepaError::validation(
                "hp_sweep.required_file",
                format!("HP sweep output path is not a file: {}", path.display()),
                "fix the HP sweep writer so required outputs are regular files",
            ));
        }
        states.push(json!({
            "path": rel_path,
            "sha256": hex_bytes(&file_sha256(&path)?),
            "size_bytes": metadata.len(),
        }));
    }
    Ok(states)
}

const MULTISEED_AGGREGATE_SCHEMA_VERSION: u64 = 1;
const MULTISEED_CHILD_REQUIRED_FILES: &[&str] = &[
    "run_manifest.json",
    "fsv_report.json",
    "data_manifest.json",
    "artifact_hashes.json",
    "paper_tables/table_prediction_metrics.csv",
    "paper_tables/table_ablation.csv",
    "plots/plot_manifest.json",
    "plots/prediction_val_mse.svg",
    "plots/career_ablation_val_mse.svg",
];
const MULTISEED_OUTPUT_REQUIRED_FILES: &[&str] = &[
    "aggregate_manifest.json",
    "paper_tables/table_prediction_metrics_multiseed.csv",
    "paper_tables/table_ablation_multiseed.csv",
    "paper_tables/table_paired_ablation_tests.csv",
    "paper_tables/table_seed_failures.csv",
    "plots/multiseed_val_mse_box.svg",
    "plots/multiseed_paired_delta_forest.svg",
    "plots/plot_manifest.json",
];
const MULTISEED_PREDICTION_METRICS: &[&str] = &[
    "train_rows",
    "val_rows",
    "test_rows",
    "parameter_count",
    "val_latent_mse",
    "val_latent_cosine",
    "test_latent_mse",
    "test_latent_cosine",
    "val_action_contrast_acc",
    "random_init_cosine",
    "shuffled_target_cosine",
];
const MULTISEED_ABLATION_AGGREGATE_METRICS: &[&str] = &[
    "panel_slots",
    "input_dim",
    "target_dim",
    "latent_dim",
    "action_dim",
    "parameter_count",
    "val_latent_mse",
    "val_latent_cosine",
    "test_latent_mse",
    "test_latent_cosine",
    "val_action_contrast_acc",
    "random_init_cosine",
    "shuffled_target_cosine",
];
const MULTISEED_PAIRED_METRICS: &[&str] = &[
    "val_latent_mse",
    "val_latent_cosine",
    "test_latent_mse",
    "test_latent_cosine",
    "val_action_contrast_acc",
    "random_init_cosine",
    "shuffled_target_cosine",
];
const MULTISEED_BASELINE_ABLATION_ID: &str = "A0_taxonomy_only_action_vicreg";

#[derive(Debug, Clone)]
struct MultiseedBundle {
    canonical_path: String,
    run_id: String,
    status: String,
    fsv_status: String,
    purpose: String,
    career_event_count: Option<u64>,
    career_training_seed: Option<u64>,
    manifest_sha256: String,
    fsv_sha256: String,
    failure_code: Option<String>,
    failure_message: Option<String>,
    prediction_table: Option<CiCsvTable>,
    ablation_table: Option<CiCsvTable>,
}

impl MultiseedBundle {
    fn passed(&self) -> bool {
        self.status == "passed" && self.fsv_status == "passed"
    }
}

#[derive(Debug, Clone)]
struct MultiseedMetricSummary {
    key: String,
    metric: String,
    n: usize,
    mean: f64,
    std: f64,
    ci_lower: f64,
    ci_upper: f64,
    min: f64,
    max: f64,
    values: Vec<f64>,
}

#[derive(Debug, Clone)]
struct MultiseedAggregateTable {
    csv: String,
    row_count: usize,
    summaries: Vec<MultiseedMetricSummary>,
    key_order: Vec<String>,
    headers: Vec<String>,
}

#[derive(Debug, Clone)]
struct MultiseedPairedRow {
    metric: String,
    comparison_ablation_id: String,
    n: usize,
    mean_delta: f64,
    std_delta: f64,
    ci_lower: f64,
    ci_upper: f64,
    min_delta: f64,
    max_delta: f64,
    sign_flip_p: f64,
}

fn multiseed_aggregate_command(args: MultiseedAggregateArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    validate_multiseed_args(&args)?;

    let operation = "dynamicjepa_multiseed_aggregate";
    let before_state = json!({
        "bundle_root": ci_path_state(&args.bundle_root)?,
        "output_root": ci_path_state(&args.output_root)?,
    });
    if args.output_root.exists() {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.output_root",
            format!(
                "output root already exists: {}",
                args.output_root.display()
            ),
            "choose a fresh aggregate bundle path; multiseed aggregation never overwrites prior evidence",
        ));
    }

    let bundle_paths = discover_multiseed_bundle_paths(&args)?;
    if let Some(expected) = args.expected_bundle_count {
        if bundle_paths.len() != expected {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.expected_bundle_count",
                format!(
                    "matched {} child bundles, expected {expected}",
                    bundle_paths.len()
                ),
                "run the missing paper-small seeds or tighten --bundle/--bundle-glob to the intended immutable bundle set",
            ));
        }
    }

    let mut bundles = Vec::new();
    for path in &bundle_paths {
        bundles.push(read_multiseed_bundle(path, &args)?);
    }
    validate_multiseed_bundle_set(&bundles, &args)?;

    let passed = bundles
        .iter()
        .filter(|bundle| bundle.passed())
        .collect::<Vec<_>>();
    if passed.len() < args.min_passed {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.min_passed",
            format!(
                "only {} child bundles passed, minimum required is {}",
                passed.len(),
                args.min_passed
            ),
            "preserve failed bundles, fix the root cause, and rerun enough seeds to meet the paper-quality aggregate threshold",
        ));
    }

    let prediction = aggregate_multiseed_table(
        "table_prediction_metrics",
        "domain",
        MULTISEED_PREDICTION_METRICS,
        &passed,
        args.bootstrap_iters,
        args.bootstrap_seed,
        args.confidence,
    )?;
    let ablation = aggregate_multiseed_table(
        "table_ablation",
        "ablation_id",
        MULTISEED_ABLATION_AGGREGATE_METRICS,
        &passed,
        args.bootstrap_iters,
        args.bootstrap_seed ^ 0xA61A_7100_0000_0001,
        args.confidence,
    )?;
    let paired_tests = aggregate_multiseed_paired_tests(
        &passed,
        &ablation.key_order,
        args.bootstrap_iters,
        args.bootstrap_seed ^ 0x5EED_DA7A_0000_0002,
        args.confidence,
    )?;

    fs::create_dir_all(args.output_root.join("paper_tables")).map_err(|err| {
        DynamicJepaError::Storage {
            operation: "multiseed_aggregate.create_paper_tables".to_string(),
            cf: "filesystem".to_string(),
            message: format!(
                "failed to create {}: {err}",
                args.output_root.join("paper_tables").display()
            ),
            remediation: "choose a writable fresh --output-root".to_string(),
        }
    })?;
    fs::create_dir_all(args.output_root.join("plots")).map_err(|err| {
        DynamicJepaError::Storage {
            operation: "multiseed_aggregate.create_plots".to_string(),
            cf: "filesystem".to_string(),
            message: format!(
                "failed to create {}: {err}",
                args.output_root.join("plots").display()
            ),
            remediation: "choose a writable fresh --output-root".to_string(),
        }
    })?;

    write_text_file(
        &args
            .output_root
            .join("paper_tables/table_prediction_metrics_multiseed.csv"),
        &prediction.csv,
    )?;
    write_text_file(
        &args
            .output_root
            .join("paper_tables/table_ablation_multiseed.csv"),
        &ablation.csv,
    )?;
    write_text_file(
        &args
            .output_root
            .join("paper_tables/table_paired_ablation_tests.csv"),
        &paired_tests_csv(&paired_tests, &passed),
    )?;
    write_text_file(
        &args
            .output_root
            .join("paper_tables/table_seed_failures.csv"),
        &seed_failures_csv(&bundles),
    )?;
    write_multiseed_val_mse_box_svg(
        &args.output_root.join("plots/multiseed_val_mse_box.svg"),
        &prediction.summaries,
    )?;
    write_multiseed_delta_forest_svg(
        &args
            .output_root
            .join("plots/multiseed_paired_delta_forest.svg"),
        &paired_tests,
    )?;
    write_multiseed_plot_manifest(&args.output_root)?;

    let manifest = json!({
        "bundle_schema_version": DYNAMICJEPA_BUNDLE_SCHEMA_VERSION,
        "schema_version": MULTISEED_AGGREGATE_SCHEMA_VERSION,
        "status": "passed",
        "created_at_unix_ms": now_unix_ms()?,
        "operation": operation,
        "selection": {
            "bundle_root": args.bundle_root,
            "bundle_glob": args.bundle_glob,
            "explicit_bundles": args.bundles,
            "expected_bundle_count": args.expected_bundle_count,
            "min_passed": args.min_passed,
            "expected_purpose": args.expected_purpose,
            "expected_career_event_count": args.expected_career_event_count,
            "required_career_training_seeds": args.required_career_training_seeds,
        },
        "bootstrap": {
            "method": "BCa mean bootstrap",
            "iterations": args.bootstrap_iters,
            "seed": args.bootstrap_seed,
            "confidence": args.confidence,
        },
        "source_bundles": multiseed_bundle_manifest_rows(&bundles),
        "passed_count": passed.len(),
        "failed_count": bundles.iter().filter(|bundle| !bundle.passed()).count(),
        "passed_run_ids": passed.iter().map(|bundle| bundle.run_id.clone()).collect::<Vec<_>>(),
        "aggregate_tables": {
            "table_prediction_metrics_multiseed": {
                "row_count": prediction.row_count,
                "source_headers": prediction.headers,
                "keys": prediction.key_order,
            },
            "table_ablation_multiseed": {
                "row_count": ablation.row_count,
                "source_headers": ablation.headers,
                "keys": ablation.key_order,
            },
            "table_paired_ablation_tests": {
                "row_count": paired_tests.len(),
                "baseline_ablation_id": MULTISEED_BASELINE_ABLATION_ID,
            },
        },
    });
    write_json_file(&args.output_root.join("aggregate_manifest.json"), &manifest)?;

    let required_file_states =
        multiseed_relative_file_states(&args.output_root, MULTISEED_OUTPUT_REQUIRED_FILES)?;
    let fsv_report = json!({
        "schema_version": MULTISEED_AGGREGATE_SCHEMA_VERSION,
        "status": "passed",
        "source_of_truth": args.output_root,
        "created_at_unix_ms": now_unix_ms()?,
        "child_bundle_count": bundles.len(),
        "passed_child_bundle_count": passed.len(),
        "failed_child_bundle_count": bundles.iter().filter(|bundle| !bundle.passed()).count(),
        "child_manifest_hashes": bundles.iter().map(|bundle| json!({
            "run_id": bundle.run_id,
            "career_training_seed": bundle.career_training_seed,
            "status": bundle.status,
            "fsv_status": bundle.fsv_status,
            "run_manifest_sha256": bundle.manifest_sha256,
            "fsv_report_sha256": bundle.fsv_sha256,
            "path": bundle.canonical_path,
        })).collect::<Vec<_>>(),
        "required_file_states": required_file_states,
        "readback_checks": {
            "prediction_multiseed_rows": prediction.row_count,
            "ablation_multiseed_rows": ablation.row_count,
            "paired_ablation_test_rows": paired_tests.len(),
            "seed_failure_rows": bundles.iter().filter(|bundle| !bundle.passed()).count(),
        },
    });
    write_json_file(
        &args.output_root.join("aggregate_fsv_report.json"),
        &fsv_report,
    )?;

    let manifest_readback = read_json_file(&args.output_root.join("aggregate_manifest.json"))?;
    let fsv_readback = read_json_file(&args.output_root.join("aggregate_fsv_report.json"))?;
    let prediction_readback = read_ci_csv_table(
        &args
            .output_root
            .join("paper_tables/table_prediction_metrics_multiseed.csv"),
        "table_prediction_metrics_multiseed",
    )?;
    let ablation_readback = read_ci_csv_table(
        &args
            .output_root
            .join("paper_tables/table_ablation_multiseed.csv"),
        "table_ablation_multiseed",
    )?;
    let paired_readback = read_ci_csv_table(
        &args
            .output_root
            .join("paper_tables/table_paired_ablation_tests.csv"),
        "table_paired_ablation_tests",
    )?;
    let seed_failures_readback = read_ci_csv_table_allow_empty(
        &args
            .output_root
            .join("paper_tables/table_seed_failures.csv"),
        "table_seed_failures",
    )?;
    let after_state = json!({
        "output_root": ci_path_state(&args.output_root)?,
        "aggregate_manifest": ci_path_state(&args.output_root.join("aggregate_manifest.json"))?,
        "aggregate_fsv_report": ci_path_state(&args.output_root.join("aggregate_fsv_report.json"))?,
        "prediction_multiseed_table": ci_path_state(&args.output_root.join("paper_tables/table_prediction_metrics_multiseed.csv"))?,
        "ablation_multiseed_table": ci_path_state(&args.output_root.join("paper_tables/table_ablation_multiseed.csv"))?,
        "paired_ablation_tests_table": ci_path_state(&args.output_root.join("paper_tables/table_paired_ablation_tests.csv"))?,
        "seed_failures_table": ci_path_state(&args.output_root.join("paper_tables/table_seed_failures.csv"))?,
        "plots": [
            ci_path_state(&args.output_root.join("plots/multiseed_val_mse_box.svg"))?,
            ci_path_state(&args.output_root.join("plots/multiseed_paired_delta_forest.svg"))?,
            ci_path_state(&args.output_root.join("plots/plot_manifest.json"))?,
        ],
    });

    Ok(CommandOutcome {
        value: json!({
            "operation": operation,
            "status": "ok",
            "source_of_truth": args.output_root,
            "before_state": before_state,
            "after_state": after_state,
            "matched_bundle_count": bundles.len(),
            "passed_bundle_count": passed.len(),
            "failed_bundle_count": bundles.iter().filter(|bundle| !bundle.passed()).count(),
            "aggregate_manifest": manifest_readback,
            "aggregate_fsv_report": fsv_readback,
            "readback_row_counts": {
                "table_prediction_metrics_multiseed": prediction_readback.rows.len(),
                "table_ablation_multiseed": ablation_readback.rows.len(),
                "table_paired_ablation_tests": paired_readback.rows.len(),
                "table_seed_failures": seed_failures_readback.rows.len(),
            },
        }),
        exit_code: 0,
    })
}

fn validate_multiseed_args(args: &MultiseedAggregateArgs) -> DynamicJepaResult<()> {
    validate_research_purpose(&args.expected_purpose)?;
    if args.bundle_glob.is_some() != args.bundles.is_empty() {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.bundle_selection",
            "provide exactly one of --bundle-glob or repeated --bundle",
            "select one deterministic child bundle set for aggregate FSV",
        ));
    }
    if args.min_passed == 0 {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.min_passed",
            "--min-passed must be at least 1",
            "use at least one passed child bundle, and at least three for paper-quality aggregation",
        ));
    }
    if args.bootstrap_iters == 0 {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.bootstrap_iters",
            "--bootstrap-iters must be greater than zero",
            "use 10000 for paper tables unless debugging a local edge case",
        ));
    }
    if !(0.0..1.0).contains(&args.confidence) {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.confidence",
            format!("confidence must be in (0, 1), got {}", args.confidence),
            "use a probability such as 0.95",
        ));
    }
    let mut required = BTreeSet::new();
    for seed in &args.required_career_training_seeds {
        if !required.insert(*seed) {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.required_career_training_seeds",
                format!("duplicate required career training seed {seed}"),
                "list each required seed exactly once",
            ));
        }
    }
    Ok(())
}

fn discover_multiseed_bundle_paths(
    args: &MultiseedAggregateArgs,
) -> DynamicJepaResult<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if !args.bundles.is_empty() {
        for bundle in &args.bundles {
            paths.push(canonicalize_multiseed_path(
                bundle,
                "multiseed_aggregate.bundle",
            )?);
        }
    } else {
        let root =
            canonicalize_multiseed_path(&args.bundle_root, "multiseed_aggregate.bundle_root")?;
        let pattern = args.bundle_glob.as_deref().ok_or_else(|| {
            DynamicJepaError::validation(
                "multiseed_aggregate.bundle_glob",
                "missing --bundle-glob",
                "provide a deterministic child bundle name pattern",
            )
        })?;
        if pattern.contains('/') || pattern.contains('\\') {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.bundle_glob",
                format!("bundle glob must match child directory names only, got {pattern:?}"),
                "pass the parent directory via --bundle-root and keep --bundle-glob to a single path segment",
            ));
        }
        if pattern.contains('?') || pattern.contains('[') || pattern.contains(']') {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.bundle_glob",
                format!("unsupported glob syntax in {pattern:?}; only '*' is supported"),
                "use repeated --bundle for complex explicit selections",
            ));
        }
        for entry in fs::read_dir(&root).map_err(|err| DynamicJepaError::Storage {
            operation: "multiseed_aggregate.read_bundle_root".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to read {}: {err}", root.display()),
            remediation: "verify --bundle-root points at the research-smoke run root".to_string(),
        })? {
            let entry = entry.map_err(|err| DynamicJepaError::Storage {
                operation: "multiseed_aggregate.read_bundle_entry".to_string(),
                cf: "filesystem".to_string(),
                message: format!("failed to read an entry under {}: {err}", root.display()),
                remediation: "fix filesystem permissions under the research-smoke run root"
                    .to_string(),
            })?;
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if wildcard_match_star_only(pattern, &name) {
                let path = entry.path();
                let metadata = fs::metadata(&path).map_err(|err| DynamicJepaError::Storage {
                    operation: "multiseed_aggregate.bundle_metadata".to_string(),
                    cf: "filesystem".to_string(),
                    message: format!("failed to inspect {}: {err}", path.display()),
                    remediation: "remove broken bundle directory entries before aggregation"
                        .to_string(),
                })?;
                if !metadata.is_dir() {
                    return Err(DynamicJepaError::validation(
                        "multiseed_aggregate.bundle_entry",
                        format!("matched bundle path is not a directory: {}", path.display()),
                        "tighten --bundle-glob so it selects only research-smoke bundle directories",
                    ));
                }
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.bundle_selection",
            "matched zero child bundles",
            "run paper-small seed bundles first, then pass their directories through --bundle or --bundle-glob",
        ));
    }
    Ok(paths)
}

fn canonicalize_multiseed_path(path: &Path, field: &str) -> DynamicJepaResult<PathBuf> {
    fs::canonicalize(path).map_err(|err| {
        DynamicJepaError::validation(
            field,
            format!("failed to canonicalize {}: {err}", path.display()),
            "use an existing bundle path",
        )
    })
}

fn wildcard_match_star_only(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let mut remaining = value;
    let mut first = true;
    for part in pattern.split('*') {
        if part.is_empty() {
            first = false;
            continue;
        }
        if first && !pattern.starts_with('*') {
            let Some(rest) = remaining.strip_prefix(part) else {
                return false;
            };
            remaining = rest;
        } else if let Some(idx) = remaining.find(part) {
            remaining = &remaining[idx + part.len()..];
        } else {
            return false;
        }
        first = false;
    }
    pattern.ends_with('*') || remaining.is_empty()
}

fn read_multiseed_bundle(
    path: &Path,
    args: &MultiseedAggregateArgs,
) -> DynamicJepaResult<MultiseedBundle> {
    let canonical_path = canonicalize_multiseed_path(path, "multiseed_aggregate.bundle")?;
    let run_manifest_path = canonical_path.join("run_manifest.json");
    let fsv_report_path = canonical_path.join("fsv_report.json");
    let run_manifest = read_json_file(&run_manifest_path)?;
    validate_bundle_schema_manifest(&run_manifest_path, &run_manifest)?;
    let fsv_report = read_json_file(&fsv_report_path)?;
    let run_id = required_json_string(&run_manifest, "run_id", "run_manifest.json")?;
    let status = required_json_string(&run_manifest, "status", "run_manifest.json")?;
    let fsv_status = required_json_string(&fsv_report, "status", "fsv_report.json")?;
    let purpose = required_json_string(&run_manifest, "purpose", "run_manifest.json")?;
    if purpose != args.expected_purpose {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.purpose",
            format!(
                "bundle {} purpose is {purpose:?}, expected {:?}",
                canonical_path.display(),
                args.expected_purpose
            ),
            "aggregate only one research-smoke purpose at a time",
        ));
    }
    let career_event_count = run_manifest
        .get("career_event_count")
        .and_then(Value::as_u64);
    if let Some(expected) = args.expected_career_event_count {
        if career_event_count != Some(expected) {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.career_event_count",
                format!(
                    "bundle {} career_event_count is {:?}, expected {expected}",
                    canonical_path.display(),
                    career_event_count
                ),
                "aggregate only same-scale paper-small bundles",
            ));
        }
    }
    let career_training_seed = run_manifest
        .get("career_training_seed")
        .and_then(Value::as_u64);
    if !args.required_career_training_seeds.is_empty() && career_training_seed.is_none() {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.career_training_seed",
            format!("bundle {} missing career_training_seed", canonical_path.display()),
            "rerun paper-small with an explicit --career-training-seed so seed-level statistics are auditable",
        ));
    }

    let (failure_code, failure_message) = multiseed_failure_details(&run_manifest, &fsv_report);
    let passed = status == "passed" && fsv_status == "passed";
    let failed = status == "failed" && fsv_status == "failed";
    if !passed && !failed {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.bundle_status",
            format!(
                "bundle {} has partial status run_manifest={status:?} fsv_report={fsv_status:?}",
                canonical_path.display()
            ),
            "preserve the partial bundle for debugging, then rerun the seed into a fresh bundle",
        ));
    }

    let (prediction_table, ablation_table) = if passed {
        validate_multiseed_child_required_files(&canonical_path)?;
        (
            Some(read_ci_csv_table(
                &canonical_path.join("paper_tables/table_prediction_metrics.csv"),
                "table_prediction_metrics",
            )?),
            Some(read_ci_csv_table(
                &canonical_path.join("paper_tables/table_ablation.csv"),
                "table_ablation",
            )?),
        )
    } else {
        (None, None)
    };

    Ok(MultiseedBundle {
        canonical_path: canonical_path.display().to_string(),
        run_id,
        status,
        fsv_status,
        purpose,
        career_event_count,
        career_training_seed,
        manifest_sha256: hex_bytes(&file_sha256(&run_manifest_path)?),
        fsv_sha256: hex_bytes(&file_sha256(&fsv_report_path)?),
        failure_code,
        failure_message,
        prediction_table,
        ablation_table,
    })
}

fn required_json_string(value: &Value, key: &str, label: &str) -> DynamicJepaResult<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "multiseed_aggregate.json_field",
                format!("{label} missing non-empty string field {key}"),
                "rerun research-smoke so required manifest fields are written",
            )
        })
}

fn multiseed_failure_details(
    run_manifest: &Value,
    fsv_report: &Value,
) -> (Option<String>, Option<String>) {
    for source in [run_manifest, fsv_report] {
        if let Some(details) = source.get("failure_details").and_then(Value::as_object) {
            let code = details
                .get("error_code")
                .and_then(Value::as_str)
                .map(str::to_string);
            let message = details
                .get("error_message")
                .and_then(Value::as_str)
                .map(str::to_string);
            if code.is_some() || message.is_some() {
                return (code, message);
            }
        }
    }
    (None, None)
}

fn validate_multiseed_child_required_files(bundle: &Path) -> DynamicJepaResult<()> {
    for rel_path in MULTISEED_CHILD_REQUIRED_FILES {
        let path = bundle.join(rel_path);
        let metadata = fs::metadata(&path).map_err(|err| DynamicJepaError::Storage {
            operation: "multiseed_aggregate.child_required_file".to_string(),
            cf: "filesystem".to_string(),
            message: format!(
                "missing required passed child file {}: {err}",
                path.display()
            ),
            remediation:
                "passed bundles must contain complete FSV files, tables, artifact hashes, and plots"
                    .to_string(),
        })?;
        if !metadata.is_file() {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.child_required_file",
                format!(
                    "required child bundle path is not a file: {}",
                    path.display()
                ),
                "rerun the child paper-small bundle into a fresh directory",
            ));
        }
    }
    Ok(())
}

fn validate_multiseed_bundle_set(
    bundles: &[MultiseedBundle],
    args: &MultiseedAggregateArgs,
) -> DynamicJepaResult<()> {
    let mut run_ids = BTreeSet::new();
    let mut attempted_seeds = BTreeSet::new();
    let mut passed_seeds = BTreeSet::new();
    for bundle in bundles {
        if !run_ids.insert(bundle.run_id.clone()) {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.run_id",
                format!("duplicate child run_id {}", bundle.run_id),
                "aggregate immutable bundles with unique run_id values only",
            ));
        }
        if let Some(seed) = bundle.career_training_seed {
            attempted_seeds.insert(seed);
            if bundle.passed() && !passed_seeds.insert(seed) {
                return Err(DynamicJepaError::validation(
                    "multiseed_aggregate.career_training_seed",
                    format!("multiple passed bundles use career_training_seed {seed}"),
                    "select exactly one passed bundle per training seed; failed retries may be retained in table_seed_failures.csv",
                ));
            }
        }
    }
    if !args.required_career_training_seeds.is_empty() {
        let missing = args
            .required_career_training_seeds
            .iter()
            .copied()
            .filter(|seed| !attempted_seeds.contains(seed))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.required_career_training_seeds",
                format!("missing required career training seeds: {missing:?}"),
                "run and preserve a paper-small bundle for every required seed before aggregation",
            ));
        }
    }
    Ok(())
}

fn aggregate_multiseed_table(
    table_name: &str,
    key_column: &str,
    metric_columns: &[&str],
    passed: &[&MultiseedBundle],
    bootstrap_iters: u64,
    bootstrap_seed: u64,
    confidence: f64,
) -> DynamicJepaResult<MultiseedAggregateTable> {
    let first = passed.first().ok_or_else(|| {
        DynamicJepaError::validation(
            "multiseed_aggregate.passed_bundles",
            "no passed bundles available for table aggregation",
            "rerun enough child bundles to satisfy --min-passed",
        )
    })?;
    let first_table = multiseed_table_for_bundle(first, table_name)?;
    let headers = first_table.headers.clone();
    if !headers.iter().any(|header| header == key_column) {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.table_key",
            format!("{table_name} missing key column {key_column}"),
            "fix the child paper table writer before aggregation",
        ));
    }
    for metric in metric_columns {
        if !headers.iter().any(|header| header == metric) {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.table_metric",
                format!("{table_name} missing numeric metric column {metric}"),
                "fix the child paper table writer before aggregation",
            ));
        }
    }

    let first_rows = multiseed_rows_by_key(first_table, key_column, table_name)?;
    let key_order = first_rows.keys().cloned().collect::<Vec<_>>();
    for bundle in passed.iter().skip(1) {
        let table = multiseed_table_for_bundle(bundle, table_name)?;
        if table.headers != headers {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.table_schema",
                format!(
                    "{table_name} headers disagree for bundle {}: {:?} != {:?}",
                    bundle.run_id, table.headers, headers
                ),
                "only aggregate child bundles produced by the same table schema",
            ));
        }
        let rows = multiseed_rows_by_key(table, key_column, table_name)?;
        let keys = rows.keys().cloned().collect::<Vec<_>>();
        if keys != key_order {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.table_keys",
                format!(
                    "{table_name} key set/order disagrees for bundle {}: {:?} != {:?}",
                    bundle.run_id, keys, key_order
                ),
                "investigate missing or extra table rows before computing multiseed means",
            ));
        }
    }

    let mut csv = format!(
        "{key_column},metric,n,mean,std,bca_lower,bca_upper,min,max,source_run_ids,source_career_training_seeds\n"
    );
    let source_run_ids = passed
        .iter()
        .map(|bundle| bundle.run_id.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let source_seeds = passed
        .iter()
        .map(|bundle| {
            bundle
                .career_training_seed
                .map(|seed| seed.to_string())
                .unwrap_or_else(|| "NA".to_string())
        })
        .collect::<Vec<_>>()
        .join(";");
    let mut summaries = Vec::new();
    for key in &key_order {
        for metric in metric_columns {
            let mut values = Vec::new();
            for bundle in passed {
                let table = multiseed_table_for_bundle(bundle, table_name)?;
                let rows = multiseed_rows_by_key(table, key_column, table_name)?;
                let row = rows.get(key).ok_or_else(|| {
                    DynamicJepaError::validation(
                        "multiseed_aggregate.table_key",
                        format!("{table_name} missing key {key} in bundle {}", bundle.run_id),
                        "child table keys must be identical across passed bundles",
                    )
                })?;
                values.push(parse_multiseed_f64(
                    row.get(*metric).map(String::as_str),
                    table_name,
                    key,
                    metric,
                    &bundle.run_id,
                )?);
            }
            let summary = summarize_multiseed_metric(
                key,
                metric,
                &values,
                bootstrap_iters,
                bootstrap_seed ^ stable_u64(&format!("{table_name}:{key}:{metric}")),
                confidence,
            )?;
            csv.push_str(&format!(
                "{},{},{},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{},{}\n",
                csv_escape(&summary.key),
                csv_escape(&summary.metric),
                summary.n,
                summary.mean,
                summary.std,
                summary.ci_lower,
                summary.ci_upper,
                summary.min,
                summary.max,
                csv_escape(&source_run_ids),
                csv_escape(&source_seeds),
            ));
            summaries.push(summary);
        }
    }
    Ok(MultiseedAggregateTable {
        row_count: summaries.len(),
        csv,
        summaries,
        key_order,
        headers,
    })
}

fn multiseed_table_for_bundle<'a>(
    bundle: &'a MultiseedBundle,
    table_name: &str,
) -> DynamicJepaResult<&'a CiCsvTable> {
    match table_name {
        "table_prediction_metrics" => bundle.prediction_table.as_ref(),
        "table_ablation" => bundle.ablation_table.as_ref(),
        _ => None,
    }
    .ok_or_else(|| {
        DynamicJepaError::validation(
            "multiseed_aggregate.table",
            format!("bundle {} missing passed table {table_name}", bundle.run_id),
            "only passed bundles with complete paper tables can be aggregated",
        )
    })
}

fn multiseed_rows_by_key(
    table: &CiCsvTable,
    key_column: &str,
    table_name: &str,
) -> DynamicJepaResult<BTreeMap<String, BTreeMap<String, String>>> {
    let mut rows = BTreeMap::new();
    for row in &table.rows {
        let key = row
            .get(key_column)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                DynamicJepaError::validation(
                    "multiseed_aggregate.table_key",
                    format!("{table_name} row missing non-empty key column {key_column}"),
                    "fix the child paper table writer before aggregation",
                )
            })?
            .clone();
        if rows.insert(key.clone(), row.clone()).is_some() {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.table_key",
                format!("{table_name} contains duplicate key {key}"),
                "paper tables must have one row per aggregate key",
            ));
        }
    }
    Ok(rows)
}

fn parse_multiseed_f64(
    value: Option<&str>,
    table_name: &str,
    key: &str,
    metric: &str,
    run_id: &str,
) -> DynamicJepaResult<f64> {
    let raw = value.ok_or_else(|| {
        DynamicJepaError::validation(
            "multiseed_aggregate.numeric_metric",
            format!("{table_name} {key}.{metric} missing in bundle {run_id}"),
            "child paper tables must contain all numeric metrics",
        )
    })?;
    let parsed = raw.parse::<f64>().map_err(|err| {
        DynamicJepaError::validation(
            "multiseed_aggregate.numeric_metric",
            format!("{table_name} {key}.{metric}={raw:?} is not numeric in bundle {run_id}: {err}"),
            "fix metric serialization before aggregation",
        )
    })?;
    if !parsed.is_finite() {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.numeric_metric",
            format!("{table_name} {key}.{metric} is not finite in bundle {run_id}: {parsed}"),
            "fix training or table generation so paper metrics are finite",
        ));
    }
    Ok(parsed)
}

fn summarize_multiseed_metric(
    key: &str,
    metric: &str,
    values: &[f64],
    bootstrap_iters: u64,
    bootstrap_seed: u64,
    confidence: f64,
) -> DynamicJepaResult<MultiseedMetricSummary> {
    let (mean, std, min, max) = mean_std_min_max(values)?;
    let (ci_lower, ci_upper) =
        bca_bootstrap_mean_ci(values, bootstrap_iters, bootstrap_seed, confidence)?;
    Ok(MultiseedMetricSummary {
        key: key.to_string(),
        metric: metric.to_string(),
        n: values.len(),
        mean,
        std,
        ci_lower,
        ci_upper,
        min,
        max,
        values: values.to_vec(),
    })
}

fn aggregate_multiseed_paired_tests(
    passed: &[&MultiseedBundle],
    ablation_key_order: &[String],
    bootstrap_iters: u64,
    bootstrap_seed: u64,
    confidence: f64,
) -> DynamicJepaResult<Vec<MultiseedPairedRow>> {
    if !ablation_key_order
        .iter()
        .any(|key| key == MULTISEED_BASELINE_ABLATION_ID)
    {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.baseline_ablation",
            format!(
                "ablation table missing baseline {}",
                MULTISEED_BASELINE_ABLATION_ID
            ),
            "restore the A0 baseline ablation before computing paired tests",
        ));
    }
    let mut rows = Vec::new();
    for comparison in ablation_key_order {
        if comparison == MULTISEED_BASELINE_ABLATION_ID {
            continue;
        }
        for metric in MULTISEED_PAIRED_METRICS {
            let mut deltas = Vec::new();
            for bundle in passed {
                let table = multiseed_table_for_bundle(bundle, "table_ablation")?;
                let rows_by_key = multiseed_rows_by_key(table, "ablation_id", "table_ablation")?;
                let baseline_row =
                    rows_by_key
                        .get(MULTISEED_BASELINE_ABLATION_ID)
                        .ok_or_else(|| {
                            DynamicJepaError::validation(
                                "multiseed_aggregate.baseline_ablation",
                                format!(
                                    "bundle {} missing baseline {}",
                                    bundle.run_id, MULTISEED_BASELINE_ABLATION_ID
                                ),
                                "all passed child bundles must include the same ablation baseline",
                            )
                        })?;
                let comparison_row = rows_by_key.get(comparison).ok_or_else(|| {
                    DynamicJepaError::validation(
                        "multiseed_aggregate.comparison_ablation",
                        format!("bundle {} missing comparison {comparison}", bundle.run_id),
                        "all passed child bundles must include the same ablation comparisons",
                    )
                })?;
                let baseline = parse_multiseed_f64(
                    baseline_row.get(*metric).map(String::as_str),
                    "table_ablation",
                    MULTISEED_BASELINE_ABLATION_ID,
                    metric,
                    &bundle.run_id,
                )?;
                let value = parse_multiseed_f64(
                    comparison_row.get(*metric).map(String::as_str),
                    "table_ablation",
                    comparison,
                    metric,
                    &bundle.run_id,
                )?;
                deltas.push(value - baseline);
            }
            let (mean_delta, std_delta, min_delta, max_delta) = mean_std_min_max(&deltas)?;
            let (ci_lower, ci_upper) = bca_bootstrap_mean_ci(
                &deltas,
                bootstrap_iters,
                bootstrap_seed ^ stable_u64(&format!("{comparison}:{metric}:paired_delta")),
                confidence,
            )?;
            let sign_flip_p = paired_sign_flip_p_value(&deltas)?;
            rows.push(MultiseedPairedRow {
                metric: (*metric).to_string(),
                comparison_ablation_id: comparison.clone(),
                n: deltas.len(),
                mean_delta,
                std_delta,
                ci_lower,
                ci_upper,
                min_delta,
                max_delta,
                sign_flip_p,
            });
        }
    }
    Ok(rows)
}

fn paired_tests_csv(rows: &[MultiseedPairedRow], passed: &[&MultiseedBundle]) -> String {
    let source_run_ids = passed
        .iter()
        .map(|bundle| bundle.run_id.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let source_seeds = passed
        .iter()
        .map(|bundle| {
            bundle
                .career_training_seed
                .map(|seed| seed.to_string())
                .unwrap_or_else(|| "NA".to_string())
        })
        .collect::<Vec<_>>()
        .join(";");
    let mut csv = "metric,baseline_ablation_id,comparison_ablation_id,n,mean_delta,std_delta,bca_lower,bca_upper,min_delta,max_delta,sign_flip_p,source_run_ids,source_career_training_seeds\n".to_string();
    for row in rows {
        csv.push_str(&format!(
            "{},{},{},{},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{},{}\n",
            csv_escape(&row.metric),
            csv_escape(MULTISEED_BASELINE_ABLATION_ID),
            csv_escape(&row.comparison_ablation_id),
            row.n,
            row.mean_delta,
            row.std_delta,
            row.ci_lower,
            row.ci_upper,
            row.min_delta,
            row.max_delta,
            row.sign_flip_p,
            csv_escape(&source_run_ids),
            csv_escape(&source_seeds),
        ));
    }
    csv
}

fn seed_failures_csv(bundles: &[MultiseedBundle]) -> String {
    let mut csv = "run_id,career_training_seed,status,fsv_status,error_code,error_message,run_manifest_sha256,fsv_report_sha256,bundle_path\n".to_string();
    for bundle in bundles.iter().filter(|bundle| !bundle.passed()) {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&bundle.run_id),
            bundle
                .career_training_seed
                .map(|seed| seed.to_string())
                .unwrap_or_else(|| "NA".to_string()),
            csv_escape(&bundle.status),
            csv_escape(&bundle.fsv_status),
            csv_escape(bundle.failure_code.as_deref().unwrap_or("UNKNOWN_FAILURE")),
            csv_escape(
                bundle
                    .failure_message
                    .as_deref()
                    .unwrap_or("missing failure_details")
            ),
            csv_escape(&bundle.manifest_sha256),
            csv_escape(&bundle.fsv_sha256),
            csv_escape(&bundle.canonical_path),
        ));
    }
    csv
}

fn multiseed_bundle_manifest_rows(bundles: &[MultiseedBundle]) -> Vec<Value> {
    bundles
        .iter()
        .map(|bundle| {
            json!({
                "run_id": bundle.run_id,
                "status": bundle.status,
                "fsv_status": bundle.fsv_status,
                "purpose": bundle.purpose,
                "career_event_count": bundle.career_event_count,
                "career_training_seed": bundle.career_training_seed,
                "run_manifest_sha256": bundle.manifest_sha256,
                "fsv_report_sha256": bundle.fsv_sha256,
                "failure_code": bundle.failure_code,
                "failure_message": bundle.failure_message,
                "path": bundle.canonical_path,
            })
        })
        .collect()
}

fn multiseed_relative_file_states(
    root: &Path,
    rel_paths: &[&str],
) -> DynamicJepaResult<Vec<Value>> {
    let mut states = Vec::new();
    for rel_path in rel_paths {
        let path = root.join(rel_path);
        let metadata = fs::metadata(&path).map_err(|err| DynamicJepaError::Storage {
            operation: "multiseed_aggregate.output_required_file".to_string(),
            cf: "filesystem".to_string(),
            message: format!("missing aggregate output file {}: {err}", path.display()),
            remediation:
                "fix the aggregate writer; all declared output tables and plots must exist"
                    .to_string(),
        })?;
        if !metadata.is_file() {
            return Err(DynamicJepaError::validation(
                "multiseed_aggregate.output_required_file",
                format!("aggregate output path is not a file: {}", path.display()),
                "fix the aggregate writer so required outputs are regular files",
            ));
        }
        states.push(json!({
            "path": rel_path,
            "sha256": hex_bytes(&file_sha256(&path)?),
            "size_bytes": metadata.len(),
        }));
    }
    Ok(states)
}

fn mean_std_min_max(values: &[f64]) -> DynamicJepaResult<(f64, f64, f64, f64)> {
    if values.is_empty() {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.values",
            "cannot summarize an empty metric series",
            "ensure at least one passed bundle contributes each metric",
        ));
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let std = if values.len() > 1 {
        let variance = values
            .iter()
            .map(|value| {
                let delta = value - mean;
                delta * delta
            })
            .sum::<f64>()
            / (values.len() - 1) as f64;
        variance.sqrt()
    } else {
        0.0
    };
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Ok((mean, std, min, max))
}

fn bca_bootstrap_mean_ci(
    values: &[f64],
    iterations: u64,
    seed: u64,
    confidence: f64,
) -> DynamicJepaResult<(f64, f64)> {
    if values.is_empty() {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.bootstrap_values",
            "cannot bootstrap empty values",
            "ensure table aggregation receives passed child rows",
        ));
    }
    if values.len() == 1 {
        return Ok((values[0], values[0]));
    }
    let observed = values.iter().sum::<f64>() / values.len() as f64;
    let mut rng = DeterministicRng::new(seed);
    let mut boot = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        let mut sum = 0.0;
        for _ in 0..values.len() {
            sum += values[rng.next_usize(values.len())];
        }
        boot.push(sum / values.len() as f64);
    }
    boot.sort_by(|left, right| left.total_cmp(right));

    let less = boot.iter().filter(|value| **value < observed).count() as f64;
    let denominator = iterations as f64;
    let proportion_less = (less / denominator).clamp(0.5 / denominator, 1.0 - 0.5 / denominator);
    let z0 = inverse_standard_normal_cdf(proportion_less);

    let mut jackknife = Vec::with_capacity(values.len());
    for idx in 0..values.len() {
        let mut sum = 0.0;
        for (value_idx, value) in values.iter().enumerate() {
            if value_idx != idx {
                sum += value;
            }
        }
        jackknife.push(sum / (values.len() - 1) as f64);
    }
    let jack_mean = jackknife.iter().sum::<f64>() / jackknife.len() as f64;
    let numerator = jackknife
        .iter()
        .map(|value| {
            let delta = jack_mean - value;
            delta * delta * delta
        })
        .sum::<f64>();
    let sum_squares = jackknife
        .iter()
        .map(|value| {
            let delta = jack_mean - value;
            delta * delta
        })
        .sum::<f64>();
    let acceleration_denominator = 6.0 * sum_squares.powf(1.5);
    let acceleration = if acceleration_denominator.abs() > f64::EPSILON {
        numerator / acceleration_denominator
    } else {
        0.0
    };
    let alpha_low = (1.0 - confidence) / 2.0;
    let alpha_high = 1.0 - alpha_low;
    let adjusted_low = bca_adjusted_alpha(alpha_low, z0, acceleration);
    let adjusted_high = bca_adjusted_alpha(alpha_high, z0, acceleration);
    Ok((
        percentile_sorted(&boot, adjusted_low),
        percentile_sorted(&boot, adjusted_high),
    ))
}

fn bca_adjusted_alpha(alpha: f64, z0: f64, acceleration: f64) -> f64 {
    let z_alpha = inverse_standard_normal_cdf(alpha);
    let numerator = z0 + z_alpha;
    let denominator = 1.0 - acceleration * numerator;
    standard_normal_cdf(z0 + numerator / denominator)
}

fn percentile_sorted(sorted: &[f64], probability: f64) -> f64 {
    let probability = probability.clamp(0.0, 1.0);
    if sorted.len() == 1 {
        return sorted[0];
    }
    let position = probability * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let weight = position - lower as f64;
        sorted[lower] * (1.0 - weight) + sorted[upper] * weight
    }
}

fn paired_sign_flip_p_value(deltas: &[f64]) -> DynamicJepaResult<f64> {
    if deltas.is_empty() {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.sign_flip",
            "cannot compute paired sign-flip p-value for zero deltas",
            "ensure at least one passed bundle contributes paired ablation rows",
        ));
    }
    if deltas.len() > 20 {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.sign_flip",
            format!("exact sign-flip test supports at most 20 pairs, got {}", deltas.len()),
            "implement an explicitly reviewed Monte Carlo sign-flip method before using larger seed panels",
        ));
    }
    let observed = (deltas.iter().sum::<f64>() / deltas.len() as f64).abs();
    let total = 1_u64 << deltas.len();
    let mut extreme = 0_u64;
    for mask in 0..total {
        let mut sum = 0.0;
        for (idx, delta) in deltas.iter().enumerate() {
            if (mask >> idx) & 1 == 1 {
                sum += delta;
            } else {
                sum -= delta;
            }
        }
        if (sum / deltas.len() as f64).abs() + 1e-15 >= observed {
            extreme += 1;
        }
    }
    Ok(extreme as f64 / total as f64)
}

struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_usize(&mut self, upper: usize) -> usize {
        (self.next_u64() % upper as u64) as usize
    }
}

fn stable_u64(input: &str) -> u64 {
    let hash = sha256(input.as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&hash[..8]);
    u64::from_le_bytes(bytes)
}

fn inverse_standard_normal_cdf(p: f64) -> f64 {
    let a = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    let b = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    let c = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    let d = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let plow = 0.02425;
    let phigh = 1.0 - plow;
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    if p < plow {
        let q = (-2.0 * p.ln()).sqrt();
        return (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0);
    }
    if p > phigh {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        return -(((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0);
    }
    let q = p - 0.5;
    let r = q * q;
    (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q
        / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
}

fn standard_normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf_approx(x / 2.0_f64.sqrt()))
}

fn erf_approx(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    sign * y
}

fn write_multiseed_val_mse_box_svg(
    path: &Path,
    summaries: &[MultiseedMetricSummary],
) -> DynamicJepaResult<()> {
    let rows = summaries
        .iter()
        .filter(|summary| summary.metric == "val_latent_mse")
        .map(|summary| (summary.key.clone(), summary.values.clone()))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.val_mse_plot",
            "no val_latent_mse rows available for multiseed box plot",
            "ensure prediction aggregate includes val_latent_mse",
        ));
    }
    let width = (220.0 + rows.len() as f64 * 170.0).max(760.0);
    let height = 430.0;
    let left = 82.0;
    let right = 36.0;
    let top = 56.0;
    let bottom = 112.0;
    let plot_width = width - left - right;
    let plot_height = height - top - bottom;
    let max_value = rows
        .iter()
        .flat_map(|(_, values)| values.iter().copied())
        .fold(0.0_f64, f64::max)
        .max(1e-9);
    let slot = plot_width / rows.len() as f64;
    let mut marks = String::new();
    for (idx, (label, values)) in rows.iter().enumerate() {
        let mut sorted = values.clone();
        sorted.sort_by(|left, right| left.total_cmp(right));
        let min = percentile_sorted(&sorted, 0.0);
        let q1 = percentile_sorted(&sorted, 0.25);
        let median = percentile_sorted(&sorted, 0.5);
        let q3 = percentile_sorted(&sorted, 0.75);
        let max = percentile_sorted(&sorted, 1.0);
        let x = left + slot * idx as f64 + slot / 2.0;
        let y = |value: f64| top + plot_height - plot_height * (value / max_value);
        let box_width = 58.0;
        marks.push_str(&format!(
            r##"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{:.2}" stroke="#1f2937" stroke-width="1.5"/>
<rect x="{:.2}" y="{:.2}" width="{box_width:.2}" height="{:.2}" fill="#dbeafe" stroke="#1d4ed8" stroke-width="1.5"/>
<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#1d4ed8" stroke-width="2"/>
<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#1f2937" stroke-width="1.5"/>
<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#1f2937" stroke-width="1.5"/>
<text x="{x:.2}" y="352" text-anchor="middle" font-size="12" fill="#111827" transform="rotate(-22 {x:.2} 352)">{}</text>
"##,
            y(min),
            y(max),
            x - box_width / 2.0,
            y(q3),
            (y(q1) - y(q3)).max(1.0),
            x - box_width / 2.0,
            y(median),
            x + box_width / 2.0,
            y(median),
            x - 18.0,
            y(min),
            x + 18.0,
            y(min),
            x - 18.0,
            y(max),
            x + 18.0,
            y(max),
            xml_escape(label),
        ));
    }
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}" role="img" aria-label="Multiseed validation MSE box plot">
<rect width="100%" height="100%" fill="#ffffff"/>
<text x="{left:.0}" y="30" font-size="18" font-family="Arial, sans-serif" font-weight="700" fill="#111827">Multiseed validation latent MSE</text>
<line x1="{left:.0}" y1="{top:.0}" x2="{left:.0}" y2="{:.0}" stroke="#374151" stroke-width="1"/>
<line x1="{left:.0}" y1="{:.0}" x2="{:.0}" y2="{:.0}" stroke="#374151" stroke-width="1"/>
<text x="18" y="{:.0}" font-size="12" font-family="Arial, sans-serif" fill="#111827" transform="rotate(-90 18 {:.0})">latent MSE</text>
<text x="20" y="{top:.0}" font-size="11" font-family="Arial, sans-serif" fill="#374151">{max_value:.4}</text>
{marks}</svg>
"##,
        top + plot_height,
        top + plot_height,
        width - right,
        top + plot_height,
        top + plot_height / 2.0,
        top + plot_height / 2.0
    );
    write_text_file(path, &svg)
}

fn write_multiseed_delta_forest_svg(
    path: &Path,
    rows: &[MultiseedPairedRow],
) -> DynamicJepaResult<()> {
    let rows = rows
        .iter()
        .filter(|row| row.metric == "val_latent_mse")
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Err(DynamicJepaError::validation(
            "multiseed_aggregate.delta_plot",
            "no val_latent_mse paired deltas available for forest plot",
            "ensure paired ablation tests include val_latent_mse",
        ));
    }
    let width = 840.0;
    let height = (150.0 + rows.len() as f64 * 48.0).max(420.0);
    let left = 270.0;
    let right = 54.0;
    let top = 60.0;
    let bottom = 60.0;
    let plot_width = width - left - right;
    let plot_height = height - top - bottom;
    let min_x = rows
        .iter()
        .map(|row| row.ci_lower.min(row.mean_delta))
        .fold(0.0_f64, f64::min);
    let max_x = rows
        .iter()
        .map(|row| row.ci_upper.max(row.mean_delta))
        .fold(0.0_f64, f64::max);
    let span = (max_x - min_x).abs().max(1e-9);
    let x_for = |value: f64| left + (value - min_x) / span * plot_width;
    let zero_x = x_for(0.0);
    let mut marks = String::new();
    for (idx, row) in rows.iter().enumerate() {
        let y = top + (idx as f64 + 0.5) * (plot_height / rows.len() as f64);
        marks.push_str(&format!(
            r##"<text x="20" y="{:.2}" font-size="12" font-family="Arial, sans-serif" fill="#111827">{}</text>
<line x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="#374151" stroke-width="1.5"/>
<circle cx="{:.2}" cy="{y:.2}" r="4.5" fill="#dc2626"/>
<text x="{:.2}" y="{:.2}" font-size="11" font-family="Arial, sans-serif" fill="#374151">{:.4}</text>
"##,
            y + 4.0,
            xml_escape(&row.comparison_ablation_id),
            x_for(row.ci_lower),
            x_for(row.ci_upper),
            x_for(row.mean_delta),
            x_for(row.mean_delta) + 8.0,
            y + 4.0,
            row.mean_delta,
        ));
    }
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}" role="img" aria-label="Paired validation MSE delta forest plot">
<rect width="100%" height="100%" fill="#ffffff"/>
<text x="20" y="30" font-size="18" font-family="Arial, sans-serif" font-weight="700" fill="#111827">Paired val MSE delta vs A0 baseline</text>
<line x1="{zero_x:.2}" y1="{top:.0}" x2="{zero_x:.2}" y2="{:.0}" stroke="#9ca3af" stroke-width="1" stroke-dasharray="4 4"/>
<line x1="{left:.0}" y1="{:.0}" x2="{:.0}" y2="{:.0}" stroke="#374151" stroke-width="1"/>
<text x="{left:.0}" y="{:.0}" font-size="11" font-family="Arial, sans-serif" fill="#374151">{min_x:.4}</text>
<text x="{:.0}" y="{:.0}" text-anchor="end" font-size="11" font-family="Arial, sans-serif" fill="#374151">{max_x:.4}</text>
{marks}</svg>
"##,
        height - bottom,
        height - bottom,
        width - right,
        height - bottom,
        height - 24.0,
        width - right,
        height - 24.0
    );
    write_text_file(path, &svg)
}

fn write_multiseed_plot_manifest(output_root: &Path) -> DynamicJepaResult<()> {
    write_json_file(
        &output_root.join("plots/plot_manifest.json"),
        &json!({
            "status": "rendered",
            "plots": [
                {
                    "id": "multiseed_val_mse_box",
                    "source_table": output_root.join("paper_tables/table_prediction_metrics_multiseed.csv"),
                    "output": output_root.join("plots/multiseed_val_mse_box.svg"),
                },
                {
                    "id": "multiseed_paired_delta_forest",
                    "source_table": output_root.join("paper_tables/table_paired_ablation_tests.csv"),
                    "output": output_root.join("plots/multiseed_paired_delta_forest.svg"),
                }
            ]
        }),
    )
}

const CI_REFERENCE_SCHEMA_VERSION: u64 = 1;
const CI_REFERENCE_REQUIRED_FILES: &[&str] = &[
    "run_manifest.json",
    "fsv_report.json",
    "data_manifest.json",
    "artifact_hashes.json",
    "paper_tables/table_prediction_metrics.csv",
    "paper_tables/table_ablation.csv",
    "paper_tables/table_edge_cases.csv",
    "plots/plot_manifest.json",
    "plots/prediction_val_mse.svg",
    "plots/career_ablation_val_mse.svg",
];

fn capture_reference_command(args: CaptureReferenceArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let bundle_path = canonical_existing_path(&args.bundle, "capture-reference.bundle")?;
    let reference = capture_ci_reference_value(&args.reference_id, &args.bundle)?;
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent).map_err(|err| DynamicJepaError::Storage {
            operation: "capture_reference.create_parent".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to create {}: {err}", parent.display()),
            remediation: "choose a writable --output path under the repository reference directory"
                .to_string(),
        })?;
    }
    write_json_file(&args.output, &reference)?;
    let output_path = canonical_existing_path(&args.output, "capture-reference.output")?;
    let readback = read_json_file(&args.output)?;

    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_capture_reference",
            "status": "ok",
            "source_of_truth": [output_path],
            "input_bundle": bundle_path,
            "readback": readback,
        }),
        exit_code: 0,
    })
}

fn check_reference_command(args: CheckReferenceArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let operation = "dynamicjepa_check_reference";
    let source_of_truth = json!({
        "reference": args.reference.display().to_string(),
        "tolerances": args.tolerances.display().to_string(),
        "bundle": args.bundle.display().to_string(),
    });
    let before_state = json!({
        "reference": ci_path_state(&args.reference)?,
        "tolerances": ci_path_state(&args.tolerances)?,
        "bundle": ci_path_state(&args.bundle)?,
    });

    let reference = match read_ci_json_for_check(&args.reference) {
        Ok(value) => value,
        Err(message) => {
            return Ok(ci_reference_error(
                operation,
                "REFERENCE_CORRUPTED",
                message,
                "restore a valid checked-in reference JSON generated by dynamicjepa capture-reference",
                source_of_truth,
                before_state,
                None,
            ));
        }
    };
    let tolerances = match read_ci_json_for_check(&args.tolerances) {
        Ok(value) => value,
        Err(message) => {
            return Ok(ci_reference_error(
                operation,
                "REFERENCE_CORRUPTED",
                message,
                "restore a valid checked-in tolerance JSON before rerunning CI",
                source_of_truth,
                before_state,
                None,
            ));
        }
    };
    if let Err(message) = validate_ci_reference_schema(&reference) {
        return Ok(ci_reference_error(
            operation,
            "REFERENCE_CORRUPTED",
            message,
            "regenerate the reference with dynamicjepa capture-reference and commit the full JSON",
            source_of_truth,
            before_state,
            None,
        ));
    }
    if !tolerances.is_object() {
        return Ok(ci_reference_error(
            operation,
            "REFERENCE_CORRUPTED",
            "tolerances JSON root is not an object".to_string(),
            "restore reference/tolerances.json with per-table numeric tolerances",
            source_of_truth,
            before_state,
            None,
        ));
    }

    let current = match capture_ci_reference_value("current_bundle_probe", &args.bundle) {
        Ok(value) => value,
        Err(err) => {
            return Ok(ci_reference_error(
                operation,
                "BUNDLE_INCOMPLETE",
                err.to_string(),
                "inspect the run bundle and rerun research-smoke until all required FSV files, CSVs, hashes, and plots exist",
                source_of_truth,
                before_state,
                None,
            ));
        }
    };

    let comparison = match compare_ci_reference(&reference, &current, &tolerances) {
        Ok(value) => value,
        Err(CiReferenceCheckFailure { code, message }) => {
            return Ok(ci_reference_error(
                operation,
                code,
                message,
                "inspect the drift report, fix the root cause, and only update references through the bundle-bump workflow",
                source_of_truth,
                before_state,
                Some(json!({"current_probe": current})),
            ));
        }
    };

    if args.require_artifact_hash_match {
        match compare_ci_artifact_hashes(&reference, &current) {
            Ok(artifact_comparison) => {
                if artifact_comparison["mismatches"]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
                {
                    return Ok(ci_reference_error(
                        operation,
                        "ARTIFACT_HASH_DRIFT",
                        "one or more registered artifact hashes differ from the reference".to_string(),
                        "do not update the reference until the model artifact drift is explained and approved as a bundle-bump",
                        source_of_truth,
                        before_state,
                        Some(json!({
                            "comparison": comparison,
                            "artifact_comparison": artifact_comparison,
                            "current_probe": current,
                        })),
                    ));
                }
            }
            Err(CiReferenceCheckFailure { code, message }) => {
                return Ok(ci_reference_error(
                    operation,
                    code,
                    message,
                    "inspect artifact_hashes.json in both bundles; required artifact hashes must be complete and parseable",
                    source_of_truth,
                    before_state,
                    Some(json!({"current_probe": current})),
                ));
            }
        }
    }

    let bit_equal = comparison["file_hash_mismatches"]
        .as_array()
        .is_some_and(|items| items.is_empty());
    let status = if bit_equal {
        "bit_equal"
    } else {
        "within_tolerance"
    };
    let after_state = json!({
        "reference": ci_path_state(&args.reference)?,
        "tolerances": ci_path_state(&args.tolerances)?,
        "bundle": ci_path_state(&args.bundle)?,
    });

    Ok(CommandOutcome {
        value: json!({
            "operation": operation,
            "status": status,
            "source_of_truth": source_of_truth,
            "before_state": before_state,
            "after_state": after_state,
            "comparison": comparison,
            "current_probe": current,
        }),
        exit_code: 0,
    })
}

fn check_release_aggregate_reference_command(
    args: CheckReleaseAggregateReferenceArgs,
) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let operation = "dynamicjepa_check_release_aggregate_reference";
    let manifest_path = args.aggregate.join("aggregate_manifest.json");
    let fsv_path = args.aggregate.join("aggregate_fsv_report.json");
    let source_of_truth = json!({
        "aggregate": args.aggregate.display().to_string(),
        "reference": args.reference.display().to_string(),
        "aggregate_manifest": manifest_path.display().to_string(),
        "aggregate_fsv_report": fsv_path.display().to_string(),
        "paper_tables": args.aggregate.join("paper_tables").display().to_string(),
    });
    let before_state = json!({
        "aggregate": ci_path_state(&args.aggregate)?,
        "reference": ci_path_state(&args.reference)?,
    });

    if !args.aggregate.is_dir() {
        return Ok(ci_reference_error(
            operation,
            "VALIDATION",
            format!(
                "aggregate path does not exist or is not a directory: {}",
                args.aggregate.display()
            ),
            "run dynamicjepa multiseed-aggregate before checking the release reference",
            source_of_truth,
            before_state,
            None,
        ));
    }
    if !args.reference.is_file() {
        return Ok(ci_reference_error(
            operation,
            "VALIDATION",
            format!(
                "reference path does not exist or is not a file: {}",
                args.reference.display()
            ),
            "capture or restore reference/reference_release_v1.0.0.json",
            source_of_truth,
            before_state,
            None,
        ));
    }

    let reference = match read_ci_json_for_check(&args.reference) {
        Ok(value) => value,
        Err(message) => {
            return Ok(ci_reference_error(
                operation,
                "SCHEMA",
                message,
                "restore a valid checked-in release aggregate reference JSON",
                source_of_truth,
                before_state,
                None,
            ));
        }
    };
    if reference.get("schema_version").and_then(Value::as_u64) != Some(1)
        || reference.get("kind").and_then(Value::as_str)
            != Some("dynamicjepa_release_aggregate_reference")
    {
        return Ok(ci_reference_error(
            operation,
            "SCHEMA",
            format!(
                "reference kind is {:?}, expected dynamicjepa_release_aggregate_reference with schema_version 1",
                reference.get("kind")
            ),
            "regenerate the release reference from a passed multi-seed aggregate",
            source_of_truth,
            before_state,
            None,
        ));
    }
    let expected = match reference.get("expected").and_then(Value::as_object) {
        Some(_) => reference.get("expected").unwrap(),
        None => {
            return Ok(ci_reference_error(
                operation,
                "SCHEMA",
                "reference expected object missing",
                "regenerate the release reference from a passed multi-seed aggregate",
                source_of_truth,
                before_state,
                None,
            ));
        }
    };

    let manifest = match read_ci_json_for_check(&manifest_path) {
        Ok(value) => value,
        Err(message) => {
            return Ok(ci_reference_error(
                operation,
                "AGGREGATE_INCOMPLETE",
                message,
                "rerun dynamicjepa multiseed-aggregate and preserve the complete aggregate bundle",
                source_of_truth,
                before_state,
                None,
            ));
        }
    };
    validate_bundle_schema_manifest(&manifest_path, &manifest)?;
    let fsv = match read_ci_json_for_check(&fsv_path) {
        Ok(value) => value,
        Err(message) => {
            return Ok(ci_reference_error(
                operation,
                "AGGREGATE_INCOMPLETE",
                message,
                "rerun dynamicjepa multiseed-aggregate and preserve the complete aggregate bundle",
                source_of_truth,
                before_state,
                None,
            ));
        }
    };

    let mut diffs = Vec::new();
    for (field, actual, reference_value) in [
        (
            "manifest.status",
            ci_json_pointer_or_null(&manifest, "/status"),
            ci_json_pointer_or_null(expected, "/status"),
        ),
        (
            "manifest.passed_count",
            ci_json_pointer_or_null(&manifest, "/passed_count"),
            ci_json_pointer_or_null(expected, "/passed_count"),
        ),
        (
            "manifest.failed_count",
            ci_json_pointer_or_null(&manifest, "/failed_count"),
            ci_json_pointer_or_null(expected, "/failed_count"),
        ),
        (
            "manifest.selection.expected_bundle_count",
            ci_json_pointer_or_null(&manifest, "/selection/expected_bundle_count"),
            ci_json_pointer_or_null(expected, "/expected_bundle_count"),
        ),
        (
            "manifest.selection.min_passed",
            ci_json_pointer_or_null(&manifest, "/selection/min_passed"),
            ci_json_pointer_or_null(expected, "/min_passed"),
        ),
        (
            "manifest.selection.expected_purpose",
            ci_json_pointer_or_null(&manifest, "/selection/expected_purpose"),
            ci_json_pointer_or_null(expected, "/expected_purpose"),
        ),
        (
            "manifest.selection.expected_career_event_count",
            ci_json_pointer_or_null(&manifest, "/selection/expected_career_event_count"),
            ci_json_pointer_or_null(expected, "/expected_career_event_count"),
        ),
        (
            "manifest.selection.required_career_training_seeds",
            ci_json_pointer_or_null(&manifest, "/selection/required_career_training_seeds"),
            ci_json_pointer_or_null(expected, "/required_career_training_seeds"),
        ),
        (
            "fsv.status",
            ci_json_pointer_or_null(&fsv, "/status"),
            ci_json_pointer_or_null(expected, "/status"),
        ),
        (
            "fsv.readback_checks",
            ci_json_pointer_or_null(&fsv, "/readback_checks"),
            ci_json_pointer_or_null(expected, "/readback_checks"),
        ),
    ] {
        if actual != reference_value {
            diffs.push(json!({
                "type": "field_mismatch",
                "field": field,
                "actual": actual,
                "reference": reference_value,
            }));
        }
    }

    for relative in release_aggregate_required_files() {
        let path = args.aggregate.join(relative);
        let state = ci_path_state(&path)?;
        if !ci_path_state_is_non_empty_file(&state) {
            diffs.push(json!({
                "type": "required_file_missing_or_empty",
                "path": relative,
                "state": state,
            }));
        }
    }

    let mut table_results = Vec::new();
    for spec in RELEASE_AGGREGATE_TABLE_SPECS {
        let Some(reference_table) = reference
            .get("tables")
            .and_then(Value::as_object)
            .and_then(|tables| tables.get(spec.name))
        else {
            return Ok(ci_reference_error(
                operation,
                "SCHEMA",
                format!("reference table {} missing", spec.name),
                "regenerate the release reference from a passed multi-seed aggregate",
                source_of_truth,
                before_state,
                None,
            ));
        };
        match compare_release_aggregate_table(spec, &args.aggregate, reference_table) {
            Ok(result) => {
                diffs.extend(result.diffs);
                table_results.push(json!({
                    "table": spec.name,
                    "actual_row_count": result.actual_row_count,
                    "reference_row_count": result.reference_row_count,
                }));
            }
            Err(CiReferenceCheckFailure { code, message }) => {
                return Ok(ci_reference_error(
                    operation,
                    code,
                    message,
                    "inspect the aggregate paper tables and checked-in release reference before rerunning release-C",
                    source_of_truth,
                    before_state,
                    None,
                ));
            }
        }
    }

    let after_state = json!({
        "aggregate": ci_path_state(&args.aggregate)?,
        "reference": ci_path_state(&args.reference)?,
    });
    let mut output = json!({
        "operation": operation,
        "status": if diffs.is_empty() { "ok" } else { "error" },
        "source_of_truth": source_of_truth,
        "before_state": before_state,
        "after_state": after_state,
        "comparison": {
            "table_results": table_results,
            "diff_count": diffs.len(),
            "diffs": diffs,
        },
        "readback": {
            "manifest_status": manifest.get("status").cloned().unwrap_or(Value::Null),
            "fsv_status": fsv.get("status").cloned().unwrap_or(Value::Null),
            "passed_count": manifest.get("passed_count").cloned().unwrap_or(Value::Null),
            "failed_count": manifest.get("failed_count").cloned().unwrap_or(Value::Null),
            "readback_checks": fsv.get("readback_checks").cloned().unwrap_or(Value::Null),
        }
    });
    if output["status"] == "ok" {
        return Ok(CommandOutcome {
            value: output,
            exit_code: 0,
        });
    }
    if let Some(object) = output.as_object_mut() {
        object.insert(
            "error_code".to_string(),
            json!("RELEASE_REFERENCE_MISMATCH"),
        );
        object.insert(
            "error_message".to_string(),
            json!("release aggregate differs from checked-in aggregate reference"),
        );
        object.insert(
            "remediation".to_string(),
            json!("inspect comparison.diffs; fix the aggregate root cause or intentionally capture a new reference from a preserved passed aggregate"),
        );
    }
    Ok(CommandOutcome {
        value: output,
        exit_code: 1,
    })
}

#[derive(Debug)]
struct CiReferenceCheckFailure {
    code: &'static str,
    message: String,
}

impl CiReferenceCheckFailure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ReleaseAggregateTableSpec {
    name: &'static str,
    path: &'static str,
    key_columns: &'static [&'static str],
    exact_columns: &'static [&'static str],
    numeric_columns: &'static [&'static str],
    volatile_columns: &'static [&'static str],
}

#[derive(Debug)]
struct ReleaseAggregateTableComparison {
    actual_row_count: usize,
    reference_row_count: usize,
    diffs: Vec<Value>,
}

#[derive(Debug, Clone)]
struct ReleaseAggregateNormalizedTable {
    headers: Vec<String>,
    rows: Vec<BTreeMap<String, String>>,
}

const RELEASE_AGGREGATE_EXACT_METRICS: &[&str] = &[
    "train_rows",
    "val_rows",
    "test_rows",
    "parameter_count",
    "panel_slots",
    "input_dim",
    "target_dim",
    "latent_dim",
    "action_dim",
];

const RELEASE_AGGREGATE_TABLE_SPECS: &[ReleaseAggregateTableSpec] = &[
    ReleaseAggregateTableSpec {
        name: "table_prediction_metrics_multiseed",
        path: "paper_tables/table_prediction_metrics_multiseed.csv",
        key_columns: &["domain", "metric"],
        exact_columns: &["domain", "metric", "n", "source_career_training_seeds"],
        numeric_columns: &["mean", "std", "bca_lower", "bca_upper", "min", "max"],
        volatile_columns: &["source_run_ids"],
    },
    ReleaseAggregateTableSpec {
        name: "table_ablation_multiseed",
        path: "paper_tables/table_ablation_multiseed.csv",
        key_columns: &["ablation_id", "metric"],
        exact_columns: &["ablation_id", "metric", "n", "source_career_training_seeds"],
        numeric_columns: &["mean", "std", "bca_lower", "bca_upper", "min", "max"],
        volatile_columns: &["source_run_ids"],
    },
    ReleaseAggregateTableSpec {
        name: "table_paired_ablation_tests",
        path: "paper_tables/table_paired_ablation_tests.csv",
        key_columns: &["metric", "baseline_ablation_id", "comparison_ablation_id"],
        exact_columns: &[
            "metric",
            "baseline_ablation_id",
            "comparison_ablation_id",
            "n",
            "source_career_training_seeds",
        ],
        numeric_columns: &[
            "mean_delta",
            "std_delta",
            "bca_lower",
            "bca_upper",
            "min_delta",
            "max_delta",
            "sign_flip_p",
        ],
        volatile_columns: &["source_run_ids"],
    },
    ReleaseAggregateTableSpec {
        name: "table_seed_failures",
        path: "paper_tables/table_seed_failures.csv",
        key_columns: &["run_id"],
        exact_columns: &[
            "run_id",
            "career_training_seed",
            "status",
            "error_code",
            "error_message",
            "bundle_path",
        ],
        numeric_columns: &[],
        volatile_columns: &[],
    },
];

fn release_aggregate_required_files() -> Vec<&'static str> {
    let mut files = RELEASE_AGGREGATE_TABLE_SPECS
        .iter()
        .map(|spec| spec.path)
        .collect::<Vec<_>>();
    files.extend([
        "aggregate_manifest.json",
        "aggregate_fsv_report.json",
        "plots/multiseed_val_mse_box.svg",
        "plots/multiseed_paired_delta_forest.svg",
        "plots/plot_manifest.json",
    ]);
    files
}

fn ci_json_pointer_or_null(value: &Value, pointer: &str) -> Value {
    value.pointer(pointer).cloned().unwrap_or(Value::Null)
}

fn ci_path_state_is_non_empty_file(state: &Value) -> bool {
    state.get("exists").and_then(Value::as_bool) == Some(true)
        && state.get("kind").and_then(Value::as_str) == Some("file")
        && state
            .get("size_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|size| size > 0)
}

fn compare_release_aggregate_table(
    spec: &ReleaseAggregateTableSpec,
    aggregate_root: &Path,
    reference_table: &Value,
) -> Result<ReleaseAggregateTableComparison, CiReferenceCheckFailure> {
    let actual = normalize_release_aggregate_table(aggregate_root, spec)
        .map_err(|err| CiReferenceCheckFailure::new("AGGREGATE_INCOMPLETE", err.to_string()))?;
    let reference = reference_release_aggregate_table(spec, reference_table)?;
    let mut diffs = Vec::new();

    if actual.headers != reference.headers {
        diffs.push(json!({
            "type": "header_mismatch",
            "table": spec.name,
            "actual": actual.headers,
            "reference": reference.headers,
        }));
    }

    let actual_by_key = release_aggregate_index_rows(spec.name, spec.key_columns, &actual.rows)?;
    let reference_by_key =
        release_aggregate_index_rows(spec.name, spec.key_columns, &reference.rows)?;
    for key in reference_by_key.keys() {
        if !actual_by_key.contains_key(key) {
            diffs.push(json!({
                "type": "missing_row",
                "table": spec.name,
                "key": key,
            }));
        }
    }
    for key in actual_by_key.keys() {
        if !reference_by_key.contains_key(key) {
            diffs.push(json!({
                "type": "extra_row",
                "table": spec.name,
                "key": key,
            }));
        }
    }

    for key in actual_by_key
        .keys()
        .filter(|key| reference_by_key.contains_key(*key))
    {
        let actual_row = actual_by_key.get(key).expect("key was filtered as present");
        let reference_row = reference_by_key
            .get(key)
            .expect("key was filtered as present");
        for column in spec.exact_columns {
            if spec.volatile_columns.contains(column) {
                continue;
            }
            if actual_row.get(*column) != reference_row.get(*column) {
                diffs.push(json!({
                    "type": "exact_column_mismatch",
                    "table": spec.name,
                    "key": key,
                    "column": column,
                    "actual": actual_row.get(*column),
                    "reference": reference_row.get(*column),
                }));
            }
        }
        for column in spec.numeric_columns {
            let Some(actual_cell) = actual_row.get(*column) else {
                diffs.push(json!({
                    "type": "numeric_parse_error",
                    "table": spec.name,
                    "key": key,
                    "column": column,
                    "message": "actual row is missing the numeric column",
                }));
                continue;
            };
            let Some(reference_cell) = reference_row.get(*column) else {
                diffs.push(json!({
                    "type": "numeric_parse_error",
                    "table": spec.name,
                    "key": key,
                    "column": column,
                    "message": "reference row is missing the numeric column",
                }));
                continue;
            };
            let actual_number = match actual_cell.parse::<f64>() {
                Ok(value) => value,
                Err(err) => {
                    diffs.push(json!({
                        "type": "numeric_parse_error",
                        "table": spec.name,
                        "key": key,
                        "column": column,
                        "message": format!("actual value parse failed: {err}"),
                    }));
                    continue;
                }
            };
            let reference_number = match reference_cell.parse::<f64>() {
                Ok(value) => value,
                Err(err) => {
                    diffs.push(json!({
                        "type": "numeric_parse_error",
                        "table": spec.name,
                        "key": key,
                        "column": column,
                        "message": format!("reference value parse failed: {err}"),
                    }));
                    continue;
                }
            };
            let tolerance = release_aggregate_numeric_tolerance(reference_row, column);
            let delta = (actual_number - reference_number).abs();
            if delta > tolerance {
                diffs.push(json!({
                    "type": "numeric_out_of_tolerance",
                    "table": spec.name,
                    "key": key,
                    "column": column,
                    "actual": actual_number,
                    "reference": reference_number,
                    "abs_delta": delta,
                    "tolerance": tolerance,
                }));
            }
        }
    }

    Ok(ReleaseAggregateTableComparison {
        actual_row_count: actual.rows.len(),
        reference_row_count: reference.rows.len(),
        diffs,
    })
}

fn normalize_release_aggregate_table(
    aggregate_root: &Path,
    spec: &ReleaseAggregateTableSpec,
) -> DynamicJepaResult<ReleaseAggregateNormalizedTable> {
    let table = read_ci_csv_table_with_options(
        &aggregate_root.join(spec.path),
        spec.name,
        spec.name == "table_seed_failures",
    )?;
    let mut rows = table
        .rows
        .into_iter()
        .map(|mut row| {
            for column in spec.volatile_columns {
                row.remove(*column);
            }
            row
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        let left_key =
            release_aggregate_table_key(left, spec.key_columns).unwrap_or_else(|_| String::new());
        let right_key =
            release_aggregate_table_key(right, spec.key_columns).unwrap_or_else(|_| String::new());
        left_key.cmp(&right_key)
    });
    Ok(ReleaseAggregateNormalizedTable {
        headers: table.headers,
        rows,
    })
}

fn reference_release_aggregate_table(
    spec: &ReleaseAggregateTableSpec,
    value: &Value,
) -> Result<ReleaseAggregateNormalizedTable, CiReferenceCheckFailure> {
    let key_columns = ci_string_array(value, "key_columns").map_err(|message| {
        CiReferenceCheckFailure::new(
            "SCHEMA",
            format!("{} key_columns invalid: {message}", spec.name),
        )
    })?;
    let expected_key_columns = spec
        .key_columns
        .iter()
        .map(|column| (*column).to_string())
        .collect::<Vec<_>>();
    if key_columns != expected_key_columns {
        return Err(CiReferenceCheckFailure::new(
            "SCHEMA",
            format!(
                "{} key_columns {:?}, expected {:?}",
                spec.name, key_columns, expected_key_columns
            ),
        ));
    }
    let headers = ci_string_array(value, "headers").map_err(|message| {
        CiReferenceCheckFailure::new(
            "SCHEMA",
            format!("{} headers invalid: {message}", spec.name),
        )
    })?;
    let rows = value.get("rows").and_then(Value::as_array).ok_or_else(|| {
        CiReferenceCheckFailure::new("SCHEMA", format!("{} rows missing", spec.name))
    })?;
    let mut normalized_rows = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        let object = row.as_object().ok_or_else(|| {
            CiReferenceCheckFailure::new(
                "SCHEMA",
                format!("{} rows[{idx}] is not an object", spec.name),
            )
        })?;
        let mut mapped = BTreeMap::new();
        for (key, value) in object {
            if spec.volatile_columns.contains(&key.as_str()) {
                continue;
            }
            mapped.insert(
                key.clone(),
                ci_json_cell_to_string(value).ok_or_else(|| {
                    CiReferenceCheckFailure::new(
                        "SCHEMA",
                        format!("{} rows[{idx}].{key} has unsupported JSON type", spec.name),
                    )
                })?,
            );
        }
        normalized_rows.push(mapped);
    }
    normalized_rows.sort_by(|left, right| {
        let left_key =
            release_aggregate_table_key(left, spec.key_columns).unwrap_or_else(|_| String::new());
        let right_key =
            release_aggregate_table_key(right, spec.key_columns).unwrap_or_else(|_| String::new());
        left_key.cmp(&right_key)
    });
    Ok(ReleaseAggregateNormalizedTable {
        headers,
        rows: normalized_rows,
    })
}

fn release_aggregate_index_rows<'a>(
    table: &str,
    key_columns: &[&str],
    rows: &'a [BTreeMap<String, String>],
) -> Result<BTreeMap<String, &'a BTreeMap<String, String>>, CiReferenceCheckFailure> {
    let mut out = BTreeMap::new();
    for row in rows {
        let key = release_aggregate_table_key(row, key_columns).map_err(|message| {
            CiReferenceCheckFailure::new("SCHEMA", format!("{table} {message}"))
        })?;
        if out.insert(key.clone(), row).is_some() {
            return Err(CiReferenceCheckFailure::new(
                "SCHEMA",
                format!("{table} has duplicate key {key}"),
            ));
        }
    }
    Ok(out)
}

fn release_aggregate_table_key(
    row: &BTreeMap<String, String>,
    columns: &[&str],
) -> Result<String, String> {
    let mut parts = Vec::with_capacity(columns.len());
    for column in columns {
        parts.push(
            row.get(*column)
                .ok_or_else(|| format!("row missing key column {column}"))?
                .as_str(),
        );
    }
    Ok(parts.join("\u{1f}"))
}

fn release_aggregate_numeric_tolerance(row: &BTreeMap<String, String>, column: &str) -> f64 {
    if row
        .get("metric")
        .is_some_and(|metric| RELEASE_AGGREGATE_EXACT_METRICS.contains(&metric.as_str()))
    {
        1.0e-12
    } else if column == "sign_flip_p" {
        0.15
    } else {
        0.10
    }
}

#[derive(Debug, Clone)]
struct CiCsvTable {
    headers: Vec<String>,
    rows: Vec<BTreeMap<String, String>>,
}

type CiTableRows = Vec<BTreeMap<String, String>>;
type CiTableWithKeys = (Vec<String>, CiTableRows);

fn capture_ci_reference_value(reference_id: &str, bundle: &Path) -> DynamicJepaResult<Value> {
    let bundle_path = canonical_existing_path(bundle, "ci-reference.bundle")?;
    let run_manifest = read_json_file(&bundle.join("run_manifest.json"))?;
    validate_bundle_schema_manifest(&bundle.join("run_manifest.json"), &run_manifest)?;
    let fsv_report = read_json_file(&bundle.join("fsv_report.json"))?;
    require_json_status(&run_manifest, "passed", "run_manifest.json")?;
    require_json_status(&fsv_report, "passed", "fsv_report.json")?;

    let mut files = Vec::new();
    for rel_path in CI_REFERENCE_REQUIRED_FILES {
        let path = bundle.join(rel_path);
        let metadata = fs::metadata(&path).map_err(|err| DynamicJepaError::Storage {
            operation: "ci_reference.required_file".to_string(),
            cf: "filesystem".to_string(),
            message: format!("missing required bundle file {}: {err}", path.display()),
            remediation:
                "rerun research-smoke; CI references are only captured from complete passed bundles"
                    .to_string(),
        })?;
        if !metadata.is_file() {
            return Err(DynamicJepaError::validation(
                "ci_reference.required_file",
                format!("required bundle path is not a file: {}", path.display()),
                "remove the invalid path and rerun research-smoke to produce the expected file",
            ));
        }
        files.push(json!({
            "path": rel_path,
            "sha256": hex_bytes(&file_sha256(&path)?),
            "size_bytes": metadata.len(),
        }));
    }

    let prediction_table = read_ci_csv_table(
        &bundle.join("paper_tables/table_prediction_metrics.csv"),
        "table_prediction_metrics",
    )?;
    let ablation_table = read_ci_csv_table(
        &bundle.join("paper_tables/table_ablation.csv"),
        "table_ablation",
    )?;
    let edge_table = read_ci_csv_table(
        &bundle.join("paper_tables/table_edge_cases.csv"),
        "table_edge_cases",
    )?;
    let artifact_hashes = capture_ci_artifact_hashes(bundle)?;
    let bundle_hash_input = files
        .iter()
        .filter_map(|file| {
            Some(format!(
                "{} {}\n",
                file.get("sha256")?.as_str()?,
                file.get("path")?.as_str()?
            ))
        })
        .collect::<String>();

    Ok(json!({
        "schema_version": CI_REFERENCE_SCHEMA_VERSION,
        "reference_id": reference_id,
        "created_at_unix_ms": now_unix_ms()?,
        "source_bundle": bundle_path,
        "run": {
            "run_id": run_manifest.get("run_id").cloned().unwrap_or(Value::Null),
            "purpose": run_manifest.get("purpose").cloned().unwrap_or(Value::Null),
            "status": run_manifest.get("status").cloned().unwrap_or(Value::Null),
            "career_event_count": run_manifest.get("career_event_count").cloned().unwrap_or(Value::Null),
        },
        "fsv": {
            "status": fsv_report.get("status").cloned().unwrap_or(Value::Null),
            "verification_run_ids": fsv_report.get("verification_run_ids").cloned().unwrap_or(Value::Null),
        },
        "bundle_hash_sha256": hex_bytes(&sha256(bundle_hash_input.as_bytes())),
        "files": files,
        "artifact_hashes": artifact_hashes,
        "tables": {
            "table_prediction_metrics": ci_table_json(
                "paper_tables/table_prediction_metrics.csv",
                &["domain"],
                prediction_table,
            ),
            "table_ablation": ci_table_json(
                "paper_tables/table_ablation.csv",
                &["ablation_id"],
                ablation_table,
            ),
            "table_edge_cases": ci_table_json(
                "paper_tables/table_edge_cases.csv",
                &["domain", "edge_index", "name"],
                edge_table,
            ),
        },
    }))
}

fn require_json_status(value: &Value, expected: &str, label: &str) -> DynamicJepaResult<()> {
    let actual = value.get("status").and_then(Value::as_str).ok_or_else(|| {
        DynamicJepaError::validation(
            "ci_reference.status",
            format!("{label} is missing string status"),
            "capture references only from complete research-smoke bundles",
        )
    })?;
    if actual != expected {
        return Err(DynamicJepaError::validation(
            "ci_reference.status",
            format!("{label} status is {actual:?}, expected {expected:?}"),
            "fix the failed bundle and rerun research-smoke before capturing a reference",
        ));
    }
    Ok(())
}

fn ci_table_json(path: &str, key_columns: &[&str], table: CiCsvTable) -> Value {
    json!({
        "path": path,
        "key_columns": key_columns,
        "headers": table.headers,
        "rows": table.rows,
    })
}

fn read_ci_csv_table(path: &Path, table_name: &str) -> DynamicJepaResult<CiCsvTable> {
    read_ci_csv_table_with_options(path, table_name, false)
}

fn read_ci_csv_table_allow_empty(path: &Path, table_name: &str) -> DynamicJepaResult<CiCsvTable> {
    read_ci_csv_table_with_options(path, table_name, true)
}

fn read_ci_csv_table_with_options(
    path: &Path,
    table_name: &str,
    allow_empty_rows: bool,
) -> DynamicJepaResult<CiCsvTable> {
    let text = fs::read_to_string(path).map_err(|err| DynamicJepaError::Storage {
        operation: "ci_reference.read_csv".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to read {table_name} at {}: {err}", path.display()),
        remediation: "rerun research-smoke; required paper table CSVs must exist".to_string(),
    })?;
    let mut lines = text.lines();
    let header_line = lines.next().ok_or_else(|| {
        DynamicJepaError::validation(
            "ci_reference.csv",
            format!("{table_name} is empty: {}", path.display()),
            "research-smoke must write a header and at least one data row",
        )
    })?;
    let headers = split_ci_csv_line(header_line).map_err(|message| {
        DynamicJepaError::validation(
            "ci_reference.csv",
            format!("{table_name} header parse failed: {message}"),
            "paper table CSVs must be valid RFC-4180-style comma-separated text",
        )
    })?;
    if headers.is_empty() {
        return Err(DynamicJepaError::validation(
            "ci_reference.csv",
            format!("{table_name} has zero headers"),
            "paper table CSVs must include stable column headers",
        ));
    }
    let mut rows = Vec::new();
    for (line_idx, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cells = split_ci_csv_line(line).map_err(|message| {
            DynamicJepaError::validation(
                "ci_reference.csv",
                format!("{table_name} row {} parse failed: {message}", line_idx + 2),
                "paper table CSVs must not contain malformed quoted fields",
            )
        })?;
        if cells.len() != headers.len() {
            return Err(DynamicJepaError::validation(
                "ci_reference.csv",
                format!(
                    "{table_name} row {} has {} cells, expected {}",
                    line_idx + 2,
                    cells.len(),
                    headers.len()
                ),
                "fix the table writer; CI drift checks require rectangular CSV tables",
            ));
        }
        rows.push(
            headers
                .iter()
                .cloned()
                .zip(cells)
                .collect::<BTreeMap<_, _>>(),
        );
    }
    if rows.is_empty() && !allow_empty_rows {
        return Err(DynamicJepaError::validation(
            "ci_reference.csv",
            format!("{table_name} has no data rows"),
            "research-smoke must produce non-empty paper tables for drift detection",
        ));
    }
    Ok(CiCsvTable { headers, rows })
}

fn split_ci_csv_line(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(current);
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if in_quotes {
        return Err("unterminated quoted field".to_string());
    }
    fields.push(current);
    Ok(fields)
}

fn capture_ci_artifact_hashes(bundle: &Path) -> DynamicJepaResult<Vec<Value>> {
    let artifact_file = bundle.join("artifact_hashes.json");
    let root = read_json_file(&artifact_file)?;
    let groups = root.as_array().ok_or_else(|| {
        DynamicJepaError::validation(
            "ci_reference.artifact_hashes",
            "artifact_hashes.json root is not an array",
            "rerun research-smoke; artifact hash evidence must be an array",
        )
    })?;
    let mut rows = Vec::new();
    for (group_idx, group) in groups.iter().enumerate() {
        let domain = group.get("domain").and_then(Value::as_str).ok_or_else(|| {
            DynamicJepaError::validation(
                "ci_reference.artifact_hashes.domain",
                format!("artifact hash group {group_idx} missing domain"),
                "fix artifact_hashes.json generation before capturing a reference",
            )
        })?;
        let hashes = group
            .get("artifact_hashes")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                DynamicJepaError::validation(
                    "ci_reference.artifact_hashes.items",
                    format!("artifact hash group {domain} missing artifact_hashes array"),
                    "fix artifact_hashes.json generation before capturing a reference",
                )
            })?;
        for (item_idx, item) in hashes.iter().enumerate() {
            let relative_path = item
                .get("relative_path")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    DynamicJepaError::validation(
                        "ci_reference.artifact_hashes.relative_path",
                        format!("artifact hash {domain}[{item_idx}] missing relative_path"),
                        "artifact hash rows must name the artifact-relative file",
                    )
                })?;
            let equal = item.get("equal").and_then(Value::as_bool).unwrap_or(false);
            if !equal {
                return Err(DynamicJepaError::validation(
                    "ci_reference.artifact_hashes.equal",
                    format!("artifact hash {domain}/{relative_path} was not verified equal"),
                    "rerun FSV; references cannot be captured from bundles with corrupt artifacts",
                ));
            }
            rows.push(json!({
                "domain": domain,
                "relative_path": relative_path,
                "registry_sha256": ci_sha256_value_to_hex(
                    item.get("registry_sha256"),
                    &format!("{domain}/{relative_path}.registry_sha256"),
                )?,
                "recomputed_sha256": ci_sha256_value_to_hex(
                    item.get("recomputed_sha256"),
                    &format!("{domain}/{relative_path}.recomputed_sha256"),
                )?,
                "size_bytes": item.get("size_bytes").and_then(Value::as_u64).ok_or_else(|| {
                    DynamicJepaError::validation(
                        "ci_reference.artifact_hashes.size_bytes",
                        format!("artifact hash {domain}/{relative_path} missing size_bytes"),
                        "artifact hash rows must include physical artifact size evidence",
                    )
                })?,
            }));
        }
    }
    if rows.is_empty() {
        return Err(DynamicJepaError::validation(
            "ci_reference.artifact_hashes",
            "artifact_hashes.json contained no artifact hash rows",
            "research-smoke must verify real artifacts before a reference is captured",
        ));
    }
    Ok(rows)
}

fn ci_sha256_value_to_hex(value: Option<&Value>, field: &str) -> DynamicJepaResult<String> {
    let array = value.and_then(Value::as_array).ok_or_else(|| {
        DynamicJepaError::validation(
            "ci_reference.sha256",
            format!("{field} is not a byte array"),
            "artifact hash evidence must contain 32-byte SHA-256 arrays",
        )
    })?;
    if array.len() != 32 {
        return Err(DynamicJepaError::validation(
            "ci_reference.sha256",
            format!("{field} length is {}, expected 32", array.len()),
            "artifact hash evidence must contain 32-byte SHA-256 arrays",
        ));
    }
    let mut bytes = Vec::with_capacity(32);
    for (idx, value) in array.iter().enumerate() {
        let byte = value.as_u64().ok_or_else(|| {
            DynamicJepaError::validation(
                "ci_reference.sha256",
                format!("{field}[{idx}] is not an unsigned byte"),
                "artifact hash evidence must contain integer byte values",
            )
        })?;
        if byte > u8::MAX as u64 {
            return Err(DynamicJepaError::validation(
                "ci_reference.sha256",
                format!("{field}[{idx}]={byte} exceeds 255"),
                "artifact hash evidence must contain byte values in 0..=255",
            ));
        }
        bytes.push(byte as u8);
    }
    Ok(hex_bytes(&bytes))
}

fn read_ci_json_for_check(path: &Path) -> Result<Value, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| format!("failed to parse {} as JSON: {err}", path.display()))
}

fn validate_ci_reference_schema(reference: &Value) -> Result<(), String> {
    if reference.get("schema_version").and_then(Value::as_u64) != Some(CI_REFERENCE_SCHEMA_VERSION)
    {
        return Err(format!(
            "schema_version must be {CI_REFERENCE_SCHEMA_VERSION}"
        ));
    }
    require_ci_array(reference, "files")?;
    require_ci_array(reference, "artifact_hashes")?;
    let tables = reference
        .get("tables")
        .and_then(Value::as_object)
        .ok_or_else(|| "tables object missing".to_string())?;
    for table in [
        "table_prediction_metrics",
        "table_ablation",
        "table_edge_cases",
    ] {
        let table_value = tables
            .get(table)
            .ok_or_else(|| format!("{table} missing from reference tables"))?;
        require_ci_array(table_value, "headers")?;
        require_ci_array(table_value, "key_columns")?;
        require_ci_array(table_value, "rows")?;
        if table_value
            .get("path")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Err(format!("{table}.path missing or empty"));
        }
    }
    Ok(())
}

fn require_ci_array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    let array = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} array missing"))?;
    if array.is_empty() {
        return Err(format!("{key} array is empty"));
    }
    Ok(array)
}

fn compare_ci_reference(
    reference: &Value,
    current: &Value,
    tolerances: &Value,
) -> Result<Value, CiReferenceCheckFailure> {
    let reference_files = ci_file_map(reference)?;
    let current_files = ci_file_map(current)?;
    let mut file_hash_mismatches = Vec::new();
    for (path, (reference_sha, reference_size)) in &reference_files {
        let Some((current_sha, current_size)) = current_files.get(path) else {
            return Err(CiReferenceCheckFailure::new(
                "BUNDLE_INCOMPLETE",
                format!("current bundle is missing required file {path}"),
            ));
        };
        if reference_sha != current_sha || reference_size != current_size {
            file_hash_mismatches.push(json!({
                "path": path,
                "reference_sha256": reference_sha,
                "current_sha256": current_sha,
                "reference_size_bytes": reference_size,
                "current_size_bytes": current_size,
            }));
        }
    }

    let mut numeric_diffs = Vec::new();
    let mut out_of_tolerance = Vec::new();
    let mut structural_diffs = Vec::new();
    for table in [
        "table_prediction_metrics",
        "table_ablation",
        "table_edge_cases",
    ] {
        compare_ci_table(
            reference,
            current,
            tolerances,
            table,
            &mut numeric_diffs,
            &mut out_of_tolerance,
            &mut structural_diffs,
        )?;
    }

    if !structural_diffs.is_empty() {
        return Err(CiReferenceCheckFailure::new(
            "STRUCTURAL_DRIFT",
            serde_json::to_string_pretty(&structural_diffs)
                .unwrap_or_else(|_| "structural drift detected".to_string()),
        ));
    }
    if !out_of_tolerance.is_empty() {
        return Err(CiReferenceCheckFailure::new(
            "OUT_OF_TOLERANCE",
            serde_json::to_string_pretty(&out_of_tolerance)
                .unwrap_or_else(|_| "metric drift exceeded tolerance".to_string()),
        ));
    }

    Ok(json!({
        "file_hash_mismatches": file_hash_mismatches,
        "numeric_diffs": numeric_diffs,
        "out_of_tolerance": out_of_tolerance,
        "structural_diffs": structural_diffs,
    }))
}

fn ci_file_map(value: &Value) -> Result<BTreeMap<String, (String, u64)>, CiReferenceCheckFailure> {
    let files = value
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CiReferenceCheckFailure::new("REFERENCE_CORRUPTED", "files array missing")
        })?;
    let mut out = BTreeMap::new();
    for file in files {
        let path = file.get("path").and_then(Value::as_str).ok_or_else(|| {
            CiReferenceCheckFailure::new("REFERENCE_CORRUPTED", "file path missing")
        })?;
        let sha = file.get("sha256").and_then(Value::as_str).ok_or_else(|| {
            CiReferenceCheckFailure::new("REFERENCE_CORRUPTED", format!("{path} sha256 missing"))
        })?;
        let size = file
            .get("size_bytes")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                CiReferenceCheckFailure::new(
                    "REFERENCE_CORRUPTED",
                    format!("{path} size_bytes missing"),
                )
            })?;
        out.insert(path.to_string(), (sha.to_string(), size));
    }
    Ok(out)
}

fn compare_ci_table(
    reference: &Value,
    current: &Value,
    tolerances: &Value,
    table: &str,
    numeric_diffs: &mut Vec<Value>,
    out_of_tolerance: &mut Vec<Value>,
    structural_diffs: &mut Vec<Value>,
) -> Result<(), CiReferenceCheckFailure> {
    let (reference_key_columns, reference_rows) = ci_table_rows(reference, table)?;
    let (current_key_columns, current_rows) = ci_table_rows(current, table)?;
    if reference_key_columns != current_key_columns {
        structural_diffs.push(json!({
            "table": table,
            "kind": "key_columns_changed",
            "reference": reference_key_columns,
            "current": current_key_columns,
        }));
        return Ok(());
    }
    let reference_index = ci_index_rows(table, &reference_key_columns, &reference_rows)?;
    let current_index = ci_index_rows(table, &reference_key_columns, &current_rows)?;
    for key in current_index.keys() {
        if !reference_index.contains_key(key) {
            structural_diffs.push(json!({
                "table": table,
                "kind": "unexpected_row",
                "key": key,
            }));
        }
    }
    for (key, reference_row) in &reference_index {
        let Some(current_row) = current_index.get(key) else {
            return Err(CiReferenceCheckFailure::new(
                "BUNDLE_INCOMPLETE",
                format!("{table} missing expected row {key}"),
            ));
        };
        for (column, reference_value) in *reference_row {
            if ci_ignored_column(table, column) {
                continue;
            }
            let Some(current_value) = current_row.get(column) else {
                return Err(CiReferenceCheckFailure::new(
                    "BUNDLE_INCOMPLETE",
                    format!("{table} row {key} missing column {column}"),
                ));
            };
            if let Some(tolerance) = ci_tolerance(tolerances, table, column) {
                let reference_number = reference_value.parse::<f64>().map_err(|err| {
                    CiReferenceCheckFailure::new(
                        "REFERENCE_CORRUPTED",
                        format!("{table} {key} {column} reference value is not numeric: {err}"),
                    )
                })?;
                let current_number = current_value.parse::<f64>().map_err(|err| {
                    CiReferenceCheckFailure::new(
                        "BUNDLE_INCOMPLETE",
                        format!("{table} {key} {column} current value is not numeric: {err}"),
                    )
                })?;
                let delta = (current_number - reference_number).abs();
                let diff = json!({
                    "table": table,
                    "key": key,
                    "column": column,
                    "reference": reference_number,
                    "current": current_number,
                    "abs_delta": delta,
                    "tolerance": tolerance,
                });
                if delta > tolerance {
                    out_of_tolerance.push(diff.clone());
                }
                if delta > 0.0 {
                    numeric_diffs.push(diff);
                }
            } else if ci_numeric_metric_column_requires_tolerance(table, column) {
                return Err(CiReferenceCheckFailure::new(
                    "REFERENCE_TOLERANCE_MISSING",
                    format!("{table}.{column} is a numeric metric column but reference/tolerances.json has no tolerance"),
                ));
            } else if reference_value != current_value {
                structural_diffs.push(json!({
                    "table": table,
                    "kind": "cell_changed_without_tolerance",
                    "key": key,
                    "column": column,
                    "reference": reference_value,
                    "current": current_value,
                }));
            }
        }
    }
    Ok(())
}

fn ci_table_rows(value: &Value, table: &str) -> Result<CiTableWithKeys, CiReferenceCheckFailure> {
    let table_value = value
        .get("tables")
        .and_then(Value::as_object)
        .and_then(|tables| tables.get(table))
        .ok_or_else(|| {
            CiReferenceCheckFailure::new(
                "REFERENCE_CORRUPTED",
                format!("{table} missing from reference JSON"),
            )
        })?;
    let key_columns = ci_string_array(table_value, "key_columns").map_err(|message| {
        CiReferenceCheckFailure::new("REFERENCE_CORRUPTED", format!("{table}: {message}"))
    })?;
    let rows = table_value
        .get("rows")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CiReferenceCheckFailure::new("REFERENCE_CORRUPTED", format!("{table}.rows missing"))
        })?;
    let mut out = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        let object = row.as_object().ok_or_else(|| {
            CiReferenceCheckFailure::new(
                "REFERENCE_CORRUPTED",
                format!("{table}.rows[{idx}] is not an object"),
            )
        })?;
        let mut mapped = BTreeMap::new();
        for (key, value) in object {
            mapped.insert(
                key.clone(),
                ci_json_cell_to_string(value).ok_or_else(|| {
                    CiReferenceCheckFailure::new(
                        "REFERENCE_CORRUPTED",
                        format!("{table}.rows[{idx}].{key} has unsupported JSON type"),
                    )
                })?,
            );
        }
        out.push(mapped);
    }
    Ok((key_columns, out))
}

fn ci_string_array(value: &Value, key: &str) -> Result<Vec<String>, String> {
    let array = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} missing or not an array"))?;
    array
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{key} contains a non-string item"))
        })
        .collect()
}

fn ci_json_cell_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn ci_index_rows<'a>(
    table: &str,
    key_columns: &[String],
    rows: &'a [BTreeMap<String, String>],
) -> Result<BTreeMap<String, &'a BTreeMap<String, String>>, CiReferenceCheckFailure> {
    let mut out = BTreeMap::new();
    for row in rows {
        let mut parts = Vec::new();
        for column in key_columns {
            parts.push(row.get(column).ok_or_else(|| {
                CiReferenceCheckFailure::new(
                    "REFERENCE_CORRUPTED",
                    format!("{table} row missing key column {column}"),
                )
            })?);
        }
        let key = parts
            .into_iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("|");
        if out.insert(key.clone(), row).is_some() {
            return Err(CiReferenceCheckFailure::new(
                "REFERENCE_CORRUPTED",
                format!("{table} has duplicate key {key}"),
            ));
        }
    }
    Ok(out)
}

fn ci_tolerance(tolerances: &Value, table: &str, column: &str) -> Option<f64> {
    tolerances
        .get(table)
        .and_then(Value::as_object)
        .and_then(|columns| columns.get(column))
        .and_then(Value::as_f64)
}

fn ci_ignored_column(table: &str, column: &str) -> bool {
    column == "verification_run_id" || (table == "table_edge_cases" && column == "evidence_file")
}

fn ci_numeric_metric_column_requires_tolerance(table: &str, column: &str) -> bool {
    matches!(table, "table_prediction_metrics" | "table_ablation")
        && matches!(
            column,
            "val_latent_cosine"
                | "val_latent_mse"
                | "test_latent_cosine"
                | "test_latent_mse"
                | "random_init_cosine"
                | "shuffled_target_cosine"
                | "val_action_contrast_acc"
        )
}

fn compare_ci_artifact_hashes(
    reference: &Value,
    current: &Value,
) -> Result<Value, CiReferenceCheckFailure> {
    let reference_rows = ci_artifact_hash_map(reference)?;
    let current_rows = ci_artifact_hash_map(current)?;
    let mut mismatches = Vec::new();
    for (key, reference_row) in &reference_rows {
        let Some(current_row) = current_rows.get(key) else {
            return Err(CiReferenceCheckFailure::new(
                "BUNDLE_INCOMPLETE",
                format!("artifact hash row {key} missing from current bundle"),
            ));
        };
        if reference_row != current_row {
            mismatches.push(json!({
                "key": key,
                "reference": reference_row,
                "current": current_row,
            }));
        }
    }
    for key in current_rows.keys() {
        if !reference_rows.contains_key(key) {
            mismatches.push(json!({
                "key": key,
                "reference": null,
                "current": current_rows.get(key),
            }));
        }
    }
    Ok(json!({"mismatches": mismatches}))
}

fn ci_artifact_hash_map(
    value: &Value,
) -> Result<BTreeMap<String, BTreeMap<String, String>>, CiReferenceCheckFailure> {
    let rows = value
        .get("artifact_hashes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CiReferenceCheckFailure::new("REFERENCE_CORRUPTED", "artifact_hashes array missing")
        })?;
    let mut out = BTreeMap::new();
    for (idx, row) in rows.iter().enumerate() {
        let object = row.as_object().ok_or_else(|| {
            CiReferenceCheckFailure::new(
                "REFERENCE_CORRUPTED",
                format!("artifact_hashes[{idx}] is not an object"),
            )
        })?;
        let domain = object
            .get("domain")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CiReferenceCheckFailure::new(
                    "REFERENCE_CORRUPTED",
                    format!("artifact_hashes[{idx}].domain missing"),
                )
            })?;
        let relative_path = object
            .get("relative_path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CiReferenceCheckFailure::new(
                    "REFERENCE_CORRUPTED",
                    format!("artifact_hashes[{idx}].relative_path missing"),
                )
            })?;
        let key = format!("{domain}|{relative_path}");
        let mut mapped = BTreeMap::new();
        for field in ["registry_sha256", "recomputed_sha256", "size_bytes"] {
            mapped.insert(
                field.to_string(),
                ci_json_cell_to_string(object.get(field).ok_or_else(|| {
                    CiReferenceCheckFailure::new(
                        "REFERENCE_CORRUPTED",
                        format!("artifact_hashes[{idx}].{field} missing"),
                    )
                })?)
                .ok_or_else(|| {
                    CiReferenceCheckFailure::new(
                        "REFERENCE_CORRUPTED",
                        format!("artifact_hashes[{idx}].{field} unsupported type"),
                    )
                })?,
            );
        }
        if out.insert(key.clone(), mapped).is_some() {
            return Err(CiReferenceCheckFailure::new(
                "REFERENCE_CORRUPTED",
                format!("duplicate artifact hash key {key}"),
            ));
        }
    }
    Ok(out)
}

fn ci_reference_error(
    operation: &'static str,
    code: &'static str,
    message: impl Into<String>,
    remediation: &'static str,
    source_of_truth: Value,
    before_state: Value,
    evidence: Option<Value>,
) -> CommandOutcome {
    let mut object = Map::new();
    object.insert("operation".to_string(), json!(operation));
    object.insert("status".to_string(), json!("error"));
    object.insert("error_code".to_string(), json!(code));
    object.insert("error_message".to_string(), json!(message.into()));
    object.insert("remediation".to_string(), json!(remediation));
    object.insert("source_of_truth".to_string(), source_of_truth);
    object.insert("before_state".to_string(), before_state);
    if let Some(evidence) = evidence {
        object.insert("evidence".to_string(), evidence);
    }
    CommandOutcome {
        value: Value::Object(object),
        exit_code: 1,
    }
}

fn ci_path_state(path: &Path) -> DynamicJepaResult<Value> {
    match fs::metadata(path) {
        Ok(metadata) => {
            let kind = if metadata.is_file() {
                "file"
            } else if metadata.is_dir() {
                "directory"
            } else {
                "other"
            };
            let sha256 = if metadata.is_file() {
                Some(hex_bytes(&file_sha256(path)?))
            } else {
                None
            };
            Ok(json!({
                "path": path.display().to_string(),
                "exists": true,
                "kind": kind,
                "size_bytes": metadata.len(),
                "sha256": sha256,
            }))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(json!({
            "path": path.display().to_string(),
            "exists": false,
        })),
        Err(err) => Err(DynamicJepaError::Storage {
            operation: "ci_reference.path_state".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to inspect {}: {err}", path.display()),
            remediation: "ensure the CI runner can stat the reference and bundle paths".to_string(),
        }),
    }
}

struct ResearchSmokeContext {
    run_id: String,
    purpose: String,
    run_root: PathBuf,
    run_dir: PathBuf,
    fixtures_root: PathBuf,
    career_fixtures_root: PathBuf,
    db_root: PathBuf,
    artifact_root: PathBuf,
    onet_text_zip: Option<PathBuf>,
    career_event_count: u64,
    career_training_seed: Option<u64>,
    career_ablation_training_seed: Option<u64>,
    career_ablation_training_seed_effective: Option<u64>,
    career_ablation_training_seed_origin: String,
    hp_overrides: ResearchHyperparameterOverrides,
    hyperparameter_origin: Option<String>,
    system_specs: PathBuf,
    command_log_path: PathBuf,
    command_output_dir: PathBuf,
    decoded_records_dir: PathBuf,
    fsv_evidence_root: PathBuf,
    paper_tables_dir: PathBuf,
    plots_dir: PathBuf,
    bin: PathBuf,
    created_at_unix_ms: i64,
    command_index: usize,
    command_rows: Vec<Value>,
    domain_reports: BTreeMap<String, Value>,
    training_rows: Vec<Value>,
    artifact_rows: Vec<Value>,
    edge_rows: Vec<Value>,
    ablation_rows: Vec<Value>,
}

#[derive(Debug)]
struct ResearchCommandCapture {
    index: usize,
    name: String,
    exit_code: i32,
    stdout: String,
    row: Value,
}

impl ResearchSmokeContext {
    fn new(
        run_id: String,
        args: ResearchSmokeArgs,
        hp_overrides: ResearchHyperparameterOverrides,
    ) -> DynamicJepaResult<Self> {
        let run_root = args.run_root;
        fs::create_dir_all(&run_root).map_err(|err| DynamicJepaError::Storage {
            operation: "research_smoke.create_run_root".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to create {}: {err}", run_root.display()),
            remediation: "choose a writable --run-root".to_string(),
        })?;
        let run_dir = run_root.join(&run_id);
        if run_dir.exists() {
            return Err(DynamicJepaError::validation(
                "research_smoke.run_id",
                format!("research run directory already exists: {}", run_dir.display()),
                "use a fresh --run-id; research smoke bundles are immutable and are never overwritten",
            ));
        }
        fs::create_dir(&run_dir).map_err(|err| DynamicJepaError::Storage {
            operation: "research_smoke.create_run_dir".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to create {}: {err}", run_dir.display()),
            remediation: "use a fresh --run-id under a writable --run-root".to_string(),
        })?;

        let db_root = args.db_root.unwrap_or_else(|| run_dir.join("dbs"));
        let artifact_root = args
            .artifact_root
            .unwrap_or_else(|| run_dir.join("artifacts"));
        ensure_absent_or_empty_dir(&db_root, "research_smoke.db_root")?;
        ensure_absent_or_empty_dir(&artifact_root, "research_smoke.artifact_root")?;

        let command_output_dir = run_dir.join("command_outputs");
        let decoded_records_dir = run_dir.join("decoded_records");
        let fsv_evidence_root = run_dir.join("fsv_evidence");
        let paper_tables_dir = run_dir.join("paper_tables");
        let plots_dir = run_dir.join("plots");
        for path in [
            &db_root,
            &artifact_root,
            &command_output_dir,
            &decoded_records_dir,
            &fsv_evidence_root,
            &paper_tables_dir,
            &plots_dir,
            &run_dir.join("ablations"),
        ] {
            fs::create_dir_all(path).map_err(|err| DynamicJepaError::Storage {
                operation: "research_smoke.create_bundle_subdir".to_string(),
                cf: "filesystem".to_string(),
                message: format!("failed to create {}: {err}", path.display()),
                remediation: "choose writable run, DB, and artifact roots".to_string(),
            })?;
        }

        let bin = env::current_exe().map_err(|err| {
            DynamicJepaError::validation(
                "research_smoke.current_exe",
                format!("failed to locate current context-graph binary: {err}"),
                "run research-smoke from a built context-graph CLI binary",
            )
        })?;

        let career_event_count = research_career_event_count(
            &args.purpose,
            args.career_event_count,
            args.onet_text_zip.is_some(),
        )?;
        let (career_ablation_training_seed_effective, career_ablation_training_seed_origin) =
            resolve_career_ablation_seed_control(
                &args.fixtures_root,
                args.career_training_seed,
                args.career_ablation_training_seed,
            )?;

        let mut ctx = Self {
            run_id,
            purpose: args.purpose,
            run_root,
            run_dir,
            career_fixtures_root: args.fixtures_root.clone(),
            fixtures_root: args.fixtures_root,
            db_root,
            artifact_root,
            onet_text_zip: args.onet_text_zip,
            career_event_count,
            career_training_seed: args.career_training_seed,
            career_ablation_training_seed: args.career_ablation_training_seed,
            career_ablation_training_seed_effective,
            career_ablation_training_seed_origin,
            hp_overrides,
            hyperparameter_origin: args.hyperparameter_origin,
            system_specs: args.system_specs,
            command_log_path: PathBuf::new(),
            command_output_dir,
            decoded_records_dir,
            fsv_evidence_root,
            paper_tables_dir,
            plots_dir,
            bin,
            created_at_unix_ms: now_unix_ms()?,
            command_index: 0,
            command_rows: Vec::new(),
            domain_reports: BTreeMap::new(),
            training_rows: Vec::new(),
            artifact_rows: Vec::new(),
            edge_rows: Vec::new(),
            ablation_rows: Vec::new(),
        };
        ctx.command_log_path = ctx.run_dir.join("command_log.jsonl");
        ctx.write_run_manifest("running", Value::Null)?;
        Ok(ctx)
    }

    fn db_path(&self, label: &str) -> DynamicJepaResult<PathBuf> {
        let path = self
            .db_root
            .join(format!("{}_{}_rocksdb", self.run_id, label));
        if path.exists() {
            return Err(DynamicJepaError::validation(
                "research_smoke.db_path",
                format!("DB path already exists: {}", path.display()),
                "use a fresh --run-id or empty --db-root; research smoke never reuses DB source-of-truth paths",
            ));
        }
        Ok(path)
    }

    fn fixtures_root_for_domain(&self, domain: &str) -> PathBuf {
        if domain.starts_with("career_taxonomy") {
            self.career_fixtures_root.clone()
        } else {
            self.fixtures_root.clone()
        }
    }

    fn write_run_manifest(&self, status: &str, failure_details: Value) -> DynamicJepaResult<()> {
        let operator = env::var("USER").unwrap_or_else(|_| "local".to_string());
        let repo = env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| ".".to_string());
        write_json_file(
            &self.run_dir.join("run_manifest.json"),
            &json!({
                "bundle_schema_version": DYNAMICJEPA_BUNDLE_SCHEMA_VERSION,
                "run_id": self.run_id,
                "created_at_unix_ms": self.created_at_unix_ms,
                "updated_at_unix_ms": now_unix_ms()?,
                "purpose": self.purpose,
                "operator": operator,
                "repo": repo,
                "run_root": self.run_root,
                "run_dir": self.run_dir,
                "db_path": self.db_root,
                "artifact_root": self.artifact_root,
                "evidence_root": self.fsv_evidence_root,
                "fixtures_root": self.fixtures_root,
                "career_fixtures_root": self.career_fixtures_root,
                "onet_text_zip": self.onet_text_zip,
                "career_event_count": self.career_event_count,
                "career_training_seed": self.career_training_seed,
                "career_ablation_training_seed": self.career_ablation_training_seed,
                "career_ablation_training_seed_effective": self.career_ablation_training_seed_effective,
                "career_ablation_training_seed_origin": self.career_ablation_training_seed_origin,
                "hyperparameters": self.hp_overrides.manifest_value(),
                "hyperparameter_origin": self.hyperparameter_origin,
                "system_specs": self.system_specs,
                "status": status,
                "failure_details": failure_details,
            }),
        )
    }

    fn run_required(
        &mut self,
        name: &str,
        program: &Path,
        args: &[String],
    ) -> DynamicJepaResult<ResearchCommandCapture> {
        self.run_command(name, program, args, true)
    }

    fn run_command(
        &mut self,
        name: &str,
        program: &Path,
        args: &[String],
        required_success: bool,
    ) -> DynamicJepaResult<ResearchCommandCapture> {
        let index = self.command_index;
        self.command_index += 1;
        let started_at_unix_ms = now_unix_ms()?;
        let started = Instant::now();
        let mut command = Command::new(program);
        command.args(args);
        command.env("CONTEXT_GRAPH_ENV", "development");
        command.env("CUDA_COMPUTE_CAP", "120");
        let argv = std::iter::once(program.display().to_string())
            .chain(args.iter().cloned())
            .collect::<Vec<_>>();
        let output = match command.output() {
            Ok(output) => output,
            Err(err) => {
                let finished_at_unix_ms = now_unix_ms()?;
                let stderr = format!("failed to spawn {}: {err}", program.display());
                let capture = self.write_command_capture(CommandCaptureInput {
                    index,
                    name,
                    argv,
                    started_at_unix_ms,
                    finished_at_unix_ms,
                    elapsed_ms: started.elapsed().as_millis(),
                    exit_code: -1,
                    stdout: Vec::new(),
                    stderr: stderr.into_bytes(),
                    spawn_error: true,
                })?;
                return Err(DynamicJepaError::validation(
                    "research_smoke.command_spawn",
                    format!("command {} could not start", capture.name),
                    "inspect command_log.jsonl and command_outputs/*.stderr for the exact executable and OS error",
                ));
            }
        };
        let finished_at_unix_ms = now_unix_ms()?;
        let exit_code = output.status.code().unwrap_or(-1);
        let capture = self.write_command_capture(CommandCaptureInput {
            index,
            name,
            argv,
            started_at_unix_ms,
            finished_at_unix_ms,
            elapsed_ms: started.elapsed().as_millis(),
            exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
            spawn_error: false,
        })?;
        if required_success && capture.exit_code != 0 {
            return Err(DynamicJepaError::VerificationFailed {
                test_name: "mejepa_research_smoke".to_string(),
                message: format!(
                    "required command {} exited {}; stderr_tail={:?}",
                    capture.name, capture.exit_code, capture.row["stderr_tail"]
                ),
                evidence_path: self.run_dir.display().to_string(),
            });
        }
        Ok(capture)
    }

    fn write_command_capture(
        &mut self,
        input: CommandCaptureInput<'_>,
    ) -> DynamicJepaResult<ResearchCommandCapture> {
        let safe = safe_filename(input.name);
        let stdout_path = self
            .command_output_dir
            .join(format!("{:03}_{safe}.stdout", input.index));
        let stderr_path = self
            .command_output_dir
            .join(format!("{:03}_{safe}.stderr", input.index));
        fs::write(&stdout_path, &input.stdout).map_err(|err| DynamicJepaError::Storage {
            operation: "research_smoke.write_stdout".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to write {}: {err}", stdout_path.display()),
            remediation: "choose a writable --run-root".to_string(),
        })?;
        fs::write(&stderr_path, &input.stderr).map_err(|err| DynamicJepaError::Storage {
            operation: "research_smoke.write_stderr".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to write {}: {err}", stderr_path.display()),
            remediation: "choose a writable --run-root".to_string(),
        })?;
        let stdout = String::from_utf8_lossy(&input.stdout).to_string();
        let stderr = String::from_utf8_lossy(&input.stderr).to_string();
        let row = json!({
            "index": input.index,
            "name": input.name,
            "argv": input.argv,
            "started_at_unix_ms": input.started_at_unix_ms,
            "finished_at_unix_ms": input.finished_at_unix_ms,
            "elapsed_ms": input.elapsed_ms as u64,
            "exit_code": input.exit_code,
            "spawn_error": input.spawn_error,
            "stdout_path": stdout_path,
            "stderr_path": stderr_path,
            "stdout_sha256": hex_bytes(&sha256(&input.stdout)),
            "stderr_sha256": hex_bytes(&sha256(&input.stderr)),
            "stdout_tail": tail_lines(&stdout, 20),
            "stderr_tail": tail_lines(&stderr, 20),
        });
        append_jsonl(&self.command_log_path, &row)?;
        self.command_rows.push(row.clone());
        Ok(ResearchCommandCapture {
            index: input.index,
            name: input.name.to_string(),
            exit_code: input.exit_code,
            stdout,
            row,
        })
    }

    fn run_cli_json(&mut self, name: &str, args: Vec<String>) -> DynamicJepaResult<Value> {
        let bin = self.bin.clone();
        let capture = self.run_required(name, &bin, &args)?;
        serde_json::from_str(&capture.stdout).map_err(|err| DynamicJepaError::VerificationFailed {
            test_name: "mejepa_research_smoke".to_string(),
            message: format!(
                "command {name} did not emit one JSON object: {err}; stdout_path={}",
                capture.row["stdout_path"]
            ),
            evidence_path: self.run_dir.display().to_string(),
        })
    }

    fn finish_passed(self, summary: Value) -> DynamicJepaResult<CommandOutcome> {
        self.write_run_manifest("passed", Value::Null)?;
        Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_research_smoke",
                "status": "ok",
                "run_id": self.run_id,
                "run_dir": self.run_dir,
                "source_of_truth": {
                    "bundle": self.run_dir,
                    "rocksdb_root": self.db_root,
                    "artifact_root": self.artifact_root,
                    "evidence_root": self.fsv_evidence_root,
                },
                "summary": summary,
            }),
            exit_code: 0,
        })
    }

    fn finish_failed(self, err: DynamicJepaError) -> DynamicJepaResult<CommandOutcome> {
        let failure = json!({
            "error_code": err.code(),
            "error_message": err.to_string(),
            "remediation": remediation(&err),
        });
        self.write_run_manifest("failed", failure.clone())?;
        let fsv_path = self.run_dir.join("fsv_report.json");
        if !fsv_path.exists() {
            write_json_file(
                &fsv_path,
                &json!({
                    "status": "failed",
                    "source_of_truth": {
                        "bundle": self.run_dir,
                        "rocksdb_root": self.db_root,
                        "artifact_root": self.artifact_root,
                        "evidence_root": self.fsv_evidence_root,
                    },
                    "failure_details": failure,
                    "domain_reports": self.domain_reports,
                }),
            )?;
        }
        Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_research_smoke",
                "status": "error",
                "run_id": self.run_id,
                "run_dir": self.run_dir,
                "error_code": err.code(),
                "error_message": err.to_string(),
                "remediation": remediation(&err),
                "source_of_truth": {
                    "bundle": self.run_dir,
                    "command_log": self.command_log_path,
                    "run_manifest": self.run_dir.join("run_manifest.json"),
                },
            }),
            exit_code: 1,
        })
    }
}

struct CommandCaptureInput<'a> {
    index: usize,
    name: &'a str,
    argv: Vec<String>,
    started_at_unix_ms: i64,
    finished_at_unix_ms: i64,
    elapsed_ms: u128,
    exit_code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    spawn_error: bool,
}

fn run_research_smoke(ctx: &mut ResearchSmokeContext) -> DynamicJepaResult<Value> {
    validate_research_fixture_root(ctx)?;
    if ctx.onet_text_zip.is_some() {
        let career_fixtures = materialize_onet_career_demo_fixtures(ctx)?;
        ctx.career_fixtures_root = career_fixtures;
        validate_career_fixture_root(ctx)?;
    }
    write_environment_bundle(ctx)?;
    write_git_state_bundle(ctx)?;
    write_data_manifest_bundle(ctx)?;
    write_domain_pack_manifest_bundle(ctx)?;

    let mut db_counts_before = BTreeMap::new();
    let mut db_counts_after = BTreeMap::new();
    let domain_runs = [
        ("counter_world", "verify-counter-world"),
        ("gridworld_5x5", "verify-gridworld"),
        ("career_taxonomy", "verify-career-taxonomy"),
    ];
    for (domain, command_name) in domain_runs {
        let db = ctx.db_path(domain)?;
        let before = ctx.run_cli_json(
            &format!("{domain}_inspect_counts_before"),
            vec![
                "dynamicjepa".to_string(),
                "inspect-counts".to_string(),
                "--db".to_string(),
                db.display().to_string(),
                "--allow-missing".to_string(),
                "--json".to_string(),
            ],
        )?;
        db_counts_before.insert(domain.to_string(), before["counts"].clone());
        let artifact_root = ctx.artifact_root.clone();
        let fixtures_root = ctx.fixtures_root_for_domain(domain);
        let output = run_research_domain_verifier(
            ctx,
            domain,
            command_name,
            &db,
            &artifact_root,
            &fixtures_root,
        )?;
        let after = verify_domain_source_of_truth(ctx, domain, &db, &output)?;
        db_counts_after.insert(domain.to_string(), after["counts"].clone());
        collect_domain_evidence(ctx, domain, &output)?;
    }

    run_career_reproducibility(ctx)?;
    run_career_ablation_matrix(ctx)?;

    write_json_file(
        &ctx.run_dir.join("db_counts_before.json"),
        &db_counts_before,
    )?;
    write_json_file(&ctx.run_dir.join("db_counts_after.json"), &db_counts_after)?;
    write_json_file(
        &ctx.run_dir.join("training_metrics.json"),
        &ctx.training_rows,
    )?;
    write_json_file(
        &ctx.run_dir.join("artifact_hashes.json"),
        &ctx.artifact_rows,
    )?;
    write_json_file(&ctx.run_dir.join("edge_case_results.json"), &ctx.edge_rows)?;
    write_json_file(
        &ctx.run_dir.join("ablation_results.json"),
        &ctx.ablation_rows,
    )?;
    write_evaluation_report(ctx)?;
    write_fsv_report(ctx, "passed", Value::Null)?;
    write_paper_tables(ctx)?;
    write_plot_manifest(ctx)?;

    Ok(json!({
        "domains": ctx.domain_reports.keys().cloned().collect::<Vec<_>>(),
        "verification_runs": ctx.domain_reports
            .iter()
            .map(|(domain, report)| json!({
                "domain": domain,
                "verification_run_id": report["verification_run_id"],
                "db_path": report["db_path"],
                "evidence_dir": report["evidence_dir"],
                "status": report["status"],
            }))
            .collect::<Vec<_>>(),
        "training_metric_rows": ctx.training_rows.len(),
        "artifact_hash_rows": ctx.artifact_rows.len(),
        "edge_case_rows": ctx.edge_rows.len(),
        "ablation_rows": ctx.ablation_rows.len(),
        "command_count": ctx.command_rows.len(),
    }))
}

fn validate_research_run_id(run_id: &str) -> DynamicJepaResult<()> {
    if run_id.is_empty()
        || run_id == "."
        || run_id == ".."
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(DynamicJepaError::validation(
            "research_smoke.run_id",
            format!("invalid research run id: {run_id:?}"),
            "use only ASCII letters, digits, dot, dash, or underscore; do not include slashes or empty IDs",
        ));
    }
    Ok(())
}

fn validate_research_purpose(purpose: &str) -> DynamicJepaResult<()> {
    if !matches!(purpose, "smoke" | "paper_small" | "paper_full") {
        return Err(DynamicJepaError::validation(
            "research_smoke.purpose",
            format!("unsupported research purpose {purpose:?}"),
            "use smoke, paper_small, or paper_full",
        ));
    }
    Ok(())
}

fn research_career_event_count(
    purpose: &str,
    requested: Option<u64>,
    uses_onet: bool,
) -> DynamicJepaResult<u64> {
    if !uses_onet {
        if let Some(count) = requested {
            return Err(DynamicJepaError::validation(
                "research_smoke.career_event_count",
                format!("--career-event-count={count} was supplied without --onet-text-zip"),
                "supply --onet-text-zip to generate an O*NET-backed career fixture, or omit --career-event-count for the checked-in smoke fixture",
            ));
        }
        return Ok(0);
    }
    let count = requested.unwrap_or(match purpose {
        "smoke" => 120,
        "paper_small" => 1000,
        "paper_full" => 10000,
        _ => unreachable!("validated research purpose"),
    });
    if count < 15 {
        return Err(DynamicJepaError::validation(
            "research_smoke.career_event_count",
            format!("O*NET-backed career_event_count must be at least 15, got {count}"),
            "use enough rows to populate train, validation, and test splits plus edge-case verification",
        ));
    }
    if purpose == "paper_small" && count < 1000 {
        return Err(DynamicJepaError::validation(
            "research_smoke.career_event_count",
            format!("paper_small requires at least 1000 O*NET-derived career events, got {count}"),
            "use --purpose smoke for bounded generator tests, or raise --career-event-count for paper_small",
        ));
    }
    if purpose == "paper_full" && count < 10000 {
        return Err(DynamicJepaError::validation(
            "research_smoke.career_event_count",
            format!("paper_full requires at least 10000 O*NET-derived career events, got {count}"),
            "use --purpose paper_small for 1K-10K event runs or raise --career-event-count for paper_full",
        ));
    }
    Ok(count)
}

#[derive(Debug, Clone, Default, Serialize)]
struct ResearchHyperparameterOverrides {
    learning_rate: Option<f64>,
    latent_dim: Option<u64>,
    batch_size: Option<u64>,
    epochs: Option<u64>,
}

impl ResearchHyperparameterOverrides {
    fn from_research_args(args: &ResearchSmokeArgs) -> DynamicJepaResult<Self> {
        let overrides = Self {
            learning_rate: args.hp_learning_rate,
            latent_dim: args.hp_latent_dim,
            batch_size: args.hp_batch_size,
            epochs: args.hp_epochs,
        };
        validate_research_hyperparameter_overrides(&overrides, args.onet_text_zip.is_some())?;
        Ok(overrides)
    }

    fn is_empty(&self) -> bool {
        self.learning_rate.is_none()
            && self.latent_dim.is_none()
            && self.batch_size.is_none()
            && self.epochs.is_none()
    }

    fn manifest_value(&self) -> Value {
        json!({
            "learning_rate": self.learning_rate,
            "latent_dim": self.latent_dim,
            "batch_size": self.batch_size,
            "epochs": self.epochs,
            "overridden": !self.is_empty(),
        })
    }
}

fn validate_research_hyperparameter_overrides(
    overrides: &ResearchHyperparameterOverrides,
    uses_onet: bool,
) -> DynamicJepaResult<()> {
    if overrides.is_empty() {
        return Ok(());
    }
    if !uses_onet {
        return Err(DynamicJepaError::validation(
            "research_smoke.hyperparameters",
            "hyperparameter overrides were supplied without --onet-text-zip",
            "supply --onet-text-zip so generated O*NET training configs receive the overrides; overrides are never silently ignored",
        ));
    }
    if let Some(lr) = overrides.learning_rate {
        if !lr.is_finite() || lr <= 0.0 {
            return Err(DynamicJepaError::validation(
                "research_smoke.hp_learning_rate",
                format!("learning rate must be finite and positive, got {lr}"),
                "use a positive finite learning-rate value",
            ));
        }
    }
    if let Some(latent_dim) = overrides.latent_dim {
        if latent_dim == 0 {
            return Err(DynamicJepaError::validation(
                "research_smoke.hp_latent_dim",
                "latent dimension must be positive",
                "use a positive latent dimension compatible with encoder and predictor heads",
            ));
        }
    }
    if let Some(batch_size) = overrides.batch_size {
        if batch_size == 0 {
            return Err(DynamicJepaError::validation(
                "research_smoke.hp_batch_size",
                "batch size must be positive",
                "use a positive batch size",
            ));
        }
    }
    if let Some(epochs) = overrides.epochs {
        if epochs == 0 {
            return Err(DynamicJepaError::validation(
                "research_smoke.hp_epochs",
                "epochs must be positive",
                "use a positive epoch count",
            ));
        }
    }
    Ok(())
}

fn validate_research_career_training_seed(
    career_seed: Option<u64>,
    ablation_seed: Option<u64>,
    uses_onet: bool,
) -> DynamicJepaResult<()> {
    if career_seed.is_some() && !uses_onet {
        return Err(DynamicJepaError::validation(
            "research_smoke.career_training_seed",
            "--career-training-seed was supplied without --onet-text-zip",
            "supply --onet-text-zip so research-smoke can materialize an O*NET career train config with the requested seed",
        ));
    }
    if ablation_seed.is_some() && !uses_onet {
        return Err(DynamicJepaError::validation(
            "research_smoke.career_ablation_training_seed",
            "--career-ablation-training-seed was supplied without --onet-text-zip",
            "supply --onet-text-zip so research-smoke can materialize career ablation train configs with the requested seed",
        ));
    }
    Ok(())
}

fn resolve_career_ablation_seed_control(
    fixtures_root: &Path,
    career_seed: Option<u64>,
    explicit_ablation_seed: Option<u64>,
) -> DynamicJepaResult<(Option<u64>, String)> {
    if let Some(seed) = explicit_ablation_seed {
        return Ok((Some(seed), "explicit_cli".to_string()));
    }
    let checked_in_config = fixtures_root.join("verification/career_taxonomy_train_config.json");
    let config = read_json_file(&checked_in_config)?;
    let seed = config["seed"].as_u64().ok_or_else(|| {
        DynamicJepaError::validation(
            "research_smoke.career_ablation_training_seed",
            format!(
                "career train config {} is missing numeric seed",
                checked_in_config.display()
            ),
            "restore verification/career_taxonomy_train_config.json with an explicit seed or pass --career-ablation-training-seed",
        )
    })?;
    let origin = if career_seed.is_some() {
        "source_fixture_control_separated_from_main_seed"
    } else {
        "source_fixture_control"
    };
    Ok((Some(seed), origin.to_string()))
}

fn validate_research_fixture_root(ctx: &ResearchSmokeContext) -> DynamicJepaResult<()> {
    if !ctx.fixtures_root.exists() {
        return Err(DynamicJepaError::validation(
            "research_smoke.fixtures_root",
            format!(
                "fixtures root does not exist: {}",
                ctx.fixtures_root.display()
            ),
            "point --fixtures-root at configs/dynamicjepa from this repository",
        ));
    }
    let mut required = Vec::new();
    for domain in [
        VerificationDomain::CounterWorld,
        VerificationDomain::Gridworld,
        VerificationDomain::CareerTaxonomy,
    ] {
        required.push(domain.domain_pack_path(&ctx.fixtures_root));
        required.push(domain.train_config_path(&ctx.fixtures_root));
        for (_, path, _) in domain.edge_fixtures(&ctx.fixtures_root) {
            required.push(path);
        }
        if let Some(path) = domain.data_manifest_path(&ctx.fixtures_root) {
            required.push(path);
        }
    }
    required.push(
        ctx.fixtures_root
            .join("verification/gridworld_5x5_corpus.jsonl"),
    );
    required.push(
        ctx.fixtures_root
            .join("verification/career_taxonomy_smoke.jsonl"),
    );
    for path in required {
        if !path.exists() {
            return Err(DynamicJepaError::validation(
                "research_smoke.fixture_file",
                format!("required fixture file is missing: {}", path.display()),
                "restore the checked-in DynamicJEPA fixture set before running research smoke",
            ));
        }
    }
    Ok(())
}

fn validate_career_fixture_root(ctx: &ResearchSmokeContext) -> DynamicJepaResult<()> {
    for path in [
        VerificationDomain::CareerTaxonomy.domain_pack_path(&ctx.career_fixtures_root),
        VerificationDomain::CareerTaxonomy.train_config_path(&ctx.career_fixtures_root),
        ctx.career_fixtures_root
            .join("verification/career_taxonomy_data_manifest.json"),
        ctx.career_fixtures_root
            .join("verification/career_taxonomy_smoke.jsonl"),
    ] {
        if !path.exists() {
            return Err(DynamicJepaError::validation(
                "research_smoke.career_fixture_file",
                format!("required generated career fixture file is missing: {}", path.display()),
                "inspect the O*NET fixture generator output; all generated career files must exist before verification",
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct OnetOccupation {
    code: String,
    title: String,
    cluster: &'static str,
    job_zone: i64,
    skill_vector: [f64; 4],
}

#[derive(Debug)]
struct OnetGeneratedCorpus {
    jsonl: String,
    walk_csv: String,
    selected_occupation_count: usize,
    selected_related_edge_count: usize,
    source_occupation_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct OnetRequiredTsvMember {
    path: &'static str,
    label: &'static str,
    required_headers: &'static [&'static str],
    exact_data_rows: usize,
}

#[derive(Debug, Clone, Serialize)]
struct OnetZipPreflight {
    status: &'static str,
    source_zip_path: String,
    source_zip_sha256: String,
    archive_entry_count: usize,
    archive_file_count: usize,
    required_root: &'static str,
    required_release_marker: &'static str,
    required_license_marker: &'static str,
    source_urls: Vec<&'static str>,
    required_members: Vec<OnetZipMemberEvidence>,
}

#[derive(Debug, Clone, Serialize)]
struct OnetZipMemberEvidence {
    path: String,
    size_bytes: u64,
    compressed_size_bytes: u64,
    crc32: u32,
    row_count: Option<usize>,
    required_headers: Vec<&'static str>,
    header_columns: Option<Vec<String>>,
}

struct OnetZipMemberRead {
    text: String,
    size_bytes: u64,
    compressed_size_bytes: u64,
    crc32: u32,
}

const ONET_TEXT_ZIP_ROOT: &str = "db_30_2_text/";
const ONET_REQUIRED_RELEASE_MARKER: &str = "O*NET 30.2 Database";
const ONET_REQUIRED_LICENSE_MARKER: &str = "Creative Commons Attribution 4.0 International License";
const ONET_README_MEMBER: &str = "db_30_2_text/Read Me.txt";
const ONET_REQUIRED_FILE_COUNT: usize = 41;
const ONET_REQUIRED_TSV_MEMBERS: &[OnetRequiredTsvMember] = &[
    OnetRequiredTsvMember {
        path: "db_30_2_text/Occupation Data.txt",
        label: "Occupation Data",
        required_headers: &["O*NET-SOC Code", "Title", "Description"],
        exact_data_rows: 1016,
    },
    OnetRequiredTsvMember {
        path: "db_30_2_text/Job Zones.txt",
        label: "Job Zones",
        required_headers: &["O*NET-SOC Code", "Job Zone", "Date", "Domain Source"],
        exact_data_rows: 923,
    },
    OnetRequiredTsvMember {
        path: "db_30_2_text/Skills.txt",
        label: "Skills",
        required_headers: &[
            "O*NET-SOC Code",
            "Element ID",
            "Element Name",
            "Scale ID",
            "Data Value",
            "Date",
            "Domain Source",
        ],
        exact_data_rows: 62580,
    },
    OnetRequiredTsvMember {
        path: "db_30_2_text/Related Occupations.txt",
        label: "Related Occupations",
        required_headers: &[
            "O*NET-SOC Code",
            "Related O*NET-SOC Code",
            "Relatedness Tier",
            "Index",
        ],
        exact_data_rows: 18460,
    },
];

fn materialize_onet_career_demo_fixtures(ctx: &ResearchSmokeContext) -> DynamicJepaResult<PathBuf> {
    let onet_zip = ctx.onet_text_zip.as_ref().ok_or_else(|| {
        DynamicJepaError::validation(
            "research_smoke.onet_text_zip",
            "O*NET fixture generation was requested without --onet-text-zip",
            "pass the local O*NET 30.2 text ZIP path",
        )
    })?;
    if !onet_zip.exists() {
        let err = DynamicJepaError::validation(
            "research_smoke.onet_text_zip",
            format!("O*NET text ZIP does not exist: {}", onet_zip.display()),
            "download https://www.onetcenter.org/dl_files/database/db_30_2_text.zip and pass its local path",
        );
        write_onet_preflight_failure(ctx, onet_zip, None, &err)?;
        return Err(err);
    }
    let zip_hash = match file_sha256(onet_zip) {
        Ok(hash) => hash,
        Err(err) => {
            write_onet_preflight_failure(ctx, onet_zip, None, &err)?;
            return Err(err);
        }
    };
    let zip_hash_hex = hex_bytes(&zip_hash);
    let preflight = match validate_onet_text_zip(onet_zip, &zip_hash_hex) {
        Ok(preflight) => preflight,
        Err(err) => {
            write_onet_preflight_failure(ctx, onet_zip, Some(&zip_hash_hex), &err)?;
            return Err(err);
        }
    };
    write_json_file(&ctx.run_dir.join("onet_source_preflight.json"), &preflight)?;
    let generated = build_onet_career_corpus(onet_zip, ctx.career_event_count, &zip_hash_hex)?;
    let fixtures = ctx.run_dir.join("generated_fixtures/onet_30_2");
    fs::create_dir_all(fixtures.join("domain_packs")).map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.onet.create_domain_pack_dir".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to create generated domain pack dir: {err}"),
        remediation: "choose a writable --run-root".to_string(),
    })?;
    fs::create_dir_all(fixtures.join("verification")).map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.onet.create_verification_dir".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to create generated verification dir: {err}"),
        remediation: "choose a writable --run-root".to_string(),
    })?;

    write_onet_career_domain_pack(
        &ctx.fixtures_root
            .join("domain_packs/career_taxonomy.v1.toml"),
        &fixtures.join("domain_packs/career_taxonomy.v1.toml"),
        ctx.career_event_count,
    )?;
    write_onet_career_train_config(
        &ctx.fixtures_root
            .join("verification/career_taxonomy_train_config.json"),
        &fixtures.join("verification/career_taxonomy_train_config.json"),
        &ctx.purpose,
        ctx.career_training_seed,
        &ctx.hp_overrides,
    )?;

    let corpus_path = fixtures.join("verification/career_taxonomy_smoke.jsonl");
    write_text_file(&corpus_path, &generated.jsonl)?;
    let corpus_hash_hex = hex_bytes(&file_sha256(&corpus_path)?);
    let walk_path = fixtures.join("verification/career_taxonomy_onet_walk.csv");
    write_text_file(&walk_path, &generated.walk_csv)?;
    let walk_hash_hex = hex_bytes(&file_sha256(&walk_path)?);

    for rel in [
        "career_taxonomy_edge_empty.jsonl",
        "career_taxonomy_edge_missing_license.jsonl",
        "career_taxonomy_edge_invalid_action.jsonl",
        "career_taxonomy_edge_wrong_vector_dim.jsonl",
        "career_taxonomy_edge_oob_job_zone.jsonl",
    ] {
        copy_fixture_file(
            &ctx.fixtures_root.join("verification").join(rel),
            &fixtures.join("verification").join(rel),
        )?;
    }

    write_json_file(
        &fixtures.join("verification/career_taxonomy_data_manifest.json"),
        &json!({
            "name": "onet_30_2_generated_career_taxonomy_v1",
            "source_urls": [
                "https://www.onetcenter.org/database.html",
                "https://www.onetcenter.org/license_db.html",
                "https://www.onetcenter.org/dl_files/database/db_30_2_text.zip"
            ],
            "license": "O*NET 30.2 Database, Creative Commons Attribution 4.0 International License (CC BY 4.0). Generated fixture modifies O*NET database information for a public taxonomy benchmark; USDOL/ETA has not approved, endorsed, or tested these modifications.",
            "local_fixture": corpus_path.display().to_string(),
            "row_count": ctx.career_event_count,
            "allowed_use": "research-demo",
            "provenance_notes": "Generated deterministically from O*NET 30.2 downloadable text database files: Occupation Data, Job Zones, Skills, and Related Occupations. This is a public taxonomy benchmark, not real recruiting history.",
            "source_zip_path": onet_zip.display().to_string(),
            "source_zip_sha256": zip_hash_hex,
            "generated_fixture_sha256": corpus_hash_hex,
            "generated_walk_path": walk_path.display().to_string(),
            "generated_walk_sha256": walk_hash_hex,
            "generator": "dynamicjepa research-smoke O*NET career taxonomy generator v1",
            "source_occupation_count": generated.source_occupation_count,
            "selected_occupation_count": generated.selected_occupation_count,
            "selected_related_edge_count": generated.selected_related_edge_count,
        }),
    )?;
    Ok(fixtures)
}

fn write_onet_preflight_failure(
    ctx: &ResearchSmokeContext,
    onet_zip: &Path,
    zip_hash_hex: Option<&str>,
    err: &DynamicJepaError,
) -> DynamicJepaResult<()> {
    write_json_file(
        &ctx.run_dir.join("onet_source_preflight.json"),
        &json!({
            "status": "failed",
            "source_zip_path": onet_zip.display().to_string(),
            "source_zip_sha256": zip_hash_hex,
            "error_code": err.code(),
            "error_message": err.to_string(),
            "remediation": remediation(err),
            "source_of_truth": {
                "archive": onet_zip,
                "expected_root": ONET_TEXT_ZIP_ROOT,
                "required_release_marker": ONET_REQUIRED_RELEASE_MARKER,
                "required_license_marker": ONET_REQUIRED_LICENSE_MARKER,
                "required_members": ONET_REQUIRED_TSV_MEMBERS
                    .iter()
                    .map(|member| member.path)
                    .collect::<Vec<_>>(),
            },
        }),
    )
}

fn validate_onet_text_zip(
    zip_path: &Path,
    zip_hash_hex: &str,
) -> DynamicJepaResult<OnetZipPreflight> {
    let file = File::open(zip_path).map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.onet.open_zip".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to open {}: {err}", zip_path.display()),
        remediation: "pass a readable O*NET 30.2 text ZIP".to_string(),
    })?;
    let mut archive = ZipArchive::new(file).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.onet.zip",
            format!("failed to read O*NET ZIP {}: {err}", zip_path.display()),
            "pass the unmodified O*NET text ZIP downloaded from the Resource Center",
        )
    })?;
    let archive_entry_count = archive.len();
    if archive_entry_count == 0 {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet.zip",
            format!("O*NET ZIP {} has no entries", zip_path.display()),
            "pass the non-empty O*NET 30.2 text ZIP downloaded from the Resource Center",
        ));
    }

    let mut names = BTreeSet::new();
    let mut archive_file_count = 0usize;
    for idx in 0..archive_entry_count {
        let file = archive.by_index(idx).map_err(|err| {
            DynamicJepaError::validation(
                "research_smoke.onet.zip_entry",
                format!("failed to read O*NET ZIP entry {idx}: {err}"),
                "pass a valid O*NET 30.2 text ZIP; every archive entry must be readable",
            )
        })?;
        let raw_name = file.name().to_string();
        if file.enclosed_name().is_none() {
            return Err(DynamicJepaError::validation(
                "research_smoke.onet.zip_member_path",
                format!("O*NET ZIP member has unsafe path: {raw_name:?}"),
                "use the unmodified O*NET text ZIP; archive members must not be absolute paths or escape the archive root",
            ));
        }
        if file.encrypted() {
            return Err(DynamicJepaError::validation(
                "research_smoke.onet.zip_member_encrypted",
                format!("O*NET ZIP member is encrypted: {raw_name}"),
                "use the unmodified O*NET 30.2 text ZIP; required source files must be readable without passwords",
            ));
        }
        if !raw_name.starts_with(ONET_TEXT_ZIP_ROOT) {
            return Err(DynamicJepaError::validation(
                "research_smoke.onet.zip_root",
                format!("O*NET ZIP member {raw_name:?} is outside required root {ONET_TEXT_ZIP_ROOT:?}"),
                "use the db_30_2_text.zip archive; all text database members must live under db_30_2_text/",
            ));
        }
        if file.is_file() {
            archive_file_count += 1;
            names.insert(raw_name);
        }
    }
    if archive_file_count != ONET_REQUIRED_FILE_COUNT {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet.zip_file_count",
            format!(
                "O*NET 30.2 text ZIP has {archive_file_count} files, expected {ONET_REQUIRED_FILE_COUNT}"
            ),
            "use the complete unmodified O*NET 30.2 text database ZIP",
        ));
    }

    let readme =
        read_onet_zip_member_text_from_archive(&mut archive, zip_path, ONET_README_MEMBER)?;
    if !readme.text.contains(ONET_REQUIRED_RELEASE_MARKER) {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet.readme_release",
            format!("{ONET_README_MEMBER} does not contain {ONET_REQUIRED_RELEASE_MARKER:?}"),
            "use the O*NET 30.2 text database ZIP, not another release or format",
        ));
    }
    if !readme.text.contains(ONET_REQUIRED_LICENSE_MARKER) {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet.readme_license",
            format!("{ONET_README_MEMBER} does not contain the CC BY 4.0 license marker"),
            "use the official O*NET 30.2 text ZIP so attribution and license provenance are auditable",
        ));
    }

    let mut required_members = Vec::with_capacity(ONET_REQUIRED_TSV_MEMBERS.len() + 1);
    required_members.push(OnetZipMemberEvidence {
        path: ONET_README_MEMBER.to_string(),
        size_bytes: readme.size_bytes,
        compressed_size_bytes: readme.compressed_size_bytes,
        crc32: readme.crc32,
        row_count: None,
        required_headers: Vec::new(),
        header_columns: None,
    });
    for required in ONET_REQUIRED_TSV_MEMBERS {
        if !names.contains(required.path) {
            return Err(DynamicJepaError::validation(
                "research_smoke.onet.zip_member",
                format!("O*NET ZIP missing required member {}", required.path),
                "use the complete db_30_2_text.zip archive with Occupation Data, Job Zones, Skills, and Related Occupations",
            ));
        }
        let member = read_onet_zip_member_text_from_archive(&mut archive, zip_path, required.path)?;
        let header_columns = tsv_header_columns(&member.text, required.label)?;
        for header in required.required_headers {
            if !header_columns.iter().any(|column| column == header) {
                return Err(DynamicJepaError::validation(
                    "research_smoke.onet.tsv_header",
                    format!("{} missing required header {header:?}", required.path),
                    "use the official O*NET 30.2 tab-delimited text database files without editing column names",
                ));
            }
        }
        let rows = parse_tsv_records(&member.text, required.label)?;
        if rows.len() != required.exact_data_rows {
            return Err(DynamicJepaError::validation(
                "research_smoke.onet.tsv_row_count",
                format!(
                    "{} has {} data rows, expected {} for O*NET 30.2",
                    required.path,
                    rows.len(),
                    required.exact_data_rows
                ),
                "use the unmodified O*NET 30.2 text database ZIP; row-count drift must be investigated before generation",
            ));
        }
        required_members.push(OnetZipMemberEvidence {
            path: required.path.to_string(),
            size_bytes: member.size_bytes,
            compressed_size_bytes: member.compressed_size_bytes,
            crc32: member.crc32,
            row_count: Some(rows.len()),
            required_headers: required.required_headers.to_vec(),
            header_columns: Some(header_columns),
        });
    }

    Ok(OnetZipPreflight {
        status: "passed",
        source_zip_path: zip_path.display().to_string(),
        source_zip_sha256: zip_hash_hex.to_string(),
        archive_entry_count,
        archive_file_count,
        required_root: ONET_TEXT_ZIP_ROOT,
        required_release_marker: ONET_REQUIRED_RELEASE_MARKER,
        required_license_marker: ONET_REQUIRED_LICENSE_MARKER,
        source_urls: vec![
            "https://www.onetcenter.org/database.html",
            "https://www.onetcenter.org/license_db.html",
            "https://www.onetcenter.org/dl_files/database/db_30_2_text.zip",
        ],
        required_members,
    })
}

fn build_onet_career_corpus(
    zip_path: &Path,
    event_count: u64,
    zip_hash_hex: &str,
) -> DynamicJepaResult<OnetGeneratedCorpus> {
    let occupation_text = read_onet_zip_text(zip_path, "db_30_2_text/Occupation Data.txt")?;
    let job_zone_text = read_onet_zip_text(zip_path, "db_30_2_text/Job Zones.txt")?;
    let skills_text = read_onet_zip_text(zip_path, "db_30_2_text/Skills.txt")?;
    let related_text = read_onet_zip_text(zip_path, "db_30_2_text/Related Occupations.txt")?;

    let job_zones = parse_onet_job_zones(&job_zone_text)?;
    let skill_vectors = parse_onet_skill_vectors(&skills_text)?;
    let mut occupations = BTreeMap::new();
    let occupation_rows = parse_tsv_records(&occupation_text, "Occupation Data")?;
    let source_occupation_count = occupation_rows.len();
    for row in occupation_rows {
        let code = required_tsv_value(&row, "O*NET-SOC Code", "Occupation Data")?.to_string();
        let title = required_tsv_value(&row, "Title", "Occupation Data")?.to_string();
        let description = required_tsv_value(&row, "Description", "Occupation Data")?.to_string();
        let Some(cluster) = classify_onet_occupation(&code, &title, &description) else {
            continue;
        };
        let Some(job_zone) = job_zones.get(&code).copied() else {
            continue;
        };
        let Some(skill_vector) = skill_vectors.get(&code).copied() else {
            continue;
        };
        occupations.insert(
            code.clone(),
            OnetOccupation {
                code,
                title,
                cluster,
                job_zone,
                skill_vector,
            },
        );
    }
    if occupations.len() < 8 {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet.selected_occupations",
            format!(
                "O*NET source produced only {} selected occupations with complete job-zone and skill data",
                occupations.len()
            ),
            "inspect the O*NET ZIP contents; the generator requires complete Occupation Data, Job Zones, Skills, and Related Occupations files",
        ));
    }

    let related = parse_onet_related_edges(&related_text, &occupations)?;
    let selected_related_edge_count = related.values().map(Vec::len).sum::<usize>();
    if selected_related_edge_count < 8 {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet.related_edges",
            format!("O*NET selected related graph has only {selected_related_edge_count} edges"),
            "inspect Related Occupations.txt; selected public taxonomy occupations must have related occupation links",
        ));
    }

    let mut current_code = "15-1252.00".to_string();
    if !related.contains_key(&current_code) {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet.walk_start",
            "O*NET core computer-occupation graph is missing required start code 15-1252.00 Software Developers",
            "use the unmodified O*NET 30.2 text database; the public career demo requires the Software Developers related-occupation neighborhood",
        ));
    }
    if !occupations.contains_key(&current_code) {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet.walk_start",
            "O*NET selected occupations are missing required start code 15-1252.00 Software Developers",
            "inspect Occupation Data, Job Zones, and Skills for the Software Developers row",
        ));
    }
    related.keys().next().ok_or_else(|| {
        DynamicJepaError::validation(
            "research_smoke.onet.walk_start",
            "no selected O*NET occupation has an eligible outgoing related-occupation edge",
            "inspect the generated related graph",
        )
    })?;
    let mut out = String::new();
    let mut walk_csv =
        "step,current_code,current_title,current_cluster,next_code,next_title,next_cluster,action,mobility_label,edge_hash\n"
            .to_string();
    for step in 0..event_count {
        let neighbors = related.get(&current_code).ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.onet.walk",
                format!("walk reached occupation {current_code} with no eligible outgoing edge"),
                "the selected related graph must be closed over generated walk states",
            )
        })?;
        let current = occupations.get(&current_code).ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.onet.current_occupation",
                format!("missing selected occupation {current_code}"),
                "inspect generated O*NET occupation map",
            )
        })?;
        let next_code = choose_onet_next_code(current, neighbors, &occupations)?;
        let next = occupations.get(&next_code).ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.onet.next_occupation",
                format!("missing selected occupation {next_code}"),
                "inspect generated O*NET occupation map",
            )
        })?;
        let action = onet_action_for_transition(current, next, step);
        let mobility_label = onet_mobility_label(current, next);
        let edge_hash = format!("sha256:{zip_hash_hex}:{}:{}", current.code, next.code);
        walk_csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            step,
            csv_escape(&current.code),
            csv_escape(&current.title),
            csv_escape(current.cluster),
            csv_escape(&next.code),
            csv_escape(&next.title),
            csv_escape(next.cluster),
            csv_escape(action),
            csv_escape(mobility_label),
            csv_escape(&edge_hash),
        ));
        let event = json!({
            "trajectory_id": "onet_30_2_public_taxonomy_walk_0001",
            "step": step,
            "source": {
                "dataset": "onet_30_2_text",
                "license": "O*NET 30.2 Database CC BY 4.0",
                "url": "https://www.onetcenter.org/database.html",
                "source_hash": format!("sha256:{zip_hash_hex}"),
            },
            "state": {
                "occupation_cluster": current.cluster,
                "job_zone": current.job_zone,
                "skill_vector": rounded_skill_vector(current.skill_vector),
                "experience_years": onet_experience_years(step, event_count),
                "wage_band": onet_wage_band(current.job_zone, step),
            },
            "action": {
                "move": action,
            },
            "outcome": {
                "next_occupation_cluster": next.cluster,
                "next_job_zone": next.job_zone,
                "next_skill_vector": rounded_skill_vector(next.skill_vector),
                "mobility_label": mobility_label,
            },
        });
        out.push_str(&serde_json::to_string(&event).map_err(|err| {
            DynamicJepaError::validation(
                "research_smoke.onet.event_json",
                format!("failed to serialize generated O*NET event: {err}"),
                "generated career events must remain JSON serializable",
            )
        })?);
        out.push('\n');
        current_code = next_code;
    }
    Ok(OnetGeneratedCorpus {
        jsonl: out,
        walk_csv,
        selected_occupation_count: occupations.len(),
        selected_related_edge_count,
        source_occupation_count,
    })
}

fn read_onet_zip_text(zip_path: &Path, member: &str) -> DynamicJepaResult<String> {
    let file = File::open(zip_path).map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.onet.open_zip".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to open {}: {err}", zip_path.display()),
        remediation: "pass a readable O*NET 30.2 text ZIP".to_string(),
    })?;
    let mut archive = ZipArchive::new(file).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.onet.zip",
            format!("failed to read O*NET ZIP {}: {err}", zip_path.display()),
            "pass the unmodified O*NET text ZIP downloaded from the Resource Center",
        )
    })?;
    read_onet_zip_member_text_from_archive(&mut archive, zip_path, member).map(|member| member.text)
}

fn read_onet_zip_member_text_from_archive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    zip_path: &Path,
    member: &str,
) -> DynamicJepaResult<OnetZipMemberRead> {
    let mut member_file = archive.by_name(member).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.onet.zip_member",
            format!("O*NET ZIP missing required member {member}: {err}"),
            "use the db_30_2_text.zip archive with core text database files",
        )
    })?;
    if member_file.enclosed_name().is_none() {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet.zip_member_path",
            format!("O*NET ZIP member has unsafe path: {member:?}"),
            "use the unmodified O*NET text ZIP; archive members must not be absolute paths or escape the archive root",
        ));
    }
    if member_file.encrypted() {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet.zip_member_encrypted",
            format!("O*NET ZIP member is encrypted: {member}"),
            "use the unmodified O*NET 30.2 text ZIP; required source files must be readable without passwords",
        ));
    }
    let size_bytes = member_file.size();
    let compressed_size_bytes = member_file.compressed_size();
    let crc32 = member_file.crc32();
    let mut text = String::new();
    member_file.read_to_string(&mut text).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.onet.zip_member_utf8",
            format!(
                "required O*NET member {member} in {} is not readable UTF-8 text: {err}",
                zip_path.display()
            ),
            "use the tab-delimited text O*NET database ZIP",
        )
    })?;
    Ok(OnetZipMemberRead {
        text,
        size_bytes,
        compressed_size_bytes,
        crc32,
    })
}

fn tsv_header_columns(text: &str, label: &'static str) -> DynamicJepaResult<Vec<String>> {
    let header = text.lines().next().ok_or_else(|| {
        DynamicJepaError::validation(
            "research_smoke.onet.tsv",
            format!("{label} is empty"),
            "O*NET source files must contain a header row and data rows",
        )
    })?;
    let headers = header
        .trim_end_matches('\r')
        .split('\t')
        .map(str::to_string)
        .collect::<Vec<_>>();
    if headers.is_empty() {
        return Err(DynamicJepaError::validation(
            "research_smoke.onet.tsv_header",
            format!("{label} has no TSV columns"),
            "inspect the O*NET text file header",
        ));
    }
    Ok(headers)
}

fn parse_tsv_records(
    text: &str,
    label: &'static str,
) -> DynamicJepaResult<Vec<BTreeMap<String, String>>> {
    let mut lines = text.lines();
    let headers = tsv_header_columns(text, label)?;
    lines.next();
    let mut rows = Vec::new();
    for (idx, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let values = line
            .trim_end_matches('\r')
            .split('\t')
            .map(str::to_string)
            .collect::<Vec<_>>();
        if values.len() != headers.len() {
            return Err(DynamicJepaError::validation(
                "research_smoke.onet.tsv_width",
                format!(
                    "{label} line {} has {} columns, expected {}",
                    idx + 2,
                    values.len(),
                    headers.len()
                ),
                "do not edit the O*NET tab-delimited text files before generation",
            ));
        }
        rows.push(
            headers
                .iter()
                .cloned()
                .zip(values)
                .collect::<BTreeMap<_, _>>(),
        );
    }
    Ok(rows)
}

fn required_tsv_value<'a>(
    row: &'a BTreeMap<String, String>,
    field: &str,
    label: &'static str,
) -> DynamicJepaResult<&'a str> {
    row.get(field)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.onet.tsv_field",
                format!("{label} row is missing required field {field:?}"),
                "inspect the O*NET source file columns",
            )
        })
}

fn parse_onet_job_zones(text: &str) -> DynamicJepaResult<BTreeMap<String, i64>> {
    let mut out = BTreeMap::new();
    for row in parse_tsv_records(text, "Job Zones")? {
        let code = required_tsv_value(&row, "O*NET-SOC Code", "Job Zones")?.to_string();
        let zone = required_tsv_value(&row, "Job Zone", "Job Zones")?
            .parse::<i64>()
            .map_err(|err| {
                DynamicJepaError::validation(
                    "research_smoke.onet.job_zone",
                    format!("invalid O*NET job zone for {code}: {err}"),
                    "Job Zones.txt must contain integer zones",
                )
            })?;
        if !(1..=5).contains(&zone) {
            return Err(DynamicJepaError::validation(
                "research_smoke.onet.job_zone",
                format!("O*NET job zone for {code} is outside 1..5: {zone}"),
                "inspect Job Zones.txt before generation",
            ));
        }
        out.insert(code, zone);
    }
    Ok(out)
}

fn parse_onet_skill_vectors(text: &str) -> DynamicJepaResult<BTreeMap<String, [f64; 4]>> {
    let skill_ids = ["2.B.3.e", "2.A.1.e", "2.B.4.g", "2.A.2.a"];
    let mut partial: BTreeMap<String, [Option<f64>; 4]> = BTreeMap::new();
    for row in parse_tsv_records(text, "Skills")? {
        if required_tsv_value(&row, "Scale ID", "Skills")? != "LV" {
            continue;
        }
        let element_id = required_tsv_value(&row, "Element ID", "Skills")?;
        let Some(index) = skill_ids.iter().position(|id| *id == element_id) else {
            continue;
        };
        let code = required_tsv_value(&row, "O*NET-SOC Code", "Skills")?.to_string();
        let raw = required_tsv_value(&row, "Data Value", "Skills")?
            .parse::<f64>()
            .map_err(|err| {
                DynamicJepaError::validation(
                    "research_smoke.onet.skill_value",
                    format!("invalid O*NET skill value for {code}/{element_id}: {err}"),
                    "Skills.txt Data Value must be numeric",
                )
            })?;
        if !raw.is_finite() {
            return Err(DynamicJepaError::validation(
                "research_smoke.onet.skill_value",
                format!("non-finite O*NET skill value for {code}/{element_id}: {raw}"),
                "Skills.txt Data Value must be finite",
            ));
        }
        let normalized = (raw / 7.0).clamp(0.0, 1.0);
        partial.entry(code).or_insert([None; 4])[index] = Some(normalized);
    }
    let mut out = BTreeMap::new();
    for (code, values) in partial {
        if values.iter().all(Option::is_some) {
            out.insert(
                code,
                [
                    values[0].expect("checked"),
                    values[1].expect("checked"),
                    values[2].expect("checked"),
                    values[3].expect("checked"),
                ],
            );
        }
    }
    Ok(out)
}

fn parse_onet_related_edges(
    text: &str,
    occupations: &BTreeMap<String, OnetOccupation>,
) -> DynamicJepaResult<BTreeMap<String, Vec<String>>> {
    let mut raw: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in parse_tsv_records(text, "Related Occupations")? {
        let src = required_tsv_value(&row, "O*NET-SOC Code", "Related Occupations")?.to_string();
        let dst =
            required_tsv_value(&row, "Related O*NET-SOC Code", "Related Occupations")?.to_string();
        let Some(src_occ) = occupations.get(&src) else {
            continue;
        };
        let Some(dst_occ) = occupations.get(&dst) else {
            continue;
        };
        if is_onet_demo_core_occupation(src_occ)
            && is_onet_demo_core_occupation(dst_occ)
            && src != dst
        {
            raw.entry(src).or_default().insert(dst);
        }
    }
    let mut out = BTreeMap::new();
    for (src, dsts) in &raw {
        let filtered = dsts
            .iter()
            .filter(|dst| raw.contains_key(*dst))
            .cloned()
            .collect::<Vec<_>>();
        if !filtered.is_empty() {
            out.insert(src.clone(), filtered);
        }
    }
    Ok(out)
}

fn is_onet_demo_core_occupation(occupation: &OnetOccupation) -> bool {
    occupation.code.starts_with("15-")
        && matches!(occupation.cluster, "software" | "data" | "systems")
}

fn choose_onet_next_code(
    current: &OnetOccupation,
    neighbors: &[String],
    occupations: &BTreeMap<String, OnetOccupation>,
) -> DynamicJepaResult<String> {
    let ranked = neighbors
        .iter()
        .filter_map(|code| {
            occupations
                .get(code)
                .map(|candidate| (candidate, onet_transition_score(current, candidate), code))
        })
        .collect::<Vec<_>>();
    let candidates = if ranked
        .iter()
        .any(|(candidate, _, _)| candidate.cluster != current.cluster)
    {
        ranked
            .iter()
            .filter(|(candidate, _, _)| candidate.cluster != current.cluster)
            .copied()
            .collect::<Vec<_>>()
    } else {
        ranked.clone()
    };
    candidates
        .into_iter()
        .min_by(|(_, left_score, left_code), (_, right_score, right_code)| {
            left_score
                .total_cmp(right_score)
                .then_with(|| left_code.cmp(right_code))
        })
        .map(|(_, _, code)| code.clone())
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.onet.next_occupation",
                format!(
                    "occupation {} has no core computer-occupation neighbor with complete O*NET data",
                    current.code
                ),
                "inspect the O*NET Related Occupations, Job Zones, and Skills rows for the current occupation",
            )
        })
}

fn onet_transition_score(current: &OnetOccupation, candidate: &OnetOccupation) -> f64 {
    let cluster_penalty = if current.cluster == candidate.cluster {
        0.0
    } else {
        0.25
    };
    let job_zone_penalty = ((current.job_zone - candidate.job_zone).abs() as f64) * 0.1;
    let skill_penalty = current
        .skill_vector
        .iter()
        .zip(candidate.skill_vector.iter())
        .map(|(left, right)| {
            let delta = left - right;
            delta * delta
        })
        .sum::<f64>();
    cluster_penalty + job_zone_penalty + skill_penalty
}

fn classify_onet_occupation(code: &str, title: &str, description: &str) -> Option<&'static str> {
    let title_lower = title.to_ascii_lowercase();
    let description_lower = description.to_ascii_lowercase();
    if code.starts_with("11-")
        && (title_lower.contains("manager")
            || title_lower.contains("chief")
            || title_lower.contains("director"))
    {
        return Some("management");
    }
    if title_lower.contains("data")
        || title_lower.contains("database")
        || title_lower.contains("statistic")
        || title_lower.contains("analytics")
        || title_lower.contains("business intelligence")
    {
        return Some("data");
    }
    if title_lower.contains("software")
        || title_lower.contains("developer")
        || title_lower.contains("programmer")
        || title_lower.contains("web ")
        || title_lower.contains("application")
    {
        return Some("software");
    }
    if title_lower.contains("systems")
        || title_lower.contains("network")
        || title_lower.contains("security")
        || title_lower.contains("administrator")
        || title_lower.contains("architect")
    {
        return Some("systems");
    }
    if description_lower.contains("software")
        || description_lower.contains("developer")
        || description_lower.contains("programmer")
        || description_lower.contains("web ")
        || description_lower.contains("application")
    {
        return Some("software");
    }
    if description_lower.contains("data")
        || description_lower.contains("database")
        || description_lower.contains("statistic")
        || description_lower.contains("analytics")
        || description_lower.contains("business intelligence")
    {
        return Some("data");
    }
    if description_lower.contains("systems")
        || description_lower.contains("network")
        || description_lower.contains("security")
        || description_lower.contains("administrator")
        || description_lower.contains("architect")
    {
        return Some("systems");
    }
    if code.starts_with("15-") {
        return Some("systems");
    }
    None
}

fn rounded_skill_vector(values: [f64; 4]) -> [f64; 4] {
    [
        round4(values[0]),
        round4(values[1]),
        round4(values[2]),
        round4(values[3]),
    ]
}

fn round4(value: f64) -> f64 {
    (value * 10000.0).round() / 10000.0
}

fn onet_experience_years(step: u64, event_count: u64) -> f64 {
    let denominator = event_count
        .checked_sub(1)
        .expect("O*NET event_count is validated before corpus generation");
    let progress = (step as f64) / (denominator as f64);
    round4(1.0 + 39.0 * progress)
}

fn onet_wage_band(job_zone: i64, step: u64) -> &'static str {
    if job_zone >= 5 || step.is_multiple_of(17) {
        "high"
    } else if job_zone >= 3 {
        "mid"
    } else {
        "low"
    }
}

fn onet_action_for_transition(
    current: &OnetOccupation,
    next: &OnetOccupation,
    step: u64,
) -> &'static str {
    if current.cluster != next.cluster {
        "role_move_near"
    } else if step.is_multiple_of(13) {
        "experience_gain"
    } else if skill_sum(next.skill_vector) > skill_sum(current.skill_vector) + 0.03 {
        "skill_gain_high_demand"
    } else if step.is_multiple_of(11) {
        "noop"
    } else {
        "skill_gain_adjacent"
    }
}

fn skill_sum(values: [f64; 4]) -> f64 {
    values.iter().sum()
}

fn onet_mobility_label(current: &OnetOccupation, next: &OnetOccupation) -> &'static str {
    if current.cluster == next.cluster && current.job_zone == next.job_zone {
        "same"
    } else if current.cluster == next.cluster || (current.job_zone - next.job_zone).abs() <= 1 {
        "near_transition"
    } else {
        "far_transition"
    }
}

fn write_onet_career_domain_pack(
    src: &Path,
    dst: &Path,
    event_count: u64,
) -> DynamicJepaResult<()> {
    let text = read_text_file(src, "O*NET generated career domain pack")?;
    let mut value: toml::Value = toml::from_str(&text).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.onet.domain_pack",
            format!(
                "failed to parse base career domain pack {}: {err}",
                src.display()
            ),
            "fix the checked-in career domain pack before generating O*NET demo fixtures",
        )
    })?;
    let dataset_rows = event_count.checked_sub(1).ok_or_else(|| {
        DynamicJepaError::validation(
            "research_smoke.onet.event_count",
            "event_count must be at least 1",
            "use a positive generated career event count",
        )
    })?;
    let splits = split_counts_for_row_count(dataset_rows);
    let policy = value
        .get_mut("verification_policy")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.onet.domain_pack.verification_policy",
                "base career domain pack missing verification_policy table",
                "fix the checked-in career domain pack before generation",
            )
        })?;
    set_toml_integer(policy, "expected_trajectory_count", 1);
    set_toml_integer(policy, "expected_event_count", event_count);
    set_toml_integer(policy, "expected_panel_count", event_count);
    set_toml_integer(policy, "expected_dataset_row_count", dataset_rows);
    set_toml_integer(policy, "expected_train_rows_min", splits["train"]);
    set_toml_integer(policy, "expected_train_rows_max", splits["train"]);
    set_toml_integer(policy, "expected_val_rows_min", splits["val"]);
    set_toml_integer(policy, "expected_val_rows_max", splits["val"]);
    set_toml_integer(policy, "expected_test_rows_min", splits["test"]);
    set_toml_integer(policy, "expected_test_rows_max", splits["test"]);
    let serialized = toml::to_string_pretty(&value).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.onet.domain_pack.serialize",
            format!("failed to serialize generated career domain pack: {err}"),
            "inspect the O*NET domain-pack mutation",
        )
    })?;
    write_text_file(dst, &serialized)?;
    parse_domain_pack_file(dst)?;
    Ok(())
}

fn split_counts_for_row_count(total_rows: u64) -> BTreeMap<String, u64> {
    let train = ((total_rows * 7) / 10).max(1);
    let remaining = total_rows.saturating_sub(train);
    let val = if remaining == 0 {
        0
    } else {
        ((total_rows * 2).div_ceil(10)).max(1).min(remaining)
    };
    BTreeMap::from([
        ("train".to_string(), train),
        ("val".to_string(), val),
        ("test".to_string(), total_rows.saturating_sub(train + val)),
    ])
}

fn set_toml_integer(table: &mut toml::map::Map<String, toml::Value>, key: &str, value: u64) {
    table.insert(key.to_string(), toml::Value::Integer(value as i64));
}

fn write_onet_career_train_config(
    src: &Path,
    dst: &Path,
    purpose: &str,
    career_training_seed: Option<u64>,
    hp_overrides: &ResearchHyperparameterOverrides,
) -> DynamicJepaResult<()> {
    let mut config = read_json_file(src)?;
    let max_seconds = match purpose {
        "smoke" => 180,
        "paper_small" => 1800,
        "paper_full" => 7200,
        _ => unreachable!("validated research purpose"),
    };
    config["stopping"]["max_seconds"] = json!(max_seconds);
    config["stopping"]["target"] = json!(0.20);
    if matches!(purpose, "paper_small" | "paper_full") {
        config["schedule"]["batch_size"] = json!(64);
    }
    if let Some(seed) = career_training_seed {
        config["seed"] = json!(seed);
    }
    apply_research_hp_overrides(
        &mut config,
        hp_overrides,
        "research_smoke.onet.train_config",
    )?;
    let parsed: TrainConfig = serde_json::from_value(config.clone()).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.onet.train_config",
            format!("generated O*NET career train config is invalid: {err}"),
            "fix the checked-in career training config before generation",
        )
    })?;
    parsed.validate()?;
    write_json_file(dst, &config)
}

fn apply_research_hp_overrides(
    config: &mut Value,
    overrides: &ResearchHyperparameterOverrides,
    field_prefix: &str,
) -> DynamicJepaResult<()> {
    if overrides.is_empty() {
        return Ok(());
    }
    if let Some(lr) = overrides.learning_rate {
        require_json_object_path(config, &["optim"], field_prefix)?;
        config["optim"]["lr"] = json!(lr);
    }
    if let Some(latent_dim) = overrides.latent_dim {
        require_json_object_path(config, &["model", "encoder"], field_prefix)?;
        require_json_object_path(config, &["model", "predictor"], field_prefix)?;
        config["model"]["encoder"]["out_dim"] = json!(latent_dim);
        config["model"]["predictor"]["out_dim"] = json!(latent_dim);
    }
    if let Some(batch_size) = overrides.batch_size {
        require_json_object_path(config, &["schedule"], field_prefix)?;
        config["schedule"]["batch_size"] = json!(batch_size);
    }
    if let Some(epochs) = overrides.epochs {
        require_json_object_path(config, &["schedule"], field_prefix)?;
        config["schedule"]["epochs"] = json!(epochs);
    }
    Ok(())
}

fn require_json_object_path(
    value: &Value,
    path: &[&str],
    field_prefix: &str,
) -> DynamicJepaResult<()> {
    let mut cursor = value;
    let mut field = field_prefix.to_string();
    for segment in path {
        field.push('.');
        field.push_str(segment);
        cursor = cursor.get(*segment).ok_or_else(|| {
            DynamicJepaError::schema(
                field.clone(),
                format!("generated train config is missing object path {field}"),
                "restore the checked-in DynamicJEPA train config schema before applying HP overrides",
            )
        })?;
        if !cursor.is_object() {
            return Err(DynamicJepaError::schema(
                field,
                "generated train config path exists but is not a JSON object",
                "restore the checked-in DynamicJEPA train config schema before applying HP overrides",
            ));
        }
    }
    Ok(())
}

fn write_environment_bundle(ctx: &mut ResearchSmokeContext) -> DynamicJepaResult<()> {
    if !ctx.system_specs.exists() {
        return Err(DynamicJepaError::validation(
            "research_smoke.system_specs",
            format!("system specs file does not exist: {}", ctx.system_specs.display()),
            "pass --system-specs with a local hardware specification document, or restore docs2/systemspecs.md",
        ));
    }
    let system_specs_sha256 = hex_bytes(&file_sha256(&ctx.system_specs)?);
    let smi = ctx.run_required(
        "env_nvidia_smi",
        Path::new("nvidia-smi"),
        &[
            "--query-gpu=name,driver_version,compute_cap,memory.total".to_string(),
            "--format=csv,noheader".to_string(),
        ],
    )?;
    let nvcc = ctx.run_required(
        "env_nvcc_version",
        Path::new("nvcc"),
        &["--version".to_string()],
    )?;
    let rustc = ctx.run_required(
        "env_rustc_version",
        Path::new("rustc"),
        &["--version".to_string()],
    )?;
    let cargo = ctx.run_required(
        "env_cargo_version",
        Path::new("cargo"),
        &["--version".to_string()],
    )?;
    let candle_tree = ctx.run_required(
        "env_cargo_tree_candle_core",
        Path::new("cargo"),
        &[
            "tree".to_string(),
            "-p".to_string(),
            "candle-core".to_string(),
        ],
    )?;
    let uname = ctx.run_required("env_uname", Path::new("uname"), &["-a".to_string()])?;
    let lscpu = ctx.run_required("env_lscpu", Path::new("lscpu"), &[])?;
    let free = ctx.run_required("env_free", Path::new("free"), &["-h".to_string()])?;

    let nvidia_fields = smi
        .stdout
        .lines()
        .next()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    if nvidia_fields.len() < 4
        || !nvidia_fields[0].contains("RTX 5090")
        || nvidia_fields[2] != "12.0"
    {
        return Err(DynamicJepaError::VerificationFailed {
            test_name: "mejepa_research_smoke_environment".to_string(),
            message: format!(
                "expected RTX 5090 compute capability 12.0, got nvidia-smi row {:?}",
                smi.stdout.lines().next().unwrap_or("")
            ),
            evidence_path: ctx.run_dir.display().to_string(),
        });
    }
    if !nvcc.stdout.contains("release 13.2") {
        return Err(DynamicJepaError::VerificationFailed {
            test_name: "mejepa_research_smoke_environment".to_string(),
            message: "expected nvcc release 13.2 in environment smoke".to_string(),
            evidence_path: ctx.run_dir.display().to_string(),
        });
    }

    let env_json = json!({
        "effective_cuda_compute_cap": "120",
        "CUDA_COMPUTE_CAP_env": env::var("CUDA_COMPUTE_CAP").ok(),
        "CUDA_HOME": env::var("CUDA_HOME").ok(),
        "LD_LIBRARY_PATH_present": env::var("LD_LIBRARY_PATH").map(|v| !v.is_empty()).unwrap_or(false),
        "rustc": rustc.stdout.trim(),
        "cargo": cargo.stdout.trim(),
        "candle_tree_stdout_sha256": candle_tree.row["stdout_sha256"],
        "uname": uname.stdout.trim(),
        "free": free.stdout,
        "system_specs": {
            "path": ctx.system_specs.display().to_string(),
            "sha256": system_specs_sha256,
        },
        "command_indices": {
            "nvidia_smi": smi.index,
            "nvcc": nvcc.index,
            "rustc": rustc.index,
            "cargo": cargo.index,
            "candle_tree": candle_tree.index,
            "uname": uname.index,
            "lscpu": lscpu.index,
            "free": free.index,
        },
    });
    let hardware_json = json!({
        "gpu": {
            "name": nvidia_fields[0],
            "driver_version": nvidia_fields[1],
            "compute_capability": nvidia_fields[2],
            "memory_total": nvidia_fields[3],
        },
        "nvidia_smi_raw": smi.stdout,
        "nvcc_version_raw": nvcc.stdout,
        "cpu_raw": lscpu.stdout,
        "memory_raw": free.stdout,
    });
    write_json_file(&ctx.run_dir.join("environment.json"), &env_json)?;
    write_json_file(&ctx.run_dir.join("hardware.json"), &hardware_json)?;
    Ok(())
}

fn write_git_state_bundle(ctx: &mut ResearchSmokeContext) -> DynamicJepaResult<()> {
    let head = ctx.run_required(
        "git_rev_parse_head",
        Path::new("git"),
        &["rev-parse".to_string(), "HEAD".to_string()],
    )?;
    let status = ctx.run_required(
        "git_status_short",
        Path::new("git"),
        &[
            "status".to_string(),
            "--short".to_string(),
            "--untracked-files=all".to_string(),
        ],
    )?;
    let diff = ctx.run_required("git_diff", Path::new("git"), &["diff".to_string()])?;
    let ls_files = ctx.run_required(
        "git_ls_files_smokedocs",
        Path::new("git"),
        &[
            "ls-files".to_string(),
            "docs/5090jepa/smokedocs".to_string(),
            "configs/dynamicjepa".to_string(),
        ],
    )?;
    write_json_file(
        &ctx.run_dir.join("git_state.json"),
        &json!({
            "head": head.stdout.trim(),
            "status_short": status.stdout.lines().collect::<Vec<_>>(),
            "dirty_tree": !status.stdout.trim().is_empty(),
            "tracked_diff_sha256": hex_bytes(&sha256(diff.stdout.as_bytes())),
            "relevant_files": ls_files.stdout.lines().collect::<Vec<_>>(),
            "command_indices": {
                "head": head.index,
                "status": status.index,
                "diff": diff.index,
                "ls_files": ls_files.index,
            },
        }),
    )
}

fn write_data_manifest_bundle(ctx: &mut ResearchSmokeContext) -> DynamicJepaResult<()> {
    let path = ctx
        .career_fixtures_root
        .join("verification/career_taxonomy_data_manifest.json");
    let manifest: Value = read_json_file(&path)?;
    for field in [
        "name",
        "source_urls",
        "license",
        "local_fixture",
        "allowed_use",
        "row_count",
    ] {
        if manifest.get(field).is_none() {
            return Err(DynamicJepaError::validation(
                "research_smoke.data_manifest",
                format!("{} missing required field {field}", path.display()),
                "restore complete source URL, license, fixture, allowed-use, and row-count metadata",
            ));
        }
    }
    let local_fixture = manifest["local_fixture"].as_str().ok_or_else(|| {
        DynamicJepaError::validation(
            "research_smoke.data_manifest.local_fixture",
            "local_fixture must be a string",
            "fix career_taxonomy_data_manifest.json",
        )
    })?;
    let fixture_candidate = PathBuf::from(local_fixture);
    let fixture_path = if fixture_candidate.exists() {
        fixture_candidate
    } else if let Ok(relative_to_fixtures_root) =
        fixture_candidate.strip_prefix(Path::new("configs/dynamicjepa"))
    {
        ctx.fixtures_root.join(relative_to_fixtures_root)
    } else {
        ctx.fixtures_root.join(local_fixture)
    };
    let local_hash = file_sha256(&fixture_path)?;
    let mut output = Map::new();
    output.insert("source_manifest_path".to_string(), json!(path));
    output.insert(
        "source_manifest_sha256".to_string(),
        json!(hex_bytes(&file_sha256(&path)?)),
    );
    output.insert("sources".to_string(), json!([manifest]));
    output.insert("local_fixture_path".to_string(), json!(fixture_path));
    output.insert(
        "local_fixture_sha256".to_string(),
        json!(hex_bytes(&local_hash)),
    );
    let onet_preflight_path = ctx.run_dir.join("onet_source_preflight.json");
    if onet_preflight_path.exists() {
        output.insert(
            "onet_source_preflight_path".to_string(),
            json!(onet_preflight_path),
        );
        output.insert(
            "onet_source_preflight_sha256".to_string(),
            json!(hex_bytes(&file_sha256(&onet_preflight_path)?)),
        );
        output.insert(
            "onet_source_preflight".to_string(),
            read_json_file(&onet_preflight_path)?,
        );
    }
    write_json_file(&ctx.run_dir.join("data_manifest.json"), &output)
}

fn write_domain_pack_manifest_bundle(ctx: &ResearchSmokeContext) -> DynamicJepaResult<()> {
    let mut domains = Vec::new();
    for domain in [
        VerificationDomain::CounterWorld,
        VerificationDomain::Gridworld,
        VerificationDomain::CareerTaxonomy,
    ] {
        let fixture_root = if matches!(domain, VerificationDomain::CareerTaxonomy) {
            &ctx.career_fixtures_root
        } else {
            &ctx.fixtures_root
        };
        let path = domain.domain_pack_path(fixture_root);
        let pack = parse_domain_pack_file(&path)?;
        domains.push(json!({
            "domain": pack.id.0,
            "version": pack.version,
            "title": pack.title,
            "file": path,
            "sha256": hex_bytes(&file_sha256(&path)?),
            "instrument_count": pack.instrument_specs.len(),
            "adapter_count": pack.adapter_specs.len(),
            "objective_count": pack.objective_specs.len(),
            "verification_policy": pack.verification_policy,
            "source_hash_sha256": hex_bytes(&pack.source_hash),
        }));
    }
    write_json_file(
        &ctx.run_dir.join("domain_pack_manifest.json"),
        &json!({ "domains": domains }),
    )
}

fn run_research_domain_verifier(
    ctx: &mut ResearchSmokeContext,
    domain: &str,
    command_name: &str,
    db: &Path,
    artifact_root: &Path,
    fixtures_root: &Path,
) -> DynamicJepaResult<Value> {
    ctx.run_cli_json(
        &format!("{domain}_{command_name}"),
        vec![
            "dynamicjepa".to_string(),
            command_name.to_string(),
            "--db".to_string(),
            db.display().to_string(),
            "--fixtures-root".to_string(),
            fixtures_root.display().to_string(),
            "--artifact-root".to_string(),
            artifact_root.display().to_string(),
            "--evidence-root".to_string(),
            ctx.fsv_evidence_root.join(domain).display().to_string(),
            "--json".to_string(),
        ],
    )
}

fn verify_domain_source_of_truth(
    ctx: &mut ResearchSmokeContext,
    domain: &str,
    db: &Path,
    verifier_output: &Value,
) -> DynamicJepaResult<Value> {
    if verifier_output["status"].as_str() != Some("ok") {
        return Err(DynamicJepaError::VerificationFailed {
            test_name: "mejepa_research_smoke_domain".to_string(),
            message: format!("domain {domain} verifier returned non-ok status"),
            evidence_path: ctx.run_dir.display().to_string(),
        });
    }
    let after = ctx.run_cli_json(
        &format!("{domain}_inspect_counts_after"),
        vec![
            "dynamicjepa".to_string(),
            "inspect-counts".to_string(),
            "--db".to_string(),
            db.display().to_string(),
            "--json".to_string(),
        ],
    )?;
    if after["counts"] != verifier_output["after_counts"] {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "research smoke count readback mismatch for {domain}: verifier_after={} inspect_counts={}",
                verifier_output["after_counts"], after["counts"]
            ),
        });
    }
    for cf in [
        CF_DJ_VERIFICATION_RUNS,
        CF_DJ_MODEL_ARTIFACTS,
        CF_DJ_TRAINING_RUNS,
        CF_DJ_PAIRWISE_READINGS,
        CF_DJ_PLAN_TRACES,
        CF_DJ_GUARD_DECISIONS,
    ] {
        let readback = ctx.run_cli_json(
            &format!("{domain}_inspect_cf_{cf}"),
            vec![
                "dynamicjepa".to_string(),
                "inspect-cf".to_string(),
                "--db".to_string(),
                db.display().to_string(),
                "--cf".to_string(),
                cf.to_string(),
                "--limit".to_string(),
                "20".to_string(),
                "--json".to_string(),
            ],
        )?;
        write_json_file(
            &ctx.decoded_records_dir.join(format!("{domain}_{cf}.json")),
            &readback,
        )?;
        let expected = after["counts"][cf].as_u64().unwrap_or(0);
        if expected > 0 {
            let rows = readback["decoded_records"]["rows"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0);
            if rows == 0 {
                return Err(DynamicJepaError::SourceOfTruthMissing {
                    cf: cf.to_string(),
                    key: Vec::new(),
                });
            }
        }
    }
    Ok(after)
}

fn collect_domain_evidence(
    ctx: &mut ResearchSmokeContext,
    domain: &str,
    verifier_output: &Value,
) -> DynamicJepaResult<()> {
    let evidence_dir = evidence_dir_from_output(verifier_output)?;
    let training_summary = read_json_file(&evidence_dir.join("decoded/training_summary.json"))?;
    let artifact_hashes =
        read_json_file(&evidence_dir.join("artifact_files_recomputed_hashes.json"))?;
    let train_config_path = path_value(
        &training_summary["summary"]["artifact_root"],
        "training_summary.summary.artifact_root",
    )?
    .join("config.json");
    let train_config = read_json_file(&train_config_path)?;
    let hyperparameters = training_hyperparameters_from_config(&train_config)?;
    let summary_json = read_json_file(&evidence_dir.join("summary.json"))?;
    let verification_record = read_json_file(&evidence_dir.join("verification_run_record.json"))?;
    write_json_file(
        &ctx.decoded_records_dir
            .join(format!("{domain}_verification_run_record.json")),
        &verification_record,
    )?;
    write_json_file(
        &ctx.decoded_records_dir
            .join(format!("{domain}_step_summary.json")),
        &summary_json,
    )?;
    ctx.training_rows.push(json!({
        "domain": domain,
        "verification_run_id": verifier_output["verification_run_id"],
        "db_path": verifier_output["db_path"],
        "evidence_dir": verifier_output["evidence_dir"],
        "artifact_root": verifier_output["artifact_root"],
        "split_counts": verifier_output["summary"]["split_counts"],
        "metrics": training_summary["summary"]["metrics"],
        "parameter_count": training_summary["summary"]["metrics"]["parameter_count_trainable"],
        "train_config_path": train_config_path,
        "hyperparameters": hyperparameters,
        "created_ids": training_summary["created_ids"],
    }));
    ctx.artifact_rows.push(json!({
        "domain": domain,
        "verification_run_id": verifier_output["verification_run_id"],
        "artifact_hashes": artifact_hashes,
    }));

    let mut edge_idx = 0usize;
    loop {
        let path = evidence_dir.join(format!("edge_{edge_idx:02}.json"));
        if !path.exists() {
            break;
        }
        let edge = read_json_file(&path)?;
        ctx.edge_rows.push(json!({
            "domain": domain,
            "edge_index": edge_idx,
            "evidence_file": path,
            "edge": edge,
        }));
        edge_idx += 1;
    }
    ctx.domain_reports.insert(
        domain.to_string(),
        json!({
            "status": "passed",
            "verification_run_id": verifier_output["verification_run_id"],
            "db_path": verifier_output["db_path"],
            "artifact_root": verifier_output["artifact_root"],
            "evidence_dir": verifier_output["evidence_dir"],
            "after_counts": verifier_output["after_counts"],
            "summary": verifier_output["summary"],
            "edge_case_count": edge_idx,
        }),
    );
    Ok(())
}

fn run_career_reproducibility(ctx: &mut ResearchSmokeContext) -> DynamicJepaResult<()> {
    let base = ctx
        .training_rows
        .iter()
        .find(|row| row["domain"] == "career_taxonomy")
        .cloned()
        .ok_or_else(|| DynamicJepaError::VerificationFailed {
            test_name: "mejepa_research_smoke_reproducibility".to_string(),
            message: "career base metrics missing before reproducibility run".to_string(),
            evidence_path: ctx.run_dir.display().to_string(),
        })?;
    let db = ctx.db_path("career_taxonomy_repro")?;
    let artifact_root = ctx.artifact_root.join("reproducibility");
    fs::create_dir_all(&artifact_root).map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.create_repro_artifact_root".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to create {}: {err}", artifact_root.display()),
        remediation: "use a writable artifact root".to_string(),
    })?;
    let repro_fixtures = ctx.career_fixtures_root.clone();
    let output = run_research_domain_verifier(
        ctx,
        "career_taxonomy_repro",
        "verify-career-taxonomy",
        &db,
        &artifact_root,
        &repro_fixtures,
    )?;
    let after = verify_domain_source_of_truth(ctx, "career_taxonomy_repro", &db, &output)?;
    let evidence_dir = evidence_dir_from_output(&output)?;
    let training_summary = read_json_file(&evidence_dir.join("decoded/training_summary.json"))?;
    let artifact_hashes =
        read_json_file(&evidence_dir.join("artifact_files_recomputed_hashes.json"))?;
    ctx.artifact_rows.push(json!({
        "domain": "career_taxonomy_repro",
        "verification_run_id": output["verification_run_id"],
        "artifact_hashes": artifact_hashes,
    }));
    let metrics = &training_summary["summary"]["metrics"];
    let mut metric_deltas = BTreeMap::new();
    for metric in [
        "val_latent_mse",
        "val_latent_cosine",
        "test_latent_mse",
        "test_latent_cosine",
        "val_action_contrast_acc",
        "test_action_contrast_acc",
        "train_action_contrast_acc",
        "random_init_cosine",
        "shuffled_target_cosine",
        "val_shuffled_target_cosine",
        "test_shuffled_target_cosine",
        "train_shuffled_target_cosine",
    ] {
        let lhs = base["metrics"][metric].as_f64().ok_or_else(|| {
            DynamicJepaError::VerificationFailed {
                test_name: "mejepa_research_smoke_reproducibility".to_string(),
                message: format!("base career metric {metric} missing"),
                evidence_path: ctx.run_dir.display().to_string(),
            }
        })?;
        let rhs = metrics[metric]
            .as_f64()
            .ok_or_else(|| DynamicJepaError::VerificationFailed {
                test_name: "mejepa_research_smoke_reproducibility".to_string(),
                message: format!("repro career metric {metric} missing"),
                evidence_path: ctx.run_dir.display().to_string(),
            })?;
        let delta = (lhs - rhs).abs();
        metric_deltas.insert(metric.to_string(), delta);
        if delta > 1e-6 {
            return Err(DynamicJepaError::VerificationFailed {
                test_name: "mejepa_research_smoke_reproducibility".to_string(),
                message: format!(
                    "same-seed career reproducibility drift exceeded 1e-6 for {metric}: base={lhs} repro={rhs} delta={delta}"
                ),
                evidence_path: ctx.run_dir.display().to_string(),
            });
        }
    }
    write_json_file(
        &ctx.run_dir.join("reproducibility.json"),
        &json!({
            "status": "passed",
            "base_verification_run_id": base["verification_run_id"],
            "repro_verification_run_id": output["verification_run_id"],
            "db_counts_after_repro": after["counts"],
            "metric_deltas": metric_deltas,
            "tolerance": 1e-6,
            "source_of_truth": {
                "base_evidence_dir": base["evidence_dir"],
                "repro_evidence_dir": output["evidence_dir"],
                "repro_db_path": db,
            },
        }),
    )?;
    ctx.domain_reports.insert(
        "career_taxonomy_repro".to_string(),
        json!({
            "status": "passed",
            "verification_run_id": output["verification_run_id"],
            "db_path": output["db_path"],
            "artifact_root": output["artifact_root"],
            "evidence_dir": output["evidence_dir"],
            "after_counts": output["after_counts"],
            "summary": output["summary"],
        }),
    );
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct CareerAblationSpec {
    ablation_id: &'static str,
    domain_label: &'static str,
    panel: &'static str,
    include_skill_vectors: bool,
    include_transition_distance: bool,
    include_pairwise_features: bool,
    action_conditioning: bool,
    vicreg: bool,
    stopping_target: f64,
    max_seconds: u64,
}

fn run_career_ablation_matrix(ctx: &mut ResearchSmokeContext) -> DynamicJepaResult<()> {
    let ablation_training_seed = career_ablation_training_seed(ctx)?;
    let base_spec = CareerAblationSpec {
        ablation_id: "A1_taxonomy_skill_action_vicreg",
        domain_label: "career_taxonomy",
        panel: "taxonomy_plus_skills",
        include_skill_vectors: true,
        include_transition_distance: false,
        include_pairwise_features: false,
        action_conditioning: true,
        vicreg: true,
        stopping_target: 0.20,
        max_seconds: 120,
    };
    let base_domain_pack = ctx
        .career_fixtures_root
        .join("domain_packs/career_taxonomy.v1.toml");
    let base_train_config = ctx
        .career_fixtures_root
        .join("verification/career_taxonomy_train_config.json");
    push_career_ablation_row_from_evidence(
        ctx,
        base_spec,
        &base_domain_pack,
        &base_train_config,
        false,
    )?;

    let specs = [
        CareerAblationSpec {
            ablation_id: "A0_taxonomy_only_action_vicreg",
            domain_label: "career_taxonomy_ablation_A0_taxonomy_only",
            panel: "taxonomy_scalar_categorical",
            include_skill_vectors: false,
            include_transition_distance: false,
            include_pairwise_features: false,
            action_conditioning: true,
            vicreg: true,
            stopping_target: 0.08,
            max_seconds: 180,
        },
        CareerAblationSpec {
            ablation_id: "A2_taxonomy_skill_transition_action_vicreg",
            domain_label: "career_taxonomy_ablation_A2_transition",
            panel: "taxonomy_plus_skills_plus_transition_distance",
            include_skill_vectors: true,
            include_transition_distance: true,
            include_pairwise_features: false,
            action_conditioning: true,
            vicreg: true,
            stopping_target: 0.08,
            max_seconds: 180,
        },
        CareerAblationSpec {
            ablation_id: "A3_taxonomy_skill_transition_no_action_vicreg",
            domain_label: "career_taxonomy_ablation_A3_no_action",
            panel: "taxonomy_plus_skills_plus_transition_distance",
            include_skill_vectors: true,
            include_transition_distance: true,
            include_pairwise_features: false,
            action_conditioning: false,
            vicreg: true,
            stopping_target: 0.50,
            max_seconds: 180,
        },
        CareerAblationSpec {
            ablation_id: "A4_taxonomy_skill_transition_pairwise_action_vicreg",
            domain_label: "career_taxonomy_ablation_A4_pairwise",
            panel: "taxonomy_plus_skills_plus_transition_distance_plus_pairwise",
            include_skill_vectors: true,
            include_transition_distance: true,
            include_pairwise_features: true,
            action_conditioning: true,
            vicreg: true,
            stopping_target: 0.08,
            max_seconds: 180,
        },
        CareerAblationSpec {
            ablation_id: "A5_taxonomy_skill_transition_action_no_vicreg",
            domain_label: "career_taxonomy_ablation_A5_no_vicreg",
            panel: "taxonomy_plus_skills_plus_transition_distance",
            include_skill_vectors: true,
            include_transition_distance: true,
            include_pairwise_features: false,
            action_conditioning: true,
            vicreg: false,
            stopping_target: 0.08,
            max_seconds: 180,
        },
    ];

    for spec in specs {
        let fixtures = materialize_career_ablation_fixtures(ctx, spec, ablation_training_seed)?;
        let db = ctx.db_path(spec.domain_label)?;
        let artifact_root = ctx.artifact_root.join("ablations").join(spec.ablation_id);
        fs::create_dir_all(&artifact_root).map_err(|err| DynamicJepaError::Storage {
            operation: "research_smoke.create_ablation_artifact_root".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to create {}: {err}", artifact_root.display()),
            remediation: "use a writable artifact root".to_string(),
        })?;
        let output = run_research_domain_verifier(
            ctx,
            spec.domain_label,
            "verify-career-taxonomy",
            &db,
            &artifact_root,
            &fixtures,
        );
        let output = output?;
        let after = verify_domain_source_of_truth(ctx, spec.domain_label, &db, &output)?;
        ctx.domain_reports.insert(
            spec.domain_label.to_string(),
            json!({
                "status": "passed",
                "verification_run_id": output["verification_run_id"],
                "db_path": output["db_path"],
                "artifact_root": output["artifact_root"],
                "evidence_dir": output["evidence_dir"],
                "after_counts": output["after_counts"],
                "summary": output["summary"],
            }),
        );
        push_career_ablation_row_from_output(
            ctx,
            spec,
            &output,
            &after["counts"],
            &fixtures.join("domain_packs/career_taxonomy.v1.toml"),
            &fixtures.join("verification/career_taxonomy_train_config.json"),
            true,
        )?;
    }
    Ok(())
}

fn career_ablation_training_seed(ctx: &ResearchSmokeContext) -> DynamicJepaResult<Option<u64>> {
    Ok(ctx.career_ablation_training_seed_effective)
}

fn materialize_career_ablation_fixtures(
    ctx: &ResearchSmokeContext,
    spec: CareerAblationSpec,
    ablation_training_seed: Option<u64>,
) -> DynamicJepaResult<PathBuf> {
    let fixtures = ctx.run_dir.join("ablations").join(spec.ablation_id);
    fs::create_dir_all(fixtures.join("domain_packs")).map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.create_ablation_fixtures".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to create ablation domain fixtures: {err}"),
        remediation: "use a writable --run-root".to_string(),
    })?;
    fs::create_dir_all(fixtures.join("verification")).map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.create_ablation_fixtures".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to create ablation verification fixtures: {err}"),
        remediation: "use a writable --run-root".to_string(),
    })?;

    let source_fixtures_root = &ctx.career_fixtures_root;
    mutate_career_domain_pack_for_ablation(
        &source_fixtures_root.join("domain_packs/career_taxonomy.v1.toml"),
        &fixtures.join("domain_packs/career_taxonomy.v1.toml"),
        spec,
    )?;
    write_career_ablation_train_config(
        &source_fixtures_root.join("verification/career_taxonomy_train_config.json"),
        &fixtures.join("verification/career_taxonomy_train_config.json"),
        spec,
        ablation_training_seed,
        &ctx.hp_overrides,
    )?;
    copy_fixture_file(
        &source_fixtures_root.join("verification/career_taxonomy_data_manifest.json"),
        &fixtures.join("verification/career_taxonomy_data_manifest.json"),
    )?;
    for rel in [
        "career_taxonomy_smoke.jsonl",
        "career_taxonomy_edge_empty.jsonl",
        "career_taxonomy_edge_missing_license.jsonl",
        "career_taxonomy_edge_invalid_action.jsonl",
        "career_taxonomy_edge_wrong_vector_dim.jsonl",
        "career_taxonomy_edge_oob_job_zone.jsonl",
    ] {
        transform_career_jsonl_for_ablation(
            &source_fixtures_root.join("verification").join(rel),
            &fixtures.join("verification").join(rel),
            spec,
            rel,
        )?;
    }
    Ok(fixtures)
}

fn mutate_career_domain_pack_for_ablation(
    src: &Path,
    dst: &Path,
    spec: CareerAblationSpec,
) -> DynamicJepaResult<()> {
    let text = read_text_file(src, "career ablation domain pack")?;
    let mut value: toml::Value = toml::from_str(&text).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.ablation_domain_pack.toml",
            format!("failed to parse {}: {err}", src.display()),
            "fix the checked-in career domain pack before generating ablation variants",
        )
    })?;

    if !spec.include_skill_vectors {
        remove_toml_schema_field(&mut value, "state", "skill_vector")?;
        remove_toml_schema_field(&mut value, "outcome", "next_skill_vector")?;
        remove_toml_instruments(&mut value, &["skill_vector", "next_skill_vector"])?;
        remove_toml_adapter_mappings(
            &mut value,
            &["state.skill_vector", "outcome.next_skill_vector"],
        )?;
        set_toml_objective_target(&mut value, "outcome.next_occupation_cluster")?;
    }
    if spec.include_transition_distance {
        push_toml_schema_field(
            &mut value,
            "state",
            toml_field_table(
                "transition_distance",
                toml_kind_table("f64", &[]),
                true,
                Some(0.0),
                Some(1.0),
            ),
        )?;
        push_toml_instrument(&mut value, transition_distance_instrument())?;
        insert_toml_adapter_mapping(
            &mut value,
            "state.transition_distance",
            "$.state.transition_distance",
        )?;
    }
    if spec.include_pairwise_features {
        push_toml_schema_field(
            &mut value,
            "state",
            toml_field_table(
                "occupation_move_pair",
                toml_kind_table("categorical", &occupation_move_pair_variants()),
                true,
                None,
                None,
            ),
        )?;
        push_toml_instrument(&mut value, occupation_move_pair_instrument())?;
        insert_toml_adapter_mapping(
            &mut value,
            "state.occupation_move_pair",
            "$.state.occupation_move_pair",
        )?;
    }

    let serialized = toml::to_string_pretty(&value).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.ablation_domain_pack.serialize",
            format!(
                "failed to serialize ablation domain pack {}: {err}",
                dst.display()
            ),
            "inspect the TOML mutation code for a non-serializable value",
        )
    })?;
    write_text_file(dst, &serialized)?;
    parse_domain_pack_file(dst)?;
    Ok(())
}

fn write_career_ablation_train_config(
    src: &Path,
    dst: &Path,
    spec: CareerAblationSpec,
    ablation_training_seed: Option<u64>,
    hp_overrides: &ResearchHyperparameterOverrides,
) -> DynamicJepaResult<()> {
    let mut config = read_json_file(src)?;
    if config.get("loss").and_then(Value::as_object).is_none() {
        return Err(DynamicJepaError::validation(
            "research_smoke.ablation_config.loss",
            format!("{} is missing required loss object", src.display()),
            "fix the checked-in training config before generating ablation configs",
        ));
    }
    if config
        .get("model")
        .and_then(|model| model.get("predictor"))
        .and_then(Value::as_object)
        .is_none()
    {
        return Err(DynamicJepaError::validation(
            "research_smoke.ablation_config.model.predictor",
            format!(
                "{} is missing required model.predictor object",
                src.display()
            ),
            "fix the checked-in training config before generating ablation configs",
        ));
    }
    config["model"]["predictor"]["ignore_action"] = json!(!spec.action_conditioning);
    config["stopping"]["target"] = json!(spec.stopping_target);
    config["stopping"]["max_seconds"] = json!(spec.max_seconds);
    if let Some(seed) = ablation_training_seed {
        config["seed"] = json!(seed);
    }
    if !spec.vicreg {
        config["loss"]["vicreg_variance_weight"] = json!(0.0);
        config["loss"]["vicreg_covariance_weight"] = json!(0.0);
    }
    apply_research_hp_overrides(&mut config, hp_overrides, "research_smoke.ablation_config")?;
    let parsed: TrainConfig = serde_json::from_value(config.clone()).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.ablation_config",
            format!("generated ablation config is invalid: {err}"),
            "fix ablation config mutation before running the verifier",
        )
    })?;
    parsed.validate()?;
    write_json_file(dst, &config)
}

fn transform_career_jsonl_for_ablation(
    src: &Path,
    dst: &Path,
    spec: CareerAblationSpec,
    fixture_name: &str,
) -> DynamicJepaResult<()> {
    let text = read_text_file(src, "career ablation JSONL")?;
    let mut out = String::new();
    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut value: Value = serde_json::from_str(line).map_err(|err| {
            DynamicJepaError::validation(
                "research_smoke.ablation_jsonl",
                format!("failed to parse {} line {}: {err}", src.display(), idx + 1),
                "checked-in career fixtures must be valid JSONL before ablation transforms run",
            )
        })?;
        transform_career_event_for_ablation(&mut value, spec, fixture_name)?;
        let line = serde_json::to_string(&value).map_err(|err| {
            DynamicJepaError::validation(
                "research_smoke.ablation_jsonl.serialize",
                format!(
                    "failed to serialize {} line {}: {err}",
                    dst.display(),
                    idx + 1
                ),
                "ablation fixture rows must remain JSON serializable",
            )
        })?;
        out.push_str(&line);
        out.push('\n');
    }
    write_text_file(dst, &out)
}

fn transform_career_event_for_ablation(
    value: &mut Value,
    spec: CareerAblationSpec,
    fixture_name: &str,
) -> DynamicJepaResult<()> {
    let root = value.as_object().ok_or_else(|| {
        DynamicJepaError::validation(
            "research_smoke.ablation_jsonl.row",
            "career fixture row must be a JSON object",
            "fix the checked-in career JSONL fixtures",
        )
    })?;
    let outcome_label = root
        .get("outcome")
        .and_then(|outcome| outcome.get("mobility_label"))
        .and_then(Value::as_str)
        .unwrap_or("same")
        .to_string();
    let action_move = root
        .get("action")
        .and_then(|action| action.get("move"))
        .and_then(Value::as_str)
        .unwrap_or("noop")
        .to_string();
    let occupation_cluster = root
        .get("state")
        .and_then(|state| state.get("occupation_cluster"))
        .and_then(Value::as_str)
        .unwrap_or("software")
        .to_string();
    let object = value.as_object_mut().ok_or_else(|| {
        DynamicJepaError::validation(
            "research_smoke.ablation_jsonl.row",
            "career fixture row must be a JSON object",
            "fix the checked-in career JSONL fixtures",
        )
    })?;
    let state = object
        .get_mut("state")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.ablation_jsonl.state",
                "career fixture row is missing object state",
                "fix the checked-in career JSONL fixtures",
            )
        })?;

    if spec.include_transition_distance {
        let distance = match outcome_label.as_str() {
            "same" => 0.0,
            "near_transition" => 0.5,
            "far_transition" => 1.0,
            other => {
                return Err(DynamicJepaError::validation(
                    "research_smoke.ablation_jsonl.mobility_label",
                    format!("unsupported mobility_label {other:?}"),
                    "career ablation fixtures must use declared mobility labels",
                ));
            }
        };
        state.insert("transition_distance".to_string(), json!(distance));
    }
    if spec.include_pairwise_features {
        let pair_action = if career_action_moves()
            .iter()
            .any(|move_id| move_id == &action_move)
        {
            action_move.as_str()
        } else {
            "noop"
        };
        let pair = format!("{occupation_cluster}__{pair_action}");
        if !occupation_move_pair_variants()
            .iter()
            .any(|item| item == &pair)
        {
            return Err(DynamicJepaError::validation(
                "research_smoke.ablation_jsonl.occupation_move_pair",
                format!("derived pair {pair:?} is not declared"),
                "update occupation/action variants together before running pairwise ablations",
            ));
        }
        state.insert("occupation_move_pair".to_string(), json!(pair));
    }
    if !spec.include_skill_vectors {
        if fixture_name == "career_taxonomy_edge_wrong_vector_dim.jsonl" {
            state.insert("skill_vector".to_string(), json!([0.2, 0.1, 0.1]));
        } else {
            state.remove("skill_vector");
            if let Some(outcome) = object.get_mut("outcome").and_then(Value::as_object_mut) {
                outcome.remove("next_skill_vector");
            }
        }
    }
    Ok(())
}

fn push_career_ablation_row_from_evidence(
    ctx: &mut ResearchSmokeContext,
    spec: CareerAblationSpec,
    domain_pack_path: &Path,
    train_config_path: &Path,
    record_artifact_row: bool,
) -> DynamicJepaResult<()> {
    let report = ctx
        .domain_reports
        .get(spec.domain_label)
        .cloned()
        .ok_or_else(|| DynamicJepaError::VerificationFailed {
            test_name: "mejepa_research_smoke_ablation".to_string(),
            message: format!(
                "domain report {} missing before ablation row build",
                spec.domain_label
            ),
            evidence_path: ctx.run_dir.display().to_string(),
        })?;
    push_career_ablation_row(
        ctx,
        CareerAblationRowInput {
            spec,
            verification_run_id: report["verification_run_id"].clone(),
            db_path: report["db_path"].clone(),
            artifact_root: report["artifact_root"].clone(),
            evidence_dir_value: report["evidence_dir"].clone(),
            after_counts: report["after_counts"].clone(),
            domain_pack_path,
            train_config_path,
            record_artifact_row,
        },
    )
}

fn push_career_ablation_row_from_output(
    ctx: &mut ResearchSmokeContext,
    spec: CareerAblationSpec,
    output: &Value,
    after_counts: &Value,
    domain_pack_path: &Path,
    train_config_path: &Path,
    record_artifact_row: bool,
) -> DynamicJepaResult<()> {
    push_career_ablation_row(
        ctx,
        CareerAblationRowInput {
            spec,
            verification_run_id: output["verification_run_id"].clone(),
            db_path: output["db_path"].clone(),
            artifact_root: output["artifact_root"].clone(),
            evidence_dir_value: output["evidence_dir"].clone(),
            after_counts: after_counts.clone(),
            domain_pack_path,
            train_config_path,
            record_artifact_row,
        },
    )
}

struct CareerAblationRowInput<'a> {
    spec: CareerAblationSpec,
    verification_run_id: Value,
    db_path: Value,
    artifact_root: Value,
    evidence_dir_value: Value,
    after_counts: Value,
    domain_pack_path: &'a Path,
    train_config_path: &'a Path,
    record_artifact_row: bool,
}

fn push_career_ablation_row(
    ctx: &mut ResearchSmokeContext,
    input: CareerAblationRowInput<'_>,
) -> DynamicJepaResult<()> {
    let evidence_dir = path_value(&input.evidence_dir_value, "ablation.evidence_dir")?;
    let training_summary = read_json_file(&evidence_dir.join("decoded/training_summary.json"))?;
    let dataset_train = read_json_file(&evidence_dir.join("decoded/dataset_train.json"))?;
    let artifact_hashes =
        read_json_file(&evidence_dir.join("artifact_files_recomputed_hashes.json"))?;
    let domain_pack = parse_domain_pack_file(input.domain_pack_path)?;
    let train_config = read_json_file(input.train_config_path)?;
    let hyperparameters = training_hyperparameters_from_config(&train_config)?;
    if input.record_artifact_row {
        ctx.artifact_rows.push(json!({
            "domain": input.spec.domain_label,
            "verification_run_id": input.verification_run_id.clone(),
            "artifact_hashes": artifact_hashes.clone(),
        }));
    }
    let metrics = training_summary["summary"]["metrics"].clone();
    ctx.ablation_rows.push(json!({
        "ablation_id": input.spec.ablation_id,
        "domain": "career_taxonomy",
        "domain_run_label": input.spec.domain_label,
        "panel": input.spec.panel,
        "panel_slots": domain_pack.instrument_specs.len(),
        "input_dim": dataset_train["summary"]["input_dim"],
        "target_dim": dataset_train["summary"]["target_dim"],
        "latent_dim": train_config["model"]["encoder"]["out_dim"],
        "action_dim": dataset_train["summary"]["action_dim"],
        "action_conditioning": input.spec.action_conditioning,
        "pairwise_features": input.spec.include_pairwise_features,
        "vicreg": input.spec.vicreg,
        "objective_configuration": {
            "vicreg_variance_weight": train_config["loss"]["vicreg_variance_weight"],
            "vicreg_covariance_weight": train_config["loss"]["vicreg_covariance_weight"],
            "ignore_action": train_config["model"]["predictor"]["ignore_action"],
            "stopping_target": train_config["stopping"]["target"],
            "max_seconds": train_config["stopping"]["max_seconds"],
        },
        "hyperparameters": hyperparameters,
        "parameter_count": metrics["parameter_count_trainable"],
        "metrics": metrics,
        "artifact_id": training_summary["created_ids"]["artifact_id"],
        "artifact_hashes": artifact_hashes,
        "source_verification_run_id": input.verification_run_id,
        "db_counts_after": input.after_counts,
        "source_of_truth": {
            "db_path": input.db_path,
            "evidence_dir": input.evidence_dir_value,
            "artifact_root": input.artifact_root,
            "domain_pack": input.domain_pack_path,
            "train_config": input.train_config_path,
            "dataset_train": evidence_dir.join("decoded/dataset_train.json"),
        },
    }));
    Ok(())
}

fn training_hyperparameters_from_config(config: &Value) -> DynamicJepaResult<Value> {
    let lr = config["optim"]["lr"].as_f64().ok_or_else(|| {
        DynamicJepaError::schema(
            "train_config.optim.lr",
            "training config is missing numeric optim.lr",
            "inspect the generated or artifact config.json; HP tables require the exact persisted training hyperparameters",
        )
    })?;
    let latent_dim = config["model"]["encoder"]["out_dim"].as_u64().ok_or_else(|| {
        DynamicJepaError::schema(
            "train_config.model.encoder.out_dim",
            "training config is missing numeric model.encoder.out_dim",
            "inspect the generated or artifact config.json; HP tables require the exact persisted training hyperparameters",
        )
    })?;
    let predictor_dim = config["model"]["predictor"]["out_dim"].as_u64().ok_or_else(|| {
        DynamicJepaError::schema(
            "train_config.model.predictor.out_dim",
            "training config is missing numeric model.predictor.out_dim",
            "inspect the generated or artifact config.json; HP tables require matching encoder/predictor dimensions",
        )
    })?;
    if predictor_dim != latent_dim {
        return Err(DynamicJepaError::schema(
            "train_config.model.predictor.out_dim",
            format!("predictor out_dim {predictor_dim} does not match encoder out_dim {latent_dim}"),
            "repair the generated train config mutation; HP sweeps require matched latent dimensions",
        ));
    }
    let batch_size = config["schedule"]["batch_size"].as_u64().ok_or_else(|| {
        DynamicJepaError::schema(
            "train_config.schedule.batch_size",
            "training config is missing numeric schedule.batch_size",
            "inspect the generated or artifact config.json; HP tables require the exact persisted training hyperparameters",
        )
    })?;
    let epochs = config["schedule"]["epochs"].as_u64().ok_or_else(|| {
        DynamicJepaError::schema(
            "train_config.schedule.epochs",
            "training config is missing numeric schedule.epochs",
            "inspect the generated or artifact config.json; HP tables require the exact persisted training hyperparameters",
        )
    })?;
    let seed = config["seed"].as_u64().ok_or_else(|| {
        DynamicJepaError::schema(
            "train_config.seed",
            "training config is missing numeric seed",
            "restore train config seed recording before creating research bundles",
        )
    })?;
    Ok(json!({
        "lr": lr,
        "latent_dim": latent_dim,
        "batch_size": batch_size,
        "epochs": epochs,
        "seed": seed,
    }))
}

fn path_value(value: &Value, field: &str) -> DynamicJepaResult<PathBuf> {
    value.as_str().map(PathBuf::from).ok_or_else(|| {
        DynamicJepaError::validation(
            field,
            format!("{field} must be a string path, got {value}"),
            "inspect the verifier output; source-of-truth paths must be serialized strings",
        )
    })
}

fn remove_toml_schema_field(
    value: &mut toml::Value,
    scope: &str,
    name: &str,
) -> DynamicJepaResult<()> {
    let fields = toml_schema_fields_mut(value, scope)?;
    let before = fields.len();
    fields.retain(|field| {
        field
            .get("name")
            .and_then(toml::Value::as_str)
            .map(|field_name| field_name != name)
            .unwrap_or(true)
    });
    if fields.len() == before {
        return Err(DynamicJepaError::validation(
            "research_smoke.ablation_domain_pack.schema",
            format!("schema field {scope}.{name} was not found"),
            "fix the checked-in career domain pack or the ablation mutation spec",
        ));
    }
    Ok(())
}

fn push_toml_schema_field(
    value: &mut toml::Value,
    scope: &str,
    field: toml::Value,
) -> DynamicJepaResult<()> {
    let name = field
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.ablation_domain_pack.schema",
                "generated schema field missing name",
                "fix the ablation TOML field builder",
            )
        })?
        .to_string();
    let fields = toml_schema_fields_mut(value, scope)?;
    if fields.iter().any(|existing| {
        existing
            .get("name")
            .and_then(toml::Value::as_str)
            .map(|existing_name| existing_name == name)
            .unwrap_or(false)
    }) {
        return Err(DynamicJepaError::validation(
            "research_smoke.ablation_domain_pack.schema",
            format!("schema field {scope}.{name} already exists"),
            "do not add duplicate ablation schema fields",
        ));
    }
    fields.push(field);
    Ok(())
}

fn toml_schema_fields_mut<'a>(
    value: &'a mut toml::Value,
    scope: &str,
) -> DynamicJepaResult<&'a mut Vec<toml::Value>> {
    value
        .get_mut(scope)
        .and_then(toml::Value::as_table_mut)
        .and_then(|table| table.get_mut("fields"))
        .and_then(toml::Value::as_array_mut)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.ablation_domain_pack.schema",
                format!("domain pack missing {scope}.fields array"),
                "fix the checked-in career domain pack before generating ablations",
            )
        })
}

fn remove_toml_instruments(value: &mut toml::Value, ids: &[&str]) -> DynamicJepaResult<()> {
    let instruments = value
        .get_mut("instruments")
        .and_then(toml::Value::as_array_mut)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.ablation_domain_pack.instruments",
                "domain pack missing instruments array",
                "fix the checked-in career domain pack before generating ablations",
            )
        })?;
    for id in ids {
        let before = instruments.len();
        instruments.retain(|instrument| {
            instrument
                .get("id")
                .and_then(toml::Value::as_str)
                .map(|instrument_id| instrument_id != *id)
                .unwrap_or(true)
        });
        if instruments.len() == before {
            return Err(DynamicJepaError::validation(
                "research_smoke.ablation_domain_pack.instruments",
                format!("instrument {id:?} was not found"),
                "fix the checked-in career domain pack or the ablation mutation spec",
            ));
        }
    }
    Ok(())
}

fn push_toml_instrument(value: &mut toml::Value, instrument: toml::Value) -> DynamicJepaResult<()> {
    let id = instrument
        .get("id")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.ablation_domain_pack.instruments",
                "generated instrument missing id",
                "fix the ablation TOML instrument builder",
            )
        })?
        .to_string();
    let instruments = value
        .get_mut("instruments")
        .and_then(toml::Value::as_array_mut)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.ablation_domain_pack.instruments",
                "domain pack missing instruments array",
                "fix the checked-in career domain pack before generating ablations",
            )
        })?;
    if instruments.iter().any(|existing| {
        existing
            .get("id")
            .and_then(toml::Value::as_str)
            .map(|existing_id| existing_id == id)
            .unwrap_or(false)
    }) {
        return Err(DynamicJepaError::validation(
            "research_smoke.ablation_domain_pack.instruments",
            format!("instrument {id:?} already exists"),
            "do not add duplicate ablation instruments",
        ));
    }
    instruments.push(instrument);
    Ok(())
}

fn remove_toml_adapter_mappings(value: &mut toml::Value, keys: &[&str]) -> DynamicJepaResult<()> {
    let mapping = first_toml_adapter_mapping_mut(value)?;
    for key in keys {
        if mapping.remove(*key).is_none() {
            return Err(DynamicJepaError::validation(
                "research_smoke.ablation_domain_pack.adapter_mapping",
                format!("adapter mapping {key:?} was not found"),
                "fix the checked-in career domain pack or the ablation mutation spec",
            ));
        }
    }
    Ok(())
}

fn insert_toml_adapter_mapping(
    value: &mut toml::Value,
    key: &str,
    source_path: &str,
) -> DynamicJepaResult<()> {
    let mapping = first_toml_adapter_mapping_mut(value)?;
    if mapping.contains_key(key) {
        return Err(DynamicJepaError::validation(
            "research_smoke.ablation_domain_pack.adapter_mapping",
            format!("adapter mapping {key:?} already exists"),
            "do not add duplicate ablation adapter mappings",
        ));
    }
    mapping.insert(
        key.to_string(),
        toml::Value::String(source_path.to_string()),
    );
    Ok(())
}

fn first_toml_adapter_mapping_mut(
    value: &mut toml::Value,
) -> DynamicJepaResult<&mut toml::map::Map<String, toml::Value>> {
    value
        .get_mut("adapters")
        .and_then(toml::Value::as_array_mut)
        .and_then(|adapters| adapters.first_mut())
        .and_then(toml::Value::as_table_mut)
        .and_then(|adapter| adapter.get_mut("mapping"))
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.ablation_domain_pack.adapter_mapping",
                "domain pack missing first adapter mapping table",
                "fix the checked-in career domain pack before generating ablations",
            )
        })
}

fn set_toml_objective_target(value: &mut toml::Value, target: &str) -> DynamicJepaResult<()> {
    let objective = value
        .get_mut("objectives")
        .and_then(toml::Value::as_array_mut)
        .and_then(|objectives| objectives.first_mut())
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "research_smoke.ablation_domain_pack.objectives",
                "domain pack missing first objective table",
                "fix the checked-in career domain pack before generating ablations",
            )
        })?;
    objective.insert(
        "target".to_string(),
        toml::Value::String(target.to_string()),
    );
    Ok(())
}

fn toml_field_table(
    name: &str,
    kind: toml::Value,
    required: bool,
    min: Option<f64>,
    max: Option<f64>,
) -> toml::Value {
    let mut table = toml::map::Map::new();
    table.insert("name".to_string(), toml::Value::String(name.to_string()));
    table.insert("kind".to_string(), kind);
    table.insert("required".to_string(), toml::Value::Boolean(required));
    if let Some(min) = min {
        table.insert("min".to_string(), toml::Value::Float(min));
    }
    if let Some(max) = max {
        table.insert("max".to_string(), toml::Value::Float(max));
    }
    toml::Value::Table(table)
}

fn toml_kind_table(kind_type: &str, variants: &[String]) -> toml::Value {
    let mut table = toml::map::Map::new();
    table.insert(
        "type".to_string(),
        toml::Value::String(kind_type.to_string()),
    );
    if !variants.is_empty() {
        table.insert("variants".to_string(), toml_string_array(variants));
    }
    toml::Value::Table(table)
}

fn transition_distance_instrument() -> toml::Value {
    let mut table = toml::map::Map::new();
    table.insert(
        "id".to_string(),
        toml::Value::String("transition_distance_scalar".to_string()),
    );
    table.insert("kind".to_string(), toml_kind_table("scalar", &[]));
    table.insert(
        "input_fields".to_string(),
        toml_string_array(&["state.transition_distance".to_string()]),
    );
    table.insert(
        "output_shape".to_string(),
        toml::Value::Array(vec![toml::Value::Integer(1)]),
    );
    table.insert(
        "normalization".to_string(),
        normalization_table("none", None, None),
    );
    table.insert("required".to_string(), toml::Value::Boolean(true));
    table.insert(
        "pair_kinds".to_string(),
        toml_string_array(&["cosine".to_string()]),
    );
    table.insert("version".to_string(), toml::Value::Integer(1));
    table.insert(
        "validation".to_string(),
        validation_table(Some(0.0), Some(1.0)),
    );
    toml::Value::Table(table)
}

fn occupation_move_pair_instrument() -> toml::Value {
    let variants = occupation_move_pair_variants();
    let mut table = toml::map::Map::new();
    table.insert(
        "id".to_string(),
        toml::Value::String("occupation_move_pair_onehot".to_string()),
    );
    table.insert(
        "kind".to_string(),
        toml_kind_table("categorical", &variants),
    );
    table.insert(
        "input_fields".to_string(),
        toml_string_array(&["state.occupation_move_pair".to_string()]),
    );
    table.insert(
        "output_shape".to_string(),
        toml::Value::Array(vec![toml::Value::Integer(variants.len() as i64)]),
    );
    table.insert(
        "normalization".to_string(),
        normalization_table("none", None, None),
    );
    table.insert("required".to_string(), toml::Value::Boolean(true));
    table.insert(
        "pair_kinds".to_string(),
        toml_string_array(&["cosine".to_string()]),
    );
    table.insert("version".to_string(), toml::Value::Integer(1));
    table.insert("validation".to_string(), validation_table(None, None));
    toml::Value::Table(table)
}

fn normalization_table(kind: &str, mean: Option<f64>, std: Option<f64>) -> toml::Value {
    let mut table = toml::map::Map::new();
    table.insert("type".to_string(), toml::Value::String(kind.to_string()));
    if let Some(mean) = mean {
        table.insert("mean".to_string(), toml::Value::Float(mean));
    }
    if let Some(std) = std {
        table.insert("std".to_string(), toml::Value::Float(std));
    }
    toml::Value::Table(table)
}

fn validation_table(min: Option<f64>, max: Option<f64>) -> toml::Value {
    let mut table = toml::map::Map::new();
    table.insert("require_finite".to_string(), toml::Value::Boolean(true));
    table.insert("reject_nan".to_string(), toml::Value::Boolean(true));
    table.insert("reject_inf".to_string(), toml::Value::Boolean(true));
    if let Some(min) = min {
        table.insert("min".to_string(), toml::Value::Float(min));
    }
    if let Some(max) = max {
        table.insert("max".to_string(), toml::Value::Float(max));
    }
    toml::Value::Table(table)
}

fn toml_string_array(values: &[String]) -> toml::Value {
    toml::Value::Array(
        values
            .iter()
            .map(|value| toml::Value::String(value.clone()))
            .collect(),
    )
}

fn occupation_move_pair_variants() -> Vec<String> {
    let occupations = ["software", "data", "systems", "management"];
    let moves = career_action_moves();
    occupations
        .iter()
        .flat_map(|occupation| {
            moves
                .iter()
                .map(move |move_id| format!("{occupation}__{move_id}"))
        })
        .collect()
}

fn career_action_moves() -> [&'static str; 5] {
    [
        "skill_gain_adjacent",
        "skill_gain_high_demand",
        "role_move_near",
        "experience_gain",
        "noop",
    ]
}

fn write_evaluation_report(ctx: &ResearchSmokeContext) -> DynamicJepaResult<()> {
    let rows = ctx
        .training_rows
        .iter()
        .map(|row| {
            json!({
                "domain": row["domain"],
                "verification_run_id": row["verification_run_id"],
                "split_counts": row["split_counts"],
                "metrics": row["metrics"],
                "planning": ctx.domain_reports
                    .get(row["domain"].as_str().unwrap_or(""))
                    .map(|report| json!({
                        "plan_trace_id": report["summary"]["plan_trace_id"],
                        "prediction_id": report["summary"]["prediction_id"],
                        "surprise_count": report["summary"]["surprise_count"],
                    }))
                    .unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    write_json_file(
        &ctx.run_dir.join("evaluation_report.json"),
        &json!({
            "status": "passed",
            "metric_rows": rows,
            "ablation_rows": ctx.ablation_rows,
            "edge_case_count": ctx.edge_rows.len(),
        }),
    )
}

fn write_fsv_report(
    ctx: &ResearchSmokeContext,
    status: &str,
    failure_details: Value,
) -> DynamicJepaResult<()> {
    write_json_file(
        &ctx.run_dir.join("fsv_report.json"),
        &json!({
            "status": status,
            "source_of_truth": {
                "bundle": ctx.run_dir,
                "rocksdb_root": ctx.db_root,
                "artifact_root": ctx.artifact_root,
                "evidence_root": ctx.fsv_evidence_root,
                "column_families": DJ_CF_NAMES,
            },
            "failure_details": failure_details,
            "readbacks": ctx.domain_reports,
            "edge_cases": ctx.edge_rows,
            "artifact_hash_checks": ctx.artifact_rows,
            "verification_run_ids": ctx.domain_reports
                .values()
                .map(|report| report["verification_run_id"].clone())
                .collect::<Vec<_>>(),
        }),
    )
}

fn write_paper_tables(ctx: &ResearchSmokeContext) -> DynamicJepaResult<()> {
    let mut runtime = "name,index,exit_code,elapsed_ms,stdout_sha256,stderr_sha256\n".to_string();
    for row in &ctx.command_rows {
        runtime.push_str(&format!(
            "{},{},{},{},{},{}\n",
            csv_escape(row["name"].as_str().unwrap_or("")),
            row["index"].as_u64().unwrap_or(0),
            row["exit_code"].as_i64().unwrap_or(-1),
            row["elapsed_ms"].as_u64().unwrap_or(0),
            csv_escape(row["stdout_sha256"].as_str().unwrap_or("")),
            csv_escape(row["stderr_sha256"].as_str().unwrap_or(""))
        ));
    }
    write_text_file(&ctx.paper_tables_dir.join("table_runtime.csv"), &runtime)?;

    let mut metrics =
        "domain,verification_run_id,train_rows,val_rows,test_rows,parameter_count,val_latent_mse,val_latent_cosine,test_latent_mse,test_latent_cosine,val_action_contrast_acc,random_init_cosine,shuffled_target_cosine\n"
            .to_string();
    for row in &ctx.training_rows {
        metrics.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(row["domain"].as_str().unwrap_or("")),
            csv_escape(row["verification_run_id"].as_str().unwrap_or("")),
            row["split_counts"]["train"].as_u64().unwrap_or(0),
            row["split_counts"]["val"].as_u64().unwrap_or(0),
            row["split_counts"]["test"].as_u64().unwrap_or(0),
            row["metrics"]["parameter_count_trainable"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["val_latent_mse"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["val_latent_cosine"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["test_latent_mse"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["test_latent_cosine"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["val_action_contrast_acc"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["random_init_cosine"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["shuffled_target_cosine"]
                .as_f64()
                .unwrap_or(f64::NAN),
        ));
    }
    write_text_file(
        &ctx.paper_tables_dir.join("table_prediction_metrics.csv"),
        &metrics,
    )?;

    let mut ablation =
        "ablation_id,domain_run_label,panel,panel_slots,input_dim,target_dim,latent_dim,action_dim,action_conditioning,pairwise_features,vicreg,parameter_count,verification_run_id,val_latent_mse,val_latent_cosine,test_latent_mse,test_latent_cosine,val_action_contrast_acc,random_init_cosine,shuffled_target_cosine\n"
            .to_string();
    for row in &ctx.ablation_rows {
        ablation.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(row["ablation_id"].as_str().unwrap_or("")),
            csv_escape(row["domain_run_label"].as_str().unwrap_or("")),
            csv_escape(row["panel"].as_str().unwrap_or("")),
            row["panel_slots"].as_u64().unwrap_or(0),
            row["input_dim"].as_u64().unwrap_or(0),
            row["target_dim"].as_u64().unwrap_or(0),
            row["latent_dim"].as_u64().unwrap_or(0),
            row["action_dim"].as_u64().unwrap_or(0),
            row["action_conditioning"].as_bool().unwrap_or(false),
            row["pairwise_features"].as_bool().unwrap_or(false),
            row["vicreg"].as_bool().unwrap_or(false),
            row["parameter_count"].as_f64().unwrap_or(f64::NAN),
            csv_escape(row["source_verification_run_id"].as_str().unwrap_or("")),
            row["metrics"]["val_latent_mse"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["val_latent_cosine"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["test_latent_mse"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["test_latent_cosine"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["val_action_contrast_acc"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["random_init_cosine"]
                .as_f64()
                .unwrap_or(f64::NAN),
            row["metrics"]["shuffled_target_cosine"]
                .as_f64()
                .unwrap_or(f64::NAN),
        ));
    }
    write_text_file(&ctx.paper_tables_dir.join("table_ablation.csv"), &ablation)?;

    write_collapse_diagnostics_table(ctx)?;

    let mut coverage = "domain,verification_run_id,events,panels,trajectories,dataset_shards,training_runs,artifacts,predictions,plan_traces,guard_decisions,surprises,verification_status\n".to_string();
    for (domain, report) in &ctx.domain_reports {
        coverage.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(domain),
            csv_escape(report["verification_run_id"].as_str().unwrap_or("")),
            report["after_counts"][CF_DJ_RAW_EVENTS]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_LATENT_PANELS]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_TRAJECTORIES]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_DATASET_SHARDS]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_TRAINING_RUNS]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_MODEL_ARTIFACTS]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_PREDICTIONS]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_PLAN_TRACES]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_GUARD_DECISIONS]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_SURPRISE_EVENTS]
                .as_u64()
                .unwrap_or(0),
            csv_escape(report["status"].as_str().unwrap_or("")),
        ));
    }
    write_text_file(
        &ctx.paper_tables_dir
            .join("table_domain_runtime_coverage.csv"),
        &coverage,
    )?;

    let mut feasibility =
        "run_label,command_count,total_elapsed_ms,artifact_hash_rows,edge_case_rows,ablation_rows,rocksdb_root,artifact_root,evidence_root\n"
            .to_string();
    let total_elapsed_ms = ctx
        .command_rows
        .iter()
        .map(|row| row["elapsed_ms"].as_u64().unwrap_or(0))
        .sum::<u64>();
    feasibility.push_str(&format!(
        "{},{},{},{},{},{},{},{},{}\n",
        csv_escape(&ctx.run_id),
        ctx.command_rows.len(),
        total_elapsed_ms,
        ctx.artifact_rows.len(),
        ctx.edge_rows.len(),
        ctx.ablation_rows.len(),
        csv_escape(&ctx.db_root.display().to_string()),
        csv_escape(&ctx.artifact_root.display().to_string()),
        csv_escape(&ctx.fsv_evidence_root.display().to_string()),
    ));
    write_text_file(
        &ctx.paper_tables_dir
            .join("table_single_gpu_feasibility.csv"),
        &feasibility,
    )?;

    let mut audit = "domain,verification_run_id,db_path,evidence_dir,dj_verification_runs,dj_model_artifacts,dj_predictions,dj_plan_traces,dj_guard_decisions\n".to_string();
    for (domain, report) in &ctx.domain_reports {
        audit.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            csv_escape(domain),
            csv_escape(report["verification_run_id"].as_str().unwrap_or("")),
            csv_escape(report["db_path"].as_str().unwrap_or("")),
            csv_escape(report["evidence_dir"].as_str().unwrap_or("")),
            report["after_counts"][CF_DJ_VERIFICATION_RUNS]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_MODEL_ARTIFACTS]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_PREDICTIONS]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_PLAN_TRACES]
                .as_u64()
                .unwrap_or(0),
            report["after_counts"][CF_DJ_GUARD_DECISIONS]
                .as_u64()
                .unwrap_or(0),
        ));
    }
    write_text_file(&ctx.paper_tables_dir.join("table_auditability.csv"), &audit)?;

    let mut edges = "domain,edge_index,name,raw_delta,adapter_delta,normalized_delta,transition_delta,audit_delta,evidence_file\n".to_string();
    for row in &ctx.edge_rows {
        let expected = &row["edge"]["expected_delta"];
        edges.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            csv_escape(row["domain"].as_str().unwrap_or("")),
            row["edge_index"].as_u64().unwrap_or(0),
            csv_escape(row["edge"]["name"].as_str().unwrap_or("")),
            expected[CF_DJ_RAW_EVENTS].as_i64().unwrap_or(0),
            expected[CF_DJ_ADAPTER_RUNS].as_i64().unwrap_or(0),
            expected[CF_DJ_NORMALIZED_STATES].as_i64().unwrap_or(0),
            expected[CF_DJ_TRANSITIONS].as_i64().unwrap_or(0),
            expected[CF_DJ_AUDIT_LOG].as_i64().unwrap_or(0),
            csv_escape(row["evidence_file"].as_str().unwrap_or("")),
        ));
    }
    write_text_file(&ctx.paper_tables_dir.join("table_edge_cases.csv"), &edges)?;

    let data = read_json_file(&ctx.run_dir.join("data_manifest.json"))?;
    let mut sources = "name,license,allowed_use,fixture_sha256,row_count\n".to_string();
    for source in data["sources"].as_array().cloned().unwrap_or_default() {
        sources.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_escape(source["name"].as_str().unwrap_or("")),
            csv_escape(source["license"].as_str().unwrap_or("")),
            csv_escape(source["allowed_use"].as_str().unwrap_or("")),
            csv_escape(data["local_fixture_sha256"].as_str().unwrap_or("")),
            source["row_count"].as_u64().unwrap_or(0),
        ));
    }
    write_text_file(
        &ctx.paper_tables_dir.join("table_data_sources.csv"),
        &sources,
    )?;
    Ok(())
}

fn write_collapse_diagnostics_table(ctx: &ResearchSmokeContext) -> DynamicJepaResult<()> {
    let mut csv = "run_id,row_kind,domain,ablation_id,verification_run_id,seed,lr,latent_dim,batch_size,val_var_min,val_var_p25,val_var_median,val_var_p75,val_var_max,covar_frob,eff_rank,eff_rank_ratio,alignment,uniformity,sigreg_kl,sigreg_kl_clamped,latent_collapsed,val_latent_cosine,val_latent_mse\n".to_string();
    let mut row_count = 0usize;
    for row in &ctx.training_rows {
        append_collapse_row(&mut csv, &ctx.run_id, "domain", row, "")?;
        row_count += 1;
    }
    for row in &ctx.ablation_rows {
        let ablation_id = row["ablation_id"].as_str().unwrap_or("");
        append_collapse_row(&mut csv, &ctx.run_id, "ablation", row, ablation_id)?;
        row_count += 1;
    }
    if row_count == 0 {
        return Err(DynamicJepaError::validation(
            "research_smoke.collapse_table",
            "cannot write collapse diagnostics table with zero rows",
            "ensure training and ablation evidence is collected before writing paper tables",
        ));
    }
    write_text_file(
        &ctx.paper_tables_dir.join("table_collapse_diagnostics.csv"),
        &csv,
    )
}

fn append_collapse_row(
    csv: &mut String,
    run_id: &str,
    row_kind: &str,
    row: &Value,
    ablation_id: &str,
) -> DynamicJepaResult<()> {
    let metrics = &row["metrics"];
    let hp = &row["hyperparameters"];
    let computed = required_metric_f64(metrics, "collapse_diagnostics_computed")?;
    if computed != 1.0 {
        return Err(DynamicJepaError::VerificationFailed {
            test_name: "mejepa_research_smoke_collapse_diagnostics".to_string(),
            message: format!(
                "collapse diagnostics were not computed for row_kind={row_kind} domain={} ablation_id={ablation_id}",
                row["domain"].as_str().unwrap_or("")
            ),
            evidence_path: row["evidence_dir"]
                .as_str()
                .unwrap_or("research_smoke_table")
                .to_string(),
        });
    }
    csv.push_str(&format!(
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
        csv_escape(run_id),
        csv_escape(row_kind),
        csv_escape(row["domain"].as_str().unwrap_or("")),
        csv_escape(ablation_id),
        csv_escape(row["verification_run_id"].as_str().unwrap_or("")),
        required_json_u64_cell(hp, "seed")?,
        required_json_f64_cell(hp, "lr")?,
        required_json_u64_cell(hp, "latent_dim")?,
        required_json_u64_cell(hp, "batch_size")?,
        required_metric_cell(metrics, "collapse_val_var_min")?,
        required_metric_cell(metrics, "collapse_val_var_p25")?,
        required_metric_cell(metrics, "collapse_val_var_median")?,
        required_metric_cell(metrics, "collapse_val_var_p75")?,
        required_metric_cell(metrics, "collapse_val_var_max")?,
        required_metric_cell(metrics, "collapse_covar_frob")?,
        required_metric_cell(metrics, "collapse_eff_rank")?,
        required_metric_cell(metrics, "collapse_eff_rank_ratio")?,
        required_metric_cell(metrics, "collapse_alignment")?,
        required_metric_cell(metrics, "collapse_uniformity")?,
        required_metric_cell(metrics, "collapse_sigreg_kl")?,
        required_metric_cell(metrics, "collapse_sigreg_kl_clamped")?,
        required_metric_cell(metrics, "collapse_latent_collapsed")?,
        required_metric_cell(metrics, "val_latent_cosine")?,
        required_metric_cell(metrics, "val_latent_mse")?,
    ));
    Ok(())
}

fn required_json_f64_cell(value: &Value, key: &str) -> DynamicJepaResult<String> {
    let raw = value.get(key).ok_or_else(|| {
        DynamicJepaError::schema(
            format!("hyperparameters.{key}"),
            format!("missing hyperparameter {key}"),
            "inspect generated config.json; paper tables require explicit HP provenance",
        )
    })?;
    value_to_f64_cell(raw, &format!("hyperparameters.{key}"))
}

fn required_json_u64_cell(value: &Value, key: &str) -> DynamicJepaResult<String> {
    let raw = value.get(key).ok_or_else(|| {
        DynamicJepaError::schema(
            format!("hyperparameters.{key}"),
            format!("missing hyperparameter {key}"),
            "inspect generated config.json; paper tables require explicit HP provenance",
        )
    })?;
    raw.as_u64().map(|item| item.to_string()).ok_or_else(|| {
        DynamicJepaError::schema(
            format!("hyperparameters.{key}"),
            format!("hyperparameter {key} is not an unsigned integer: {raw}"),
            "inspect generated config.json; paper tables require typed HP provenance",
        )
    })
}

fn required_metric_f64(metrics: &Value, key: &str) -> DynamicJepaResult<f64> {
    let raw = metrics.get(key).ok_or_else(|| {
        DynamicJepaError::schema(
            format!("metrics.{key}"),
            format!("missing training metric {key}"),
            "inspect training artifact metrics.json; collapse diagnostics must be emitted by training",
        )
    })?;
    raw.as_f64().ok_or_else(|| {
        DynamicJepaError::schema(
            format!("metrics.{key}"),
            format!("training metric {key} is not numeric: {raw}"),
            "inspect training artifact metrics.json; finite collapse flags must be serialized as numbers",
        )
    })
}

fn required_metric_cell(metrics: &Value, key: &str) -> DynamicJepaResult<String> {
    let raw = metrics.get(key).ok_or_else(|| {
        DynamicJepaError::schema(
            format!("metrics.{key}"),
            format!("missing training metric {key}"),
            "inspect training artifact metrics.json; collapse diagnostics must be emitted by training",
        )
    })?;
    value_to_f64_cell(raw, &format!("metrics.{key}"))
}

fn value_to_f64_cell(raw: &Value, field: &str) -> DynamicJepaResult<String> {
    if raw.is_null() {
        return Ok("nan".to_string());
    }
    let value = raw.as_f64().ok_or_else(|| {
        DynamicJepaError::schema(
            field,
            format!("{field} is not numeric or null: {raw}"),
            "inspect the source JSON; paper tables require numeric metrics or null for NaN diagnostics",
        )
    })?;
    Ok(if value.is_finite() {
        value.to_string()
    } else {
        "nan".to_string()
    })
}

fn write_plot_manifest(ctx: &ResearchSmokeContext) -> DynamicJepaResult<()> {
    let prediction_plot = ctx.plots_dir.join("prediction_val_mse.svg");
    let ablation_plot = ctx.plots_dir.join("career_ablation_val_mse.svg");
    write_metric_bar_svg(
        &prediction_plot,
        "Validation latent MSE by domain",
        ctx.training_rows
            .iter()
            .map(|row| {
                (
                    row["domain"].as_str().unwrap_or("").to_string(),
                    row["metrics"]["val_latent_mse"].as_f64().unwrap_or(0.0),
                )
            })
            .collect::<Vec<_>>(),
    )?;
    write_metric_bar_svg(
        &ablation_plot,
        "Career validation latent MSE ablation",
        ctx.ablation_rows
            .iter()
            .map(|row| {
                (
                    row["ablation_id"].as_str().unwrap_or("").to_string(),
                    row["metrics"]["val_latent_mse"].as_f64().unwrap_or(0.0),
                )
            })
            .collect::<Vec<_>>(),
    )?;
    write_json_file(
        &ctx.plots_dir.join("plot_manifest.json"),
        &json!({
            "status": "rendered",
            "plots": [
                {
                    "id": "prediction_val_mse",
                    "source_table": ctx.paper_tables_dir.join("table_prediction_metrics.csv"),
                    "output": prediction_plot
                },
                {
                    "id": "career_ablation_val_mse",
                    "source_table": ctx.paper_tables_dir.join("table_ablation.csv"),
                    "output": ablation_plot
                }
            ]
        }),
    )
}

fn write_metric_bar_svg(
    path: &Path,
    title: &str,
    rows: Vec<(String, f64)>,
) -> DynamicJepaResult<()> {
    if rows.is_empty() {
        return Err(DynamicJepaError::validation(
            "research_smoke.plot_rows",
            format!("cannot render {} with zero rows", path.display()),
            "ensure metric rows are generated before writing plots",
        ));
    }
    let width = 720.0;
    let height = 420.0;
    let left = 72.0;
    let right = 24.0;
    let top = 54.0;
    let bottom = 96.0;
    let plot_width = width - left - right;
    let plot_height = height - top - bottom;
    let max_value = rows
        .iter()
        .map(|(_, value)| *value)
        .fold(0.0f64, f64::max)
        .max(1e-9);
    let slot_width = plot_width / rows.len() as f64;
    let bar_width = (slot_width * 0.62).max(18.0);
    let mut bars = String::new();
    for (idx, (label, value)) in rows.iter().enumerate() {
        let bar_height = plot_height * (*value / max_value);
        let x = left + idx as f64 * slot_width + (slot_width - bar_width) / 2.0;
        let y = top + plot_height - bar_height;
        let label_x = left + idx as f64 * slot_width + slot_width / 2.0;
        let value_y = (y - 8.0).max(18.0);
        bars.push_str(&format!(
            r##"<rect x="{x:.2}" y="{y:.2}" width="{bar_width:.2}" height="{bar_height:.2}" fill="#2563eb"/>
<text x="{label_x:.2}" y="{value_y:.2}" text-anchor="middle" font-size="12" fill="#111827">{value:.4}</text>
<text x="{label_x:.2}" y="350" text-anchor="middle" font-size="12" fill="#111827" transform="rotate(-22 {label_x:.2} 350)">{}</text>
"##,
            xml_escape(label)
        ));
    }
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}" role="img" aria-label="{}">
<rect width="100%" height="100%" fill="#ffffff"/>
<text x="{left:.0}" y="30" font-size="18" font-family="Arial, sans-serif" font-weight="700" fill="#111827">{}</text>
<line x1="{left:.0}" y1="{top:.0}" x2="{left:.0}" y2="{:.0}" stroke="#374151" stroke-width="1"/>
<line x1="{left:.0}" y1="{:.0}" x2="{:.0}" y2="{:.0}" stroke="#374151" stroke-width="1"/>
<text x="18" y="{:.0}" font-size="12" font-family="Arial, sans-serif" fill="#111827" transform="rotate(-90 18 {:.0})">latent MSE</text>
<text x="18" y="{top:.0}" font-size="11" font-family="Arial, sans-serif" fill="#374151">{max_value:.4}</text>
{bars}</svg>
"##,
        xml_escape(title),
        xml_escape(title),
        top + plot_height,
        top + plot_height,
        width - right,
        top + plot_height,
        top + plot_height / 2.0,
        top + plot_height / 2.0
    );
    write_text_file(path, &svg)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn evidence_dir_from_output(output: &Value) -> DynamicJepaResult<PathBuf> {
    let raw =
        output["evidence_dir"]
            .as_str()
            .ok_or_else(|| DynamicJepaError::VerificationFailed {
                test_name: "mejepa_research_smoke_evidence".to_string(),
                message: "verifier output missing evidence_dir".to_string(),
                evidence_path: ".".to_string(),
            })?;
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        Ok(path)
    } else {
        env::current_dir().map(|cwd| cwd.join(path)).map_err(|err| {
            DynamicJepaError::validation(
                "research_smoke.cwd",
                format!("failed to resolve current directory for evidence path: {err}"),
                "run from a valid working directory",
            )
        })
    }
}

fn ensure_absent_or_empty_dir(path: &Path, field: &str) -> DynamicJepaResult<()> {
    if path.exists() {
        let mut entries = fs::read_dir(path).map_err(|err| DynamicJepaError::Storage {
            operation: "research_smoke.read_dir".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to read {}: {err}", path.display()),
            remediation: "choose a readable fresh path".to_string(),
        })?;
        if entries
            .next()
            .transpose()
            .map_err(|err| DynamicJepaError::Storage {
                operation: "research_smoke.read_dir_entry".to_string(),
                cf: "filesystem".to_string(),
                message: format!("failed to inspect {}: {err}", path.display()),
                remediation: "choose a readable fresh path".to_string(),
            })?
            .is_some()
        {
            return Err(DynamicJepaError::validation(
                field,
                format!("path exists and is not empty: {}", path.display()),
                "use a fresh empty path; research smoke does not merge evidence with prior runs",
            ));
        }
    }
    Ok(())
}

fn read_json_file(path: &Path) -> DynamicJepaResult<Value> {
    let bytes = fs::read(path).map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.read_json_file".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to read {}: {err}", path.display()),
        remediation: "inspect the referenced evidence path; required JSON evidence must exist"
            .to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.json_file",
            format!("failed to parse {}: {err}", path.display()),
            "fix the JSON evidence source before rerunning research smoke",
        )
    })
}

fn read_text_file(path: &Path, field: &str) -> DynamicJepaResult<String> {
    fs::read_to_string(path).map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.read_text_file".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to read {}: {err}", path.display()),
        remediation: format!("inspect {field}; required text evidence must exist"),
    })
}

fn write_text_file(path: &Path, text: &str) -> DynamicJepaResult<()> {
    fs::write(path, text).map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.write_text_file".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to write {}: {err}", path.display()),
        remediation: "use a writable --run-root".to_string(),
    })
}

fn append_jsonl(path: &Path, value: &Value) -> DynamicJepaResult<()> {
    let text = serde_json::to_string(value).map_err(|err| {
        DynamicJepaError::validation(
            "research_smoke.command_log_json",
            format!("failed to serialize command log row: {err}"),
            "command log rows must be JSON serializable",
        )
    })?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| DynamicJepaError::Storage {
            operation: "research_smoke.open_command_log".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to open {}: {err}", path.display()),
            remediation: "use a writable --run-root".to_string(),
        })?;
    writeln!(file, "{text}").map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.write_command_log".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to write {}: {err}", path.display()),
        remediation: "use a writable --run-root".to_string(),
    })
}

fn tail_lines(text: &str, limit: usize) -> Vec<String> {
    let lines = text.lines().map(str::to_string).collect::<Vec<_>>();
    let start = lines.len().saturating_sub(limit);
    lines[start..].to_vec()
}

fn safe_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn copy_fixture_file(src: &Path, dst: &Path) -> DynamicJepaResult<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|err| DynamicJepaError::Storage {
            operation: "research_smoke.copy_fixture_parent".to_string(),
            cf: "filesystem".to_string(),
            message: format!("failed to create {}: {err}", parent.display()),
            remediation: "use a writable --run-root".to_string(),
        })?;
    }
    fs::copy(src, dst).map_err(|err| DynamicJepaError::Storage {
        operation: "research_smoke.copy_fixture".to_string(),
        cf: "filesystem".to_string(),
        message: format!(
            "failed to copy {} to {}: {err}",
            src.display(),
            dst.display()
        ),
        remediation: "restore checked-in fixtures and use a writable --run-root".to_string(),
    })?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum VerificationDomain {
    CounterWorld,
    Gridworld,
    CareerTaxonomy,
}

impl VerificationDomain {
    fn id(self) -> &'static str {
        match self {
            Self::CounterWorld => "counter_world",
            Self::Gridworld => "gridworld_5x5",
            Self::CareerTaxonomy => "career_taxonomy",
        }
    }

    fn operation(self) -> &'static str {
        match self {
            Self::CounterWorld => "dynamicjepa_verify_counter_world",
            Self::Gridworld => "dynamicjepa_verify_gridworld",
            Self::CareerTaxonomy => "dynamicjepa_verify_career_taxonomy",
        }
    }

    fn test_name(self) -> &'static str {
        match self {
            Self::CounterWorld => "phase10_verify_counter_world",
            Self::Gridworld => "phase10_verify_gridworld",
            Self::CareerTaxonomy => "mejepa_research_smoke_career_taxonomy",
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
            Self::CareerTaxonomy => {
                fixtures_root.join("verification/career_taxonomy_train_config.json")
            }
        }
    }

    fn corpus_path(self, fixtures_root: &Path, evidence_dir: &Path) -> DynamicJepaResult<PathBuf> {
        match self {
            Self::CounterWorld => write_generated_counter_verification_fixture(evidence_dir),
            Self::Gridworld => Ok(fixtures_root.join("verification/gridworld_5x5_corpus.jsonl")),
            Self::CareerTaxonomy => {
                Ok(fixtures_root.join("verification/career_taxonomy_smoke.jsonl"))
            }
        }
    }

    fn data_manifest_path(self, fixtures_root: &Path) -> Option<PathBuf> {
        match self {
            Self::CareerTaxonomy => {
                Some(fixtures_root.join("verification/career_taxonomy_data_manifest.json"))
            }
            _ => None,
        }
    }

    fn edge_fixtures(self, fixtures_root: &Path) -> Vec<(&'static str, PathBuf, EdgeExpectation)> {
        match self {
            Self::CounterWorld => vec![
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
            Self::Gridworld => vec![
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
            Self::CareerTaxonomy => vec![
                (
                    "empty_input",
                    fixtures_root.join("verification/career_taxonomy_edge_empty.jsonl"),
                    EdgeExpectation::empty_input(),
                ),
                (
                    "missing_source_license",
                    fixtures_root.join("verification/career_taxonomy_edge_missing_license.jsonl"),
                    EdgeExpectation::failed_adapter(),
                ),
                (
                    "invalid_action",
                    fixtures_root.join("verification/career_taxonomy_edge_invalid_action.jsonl"),
                    EdgeExpectation::failed_adapter(),
                ),
                (
                    "wrong_skill_vector_dim",
                    fixtures_root.join("verification/career_taxonomy_edge_wrong_vector_dim.jsonl"),
                    EdgeExpectation::failed_adapter(),
                ),
                (
                    "oob_job_zone",
                    fixtures_root.join("verification/career_taxonomy_edge_oob_job_zone.jsonl"),
                    EdgeExpectation::failed_adapter(),
                ),
            ],
        }
    }

    fn expected_happy_counts(
        self,
        domain_pack: &DomainPack,
        edge_fixtures: &[(&'static str, PathBuf, EdgeExpectation)],
        surprise_count: u64,
    ) -> DynamicJepaResult<BTreeMap<String, u64>> {
        let event_count = required_verification_count(
            domain_pack.verification_policy.expected_event_count,
            "expected_event_count",
        )?;
        let panel_count = required_verification_count(
            domain_pack.verification_policy.expected_panel_count,
            "expected_panel_count",
        )?;
        let trajectory_count = required_verification_count(
            domain_pack.verification_policy.expected_trajectory_count,
            "expected_trajectory_count",
        )?;
        let candidate_count = match &domain_pack.planner_policy.candidate_actions {
            CandidateActionConfig::EnumeratedDeltas { deltas } => deltas.len() as u64,
            CandidateActionConfig::EnumeratedMoves { moves } => moves.len() as u64,
        };
        let instrument_count = domain_pack.instrument_specs.len() as u64;
        let pairwise_per_panel = instrument_count.saturating_mul(instrument_count.saturating_sub(1)) / 2;
        let edge_raw_delta = sum_edge_delta(edge_fixtures, |edge| edge.raw_delta)?;
        let edge_adapter_delta = sum_edge_delta(edge_fixtures, |edge| edge.adapter_delta)?;
        let counts = BTreeMap::from([
            (CF_DJ_DOMAIN_PACKS.to_string(), 1),
            (CF_DJ_DOMAIN_PACK_BY_NAME_VERSION.to_string(), 1),
            (CF_DJ_ADAPTER_REGISTRY.to_string(), 1),
            (CF_DJ_DATASET_SHARDS.to_string(), 3),
            (CF_DJ_MODEL_ARTIFACTS.to_string(), 1),
            (CF_DJ_PLAN_TRACES.to_string(), 1),
            (CF_DJ_PREDICTIONS.to_string(), candidate_count + 1),
            (CF_DJ_SKILL_POLICIES.to_string(), 1),
            (CF_DJ_GUARD_DECISIONS.to_string(), candidate_count),
            (CF_DJ_SURPRISE_EVENTS.to_string(), surprise_count),
            (CF_DJ_TRAINING_RUNS.to_string(), 1),
            (CF_DJ_VERIFICATION_RUNS.to_string(), 1),
            (
                CF_DJ_INSTRUMENT_REGISTRY.to_string(),
                domain_pack.instrument_specs.len() as u64,
            ),
            (CF_DJ_RAW_EVENTS.to_string(), event_count + edge_raw_delta),
            (
                CF_DJ_ADAPTER_RUNS.to_string(),
                event_count + edge_adapter_delta,
            ),
            (CF_DJ_NORMALIZED_STATES.to_string(), panel_count),
            (CF_DJ_ACTIONS.to_string(), event_count + candidate_count),
            (CF_DJ_OUTCOMES.to_string(), panel_count),
            (CF_DJ_TRANSITIONS.to_string(), panel_count),
            (
                CF_DJ_INSTRUMENT_READINGS.to_string(),
                panel_count * instrument_count,
            ),
            (
                CF_DJ_PAIRWISE_READINGS.to_string(),
                panel_count * pairwise_per_panel,
            ),
            (CF_DJ_LATENT_PANELS.to_string(), panel_count),
            (CF_DJ_TRAJECTORIES.to_string(), trajectory_count),
        ]);
        Ok(counts)
    }
}

fn required_verification_count(value: Option<u64>, field: &str) -> DynamicJepaResult<u64> {
    value.ok_or_else(|| {
        DynamicJepaError::validation(
            format!("domain_pack.verification_policy.{field}"),
            format!("{field} is required by bundled verification"),
            "declare exact expected counts in the domain pack verification policy",
        )
    })
}

fn sum_edge_delta(
    edge_fixtures: &[(&'static str, PathBuf, EdgeExpectation)],
    extract: impl Fn(EdgeExpectation) -> i64,
) -> DynamicJepaResult<u64> {
    edge_fixtures.iter().try_fold(0u64, |acc, (_, _, edge)| {
        let delta = extract(*edge);
        if delta < 0 {
            return Err(DynamicJepaError::validation(
                "verification.edge_delta",
                format!("edge delta cannot be negative: {delta}"),
                "edge expectations must only add failed/audit evidence rows",
            ));
        }
        Ok(acc + delta as u64)
    })
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
    let parsed_domain_pack = parse_domain_pack_file(&domain_path)?;
    let corpus_path = ctx
        .domain
        .corpus_path(&ctx.fixtures_root, &ctx.evidence_dir)?;
    let train_config = ctx.domain.train_config_path(&ctx.fixtures_root);
    ctx.add_fixture_hash(&domain_path)?;
    ctx.add_fixture_hash(&corpus_path)?;
    ctx.add_fixture_hash(&train_config)?;
    if let Some(manifest_path) = ctx.domain.data_manifest_path(&ctx.fixtures_root) {
        ctx.add_fixture_hash(&manifest_path)?;
        let manifest_json: Value =
            serde_json::from_slice(&fs::read(&manifest_path).map_err(|err| {
                DynamicJepaError::validation(
                    "verify.data_manifest",
                    format!(
                        "failed to read data manifest {}: {err}",
                        manifest_path.display()
                    ),
                    "provide a checked-in data manifest with source URL and license fields",
                )
            })?)
            .map_err(|err| {
                DynamicJepaError::validation(
                    "verify.data_manifest",
                    format!(
                        "failed to parse data manifest {}: {err}",
                        manifest_path.display()
                    ),
                    "fix the data manifest JSON before running research smoke",
                )
            })?;
        for field in [
            "name",
            "source_urls",
            "license",
            "local_fixture",
            "allowed_use",
        ] {
            if manifest_json.get(field).is_none() {
                return Err(ctx.fail(format!(
                    "data manifest {} is missing required field {field}",
                    manifest_path.display()
                )));
            }
        }
        ctx.decoded
            .insert("data_manifest".to_string(), manifest_json);
    }
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
            allow_missing: true,
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
    let expected_panel_count = required_verification_count(
        parsed_domain_pack.verification_policy.expected_panel_count,
        "expected_panel_count",
    )?;
    assert_json_u64(
        ctx,
        &panels["summary"]["materialized"],
        expected_panel_count,
        "materialized panel count",
    )?;
    let fsv9_pairwise = verify_pairwise_integrity_for_domain(ctx, &parsed_domain_pack)?;
    ctx.decoded.insert(
        "fsv9_pairwise_readings_integrity".to_string(),
        fsv9_pairwise,
    );

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
    let expected_trajectory_count = required_verification_count(
        parsed_domain_pack
            .verification_policy
            .expected_trajectory_count,
        "expected_trajectory_count",
    )?;
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
    let expected_splits = expected_split_counts(&parsed_domain_pack.verification_policy)?;
    assert_split_counts(ctx, &split_counts, &expected_splits)?;

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
    assert_training_metrics(ctx, &train, &train_config)?;
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
            allow_missing: false,
            json: true,
        }),
        ctx,
    )?;
    let edge_fixtures = ctx.domain.edge_fixtures(&ctx.fixtures_root);
    let mut expected_counts =
        ctx.domain
            .expected_happy_counts(&parsed_domain_pack, &edge_fixtures, surprise_count)?;
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
    ctx.expected
        .insert("split_counts".to_string(), json!(expected_splits));
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
            allow_missing: false,
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
            allow_missing: false,
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

fn verify_pairwise_integrity_for_domain(
    ctx: &VerificationContext,
    domain_pack: &DomainPack,
) -> DynamicJepaResult<Value> {
    let started_at_unix_ms = now_unix_ms()?;
    let store = open_existing_store(&ctx.db, "verify_pairwise_integrity")?;
    let db = store.dynamicjepa_db();
    let pre_state = snapshot_dj_counts(db)?;
    let panels = list_latent_panels(db, 1_000_000, 0)?
        .into_iter()
        .filter(|panel| panel.header.domain_pack_id == domain_pack.id)
        .collect::<Vec<_>>();
    let instrument_specs = domain_pack
        .instrument_specs
        .iter()
        .map(|spec| (spec.instrument_id.as_str(), spec))
        .collect::<BTreeMap<_, _>>();
    let expected_per_event = domain_pack
        .instrument_specs
        .len()
        .saturating_mul(domain_pack.instrument_specs.len().saturating_sub(1))
        / 2;
    let mut pair_count_passed = 0u64;
    let mut cosine_passed = 0u64;
    let mut hash_passed = 0u64;
    let mut bitset_passed = 0u64;
    let mut sample_rows = Vec::new();
    for panel in &panels {
        let pairwise_rows = list_pairwise_readings_for_event(db, panel.event_id)?;
        if pairwise_rows.len() != expected_per_event {
            return Err(DynamicJepaError::PairwiseRowCountMismatch {
                event_id: panel.event_id.as_uuid(),
                expected: expected_per_event as u64,
                actual: pairwise_rows.len() as u64,
            });
        }
        pair_count_passed += 1;
        let instrument_rows = list_instrument_readings_for_event(db, panel.event_id)?;
        let readings_by_id = instrument_rows
            .iter()
            .filter(|reading| matches!(reading.status, ReadingStatus::Ok))
            .map(|reading| (reading.instrument_id.as_str(), reading))
            .collect::<BTreeMap<_, _>>();
        for pairwise in pairwise_rows {
            let left = readings_by_id
                .get(pairwise.instrument_j.as_str())
                .copied()
                .ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_INSTRUMENT_READINGS.to_string(),
                    key: instrument_reading_key(panel.event_id, &pairwise.instrument_j).to_vec(),
                })?;
            let right = readings_by_id
                .get(pairwise.instrument_k.as_str())
                .copied()
                .ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_INSTRUMENT_READINGS.to_string(),
                    key: instrument_reading_key(panel.event_id, &pairwise.instrument_k).to_vec(),
                })?;
            let recomputed = cosine_similarity_padded_eps(
                &left.output_dense,
                &right.output_dense,
                "FSV-9.cosine_recompute",
            )?;
            if (recomputed - pairwise.cosine_agreement).abs() > 1.0e-4 {
                return Err(DynamicJepaError::PairwiseCosineDrift {
                    pairwise_id: pairwise.pairwise_id.as_uuid(),
                    expected: recomputed,
                    actual: pairwise.cosine_agreement,
                });
            }
            cosine_passed += 1;
            let left_spec = instrument_specs
                .get(pairwise.instrument_j.as_str())
                .copied()
                .ok_or_else(|| DynamicJepaError::PairwiseInstrumentHashDrift {
                    pairwise_id: pairwise.pairwise_id.as_uuid(),
                    instrument_id: pairwise.instrument_j.to_string(),
                })?;
            let right_spec = instrument_specs
                .get(pairwise.instrument_k.as_str())
                .copied()
                .ok_or_else(|| DynamicJepaError::PairwiseInstrumentHashDrift {
                    pairwise_id: pairwise.pairwise_id.as_uuid(),
                    instrument_id: pairwise.instrument_k.to_string(),
                })?;
            if pairwise.instrument_j_artifact_hash != instrument_artifact_hash32(left_spec)? {
                return Err(DynamicJepaError::PairwiseInstrumentHashDrift {
                    pairwise_id: pairwise.pairwise_id.as_uuid(),
                    instrument_id: pairwise.instrument_j.to_string(),
                });
            }
            if pairwise.instrument_k_artifact_hash != instrument_artifact_hash32(right_spec)? {
                return Err(DynamicJepaError::PairwiseInstrumentHashDrift {
                    pairwise_id: pairwise.pairwise_id.as_uuid(),
                    instrument_id: pairwise.instrument_k.to_string(),
                });
            }
            hash_passed += 1;
            verify_pairwise_bitset(left_spec, right_spec, &pairwise)?;
            bitset_passed += 1;
            if sample_rows.len() < 5 {
                sample_rows.push(json!({
                    "pairwise_id": pairwise.pairwise_id.to_string(),
                    "event_id": pairwise.event_id.to_string(),
                    "instrument_j": pairwise.instrument_j.to_string(),
                    "instrument_k": pairwise.instrument_k.to_string(),
                    "cosine_agreement": pairwise.cosine_agreement,
                    "kinds_emitted": pairwise.kinds_emitted.iter_kinds().collect::<Vec<_>>(),
                }));
            }
        }
    }
    let post_state = snapshot_dj_counts(db)?;
    let ended_at_unix_ms = now_unix_ms()?;
    let evidence = json!({
        "fsv_id": "FSV-9",
        "fsv_check": "pairwise_readings_integrity",
        "schema_version": 1,
        "bundle_schema_version": DYNAMICJEPA_BUNDLE_SCHEMA_VERSION,
        "started_at_unix_ms": started_at_unix_ms,
        "ended_at_unix_ms": ended_at_unix_ms,
        "domain": domain_pack.id.to_string(),
        "pre_state": pre_state,
        "post_state": post_state,
        "expected_pairwise_rows_per_event": expected_per_event,
        "checks": [
            {"name": "pair_count_per_event", "passed": pair_count_passed, "failed": 0},
            {"name": "cosine_recompute_within_tolerance", "passed": cosine_passed, "failed": 0},
            {"name": "instrument_hash_match", "passed": hash_passed, "failed": 0},
            {"name": "kinds_emitted_bitset_consistent", "passed": bitset_passed, "failed": 0}
        ],
        "sample_rows": sample_rows,
        "status": "passed",
    });
    write_json_file(
        &ctx.evidence_dir
            .join("fsv9_pairwise_readings_integrity.json"),
        &evidence,
    )?;
    Ok(evidence)
}

fn verify_pairwise_bitset(
    left: &InstrumentSpec,
    right: &InstrumentSpec,
    pairwise: &PairwiseReading,
) -> DynamicJepaResult<()> {
    pairwise.kinds_emitted.validate()?;
    let rank_expected = both_declare_pair_kind(left, right, PairKindName::RankDisagreement);
    if pairwise
        .kinds_emitted
        .has(PairKindBitset::RANK_DISAGREEMENT)
        != rank_expected
        || pairwise.rank_disagreement.is_some() != rank_expected
    {
        return Err(DynamicJepaError::validation(
            "PairwiseReading.rank_disagreement",
            format!(
                "rank bit/value mismatch for pairwise row {}",
                pairwise.pairwise_id
            ),
            "keep PairKindBitset and optional pair-kind fields consistent with the domain pack declaration",
        ));
    }
    for (bit, present, field) in [
        (
            PairKindBitset::MODALITY_CONTRADICTION,
            pairwise.modality_contradiction.is_some(),
            "modality_contradiction",
        ),
        (
            PairKindBitset::SPARSE_DENSE_MISMATCH,
            pairwise.sparse_dense_mismatch.is_some(),
            "sparse_dense_mismatch",
        ),
        (
            PairKindBitset::TEMPORAL_SURPRISE,
            pairwise.temporal_surprise.is_some(),
            "temporal_surprise",
        ),
        (
            PairKindBitset::CAUSAL_DIRECTION,
            pairwise.causal_direction_disagreement.is_some(),
            "causal_direction_disagreement",
        ),
        (
            PairKindBitset::SAFETY_PROXIMITY,
            pairwise.safety_proximity.is_some(),
            "safety_proximity",
        ),
    ] {
        if pairwise.kinds_emitted.has(bit) || present {
            return Err(DynamicJepaError::PairwiseUnsupportedKind {
                instrument_j: pairwise.instrument_j.to_string(),
                instrument_k: pairwise.instrument_k.to_string(),
                kind: field.to_string(),
                message: "unsupported optional pair-kind was persisted".to_string(),
            });
        }
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
    expected: &BTreeMap<String, u64>,
) -> DynamicJepaResult<()> {
    if split_counts != expected {
        return Err(ctx.fail(format!(
            "split counts mismatch expected={expected:?} actual={split_counts:?}"
        )));
    }
    Ok(())
}

fn expected_split_counts(policy: &VerificationPolicy) -> DynamicJepaResult<BTreeMap<String, u64>> {
    let train = exact_policy_count(
        policy.expected_train_rows_min,
        policy.expected_train_rows_max,
        "expected_train_rows",
    )?;
    let val = exact_policy_count(
        policy.expected_val_rows_min,
        policy.expected_val_rows_max,
        "expected_val_rows",
    )?;
    let test = exact_policy_count(
        policy.expected_test_rows_min,
        policy.expected_test_rows_max,
        "expected_test_rows",
    )?;
    let total = train + val + test;
    if let Some(expected_total) = policy.expected_dataset_row_count {
        if expected_total != total {
            return Err(DynamicJepaError::validation(
                "domain_pack.verification_policy.expected_dataset_row_count",
                format!(
                    "expected_dataset_row_count={expected_total} but exact split total is {total}"
                ),
                "make dataset and split count declarations internally consistent",
            ));
        }
    }
    Ok(BTreeMap::from([
        ("train".to_string(), train),
        ("val".to_string(), val),
        ("test".to_string(), test),
    ]))
}

fn exact_policy_count(min: Option<u64>, max: Option<u64>, field: &str) -> DynamicJepaResult<u64> {
    let min = min.ok_or_else(|| {
        DynamicJepaError::validation(
            format!("domain_pack.verification_policy.{field}_min"),
            format!("{field}_min is required by bundled verification"),
            "declare exact split counts in the domain pack verification policy",
        )
    })?;
    let max = max.ok_or_else(|| {
        DynamicJepaError::validation(
            format!("domain_pack.verification_policy.{field}_max"),
            format!("{field}_max is required by bundled verification"),
            "declare exact split counts in the domain pack verification policy",
        )
    })?;
    if min != max {
        return Err(DynamicJepaError::validation(
            format!("domain_pack.verification_policy.{field}"),
            format!("bundled verification requires exact counts but saw min={min} max={max}"),
            "set min and max to the deterministic expected row count for this fixture",
        ));
    }
    Ok(min)
}

fn assert_training_metrics(
    ctx: &VerificationContext,
    train: &Value,
    train_config_path: &Path,
) -> DynamicJepaResult<()> {
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
        VerificationDomain::CareerTaxonomy => {
            let train_config = read_json_file(train_config_path)?;
            let configured_target = train_config["stopping"]["target"].as_f64().ok_or_else(|| {
                DynamicJepaError::validation(
                    "verify.train_config.stopping.target",
                    format!(
                        "training config {} is missing numeric stopping.target",
                        train_config_path.display()
                    ),
                    "career verification requires an explicit target for the primary run and every ablation",
                )
            })?;
            let max_val_mse = configured_target.max(0.25);
            let min_val_cosine = 0.70;
            if val_mse > max_val_mse || val_cosine < min_val_cosine {
                return Err(ctx.fail(format!(
                    "career_taxonomy metrics outside smoke gate: val_latent_mse={val_mse} max_val_latent_mse={max_val_mse} val_latent_cosine={val_cosine} min_val_latent_cosine={min_val_cosine}"
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
    pairwise_reading_ids: Vec<String>,
}

impl PanelCreatedIds {
    fn push(&mut self, evidence: MaterializedPanelEvidence) {
        self.panel_ids.push(evidence.panel_id);
        self.event_ids.push(evidence.event_id);
        self.transition_ids.push(evidence.transition_id);
        self.instrument_reading_ids
            .extend(evidence.instrument_reading_ids);
        self.pairwise_reading_ids
            .extend(evidence.pairwise_reading_ids);
    }
}

struct MaterializedPanelEvidence {
    panel_id: String,
    event_id: String,
    transition_id: String,
    instrument_reading_ids: Vec<String>,
    pairwise_reading_ids: Vec<String>,
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
    stable_selection_hash: [u8; 32],
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
            let stable_selection_hash =
                stable_negative_panel_selection_hash(&panel, &outcome_signature)?;
            Ok(NegativePoolEntry {
                panel,
                outcome_signature,
                stable_selection_hash,
            })
        })
        .collect::<DynamicJepaResult<Vec<_>>>()?;
    pool.sort_by(|left, right| {
        left.stable_selection_hash
            .cmp(&right.stable_selection_hash)
            .then_with(|| left.outcome_signature.cmp(&right.outcome_signature))
    });
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
    let mut candidates = negative_pool
        .iter()
        .filter(|entry| {
            entry.panel.panel_id != row.target_panel.panel_id
                && entry.outcome_signature != target_signature
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.stable_selection_hash
            .cmp(&right.stable_selection_hash)
            .then_with(|| left.outcome_signature.cmp(&right.outcome_signature))
    });
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
    let input_panel_feature_hash = stable_panel_feature_hash(&row.input_panel)?;
    let target_panel_feature_hash = stable_panel_feature_hash(&row.target_panel)?;
    let seed = serde_json::to_vec(&json!({
        "domain_pack_id": domain_pack.id.to_string(),
        "domain_pack_version": &domain_pack.version,
        "row_index_in_trajectory": row.row_index_in_trajectory,
        "input_panel_feature_sha256": hex_bytes(&input_panel_feature_hash),
        "target_panel_feature_sha256": hex_bytes(&target_panel_feature_hash),
        "target_outcome_signature_sha256": hex_bytes(&sha256(target_signature.as_bytes())),
    }))
    .map_err(|err| {
        DynamicJepaError::validation(
            "compile-dataset.negative_sampling_seed",
            format!("failed to serialize stable negative sampling seed: {err}"),
            "negative sampling seed payload must remain JSON serializable",
        )
    })?;
    let hash = context_graph_storage::dynamicjepa::keys::blake3_128(&seed);
    let idx = (u128::from_be_bytes(hash) % candidates.len() as u128) as usize;
    Ok(candidates[idx].panel.panel_id)
}

fn stable_panel_feature_hash(panel: &LatentPanel) -> DynamicJepaResult<[u8; 32]> {
    let bytes = serde_json::to_vec(&json!({
        "domain_pack_id": panel.header.domain_pack_id.to_string(),
        "domain_pack_version": &panel.header.domain_pack_version,
        "ordered_slots": &panel.ordered_slots,
        "slot_vectors": &panel.slot_vectors,
        "slot_masks": &panel.slot_masks,
        "materializer_version": panel.materializer_version,
    }))
    .map_err(|err| {
        DynamicJepaError::validation(
            "compile-dataset.stable_panel_feature_hash",
            format!("failed to serialize stable panel feature hash payload: {err}"),
            "panel feature hash payload must exclude run-local UUIDs and remain JSON serializable",
        )
    })?;
    Ok(sha256(&bytes))
}

fn stable_negative_panel_selection_hash(
    panel: &LatentPanel,
    outcome_signature: &str,
) -> DynamicJepaResult<[u8; 32]> {
    let feature_hash = stable_panel_feature_hash(panel)?;
    let bytes = serde_json::to_vec(&json!({
        "feature_sha256": hex_bytes(&feature_hash),
        "outcome_signature_sha256": hex_bytes(&sha256(outcome_signature.as_bytes())),
    }))
    .map_err(|err| {
        DynamicJepaError::validation(
            "compile-dataset.stable_negative_panel_selection_hash",
            format!("failed to serialize stable negative panel key: {err}"),
            "negative panel ordering must exclude run-local UUIDs and remain JSON serializable",
        )
    })?;
    Ok(sha256(&bytes))
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
        process_fixture_line(FixtureLineRequest {
            db,
            domain_pack,
            adapter,
            source_uri,
            source_offset: line_offset,
            line_bytes: line,
            run_id,
            created,
        })?;
    }
    Ok(())
}

struct FixtureLineRequest<'a> {
    db: &'a rocksdb::DB,
    domain_pack: &'a DomainPack,
    adapter: &'a AdapterSpec,
    source_uri: &'a str,
    source_offset: u64,
    line_bytes: &'a [u8],
    run_id: Uuid,
    created: &'a mut IngestCreatedIds,
}

fn process_fixture_line(request: FixtureLineRequest<'_>) -> DynamicJepaResult<()> {
    let FixtureLineRequest {
        db,
        domain_pack,
        adapter,
        source_uri,
        source_offset,
        line_bytes,
        run_id,
        created,
    } = request;
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
    process_existing_raw_event(
        db,
        &domain_pack,
        &adapter,
        &raw_event,
        started_run,
        run_id,
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
                validate_numeric_bounds(value, field, &format!("{field_path}[{idx}]"))?;
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
    let mut pairwise_readings =
        match compute_pairwise_readings(domain_pack, &readings, transition.header.source_run_id) {
            Ok(rows) => rows,
            Err(err) => {
                let audit = build_audit(
                    "materialize_pairwise",
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
        };
    let pairwise_reading_ids = pairwise_readings
        .iter()
        .map(|reading| reading.pairwise_id)
        .collect::<Vec<_>>();
    let panel_hash = panel_hash(
        domain_pack,
        &reading_ids,
        &pairwise_reading_ids,
        &slots,
        &vectors,
    )?;
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
        pairwise_reading_ids: pairwise_reading_ids.clone(),
        ordered_slots: slots,
        slot_vectors: vectors,
        slot_masks: masks,
        panel_hash,
        materializer_version: 1,
    };
    panel.refresh_content_hash()?;
    let mut panel_audit = build_audit(
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
    set_audit_signal_yield(
        &mut panel_audit,
        signal_yield_for_operation(
            "materialize_panel",
            SignalYieldDimensions {
                n_panel_slots: u32::try_from(readings.len()).map_err(|_| {
                    DynamicJepaError::validation(
                        "materialize_panel.instrument_count",
                        "instrument count exceeds u32 signal-yield capacity",
                        "reduce the domain pack instrument count",
                    )
                })?,
                n_active_modalities: 0,
                k_candidates: 0,
            },
        )?,
    )?;
    let pairwise_output_ids = pairwise_reading_ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut pairwise_audit = build_audit(
        "materialize_pairwise",
        AuditStatus::Ok,
        reading_ids.iter().map(Uuid::to_string).collect(),
        pairwise_output_ids,
        vec![CF_DJ_PAIRWISE_READINGS.to_string(), CF_DJ_AUDIT_LOG.to_string()],
        pairwise_readings
            .iter()
            .map(|reading| reading.header.content_hash)
            .collect(),
    )?;
    let successful_reading_count = readings
        .iter()
        .filter(|reading| matches!(reading.status, ReadingStatus::Ok))
        .count();
    set_audit_signal_yield(
        &mut pairwise_audit,
        signal_yield_for_operation(
            "materialize_pairwise",
            SignalYieldDimensions {
                n_panel_slots: u32::try_from(successful_reading_count).map_err(|_| {
                    DynamicJepaError::validation(
                        "materialize_pairwise.successful_reading_count",
                        "successful reading count exceeds u32 signal-yield capacity",
                        "reduce the domain pack instrument count",
                    )
                })?,
                n_active_modalities: 0,
                k_candidates: 0,
            },
        )?,
    )?;
    put_panel_with_readings_and_pairwise_batch(
        db,
        &readings,
        &pairwise_readings,
        &panel,
        &[panel_audit, pairwise_audit],
    )?;

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
    let stored_pairwise = list_pairwise_readings_for_event(db, panel.event_id)?;
    pairwise_readings.sort_by_key(|reading| reading.pairwise_id);
    let mut stored_pairwise_sorted = stored_pairwise;
    stored_pairwise_sorted.sort_by_key(|reading| reading.pairwise_id);
    if stored_pairwise_sorted != pairwise_readings {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "pairwise readings readback mismatch for event {}",
                panel.event_id
            ),
        });
    }
    Ok(MaterializedPanelEvidence {
        panel_id: panel_id.to_string(),
        event_id: state.source_event_id.to_string(),
        transition_id: transition.transition_id.to_string(),
        instrument_reading_ids: reading_ids.iter().map(Uuid::to_string).collect(),
        pairwise_reading_ids: pairwise_reading_ids.iter().map(ToString::to_string).collect(),
    })
}

struct PairwiseSource<'a> {
    spec: &'a InstrumentSpec,
    reading: &'a InstrumentReading,
    artifact_hash: [u8; 32],
}

fn compute_pairwise_readings(
    domain_pack: &DomainPack,
    readings: &[InstrumentReading],
    source_run_id: Option<Uuid>,
) -> DynamicJepaResult<Vec<PairwiseReading>> {
    let mut readings_by_instrument = BTreeMap::new();
    for reading in readings {
        if readings_by_instrument
            .insert(reading.instrument_id.as_str(), reading)
            .is_some()
        {
            return Err(DynamicJepaError::validation(
                "InstrumentReading.instrument_id",
                format!("duplicate reading for instrument {}", reading.instrument_id),
                "write one same-event instrument reading per domain-pack instrument",
            ));
        }
    }

    let mut successful = Vec::new();
    for spec in &domain_pack.instrument_specs {
        if !spec.pair_kinds.contains(&PairKindName::Cosine) {
            return Err(DynamicJepaError::PairwiseCosineKindMissing {
                instrument_id: spec.instrument_id.to_string(),
                pair_kinds: spec.pair_kinds.iter().map(ToString::to_string).collect(),
            });
        }
        let reading = readings_by_instrument
            .get(spec.instrument_id.as_str())
            .copied()
            .ok_or_else(|| DynamicJepaError::validation(
                "InstrumentReading.instrument_id",
                format!(
                    "panel materialization did not produce reading for instrument {}",
                    spec.instrument_id
                ),
                "compute every domain-pack instrument before pairwise materialization",
            ))?;
        if matches!(reading.status, ReadingStatus::Ok) {
            successful.push(PairwiseSource {
                spec,
                reading,
                artifact_hash: instrument_artifact_hash32(spec)?,
            });
        }
    }

    let mut out = Vec::with_capacity(successful.len().saturating_mul(successful.len()) / 2);
    let created_at_unix_ms = now_unix_ms()? as u64;
    for j in 0..successful.len() {
        for k in (j + 1)..successful.len() {
            let left = &successful[j];
            let right = &successful[k];
            let mut kinds = PairKindBitset(PairKindBitset::COSINE_AGREEMENT);
            let cosine = cosine_similarity_padded_eps(
                &left.reading.output_dense,
                &right.reading.output_dense,
                "PairwiseReading.cosine_agreement",
            )?;
            let rank_disagreement = if both_declare_pair_kind(
                left.spec,
                right.spec,
                PairKindName::RankDisagreement,
            ) {
                kinds.set(PairKindName::RankDisagreement.bit());
                Some(rank_disagreement_padded(
                    &left.reading.output_dense,
                    &right.reading.output_dense,
                )?)
            } else {
                None
            };
            reject_unsupported_emitted_pair_kinds(left.spec, right.spec)?;
            let pairwise_id = PairwiseReadingId::new_v4();
            let mut row = PairwiseReading {
                header: DjRecordHeader::new(
                    pairwise_id.0,
                    PAIRWISE_READING_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    created_at_unix_ms as i64,
                    source_run_id,
                ),
                pairwise_id,
                event_id: left.reading.event_id,
                instrument_j: left.spec.instrument_id.clone(),
                instrument_k: right.spec.instrument_id.clone(),
                instrument_j_artifact_hash: left.artifact_hash,
                instrument_k_artifact_hash: right.artifact_hash,
                kinds_emitted: kinds,
                cosine_agreement: cosine,
                rank_disagreement,
                modality_contradiction: None,
                sparse_dense_mismatch: None,
                temporal_surprise: None,
                causal_direction_disagreement: None,
                safety_proximity: None,
                created_at_unix_ms,
                validation_status: ReadingStatus::Ok,
            };
            row.refresh_content_hash()?;
            out.push(row);
        }
    }
    Ok(out)
}

fn reject_unsupported_emitted_pair_kinds(
    left: &InstrumentSpec,
    right: &InstrumentSpec,
) -> DynamicJepaResult<()> {
    for kind in [
        PairKindName::ModalityContradiction,
        PairKindName::SparseDenseMismatch,
        PairKindName::TemporalSurprise,
        PairKindName::CausalDirectionDisagreement,
        PairKindName::SafetyProximity,
    ] {
        if both_declare_pair_kind(left, right, kind) {
            return Err(DynamicJepaError::PairwiseUnsupportedKind {
                instrument_j: left.instrument_id.to_string(),
                instrument_k: right.instrument_id.to_string(),
                kind: kind.to_string(),
                message: "the current strict TOML schema has no required metadata to compute this pair-kind deterministically".to_string(),
            });
        }
    }
    Ok(())
}

fn both_declare_pair_kind(
    left: &InstrumentSpec,
    right: &InstrumentSpec,
    kind: PairKindName,
) -> bool {
    left.pair_kinds.contains(&kind) && right.pair_kinds.contains(&kind)
}

fn cosine_similarity_padded_eps(left: &[f32], right: &[f32], field: &str) -> DynamicJepaResult<f32> {
    if left.is_empty() || right.is_empty() {
        return Err(DynamicJepaError::validation(
            field,
            "cosine requires two non-empty dense vectors",
            "persist pairwise rows only for successful instrument readings",
        ));
    }
    let dim = left.len().max(right.len());
    let mut dot = 0.0f64;
    let mut left_norm_sq = 0.0f64;
    let mut right_norm_sq = 0.0f64;
    for idx in 0..dim {
        let left_value = left.get(idx).copied().unwrap_or(0.0) as f64;
        let right_value = right.get(idx).copied().unwrap_or(0.0) as f64;
        dot += left_value * right_value;
        left_norm_sq += left_value * left_value;
        right_norm_sq += right_value * right_value;
    }
    let denom = left_norm_sq.sqrt().max(1.0e-8) * right_norm_sq.sqrt().max(1.0e-8);
    let raw = dot / denom;
    if !raw.is_finite() || raw.abs() > 1.0001 {
        return Err(DynamicJepaError::validation(
            field,
            format!("computed cosine is outside [-1,1]: {raw}"),
            "inspect instrument output vectors for non-finite values or numeric overflow",
        ));
    }
    Ok(raw.clamp(-1.0, 1.0) as f32)
}

fn rank_disagreement_padded(left: &[f32], right: &[f32]) -> DynamicJepaResult<f32> {
    if left.is_empty() || right.is_empty() {
        return Err(DynamicJepaError::validation(
            "PairwiseReading.rank_disagreement",
            "rank disagreement requires two non-empty dense vectors",
            "persist pairwise rows only for successful instrument readings",
        ));
    }
    let dim = left.len().max(right.len());
    if dim <= 1 {
        return Ok(0.0);
    }
    let left_rank = dense_descending_ranks(left, dim);
    let right_rank = dense_descending_ranks(right, dim);
    let normalizer = (dim - 1) as f32;
    let mean = left_rank
        .iter()
        .zip(right_rank.iter())
        .map(|(lhs, rhs)| lhs.abs_diff(*rhs) as f32 / normalizer)
        .sum::<f32>()
        / dim as f32;
    if !mean.is_finite() {
        return Err(DynamicJepaError::validation(
            "PairwiseReading.rank_disagreement",
            "rank disagreement computation produced a non-finite value",
            "inspect instrument output vectors before enabling rank_disagreement",
        ));
    }
    Ok(mean)
}

fn dense_descending_ranks(values: &[f32], dim: usize) -> Vec<usize> {
    let mut indexed = (0..dim)
        .map(|idx| (idx, values.get(idx).copied().unwrap_or(0.0)))
        .collect::<Vec<_>>();
    indexed.sort_by(|(left_idx, left_value), (right_idx, right_value)| {
        right_value
            .total_cmp(left_value)
            .then_with(|| left_idx.cmp(right_idx))
    });
    let mut ranks = vec![0usize; dim];
    for (rank, (idx, _)) in indexed.into_iter().enumerate() {
        ranks[idx] = rank;
    }
    ranks
}

fn instrument_artifact_hash32(spec: &InstrumentSpec) -> DynamicJepaResult<[u8; 32]> {
    let bytes = serde_json::to_vec(spec).map_err(|err| {
        DynamicJepaError::validation(
            "InstrumentSpec.artifact_hash",
            format!("failed to serialize instrument spec for hash: {err}"),
            "instrument specs must remain JSON serializable for pairwise hash provenance",
        )
    })?;
    Ok(sha256(&bytes))
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
        InstrumentKind::Vector { dim } => {
            if inputs.len() != 1 {
                return Err(instrument_failed(
                    spec,
                    state.source_event_id,
                    "vector instrument requires exactly one input field",
                    spec.input_fields.join(","),
                ));
            }
            let vector = match &inputs[0].1 {
                FieldValue::Vector(values) => values,
                other => {
                    return Err(instrument_failed(
                        spec,
                        state.source_event_id,
                        format!(
                            "vector input {} is {} not vector",
                            inputs[0].0,
                            field_value_kind(other)
                        ),
                        inputs[0].0.clone(),
                    ));
                }
            };
            if vector.len() != *dim {
                return Err(instrument_failed(
                    spec,
                    state.source_event_id,
                    format!(
                        "vector input {} expected dim {dim}, got {}",
                        inputs[0].0,
                        vector.len()
                    ),
                    inputs[0].0.clone(),
                ));
            }
            let mut out = Vec::with_capacity(*dim);
            for (idx, value) in vector.iter().enumerate() {
                validate_instrument_numeric(
                    spec,
                    *value as f64,
                    &format!("{}[{idx}]", inputs[0].0),
                    state.source_event_id,
                )?;
                out.push(normalize_value(*value as f64, &spec.normalization)? as f32);
            }
            out
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
    pairwise_reading_ids: &[PairwiseReadingId],
    slots: &[PanelSlot],
    vectors: &[Vec<f32>],
) -> DynamicJepaResult<[u8; 32]> {
    let mut sorted_reading_ids = reading_ids.iter().map(Uuid::to_string).collect::<Vec<_>>();
    sorted_reading_ids.sort();
    let mut sorted_pairwise_ids = pairwise_reading_ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    sorted_pairwise_ids.sort();
    let bytes = serde_json::to_vec(&json!({
        "domain_pack_id": domain_pack.id.to_string(),
        "reading_ids": sorted_reading_ids,
        "pairwise_reading_ids": sorted_pairwise_ids,
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
        CF_DJ_PAIRWISE_READINGS: inspect_cf(db, CF_DJ_PAIRWISE_READINGS, 40, 0)?,
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
        signal_yield: 0,
    };
    record.validate()?;
    Ok(record)
}

fn set_audit_signal_yield(audit: &mut DjAuditRecord, signal_yield: u32) -> DynamicJepaResult<()> {
    audit.signal_yield = signal_yield;
    audit.validate()
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

fn parse_threshold_calibration_id(
    value: &str,
    field: &str,
) -> DynamicJepaResult<ThresholdCalibrationId> {
    Ok(ThresholdCalibrationId(parse_uuid(value, field)?))
}

fn parse_hex_key(value: &str, field: &str) -> DynamicJepaResult<Vec<u8>> {
    let text = value.trim();
    if text.is_empty() || !text.len().is_multiple_of(2) {
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
    let instrument_specs = parsed
        .instruments
        .into_iter()
        .enumerate()
        .map(|(idx, spec)| instrument_from_toml(spec, idx))
        .collect::<DynamicJepaResult<Vec<_>>>()?;
    let constellation = constellation_from_toml(parsed.constellation)?;
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
        instrument_specs,
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
        constellation,
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
    let instrument_id = InstrumentId::new(spec.id)?;
    let pair_kinds = spec.pair_kinds.ok_or_else(|| {
        DynamicJepaError::PairwiseCosineKindMissing {
            instrument_id: instrument_id.to_string(),
            pair_kinds: Vec::new(),
        }
    })?;
    let pair_kinds = pair_kinds
        .into_iter()
        .map(|kind| PairKindName::parse(&kind))
        .collect::<DynamicJepaResult<Vec<_>>>()?;
    Ok(InstrumentSpec {
        instrument_id,
        kind: instrument_kind_from_toml(spec.kind, &format!("instruments[{idx}].kind"))?,
        input_fields: spec.input_fields,
        output_shape: spec.output_shape,
        normalization: normalization_from_toml(
            spec.normalization,
            &format!("instruments[{idx}].normalization"),
        )?,
        required: spec.required,
        model_ref: spec.model_ref,
        pair_kinds,
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
            "vector" => Err(toml_validation_error(
                field,
                "vector instrument kind must declare dim: kind = { type = \"vector\", dim = N }",
            )),
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
            "vector" => Ok(InstrumentKind::Vector {
                dim: table.dim.ok_or_else(|| {
                    toml_validation_error(field, "vector instrument kind missing dim")
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

fn constellation_from_toml(
    value: Option<ConstellationToml>,
) -> DynamicJepaResult<Option<ConstellationConfig>> {
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(Some(ConstellationConfig {
        global: value
            .global
            .map(|global| -> DynamicJepaResult<ConstellationGlobalSpec> {
                Ok(ConstellationGlobalSpec {
                    modalities: modalities_from_toml(
                        global.modalities,
                        "constellation.global.modalities",
                    )?,
                    require_loo_min: global.require_loo_min,
                    calibration_percentile: global.calibration_percentile,
                    calibration_set_size: global.calibration_set_size,
                })
            })
            .transpose()?,
        subjects: value
            .subjects
            .into_iter()
            .map(|subject| -> DynamicJepaResult<ConstellationSubjectSpec> {
                Ok(ConstellationSubjectSpec {
                    id: subject.id,
                    field: subject.field,
                    value: subject.value,
                    modalities: modalities_from_toml(
                        subject.modalities,
                        "constellation.subjects.modalities",
                    )?,
                    require_loo_min: subject.require_loo_min,
                    calibration_percentile: subject.calibration_percentile,
                    calibration_set_size: subject.calibration_set_size,
                })
            })
            .collect::<DynamicJepaResult<Vec<_>>>()?,
    }))
}

fn modalities_from_toml(
    value: ModalitiesToml,
    field: &str,
) -> DynamicJepaResult<ConstellationModalities> {
    match value {
        ModalitiesToml::Name(name) if name == "all" => Ok(ConstellationModalities::All),
        ModalitiesToml::Name(name) => Err(toml_validation_error(
            field,
            format!("unsupported modalities string {name:?}; use \"all\" or a list of instrument ids"),
        )),
        ModalitiesToml::List(values) => Ok(ConstellationModalities::List(
            values
                .into_iter()
                .map(InstrumentId::new)
                .collect::<DynamicJepaResult<Vec<_>>>()?,
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
    validate_bundle_schema_for_db_path(path)?;
    RocksDbTeleologicalStore::open_with_config(path, TeleologicalStoreConfig::default()).map_err(
        |err| DynamicJepaError::Storage {
            operation: "open_dynamicjepa_db".to_string(),
            cf: "rocksdb".to_string(),
            message: format!("failed to open {}: {err}", path.display()),
            remediation: "verify the DB path is a writable demo path and no active process owns the RocksDB lock".to_string(),
        },
    )
}

fn open_existing_store(
    path: &Path,
    operation: &str,
) -> DynamicJepaResult<RocksDbTeleologicalStore> {
    validate_bundle_schema_for_db_path(path)?;
    if !path.exists() {
        return Err(DynamicJepaError::validation(
            format!("{operation}.db"),
            format!("DB path does not exist: {}", path.display()),
            "run the command that creates the RocksDB source of truth before inspecting it; inspection never creates DB paths",
        ));
    }
    let config = TeleologicalStoreConfig {
        create_if_missing: false,
        create_missing_column_families: false,
        ..TeleologicalStoreConfig::default()
    };
    RocksDbTeleologicalStore::open_with_config(path, config).map_err(|err| {
        DynamicJepaError::Storage {
            operation: operation.to_string(),
            cf: "rocksdb".to_string(),
            message: format!("failed to open existing {}: {err}", path.display()),
            remediation: "verify the DB path is a complete existing source-of-truth RocksDB and no active process owns the RocksDB lock".to_string(),
        }
    })
}

fn validate_bundle_schema_for_db_path(db_path: &Path) -> DynamicJepaResult<()> {
    for ancestor in db_path.ancestors().take(8) {
        let manifests = BUNDLE_SCHEMA_MANIFESTS
            .iter()
            .map(|name| ancestor.join(name))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        if manifests.is_empty() {
            continue;
        }
        for manifest_path in manifests {
            let manifest = read_json_file(&manifest_path)?;
            validate_bundle_schema_manifest(&manifest_path, &manifest)?;
        }
        return Ok(());
    }
    Ok(())
}

fn validate_bundle_schema_manifest(
    manifest_path: &Path,
    manifest: &Value,
) -> DynamicJepaResult<()> {
    let found = match manifest.get("bundle_schema_version") {
        Some(value) => value
            .as_u64()
            .and_then(|raw| u32::try_from(raw).ok())
            .unwrap_or(0),
        None => 1,
    };
    if found != DYNAMICJEPA_BUNDLE_SCHEMA_VERSION {
        return Err(DynamicJepaError::BundleSchemaVersionMismatch {
            expected: DYNAMICJEPA_BUNDLE_SCHEMA_VERSION,
            found,
            manifest_path: manifest_path.display().to_string(),
        });
    }
    Ok(())
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

fn zero_dj_counts() -> BTreeMap<String, u64> {
    DJ_CF_NAMES
        .iter()
        .map(|cf| ((*cf).to_string(), 0_u64))
        .collect()
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
        DynamicJepaError::BundleSchemaVersionMismatch {
            expected,
            found,
            manifest_path,
        } => {
            object.insert(
                "expected_bundle_schema_version".to_string(),
                json!(expected),
            );
            object.insert("found_bundle_schema_version".to_string(), json!(found));
            object.insert("manifest_path".to_string(), json!(manifest_path));
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
        DynamicJepaError::BundleSchemaVersionMismatch { .. } => Some(
            "rebuild the DynamicJEPA bundle with the v2 binary; v1 manifests are rejected before RocksDB is opened"
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
    constellation: Option<ConstellationToml>,
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
    pair_kinds: Option<Vec<String>>,
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
struct ConstellationToml {
    global: Option<ConstellationGlobalToml>,
    #[serde(default)]
    subjects: Vec<ConstellationSubjectToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstellationGlobalToml {
    modalities: ModalitiesToml,
    require_loo_min: f32,
    calibration_percentile: u8,
    calibration_set_size: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstellationSubjectToml {
    id: String,
    field: String,
    value: String,
    modalities: ModalitiesToml,
    require_loo_min: f32,
    calibration_percentile: u8,
    calibration_set_size: u32,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ModalitiesToml {
    Name(String),
    List(Vec<String>),
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
