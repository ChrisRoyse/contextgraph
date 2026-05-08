//! DynamicJEPA MCP request DTOs.

use std::path::PathBuf;

use serde::Deserialize;

fn default_limit_20() -> usize {
    20
}

fn default_limit_100() -> usize {
    100
}

fn default_zero() -> usize {
    0
}

fn default_domain_version() -> String {
    "1.0.0".to_string()
}

fn default_subject_global() -> String {
    "global".to_string()
}

fn default_modality_all() -> String {
    "all".to_string()
}

fn default_estimator_ksg() -> String {
    "ksg".to_string()
}

fn default_trajectory_policy() -> String {
    "by_domain_session".to_string()
}

fn default_dataset_policy() -> String {
    "one_step".to_string()
}

fn default_split() -> String {
    "train".to_string()
}

fn default_binding_method() -> String {
    "explicit_mapping".to_string()
}

fn default_binding_kind() -> String {
    "event_to_trajectory".to_string()
}

fn default_score() -> f32 {
    1.0
}

fn default_mi_sample_size() -> usize {
    1000
}

fn default_ksg_k() -> usize {
    5
}

fn default_mi_bootstrap_iters() -> usize {
    1000
}

fn default_mi_seed() -> u64 {
    20260501
}

fn default_transfer_seeds() -> Vec<u64> {
    vec![42, 43, 44, 45, 46]
}

fn default_transfer_source_events() -> usize {
    1000
}

fn default_transfer_target_events() -> usize {
    200
}

fn default_transfer_bootstrap_iters() -> usize {
    10_000
}

fn default_transfer_train_epochs() -> usize {
    160
}

fn default_transfer_batch_size() -> usize {
    64
}

fn default_transfer_max_seconds() -> u64 {
    120
}

fn default_transfer_learning_rate() -> f64 {
    0.001
}

fn default_transfer_stopping_target() -> f64 {
    0.20
}

fn default_semantic_max_files() -> usize {
    5000
}

fn default_min_raw_events() -> usize {
    60
}

fn default_min_tool_families() -> usize {
    3
}

fn default_min_languages() -> usize {
    2
}

fn default_min_patch_deltas() -> usize {
    3
}

fn default_min_compiler_checked() -> usize {
    1
}

fn default_min_margin() -> f64 {
    0.0
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RegisterDomainPackRequest {
    pub db_path: PathBuf,
    pub file: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ListDomainPacksRequest {
    pub db_path: PathBuf,
    #[serde(default = "default_limit_100")]
    pub limit: usize,
    #[serde(default = "default_zero")]
    pub offset: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetDomainPackRequest {
    pub db_path: PathBuf,
    pub id: String,
    #[serde(default = "default_domain_version")]
    pub domain_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InspectCountsRequest {
    pub db_path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InspectCfRequest {
    pub db_path: PathBuf,
    pub cf: String,
    pub key_hex: Option<String>,
    #[serde(default = "default_limit_20")]
    pub limit: usize,
    #[serde(default = "default_zero")]
    pub offset: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IngestEventRequest {
    pub db_path: PathBuf,
    pub domain: String,
    pub adapter: String,
    pub file: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RunAdapterRequest {
    pub db_path: PathBuf,
    pub event_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MaterializePanelRequest {
    pub db_path: PathBuf,
    pub transition_id: Option<String>,
    #[serde(default)]
    pub all_pending: bool,
    pub domain: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetPanelRequest {
    pub db_path: PathBuf,
    pub panel_id: String,
    #[serde(default)]
    pub include_readings: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ListInstrumentReadingsRequest {
    pub db_path: PathBuf,
    pub event_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreateBindingRequest {
    pub db_path: PathBuf,
    pub left_cf: String,
    pub left_key: String,
    pub right_cf: String,
    pub right_key: String,
    #[serde(default = "default_binding_method")]
    pub method: String,
    #[serde(default = "default_binding_kind")]
    pub kind: String,
    #[serde(default = "default_score")]
    pub score: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ListBindingsRequest {
    pub db_path: PathBuf,
    pub entity: Option<String>,
    #[serde(default = "default_limit_100")]
    pub limit: usize,
    #[serde(default = "default_zero")]
    pub offset: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CompileTrajectoriesRequest {
    pub db_path: PathBuf,
    pub domain: String,
    #[serde(default = "default_trajectory_policy")]
    pub policy: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetTrajectoryRequest {
    pub db_path: PathBuf,
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ListTrajectoriesRequest {
    pub db_path: PathBuf,
    pub domain: String,
    #[serde(default = "default_limit_100")]
    pub limit: usize,
    #[serde(default = "default_zero")]
    pub offset: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CompileDatasetRequest {
    pub db_path: PathBuf,
    pub domain: String,
    #[serde(default = "default_dataset_policy")]
    pub policy: String,
    #[serde(default = "default_split")]
    pub split: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetDatasetShardRequest {
    pub db_path: PathBuf,
    pub dataset_id: String,
    pub shard_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InspectDatasetRowRequest {
    pub db_path: PathBuf,
    pub dataset_id: String,
    pub shard_id: String,
    pub row: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TrainRequest {
    pub db_path: PathBuf,
    pub dataset_id: String,
    pub config: PathBuf,
    pub artifact_root: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetTrainingRunRequest {
    pub db_path: PathBuf,
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetArtifactRequest {
    pub db_path: PathBuf,
    pub id: String,
    #[serde(default)]
    pub verify_files: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PredictRequest {
    pub db_path: PathBuf,
    pub artifact_id: String,
    pub panel_id: String,
    pub action_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PlanRequest {
    pub db_path: PathBuf,
    pub artifact_id: String,
    pub panel_id: String,
    pub skill_id: String,
    pub candidate_action_json: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RecordSurpriseRequest {
    pub db_path: PathBuf,
    pub prediction_id: String,
    pub observed_outcome_id: String,
    pub observed_panel_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BuildConstellationRequest {
    pub db_path: PathBuf,
    pub domain: String,
    #[serde(default = "default_domain_version")]
    pub domain_version: String,
    #[serde(default = "default_subject_global")]
    pub subject: String,
    pub source_event_selector: Option<String>,
    pub built_by_run_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ListConstellationsRequest {
    pub db_path: PathBuf,
    pub domain: Option<String>,
    pub subject: Option<String>,
    #[serde(default = "default_limit_100")]
    pub limit: usize,
    #[serde(default = "default_zero")]
    pub offset: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetConstellationRequest {
    pub db_path: PathBuf,
    pub domain: String,
    #[serde(default = "default_domain_version")]
    pub domain_version: String,
    #[serde(default = "default_subject_global")]
    pub subject: String,
    pub modality: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CalibrateThresholdRequest {
    pub db_path: PathBuf,
    pub domain: String,
    #[serde(default = "default_domain_version")]
    pub domain_version: String,
    #[serde(default = "default_subject_global")]
    pub subject: String,
    #[serde(default = "default_modality_all")]
    pub modality: String,
    pub calibration_event_selector: Option<String>,
    pub percentile: Option<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RecalibrateThresholdRequest {
    pub db_path: PathBuf,
    pub domain: String,
    #[serde(default = "default_domain_version")]
    pub domain_version: String,
    #[serde(default = "default_subject_global")]
    pub subject: String,
    #[serde(default = "default_modality_all")]
    pub modality: String,
    pub calibration_event_selector: Option<String>,
    pub supersedes: Option<String>,
    pub reason: Option<String>,
    pub percentile: Option<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ComputeMcRatioRequest {
    pub db_path: PathBuf,
    pub domain: String,
    #[serde(default = "default_domain_version")]
    pub domain_version: String,
    pub output_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AuditPairwiseMiRequest {
    pub db_path: PathBuf,
    pub domain: String,
    #[serde(default = "default_domain_version")]
    pub domain_version: String,
    #[serde(default = "default_mi_sample_size")]
    pub sample_size: usize,
    #[serde(default = "default_estimator_ksg")]
    pub estimator: String,
    #[serde(default = "default_ksg_k")]
    pub ksg_k: usize,
    #[serde(default = "default_mi_bootstrap_iters")]
    pub bootstrap_iters: usize,
    #[serde(default = "default_mi_seed")]
    pub seed: u64,
    pub output_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CrossDomainTransferRequest {
    pub output_root: PathBuf,
    #[serde(default = "default_transfer_seeds")]
    pub seeds: Vec<u64>,
    #[serde(default = "default_transfer_source_events")]
    pub source_events: usize,
    #[serde(default = "default_transfer_target_events")]
    pub target_events: usize,
    #[serde(default = "default_transfer_bootstrap_iters")]
    pub bootstrap_iters: usize,
    #[serde(default = "default_transfer_train_epochs")]
    pub train_epochs: usize,
    #[serde(default = "default_transfer_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_transfer_max_seconds")]
    pub max_seconds_per_training: u64,
    #[serde(default = "default_transfer_learning_rate")]
    pub learning_rate: f64,
    #[serde(default = "default_transfer_stopping_target")]
    pub stopping_target: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BuildSemanticIndexRequest {
    pub repo: PathBuf,
    pub output: PathBuf,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default = "default_semantic_max_files")]
    pub max_files: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ValidateCorpusDiversityRequest {
    pub db_path: PathBuf,
    #[serde(default = "default_min_raw_events")]
    pub min_raw_events: usize,
    #[serde(default = "default_min_tool_families")]
    pub min_tool_families: usize,
    #[serde(default = "default_min_languages")]
    pub min_languages: usize,
    #[serde(default = "default_min_patch_deltas")]
    pub min_patch_deltas: usize,
    #[serde(default = "default_min_compiler_checked")]
    pub min_compiler_checked: usize,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AttributeTestDeltaRequest {
    pub repo: PathBuf,
    pub coverage_json: PathBuf,
    pub changed_files_json: PathBuf,
    pub failures_before_json: PathBuf,
    pub failures_after_json: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CompareShadowUtilityRequest {
    pub db_path: PathBuf,
    pub candidate_artifact_id: String,
    pub active_artifact_id: String,
    #[serde(default = "default_min_margin")]
    pub min_margin: f64,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetPredictionRequest {
    pub db_path: PathBuf,
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetPlanTraceRequest {
    pub db_path: PathBuf,
    pub id: String,
    #[serde(default)]
    pub include_predictions: bool,
    #[serde(default)]
    pub include_guards: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetSurpriseRequest {
    pub db_path: PathBuf,
    pub id: String,
}
