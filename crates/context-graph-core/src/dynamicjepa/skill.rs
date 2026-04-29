use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{DomainPackId, SkillId};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::Validate;
use serde::{Deserialize, Serialize};

pub const SKILL_POLICY_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPolicyRecord {
    pub header: DjRecordHeader,
    pub skill_id: SkillId,
    pub domain_pack_id: DomainPackId,
    pub skill_name: String,
    pub strategy: SkillStrategy,
    pub version: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillStrategy {
    EnumerateDeclaredActions,
}

impl Validate for SkillPolicyRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.skill_id.validate()?;
        self.domain_pack_id.validate()?;
        if self.skill_name.trim().is_empty() || self.version == 0 {
            return Err(DynamicJepaError::validation(
                "SkillPolicyRecord",
                "skill_name must be non-empty and version >= 1",
                "persist a named skill policy such as enumerate_declared_actions",
            ));
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(
    SkillPolicyRecord,
    SKILL_POLICY_RECORD_VERSION,
    "SkillPolicyRecord"
);
