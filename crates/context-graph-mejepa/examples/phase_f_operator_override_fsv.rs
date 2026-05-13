//! TASK-PREDICT-012 FSV — operator override persistence + readback.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, ensure, Context, Result};
use chrono::Utc;
use context_graph_mejepa::open_infer_rocksdb;
use context_graph_mejepa::operator_override::{
    load_operator_override, persist_operator_override, OperatorOverride, OperatorOverrideError,
    OverrideVerdict, OPERATOR_OVERRIDE_SAMPLING_WEIGHT,
};
use context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-operator-override-fsv";

fn main() -> Result<()> {
    let stamp = Utc::now().timestamp_millis();
    let fsv = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv)?;
    let run_root = fsv.join(format!("run-{stamp}-{}", std::process::id()));
    fs::create_dir_all(&run_root)?;
    let db_path = run_root.join("infer-rocksdb");

    let db = open_infer_rocksdb(&db_path).context("open infer rocksdb for override FSV")?;
    let build_release_sha = git_head_short()?;
    let cf_count_before = count_cf_entries(db.as_ref(), CF_MEJEPA_OPERATOR_OVERRIDES)?;
    let record = OperatorOverride {
        prediction_id: [0xAB; 16],
        override_verdict: OverrideVerdict::Fail,
        reason: "operator marked verdict as Fail after manual inspection".to_string(),
        operator_id: "fsv-operator".to_string(),
        created_at_unix_ms: stamp,
        sampling_weight_multiplier: OPERATOR_OVERRIDE_SAMPLING_WEIGHT,
    };

    // --- happy path: persist + readback ----------------------------------
    persist_operator_override(db.as_ref(), &record).context("persist override")?;
    let loaded = load_operator_override(db.as_ref(), record.prediction_id())?
        .ok_or_else(|| anyhow!("readback returned None"))?;
    let persist_readback_pass = loaded == record;
    ensure!(persist_readback_pass, "loaded record diverged from input");

    // --- boundary cases --------------------------------------------------
    let empty_reason_err = persist_operator_override(
        db.as_ref(),
        &OperatorOverride {
            reason: "  ".to_string(),
            ..record.clone()
        },
    )
    .expect_err("empty reason must fail closed");
    let empty_reason_pass = matches!(empty_reason_err, OperatorOverrideError::ReasonEmpty);
    ensure!(empty_reason_pass);

    let empty_op_err = persist_operator_override(
        db.as_ref(),
        &OperatorOverride {
            operator_id: "".to_string(),
            ..record.clone()
        },
    )
    .expect_err("empty operator_id must fail closed");
    let empty_operator_pass = matches!(empty_op_err, OperatorOverrideError::OperatorEmpty);
    ensure!(empty_operator_pass);

    let long_reason_err = persist_operator_override(
        db.as_ref(),
        &OperatorOverride {
            reason: "x".repeat(4097),
            ..record.clone()
        },
    )
    .expect_err("over-long reason must fail closed");
    let long_reason_pass = matches!(long_reason_err, OperatorOverrideError::ReasonTooLong);
    ensure!(long_reason_pass);

    let zero_prediction_id_err = persist_operator_override(
        db.as_ref(),
        &OperatorOverride {
            prediction_id: [0; 16],
            ..record.clone()
        },
    )
    .expect_err("zero prediction-id must fail closed");
    let zero_prediction_id_pass = matches!(
        zero_prediction_id_err,
        OperatorOverrideError::PredictionIdZero
    );
    ensure!(zero_prediction_id_pass);

    // --- absent key returns None, not an error --------------------------
    let absent_id = context_graph_mejepa::PredictionId([0xCD; 16]);
    let absent = load_operator_override(db.as_ref(), absent_id)?;
    let absent_key_pass = absent.is_none();
    ensure!(absent_key_pass, "absent prediction-id must return None");

    let cf_count_after = count_cf_entries(db.as_ref(), CF_MEJEPA_OPERATOR_OVERRIDES)?;
    drop(db);

    // Independent re-open verifies persistence survives close.
    let reopened = open_infer_rocksdb(&db_path).context("reopen infer rocksdb")?;
    let reopened_loaded = load_operator_override(reopened.as_ref(), record.prediction_id())?
        .ok_or_else(|| anyhow!("re-open readback returned None"))?;
    let reopen_readback_pass = reopened_loaded == record;
    ensure!(reopen_readback_pass, "re-open mismatch");

    let physical_artifacts = physical_artifacts(&db_path)?;
    ensure!(
        physical_artifacts["total_bytes"].as_u64().unwrap_or(0) > 0,
        "RocksDB physical artifact byte count must be non-zero"
    );
    ensure!(
        physical_artifacts["sst_file_count"].as_u64().unwrap_or(0) > 0,
        "RocksDB flush should produce at least one SST file"
    );
    let all_passed = persist_readback_pass
        && reopen_readback_pass
        && empty_reason_pass
        && empty_operator_pass
        && long_reason_pass
        && zero_prediction_id_pass
        && absent_key_pass
        && cf_count_after == cf_count_before + 1;

    let artifact_path = fsv.join("operator_override_fsv.json");
    let artifact = json!({
        "fsv_root": fsv,
        "task_id": "TASK-PREDICT-012",
        "issue": 60,
        "started_at_unix_ms": stamp,
        "build_release_sha": build_release_sha,
        "happy_path": [
            {
                "case": "persist_then_load_returns_same_record",
                "sot": CF_MEJEPA_OPERATOR_OVERRIDES,
                "before": cf_count_before,
                "trigger": "persist_operator_override + load_operator_override",
                "expected": record,
                "actual": loaded,
                "pass": persist_readback_pass,
                "evidence_path": db_path,
            },
            {
                "case": "reopen_db_preserves_record",
                "sot": CF_MEJEPA_OPERATOR_OVERRIDES,
                "before": cf_count_before,
                "trigger": "drop(db); open_infer_rocksdb; load_operator_override",
                "expected": record,
                "actual": reopened_loaded,
                "pass": reopen_readback_pass,
                "evidence_path": db_path,
            }
        ],
        "boundary_cases": [
            {"case": "empty_reason", "expected": "MEJEPA_OPERATOR_OVERRIDE_REASON_EMPTY",
             "actual": empty_reason_err.to_string(), "pass": empty_reason_pass},
            {"case": "empty_operator", "expected": "MEJEPA_OPERATOR_OVERRIDE_OPERATOR_EMPTY",
             "actual": empty_op_err.to_string(), "pass": empty_operator_pass},
            {"case": "long_reason", "expected": "MEJEPA_OPERATOR_OVERRIDE_REASON_TOO_LONG",
             "actual": long_reason_err.to_string(), "pass": long_reason_pass},
            {"case": "zero_prediction_id", "expected": "MEJEPA_OPERATOR_OVERRIDE_PREDICTION_ID_ZERO",
             "actual": zero_prediction_id_err.to_string(), "pass": zero_prediction_id_pass},
            {"case": "absent_key_returns_none", "expected": "None",
             "actual": format!("{:?}", absent), "pass": absent_key_pass},
        ],
        "all_passed": all_passed,
        "cf_counts_before": {"CF_MEJEPA_OPERATOR_OVERRIDES": cf_count_before},
        "cf_counts_after": {"CF_MEJEPA_OPERATOR_OVERRIDES": cf_count_after},
        "readback_equal": persist_readback_pass && reopen_readback_pass,
        "sampling_weight_multiplier": OPERATOR_OVERRIDE_SAMPLING_WEIGHT,
        "physical_artifacts": physical_artifacts,
        "source_of_truth": {
            "writer": "context_graph_mejepa::operator_override::persist_operator_override",
            "consumer": "BatchSampler reads CF_MEJEPA_OPERATOR_OVERRIDES on next training batch",
        },
    });
    ensure!(all_passed, "operator override FSV assertions failed");
    write_json_readback(&artifact_path, &artifact)?;
    let bytes = fs::read(&artifact_path)?;
    let sha = format!("{:x}", Sha256::digest(&bytes));
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artifact_path": artifact_path,
            "artifact_sha256": sha,
            "all_passed": all_passed,
            "readback_equal": persist_readback_pass && reopen_readback_pass,
        }))?
    );
    Ok(())
}

fn count_cf_entries(db: &rocksdb::DB, cf_name: &str) -> Result<u64> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| anyhow!("CF {cf_name} missing"))?;
    let mut count = 0_u64;
    for item in db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

fn write_json_readback(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes).with_context(|| format!("write {}", path.display()))?;
    let readback_bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    ensure!(
        readback_bytes == bytes,
        "artifact byte readback mismatch for {}",
        path.display()
    );
    let readback: Value = serde_json::from_slice(&readback_bytes)
        .with_context(|| format!("parse {}", path.display()))?;
    ensure!(
        readback == *value,
        "artifact JSON readback mismatch for {}",
        path.display()
    );
    Ok(())
}

fn physical_artifacts(root: &Path) -> Result<Value> {
    let mut total_bytes = 0_u64;
    let mut sst_file_count = 0_u64;
    let mut sample_files = Vec::new();
    collect_physical_artifacts(
        root,
        &mut total_bytes,
        &mut sst_file_count,
        &mut sample_files,
    )?;
    Ok(json!({
        "root": root,
        "total_bytes": total_bytes,
        "sst_file_count": sst_file_count,
        "sample_files": sample_files,
    }))
}

fn collect_physical_artifacts(
    path: &Path,
    total_bytes: &mut u64,
    sst_file_count: &mut u64,
    sample_files: &mut Vec<Value>,
) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read_dir {}", path.display()))? {
        let entry = entry?;
        let entry_path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            collect_physical_artifacts(&entry_path, total_bytes, sst_file_count, sample_files)?;
            continue;
        }
        *total_bytes = total_bytes.saturating_add(metadata.len());
        if entry_path.extension().is_some_and(|ext| ext == "sst") {
            *sst_file_count += 1;
        }
        if sample_files.len() < 20 {
            sample_files.push(json!({
                "path": entry_path,
                "bytes": metadata.len(),
            }));
        }
    }
    Ok(())
}

fn git_head_short() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .context("git rev-parse --short HEAD")?;
    ensure!(
        output.status.success(),
        "git rev-parse failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sha = String::from_utf8(output.stdout)?.trim().to_string();
    ensure!(!sha.is_empty(), "git rev-parse returned empty SHA");
    Ok(sha)
}
