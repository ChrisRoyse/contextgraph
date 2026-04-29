use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{BindingId, DomainPackId};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::{ensure_finite, Validate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const BINDING_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BindingRecord {
    pub header: DjRecordHeader,
    pub binding_id: BindingId,
    pub binding_kind: BindingKind,
    pub left_ref: BindingRef,
    pub right_ref: BindingRef,
    pub evidence_refs: Vec<BindingRef>,
    pub score: f32,
    pub method: BindingMethod,
    pub left_domain_pack_id: DomainPackId,
    pub right_domain_pack_id: DomainPackId,
    pub created_by_run_id: Uuid,
    pub version: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingKind {
    EntityToEntity,
    EventToTrajectory,
    StateToGoal,
    PanelToMemory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingRef {
    pub cf: String,
    pub key_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingMethod {
    IdEquality,
    ExplicitMapping,
}

impl Validate for BindingRef {
    fn validate(&self) -> DynamicJepaResult<()> {
        if self.cf.trim().is_empty() || self.key_bytes.is_empty() {
            return Err(DynamicJepaError::validation(
                "BindingRef",
                "cf and key_bytes must be non-empty",
                "bind only concrete source-of-truth records",
            ));
        }
        Ok(())
    }
}

impl Validate for BindingRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.binding_id.validate()?;
        self.left_ref.validate()?;
        self.right_ref.validate()?;
        if self.evidence_refs.is_empty() {
            return Err(DynamicJepaError::BindingEvidenceMissing {
                binding_id: self.binding_id.0,
                evidence_ref: "evidence_refs".to_string(),
            });
        }
        for evidence in &self.evidence_refs {
            evidence.validate()?;
        }
        ensure_finite(self.score, "BindingRecord.score")?;
        if !(0.0..=1.0).contains(&self.score) {
            return Err(DynamicJepaError::validation(
                "BindingRecord.score",
                format!("score must be in [0,1], got {}", self.score),
                "write a calibrated binding score",
            ));
        }
        self.left_domain_pack_id.validate()?;
        self.right_domain_pack_id.validate()?;
        if self.created_by_run_id.is_nil() {
            return Err(DynamicJepaError::validation(
                "BindingRecord.created_by_run_id",
                "created_by_run_id must not be nil",
                "persist the run id that created this binding",
            ));
        }
        if self.version == 0 {
            return Err(DynamicJepaError::validation(
                "BindingRecord.version",
                "binding version must be >= 1",
                "set binding version=1",
            ));
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(BindingRecord, BINDING_RECORD_VERSION, "BindingRecord");
