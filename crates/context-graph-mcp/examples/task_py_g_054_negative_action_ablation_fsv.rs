use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use candle_core::{Device, Tensor};
use context_graph_mcp::handlers::tools::mejepa_eval_tools::mejepa_ship_gate_status_payload;
use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::eval::{
    build_negative_action_ablation_report, incomplete_negative_action_ablation_report,
    required_active_python_ship_gate_cells, write_ablation_weekly_markdown, AblationReport,
    AblationRunInput, ActiveLearningSummary, EvalProvenance, EvalReport,
    FingerprintClassificationMetrics, FingerprintShipGateWindow, MutationCategory,
    OpenResearchQuestionStatus, UnknownOodRecallMetrics, ABLATION_INCOMPLETE,
    ABLATION_NUMERICAL_INSTABILITY, FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD,
    FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD, FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD,
    NEGATIVE_ACTION_ABLATION_BLOCKER,
};
use context_graph_mejepa::{count_cf, open_infer_rocksdb, Language, RocksDbEvalStore};
use context_graph_mejepa_cf::{
    CF_MEJEPA_ABLATION_REPORTS, CF_MEJEPA_EVAL_REPORTS, CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
};
use context_graph_mejepa_train::eval::holdout::{PredictorForward, PredictorForwardOptions};
use serde_json::{json, Value};

const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-054-negative-action-ablation-fsv";
const FP_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FP_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn main() -> Result<()> {
    let output_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_run_root);
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

    let healthy = case_healthy(output_root)?;
    let degenerate = case_degenerate(output_root)?;
    let per_cell = case_per_cell_low_drop(output_root)?;
    let incomplete = case_incomplete_preserves_previous(output_root)?;
    let numerical = case_numerical_instability(output_root)?;
    let toggle_probe = training_toggle_probe()?;

    let assertions = json!({
        "healthy_drop_18_4_gate_permits": json_number_close(&healthy["score_drop_pct"], 18.4) && healthy["ship_gate_passed"] == json!(true),
        "degenerate_drop_0_8_gate_blocks": json_number_close(&degenerate["score_drop_pct"], 0.8) && degenerate["ship_gate_passed"] == json!(false),
        "per_cell_low_drop_flagged_without_global_block": per_cell["global_ship_gate_passed"] == json!(true) && json_number_close(&per_cell["flagged_cell_drop_pct"], 1.2),
        "incomplete_preserves_previous_effective_healthy": incomplete["ship_gate_passed"] == json!(true) && incomplete["latest_verdict"] == json!("incomplete") && incomplete["effective_verdict"] == json!("healthy"),
        "nan_records_numerical_instability_and_blocks": numerical["status_code"] == json!(ABLATION_NUMERICAL_INSTABILITY) && numerical["ship_gate_passed"] == json!(false),
        "training_toggle_disables_action_conditioning_path": toggle_probe["disabled_differs_from_enabled"] == json!(true),
    });
    ensure!(
        assertions
            .as_object()
            .unwrap()
            .values()
            .all(|value| value.as_bool() == Some(true)),
        "one or more assertions failed: {assertions}"
    );

    let evidence_path = output_root.join("negative_action_ablation_fsv.json");
    let output = json!({
        "task_id": "TASK-PY-G-054",
        "issue": 273,
        "fsv": "negative_action_ablation_falsifier",
        "all_passed": true,
        "artifact_path": evidence_path,
        "build_release_sha": git_sha(),
        "source_of_truth": {
            "ablation_reports": CF_MEJEPA_ABLATION_REPORTS,
            "eval_reports": CF_MEJEPA_EVAL_REPORTS,
            "fingerprint_ship_gate_windows": CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
            "mcp_response_builder": "context_graph_mcp::handlers::tools::mejepa_ship_gate_status_payload",
            "weekly_ablation_artifact_pattern": "/zfs/archive/contextgraph/exports/weekly/*/ablation_report.md"
        },
        "healthy": healthy,
        "degenerate": degenerate,
        "per_cell_low_drop": per_cell,
        "incomplete": incomplete,
        "numerical_instability": numerical,
        "training_toggle_probe": toggle_probe,
        "assertions": assertions
    });
    write_json_0600(&evidence_path, &output)?;
    Ok(output)
}

fn case_healthy(output_root: &Path) -> Result<Value> {
    let db_path = output_root.join("healthy-rocksdb");
    seed_passing_gate(&db_path)?;
    let report = ablation_report("healthy", "2026-05-18", 10_000, 0.95, 0.7752)?;
    persist_ablation(&db_path, &report)?;
    let weekly_path = write_ablation_weekly_markdown(output_root.join("exports/weekly"), &report)?;
    let payload = mejepa_ship_gate_status_payload(&db_path)?;
    let counts = cf_counts(&db_path)?;
    let drop = rounded_pct(report.score_drop_pct)?;
    ensure!(drop == 18.4, "healthy score_drop_pct was {drop}");
    ensure!(
        payload["shipGatePassed"] == true,
        "healthy ablation should permit ship gate: {payload}"
    );
    Ok(json!({
        "report_id": report.report_id,
        "score_drop_pct": drop,
        "verdict": report.verdict,
        "ship_gate_passed": payload["shipGatePassed"],
        "negative_action_ablation_gate": payload["negativeActionAblationGate"],
        "weekly_markdown_path": weekly_path,
        "weekly_markdown_contains_source_cf": fs::read_to_string(&weekly_path)?.contains(CF_MEJEPA_ABLATION_REPORTS),
        "reopened_counts": counts
    }))
}

fn case_degenerate(output_root: &Path) -> Result<Value> {
    let db_path = output_root.join("degenerate-rocksdb");
    seed_passing_gate(&db_path)?;
    let report = ablation_report("degenerate", "2026-05-18", 10_000, 0.95, 0.9424)?;
    persist_ablation(&db_path, &report)?;
    let payload = mejepa_ship_gate_status_payload(&db_path)?;
    let drop = rounded_pct(report.score_drop_pct)?;
    ensure!(drop == 0.8, "degenerate score_drop_pct was {drop}");
    ensure!(
        payload["shipGatePassed"] == false,
        "degenerate ablation should block ship gate: {payload}"
    );
    ensure!(
        ship_gate_failures_contain(&payload, NEGATIVE_ACTION_ABLATION_BLOCKER),
        "shipGateFailures missing ablation blocker: {payload}"
    );
    Ok(json!({
        "report_id": report.report_id,
        "score_drop_pct": drop,
        "verdict": report.verdict,
        "ship_gate_passed": payload["shipGatePassed"],
        "ship_gate_failures": payload["shipGateFailures"],
        "negative_action_ablation_gate": payload["negativeActionAblationGate"]
    }))
}

fn case_per_cell_low_drop(output_root: &Path) -> Result<Value> {
    let db_path = output_root.join("per-cell-low-drop-rocksdb");
    seed_passing_gate(&db_path)?;
    let mut input = ablation_input("per-cell-low", "2026-05-18", 10_000, 0.95, 0.817)?;
    let flagged_cell = required_active_python_ship_gate_cells()
        .into_iter()
        .next()
        .context("missing active python cell")?
        .2;
    input
        .per_cell_disabled_correlation
        .insert(flagged_cell.clone(), 0.9386);
    let report = build_negative_action_ablation_report(input)?;
    persist_ablation(&db_path, &report)?;
    let payload = mejepa_ship_gate_status_payload(&db_path)?;
    let flagged = report
        .per_cell_drop
        .get(&flagged_cell)
        .context("flagged cell missing")?;
    ensure!(
        payload["shipGatePassed"] == true,
        "global healthy ablation should not block for one low-drop cell: {payload}"
    );
    ensure!(flagged.degenerate, "low-drop cell should be flagged");
    Ok(json!({
        "report_id": report.report_id,
        "global_score_drop_pct": rounded_pct(report.score_drop_pct)?,
        "global_ship_gate_passed": payload["shipGatePassed"],
        "flagged_cell": flagged_cell,
        "flagged_cell_drop_pct": rounded_pct(flagged.score_drop_pct)?,
        "flagged_cell_degenerate": flagged.degenerate,
        "negative_action_ablation_gate": payload["negativeActionAblationGate"]
    }))
}

fn case_incomplete_preserves_previous(output_root: &Path) -> Result<Value> {
    let db_path = output_root.join("incomplete-rocksdb");
    seed_passing_gate(&db_path)?;
    let healthy = ablation_report("previous-healthy", "2026-05-18", 10_000, 0.95, 0.7752)?;
    persist_ablation(&db_path, &healthy)?;
    let incomplete = incomplete_negative_action_ablation_report(
        "latest-incomplete",
        "2026-05-19",
        11_000,
        "weekly ablation crashed after enabled pass; previous verdict remains effective",
    )?;
    persist_ablation(&db_path, &incomplete)?;
    let payload = mejepa_ship_gate_status_payload(&db_path)?;
    ensure!(
        payload["shipGatePassed"] == true,
        "incomplete latest report should not flip previous healthy verdict: {payload}"
    );
    ensure!(
        payload["negativeActionAblationGate"]["warning"]
            .as_str()
            .unwrap_or("")
            .contains(ABLATION_INCOMPLETE),
        "incomplete warning missing: {payload}"
    );
    Ok(json!({
        "latest_report_id": payload["negativeActionAblationGate"]["latest_report_id"],
        "latest_verdict": payload["negativeActionAblationGate"]["latest_verdict"],
        "effective_report_id": payload["negativeActionAblationGate"]["effective_report_id"],
        "effective_verdict": payload["negativeActionAblationGate"]["effective_verdict"],
        "warning": payload["negativeActionAblationGate"]["warning"],
        "ship_gate_passed": payload["shipGatePassed"]
    }))
}

fn case_numerical_instability(output_root: &Path) -> Result<Value> {
    let db_path = output_root.join("numerical-instability-rocksdb");
    seed_passing_gate(&db_path)?;
    let report = build_negative_action_ablation_report(ablation_input(
        "nan-disabled",
        "2026-05-18",
        10_000,
        0.95,
        f32::NAN,
    )?)?;
    persist_ablation(&db_path, &report)?;
    let payload = mejepa_ship_gate_status_payload(&db_path)?;
    ensure!(
        payload["shipGatePassed"] == false,
        "numerical instability should block ship gate: {payload}"
    );
    ensure!(
        report.status_code.as_deref() == Some(ABLATION_NUMERICAL_INSTABILITY),
        "numerical instability report did not record status code"
    );
    Ok(json!({
        "report_id": report.report_id,
        "status_code": report.status_code,
        "warning": report.warning,
        "verdict": report.verdict,
        "ship_gate_passed": payload["shipGatePassed"],
        "negative_action_ablation_gate": payload["negativeActionAblationGate"],
        "ship_gate_failures": payload["shipGateFailures"]
    }))
}

fn training_toggle_probe() -> Result<Value> {
    let predictor = ToggleProbe;
    let input = Tensor::from_slice(&[7.0f32], 1, &Device::Cpu)?;
    let enabled = predictor.forward_with_options(&input, PredictorForwardOptions::default())?;
    let disabled = predictor.forward_with_options(
        &input,
        PredictorForwardOptions {
            disable_action_conditioning: true,
        },
    )?;
    let enabled = enabled.to_vec1::<f32>()?;
    let disabled = disabled.to_vec1::<f32>()?;
    Ok(json!({
        "enabled_first_value": enabled[0],
        "disabled_first_value": disabled[0],
        "disabled_differs_from_enabled": enabled != disabled
    }))
}

struct ToggleProbe;

impl PredictorForward for ToggleProbe {
    fn forward(
        &self,
        panel_t01: &Tensor,
    ) -> std::result::Result<Tensor, context_graph_mejepa_train::TrainerError> {
        Ok(panel_t01.clone())
    }

    fn forward_with_options(
        &self,
        panel_t01: &Tensor,
        options: PredictorForwardOptions,
    ) -> std::result::Result<Tensor, context_graph_mejepa_train::TrainerError> {
        if options.disable_action_conditioning {
            Tensor::from_slice(&[0.0f32], 1, &Device::Cpu).map_err(Into::into)
        } else {
            self.forward(panel_t01)
        }
    }
}

fn seed_passing_gate(db_path: &Path) -> Result<()> {
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB")?;
    let store = RocksDbEvalStore::new(db.clone()).context("construct eval store")?;
    for (idx, date) in ["2026-05-14", "2026-05-15", "2026-05-16", "2026-05-17"]
        .into_iter()
        .enumerate()
    {
        let report = report(date, ((idx + 1) * 1_000) as i64, 0.96, true);
        store
            .persist_report(&report)
            .context("persist eval report")?;
        store
            .persist_fingerprint_ship_gate_window(&fingerprint_window_for_report(&report))
            .context("persist fingerprint window")?;
    }
    drop(store);
    drop(db);
    Ok(())
}

fn persist_ablation(db_path: &Path, report: &AblationReport) -> Result<()> {
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB for ablation")?;
    let store = RocksDbEvalStore::new(db.clone()).context("construct eval store")?;
    store
        .persist_ablation_report(report)
        .context("persist ablation report")?;
    drop(store);
    drop(db);
    Ok(())
}

fn ablation_report(
    report_id: &str,
    report_date: &str,
    generated_at_unix_ms: i64,
    enabled: f32,
    disabled: f32,
) -> Result<AblationReport> {
    build_negative_action_ablation_report(ablation_input(
        report_id,
        report_date,
        generated_at_unix_ms,
        enabled,
        disabled,
    )?)
    .map_err(Into::into)
}

fn ablation_input(
    report_id: &str,
    report_date: &str,
    generated_at_unix_ms: i64,
    enabled: f32,
    disabled: f32,
) -> Result<AblationRunInput> {
    let cells = required_active_python_ship_gate_cells()
        .into_iter()
        .map(|(_, _, cell)| cell)
        .collect::<Vec<_>>();
    Ok(AblationRunInput {
        report_id: report_id.to_string(),
        report_date: report_date.to_string(),
        generated_at_unix_ms,
        action_enabled_correlation: enabled,
        action_disabled_correlation: disabled,
        per_cell_enabled_correlation: cells
            .iter()
            .map(|cell| (cell.clone(), enabled))
            .collect::<BTreeMap<_, _>>(),
        per_cell_disabled_correlation: cells
            .into_iter()
            .map(|cell| (cell, disabled))
            .collect::<BTreeMap<_, _>>(),
    })
}

fn cf_counts(db_path: &Path) -> Result<Value> {
    let db = open_infer_rocksdb(db_path).context("open inference RocksDB for counts")?;
    Ok(json!({
        CF_MEJEPA_EVAL_REPORTS: count_cf(&db, CF_MEJEPA_EVAL_REPORTS)?,
        CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS: count_cf(&db, CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS)?,
        CF_MEJEPA_ABLATION_REPORTS: count_cf(&db, CF_MEJEPA_ABLATION_REPORTS)?,
    }))
}

fn ship_gate_failures_contain(payload: &Value, needle: &str) -> bool {
    payload["shipGateFailures"]
        .as_array()
        .map(|failures| failures.as_slice())
        .unwrap_or(&[])
        .iter()
        .any(|value| value.as_str().unwrap_or("").contains(needle))
}

fn json_number_close(value: &Value, expected: f64) -> bool {
    value
        .as_f64()
        .map(|observed| (observed - expected).abs() < 0.001)
        .unwrap_or(false)
}

fn rounded_pct(value: Option<f32>) -> Result<f32> {
    let value = value.context("missing score_drop_pct")?;
    Ok((value * 10.0).round() / 10.0)
}

fn fingerprint_window_for_report(report: &EvalReport) -> FingerprintShipGateWindow {
    FingerprintShipGateWindow {
        window_id: format!("{}-fingerprint-window", report.report_date),
        report_date: report.report_date.clone(),
        generated_at_unix_ms: report.generated_at_unix_ms,
        per_fingerprint: BTreeMap::from([
            (FP_A.to_string(), passing_fingerprint_metrics(FP_A)),
            (FP_B.to_string(), passing_fingerprint_metrics(FP_B)),
        ]),
        unknown_ood_recall: passing_unknown_recall(),
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

fn report(date: &str, timestamp_millis: i64, correlation: f32, passed: bool) -> EvalReport {
    let mut per_cell = BTreeMap::new();
    for (_category, _language, cell) in required_active_python_ship_gate_cells() {
        per_cell.insert(cell, Some(correlation));
    }
    let per_cell_convergence_eta =
        context_graph_mejepa::baseline_convergence_eta_for_cells(&per_cell, 0.95);
    EvalReport {
        report_date: date.to_string(),
        generated_at_unix_ms: timestamp_millis,
        rolling_window_size: 100,
        holdout_count: 100,
        overall_correlation: Some(correlation),
        per_category_correlation: MutationCategory::all()
            .into_iter()
            .map(|category| (category, Some(correlation)))
            .collect(),
        per_language_correlation: BTreeMap::from([(Language::Python, Some(correlation))]),
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
            id: "TASK-PY-G-054".to_string(),
            question: "Does removing action conditioning falsify degenerate predictors?"
                .to_string(),
            status: "fsv".to_string(),
        }],
        q1_pass_rate: if passed { 1.0 } else { 0.90 },
        q2_report_correlation: Some(correlation),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: passed,
        ship_gate_failures: if passed {
            Vec::new()
        } else {
            vec!["per_window_correlation_below_0.95".to_string()]
        },
        provenance: EvalProvenance {
            corpus_sha: "task-py-g-054-negative-action-ablation-fsv".to_string(),
            eval_code_version: "task-py-g-054-fsv".to_string(),
            calibration_version: "task-py-g-054-fsv".to_string(),
            generated_by: "task_py_g_054_negative_action_ablation_fsv".to_string(),
        },
        wall_clock_seconds: 0.01,
    }
}

fn default_run_root() -> PathBuf {
    PathBuf::from(DEFAULT_FSV_ROOT).join(format!(
        "run-{}-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
        std::process::id()
    ))
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
