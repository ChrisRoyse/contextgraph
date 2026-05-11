pub mod chain;
pub mod invariant_scan;
pub mod writer;

use crate::learning_signal::LearningSignal;
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub const CF_MEJEPA_TRAIN_CERTS: &str = "CF_MEJEPA_TRAIN_CERTS";
pub const CF_MEJEPA_HOLDOUT_REPORTS: &str = "CF_MEJEPA_HOLDOUT_REPORTS";
pub const CF_MEJEPA_WEIGHT_BLOBS: &str = "CF_MEJEPA_WEIGHT_BLOBS";
pub const CF_MEJEPA_EPOCH_WITNESS: &str = "CF_MEJEPA_EPOCH_WITNESS";
pub const MEJEPA_TRAIN_CFS: &[&str] = &[
    CF_MEJEPA_TRAIN_CERTS,
    CF_MEJEPA_HOLDOUT_REPORTS,
    CF_MEJEPA_WEIGHT_BLOBS,
    CF_MEJEPA_EPOCH_WITNESS,
];

pub fn open_train_rocksdb(path: impl AsRef<Path>) -> Result<Arc<DB>, crate::error::TrainerError> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let descriptors = MEJEPA_TRAIN_CFS
        .iter()
        .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
        .collect::<Vec<_>>();
    let db = DB::open_cf_descriptors(&opts, path.as_ref(), descriptors)?;
    for cf in MEJEPA_TRAIN_CFS {
        if db.cf_handle(cf).is_none() {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("missing training column family {cf} after open"),
            ));
        }
    }
    Ok(Arc::new(db))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingCertificate {
    pub step: u64,
    pub epoch: u32,
    pub signal: LearningSignal,
    pub loss_components: HashMap<String, f32>,
    pub training_mode: String,
    pub distillation_cycle_id: Option<u64>,
    pub distillation_loss_mean: Option<f32>,
    pub distillation_skipped_count: Option<u64>,
    pub counterfactual_smoothness: Option<f32>,
    pub counterfactual_smoothness_anomaly: Option<bool>,
    pub cross_task_transfer_indices: Vec<usize>,
    pub cross_task_transfer_fallback_count: u64,
    pub predictor_redundancy_pairwise_mi: f32,
    pub predictor_redundancy_pairwise_mi_source: String,
    pub holdout_promotion: Option<bool>,
    pub generic_only_warning: Option<String>,
    pub phase3_dod_passed: Option<bool>,
    pub grad_norm_running_mean: f32,
    pub parent_witness_hash: String,
    pub self_hash: String,
    pub merkle_root: String,
    pub code_version: String,
    pub corpus_sha: String,
    pub embedder_versions: HashMap<String, String>,
    pub frozen_at: String,
}

impl TrainingCertificate {
    pub fn validate_phase3(&self) -> Result<(), crate::error::TrainerError> {
        self.signal.validate()?;
        if self.signal.delta_xi_components.target_collapse != 0.0 {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaInstrumentGradientLeak,
                format!(
                    "target_collapse={} at step {}",
                    self.signal.delta_xi_components.target_collapse, self.step
                ),
            )
            .with_step(self.step));
        }
        if !self.predictor_redundancy_pairwise_mi.is_finite()
            || !(0.0..=1.0).contains(&self.predictor_redundancy_pairwise_mi)
        {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                format!(
                    "predictor_redundancy_pairwise_mi={} at step {} must be finite in [0,1]",
                    self.predictor_redundancy_pairwise_mi, self.step
                ),
            )
            .with_step(self.step));
        }
        if self
            .predictor_redundancy_pairwise_mi_source
            .trim()
            .is_empty()
        {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                format!(
                    "predictor_redundancy_pairwise_mi_source is empty at step {}",
                    self.step
                ),
            )
            .with_step(self.step));
        }
        if (self.predictor_redundancy_pairwise_mi
            - self.signal.delta_xi_components.predictor_redundancy)
            .abs()
            > 1e-6
        {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                format!(
                    "top-level pairwise MI {} diverges from signal ΔΞ predictor_redundancy {} at step {}",
                    self.predictor_redundancy_pairwise_mi,
                    self.signal.delta_xi_components.predictor_redundancy,
                    self.step
                ),
            )
            .with_step(self.step));
        }
        for (key, value) in &self.loss_components {
            if !value.is_finite() {
                return Err(crate::error::TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainLossNan,
                    format!("loss component {key} is non-finite"),
                )
                .with_step(self.step));
            }
        }
        Ok(())
    }
}

pub use chain::{
    body_canonical_json, compute_merkle_root, compute_parent_hash, compute_self_hash, verify_chain,
    ChainVerificationReport, GENESIS_PARENT_HASH,
};
pub use invariant_scan::scan_target_collapse_invariant;
pub use writer::TrainCertWriter;
