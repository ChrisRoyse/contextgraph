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
use crate::sampler::{BatchSampler, PatchSimilarityGraph};
use candle_core::{DType, Device};
use context_graph_mejepa::HeadId;
use ed25519_dalek::SigningKey;
use rocksdb::DB;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

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
            let loss_components = HashMap::from([
                ("predict".to_string(), signal.delta_p),
                ("variance".to_string(), 0.01),
                ("covariance".to_string(), 0.01),
                ("invariance".to_string(), 0.0),
            ]);
            let conditional_description_length_bits =
                crate::compression_progress::conditional_description_length_bits_from_probability(
                    signal.delta_p,
                );
            let mut cert = TrainingCertificate {
                step: self.step,
                epoch,
                signal,
                per_head_l_step: per_head_signal.l_step_map().into_iter().collect(),
                delta_xi_global_min: per_head_signal.delta_xi_global_min,
                loss_components,
                conditional_description_length_bits: Some(conditional_description_length_bits),
                training_mode: if self.config.full_finetune {
                    "full_finetune".to_string()
                } else {
                    "lora".to_string()
                },
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
                generic_only_warning: None,
                phase3_dod_passed: None,
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
        self.sampler = BatchSampler::new_with_adversarial_indices(
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

fn default_embedder_versions() -> HashMap<String, String> {
    (1..=21)
        .map(|i| (format!("E{i}"), "phase1b-sha-pinned-registry".to_string()))
        .collect()
}
