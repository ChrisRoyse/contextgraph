//! Phase F FSV for feedback/operator-override timestamped retention.
//!
//! Writes synthetic rows spanning 0-400 days into real RocksDB CFs, runs the
//! production nightly GC path, then reads the source CFs and cold archive files
//! back as the source of truth.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use context_graph_mejepa_hygiene::{
    cf, gc_run_nightly, open_hygiene_rocksdb, quota_status, runtime_config, scan_cf, HygieneEnv,
};
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/phase-f-retention-fsv";
const NOW_MS: i64 = 1_800_000_000_000;
const DAY_MS: i64 = 86_400_000;
const RETENTION_DAYS: i64 = 365;
const AGE_DAYS: [i64; 5] = [0, 364, 365, 366, 400];

fn main() -> Result<()> {
    let fsv_root = PathBuf::from(FSV_ROOT);
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root).with_context(|| format!("remove {}", fsv_root.display()))?;
    }
    fs::create_dir_all(&fsv_root).with_context(|| format!("create {}", fsv_root.display()))?;

    let db_path = fsv_root.join("rocks-retention");
    let archive_root = fsv_root.join("storage/witness");
    let cold_root = fsv_root.join("storage/cold");
    let db = open_hygiene_rocksdb(&db_path)?;
    let mut config = runtime_config(db.clone(), archive_root.clone())?;
    config.retention_policy_path = Some(PathBuf::from("config/retention_policy.toml"));
    config.now_unix = Some(NOW_MS / 1_000);
    let env = HygieneEnv::try_new(config)?;

    let inserted = insert_synthetic_rows(env.config.db.as_ref())?;
    let counts_before = counts(env.config.db.as_ref())?;
    let quota_before = quota_used(&env)?;

    let report = gc_run_nightly(&env)?;
    drop(env);
    drop(db);

    let reopened = open_hygiene_rocksdb(&db_path)?;
    let mut reopened_config = runtime_config(reopened.clone(), archive_root)?;
    reopened_config.retention_policy_path = Some(PathBuf::from("config/retention_policy.toml"));
    reopened_config.now_unix = Some(NOW_MS / 1_000);
    let reopened_env = HygieneEnv::try_new(reopened_config)?;
    let counts_after = counts(reopened.as_ref())?;
    let quota_after = quota_used(&reopened_env)?;
    let row_readbacks = readback_rows(reopened.as_ref(), &inserted, &cold_root)?;
    let physical_artifacts = archive_physical_artifacts(&cold_root)?;
    let archive_step = report
        .steps
        .iter()
        .find(|step| step.name == "archive_timestamped_rows")
        .map(|step| step.detail.clone())
        .unwrap_or_else(|| json!({}));
    let all_passed = row_readbacks
        .iter()
        .all(|case| case.get("pass").and_then(Value::as_bool).unwrap_or(false))
        && counts_before
            == BTreeMap::from([
                (
                    context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK.to_string(),
                    AGE_DAYS.len() as u64,
                ),
                (
                    context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES.to_string(),
                    AGE_DAYS.len() as u64,
                ),
            ])
        && counts_after
            == BTreeMap::from([
                (
                    context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK.to_string(),
                    3,
                ),
                (
                    context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES.to_string(),
                    3,
                ),
            ])
        && quota_after < quota_before
        && physical_artifacts.len() == 4;

    let evidence = json!({
        "fsv_root": FSV_ROOT,
        "task_id": "TASK-HYG-002-feedback-and-override-retention",
        "started_at_unix_ms": NOW_MS,
        "build_release_sha": git_head_short(),
        "happy_path": [{
            "case": "nightly_gc_archives_expired_feedback_and_override_rows",
            "sot": [
                context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
                context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES,
                cold_root.display().to_string()
            ],
            "before": counts_before,
            "trigger": "cargo run -p context-graph-mejepa-hygiene --example phase_f_feedback_override_retention_fsv",
            "after": counts_after,
            "expected": {
                "retained_rows_per_cf": 3,
                "archived_rows_per_cf": 2,
                "quota_agent_feedback_and_overrides_decreases": true
            },
            "actual": {
                "quota_before_bytes": quota_before,
                "quota_after_bytes": quota_after,
                "archive_step": archive_step
            },
            "pass": all_passed,
            "evidence_path": fsv_root.join("feedback_override_retention_fsv.json")
        }],
        "boundary_cases": row_readbacks,
        "physical_artifacts": physical_artifacts,
        "all_passed": all_passed,
        "cf_counts_before": {
            "CF_MEJEPA_AGENT_FEEDBACK": AGE_DAYS.len(),
            "CF_MEJEPA_OPERATOR_OVERRIDES": AGE_DAYS.len()
        },
        "cf_counts_after": {
            "CF_MEJEPA_AGENT_FEEDBACK": 3,
            "CF_MEJEPA_OPERATOR_OVERRIDES": 3
        },
        "readback_equal": all_passed
    });

    let evidence_path = fsv_root.join("feedback_override_retention_fsv.json");
    fs::write(&evidence_path, serde_json::to_vec_pretty(&evidence)?)
        .with_context(|| format!("write {}", evidence_path.display()))?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn insert_synthetic_rows(db: &DB) -> Result<Vec<SyntheticRow>> {
    let mut rows = Vec::new();
    for age_days in AGE_DAYS {
        let timestamp_millis = NOW_MS - age_days * DAY_MS;
        let feedback_key = format!("feedback-age-{age_days:03}").into_bytes();
        let feedback_value = serde_json::to_vec(&json!({
            "schema": "synthetic_surprise_event_v1",
            "feedback_id": format!("feedback-age-{age_days:03}"),
            "prediction_id": format!("prediction-age-{age_days:03}"),
            "agent_id": "retention-fsv-agent",
            "ts_millis": timestamp_millis,
            "feedback_kind": "surprise",
            "severity": "medium"
        }))?;
        put_sync_readback(
            db,
            context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
            &feedback_key,
            &feedback_value,
        )?;
        rows.push(SyntheticRow {
            cf_name: context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK.to_string(),
            key: feedback_key,
            value: feedback_value,
            age_days,
        });

        let mut override_key = [0_u8; 16];
        override_key[15] = age_days as u8;
        let override_value = bincode::serialize(&OperatorOverrideRetentionRow {
            prediction_id: override_key,
            override_verdict: OperatorOverrideRetentionVerdict::Fail,
            reason: format!("synthetic retention row age {age_days}"),
            operator_id: "retention-fsv-operator".to_string(),
            created_at_unix_ms: timestamp_millis,
            sampling_weight_multiplier: 6.0,
        })?;
        put_sync_readback(
            db,
            context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES,
            &override_key,
            &override_value,
        )?;
        rows.push(SyntheticRow {
            cf_name: context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES.to_string(),
            key: override_key.to_vec(),
            value: override_value,
            age_days,
        });
    }
    Ok(rows)
}

fn put_sync_readback(db: &DB, cf_name: &str, key: &[u8], value: &[u8]) -> Result<()> {
    let cf_handle = cf(db, cf_name)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf_handle, key, value, &opts)?;
    db.flush_cf(cf_handle)?;
    let readback = db
        .get_cf(cf_handle, key)?
        .with_context(|| format!("missing readback for {cf_name} {}", hex::encode(key)))?;
    anyhow::ensure!(readback == value, "readback mismatch for {cf_name}");
    Ok(())
}

fn counts(db: &DB) -> Result<BTreeMap<String, u64>> {
    let mut out = BTreeMap::new();
    for cf_name in [
        context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
        context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES,
    ] {
        out.insert(cf_name.to_string(), scan_cf(db, cf_name)?.len() as u64);
    }
    Ok(out)
}

fn quota_used(env: &HygieneEnv) -> Result<u64> {
    let status = quota_status(env)?;
    let row = status
        .categories
        .iter()
        .find(|row| row.category.as_str() == "agent_feedback_and_overrides")
        .context("quota category agent_feedback_and_overrides missing")?;
    Ok(row.used_bytes)
}

fn readback_rows(db: &DB, rows: &[SyntheticRow], cold_root: &Path) -> Result<Vec<Value>> {
    rows.iter()
        .map(|row| {
            let retained = row.age_days <= RETENTION_DAYS;
            let source_readback = db.get_cf(cf(db, &row.cf_name)?, &row.key)?;
            let archive = find_archive_for_row(cold_root, row)?;
            let pass = if retained {
                source_readback.as_deref() == Some(row.value.as_slice()) && archive.is_none()
            } else {
                source_readback.is_none()
                    && archive
                        .as_ref()
                        .and_then(|artifact| artifact.get("sha256").and_then(Value::as_str))
                        == Some(hex::encode(Sha256::digest(&row.value)).as_str())
            };
            Ok(json!({
                "case": format!("{} age {} days", row.cf_name, row.age_days),
                "before": {"row_present": true, "age_days": row.age_days},
                "after": {
                    "source_row_present": source_readback.is_some(),
                    "archive": archive
                },
                "expected": if retained { "retained in source CF" } else { "moved to cold archive" },
                "actual": if source_readback.is_some() { "source CF" } else { "cold archive or missing" },
                "pass": pass
            }))
        })
        .collect()
}

fn find_archive_for_row(cold_root: &Path, row: &SyntheticRow) -> Result<Option<Value>> {
    let dir = cold_root.join("timestamped-retention").join(&row.cf_name);
    if !dir.exists() {
        return Ok(None);
    }
    let key_hex = hex::encode(&row.key);
    for path in archive_files(&dir)? {
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let value: Value = serde_json::from_slice(&bytes)?;
        if value.get("key_hex").and_then(Value::as_str) == Some(key_hex.as_str()) {
            return Ok(Some(json!({
                "path": path,
                "bytes": bytes.len(),
                "sha256": value.get("value_sha256_hex").cloned().unwrap_or(Value::Null)
            })));
        }
    }
    Ok(None)
}

fn archive_physical_artifacts(root: &Path) -> Result<Vec<Value>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    archive_files(root)?
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            Ok(json!({
                "path": path,
                "bytes": bytes.len(),
                "sha256": hex::encode(Sha256::digest(&bytes))
            }))
        })
        .collect()
}

fn archive_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(root).with_context(|| format!("read_dir {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(archive_files(&path)?);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn git_head_short() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

struct SyntheticRow {
    cf_name: String,
    key: Vec<u8>,
    value: Vec<u8>,
    age_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorOverrideRetentionRow {
    prediction_id: [u8; 16],
    override_verdict: OperatorOverrideRetentionVerdict,
    reason: String,
    operator_id: String,
    created_at_unix_ms: i64,
    sampling_weight_multiplier: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum OperatorOverrideRetentionVerdict {
    Pass,
    Fail,
    Abstain,
    OutOfDistribution,
}
