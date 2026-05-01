//! Phase 12 DynamicJEPA MC-ratio full-state verification.
//!
//! These tests use the real CLI binary, real RocksDB databases, and checked-in
//! deterministic fixtures. Edge cases intentionally corrupt the real audit CF
//! after a successful run, then verify compute-mc-ratio fails typed instead of
//! masking the broken source of truth.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use context_graph_storage::dynamicjepa::column_families::CF_DJ_AUDIT_LOG;
use context_graph_storage::dynamicjepa::{put_audit_record, AuditStatus, DjAuditRecord};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use serde_json::Value;
use uuid::Uuid;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn context_graph_bin() -> &'static str {
    env!("CARGO_BIN_EXE_context-graph")
}

fn run_cli(args: &[&str]) -> (bool, Value) {
    let output = Command::new(context_graph_bin())
        .args(args)
        .env("CONTEXT_GRAPH_ENV", "development")
        .output()
        .expect("execute context-graph binary");
    let stdout = String::from_utf8(output.stdout).expect("CLI stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("CLI stderr is UTF-8");
    assert!(
        !stdout.trim().is_empty(),
        "CLI produced empty stdout; stderr={stderr}"
    );
    let value: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("stdout must be one JSON object: {err}\nstdout={stdout}"));
    (output.status.success(), value)
}

fn run_ok(args: &[&str]) -> Value {
    let (success, value) = run_cli(args);
    assert!(
        success,
        "command expected success but failed:\n{}",
        serde_json::to_string_pretty(&value).expect("pretty JSON")
    );
    assert_eq!(value["status"], "ok");
    value
}

fn run_err(args: &[&str]) -> Value {
    let (success, value) = run_cli(args);
    assert!(
        !success,
        "command expected failure but succeeded:\n{}",
        serde_json::to_string_pretty(&value).expect("pretty JSON")
    );
    assert_eq!(value["status"], "error");
    value
}

fn count(value: &Value, cf: &str) -> u64 {
    value
        .get("counts")
        .or_else(|| value.get("after_counts"))
        .and_then(|counts| counts.get(cf))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing count for {cf}: {value:#}"))
}

fn setup_counter_bundle(db: &Path) {
    let root = repo_root();
    let domain = root.join("configs/dynamicjepa/domain_packs/counter_world.v1.toml");
    let happy = root.join("configs/dynamicjepa/verification/counter_world_happy_path.jsonl");
    run_ok(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        db.to_str().expect("db utf8"),
        "--file",
        domain.to_str().expect("domain utf8"),
        "--json",
    ]);
    run_ok(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        db.to_str().expect("db utf8"),
        "--domain",
        "counter_world",
        "--adapter",
        "json_event",
        "--file",
        happy.to_str().expect("fixture utf8"),
        "--json",
    ]);
    run_ok(&[
        "dynamicjepa",
        "materialize-panel",
        "--db",
        db.to_str().expect("db utf8"),
        "--all-pending",
        "--domain",
        "counter_world",
        "--json",
    ]);
}

fn insert_audit_row(db: &Path, operation: &str, signal_yield: u32) {
    let config = TeleologicalStoreConfig {
        create_if_missing: false,
        create_missing_column_families: false,
        ..TeleologicalStoreConfig::default()
    };
    let store = RocksDbTeleologicalStore::open_with_config(db, config).expect("open RocksDB");
    let timestamp_unix_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos() as u64;
    let record = DjAuditRecord {
        audit_id: Uuid::new_v4(),
        timestamp_unix_nanos,
        operation: operation.to_string(),
        actor: "phase12_fsv_test".to_string(),
        input_ids: Vec::new(),
        output_ids: Vec::new(),
        cfs_touched: vec![CF_DJ_AUDIT_LOG.to_string()],
        content_hashes: Vec::new(),
        status: AuditStatus::Ok,
        verification_run_id: None,
        signal_yield,
    };
    put_audit_record(store.dynamicjepa_db(), &record).expect("insert audit row");
}

#[test]
fn dynamicjepa_phase12_compute_mc_ratio_and_edges() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let db = temp.path().join("counter_mc_ratio_rocksdb");
    let output_root = temp.path().join("mc_ratio_out");
    setup_counter_bundle(&db);

    let before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        db.to_str().expect("db utf8"),
        "--json",
    ]);
    assert_eq!(count(&before, "dj_audit_log"), 16);
    println!(
        "FSV12 happy BEFORE counts: {}",
        serde_json::to_string(&before["counts"]).expect("counts json")
    );

    let computed = run_ok(&[
        "dynamicjepa",
        "compute-mc-ratio",
        "--db",
        db.to_str().expect("db utf8"),
        "--domain",
        "counter_world",
        "--output-dir",
        output_root.to_str().expect("output utf8"),
        "--json",
    ]);
    assert_eq!(computed["summary"]["n_events"], 3);
    assert_eq!(computed["summary"]["total_signals"], 21);
    assert_eq!(computed["summary"]["mc_ratio_measured"], 7.0);
    println!(
        "FSV12 happy AFTER summary: {}",
        serde_json::to_string(&computed["summary"]).expect("summary json")
    );
    assert!(output_root
        .join("paper_tables/table_mc_ratio.csv")
        .is_file());
    assert!(output_root.join("mc_ratio_per_stage.json").is_file());
    assert!(output_root
        .join("plots/signal_density_per_event_distribution.svg")
        .is_file());
    let table = fs::read_to_string(output_root.join("paper_tables/table_mc_ratio.csv"))
        .expect("read table");
    assert!(table.contains("counter_world,2,10,15,1,3,21"));
    let breakdown: Value = serde_json::from_str(
        &fs::read_to_string(output_root.join("mc_ratio_per_stage.json")).unwrap(),
    )
    .expect("parse breakdown");
    assert_eq!(
        breakdown["reports"][0]["stage_rows"]
            .as_array()
            .expect("stage rows")
            .iter()
            .find(|row| row["operation"] == "materialize_pairwise")
            .expect("pairwise stage")["total_signal_yield"],
        3
    );
    println!(
        "FSV12 happy TABLE row: {}",
        table.lines().nth(1).expect("table row")
    );

    let zero_db = temp.path().join("zero_event_rocksdb");
    let root = repo_root();
    let domain = root.join("configs/dynamicjepa/domain_packs/counter_world.v1.toml");
    run_ok(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        zero_db.to_str().expect("db utf8"),
        "--file",
        domain.to_str().expect("domain utf8"),
        "--json",
    ]);
    let zero_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        zero_db.to_str().expect("db utf8"),
        "--json",
    ]);
    println!(
        "FSV12 zero-event BEFORE counts: {}",
        serde_json::to_string(&zero_before["counts"]).expect("counts json")
    );
    let zero_err = run_err(&[
        "dynamicjepa",
        "compute-mc-ratio",
        "--db",
        zero_db.to_str().expect("db utf8"),
        "--domain",
        "counter_world",
        "--output-dir",
        temp.path().join("zero_out").to_str().expect("out utf8"),
        "--json",
    ]);
    assert_eq!(zero_err["error_code"], "SIGNAL_YIELD_EVENT_COUNT_MISMATCH");
    let zero_after = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        zero_db.to_str().expect("db utf8"),
        "--json",
    ]);
    println!(
        "FSV12 zero-event AFTER error={} counts={}",
        zero_err["error_code"],
        serde_json::to_string(&zero_after["counts"]).expect("counts json")
    );

    let unknown_db = temp.path().join("unknown_op_rocksdb");
    setup_counter_bundle(&unknown_db);
    let unknown_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        unknown_db.to_str().expect("db utf8"),
        "--json",
    ]);
    insert_audit_row(&unknown_db, "phase12_unknown_operation", 0);
    let unknown_err = run_err(&[
        "dynamicjepa",
        "compute-mc-ratio",
        "--db",
        unknown_db.to_str().expect("db utf8"),
        "--domain",
        "counter_world",
        "--output-dir",
        temp.path().join("unknown_out").to_str().expect("out utf8"),
        "--json",
    ]);
    assert_eq!(unknown_err["error_code"], "SIGNAL_YIELD_UNKNOWN_OPERATION");
    let unknown_after = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        unknown_db.to_str().expect("db utf8"),
        "--json",
    ]);
    println!(
        "FSV12 unknown-op BEFORE audit={} AFTER error={} audit={}",
        count(&unknown_before, "dj_audit_log"),
        unknown_err["error_code"],
        count(&unknown_after, "dj_audit_log")
    );

    let drift_db = temp.path().join("drift_rocksdb");
    setup_counter_bundle(&drift_db);
    let drift_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        drift_db.to_str().expect("db utf8"),
        "--json",
    ]);
    insert_audit_row(&drift_db, "ingest_event", 99);
    let drift_err = run_err(&[
        "dynamicjepa",
        "compute-mc-ratio",
        "--db",
        drift_db.to_str().expect("db utf8"),
        "--domain",
        "counter_world",
        "--output-dir",
        temp.path().join("drift_out").to_str().expect("out utf8"),
        "--json",
    ]);
    assert_eq!(drift_err["error_code"], "SIGNAL_YIELD_TABLE_DRIFT");
    let drift_after = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        drift_db.to_str().expect("db utf8"),
        "--json",
    ]);
    println!(
        "FSV12 drift BEFORE audit={} AFTER error={} audit={}",
        count(&drift_before, "dj_audit_log"),
        drift_err["error_code"],
        count(&drift_after, "dj_audit_log")
    );
}
