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
pub const CF_MEJEPA_SAMPLER_REWARDS: &str = context_graph_mejepa_cf::CF_MEJEPA_SAMPLER_REWARDS;
pub const MEJEPA_TRAIN_CFS: &[&str] = &[
    CF_MEJEPA_TRAIN_CERTS,
    CF_MEJEPA_HOLDOUT_REPORTS,
    CF_MEJEPA_WEIGHT_BLOBS,
    CF_MEJEPA_EPOCH_WITNESS,
    CF_MEJEPA_SAMPLER_REWARDS,
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
    pub per_head_l_step: HashMap<String, f32>,
    pub delta_xi_global_min: f32,
    pub loss_components: HashMap<String, f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditional_description_length_bits: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverse_map_quality_bits: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l_entropy_nats: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l_entropy_weighted: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l_entropy_lambda: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l_entropy_sample_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l_entropy_estimator: Option<String>,
    pub training_mode: String,
    #[serde(default)]
    pub trained_predictor: bool,
    #[serde(default)]
    pub predictor_parameter_update_count: u64,
    #[serde(default)]
    pub checkpoint_readback_verified: bool,
    #[serde(default)]
    pub ship_gate_countable_training: bool,
    pub distillation_cycle_id: Option<u64>,
    pub distillation_loss_mean: Option<f32>,
    pub distillation_skipped_count: Option<u64>,
    pub counterfactual_smoothness: Option<f32>,
    pub counterfactual_smoothness_anomaly: Option<bool>,
    pub adversarial_mix_target_ratio: f32,
    pub adversarial_mix_count: usize,
    pub adversarial_mix_example_indices: Vec<usize>,
    pub adversarial_mix_fallback_count: u64,
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
        if !self.trained_predictor {
            if self.predictor_parameter_update_count != 0 {
                return Err(crate::error::TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                    format!(
                        "certificate step {} has trained_predictor=false but predictor_parameter_update_count={}",
                        self.step, self.predictor_parameter_update_count
                    ),
                )
                .with_step(self.step));
            }
            if self.checkpoint_readback_verified {
                return Err(crate::error::TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                    format!(
                        "certificate step {} has trained_predictor=false but checkpoint_readback_verified=true",
                        self.step
                    ),
                )
                .with_step(self.step));
            }
            if self.ship_gate_countable_training {
                return Err(crate::error::TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                    format!(
                        "certificate step {} has trained_predictor=false but ship_gate_countable_training=true",
                        self.step
                    ),
                )
                .with_step(self.step));
            }
        }
        if self.ship_gate_countable_training
            && (!self.trained_predictor
                || self.predictor_parameter_update_count == 0
                || !self.checkpoint_readback_verified)
        {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                format!(
                    "certificate step {} is ship-gate countable without trained predictor, optimizer updates, and verified checkpoint readback",
                    self.step
                ),
            )
            .with_step(self.step));
        }
        if !self.delta_xi_global_min.is_finite() || !(0.0..=1.0).contains(&self.delta_xi_global_min)
        {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                format!(
                    "delta_xi_global_min={} at step {} must be finite in [0,1]",
                    self.delta_xi_global_min, self.step
                ),
            )
            .with_step(self.step));
        }
        if !self.adversarial_mix_target_ratio.is_finite()
            || !(0.0..=1.0).contains(&self.adversarial_mix_target_ratio)
        {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                format!(
                    "adversarial_mix_target_ratio={} at step {} must be finite in [0,1]",
                    self.adversarial_mix_target_ratio, self.step
                ),
            )
            .with_step(self.step));
        }
        if self.adversarial_mix_count != self.adversarial_mix_example_indices.len() {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                format!(
                    "adversarial_mix_count={} at step {} does not match {} indices",
                    self.adversarial_mix_count,
                    self.step,
                    self.adversarial_mix_example_indices.len()
                ),
            )
            .with_step(self.step));
        }
        for head in context_graph_mejepa::HeadId::ALL {
            let Some(value) = self.per_head_l_step.get(head.as_str()) else {
                return Err(crate::error::TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                    format!(
                        "per_head_l_step missing {} at step {}",
                        head.as_str(),
                        self.step
                    ),
                )
                .with_step(self.step));
            };
            if !value.is_finite() || !(0.0..=1.0).contains(value) {
                return Err(crate::error::TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                    format!(
                        "per_head_l_step[{}]={} at step {} must be finite in [0,1]",
                        head.as_str(),
                        value,
                        self.step
                    ),
                )
                .with_step(self.step));
            }
        }
        if self.per_head_l_step.len() != context_graph_mejepa::HeadId::ALL.len() {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                format!(
                    "per_head_l_step has {} entries at step {}; expected {}",
                    self.per_head_l_step.len(),
                    self.step,
                    context_graph_mejepa::HeadId::ALL.len()
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
        if let Some(bits) = self.conditional_description_length_bits {
            if !bits.is_finite() || bits < 0.0 {
                return Err(crate::error::TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                    format!(
                        "conditional_description_length_bits={} at step {} must be finite and non-negative",
                        bits, self.step
                    ),
                )
                .with_step(self.step));
            }
        }
        if let Some(bits) = self.inverse_map_quality_bits {
            if !bits.is_finite() || bits < 0.0 {
                return Err(crate::error::TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                    format!(
                        "inverse_map_quality_bits={} at step {} must be finite and non-negative",
                        bits, self.step
                    ),
                )
                .with_step(self.step));
            }
        }
        if let Some(value) = self.l_entropy_nats {
            if !value.is_finite() || value < 0.0 {
                return Err(crate::error::TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                    format!(
                        "l_entropy_nats={} at step {} must be finite and non-negative",
                        value, self.step
                    ),
                )
                .with_step(self.step));
            }
        }
        if let Some(value) = self.l_entropy_weighted {
            if !value.is_finite() || value < 0.0 {
                return Err(crate::error::TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                    format!(
                        "l_entropy_weighted={} at step {} must be finite and non-negative",
                        value, self.step
                    ),
                )
                .with_step(self.step));
            }
        }
        if let Some(value) = self.l_entropy_lambda {
            if !value.is_finite() || value < 0.0 {
                return Err(crate::error::TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                    format!(
                        "l_entropy_lambda={} at step {} must be finite and non-negative",
                        value, self.step
                    ),
                )
                .with_step(self.step));
            }
        }
        if matches!(self.l_entropy_sample_count, Some(0)) {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                format!("l_entropy_sample_count=0 at step {}", self.step),
            )
            .with_step(self.step));
        }
        if self.l_entropy_nats.is_some() && !self.loss_components.contains_key("entropy") {
            return Err(crate::error::TrainerError::new(
                crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                format!(
                    "l_entropy_nats present but loss_components.entropy missing at step {}",
                    self.step
                ),
            )
            .with_step(self.step));
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
