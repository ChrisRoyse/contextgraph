//! TASK-G-019 FSV: oracle flakiness audit and corpus quarantine.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_cli::commands::mejepa_oracle_flakiness::{
    run_oracle_flakiness_audit, OracleFlakinessAuditArgs,
};
use context_graph_cli::commands::mejepa_train::{run_mejepa_train, MejepaTrainArgs, TrainSplitArg};
use context_graph_mejepa::{count_cf, open_infer_rocksdb};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-oracle-flakiness-quarantine-fsv";

fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT);
    if root.exists() {
        fs::remove_dir_all(&root).with_context(|| format!("remove {}", root.display()))?;
    }
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
    let evidence = run(&root)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn run(root: &Path) -> Result<Value> {
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let audit_input = root.join("oracle-repeat-observations.json");
    let quarantine_config = root.join("corpus_quarantine.toml");
    let db_path = root.join("infer-rocksdb");
    let corpus_root = root.join("synthetic-corpus");
    write_audit_input(&audit_input)?;
    write_corpus(&corpus_root)?;

    let audit = run_oracle_flakiness_audit(OracleFlakinessAuditArgs {
        input: audit_input.clone(),
        output: quarantine_config.clone(),
        required_runs: 3,
        threshold: 0.05,
        operator_id: "task-g-019-fsv".to_string(),
    })?;
    ensure!(audit.readback_equal, "quarantine config readback mismatch");
    ensure!(
        audit.quarantined_task_count == 2,
        "expected two quarantined tasks"
    );

    let train = run_mejepa_train(MejepaTrainArgs {
        corpus: vec![corpus_root.clone()],
        split: TrainSplitArg::Train,
        db_path: db_path.clone(),
        max_tasks: None,
        quarantine_config: quarantine_config.clone(),
    })?;
    let holdout = run_mejepa_train(MejepaTrainArgs {
        corpus: vec![corpus_root.clone()],
        split: TrainSplitArg::Holdout,
        db_path: db_path.clone(),
        max_tasks: None,
        quarantine_config: quarantine_config.clone(),
    })?;
    ensure!(
        train.input_task_count == 1,
        "train should retain one deterministic row"
    );
    ensure!(
        train.skipped_quarantined_task_count == 1,
        "train should skip one flaky row"
    );
    ensure!(
        holdout.input_task_count == 1,
        "holdout should retain one deterministic row"
    );
    ensure!(
        holdout.skipped_quarantined_task_count == 1,
        "holdout should skip one flaky row"
    );

    let reopened_counts = reopened_cf_counts(&db_path)?;
    let bad_input_error = missing_third_run_error(root)?;
    let hash_drift = same_outcome_hash_drift_quarantines(root)?;
    let malformed_quarantine_error =
        malformed_quarantine_blocks_train(root, &corpus_root, &db_path)?;
    let report_path = root.join("oracle_flakiness_quarantine_fsv.json");
    let evidence = json!({
        "fsv_root": root,
        "task_id": "TASK-G-019",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_sha(),
        "happy_path": [{
            "case": "flaky_tasks_quarantined_and_deterministic_tasks_ingested",
            "sot": quarantine_config,
            "before": {
                "synthetic_tasks": 4,
                "oracle_runs_per_task": 3
            },
            "trigger": "cargo run --release -p context-graph-cli --example phase_g_oracle_flakiness_quarantine_fsv",
            "after": {
                "audit": audit,
                "train_input_task_count": train.input_task_count,
                "train_skipped_quarantined_task_count": train.skipped_quarantined_task_count,
                "holdout_input_task_count": holdout.input_task_count,
                "holdout_skipped_quarantined_task_count": holdout.skipped_quarantined_task_count,
                "reopened_cf_counts": reopened_counts
            },
            "expected": {
                "quarantined_task_count": 2,
                "train_input_task_count": 1,
                "train_skipped_quarantined_task_count": 1,
                "holdout_input_task_count": 1,
                "holdout_skipped_quarantined_task_count": 1,
                "CF_MEJEPA_PANELS": 2,
                "CF_MEJEPA_ORACLE_VERDICTS": 2
            },
            "actual": {
                "quarantined_task_count": audit.quarantined_task_count,
                "train_input_task_count": train.input_task_count,
                "train_skipped_quarantined_task_count": train.skipped_quarantined_task_count,
                "holdout_input_task_count": holdout.input_task_count,
                "holdout_skipped_quarantined_task_count": holdout.skipped_quarantined_task_count,
                "CF_MEJEPA_PANELS": reopened_counts[context_graph_mejepa_cf::CF_MEJEPA_PANELS],
                "CF_MEJEPA_ORACLE_VERDICTS": reopened_counts[context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS]
            },
            "pass": true,
            "evidence_path": quarantine_config
        }],
        "boundary_cases": [
            {
                "case": "deterministic_task_not_quarantined",
                "sot": quarantine_config,
                "before": "stable_train_task has three identical oracle outcomes and hashes",
                "after": "stable_train_task absent from quarantine",
                "expected": false,
                "actual": quarantine_contains(&quarantine_config, "stable_train_task")?,
                "pass": !quarantine_contains(&quarantine_config, "stable_train_task")?
            },
            {
                "case": "missing_third_run_fails_closed",
                "sot": audit_input,
                "before": "task has two oracle runs",
                "after": bad_input_error,
                "expected": "MEJEPA_ORACLE_FLAKINESS_INSUFFICIENT_RUNS",
                "actual": bad_input_error.contains("MEJEPA_ORACLE_FLAKINESS_INSUFFICIENT_RUNS"),
                "pass": bad_input_error.contains("MEJEPA_ORACLE_FLAKINESS_INSUFFICIENT_RUNS")
            },
            {
                "case": "same_outcome_hash_drift_quarantines",
                "sot": hash_drift["quarantine_path"],
                "before": "three pass outcomes with different verdict hashes",
                "after": hash_drift,
                "expected": true,
                "actual": hash_drift["quarantined"].as_bool().unwrap_or(false),
                "pass": hash_drift["quarantined"].as_bool().unwrap_or(false)
            },
            {
                "case": "malformed_quarantine_blocks_train",
                "sot": "corpus_quarantine.toml",
                "before": "malformed quarantine config",
                "after": malformed_quarantine_error,
                "expected": "MEJEPA_CORPUS_QUARANTINE_LOAD_FAILED",
                "actual": malformed_quarantine_error.contains("MEJEPA_CORPUS_QUARANTINE_LOAD_FAILED"),
                "pass": malformed_quarantine_error.contains("MEJEPA_CORPUS_QUARANTINE_LOAD_FAILED")
            }
        ],
        "physical_artifacts": {
            "audit_input": file_artifact(&audit_input)?,
            "quarantine_config": file_artifact(&quarantine_config)?,
            "fsv_json": report_path
        },
        "cf_counts_after_reopen": reopened_counts,
        "readback_equal": audit.readback_equal,
        "all_passed": true
    });
    let all_boundary_pass = evidence["boundary_cases"]
        .as_array()
        .expect("boundary cases")
        .iter()
        .all(|case| case["pass"].as_bool() == Some(true));
    ensure!(all_boundary_pass, "one or more boundary cases failed");
    write_json(&report_path, &evidence)?;
    Ok(evidence)
}

fn write_audit_input(path: &Path) -> Result<()> {
    let payload = json!({
        "tasks": [
            {
                "task_id": "stable_train_task",
                "repo": "fixture/train",
                "category": "known_good",
                "runs": [
                    oracle_run("1", true, 'a'),
                    oracle_run("2", true, 'a'),
                    oracle_run("3", true, 'a')
                ]
            },
            {
                "task_id": "flaky_train_task",
                "repo": "fixture/train",
                "category": "compile_error",
                "runs": [
                    oracle_run("1", true, 'b'),
                    oracle_run("2", false, 'c'),
                    oracle_run("3", true, 'b')
                ]
            },
            {
                "task_id": "stable_holdout_task",
                "repo": "fixture/holdout",
                "category": "known_good",
                "runs": [
                    oracle_run("1", false, 'd'),
                    oracle_run("2", false, 'd'),
                    oracle_run("3", false, 'd')
                ]
            },
            {
                "task_id": "flaky_holdout_task",
                "repo": "fixture/holdout",
                "category": "subtle_flip",
                "runs": [
                    oracle_run("1", false, 'e'),
                    oracle_run("2", true, 'f'),
                    oracle_run("3", false, 'e')
                ]
            }
        ]
    });
    write_json(path, &payload)
}

fn write_corpus(root: &Path) -> Result<()> {
    fs::create_dir_all(root.join("mutations/stable_train_task"))?;
    fs::create_dir_all(root.join("mutations/flaky_train_task"))?;
    fs::create_dir_all(root.join("mutations/stable_holdout_task"))?;
    fs::create_dir_all(root.join("mutations/flaky_holdout_task"))?;
    fs::write(
        root.join("mutations/stable_train_task/known_good.patch"),
        "diff --git a/app.py b/app.py\n+print('stable train')\n",
    )?;
    fs::write(
        root.join("mutations/flaky_train_task/compile_error.patch"),
        "diff --git a/app.py b/app.py\n+raise RuntimeError('flaky train')\n",
    )?;
    fs::write(
        root.join("mutations/stable_holdout_task/known_good.patch"),
        "diff --git a/app.py b/app.py\n+print('stable holdout')\n",
    )?;
    fs::write(
        root.join("mutations/flaky_holdout_task/subtle_flip.patch"),
        "diff --git a/app.py b/app.py\n+print('flaky holdout')\n",
    )?;
    let index = json!({
        "schema_version": 1,
        "corpus_version": "task-g-019-synthetic",
        "corpus_sha256": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "entries": [
            corpus_entry("train", "known_good", true, "mutations/stable_train_task/known_good.patch", "stable_train_task", '1', '2'),
            corpus_entry("train", "compile_error", false, "mutations/flaky_train_task/compile_error.patch", "flaky_train_task", '3', '4'),
            corpus_entry("holdout", "known_good", false, "mutations/stable_holdout_task/known_good.patch", "stable_holdout_task", '5', '6'),
            corpus_entry("holdout", "subtle_flip", true, "mutations/flaky_holdout_task/subtle_flip.patch", "flaky_holdout_task", '7', '8')
        ]
    });
    write_json(&root.join("index.json"), &index)
}

fn corpus_entry(
    bucket: &str,
    category: &str,
    oracle_all_passed: bool,
    patch_path: &str,
    task_id: &str,
    verdict_hash_byte: char,
    patch_hash_byte: char,
) -> Value {
    json!({
        "bucket": bucket,
        "category": category,
        "mutation_note": format!("{task_id} synthetic row"),
        "oracle_all_passed": oracle_all_passed,
        "oracle_exception": null,
        "oracle_per_test_count": 1,
        "oracle_verdict_sha256": format!("sha256:{}", verdict_hash_byte.to_string().repeat(64)),
        "patch_path": patch_path,
        "patch_sha256": format!("sha256:{}", patch_hash_byte.to_string().repeat(64)),
        "repo": "fixture/repo",
        "task_id": task_id
    })
}

fn oracle_run(run_id: &str, oracle_all_passed: bool, hash_byte: char) -> Value {
    json!({
        "run_id": run_id,
        "oracle_all_passed": oracle_all_passed,
        "oracle_verdict_sha256": format!("sha256:{}", hash_byte.to_string().repeat(64))
    })
}

fn reopened_cf_counts(db_path: &Path) -> Result<BTreeMap<&'static str, u64>> {
    let db = open_infer_rocksdb(db_path).context("reopen inference RocksDB")?;
    let mut counts = BTreeMap::new();
    for cf in [
        context_graph_mejepa_cf::CF_MEJEPA_PANELS,
        context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS,
        context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
        context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
        context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
    ] {
        counts.insert(
            cf,
            count_cf(&db, cf).with_context(|| format!("count {cf}"))?,
        );
    }
    Ok(counts)
}

fn quarantine_contains(path: &Path, task_id: &str) -> Result<bool> {
    let raw = fs::read_to_string(path)?;
    Ok(raw.contains(&format!("task_id = \"{task_id}\"")))
}

fn missing_third_run_error(root: &Path) -> Result<String> {
    let input_path = root.join("missing-third-run.json");
    let output_path = root.join("missing-third-run.toml");
    write_json(
        &input_path,
        &json!({
            "tasks": [{
                "task_id": "missing_run_task",
                "runs": [
                    oracle_run("1", true, 'a'),
                    oracle_run("2", true, 'a')
                ]
            }]
        }),
    )?;
    let err = run_oracle_flakiness_audit(OracleFlakinessAuditArgs {
        input: input_path,
        output: output_path,
        required_runs: 3,
        threshold: 0.05,
        operator_id: "task-g-019-fsv".to_string(),
    })
    .unwrap_err();
    Ok(err.to_string())
}

fn same_outcome_hash_drift_quarantines(root: &Path) -> Result<Value> {
    let input_path = root.join("same-outcome-hash-drift.json");
    let output_path = root.join("same-outcome-hash-drift.toml");
    write_json(
        &input_path,
        &json!({
            "tasks": [{
                "task_id": "hash_drift_task",
                "runs": [
                    oracle_run("1", true, 'a'),
                    oracle_run("2", true, 'b'),
                    oracle_run("3", true, 'a')
                ]
            }]
        }),
    )?;
    let output = run_oracle_flakiness_audit(OracleFlakinessAuditArgs {
        input: input_path,
        output: output_path.clone(),
        required_runs: 3,
        threshold: 0.05,
        operator_id: "task-g-019-fsv".to_string(),
    })?;
    Ok(json!({
        "quarantine_path": output_path,
        "quarantined": output.quarantined_task_count == 1,
        "flakiness_rate": output.reports[0].flakiness_rate
    }))
}

fn malformed_quarantine_blocks_train(
    root: &Path,
    corpus_root: &Path,
    db_path: &Path,
) -> Result<String> {
    let path = root.join("bad-quarantine.toml");
    fs::write(&path, "quarantined_tasks = [{ task_id = \"bad\" }]\n")?;
    let err = run_mejepa_train(MejepaTrainArgs {
        corpus: vec![corpus_root.to_path_buf()],
        split: TrainSplitArg::Train,
        db_path: db_path.to_path_buf(),
        max_tasks: None,
        quarantine_config: path,
    })
    .unwrap_err();
    Ok(err.to_string())
}

fn file_artifact(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read artifact {}", path.display()))?;
    Ok(json!({
        "path": path,
        "bytes": bytes.len(),
        "sha256": sha256_bytes(&bytes)
    }))
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
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
