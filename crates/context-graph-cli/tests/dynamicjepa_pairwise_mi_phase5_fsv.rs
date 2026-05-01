//! Phase 5 pairwise MI audit full-state verification.
//!
//! This test uses the real CLI binary, real RocksDB databases, and checked-in
//! DynamicJEPA fixture data. It verifies the audit by reading the RocksDB CFs
//! and the generated CSV/JSON/SVG files after command execution.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

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

fn write_gridworld_subset(root: &Path, temp: &Path, line_count: usize) -> PathBuf {
    let corpus = fs::read_to_string(
        root.join("configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl"),
    )
    .expect("read gridworld corpus");
    let subset = corpus
        .lines()
        .take(line_count)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let path = temp.join(format!("gridworld_pairwise_mi_{line_count}.jsonl"));
    fs::write(&path, subset).expect("write gridworld subset");
    path
}

fn write_degenerate_gridworld(root: &Path, temp: &Path, line_count: usize) -> PathBuf {
    let corpus = fs::read_to_string(
        root.join("configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl"),
    )
    .expect("read gridworld corpus");
    let first = corpus
        .lines()
        .next()
        .expect("gridworld corpus has first line");
    let text = (0..line_count)
        .map(|_| first)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let path = temp.join(format!(
        "gridworld_pairwise_mi_degenerate_{line_count}.jsonl"
    ));
    fs::write(&path, text).expect("write degenerate gridworld subset");
    path
}

fn setup_gridworld(db: &Path, fixture: &Path) {
    let root = repo_root();
    run_ok(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        db.to_str().expect("db utf8"),
        "--file",
        root.join("configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml")
            .to_str()
            .expect("domain utf8"),
        "--json",
    ]);
    run_ok(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        db.to_str().expect("db utf8"),
        "--domain",
        "gridworld_5x5",
        "--adapter",
        "json_event",
        "--file",
        fixture.to_str().expect("fixture utf8"),
        "--json",
    ]);
    run_ok(&[
        "dynamicjepa",
        "materialize-panel",
        "--db",
        db.to_str().expect("db utf8"),
        "--all-pending",
        "--domain",
        "gridworld_5x5",
        "--json",
    ]);
}

#[test]
fn dynamicjepa_pairwise_mi_audit_fsv_real_rocksdb() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let root = repo_root();
    let db = temp.path().join("gridworld_pairwise_mi_rocksdb");
    let fixture = write_gridworld_subset(&root, temp.path(), 60);
    let output_root = temp.path().join("pairwise_mi_out");
    setup_gridworld(&db, &fixture);

    let before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        db.to_str().expect("db utf8"),
        "--json",
    ]);
    println!(
        "FSV-PHASE5-MI happy BEFORE counts: {}",
        serde_json::to_string(&before["counts"]).expect("counts json")
    );
    assert_eq!(count(&before, "dj_normalized_states"), 60);
    assert_eq!(count(&before, "dj_instrument_readings"), 180);
    assert_eq!(count(&before, "dj_pairwise_readings"), 180);

    let audited = run_ok(&[
        "dynamicjepa",
        "audit-pairwise-mi",
        "--db",
        db.to_str().expect("db utf8"),
        "--domain",
        "gridworld_5x5",
        "--sample-size",
        "50",
        "--ksg-k",
        "3",
        "--bootstrap-iters",
        "16",
        "--seed",
        "20260501",
        "--output-dir",
        output_root.to_str().expect("output utf8"),
        "--json",
    ]);
    println!(
        "FSV-PHASE5-MI happy AFTER summary: {}",
        serde_json::to_string(&audited["summary"]).expect("summary json")
    );
    assert_eq!(audited["summary"]["domain"], "gridworld_5x5");
    assert_eq!(audited["summary"]["n_panel_slots"], 3);
    assert_eq!(audited["summary"]["n_pairs"], 3);
    assert_eq!(
        audited["decoded_records"]["pairwise_reading_rows_verified"],
        150
    );
    assert_eq!(audited["count_delta"]["dj_audit_log"], 1);
    assert!(output_root.join("pairwise_mi_matrix.csv").is_file());
    assert!(output_root
        .join("paper_tables/table_pairwise_mi_summary.csv")
        .is_file());
    assert!(output_root.join("pairwise_mi_heatmap.svg").is_file());
    assert!(output_root.join("pairwise_mi_audit.json").is_file());
    let matrix = fs::read_to_string(output_root.join("pairwise_mi_matrix.csv"))
        .expect("read pairwise matrix");
    assert_eq!(matrix.lines().count(), 4);
    let audit_json: Value = serde_json::from_str(
        &fs::read_to_string(output_root.join("pairwise_mi_audit.json")).unwrap(),
    )
    .expect("parse pairwise audit JSON");
    assert_eq!(audit_json["estimator"]["sample_size"], 50);
    assert_eq!(
        audit_json["matrix"].as_array().expect("matrix rows").len(),
        3
    );
    println!(
        "FSV-PHASE5-MI happy MATRIX first row: {}",
        matrix.lines().nth(1).expect("matrix first data row")
    );

    let audit_log = run_ok(&[
        "dynamicjepa",
        "inspect-cf",
        "--db",
        db.to_str().expect("db utf8"),
        "--cf",
        "dj_audit_log",
        "--limit",
        "400",
        "--json",
    ]);
    assert!(audit_log["decoded_records"]["rows"]
        .as_array()
        .expect("audit rows")
        .iter()
        .any(|row| row["decoded"]["operation"] == "audit_pairwise_mi"));
    println!(
        "FSV-PHASE5-MI happy AUDIT rows: {}",
        audit_log["decoded_records"]["rows"]
            .as_array()
            .expect("audit rows")
            .len()
    );

    let output_exists_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        db.to_str().expect("db utf8"),
        "--json",
    ]);
    let output_exists = run_err(&[
        "dynamicjepa",
        "audit-pairwise-mi",
        "--db",
        db.to_str().expect("db utf8"),
        "--domain",
        "gridworld_5x5",
        "--sample-size",
        "50",
        "--ksg-k",
        "3",
        "--bootstrap-iters",
        "16",
        "--output-dir",
        output_root.to_str().expect("output utf8"),
        "--json",
    ]);
    println!(
        "FSV-PHASE5-MI edge output-dir BEFORE={} AFTER={} ERROR={}",
        output_exists_before["counts"], output_exists["after_counts"], output_exists["error_code"]
    );
    assert_eq!(output_exists["error_code"], "MI_AUDIT_OUTPUT_DIR_EXISTS");
    assert_eq!(
        output_exists["after_counts"]["dj_audit_log"],
        output_exists_before["counts"]["dj_audit_log"]
    );

    let small_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        db.to_str().expect("db utf8"),
        "--json",
    ]);
    let small = run_err(&[
        "dynamicjepa",
        "audit-pairwise-mi",
        "--db",
        db.to_str().expect("db utf8"),
        "--domain",
        "gridworld_5x5",
        "--sample-size",
        "49",
        "--ksg-k",
        "3",
        "--bootstrap-iters",
        "16",
        "--output-dir",
        temp.path()
            .join("pairwise_mi_too_small")
            .to_str()
            .expect("output utf8"),
        "--json",
    ]);
    println!(
        "FSV-PHASE5-MI edge sample-size BEFORE={} AFTER={} ERROR={}",
        small_before["counts"], small["after_counts"], small["error_code"]
    );
    assert_eq!(small["error_code"], "MI_AUDIT_SAMPLE_SIZE_TOO_SMALL");
    assert_eq!(
        small["after_counts"]["dj_audit_log"],
        small_before["counts"]["dj_audit_log"]
    );

    let degenerate_db = temp.path().join("gridworld_pairwise_mi_degenerate_rocksdb");
    let degenerate_fixture = write_degenerate_gridworld(&root, temp.path(), 50);
    setup_gridworld(&degenerate_db, &degenerate_fixture);
    let degenerate_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        degenerate_db.to_str().expect("db utf8"),
        "--json",
    ]);
    let degenerate = run_err(&[
        "dynamicjepa",
        "audit-pairwise-mi",
        "--db",
        degenerate_db.to_str().expect("db utf8"),
        "--domain",
        "gridworld_5x5",
        "--sample-size",
        "50",
        "--ksg-k",
        "3",
        "--bootstrap-iters",
        "16",
        "--output-dir",
        temp.path()
            .join("pairwise_mi_degenerate")
            .to_str()
            .expect("output utf8"),
        "--json",
    ]);
    println!(
        "FSV-PHASE5-MI edge degenerate BEFORE={} AFTER={} ERROR={}",
        degenerate_before["counts"], degenerate["after_counts"], degenerate["error_code"]
    );
    assert_eq!(degenerate["error_code"], "MI_AUDIT_DEGENERATE_INPUT");
    assert_eq!(
        degenerate["after_counts"]["dj_audit_log"],
        degenerate_before["counts"]["dj_audit_log"]
    );
}
