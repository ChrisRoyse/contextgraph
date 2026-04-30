//! 5090 ME-JEPA research-smoke bundle full-state verification.
//!
//! This invokes the real CLI binary, real RocksDB stores, real checked-in
//! DynamicJEPA fixtures, real CUDA training, and then independently inspects the
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

fn count(value: &Value, cf: &str) -> u64 {
    value["counts"][cf]
        .as_u64()
        .unwrap_or_else(|| panic!("missing count for {cf}: {value:#}"))
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
        "--json",
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(result["summary"]["training_metric_rows"], 3);
    assert_eq!(result["summary"]["artifact_hash_rows"], 3);
    assert_eq!(result["summary"]["edge_case_rows"], 11);
    assert_eq!(result["summary"]["ablation_rows"], 1);

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
        fsv["readbacks"]["career_taxonomy_novicreg"]["after_counts"]["dj_plan_traces"],
        1
    );

    for table in [
        "table_runtime.csv",
        "table_prediction_metrics.csv",
        "table_ablation.csv",
        "table_auditability.csv",
        "table_edge_cases.csv",
        "table_data_sources.csv",
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
}
