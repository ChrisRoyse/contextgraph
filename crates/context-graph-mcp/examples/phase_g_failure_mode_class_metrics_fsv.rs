//! FSV for TASK-G-020 per-failure-mode-class precision/recall reporting.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mcp::daemon::{write_weekly_eval_markdown, WeeklyOperationalSummary};
use context_graph_mejepa::eval::metrics::{ship_gate_failures, ShipGateInputs};
use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::{
    ActiveLearningSummary, ChunkId, ConformalSet, EvalConfig, EvalObservation, EvalProvenance,
    EvalReport, FailureModeClass, Language, MutationCategory, OracleOutcome, PredictedFailureMode,
    RealityPrediction, RealityPredictionBuilder, RootCauseClass, Severity, TaskId, Verdict,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-failure-mode-class-metrics-fsv";
const TASK_ID: &str = "TASK-G-020";

fn main() -> Result<()> {
    let output_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_FSV_ROOT));
    let evidence = run(&output_root)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn run(output_root: &Path) -> Result<Value> {
    if output_root.exists() {
        fs::remove_dir_all(output_root)
            .with_context(|| format!("remove {}", output_root.display()))?;
    }
    fs::create_dir_all(output_root).with_context(|| format!("create {}", output_root.display()))?;
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let run_root = output_root.join(format!("run-{started_at_unix_ms}-{}", std::process::id()));
    fs::create_dir_all(&run_root).with_context(|| format!("create {}", run_root.display()))?;

    let config = EvalConfig::default();
    let observations = synthetic_confusion_matrix()?;
    let metrics = context_graph_mejepa::compute_failure_mode_class_metrics(&observations, &config)
        .context("compute per-failure-mode-class metrics")?;
    ensure!(
        metrics.len() == FailureModeClass::all().len(),
        "metrics did not include every FailureModeClass"
    );

    let failures = ship_gate_failures(ShipGateInputs {
        observations: &observations,
        report_overall: Some(0.96),
        per_cell: &BTreeMap::new(),
        cell_exemptions: &BTreeMap::new(),
        conformal: &BTreeMap::new(),
        ood: &BTreeMap::new(),
        gtau: &BTreeMap::new(),
        per_failure_mode_class: &metrics,
        regression_checks: &[],
        q1_pass_rate: 1.0,
        q3_side_effect_agreement: Some(1.0),
        config: &config,
    });
    ensure!(
        failures
            .iter()
            .any(|failure| failure.contains("wrong_algorithm")),
        "ship gate did not call out wrong_algorithm weakness"
    );
    ensure!(
        failures
            .iter()
            .any(|failure| failure.contains("compile_error")),
        "ship gate did not call out compile_error weakness"
    );
    ensure!(
        failures.iter().any(|failure| failure.contains("timeout")),
        "ship gate did not call out timeout weakness"
    );

    let report = synthetic_report(metrics, failures);
    report.validate().context("validate synthetic EvalReport")?;

    let report_json_path = run_root.join("weekly.json");
    write_json_0600(&report_json_path, &report)
        .with_context(|| format!("write {}", report_json_path.display()))?;
    let report_readback: EvalReport = serde_json::from_slice(
        &fs::read(&report_json_path)
            .with_context(|| format!("read {}", report_json_path.display()))?,
    )
    .context("deserialize report readback")?;
    let report_hash = report.determinism_hash()?;
    ensure!(
        report_readback.determinism_hash()? == report_hash,
        "weekly.json readback hash mismatch"
    );

    let markdown_path = run_root.join("weekly.md");
    write_weekly_eval_markdown(
        &markdown_path,
        &report,
        &report_hash,
        &WeeklyOperationalSummary::new(0, 0, 0, 0, 0, 0),
    )
    .context("write weekly markdown")?;
    let markdown = fs::read_to_string(&markdown_path)
        .with_context(|| format!("read {}", markdown_path.display()))?;
    let expected_rows = expected_markdown_rows();
    let rendered_rows = expected_rows
        .iter()
        .filter(|row| markdown.contains(*row))
        .count();
    ensure!(
        rendered_rows == expected_rows.len(),
        "not every expected failure-mode metrics row rendered"
    );

    let boundary_cases = vec![
        boundary_all_classes_present(&report),
        boundary_zero_support_class_passes(&report),
        boundary_low_precision_only(&report),
        boundary_low_recall_only(&report),
        boundary_low_precision_and_recall(&report),
        boundary_missing_class_rejected(&report),
    ];
    let all_boundaries_pass = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let evidence_path = output_root.join("failure_mode_class_metrics_fsv.json");
    let evidence = json!({
        "fsv_root": output_root,
        "task_id": TASK_ID,
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_sha(),
        "run_root": run_root,
        "happy_path": [{
            "case": "synthetic_confusion_matrix_renders_failure_mode_metrics",
            "sot": {
                "json_report": report_json_path,
                "weekly_markdown": markdown_path
            },
            "before": null,
            "trigger": "cargo run --release -p context-graph-mcp --example phase_g_failure_mode_class_metrics_fsv",
            "after": {
                "report_hash": report_hash,
                "markdown_sha256": sha256_bytes(markdown.as_bytes()),
                "metric_classes": report.per_failure_mode_class.len(),
                "rendered_rows": rendered_rows,
                "ship_gate_failures": report.ship_gate_failures
            },
            "expected": {
                "metric_classes": FailureModeClass::all().len(),
                "thresholds": {
                    "precision": 0.90,
                    "recall": 0.70
                },
                "rows": expected_rows
            },
            "actual": {
                "metric_classes": report.per_failure_mode_class.len(),
                "rows_present": rendered_rows
            },
            "pass": true,
            "evidence_path": markdown_path
        }],
        "boundary_cases": boundary_cases,
        "all_passed": all_boundaries_pass,
        "readback_equal": true,
        "physical_artifacts": {
            "weekly_json": file_artifact(&report_json_path)?,
            "weekly_markdown": file_artifact(&markdown_path)?,
            "fsv_json": evidence_path
        }
    });
    ensure!(
        evidence["all_passed"].as_bool() == Some(true),
        "FSV evidence did not pass"
    );
    write_json_0600(&evidence_path, &evidence)
        .with_context(|| format!("write {}", evidence_path.display()))?;
    Ok(evidence)
}

fn synthetic_confusion_matrix() -> Result<Vec<EvalObservation>> {
    Ok(vec![
        observation(
            "off-by-one-tp-1",
            OracleOutcome::Fail,
            vec![FailureModeClass::OffByOne],
            vec![FailureModeClass::OffByOne],
        )?,
        observation(
            "off-by-one-tp-2",
            OracleOutcome::Fail,
            vec![FailureModeClass::OffByOne],
            vec![FailureModeClass::OffByOne],
        )?,
        observation(
            "wrong-algorithm-fp",
            OracleOutcome::Pass,
            vec![],
            vec![FailureModeClass::WrongAlgorithm],
        )?,
        observation(
            "wrong-algorithm-tp",
            OracleOutcome::Fail,
            vec![FailureModeClass::WrongAlgorithm],
            vec![FailureModeClass::WrongAlgorithm],
        )?,
        observation(
            "compile-error-tp",
            OracleOutcome::Fail,
            vec![FailureModeClass::CompileError],
            vec![FailureModeClass::CompileError],
        )?,
        observation(
            "compile-error-fn",
            OracleOutcome::Fail,
            vec![FailureModeClass::CompileError],
            vec![],
        )?,
        observation(
            "timeout-fn",
            OracleOutcome::Fail,
            vec![FailureModeClass::Timeout],
            vec![],
        )?,
        observation(
            "timeout-fp",
            OracleOutcome::Pass,
            vec![],
            vec![FailureModeClass::Timeout],
        )?,
    ])
}

fn observation(
    slug: &str,
    actual_oracle: OracleOutcome,
    actual_failure_modes: Vec<FailureModeClass>,
    predicted_failure_modes: Vec<FailureModeClass>,
) -> Result<EvalObservation> {
    let task_id = TaskId(format!("task-g-020-{slug}"));
    Ok(EvalObservation {
        task_id: task_id.clone(),
        mutation_category: MutationCategory::CompileError,
        language: Language::Python,
        actual_oracle,
        actual_failure_modes,
        prediction: prediction(&task_id, slug, predicted_failure_modes)?,
        gtau_passed: true,
        approved: true,
        live_prediction_readback: true,
        latency_ms: 1.0,
    })
}

fn prediction(
    task_id: &TaskId,
    slug: &str,
    failure_classes: Vec<FailureModeClass>,
) -> Result<RealityPrediction> {
    let predicted_failure_modes = failure_classes
        .into_iter()
        .map(|failure_class| failure_mode(slug, failure_class))
        .collect::<Vec<_>>();
    let predicted_oracle_pass = if predicted_failure_modes.is_empty() {
        0.95
    } else {
        0.05
    };
    Ok(RealityPredictionBuilder::from_parts(
        task_id.clone(),
        [20; 16],
        Language::Python,
        ConformalSet::try_new(vec![OracleOutcome::Pass, OracleOutcome::Fail], 0.1, 0.5)?,
    )
    .prediction_id(prediction_id(slug))
    .verdict(if predicted_failure_modes.is_empty() {
        Verdict::Pass
    } else {
        Verdict::Fail
    })
    .predicted_oracle_pass(predicted_oracle_pass)
    .predicted_test_pass(vec![predicted_oracle_pass])
    .predicted_runtime_trace([0.0; 32])
    .ood_score(0.0)
    .calibrated_confidence(0.95)
    .predicted_failure_modes(predicted_failure_modes)
    .calibration_version("task-g-020-fsv")
    .build()?)
}

fn failure_mode(slug: &str, failure_class: FailureModeClass) -> PredictedFailureMode {
    PredictedFailureMode {
        failure_class,
        chunk: ChunkId(format!("{slug}#chunk")),
        line_range: (1, 2),
        confidence: 0.95,
        severity: Severity::High,
        explanation: format!("synthetic {}", failure_class.slug()),
        contributing_embedders: Vec::new(),
        root_cause_class: RootCauseClass::LogicError,
    }
}

fn prediction_id(slug: &str) -> [u8; 16] {
    let digest = Sha256::digest(slug.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn synthetic_report(
    per_failure_mode_class: BTreeMap<
        FailureModeClass,
        context_graph_mejepa::FailureModeClassMetrics,
    >,
    ship_gate_failures: Vec<String>,
) -> EvalReport {
    EvalReport {
        report_date: "2026-05-13".to_string(),
        generated_at_unix_ms: 1_778_666_683_000,
        rolling_window_size: 8,
        holdout_count: 8,
        overall_correlation: Some(0.96),
        per_category_correlation: BTreeMap::new(),
        per_language_correlation: BTreeMap::new(),
        per_cell_correlation: BTreeMap::new(),
        cell_exemptions: BTreeMap::new(),
        bayesian_shrinkage: BTreeMap::new(),
        conformal_coverage_health: BTreeMap::new(),
        ood_calibration_health: BTreeMap::new(),
        gtau_pass_rate: BTreeMap::new(),
        per_prediction_class_calibration: BTreeMap::new(),
        per_failure_mode_class,
        active_learning: ActiveLearningSummary {
            queued_count: 0,
            evicted_count: 0,
            ood_escalation_count: 0,
        },
        state_transfer_diagnostic: None,
        per_cell_state_transfer: BTreeMap::new(),
        failing_cell_classifications: BTreeMap::new(),
        aux_head_distillation: None,
        regression_checks: Vec::new(),
        open_research_questions: Vec::new(),
        q1_pass_rate: 1.0,
        q2_report_correlation: Some(0.96),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: false,
        ship_gate_failures,
        provenance: EvalProvenance {
            corpus_sha: "task-g-020-synthetic-confusion-matrix".to_string(),
            eval_code_version: "task-g-020-fsv".to_string(),
            calibration_version: "task-g-020-fsv".to_string(),
            generated_by: "phase_g_failure_mode_class_metrics_fsv".to_string(),
        },
        wall_clock_seconds: 0.01,
    }
}

fn expected_markdown_rows() -> Vec<String> {
    vec![
        "| off_by_one | 1.000000 | 1.000000 | 1.000000 | 2 | 0 | 0 | 8 | true | none |".to_string(),
        "| wrong_algorithm | 0.500000 | 1.000000 | 0.666667 | 1 | 1 | 0 | 8 | false | low_precision |".to_string(),
        "| compile_error | 1.000000 | 0.500000 | 0.666667 | 1 | 0 | 1 | 8 | false | low_recall |".to_string(),
        "| timeout | 0.000000 | 0.000000 | 0.000000 | 0 | 1 | 1 | 8 | false | low_precision_and_low_recall |".to_string(),
    ]
}

fn boundary_all_classes_present(report: &EvalReport) -> Value {
    json!({
        "case": "all_failure_mode_classes_present",
        "expected": FailureModeClass::all().len(),
        "actual": report.per_failure_mode_class.len(),
        "pass": report.per_failure_mode_class.len() == FailureModeClass::all().len()
    })
}

fn boundary_zero_support_class_passes(report: &EvalReport) -> Value {
    let metrics = report
        .per_failure_mode_class
        .get(&FailureModeClass::NameError)
        .expect("NameError metrics");
    json!({
        "case": "zero_support_class_vacuously_passes",
        "expected": {
            "precision": 1.0,
            "recall": 1.0,
            "passed": true
        },
        "actual": metrics,
        "pass": metrics.precision == 1.0 && metrics.recall == 1.0 && metrics.passed_threshold
    })
}

fn boundary_low_precision_only(report: &EvalReport) -> Value {
    let metrics = report
        .per_failure_mode_class
        .get(&FailureModeClass::WrongAlgorithm)
        .expect("WrongAlgorithm metrics");
    json!({
        "case": "low_precision_only",
        "expected": "low_precision",
        "actual": metrics,
        "pass": metrics.weakness.as_deref() == Some("low_precision")
    })
}

fn boundary_low_recall_only(report: &EvalReport) -> Value {
    let metrics = report
        .per_failure_mode_class
        .get(&FailureModeClass::CompileError)
        .expect("CompileError metrics");
    json!({
        "case": "low_recall_only",
        "expected": "low_recall",
        "actual": metrics,
        "pass": metrics.weakness.as_deref() == Some("low_recall")
    })
}

fn boundary_low_precision_and_recall(report: &EvalReport) -> Value {
    let metrics = report
        .per_failure_mode_class
        .get(&FailureModeClass::Timeout)
        .expect("Timeout metrics");
    json!({
        "case": "low_precision_and_recall",
        "expected": "low_precision_and_low_recall",
        "actual": metrics,
        "pass": metrics.weakness.as_deref() == Some("low_precision_and_low_recall")
    })
}

fn boundary_missing_class_rejected(report: &EvalReport) -> Value {
    let mut invalid = report.clone();
    invalid
        .per_failure_mode_class
        .remove(&FailureModeClass::NoTestsCollected);
    let err = invalid.validate().expect_err("missing class should fail");
    json!({
        "case": "missing_failure_mode_class_rejected",
        "expected": "MEJEPA_EVAL_INVALID_INPUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_INVALID_INPUT"
    })
}

fn file_artifact(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(json!({
        "path": path,
        "bytes": bytes.len(),
        "sha256": sha256_bytes(&bytes)
    }))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn git_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
