// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use rocksdb::{WriteBatch, WriteOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::entry::EntryId;
use crate::error::{OpsError, OpsResult};
use crate::quota::{quota_check_and_evict, quota_status};
use crate::reports::{GcReport, GcStepReport};
use crate::retention::{load_retention_policy, retained_by_rule, RetentionRule};
use crate::storage::{
    cf, encode_cf_json, list_meta, open_exclusive_lock, put_readback, scan_cf, HygieneEnv,
};
use crate::tier_ops::tier_demote_all;
use crate::witness_compress::{verify_witness_integrity, witness_compress_old_segments};

pub fn gc_run_nightly(env: &HygieneEnv) -> OpsResult<GcReport> {
    let started_unix = env.now_unix();
    let lock_path = gc_lock_path();
    let _lock = open_exclusive_lock(&lock_path)?;
    let retention_rules = retention_rules_for_env(env)?;

    let mut steps = Vec::new();
    steps.push(step("tier_demote", || {
        Ok(serde_json::to_value(tier_demote_all(env)?)?)
    })?);
    steps.push(step("compact_cf", || compact_hygiene_cfs(env))?);
    steps.push(step("archive_timestamped_rows", || {
        archive_expired_timestamped_rows(env, &retention_rules)
    })?);
    steps.push(step("quota_check_and_evict", || {
        Ok(serde_json::to_value(quota_check_and_evict(env)?)?)
    })?);
    steps.push(step("prune_checkpoints", || {
        prune_cf_keep_latest(
            env,
            context_graph_mejepa_cf::CF_MEJEPA_WEIGHT_BLOBS,
            10,
            &retention_rules,
        )
    })?);
    steps.push(step("prune_calibration", || {
        prune_cf_keep_latest(
            env,
            context_graph_mejepa_cf::CF_MEJEPA_CALIBRATION_HISTORY,
            30,
            &retention_rules,
        )
    })?);
    steps.push(step("witness_compress", || {
        Ok(serde_json::to_value(witness_compress_old_segments(env)?)?)
    })?);
    let witness_after = verify_witness_integrity(env)?;
    steps.push(GcStepReport {
        name: "witness_verify_integrity".to_string(),
        ok: true,
        detail: serde_json::to_value(&witness_after)?,
    });
    let quota_after = quota_status(env)?;
    let completed_unix = env.now_unix();
    let report_key = completed_unix.to_be_bytes();
    let report = GcReport {
        started_unix,
        completed_unix,
        steps,
        quota_after,
        witness_after,
        source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY.to_string(),
        report_key_hex: hex::encode(report_key),
    };
    put_readback(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY,
        &report_key,
        &encode_cf_json(&report)?,
    )?;
    Ok(report)
}

fn step<F>(name: &str, run: F) -> OpsResult<GcStepReport>
where
    F: FnOnce() -> OpsResult<serde_json::Value>,
{
    Ok(GcStepReport {
        name: name.to_string(),
        ok: true,
        detail: run()?,
    })
}

fn compact_hygiene_cfs(env: &HygieneEnv) -> OpsResult<serde_json::Value> {
    let mut compacted = Vec::new();
    for cf_name in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
        env.config
            .db
            .compact_range_cf(cf(&env.config.db, cf_name)?, None::<&[u8]>, None::<&[u8]>);
        compacted.push(cf_name);
    }
    Ok(serde_json::json!({ "compactedColumnFamilies": compacted }))
}

fn archive_expired_timestamped_rows(
    env: &HygieneEnv,
    retention_rules: &BTreeMap<String, RetentionRule>,
) -> OpsResult<serde_json::Value> {
    let cold_root = cold_archive_root(&env.config.archive_root);
    let mut reports = Vec::new();
    for cf_name in [
        context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
        context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES,
    ] {
        let Some(rule) = retention_rules.get(cf_name) else {
            reports.push(serde_json::json!({
                "cf": cf_name,
                "retentionPolicyActive": false,
                "archived": 0,
                "retained": scan_cf(&env.config.db, cf_name)?.len()
            }));
            continue;
        };
        let report = archive_expired_rows_for_cf(env, cf_name, rule, &cold_root)?;
        reports.push(serde_json::to_value(report)?);
    }
    Ok(serde_json::json!({
        "coldArchiveRoot": cold_root,
        "columnFamilies": reports
    }))
}

fn archive_expired_rows_for_cf(
    env: &HygieneEnv,
    cf_name: &str,
    rule: &RetentionRule,
    cold_root: &Path,
) -> OpsResult<TimestampedCfArchiveReport> {
    let rows = scan_cf(&env.config.db, cf_name)?;
    let now_millis = env.now_unix().saturating_mul(1_000);
    let ttl_millis = i64::from(rule.minimum_retention_days).saturating_mul(86_400_000);
    let mut batch = WriteBatch::default();
    let mut archives = Vec::new();
    let mut retained = 0usize;

    for (key, value) in rows {
        let row_timestamp_millis = timestamp_millis_for_row(cf_name, &value)?;
        if !timestamp_is_expired(row_timestamp_millis, now_millis, ttl_millis) {
            retained += 1;
            continue;
        }
        let archive = write_timestamped_row_archive(
            cold_root,
            cf_name,
            &key,
            &value,
            row_timestamp_millis,
            env.now_unix(),
            rule,
        )?;
        batch.delete_cf(cf(&env.config.db, cf_name)?, &key);
        archives.push(archive);
    }

    if !archives.is_empty() {
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        env.config.db.write_opt(batch, &write_opts)?;
        env.config.db.flush_cf(cf(&env.config.db, cf_name)?)?;
        for archive in &archives {
            verify_archived_row_deleted(env, cf_name, archive)?;
        }
    }

    Ok(TimestampedCfArchiveReport {
        cf: cf_name.to_string(),
        retention_class: rule.retention_class.clone(),
        minimum_retention_days: rule.minimum_retention_days,
        retention_policy_active: true,
        archived: archives.len(),
        retained,
        archives,
    })
}

fn timestamp_is_expired(row_timestamp_millis: i64, now_millis: i64, ttl_millis: i64) -> bool {
    if row_timestamp_millis > now_millis {
        return false;
    }
    now_millis.saturating_sub(row_timestamp_millis) > ttl_millis
}

fn timestamp_millis_for_row(cf_name: &str, value: &[u8]) -> OpsResult<i64> {
    let timestamp = match cf_name {
        context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK => {
            let row: serde_json::Value = serde_json::from_slice(value)?;
            row.get("ts_millis")
                .and_then(serde_json::Value::as_i64)
                .ok_or_else(|| {
                    OpsError::invalid(
                        "retention.ts_millis",
                        "CF_MEJEPA_AGENT_FEEDBACK row missing integer ts_millis",
                    )
                })?
        }
        context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES => {
            let row: OperatorOverrideRetentionRow = bincode::deserialize(value).map_err(|err| {
                OpsError::invalid(
                    "retention.created_at_unix_ms",
                    format!("CF_MEJEPA_OPERATOR_OVERRIDES row failed decode: {err}"),
                )
            })?;
            row.created_at_unix_ms
        }
        other => {
            return Err(OpsError::invalid(
                "retention.cf_name",
                format!("unsupported timestamped retention CF {other}"),
            ));
        }
    };
    if timestamp < 0 {
        return Err(OpsError::invalid(
            "retention.timestamp_millis",
            format!("timestamp must be non-negative; got {timestamp}"),
        ));
    }
    Ok(timestamp)
}

fn cold_archive_root(archive_root: &Path) -> PathBuf {
    if archive_root.file_name().and_then(|name| name.to_str()) == Some("cold") {
        return archive_root.to_path_buf();
    }
    if archive_root.file_name().and_then(|name| name.to_str()) == Some("witness") {
        if let Some(parent) = archive_root.parent() {
            if parent.file_name().and_then(|name| name.to_str()) == Some("storage") {
                return parent.join("cold");
            }
        }
    }
    archive_root.join("cold")
}

fn write_timestamped_row_archive(
    cold_root: &Path,
    cf_name: &str,
    key: &[u8],
    value: &[u8],
    row_timestamp_millis: i64,
    archived_at_unix: i64,
    rule: &RetentionRule,
) -> OpsResult<TimestampedRowArchive> {
    let value_sha256_hex = hex::encode(Sha256::digest(value));
    let key_hex = hex::encode(key);
    let file_name = format!("{key_hex}-{}.json", &value_sha256_hex[..12]);
    let path = cold_root
        .join("timestamped-retention")
        .join(cf_name)
        .join(file_name);
    let archive = TimestampedRowArchive {
        schema: "mejepa_timestamped_retention_archive_v1".to_string(),
        cf_name: cf_name.to_string(),
        key_hex,
        row_timestamp_millis,
        archived_at_unix,
        retention_class: rule.retention_class.clone(),
        minimum_retention_days: rule.minimum_retention_days,
        value_encoding: value_encoding_for_cf(cf_name).to_string(),
        value_len_bytes: value.len() as u64,
        value_sha256_hex,
        value_hex: hex::encode(value),
        archive_path: path.clone(),
    };
    write_synced_json(&path, &archive)?;
    Ok(archive)
}

fn verify_archived_row_deleted(
    env: &HygieneEnv,
    cf_name: &str,
    archive: &TimestampedRowArchive,
) -> OpsResult<()> {
    let key = hex::decode(&archive.key_hex).map_err(|err| {
        OpsError::invalid(
            "retention.archive_key_hex",
            format!("archive key hex failed decode: {err}"),
        )
    })?;
    let row = env.config.db.get_cf(cf(&env.config.db, cf_name)?, key)?;
    if row.is_some() {
        return Err(OpsError::invalid(
            "retention.archive_delete_readback",
            format!("{cf_name} row {} remained after archive", archive.key_hex),
        ));
    }
    let bytes = fs::read(&archive.archive_path)
        .map_err(|err| OpsError::io("read", &archive.archive_path, err))?;
    let decoded: TimestampedRowArchive = serde_json::from_slice(&bytes)?;
    if decoded.value_sha256_hex != archive.value_sha256_hex || decoded.key_hex != archive.key_hex {
        return Err(OpsError::invalid(
            "retention.archive_readback",
            format!(
                "archive readback mismatch for {}",
                archive.archive_path.display()
            ),
        ));
    }
    Ok(())
}

fn value_encoding_for_cf(cf_name: &str) -> &'static str {
    match cf_name {
        context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK => "json",
        context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES => "bincode",
        _ => "unknown",
    }
}

fn write_synced_json<T: Serialize>(path: &Path, value: &T) -> OpsResult<()> {
    let parent = path.parent().ok_or_else(|| {
        OpsError::invalid(
            "retention.archive_path",
            format!("archive path {} has no parent", path.display()),
        )
    })?;
    fs::create_dir_all(parent).map_err(|err| OpsError::io("create_dir_all", parent, err))?;
    let tmp_path = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(value)?;
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
            .map_err(|err| OpsError::io("open", &tmp_path, err))?;
        file.write_all(&bytes)
            .map_err(|err| OpsError::io("write_all", &tmp_path, err))?;
        file.sync_all()
            .map_err(|err| OpsError::io("sync_all", &tmp_path, err))?;
    }
    fs::rename(&tmp_path, path).map_err(|err| OpsError::io("rename", &tmp_path, err))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimestampedCfArchiveReport {
    cf: String,
    retention_class: String,
    minimum_retention_days: u32,
    #[serde(rename = "retentionPolicyActive")]
    retention_policy_active: bool,
    archived: usize,
    retained: usize,
    archives: Vec<TimestampedRowArchive>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimestampedRowArchive {
    schema: String,
    cf_name: String,
    key_hex: String,
    row_timestamp_millis: i64,
    archived_at_unix: i64,
    retention_class: String,
    minimum_retention_days: u32,
    value_encoding: String,
    value_len_bytes: u64,
    value_sha256_hex: String,
    value_hex: String,
    archive_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorOverrideRetentionRow {
    _prediction_id: [u8; 16],
    _override_verdict: OperatorOverrideRetentionVerdict,
    _reason: String,
    _operator_id: String,
    created_at_unix_ms: i64,
    _sampling_weight_multiplier: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
enum OperatorOverrideRetentionVerdict {
    Pass,
    Fail,
    Abstain,
    OutOfDistribution,
}

fn prune_cf_keep_latest(
    env: &HygieneEnv,
    cf_name: &str,
    keep: usize,
    retention_rules: &BTreeMap<String, RetentionRule>,
) -> OpsResult<serde_json::Value> {
    let mut rows = scan_cf(&env.config.db, cf_name)?;
    if rows.len() <= keep {
        return Ok(serde_json::json!({ "cf": cf_name, "deleted": 0, "kept": rows.len() }));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let delete_count = rows.len() - keep;
    let metas = list_meta(&env.config.db)?;
    let meta_by_key = metas
        .iter()
        .filter(|m| m.entry_id.cf_name == cf_name)
        .map(|m| (m.entry_id.key.clone(), m.clone()))
        .collect::<BTreeMap<_, _>>();
    let gold_keys = meta_by_key
        .iter()
        .filter(|(_, meta)| meta.gold)
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    let retention_rule = retention_rules.get(cf_name);
    let mut batch = WriteBatch::default();
    let mut deleted = 0usize;
    let mut deleted_keys = Vec::new();
    let mut retention_skipped = Vec::new();
    for (key, _) in rows.into_iter().take(delete_count) {
        if gold_keys.contains(&key) {
            continue;
        }
        if let (Some(rule), Some(meta)) = (retention_rule, meta_by_key.get(&key)) {
            if retained_by_rule(meta, rule, env.now_unix()) {
                retention_skipped.push(serde_json::json!({
                    "key_hex": hex::encode(&key),
                    "created_unix": meta.created_unix,
                    "retained_until_unix": rule.retained_until_unix(meta.created_unix),
                    "retention_class": &rule.retention_class,
                    "minimum_retention_days": rule.minimum_retention_days
                }));
                continue;
            }
        }
        batch.delete_cf(cf(&env.config.db, cf_name)?, &key);
        batch.delete_cf(
            cf(
                &env.config.db,
                context_graph_mejepa_cf::CF_MEJEPA_PANEL_META,
            )?,
            crate::storage::meta_key(&EntryId::new(cf_name, key.clone())),
        );
        deleted_keys.push(hex::encode(&key));
        deleted += 1;
    }
    if deleted > 0 {
        env.config.db.write(batch)?;
    }
    Ok(serde_json::json!({
        "cf": cf_name,
        "deleted": deleted,
        "keptAtLeast": keep,
        "deletedKeysHex": deleted_keys,
        "retentionSkipped": retention_skipped,
        "retentionPolicyActive": retention_rule.is_some()
    }))
}

fn retention_rules_for_env(env: &HygieneEnv) -> OpsResult<BTreeMap<String, RetentionRule>> {
    let Some(path) = &env.config.retention_policy_path else {
        return Ok(BTreeMap::new());
    };
    load_retention_policy(path)?.by_cf()
}

fn gc_lock_path() -> PathBuf {
    let root = std::env::var("CONTEXTGRAPH_DATA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/zfs/archive/contextgraph"));
    root.join("state/locks/mejepa-gc.lock")
}
