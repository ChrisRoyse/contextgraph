use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use candle_core::{DType, Device, Tensor};
use clap::Args;
use context_graph_mejepa_instruments::{InstrumentSlot, Panel};
use serde::{Deserialize, Serialize};

use crate::calibration::CalibrationStore;
use crate::compiler::{
    DeterministicConstellationGuard, DeterministicOracleHead, DeterministicPredictor, FrozenTarget,
    IdentityFrozenTarget, MeJepaCompiler, OracleHead, OracleScores, PatchWitnessReader, Predictor,
    TctConstellationGuard,
};
use crate::config::{
    resolve_required_trained_checkpoint_manifest_path, MeJepaInferConfig, PredictorConfig,
    PANEL_DIM,
};
use crate::conformal::{non_conformity_score, CalibrationExample};
use crate::data_models::{PredictedPanel, TargetProvenance};
use crate::error::MejepaInferError;
use crate::fixtures::fixture_patch_context;
use crate::frozen_target::FrozenTargetAdapter;
use crate::ood::{ood_score_from_norm_sq, separation_auc, squared_l2};
use crate::predictor::MeJepaPredictor;
use crate::predictor_checkpoint::{
    load_verified_trained_predictor_checkpoint, LoadedPredictorCheckpoint,
};
use crate::store::RocksDbInferStore;
use context_graph_mejepa_tct::{
    ConstellationStore, EntityType as TctEntityType, MutationCategory as TctMutationCategory,
};

pub const FIXTURE_COMPILER_EVIDENCE_CLASS: &str = "fixture_only_non_ship_gate";
pub const SLOT_PRESERVING_CUDA_COMPILER_EVIDENCE_CLASS: &str =
    "slot_preserving_cuda_non_synthetic_untrained_checkpoint_blocked";
pub const SLOT_PRESERVING_CUDA_TRAINED_COMPILER_EVIDENCE_CLASS: &str =
    "slot_preserving_cuda_trained_checkpoint_loaded";
pub const TRAINED_CHECKPOINT_ATTESTATION_KEY: &str = "MEJEPA_TRAINED_CHECKPOINT_LOADED";
pub const NATIVE_ACTIVE_CONSTELLATION_ADAPTER_ATTESTATION_KEY: &str =
    "MEJEPA_NATIVE_ACTIVE_CONSTELLATION_ADAPTER_CITED";

#[derive(Debug, Clone, Args)]
pub struct InferTestArgs {
    #[arg(long)]
    pub calibration: PathBuf,
    #[arg(long, default_value_t = 0.10)]
    pub alpha: f32,
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-infer-fsv")]
    pub output_fsv: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferTestReport {
    pub stated_coverage: f32,
    pub empirical_coverage: f32,
    pub prediction_latency_p50_ms: f32,
    pub prediction_latency_p99_ms: f32,
    pub ood_separation_auc: f32,
    pub ood_separation_auc_source: String,
    pub num_calibration_samples: usize,
    pub calibration_version: String,
}

pub fn run_infer_test(
    args: InferTestArgs,
    compiler: &MeJepaCompiler,
    store: &CalibrationStore,
) -> Result<InferTestReport, MejepaInferError> {
    fs::create_dir_all(&args.output_fsv)
        .map_err(|source| MejepaInferError::io("create_dir_all", &args.output_fsv, source))?;
    let examples = load_calibration_examples(&args.calibration)?;
    let norm_sq = examples
        .iter()
        .map(|example| {
            let score =
                non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)?;
            Ok(score * score)
        })
        .collect::<Result<Vec<_>, MejepaInferError>>()?;
    let record = store.calibrate(
        &examples,
        &norm_sq,
        args.alpha,
        30,
        0.30,
        [4; 32],
        BTreeMap::new(),
    )?;

    let holdout_start = (examples.len() * 8 / 10).min(examples.len().saturating_sub(1));
    let holdout = &examples[holdout_start..];
    let covered = holdout
        .iter()
        .filter(|example| {
            non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)
                .map(|score| score <= record.tau)
                .unwrap_or(false)
        })
        .count();
    let empirical_coverage = covered as f32 / holdout.len() as f32;

    // M-H1 / issue #484: compute REAL OOD separation AUC from the calibration
    // holdout split using the production OOD subsystem (`crate::ood`). The
    // previous synthetic `vec![0.05; 100]` vs `vec![0.95; 100]` distributions
    // are replaced with per-holdout-example residual-driven OOD scores produced
    // by the same `ood_score_from_norm_sq(norm_sq, sigma_squared)` primitive the
    // production predictor uses (`crate::compiler` / `crate::verdict_assembly`).
    //
    // The two populations for `separation_auc`:
    // - in_dist: holdout examples whose binary predicted_test_pass (rounded at
    //   0.5) MATCHES the binary actual_test_pass (oracle agreement).
    // - ood:     holdout examples whose predictor disagreed with the oracle.
    //
    // The AUC then measures: "do held-out examples where the predictor was
    // wrong actually receive higher OOD scores than examples where the
    // predictor was right?" That is the binary-doctrine OOD discrimination
    // question — a property the OOD subsystem must satisfy on the calibration
    // holdout to be admissible per CLAUDE.md §1 Q2.
    //
    // If either partition is empty, the holdout cannot answer the OOD
    // discrimination question — we fail closed with
    // MEJEPA_INFER_TEST_CALIBRATION_HOLDOUT_UNAVAILABLE rather than synthesize
    // a passing AUC.
    let (ood_separation_auc, ood_separation_auc_source) =
        compute_real_ood_separation_auc(holdout, record.sigma_squared)?;

    let repo_root = args.output_fsv.join("infer-test-repo");
    let (patch, context) = fixture_patch_context(&repo_root, "approve")?;
    let mut latencies = Vec::with_capacity(1_000);
    for _ in 0..1_000 {
        let start = Instant::now();
        let _ = compiler.verify(&patch, &context)?;
        latencies.push(start.elapsed().as_secs_f32() * 1_000.0);
    }
    latencies.sort_by(|a, b| a.total_cmp(b));
    let p50 = latencies[latencies.len() / 2];
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];

    let report = InferTestReport {
        stated_coverage: 1.0 - args.alpha,
        empirical_coverage,
        prediction_latency_p50_ms: p50,
        prediction_latency_p99_ms: p99,
        ood_separation_auc,
        ood_separation_auc_source,
        num_calibration_samples: examples.len(),
        calibration_version: record.version,
    };
    write_json_0600(&args.output_fsv.join("infer-test-report.json"), &report)?;
    write_json_0600(&args.output_fsv.join("latency-histogram.json"), &latencies)?;
    Ok(report)
}

/// Compute the real OOD separation AUC from the calibration holdout split.
///
/// Each holdout example contributes one real OOD score (via
/// `ood_score_from_norm_sq(squared_l2, sigma_squared)`). Examples whose
/// predictor agreed with the oracle (binary rounded match) form the
/// in-distribution population; examples whose predictor disagreed form the OOD
/// population. The AUC is the pairwise probability that an OOD example
/// receives a higher OOD score than an in-distribution example.
///
/// Returns `Err(InferTestCalibrationHoldoutUnavailable)` if the holdout is
/// empty, single-class (all-agree or all-disagree), or all residual norms are
/// non-finite — in any of those cases no real OOD discrimination metric can be
/// computed and we must NOT synthesize a passing AUC.
pub(crate) fn compute_real_ood_separation_auc(
    holdout: &[CalibrationExample],
    sigma_squared: f32,
) -> Result<(f32, String), MejepaInferError> {
    if holdout.is_empty() {
        return Err(MejepaInferError::InferTestCalibrationHoldoutUnavailable {
            detail: "calibration holdout split is empty; cannot exercise OOD subsystem".to_string(),
        });
    }
    if !sigma_squared.is_finite() || sigma_squared <= 0.0 {
        return Err(MejepaInferError::InferTestCalibrationHoldoutUnavailable {
            detail: format!(
                "calibration sigma_squared={sigma_squared} is not a valid OOD scale; refusing to compute AUC"
            ),
        });
    }
    let mut in_dist_scores: Vec<f32> = Vec::with_capacity(holdout.len());
    let mut ood_scores: Vec<f32> = Vec::with_capacity(holdout.len());
    for (idx, example) in holdout.iter().enumerate() {
        if example.predicted_test_pass.len() != example.actual_test_pass.len() {
            return Err(MejepaInferError::InferTestCalibrationHoldoutUnavailable {
                detail: format!(
                    "holdout[{idx}].predicted_test_pass.len={} != actual_test_pass.len={}",
                    example.predicted_test_pass.len(),
                    example.actual_test_pass.len()
                ),
            });
        }
        if example.predicted_test_pass.is_empty() {
            return Err(MejepaInferError::InferTestCalibrationHoldoutUnavailable {
                detail: format!("holdout[{idx}] has zero test predictions"),
            });
        }
        let norm_sq =
            squared_l2(&example.predicted_test_pass, &example.actual_test_pass).map_err(|err| {
                MejepaInferError::InferTestCalibrationHoldoutUnavailable {
                    detail: format!(
                        "holdout[{idx}] squared_l2 failed: {err} (real OOD score requires finite residuals)"
                    ),
                }
            })?;
        let score = ood_score_from_norm_sq(norm_sq, sigma_squared);
        if !score.is_finite() {
            return Err(MejepaInferError::InferTestCalibrationHoldoutUnavailable {
                detail: format!(
                    "holdout[{idx}] ood_score_from_norm_sq returned non-finite for norm_sq={norm_sq} sigma_squared={sigma_squared}"
                ),
            });
        }
        // Per-test binary agreement: predictor agreed iff (predicted_test_pass[i] >= 0.5)
        // matches (actual_test_pass[i] >= 0.5) for ALL tests in the example.
        let predictor_agreed_with_oracle = example
            .predicted_test_pass
            .iter()
            .zip(example.actual_test_pass.iter())
            .all(|(predicted, actual)| (*predicted >= 0.5) == (*actual >= 0.5));
        if predictor_agreed_with_oracle {
            in_dist_scores.push(score);
        } else {
            ood_scores.push(score);
        }
    }
    if in_dist_scores.is_empty() || ood_scores.is_empty() {
        return Err(MejepaInferError::InferTestCalibrationHoldoutUnavailable {
            detail: format!(
                "holdout is single-class: in_dist={} ood={}; cannot compute OOD separation AUC without both populations",
                in_dist_scores.len(),
                ood_scores.len()
            ),
        });
    }
    let auc = separation_auc(&in_dist_scores, &ood_scores)?;
    Ok((auc, "real_ood_score_readback".to_string()))
}

pub fn dod_satisfied(report: &InferTestReport) -> bool {
    (0.88..=0.92).contains(&report.empirical_coverage)
        && report.prediction_latency_p50_ms < 100.0
        && report.ood_separation_auc_source == "real_ood_score_readback"
        && report.ood_separation_auc > 0.85
}

pub fn build_fixture_deterministic_compiler(
    repo_root: PathBuf,
    store: Arc<RocksDbInferStore>,
    calibration: CalibrationStore,
    mut config: MeJepaInferConfig,
) -> Result<MeJepaCompiler, MejepaInferError> {
    config.outcome_set_max = config.outcome_set_max.max(1);
    let system_cost_counters = store.system_cost_counters();
    MeJepaCompiler::new_with_system_cost_counters(
        config,
        Arc::new(DeterministicPredictor),
        Arc::new(IdentityFrozenTarget),
        Arc::new(DeterministicOracleHead),
        Arc::new(DeterministicConstellationGuard::default()),
        Arc::new(PatchWitnessReader),
        store,
        calibration,
        repo_root,
        system_cost_counters,
    )
}

pub fn build_slot_preserving_cuda_compiler(
    repo_root: PathBuf,
    store: Arc<RocksDbInferStore>,
    calibration: CalibrationStore,
    mut config: MeJepaInferConfig,
) -> Result<MeJepaCompiler, MejepaInferError> {
    config.outcome_set_max = config.outcome_set_max.max(1);
    let system_cost_counters = store.system_cost_counters();
    let mut instrument_versions = BTreeMap::new();
    for slot in InstrumentSlot::all() {
        instrument_versions.insert(
            slot.slug().to_string(),
            format!("slot-preserving-real-compiler-v1:dim={}", slot.dim()),
        );
    }
    let frozen_target = FrozenTargetAdapter::new(TargetProvenance::new(
        "slot-preserving-frozen-instrument-panel",
        instrument_versions,
        0,
        None,
    ));
    let constellation_store = ConstellationStore::new(store.db())?;
    let latest_constellation_version = constellation_store.latest_version()?;
    let constellation =
        constellation_store.load_without_runtime_checks(latest_constellation_version)?;
    let constellation_guard = TctConstellationGuard::new(
        constellation,
        TctMutationCategory::KnownGood,
        TctEntityType::Function,
    )?;
    let device = Device::new_cuda(0).map_err(crate::error::PredictorError::from)?;
    let predictor_config = PredictorConfig::default();
    let mut predictor = MeJepaPredictor::new(predictor_config.clone(), frozen_target, device, 1)?;
    let manifest_path = resolve_required_trained_checkpoint_manifest_path(
        config.trained_checkpoint_manifest_path.as_ref(),
    )?;
    config.trained_checkpoint_manifest_path = Some(manifest_path.clone());
    let loaded_checkpoint = Some(load_verified_trained_predictor_checkpoint(
        &mut predictor,
        &manifest_path,
        &predictor_config,
    )?);
    let loaded_checkpoint = Arc::new(loaded_checkpoint);
    let shared = Arc::new(Mutex::new(predictor));
    MeJepaCompiler::new_with_system_cost_counters(
        config,
        Arc::new(SlotPreservingCudaPredictor {
            shared: Arc::clone(&shared),
        }),
        Arc::new(SlotPreservingCudaFrozenTarget {
            shared: Arc::clone(&shared),
        }),
        Arc::new(SlotPreservingCudaOracleHead {
            shared,
            loaded_checkpoint,
        }),
        Arc::new(constellation_guard),
        Arc::new(PatchWitnessReader),
        store,
        calibration,
        repo_root,
        system_cost_counters,
    )
}

struct SlotPreservingCudaPredictor {
    shared: Arc<Mutex<MeJepaPredictor>>,
}

impl Predictor for SlotPreservingCudaPredictor {
    fn predict(&self, panel_t0: &Panel, panel_t1: &Panel) -> Result<Panel, MejepaInferError> {
        let filled_mask = panel_t0.filled_mask() | panel_t1.filled_mask();
        let predictor = lock_real_predictor(&self.shared, "predictor")?;
        let panel_t0 = panel_to_tensor(panel_t0, predictor.device(), predictor.dtype())?;
        let panel_t1 = panel_to_tensor(panel_t1, predictor.device(), predictor.dtype())?;
        let predicted = predictor.forward(&panel_t0, &panel_t1)?;
        candle(predictor.device().synchronize())?;
        tensor_to_panel(&predicted.tensor, filled_mask)
    }
}

struct SlotPreservingCudaFrozenTarget {
    shared: Arc<Mutex<MeJepaPredictor>>,
}

impl FrozenTarget for SlotPreservingCudaFrozenTarget {
    fn target(&self, panel_t2: &Panel) -> Result<Panel, MejepaInferError> {
        let predictor = lock_real_predictor(&self.shared, "frozen_target")?;
        let target = predictor.frozen_target_adapter().encode_target(
            panel_t2,
            predictor.device(),
            predictor.dtype(),
        )?;
        if target.panel_dim != PANEL_DIM || target.batch_size != 1 {
            return Err(MejepaInferError::DimMismatch {
                expected: PANEL_DIM,
                actual: target.panel_dim,
                context: "real frozen target adapter returned an invalid target panel".to_string(),
            });
        }
        Ok(panel_t2.clone())
    }
}

struct SlotPreservingCudaOracleHead {
    shared: Arc<Mutex<MeJepaPredictor>>,
    loaded_checkpoint: Arc<Option<LoadedPredictorCheckpoint>>,
}

impl OracleHead for SlotPreservingCudaOracleHead {
    fn score(&self, predicted_panel: &Panel) -> Result<OracleScores, MejepaInferError> {
        let predictor = lock_real_predictor(&self.shared, "oracle_head")?;
        let tensor = panel_to_tensor(predicted_panel, predictor.device(), predictor.dtype())?;
        let predicted = PredictedPanel {
            tensor,
            batch_size: 1,
            panel_dim: PANEL_DIM,
            dtype: format!("{:?}", predictor.dtype()),
        };
        let logits = predictor.oracle_head().predict_logits(&predicted)?;
        let probabilities = predictor.oracle_head().predict_probabilities(&logits)?;
        let values = candle(candle(probabilities.tensor.to_dtype(DType::F32))?.flatten_all())?
            .to_vec1::<f32>()
            .map_err(crate::error::PredictorError::from)?;
        if values.len() < 2 {
            return Err(MejepaInferError::DimMismatch {
                expected: 2,
                actual: values.len(),
                context:
                    "real oracle head must emit oracle-pass plus at least one test probability"
                        .to_string(),
            });
        }
        let runtime_slot = predicted_panel.slot(InstrumentSlot::ERuntime);
        let mut predicted_runtime_trace = [0.0; 32];
        for (dst, src) in predicted_runtime_trace
            .iter_mut()
            .zip(runtime_slot.iter().copied())
        {
            *dst = src;
        }
        let checkpoint_loaded = self.loaded_checkpoint.as_ref().as_ref();
        let mut granger_attestations = BTreeMap::from([
            ("MEJEPA_REAL_COMPILER".to_string(), 1.0),
            (
                "MEJEPA_REAL_COMPILER_EVIDENCE_CLASS".to_string(),
                if values[0].is_finite() { 1.0 } else { 0.0 },
            ),
            (
                TRAINED_CHECKPOINT_ATTESTATION_KEY.to_string(),
                checkpoint_loaded.map(|_| 1.0).unwrap_or(0.0),
            ),
        ]);
        if let Some(checkpoint) = checkpoint_loaded {
            granger_attestations.insert(
                format!("checkpoint_sha256:{}", checkpoint.checkpoint_sha256),
                1.0,
            );
            granger_attestations.insert(
                format!("checkpoint_manifest_sha256:{}", checkpoint.manifest_sha256),
                1.0,
            );
            granger_attestations.insert(
                format!(
                    "checkpoint_architecture_sha256:{}",
                    checkpoint.architecture_sha256
                ),
                1.0,
            );
            granger_attestations.insert(
                format!(
                    "checkpoint_training_certificate_sha256:{}",
                    checkpoint.training_certificate_sha256
                ),
                1.0,
            );
            granger_attestations.insert(
                format!(
                    "checkpoint_trained_weight_sha256:{}",
                    checkpoint.trained_weight_sha256
                ),
                1.0,
            );
            if let Some(adapter) = &checkpoint.native_active_constellation_adapter {
                granger_attestations.insert(
                    NATIVE_ACTIVE_CONSTELLATION_ADAPTER_ATTESTATION_KEY.to_string(),
                    1.0,
                );
                granger_attestations.insert(
                    format!(
                        "native_active_constellation_adapter_manifest_sha256:{}",
                        adapter.manifest_sha256
                    ),
                    1.0,
                );
                granger_attestations.insert(
                    format!(
                        "native_active_constellation_adapter_checkpoint_sha256:{}",
                        adapter.checkpoint_sha256
                    ),
                    1.0,
                );
                granger_attestations.insert(
                    format!(
                        "native_active_constellation_adapter_training_certificate_sha256:{}",
                        adapter.training_certificate_sha256
                    ),
                    1.0,
                );
            }
        }
        Ok(OracleScores {
            predicted_oracle_pass: values[0],
            predicted_test_pass: values[1..].to_vec(),
            predicted_runtime_trace,
            granger_attestations,
        })
    }
}

fn lock_real_predictor<'a>(
    shared: &'a Mutex<MeJepaPredictor>,
    head: &'static str,
) -> Result<MutexGuard<'a, MeJepaPredictor>, MejepaInferError> {
    shared.lock().map_err(|_| MejepaInferError::InvalidInput {
        field: format!("real_compiler.{head}"),
        detail: "slot-preserving CUDA compiler mutex was poisoned".to_string(),
    })
}

fn panel_to_tensor(
    panel: &Panel,
    device: &Device,
    dtype: DType,
) -> Result<Tensor, MejepaInferError> {
    if panel.data().len() != PANEL_DIM {
        return Err(MejepaInferError::DimMismatch {
            expected: PANEL_DIM,
            actual: panel.data().len(),
            context: "panel_to_tensor real compiler input".to_string(),
        });
    }
    candle(candle(Tensor::from_slice(panel.data(), (1, PANEL_DIM), device))?.to_dtype(dtype))
}

fn tensor_to_panel(tensor: &Tensor, filled_mask: u16) -> Result<Panel, MejepaInferError> {
    let values = candle(candle(tensor.to_dtype(DType::F32))?.flatten_all())?
        .to_vec1::<f32>()
        .map_err(crate::error::PredictorError::from)?;
    if values.len() != PANEL_DIM {
        return Err(MejepaInferError::DimMismatch {
            expected: PANEL_DIM,
            actual: values.len(),
            context: "real compiler predicted tensor length".to_string(),
        });
    }
    Ok(Panel::try_new(values, filled_mask)?)
}

fn candle<T>(result: Result<T, candle_core::Error>) -> Result<T, MejepaInferError> {
    result
        .map_err(crate::error::PredictorError::from)
        .map_err(MejepaInferError::from)
}

pub fn load_calibration_examples(
    path: &PathBuf,
) -> Result<Vec<CalibrationExample>, MejepaInferError> {
    let mut examples = Vec::new();
    if path.is_file() {
        let bytes = fs::read(path).map_err(|source| MejepaInferError::io("read", path, source))?;
        examples = serde_json::from_slice(&bytes)?;
    } else {
        let entries =
            fs::read_dir(path).map_err(|source| MejepaInferError::io("read_dir", path, source))?;
        for entry in entries {
            let entry =
                entry.map_err(|source| MejepaInferError::io("read_dir_entry", path, source))?;
            let entry_path = entry.path();
            if entry_path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&entry_path)
                .map_err(|source| MejepaInferError::io("read", &entry_path, source))?;
            let example: CalibrationExample = serde_json::from_slice(&bytes)?;
            examples.push(example);
        }
    }
    if examples.is_empty() {
        return Err(MejepaInferError::ConformalInsufficientSamples {
            language: None,
            expected: 1,
            actual: 0,
        });
    }
    Ok(examples)
}

pub fn write_json_0600<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), MejepaInferError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| MejepaInferError::io("create_dir_all", parent, source))?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|source| MejepaInferError::io("open", path, source))?
    };
    #[cfg(not(unix))]
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|source| MejepaInferError::io("open", path, source))?;
    file.write_all(&bytes)
        .map_err(|source| MejepaInferError::io("write", path, source))?;
    let readback = fs::read(path).map_err(|source| MejepaInferError::io("read", path, source))?;
    if readback != bytes {
        return Err(MejepaInferError::InvalidInput {
            field: "fsv_readback".to_string(),
            detail: format!(
                "{} readback bytes differ from written bytes",
                path.display()
            ),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .map_err(|source| MejepaInferError::io("metadata", path, source))?
            .permissions()
            .mode()
            & 0o777;
        if mode != 0o600 {
            return Err(MejepaInferError::InvalidInput {
                field: "fsv_mode".to_string(),
                detail: format!("{} mode {mode:o} != 600", path.display()),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn infer_report(
        empirical_coverage: f32,
        latency_p50_ms: f32,
        ood_separation_auc: f32,
        ood_separation_auc_source: &str,
    ) -> InferTestReport {
        InferTestReport {
            stated_coverage: 0.90,
            empirical_coverage,
            prediction_latency_p50_ms: latency_p50_ms,
            prediction_latency_p99_ms: latency_p50_ms,
            ood_separation_auc,
            ood_separation_auc_source: ood_separation_auc_source.to_string(),
            num_calibration_samples: 100,
            calibration_version: "test-calibration".to_string(),
        }
    }

    #[test]
    fn dod_rejects_not_measured_ood_auc_even_when_perfect() {
        let report = infer_report(0.90, 10.0, 1.0, "not_measured_no_real_ood_scores");

        assert!(!dod_satisfied(&report));
    }

    #[test]
    fn dod_accepts_real_ood_readback_when_all_thresholds_pass() {
        let report = infer_report(0.90, 10.0, 0.90, "real_ood_score_readback");

        assert!(dod_satisfied(&report));
    }

    #[test]
    fn dod_rejects_low_real_ood_auc() {
        let report = infer_report(0.90, 10.0, 0.85, "real_ood_score_readback");

        assert!(!dod_satisfied(&report));
    }

    #[test]
    fn dod_rejects_bad_coverage_even_with_real_ood_readback() {
        let report = infer_report(0.87, 10.0, 0.90, "real_ood_score_readback");

        assert!(!dod_satisfied(&report));
    }

    // ------ M-H1 / issue #484 regression tests for real OOD readback ------

    fn calibration_example(predicted: Vec<f32>, actual: Vec<f32>) -> CalibrationExample {
        CalibrationExample {
            language: crate::Language::Python,
            predicted_test_pass: predicted,
            actual_test_pass: actual,
        }
    }

    #[test]
    fn ood_separation_auc_fails_closed_on_empty_holdout() {
        // M-H1 regression: empty holdout means OOD subsystem cannot be
        // exercised — must fail closed, never synthesize.
        let err = compute_real_ood_separation_auc(&[], 1.0).unwrap_err();
        assert_eq!(
            err.code(),
            "MEJEPA_INFER_TEST_CALIBRATION_HOLDOUT_UNAVAILABLE"
        );
    }

    #[test]
    fn ood_separation_auc_fails_closed_on_single_class_holdout_all_agree() {
        // M-H1 regression: if every holdout example has predictor=oracle
        // agreement, the OOD population is empty and AUC is undefined.
        // Must fail closed.
        let holdout = vec![
            calibration_example(vec![0.9, 0.95], vec![1.0, 1.0]),
            calibration_example(vec![0.1, 0.05], vec![0.0, 0.0]),
            calibration_example(vec![0.85, 0.92], vec![1.0, 1.0]),
        ];
        let err = compute_real_ood_separation_auc(&holdout, 1.0).unwrap_err();
        assert_eq!(
            err.code(),
            "MEJEPA_INFER_TEST_CALIBRATION_HOLDOUT_UNAVAILABLE"
        );
    }

    #[test]
    fn ood_separation_auc_fails_closed_on_single_class_holdout_all_disagree() {
        // M-H1 regression: if every holdout example has predictor≠oracle,
        // the in-distribution population is empty and AUC is undefined.
        let holdout = vec![
            calibration_example(vec![0.9, 0.95], vec![0.0, 0.0]),
            calibration_example(vec![0.1, 0.05], vec![1.0, 1.0]),
        ];
        let err = compute_real_ood_separation_auc(&holdout, 1.0).unwrap_err();
        assert_eq!(
            err.code(),
            "MEJEPA_INFER_TEST_CALIBRATION_HOLDOUT_UNAVAILABLE"
        );
    }

    #[test]
    fn ood_separation_auc_fails_closed_on_invalid_sigma_squared() {
        // M-H1 regression: sigma_squared must come from the real calibration
        // record; if it's non-positive/non-finite the OOD scale is invalid.
        let holdout = vec![calibration_example(vec![0.9], vec![1.0])];
        let err_zero = compute_real_ood_separation_auc(&holdout, 0.0).unwrap_err();
        assert_eq!(
            err_zero.code(),
            "MEJEPA_INFER_TEST_CALIBRATION_HOLDOUT_UNAVAILABLE"
        );

        let err_nan = compute_real_ood_separation_auc(&holdout, f32::NAN).unwrap_err();
        assert_eq!(
            err_nan.code(),
            "MEJEPA_INFER_TEST_CALIBRATION_HOLDOUT_UNAVAILABLE"
        );

        let err_neg = compute_real_ood_separation_auc(&holdout, -1.0).unwrap_err();
        assert_eq!(
            err_neg.code(),
            "MEJEPA_INFER_TEST_CALIBRATION_HOLDOUT_UNAVAILABLE"
        );
    }

    #[test]
    fn ood_separation_auc_returns_real_score_with_mixed_holdout() {
        // M-H1 regression: with a real two-class holdout (some predictor=oracle
        // and some predictor≠oracle), the AUC must come from
        // ood::separation_auc against the REAL ood_score_from_norm_sq output.
        //
        // Construction:
        //  - in_dist examples: predicted close to actual → small residual → low OOD score
        //  - ood examples:     predicted far from actual → large residual → high OOD score
        // Therefore separation_auc should be ~1.0 (perfect discrimination).
        let holdout = vec![
            // in_dist: predicted matches actual at threshold 0.5
            calibration_example(vec![0.95, 0.05], vec![1.0, 0.0]),
            calibration_example(vec![0.9, 0.1], vec![1.0, 0.0]),
            calibration_example(vec![0.85, 0.15], vec![1.0, 0.0]),
            // ood: predictor flipped vs oracle
            calibration_example(vec![0.9, 0.05], vec![0.0, 1.0]),
            calibration_example(vec![0.95, 0.1], vec![0.0, 1.0]),
        ];
        let (auc, source) = compute_real_ood_separation_auc(&holdout, 1.0).unwrap();
        assert_eq!(source, "real_ood_score_readback");
        // OOD examples have far larger residuals than in_dist; AUC must be ≥ in_dist AUC
        // and strictly > 0.5 because of the structural separation.
        assert!(
            auc > 0.5,
            "real OOD subsystem must discriminate; got auc={auc}"
        );
        assert!(
            auc.is_finite() && (0.0..=1.0).contains(&auc),
            "auc out of range: {auc}"
        );
    }

    #[test]
    fn ood_separation_auc_fails_closed_on_dim_mismatch() {
        // M-H1 regression: malformed holdout (predicted/actual length mismatch)
        // must fail closed rather than silently truncate or zero-pad.
        let holdout = vec![calibration_example(vec![0.9, 0.1], vec![1.0])];
        let err = compute_real_ood_separation_auc(&holdout, 1.0).unwrap_err();
        assert_eq!(
            err.code(),
            "MEJEPA_INFER_TEST_CALIBRATION_HOLDOUT_UNAVAILABLE"
        );
    }
}
