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
