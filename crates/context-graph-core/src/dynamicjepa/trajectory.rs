use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{BindingId, PanelId, TrajectoryId, TransitionId};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::Validate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const TRAJECTORY_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrajectoryRecord {
    pub header: DjRecordHeader,
    pub trajectory_id: TrajectoryId,
    pub segmentation_policy_id: String,
    pub ordered_transition_ids: Vec<TransitionId>,
    pub ordered_panel_ids: Vec<PanelId>,
    pub start_time_unix_ms: i64,
    pub end_time_unix_ms: i64,
    pub entity_refs: Vec<String>,
    pub binding_refs: Vec<BindingId>,
    pub trajectory_hash: [u8; 32],
    pub record_count: u32,
}

impl Validate for TrajectoryRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.trajectory_id.validate()?;
        if self.segmentation_policy_id.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                "TrajectoryRecord.segmentation_policy_id",
                "segmentation policy must not be empty",
                "use by_domain_session for the demo",
            ));
        }
        if self.ordered_transition_ids.is_empty() || self.ordered_panel_ids.is_empty() {
            return Err(DynamicJepaError::TrajectoryInvariantViolation {
                message: "trajectory must reference transitions and panels".to_string(),
                trajectory_id: self.trajectory_id.0,
            });
        }
        if self.ordered_transition_ids.len() != self.ordered_panel_ids.len() {
            return Err(DynamicJepaError::TrajectoryInvariantViolation {
                message: format!(
                    "transition count {} != panel count {}",
                    self.ordered_transition_ids.len(),
                    self.ordered_panel_ids.len()
                ),
                trajectory_id: self.trajectory_id.0,
            });
        }
        let mut seen = BTreeSet::new();
        for id in &self.ordered_transition_ids {
            id.validate()?;
            if !seen.insert(id.0) {
                return Err(DynamicJepaError::TrajectoryInvariantViolation {
                    message: format!("duplicate transition id {}", id.0),
                    trajectory_id: self.trajectory_id.0,
                });
            }
        }
        for id in &self.ordered_panel_ids {
            id.validate()?;
        }
        if self.start_time_unix_ms > self.end_time_unix_ms {
            return Err(DynamicJepaError::TrajectoryInvariantViolation {
                message: "start_time_unix_ms exceeds end_time_unix_ms".to_string(),
                trajectory_id: self.trajectory_id.0,
            });
        }
        if self.record_count as usize != self.ordered_transition_ids.len() {
            return Err(DynamicJepaError::TrajectoryInvariantViolation {
                message: "record_count does not match transition count".to_string(),
                trajectory_id: self.trajectory_id.0,
            });
        }
        if self.trajectory_hash == [0; 32] {
            return Err(DynamicJepaError::TrajectoryInvariantViolation {
                message: "trajectory_hash must be computed".to_string(),
                trajectory_id: self.trajectory_id.0,
            });
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(
    TrajectoryRecord,
    TRAJECTORY_RECORD_VERSION,
    "TrajectoryRecord"
);
