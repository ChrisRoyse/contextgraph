use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{
    ActionId, DatasetId, DomainPackId, ModelArtifactId, PanelId, PredictionId, TrainingRunId,
};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::Validate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const TRAINING_RUN_RECORD_VERSION: u8 = 1;
pub const PREDICTION_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingRunRecord {
    pub header: DjRecordHeader,
    pub training_run_id: TrainingRunId,
    pub domain_pack_id: DomainPackId,
    pub dataset_id: DatasetId,
    pub started_at_unix_ms: i64,
    pub finished_at_unix_ms: Option<i64>,
    pub status: TrainingRunStatus,
    pub training_config_hash: [u8; 32],
    pub objective_ids: Vec<String>,
    pub metrics: BTreeMap<String, f64>,
    pub artifact_ids: Vec<ModelArtifactId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TrainingRunStatus {
    Started,
    Completed,
    Failed { error: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

impl Validate for TrainingRunRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.training_run_id.validate()?;
        self.domain_pack_id.validate()?;
        self.dataset_id.validate()?;
        if self.started_at_unix_ms < 0 {
            return Err(DynamicJepaError::TrainingFailed {
                training_run_id: self.training_run_id.0,
                message: "started_at_unix_ms must be non-negative".to_string(),
                remediation: "write Unix epoch milliseconds".to_string(),
            });
        }
        if let Some(finished) = self.finished_at_unix_ms {
            if finished < self.started_at_unix_ms {
                return Err(DynamicJepaError::TrainingFailed {
                    training_run_id: self.training_run_id.0,
                    message: "finished_at_unix_ms precedes started_at_unix_ms".to_string(),
                    remediation: "write monotonic training run timestamps".to_string(),
                });
            }
        }
        if self.training_config_hash == [0; 32] {
            return Err(DynamicJepaError::TrainingFailed {
                training_run_id: self.training_run_id.0,
                message: "training_config_hash must be computed".to_string(),
                remediation: "hash the canonical training config before starting the run"
                    .to_string(),
            });
        }
        if self.objective_ids.is_empty() {
            return Err(DynamicJepaError::TrainingFailed {
                training_run_id: self.training_run_id.0,
                message: "objective_ids must not be empty".to_string(),
                remediation: "copy objectives from the dataset/domain pack".to_string(),
            });
        }
        for (metric, value) in &self.metrics {
            if metric.trim().is_empty() || !value.is_finite() {
                return Err(DynamicJepaError::TrainingFailed {
                    training_run_id: self.training_run_id.0,
                    message: format!("invalid metric {metric:?}={value}"),
                    remediation: "record only finite named metrics".to_string(),
                });
            }
        }
        if matches!(self.status, TrainingRunStatus::Completed) && self.artifact_ids.is_empty() {
            return Err(DynamicJepaError::TrainingFailed {
                training_run_id: self.training_run_id.0,
                message: "completed training run must reference at least one artifact".to_string(),
                remediation: "register artifact and update training run atomically".to_string(),
            });
        }
        for id in &self.artifact_ids {
            id.validate()?;
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(
    TrainingRunRecord,
    TRAINING_RUN_RECORD_VERSION,
    "TrainingRunRecord"
);

impl Validate for PredictionRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.prediction_id.validate()?;
        self.model_artifact_id.validate()?;
        self.input_panel_id.validate()?;
        self.candidate_action_id.validate()?;
        if self.model_artifact_hash_at_inference == [0; 32] {
            return Err(DynamicJepaError::PredictionInputMissing {
                target_id: self.model_artifact_id.to_string(),
                cf: "dj_model_artifacts".to_string(),
            });
        }
        if self.predicted_next_panel_vec.is_empty() {
            return Err(DynamicJepaError::validation(
                "PredictionRecord.predicted_next_panel_vec",
                "prediction vector must not be empty",
                "write the actual predictor output",
            ));
        }
        for (idx, value) in self.predicted_next_panel_vec.iter().enumerate() {
            if !value.is_finite() {
                return Err(DynamicJepaError::validation(
                    format!("PredictionRecord.predicted_next_panel_vec[{idx}]"),
                    format!("predicted value must be finite, got {value}"),
                    "abort prediction on NaN or infinity",
                ));
            }
        }
        if !self.uncertainty.is_finite() {
            return Err(DynamicJepaError::validation(
                "PredictionRecord.uncertainty",
                "uncertainty must be finite",
                "record a finite uncertainty scalar",
            ));
        }
        for (objective, score) in &self.objective_scores {
            if objective.trim().is_empty() || !score.is_finite() {
                return Err(DynamicJepaError::validation(
                    "PredictionRecord.objective_scores",
                    format!("invalid objective score {objective:?}={score}"),
                    "record finite objective scores with non-empty names",
                ));
            }
        }
        if self.created_at_unix_ms < 0 {
            return Err(DynamicJepaError::validation(
                "PredictionRecord.created_at_unix_ms",
                "timestamp must be non-negative",
                "write Unix epoch milliseconds",
            ));
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(
    PredictionRecord,
    PREDICTION_RECORD_VERSION,
    "PredictionRecord"
);
