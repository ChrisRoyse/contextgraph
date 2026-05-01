use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{ActionId, ConstellationId, GuardDecisionId, InstrumentId};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::{ensure_finite, Validate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const GUARD_DECISION_RECORD_VERSION: u8 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuardDecisionRecord {
    pub header: DjRecordHeader,
    pub guard_decision_id: GuardDecisionId,
    pub plan_trace_id: Uuid,
    pub guard_id: String,
    pub candidate_action_id: ActionId,
    pub decision: GuardDecision,
    pub evidence_refs: Vec<String>,
    pub threshold_values: BTreeMap<String, f64>,
    #[serde(default)]
    pub utility_score: Option<f32>,
    #[serde(default)]
    pub utility_decision: Option<GuardDecision>,
    #[serde(default)]
    pub gtau_decision: Option<GuardDecision>,
    #[serde(default)]
    pub constellation_uuid: Option<ConstellationId>,
    #[serde(default)]
    pub cosine_to_centroid_per_modality: Option<BTreeMap<InstrumentId, f32>>,
    #[serde(default)]
    pub tau_per_modality: Option<BTreeMap<InstrumentId, f32>>,
    #[serde(default)]
    pub gtau_failed_modalities: Option<Vec<InstrumentId>>,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuardDecision {
    Allow,
    Reject {
        reason_code: String,
        reason_message: String,
    },
}

impl Validate for GuardDecisionRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.guard_decision_id.validate()?;
        if self.plan_trace_id.is_nil() {
            return Err(DynamicJepaError::validation(
                "GuardDecisionRecord.plan_trace_id",
                "plan_trace_id must not be nil",
                "link guard decisions to a concrete plan trace",
            ));
        }
        if self.guard_id.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                "GuardDecisionRecord.guard_id",
                "guard_id must not be empty",
                "record the guard that produced the decision",
            ));
        }
        self.candidate_action_id.validate()?;
        if let GuardDecision::Reject {
            reason_code,
            reason_message,
        } = &self.decision
        {
            if reason_code.trim().is_empty() || reason_message.trim().is_empty() {
                return Err(DynamicJepaError::GuardRejected {
                    guard_id: self.guard_id.clone(),
                    reason_code: reason_code.clone(),
                    action_id: self.candidate_action_id.0,
                });
            }
        }
        for (name, value) in &self.threshold_values {
            if name.trim().is_empty() || !value.is_finite() {
                return Err(DynamicJepaError::validation(
                    "GuardDecisionRecord.threshold_values",
                    format!("invalid threshold {name:?}={value}"),
                    "record finite guard thresholds",
                ));
            }
        }
        if let Some(score) = self.utility_score {
            ensure_finite(score, "GuardDecisionRecord.utility_score")?;
        }
        if let Some(decision) = &self.utility_decision {
            validate_guard_decision(decision, "GuardDecisionRecord.utility_decision")?;
        }
        if let Some(decision) = &self.gtau_decision {
            validate_guard_decision(decision, "GuardDecisionRecord.gtau_decision")?;
        }
        if let Some(id) = self.constellation_uuid {
            id.validate()?;
        }
        if let Some(cosines) = &self.cosine_to_centroid_per_modality {
            validate_modality_score_map(
                cosines,
                "GuardDecisionRecord.cosine_to_centroid_per_modality",
                true,
            )?;
        }
        if let Some(taus) = &self.tau_per_modality {
            validate_modality_score_map(taus, "GuardDecisionRecord.tau_per_modality", false)?;
        }
        if let Some(failed) = &self.gtau_failed_modalities {
            for id in failed {
                id.validate()?;
            }
        }
        if self.created_at_unix_ms < 0 {
            return Err(DynamicJepaError::validation(
                "GuardDecisionRecord.created_at_unix_ms",
                "timestamp must be non-negative",
                "write Unix epoch milliseconds",
            ));
        }
        Ok(())
    }
}

fn validate_guard_decision(decision: &GuardDecision, field: &str) -> DynamicJepaResult<()> {
    if let GuardDecision::Reject {
        reason_code,
        reason_message,
    } = decision
    {
        if reason_code.trim().is_empty() || reason_message.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                field,
                "reject decisions must include reason_code and reason_message",
                "write operator-visible guard rejection details",
            ));
        }
    }
    Ok(())
}

fn validate_modality_score_map(
    values: &BTreeMap<InstrumentId, f32>,
    field: &str,
    enforce_cosine_range: bool,
) -> DynamicJepaResult<()> {
    if values.is_empty() {
        return Err(DynamicJepaError::validation(
            field,
            "modality score map must not be empty when present",
            "write None or at least one modality score",
        ));
    }
    for (instrument_id, value) in values {
        instrument_id.validate()?;
        ensure_finite(*value, &format!("{field}.{instrument_id}"))?;
        if enforce_cosine_range && !(-1.0..=1.0).contains(value) {
            return Err(DynamicJepaError::validation(
                field,
                format!("cosine score for {instrument_id} must be in [-1,1], got {value}"),
                "store normalized cosine-to-centroid scores",
            ));
        }
    }
    Ok(())
}

crate::impl_dynamic_jepa_record!(
    GuardDecisionRecord,
    GUARD_DECISION_RECORD_VERSION,
    "GuardDecisionRecord"
);
