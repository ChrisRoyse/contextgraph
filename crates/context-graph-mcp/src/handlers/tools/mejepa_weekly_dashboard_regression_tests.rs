use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use context_graph_mejepa::eval::{
    compute_calibration_for_samples, synthetic_holdout, CalibrationSample, RocksDbEvalStore,
};
use context_graph_mejepa::{
    ActiveLearningSummary, AgentId, ConformalHealthEntry, EvalProvenance, EvalReport, FeedbackId,
    FeedbackKind, Language, PredictionId, RegressionCheck, StateTransferDiagnostic, SurpriseEvent,
    SurpriseSeverity, WitnessHash,
};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde_json::json;

use crate::protocol::JsonRpcId;

#[tokio::test]
async fn weekly_eval_dashboard_writes_phase_f_fsv_artifact() {
    let (handlers, _handler_tempdir) =
        crate::handlers::tests::create_protocol_test_handlers().await;
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let fsv_root = std::env::var("CG_WEEKLY_DASHBOARD_FSV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/contextgraph/fsv/phase-f-weekly-dashboard-fsv"));
    std::fs::create_dir_all(&fsv_root).unwrap();
    let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    std::fs::create_dir_all(&run_root).unwrap();

    let happy = seed_dashboard_fixture(&run_root.join("happy"), true, true);
    let happy_response = handlers
        .call_mejepa_weekly_eval_dashboard(
            Some(JsonRpcId::Number(38001)),
            json!({
                "dbPath": happy.db_path,
                "dRoot": happy.d_root,
                "exportsRoot": happy.exports_root,
                "maxCells": 10
            }),
        )
        .await;
    assert!(happy_response.error.is_none());
    let happy_result = happy_response.result.unwrap();
    let happy_structured = happy_result["structuredContent"].clone();
    let happy_calibration_ece = happy_structured["calibration"]["perPredictionClassCalibration"]
        ["q2_oracle_pass"]["expected_calibration_error"]
        .as_f64()
        .unwrap_or(-1.0);
    let happy_pass = happy_result["isError"] == false
        && happy_structured["holdoutFeed"]["status"] == "ready"
        && happy_structured["weeklyFiles"]["status"] == "present"
        && happy_structured["q4Freeze"]["status"] == "frozen"
        && happy_structured["requiredWeeklySections"]["allPresent"] == true
        && happy_structured["agentFeedback"]["counts"]["surprise"] == 1
        && (happy_calibration_ece - 0.25).abs() < f64::EPSILON;

    let missing_holdout = seed_dashboard_fixture(&run_root.join("missing-holdout"), false, true);
    let missing_holdout_response = handlers
        .call_mejepa_weekly_eval_dashboard(
            Some(JsonRpcId::Number(38002)),
            json!({
                "dbPath": missing_holdout.db_path,
                "dRoot": missing_holdout.d_root,
                "exportsRoot": missing_holdout.exports_root,
                "maxCells": 10
            }),
        )
        .await;
    let missing_holdout_result = missing_holdout_response.result.unwrap();

    let corrupt_export = seed_dashboard_fixture(&run_root.join("corrupt-export"), true, true);
    std::fs::write(
        corrupt_export
            .exports_root
            .join(&corrupt_export.report_date)
            .join("weekly.json"),
        b"{not-json",
    )
    .unwrap();
    let corrupt_export_response = handlers
        .call_mejepa_weekly_eval_dashboard(
            Some(JsonRpcId::Number(38003)),
            json!({
                "dbPath": corrupt_export.db_path,
                "dRoot": corrupt_export.d_root,
                "exportsRoot": corrupt_export.exports_root,
                "maxCells": 10
            }),
        )
        .await;
    let corrupt_export_result = corrupt_export_response.result.unwrap();

    let q4_growth = seed_dashboard_fixture(&run_root.join("q4-growth"), true, true);
    seed_q4_growth_row(&q4_growth.db_path);
    let q4_growth_response = handlers
        .call_mejepa_weekly_eval_dashboard(
            Some(JsonRpcId::Number(38005)),
            json!({
                "dbPath": q4_growth.db_path,
                "dRoot": q4_growth.d_root,
                "exportsRoot": q4_growth.exports_root,
                "maxCells": 10
            }),
        )
        .await;
    let q4_growth_result = q4_growth_response.result.unwrap();

    let raw_pass_low_cell = seed_dashboard_fixture_with_report(
        &run_root.join("raw-pass-low-cell"),
        true,
        true,
        synthetic_dashboard_report_with_cell_floor(
            "2026-05-14-raw-pass-low-cell",
            0.40,
            true,
            Vec::new(),
        ),
    );
    let raw_pass_low_cell_response = handlers
        .call_mejepa_weekly_eval_dashboard(
            Some(JsonRpcId::Number(38006)),
            json!({
                "dbPath": raw_pass_low_cell.db_path,
                "dRoot": raw_pass_low_cell.d_root,
                "exportsRoot": raw_pass_low_cell.exports_root,
                "maxCells": 10
            }),
        )
        .await;
    let raw_pass_low_cell_result = raw_pass_low_cell_response.result.unwrap();

    let unknown_arg_response = handlers
        .call_mejepa_weekly_eval_dashboard(
            Some(JsonRpcId::Number(38004)),
            json!({"dbPath": happy.db_path, "unexpected": true}),
        )
        .await;
    let unknown_arg_result = unknown_arg_response.result.unwrap();

    let boundary_cases = vec![
        json!({
            "case": "missing_holdout_feed_is_visible",
            "expected": "MEJEPA_WEEKLY_HOLDOUT_MISSING",
            "actual": missing_holdout_result,
            "pass": missing_holdout_result["structuredContent"]["holdoutFeed"]["errorCode"]
                == "MEJEPA_WEEKLY_HOLDOUT_MISSING"
        }),
        json!({
            "case": "corrupt_weekly_json_is_degraded",
            "expected": "weeklyFiles.jsonHashReadback.status=invalid",
            "actual": corrupt_export_result,
            "pass": corrupt_export_result["structuredContent"]["weeklyFiles"]["jsonHashReadback"]["status"]
                == "invalid"
        }),
        json!({
            "case": "q4_frozen_cf_growth_fails_closed",
            "expected": "q4Freeze.status=growth_detected",
            "actual": q4_growth_result,
            "pass": q4_growth_result["structuredContent"]["q4Freeze"]["status"]
                == "growth_detected"
                && q4_growth_result["structuredContent"]["q4Freeze"]["failClosed"] == true
        }),
        json!({
            "case": "raw_ship_gate_passed_true_low_cell_blocks_dashboard",
            "expected": "report.rawShipGatePassed=true but currentShipGate.status=blocked and overallStatus!=healthy",
            "actual": raw_pass_low_cell_result,
            "pass": raw_pass_low_cell_result["structuredContent"]["report"]["rawShipGatePassed"] == true
                && raw_pass_low_cell_result["structuredContent"]["currentShipGate"]["status"] == "blocked"
                && raw_pass_low_cell_result["structuredContent"]["currentShipGate"]["shipGatePassed"] == false
                && raw_pass_low_cell_result["structuredContent"]["overallStatus"] != "healthy"
                && raw_pass_low_cell_result["structuredContent"]["currentShipGate"]["latestWindowFailures"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|failure| failure.as_str().unwrap_or("").contains("0.400000 < 0.950000"))
        }),
        json!({
            "case": "unknown_argument_fails_schema",
            "expected": "schema validation failed",
            "actual": unknown_arg_result,
            "pass": unknown_arg_result["isError"] == true
                && unknown_arg_result["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains("schema validation failed")
        }),
    ];
    let all_passed = happy_pass && boundary_cases.iter().all(|case| case["pass"] == true);
    let evidence = json!({
        "fsv_root": fsv_root,
        "task_id": "TASK-OBS-001",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_head_short_for_test(),
        "happy_path": [{
            "case": "weekly_dashboard_reads_report_exports_holdout_and_feedback",
            "sot": [
                "CF_MEJEPA_EVAL_REPORTS",
                "CF_MEJEPA_AGENT_FEEDBACK",
                "state/gold-labels/weekly_holdout_panels.json",
                "exports/eval/<date>/weekly.md",
                "exports/eval/<date>/weekly.json"
            ],
            "before": null,
            "trigger": "cargo test -p context-graph-mcp weekly_eval_dashboard_writes_phase_f_fsv_artifact",
            "after": happy_structured,
            "expected": "holdout ready, weekly files present, required sections present, surprise feedback counted, per-class calibration surfaced",
            "actual": happy_pass,
            "pass": happy_pass,
            "evidence_path": happy.run_root
        }],
        "boundary_cases": boundary_cases,
        "all_passed": all_passed,
        "readback_equal": true,
        "physical_artifacts": {
            "run_root": run_root
        }
    });
    assert!(all_passed);
    let evidence_path = fsv_root.join("weekly_dashboard_fsv.json");
    context_graph_mejepa::eval::report::write_json_0600(&evidence_path, &evidence).unwrap();
    assert!(std::fs::metadata(&evidence_path).unwrap().len() > 0);
}

struct DashboardFixture {
    run_root: PathBuf,
    db_path: PathBuf,
    d_root: PathBuf,
    exports_root: PathBuf,
    report_date: String,
}

fn seed_dashboard_fixture(
    run_root: &Path,
    write_holdout: bool,
    write_exports: bool,
) -> DashboardFixture {
    seed_dashboard_fixture_with_report(
        run_root,
        write_holdout,
        write_exports,
        synthetic_dashboard_report("2026-05-13-phase-f-dashboard"),
    )
}

fn seed_dashboard_fixture_with_report(
    run_root: &Path,
    write_holdout: bool,
    write_exports: bool,
    report: EvalReport,
) -> DashboardFixture {
    std::fs::create_dir_all(run_root).unwrap();
    let db_path = run_root.join("storage-db");
    let d_root = run_root.join("d-root");
    let exports_root = d_root.join("exports/eval");
    let report_date = report.report_date.clone();
    std::fs::create_dir_all(&d_root).unwrap();
    let db = context_graph_mejepa_hygiene::open_hygiene_rocksdb(&db_path).unwrap();
    let eval_store = RocksDbEvalStore::new(db.clone()).unwrap();
    eval_store.persist_report(&report).unwrap();
    seed_surprise_feedback(db.as_ref());
    write_q4_freeze_baseline(&d_root, db.as_ref());
    if write_holdout {
        let repo = run_root.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let holdout = synthetic_holdout(&repo).unwrap();
        let holdout_path = d_root.join("state/gold-labels/weekly_holdout_panels.json");
        context_graph_mejepa::eval::report::write_json_0600(&holdout_path, &holdout).unwrap();
    }
    if write_exports {
        let report_dir = exports_root.join(&report_date);
        std::fs::create_dir_all(&report_dir).unwrap();
        context_graph_mejepa::eval::report::write_json_0600(
            &report_dir.join("weekly.json"),
            &report,
        )
        .unwrap();
        write_text_0600(
            &report_dir.join("weekly.md"),
            weekly_markdown_with_required_sections(),
        );
    }
    for cf in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
        if let Some(handle) = db.cf_handle(cf) {
            db.flush_cf(handle).unwrap();
        }
    }
    drop(db);
    DashboardFixture {
        run_root: run_root.to_path_buf(),
        db_path,
        d_root,
        exports_root,
        report_date,
    }
}

fn write_q4_freeze_baseline(d_root: &Path, db: &DB) {
    let mut counts = context_graph_mejepa_cf::FROZEN_Q4_CFS
        .iter()
        .map(|cf| {
            let handle = db.cf_handle(cf).unwrap();
            let count = db.iterator_cf(handle, IteratorMode::Start).count();
            (cf.to_string(), count)
        })
        .collect::<BTreeMap<_, _>>();
    counts.insert(
        context_graph_mejepa_cf::FROZEN_Q4_CALIBRATION_COUNT_KEY.to_string(),
        q4_calibration_count(db),
    );
    let path = d_root.join("state/q4-freeze/q4_frozen_cf_counts.json");
    context_graph_mejepa::eval::report::write_json_0600(&path, &counts).unwrap();
}

fn seed_q4_growth_row(db_path: &Path) {
    let db = context_graph_mejepa_hygiene::open_hygiene_rocksdb(db_path).unwrap();
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_Q4_PERF_LABELS)
        .unwrap();
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, b"forbidden-q4-growth", b"display-only row grew", &opts)
        .unwrap();
    let readback = db.get_cf(cf, b"forbidden-q4-growth").unwrap().unwrap();
    assert_eq!(readback.as_slice(), b"display-only row grew");
    let calibration_cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_HEAD_CALIBRATIONS)
        .unwrap();
    db.put_cf_opt(
        calibration_cf,
        b"q4cal::forbidden-growth",
        b"display-only q4 calibration grew",
        &opts,
    )
    .unwrap();
    let calibration_readback = db
        .get_cf(calibration_cf, b"q4cal::forbidden-growth")
        .unwrap()
        .unwrap();
    assert_eq!(
        calibration_readback.as_slice(),
        b"display-only q4 calibration grew"
    );
}

fn q4_calibration_count(db: &DB) -> usize {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_HEAD_CALIBRATIONS)
        .unwrap();
    db.iterator_cf(cf, IteratorMode::Start)
        .filter_map(Result::ok)
        .filter(|(key, _)| {
            key.as_ref()
                .starts_with(context_graph_mejepa_cf::FROZEN_Q4_CALIBRATION_KEY_PREFIX.as_bytes())
        })
        .count()
}

fn synthetic_dashboard_report(report_date: &str) -> EvalReport {
    synthetic_dashboard_report_with_cell_floor(report_date, 0.96, true, Vec::new())
}

fn synthetic_dashboard_report_with_cell_floor(
    report_date: &str,
    first_cell_correlation: f32,
    raw_ship_gate_passed: bool,
    raw_ship_gate_failures: Vec<String>,
) -> EvalReport {
    let mut per_cell_correlation = BTreeMap::new();
    let mut per_category_correlation = BTreeMap::new();
    for (idx, (category, _language, cell)) in
        context_graph_mejepa::eval::required_active_python_ship_gate_cells()
            .into_iter()
            .enumerate()
    {
        let value = if idx == 0 {
            first_cell_correlation
        } else {
            0.96
        };
        per_cell_correlation.insert(cell.clone(), Some(value));
        per_category_correlation.insert(category, Some(value));
    }
    let per_cell_convergence_eta =
        context_graph_mejepa::baseline_convergence_eta_for_cells(&per_cell_correlation, 0.95);
    let mut per_cell_state_transfer = BTreeMap::new();
    let mut bayesian_shrinkage = BTreeMap::new();
    for cell in per_cell_correlation.keys() {
        per_cell_state_transfer.insert(
            cell.clone(),
            Some(StateTransferDiagnostic {
                wasserstein_1: 0.05,
                transfer_score: 0.92,
                performance_deploy: 0.91,
            }),
        );
        bayesian_shrinkage.insert(cell.clone(), 0.95);
    }
    EvalReport {
        report_date: report_date.to_string(),
        generated_at_unix_ms: 1_778_640_000_000,
        rolling_window_size: 100,
        holdout_count: 4,
        overall_correlation: Some(0.96),
        per_category_correlation,
        per_language_correlation: BTreeMap::from([(Language::Python, Some(0.96))]),
        per_cell_correlation,
        cell_exemptions: BTreeMap::new(),
        bayesian_shrinkage,
        conformal_coverage_health: BTreeMap::from([(
            Language::Python,
            ConformalHealthEntry {
                expected_coverage: 0.90,
                empirical_coverage: 0.90,
                sample_count: 4,
                within_band: true,
            },
        )]),
        ood_calibration_health: BTreeMap::from([(Language::Python, Some(0.88))]),
        gtau_pass_rate: BTreeMap::from([(Language::Python, 0.98)]),
        per_prediction_class_calibration: BTreeMap::from([(
            "q2_oracle_pass".to_string(),
            compute_calibration_for_samples(
                "q2_oracle_pass",
                &[
                    CalibrationSample::try_new(0.25, false).unwrap(),
                    CalibrationSample::try_new(0.25, false).unwrap(),
                    CalibrationSample::try_new(0.75, true).unwrap(),
                    CalibrationSample::try_new(0.75, true).unwrap(),
                ],
                2,
                0.02,
            )
            .unwrap(),
        )]),
        per_failure_mode_class: context_graph_mejepa::empty_failure_mode_class_metrics(
            4,
            &context_graph_mejepa::EvalConfig::default(),
        ),
        per_cell_convergence_eta,
        active_learning: ActiveLearningSummary {
            queued_count: 1,
            evicted_count: 0,
            ood_escalation_count: 0,
        },
        state_transfer_diagnostic: Some(StateTransferDiagnostic {
            wasserstein_1: 0.05,
            transfer_score: 0.92,
            performance_deploy: 0.91,
        }),
        per_cell_state_transfer,
        failing_cell_classifications: BTreeMap::new(),
        aux_head_distillation: None,
        regression_checks: vec![RegressionCheck {
            name: "overall_correlation".to_string(),
            previous: 0.95,
            current: 0.96,
            drop: 0.0,
            passed: true,
        }],
        open_research_questions: Vec::new(),
        q1_pass_rate: 1.0,
        q2_report_correlation: Some(0.96),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: raw_ship_gate_passed,
        ship_gate_failures: raw_ship_gate_failures,
        provenance: EvalProvenance {
            corpus_sha: "phase-f-dashboard-corpus".to_string(),
            eval_code_version: "phase-f-dashboard-test".to_string(),
            calibration_version: "phase-f-dashboard-calibration".to_string(),
            generated_by: "weekly-dashboard-test".to_string(),
        },
        wall_clock_seconds: 0.1,
    }
}

fn seed_surprise_feedback(db: &DB) {
    let event = SurpriseEvent::try_new(SurpriseEvent {
        feedback_id: FeedbackId([1; 16]),
        prediction_id: PredictionId([2; 16]),
        agent_id: AgentId("agent:weekly-dashboard-test".to_string()),
        ts_millis: 1_778_640_000_001,
        feedback_kind: FeedbackKind::Surprise,
        agent_explanation: "synthetic surprise for weekly dashboard FSV".to_string(),
        actual_outcome: Some(context_graph_mejepa::ActualOutcome {
            oracle_outcome: context_graph_mejepa::OracleOutcome::Fail,
            failed_tests: Vec::new(),
            runtime_ms: Some(10),
            notes: "synthetic".to_string(),
        }),
        severity: SurpriseSeverity::High,
        extra_structured_data: json!({"source": "weekly-dashboard-fsv"}),
        witness_hash: WitnessHash([3; 32]),
    })
    .unwrap();
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)
        .unwrap();
    let bytes = serde_json::to_vec(&event).unwrap();
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, b"feedback:weekly-dashboard-fsv", &bytes, &opts)
        .unwrap();
    let readback = db
        .get_cf(cf, b"feedback:weekly-dashboard-fsv")
        .unwrap()
        .unwrap();
    assert_eq!(readback, bytes);
}

fn weekly_markdown_with_required_sections() -> &'static str {
    "# ME-JEPA Weekly Evaluation\n\n\
     ## Catastrophic Events\n\n- none\n\n\
     ## Per-Failure-Mode-Class Metrics\n\n\
     | failure_mode_class | precision | recall | f1 | tp | fp | fn | sample_count | passed | weakness |\n\
     |---|---:|---:|---:|---:|---:|---:|---:|---|---|\n\
     | name_error | 1.000000 | 1.000000 | 1.000000 | 0 | 0 | 0 | 4 | true | none |\n\n\
     ## Per-Cell Ship Gate Status\n\n- known_good::python: pass\n\n\
     ## Convergence ETA Per Cell\n\n\
     | cell | latest_correlation | eta_status | estimated_passing_window | confidence_interval | slope_per_window | r_squared | history_windows | valid_points |\n\
     |---|---:|---|---:|---|---:|---:|---:|---:|\n\
     | known_good::python | 0.960000 | already_passing | 0 | 0..0 | unavailable | unavailable | 1 | 1 |\n\n\
     ## Active-Learning Queue Summary\n\n- queued_count: 1\n\n\
     ## Curiosity Ranking\n\n- no curiosity-ranked entries\n\n\
     ## Agent-Feedback Summary\n\n- surprise: 1\n\n\
     ## Drift Events Of The Week\n\n- none\n\n\
     ## Promotions / Rollbacks Of The Week\n\n- none\n\n\
     ## Storage Utilization\n\n- ok\n\n\
     ## Per-Cell State Transfer T\n\n\
     | state_transfer_per_cell | transfer_score | wasserstein_1 | performance_deploy | candidate |\n\
     |---|---:|---:|---:|---|\n\
     | known_good::python | 0.920000 | 0.050000 | 0.910000 | hold |\n\n\
     ## Ship Gate Failures\n\n- none\n"
}

fn write_text_0600(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .unwrap()
    };
    #[cfg(not(unix))]
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .unwrap();
    file.write_all(text.as_bytes()).unwrap();
    file.sync_all().unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
}

fn git_head_short_for_test() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
