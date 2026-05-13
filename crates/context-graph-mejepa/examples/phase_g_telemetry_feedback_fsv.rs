use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::eval::{LatencyTelemetry, ProductionTelemetryWindow, RocksDbEvalStore};
use context_graph_mejepa::heal::{
    pending_promotions, read_status_snapshot, write_json_atomic_readback, HealReport,
    HealRocksStore, SchedulerState, SelfOptimConfig, TickName,
};
use context_graph_mejepa::system_cost::HealTickerTelemetrySnapshot;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde_json::{json, Value};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-telemetry-feedback-fsv";
const REPORT_NAME: &str = "telemetry_feedback_fsv.json";

fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT);
    if root.exists() {
        fs::remove_dir_all(&root).with_context(|| format!("remove {}", root.display()))?;
    }
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;

    let cases = [
        run_case(
            &root,
            "both_gates_queue_recalibration_and_retrain",
            vec![telemetry_window(
                "both-breach-latest",
                1_778_680_000_001,
                0.05,
                0.93,
            )],
            &[
                "telemetry_conformal_recalibration",
                "telemetry_full_retrain_candidate",
            ],
            1,
            "telemetry_feedback_actions_queued",
        )?,
        run_case(
            &root,
            "edge_exact_band_and_agreement_floor_no_action",
            vec![telemetry_window(
                "exact-thresholds",
                1_778_680_000_002,
                0.02,
                0.95,
            )],
            &[],
            0,
            "telemetry_feedback_within_thresholds",
        )?,
        run_case(
            &root,
            "edge_only_conformal_ece_queues_recalibration",
            vec![telemetry_window(
                "only-recalibration",
                1_778_680_000_003,
                0.021,
                0.97,
            )],
            &["telemetry_conformal_recalibration"],
            0,
            "telemetry_feedback_actions_queued",
        )?,
        run_case(
            &root,
            "edge_only_agreement_queues_retrain_candidate",
            vec![telemetry_window(
                "only-retrain",
                1_778_680_000_004,
                0.0,
                0.94,
            )],
            &["telemetry_full_retrain_candidate"],
            1,
            "telemetry_feedback_actions_queued",
        )?,
        run_case(
            &root,
            "edge_latest_healthy_window_suppresses_older_bad_window",
            vec![
                telemetry_window("older-bad-window", 1_778_680_000_005, 0.08, 0.80),
                telemetry_window("newer-healthy-window", 1_778_680_000_006, 0.0, 0.99),
            ],
            &[],
            0,
            "telemetry_feedback_within_thresholds",
        )?,
        run_case(
            &root,
            "edge_no_telemetry_window_no_action",
            Vec::new(),
            &[],
            0,
            "production_telemetry_absent",
        )?,
    ];
    let report_path = root.join(REPORT_NAME);
    let report = json!({
        "schema_version": 1,
        "task_id": "TASK-G-009",
        "build_release_sha": current_git_sha()?,
        "source_of_truth": {
            "rocksdb_column_families": [
                context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY,
                context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS,
                context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS
            ],
            "scheduler_status_file": "self_optimization_status.json",
            "report_path": report_path
        },
        "happy_path": cases[0],
        "edge_cases": &cases[1..],
        "case_count": cases.len(),
        "all_passed": cases.iter().all(|case| case["pass"] == json!(true)),
        "readback_equal": true
    });
    write_json_atomic_readback(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    let canonical_report: Value = serde_json::from_slice(&serde_json::to_vec(&report)?)?;
    ensure!(
        readback == canonical_report,
        "telemetry feedback FSV report readback mismatch"
    );
    ensure!(
        readback["all_passed"] == json!(true),
        "telemetry feedback FSV did not record all_passed=true"
    );
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run_case(
    root: &Path,
    case_name: &str,
    windows: Vec<ProductionTelemetryWindow>,
    expected_trigger_reasons: &[&str],
    expected_pending_promotions: usize,
    expected_action: &str,
) -> Result<Value> {
    let case_root = root.join(case_name);
    fs::create_dir_all(&case_root).with_context(|| format!("create {}", case_root.display()))?;
    let db_path = case_root.join("rocksdb");
    let status_path = case_root.join("self_optimization_status.json");
    let db = open_db(&db_path)?;
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    for window in &windows {
        eval_store.persist_production_telemetry(window)?;
        let loaded = eval_store
            .load_production_telemetry(&window.window_id)?
            .context("production telemetry readback missing after persist")?;
        ensure!(loaded == *window, "production telemetry readback mismatch");
    }
    let config = SelfOptimConfig {
        status_path: status_path.clone(),
        hygiene_archive_root: case_root.join("archive"),
        witness_chain_path: case_root.join("witness-chain.bin"),
        test_outcome_root: case_root.join("test-outcomes"),
        ..SelfOptimConfig::default()
    };
    let mut scheduler = SchedulerState::open(db.clone(), config)?;
    let before = physical_state(db.as_ref(), &status_path)?;
    scheduler.run_tick(TickName::TelemetryFeedback)?;
    let after = physical_state(db.as_ref(), &status_path)?;
    let snapshot = read_status_snapshot(&status_path)?;
    let tick = snapshot
        .last_success
        .get("telemetry_feedback")
        .with_context(|| format!("{case_name} missing telemetry_feedback tick record"))?;
    ensure!(
        tick.action == expected_action,
        "{case_name} expected action {expected_action}, got {}",
        tick.action
    );
    let after_reports = telemetry_feedback_reports(db.as_ref())?;
    let after_trigger_reasons = after_reports
        .iter()
        .map(|report| {
            report["triggerReason"]
                .as_str()
                .context("triggerReason must be string")
        })
        .collect::<Result<Vec<_>>>()?;
    for expected in expected_trigger_reasons {
        ensure!(
            after_trigger_reasons.contains(expected),
            "{case_name} missing expected trigger reason {expected}; got {after_trigger_reasons:?}"
        );
    }
    ensure!(
        after_reports.len() == expected_trigger_reasons.len(),
        "{case_name} expected {} telemetry feedback heal reports, got {}",
        expected_trigger_reasons.len(),
        after_reports.len()
    );
    let heal_store = HealRocksStore::from_db(db.clone())?;
    let pending = pending_promotions(heal_store.as_ref())?;
    ensure!(
        pending.len() == expected_pending_promotions,
        "{case_name} expected {expected_pending_promotions} pending retrain promotions, got {}",
        pending.len()
    );
    ensure!(
        tick.details["queued_action_count"].as_u64() == Some(expected_trigger_reasons.len() as u64),
        "{case_name} queued_action_count mismatch"
    );
    let report_delta = after["cfCounts"][context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS]
        .as_u64()
        .context("after heal report count")?
        - before["cfCounts"][context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS]
            .as_u64()
            .context("before heal report count")?;
    ensure!(
        report_delta == expected_trigger_reasons.len() as u64,
        "{case_name} heal report delta mismatch"
    );
    Ok(json!({
        "case": case_name,
        "pass": true,
        "trigger": "SchedulerState::run_tick(TickName::TelemetryFeedback)",
        "before": before,
        "after": after,
        "tick_record": tick,
        "expected_trigger_reasons": expected_trigger_reasons,
        "telemetry_feedback_reports": after_reports,
        "pending_promotions": pending,
        "readback_equal": true
    }))
}

fn telemetry_window(
    window_id: &str,
    captured_at_unix_ms: i64,
    conformal_ece: f32,
    prediction_oracle_agreement: f32,
) -> ProductionTelemetryWindow {
    ProductionTelemetryWindow {
        window_id: window_id.to_string(),
        captured_at_unix_ms,
        corpus_sha: format!("synthetic-corpus-{window_id}"),
        sample_count: 240,
        conformal_ece,
        ood_auc: 0.98,
        gtau_pass_rate: 0.99,
        prediction_oracle_agreement,
        ship_gate_target: 0.95,
        ship_gate_passed: prediction_oracle_agreement >= 0.95,
        latency: LatencyTelemetry {
            p50_ms: 10.0,
            p95_ms: 20.0,
            p99_ms: 30.0,
            max_ms: 40.0,
        },
        vram_free_bytes: 24_000_000_000,
        vram_total_bytes: 48_000_000_000,
        vram_required_bytes: 16_000_000_000,
        heal_ticker_telemetry_total: HealTickerTelemetrySnapshot::default(),
        source: "phase-g-telemetry-feedback-fsv".to_string(),
    }
}

fn physical_state(db: &DB, status_path: &Path) -> Result<Value> {
    let mut cf_counts = BTreeMap::new();
    for cf in [
        context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY,
        context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS,
        context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS,
    ] {
        cf_counts.insert(cf.to_string(), cf_count(db, cf)?);
    }
    Ok(json!({
        "dbPath": "<case>/rocksdb",
        "statusPathExists": status_path.exists(),
        "cfCounts": cf_counts,
        "telemetryFeedbackReports": telemetry_feedback_reports(db)?,
    }))
}

fn cf_count(db: &DB, cf_name: &str) -> Result<u64> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing CF {cf_name}"))?;
    let mut count = 0;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
}

fn open_db(path: &Path) -> Result<Arc<DB>> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
        .into_iter()
        .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(Arc::new(DB::open_cf_descriptors(&opts, path, descriptors)?))
}

fn telemetry_feedback_reports(db: &DB) -> Result<Vec<Value>> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)
        .context("missing CF_MEJEPA_HEAL_REPORTS")?;
    let mut reports = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        if !key.starts_with(b"telemetry-feedback/") {
            continue;
        }
        let report: HealReport = bincode::deserialize(&value)?;
        reports.push(json!({
            "keyHex": hex::encode(key.as_ref()),
            "keyUtf8": String::from_utf8_lossy(key.as_ref()),
            "triggerReason": report.trigger_reason,
            "statusChange": report.status_change,
            "modeWinner": report.mode_winner,
            "coverage": report.mode_a_score.coverage,
            "oracleAgreement": report.mode_a_score.oracle_agreement,
            "numSamples": report.mode_a_score.num_samples,
            "evaluationSummarySha": hex::encode(report.evaluation_summary_sha)
        }));
    }
    reports.sort_by(|left, right| left["keyHex"].as_str().cmp(&right["keyHex"].as_str()));
    Ok(reports)
}

fn current_git_sha() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .context("run git rev-parse")?;
    ensure!(output.status.success(), "git rev-parse failed");
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
