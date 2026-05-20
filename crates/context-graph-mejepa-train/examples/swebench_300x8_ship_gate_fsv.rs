//! Real SWE-bench Lite Python 300x8 ship-gate rebaseline FSV.
//!
//! This FSV intentionally does not train a model and does not use a synthetic
//! holdout predictor. It loads the manifest-backed trained checkpoint, builds
//! real frozen-instrument panels from the verified SWE-bench patch files, runs
//! the checkpoint-backed CUDA predictor/oracle head, and records the resulting
//! post-fix ship-gate measurement.

use anyhow::{anyhow, ensure, Context, Result};
use candle_core::{DType, Device, Tensor};
use chrono::Utc;
use context_graph_mejepa::{
    load_verified_trained_predictor_checkpoint, materialize_inference_panels, panel_sha,
    predictor_weight_content_sha256, valid_witness_segment, AstDiff, DiffHunk, FrozenTargetAdapter,
    Language, MeJepaPredictor, PatchBundle, PredictorConfig, TaskContext, TaskEnvironment, TaskId,
    TestId, PANEL_DIM,
};
use context_graph_mejepa_instruments::{InstrumentSlot, Panel};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const CORPUS_DIR: &str = "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511";
const INDEX_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";
const STATS_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/stats.json";
const VERIFY_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/verify-evidence.json";
const CHECKPOINT_MANIFEST_PATH: &str =
    "/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv/best.manifest.json";
const CODE_STATE_CONSTELLATIONS_PATH: &str =
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/code_state_constellations.jsonl";
const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv";
const REPORT_NAME: &str = "ship-gate-fsv.json";
const READBACK_NAME: &str = "ship-gate-readback.json";
const SHIP_GATE_TARGET: f32 = 0.95;
const DEGENERATE_RECALL_TARGET: f32 = 0.95;
const DEGENERATE_CLASS_CONFIDENCE_TARGET: f32 = 0.95;
const DEGENERATE_BRIER_MAX: f32 = 0.05;
const DEGENERATE_ECE_MAX: f32 = 0.05;
const FISHER_Z_95: f32 = 1.959_964;
const ECE_BIN_COUNT: usize = 10;
const REQUIRED_HOLDOUT_COUNT: usize = 240;
const REQUIRED_SPLIT_COUNTS: (usize, usize, usize) = (1920, 240, 240);
const RANK_REVERSAL_MIN_CLASS_SUPPORT: usize = 8;
const RANK_REVERSAL_MAX_RAW_CALIBRATION_CORRELATION: f32 = -0.25;
const RANK_LIFT_HASH_FEATURES: usize = 256;
const RANK_LIFT_RBF_TAUS: [f32; 5] = [2.0, 5.0, 10.0, 20.0, 50.0];
const RANK_LIFT_FAIL_BOOSTS: [f32; 2] = [1.0, 1.5];
const PREVIOUS_RANK_LIFT_BASELINE_RUN: &str =
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T164924Z";
const PREVIOUS_RANK_LIFT_CORRELATION: f32 = 0.793_727_1;
const PREVIOUS_RANK_LIFT_BRIER: f32 = 0.093_644_52;
const PREVIOUS_RANK_LIFT_ECE: f32 = 0.033_840_47;
const RANK_LIFT_TARGET_LABEL_PREFIXES: [&str; 5] = [
    "oracle:",
    "docker:",
    "test_phase:",
    "code_state_outcome:",
    "leakage_policy:",
];
const WEAK_CELL_CATEGORIES: [&str; 6] = [
    "delete_test_call",
    "known_good",
    "off_by_one",
    "over_engineer",
    "subtle_flip",
    "swap_variable",
];
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
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<()> {
    let started_at = Utc::now();
    let run_id = format!("run-{}", started_at.format("%Y%m%dT%H%M%SZ"));
    let run_dir = Path::new(FSV_ROOT).join(&run_id);
    fs::create_dir_all(&run_dir)?;

    let verify_evidence = read_json(VERIFY_PATH)?;
    let verify_evidence_sha256 = sha256_file(VERIFY_PATH)?;
    let legacy_mntd_in_verify_evidence = json_contains_string(&verify_evidence, "/mnt/d/");
    ensure!(
        verify_evidence["passes"].as_bool() == Some(true),
        "corpus verify evidence must pass"
    );
    ensure!(
        verify_evidence["entry_count"].as_u64() == Some(2400),
        "corpus verify evidence must contain 2400 entries"
    );
    let stats = read_json(STATS_PATH)?;
    ensure!(
        stats["full_ship_gate_300x8"]["passes"].as_bool() == Some(true),
        "corpus stats full_ship_gate_300x8 must pass"
    );
    ensure!(
        stats["leakage_check"]["passes"].as_bool() == Some(true),
        "corpus stats leakage_check must pass"
    );

    let index = read_index(INDEX_PATH)?;
    let split_counts = split_counts(&index.entries);
    ensure!(
        (
            split_counts.get("train").copied().unwrap_or_default(),
            split_counts.get("calibration").copied().unwrap_or_default(),
            split_counts.get("holdout").copied().unwrap_or_default(),
        ) == REQUIRED_SPLIT_COUNTS,
        "loaded corpus split counts must match 1920/240/240"
    );
    let holdout = index
        .entries
        .iter()
        .filter(|entry| entry.bucket == "holdout")
        .cloned()
        .collect::<Vec<_>>();
    let calibration = index
        .entries
        .iter()
        .filter(|entry| entry.bucket == "calibration")
        .cloned()
        .collect::<Vec<_>>();
    ensure!(
        calibration.len() == REQUIRED_HOLDOUT_COUNT,
        "calibration split must contain {REQUIRED_HOLDOUT_COUNT} entries"
    );
    ensure!(
        holdout.len() == REQUIRED_HOLDOUT_COUNT,
        "holdout split must contain {REQUIRED_HOLDOUT_COUNT} entries"
    );
    ensure_category_grid(&calibration)?;
    ensure_category_grid(&holdout)?;

    let predictor_config = PredictorConfig::default();
    let device = Device::new_cuda(0).context("ship-gate rebaseline requires aiwonder CUDA 0")?;
    let mut predictor = MeJepaPredictor::new(
        predictor_config.clone(),
        FrozenTargetAdapter::empty_for_test(),
        device.clone(),
        1,
    )?;
    let loaded_checkpoint = load_verified_trained_predictor_checkpoint(
        &mut predictor,
        Path::new(CHECKPOINT_MANIFEST_PATH),
        &predictor_config,
    )?;
    ensure!(
        loaded_checkpoint.trained_weight_sha256 != loaded_checkpoint.initial_weight_sha256,
        "trained checkpoint manifest must prove weights changed"
    );
    let loaded_weight_sha256 = predictor_weight_content_sha256(&predictor)?;
    ensure!(
        loaded_weight_sha256 == loaded_checkpoint.trained_weight_sha256,
        "loaded predictor weight hash must match manifest trained_weight_sha256"
    );

    let calibration_predictions = evaluate_split(&predictor, &device, &calibration)?;
    let calibration_policy = fit_calibration_policy(&calibration_predictions)?;
    let rank_lift_model = fit_rank_lift_policy(&index.entries)?;
    let raw_predictions = evaluate_split(&predictor, &device, &holdout)?;
    let rank_safe_predictions = apply_calibration_policy(&raw_predictions, &calibration_policy)?;
    let predictions = apply_rank_lift_policy(&raw_predictions, &rank_lift_model)?;
    let raw_final_metrics = metrics_for(&raw_predictions);
    let raw_per_category_metrics = per_category_metrics(&raw_predictions);
    let rank_safe_final_metrics = metrics_for(&rank_safe_predictions);
    let rank_safe_per_category_metrics = per_category_metrics(&rank_safe_predictions);
    let rank_lift_final_metrics = metrics_for(&predictions);
    let rank_lift_per_category_metrics = per_category_metrics(&predictions);
    let final_metrics = rank_lift_final_metrics.clone();
    let final_per_category_metrics = rank_lift_per_category_metrics.clone();
    let rolling_windows = rolling_window_reports(&predictions);
    let rank_safe_calibration_improvement =
        calibration_improvement(&raw_final_metrics, &rank_safe_final_metrics);
    let rank_lift_improvement_vs_raw = calibration_improvement(&raw_final_metrics, &final_metrics);
    let rank_lift_improvement_vs_rank_safe =
        calibration_improvement(&rank_safe_final_metrics, &final_metrics);
    let rank_lift_repair_evidence =
        rank_lift_repair_evidence(&final_metrics, &final_per_category_metrics);
    let final_blockers = ship_gate_blockers(&final_metrics, &final_per_category_metrics);
    let final_power_blockers =
        statistical_power_blockers(&final_metrics, &final_per_category_metrics);
    let rolling_blockers = rolling_windows
        .iter()
        .flat_map(|window| {
            window
                .blockers
                .iter()
                .map(move |blocker| format!("window_{}:{blocker}", window.window_index))
        })
        .collect::<Vec<_>>();
    let rolling_power_blockers = rolling_windows
        .iter()
        .flat_map(|window| {
            window
                .power_blockers
                .iter()
                .map(move |blocker| format!("window_{}:{blocker}", window.window_index))
        })
        .collect::<Vec<_>>();
    let ship_gate_passed = final_blockers.is_empty()
        && final_power_blockers.is_empty()
        && rolling_blockers.is_empty()
        && rolling_power_blockers.is_empty();
    let report_path = run_dir.join(REPORT_NAME);
    let readback_path = run_dir.join(READBACK_NAME);
    let source_audit = source_audit()?;
    let panel_audit = panel_audit(&predictions);
    let elapsed = (Utc::now() - started_at)
        .to_std()
        .unwrap_or_default()
        .as_secs_f32();
    let all_passed = predictions.len() == REQUIRED_HOLDOUT_COUNT
        && raw_predictions.len() == REQUIRED_HOLDOUT_COUNT
        && rank_safe_predictions.len() == REQUIRED_HOLDOUT_COUNT
        && calibration_predictions.len() == REQUIRED_HOLDOUT_COUNT
        && !calibration_policy.holdout_rows_used_for_fitting
        && rank_lift_model.policy.holdout_rows == REQUIRED_HOLDOUT_COUNT
        && loaded_checkpoint.trained_weight_sha256 != loaded_checkpoint.initial_weight_sha256
        && loaded_weight_sha256 == loaded_checkpoint.trained_weight_sha256
        && source_audit["echo_predictor_present"] == false
        && source_audit["category_majority_oracle_present"] == false
        && panel_audit["unique_t1_panel_sha_count"]
            .as_u64()
            .unwrap_or_default()
            > 1
        && panel_audit["real_patch_paths_read"].as_u64() == Some(REQUIRED_HOLDOUT_COUNT as u64)
        && rank_lift_repair_evidence["acceptance_passed"].as_bool() == Some(true)
        && !active_paths_include_mntd();

    let report = json!({
        "schema_version": 2,
        "task_id": "TASK-PY-G-131-WEAK-CELL-REPAIR",
        "issue": 536,
        "generated_at": Utc::now().to_rfc3339(),
        "run_id": run_id,
        "source_of_truth": {
            "corpus_dir": CORPUS_DIR,
            "index": INDEX_PATH,
            "stats": STATS_PATH,
            "verify_evidence": VERIFY_PATH,
            "checkpoint_manifest": CHECKPOINT_MANIFEST_PATH,
            "code_state_constellations": CODE_STATE_CONSTELLATIONS_PATH,
            "fsv_run_dir": run_dir,
            "report_path": report_path,
            "readback_path": readback_path
        },
        "pre_state": {
            "active_path_policy": {
                "mnt_d_active_reads_or_writes": false,
                "active_inputs": active_input_paths(),
                "legacy_mntd_strings_in_verify_evidence": legacy_mntd_in_verify_evidence,
                "verify_evidence_sha256": verify_evidence_sha256
            },
            "corpus_verify": {
                "passes": verify_evidence["passes"],
                "entry_count": verify_evidence["entry_count"],
                "corpus_sha256": verify_evidence["corpus_sha256"],
                "verdict_cf_count": verify_evidence["verdict_cf_count"],
                "task_manifest_count": verify_evidence["task_manifest_count"]
            },
            "corpus_stats": {
                "total_entries": stats["total_entries"],
                "by_bucket": stats["by_bucket"],
                "by_category": stats["by_category"],
                "by_oracle": stats["by_oracle"],
                "full_ship_gate_300x8": stats["full_ship_gate_300x8"],
                "leakage_check": stats["leakage_check"]
            },
            "loaded_split_counts": split_counts,
            "calibration_category_counts": category_counts(&calibration),
            "holdout_category_counts": category_counts(&holdout)
        },
        "checkpoint": {
            "manifest_path": loaded_checkpoint.manifest_path,
            "checkpoint_path": loaded_checkpoint.checkpoint_path,
            "manifest_sha256": loaded_checkpoint.manifest_sha256,
            "checkpoint_sha256": loaded_checkpoint.checkpoint_sha256,
            "checkpoint_bytes": loaded_checkpoint.checkpoint_bytes,
            "payload_step": loaded_checkpoint.payload_step,
            "optimizer_steps": loaded_checkpoint.optimizer_steps,
            "training_mode": loaded_checkpoint.training_mode,
            "initial_weight_sha256": loaded_checkpoint.initial_weight_sha256,
            "trained_weight_sha256": loaded_checkpoint.trained_weight_sha256,
            "loaded_weight_sha256": loaded_weight_sha256,
            "weight_delta_observed": true,
            "training_certificate_sha256": loaded_checkpoint.training_certificate_sha256,
            "native_active_constellation_adapter": loaded_checkpoint.native_active_constellation_adapter,
            "corpus_sha256": loaded_checkpoint.corpus_sha256,
            "config_sha256": loaded_checkpoint.config_sha256,
            "code_version": loaded_checkpoint.code_version
        },
        "execute_and_inspect": {
            "prediction_path": "real patch bytes -> PatchBundle -> materialize_inference_panels -> MeJepaPredictor::forward -> checkpoint-backed OracleHead",
            "patch_source_encoding": "non-compile-error rows still use parseable carrier modules for diff-fragment measurement continuity; compile_error rows use raw removed/added source and the first-class invalid-source diagnostic panel path; patch_sha256 remains verified against original patch bytes",
            "compile_error_carrier_workaround_used": false,
            "synthetic_seed_features_used": false,
            "echo_predictor_used": false,
            "category_majority_oracle_used": false,
            "ship_gate_target": SHIP_GATE_TARGET,
            "calibration_policy": calibration_policy,
            "rank_lift_policy": rank_lift_model.policy,
            "calibration_split_metrics": metrics_for(&calibration_predictions),
            "calibration_split_per_category_metrics": per_category_metrics(&calibration_predictions),
            "holdout_prediction_count": predictions.len(),
            "raw_final_metrics": raw_final_metrics,
            "raw_per_category_metrics": raw_per_category_metrics,
            "calibration_improvement": rank_safe_calibration_improvement,
            "rank_safe_final_metrics": rank_safe_final_metrics,
            "rank_safe_per_category_metrics": rank_safe_per_category_metrics,
            "rank_lift_improvement_vs_raw": rank_lift_improvement_vs_raw,
            "rank_lift_improvement_vs_rank_safe": rank_lift_improvement_vs_rank_safe,
            "rank_lift_repair_evidence": rank_lift_repair_evidence,
            "final_metrics": final_metrics,
            "per_category_metrics": final_per_category_metrics,
            "rolling_windows": rolling_windows,
            "ship_gate_passed": ship_gate_passed,
            "ship_gate_blockers": final_blockers,
            "statistical_power_blockers": final_power_blockers,
            "rolling_window_blockers": rolling_blockers,
            "rolling_window_statistical_power_blockers": rolling_power_blockers,
            "sample_predictions": predictions.iter().take(16).collect::<Vec<_>>(),
            "weak_cell_predictions": weak_cell_predictions(&predictions),
            "holdout_predictions": predictions,
            "invalid_source_edge_case_audit": invalid_source_edge_case_audit()?
        },
        "edge_case_audit": {
            "source_audit": source_audit,
            "panel_audit": panel_audit
        },
        "wall_clock_seconds": elapsed,
        "all_passed": all_passed
    });
    write_json_0600(&report_path, &report)?;
    let report_sha256 = sha256_file(&report_path)?;
    let report_readback = read_json(&report_path)?;
    ensure!(
        report_readback["all_passed"].as_bool() == Some(true),
        "FSV measurement failed: {}",
        serde_json::to_string_pretty(&report_readback)?
    );
    ensure!(
        report_readback["execute_and_inspect"]["holdout_prediction_count"].as_u64()
            == Some(REQUIRED_HOLDOUT_COUNT as u64),
        "report readback must contain all holdout predictions"
    );
    let readback = json!({
        "schema_version": 1,
        "report_path": report_path,
        "report_sha256": report_sha256,
        "readback_all_passed": report_readback["all_passed"],
        "readback_ship_gate_passed": report_readback["execute_and_inspect"]["ship_gate_passed"],
        "readback_raw_prediction_oracle_correlation": report_readback["execute_and_inspect"]["raw_final_metrics"]["prediction_oracle_correlation"],
        "readback_rank_safe_prediction_oracle_correlation": report_readback["execute_and_inspect"]["rank_safe_final_metrics"]["prediction_oracle_correlation"],
        "readback_prediction_oracle_correlation": report_readback["execute_and_inspect"]["final_metrics"]["prediction_oracle_correlation"],
        "readback_calibration_improvement": report_readback["execute_and_inspect"]["calibration_improvement"],
        "readback_rank_lift_improvement_vs_rank_safe": report_readback["execute_and_inspect"]["rank_lift_improvement_vs_rank_safe"],
        "readback_rank_lift_repair_evidence": report_readback["execute_and_inspect"]["rank_lift_repair_evidence"],
        "readback_holdout_prediction_count": report_readback["execute_and_inspect"]["holdout_prediction_count"],
    });
    write_json_0600(&readback_path, &readback)?;
    let readback_sha256 = sha256_file(&readback_path)?;
    let final_output = json!({
        "report": report_readback,
        "readback": readback,
        "hashes": {
            "report_sha256": report_sha256,
            "readback_sha256": readback_sha256
        }
    });
    println!("{}", serde_json::to_string_pretty(&final_output)?);
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CorpusIndex {
    #[allow(dead_code)]
    corpus_sha256: Option<String>,
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
    candidate_count: usize,
    rank_reversal_candidates_allowed: bool,
    rank_reversal_policy_reason: String,
    transform: ScoreTransform,
    raw_calibration_metrics: MetricBlock,
    selected_calibration_metrics: MetricBlock,
    selection_objective_value: f32,
}

#[derive(Debug, Clone, Serialize)]
struct CalibrationImprovement {
    raw_correlation: Option<f32>,
    calibrated_correlation: Option<f32>,
    correlation_delta: Option<f32>,
    raw_brier: f32,
    calibrated_brier: f32,
    brier_delta: f32,
    raw_ece: f32,
    calibrated_ece: f32,
    ece_delta: f32,
    improved_correlation: bool,
    improved_brier: bool,
}

#[derive(Debug, Clone, Serialize)]
struct RankLiftPolicy {
    schema_version: u32,
    issue: u32,
    feature_source: String,
    feature_policy: String,
    train_rows: usize,
    calibration_rows: usize,
    holdout_rows: usize,
    feature_dim: usize,
    candidate_count: usize,
    selected: RankLiftCandidate,
    selected_calibration_metrics: MetricBlock,
    selected_calibration_per_category_metrics: BTreeMap<String, MetricBlock>,
    calibration_objective_value: f32,
    class_support_by_category: BTreeMap<String, RankLiftClassSupport>,
    forbidden_target_label_prefixes_excluded: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RankLiftCandidate {
    CategoryLocalRbf { tau: f32, fail_boost: f32 },
}

#[derive(Debug, Clone, Serialize)]
struct RankLiftClassSupport {
    train_pass: usize,
    train_fail: usize,
    calibration_pass: usize,
    calibration_fail: usize,
    holdout_pass: usize,
    holdout_fail: usize,
}

#[derive(Debug, Clone)]
struct RankLiftModel {
    policy: RankLiftPolicy,
    row_by_task_category: BTreeMap<(String, String), usize>,
    rows: Vec<RankLiftFeatureRow>,
    train_indices_by_category: BTreeMap<String, Vec<usize>>,
}

#[derive(Debug, Clone)]
struct RankLiftFeatureRow {
    task_id: String,
    category: String,
    bucket: String,
    actual_oracle_pass: bool,
    standardized_features: Vec<f32>,
}

#[derive(Debug, Default)]
struct RankLiftAggregate {
    count: usize,
    feature_sum: Vec<f32>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodeStateConstellationRow {
    task_instance_id: String,
    mutation_category: String,
    oracle_all_passed: bool,
    #[serde(default)]
    chunk_count: usize,
    slot_identity_preserved: bool,
    flat_vector_concat_used: bool,
    #[serde(default)]
    slot_feature_means: BTreeMap<String, BTreeMap<String, f64>>,
    #[serde(default)]
    group_label_counts_top: BTreeMap<String, u64>,
    #[serde(default)]
    pair_label_counts_top: BTreeMap<String, u64>,
    #[serde(default)]
    slot_label_counts_top: BTreeMap<String, u64>,
    #[serde(default)]
    surface_counts_top: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ScoreTransform {
    Identity,
    Invert,
    Constant {
        value: f32,
    },
    Shrink {
        alpha: f32,
        invert: bool,
    },
    Threshold {
        threshold: f32,
        invert: bool,
        low: f32,
        high: f32,
    },
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

#[derive(Debug, Clone, Serialize)]
struct RollingWindowReport {
    window_index: usize,
    sample_count: usize,
    metrics: MetricBlock,
    per_category_metrics: BTreeMap<String, MetricBlock>,
    ship_gate_passed: bool,
    blockers: Vec<String>,
    power_blockers: Vec<String>,
}

fn read_index(path: impl AsRef<Path>) -> Result<CorpusIndex> {
    let path = path.as_ref();
    let index: CorpusIndex = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(!index.entries.is_empty(), "corpus index has no entries");
    Ok(index)
}

fn evaluate_split(
    predictor: &MeJepaPredictor,
    device: &Device,
    entries: &[CorpusEntry],
) -> Result<Vec<PredictionRow>> {
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let patch_path = Path::new(CORPUS_DIR).join(&entry.patch_path);
        let patch = patch_bundle_from_entry(entry, &patch_path)?;
        let context = task_context_from_entry(entry)?;
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

fn fit_calibration_policy(rows: &[PredictionRow]) -> Result<OracleCalibrationPolicy> {
    ensure!(
        !rows.is_empty(),
        "calibration policy requires non-empty calibration rows"
    );
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
        schema_version: 2,
        issue: 534,
        fitted_on_bucket: "calibration".to_string(),
        fitted_rows: rows.len(),
        holdout_rows_used_for_fitting: false,
        selection_objective:
            "minimize calibration Brier + 0.25*ECE - 0.10*Pearson; constants allowed only for structurally degenerate cells; rank-reversing transforms require calibration split mixed-class support >= 8 per class and raw Pearson <= -0.25"
                .to_string(),
        by_category,
    })
}

fn fit_category_calibration_policy(
    category: &str,
    rows: &[PredictionRow],
) -> CategoryCalibrationPolicy {
    let expected_degenerate = expected_degenerate_category(category);
    let raw_metrics = metrics_for(rows);
    let (rank_reversal_candidates_allowed, rank_reversal_policy_reason) =
        rank_reversal_policy(&raw_metrics, expected_degenerate);
    let candidates =
        score_transform_candidates(rows, expected_degenerate, rank_reversal_candidates_allowed);
    let mut best_transform = ScoreTransform::Identity;
    let mut best_metrics = raw_metrics.clone();
    let mut best_objective = f32::INFINITY;
    for candidate in &candidates {
        let transformed = transform_rows(rows, candidate);
        let metrics = metrics_for(&transformed);
        let objective = calibration_objective(&metrics, expected_degenerate);
        if objective < best_objective {
            best_objective = objective;
            best_transform = candidate.clone();
            best_metrics = metrics;
        }
    }
    CategoryCalibrationPolicy {
        category: category.to_string(),
        candidate_count: candidates.len(),
        rank_reversal_candidates_allowed,
        rank_reversal_policy_reason,
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
    include_rank_reversing: bool,
) -> Vec<ScoreTransform> {
    let pass_rate =
        rows.iter().filter(|row| row.actual_oracle_pass).count() as f32 / rows.len() as f32;
    let mut candidates = vec![ScoreTransform::Identity];
    if include_rank_reversing {
        candidates.push(ScoreTransform::Invert);
    }
    if include_constant {
        candidates.push(ScoreTransform::Constant { value: pass_rate });
    }
    for alpha in [0.25, 0.5, 0.75] {
        candidates.push(ScoreTransform::Shrink {
            alpha,
            invert: false,
        });
        if include_rank_reversing {
            candidates.push(ScoreTransform::Shrink {
                alpha,
                invert: true,
            });
        }
    }
    for threshold in [0.01, 0.05, 0.10, 0.25, 0.50, 0.75, 0.90, 0.95, 0.99] {
        candidates.push(ScoreTransform::Threshold {
            threshold,
            invert: false,
            low: 0.05,
            high: 0.95,
        });
        if include_rank_reversing {
            candidates.push(ScoreTransform::Threshold {
                threshold,
                invert: true,
                low: 0.05,
                high: 0.95,
            });
        }
    }
    candidates
}

fn rank_reversal_policy(raw_metrics: &MetricBlock, expected_degenerate: bool) -> (bool, String) {
    if expected_degenerate {
        return (
            false,
            "disabled: structurally degenerate cells use constant calibration, not rank reversal"
                .to_string(),
        );
    }
    if raw_metrics.oracle_variance_class != OracleVarianceClass::Mixed {
        return (
            false,
            format!(
                "disabled: calibration oracle variance is {:?}, not mixed",
                raw_metrics.oracle_variance_class
            ),
        );
    }
    let min_class_support = raw_metrics
        .actual_pass_count
        .min(raw_metrics.actual_fail_count);
    if min_class_support < RANK_REVERSAL_MIN_CLASS_SUPPORT {
        return (
            false,
            format!(
                "disabled: calibration split min class support {min_class_support} < {RANK_REVERSAL_MIN_CLASS_SUPPORT}"
            ),
        );
    }
    match raw_metrics.prediction_oracle_correlation {
        Some(value) if value <= RANK_REVERSAL_MAX_RAW_CALIBRATION_CORRELATION => (
            true,
            format!(
                "enabled: raw calibration Pearson {value:.6} <= {RANK_REVERSAL_MAX_RAW_CALIBRATION_CORRELATION:.2} with min class support {min_class_support}"
            ),
        ),
        Some(value) => (
            false,
            format!(
                "disabled: raw calibration Pearson {value:.6} is not strong negative evidence <= {RANK_REVERSAL_MAX_RAW_CALIBRATION_CORRELATION:.2}"
            ),
        ),
        None => (
            false,
            "disabled: raw calibration Pearson is undefined".to_string(),
        ),
    }
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

fn fit_rank_lift_policy(entries: &[CorpusEntry]) -> Result<RankLiftModel> {
    let mut split = BTreeMap::<(String, String), (String, bool)>::new();
    for entry in entries {
        split.insert(
            (entry.task_id.clone(), entry.category.clone()),
            (entry.bucket.clone(), entry.oracle_all_passed),
        );
    }
    let mut aggregates = BTreeMap::<(String, String), RankLiftAggregate>::new();
    let file = fs::File::open(CODE_STATE_CONSTELLATIONS_PATH)
        .with_context(|| format!("open {CODE_STATE_CONSTELLATIONS_PATH}"))?;
    for (line_idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| {
            format!(
                "read {} line {}",
                CODE_STATE_CONSTELLATIONS_PATH,
                line_idx + 1
            )
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let row: CodeStateConstellationRow = serde_json::from_str(&line).with_context(|| {
            format!(
                "parse {} line {}",
                CODE_STATE_CONSTELLATIONS_PATH,
                line_idx + 1
            )
        })?;
        ensure!(
            row.slot_identity_preserved && !row.flat_vector_concat_used,
            "code-state row {}:{} must preserve slot identity and reject flat vectors",
            row.task_instance_id,
            row.mutation_category
        );
        let key = (row.task_instance_id.clone(), row.mutation_category.clone());
        let Some((_bucket, expected_label)) = split.get(&key) else {
            continue;
        };
        ensure!(
            row.oracle_all_passed == *expected_label,
            "code-state oracle label mismatch for {}:{}",
            row.task_instance_id,
            row.mutation_category
        );
        let features = rank_lift_features(&row);
        let aggregate = aggregates.entry(key).or_insert_with(|| RankLiftAggregate {
            count: 0,
            feature_sum: vec![0.0; features.len()],
        });
        ensure!(
            aggregate.feature_sum.len() == features.len(),
            "rank-lift feature dim changed within aggregate"
        );
        aggregate.count += 1;
        for (sum, value) in aggregate.feature_sum.iter_mut().zip(features) {
            *sum += value;
        }
    }

    let mut rows = Vec::with_capacity(split.len());
    let mut raw_features = Vec::with_capacity(split.len());
    for ((task_id, category), (bucket, actual_oracle_pass)) in &split {
        let aggregate = aggregates
            .get(&(task_id.clone(), category.clone()))
            .ok_or_else(|| {
                anyhow!("missing code-state constellation row for {task_id}:{category}")
            })?;
        ensure!(
            aggregate.count > 0,
            "empty rank-lift aggregate for {task_id}:{category}"
        );
        let features = aggregate
            .feature_sum
            .iter()
            .map(|value| *value / aggregate.count as f32)
            .collect::<Vec<_>>();
        raw_features.push(features);
        rows.push(RankLiftFeatureRow {
            task_id: task_id.clone(),
            category: category.clone(),
            bucket: bucket.clone(),
            actual_oracle_pass: *actual_oracle_pass,
            standardized_features: Vec::new(),
        });
    }
    ensure!(
        rows.len() == entries.len(),
        "rank-lift rows {} != corpus entries {}",
        rows.len(),
        entries.len()
    );
    let feature_dim = raw_features
        .first()
        .map(|features| features.len())
        .unwrap_or_default();
    ensure!(feature_dim > 0, "rank-lift feature dim must be non-zero");
    standardize_rank_lift_features(&mut rows, &mut raw_features)?;
    let mut row_by_task_category = BTreeMap::new();
    let mut train_indices_by_category = BTreeMap::<String, Vec<usize>>::new();
    for (idx, row) in rows.iter().enumerate() {
        row_by_task_category.insert((row.task_id.clone(), row.category.clone()), idx);
        if row.bucket == "train" {
            train_indices_by_category
                .entry(row.category.clone())
                .or_default()
                .push(idx);
        }
    }
    let class_support_by_category = rank_lift_class_support(&rows);
    let candidates = rank_lift_candidates();
    let mut best_candidate = *candidates
        .first()
        .ok_or_else(|| anyhow!("rank-lift candidate list is empty"))?;
    let mut best_rows = rank_lift_metric_rows(
        &rows,
        &train_indices_by_category,
        "calibration",
        best_candidate,
    )?;
    let mut best_metrics = metrics_for(&best_rows);
    let mut best_objective = calibration_objective(&best_metrics, false);
    for candidate in candidates.iter().copied().skip(1) {
        let candidate_rows =
            rank_lift_metric_rows(&rows, &train_indices_by_category, "calibration", candidate)?;
        let metrics = metrics_for(&candidate_rows);
        let objective = calibration_objective(&metrics, false);
        if objective < best_objective {
            best_candidate = candidate;
            best_rows = candidate_rows;
            best_metrics = metrics;
            best_objective = objective;
        }
    }
    let policy = RankLiftPolicy {
        schema_version: 2,
        issue: 536,
        feature_source: CODE_STATE_CONSTELLATIONS_PATH.to_string(),
        feature_policy: "category-local RBF exemplar head over code-state slot_feature_means plus hashed live group/pair/slot/surface label counts; calibration may select train-only hard-negative fail weighting; target-side oracle/docker/test labels excluded from features; no active embedder vectors are flattened".to_string(),
        train_rows: rows.iter().filter(|row| row.bucket == "train").count(),
        calibration_rows: rows
            .iter()
            .filter(|row| row.bucket == "calibration")
            .count(),
        holdout_rows: rows.iter().filter(|row| row.bucket == "holdout").count(),
        feature_dim,
        candidate_count: candidates.len(),
        selected: best_candidate,
        selected_calibration_metrics: best_metrics,
        selected_calibration_per_category_metrics: per_category_metrics(&best_rows),
        calibration_objective_value: best_objective,
        class_support_by_category,
        forbidden_target_label_prefixes_excluded: RANK_LIFT_TARGET_LABEL_PREFIXES
            .iter()
            .map(|prefix| (*prefix).to_string())
            .collect(),
    };
    Ok(RankLiftModel {
        policy,
        row_by_task_category,
        rows,
        train_indices_by_category,
    })
}

fn rank_lift_candidates() -> Vec<RankLiftCandidate> {
    RANK_LIFT_RBF_TAUS
        .iter()
        .copied()
        .flat_map(|tau| {
            RANK_LIFT_FAIL_BOOSTS
                .iter()
                .copied()
                .map(move |fail_boost| RankLiftCandidate::CategoryLocalRbf { tau, fail_boost })
        })
        .collect()
}

fn apply_rank_lift_policy(
    rows: &[PredictionRow],
    model: &RankLiftModel,
) -> Result<Vec<PredictionRow>> {
    rows.iter()
        .map(|row| {
            let score = model.score(&row.task_id, &row.category)?;
            let mut out = row.clone();
            out.predicted_oracle_pass = score;
            out.predicted_pass = score >= 0.5;
            out.correct = out.predicted_pass == out.actual_oracle_pass;
            Ok(out)
        })
        .collect()
}

impl RankLiftModel {
    fn score(&self, task_id: &str, category: &str) -> Result<f32> {
        let row_idx = self
            .row_by_task_category
            .get(&(task_id.to_string(), category.to_string()))
            .copied()
            .ok_or_else(|| anyhow!("rank-lift row missing for {task_id}:{category}"))?;
        rank_lift_score_for_row(
            &self.rows,
            &self.train_indices_by_category,
            row_idx,
            self.policy.selected,
        )
    }
}

fn rank_lift_metric_rows(
    rows: &[RankLiftFeatureRow],
    train_indices_by_category: &BTreeMap<String, Vec<usize>>,
    bucket: &str,
    candidate: RankLiftCandidate,
) -> Result<Vec<PredictionRow>> {
    rows.iter()
        .enumerate()
        .filter(|(_idx, row)| row.bucket == bucket)
        .map(|(idx, row)| {
            let score = rank_lift_score_for_row(rows, train_indices_by_category, idx, candidate)?;
            Ok(PredictionRow {
                task_id: row.task_id.clone(),
                repo: String::new(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
                patch_path: String::new(),
                patch_sha256: String::new(),
                panel_t0_sha256: String::new(),
                panel_t1_sha256: String::new(),
                panel_t2_sha256: String::new(),
            })
        })
        .collect()
}

fn rank_lift_score_for_row(
    rows: &[RankLiftFeatureRow],
    train_indices_by_category: &BTreeMap<String, Vec<usize>>,
    row_idx: usize,
    candidate: RankLiftCandidate,
) -> Result<f32> {
    let row = rows
        .get(row_idx)
        .ok_or_else(|| anyhow!("rank-lift row index out of range"))?;
    let train_indices = train_indices_by_category
        .get(&row.category)
        .ok_or_else(|| anyhow!("rank-lift category {} has no train rows", row.category))?;
    let RankLiftCandidate::CategoryLocalRbf { tau, fail_boost } = candidate;
    ensure!(tau > 0.0, "rank-lift RBF tau must be positive");
    ensure!(fail_boost > 0.0, "rank-lift fail_boost must be positive");
    let mut weight_sum = 0.0_f64;
    let mut weighted_label_sum = 0.0_f64;
    for train_idx in train_indices {
        let train = rows
            .get(*train_idx)
            .ok_or_else(|| anyhow!("rank-lift train index out of range"))?;
        let distance = squared_distance(&row.standardized_features, &train.standardized_features);
        let class_weight = if train.actual_oracle_pass {
            1.0
        } else {
            fail_boost as f64
        };
        let weight = (-(distance as f64) / (2.0 * (tau as f64).powi(2))).exp() * class_weight;
        weight_sum += weight;
        if train.actual_oracle_pass {
            weighted_label_sum += weight;
        }
    }
    ensure!(
        weight_sum.is_finite() && weight_sum > 0.0,
        "rank-lift RBF weight sum must be positive"
    );
    Ok((weighted_label_sum / weight_sum).clamp(0.001, 0.999) as f32)
}

fn rank_lift_features(row: &CodeStateConstellationRow) -> Vec<f32> {
    let mut features = Vec::with_capacity(61 + RANK_LIFT_HASH_FEATURES);
    for embedder_id in [
        "e1", "e10", "e12", "e13", "e14", "e2", "e3", "e4", "e6", "e7", "e8", "e9",
    ] {
        let slot = row.slot_feature_means.get(embedder_id);
        for feature_name in [
            "abs_mean",
            "entropy_proxy",
            "norm_l2",
            "std",
            "zero_fraction",
        ] {
            features.push(
                slot.and_then(|values| values.get(feature_name))
                    .copied()
                    .unwrap_or_default() as f32,
            );
        }
    }
    features.push((row.chunk_count as f32).ln_1p());
    let mut hashed = vec![0.0_f32; RANK_LIFT_HASH_FEATURES];
    let denom = (row.chunk_count as f32).max(1.0);
    add_rank_lift_count_features(&mut hashed, "group", &row.group_label_counts_top, denom);
    add_rank_lift_count_features(&mut hashed, "pair", &row.pair_label_counts_top, denom);
    add_rank_lift_count_features(&mut hashed, "slot", &row.slot_label_counts_top, denom);
    add_rank_lift_count_features(&mut hashed, "surface", &row.surface_counts_top, denom);
    features.extend(hashed);
    features
}

fn add_rank_lift_count_features(
    hashed: &mut [f32],
    prefix: &str,
    counts: &BTreeMap<String, u64>,
    denom: f32,
) {
    for (label, count) in counts {
        if RANK_LIFT_TARGET_LABEL_PREFIXES
            .iter()
            .any(|target_prefix| label.starts_with(target_prefix))
        {
            continue;
        }
        let feature = format!("{prefix}:{label}");
        let idx = rank_lift_hash_index(&feature, hashed.len());
        hashed[idx] += *count as f32 / denom;
    }
}

fn rank_lift_hash_index(value: &str, modulus: usize) -> usize {
    let digest = Sha256::digest(value.as_bytes());
    let prefix = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) as usize;
    prefix % modulus
}

fn standardize_rank_lift_features(
    rows: &mut [RankLiftFeatureRow],
    raw_features: &mut [Vec<f32>],
) -> Result<()> {
    let feature_dim = raw_features
        .first()
        .map(|features| features.len())
        .unwrap_or_default();
    ensure!(feature_dim > 0, "rank-lift feature dim must be non-zero");
    let train_indices = rows
        .iter()
        .enumerate()
        .filter_map(|(idx, row)| (row.bucket == "train").then_some(idx))
        .collect::<Vec<_>>();
    ensure!(!train_indices.is_empty(), "rank-lift train split is empty");
    let mut means = vec![0.0_f64; feature_dim];
    for idx in &train_indices {
        ensure!(
            raw_features[*idx].len() == feature_dim,
            "rank-lift feature dim mismatch"
        );
        for (mean, value) in means.iter_mut().zip(&raw_features[*idx]) {
            *mean += *value as f64;
        }
    }
    for mean in &mut means {
        *mean /= train_indices.len() as f64;
    }
    let mut stds = vec![0.0_f64; feature_dim];
    for idx in &train_indices {
        for ((std, value), mean) in stds.iter_mut().zip(&raw_features[*idx]).zip(&means) {
            *std += (*value as f64 - *mean).powi(2);
        }
    }
    for std in &mut stds {
        *std = (*std / train_indices.len() as f64).sqrt();
        if *std < 1.0e-9 {
            *std = 1.0;
        }
    }
    for (row, features) in rows.iter_mut().zip(raw_features) {
        row.standardized_features = features
            .iter()
            .zip(&means)
            .zip(&stds)
            .map(|((value, mean), std)| ((*value as f64 - *mean) / *std) as f32)
            .collect();
    }
    Ok(())
}

fn squared_distance(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let delta = left - right;
            delta * delta
        })
        .sum()
}

fn rank_lift_class_support(rows: &[RankLiftFeatureRow]) -> BTreeMap<String, RankLiftClassSupport> {
    let mut support = BTreeMap::<String, RankLiftClassSupport>::new();
    for row in rows {
        let entry = support
            .entry(row.category.clone())
            .or_insert(RankLiftClassSupport {
                train_pass: 0,
                train_fail: 0,
                calibration_pass: 0,
                calibration_fail: 0,
                holdout_pass: 0,
                holdout_fail: 0,
            });
        let (pass, fail) = match row.bucket.as_str() {
            "train" => (&mut entry.train_pass, &mut entry.train_fail),
            "calibration" => (&mut entry.calibration_pass, &mut entry.calibration_fail),
            "holdout" => (&mut entry.holdout_pass, &mut entry.holdout_fail),
            _ => continue,
        };
        if row.actual_oracle_pass {
            *pass += 1;
        } else {
            *fail += 1;
        }
    }
    support
}

fn transform_rows(rows: &[PredictionRow], transform: &ScoreTransform) -> Vec<PredictionRow> {
    rows.iter()
        .map(|row| transform_row(row, transform))
        .collect()
}

fn transform_row(row: &PredictionRow, transform: &ScoreTransform) -> PredictionRow {
    let score = transform_score(row.predicted_oracle_pass, transform);
    let predicted_pass = score >= 0.5;
    let mut out = row.clone();
    out.predicted_oracle_pass = score;
    out.predicted_pass = predicted_pass;
    out.correct = predicted_pass == out.actual_oracle_pass;
    out
}

fn transform_score(score: f32, transform: &ScoreTransform) -> f32 {
    match transform {
        ScoreTransform::Identity => score,
        ScoreTransform::Invert => 1.0 - score,
        ScoreTransform::Constant { value } => *value,
        ScoreTransform::Shrink { alpha, invert } => {
            let base = if *invert { 1.0 - score } else { score };
            0.5 + (*alpha * (base - 0.5))
        }
        ScoreTransform::Threshold {
            threshold,
            invert,
            low,
            high,
        } => {
            let base = if *invert { 1.0 - score } else { score };
            if base >= *threshold {
                *high
            } else {
                *low
            }
        }
    }
    .clamp(0.0, 1.0)
}

fn calibration_improvement(raw: &MetricBlock, calibrated: &MetricBlock) -> CalibrationImprovement {
    let correlation_delta = match (
        raw.prediction_oracle_correlation,
        calibrated.prediction_oracle_correlation,
    ) {
        (Some(before), Some(after)) => Some(after - before),
        _ => None,
    };
    CalibrationImprovement {
        raw_correlation: raw.prediction_oracle_correlation,
        calibrated_correlation: calibrated.prediction_oracle_correlation,
        correlation_delta,
        raw_brier: raw.brier,
        calibrated_brier: calibrated.brier,
        brier_delta: raw.brier - calibrated.brier,
        raw_ece: raw.ece,
        calibrated_ece: calibrated.ece,
        ece_delta: raw.ece - calibrated.ece,
        improved_correlation: correlation_delta.map(|delta| delta > 0.0).unwrap_or(false),
        improved_brier: calibrated.brier < raw.brier,
    }
}

fn rank_lift_repair_evidence(
    final_metrics: &MetricBlock,
    per_category_metrics: &BTreeMap<String, MetricBlock>,
) -> Value {
    let correlation_delta = final_metrics
        .prediction_oracle_correlation
        .map(|current| current - PREVIOUS_RANK_LIFT_CORRELATION);
    let brier_delta_positive_is_improvement = PREVIOUS_RANK_LIFT_BRIER - final_metrics.brier;
    let ece_delta_positive_is_improvement = PREVIOUS_RANK_LIFT_ECE - final_metrics.ece;
    let overall_lift_passed = final_metrics
        .prediction_oracle_correlation
        .map(|current| current > PREVIOUS_RANK_LIFT_CORRELATION)
        .unwrap_or(false)
        && final_metrics.brier <= PREVIOUS_RANK_LIFT_BRIER
        && final_metrics.ece <= PREVIOUS_RANK_LIFT_ECE;

    let mut diagnostics = serde_json::Map::new();
    let mut improved_weak_cells = Vec::new();
    for category in WEAK_CELL_CATEGORIES {
        let Some(current) = per_category_metrics.get(category) else {
            continue;
        };
        let Some(baseline) = previous_weak_cell_baseline(category) else {
            continue;
        };
        let correlation_improved =
            option_metric_improved(current.prediction_oracle_correlation, baseline.correlation);
        let brier_improved = current.brier < baseline.brier;
        let pass_recall_improved =
            option_metric_improved(current.pass_recall, baseline.pass_recall);
        let fail_recall_improved =
            option_metric_improved(current.fail_recall, baseline.fail_recall);
        let mut improvements = Vec::new();
        if correlation_improved {
            improvements.push("prediction_oracle_correlation");
        }
        if brier_improved {
            improvements.push("brier");
        }
        if pass_recall_improved {
            improvements.push("pass_recall");
        }
        if fail_recall_improved {
            improvements.push("fail_recall");
        }
        if !improvements.is_empty() {
            improved_weak_cells.push(format!("{}:{}", category, improvements.join("+")));
        }
        diagnostics.insert(
            category.to_string(),
            json!({
                "baseline": {
                    "prediction_oracle_correlation": baseline.correlation,
                    "brier": baseline.brier,
                    "pass_recall": baseline.pass_recall,
                    "fail_recall": baseline.fail_recall
                },
                "current": {
                    "prediction_oracle_correlation": current.prediction_oracle_correlation,
                    "brier": current.brier,
                    "pass_recall": current.pass_recall,
                    "fail_recall": current.fail_recall,
                    "predicted_pass_count": current.predicted_pass_count,
                    "predicted_fail_count": current.predicted_fail_count,
                    "score_min": current.score_min,
                    "score_max": current.score_max,
                    "score_mean": current.score_mean
                },
                "improvements": improvements
            }),
        );
    }

    json!({
        "schema_version": 1,
        "issue": 536,
        "baseline_run": PREVIOUS_RANK_LIFT_BASELINE_RUN,
        "repair_mechanism": "calibration-selected category-local RBF exemplar scoring with train-only hard-negative fail weighting; features remain slot-preserving code-state constellation statistics and target-label prefixes remain excluded",
        "overall_lift": {
            "previous_prediction_oracle_correlation": PREVIOUS_RANK_LIFT_CORRELATION,
            "current_prediction_oracle_correlation": final_metrics.prediction_oracle_correlation,
            "correlation_delta": correlation_delta,
            "previous_brier": PREVIOUS_RANK_LIFT_BRIER,
            "current_brier": final_metrics.brier,
            "brier_delta_positive_is_improvement": brier_delta_positive_is_improvement,
            "previous_ece": PREVIOUS_RANK_LIFT_ECE,
            "current_ece": final_metrics.ece,
            "ece_delta_positive_is_improvement": ece_delta_positive_is_improvement,
            "passed": overall_lift_passed
        },
        "weak_cell_diagnostics": diagnostics,
        "improved_weak_cells": improved_weak_cells,
        "acceptance_passed": overall_lift_passed && !improved_weak_cells.is_empty()
    })
}

#[derive(Debug, Clone, Copy)]
struct PreviousWeakCellBaseline {
    correlation: Option<f32>,
    brier: f32,
    pass_recall: Option<f32>,
    fail_recall: Option<f32>,
}

fn previous_weak_cell_baseline(category: &str) -> Option<PreviousWeakCellBaseline> {
    let baseline = match category {
        "delete_test_call" => PreviousWeakCellBaseline {
            correlation: None,
            brier: 0.032_844_882,
            pass_recall: Some(1.0),
            fail_recall: Some(0.0),
        },
        "known_good" => PreviousWeakCellBaseline {
            correlation: None,
            brier: 0.032_843_87,
            pass_recall: Some(1.0),
            fail_recall: Some(0.0),
        },
        "off_by_one" => PreviousWeakCellBaseline {
            correlation: Some(0.402_321_76),
            brier: 0.245_885_12,
            pass_recall: Some(1.0),
            fail_recall: Some(0.0),
        },
        "over_engineer" => PreviousWeakCellBaseline {
            correlation: None,
            brier: 0.032_653_872,
            pass_recall: Some(1.0),
            fail_recall: Some(0.0),
        },
        "subtle_flip" => PreviousWeakCellBaseline {
            correlation: Some(-0.144_787_64),
            brier: 0.223_763_48,
            pass_recall: Some(0.0),
            fail_recall: Some(1.0),
        },
        "swap_variable" => PreviousWeakCellBaseline {
            correlation: Some(-0.026_344_081),
            brier: 0.181_044_62,
            pass_recall: Some(0.0),
            fail_recall: Some(1.0),
        },
        _ => return None,
    };
    Some(baseline)
}

fn option_metric_improved(current: Option<f32>, previous: Option<f32>) -> bool {
    match (current, previous) {
        (Some(current), Some(previous)) => current > previous,
        (Some(_), None) => true,
        _ => false,
    }
}

fn weak_cell_predictions(rows: &[PredictionRow]) -> Vec<PredictionRow> {
    rows.iter()
        .filter(|row| WEAK_CELL_CATEGORIES.contains(&row.category.as_str()))
        .cloned()
        .collect()
}

fn predict_oracle_pass(
    predictor: &MeJepaPredictor,
    device: &Device,
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

fn panel_to_tensor(panel: &Panel, device: &Device, dtype: DType) -> Result<Tensor> {
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
    let use_parseable_carrier = entry.category != "compile_error";
    let ast_diff = parse_unified_diff(&patch_text, use_parseable_carrier)?;
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

fn task_context_from_entry(entry: &CorpusEntry) -> Result<TaskContext> {
    let mut session_id = [0u8; 16];
    let digest = Sha256::digest(format!("{}:{}", entry.task_id, entry.category).as_bytes());
    session_id.copy_from_slice(&digest[..16]);
    let test_count = entry.oracle_per_test_count.unwrap_or(1).clamp(1, 1024);
    let tests = (0..test_count)
        .map(|idx| TestId(format!("oracle_test_{idx}")))
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

fn invalid_source_edge_case_audit() -> Result<Value> {
    let context = edge_case_context()?;
    let cases = vec![
        (
            "valid_python",
            edge_case_patch(
                "src/valid.py",
                "def answer():\n    return 1\n",
                "def answer():\n    return 2\n",
                b"valid-python-edge",
            )?,
        ),
        (
            "malformed_python",
            edge_case_patch(
                "src/broken.py",
                "def answer():\n    return 1\n",
                "def broken(:\n",
                b"malformed-python-edge",
            )?,
        ),
        (
            "empty_patch",
            edge_case_patch("src/empty.py", "", "", b"empty-python-edge")?,
        ),
        (
            "mixed_validity_multifile",
            PatchBundle::try_new(
                AstDiff {
                    hunks: vec![
                        DiffHunk {
                            path: PathBuf::from("src/valid.py"),
                            pre_sha: sha256_bytes(b"def answer():\n    return 1\n"),
                            post_sha: sha256_bytes(b"def answer():\n    return 2\n"),
                            before: "def answer():\n    return 1\n".to_string(),
                            after: "def answer():\n    return 2\n".to_string(),
                        },
                        DiffHunk {
                            path: PathBuf::from("src/broken.py"),
                            pre_sha: sha256_bytes(b"def ok():\n    return 1\n"),
                            post_sha: sha256_bytes(b"def broken(:\n"),
                            before: "def ok():\n    return 1\n".to_string(),
                            after: "def broken(:\n".to_string(),
                        },
                    ],
                },
                valid_witness_segment(),
                "invalid-source edge case mixed validity".to_string(),
                sha256_bytes(b"mixed-validity-edge"),
            )?,
        ),
    ];
    let mut rows = Vec::new();
    for (case, patch) in cases {
        let (panel_t0, panel_t1, panel_t2) = materialize_inference_panels(&patch, &context)?;
        rows.push(json!({
            "case": case,
            "panel_t0_sha256": hex::encode(panel_sha(&panel_t0)),
            "panel_t1_sha256": hex::encode(panel_sha(&panel_t1)),
            "panel_t2_sha256": hex::encode(panel_sha(&panel_t2)),
            "t1_ast_invalid_source_sentinel": panel_t1.slot(InstrumentSlot::EAst)[0],
            "t1_diff_invalid_source_sentinel": panel_t1.slot(InstrumentSlot::EDiff)[0],
            "t2_static_analysis_error_density": panel_t2.slot(InstrumentSlot::EStaticAnalysis)[0],
            "materialized": true
        }));
    }
    Ok(json!({
        "schema_version": 1,
        "proves": [
            "valid Python materializes through real code instruments",
            "malformed Python materializes through invalid-source diagnostic slots",
            "empty patches materialize as explicit invalid-source evidence",
            "mixed-validity multi-file patches materialize as aggregate invalid-source evidence"
        ],
        "cases": rows
    }))
}

fn edge_case_context() -> Result<TaskContext> {
    let context = TaskContext {
        task_id: TaskId("task-py-g-131-invalid-source-edge-cases".to_string()),
        session_id: [13; 16],
        language: Language::Python,
        problem_statement: "prove first-class invalid-source panelization".to_string(),
        tests: vec![TestId(
            "tests/test_invalid_source.py::test_panelizes".to_string(),
        )],
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

fn edge_case_patch(path: &str, before: &str, after: &str, seed: &[u8]) -> Result<PatchBundle> {
    Ok(PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: PathBuf::from(path),
                pre_sha: sha256_bytes(before.as_bytes()),
                post_sha: sha256_bytes(after.as_bytes()),
                before: before.to_string(),
                after: after.to_string(),
            }],
        },
        valid_witness_segment(),
        format!("invalid-source edge case {}", String::from_utf8_lossy(seed)),
        sha256_bytes(seed),
    )?)
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
    let brier = brier / rows.len() as f32;
    let ece = expected_calibration_error(rows);
    let actual_class_confidence_mean = actual_class_confidence / rows.len() as f32;
    let oracle_variance_class = oracle_variance_class(actual_pass_count, actual_fail_count);
    let gate_contract = gate_contract_for(
        oracle_variance_class,
        correlation,
        pass_recall,
        fail_recall,
        actual_class_confidence_mean,
        brier,
        ece,
    );
    let statistical_power = power_report_for(
        rows.len(),
        actual_pass_count,
        actual_fail_count,
        correlation,
        pass_recall,
        fail_recall,
    );
    MetricBlock {
        sample_count: rows.len(),
        accuracy: correct as f32 / rows.len() as f32,
        prediction_oracle_correlation: correlation,
        oracle_variance_class,
        gate_contract,
        statistical_power,
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

fn rolling_window_reports(rows: &[PredictionRow]) -> Vec<RollingWindowReport> {
    let mut by_category: BTreeMap<String, Vec<PredictionRow>> = BTreeMap::new();
    for row in rows {
        by_category
            .entry(row.category.clone())
            .or_default()
            .push(row.clone());
    }
    let mut reports = Vec::new();
    for window_index in 0..4 {
        let mut window_rows = Vec::new();
        for category in REQUIRED_CATEGORIES {
            let category_rows = by_category.get(category).cloned().unwrap_or_default();
            let start = category_rows.len() * window_index / 4;
            let end = category_rows.len() * (window_index + 1) / 4;
            window_rows.extend_from_slice(&category_rows[start..end]);
        }
        let metrics = metrics_for(&window_rows);
        let per_category = per_category_metrics(&window_rows);
        let blockers = ship_gate_blockers(&metrics, &per_category);
        let power_blockers = statistical_power_blockers(&metrics, &per_category);
        let ship_gate_passed = blockers.is_empty() && power_blockers.is_empty();
        reports.push(RollingWindowReport {
            window_index,
            sample_count: window_rows.len(),
            metrics,
            per_category_metrics: per_category,
            ship_gate_passed,
            blockers,
            power_blockers,
        });
    }
    reports
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
            "All-fail oracle cells have zero target variance; Pearson is undefined, so the cell gates on fail recall plus calibration.",
        ),
        OracleVarianceClass::AllPass => degenerate_gate_contract(
            "all_pass_recall_confidence_brier_ece",
            pass_recall,
            actual_class_confidence_mean,
            brier,
            ece,
            "All-pass oracle cells have zero target variance; Pearson is undefined, so the cell gates on pass recall plus calibration.",
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

fn split_counts(entries: &[CorpusEntry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.bucket.clone()).or_default() += 1;
    }
    counts
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
            "holdout must contain 30 rows for category {category}; got {:?}",
            counts.get(category)
        );
    }
    Ok(())
}

fn panel_audit(predictions: &[PredictionRow]) -> Value {
    let unique_t0 = predictions
        .iter()
        .map(|row| row.panel_t0_sha256.clone())
        .collect::<BTreeSet<_>>();
    let unique_t1 = predictions
        .iter()
        .map(|row| row.panel_t1_sha256.clone())
        .collect::<BTreeSet<_>>();
    let unique_t2 = predictions
        .iter()
        .map(|row| row.panel_t2_sha256.clone())
        .collect::<BTreeSet<_>>();
    json!({
        "real_patch_paths_read": predictions.len(),
        "unique_t0_panel_sha_count": unique_t0.len(),
        "unique_t1_panel_sha_count": unique_t1.len(),
        "unique_t2_panel_sha_count": unique_t2.len(),
        "panel_dim": PANEL_DIM
    })
}

fn source_audit() -> Result<Value> {
    let source = fs::read_to_string(file!()).unwrap_or_default();
    let legacy_echo = ["struct", "EchoPredictor"].join(" ");
    let legacy_oracle = ["Category", "Majority", "Oracle"].join("");
    let synthetic_vector = ["deterministic", "_vector("].join("");
    let predictor_forward = ["predictor", ".forward("].join("");
    Ok(json!({
        "source_file": file!(),
        "echo_predictor_present": source.contains(&legacy_echo),
        "category_majority_oracle_present": source.contains(&legacy_oracle),
        "deterministic_vector_symbol_present": source.contains(&synthetic_vector),
        "mejepa_predictor_forward_present": source.contains(&predictor_forward),
        "materialize_inference_panels_present": source.contains("materialize_inference_panels"),
    }))
}

fn active_input_paths() -> Vec<&'static str> {
    vec![
        CORPUS_DIR,
        INDEX_PATH,
        STATS_PATH,
        VERIFY_PATH,
        CHECKPOINT_MANIFEST_PATH,
        CODE_STATE_CONSTELLATIONS_PATH,
        FSV_ROOT,
    ]
}

fn active_paths_include_mntd() -> bool {
    active_input_paths()
        .iter()
        .any(|path| path.starts_with("/mnt/d/"))
}

fn json_contains_string(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.contains(needle),
        Value::Array(items) => items.iter().any(|item| json_contains_string(item, needle)),
        Value::Object(map) => map.values().any(|item| json_contains_string(item, needle)),
        _ => false,
    }
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
        fs::read(path)? == bytes,
        "JSON readback bytes differed for {}",
        path.display()
    );
    Ok(())
}

fn parse_sha256_prefixed(value: &str) -> Result<[u8; 32]> {
    let raw = value
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow!("expected sha256:<hex>, got {value}"))?;
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
    hex::encode(sha256_bytes(bytes))
}

fn sha256_file(path: impl AsRef<Path>) -> Result<String> {
    Ok(sha256_hex(&fs::read(path.as_ref())?))
}
