//! FSV harness for the Phase F operator runbook CLI commands.
//!
//! Exercises `mejepa daemon-status`, `mejepa report-weekly`,
//! `mejepa eval-ship-gate`, and `mejepa rollback` against synthetic but
//! schema-valid Source-of-Truth state under `/mnt/d/contextgraph/fsv/`.
//! The rollback happy path is already FSV'd at the heal layer by
//! `rollback_uses_persisted_heal_report_source_of_truth`; here we cover
//! the CLI wrapper's validation + error surface so the operator binding
//! is independently verified.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, ensure, Context, Result};
use chrono::Utc;
use context_graph_cli::commands::mejepa_runbook::{
    run_daemon_status, run_eval_ship_gate, run_pause, run_report_weekly, run_rollback,
    run_session_cleanup, DaemonStatusArgs, EvalShipGateArgs, PauseArgs, ReportWeeklyArgs,
    RollbackArgs, SessionCleanupArgs,
};
use context_graph_mejepa::eval::types::{
    ActiveLearningSummary, EvalProvenance, EvalReport, OpenResearchQuestionStatus,
};
use context_graph_mejepa::{open_infer_rocksdb, RocksDbEvalStore};
use context_graph_mejepa_cf::{CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_SHIFT_WATERMARK};
use rocksdb::WriteOptions;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-runbook-cli-fsv";

fn main() -> Result<()> {
    let started_at_unix_ms = Utc::now().timestamp_millis();
    let fsv_root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv_root).context("create FSV root")?;
    let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    fs::create_dir_all(&run_root).context("create run root")?;
    let build_release_sha = git_head_short()?;

    // --- daemon-status happy path -----------------------------------------
    let state_root = run_root.join("scheduler-state");
    fs::create_dir_all(&state_root).context("create scheduler state root")?;
    let status_path = state_root.join("self_optimization_status.json");
    let scheduler_status = json!({
        "status": "running",
        "started_at_unix_ms": started_at_unix_ms,
        "updated_at_unix_ms": started_at_unix_ms + 1,
        "ticker_counts": {"observe": 3_u32, "drift": 1_u32},
        "last_ticker": "observe",
    });
    fs::write(
        &status_path,
        serde_json::to_vec_pretty(&scheduler_status).context("encode scheduler status")?,
    )
    .context("write scheduler status")?;

    let daemon_output = run_daemon_status(DaemonStatusArgs {
        state_root: state_root.clone(),
    })
    .context("run daemon-status happy path")?;
    ensure!(
        daemon_output.scheduler_status == scheduler_status,
        "daemon-status did not return persisted scheduler-status JSON"
    );
    ensure!(
        daemon_output.status_path == status_path,
        "daemon-status reported wrong status path"
    );

    // --- report-weekly + eval-ship-gate happy path ------------------------
    let db_path = run_root.join("infer-rocksdb");
    let report_date = "2026-05-13";
    let synthetic_report = build_synthetic_report(report_date, 0.93, true, Vec::new());
    let stored_hash = synthetic_report
        .determinism_hash()
        .map_err(|err| anyhow!("hash synthetic report: {err}"))?;
    persist_synthetic_report(&db_path, &synthetic_report)?;

    let report_output = run_report_weekly(ReportWeeklyArgs {
        db_path: db_path.clone(),
        date: report_date.to_string(),
    })
    .context("run report-weekly happy path")?;
    ensure!(
        report_output.determinism_hash == stored_hash,
        "report-weekly determinism hash mismatch"
    );
    ensure!(
        report_output.overall_correlation == synthetic_report.overall_correlation,
        "report-weekly overall correlation mismatch"
    );
    ensure!(
        report_output.ship_gate_passed,
        "report-weekly should expose ship_gate_passed=true"
    );

    let ship_gate_output = run_eval_ship_gate(EvalShipGateArgs {
        db_path: db_path.clone(),
    })
    .context("run eval-ship-gate happy path")?;
    ensure!(
        ship_gate_output.ship_gate_passed,
        "eval-ship-gate ship_gate_passed=false unexpectedly"
    );
    ensure!(
        ship_gate_output.latest_report_date == report_date,
        "eval-ship-gate returned wrong report_date"
    );

    // --- failing-gate happy path: persist a second-day report that fails --
    let failing_report = build_synthetic_report(
        "2026-05-14",
        0.80,
        false,
        vec!["per_cell_min_correlation_below_0.95".to_string()],
    );
    persist_synthetic_report(&db_path, &failing_report)?;
    let failing_output = run_eval_ship_gate(EvalShipGateArgs {
        db_path: db_path.clone(),
    })
    .context("run eval-ship-gate failing path")?;
    ensure!(
        !failing_output.ship_gate_passed,
        "eval-ship-gate should report ship_gate_passed=false after failing report"
    );
    ensure!(
        failing_output.latest_report_date == "2026-05-14",
        "eval-ship-gate should select the most recently generated report"
    );

    // --- pause happy path -------------------------------------------------
    let pause_path = run_root.join("pause-state/predictions_paused_until.json");
    let pause_output = run_pause(PauseArgs {
        state_path: pause_path.clone(),
        duration_mins: 15,
        reason: "phase-f-fsv".to_string(),
    })
    .context("run pause happy path")?;
    let pause_readback: Value =
        serde_json::from_slice(&fs::read(&pause_path).context("read pause SoT")?)?;
    ensure!(
        pause_output.readback_equal,
        "pause command must report readback_equal=true"
    );
    let paused_until_ms = pause_readback["pausedUntilUnixMs"]
        .as_i64()
        .ok_or_else(|| anyhow!("pause readback missing pausedUntilUnixMs"))?;
    let set_at_ms = pause_readback["setAtUnixMs"]
        .as_i64()
        .ok_or_else(|| anyhow!("pause readback missing setAtUnixMs"))?;
    ensure!(
        paused_until_ms > set_at_ms,
        "pause readback must persist a future pausedUntilUnixMs"
    );

    // --- session-cleanup happy path --------------------------------------
    let cleanup_db = run_root.join("session-cleanup-rocksdb");
    let cleanup_session_id = "00112233445566778899aabbccddeeff";
    seed_session_cleanup_db(&cleanup_db, cleanup_session_id)?;
    let cleanup_output = run_session_cleanup(SessionCleanupArgs {
        db_path: cleanup_db.clone(),
        session_id: cleanup_session_id.to_string(),
    })
    .context("run session-cleanup happy path")?;
    ensure!(
        cleanup_output.live_predictions_deleted == 2,
        "session-cleanup should delete both live-prediction rows"
    );
    ensure!(
        cleanup_output.shift_watermark_deleted,
        "session-cleanup should delete the session watermark"
    );
    ensure!(
        cleanup_output.readback_equal,
        "session-cleanup must verify deletion readback"
    );

    // --- boundary cases ---------------------------------------------------
    let boundary_cases = run_boundary_cases(&run_root, &db_path)?;
    let failed: Vec<&Value> = boundary_cases
        .iter()
        .filter(|case| case["pass"].as_bool() != Some(true))
        .collect();
    if !failed.is_empty() {
        eprintln!("FAILED boundary cases:");
        for case in &failed {
            eprintln!(
                "  - {} expected={} actual={}",
                case["case"], case["expected"], case["actual"]
            );
        }
        return Err(anyhow!("boundary cases must all pass"));
    }

    // --- write the FSV artifact -------------------------------------------
    let artifact_path = fsv_root.join("runbook_cli_fsv.json");
    let cf_counts_after = json!({
        "CF_MEJEPA_EVAL_REPORTS": count_eval_reports(&db_path)?,
    });
    let artifact = json!({
        "fsv_root": fsv_root,
        "task_ids": [
            "TASK-RUN-002",
            "TASK-RUN-003",
            "TASK-RUN-006",
            "TASK-RUN-007",
            "TASK-RUN-008",
            "TASK-RUN-009"
        ],
        "issues": [73, 74, 76, 77, 78, 79],
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": build_release_sha,
        "trigger": "cargo run --release -p context-graph-cli --example phase_f_runbook_cli_fsv",
        "happy_path": [
            {
                "case": "daemon_status_reads_persisted_status_json",
                "sot": json!({"kind": "file", "path": status_path}),
                "before": null,
                "after": daemon_output.scheduler_status.clone(),
                "expected": scheduler_status,
                "actual": daemon_output.scheduler_status.clone(),
                "pass": true,
                "evidence_path": status_path,
            },
            {
                "case": "report_weekly_loads_persisted_report_for_date",
                "sot": "CF_MEJEPA_EVAL_REPORTS",
                "before": null,
                "after": json!({
                    "date": report_output.date,
                    "ship_gate_passed": report_output.ship_gate_passed,
                    "overall_correlation": report_output.overall_correlation,
                    "determinism_hash": report_output.determinism_hash,
                }),
                "expected": json!({
                    "date": report_date,
                    "ship_gate_passed": true,
                    "overall_correlation": synthetic_report.overall_correlation,
                    "determinism_hash": stored_hash,
                }),
                "actual": json!({
                    "date": report_output.date,
                    "ship_gate_passed": report_output.ship_gate_passed,
                    "overall_correlation": report_output.overall_correlation,
                    "determinism_hash": report_output.determinism_hash,
                }),
                "pass": true,
                "evidence_path": db_path.clone(),
            },
            {
                "case": "eval_ship_gate_returns_latest_failing_report",
                "sot": "CF_MEJEPA_EVAL_REPORTS",
                "before": json!({"ship_gate_passed_first": true}),
                "after": json!({
                    "latest_report_date": failing_output.latest_report_date,
                    "ship_gate_passed": failing_output.ship_gate_passed,
                    "ship_gate_failures": failing_output.ship_gate_failures,
                }),
                "expected": json!({
                    "latest_report_date": "2026-05-14",
                    "ship_gate_passed": false,
                    "ship_gate_failures": ["per_cell_min_correlation_below_0.95"],
                }),
                "actual": json!({
                    "latest_report_date": failing_output.latest_report_date,
                    "ship_gate_passed": failing_output.ship_gate_passed,
                    "ship_gate_failures": failing_output.ship_gate_failures,
                }),
                "pass": true,
                "evidence_path": db_path.clone(),
            },
            {
                "case": "pause_writes_future_pause_state",
                "sot": json!({"kind": "file", "path": pause_path}),
                "before": null,
                "after": pause_readback,
                "expected": json!({
                    "reason": "phase-f-fsv",
                    "paused_until_gt_set_at": true,
                    "readback_equal": true,
                }),
                "actual": json!({
                    "reason": pause_output.reason,
                    "paused_until_gt_set_at": pause_output.paused_until_unix_ms > pause_output.set_at_unix_ms,
                    "readback_equal": pause_output.readback_equal,
                }),
                "pass": true,
                "evidence_path": pause_path,
            },
            {
                "case": "session_cleanup_deletes_live_predictions_and_watermark",
                "sot": json!({
                    "dbPath": cleanup_db,
                    "columnFamilies": [CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_SHIFT_WATERMARK]
                }),
                "before": json!({
                    "live_predictions": 2,
                    "shift_watermark": true,
                }),
                "after": json!({
                    "live_predictions_deleted": cleanup_output.live_predictions_deleted,
                    "shift_watermark_deleted": cleanup_output.shift_watermark_deleted,
                    "readback_equal": cleanup_output.readback_equal,
                }),
                "expected": json!({
                    "live_predictions_deleted": 2,
                    "shift_watermark_deleted": true,
                    "readback_equal": true,
                }),
                "actual": json!({
                    "live_predictions_deleted": cleanup_output.live_predictions_deleted,
                    "shift_watermark_deleted": cleanup_output.shift_watermark_deleted,
                    "readback_equal": cleanup_output.readback_equal,
                }),
                "pass": true,
                "evidence_path": cleanup_db,
            }
        ],
        "boundary_cases": boundary_cases,
        "all_passed": true,
        "cf_counts_before": json!({"CF_MEJEPA_EVAL_REPORTS": 0}),
        "cf_counts_after": cf_counts_after,
        "readback_equal": true,
        "physical_artifacts": physical_artifacts(&run_root)?,
        "external_evidence": {
            "rollback_happy_path_fsv": "context_graph_mejepa::heal::promote::tests::rollback_uses_persisted_heal_report_source_of_truth"
        },
    });
    let bytes = serde_json::to_vec_pretty(&artifact).context("serialize FSV artifact")?;
    fs::write(&artifact_path, &bytes).context("write FSV artifact")?;
    let artifact_sha256 = format!("{:x}", Sha256::digest(&bytes));
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artifact_path": artifact_path,
            "artifact_sha256": artifact_sha256,
            "all_passed": true,
            "readback_equal": true,
        }))?
    );
    Ok(())
}

fn build_synthetic_report(
    date: &str,
    overall_correlation: f32,
    ship_gate_passed: bool,
    ship_gate_failures: Vec<String>,
) -> EvalReport {
    EvalReport {
        report_date: date.to_string(),
        generated_at_unix_ms: Utc::now().timestamp_millis(),
        rolling_window_size: 100,
        holdout_count: 300,
        overall_correlation: Some(overall_correlation),
        per_category_correlation: BTreeMap::new(),
        per_language_correlation: BTreeMap::new(),
        per_cell_correlation: BTreeMap::new(),
        bayesian_shrinkage: BTreeMap::new(),
        conformal_coverage_health: BTreeMap::new(),
        ood_calibration_health: BTreeMap::new(),
        gtau_pass_rate: BTreeMap::new(),
        per_prediction_class_calibration: BTreeMap::new(),
        active_learning: ActiveLearningSummary {
            queued_count: 0,
            evicted_count: 0,
            ood_escalation_count: 0,
        },
        state_transfer_diagnostic: None,
        per_cell_state_transfer: BTreeMap::new(),
        aux_head_distillation: None,
        regression_checks: Vec::new(),
        open_research_questions: vec![OpenResearchQuestionStatus {
            id: "RQ-1".to_string(),
            question: "Does ship-gate cleanly bind to operator CLI?".to_string(),
            status: "open".to_string(),
        }],
        q1_pass_rate: 0.97,
        q2_report_correlation: Some(overall_correlation),
        q3_side_effect_agreement: Some(0.95),
        ship_gate_passed,
        ship_gate_failures,
        provenance: EvalProvenance {
            corpus_sha: "phase-f-runbook-fsv".to_string(),
            eval_code_version: "fsv-1".to_string(),
            calibration_version: "fsv-cal-1".to_string(),
            generated_by: "phase_f_runbook_cli_fsv".to_string(),
        },
        wall_clock_seconds: 1.0,
    }
}

fn persist_synthetic_report(db_path: &Path, report: &EvalReport) -> Result<()> {
    let db = open_infer_rocksdb(db_path).context("open infer rocksdb")?;
    let store =
        RocksDbEvalStore::new(db.clone()).map_err(|err| anyhow!("open eval store: {err}"))?;
    store
        .persist_report(report)
        .map_err(|err| anyhow!("persist synthetic report: {err}"))?;
    drop(store);
    drop(db);
    Ok(())
}

fn run_boundary_cases(run_root: &Path, db_path: &Path) -> Result<Vec<Value>> {
    let mut cases = Vec::new();

    // 1. daemon-status: missing state root
    let missing_state_root = run_root.join("missing-scheduler-state");
    let err = run_daemon_status(DaemonStatusArgs {
        state_root: missing_state_root.clone(),
    })
    .expect_err("missing state root must fail closed");
    cases.push(boundary(
        "daemon_status_missing_state_root",
        "MEJEPA_RUNBOOK_STATEROOT_MISSING",
        &err,
    ));

    // 2. daemon-status: invalid JSON in status file
    let bad_state_root = run_root.join("bad-scheduler-state");
    fs::create_dir_all(&bad_state_root).context("create bad-scheduler-state")?;
    fs::write(
        bad_state_root.join("self_optimization_status.json"),
        "{not valid json",
    )
    .context("write bad status json")?;
    let err = run_daemon_status(DaemonStatusArgs {
        state_root: bad_state_root,
    })
    .expect_err("invalid scheduler-status JSON must fail closed");
    cases.push(boundary(
        "daemon_status_invalid_json",
        "MEJEPA_RUNBOOK_DAEMON_STATUS_JSON_INVALID",
        &err,
    ));

    // 3. report-weekly: empty date
    let err = run_report_weekly(ReportWeeklyArgs {
        db_path: db_path.to_path_buf(),
        date: "   ".to_string(),
    })
    .expect_err("empty date must fail closed");
    cases.push(boundary(
        "report_weekly_empty_date",
        "MEJEPA_RUNBOOK_DATE_EMPTY",
        &err,
    ));

    // 4. report-weekly: malformed date
    let err = run_report_weekly(ReportWeeklyArgs {
        db_path: db_path.to_path_buf(),
        date: "2026/05/13".to_string(),
    })
    .expect_err("malformed date must fail closed");
    cases.push(boundary(
        "report_weekly_malformed_date",
        "MEJEPA_RUNBOOK_DATE_INVALID",
        &err,
    ));

    // 5. report-weekly: no report for date
    let err = run_report_weekly(ReportWeeklyArgs {
        db_path: db_path.to_path_buf(),
        date: "1999-01-01".to_string(),
    })
    .expect_err("absent date must fail closed");
    cases.push(boundary(
        "report_weekly_date_not_found",
        "MEJEPA_RUNBOOK_REPORT_NOT_FOUND",
        &err,
    ));

    // 6. eval-ship-gate: empty inference DB
    let empty_db_path = run_root.join("empty-infer-rocksdb");
    let err = run_eval_ship_gate(EvalShipGateArgs {
        db_path: empty_db_path,
    })
    .expect_err("empty CF_MEJEPA_EVAL_REPORTS must fail closed");
    cases.push(boundary(
        "eval_ship_gate_no_report",
        "MEJEPA_RUNBOOK_LATEST_REPORT_MISSING",
        &err,
    ));

    // 7. rollback: empty heal DB path rejected at the validation layer
    let err = run_rollback(RollbackArgs {
        heal_db_path: PathBuf::new(),
        witness_chain_path: run_root.join("rollback-chain.bin"),
        target_witness_offset: 0,
    })
    .expect_err("empty heal db path must fail closed");
    cases.push(boundary(
        "rollback_empty_heal_db_path",
        "MEJEPA_RUNBOOK_HEAL_DB_PATH_EMPTY",
        &err,
    ));

    // 8. rollback: empty witness chain path rejected at the validation layer
    let witness_boundary_heal_db = run_root.join("rollback-witness-boundary-rocksdb");
    {
        let _opened = context_graph_mejepa::heal::HealRocksStore::open(&witness_boundary_heal_db)
            .map_err(|err| anyhow!("MEJEPA_RUNBOOK_FSV_HEAL_DB_INIT_FAILED: {err}"))?;
    }
    let err = run_rollback(RollbackArgs {
        heal_db_path: witness_boundary_heal_db,
        witness_chain_path: PathBuf::new(),
        target_witness_offset: 0,
    })
    .expect_err("empty witness chain path must fail closed");
    cases.push(boundary(
        "rollback_empty_witness_chain_path",
        "MEJEPA_RUNBOOK_WITNESSCHAINPATH_EMPTY",
        &err,
    ));

    // 9. rollback: target offset not in CF_MEJEPA_HEAL_REPORTS — the
    //    rocksdb path is created lazily but the rollback fails with the
    //    inner `RollbackTargetGone` error code, wrapped by our CLI code.
    let heal_db = run_root.join("heal-rocksdb-fresh");
    {
        let _opened = context_graph_mejepa::heal::HealRocksStore::open(&heal_db)
            .map_err(|err| anyhow!("MEJEPA_RUNBOOK_FSV_HEAL_DB_INIT_FAILED: {err}"))?;
    }
    let err = run_rollback(RollbackArgs {
        heal_db_path: heal_db,
        witness_chain_path: run_root.join("rollback-chain.bin"),
        target_witness_offset: 9_999_999,
    })
    .expect_err("nonexistent target offset must fail closed");
    cases.push(boundary(
        "rollback_target_offset_missing",
        "MEJEPA_RUNBOOK_ROLLBACK_FAILED",
        &err,
    ));

    // 10. pause: zero duration rejected before writing state.
    let err = run_pause(PauseArgs {
        state_path: run_root.join("pause-zero.json"),
        duration_mins: 0,
        reason: "phase-f-fsv".to_string(),
    })
    .expect_err("zero pause duration must fail closed");
    cases.push(boundary(
        "pause_zero_duration",
        "MEJEPA_RUNBOOK_PAUSE_DURATION_ZERO",
        &err,
    ));

    // 11. pause: maximum + 1 duration rejected.
    let err = run_pause(PauseArgs {
        state_path: run_root.join("pause-too-long.json"),
        duration_mins: 7 * 24 * 60 + 1,
        reason: "phase-f-fsv".to_string(),
    })
    .expect_err("maximum + 1 pause duration must fail closed");
    cases.push(boundary(
        "pause_duration_exceeds_week",
        "MEJEPA_RUNBOOK_PAUSE_DURATION_EXCEEDS_WEEK",
        &err,
    ));

    // 12. pause: empty reason rejected.
    let err = run_pause(PauseArgs {
        state_path: run_root.join("pause-empty-reason.json"),
        duration_mins: 1,
        reason: "   ".to_string(),
    })
    .expect_err("empty pause reason must fail closed");
    cases.push(boundary(
        "pause_empty_reason",
        "MEJEPA_RUNBOOK_PAUSE_REASON_EMPTY",
        &err,
    ));

    // 13. pause: control characters in reason rejected.
    let err = run_pause(PauseArgs {
        state_path: run_root.join("pause-control-reason.json"),
        duration_mins: 1,
        reason: "bad\nreason".to_string(),
    })
    .expect_err("control-character pause reason must fail closed");
    cases.push(boundary(
        "pause_control_character_reason",
        "MEJEPA_RUNBOOK_PAUSE_REASON_INVALID",
        &err,
    ));

    // 14. pause: empty state path rejected.
    let err = run_pause(PauseArgs {
        state_path: PathBuf::new(),
        duration_mins: 1,
        reason: "phase-f-fsv".to_string(),
    })
    .expect_err("empty pause state path must fail closed");
    cases.push(boundary(
        "pause_empty_state_path",
        "MEJEPA_RUNBOOK_PAUSE_STATE_PATH_EMPTY",
        &err,
    ));

    // 15. session-cleanup: malformed session id rejected before DB open.
    let err = run_session_cleanup(SessionCleanupArgs {
        db_path: db_path.to_path_buf(),
        session_id: "not-hex".to_string(),
    })
    .expect_err("malformed session id must fail closed");
    cases.push(boundary(
        "session_cleanup_invalid_session_id",
        "MEJEPA_RUNBOOK_SESSION_ID_INVALID",
        &err,
    ));

    Ok(cases)
}

fn seed_session_cleanup_db(db_path: &Path, session_hex: &str) -> Result<()> {
    let mut session_id = [0_u8; 16];
    hex::decode_to_slice(session_hex, &mut session_id)
        .context("decode session id for cleanup FSV")?;
    let db = open_infer_rocksdb(db_path).context("open session-cleanup infer rocksdb")?;
    let live_cf = db
        .cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
        .ok_or_else(|| anyhow!("missing CF_MEJEPA_LIVE_PREDICTIONS"))?;
    let watermark_cf = db
        .cf_handle(CF_MEJEPA_SHIFT_WATERMARK)
        .ok_or_else(|| anyhow!("missing CF_MEJEPA_SHIFT_WATERMARK"))?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    let mut key_one = session_id.to_vec();
    key_one.extend_from_slice(b":prediction-one");
    let mut key_two = session_id.to_vec();
    key_two.extend_from_slice(b":prediction-two");
    db.put_cf_opt(live_cf, &key_one, b"prediction-one", &opts)
        .context("write live prediction one")?;
    db.put_cf_opt(live_cf, &key_two, b"prediction-two", &opts)
        .context("write live prediction two")?;
    db.put_cf_opt(watermark_cf, session_id, b"watermark", &opts)
        .context("write shift watermark")?;
    db.flush_cf(live_cf).context("flush live prediction CF")?;
    db.flush_cf(watermark_cf)
        .context("flush shift watermark CF")?;
    drop(db);
    Ok(())
}

fn boundary(case: &str, expected: &str, err: &anyhow::Error) -> Value {
    let actual = format!("{err:#}");
    json!({
        "case": case,
        "before": null,
        "after": null,
        "expected": expected,
        "actual": actual,
        "pass": actual.contains(expected),
    })
}

fn count_eval_reports(db_path: &Path) -> Result<u64> {
    let db = open_infer_rocksdb(db_path).context("reopen infer rocksdb for count")?;
    let count = context_graph_mejepa::count_cf(
        db.as_ref(),
        context_graph_mejepa::eval::store::CF_MEJEPA_EVAL_REPORTS,
    )
    .context("count CF_MEJEPA_EVAL_REPORTS")?;
    Ok(count)
}

fn physical_artifacts(run_root: &Path) -> Result<Value> {
    let mut total_bytes = 0u64;
    let mut file_count = 0u64;
    let mut sst_files = Vec::new();
    collect(run_root, &mut total_bytes, &mut file_count, &mut sst_files)?;
    Ok(json!({
        "run_root": run_root,
        "file_count": file_count,
        "total_bytes": total_bytes,
        "sst_files": sst_files,
    }))
}

fn collect(
    path: &Path,
    total_bytes: &mut u64,
    file_count: &mut u64,
    sst_files: &mut Vec<Value>,
) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read dir {}", path.display()))? {
        let entry = entry?;
        let p = entry.path();
        let meta = entry.metadata()?;
        if meta.is_dir() {
            collect(&p, total_bytes, file_count, sst_files)?;
            continue;
        }
        *file_count += 1;
        *total_bytes += meta.len();
        if p.extension().and_then(|ext| ext.to_str()) == Some("sst") {
            sst_files.push(json!({"path": p, "bytes": meta.len()}));
        }
    }
    Ok(())
}

fn git_head_short() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .context("git rev-parse --short HEAD")?;
    ensure!(
        output.status.success(),
        "git rev-parse failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
