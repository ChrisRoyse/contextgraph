use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::eval::{
    training_holdout_distribution_drift, LatencyTelemetry, ProductionTelemetryWindow,
    RocksDbEvalStore, TrainingHoldoutDistributionDrift, TRAINING_HOLDOUT_DRIFT_ALERT_CODE,
    TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES,
};
use context_graph_mejepa::heal::{
    read_status_snapshot, write_json_atomic_readback, HealReport, SchedulerState, SelfOptimConfig,
    TickName,
};
use context_graph_mejepa::system_cost::HealTickerTelemetrySnapshot;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde_json::{json, Value};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-training-holdout-drift-fsv";
const REPORT_NAME: &str = "training_holdout_drift_fsv.json";

fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT);
    if root.exists() {
        fs::remove_dir_all(&root).with_context(|| format!("remove {}", root.display()))?;
    }
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;

    let matched = training_holdout_distribution_drift(
        &two_cell_counts(50, 50),
        &two_cell_counts(50, 50),
        TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES,
    )?;
    let skewed_sustained = training_holdout_distribution_drift(
        &two_cell_counts(99, 1),
        &two_cell_counts(50, 50),
        TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES,
    )?;
    let skewed_not_sustained = training_holdout_distribution_drift(
        &two_cell_counts(99, 1),
        &two_cell_counts(50, 50),
        TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES - 1,
    )?;

    let cases = [
        run_scheduler_case(
            &root,
            "skewed_sustained_alert_fires",
            Some(skewed_sustained),
            &["training_holdout_distribution_drift"],
            "telemetry_feedback_actions_queued",
            true,
        )?,
        run_scheduler_case(
            &root,
            "matched_distribution_no_alert",
            Some(matched),
            &[],
            "telemetry_feedback_within_thresholds",
            false,
        )?,
        run_scheduler_case(
            &root,
            "skewed_but_99_batches_no_alert",
            Some(skewed_not_sustained),
            &[],
            "telemetry_feedback_within_thresholds",
            false,
        )?,
        run_invalid_window_case(&root)?,
    ];

    let report_path = root.join(REPORT_NAME);
    let report = json!({
        "schema_version": 1,
        "task_id": "TASK-G-017",
        "build_release_sha": current_git_sha()?,
        "fsv_root": root,
        "source_of_truth": {
            "rocksdb_column_families": [
                context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY,
                context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS
            ],
            "alert_code": TRAINING_HOLDOUT_DRIFT_ALERT_CODE,
            "kl_threshold": context_graph_mejepa::eval::TRAINING_HOLDOUT_DRIFT_KL_THRESHOLD,
            "required_sustained_batches": TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES,
            "report_path": report_path
        },
        "happy_path": cases[0],
        "boundary_cases": &cases[1..],
        "case_count": cases.len(),
        "all_passed": cases.iter().all(|case| case["pass"] == json!(true)),
        "readback_equal": true,
        "physical_artifacts": physical_artifacts(&root)?
    });
    write_json_atomic_readback(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    let canonical_report: Value = serde_json::from_slice(&serde_json::to_vec(&report)?)?;
    ensure!(
        readback == canonical_report,
        "training/holdout drift FSV readback mismatch"
    );
    ensure!(
        readback["all_passed"] == json!(true),
        "training/holdout drift FSV did not record all_passed=true"
    );
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run_scheduler_case(
    root: &Path,
    case_name: &str,
    drift: Option<TrainingHoldoutDistributionDrift>,
    expected_trigger_reasons: &[&str],
    expected_action: &str,
    expect_alert_code_in_action: bool,
) -> Result<Value> {
    let case_root = root.join(case_name);
    fs::create_dir_all(&case_root).with_context(|| format!("create {}", case_root.display()))?;
    let db_path = case_root.join("rocksdb");
    let status_path = case_root.join("self_optimization_status.json");
    let db = open_db(&db_path)?;
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    let window = telemetry_window(case_name, 1_778_700_000_000, drift);
    let before = physical_state(db.as_ref(), &status_path)?;
    eval_store.persist_production_telemetry(&window)?;
    let loaded = eval_store
        .load_production_telemetry(&window.window_id)?
        .context("production telemetry readback missing after persist")?;
    ensure!(loaded == window, "{case_name} telemetry readback mismatch");

    let config = SelfOptimConfig {
        status_path: status_path.clone(),
        hygiene_archive_root: case_root.join("archive"),
        witness_chain_path: case_root.join("witness-chain.bin"),
        test_outcome_root: case_root.join("test-outcomes"),
        ..SelfOptimConfig::default()
    };
    let mut scheduler = SchedulerState::open(db.clone(), config)?;
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
    let actions = tick.details["actions"]
        .as_array()
        .context("tick actions must be an array")?;
    if expect_alert_code_in_action {
        ensure!(
            actions.iter().any(|action| {
                action["reason"]
                    .as_str()
                    .map(|reason| reason.contains(TRAINING_HOLDOUT_DRIFT_ALERT_CODE))
                    .unwrap_or(false)
            }),
            "{case_name} missing {TRAINING_HOLDOUT_DRIFT_ALERT_CODE} in telemetry action reason"
        );
    } else {
        ensure!(
            actions.is_empty(),
            "{case_name} expected no telemetry actions, got {actions:?}"
        );
    }
    let reports = telemetry_feedback_reports(db.as_ref())?;
    let trigger_reasons = reports
        .iter()
        .map(|report| {
            report["triggerReason"]
                .as_str()
                .context("triggerReason must be string")
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        trigger_reasons == expected_trigger_reasons,
        "{case_name} trigger reasons mismatch: got {trigger_reasons:?}"
    );

    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY)
        .context("missing production telemetry cf")?;
    db.flush_cf(cf)?;
    let reports_cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)
        .context("missing heal reports cf")?;
    db.flush_cf(reports_cf)?;
    drop(scheduler);
    drop(eval_store);
    drop(db);

    let reopened = open_db(&db_path)?;
    let reopened_store = RocksDbEvalStore::new(reopened.clone())?;
    let reopened_window = reopened_store
        .load_production_telemetry(&window.window_id)?
        .context("reopened telemetry window missing")?;
    ensure!(
        reopened_window == window,
        "{case_name} reopened telemetry mismatch"
    );
    let reopened_reports = telemetry_feedback_reports(reopened.as_ref())?;
    ensure!(
        reopened_reports == reports,
        "{case_name} reopened heal reports mismatch"
    );
    let reopened_state = physical_state(reopened.as_ref(), &status_path)?;

    Ok(json!({
        "case": case_name,
        "pass": true,
        "trigger": "RocksDbEvalStore::persist_production_telemetry + SchedulerState::run_tick(TickName::TelemetryFeedback)",
        "before": before,
        "after": after,
        "reopened": reopened_state,
        "window": window,
        "tick_record": tick,
        "telemetry_feedback_reports": reports,
        "expected_trigger_reasons": expected_trigger_reasons,
        "readback_equal": true
    }))
}

fn run_invalid_window_case(root: &Path) -> Result<Value> {
    let case_name = "invalid_empty_distribution_fails_closed";
    let case_root = root.join(case_name);
    fs::create_dir_all(&case_root).with_context(|| format!("create {}", case_root.display()))?;
    let db = open_db(&case_root.join("rocksdb"))?;
    let store = RocksDbEvalStore::new(db.clone())?;
    let invalid = TrainingHoldoutDistributionDrift {
        kl_divergence: 0.0,
        threshold: 0.5,
        sustained_batch_count: TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES,
        required_sustained_batches: TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES,
        alert_fired: false,
        alert_code: None,
        training_distribution: BTreeMap::new(),
        holdout_distribution: BTreeMap::new(),
    };
    let window = telemetry_window(case_name, 1_778_700_000_100, Some(invalid));
    let status_path = case_root.join("self_optimization_status.json");
    let before = physical_state(db.as_ref(), &status_path)?;
    let err = store
        .persist_production_telemetry(&window)
        .expect_err("invalid distribution must fail closed");
    let after = physical_state(db.as_ref(), &status_path)?;
    ensure!(err.code() == "MEJEPA_EVAL_INVALID_INPUT");
    ensure!(
        before["cfCounts"] == after["cfCounts"],
        "invalid window changed CF counts"
    );
    Ok(json!({
        "case": case_name,
        "pass": true,
        "trigger": "persist invalid ProductionTelemetryWindow.training_holdout_distribution_drift",
        "before": before,
        "after": after,
        "error_code": err.code(),
        "error": err.to_string(),
        "readback_equal": true
    }))
}

fn telemetry_window(
    window_id: &str,
    captured_at_unix_ms: i64,
    drift: Option<TrainingHoldoutDistributionDrift>,
) -> ProductionTelemetryWindow {
    ProductionTelemetryWindow {
        window_id: window_id.to_string(),
        captured_at_unix_ms,
        corpus_sha: format!("synthetic-corpus-{window_id}"),
        sample_count: 240,
        conformal_ece: 0.0,
        ood_auc: 0.98,
        gtau_pass_rate: 0.99,
        prediction_oracle_agreement: 0.99,
        ship_gate_target: 0.95,
        ship_gate_passed: true,
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
        training_holdout_distribution_drift: drift,
        source: "phase-g-training-holdout-drift-fsv".to_string(),
    }
}

fn two_cell_counts(left: u64, right: u64) -> BTreeMap<String, u64> {
    BTreeMap::from([
        ("known_good::python".to_string(), left),
        ("compile_error::python".to_string(), right),
    ])
}

fn physical_state(db: &DB, status_path: &Path) -> Result<Value> {
    let mut cf_counts = BTreeMap::new();
    for cf in [
        context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY,
        context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS,
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

fn physical_artifacts(root: &Path) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    collect_sst_files(root, &mut out)?;
    out.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    Ok(out)
}

fn collect_sst_files(path: &Path, out: &mut Vec<Value>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path).with_context(|| format!("read dir {}", path.display()))? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            collect_sst_files(&child, out)?;
        } else if child.extension().and_then(|ext| ext.to_str()) == Some("sst") {
            let metadata = fs::metadata(&child)?;
            out.push(json!({
                "path": child,
                "bytes": metadata.len()
            }));
        }
    }
    Ok(())
}

fn current_git_sha() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .context("run git rev-parse")?;
    ensure!(output.status.success(), "git rev-parse failed");
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
