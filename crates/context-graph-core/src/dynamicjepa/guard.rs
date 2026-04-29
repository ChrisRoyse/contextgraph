use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{ActionId, GuardDecisionId};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::Validate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const GUARD_DECISION_RECORD_VERSION: u8 = 1;

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

crate::impl_dynamic_jepa_record!(
    GuardDecisionRecord,
    GUARD_DECISION_RECORD_VERSION,
    "GuardDecisionRecord"
);
