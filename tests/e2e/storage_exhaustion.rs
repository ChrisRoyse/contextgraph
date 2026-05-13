use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, ensure, Context, Result};
use context_graph_mejepa_hygiene::{
    cf, count_and_bytes_cf, encode_cf_json, meta_key, open_hygiene_rocksdb, quota_check_and_evict,
    quota_status, EntryId, HygieneEntryMeta, HygieneEnv, RuntimeConfig, StorageCategory, Tier,
    BYTES_PER_GB,
};
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const TASK_ID: &str = "TASK-TEST-005";
const MIB: usize = 1024 * 1024;
const QUOTA_BYTES: u64 = BYTES_PER_GB;
const MAX_RECOVERY_RATIO: f64 = 1.10;
const LAST_REPORT_KEY: &[u8] = b"last_quota_report";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StorageExhaustionFsvReport {
    pub fsv_root: String,
    pub task_id: String,
    pub build_release_sha: String,
    pub source_of_truth: StorageExhaustionSourceOfTruth,
    pub happy_path: Vec<StepEvidence>,
    pub boundary_cases: Vec<BoundaryEvidence>,
    pub cf_counts_before: BTreeMap<String, CfCountBytes>,
    pub cf_counts_after: BTreeMap<String, CfCountBytes>,
    pub physical_artifacts: Vec<FileArtifact>,
    pub readback_equal: bool,
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StorageExhaustionSourceOfTruth {
    pub hygiene_db_path: String,
    pub archive_root: String,
    pub quota_state_cf: String,
    pub saturated_cf: String,
    pub report_path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CfCountBytes {
    pub rows: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StepEvidence {
    pub step: String,
    pub trigger: String,
    pub source_of_truth: String,
    pub before: Value,
    pub after: Value,
    pub expected: Value,
    pub actual: Value,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BoundaryEvidence {
    pub case: String,
    pub trigger: String,
    pub expected_error_code: String,
    pub actual_error_code: String,
    pub before: Value,
    pub after: Value,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FileArtifact {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

struct Harness {
    fsv_root: PathBuf,
    db_path: PathBuf,
    archive_root: PathBuf,
}

pub fn default_fsv_root() -> PathBuf {
    PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-storage-exhaustion-fsv")
}

pub fn default_report_path(fsv_root: &Path) -> PathBuf {
    fsv_root.join("storage_exhaustion_fsv.json")
}

pub fn run_storage_exhaustion_fsv(fsv_root: &Path) -> Result<StorageExhaustionFsvReport> {
    remove_existing_fsv_root(fsv_root)?;
    fs::create_dir_all(fsv_root)?;
    let harness = Harness {
        fsv_root: fsv_root.to_path_buf(),
        db_path: fsv_root.join("hygiene-rocksdb"),
        archive_root: fsv_root.join("archives"),
    };
    fs::create_dir_all(&harness.archive_root)?;
    let report_path = default_report_path(fsv_root);
    let build_release_sha = git_head_short();

    let cf_counts_before = {
        let env = harness.env()?;
        count_relevant_cfs(env.config.db.as_ref())?
    };

    let (happy_path, cf_counts_after) = {
        let env = harness.env()?;
        let mut steps = Vec::new();
        let seed_before = quota_status(&env)?;
        seed_shift_rows(&env, 6, false, 0xA5)?;
        let saturated = quota_status(&env)?;
        let saturated_row = category_row(&saturated, StorageCategory::ShiftLogSubscriberState)?;
        let seed_pass =
            saturated_row.over_budget && saturated.total_used_bytes > seed_before.total_used_bytes;
        steps.push(StepEvidence {
            step: "seed_quota_saturation".to_string(),
            trigger: "seed six 1MiB shift-watermark rows with eviction metadata".to_string(),
            source_of_truth: context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK.to_string(),
            before: json!(seed_before),
            after: json!({
                "quotaStatus": saturated,
                "saturatedCategory": saturated_row,
                "rowBytes": count_and_bytes_cf(&env.config.db, context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)?
            }),
            expected: json!({"shiftLogSubscriberStateOverBudget": true}),
            actual: json!({"shiftLogSubscriberStateOverBudget": saturated_row.over_budget}),
            pass: seed_pass,
        });

        let report = quota_check_and_evict(&env)?;
        let after = quota_status(&env)?;
        let after_row = category_row(&after, StorageCategory::ShiftLogSubscriberState)?;
        let recovered_within_cap = (after.total_used_bytes as f64)
            <= (after.total_quota_bytes as f64 * MAX_RECOVERY_RATIO);
        let quota_pass = !after_row.over_budget
            && recovered_within_cap
            && !report.evicted.is_empty()
            && report.before.categories.iter().any(|row| row.over_budget);
        steps.push(StepEvidence {
            step: "quota_eviction_recovery".to_string(),
            trigger: "quota_check_and_evict".to_string(),
            source_of_truth: format!(
                "{} + {}",
                context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
                context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE
            ),
            before: json!(report.before),
            after: json!({
                "quotaStatus": after,
                "evictionReport": report,
                "rowBytes": count_and_bytes_cf(&env.config.db, context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)?
            }),
            expected: json!({
                "categoryOverBudget": false,
                "totalUsedWithin110PercentOfCap": true,
                "evictionsAtLeast": 1
            }),
            actual: json!({
                "categoryOverBudget": after_row.over_budget,
                "totalUsedWithin110PercentOfCap": recovered_within_cap,
                "evictions": report.evicted.len()
            }),
            pass: quota_pass,
        });

        flush_hygiene_db(env.config.db.as_ref())?;
        let last_report = read_last_quota_report(env.config.db.as_ref())?;
        let counts_after = count_relevant_cfs(env.config.db.as_ref())?;
        steps.push(StepEvidence {
            step: "quota_report_readback".to_string(),
            trigger: "read CF_MEJEPA_QUOTA_STATE[last_quota_report] after flush".to_string(),
            source_of_truth: context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE.to_string(),
            before: json!(null),
            after: json!({
                "lastReportEvictions": last_report.evicted.len(),
                "quotaStateRows": counts_after
                    .get(context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE)
                    .map(|count| count.rows)
                    .unwrap_or_default()
            }),
            expected: json!({"lastReportPresent": true, "evictionsAtLeast": 1}),
            actual: json!({"lastReportPresent": true, "evictions": last_report.evicted.len()}),
            pass: !last_report.evicted.is_empty(),
        });
        (steps, counts_after)
    };

    let reopened_counts = {
        let env = harness.env()?;
        let counts = count_relevant_cfs(env.config.db.as_ref())?;
        let status = quota_status(&env)?;
        let row = category_row(&status, StorageCategory::ShiftLogSubscriberState)?;
        ensure!(!row.over_budget, "quota over budget after reopen");
        counts
    };
    ensure!(
        reopened_counts == cf_counts_after,
        "hygiene CF counts changed after reopen"
    );

    let boundary_cases = vec![
        boundary_empty_db(&harness.fsv_root.join("boundary-empty"))?,
        boundary_invalid_quota(&harness.fsv_root.join("boundary-invalid-quota"))?,
        boundary_all_gold_unrecoverable(&harness.fsv_root.join("boundary-all-gold"))?,
        boundary_untracked_rows_fail_closed(&harness.fsv_root.join("boundary-untracked"))?,
    ];

    let all_passed =
        happy_path.iter().all(|step| step.pass) && boundary_cases.iter().all(|case| case.pass);
    let failed_steps = happy_path
        .iter()
        .filter(|step| !step.pass)
        .map(|step| step.step.as_str())
        .collect::<Vec<_>>();
    let failed_boundaries = boundary_cases
        .iter()
        .filter(|case| !case.pass)
        .map(|case| case.case.as_str())
        .collect::<Vec<_>>();
    ensure!(
        all_passed,
        "storage exhaustion FSV contains failed evidence: steps={failed_steps:?} boundaries={failed_boundaries:?}"
    );
    Ok(StorageExhaustionFsvReport {
        fsv_root: fsv_root.display().to_string(),
        task_id: TASK_ID.to_string(),
        build_release_sha,
        source_of_truth: StorageExhaustionSourceOfTruth {
            hygiene_db_path: harness.db_path.display().to_string(),
            archive_root: harness.archive_root.display().to_string(),
            quota_state_cf: context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE.to_string(),
            saturated_cf: context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK.to_string(),
            report_path: report_path.display().to_string(),
        },
        happy_path,
        boundary_cases,
        cf_counts_before,
        cf_counts_after: reopened_counts,
        physical_artifacts: collect_file_artifacts(fsv_root)?,
        readback_equal: true,
        all_passed,
    })
}

pub fn write_report_with_readback(report: &StorageExhaustionFsvReport, path: &Path) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    write_bytes_readback(path, &bytes)?;
    let readback: StorageExhaustionFsvReport = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        readback == *report,
        "storage exhaustion FSV readback mismatch"
    );
    Ok(())
}

impl Harness {
    fn env(&self) -> Result<HygieneEnv> {
        open_env(&self.db_path, &self.archive_root, QUOTA_BYTES)
    }
}

fn open_env(db_path: &Path, archive_root: &Path, total_quota_bytes: u64) -> Result<HygieneEnv> {
    let db = open_hygiene_rocksdb(db_path)?;
    let config = RuntimeConfig {
        db,
        archive_root: archive_root.to_path_buf(),
        total_quota_bytes,
        witness_segment_size: 1024,
        witness_min_age_days: 1,
        now_unix: Some(2_000_000_000),
        self_healing: None,
    };
    Ok(HygieneEnv::try_new(config)?)
}

fn seed_shift_rows(env: &HygieneEnv, rows: u64, gold: bool, byte: u8) -> Result<()> {
    let payload = vec![byte; MIB];
    for idx in 0..rows {
        let key = idx.to_be_bytes();
        put_sync_readback(
            env.config.db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
            &key,
            &payload,
        )?;
        let mut meta = HygieneEntryMeta::new(
            EntryId::new(context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK, key),
            StorageCategory::ShiftLogSubscriberState,
            Tier::Frozen,
            payload.len() as u64,
            env.now_unix(),
        );
        meta.gold = gold;
        meta.frequency.score = 0.0;
        write_meta_sync(env.config.db.as_ref(), &meta)?;
    }
    Ok(())
}

fn seed_untracked_shift_rows(env: &HygieneEnv, rows: u64) -> Result<()> {
    let payload = vec![0xD6; MIB];
    for idx in 0..rows {
        put_sync_readback(
            env.config.db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
            &idx.to_be_bytes(),
            &payload,
        )?;
    }
    Ok(())
}

fn boundary_empty_db(root: &Path) -> Result<BoundaryEvidence> {
    fs::create_dir_all(root)?;
    let env = open_env(&root.join("rocksdb"), &root.join("archives"), QUOTA_BYTES)?;
    let before = quota_status(&env)?;
    let report = quota_check_and_evict(&env)?;
    let after = quota_status(&env)?;
    let pass = before.total_used_bytes == after.total_used_bytes && report.evicted.is_empty();
    Ok(BoundaryEvidence {
        case: "empty_db_noop".to_string(),
        trigger: "quota_check_and_evict on empty hygiene DB".to_string(),
        expected_error_code: "none".to_string(),
        actual_error_code: "none".to_string(),
        before: json!(before),
        after: json!({"quotaStatus": after, "evictions": report.evicted.len()}),
        pass,
    })
}

fn boundary_invalid_quota(root: &Path) -> Result<BoundaryEvidence> {
    fs::create_dir_all(root)?;
    let db = open_hygiene_rocksdb(root.join("rocksdb"))?;
    let config = RuntimeConfig {
        db,
        archive_root: root.join("archives"),
        total_quota_bytes: 0,
        witness_segment_size: 1024,
        witness_min_age_days: 1,
        now_unix: Some(2_000_000_000),
        self_healing: None,
    };
    let err = match HygieneEnv::try_new(config) {
        Ok(_) => anyhow::bail!("MEJEPA_STORAGE_EXHAUSTION_INVALID_QUOTA_ACCEPTED"),
        Err(err) => err,
    };
    Ok(BoundaryEvidence {
        case: "invalid_quota_rejected".to_string(),
        trigger: "HygieneEnv::try_new(total_quota_bytes=0)".to_string(),
        expected_error_code: "MEJEPA_HYGIENE_INVALID_CONFIG".to_string(),
        actual_error_code: err.code.to_string(),
        before: json!({"envCreated": false}),
        after: json!({"envCreated": false, "errorKind": err.kind}),
        pass: err.code == "MEJEPA_HYGIENE_INVALID_CONFIG",
    })
}

fn boundary_all_gold_unrecoverable(root: &Path) -> Result<BoundaryEvidence> {
    fs::create_dir_all(root)?;
    let env = open_env(&root.join("rocksdb"), &root.join("archives"), QUOTA_BYTES)?;
    seed_shift_rows(&env, 6, true, 0xC7)?;
    let before = quota_status(&env)?;
    let row_count_before = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
    )?
    .0;
    let err = quota_check_and_evict(&env).expect_err("all gold rows must be unrecoverable");
    let row_count_after = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
    )?
    .0;
    let quota_state = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
    )?;
    Ok(BoundaryEvidence {
        case: "all_gold_overflow_unrecoverable".to_string(),
        trigger: "quota_check_and_evict with only gold over-budget candidates".to_string(),
        expected_error_code: "MEJEPA_HYGIENE_QUOTA_UNRECOVERABLE".to_string(),
        actual_error_code: err.code.to_string(),
        before: json!({"quotaStatus": before, "rows": row_count_before}),
        after: json!({"rows": row_count_after, "quotaStateRowsBytes": quota_state, "errorKind": err.kind}),
        pass: err.code == "MEJEPA_HYGIENE_QUOTA_UNRECOVERABLE"
            && row_count_before == row_count_after
            && quota_state.0 >= 1,
    })
}

fn boundary_untracked_rows_fail_closed(root: &Path) -> Result<BoundaryEvidence> {
    fs::create_dir_all(root)?;
    let env = open_env(&root.join("rocksdb"), &root.join("archives"), QUOTA_BYTES)?;
    seed_untracked_shift_rows(&env, 6)?;
    let before = quota_status(&env)?;
    let before_rows = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
    )?;
    let err = quota_check_and_evict(&env).expect_err("untracked over-budget rows must fail closed");
    let after_rows = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
    )?;
    Ok(BoundaryEvidence {
        case: "untracked_rows_fail_closed".to_string(),
        trigger: "quota_check_and_evict with over-budget rows lacking metadata".to_string(),
        expected_error_code: "MEJEPA_HYGIENE_QUOTA_UNRECOVERABLE".to_string(),
        actual_error_code: err.code.to_string(),
        before: json!({"quotaStatus": before, "rowsBytes": before_rows}),
        after: json!({"rowsBytes": after_rows, "errorKind": err.kind}),
        pass: err.code == "MEJEPA_HYGIENE_QUOTA_UNRECOVERABLE" && before_rows == after_rows,
    })
}

fn read_last_quota_report(db: &DB) -> Result<context_graph_mejepa_hygiene::QuotaEvictionReport> {
    let bytes = db
        .get_cf(
            cf(db, context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE)?,
            LAST_REPORT_KEY,
        )?
        .ok_or_else(|| anyhow!("MEJEPA_STORAGE_EXHAUSTION_LAST_REPORT_MISSING"))?;
    Ok(serde_json::from_slice::<
        context_graph_mejepa_hygiene::QuotaEvictionReport,
    >(&bytes)?)
}

fn count_relevant_cfs(db: &DB) -> Result<BTreeMap<String, CfCountBytes>> {
    let mut out = BTreeMap::new();
    for cf_name in [
        context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
        context_graph_mejepa_cf::CF_MEJEPA_PANEL_META,
        context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
    ] {
        let (rows, bytes) = count_and_bytes_cf(db, cf_name)?;
        out.insert(cf_name.to_string(), CfCountBytes { rows, bytes });
    }
    Ok(out)
}

fn category_row(
    status: &context_graph_mejepa_hygiene::QuotaStatus,
    category: StorageCategory,
) -> Result<context_graph_mejepa_hygiene::QuotaCategoryStatus> {
    status
        .categories
        .iter()
        .find(|row| row.category == category)
        .cloned()
        .ok_or_else(|| anyhow!("missing quota status row for {}", category.as_str()))
}

fn put_sync_readback(db: &DB, cf_name: &str, key: &[u8], value: &[u8]) -> Result<()> {
    let handle = cf(db, cf_name)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(handle, key, value, &opts)?;
    let readback = db
        .get_cf(handle, key)?
        .ok_or_else(|| anyhow!("missing row after sync write to {cf_name}"))?;
    ensure!(
        readback == value,
        "sync write readback mismatch in {cf_name}"
    );
    Ok(())
}

fn write_meta_sync(db: &DB, meta: &HygieneEntryMeta) -> Result<()> {
    meta.validate().map_err(|detail| anyhow!(detail))?;
    put_sync_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_PANEL_META,
        &meta_key(&meta.entry_id),
        &encode_cf_json(meta)?,
    )
}

fn flush_hygiene_db(db: &DB) -> Result<()> {
    for cf_name in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
        if let Some(cf) = db.cf_handle(cf_name) {
            db.flush_cf(cf)?;
        }
    }
    Ok(())
}

fn write_bytes_readback(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .with_context(|| format!("open write {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let readback = fs::read(path)?;
    ensure!(
        readback == bytes,
        "file readback mismatch for {}",
        path.display()
    );
    Ok(())
}

fn remove_existing_fsv_root(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let temp_dir = std::env::temp_dir();
    let allowed = path.starts_with("/mnt/d/contextgraph/fsv/") || path.starts_with(&temp_dir);
    ensure!(
        allowed,
        "refusing to remove existing storage-exhaustion FSV root outside /mnt/d/contextgraph/fsv or temp dir: {}",
        path.display()
    );
    let mut last_err = None;
    for attempt in 0..5 {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                std::thread::sleep(Duration::from_millis(50 * (attempt + 1) as u64));
            }
        }
    }
    if !path.exists() {
        return Ok(());
    }
    Err(last_err.expect("remove_dir_all attempted at least once"))
        .with_context(|| format!("remove {}", path.display()))
}

fn collect_file_artifacts(root: &Path) -> Result<Vec<FileArtifact>> {
    let mut out = Vec::new();
    collect_file_artifacts_inner(root, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn collect_file_artifacts_inner(path: &Path, out: &mut Vec<FileArtifact>) -> Result<()> {
    if path.is_file() {
        let bytes = fs::read(path)?;
        out.push(FileArtifact {
            path: path.display().to_string(),
            bytes: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(&bytes)),
        });
        return Ok(());
    }
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            collect_file_artifacts_inner(&entry?.path(), out)?;
        }
    }
    Ok(())
}

fn git_head_short() -> String {
    match Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_test_005_storage_exhaustion_recovers_within_quota() {
        let fsv_root = default_fsv_root();
        let report_path = default_report_path(&fsv_root);
        let report = run_storage_exhaustion_fsv(&fsv_root).expect("storage exhaustion FSV");
        write_report_with_readback(&report, &report_path).expect("write storage FSV report");
        assert!(report.all_passed);
        assert!(report.readback_equal);
        assert!(report.happy_path.len() >= 3);
        assert!(report.boundary_cases.len() >= 3);
        assert_eq!(
            report
                .cf_counts_after
                .get(context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)
                .map(|count| count.rows),
            Some(4)
        );
    }
}
