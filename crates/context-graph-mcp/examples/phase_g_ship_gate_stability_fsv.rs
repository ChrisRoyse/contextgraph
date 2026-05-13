use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mcp::handlers::tools::mejepa_eval_tools::mejepa_ship_gate_status_payload;
use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::eval::{
    cell_key, required_active_python_ship_gate_cells, ActiveLearningSummary, EvalProvenance,
    EvalReport, MutationCategory, OpenResearchQuestionStatus, SHIP_GATE_STABILITY_BLOCKER,
};
use context_graph_mejepa::{count_cf, open_infer_rocksdb, Language, RocksDbEvalStore};
use context_graph_mejepa_cf::{CF_MEJEPA_EVAL_REPORTS, CF_MEJEPA_MODEL_PROMOTIONS};
use rocksdb::WriteOptions;
use serde_json::{json, Value};

const DEFAULT_FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-ship-gate-stability-fsv";

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
    let db_path = output_root.join("infer-rocksdb");

    persist_reports(
        &db_path,
        &[
            report("2026-05-13", 1_000, 0.96, true),
            report("2026-05-14", 2_000, 0.96, true),
            report("2026-05-15", 3_000, 0.96, true),
        ],
    )?;
    let after_three = mejepa_ship_gate_status_payload(&db_path)?;

    persist_reports(&db_path, &[report("2026-05-16", 4_000, 0.96, true)])?;
    let after_four = mejepa_ship_gate_status_payload(&db_path)?;

    persist_reports(&db_path, &[report("2026-05-17", 5_000, 0.90, false)])?;
    let after_failure = mejepa_ship_gate_status_payload(&db_path)?;

    persist_reports(
        &db_path,
        &[
            report("2026-05-18", 6_000, 0.97, true),
            report("2026-05-19", 7_000, 0.97, true),
        ],
    )?;
    let before_promotion_reset = mejepa_ship_gate_status_payload(&db_path)?;
    let promotion_key = persist_model_promotion_reset(&db_path, 8_000)?;
    let after_promotion_reset = mejepa_ship_gate_status_payload(&db_path)?;

    let stale_db_path = output_root.join("stale-80-cell-rocksdb");
    persist_reports(&stale_db_path, &[stale_non_python_report()])?;
    let stale_80_error = mejepa_ship_gate_status_payload(&stale_db_path)
        .unwrap_err()
        .to_string();

    let reopened_counts = cf_counts(&db_path)?;
    ensure!(after_three["consecutivePassingWindows"] == 3);
    ensure!(after_three["shipGatePassed"] == false);
    ensure!(after_four["consecutivePassingWindows"] == 4);
    ensure!(after_four["shipGatePassed"] == true);
    ensure!(after_failure["consecutivePassingWindows"] == 0);
    ensure!(after_failure["shipGatePassed"] == false);
    ensure!(before_promotion_reset["consecutivePassingWindows"] == 2);
    ensure!(after_promotion_reset["consecutivePassingWindows"] == 0);
    ensure!(after_promotion_reset["modelPromotionResetCount"] == 1);
    ensure!(stale_80_error.contains("MEJEPA_ACTIVE_PYTHON_GATE_STALE_GRID"));

    let assertions = json!({
        "mcp_response_includes_consecutive_passing_windows": after_four.get("consecutivePassingWindows").is_some(),
        "three_passing_not_ready": after_three["consecutivePassingWindows"] == 3 && after_three["shipGatePassed"] == false,
        "fourth_passing_ready": after_four["consecutivePassingWindows"] == 4 && after_four["shipGatePassed"] == true,
        "failing_window_resets": after_failure["consecutivePassingWindows"] == 0 && after_failure["shipGatePassed"] == false,
        "stability_blocker_returned_before_four": after_three["shipGateFailures"].as_array().unwrap().iter().any(|value| value.as_str().unwrap_or("").starts_with(SHIP_GATE_STABILITY_BLOCKER)),
        "model_promotion_resets": after_promotion_reset["consecutivePassingWindows"] == 0 && after_promotion_reset["modelPromotionResetCount"] == 1,
        "stale_80_cell_gate_rejected": stale_80_error.contains("MEJEPA_ACTIVE_PYTHON_GATE_STALE_GRID"),
        "reopened_report_rows": reopened_counts[CF_MEJEPA_EVAL_REPORTS] == 7,
        "reopened_model_promotion_rows": reopened_counts[CF_MEJEPA_MODEL_PROMOTIONS] == 1,
    });
    ensure!(
        assertions
            .as_object()
            .unwrap()
            .values()
            .all(|value| value.as_bool() == Some(true)),
        "one or more assertions failed: {assertions}"
    );

    let report_path = output_root.join("ship_gate_stability_fsv.json");
    let output = json!({
        "task_id": "TASK-G-006",
        "fsv": "phase_g_ship_gate_stability",
        "all_passed": true,
        "artifact_path": report_path,
        "build_release_sha": git_sha(),
        "source_of_truth": {
            "eval_reports": CF_MEJEPA_EVAL_REPORTS,
            "model_promotions": CF_MEJEPA_MODEL_PROMOTIONS,
            "mcp_response_builder": "context_graph_mcp::handlers::tools::mejepa_ship_gate_status_payload"
        },
        "promotion_reset_key": promotion_key,
        "after_three": after_three,
        "after_four": after_four,
        "after_failure": after_failure,
        "before_promotion_reset": before_promotion_reset,
        "after_promotion_reset": after_promotion_reset,
        "stale_80_error": stale_80_error,
        "reopened_counts": reopened_counts,
        "assertions": assertions,
    });
    write_json_0600(&report_path, &output)?;
    Ok(output)
}

fn persist_reports(db_path: &Path, reports: &[EvalReport]) -> Result<()> {
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
        CF_MEJEPA_MODEL_PROMOTIONS: count_cf(&db, CF_MEJEPA_MODEL_PROMOTIONS)?,
    }))
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
            id: "TASK-G-006".to_string(),
            question: "Does ship-gate readiness require four consecutive passing windows?"
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
            corpus_sha: "phase-g-ship-gate-stability-fsv".to_string(),
            eval_code_version: "task-g-006-fsv".to_string(),
            calibration_version: "task-g-006-fsv".to_string(),
            generated_by: "phase_g_ship_gate_stability_fsv".to_string(),
        },
        wall_clock_seconds: 0.01,
    }
}

fn stale_non_python_report() -> EvalReport {
    let mut per_cell = BTreeMap::new();
    for category in MutationCategory::all() {
        for language in stale_non_python_languages() {
            per_cell.insert(cell_key(category, language), Some(0.97));
        }
    }
    let per_cell_convergence_eta =
        context_graph_mejepa::baseline_convergence_eta_for_cells(&per_cell, 0.95);
    EvalReport {
        report_date: "2026-05-20-stale-80-cell-grid".to_string(),
        generated_at_unix_ms: 9_000,
        rolling_window_size: 800,
        holdout_count: 800,
        overall_correlation: Some(0.97),
        per_category_correlation: MutationCategory::all()
            .into_iter()
            .map(|category| (category, Some(0.97)))
            .collect(),
        per_language_correlation: stale_non_python_languages()
            .into_iter()
            .map(|language| (language, Some(0.97)))
            .collect(),
        per_cell_correlation: per_cell,
        cell_exemptions: BTreeMap::new(),
        bayesian_shrinkage: BTreeMap::new(),
        conformal_coverage_health: BTreeMap::new(),
        ood_calibration_health: BTreeMap::new(),
        gtau_pass_rate: BTreeMap::new(),
        per_prediction_class_calibration: BTreeMap::new(),
        per_failure_mode_class: context_graph_mejepa::empty_failure_mode_class_metrics(
            800,
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
            id: "TASK-PY-G-036".to_string(),
            question: "Does MCP ship-gate status reject stale non-Python 80-cell reports?"
                .to_string(),
            status: "fsv".to_string(),
        }],
        q1_pass_rate: 1.0,
        q2_report_correlation: Some(0.97),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: true,
        ship_gate_failures: Vec::new(),
        provenance: EvalProvenance {
            corpus_sha: "phase-g-ship-gate-stability-fsv".to_string(),
            eval_code_version: "task-py-g-036-fsv".to_string(),
            calibration_version: "task-py-g-036-fsv".to_string(),
            generated_by: "phase_g_ship_gate_stability_fsv".to_string(),
        },
        wall_clock_seconds: 0.01,
    }
}

fn stale_non_python_languages() -> [Language; 10] {
    [
        Language::Rust,
        Language::Javascript,
        Language::Typescript,
        Language::Go,
        Language::Java,
        Language::C,
        Language::Cpp,
        Language::CSharp,
        Language::Ruby,
        Language::Php,
    ]
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
