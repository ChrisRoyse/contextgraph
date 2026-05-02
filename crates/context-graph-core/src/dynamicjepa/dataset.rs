use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{ActionId, DatasetId, DatasetShardId, PanelId, TrajectoryId};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::Validate;
use serde::{Deserialize, Serialize};

pub const DATASET_SHARD_RECORD_VERSION: u8 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetShardRecord {
    pub header: DjRecordHeader,
    pub dataset_id: DatasetId,
    pub shard_id: DatasetShardId,
    pub source_trajectory_ids: Vec<TrajectoryId>,
    pub split_name: String,
    pub row_count: u32,
    pub input_panel_ids: Vec<PanelId>,
    pub target_panel_ids: Vec<PanelId>,
    pub action_ids: Vec<ActionId>,
    pub negative_panel_ids: Vec<PanelId>,
    pub objective_ids: Vec<String>,
    pub shape_summary: ShapeSummary,
    pub source_hashes: Vec<[u8; 32]>,
    pub leakage_report: LeakageReport,
    pub compiler_version: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShapeSummary {
    pub input_dim: usize,
    pub target_dim: usize,
    pub action_dim: usize,
    pub n_train_rows: u32,
    pub n_val_rows: u32,
    pub n_test_rows: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeakageReport {
    pub future_in_input_count: u64,
    pub same_panel_input_target_count: u64,
    pub negative_equals_target_count: u64,
    pub negative_feature_equals_target_count: u64,
    pub split_overlap_count: u64,
}

impl Validate for ShapeSummary {
    fn validate(&self) -> DynamicJepaResult<()> {
        if self.input_dim == 0 || self.target_dim == 0 || self.action_dim == 0 {
            return Err(DynamicJepaError::validation(
                "ShapeSummary",
                "input_dim, target_dim, and action_dim must be positive",
                "derive shape summary from actual panel/action slots",
            ));
        }
        Ok(())
    }
}

impl Validate for LeakageReport {
    fn validate(&self) -> DynamicJepaResult<()> {
        if self.future_in_input_count != 0
            || self.same_panel_input_target_count != 0
            || self.negative_equals_target_count != 0
            || self.negative_feature_equals_target_count != 0
            || self.split_overlap_count != 0
        {
            return Err(DynamicJepaError::DatasetLeakageDetected {
                message: format!("non-zero leakage report: {:?}", self),
                dataset_id: uuid::Uuid::nil(),
            });
        }
        Ok(())
    }
}

impl Validate for DatasetShardRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.dataset_id.validate()?;
        self.shard_id.validate()?;
        if !matches!(self.split_name.as_str(), "train" | "val" | "test") {
            return Err(DynamicJepaError::validation(
                "DatasetShardRecord.split_name",
                format!("unsupported split {:?}", self.split_name),
                "use train, val, or test",
            ));
        }
        let row_count = self.row_count as usize;
        let lengths = [
            self.input_panel_ids.len(),
            self.target_panel_ids.len(),
            self.action_ids.len(),
            self.negative_panel_ids.len(),
        ];
        if lengths.iter().any(|len| *len != row_count) {
            return Err(DynamicJepaError::DatasetLeakageDetected {
                message: format!(
                    "row_count {row_count} does not match row vector lengths {lengths:?}"
                ),
                dataset_id: self.dataset_id.0,
            });
        }
        if self.row_count == 0 {
            return Err(DynamicJepaError::DatasetLeakageDetected {
                message: "dataset shard row_count must be > 0".to_string(),
                dataset_id: self.dataset_id.0,
            });
        }
        for id in &self.source_trajectory_ids {
            id.validate()?;
        }
        for id in &self.input_panel_ids {
            id.validate()?;
        }
        for id in &self.target_panel_ids {
            id.validate()?;
        }
        for id in &self.action_ids {
            id.validate()?;
        }
        for id in &self.negative_panel_ids {
            id.validate()?;
        }
        if self.objective_ids.is_empty() {
            return Err(DynamicJepaError::validation(
                "DatasetShardRecord.objective_ids",
                "dataset shard must reference at least one objective",
                "copy objective ids from the registered domain pack",
            ));
        }
        self.shape_summary.validate()?;
        self.leakage_report.validate()?;
        if self.compiler_version == 0 {
            return Err(DynamicJepaError::validation(
                "DatasetShardRecord.compiler_version",
                "compiler_version must be >= 1",
                "set the current dataset compiler version",
            ));
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(
    DatasetShardRecord,
    DATASET_SHARD_RECORD_VERSION,
    "DatasetShardRecord"
);
