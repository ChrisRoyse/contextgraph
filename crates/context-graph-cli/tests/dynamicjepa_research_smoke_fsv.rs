//! 5090 ME-JEPA research-smoke bundle full-state verification.
//!
//! This invokes the real CLI binary, real RocksDB stores, the real O*NET-backed
//! career smoke source, real CUDA training, and then independently inspects the
//! persisted bundle and DB source-of-truth state.

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
        .env("CUDA_COMPUTE_CAP", "120")
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

fn run_err(args: &[&str]) -> (String, String) {
    let output = Command::new(context_graph_bin())
        .args(args)
        .env("CONTEXT_GRAPH_ENV", "development")
        .env("CUDA_COMPUTE_CAP", "120")
        .output()
        .expect("execute context-graph binary");
    let stdout = String::from_utf8(output.stdout).expect("CLI stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("CLI stderr is UTF-8");
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nargs={args:?}\nstdout={stdout}\nstderr={stderr}"
    );
    (stdout, stderr)
}

fn count(value: &Value, cf: &str) -> u64 {
    value["counts"][cf]
        .as_u64()
        .unwrap_or_else(|| panic!("missing count for {cf}: {value:#}"))
}

#[test]
fn research_smoke_fails_closed_for_paper_purpose_without_onet_source() {
    let root = repo_root();
    let run_root = root.join("tmp/5090jepa_evidence/research_smoke/test_runtime");
    fs::create_dir_all(&run_root).expect("create research smoke test runtime root");
    let run_id = format!("test_research_smoke_no_onet_{}", std::process::id());
    let run_dir = run_root.join(&run_id);
    if run_dir.exists() {
        fs::remove_dir_all(&run_dir).expect("remove previous research smoke test run");
    }

    let (_stdout, stderr) = run_err(&[
        "dynamicjepa",
        "research-smoke",
        "--run-id",
        &run_id,
        "--run-root",
        run_root.to_str().expect("run root utf8"),
        "--fixtures-root",
        root.join("configs/dynamicjepa")
            .to_str()
            .expect("fixtures path utf8"),
        "--purpose",
        "paper_small",
        "--json",
    ]);
    assert!(
        stderr.contains("requires --onet-text-zip"),
        "missing O*NET remediation in stderr: {stderr}"
    );
    assert!(
        !run_dir.exists(),
        "paper_small without O*NET source must fail before creating bundle source of truth"
    );
}

#[test]
fn research_smoke_fails_closed_for_custom_event_count_without_onet_source() {
    let root = repo_root();
    let run_root = root.join("tmp/5090jepa_evidence/research_smoke/test_runtime");
    fs::create_dir_all(&run_root).expect("create research smoke test runtime root");
    let run_id = format!(
        "test_research_smoke_event_count_without_onet_{}",
        std::process::id()
    );
    let run_dir = run_root.join(&run_id);
    if run_dir.exists() {
        fs::remove_dir_all(&run_dir).expect("remove previous research smoke test run");
    }

    let (_stdout, stderr) = run_err(&[
        "dynamicjepa",
        "research-smoke",
        "--run-id",
        &run_id,
        "--run-root",
        run_root.to_str().expect("run root utf8"),
        "--fixtures-root",
        root.join("configs/dynamicjepa")
            .to_str()
            .expect("fixtures path utf8"),
        "--career-event-count",
        "120",
        "--json",
    ]);
    assert!(
        stderr.contains("supply --onet-text-zip"),
        "missing custom-count remediation in stderr: {stderr}"
    );
    assert!(
        !run_dir.exists(),
        "custom career-event-count without O*NET source must fail before creating bundle source of truth"
    );
}

#[test]
fn research_smoke_persists_paper_bundle_and_db_readbacks() {
    let root = repo_root();
    let run_root = root.join("tmp/5090jepa_evidence/research_smoke/test_runtime");
    fs::create_dir_all(&run_root).expect("create research smoke test runtime root");
    let run_id = format!("test_research_smoke_{}", std::process::id());
    let run_dir = run_root.join(&run_id);
    if run_dir.exists() {
        fs::remove_dir_all(&run_dir).expect("remove previous research smoke test run");
    }
    let onet_zip = root.join("tmp/onet_probe/db_30_2_text.zip");
    assert!(
        onet_zip.exists(),
        "real O*NET 30.2 text ZIP is required for research smoke FSV: {}",
        onet_zip.display()
    );

    let result = run_ok(&[
        "dynamicjepa",
        "research-smoke",
        "--run-id",
        &run_id,
        "--run-root",
        run_root.to_str().expect("run root utf8"),
        "--fixtures-root",
        root.join("configs/dynamicjepa")
            .to_str()
            .expect("fixtures path utf8"),
        "--onet-text-zip",
        onet_zip.to_str().expect("O*NET ZIP path utf8"),
        "--system-specs",
        root.join("docs2/systemspecs.md")
            .to_str()
            .expect("system specs path utf8"),
        "--json",
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(result["summary"]["training_metric_rows"], 3);
    assert_eq!(result["summary"]["artifact_hash_rows"], 9);
    assert_eq!(result["summary"]["edge_case_rows"], 11);
    assert_eq!(result["summary"]["ablation_rows"], 6);

    let manifest: Value = serde_json::from_slice(
        &fs::read(run_dir.join("run_manifest.json")).expect("read run manifest"),
    )
    .expect("parse run manifest");
    assert_eq!(manifest["status"], "passed");

    let fsv: Value = serde_json::from_slice(
        &fs::read(run_dir.join("fsv_report.json")).expect("read fsv report"),
    )
    .expect("parse fsv report");
    assert_eq!(fsv["status"], "passed");
    assert_eq!(
        fsv["readbacks"]["counter_world"]["after_counts"]["dj_surprise_events"],
        1
    );
    assert_eq!(
        fsv["readbacks"]["career_taxonomy"]["after_counts"]["dj_model_artifacts"],
        1
    );
    assert_eq!(
        fsv["readbacks"]["career_taxonomy_ablation_A5_no_vicreg"]["after_counts"]["dj_plan_traces"],
        1
    );
    assert_eq!(
        fsv["readbacks"]["career_taxonomy_ablation_A4_pairwise"]["after_counts"]
            ["dj_instrument_registry"],
        12
    );

    for table in [
        "table_runtime.csv",
        "table_prediction_metrics.csv",
        "table_ablation.csv",
        "table_auditability.csv",
        "table_edge_cases.csv",
        "table_data_sources.csv",
        "table_domain_runtime_coverage.csv",
        "table_single_gpu_feasibility.csv",
    ] {
        assert!(
            run_dir.join("paper_tables").join(table).exists(),
            "missing generated paper table {table}"
        );
    }
    for plot in [
        "prediction_val_mse.svg",
        "career_ablation_val_mse.svg",
        "plot_manifest.json",
    ] {
        assert!(
            run_dir.join("plots").join(plot).exists(),
            "missing generated plot artifact {plot}"
        );
    }

    let career_db = run_dir
        .join("dbs")
        .join(format!("{run_id}_career_taxonomy_rocksdb"));
    let counts = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        career_db.to_str().expect("career db path utf8"),
        "--json",
    ]);
    assert_eq!(count(&counts, "dj_verification_runs"), 1);
    assert_eq!(count(&counts, "dj_model_artifacts"), 1);
    assert_eq!(count(&counts, "dj_predictions"), 6);
    assert_eq!(count(&counts, "dj_guard_decisions"), 5);

    let repro: Value = serde_json::from_slice(
        &fs::read(run_dir.join("reproducibility.json")).expect("read reproducibility report"),
    )
    .expect("parse reproducibility report");
    assert_eq!(repro["status"], "passed");
    assert_eq!(repro["metric_deltas"]["val_latent_mse"], 0.0);

    let ablations: Value = serde_json::from_slice(
        &fs::read(run_dir.join("ablation_results.json")).expect("read ablations"),
    )
    .expect("parse ablations");
    let ids = ablations
        .as_array()
        .expect("ablation_results is array")
        .iter()
        .map(|row| row["ablation_id"].as_str().expect("ablation id"))
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "A1_taxonomy_skill_action_vicreg",
            "A0_taxonomy_only_action_vicreg",
            "A2_taxonomy_skill_transition_action_vicreg",
            "A3_taxonomy_skill_transition_no_action_vicreg",
            "A4_taxonomy_skill_transition_pairwise_action_vicreg",
            "A5_taxonomy_skill_transition_action_no_vicreg",
        ]
    );
}
