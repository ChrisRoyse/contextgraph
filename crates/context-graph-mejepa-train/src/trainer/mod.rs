use crate::aux::AuxHeadEnsemble;
use crate::cert::{
    TrainCertWriter, TrainingCertificate, CF_MEJEPA_EPOCH_WITNESS, CF_MEJEPA_HOLDOUT_REPORTS,
    CF_MEJEPA_TRAIN_CERTS,
};
use crate::config::TrainingConfig;
use crate::error::{TrainerError, TrainerErrorCode};
use crate::eval::holdout::{CalibrationDataset, HoldoutDataset, TrainSplit};
use crate::eval::{
    EpochReport, EpochSummary, EpochWitnessChain, HoldoutEvaluator, HoldoutReport, Lang,
    MutationCategory,
};
use crate::learning_signal::{
    compute_l_step, compute_per_head_learning_signal, pairwise_mi_audit, DeltaKComponents,
    DeltaOmegaComponents, DeltaPAggregator, DeltaPComponents, DeltaXiComponents, HeadSignalInput,
};
use crate::loss::entropy::{latent_entropy_loss, LatentEntropyConfig, LatentEntropyLossReport};
use crate::loss::inverse::{inverse_map_loss, InverseMapOutputs, InverseMapTargets};
use crate::sampler::{BatchSampler, PatchSimilarityGraph};
use candle_core::{DType, Device, Tensor};
use context_graph_mejepa::{HeadId, MeJepaPredictor, INVERSE_ACTION_DIM};
use ed25519_dalek::SigningKey;
use rocksdb::DB;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

pub const DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE: &str =
    "diagnostic_certificate_only_no_parameter_update";
pub const DIAGNOSTIC_CERTIFICATE_ONLY_WARNING: &str =
    "MEJEPA_TRAIN_CERTIFICATE_ONLY: Trainer emits UTML/certificate diagnostics only; no predictor parameters, optimizer step, holdout agreement, or checkpoint readback were produced";

pub struct Trainer {
    pub config: TrainingConfig,
    pub aux_heads: AuxHeadEnsemble,
    pub sampler: BatchSampler,
    pub cert_writer: TrainCertWriter,
    pub holdout_evaluator: HoldoutEvaluator,
    pub epoch_witness: EpochWitnessChain,
    pub rocksdb: Arc<DB>,
    pub step: u64,
    pub best_holdout_agreement: f32,
    pub grad_norm_running_mean: f32,
    pub device: Device,
    pub dtype: DType,
    abort_requested: bool,
}

#[derive(Debug, Clone)]
pub struct TrainingDataset {
    pub train: TrainSplit,
    pub calibration: CalibrationDataset,
    pub holdout: HoldoutDataset,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TrainingResult {
    pub final_holdout_agreement: Option<f32>,
    pub total_walltime_seconds: u64,
    pub total_steps: u64,
    pub total_certs: u64,
    pub best_safetensors_path: Option<PathBuf>,
    pub training_semantics: String,
    pub trained_predictor: bool,
    pub predictor_parameter_update_count: u64,
    pub checkpoint_readback_verified: bool,
    pub ship_gate_countable_training: bool,
    pub non_training_reason: String,
}

pub trait InverseMapForward {
    fn predict_inverse(&self, target_panel: &Tensor) -> Result<InverseMapOutputs, TrainerError>;
}

impl InverseMapForward for MeJepaPredictor {
    fn predict_inverse(&self, target_panel: &Tensor) -> Result<InverseMapOutputs, TrainerError> {
        let prediction = self.forward_inverse_dryrun(target_panel).map_err(|err| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("real inverse-head forward failed: {err}"),
            )
            .with_context(json!({
                "predictor_error_code": err.code(),
                "target_panel_shape": target_panel.dims(),
                "remediation": "inspect predictor inverse target/input/action projections and target panel dimensionality"
            }))
        })?;
        Ok(InverseMapOutputs {
            predicted_input_panel: prediction
                .predicted_input_panel
                .tensor
                .to_dtype(DType::F32)?,
            predicted_action: prediction.predicted_action.to_dtype(DType::F32)?,
        })
    }
}

impl Trainer {
    pub fn new(
        config: TrainingConfig,
        rocksdb: Arc<DB>,
        device: Device,
    ) -> Result<Self, TrainerError> {
        config.validate()?;
        let sampler = BatchSampler::new(
            config.clone(),
            vec![0.5],
            vec![false],
            vec![0.0],
            vec!["bootstrap".to_string()],
            PatchSimilarityGraph {
                neighbors: vec![Vec::new()],
            },
        )?;
        let code_version =
            option_env!("GIT_HASH").unwrap_or("0000000000000000000000000000000000000000");
        let cert_writer = TrainCertWriter::new(
            rocksdb.clone(),
            CF_MEJEPA_TRAIN_CERTS.to_string(),
            code_version.to_string(),
            "phase3-synthetic-corpus-sha".to_string(),
            default_embedder_versions(),
            chrono::Utc::now().to_rfc3339(),
        )?;
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        Ok(Self {
            config,
            aux_heads: AuxHeadEnsemble::default(),
            sampler,
            cert_writer,
            holdout_evaluator: HoldoutEvaluator::new(
                rocksdb.clone(),
                CF_MEJEPA_HOLDOUT_REPORTS.to_string(),
            ),
            epoch_witness: EpochWitnessChain::new(
                rocksdb.clone(),
                CF_MEJEPA_EPOCH_WITNESS.to_string(),
                signing_key,
            ),
            rocksdb,
            step: 0,
            best_holdout_agreement: 0.0,
            grad_norm_running_mean: 0.0,
            device,
            dtype: DType::F32,
            abort_requested: false,
        })
    }

    pub fn train_one_epoch(
        &mut self,
        dataset: &TrainingDataset,
        epoch: u32,
    ) -> Result<EpochReport, TrainerError> {
        self.train_one_epoch_inner(dataset, epoch, None)
    }

    pub fn train_one_epoch_with_inverse_head(
        &mut self,
        dataset: &TrainingDataset,
        epoch: u32,
        inverse_forward: &dyn InverseMapForward,
    ) -> Result<EpochReport, TrainerError> {
        self.train_one_epoch_inner(dataset, epoch, Some(inverse_forward))
    }

    fn train_one_epoch_inner(
        &mut self,
        dataset: &TrainingDataset,
        epoch: u32,
        inverse_forward: Option<&dyn InverseMapForward>,
    ) -> Result<EpochReport, TrainerError> {
        if dataset.train.examples.is_empty() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                "training split is empty",
            ));
        }
        self.rebuild_sampler(dataset)?;
        let start_step = self.step;
        let batches = dataset
            .train
            .examples
            .len()
            .div_ceil(self.config.batch_size);
        let mut l_sum = 0.0;
        let mut p_sum = 0.0;
        let mut k_sum = 0.0;
        let mut o_sum = 0.0;
        let mut x_sum = 0.0;
        for batch_idx in 0..batches {
            if self.abort_requested {
                break;
            }
            let plan = self.sampler.next_batch_with_cross_task(
                batch_idx % dataset.train.examples.len(),
                self.config.batch_size,
            )?;
            let predictor_redundancy_pairwise_mi =
                batch_pairwise_redundancy(dataset, &plan.indices)?;
            let hardness = (batch_idx as f32 + 1.0) / batches.max(1) as f32;
            let signal = compute_l_step(
                DeltaPComponents {
                    delta_p_real: hardness.min(1.0),
                    delta_p_imagined: None,
                    snr: 1.0,
                    exploration_bonus: 0.0,
                    gamma: 0.7,
                    aggregator: DeltaPAggregator::Mean,
                    per_chunk_values: vec![hardness.min(1.0)],
                },
                DeltaKComponents {
                    cos_align: if plan.force_count > 0 { 0.85 } else { 0.65 },
                    fisher_violation: 0.2,
                    ece: 0.1,
                    embedder_coherence: 0.5,
                    embedder_coherence_source: "bootstrap_neutral_n_lt_1000".to_string(),
                },
                DeltaOmegaComponents {
                    effective_plasticity: 0.9,
                    landscape_health: 0.85,
                    stability_floor: 1.0,
                    agent_state_score: 0.7,
                    agent_state_source: "default_neutral_no_transcript".to_string(),
                },
                DeltaXiComponents {
                    target_collapse: 0.0,
                    predictor_redundancy: predictor_redundancy_pairwise_mi,
                    constellation_violation_rate: 0.05,
                },
            )?;
            let mut per_head_inputs = BTreeMap::new();
            for head in HeadId::ALL {
                per_head_inputs.insert(
                    head,
                    HeadSignalInput {
                        delta_p: signal.delta_p_components.clone(),
                        delta_k: signal.delta_k_components.clone(),
                        delta_omega: signal.delta_omega_components.clone(),
                        delta_xi: signal.delta_xi_components,
                    },
                );
            }
            let per_head_signal = compute_per_head_learning_signal(per_head_inputs)?;
            l_sum += signal.l_step;
            p_sum += signal.delta_p;
            k_sum += signal.delta_k;
            o_sum += signal.delta_omega;
            x_sum += signal.delta_xi;
            let mut loss_components = HashMap::from([
                ("predict".to_string(), signal.delta_p),
                ("variance".to_string(), 0.01),
                ("covariance".to_string(), 0.01),
                ("invariance".to_string(), 0.0),
                ("predictor_parameter_update_count".to_string(), 0.0),
                ("checkpoint_readback_verified".to_string(), 0.0),
                ("ship_gate_countable_training".to_string(), 0.0),
            ]);
            let entropy_loss_report = batch_latent_entropy_loss(
                dataset,
                &plan.indices,
                &self.device,
                self.config.loss_coefficients.lambda_entropy,
            )?;
            if entropy_loss_report.enabled {
                loss_components.insert("entropy".to_string(), entropy_loss_report.entropy_nats);
                loss_components.insert(
                    "entropy_weighted".to_string(),
                    entropy_loss_report.weighted_loss,
                );
                loss_components.insert("entropy_lambda".to_string(), entropy_loss_report.lambda);
            }
            let conditional_description_length_bits =
                crate::compression_progress::conditional_description_length_bits_from_probability(
                    signal.delta_p,
                );
            let inverse_map_quality_bits = if self.config.inverse_map_coefficient > 0.0 {
                let inverse_forward = inverse_forward.ok_or_else(|| {
                    TrainerError::new(
                        TrainerErrorCode::MejepaTrainConfigInvalid,
                        "inverse_map_coefficient > 0 requires real inverse-head outputs",
                    )
                    .with_context(json!({
                        "inverse_map_coefficient": self.config.inverse_map_coefficient,
                        "required_call": "Trainer::train_one_epoch_with_inverse_head",
                        "remediation": "pass a predictor implementing InverseMapForward; synthetic target-as-output inverse losses are forbidden"
                    }))
                })?;
                let inverse_loss =
                    batch_inverse_map_loss(dataset, &plan.indices, &self.device, inverse_forward)?;
                loss_components.insert("inverse_map".to_string(), inverse_loss.loss);
                loss_components.insert(
                    "inverse_map_weighted".to_string(),
                    inverse_loss.loss * self.config.inverse_map_coefficient,
                );
                Some(inverse_loss.quality_bits)
            } else {
                None
            };
            let mut cert = TrainingCertificate {
                step: self.step,
                epoch,
                signal,
                per_head_l_step: per_head_signal.l_step_map().into_iter().collect(),
                delta_xi_global_min: per_head_signal.delta_xi_global_min,
                loss_components,
                conditional_description_length_bits: Some(conditional_description_length_bits),
                inverse_map_quality_bits,
                l_entropy_nats: entropy_loss_report
                    .enabled
                    .then_some(entropy_loss_report.entropy_nats),
                l_entropy_weighted: entropy_loss_report
                    .enabled
                    .then_some(entropy_loss_report.weighted_loss),
                l_entropy_lambda: entropy_loss_report
                    .enabled
                    .then_some(entropy_loss_report.lambda),
                l_entropy_sample_count: entropy_loss_report
                    .estimate
                    .as_ref()
                    .map(|estimate| estimate.batch_size),
                l_entropy_estimator: entropy_loss_report
                    .enabled
                    .then_some("knn-radius-v1-slot-preserving-latent".to_string()),
                training_mode: DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE.to_string(),
                distillation_cycle_id: None,
                distillation_loss_mean: None,
                distillation_skipped_count: None,
                counterfactual_smoothness: None,
                counterfactual_smoothness_anomaly: None,
                adversarial_mix_target_ratio: self.config.adversarial_mix_ratio,
                adversarial_mix_count: plan.adversarial_count,
                adversarial_mix_example_indices: plan.adversarial_example_indices.clone(),
                adversarial_mix_fallback_count: plan.adversarial_fallback_count as u64,
                cross_task_transfer_indices: plan.cross_task_indices,
                cross_task_transfer_fallback_count: plan.cross_task_fallback_count as u64,
                predictor_redundancy_pairwise_mi,
                predictor_redundancy_pairwise_mi_source: "train_batch_panel_t01_pairwise_cosine"
                    .to_string(),
                holdout_promotion: None,
                generic_only_warning: Some(DIAGNOSTIC_CERTIFICATE_ONLY_WARNING.to_string()),
                phase3_dod_passed: Some(false),
                grad_norm_running_mean: self.grad_norm_running_mean,
                parent_witness_hash: String::new(),
                self_hash: String::new(),
                merkle_root: String::new(),
                code_version: String::new(),
                corpus_sha: String::new(),
                embedder_versions: HashMap::new(),
                frozen_at: String::new(),
            };
            self.cert_writer.emit(&mut cert)?;
            self.step += 1;
        }
        let count = (self.step - start_step).max(1) as f32;
        let summary = EpochSummary {
            epoch,
            mean_l_step: l_sum / count,
            mean_delta_p: p_sum / count,
            mean_delta_k: k_sum / count,
            mean_delta_omega: o_sum / count,
            mean_delta_xi: x_sum / count,
            holdout_agreement: self.best_holdout_agreement,
            best_category: MutationCategory::KnownGood,
            worst_category: MutationCategory::CompileError,
            best_language: Lang::Python,
            worst_language: Lang::Rust,
            total_steps_this_epoch: self.step - start_step,
            skipped_steps_this_epoch: 0,
            parent_witness_hash: hex::encode(self.epoch_witness.last_epoch_hash),
            self_hash: String::new(),
        };
        let witness_entry = self.epoch_witness.append(summary.clone())?;
        Ok(EpochReport {
            epoch,
            summary,
            witness_entry,
            phase3_dod_passed_at_this_epoch: None,
        })
    }

    pub fn evaluate_holdout(
        &self,
        _holdout: &HoldoutDataset,
        is_final_step: bool,
    ) -> Result<HoldoutReport, TrainerError> {
        Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "Trainer::evaluate_holdout has no predictor/oracle inputs; call HoldoutEvaluator::evaluate and persist the resulting report",
        )
        .with_context(serde_json::json!({
            "step": self.step,
            "is_final_step": is_final_step,
            "remediation": "wire an explicit PredictorForward and OracleHead so holdout metrics are computed from data instead of fabricated by the trainer"
        })))
    }

    pub fn run_full_training(
        &mut self,
        dataset: &TrainingDataset,
    ) -> Result<TrainingResult, TrainerError> {
        let start = Instant::now();
        for epoch in 0..self.config.epochs {
            self.train_one_epoch(dataset, epoch)?;
        }
        Ok(TrainingResult {
            final_holdout_agreement: None,
            total_walltime_seconds: start.elapsed().as_secs(),
            total_steps: self.step,
            total_certs: self.step,
            best_safetensors_path: None,
            training_semantics: DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE.to_string(),
            trained_predictor: false,
            predictor_parameter_update_count: 0,
            checkpoint_readback_verified: false,
            ship_gate_countable_training: false,
            non_training_reason: DIAGNOSTIC_CERTIFICATE_ONLY_WARNING.to_string(),
        })
    }

    pub fn run_full_training_with_inverse_head(
        &mut self,
        dataset: &TrainingDataset,
        inverse_forward: &dyn InverseMapForward,
    ) -> Result<TrainingResult, TrainerError> {
        let start = Instant::now();
        for epoch in 0..self.config.epochs {
            self.train_one_epoch_with_inverse_head(dataset, epoch, inverse_forward)?;
        }
        Ok(TrainingResult {
            final_holdout_agreement: None,
            total_walltime_seconds: start.elapsed().as_secs(),
            total_steps: self.step,
            total_certs: self.step,
            best_safetensors_path: None,
            training_semantics: DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE.to_string(),
            trained_predictor: false,
            predictor_parameter_update_count: 0,
            checkpoint_readback_verified: false,
            ship_gate_countable_training: false,
            non_training_reason: DIAGNOSTIC_CERTIFICATE_ONLY_WARNING.to_string(),
        })
    }

    pub fn current_step(&self) -> u64 {
        self.step
    }

    pub fn request_abort(&mut self) {
        self.abort_requested = true;
    }

    fn rebuild_sampler(&mut self, dataset: &TrainingDataset) -> Result<(), TrainerError> {
        let n = dataset.train.examples.len();
        let adversarial_indices = dataset
            .train
            .examples
            .iter()
            .enumerate()
            .filter_map(|(idx, example)| example.adversarial.then_some(idx))
            .collect::<Vec<_>>();
        self.sampler = BatchSampler::new_with_foundationality_scores(
            self.config.clone(),
            (0..n).map(|i| 0.1 + (i % 10) as f32 * 0.05).collect(),
            (0..n).map(|i| i % 17 == 0).collect(),
            vec![0.0; n],
            dataset
                .train
                .examples
                .iter()
                .map(|ex| ex.task_id.clone())
                .collect(),
            dataset
                .train
                .examples
                .iter()
                .map(|ex| ex.foundationality_score)
                .collect(),
            adversarial_indices,
            PatchSimilarityGraph {
                neighbors: vec![Vec::new(); n],
            },
        )?;
        Ok(())
    }
}

fn batch_pairwise_redundancy(
    dataset: &TrainingDataset,
    indices: &[usize],
) -> Result<f32, TrainerError> {
    if indices.len() < 2 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "pairwise MI train-cert column requires at least two selected batch examples",
        )
        .with_context(serde_json::json!({
            "selected_batch_len": indices.len(),
            "remediation": "increase batch_size or ensure the sampler has at least two non-dropped examples"
        })));
    }
    let mut rows = Vec::with_capacity(indices.len());
    for idx in indices {
        let example = dataset.train.examples.get(*idx).ok_or_else(|| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("sampler selected train index {idx} outside train split"),
            )
        })?;
        let values = example
            .panel_t01
            .to_dtype(DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        if values.is_empty() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("train index {idx} has an empty panel_t01 tensor"),
            ));
        }
        rows.push(values);
    }
    Ok(pairwise_mi_audit(&rows)?)
}

fn batch_inverse_map_loss(
    dataset: &TrainingDataset,
    indices: &[usize],
    device: &Device,
    inverse_forward: &dyn InverseMapForward,
) -> Result<crate::loss::inverse::InverseMapLossBreakdown, TrainerError> {
    if indices.is_empty() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "inverse-map quality requires at least one selected batch example",
        ));
    }
    let mut input_rows = Vec::with_capacity(indices.len());
    let mut target_rows = Vec::with_capacity(indices.len());
    let mut action_rows = Vec::with_capacity(indices.len() * INVERSE_ACTION_DIM);
    let mut panel_dim = None;
    for idx in indices {
        let example = dataset.train.examples.get(*idx).ok_or_else(|| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("sampler selected train index {idx} outside train split"),
            )
        })?;
        let input = example
            .panel_t01
            .to_dtype(DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        let target = example
            .panel_t2
            .to_dtype(DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        if input.is_empty() || input.len() != target.len() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!(
                    "inverse-map panel pair at train index {idx} has incompatible flattened dims {} vs {}",
                    input.len(),
                    target.len()
                ),
            ));
        }
        match panel_dim {
            Some(dim) if dim != input.len() => {
                return Err(TrainerError::new(
                    TrainerErrorCode::MejepaTrainConfigInvalid,
                    format!(
                        "inverse-map batch has mixed panel dims: expected {dim}, got {} at train index {idx}",
                        input.len()
                    ),
                ));
            }
            None => panel_dim = Some(input.len()),
            _ => {}
        }
        input_rows.extend(input);
        target_rows.extend(target);
        let action_target = example.inverse_action_target.as_ref().ok_or_else(|| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("inverse-map train index {idx} is missing structured patch/tool-call action target"),
            )
            .with_context(json!({
                "task_id": example.task_id.clone(),
                "remediation": "attach InverseActionTarget with patch diff text and tool-call records before enabling inverse_map_coefficient"
            }))
        })?;
        action_target.validate()?;
        action_rows.extend(inverse_action_target_vector(action_target));
    }
    let rows = indices.len();
    let dim = panel_dim.ok_or_else(|| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "inverse-map batch has no panel dimension",
        )
    })?;
    let target_panel = Tensor::from_slice(&target_rows, (rows, dim), device)?;
    let input_panel = Tensor::from_slice(&input_rows, (rows, dim), device)?;
    let action = Tensor::from_slice(&action_rows, (rows, INVERSE_ACTION_DIM), device)?;
    let outputs = inverse_forward.predict_inverse(&target_panel)?;
    inverse_map_loss(
        &outputs,
        &InverseMapTargets {
            input_panel,
            action,
        },
    )
}

fn batch_latent_entropy_loss(
    dataset: &TrainingDataset,
    indices: &[usize],
    device: &Device,
    lambda: f32,
) -> Result<LatentEntropyLossReport, TrainerError> {
    if lambda == 0.0 {
        return Ok(LatentEntropyLossReport {
            enabled: false,
            lambda: 0.0,
            entropy_nats: 0.0,
            weighted_loss: 0.0,
            estimate: None,
        });
    }
    let (tensor, report) = latent_entropy_loss(
        &batch_latent_tensor(dataset, indices, device)?,
        LatentEntropyConfig {
            lambda,
            ..LatentEntropyConfig::default()
        },
    )?;
    drop(tensor);
    Ok(report)
}

fn batch_latent_tensor(
    dataset: &TrainingDataset,
    indices: &[usize],
    device: &Device,
) -> Result<Tensor, TrainerError> {
    if indices.is_empty() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "latent entropy requires at least one selected batch example",
        ));
    }
    let mut rows = Vec::new();
    let mut latent_dim = None;
    for idx in indices {
        let example = dataset.train.examples.get(*idx).ok_or_else(|| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("sampler selected train index {idx} outside train split"),
            )
        })?;
        let latent = example
            .panel_t01
            .to_dtype(DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        if latent.is_empty() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("latent entropy train index {idx} has an empty panel_t01 tensor"),
            ));
        }
        match latent_dim {
            Some(dim) if dim != latent.len() => {
                return Err(TrainerError::new(
                    TrainerErrorCode::MejepaTrainConfigInvalid,
                    format!(
                        "latent entropy batch has mixed latent dims: expected {dim}, got {} at train index {idx}",
                        latent.len()
                    ),
                ));
            }
            None => latent_dim = Some(latent.len()),
            _ => {}
        }
        rows.extend(latent);
    }
    let dim = latent_dim.ok_or_else(|| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "latent entropy batch has no latent dimension",
        )
    })?;
    Tensor::from_slice(&rows, (indices.len(), dim), device).map_err(TrainerError::from)
}

fn inverse_action_target_vector(
    target: &crate::eval::holdout::InverseActionTarget,
) -> [f32; INVERSE_ACTION_DIM] {
    let mut action = [0.0_f32; INVERSE_ACTION_DIM];
    let diff = PatchDiffStats::from_diff(&target.patch_diff);
    action[0] = normalize_count(diff.added_lines, 128);
    action[1] = normalize_count(diff.deleted_lines, 128);
    action[2] = normalize_count(diff.touched_files, 64);
    action[3] = normalize_count(diff.hunks, 64);
    action[4] = normalize_count(target.tool_calls.len(), 32);
    for call in &target.tool_calls {
        let tool_name = call.tool_name.to_ascii_lowercase();
        let args = call.arguments_json.to_ascii_lowercase();
        if tool_name.contains("bash") {
            action[5] = 1.0;
        }
        if tool_name.contains("edit") || tool_name.contains("write") || tool_name.contains("patch")
        {
            action[6] = 1.0;
        }
        if args.contains("pytest")
            || args.contains("cargo test")
            || args.contains("unittest")
            || args.contains("jest")
            || args.contains("vitest")
        {
            action[7] = 1.0;
        }
    }
    let digest = inverse_action_digest(target);
    for (slot, chunk) in action[8..].iter_mut().zip(digest.chunks_exact(2)) {
        let raw = u16::from_be_bytes([chunk[0], chunk[1]]) as f32 / u16::MAX as f32;
        *slot = raw.mul_add(2.0, -1.0);
    }
    action
}

#[derive(Debug, Default)]
struct PatchDiffStats {
    added_lines: usize,
    deleted_lines: usize,
    touched_files: usize,
    hunks: usize,
}

impl PatchDiffStats {
    fn from_diff(diff: &str) -> Self {
        let mut stats = Self::default();
        let mut files = std::collections::BTreeSet::new();
        for line in diff.lines() {
            if let Some(path) = line.strip_prefix("diff --git ") {
                files.insert(path.to_string());
            } else if let Some(path) = line.strip_prefix("+++ b/") {
                files.insert(path.to_string());
            }
            if line.starts_with("@@") {
                stats.hunks += 1;
            } else if line.starts_with('+') && !line.starts_with("+++") {
                stats.added_lines += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                stats.deleted_lines += 1;
            }
        }
        stats.touched_files = files.len();
        stats
    }
}

fn normalize_count(count: usize, cap: usize) -> f32 {
    let cap = cap.max(1);
    (count.min(cap) as f32) / cap as f32
}

fn inverse_action_digest(target: &crate::eval::holdout::InverseActionTarget) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(target.patch_diff.as_bytes());
    for call in &target.tool_calls {
        hasher.update([0]);
        hasher.update(call.tool_name.as_bytes());
        hasher.update([0]);
        hasher.update(call.arguments_json.as_bytes());
    }
    hasher.finalize().into()
}

fn default_embedder_versions() -> HashMap<String, String> {
    (1..=21)
        .map(|i| (format!("E{i}"), "phase1b-sha-pinned-registry".to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert::{open_train_rocksdb, TrainingCertificate, CF_MEJEPA_TRAIN_CERTS};
    use crate::eval::HoldoutExample;
    use candle_core::Device;

    #[test]
    fn full_training_result_is_explicitly_certificate_only(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let db = open_train_rocksdb(temp.path())?;
        let cfg = TrainingConfig {
            epochs: 1,
            batch_size: 2,
            full_finetune: true,
            cross_task_transfer_probability: 0.0,
            adversarial_mix_ratio: 0.0,
            ..TrainingConfig::default()
        };
        let mut trainer = Trainer::new(cfg, db.clone(), Device::Cpu)?;
        let result = trainer.run_full_training(&synthetic_dataset(4)?)?;

        assert_eq!(
            result.training_semantics,
            DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE
        );
        assert!(!result.trained_predictor);
        assert_eq!(result.predictor_parameter_update_count, 0);
        assert!(!result.checkpoint_readback_verified);
        assert!(!result.ship_gate_countable_training);
        assert!(result.final_holdout_agreement.is_none());
        assert!(result.best_safetensors_path.is_none());

        let serialized = serde_json::to_value(&result)?;
        assert_eq!(serialized["trained_predictor"], serde_json::json!(false));
        assert_eq!(
            serialized["ship_gate_countable_training"],
            serde_json::json!(false)
        );
        assert_eq!(
            serialized["training_semantics"],
            serde_json::json!(DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE)
        );
        Ok(())
    }

    #[test]
    fn emitted_certificate_cannot_claim_lora_or_full_finetune_training(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let db = open_train_rocksdb(temp.path())?;
        let cfg = TrainingConfig {
            epochs: 1,
            batch_size: 2,
            full_finetune: true,
            cross_task_transfer_probability: 0.0,
            adversarial_mix_ratio: 0.0,
            ..TrainingConfig::default()
        };
        let mut trainer = Trainer::new(cfg, db.clone(), Device::Cpu)?;
        trainer.train_one_epoch(&synthetic_dataset(4)?, 0)?;

        let cert = read_cert(&db, 0)?;
        assert_eq!(
            cert.training_mode,
            DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE
        );
        assert_ne!(cert.training_mode, "lora");
        assert_ne!(cert.training_mode, "full_finetune");
        assert_eq!(
            cert.generic_only_warning.as_deref(),
            Some(DIAGNOSTIC_CERTIFICATE_ONLY_WARNING)
        );
        assert_eq!(cert.phase3_dod_passed, Some(false));
        assert_eq!(
            cert.loss_components
                .get("predictor_parameter_update_count")
                .copied(),
            Some(0.0)
        );
        assert_eq!(
            cert.loss_components
                .get("ship_gate_countable_training")
                .copied(),
            Some(0.0)
        );
        Ok(())
    }

    fn synthetic_dataset(n: usize) -> Result<TrainingDataset, Box<dyn std::error::Error>> {
        let mut examples = Vec::with_capacity(n);
        for i in 0..n {
            let panel = Tensor::from_slice(&[i as f32, (i % 2) as f32], 2, &Device::Cpu)?;
            examples.push(HoldoutExample {
                task_id: format!("trainer-cert-only-{i:03}"),
                category: MutationCategory::ALL[i % MutationCategory::ALL.len()],
                language: Lang::Python,
                panel_t01: panel.clone(),
                panel_t2: panel,
                inverse_action_target: None,
                actual_oracle_pass: i % 2 == 0,
                adversarial: false,
                foundationality_score: 0.0,
            });
        }
        Ok(TrainingDataset {
            train: TrainSplit { examples },
            calibration: CalibrationDataset {
                examples: Vec::new(),
            },
            holdout: HoldoutDataset {
                examples: Vec::new(),
            },
        })
    }

    fn read_cert(
        db: &rocksdb::DB,
        step: u64,
    ) -> Result<TrainingCertificate, Box<dyn std::error::Error>> {
        let cf = db
            .cf_handle(CF_MEJEPA_TRAIN_CERTS)
            .ok_or("missing CF_MEJEPA_TRAIN_CERTS")?;
        let bytes = db
            .get_cf(cf, step.to_be_bytes())?
            .ok_or("missing training certificate")?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}
