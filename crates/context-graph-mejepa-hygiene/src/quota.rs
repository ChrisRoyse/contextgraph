// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::fs;
use std::path::Path;

use rocksdb::WriteBatch;

use crate::categories::StorageCategory;
use crate::error::{OpsError, OpsErrorKind, OpsResult};
use crate::quota_types::{EvictionCandidate, QuotaStateRecord};
use crate::reports::{EvictionRecord, QuotaCategoryStatus, QuotaEvictionReport, QuotaStatus};
use crate::storage::{
    cf, count_and_bytes_cf, encode_cf_json, list_meta, meta_key, open_exclusive_lock,
    operation_lock_path, put_readback, read_meta, HygieneEnv,
};

const UNRECOVERABLE_PREFIX: &[u8] = b"unrecoverable:";
const LAST_REPORT_KEY: &[u8] = b"last_quota_report";
pub const QUOTA_LAST_REPORT_KEY: &[u8] = LAST_REPORT_KEY;

pub fn quota_status(env: &HygieneEnv) -> OpsResult<QuotaStatus> {
    let mut categories = Vec::new();
    let mut total_used = 0u64;
    let mut unrecoverable_categories = Vec::new();
    for category in StorageCategory::all() {
        let mut used = 0u64;
        for cf_name in category.cf_names() {
            used = used.saturating_add(count_and_bytes_cf(&env.config.db, cf_name)?.1);
        }
        if category == StorageCategory::WitnessChains {
            used = used.saturating_add(archive_dir_size(&env.config.archive_root)?);
        }
        total_used = total_used.saturating_add(used);
        let budget = category.budget_bytes(env.config.total_quota_bytes);
        if is_unrecoverable(env, category)? {
            unrecoverable_categories.push(category);
        }
        categories.push(QuotaCategoryStatus {
            category,
            used_bytes: used,
            budget_bytes: budget,
            over_budget: used > budget,
        });
    }
    Ok(QuotaStatus {
        total_used_bytes: total_used,
        total_quota_bytes: env.config.total_quota_bytes,
        categories,
        unrecoverable_categories,
    })
}

pub fn quota_check_and_evict(env: &HygieneEnv) -> OpsResult<QuotaEvictionReport> {
    let _lock = open_exclusive_lock(&operation_lock_path(&env.config.archive_root, "quota"))?;
    clear_recoverable_states(env)?;
    let before = quota_status(env)?;
    if let Some(category) = before.unrecoverable_categories.first().copied() {
        let row = before
            .categories
            .iter()
            .find(|row| row.category == category)
            .ok_or_else(|| {
                OpsError::invalid(
                    "quota.status",
                    format!(
                        "unrecoverable category {} missing status row",
                        category.as_str()
                    ),
                )
            })?;
        return Err(OpsError::new(OpsErrorKind::QuotaUnrecoverable {
            category: category.as_str().to_string(),
            used_bytes: row.used_bytes,
            budget_bytes: row.budget_bytes,
        }));
    }
    let mut evicted = Vec::new();
    for row in before.categories.iter().filter(|row| row.over_budget) {
        let mut candidates = collect_candidates(env, row.category)?;
        candidates.sort_by(|a, b| {
            a.gold
                .cmp(&b.gold)
                .then_with(|| a.score.total_cmp(&b.score))
                .then_with(|| a.last_read_unix.cmp(&b.last_read_unix))
        });
        let mut used = row.used_bytes;
        for candidate in candidates.into_iter().filter(|candidate| !candidate.gold) {
            if used <= row.budget_bytes {
                break;
            }
            delete_candidate(env, &candidate)?;
            used = used.saturating_sub(candidate.size_bytes);
            evicted.push(EvictionRecord {
                entry_id: candidate.entry_id,
                category: candidate.category,
                bytes_deleted: candidate.size_bytes,
                score: candidate.score,
                tier: candidate.tier,
            });
        }
        if used > row.budget_bytes {
            mark_unrecoverable(env, row.category, used, row.budget_bytes)?;
            return Err(OpsError::new(OpsErrorKind::QuotaUnrecoverable {
                category: row.category.as_str().to_string(),
                used_bytes: used,
                budget_bytes: row.budget_bytes,
            }));
        }
    }
    let after = quota_status(env)?;
    let report = QuotaEvictionReport {
        before,
        after,
        evicted,
    };
    record_quota_report(env, &report)?;
    Ok(report)
}

pub fn record_quota_report(env: &HygieneEnv, report: &QuotaEvictionReport) -> OpsResult<()> {
    put_readback(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
        LAST_REPORT_KEY,
        &encode_cf_json(&report)?,
    )
}

fn collect_candidates(
    env: &HygieneEnv,
    category: StorageCategory,
) -> OpsResult<Vec<EvictionCandidate>> {
    if !category.quota_evictable() {
        return Ok(Vec::new());
    }
    let mut candidates = Vec::new();
    for meta in list_meta(&env.config.db)? {
        if meta.category != category || meta.corrupt {
            continue;
        }
        candidates.push(EvictionCandidate {
            entry_id: meta.entry_id.clone(),
            category,
            size_bytes: meta.size_bytes,
            score: meta.frequency.score,
            tier: meta.tier,
            gold: meta.gold,
            last_read_unix: meta.frequency.last_read_unix,
        });
    }
    Ok(candidates)
}

fn delete_candidate(env: &HygieneEnv, candidate: &EvictionCandidate) -> OpsResult<()> {
    let mut batch = WriteBatch::default();
    batch.delete_cf(
        cf(&env.config.db, &candidate.entry_id.cf_name)?,
        &candidate.entry_id.key,
    );
    batch.delete_cf(
        cf(
            &env.config.db,
            context_graph_mejepa_cf::CF_MEJEPA_PANEL_META,
        )?,
        meta_key(&candidate.entry_id),
    );
    env.config.db.write(batch)?;
    let value = env.config.db.get_cf(
        cf(&env.config.db, &candidate.entry_id.cf_name)?,
        &candidate.entry_id.key,
    )?;
    if value.is_some() {
        return Err(OpsError::invalid(
            "quota.evict_readback",
            "candidate still exists after eviction",
        ));
    }
    let meta = read_meta(&env.config.db, &candidate.entry_id)?;
    if meta.is_some() {
        return Err(OpsError::invalid(
            "quota.evict_meta_readback",
            "candidate metadata still exists after eviction",
        ));
    }
    Ok(())
}

fn mark_unrecoverable(
    env: &HygieneEnv,
    category: StorageCategory,
    used: u64,
    budget: u64,
) -> OpsResult<()> {
    let record = QuotaStateRecord {
        category,
        unrecoverable: true,
        reason: format!(
            "all evictable candidates exhausted; used_bytes={used}, budget_bytes={budget}"
        ),
        updated_unix: env.now_unix(),
    };
    put_readback(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
        &unrecoverable_key(category),
        &encode_cf_json(&record)?,
    )
}

fn is_unrecoverable(env: &HygieneEnv, category: StorageCategory) -> OpsResult<bool> {
    Ok(env
        .config
        .db
        .get_cf(
            cf(
                &env.config.db,
                context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
            )?,
            unrecoverable_key(category),
        )?
        .is_some())
}

fn clear_recoverable_states(env: &HygieneEnv) -> OpsResult<()> {
    let mut batch = WriteBatch::default();
    let mut changed = false;
    let quota_cf = cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
    )?;
    for category in StorageCategory::all() {
        if !is_unrecoverable(env, category)? {
            continue;
        }
        let mut used = 0u64;
        for cf_name in category.cf_names() {
            used = used.saturating_add(count_and_bytes_cf(&env.config.db, cf_name)?.1);
        }
        if category == StorageCategory::WitnessChains {
            used = used.saturating_add(archive_dir_size(&env.config.archive_root)?);
        }
        if used <= category.budget_bytes(env.config.total_quota_bytes) {
            batch.delete_cf(quota_cf, unrecoverable_key(category));
            changed = true;
        }
    }
    if changed {
        env.config.db.write(batch)?;
        for category in StorageCategory::all() {
            let key = unrecoverable_key(category);
            let value = env.config.db.get_cf(quota_cf, &key)?;
            if value.is_some() {
                let mut used = 0u64;
                for cf_name in category.cf_names() {
                    used = used.saturating_add(count_and_bytes_cf(&env.config.db, cf_name)?.1);
                }
                if category == StorageCategory::WitnessChains {
                    used = used.saturating_add(archive_dir_size(&env.config.archive_root)?);
                }
                if used <= category.budget_bytes(env.config.total_quota_bytes) {
                    return Err(OpsError::invalid(
                        "quota.unrecoverable_readback",
                        format!(
                            "stale unrecoverable state remained for {} after clear",
                            category.as_str()
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn unrecoverable_key(category: StorageCategory) -> Vec<u8> {
    let mut out = UNRECOVERABLE_PREFIX.to_vec();
    out.extend_from_slice(category.as_str().as_bytes());
    out
}

fn archive_dir_size(path: &Path) -> OpsResult<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in fs::read_dir(path).map_err(|err| OpsError::io("read_dir", path, err))? {
        let entry = entry.map_err(|err| OpsError::io("read_dir_entry", path, err))?;
        let entry_path = entry.path();
        let meta = fs::symlink_metadata(&entry_path)
            .map_err(|err| OpsError::io("metadata", &entry_path, err))?;
        if meta.file_type().is_symlink() {
            return Err(OpsError::invalid(
                "archive_root",
                format!("archive tree contains symlink {}", entry_path.display()),
            ));
        }
        if meta.is_dir() {
            total = total.saturating_add(archive_dir_size(&entry_path)?);
        } else {
            total = total.saturating_add(meta.len());
        }
    }
    Ok(total)
}
