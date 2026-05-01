//! DynamicJEPA append-only audit records.

use context_graph_core::dynamicjepa::{DynamicJepaError, DynamicJepaResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DJ_AUDIT_RECORD_VERSION: u8 = 2;

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
    pub signal_yield: u32,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalYieldDimensions {
    pub n_panel_slots: u32,
    pub n_active_modalities: u32,
    pub k_candidates: u32,
}

pub fn signal_yield_for_operation(
    operation_kind: &str,
    dimensions: SignalYieldDimensions,
) -> DynamicJepaResult<u32> {
    let n = dimensions.n_panel_slots;
    let m = dimensions.n_active_modalities;
    let k = dimensions.k_candidates;
    match operation_kind {
        "ingest_event" => Ok(1),
        "run_adapter_success" => Ok(1),
        "run_adapter_failure" => Ok(0),
        "materialize_panel" => Ok(n),
        "materialize_pairwise" => {
            let pairs =
                n.checked_mul(n.checked_sub(1).ok_or_else(|| {
                    DynamicJepaError::validation(
                        "SignalYieldDimensions.n_panel_slots",
                        "n_panel_slots underflow while computing pairwise signal yield",
                        "use a valid panel slot count before writing signal yield",
                    )
                })?)
                .ok_or_else(|| {
                    DynamicJepaError::validation(
                        "SignalYieldDimensions.n_panel_slots",
                        "n_panel_slots overflow while computing pairwise signal yield",
                        "use realistic bounded panel dimensions",
                    )
                })? / 2;
            Ok(pairs)
        }
        "compile_transition" => Ok(2),
        "compile_trajectories" => Ok(0),
        "compile_dataset" => Ok(0),
        "train_predictor" => Ok(0),
        "register_artifact" => Ok(0),
        "predict" => n.checked_add(m).ok_or_else(|| {
            DynamicJepaError::validation(
                "SignalYieldDimensions.predict",
                "panel slot and modality counts overflowed signal yield",
                "use realistic bounded panel dimensions",
            )
        }),
        "plan" => {
            let per_candidate = n
                .checked_add(m)
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| {
                    DynamicJepaError::validation(
                        "SignalYieldDimensions.plan",
                        "plan per-candidate signal count overflowed",
                        "use realistic bounded panel dimensions",
                    )
                })?;
            k.checked_mul(per_candidate).ok_or_else(|| {
                DynamicJepaError::validation(
                    "SignalYieldDimensions.plan",
                    "k_candidates overflowed plan signal yield",
                    "use realistic bounded planning dimensions",
                )
            })
        }
        "record_surprise" => Ok(1),
        "verify_artifact_files" => Ok(0),
        "verify_counter_world" => Ok(0),
        "verify_gridworld" => Ok(0),
        "verify_career_taxonomy" => Ok(0),
        "research_smoke" => Ok(0),
        "build_constellation" => Ok(0),
        "calibrate_threshold" => Ok(0),
        "recalibrate_threshold" => Ok(0),
        "audit_pairwise_mi" => Ok(0),
        "compute_mc_ratio" => Ok(0),
        "inspect_counts" => Ok(0),
        "inspect_cf" => Ok(0),
        "register_domain_pack" => Ok(0),
        "bind" => Ok(1),
        "verification_run" => Ok(0),
        other => Err(DynamicJepaError::SignalYieldUnknownOperation {
            operation: other.to_string(),
        }),
    }
}
