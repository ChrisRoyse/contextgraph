use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{
    ActionId, AdapterId, DomainPackId, EventId, OutcomeId, StateId, TransitionId,
};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::{ensure_uuid, Validate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const ADAPTER_RUN_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterSpec {
    pub adapter_id: AdapterId,
    pub kind: String,
    pub version: u8,
    pub mapping: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterRunRecord {
    pub header: DjRecordHeader,
    pub adapter_run_id: Uuid,
    pub adapter_id: AdapterId,
    pub domain_pack_id: DomainPackId,
    pub event_id: EventId,
    pub started_at_unix_ms: i64,
    pub finished_at_unix_ms: Option<i64>,
    pub status: AdapterRunStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub field_path: Option<String>,
    pub expected_kind: Option<String>,
    pub actual_kind: Option<String>,
    pub output_state_id: Option<StateId>,
    pub output_action_id: Option<ActionId>,
    pub output_outcome_id: Option<OutcomeId>,
    pub output_transition_id: Option<TransitionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdapterRunStatus {
    Started,
    Completed,
    Failed,
}

impl Validate for AdapterSpec {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.adapter_id.validate()?;
        if self.kind != "json_event" {
            return Err(DynamicJepaError::validation(
                "AdapterSpec.kind",
                format!("unsupported adapter kind {:?}", self.kind),
                "Phase 1/5090 demo supports only json_event",
            ));
        }
        if self.version == 0 {
            return Err(DynamicJepaError::validation(
                "AdapterSpec.version",
                "adapter version must be >= 1",
                "set version=1 for json_event",
            ));
        }
        if self.mapping.is_empty() {
            return Err(DynamicJepaError::validation(
                "AdapterSpec.mapping",
                "adapter mapping must not be empty",
                "map every required domain field to a JSON path",
            ));
        }
        for (target, source) in &self.mapping {
            if !source.starts_with("$.") {
                return Err(DynamicJepaError::validation(
                    format!("AdapterSpec.mapping.{target}"),
                    format!("source path {source:?} is outside the supported $.a.b subset"),
                    "use dotted JSON paths beginning with $. and no wildcards",
                ));
            }
        }
        Ok(())
    }
}

impl Validate for AdapterRunRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        ensure_uuid(self.adapter_run_id, "AdapterRunRecord.adapter_run_id")?;
        self.adapter_id.validate()?;
        self.domain_pack_id.validate()?;
        self.event_id.validate()?;
        if self.started_at_unix_ms < 0 {
            return Err(DynamicJepaError::validation(
                "AdapterRunRecord.started_at_unix_ms",
                "timestamp must be non-negative",
                "write Unix epoch milliseconds",
            ));
        }
        if let Some(finished) = self.finished_at_unix_ms {
            if finished < self.started_at_unix_ms {
                return Err(DynamicJepaError::validation(
                    "AdapterRunRecord.finished_at_unix_ms",
                    "finished timestamp precedes started timestamp",
                    "write monotonic adapter run timestamps",
                ));
            }
        }
        match self.status {
            AdapterRunStatus::Completed => {
                if self.output_state_id.is_none()
                    || self.output_action_id.is_none()
                    || self.output_outcome_id.is_none()
                    || self.output_transition_id.is_none()
                {
                    return Err(DynamicJepaError::validation(
                        "AdapterRunRecord.status",
                        "completed adapter run must reference all normalized outputs",
                        "write state/action/outcome/transition ids in the same batch",
                    ));
                }
            }
            AdapterRunStatus::Failed => {
                if self.error_code.is_none() || self.error_message.is_none() {
                    return Err(DynamicJepaError::validation(
                        "AdapterRunRecord.status",
                        "failed adapter run must carry error_code and error_message",
                        "persist the structured adapter failure instead of dropping context",
                    ));
                }
            }
            AdapterRunStatus::Started => {}
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(
    AdapterRunRecord,
    ADAPTER_RUN_RECORD_VERSION,
    "AdapterRunRecord"
);
