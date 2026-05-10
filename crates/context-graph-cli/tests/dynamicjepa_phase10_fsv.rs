//! Phase 10 bundled DynamicJEPA verification command smoke test.
//!
//! This test invokes the real CLI binary against real RocksDB paths and real
//! fixtures. It does not mock the storage, trainer, artifact registry, or
//! verification-row persistence path.

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

fn run_ok(args: &[&str]) -> Value {
    let output = Command::new(context_graph_bin())
        .args(args)
        .env("CONTEXT_GRAPH_ENV", "development")
        .output()
        .expect("execute context-graph binary");
    let stdout = String::from_utf8(output.stdout).expect("CLI stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("CLI stderr is UTF-8");
    assert!(
        output.status.success(),
        "command failed\nargs={args:?}\nstdout={stdout}\nstderr={stderr}"
    );
    serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("stdout must be one JSON object: {err}\nstdout={stdout}"))
}

fn count(value: &Value, cf: &str) -> u64 {
    value["counts"][cf]
        .as_u64()
        .unwrap_or_else(|| panic!("missing count for {cf}: {value:#}"))
}

fn run_verify_command(root: &Path, run_root: &Path, command: &str, domain: &str) -> Value {
    let db_path = run_root.join(format!("{domain}_rocksdb"));
    let artifact_root = run_root.join(format!("{domain}_artifacts"));
    let evidence_root = run_root.join(format!("{domain}_evidence"));
    run_ok(&[
        "dynamicjepa",
        command,
        "--db",
        db_path.to_str().expect("db path utf8"),
        "--fixtures-root",
        root.join("configs/dynamicjepa")
            .to_str()
            .expect("fixtures path utf8"),
        "--artifact-root",
        artifact_root.to_str().expect("artifact path utf8"),
        "--evidence-root",
        evidence_root.to_str().expect("evidence path utf8"),
        "--json",
    ])
}

#[test]
fn phase10_verify_commands_persist_real_verification_rows() {
    let root = repo_root();
    let run_root = root.join("tmp/5090jepa_evidence/phase10/test_runtime");
    if run_root.exists() {
        fs::remove_dir_all(&run_root).expect("remove previous phase10 runtime");
    }
    fs::create_dir_all(&run_root).expect("create phase10 runtime");

    let counter = run_verify_command(&root, &run_root, "verify-counter-world", "counter_world");
    assert_eq!(counter["status"], "ok");
    assert_eq!(counter["summary"]["split_counts"]["train"], 41);
    assert_eq!(counter["after_counts"]["dj_verification_runs"], 1);
    assert_eq!(counter["after_counts"]["dj_surprise_events"], 1);

    let career = run_verify_command(
        &root,
        &run_root,
        "verify-career-taxonomy",
        "career_taxonomy",
    );
    assert_eq!(career["status"], "ok");
    assert_eq!(career["summary"]["split_counts"]["train"], 9);
    assert_eq!(career["after_counts"]["dj_verification_runs"], 1);
    assert_eq!(career["after_counts"]["dj_model_artifacts"], 1);
    assert_eq!(career["after_counts"]["dj_plan_traces"], 1);
    assert_eq!(career["after_counts"]["dj_guard_decisions"], 5);

    for (domain, value, expected_edges) in [
        ("counter_world", counter, 3usize),
        ("career_taxonomy", career, 5usize),
    ] {
        let db_path = run_root.join(format!("{domain}_rocksdb"));
        let counts = run_ok(&[
            "dynamicjepa",
            "inspect-counts",
            "--db",
            db_path.to_str().expect("db path utf8"),
            "--json",
        ]);
        assert_eq!(count(&counts, "dj_verification_runs"), 1);
        assert_eq!(
            count(&counts, "dj_model_artifacts"),
            value["after_counts"]["dj_model_artifacts"]
                .as_u64()
                .expect("artifact count")
        );
        let evidence_dir = root.join(
            value["evidence_dir"]
                .as_str()
                .expect("verification evidence_dir"),
        );
        assert!(
            evidence_dir.join("verification_run_record.json").exists(),
            "missing verification evidence for {domain}"
        );
        for edge_idx in 0..expected_edges {
            assert!(
                evidence_dir
                    .join(format!("edge_{edge_idx:02}.json"))
                    .exists(),
                "missing edge {edge_idx} evidence file for {domain}"
            );
        }
    }
}

#[test]
#[ignore = "runs the full 8,000-event gridworld verifier and trains a real model; run explicitly with --ignored when validating the heavy gridworld FSV path"]
fn phase10_verify_gridworld_full_real_fsv() {
    let root = repo_root();
    let run_root = root.join("tmp/5090jepa_evidence/phase10/gridworld_full_runtime");
    if run_root.exists() {
        fs::remove_dir_all(&run_root).expect("remove previous gridworld phase10 runtime");
    }
    fs::create_dir_all(&run_root).expect("create gridworld phase10 runtime");

    let gridworld = run_verify_command(&root, &run_root, "verify-gridworld", "gridworld_5x5");
    assert_eq!(gridworld["status"], "ok");
    assert_eq!(gridworld["summary"]["split_counts"]["train"], 4977);
    assert_eq!(gridworld["after_counts"]["dj_verification_runs"], 1);
    assert_eq!(gridworld["after_counts"]["dj_surprise_events"], 0);

    let db_path = run_root.join("gridworld_5x5_rocksdb");
    let counts = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        db_path.to_str().expect("db path utf8"),
        "--json",
    ]);
    assert_eq!(count(&counts, "dj_verification_runs"), 1);
    assert_eq!(count(&counts, "dj_model_artifacts"), 1);
    let evidence_dir = root.join(
        gridworld["evidence_dir"]
            .as_str()
            .expect("verification evidence_dir"),
    );
    assert!(
        evidence_dir.join("verification_run_record.json").exists(),
        "missing gridworld verification evidence"
    );
    for edge_idx in 0..3 {
        assert!(
            evidence_dir
                .join(format!("edge_{edge_idx:02}.json"))
                .exists(),
            "missing gridworld edge {edge_idx} evidence file"
        );
    }
}
