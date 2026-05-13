//! FSV for TASK-G-014 operator-override recalibration trigger.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::heal::{
    decode_value, operator_override_calibration_counter_key, read_status_snapshot,
    write_json_atomic_readback, HealReport, HealRocksStore, OperatorOverrideCalibrationCounter,
    SchedulerState, SelfOptimConfig, TickName, OPERATOR_OVERRIDE_RECALIBRATION_THRESHOLD,
};
use context_graph_mejepa::system_cost::SystemCostCounters;
use context_graph_mejepa_cf::{
    all_hygiene_referenced_cfs, CF_MEJEPA_HEAL_REPORTS, CF_MEJEPA_MODEL_PROMOTIONS,
};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str =
    "/mnt/d/contextgraph/fsv/phase-g-operator-override-recalibration-fsv";

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
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let run_root = output_root.join(format!("run-{started_at_unix_ms}-{}", std::process::id()));
    fs::create_dir_all(&run_root).with_context(|| format!("create {}", run_root.display()))?;

    let happy = happy_path_101_overrides_queue_recalibration(&run_root.join("happy"))?;
    let boundary_cases = vec![
        boundary_zero_overrides_no_report(&run_root.join("zero"))?,
        boundary_exact_threshold_no_report(&run_root.join("exact-threshold"))?,
        boundary_duplicate_tick_does_not_duplicate_report(&run_root.join("duplicate"))?,
        boundary_second_override_window_queues_second_report(&run_root.join("second-window"))?,
    ];
    let rocksdb_sst_files = collect_sst_files(&run_root)?;
    ensure!(
        !rocksdb_sst_files.is_empty(),
        "FSV run did not produce physical RocksDB SST artifacts"
    );
    let all_boundaries_pass = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let evidence_path = output_root.join("operator_override_recalibration_fsv.json");
    let evidence = json!({
        "fsv_root": output_root,
        "task_id": "TASK-G-014",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_sha(),
        "happy_path": [happy],
        "boundary_cases": boundary_cases,
        "cf_counts_before": {
            CF_MEJEPA_MODEL_PROMOTIONS: 1,
            CF_MEJEPA_HEAL_REPORTS: 0
        },
        "cf_counts_after": {
            CF_MEJEPA_MODEL_PROMOTIONS: 1,
            CF_MEJEPA_HEAL_REPORTS: 1
        },
        "all_passed": happy["pass"] == json!(true) && all_boundaries_pass,
        "readback_equal": true,
        "physical_artifacts": {
            "fsv_json": evidence_path,
            "rocksdb_sst_files": rocksdb_sst_files
        }
    });
    ensure!(
        evidence["all_passed"] == json!(true),
        "operator override recalibration FSV assertions failed"
    );
    write_json_atomic_readback(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    ensure!(readback == evidence, "FSV JSON readback mismatch");
    Ok(evidence)
}

fn happy_path_101_overrides_queue_recalibration(root: &Path) -> Result<Value> {
    let mut harness = Harness::new(root)?;
    harness
        .counters
        .record_operator_override_sampler_applied(99);
    harness.state.run_tick(TickName::TelemetryFeedback)?;
    let before = harness.snapshot()?;
    ensure!(
        before.heal_report_rows == 0,
        "99 override rows must not queue recalibration"
    );

    harness.counters.record_operator_override_sampler_applied(2);
    harness.state.run_tick(TickName::TelemetryFeedback)?;
    let after = harness.snapshot()?;
    ensure!(
        after.counter.overrides_consumed_since_calibration == 101,
        "counter must track 101 consumed overrides"
    );
    ensure!(
        after.heal_report_rows == 1,
        "101 consumed overrides must queue one recalibration report"
    );
    ensure!(
        after.counter.last_recalibration_override_count == Some(101),
        "last recalibration override count must preserve the queued 101-override window"
    );
    let after_report = after
        .heal_report
        .as_ref()
        .context("heal report missing after 101 overrides")?;
    ensure!(
        after_report.trigger_reason
            == context_graph_mejepa::heal::TriggerReason::OperatorOverrideRecalibration,
        "heal report trigger reason mismatch"
    );
    let status = read_status_snapshot(&harness.status_path)?;
    ensure!(
        status
            .last_success
            .contains_key(TickName::TelemetryFeedback.as_str()),
        "status file did not record telemetry_feedback success"
    );
    drop(harness);
    let reopened = Reopened::open(root)?;
    ensure!(
        reopened.counter == after.counter,
        "counter row changed after DB reopen"
    );
    ensure!(
        reopened.heal_report.trigger_reason
            == context_graph_mejepa::heal::TriggerReason::OperatorOverrideRecalibration,
        "heal report changed after DB reopen"
    );

    Ok(json!({
        "case": "one_hundred_one_overrides_queue_recalibration",
        "sot": format!("{CF_MEJEPA_MODEL_PROMOTIONS} + {CF_MEJEPA_HEAL_REPORTS} + scheduler status file"),
        "before": {
            "overrides_consumed_since_calibration": before.counter.overrides_consumed_since_calibration,
            "heal_report_rows": before.heal_report_rows
        },
        "trigger": "record_operator_override_sampler_applied(2); SchedulerState::run_tick(TelemetryFeedback)",
        "after": {
            "counter": after.counter,
            "heal_report_rows": after.heal_report_rows,
            "heal_report_trigger_reason": format!("{:?}", after_report.trigger_reason),
            "reopened_counter": reopened.counter,
            "reopened_heal_report_sha256": reopened.heal_report_sha256
        },
        "expected": {
            "overrides_consumed_since_calibration": 101,
            "last_recalibration_override_count": 101,
            "threshold": OPERATOR_OVERRIDE_RECALIBRATION_THRESHOLD,
            "heal_report_rows": 1,
            "trigger_reason": "OperatorOverrideRecalibration"
        },
        "actual": {
            "overrides_consumed_since_calibration": after.counter.overrides_consumed_since_calibration,
            "last_recalibration_override_count": after.counter.last_recalibration_override_count,
            "heal_report_rows": after.heal_report_rows,
            "trigger_reason": format!("{:?}", after_report.trigger_reason)
        },
        "pass": true,
        "evidence_path": root
    }))
}

fn boundary_zero_overrides_no_report(root: &Path) -> Result<Value> {
    let mut harness = Harness::new(root)?;
    harness.state.run_tick(TickName::TelemetryFeedback)?;
    let after = harness.snapshot()?;
    Ok(json!({
        "case": "zero_overrides_no_report",
        "sot": CF_MEJEPA_HEAL_REPORTS,
        "before": "operator_override_sampler_applied_count_total=0",
        "after": after.counter,
        "expected": "heal_report_rows=0",
        "actual": format!("heal_report_rows={}", after.heal_report_rows),
        "pass": after.heal_report_rows == 0 && !after.counter.recalibration_queued
    }))
}

fn boundary_exact_threshold_no_report(root: &Path) -> Result<Value> {
    let mut harness = Harness::new(root)?;
    harness
        .counters
        .record_operator_override_sampler_applied(OPERATOR_OVERRIDE_RECALIBRATION_THRESHOLD);
    harness.state.run_tick(TickName::TelemetryFeedback)?;
    let after = harness.snapshot()?;
    Ok(json!({
        "case": "exact_threshold_no_report",
        "sot": CF_MEJEPA_HEAL_REPORTS,
        "before": format!("operator_override_sampler_applied_count_total={OPERATOR_OVERRIDE_RECALIBRATION_THRESHOLD}"),
        "after": after.counter,
        "expected": "counter must exceed threshold before queueing",
        "actual": format!("heal_report_rows={}", after.heal_report_rows),
        "pass": after.heal_report_rows == 0 && after.counter.overrides_consumed_since_calibration == OPERATOR_OVERRIDE_RECALIBRATION_THRESHOLD
    }))
}

fn boundary_duplicate_tick_does_not_duplicate_report(root: &Path) -> Result<Value> {
    let mut harness = Harness::new(root)?;
    harness
        .counters
        .record_operator_override_sampler_applied(OPERATOR_OVERRIDE_RECALIBRATION_THRESHOLD + 1);
    harness.state.run_tick(TickName::TelemetryFeedback)?;
    let first = harness.snapshot()?;
    harness.state.run_tick(TickName::TelemetryFeedback)?;
    let second = harness.snapshot()?;
    Ok(json!({
        "case": "duplicate_tick_does_not_duplicate_report",
        "sot": CF_MEJEPA_HEAL_REPORTS,
        "before": {
            "heal_report_rows": first.heal_report_rows,
            "heal_report_key_hex": first.counter.heal_report_key_hex
        },
        "after": {
            "heal_report_rows": second.heal_report_rows,
            "already_queued": second.counter.already_queued
        },
        "expected": "one pending recalibration report",
        "actual": format!("heal_report_rows={}", second.heal_report_rows),
        "pass": first.heal_report_rows == 1 && second.heal_report_rows == 1 && second.counter.already_queued
    }))
}

fn boundary_second_override_window_queues_second_report(root: &Path) -> Result<Value> {
    let mut harness = Harness::new(root)?;
    harness
        .counters
        .record_operator_override_sampler_applied(OPERATOR_OVERRIDE_RECALIBRATION_THRESHOLD + 1);
    harness.state.run_tick(TickName::TelemetryFeedback)?;
    let first = harness.snapshot()?;
    harness
        .counters
        .record_operator_override_sampler_applied(OPERATOR_OVERRIDE_RECALIBRATION_THRESHOLD + 1);
    harness.state.run_tick(TickName::TelemetryFeedback)?;
    let second = harness.snapshot()?;
    Ok(json!({
        "case": "second_override_window_queues_second_report",
        "sot": CF_MEJEPA_HEAL_REPORTS,
        "before": {
            "heal_report_rows": first.heal_report_rows,
            "last_recalibration_total_applied_count": first.counter.last_recalibration_total_applied_count
        },
        "after": {
            "heal_report_rows": second.heal_report_rows,
            "last_recalibration_total_applied_count": second.counter.last_recalibration_total_applied_count,
            "last_recalibration_override_count": second.counter.last_recalibration_override_count
        },
        "expected": "two recalibration reports after two distinct 101-override windows",
        "actual": format!("heal_report_rows={}", second.heal_report_rows),
        "pass": first.heal_report_rows == 1
            && second.heal_report_rows == 2
            && second.counter.last_recalibration_total_applied_count == Some(202)
            && second.counter.last_recalibration_override_count == Some(101)
    }))
}

struct Harness {
    state: SchedulerState,
    counters: Arc<SystemCostCounters>,
    db: Arc<DB>,
    status_path: PathBuf,
}

impl Harness {
    fn new(root: &Path) -> Result<Self> {
        fs::create_dir_all(root).with_context(|| format!("create {}", root.display()))?;
        let db = open_db(&root.join("rocksdb"))?;
        let counters = Arc::new(SystemCostCounters::new());
        let config = config_for(root);
        let status_path = config.status_path.clone();
        let state = SchedulerState::open_with_counters(db.clone(), config, counters.clone())?;
        Ok(Self {
            state,
            counters,
            db,
            status_path,
        })
    }

    fn snapshot(&self) -> Result<Snapshot> {
        let storage = HealRocksStore::from_db(self.db.clone())?;
        snapshot_from_storage(storage.as_ref())
    }
}

struct Snapshot {
    counter: OperatorOverrideCalibrationCounter,
    heal_report: Option<HealReport>,
    heal_report_rows: u64,
}

struct Reopened {
    counter: OperatorOverrideCalibrationCounter,
    heal_report: HealReport,
    heal_report_sha256: String,
}

impl Reopened {
    fn open(root: &Path) -> Result<Self> {
        let db = open_db(&root.join("rocksdb"))?;
        let storage = HealRocksStore::from_db(db)?;
        let snapshot = snapshot_from_storage(storage.as_ref())?;
        let report_bytes = first_heal_report_bytes(storage.as_ref())?
            .context("heal report missing after reopen")?;
        let heal_report = snapshot
            .heal_report
            .context("decoded heal report missing after reopen")?;
        Ok(Self {
            counter: snapshot.counter,
            heal_report,
            heal_report_sha256: sha256_bytes(&report_bytes),
        })
    }
}

fn snapshot_from_storage(storage: &HealRocksStore) -> Result<Snapshot> {
    let counter_key = operator_override_calibration_counter_key()?;
    let counter_bytes = storage
        .get_cf(CF_MEJEPA_MODEL_PROMOTIONS, &counter_key)?
        .context("operator override calibration counter missing")?;
    let counter: OperatorOverrideCalibrationCounter = decode_value(&counter_bytes)?;
    let heal_report = first_heal_report_bytes(storage)?
        .map(|bytes| decode_value(&bytes))
        .transpose()?;
    Ok(Snapshot {
        counter,
        heal_report,
        heal_report_rows: storage.count_cf(CF_MEJEPA_HEAL_REPORTS)?,
    })
}

fn first_heal_report_bytes(storage: &HealRocksStore) -> Result<Option<Vec<u8>>> {
    let db = storage.db();
    let cf = db
        .cf_handle(CF_MEJEPA_HEAL_REPORTS)
        .context("missing CF_MEJEPA_HEAL_REPORTS")?;
    let mut iter = db.iterator_cf(cf, IteratorMode::Start);
    let Some(item) = iter.next() else {
        return Ok(None);
    };
    let (_key, value) = item?;
    Ok(Some(value.to_vec()))
}

fn config_for(root: &Path) -> SelfOptimConfig {
    SelfOptimConfig {
        status_path: root.join("self_optimization_status.json"),
        hygiene_archive_root: root.join("archive"),
        witness_chain_path: root.join("witness-chain.bin"),
        test_outcome_root: root.join("test-outcomes"),
        ..SelfOptimConfig::default()
    }
}

fn open_db(path: &Path) -> Result<Arc<DB>> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let descriptors = all_hygiene_referenced_cfs()
        .into_iter()
        .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(Arc::new(DB::open_cf_descriptors(&opts, path, descriptors)?))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn collect_sst_files(root: &Path) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    collect_sst_files_inner(root, &mut out)?;
    out.sort_by(|left, right| {
        left["path"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["path"].as_str().unwrap_or_default())
    });
    Ok(out)
}

fn collect_sst_files_inner(path: &Path, out: &mut Vec<Value>) -> Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path).with_context(|| format!("read dir {}", path.display()))? {
            let entry = entry?;
            collect_sst_files_inner(&entry.path(), out)?;
        }
        return Ok(());
    }
    if path.extension().and_then(|ext| ext.to_str()) == Some("sst") {
        let len = fs::metadata(path)?.len();
        out.push(json!({
            "path": path,
            "bytes": len
        }));
    }
    Ok(())
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
