use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mcp::handlers::tools::mejepa_eval_tools::mejepa_ship_gate_status_payload;
use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::eval::{
    required_active_python_ship_gate_cells, ActiveLearningSummary, EvalProvenance, EvalReport,
    FingerprintClassificationMetrics, FingerprintShipGateWindow, MutationCategory,
    OpenResearchQuestionStatus, UnknownOodRecallMetrics, FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD,
    FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD, FINGERPRINT_SHIP_GATE_STABILITY_BLOCKER,
    FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD,
};
use context_graph_mejepa::{count_cf, open_infer_rocksdb, Language, RocksDbEvalStore};
use context_graph_mejepa_cf::{
    CF_MEJEPA_EVAL_REPORTS, CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS, CF_MEJEPA_MODEL_PROMOTIONS,
};
use rocksdb::WriteOptions;
use serde_json::{json, Value};

const DEFAULT_FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/phase-g-fingerprint-ship-gate-fsv";
const FP_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FP_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

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

    let main_db = output_root.join("fingerprint-stability-rocksdb");
    persist_eval_and_fingerprint_windows(
        &main_db,
        &[
            (
                report("2026-05-13", 1_000),
                passing_window("w1", "2026-05-13", 1_000),
            ),
            (
                report("2026-05-14", 2_000),
                passing_window("w2", "2026-05-14", 2_000),
            ),
            (
                report("2026-05-15", 3_000),
                passing_window("w3", "2026-05-15", 3_000),
            ),
        ],
    )?;
    let after_three = mejepa_ship_gate_status_payload(&main_db)?;

    persist_eval_and_fingerprint_windows(
        &main_db,
        &[(
            report("2026-05-16", 4_000),
            passing_window("w4", "2026-05-16", 4_000),
        )],
    )?;
    let after_four = mejepa_ship_gate_status_payload(&main_db)?;

    persist_eval_and_fingerprint_windows(
        &main_db,
        &[(
            report("2026-05-17", 5_000),
            failing_accuracy_precision_window("w5", "2026-05-17", 5_000),
        )],
    )?;
    let after_fingerprint_failure = mejepa_ship_gate_status_payload(&main_db)?;

    let missing_fp_db = output_root.join("missing-fingerprint-windows-rocksdb");
    persist_eval_reports(
        &missing_fp_db,
        &[
            report("2026-05-13", 1_000),
            report("2026-05-14", 2_000),
            report("2026-05-15", 3_000),
            report("2026-05-16", 4_000),
        ],
    )?;
    let missing_fingerprint_windows = mejepa_ship_gate_status_payload(&missing_fp_db)?;

    let unknown_recall_db = output_root.join("unknown-recall-rocksdb");
    persist_eval_and_fingerprint_windows(
        &unknown_recall_db,
        &[
            (
                report("2026-05-13", 1_000),
                passing_window("u1", "2026-05-13", 1_000),
            ),
            (
                report("2026-05-14", 2_000),
                passing_window("u2", "2026-05-14", 2_000),
            ),
            (
                report("2026-05-15", 3_000),
                passing_window("u3", "2026-05-15", 3_000),
            ),
            (
                report("2026-05-16", 4_000),
                failing_unknown_recall_window("u4", "2026-05-16", 4_000),
            ),
        ],
    )?;
    let after_unknown_recall_failure = mejepa_ship_gate_status_payload(&unknown_recall_db)?;

    let reset_db = output_root.join("promotion-reset-rocksdb");
    persist_eval_and_fingerprint_windows(
        &reset_db,
        &[
            (
                report("2026-05-13", 1_000),
                passing_window("r1", "2026-05-13", 1_000),
            ),
            (
                report("2026-05-14", 2_000),
                passing_window("r2", "2026-05-14", 2_000),
            ),
            (
                report("2026-05-15", 3_000),
                passing_window("r3", "2026-05-15", 3_000),
            ),
            (
                report("2026-05-16", 4_000),
                passing_window("r4", "2026-05-16", 4_000),
            ),
        ],
    )?;
    let before_promotion_reset = mejepa_ship_gate_status_payload(&reset_db)?;
    let promotion_key = persist_model_promotion_reset(&reset_db, 5_000)?;
    let after_promotion_reset = mejepa_ship_gate_status_payload(&reset_db)?;

    let main_counts = cf_counts(&main_db)?;
    let reset_counts = cf_counts(&reset_db)?;

    ensure!(after_three["shipGatePassed"] == false);
    ensure!(after_three["fingerprintShipGatePassed"] == false);
    ensure!(after_four["shipGatePassed"] == true);
    ensure!(after_four["fingerprintShipGatePassed"] == true);
    ensure!(after_fingerprint_failure["shipGatePassed"] == false);
    ensure!(after_fingerprint_failure["cellShipGatePassed"] == true);
    ensure!(after_fingerprint_failure["fingerprintShipGatePassed"] == false);
    ensure!(status_failures_contain(
        &after_fingerprint_failure,
        "accuracy"
    ));
    ensure!(status_failures_contain(
        &after_fingerprint_failure,
        "precision"
    ));
    ensure!(missing_fingerprint_windows["cellShipGatePassed"] == true);
    ensure!(missing_fingerprint_windows["fingerprintShipGatePassed"] == false);
    ensure!(status_failures_contain(
        &missing_fingerprint_windows,
        FINGERPRINT_SHIP_GATE_STABILITY_BLOCKER
    ));
    ensure!(after_unknown_recall_failure["cellShipGatePassed"] == true);
    ensure!(after_unknown_recall_failure["fingerprintShipGatePassed"] == false);
    ensure!(status_failures_contain(
        &after_unknown_recall_failure,
        "unknown_ood_recall"
    ));
    ensure!(before_promotion_reset["shipGatePassed"] == true);
    ensure!(after_promotion_reset["shipGatePassed"] == false);
    ensure!(after_promotion_reset["fingerprintShipGate"]["consecutive_passing_windows"] == 0);

    let assertions = json!({
        "three_windows_not_ready": after_three["fingerprintShipGate"]["consecutive_passing_windows"] == 3 && after_three["shipGatePassed"] == false,
        "fourth_window_ready": after_four["fingerprintShipGate"]["consecutive_passing_windows"] == 4 && after_four["shipGatePassed"] == true,
        "accuracy_precision_failure_resets": after_fingerprint_failure["fingerprintShipGate"]["consecutive_passing_windows"] == 0
            && status_failures_contain(&after_fingerprint_failure, "accuracy")
            && status_failures_contain(&after_fingerprint_failure, "precision"),
        "missing_fingerprint_windows_fail_closed": missing_fingerprint_windows["cellShipGatePassed"] == true
            && missing_fingerprint_windows["fingerprintShipGatePassed"] == false
            && status_failures_contain(&missing_fingerprint_windows, FINGERPRINT_SHIP_GATE_STABILITY_BLOCKER),
        "unknown_ood_recall_failure_resets": after_unknown_recall_failure["fingerprintShipGate"]["consecutive_passing_windows"] == 0
            && status_failures_contain(&after_unknown_recall_failure, "unknown_ood_recall"),
        "model_promotion_resets_fingerprint_gate": before_promotion_reset["fingerprintShipGatePassed"] == true
            && after_promotion_reset["fingerprintShipGatePassed"] == false
            && after_promotion_reset["fingerprintShipGate"]["model_promotion_reset_count"] == 1,
        "reopened_main_eval_report_rows": main_counts[CF_MEJEPA_EVAL_REPORTS] == 5,
        "reopened_main_fingerprint_window_rows": main_counts[CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS] == 5,
        "reopened_reset_model_promotion_rows": reset_counts[CF_MEJEPA_MODEL_PROMOTIONS] == 1,
    });
    ensure!(
        assertions
            .as_object()
            .unwrap()
            .values()
            .all(|value| value.as_bool() == Some(true)),
        "one or more assertions failed: {assertions}"
    );

    let report_path = output_root.join("fingerprint_ship_gate_fsv.json");
    let output = json!({
        "task_id": "TASK-FP-007",
        "fsv": "phase_g_fingerprint_ship_gate",
        "all_passed": true,
        "artifact_path": report_path,
        "build_release_sha": git_sha(),
        "source_of_truth": {
            "eval_reports": CF_MEJEPA_EVAL_REPORTS,
            "fingerprint_ship_gate_windows": CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
            "model_promotions": CF_MEJEPA_MODEL_PROMOTIONS,
            "mcp_response_builder": "context_graph_mcp::handlers::tools::mejepa_ship_gate_status_payload"
        },
        "promotion_reset_key": promotion_key,
        "after_three": after_three,
        "after_four": after_four,
        "after_fingerprint_failure": after_fingerprint_failure,
        "missing_fingerprint_windows": missing_fingerprint_windows,
        "after_unknown_recall_failure": after_unknown_recall_failure,
        "before_promotion_reset": before_promotion_reset,
        "after_promotion_reset": after_promotion_reset,
        "main_counts": main_counts,
        "reset_counts": reset_counts,
        "assertions": assertions,
    });
    write_json_0600(&report_path, &output)?;
    Ok(output)
}

fn persist_eval_and_fingerprint_windows(
    db_path: &Path,
    pairs: &[(EvalReport, FingerprintShipGateWindow)],
) -> Result<()> {
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB")?;
    let store = RocksDbEvalStore::new(db.clone()).context("construct eval store")?;
    for (report, window) in pairs {
        store
            .persist_report(report)
            .context("persist eval report")?;
        store
            .persist_fingerprint_ship_gate_window(window)
            .context("persist fingerprint ship-gate window")?;
    }
    drop(store);
    drop(db);
    Ok(())
}

fn persist_eval_reports(db_path: &Path, reports: &[EvalReport]) -> Result<()> {
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB")?;
    let store = RocksDbEvalStore::new(db.clone()).context("construct eval store")?;
    for report in reports {
        store
            .persist_report(report)
            .context("persist eval report")?;
    }
    drop(store);
    drop(db);
    Ok(())
}

fn persist_model_promotion_reset(db_path: &Path, timestamp_millis: i64) -> Result<String> {
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB for promotion reset")?;
    let cf = db
        .cf_handle(CF_MEJEPA_MODEL_PROMOTIONS)
        .context("missing CF_MEJEPA_MODEL_PROMOTIONS")?;
    let key = format!("phase_e/per-cell-promotion/{timestamp_millis:020}");
    let value = serde_json::to_vec(&json!({
        "accepted": true,
        "recorded_at_unix_ms": timestamp_millis,
        "source_of_truth_cf": CF_MEJEPA_MODEL_PROMOTIONS
    }))?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key.as_bytes(), &value, &opts)?;
    let readback = db
        .get_cf(cf, key.as_bytes())?
        .context("missing promotion reset readback")?;
    ensure!(readback == value, "promotion reset readback mismatch");
    drop(db);
    Ok(key)
}

fn cf_counts(db_path: &Path) -> Result<Value> {
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB for counts")?;
    Ok(json!({
        CF_MEJEPA_EVAL_REPORTS: count_cf(&db, CF_MEJEPA_EVAL_REPORTS)?,
        CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS: count_cf(&db, CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS)?,
        CF_MEJEPA_MODEL_PROMOTIONS: count_cf(&db, CF_MEJEPA_MODEL_PROMOTIONS)?,
    }))
}

fn passing_window(
    window_id: &str,
    report_date: &str,
    timestamp_millis: i64,
) -> FingerprintShipGateWindow {
    FingerprintShipGateWindow {
        window_id: window_id.to_string(),
        report_date: report_date.to_string(),
        generated_at_unix_ms: timestamp_millis,
        per_fingerprint: BTreeMap::from([
            (FP_A.to_string(), passing_fingerprint_metrics(FP_A)),
            (FP_B.to_string(), passing_fingerprint_metrics(FP_B)),
        ]),
        unknown_ood_recall: passing_unknown_recall(),
        passed_window: true,
        failures: Vec::new(),
    }
}

fn failing_accuracy_precision_window(
    window_id: &str,
    report_date: &str,
    timestamp_millis: i64,
) -> FingerprintShipGateWindow {
    FingerprintShipGateWindow {
        window_id: window_id.to_string(),
        report_date: report_date.to_string(),
        generated_at_unix_ms: timestamp_millis,
        per_fingerprint: BTreeMap::from([
            (FP_A.to_string(), failing_accuracy_metrics(FP_A)),
            (FP_B.to_string(), failing_precision_metrics(FP_B)),
        ]),
        unknown_ood_recall: passing_unknown_recall(),
        passed_window: false,
        failures: vec![
            format!("fingerprint {FP_A} accuracy 0.900000 < 0.950000"),
            format!("fingerprint {FP_B} precision 0.947368 < 0.950000"),
        ],
    }
}

fn failing_unknown_recall_window(
    window_id: &str,
    report_date: &str,
    timestamp_millis: i64,
) -> FingerprintShipGateWindow {
    FingerprintShipGateWindow {
        window_id: window_id.to_string(),
        report_date: report_date.to_string(),
        generated_at_unix_ms: timestamp_millis,
        per_fingerprint: BTreeMap::from([
            (FP_A.to_string(), passing_fingerprint_metrics(FP_A)),
            (FP_B.to_string(), passing_fingerprint_metrics(FP_B)),
        ]),
        unknown_ood_recall: UnknownOodRecallMetrics {
            actual_unknown_count: 10,
            detected_unknown_count: 8,
            missed_unknown_count: 2,
            recall: 0.8,
            recall_threshold: FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD,
            passed_threshold: false,
        },
        passed_window: false,
        failures: vec!["unknown_ood_recall 0.800000 < 0.900000".to_string()],
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

fn failing_accuracy_metrics(fingerprint_id: &str) -> FingerprintClassificationMetrics {
    FingerprintClassificationMetrics {
        fingerprint_id: fingerprint_id.to_string(),
        sample_count: 20,
        true_positive_count: 10,
        true_negative_count: 8,
        false_positive_count: 0,
        false_negative_count: 2,
        accuracy: 0.90,
        precision: 1.0,
        accuracy_threshold: FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD,
        precision_threshold: FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD,
        passed_threshold: false,
    }
}

fn failing_precision_metrics(fingerprint_id: &str) -> FingerprintClassificationMetrics {
    FingerprintClassificationMetrics {
        fingerprint_id: fingerprint_id.to_string(),
        sample_count: 20,
        true_positive_count: 18,
        true_negative_count: 1,
        false_positive_count: 1,
        false_negative_count: 0,
        accuracy: 0.95,
        precision: 18.0_f32 / 19.0_f32,
        accuracy_threshold: FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD,
        precision_threshold: FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD,
        passed_threshold: false,
    }
}

fn passing_unknown_recall() -> UnknownOodRecallMetrics {
    UnknownOodRecallMetrics {
        actual_unknown_count: 10,
        detected_unknown_count: 9,
        missed_unknown_count: 1,
        recall: 0.9,
        recall_threshold: FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD,
        passed_threshold: true,
    }
}

fn report(date: &str, timestamp_millis: i64) -> EvalReport {
    let mut per_cell = BTreeMap::new();
    for (_category, _language, cell) in required_active_python_ship_gate_cells() {
        per_cell.insert(cell, Some(0.96));
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
            .map(|category| (category, Some(0.96)))
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
            id: "TASK-FP-007".to_string(),
            question: "Does the ship gate require per-fingerprint metrics across four windows?"
                .to_string(),
            status: "fsv".to_string(),
        }],
        q1_pass_rate: 1.0,
        q2_report_correlation: Some(0.96),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: true,
        ship_gate_failures: Vec::new(),
        provenance: EvalProvenance {
            corpus_sha: "phase-g-fingerprint-ship-gate-fsv".to_string(),
            eval_code_version: "task-fp-007-fsv".to_string(),
            calibration_version: "task-fp-007-fsv".to_string(),
            generated_by: "phase_g_fingerprint_ship_gate_fsv".to_string(),
        },
        wall_clock_seconds: 0.01,
    }
}

fn status_failures_contain(status: &Value, needle: &str) -> bool {
    status["shipGateFailures"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str().unwrap_or("").contains(needle))
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
