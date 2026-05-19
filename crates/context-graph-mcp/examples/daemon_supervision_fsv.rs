use context_graph_mcp::daemon::{
    ensure_mejepa_column_families, run_weekly_eval_job_once, start_supervised_flywheel_tasks,
    FlywheelDaemonConfig, FlywheelJobRunRecord,
};
use context_graph_mcp::daemon_validate::validate_daemon_root;
use context_graph_mejepa::eval::{corpus_sha_from_holdout, synthetic_holdout};
use context_graph_mejepa::{
    CalibrationExample, CalibrationStore, EmbedderId, Language, TrainCertSummary,
};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/mcp-daemon-supervision-fsv";

fn main() -> FsvResult<()> {
    let fsv_root = PathBuf::from(FSV_ROOT);
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&fsv_root)?;
    fs::set_permissions(&fsv_root, fs::Permissions::from_mode(0o700))?;
    let daemon_root = fsv_root.join("daemon-root");
    fs::create_dir_all(&daemon_root)?;
    fs::set_permissions(&daemon_root, fs::Permissions::from_mode(0o700))?;
    std::env::set_var("CONTEXTGRAPH_DATA_ROOT", &daemon_root);

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_fsv(fsv_root, daemon_root))
}

async fn run_fsv(fsv_root: PathBuf, daemon_root: PathBuf) -> FsvResult<()> {
    let paths = validate_daemon_root(&daemon_root)?;
    let repo_root = daemon_root.join("repo");
    fs::create_dir_all(&repo_root)?;
    let db = context_graph_mejepa_hygiene::open_hygiene_rocksdb(&paths.storage_db)?;
    let holdout = seed_weekly_eval_inputs(db.clone(), &paths.root, &repo_root)?;
    let before = physical_state(
        db.as_ref(),
        &paths.storage_db,
        &paths.scheduler_state_dir,
        true,
    )?;

    let config = FlywheelDaemonConfig::from_paths(paths.clone(), repo_root.clone())
        .with_test_scheduler_periods(Duration::from_millis(500), Duration::from_millis(500))?;
    let mut handle = start_supervised_flywheel_tasks(config, db.clone())?;
    let observed = wait_for_scheduled_evidence(
        db.clone(),
        &paths.storage_db,
        &paths.scheduler_state_dir,
        Duration::from_secs(30),
    )
    .await?;
    handle.shutdown().await?;
    flush_all_cfs(db.as_ref())?;
    let after_shutdown = physical_state(
        db.as_ref(),
        &paths.storage_db,
        &paths.scheduler_state_dir,
        true,
    )?;
    drop(handle);
    drop(db);

    let reopened = context_graph_mejepa_hygiene::open_hygiene_rocksdb(&paths.storage_db)?;
    let after_reopen = physical_state(
        reopened.as_ref(),
        &paths.storage_db,
        &paths.scheduler_state_dir,
        true,
    )?;
    require(
        "job run ledger persisted after reopen",
        after_reopen["cf_counts"][context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS]
            .as_u64()
            .unwrap_or_default()
            >= 8,
    )?;
    require(
        "GC report persisted after reopen",
        after_reopen["cf_counts"][context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY]
            .as_u64()
            .unwrap_or_default()
            >= 1,
    )?;
    require(
        "weekly eval report persisted after reopen",
        after_reopen["cf_counts"][context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS]
            .as_u64()
            .unwrap_or_default()
            >= 1,
    )?;
    drop(reopened);

    let boundary_cases = vec![
        edge_missing_job_run_cf(&fsv_root)?,
        edge_zero_scheduler_period(&fsv_root)?,
        edge_missing_weekly_holdout(&fsv_root).await?,
    ];

    let evidence = json!({
        "fsv_root": fsv_root,
        "task_id": "GH-8-production-daemon-supervision",
        "source_of_truth": {
            "db_path": paths.storage_db,
            "job_run_cf": context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS,
            "gc_cf": context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY,
            "eval_cf": context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS,
            "supervisor_status_file": paths.scheduler_state_dir.join("flywheel_supervisor_status.json"),
            "nightly_gc_status_file": paths.scheduler_state_dir.join("nightly_gc_status.json"),
            "weekly_eval_status_file": paths.scheduler_state_dir.join("weekly_eval_status.json"),
        },
        "happy_path": {
            "before_state": before,
            "trigger": "start_supervised_flywheel_tasks with 500ms GC/eval scheduler periods",
            "observed_while_running": observed,
            "after_shutdown_state": after_shutdown,
            "after_reopen_state": after_reopen,
            "holdout_count": holdout.len(),
            "holdout_corpus_sha": corpus_sha_from_holdout(&holdout),
            "passes": true,
        },
        "boundary_cases": boundary_cases,
        "physical_artifacts": rocksdb_file_state(&paths.storage_db)?,
        "passes": true,
    });
    let evidence_path = PathBuf::from(FSV_ROOT).join("daemon-supervision-evidence.json");
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require("evidence JSON readback matches", readback == evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn seed_weekly_eval_inputs(
    db: Arc<DB>,
    daemon_root: &Path,
    repo_root: &Path,
) -> FsvResult<Vec<context_graph_mejepa::eval::HoldoutPanel>> {
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![0.50],
            actual_test_pass: vec![if idx % 2 == 0 { 1.0 } else { 0.0 }],
        })
        .collect::<Vec<_>>();
    calibration.calibrate(
        &examples,
        &[0.01; 40],
        0.10,
        30,
        0.30,
        [8; 32],
        BTreeMap::<EmbedderId, String>::new(),
    )?;
    let train_cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
        .ok_or("missing train cert CF")?;
    let cert = TrainCertSummary {
        step: 1,
        delta_omega: 0.9,
        delta_xi: 0.9,
        witness_offset: 88,
    };
    let cert_bytes = bincode::serialize(&cert)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(
        train_cf,
        b"cert:daemon-supervision-fsv:0001",
        &cert_bytes,
        &opts,
    )?;
    let cert_readback = db
        .get_cf(train_cf, b"cert:daemon-supervision-fsv:0001")?
        .ok_or("train cert readback missing")?;
    require(
        "train cert readback bytes match",
        cert_readback == cert_bytes,
    )?;

    let holdout = synthetic_holdout(repo_root)?;
    let holdout_path = daemon_root.join("state/gold-labels/weekly_holdout_panels.json");
    write_json_checked(&holdout_path, &holdout)?;
    Ok(holdout)
}

async fn wait_for_scheduled_evidence(
    db: Arc<DB>,
    db_path: &Path,
    scheduler_state_dir: &Path,
    timeout: Duration,
) -> FsvResult<Value> {
    let started = Instant::now();
    loop {
        let state = physical_state(db.as_ref(), db_path, scheduler_state_dir, false)?;
        let job_records = state["job_records"].as_array().cloned().unwrap_or_default();
        let has_gc_completion = job_records.iter().any(|record| {
            record["task"] == "nightly_gc_scheduler"
                && record["event"] == "scheduled_run"
                && record["status"] == "completed"
        });
        let has_eval_completion = job_records.iter().any(|record| {
            record["task"] == "weekly_eval_scheduler"
                && record["event"] == "scheduled_run"
                && record["status"] == "completed"
        });
        let gc_count = state["cf_counts"][context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY]
            .as_u64()
            .unwrap_or_default();
        let eval_count = state["cf_counts"][context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS]
            .as_u64()
            .unwrap_or_default();
        if has_gc_completion && has_eval_completion && gc_count > 0 && eval_count > 0 {
            return Ok(state);
        }
        if started.elapsed() > timeout {
            return Err(format!(
                "MEJEPA_DAEMON_SUPERVISION_FSV_TIMEOUT: missing scheduled evidence after {timeout:?}; last_state={}",
                serde_json::to_string_pretty(&state)?
            )
            .into());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn edge_missing_job_run_cf(fsv_root: &Path) -> FsvResult<Value> {
    let path = fsv_root.join("edge-missing-job-run-cf-db");
    let db = open_without_job_run_cf(&path)?;
    let before = json!({
        "cf_names": DB::list_cf(&Options::default(), &path)?,
        "job_run_cf_present": db.cf_handle(context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS).is_some(),
    });
    let err = ensure_mejepa_column_families(&db).expect_err("missing CF must fail closed");
    let after = json!({
        "cf_names": DB::list_cf(&Options::default(), &path)?,
        "job_run_cf_present": db.cf_handle(context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS).is_some(),
    });
    require(
        "missing job-run CF reports MEJEPA_CF_MISSING",
        err.to_string().contains("MEJEPA_CF_MISSING"),
    )?;
    Ok(json!({
        "case": "missing_job_run_cf",
        "source_of_truth": "RocksDB column family list",
        "before_state": before,
        "trigger": "ensure_mejepa_column_families",
        "after_state": after,
        "error": err.to_string(),
        "passes": true,
    }))
}

fn edge_zero_scheduler_period(fsv_root: &Path) -> FsvResult<Value> {
    let root = fsv_root.join("edge-zero-period-root");
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let paths = validate_daemon_root(&root)?;
    let db = context_graph_mejepa_hygiene::open_hygiene_rocksdb(&paths.storage_db)?;
    let before = physical_state(
        db.as_ref(),
        &paths.storage_db,
        &paths.scheduler_state_dir,
        true,
    )?;
    let err = FlywheelDaemonConfig::from_paths(paths.clone(), root.join("repo"))
        .with_test_scheduler_periods(Duration::ZERO, Duration::from_secs(1))
        .expect_err("zero scheduler period must fail closed");
    let after = physical_state(
        db.as_ref(),
        &paths.storage_db,
        &paths.scheduler_state_dir,
        true,
    )?;
    require(
        "zero period does not write job rows",
        before["cf_counts"][context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS]
            == after["cf_counts"][context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS],
    )?;
    Ok(json!({
        "case": "zero_scheduler_period",
        "source_of_truth": context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS,
        "before_state": before,
        "trigger": "FlywheelDaemonConfig::with_test_scheduler_periods(Duration::ZERO, 1s)",
        "after_state": after,
        "error": err.to_string(),
        "passes": true,
    }))
}

async fn edge_missing_weekly_holdout(fsv_root: &Path) -> FsvResult<Value> {
    let root = fsv_root.join("edge-missing-weekly-holdout-root");
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let paths = validate_daemon_root(&root)?;
    let repo = root.join("repo");
    fs::create_dir_all(&repo)?;
    let db = context_graph_mejepa_hygiene::open_hygiene_rocksdb(&paths.storage_db)?;
    seed_calibration_only(db.clone())?;
    let config = FlywheelDaemonConfig::from_paths(paths.clone(), repo);
    let before = physical_state(
        db.as_ref(),
        &paths.storage_db,
        &paths.scheduler_state_dir,
        true,
    )?;
    let err = run_weekly_eval_job_once(&config, db.clone())
        .await
        .expect_err("missing holdout must fail closed");
    let after = physical_state(
        db.as_ref(),
        &paths.storage_db,
        &paths.scheduler_state_dir,
        true,
    )?;
    let job_records = after["job_records"].as_array().cloned().unwrap_or_default();
    let has_error_record = job_records.iter().any(|record| {
        record["task"] == "weekly_eval_scheduler"
            && record["event"] == "scheduled_run"
            && record["status"] == "error"
            && record["error_code"] == "MEJEPA_EVAL_HOLDOUT_UNAVAILABLE"
    });
    require("weekly eval error record persisted", has_error_record)?;
    require(
        "missing holdout does not create eval report",
        before["cf_counts"][context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS]
            == after["cf_counts"][context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS],
    )?;
    Ok(json!({
        "case": "missing_weekly_holdout",
        "source_of_truth": [
            context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS,
            context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS
        ],
        "before_state": before,
        "trigger": "run_weekly_eval_job_once without state/gold-labels/weekly_holdout_panels.json",
        "after_state": after,
        "error": err.to_string(),
        "passes": true,
    }))
}

fn seed_calibration_only(db: Arc<DB>) -> FsvResult<()> {
    let calibration = CalibrationStore::new(db, 30)?;
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![0.50],
            actual_test_pass: vec![if idx % 2 == 0 { 1.0 } else { 0.0 }],
        })
        .collect::<Vec<_>>();
    calibration.calibrate(
        &examples,
        &[0.01; 40],
        0.10,
        30,
        0.30,
        [8; 32],
        BTreeMap::<EmbedderId, String>::new(),
    )?;
    Ok(())
}

fn physical_state(
    db: &DB,
    db_path: &Path,
    scheduler_state_dir: &Path,
    include_rocksdb_files: bool,
) -> FsvResult<Value> {
    let mut counts = serde_json::Map::new();
    for cf in [
        context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS,
        context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY,
        context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS,
        context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
        context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS,
    ] {
        counts.insert(cf.to_string(), json!(count_cf(db, cf)?));
    }
    let rocksdb_files = if include_rocksdb_files {
        rocksdb_file_state(db_path)?
    } else {
        json!({
            "omitted": true,
            "reason": "live RocksDB file manifests are volatile while the daemon is writing; stable file proof is captured after shutdown, flush, and reopen",
        })
    };
    Ok(json!({
        "db_path": db_path,
        "cf_counts": Value::Object(counts),
        "job_records": read_job_run_records(db)?,
        "status_files": read_status_files(scheduler_state_dir)?,
        "rocksdb_files": rocksdb_files,
    }))
}

fn read_job_run_records(db: &DB) -> FsvResult<Vec<Value>> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS)
        .ok_or("missing flywheel job-run CF")?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let record: FlywheelJobRunRecord = serde_json::from_slice(&value)?;
        out.push(serde_json::to_value(record)?);
    }
    Ok(out)
}

fn read_status_files(scheduler_state_dir: &Path) -> FsvResult<Value> {
    let mut out = serde_json::Map::new();
    for name in [
        "flywheel_supervisor_status.json",
        "nightly_gc_status.json",
        "weekly_eval_status.json",
        "self_optimization_status.json",
    ] {
        let path = scheduler_state_dir.join(name);
        let value = if path.exists() {
            serde_json::from_slice::<Value>(&fs::read(&path)?)?
        } else {
            json!({"exists": false})
        };
        out.insert(name.to_string(), value);
    }
    Ok(Value::Object(out))
}

fn count_cf(db: &DB, cf_name: &str) -> FsvResult<u64> {
    let cf = db.cf_handle(cf_name).ok_or(format!("missing {cf_name}"))?;
    let mut count = 0u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

fn flush_all_cfs(db: &DB) -> FsvResult<()> {
    for cf in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
        let handle = db.cf_handle(cf).ok_or(format!("missing {cf}"))?;
        db.flush_cf(handle)?;
    }
    Ok(())
}

fn open_without_job_run_cf(path: &Path) -> FsvResult<DB> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
        .into_iter()
        .filter(|cf| *cf != context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS)
        .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(DB::open_cf_descriptors(&opts, path, descriptors)?)
}

fn rocksdb_file_state(path: &Path) -> FsvResult<Value> {
    let mut files = Vec::new();
    collect_rocksdb_files(path, &mut files)?;
    let total_bytes = files.iter().map(|item| item.1).sum::<u64>();
    let sst_count = files
        .iter()
        .filter(|item| item.0.extension().and_then(|ext| ext.to_str()) == Some("sst"))
        .count();
    Ok(json!({
        "file_count": files.len(),
        "sst_count": sst_count,
        "total_bytes": total_bytes,
        "files": files.into_iter().map(|(path, bytes)| json!({
            "path": path,
            "bytes": bytes
        })).collect::<Vec<_>>()
    }))
}

fn collect_rocksdb_files(path: &Path, out: &mut Vec<(PathBuf, u64)>) -> FsvResult<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        let meta = entry.metadata()?;
        if meta.is_dir() {
            collect_rocksdb_files(&child, out)?;
        } else {
            out.push((child, meta.len()));
        }
    }
    Ok(())
}

fn write_json_checked(path: &Path, value: &impl Serialize) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    require("file byte readback equality", fs::read(path)? == bytes)?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("daemon supervision FSV assertion failed: {label}").into())
    }
}
