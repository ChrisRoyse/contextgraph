// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::path::PathBuf;

use rocksdb::WriteBatch;

use crate::entry::EntryId;
use crate::error::OpsResult;
use crate::quota::{quota_check_and_evict, quota_status};
use crate::reports::{GcReport, GcStepReport};
use crate::storage::{
    cf, encode_cf_json, list_meta, open_exclusive_lock, put_readback, scan_cf, HygieneEnv,
};
use crate::tier_ops::tier_demote_all;
use crate::witness_compress::{verify_witness_integrity, witness_compress_old_segments};

pub fn gc_run_nightly(env: &HygieneEnv) -> OpsResult<GcReport> {
    let started_unix = env.now_unix();
    let lock_path = gc_lock_path();
    let _lock = open_exclusive_lock(&lock_path)?;

    let mut steps = Vec::new();
    steps.push(step("tier_demote", || {
        Ok(serde_json::to_value(tier_demote_all(env)?)?)
    })?);
    steps.push(step("compact_cf", || compact_hygiene_cfs(env))?);
    steps.push(step("quota_check_and_evict", || {
        Ok(serde_json::to_value(quota_check_and_evict(env)?)?)
    })?);
    steps.push(step("prune_checkpoints", || {
        prune_cf_keep_latest(env, context_graph_mejepa_cf::CF_MEJEPA_WEIGHT_BLOBS, 10)
    })?);
    steps.push(step("prune_calibration", || {
        prune_cf_keep_latest(
            env,
            context_graph_mejepa_cf::CF_MEJEPA_CALIBRATION_HISTORY,
            30,
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

fn prune_cf_keep_latest(
    env: &HygieneEnv,
    cf_name: &str,
    keep: usize,
) -> OpsResult<serde_json::Value> {
    let mut rows = scan_cf(&env.config.db, cf_name)?;
    if rows.len() <= keep {
        return Ok(serde_json::json!({ "cf": cf_name, "deleted": 0, "kept": rows.len() }));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let delete_count = rows.len() - keep;
    let metas = list_meta(&env.config.db)?;
    let gold_keys = metas
        .into_iter()
        .filter(|m| m.entry_id.cf_name == cf_name && m.gold)
        .map(|m| m.entry_id.key)
        .collect::<std::collections::BTreeSet<_>>();
    let mut batch = WriteBatch::default();
    let mut deleted = 0usize;
    for (key, _) in rows.into_iter().take(delete_count) {
        if gold_keys.contains(&key) {
            continue;
        }
        batch.delete_cf(cf(&env.config.db, cf_name)?, &key);
        batch.delete_cf(
            cf(
                &env.config.db,
                context_graph_mejepa_cf::CF_MEJEPA_PANEL_META,
            )?,
            crate::storage::meta_key(&EntryId::new(cf_name, key.clone())),
        );
        deleted += 1;
    }
    if deleted > 0 {
        env.config.db.write(batch)?;
    }
    Ok(serde_json::json!({ "cf": cf_name, "deleted": deleted, "keptAtLeast": keep }))
}

fn gc_lock_path() -> PathBuf {
    let root = std::env::var("CONTEXTGRAPH_DATA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/mnt/d/contextgraph"));
    root.join("state/locks/mejepa-gc.lock")
}
