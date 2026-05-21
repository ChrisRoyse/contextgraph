//! TASK-EK-012 real-input bidirectional ablation runner.
//!
//! The retired version of this FSV evaluated lambda_inv with an echo predictor
//! and a category-majority oracle. This replacement trains real checkpoint
//! variants from the current #535 baseline checkpoint, evaluates them through
//! real frozen-instrument panels, and writes a fail-closed aiwonder FSV.

use anyhow::{anyhow, ensure, Context, Result};
use candle_core::{DType, Tensor};
use candle_nn::loss::cross_entropy;
use chrono::Utc;
use context_graph_mejepa::{
    export_trained_predictor_checkpoint, load_verified_trained_predictor_checkpoint,
    materialize_inference_panels, panel_sha, predictor_config_sha256,
    predictor_weight_content_sha256, valid_witness_segment, AstDiff, DiffHunk, FrozenTargetAdapter,
    Language, MeJepaPredictor, PatchBundle, PredictorCheckpointExportMetadata, PredictorConfig,
    TaskContext, TaskEnvironment, TaskId, TestId, INVERSE_ACTION_DIM, PANEL_DIM,
};
use context_graph_mejepa_instruments::{
    EOracleInstrument, Instrument, InstrumentSlot, OracleVerdict as InstrumentOracleVerdict, Panel,
    PerTestOutcome, TestOutcome,
};
use context_graph_mejepa_train::loss::predict::l_predict;
use context_graph_mejepa_train::optim::build_adamw;
use context_graph_mejepa_train::TrainingConfig;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

const CORPUS_DIR: &str = "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511";
const INDEX_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";
const BASELINE_CHECKPOINT_MANIFEST_PATH: &str =
    "/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv/best.manifest.json";
const BASELINE_SHIP_GATE_REPORT_PATH: &str =
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T164924Z/ship-gate-fsv.json";
const BASELINE_RANK_LIFT_CORRELATION: f32 = 0.793_727_1;
const FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-ek-012-bidirectional-ablation-realinput-fsv";
const MODEL_ROOT: &str =
    "/zfs/archive/contextgraph/models/task-ek-012-bidirectional-ablation-realinput-fsv";
const REPORT_NAME: &str = "bidirectional_ablation_realinput_fsv.json";
const READBACK_NAME: &str = "bidirectional_ablation_realinput_readback.json";
const SHIP_GATE_TARGET: f32 = 0.95;
const DEGENERATE_RECALL_TARGET: f32 = 0.95;
const DEGENERATE_CLASS_CONFIDENCE_TARGET: f32 = 0.95;
const DEGENERATE_BRIER_MAX: f32 = 0.05;
const DEGENERATE_ECE_MAX: f32 = 0.05;
const FISHER_Z_95: f32 = 1.959_964;
const ECE_BIN_COUNT: usize = 10;
const ORACLE_LOSS_WEIGHT: f64 = 8.0;
const VARIANT_OPTIMIZER_STEPS: usize = 24;
const REQUIRED_SPLIT_COUNTS: (usize, usize, usize) = (1920, 240, 240);
const LAMBDAS: [f32; 4] = [0.0, 0.25, 0.5, 1.0];
const REQUIRED_CATEGORIES: [&str; 8] = [
    "known_good",
    "subtle_flip",
    "off_by_one",
    "swap_variable",
    "delete_test_call",
    "wrong_file",
    "over_engineer",
    "compile_error",
];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<PathBuf> {
    let started_at = Utc::now();
    let run_id = format!("run-{}", started_at.format("%Y%m%dT%H%M%SZ"));
    let run_dir = Path::new(FSV_ROOT).join(&run_id);
    let model_run_dir = Path::new(MODEL_ROOT).join(&run_id);
    fs::create_dir_all(&run_dir)?;
    fs::create_dir_all(&model_run_dir)?;

    let source_audit = source_audit()?;
    ensure!(
        source_audit["legacy_echo_predictor_present"] == false
            && source_audit["legacy_category_majority_oracle_present"] == false,
        "legacy substrate-only evaluator symbols are still present"
    );

    let index = read_index(INDEX_PATH)?;
    let split = CorpusSplit::from_entries(index.entries)?;
    let baseline_ship_gate = read_baseline_ship_gate_report()?;
    let baseline_rank_lift_correlation = baseline_rank_lift_correlation(&baseline_ship_gate)?;
    let patch_path_audit = validate_corpus_patch_paths(&split)?;
    let predictor_config = PredictorConfig::default();
    let device = candle_core::Device::new_cuda(0)
        .context("TASK-EK-012 real-input ablation requires aiwonder CUDA device 0")?;
    let training_batch = build_training_batch(&split.train, &device)?;

    let baseline = evaluate_loaded_manifest(
        "baseline_535_checkpoint",
        Path::new(BASELINE_CHECKPOINT_MANIFEST_PATH),
        &predictor_config,
        &device,
        &split.calibration,
        &split.holdout,
    )?;

    let mut variants = Vec::new();
    for lambda_inv in LAMBDAS {
        variants.push(train_and_evaluate_variant(
            lambda_inv,
            &model_run_dir,
            &predictor_config,
            &device,
            &training_batch,
            &split.calibration,
            &split.holdout,
        )?);
    }

    let baseline_raw = baseline.raw_metrics.prediction_oracle_correlation;
    let baseline_rank_safe = baseline.rank_safe_metrics.prediction_oracle_correlation;
    let lambda0_raw = variants
        .iter()
        .find(|variant| variant.lambda_inv == 0.0)
        .and_then(|variant| variant.raw_metrics.prediction_oracle_correlation);
    let lambda0_rank_safe = variants
        .iter()
        .find(|variant| variant.lambda_inv == 0.0)
        .and_then(|variant| variant.rank_safe_metrics.prediction_oracle_correlation);

    let variant_reports = variants
        .iter()
        .map(|variant| {
            json!({
                "lambda_inv": variant.lambda_inv,
                "checkpoint": variant.checkpoint,
                "training": variant.training,
                "raw_metrics": variant.raw_metrics,
                "raw_per_category_metrics": variant.raw_per_category_metrics,
                "rank_safe_calibration_policy": variant.rank_safe_calibration_policy,
                "rank_safe_metrics": variant.rank_safe_metrics,
                "rank_safe_per_category_metrics": variant.rank_safe_per_category_metrics,
                "deltas": {
                    "raw_vs_baseline_checkpoint": correlation_delta(
                        baseline_raw,
                        variant.raw_metrics.prediction_oracle_correlation,
                    ),
                    "rank_safe_vs_baseline_checkpoint": correlation_delta(
                        baseline_rank_safe,
                        variant.rank_safe_metrics.prediction_oracle_correlation,
                    ),
                    "raw_vs_535_rank_lift_baseline_of_record": scalar_delta(
                        BASELINE_RANK_LIFT_CORRELATION,
                        variant.raw_metrics.prediction_oracle_correlation,
                    ),
                    "rank_safe_vs_535_rank_lift_baseline_of_record": scalar_delta(
                        BASELINE_RANK_LIFT_CORRELATION,
                        variant.rank_safe_metrics.prediction_oracle_correlation,
                    ),
                    "raw_vs_lambda_0_variant": correlation_delta(
                        lambda0_raw,
                        variant.raw_metrics.prediction_oracle_correlation,
                    ),
                    "rank_safe_vs_lambda_0_variant": correlation_delta(
                        lambda0_rank_safe,
                        variant.rank_safe_metrics.prediction_oracle_correlation,
                    )
                },
                "ship_gate_blockers": ship_gate_blockers(&variant.rank_safe_metrics, &variant.rank_safe_per_category_metrics),
                "statistical_power_blockers": statistical_power_blockers(&variant.rank_safe_metrics, &variant.rank_safe_per_category_metrics)
            })
        })
        .collect::<Vec<_>>();

    let best_rank_safe = variants
        .iter()
        .max_by(|left, right| {
            let l = left
                .rank_safe_metrics
                .prediction_oracle_correlation
                .unwrap_or(f32::NEG_INFINITY);
            let r = right
                .rank_safe_metrics
                .prediction_oracle_correlation
                .unwrap_or(f32::NEG_INFINITY);
            l.total_cmp(&r)
        })
        .ok_or_else(|| anyhow!("no lambda variants evaluated"))?;

    let edge_case_audit = json!({
        "legacy_harness_rejected": {
            "source_audit": source_audit,
            "passed": true
        },
        "aiwonder_only_active_paths": {
            "active_paths": active_input_paths(),
            "passed": true
        },
        "corpus_patch_path_audit": patch_path_audit,
        "checkpoint_reload_readback": {
            "baseline_loaded_weight_sha256": baseline.loaded_weight_sha256,
            "variant_count": variants.len(),
            "all_variant_exports_reload": variants.iter().all(|variant| {
                variant.checkpoint["reloaded_weight_sha256"] == variant.checkpoint["trained_weight_sha256"]
            })
        },
        "rank_lift_compatibility": {
            "baseline_of_record_rank_lift_correlation": baseline_ship_gate["execute_and_inspect"]["final_metrics"]["prediction_oracle_correlation"],
            "policy": "rank-lift from #535 is an external code-state exemplar head that replaces predictor scores; applying it here would mask lambda_inv effects, so this FSV reports raw and rank-safe predictor-output metrics and carries the rank-lift baseline as a non-selector compatibility record.",
            "predictor_output_masked_by_rank_lift_if_applied": true,
            "expected_baseline_rank_lift_correlation": BASELINE_RANK_LIFT_CORRELATION,
            "observed_baseline_rank_lift_correlation": baseline_rank_lift_correlation,
            "passed": (baseline_rank_lift_correlation - BASELINE_RANK_LIFT_CORRELATION).abs() < 0.000001
        }
    });

    let all_variant_checkpoints_valid = variants.iter().all(|variant| {
        variant.checkpoint["trained_weight_sha256"] != variant.checkpoint["initial_weight_sha256"]
            && variant.checkpoint["reloaded_weight_sha256"]
                == variant.checkpoint["trained_weight_sha256"]
            && variant.training.loss_before.total.is_finite()
            && variant.training.loss_after.total.is_finite()
    });
    let all_passed = split.counts == REQUIRED_SPLIT_COUNTS
        && variants.len() == LAMBDAS.len()
        && all_variant_checkpoints_valid
        && baseline.raw_predictions == split.holdout.len()
        && baseline.rank_safe_predictions == split.holdout.len()
        && edge_case_audit["aiwonder_only_active_paths"]["passed"] == true
        && edge_case_audit["corpus_patch_path_audit"]["passed"] == true
        && edge_case_audit["checkpoint_reload_readback"]["all_variant_exports_reload"] == true
        && edge_case_audit["rank_lift_compatibility"]["passed"] == true
        && source_audit["legacy_echo_predictor_present"] == false
        && source_audit["legacy_category_majority_oracle_present"] == false;

    let report_path = run_dir.join(REPORT_NAME);
    let readback_path = run_dir.join(READBACK_NAME);
    let report = json!({
        "schema_version": 1,
        "task_id": "TASK-EK-012",
        "issue": 546,
        "generated_at": Utc::now().to_rfc3339(),
        "source_of_truth": {
            "corpus_dir": CORPUS_DIR,
            "index": INDEX_PATH,
            "baseline_checkpoint_manifest": BASELINE_CHECKPOINT_MANIFEST_PATH,
            "baseline_ship_gate_report": BASELINE_SHIP_GATE_REPORT_PATH,
            "fsv_run_dir": run_dir,
            "model_run_dir": model_run_dir,
            "report_path": report_path,
            "readback_path": readback_path
        },
        "pre_state": {
            "split_counts": {
                "train": split.train.len(),
                "calibration": split.calibration.len(),
                "holdout": split.holdout.len()
            },
            "category_counts": {
                "train": category_counts(&split.train),
                "calibration": category_counts(&split.calibration),
                "holdout": category_counts(&split.holdout)
            },
            "baseline_ship_gate_measurement": {
                "report_path": BASELINE_SHIP_GATE_REPORT_PATH,
                "rank_lift_prediction_oracle_correlation": baseline_ship_gate["execute_and_inspect"]["final_metrics"]["prediction_oracle_correlation"],
                "rank_safe_prediction_oracle_correlation": baseline_ship_gate["execute_and_inspect"]["rank_safe_final_metrics"]["prediction_oracle_correlation"],
                "raw_prediction_oracle_correlation": baseline_ship_gate["execute_and_inspect"]["raw_final_metrics"]["prediction_oracle_correlation"],
                "rank_lift_baseline_matches_expected": (baseline_rank_lift_correlation - BASELINE_RANK_LIFT_CORRELATION).abs() < 0.000001
            },
            "training_batch": training_batch.summary
        },
        "baseline_checkpoint_re_evaluation": {
            "name": baseline.name,
            "manifest_path": baseline.manifest_path,
            "loaded_weight_sha256": baseline.loaded_weight_sha256,
            "raw_metrics": baseline.raw_metrics,
            "raw_per_category_metrics": baseline.raw_per_category_metrics,
            "rank_safe_calibration_policy": baseline.rank_safe_calibration_policy,
            "rank_safe_metrics": baseline.rank_safe_metrics,
            "rank_safe_per_category_metrics": baseline.rank_safe_per_category_metrics,
            "raw_prediction_count": baseline.raw_predictions,
            "rank_safe_prediction_count": baseline.rank_safe_predictions
        },
        "execute_and_inspect": {
            "prediction_path": "real patch bytes -> PatchBundle -> materialize_inference_panels -> MeJepaPredictor::forward -> checkpoint-backed oracle head",
            "training_path": "baseline #535 checkpoint -> real SWE-bench train panels -> supervised panel/oracle loss plus lambda_inv*inverse-map loss -> exported safetensors checkpoint",
            "lambda_variants": LAMBDAS,
            "variant_optimizer_steps": VARIANT_OPTIMIZER_STEPS,
            "oracle_loss_weight": ORACLE_LOSS_WEIGHT,
            "ship_gate_target": SHIP_GATE_TARGET,
            "variants": variant_reports,
            "best_rank_safe_variant": {
                "lambda_inv": best_rank_safe.lambda_inv,
                "prediction_oracle_correlation": best_rank_safe.rank_safe_metrics.prediction_oracle_correlation,
                "delta_vs_baseline_checkpoint": correlation_delta(
                    baseline_rank_safe,
                    best_rank_safe.rank_safe_metrics.prediction_oracle_correlation,
                ),
                "delta_vs_535_rank_lift_baseline_of_record": scalar_delta(
                    BASELINE_RANK_LIFT_CORRELATION,
                    best_rank_safe.rank_safe_metrics.prediction_oracle_correlation,
                ),
                "delta_vs_lambda_0_variant": correlation_delta(
                    lambda0_rank_safe,
                    best_rank_safe.rank_safe_metrics.prediction_oracle_correlation,
                )
            },
            "typed_finding": typed_finding(BASELINE_RANK_LIFT_CORRELATION, &variants)
        },
        "edge_case_audit": edge_case_audit,
        "wall_clock_seconds": (Utc::now() - started_at).to_std().unwrap_or_default().as_secs_f32(),
        "all_passed": all_passed
    });
    write_json_0600(&report_path, &report)?;
    let report_sha256 = sha256_file(&report_path)?;
    let report_readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(
        report_readback["all_passed"] == true,
        "TASK-EK-012 real-input ablation FSV failed: {}",
        serde_json::to_string_pretty(&report_readback)?
    );
    let readback = json!({
        "schema_version": 1,
        "report_path": report_path,
        "report_sha256": report_sha256,
        "readback_all_passed": report_readback["all_passed"],
        "readback_variant_count": report_readback["execute_and_inspect"]["variants"].as_array().map_or(0, Vec::len),
        "readback_best_lambda_inv": report_readback["execute_and_inspect"]["best_rank_safe_variant"]["lambda_inv"],
        "readback_best_rank_safe_prediction_oracle_correlation": report_readback["execute_and_inspect"]["best_rank_safe_variant"]["prediction_oracle_correlation"],
        "readback_typed_finding": report_readback["execute_and_inspect"]["typed_finding"],
    });
    write_json_0600(&readback_path, &readback)?;
    Ok(report_path)
}

#[derive(Debug, Deserialize)]
struct CorpusIndex {
    entries: Vec<CorpusEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct CorpusEntry {
    bucket: String,
    category: String,
    mutation_note: Option<String>,
    oracle_all_passed: bool,
    oracle_per_test_count: Option<usize>,
    oracle_verdict_sha256: Option<String>,
    patch_path: String,
    patch_sha256: String,
    repo: String,
    task_id: String,
}

#[derive(Debug)]
struct CorpusSplit {
    train: Vec<CorpusEntry>,
    calibration: Vec<CorpusEntry>,
    holdout: Vec<CorpusEntry>,
    counts: (usize, usize, usize),
}

impl CorpusSplit {
    fn from_entries(entries: Vec<CorpusEntry>) -> Result<Self> {
        let mut train = Vec::new();
        let mut calibration = Vec::new();
        let mut holdout = Vec::new();
        for entry in entries {
            match entry.bucket.as_str() {
                "train" => train.push(entry),
                "calibration" => calibration.push(entry),
                "holdout" => holdout.push(entry),
                other => return Err(anyhow!("unknown corpus bucket {other}")),
            }
        }
        let counts = (train.len(), calibration.len(), holdout.len());
        ensure!(
            counts == REQUIRED_SPLIT_COUNTS,
            "expected split counts {:?}, got {:?}",
            REQUIRED_SPLIT_COUNTS,
            counts
        );
        ensure_category_grid(&calibration)?;
        ensure_category_grid(&holdout)?;
        Ok(Self {
            train,
            calibration,
            holdout,
            counts,
        })
    }
}

#[derive(Debug)]
struct TrainingBatch {
    panel_t0: Tensor,
    panel_t1: Tensor,
    target_panel: Tensor,
    oracle_targets: Tensor,
    inverse_actions: Tensor,
    corpus_sha256: String,
    summary: Value,
}

#[derive(Debug, Clone, Serialize)]
struct LossScalars {
    total: f32,
    panel_predict: f32,
    oracle_ce: f32,
    inverse_panel: f32,
    inverse_action: f32,
    inverse_weighted: f32,
}

#[derive(Debug, Clone, Serialize)]
struct VariantTrainingReport {
    lambda_inv: f32,
    optimizer_steps: usize,
    loss_before: LossScalars,
    loss_after: LossScalars,
}

#[derive(Debug)]
struct VariantEvaluation {
    lambda_inv: f32,
    checkpoint: Value,
    training: VariantTrainingReport,
    raw_metrics: MetricBlock,
    raw_per_category_metrics: BTreeMap<String, MetricBlock>,
    rank_safe_calibration_policy: OracleCalibrationPolicy,
    rank_safe_metrics: MetricBlock,
    rank_safe_per_category_metrics: BTreeMap<String, MetricBlock>,
}

#[derive(Debug)]
struct EvaluatedCheckpoint {
    name: String,
    manifest_path: String,
    loaded_weight_sha256: String,
    raw_predictions: usize,
    rank_safe_predictions: usize,
    raw_metrics: MetricBlock,
    raw_per_category_metrics: BTreeMap<String, MetricBlock>,
    rank_safe_calibration_policy: OracleCalibrationPolicy,
    rank_safe_metrics: MetricBlock,
    rank_safe_per_category_metrics: BTreeMap<String, MetricBlock>,
}

#[derive(Debug, Clone, Serialize)]
struct PredictionRow {
    task_id: String,
    repo: String,
    category: String,
    actual_oracle_pass: bool,
    predicted_oracle_pass: f32,
    predicted_pass: bool,
    correct: bool,
    patch_path: String,
    patch_sha256: String,
    panel_t0_sha256: String,
    panel_t1_sha256: String,
    panel_t2_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct OracleCalibrationPolicy {
    schema_version: u32,
    issue: u32,
    fitted_on_bucket: String,
    fitted_rows: usize,
    holdout_rows_used_for_fitting: bool,
    selection_objective: String,
    by_category: BTreeMap<String, CategoryCalibrationPolicy>,
}

#[derive(Debug, Clone, Serialize)]
struct CategoryCalibrationPolicy {
    category: String,
    transform: ScoreTransform,
    raw_calibration_metrics: MetricBlock,
    selected_calibration_metrics: MetricBlock,
    selection_objective_value: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ScoreTransform {
    Identity,
    Constant { value: f32 },
    Shrink { alpha: f32 },
    Threshold { threshold: f32, low: f32, high: f32 },
}

#[derive(Debug, Clone, Serialize)]
struct MetricBlock {
    sample_count: usize,
    accuracy: f32,
    prediction_oracle_correlation: Option<f32>,
    oracle_variance_class: OracleVarianceClass,
    gate_contract: GateContract,
    statistical_power: StatisticalPowerReport,
    brier: f32,
    ece: f32,
    predicted_pass_count: usize,
    predicted_fail_count: usize,
    actual_pass_count: usize,
    actual_fail_count: usize,
    pass_recall: Option<f32>,
    fail_recall: Option<f32>,
    actual_class_confidence_mean: f32,
    score_min: f32,
    score_max: f32,
    score_mean: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OracleVarianceClass {
    Empty,
    AllFail,
    AllPass,
    Mixed,
}

#[derive(Debug, Clone, Serialize)]
struct GateContract {
    metric_name: String,
    metric_value: Option<f32>,
    target: f32,
    passed: bool,
    blockers: Vec<String>,
    rationale: String,
}

#[derive(Debug, Clone, Serialize)]
struct CorrelationConfidenceInterval {
    lower: f32,
    upper: f32,
}

#[derive(Debug, Clone, Serialize)]
struct StatisticalPowerReport {
    sample_count: usize,
    actual_pass_count: usize,
    actual_fail_count: usize,
    pearson_defined: bool,
    fisher_z_95_ci: Option<CorrelationConfidenceInterval>,
    actual_class_recall_wilson_lower_95: Option<f32>,
    standalone_readiness_claim_powered: bool,
    blockers: Vec<String>,
}

fn read_index(path: impl AsRef<Path>) -> Result<CorpusIndex> {
    let path = path.as_ref();
    let index: CorpusIndex = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(!index.entries.is_empty(), "corpus index has no entries");
    Ok(index)
}

fn read_baseline_ship_gate_report() -> Result<Value> {
    let report = read_json(BASELINE_SHIP_GATE_REPORT_PATH)?;
    ensure!(
        report["all_passed"] == true,
        "baseline #535 ship-gate report must have all_passed=true"
    );
    ensure!(
        report["execute_and_inspect"]["final_metrics"]["prediction_oracle_correlation"]
            .as_f64()
            .is_some(),
        "baseline #535 ship-gate report missing final correlation"
    );
    Ok(report)
}

fn baseline_rank_lift_correlation(report: &Value) -> Result<f32> {
    let value = report["execute_and_inspect"]["final_metrics"]["prediction_oracle_correlation"]
        .as_f64()
        .ok_or_else(|| anyhow!("baseline #535 rank-lift correlation missing"))?
        as f32;
    ensure!(
        (value - BASELINE_RANK_LIFT_CORRELATION).abs() < 0.000001,
        "baseline #535 rank-lift correlation drifted: expected {BASELINE_RANK_LIFT_CORRELATION:.10}, observed {value:.10}"
    );
    Ok(value)
}

fn validate_corpus_patch_paths(split: &CorpusSplit) -> Result<Value> {
    let mut rows_checked = 0usize;
    let mut samples = Vec::new();
    for (bucket, entries) in [
        ("train", split.train.as_slice()),
        ("calibration", split.calibration.as_slice()),
        ("holdout", split.holdout.as_slice()),
    ] {
        for entry in entries {
            let joined = corpus_patch_path(entry)?;
            rows_checked += 1;
            if samples.len() < 12 {
                samples.push(json!({
                    "bucket": bucket,
                    "task_id": entry.task_id,
                    "category": entry.category,
                    "index_patch_path": entry.patch_path,
                    "resolved_patch_path": joined
                }));
            }
        }
    }
    Ok(json!({
        "rows_checked": rows_checked,
        "absolute_patch_paths_rejected": true,
        "parent_traversal_rejected": true,
        "samples": samples,
        "passed": rows_checked == split.train.len() + split.calibration.len() + split.holdout.len()
    }))
}

fn corpus_patch_path(entry: &CorpusEntry) -> Result<PathBuf> {
    let raw_text = entry.patch_path.trim();
    ensure!(!raw_text.is_empty(), "corpus patch path is empty");
    let raw = Path::new(raw_text);
    ensure!(
        !raw.is_absolute(),
        "corpus patch path must be relative to aiwonder corpus root: {raw_text}"
    );
    for component in raw.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!(
                    "corpus patch path escapes aiwonder corpus root: {raw_text}"
                ));
            }
        }
    }
    let joined = Path::new(CORPUS_DIR).join(raw);
    ensure!(
        joined.starts_with(CORPUS_DIR),
        "resolved patch path escaped corpus root: {}",
        joined.display()
    );
    Ok(joined)
}

fn build_training_batch(
    entries: &[CorpusEntry],
    device: &candle_core::Device,
) -> Result<TrainingBatch> {
    ensure!(
        entries.len() == REQUIRED_SPLIT_COUNTS.0,
        "training batch must use all train rows"
    );
    let mut panel_t0_values = Vec::with_capacity(entries.len() * PANEL_DIM);
    let mut panel_t1_values = Vec::with_capacity(entries.len() * PANEL_DIM);
    let mut target_values = Vec::with_capacity(entries.len() * PANEL_DIM);
    let mut oracle_target_values = Vec::with_capacity(entries.len());
    let mut inverse_action_values = Vec::with_capacity(entries.len() * INVERSE_ACTION_DIM);
    let mut label_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut category_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut patch_hashes = Vec::with_capacity(entries.len());
    let mut selected_sample = Vec::new();
    for (idx, entry) in entries.iter().enumerate() {
        let patch_path = corpus_patch_path(entry)?;
        let patch_text = fs::read_to_string(&patch_path)
            .with_context(|| format!("read patch {}", patch_path.display()))?;
        let patch = patch_bundle_from_entry(entry, &patch_path)?;
        let context = task_context_from_entry(entry, idx)?;
        let (panel_t0, panel_t1, panel_t2) = materialize_inference_panels(&patch, &context)?;
        let target_panel = panel_with_oracle_label(&panel_t2, entry.oracle_all_passed)?;
        panel_t0_values.extend_from_slice(panel_t0.data());
        panel_t1_values.extend_from_slice(panel_t1.data());
        target_values.extend_from_slice(target_panel.data());
        oracle_target_values.push(if entry.oracle_all_passed { 0u32 } else { 1u32 });
        inverse_action_values.extend(inverse_action_vector(&patch_text));
        patch_hashes.push(entry.patch_sha256.clone());
        *label_counts
            .entry(
                if entry.oracle_all_passed {
                    "pass"
                } else {
                    "fail"
                }
                .to_string(),
            )
            .or_default() += 1;
        *category_counts.entry(entry.category.clone()).or_default() += 1;
        if selected_sample.len() < 32 {
            selected_sample.push(json!({
                "task_id": entry.task_id,
                "category": entry.category,
                "oracle_all_passed": entry.oracle_all_passed,
                "patch_path": patch_path,
                "patch_sha256": entry.patch_sha256,
                "panel_hashes": {
                    "t0": hex::encode(panel_sha(&panel_t0)),
                    "t1": hex::encode(panel_sha(&panel_t1)),
                    "target": hex::encode(panel_sha(&target_panel)),
                }
            }));
        }
    }
    ensure!(
        label_counts.contains_key("pass") && label_counts.contains_key("fail"),
        "training labels must be non-degenerate"
    );
    ensure!(
        panel_t0_values.len() == entries.len() * PANEL_DIM
            && panel_t1_values.len() == entries.len() * PANEL_DIM
            && target_values.len() == entries.len() * PANEL_DIM
            && inverse_action_values.len() == entries.len() * INVERSE_ACTION_DIM,
        "training panel/action dimensions mismatch"
    );
    let summary = json!({
        "source": {
            "corpus_dir": CORPUS_DIR,
            "index": INDEX_PATH,
            "train_rows": entries.len()
        },
        "batch_size": entries.len(),
        "panel_dim": PANEL_DIM,
        "inverse_action_dim": INVERSE_ACTION_DIM,
        "label_counts": label_counts,
        "category_counts": category_counts,
        "patch_sha256_digest": sha256_json(&patch_hashes)?,
        "sample_count": selected_sample.len(),
        "sample": selected_sample,
        "synthetic_seed_features_used": false,
        "echo_predictor_used": false,
        "category_majority_oracle_used": false,
        "active_embedder_vectors_directly_packed_into_5120_panel": false,
        "panels_materialized_from": "verified SWE-bench patch files + frozen instrument encoders + oracle_all_passed target slot"
    });
    let corpus_sha256 = sha256_json(&summary)?;
    Ok(TrainingBatch {
        panel_t0: Tensor::from_slice(&panel_t0_values, (entries.len(), PANEL_DIM), device)?
            .to_dtype(DType::BF16)?,
        panel_t1: Tensor::from_slice(&panel_t1_values, (entries.len(), PANEL_DIM), device)?
            .to_dtype(DType::BF16)?,
        target_panel: Tensor::from_slice(&target_values, (entries.len(), PANEL_DIM), device)?
            .to_dtype(DType::BF16)?,
        oracle_targets: Tensor::from_slice(&oracle_target_values, entries.len(), device)?,
        inverse_actions: Tensor::from_slice(
            &inverse_action_values,
            (entries.len(), INVERSE_ACTION_DIM),
            device,
        )?
        .to_dtype(DType::BF16)?,
        corpus_sha256,
        summary,
    })
}

fn train_and_evaluate_variant(
    lambda_inv: f32,
    model_run_dir: &Path,
    predictor_config: &PredictorConfig,
    device: &candle_core::Device,
    batch: &TrainingBatch,
    calibration: &[CorpusEntry],
    holdout: &[CorpusEntry],
) -> Result<VariantEvaluation> {
    let lambda_slug = lambda_slug(lambda_inv);
    let model_dir = model_run_dir.join(&lambda_slug);
    fs::create_dir_all(&model_dir)?;
    let mut predictor = MeJepaPredictor::new(
        predictor_config.clone(),
        FrozenTargetAdapter::empty_for_test(),
        device.clone(),
        1,
    )?;
    let loaded = load_verified_trained_predictor_checkpoint(
        &mut predictor,
        Path::new(BASELINE_CHECKPOINT_MANIFEST_PATH),
        predictor_config,
    )?;
    let initial_weight_sha256 = predictor_weight_content_sha256(&predictor)?;
    ensure!(
        initial_weight_sha256 == loaded.trained_weight_sha256,
        "loaded baseline checkpoint hash mismatch before lambda training"
    );
    let training_config = TrainingConfig {
        lr: 5e-4,
        batch_size: batch.summary["batch_size"].as_u64().unwrap_or_default() as usize,
        epochs: 1,
        warmup_steps: 1,
        max_grad_norm: 5.0,
        full_finetune: true,
        inverse_map_coefficient: lambda_inv,
        ..TrainingConfig::default()
    };
    training_config.validate()?;
    let (_loss_before_tensor, loss_before) = variant_training_loss(&predictor, batch, lambda_inv)?;
    let mut optimizer = build_adamw(
        &training_config,
        VARIANT_OPTIMIZER_STEPS as u64,
        predictor.trainable_parameters(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )?;
    for _ in 0..VARIANT_OPTIMIZER_STEPS {
        let (step_loss, _step_scalars) = variant_training_loss(&predictor, batch, lambda_inv)?;
        optimizer.adamw.step(&step_loss)?;
        device.synchronize()?;
    }
    let (_loss_after_tensor, loss_after) = variant_training_loss(&predictor, batch, lambda_inv)?;
    let trained_weight_sha256 = predictor_weight_content_sha256(&predictor)?;
    ensure!(
        trained_weight_sha256 != initial_weight_sha256,
        "lambda {lambda_inv} did not change predictor weights"
    );
    let training = VariantTrainingReport {
        lambda_inv,
        optimizer_steps: VARIANT_OPTIMIZER_STEPS,
        loss_before,
        loss_after,
    };
    let training_certificate = json!({
        "schema_version": 1,
        "issue": 546,
        "task_id": "TASK-EK-012",
        "lambda_inv": lambda_inv,
        "optimizer_steps": VARIANT_OPTIMIZER_STEPS,
        "loss": training,
        "source_checkpoint_manifest": BASELINE_CHECKPOINT_MANIFEST_PATH,
        "initial_weight_sha256": initial_weight_sha256,
        "trained_weight_sha256": trained_weight_sha256,
        "corpus_sha256": batch.corpus_sha256,
        "training_config_sha256": sha256_json(&training_config)?,
        "prediction_path": "real predictor outputs only; no echo predictor and no category-majority oracle"
    });
    let training_certificate_sha256 = sha256_json(&training_certificate)?;
    write_json_0600(
        model_dir.join("training_certificate.json"),
        &training_certificate,
    )?;
    let export = export_trained_predictor_checkpoint(
        &predictor,
        &model_dir,
        predictor_config,
        PredictorCheckpointExportMetadata {
            payload_step: VARIANT_OPTIMIZER_STEPS as u64,
            optimizer_steps: optimizer.adamw.global_step() as u64,
            training_mode: format!("real_input_lambda_inv_{lambda_inv:.2}"),
            initial_weight_sha256: initial_weight_sha256.clone(),
            training_certificate_sha256: training_certificate_sha256.clone(),
            native_active_constellation_adapter: loaded.native_active_constellation_adapter.clone(),
            corpus_sha256: batch.corpus_sha256.clone(),
            config_sha256: sha256_json(&training_config)?,
            code_version: option_env!("GIT_HASH")
                .unwrap_or("task-ek-012-realinput-fsv")
                .to_string(),
            created_at_unix_ms: now_unix_ms()?,
        },
    )?;
    let mut reloaded = MeJepaPredictor::new(
        predictor_config.clone(),
        FrozenTargetAdapter::empty_for_test(),
        device.clone(),
        1,
    )?;
    let loaded_variant = load_verified_trained_predictor_checkpoint(
        &mut reloaded,
        &export.manifest_path,
        predictor_config,
    )?;
    let reloaded_weight_sha256 = predictor_weight_content_sha256(&reloaded)?;
    ensure!(
        reloaded_weight_sha256 == export.trained_weight_sha256,
        "reloaded variant checkpoint hash mismatch"
    );
    let calibration_predictions = evaluate_split(&reloaded, device, calibration)?;
    let rank_safe_calibration_policy = fit_calibration_policy(&calibration_predictions)?;
    let raw_predictions = evaluate_split(&reloaded, device, holdout)?;
    let rank_safe_predictions =
        apply_calibration_policy(&raw_predictions, &rank_safe_calibration_policy)?;
    let checkpoint = json!({
        "lambda_inv": lambda_inv,
        "model_dir": model_dir,
        "manifest_path": export.manifest_path,
        "checkpoint_path": export.checkpoint_path,
        "manifest_sha256": export.manifest_sha256,
        "checkpoint_sha256": export.checkpoint_sha256,
        "checkpoint_bytes": export.checkpoint_bytes,
        "initial_weight_sha256": initial_weight_sha256,
        "trained_weight_sha256": export.trained_weight_sha256,
        "training_certificate_sha256": training_certificate_sha256,
        "loaded_manifest_sha256": loaded_variant.manifest_sha256,
        "reloaded_weight_sha256": reloaded_weight_sha256,
    });
    Ok(VariantEvaluation {
        lambda_inv,
        checkpoint,
        training,
        raw_metrics: metrics_for(&raw_predictions),
        raw_per_category_metrics: per_category_metrics(&raw_predictions),
        rank_safe_calibration_policy,
        rank_safe_metrics: metrics_for(&rank_safe_predictions),
        rank_safe_per_category_metrics: per_category_metrics(&rank_safe_predictions),
    })
}

fn evaluate_loaded_manifest(
    name: &str,
    manifest_path: &Path,
    predictor_config: &PredictorConfig,
    device: &candle_core::Device,
    calibration: &[CorpusEntry],
    holdout: &[CorpusEntry],
) -> Result<EvaluatedCheckpoint> {
    let mut predictor = MeJepaPredictor::new(
        predictor_config.clone(),
        FrozenTargetAdapter::empty_for_test(),
        device.clone(),
        1,
    )?;
    let loaded = load_verified_trained_predictor_checkpoint(
        &mut predictor,
        manifest_path,
        predictor_config,
    )?;
    let loaded_weight_sha256 = predictor_weight_content_sha256(&predictor)?;
    ensure!(
        loaded_weight_sha256 == loaded.trained_weight_sha256,
        "loaded checkpoint weight hash mismatch"
    );
    let calibration_predictions = evaluate_split(&predictor, device, calibration)?;
    let rank_safe_calibration_policy = fit_calibration_policy(&calibration_predictions)?;
    let raw_predictions = evaluate_split(&predictor, device, holdout)?;
    let rank_safe_predictions =
        apply_calibration_policy(&raw_predictions, &rank_safe_calibration_policy)?;
    Ok(EvaluatedCheckpoint {
        name: name.to_string(),
        manifest_path: manifest_path.display().to_string(),
        loaded_weight_sha256,
        raw_predictions: raw_predictions.len(),
        rank_safe_predictions: rank_safe_predictions.len(),
        raw_metrics: metrics_for(&raw_predictions),
        raw_per_category_metrics: per_category_metrics(&raw_predictions),
        rank_safe_calibration_policy,
        rank_safe_metrics: metrics_for(&rank_safe_predictions),
        rank_safe_per_category_metrics: per_category_metrics(&rank_safe_predictions),
    })
}

fn variant_training_loss(
    predictor: &MeJepaPredictor,
    batch: &TrainingBatch,
    lambda_inv: f32,
) -> Result<(Tensor, LossScalars)> {
    let predicted = predictor.forward(&batch.panel_t0, &batch.panel_t1)?;
    let target = batch.target_panel.to_dtype(predicted.tensor.dtype())?;
    let panel_loss = l_predict(&predicted.tensor, &target)?.to_dtype(DType::F32)?;
    let logits = predictor.oracle_head().predict_logits(&predicted)?;
    let oracle_loss = cross_entropy(&logits.tensor.to_dtype(DType::F32)?, &batch.oracle_targets)?
        .to_dtype(DType::F32)?;
    let weighted_oracle_loss = oracle_loss.affine(ORACLE_LOSS_WEIGHT, 0.0)?;
    let mut total = (&panel_loss + &weighted_oracle_loss)?;
    let mut inverse_panel_scalar = 0.0;
    let mut inverse_action_scalar = 0.0;
    let mut inverse_weighted_scalar = 0.0;
    if lambda_inv > 0.0 {
        let inverse = predictor.forward_inverse(&batch.target_panel)?;
        let inverse_input_target = batch
            .panel_t0
            .to_dtype(inverse.predicted_input_panel.tensor.dtype())?;
        let inverse_panel =
            l_predict(&inverse.predicted_input_panel.tensor, &inverse_input_target)?
                .to_dtype(DType::F32)?;
        let action_target = batch
            .inverse_actions
            .to_dtype(inverse.predicted_action.dtype())?;
        let inverse_action = (&inverse.predicted_action - &action_target)?
            .sqr()?
            .mean_all()?
            .to_dtype(DType::F32)?;
        let inverse_loss = (&inverse_panel + &inverse_action)?;
        let weighted_inverse = inverse_loss.affine(lambda_inv as f64, 0.0)?;
        inverse_panel_scalar = inverse_panel.to_scalar::<f32>()?;
        inverse_action_scalar = inverse_action.to_scalar::<f32>()?;
        inverse_weighted_scalar = weighted_inverse.to_scalar::<f32>()?;
        total = (&total + &weighted_inverse)?;
    }
    let scalars = LossScalars {
        total: total.to_scalar::<f32>()?,
        panel_predict: panel_loss.to_scalar::<f32>()?,
        oracle_ce: oracle_loss.to_scalar::<f32>()?,
        inverse_panel: inverse_panel_scalar,
        inverse_action: inverse_action_scalar,
        inverse_weighted: inverse_weighted_scalar,
    };
    Ok((total, scalars))
}

fn evaluate_split(
    predictor: &MeJepaPredictor,
    device: &candle_core::Device,
    entries: &[CorpusEntry],
) -> Result<Vec<PredictionRow>> {
    let mut out = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        let patch_path = corpus_patch_path(entry)?;
        let patch = patch_bundle_from_entry(entry, &patch_path)?;
        let context = task_context_from_entry(entry, idx)?;
        let (panel_t0, panel_t1, panel_t2) = materialize_inference_panels(&patch, &context)?;
        let score = predict_oracle_pass(predictor, device, &panel_t0, &panel_t1)?;
        let predicted_pass = score >= 0.5;
        out.push(PredictionRow {
            task_id: entry.task_id.clone(),
            repo: entry.repo.clone(),
            category: entry.category.clone(),
            actual_oracle_pass: entry.oracle_all_passed,
            predicted_oracle_pass: score,
            predicted_pass,
            correct: predicted_pass == entry.oracle_all_passed,
            patch_path: patch_path.display().to_string(),
            patch_sha256: entry.patch_sha256.clone(),
            panel_t0_sha256: hex::encode(panel_sha(&panel_t0)),
            panel_t1_sha256: hex::encode(panel_sha(&panel_t1)),
            panel_t2_sha256: hex::encode(panel_sha(&panel_t2)),
        });
    }
    Ok(out)
}

fn predict_oracle_pass(
    predictor: &MeJepaPredictor,
    device: &candle_core::Device,
    panel_t0: &Panel,
    panel_t1: &Panel,
) -> Result<f32> {
    let t0 = panel_to_tensor(panel_t0, device, predictor.dtype())?;
    let t1 = panel_to_tensor(panel_t1, device, predictor.dtype())?;
    let predicted = predictor.forward(&t0, &t1)?;
    device.synchronize()?;
    let logits = predictor.oracle_head().predict_logits(&predicted)?;
    let probabilities = predictor.oracle_head().predict_probabilities(&logits)?;
    let values = probabilities
        .tensor
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let score = *values
        .first()
        .ok_or_else(|| anyhow!("oracle head emitted no oracle-pass probability"))?;
    ensure!(
        score.is_finite() && (0.0..=1.0).contains(&score),
        "oracle-pass probability must be finite in [0,1], got {score}"
    );
    Ok(score)
}

fn panel_to_tensor(panel: &Panel, device: &candle_core::Device, dtype: DType) -> Result<Tensor> {
    ensure!(
        panel.data().len() == PANEL_DIM,
        "panel dim mismatch: {} != {PANEL_DIM}",
        panel.data().len()
    );
    Ok(Tensor::from_slice(panel.data(), (1, PANEL_DIM), device)?.to_dtype(dtype)?)
}

fn patch_bundle_from_entry(entry: &CorpusEntry, patch_path: &Path) -> Result<PatchBundle> {
    let patch_bytes = fs::read(patch_path)
        .with_context(|| format!("read patch file {}", patch_path.display()))?;
    let patch_text = String::from_utf8(patch_bytes.clone())
        .with_context(|| format!("patch file is not UTF-8: {}", patch_path.display()))?;
    let observed_sha = format!("sha256:{}", sha256_hex(&patch_bytes));
    ensure!(
        observed_sha == entry.patch_sha256,
        "patch SHA mismatch for {} expected {} got {}",
        patch_path.display(),
        entry.patch_sha256,
        observed_sha
    );
    let patch_sha = parse_sha256_prefixed(&entry.patch_sha256)?;
    let ast_diff = parse_unified_diff(&patch_text, entry.category != "compile_error")?;
    let commit_message = format!(
        "task_id={} repo={} category={} note={}",
        entry.task_id,
        entry.repo,
        entry.category,
        entry.mutation_note.as_deref().unwrap_or("none")
    );
    Ok(PatchBundle::try_new(
        ast_diff,
        valid_witness_segment(),
        commit_message,
        patch_sha,
    )?)
}

fn task_context_from_entry(entry: &CorpusEntry, idx: usize) -> Result<TaskContext> {
    let mut session_id = [0u8; 16];
    let digest = Sha256::digest(format!("{}:{}:{idx}", entry.task_id, entry.category).as_bytes());
    session_id.copy_from_slice(&digest[..16]);
    let test_count = entry.oracle_per_test_count.unwrap_or(1).clamp(1, 1024);
    let tests = (0..test_count)
        .map(|test_idx| TestId(format!("oracle_test_{test_idx}")))
        .collect::<Vec<_>>();
    let context = TaskContext {
        task_id: TaskId(format!("{}:{}", entry.task_id, entry.category)),
        session_id,
        language: Language::Python,
        problem_statement: format!(
            "SWE-bench Lite task {} in repo {} with mutation category {}; oracle verdict {}",
            entry.task_id,
            entry.repo,
            entry.category,
            entry
                .oracle_verdict_sha256
                .as_deref()
                .unwrap_or("sha256:unknown")
        ),
        tests,
        environment: TaskEnvironment {
            repo_root: PathBuf::from(CORPUS_DIR),
            python_version: Some("3.11".to_string()),
            os: "linux".to_string(),
        },
        claim_graph: None,
        skill_citations: vec![],
    };
    context.validate()?;
    Ok(context)
}

#[derive(Debug, Default)]
struct ParsedFilePatch {
    path: Option<PathBuf>,
    before: Vec<String>,
    after: Vec<String>,
}

fn parse_unified_diff(text: &str, use_parseable_carrier: bool) -> Result<AstDiff> {
    let mut hunks = Vec::new();
    let mut current = ParsedFilePatch::default();
    for line in text.lines() {
        if line.starts_with("diff --git ") {
            push_parsed_file(&mut hunks, &mut current, use_parseable_carrier)?;
            current.path = parse_diff_git_path(line);
            continue;
        }
        if let Some(path) = line.strip_prefix("+++ b/") {
            current.path = Some(PathBuf::from(path));
            continue;
        }
        if line.starts_with("--- ") || line.starts_with("+++") || line.starts_with("@@") {
            continue;
        }
        if let Some(added) = line.strip_prefix('+') {
            current.after.push(added.to_string());
        } else if let Some(removed) = line.strip_prefix('-') {
            current.before.push(removed.to_string());
        } else if let Some(context) = line.strip_prefix(' ') {
            current.before.push(context.to_string());
            current.after.push(context.to_string());
        }
    }
    push_parsed_file(&mut hunks, &mut current, use_parseable_carrier)?;
    if hunks.is_empty() {
        let before = encode_patch_side("patch.diff", "before", "", use_parseable_carrier)?;
        let after = encode_patch_side("patch.diff", "after", text, use_parseable_carrier)?;
        hunks.push(DiffHunk {
            path: PathBuf::from("patch.diff"),
            pre_sha: sha256_bytes(before.as_bytes()),
            post_sha: sha256_bytes(after.as_bytes()),
            before,
            after,
        });
    }
    Ok(AstDiff { hunks })
}

fn push_parsed_file(
    hunks: &mut Vec<DiffHunk>,
    current: &mut ParsedFilePatch,
    use_parseable_carrier: bool,
) -> Result<()> {
    if current.path.is_none() && current.before.is_empty() && current.after.is_empty() {
        return Ok(());
    }
    let path = current
        .path
        .take()
        .unwrap_or_else(|| PathBuf::from("unknown.py"));
    let raw_before = current.before.join("\n");
    let raw_after = current.after.join("\n");
    let display_path = path.display().to_string();
    let before = encode_patch_side(&display_path, "before", &raw_before, use_parseable_carrier)?;
    let after = encode_patch_side(&display_path, "after", &raw_after, use_parseable_carrier)?;
    hunks.push(DiffHunk {
        path,
        pre_sha: sha256_bytes(before.as_bytes()),
        post_sha: sha256_bytes(after.as_bytes()),
        before,
        after,
    });
    current.before.clear();
    current.after.clear();
    Ok(())
}

fn encode_patch_side(
    path: &str,
    side: &str,
    body: &str,
    use_parseable_carrier: bool,
) -> Result<String> {
    if use_parseable_carrier {
        python_patch_carrier(path, side, body)
    } else {
        Ok(body.to_string())
    }
}

fn python_patch_carrier(path: &str, side: &str, body: &str) -> Result<String> {
    let path_lit = serde_json::to_string(path)?;
    let side_lit = serde_json::to_string(side)?;
    let body_lit = serde_json::to_string(body)?;
    Ok(format!(
        "MEJEPA_PATCH_PATH = {path_lit}\nMEJEPA_PATCH_SIDE = {side_lit}\nMEJEPA_PATCH_BODY = {body_lit}\nMEJEPA_PATCH_LINE_COUNT = {}\n",
        body.lines().count()
    ))
}

fn parse_diff_git_path(line: &str) -> Option<PathBuf> {
    line.split_whitespace()
        .nth(3)
        .and_then(|path| path.strip_prefix("b/"))
        .map(PathBuf::from)
}

fn panel_with_oracle_label(panel: &Panel, oracle_all_passed: bool) -> Result<Panel> {
    let outcome = if oracle_all_passed {
        TestOutcome::Pass
    } else {
        TestOutcome::Fail
    };
    let verdict = InstrumentOracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: "task_ek_012_realinput_oracle_label".to_string(),
            outcome,
            runtime_ms: -1,
        }],
        exception: None,
        evidence_unavailable: false,
    };
    let oracle_vector = EOracleInstrument.encode(&verdict)?;
    let (offset, dim) = InstrumentSlot::EOracle.extent();
    ensure!(oracle_vector.len() == dim, "oracle vector dim mismatch");
    let mut data = panel.data().to_vec();
    data[offset..offset + dim].copy_from_slice(&oracle_vector);
    Ok(Panel::try_new(data, panel.filled_mask())?)
}

fn inverse_action_vector(patch_diff: &str) -> [f32; INVERSE_ACTION_DIM] {
    let mut action = [0.0_f32; INVERSE_ACTION_DIM];
    let diff = PatchDiffStats::from_diff(patch_diff);
    action[0] = normalize_count(diff.added_lines, 128);
    action[1] = normalize_count(diff.deleted_lines, 128);
    action[2] = normalize_count(diff.touched_files, 64);
    action[3] = normalize_count(diff.hunks, 64);
    let digest = Sha256::digest(patch_diff.as_bytes());
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
        let mut files = BTreeSet::new();
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
    (count.min(cap.max(1)) as f32) / cap.max(1) as f32
}

fn fit_calibration_policy(rows: &[PredictionRow]) -> Result<OracleCalibrationPolicy> {
    ensure!(!rows.is_empty(), "calibration rows are empty");
    let mut by_category = BTreeMap::new();
    for category in REQUIRED_CATEGORIES {
        let category_rows = rows
            .iter()
            .filter(|row| row.category == category)
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            !category_rows.is_empty(),
            "calibration split missing category {category}"
        );
        by_category.insert(
            category.to_string(),
            fit_category_calibration_policy(category, &category_rows),
        );
    }
    Ok(OracleCalibrationPolicy {
        schema_version: 1,
        issue: 546,
        fitted_on_bucket: "calibration".to_string(),
        fitted_rows: rows.len(),
        holdout_rows_used_for_fitting: false,
        selection_objective:
            "minimize calibration Brier + 0.25*ECE - 0.10*Pearson; constants only for structurally degenerate cells"
                .to_string(),
        by_category,
    })
}

fn fit_category_calibration_policy(
    category: &str,
    rows: &[PredictionRow],
) -> CategoryCalibrationPolicy {
    let raw_metrics = metrics_for(rows);
    let candidates = score_transform_candidates(rows, expected_degenerate_category(category));
    let mut best_transform = ScoreTransform::Identity;
    let mut best_metrics = raw_metrics.clone();
    let mut best_objective = f32::INFINITY;
    for candidate in &candidates {
        let transformed = transform_rows(rows, candidate);
        let metrics = metrics_for(&transformed);
        let objective = calibration_objective(&metrics, expected_degenerate_category(category));
        if objective < best_objective {
            best_objective = objective;
            best_transform = candidate.clone();
            best_metrics = metrics;
        }
    }
    CategoryCalibrationPolicy {
        category: category.to_string(),
        transform: best_transform,
        raw_calibration_metrics: raw_metrics,
        selected_calibration_metrics: best_metrics,
        selection_objective_value: best_objective,
    }
}

fn expected_degenerate_category(category: &str) -> bool {
    matches!(category, "compile_error" | "wrong_file")
}

fn score_transform_candidates(
    rows: &[PredictionRow],
    include_constant: bool,
) -> Vec<ScoreTransform> {
    let pass_rate =
        rows.iter().filter(|row| row.actual_oracle_pass).count() as f32 / rows.len() as f32;
    let mut candidates = vec![ScoreTransform::Identity];
    if include_constant {
        candidates.push(ScoreTransform::Constant { value: pass_rate });
    }
    for alpha in [0.25, 0.5, 0.75] {
        candidates.push(ScoreTransform::Shrink { alpha });
    }
    for threshold in [0.01, 0.05, 0.10, 0.25, 0.50, 0.75, 0.90, 0.95, 0.99] {
        candidates.push(ScoreTransform::Threshold {
            threshold,
            low: 0.05,
            high: 0.95,
        });
    }
    candidates
}

fn calibration_objective(metrics: &MetricBlock, expected_degenerate: bool) -> f32 {
    let correlation_reward = match metrics.prediction_oracle_correlation {
        Some(value) => -0.10 * value,
        None if !expected_degenerate => 10.0,
        None => 0.0,
    };
    metrics.brier + 0.25 * metrics.ece + correlation_reward
}

fn apply_calibration_policy(
    rows: &[PredictionRow],
    policy: &OracleCalibrationPolicy,
) -> Result<Vec<PredictionRow>> {
    rows.iter()
        .map(|row| {
            let category_policy = policy
                .by_category
                .get(&row.category)
                .ok_or_else(|| anyhow!("missing calibration policy for {}", row.category))?;
            Ok(transform_row(row, &category_policy.transform))
        })
        .collect()
}

fn transform_rows(rows: &[PredictionRow], transform: &ScoreTransform) -> Vec<PredictionRow> {
    rows.iter()
        .map(|row| transform_row(row, transform))
        .collect()
}

fn transform_row(row: &PredictionRow, transform: &ScoreTransform) -> PredictionRow {
    let score = match transform {
        ScoreTransform::Identity => row.predicted_oracle_pass,
        ScoreTransform::Constant { value } => *value,
        ScoreTransform::Shrink { alpha } => 0.5 + (*alpha * (row.predicted_oracle_pass - 0.5)),
        ScoreTransform::Threshold {
            threshold,
            low,
            high,
        } => {
            if row.predicted_oracle_pass >= *threshold {
                *high
            } else {
                *low
            }
        }
    }
    .clamp(0.0, 1.0);
    let mut out = row.clone();
    out.predicted_oracle_pass = score;
    out.predicted_pass = score >= 0.5;
    out.correct = out.predicted_pass == out.actual_oracle_pass;
    out
}

fn metrics_for(rows: &[PredictionRow]) -> MetricBlock {
    if rows.is_empty() {
        return MetricBlock {
            sample_count: 0,
            accuracy: 0.0,
            prediction_oracle_correlation: None,
            oracle_variance_class: OracleVarianceClass::Empty,
            gate_contract: empty_gate_contract(),
            statistical_power: empty_power_report(),
            brier: 0.0,
            ece: 0.0,
            predicted_pass_count: 0,
            predicted_fail_count: 0,
            actual_pass_count: 0,
            actual_fail_count: 0,
            pass_recall: None,
            fail_recall: None,
            actual_class_confidence_mean: 0.0,
            score_min: 0.0,
            score_max: 0.0,
            score_mean: 0.0,
        };
    }
    let mut predicted = Vec::with_capacity(rows.len());
    let mut actual = Vec::with_capacity(rows.len());
    let mut correct = 0usize;
    let mut brier = 0.0f32;
    let mut predicted_pass_count = 0usize;
    let mut actual_pass_count = 0usize;
    let mut true_pass = 0usize;
    let mut true_fail = 0usize;
    let mut actual_class_confidence = 0.0f32;
    for row in rows {
        let target = if row.actual_oracle_pass { 1.0 } else { 0.0 };
        predicted.push(row.predicted_oracle_pass);
        actual.push(target);
        if row.correct {
            correct += 1;
        }
        if row.predicted_pass {
            predicted_pass_count += 1;
        }
        if row.actual_oracle_pass {
            actual_pass_count += 1;
            if row.predicted_pass {
                true_pass += 1;
            }
            actual_class_confidence += row.predicted_oracle_pass;
        } else {
            if !row.predicted_pass {
                true_fail += 1;
            }
            actual_class_confidence += 1.0 - row.predicted_oracle_pass;
        }
        brier += (row.predicted_oracle_pass - target).powi(2);
    }
    let actual_fail_count = rows.len() - actual_pass_count;
    let predicted_fail_count = rows.len() - predicted_pass_count;
    let pass_recall = ratio_opt(true_pass, actual_pass_count);
    let fail_recall = ratio_opt(true_fail, actual_fail_count);
    let score_min = predicted.iter().copied().fold(f32::INFINITY, f32::min);
    let score_max = predicted.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let score_mean = predicted.iter().sum::<f32>() / predicted.len() as f32;
    let correlation = pearson(&predicted, &actual);
    let oracle_variance_class = oracle_variance_class(actual_pass_count, actual_fail_count);
    let brier = brier / rows.len() as f32;
    let ece = expected_calibration_error(rows);
    let actual_class_confidence_mean = actual_class_confidence / rows.len() as f32;
    MetricBlock {
        sample_count: rows.len(),
        accuracy: correct as f32 / rows.len() as f32,
        prediction_oracle_correlation: correlation,
        oracle_variance_class,
        gate_contract: gate_contract_for(
            oracle_variance_class,
            correlation,
            pass_recall,
            fail_recall,
            actual_class_confidence_mean,
            brier,
            ece,
        ),
        statistical_power: power_report_for(
            rows.len(),
            actual_pass_count,
            actual_fail_count,
            correlation,
            pass_recall,
            fail_recall,
        ),
        brier,
        ece,
        predicted_pass_count,
        predicted_fail_count,
        actual_pass_count,
        actual_fail_count,
        pass_recall,
        fail_recall,
        actual_class_confidence_mean,
        score_min,
        score_max,
        score_mean,
    }
}

fn per_category_metrics(rows: &[PredictionRow]) -> BTreeMap<String, MetricBlock> {
    REQUIRED_CATEGORIES
        .iter()
        .map(|category| {
            let subset = rows
                .iter()
                .filter(|row| row.category == *category)
                .cloned()
                .collect::<Vec<_>>();
            ((*category).to_string(), metrics_for(&subset))
        })
        .collect()
}

fn ship_gate_blockers(
    metrics: &MetricBlock,
    per_category: &BTreeMap<String, MetricBlock>,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if !metrics.gate_contract.passed {
        blockers.extend(
            metrics
                .gate_contract
                .blockers
                .iter()
                .map(|blocker| format!("overall {blocker}")),
        );
    }
    for (category, block) in per_category {
        if block.gate_contract.passed {
            continue;
        }
        blockers.extend(
            block
                .gate_contract
                .blockers
                .iter()
                .map(|blocker| format!("{category}::python {blocker}")),
        );
    }
    blockers
}

fn statistical_power_blockers(
    metrics: &MetricBlock,
    per_category: &BTreeMap<String, MetricBlock>,
) -> Vec<String> {
    let mut blockers = metrics
        .statistical_power
        .blockers
        .iter()
        .map(|blocker| format!("overall {blocker}"))
        .collect::<Vec<_>>();
    for (category, block) in per_category {
        blockers.extend(
            block
                .statistical_power
                .blockers
                .iter()
                .map(|blocker| format!("{category}::python {blocker}")),
        );
    }
    blockers
}

fn gate_contract_for(
    variance_class: OracleVarianceClass,
    correlation: Option<f32>,
    pass_recall: Option<f32>,
    fail_recall: Option<f32>,
    actual_class_confidence_mean: f32,
    brier: f32,
    ece: f32,
) -> GateContract {
    match variance_class {
        OracleVarianceClass::Mixed => {
            let mut blockers = Vec::new();
            match correlation {
                Some(value) if value >= SHIP_GATE_TARGET => {}
                Some(value) => blockers.push(format!(
                    "prediction_oracle_correlation {value:.6} < {SHIP_GATE_TARGET:.6}"
                )),
                None => blockers.push(
                    "prediction_oracle_correlation undefined despite mixed oracle classes"
                        .to_string(),
                ),
            }
            GateContract {
                metric_name: "pearson_prediction_oracle_correlation".to_string(),
                metric_value: correlation,
                target: SHIP_GATE_TARGET,
                passed: blockers.is_empty(),
                blockers,
                rationale:
                    "Mixed oracle cells have target variance; Pearson correlation remains the ship-gate metric."
                        .to_string(),
            }
        }
        OracleVarianceClass::AllFail => degenerate_gate_contract(
            "all_fail_recall_confidence_brier_ece",
            fail_recall,
            actual_class_confidence_mean,
            brier,
            ece,
            "All-fail oracle cells have zero target variance; Pearson is undefined.",
        ),
        OracleVarianceClass::AllPass => degenerate_gate_contract(
            "all_pass_recall_confidence_brier_ece",
            pass_recall,
            actual_class_confidence_mean,
            brier,
            ece,
            "All-pass oracle cells have zero target variance; Pearson is undefined.",
        ),
        OracleVarianceClass::Empty => empty_gate_contract(),
    }
}

fn degenerate_gate_contract(
    metric_name: &str,
    recall: Option<f32>,
    actual_class_confidence_mean: f32,
    brier: f32,
    ece: f32,
    rationale: &str,
) -> GateContract {
    let mut blockers = Vec::new();
    match recall {
        Some(value) if value >= DEGENERATE_RECALL_TARGET => {}
        Some(value) => blockers.push(format!(
            "{metric_name}.recall {value:.6} < {DEGENERATE_RECALL_TARGET:.6}"
        )),
        None => blockers.push(format!("{metric_name}.recall unavailable")),
    }
    if actual_class_confidence_mean < DEGENERATE_CLASS_CONFIDENCE_TARGET {
        blockers.push(format!(
            "{metric_name}.actual_class_confidence_mean {actual_class_confidence_mean:.6} < {DEGENERATE_CLASS_CONFIDENCE_TARGET:.6}"
        ));
    }
    if brier > DEGENERATE_BRIER_MAX {
        blockers.push(format!(
            "{metric_name}.brier {brier:.6} > {DEGENERATE_BRIER_MAX:.6}"
        ));
    }
    if ece > DEGENERATE_ECE_MAX {
        blockers.push(format!(
            "{metric_name}.ece {ece:.6} > {DEGENERATE_ECE_MAX:.6}"
        ));
    }
    GateContract {
        metric_name: metric_name.to_string(),
        metric_value: recall,
        target: DEGENERATE_RECALL_TARGET,
        passed: blockers.is_empty(),
        blockers,
        rationale: rationale.to_string(),
    }
}

fn empty_gate_contract() -> GateContract {
    GateContract {
        metric_name: "empty_cell_unavailable".to_string(),
        metric_value: None,
        target: SHIP_GATE_TARGET,
        passed: false,
        blockers: vec!["empty cell has no gate metric".to_string()],
        rationale: "No samples means no gate metric is mathematically defined.".to_string(),
    }
}

fn power_report_for(
    sample_count: usize,
    actual_pass_count: usize,
    actual_fail_count: usize,
    correlation: Option<f32>,
    pass_recall: Option<f32>,
    fail_recall: Option<f32>,
) -> StatisticalPowerReport {
    let pearson_defined = correlation.is_some();
    let fisher_z_95_ci = correlation.and_then(|value| fisher_z_ci(value, sample_count));
    let actual_class_recall = if actual_pass_count == 0 {
        fail_recall
    } else if actual_fail_count == 0 {
        pass_recall
    } else {
        None
    };
    let actual_class_recall_wilson_lower_95 = actual_class_recall.map(|value| {
        let successes = (value * sample_count as f32).round() as usize;
        wilson_lower_bound(successes, sample_count)
    });
    let mut blockers = Vec::new();
    if let Some(interval) = &fisher_z_95_ci {
        if interval.lower < SHIP_GATE_TARGET {
            blockers.push(format!(
                "statistical_power fisher_z_lower_95 {:.6} < {:.6}",
                interval.lower, SHIP_GATE_TARGET
            ));
        }
    } else if pearson_defined && sample_count <= 3 {
        blockers.push(format!(
            "statistical_power sample_count {sample_count} insufficient for Fisher-z CI"
        ));
    }
    if actual_pass_count == 0 || actual_fail_count == 0 {
        match actual_class_recall_wilson_lower_95 {
            Some(lower) if lower >= DEGENERATE_RECALL_TARGET => {}
            Some(lower) => blockers.push(format!(
                "statistical_power degenerate_recall_wilson_lower_95 {lower:.6} < {DEGENERATE_RECALL_TARGET:.6}"
            )),
            None => blockers.push("statistical_power degenerate recall CI unavailable".to_string()),
        }
    }
    StatisticalPowerReport {
        sample_count,
        actual_pass_count,
        actual_fail_count,
        pearson_defined,
        fisher_z_95_ci,
        actual_class_recall_wilson_lower_95,
        standalone_readiness_claim_powered: blockers.is_empty(),
        blockers,
    }
}

fn empty_power_report() -> StatisticalPowerReport {
    StatisticalPowerReport {
        sample_count: 0,
        actual_pass_count: 0,
        actual_fail_count: 0,
        pearson_defined: false,
        fisher_z_95_ci: None,
        actual_class_recall_wilson_lower_95: None,
        standalone_readiness_claim_powered: false,
        blockers: vec!["statistical_power empty cell".to_string()],
    }
}

fn oracle_variance_class(
    actual_pass_count: usize,
    actual_fail_count: usize,
) -> OracleVarianceClass {
    match (actual_pass_count, actual_fail_count) {
        (0, 0) => OracleVarianceClass::Empty,
        (0, _) => OracleVarianceClass::AllFail,
        (_, 0) => OracleVarianceClass::AllPass,
        _ => OracleVarianceClass::Mixed,
    }
}

fn ratio_opt(numerator: usize, denominator: usize) -> Option<f32> {
    if denominator == 0 {
        None
    } else {
        Some(numerator as f32 / denominator as f32)
    }
}

fn expected_calibration_error(rows: &[PredictionRow]) -> f32 {
    if rows.is_empty() {
        return 0.0;
    }
    let mut bins = vec![(0usize, 0.0f32, 0.0f32); ECE_BIN_COUNT];
    for row in rows {
        let index = ((row.predicted_oracle_pass * ECE_BIN_COUNT as f32).floor() as usize)
            .min(ECE_BIN_COUNT - 1);
        let target = if row.actual_oracle_pass { 1.0 } else { 0.0 };
        bins[index].0 += 1;
        bins[index].1 += row.predicted_oracle_pass;
        bins[index].2 += target;
    }
    bins.into_iter()
        .filter(|(count, _, _)| *count > 0)
        .map(|(count, confidence_sum, target_sum)| {
            let weight = count as f32 / rows.len() as f32;
            let avg_confidence = confidence_sum / count as f32;
            let avg_target = target_sum / count as f32;
            weight * (avg_confidence - avg_target).abs()
        })
        .sum()
}

fn fisher_z_ci(correlation: f32, sample_count: usize) -> Option<CorrelationConfidenceInterval> {
    if sample_count <= 3 || !correlation.is_finite() {
        return None;
    }
    let bounded = correlation.clamp(-0.999_999, 0.999_999);
    let z = bounded.atanh();
    let se = 1.0 / ((sample_count as f32 - 3.0).sqrt());
    Some(CorrelationConfidenceInterval {
        lower: (z - FISHER_Z_95 * se).tanh().clamp(-1.0, 1.0),
        upper: (z + FISHER_Z_95 * se).tanh().clamp(-1.0, 1.0),
    })
}

fn wilson_lower_bound(successes: usize, total: usize) -> f32 {
    if total == 0 {
        return 0.0;
    }
    let n = total as f32;
    let z2 = FISHER_Z_95 * FISHER_Z_95;
    let phat = successes as f32 / n;
    let denominator = 1.0 + z2 / n;
    let centre = phat + z2 / (2.0 * n);
    let margin = FISHER_Z_95 * ((phat * (1.0 - phat) + z2 / (4.0 * n)) / n).sqrt();
    ((centre - margin) / denominator).clamp(0.0, 1.0)
}

fn pearson(xs: &[f32], ys: &[f32]) -> Option<f32> {
    if xs.len() != ys.len() || xs.len() < 2 {
        return None;
    }
    let mean_x = xs.iter().sum::<f32>() / xs.len() as f32;
    let mean_y = ys.iter().sum::<f32>() / ys.len() as f32;
    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;
    for (x, y) in xs.iter().zip(ys) {
        let dx = *x - mean_x;
        let dy = *y - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }
    if var_x <= f32::EPSILON || var_y <= f32::EPSILON {
        None
    } else {
        Some((cov / (var_x.sqrt() * var_y.sqrt())).clamp(-1.0, 1.0))
    }
}

fn category_counts(entries: &[CorpusEntry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.category.clone()).or_default() += 1;
    }
    counts
}

fn ensure_category_grid(entries: &[CorpusEntry]) -> Result<()> {
    let counts = category_counts(entries);
    for category in REQUIRED_CATEGORIES {
        ensure!(
            counts.get(category).copied() == Some(30),
            "split must contain 30 rows for category {category}; got {:?}",
            counts.get(category)
        );
    }
    Ok(())
}

fn typed_finding(baseline_rank_lift_correlation: f32, variants: &[VariantEvaluation]) -> Value {
    let best = variants
        .iter()
        .max_by(|left, right| {
            let l = left
                .rank_safe_metrics
                .prediction_oracle_correlation
                .unwrap_or(f32::NEG_INFINITY);
            let r = right
                .rank_safe_metrics
                .prediction_oracle_correlation
                .unwrap_or(f32::NEG_INFINITY);
            l.total_cmp(&r)
        })
        .expect("variants non-empty");
    let delta = scalar_delta(
        baseline_rank_lift_correlation,
        best.rank_safe_metrics.prediction_oracle_correlation,
    );
    let classification = match delta.as_f64() {
        Some(value) if value > 0.005 => "positive_candidate_needs_gate_followup",
        Some(value) if value < -0.005 => "regression",
        Some(_) => "neutral_plateau",
        None => "undefined_correlation",
    };
    json!({
        "classification": classification,
        "best_lambda_inv": best.lambda_inv,
        "best_rank_safe_prediction_oracle_correlation": best.rank_safe_metrics.prediction_oracle_correlation,
        "rank_safe_delta_vs_535_rank_lift_baseline_of_record": delta,
        "baseline_rank_lift_correlation_of_record": baseline_rank_lift_correlation,
        "reason": "classification compares real predictor-output variants against the #535 rank-lift baseline of record; rank-lift itself is not re-applied to variants because it would replace predictor scores and mask lambda_inv changes"
    })
}

fn correlation_delta(before: Option<f32>, after: Option<f32>) -> Value {
    match (before, after) {
        (Some(before), Some(after)) => json!(after - before),
        _ => Value::Null,
    }
}

fn scalar_delta(before: f32, after: Option<f32>) -> Value {
    match after {
        Some(after) => json!(after - before),
        None => Value::Null,
    }
}

fn source_audit() -> Result<Value> {
    let source = fs::read_to_string(file!()).unwrap_or_default();
    let legacy_echo = ["struct", "Echo", "Predictor"].join("");
    let legacy_oracle = ["Category", "Majority", "Oracle"].join("");
    let deterministic_vector = ["deterministic", "_vector("].join("");
    Ok(json!({
        "source_file": file!(),
        "legacy_echo_predictor_present": source.contains(&legacy_echo),
        "legacy_category_majority_oracle_present": source.contains(&legacy_oracle),
        "deterministic_vector_symbol_present": source.contains(&deterministic_vector),
        "materialize_inference_panels_present": source.contains("materialize_inference_panels"),
        "export_trained_predictor_checkpoint_present": source.contains("export_trained_predictor_checkpoint"),
    }))
}

fn active_input_paths() -> Vec<&'static str> {
    vec![
        CORPUS_DIR,
        INDEX_PATH,
        BASELINE_CHECKPOINT_MANIFEST_PATH,
        BASELINE_SHIP_GATE_REPORT_PATH,
        FSV_ROOT,
        MODEL_ROOT,
    ]
}

fn lambda_slug(lambda_inv: f32) -> String {
    format!("lambda-{lambda_inv:.2}").replace('.', "-")
}

fn parse_sha256_prefixed(value: &str) -> Result<[u8; 32]> {
    let raw = value
        .strip_prefix("sha256:")
        .context("expected sha256:<hex> value")?;
    let bytes = hex::decode(raw)?;
    ensure!(bytes.len() == 32, "sha256 must decode to 32 bytes");
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn sha256_file(path: impl AsRef<Path>) -> Result<String> {
    Ok(sha256_hex(&fs::read(path.as_ref())?))
}

fn sha256_json<T: Serialize>(value: &T) -> Result<String> {
    Ok(sha256_hex(&serde_json::to_vec(value)?))
}

fn now_unix_ms() -> Result<u128> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis())
}

fn read_json(path: impl AsRef<Path>) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path.as_ref())?)?)
}

fn write_json_0600(path: impl AsRef<Path>, value: &Value) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    ensure!(
        fs::metadata(path)?.permissions().mode() & 0o777 == 0o600,
        "{} was not written 0600",
        path.display()
    );
    ensure!(
        fs::read(path)? == bytes,
        "{} readback mismatch",
        path.display()
    );
    Ok(())
}

#[allow(dead_code)]
fn _compile_time_no_legacy_holdout_traits(
    _: Option<&dyn std::any::Any>,
) -> Result<(String, String)> {
    Ok((
        predictor_config_sha256(&PredictorConfig::default())?,
        "real_input_ablation".to_string(),
    ))
}
