//! 5090 ME-JEPA research-smoke bundle full-state verification.
//!
//! This invokes the real CLI binary, real RocksDB stores, the real O*NET-backed
//! career smoke source, real CUDA training, and then independently inspects the
//! persisted bundle and DB source-of-truth state.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

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

fn write_incomplete_onet_zip(path: &Path) {
    let file = fs::File::create(path).expect("create incomplete O*NET ZIP");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    zip.start_file("db_30_2_text/Read Me.txt", options)
        .expect("start readme member");
    zip.write_all(
        b"O*NET 30.2 Database\nThe content of the O*NET 30.2 Database is licensed under a Creative Commons Attribution 4.0 International License.\n",
    )
    .expect("write readme member");
    zip.finish().expect("finish incomplete O*NET ZIP");
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
fn research_smoke_fails_closed_for_career_seed_without_onet_source() {
    let root = repo_root();
    let run_root = root.join("tmp/5090jepa_evidence/research_smoke/test_runtime");
    fs::create_dir_all(&run_root).expect("create research smoke test runtime root");
    let run_id = format!(
        "test_research_smoke_seed_without_onet_{}",
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
        "--career-training-seed",
        "777",
        "--json",
    ]);
    assert!(
        stderr.contains("--career-training-seed was supplied without --onet-text-zip"),
        "missing career seed remediation in stderr: {stderr}"
    );
    assert!(
        !run_dir.exists(),
        "career seed without O*NET source must fail before creating bundle source of truth"
    );
}

#[test]
fn research_smoke_fails_closed_for_ablation_seed_without_onet_source() {
    let root = repo_root();
    let run_root = root.join("tmp/5090jepa_evidence/research_smoke/test_runtime");
    fs::create_dir_all(&run_root).expect("create research smoke test runtime root");
    let run_id = format!(
        "test_research_smoke_ablation_seed_without_onet_{}",
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
        "--career-ablation-training-seed",
        "777",
        "--json",
    ]);
    assert!(
        stderr.contains("--career-ablation-training-seed was supplied without --onet-text-zip"),
        "missing ablation seed remediation in stderr: {stderr}"
    );
    assert!(
        !run_dir.exists(),
        "ablation seed without O*NET source must fail before creating bundle source of truth"
    );
}

#[test]
fn research_smoke_fails_closed_for_incomplete_onet_zip_preflight() {
    let root = repo_root();
    let run_root = root.join("tmp/5090jepa_evidence/research_smoke/test_runtime");
    fs::create_dir_all(&run_root).expect("create research smoke test runtime root");
    let run_id = format!("test_research_smoke_bad_onet_zip_{}", std::process::id());
    let run_dir = run_root.join(&run_id);
    if run_dir.exists() {
        fs::remove_dir_all(&run_dir).expect("remove previous research smoke test run");
    }
    let tmp = tempfile::tempdir().expect("create temp dir for bad O*NET ZIP");
    let bad_zip = tmp.path().join("db_30_2_text_incomplete.zip");
    write_incomplete_onet_zip(&bad_zip);

    let (stdout, _stderr) = run_err(&[
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
        bad_zip.to_str().expect("bad O*NET ZIP path utf8"),
        "--career-event-count",
        "120",
        "--json",
    ]);
    let result: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("stdout must be one JSON object: {err}\nstdout={stdout}"));
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_code"], "VALIDATION");
    assert!(
        result["error_message"]
            .as_str()
            .expect("error_message is string")
            .contains("O*NET 30.2 text ZIP has 1 files, expected 41"),
        "missing O*NET preflight detail in stdout: {stdout}"
    );
    assert!(
        run_dir.exists(),
        "bad O*NET path must leave failed bundle evidence"
    );

    let manifest: Value = serde_json::from_slice(
        &fs::read(run_dir.join("run_manifest.json")).expect("read failed run manifest"),
    )
    .expect("parse failed run manifest");
    assert_eq!(manifest["status"], "failed");

    let fsv: Value = serde_json::from_slice(
        &fs::read(run_dir.join("fsv_report.json")).expect("read failed fsv"),
    )
    .expect("parse failed fsv");
    assert_eq!(fsv["status"], "failed");

    let preflight_path = run_dir.join("onet_source_preflight.json");
    let preflight: Value =
        serde_json::from_slice(&fs::read(&preflight_path).expect("read failed O*NET preflight"))
            .expect("parse failed O*NET preflight");
    assert_eq!(preflight["status"], "failed");
    assert_eq!(preflight["error_code"], "VALIDATION");
    assert_eq!(
        preflight["source_of_truth"]["expected_root"],
        "db_30_2_text/"
    );

    let db_entry_count = fs::read_dir(run_dir.join("dbs"))
        .expect("read failed bundle DB root")
        .count();
    assert_eq!(
        db_entry_count, 0,
        "O*NET preflight failure must happen before any RocksDB source-of-truth path is created"
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
        "--career-training-seed",
        "42",
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
    assert_eq!(manifest["career_training_seed"], 42);
    assert!(manifest["career_ablation_training_seed"].is_null());

    let generated_train_config: Value =
        serde_json::from_slice(
            &fs::read(run_dir.join(
                "generated_fixtures/onet_30_2/verification/career_taxonomy_train_config.json",
            ))
            .expect("read generated train config"),
        )
        .expect("parse generated train config");
    assert_eq!(generated_train_config["seed"], 42);

    let generated_a0_train_config: Value =
        serde_json::from_slice(
            &fs::read(run_dir.join(
                "ablations/A0_taxonomy_only_action_vicreg/verification/career_taxonomy_train_config.json",
            ))
            .expect("read generated A0 train config"),
        )
        .expect("parse generated A0 train config");
    assert_eq!(generated_a0_train_config["seed"], 42);

    let data_manifest: Value = serde_json::from_slice(
        &fs::read(run_dir.join("data_manifest.json")).expect("read data manifest"),
    )
    .expect("parse data manifest");
    let preflight = &data_manifest["onet_source_preflight"];
    assert_eq!(preflight["status"], "passed");
    assert_eq!(preflight["archive_file_count"], 41);
    assert_eq!(
        preflight["source_zip_sha256"],
        "b5479271931796b838f7173dc0f673a9ec961b7833ac87168fd11e92e7453741"
    );
    let member_rows = preflight["required_members"]
        .as_array()
        .expect("preflight required_members is array")
        .iter()
        .filter_map(|member| Some((member["path"].as_str()?, member["row_count"].as_u64())))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(member_rows["db_30_2_text/Occupation Data.txt"], Some(1016));
    assert_eq!(member_rows["db_30_2_text/Job Zones.txt"], Some(923));
    assert_eq!(member_rows["db_30_2_text/Skills.txt"], Some(62580));
    assert_eq!(
        member_rows["db_30_2_text/Related Occupations.txt"],
        Some(18460)
    );

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
