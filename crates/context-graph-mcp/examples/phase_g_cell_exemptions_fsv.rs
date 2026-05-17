use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mcp::handlers::tools::mejepa_eval_tools::mejepa_ship_gate_status_payload_with_exemptions;
use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::eval::{
    required_active_python_ship_gate_cells, ActiveLearningSummary, EvalProvenance, EvalReport,
    FingerprintClassificationMetrics, FingerprintShipGateWindow, MutationCategory,
    OpenResearchQuestionStatus, UnknownOodRecallMetrics, FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD,
    FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD, FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD,
};
use context_graph_mejepa::{count_cf, open_infer_rocksdb, Language, RocksDbEvalStore};
use context_graph_mejepa_cf::{CF_MEJEPA_EVAL_REPORTS, CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS};
use serde_json::{json, Value};

const DEFAULT_FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-cell-exemptions-fsv";
const EXEMPT_CELL: &str = "compile_error::python";

fn main() -> Result<()> {
    let output_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_FSV_ROOT));
    let report = run(&output_root)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run(output_root: &Path) -> Result<Value> {
    if output_root.exists() {
        fs::remove_dir_all(output_root)
            .with_context(|| format!("remove {}", output_root.display()))?;
    }
    fs::create_dir_all(output_root).with_context(|| format!("create {}", output_root.display()))?;

    let active_config = write_active_exemption_config(output_root)?;
    let empty_config = write_empty_exemption_config(output_root)?;
    let expired_config = write_expired_exemption_config(output_root)?;

    let active_db = output_root.join("active-exemption-rocksdb");
    persist_reports(&active_db, &low_exempt_cell_reports())?;
    let active_status =
        mejepa_ship_gate_status_payload_with_exemptions(&active_db, &active_config)?;

    let empty_db = output_root.join("empty-exemption-rocksdb");
    persist_reports(&empty_db, &low_exempt_cell_reports())?;
    let empty_status = mejepa_ship_gate_status_payload_with_exemptions(&empty_db, &empty_config)?;

    let expired_db = output_root.join("expired-exemption-rocksdb");
    persist_reports(&expired_db, &low_exempt_cell_reports())?;
    let expired_status =
        mejepa_ship_gate_status_payload_with_exemptions(&expired_db, &expired_config)?;

    let active_counts = cf_counts(&active_db)?;
    ensure!(active_status["shipGatePassed"] == true);
    ensure!(active_status["latestWindowShipGatePassed"] == true);
    ensure!(active_status["latestWindowRawShipGatePassed"] == false);
    ensure!(active_status["consecutivePassingWindows"] == 4);
    ensure!(cell_correlation_close(&active_status, EXEMPT_CELL, 0.10));
    ensure!(active_status["cellExemptions"][EXEMPT_CELL]["reason"].is_string());
    ensure!(!status_failures_contain_cell(&active_status, EXEMPT_CELL));

    ensure!(empty_status["shipGatePassed"] == false);
    ensure!(empty_status["consecutivePassingWindows"] == 0);
    ensure!(status_failures_contain_cell(&empty_status, EXEMPT_CELL));

    ensure!(expired_status["shipGatePassed"] == false);
    ensure!(expired_status["cellExemptions"]
        .as_object()
        .unwrap()
        .is_empty());
    ensure!(status_failures_contain_cell(&expired_status, EXEMPT_CELL));

    let assertions = json!({
        "active_exempt_low_cell_does_not_block": active_status["shipGatePassed"] == true,
        "active_exempt_cell_still_reports_correlation": cell_correlation_close(&active_status, EXEMPT_CELL, 0.10),
        "active_exemption_reason_reported": active_status["cellExemptions"][EXEMPT_CELL]["reason"].as_str().unwrap_or("").contains("impossible"),
        "active_exempt_failure_filtered": !status_failures_contain_cell(&active_status, EXEMPT_CELL),
        "empty_config_low_cell_blocks": empty_status["shipGatePassed"] == false && status_failures_contain_cell(&empty_status, EXEMPT_CELL),
        "expired_exemption_low_cell_blocks": expired_status["shipGatePassed"] == false && status_failures_contain_cell(&expired_status, EXEMPT_CELL),
        "active_report_rows_reopened": active_counts[CF_MEJEPA_EVAL_REPORTS] == 4,
        "active_fingerprint_window_rows_reopened": active_counts[CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS] == 4,
    });
    ensure!(
        assertions
            .as_object()
            .unwrap()
            .values()
            .all(|value| value.as_bool() == Some(true)),
        "one or more assertions failed: {assertions}"
    );

    let report_path = output_root.join("cell_exemptions_fsv.json");
    let output = json!({
        "task_id": "TASK-G-007",
        "fsv": "phase_g_cell_exemptions",
        "all_passed": true,
        "artifact_path": report_path,
        "build_release_sha": git_sha(),
        "source_of_truth": {
            "active_config": active_config,
            "empty_config": empty_config,
            "expired_config": expired_config,
            "eval_reports": CF_MEJEPA_EVAL_REPORTS,
            "fingerprint_ship_gate_windows": CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
            "mcp_response_builder": "context_graph_mcp::handlers::tools::mejepa_ship_gate_status_payload_with_exemptions"
        },
        "active_status": active_status,
        "empty_status": empty_status,
        "expired_status": expired_status,
        "active_counts": active_counts,
        "assertions": assertions,
    });
    write_json_0600(&report_path, &output)?;
    Ok(output)
}

fn write_active_exemption_config(output_root: &Path) -> Result<PathBuf> {
    let path = output_root.join("active_cell_exemptions.toml");
    fs::write(
        &path,
        r#"
[[exemptions]]
cell = "compile_error::python"
reason = "TASK-G-007 synthetic impossible cell approved for FSV"
operator_id = "operator"
"#,
    )?;
    Ok(path)
}

fn write_empty_exemption_config(output_root: &Path) -> Result<PathBuf> {
    let path = output_root.join("empty_cell_exemptions.toml");
    fs::write(&path, "exemptions = []\n")?;
    Ok(path)
}

fn write_expired_exemption_config(output_root: &Path) -> Result<PathBuf> {
    let path = output_root.join("expired_cell_exemptions.toml");
    fs::write(
        &path,
        r#"
[[exemptions]]
cell = "compile_error::python"
reason = "TASK-G-007 expired exemption must not apply"
operator_id = "operator"
expires_unix_ms = 1
"#,
    )?;
    Ok(path)
}

fn low_exempt_cell_reports() -> Vec<EvalReport> {
    (0..4)
        .map(|idx| {
            report(
                &format!("2026-05-{}", 13 + idx),
                1_000 + i64::from(idx) * 1_000,
            )
        })
        .collect()
}

fn persist_reports(db_path: &Path, reports: &[EvalReport]) -> Result<()> {
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB")?;
    let store = RocksDbEvalStore::new(db.clone()).context("construct eval store")?;
    for report in reports {
        store
            .persist_report(report)
            .context("persist eval report")?;
        store
            .persist_fingerprint_ship_gate_window(&passing_fingerprint_window_for_report(report))
            .context("persist fingerprint ship-gate window")?;
    }
    drop(store);
    drop(db);
    Ok(())
}

fn cf_counts(db_path: &Path) -> Result<Value> {
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB for counts")?;
    Ok(json!({
        CF_MEJEPA_EVAL_REPORTS: count_cf(&db, CF_MEJEPA_EVAL_REPORTS)?,
        CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS: count_cf(&db, CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS)?,
    }))
}

const FP_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FP_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn passing_fingerprint_window_for_report(report: &EvalReport) -> FingerprintShipGateWindow {
    FingerprintShipGateWindow {
        window_id: format!("{}-fingerprint-window", report.report_date),
        report_date: report.report_date.clone(),
        generated_at_unix_ms: report.generated_at_unix_ms,
        per_fingerprint: BTreeMap::from([
            (FP_A.to_string(), passing_fingerprint_metrics(FP_A)),
            (FP_B.to_string(), passing_fingerprint_metrics(FP_B)),
        ]),
        unknown_ood_recall: UnknownOodRecallMetrics {
            actual_unknown_count: 10,
            detected_unknown_count: 9,
            missed_unknown_count: 1,
            recall: 0.9,
            recall_threshold: FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD,
            passed_threshold: true,
        },
        passed_window: true,
        failures: Vec::new(),
    }
}

fn passing_fingerprint_metrics(fingerprint_id: &str) -> FingerprintClassificationMetrics {
    FingerprintClassificationMetrics {
        fingerprint_id: fingerprint_id.to_string(),
        sample_count: 20,
        true_positive_count: 18,
        true_negative_count: 2,
        false_positive_count: 0,
        false_negative_count: 0,
        accuracy: 1.0,
        precision: 1.0,
        accuracy_threshold: FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD,
        precision_threshold: FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD,
        passed_threshold: true,
    }
}

fn status_failures_contain_cell(status: &Value, cell: &str) -> bool {
    status["shipGateFailures"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str().unwrap_or("").contains(cell))
}

fn cell_correlation_close(status: &Value, cell: &str, expected: f64) -> bool {
    status["perCellCorrelation"][cell]
        .as_f64()
        .map(|actual| (actual - expected).abs() < 0.000_001)
        .unwrap_or(false)
}

fn report(date: &str, timestamp_millis: i64) -> EvalReport {
    let mut per_cell = BTreeMap::new();
    for (_category, _language, cell) in required_active_python_ship_gate_cells() {
        let correlation = if cell == EXEMPT_CELL { 0.10 } else { 0.96 };
        per_cell.insert(cell, Some(correlation));
    }
    let per_cell_convergence_eta =
        context_graph_mejepa::baseline_convergence_eta_for_cells(&per_cell, 0.95);
    EvalReport {
        report_date: date.to_string(),
        generated_at_unix_ms: timestamp_millis,
        rolling_window_size: 100,
        holdout_count: 100,
        overall_correlation: Some(0.96),
        per_category_correlation: MutationCategory::all()
            .into_iter()
            .map(|category| {
                (
                    category,
                    Some(if category == MutationCategory::CompileError {
                        0.10
                    } else {
                        0.96
                    }),
                )
            })
            .collect(),
        per_language_correlation: BTreeMap::from([(Language::Python, Some(0.96))]),
        per_cell_correlation: per_cell,
        cell_exemptions: BTreeMap::new(),
        bayesian_shrinkage: BTreeMap::new(),
        conformal_coverage_health: BTreeMap::new(),
        ood_calibration_health: BTreeMap::new(),
        gtau_pass_rate: BTreeMap::new(),
        per_prediction_class_calibration: BTreeMap::new(),
        per_failure_mode_class: context_graph_mejepa::empty_failure_mode_class_metrics(
            100,
            &context_graph_mejepa::EvalConfig::default(),
        ),
        per_cell_convergence_eta,
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
        open_research_questions: vec![OpenResearchQuestionStatus {
            id: "TASK-G-007".to_string(),
            question: "Do exempt impossible cells stay visible without blocking the ship gate?"
                .to_string(),
            status: "fsv".to_string(),
        }],
        q1_pass_rate: 1.0,
        q2_report_correlation: Some(0.96),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: false,
        ship_gate_failures: vec![
            "per_cell_correlation compile_error::python 0.100000 < 0.950000".to_string(),
        ],
        provenance: EvalProvenance {
            corpus_sha: "phase-g-cell-exemptions-fsv".to_string(),
            eval_code_version: "task-g-007-fsv".to_string(),
            calibration_version: "task-g-007-fsv".to_string(),
            generated_by: "phase_g_cell_exemptions_fsv".to_string(),
        },
        wall_clock_seconds: 0.01,
    }
}

fn git_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}
