use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{
    ActionId, ConstellationId, DomainPackId, ModelArtifactId, OutcomeId, PanelId, PlanTraceId,
    PredictionId, SkillId, SurpriseEventId,
};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::Validate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PLAN_TRACE_RECORD_VERSION: u8 = 2;
pub const SURPRISE_EVENT_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanTraceRecord {
    pub header: DjRecordHeader,
    pub plan_trace_id: PlanTraceId,
    pub domain_pack_id: DomainPackId,
    pub current_panel_id: PanelId,
    pub model_artifact_id: ModelArtifactId,
    pub model_artifact_hash_at_plan: [u8; 32],
    pub skill_policy_id: SkillId,
    pub candidate_action_ids: Vec<ActionId>,
    pub prediction_ids: Vec<PredictionId>,
    pub guard_decision_ids: Vec<Uuid>,
    pub utility_scores: Vec<f32>,
    pub selected_action_id: Option<ActionId>,
    #[serde(default)]
    pub no_accepted_candidate: bool,
    #[serde(default)]
    pub constellation_uuid_used: Option<ConstellationId>,
    pub status: PlanTraceStatus,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlanTraceStatus {
    Selected,
    Rejected,
    Failed { error: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurpriseEventRecord {
    pub header: DjRecordHeader,
    pub surprise_event_id: SurpriseEventId,
    pub prediction_id: PredictionId,
    pub observed_outcome_id: OutcomeId,
    pub observed_panel_id: PanelId,
    pub surprise_kind: SurpriseKind,
    pub cosine: f32,
    pub threshold: f32,
    pub error_norm: f32,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurpriseKind {
    UnexpectedOutcome,
}

impl Validate for PlanTraceRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.plan_trace_id.validate()?;
        self.domain_pack_id.validate()?;
        self.current_panel_id.validate()?;
        self.model_artifact_id.validate()?;
        self.skill_policy_id.validate()?;
        if self.model_artifact_hash_at_plan == [0; 32] {
            return Err(DynamicJepaError::ArtifactHashMismatch {
                artifact_id: self.model_artifact_id.0,
                file: "model.safetensors".to_string(),
                expected: "registered non-zero hash".to_string(),
                actual: "zero hash".to_string(),
            });
        }
        let n = self.candidate_action_ids.len();
        if n == 0 {
            return Err(DynamicJepaError::validation(
                "PlanTraceRecord.candidate_action_ids",
                "plan must evaluate at least one candidate action",
                "enumerate the declared action set before planning",
            ));
        }
        if self.prediction_ids.len() != n || self.utility_scores.len() != n {
            return Err(DynamicJepaError::validation(
                "PlanTraceRecord",
                "candidate_action_ids, prediction_ids, and utility_scores lengths must match",
                "persist complete plan evidence in one batch",
            ));
        }
        for id in &self.candidate_action_ids {
            id.validate()?;
        }
        for id in &self.prediction_ids {
            id.validate()?;
        }
        for id in &self.guard_decision_ids {
            if id.is_nil() {
                return Err(DynamicJepaError::validation(
                    "PlanTraceRecord.guard_decision_ids",
                    "guard decision ids must not be nil",
                    "persist concrete guard decision records",
                ));
            }
        }
        for (idx, score) in self.utility_scores.iter().enumerate() {
            if !score.is_finite() {
                return Err(DynamicJepaError::validation(
                    format!("PlanTraceRecord.utility_scores[{idx}]"),
                    format!("utility score must be finite, got {score}"),
                    "abort planning on NaN or infinity",
                ));
            }
        }
        if let Some(selected) = self.selected_action_id {
            selected.validate()?;
            if !self.candidate_action_ids.contains(&selected) {
                return Err(DynamicJepaError::validation(
                    "PlanTraceRecord.selected_action_id",
                    "selected action is not in candidate_action_ids",
                    "select only from the persisted candidate set",
                ));
            }
        }
        if let Some(constellation_id) = self.constellation_uuid_used {
            constellation_id.validate()?;
        }
        if matches!(self.status, PlanTraceStatus::Selected) && self.selected_action_id.is_none() {
            return Err(DynamicJepaError::validation(
                "PlanTraceRecord.status",
                "Selected plan must carry selected_action_id",
                "write selected_action_id or mark plan Rejected/Failed",
            ));
        }
        if self.created_at_unix_ms < 0 {
            return Err(DynamicJepaError::validation(
                "PlanTraceRecord.created_at_unix_ms",
                "timestamp must be non-negative",
                "write Unix epoch milliseconds",
            ));
        }
        Ok(())
    }
}

impl Validate for SurpriseEventRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.surprise_event_id.validate()?;
        self.prediction_id.validate()?;
        self.observed_outcome_id.validate()?;
        self.observed_panel_id.validate()?;
        for (field, value) in [
            ("cosine", self.cosine),
            ("threshold", self.threshold),
            ("error_norm", self.error_norm),
        ] {
            if !value.is_finite() {
                return Err(DynamicJepaError::validation(
                    format!("SurpriseEventRecord.{field}"),
                    format!("{field} must be finite, got {value}"),
                    "record finite surprise metrics",
                ));
            }
        }
        if !(-1.0..=1.0).contains(&self.threshold) {
            return Err(DynamicJepaError::validation(
                "SurpriseEventRecord.threshold",
                "threshold must be in [-1,1]",
                "copy the artifact's calibrated cosine surprise threshold",
            ));
        }
        if self.created_at_unix_ms < 0 {
            return Err(DynamicJepaError::validation(
                "SurpriseEventRecord.created_at_unix_ms",
                "timestamp must be non-negative",
                "write Unix epoch milliseconds",
            ));
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(
    PlanTraceRecord,
    PLAN_TRACE_RECORD_VERSION,
    "PlanTraceRecord"
);
crate::impl_dynamic_jepa_record!(
    SurpriseEventRecord,
    SURPRISE_EVENT_RECORD_VERSION,
    "SurpriseEventRecord"
);
