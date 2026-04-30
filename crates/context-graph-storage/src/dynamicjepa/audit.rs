//! DynamicJEPA append-only audit records.

use context_graph_core::dynamicjepa::{DynamicJepaError, DynamicJepaResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DJ_AUDIT_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DjAuditRecord {
    pub audit_id: Uuid,
    pub timestamp_unix_nanos: u64,
    pub operation: String,
    pub actor: String,
    pub input_ids: Vec<String>,
    pub output_ids: Vec<String>,
    pub cfs_touched: Vec<String>,
    pub content_hashes: Vec<[u8; 32]>,
    pub status: AuditStatus,
    pub verification_run_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditStatus {
    Ok,
    Failed { error_code: String },
}

impl DjAuditRecord {
    pub fn validate(&self) -> DynamicJepaResult<()> {
        if self.audit_id.is_nil() {
            return Err(DynamicJepaError::validation(
                "DjAuditRecord.audit_id",
                "audit_id must not be nil",
                "generate a concrete audit id at the writer boundary",
            ));
        }
        if self.operation.trim().is_empty()
            || self.actor.trim().is_empty()
            || self.cfs_touched.is_empty()
        {
            return Err(DynamicJepaError::validation(
                "DjAuditRecord",
                "operation, actor, and cfs_touched must be populated",
                "write operator-visible audit context with every persisted operation",
            ));
        }
        if let AuditStatus::Failed { error_code } = &self.status {
            if error_code.trim().is_empty() {
                return Err(DynamicJepaError::validation(
                    "DjAuditRecord.status.error_code",
                    "failed audit status must carry an error_code",
                    "copy DynamicJepaError::code() into the audit row",
                ));
            }
        }
        Ok(())
    }
}
