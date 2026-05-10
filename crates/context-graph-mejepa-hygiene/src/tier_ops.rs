// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use rocksdb::WriteBatch;

use crate::entry::{EntryId, HygieneEntryMeta, TierTransition};
use crate::error::{OpsError, OpsErrorKind, OpsResult};
use crate::reports::TierTransitionReport;
use crate::storage::{cf, encode_cf_json, list_meta, meta_key, read_meta, write_meta, HygieneEnv};
use crate::tier::{decayed_score, decode_from_tier, encode_for_tier, tier_for_score, Tier};

pub fn tier_demote(
    env: &HygieneEnv,
    entry_id: &EntryId,
    target: Tier,
) -> OpsResult<TierTransition> {
    transition(env, entry_id, target, "demote")
}

pub fn tier_promote(
    env: &HygieneEnv,
    entry_id: &EntryId,
    target: Tier,
) -> OpsResult<TierTransition> {
    transition(env, entry_id, target, "promote")
}

pub fn tier_demote_all(env: &HygieneEnv) -> OpsResult<TierTransitionReport> {
    let mut transitions = Vec::new();
    let mut corrupt_entries = Vec::new();
    let mut fidelity_lost_entries = Vec::new();
    for meta in list_meta(&env.config.db)? {
        if meta.corrupt {
            corrupt_entries.push(meta.entry_id.clone());
            continue;
        }
        let score = decayed_score(
            meta.frequency.score,
            meta.frequency.last_read_unix,
            env.now_unix(),
        )?;
        let target = tier_for_score(score)?;
        if target < meta.tier {
            match tier_demote(env, &meta.entry_id, target) {
                Ok(report) => {
                    if target.is_lossy() {
                        fidelity_lost_entries.push(meta.entry_id.clone());
                    }
                    transitions.push(report);
                }
                Err(err) => {
                    tracing::error!(error = ?err, entry = ?meta.entry_id, "tier demotion failed");
                    corrupt_entries.push(meta.entry_id.clone());
                }
            }
        }
    }
    Ok(TierTransitionReport {
        transitions,
        corrupt_entries,
        fidelity_lost_entries,
    })
}

pub fn tier_promote_all(env: &HygieneEnv) -> OpsResult<TierTransitionReport> {
    let mut transitions = Vec::new();
    let mut corrupt_entries = Vec::new();
    let mut fidelity_lost_entries = Vec::new();
    for meta in list_meta(&env.config.db)? {
        if meta.corrupt {
            corrupt_entries.push(meta.entry_id.clone());
            continue;
        }
        let score = decayed_score(
            meta.frequency.score,
            meta.frequency.last_read_unix,
            env.now_unix(),
        )?;
        let target = tier_for_score(score)?;
        if target > meta.tier {
            match tier_promote(env, &meta.entry_id, target) {
                Ok(report) => {
                    if target.is_lossy() {
                        fidelity_lost_entries.push(meta.entry_id.clone());
                    }
                    transitions.push(report);
                }
                Err(err) => {
                    tracing::error!(error = ?err, entry = ?meta.entry_id, "tier promotion failed");
                    corrupt_entries.push(meta.entry_id.clone());
                }
            }
        }
    }
    Ok(TierTransitionReport {
        transitions,
        corrupt_entries,
        fidelity_lost_entries,
    })
}

fn transition(
    env: &HygieneEnv,
    entry_id: &EntryId,
    target: Tier,
    reason: &str,
) -> OpsResult<TierTransition> {
    let entry_lock = env.lock_for(entry_id);
    let _guard = entry_lock.lock();
    let db = &env.config.db;
    let mut meta = read_meta(db, entry_id)?.ok_or_else(|| {
        OpsError::new(OpsErrorKind::CorruptMetadata {
            key_hex: hex::encode(meta_key(entry_id)),
            detail: "missing hygiene metadata for tier transition".to_string(),
        })
    })?;
    if meta.tier == target {
        return Ok(TierTransition {
            from: meta.tier,
            to: target,
            at_unix: env.now_unix(),
            reason: "noop".to_string(),
            before_bytes: meta.size_bytes,
            after_bytes: meta.size_bytes,
        });
    }
    if reason == "demote" && target > meta.tier {
        return Err(OpsError::invalid(
            "tier.target",
            format!(
                "demote target {target:?} is hotter than current {:?}",
                meta.tier
            ),
        ));
    }
    if reason == "promote" && target < meta.tier {
        return Err(OpsError::invalid(
            "tier.target",
            format!(
                "promote target {target:?} is colder than current {:?}",
                meta.tier
            ),
        ));
    }
    let handle = cf(db, &entry_id.cf_name)?;
    let bytes = db.get_cf(handle, &entry_id.key)?.ok_or_else(|| {
        OpsError::invalid(
            "tier.entry",
            format!(
                "missing source row {}:{}",
                entry_id.cf_name,
                hex::encode(&entry_id.key)
            ),
        )
    })?;
    let values = match decode_from_tier(&bytes, meta.tier) {
        Ok(values) => values,
        Err(err) => {
            meta.corrupt = true;
            write_meta(db, &meta)?;
            return Err(err);
        }
    };
    let encoded = match encode_for_tier(&values, target) {
        Ok(bytes) => bytes,
        Err(err) => {
            meta.corrupt = true;
            write_meta(db, &meta)?;
            return Err(err);
        }
    };
    let transition = TierTransition {
        from: meta.tier,
        to: target,
        at_unix: env.now_unix(),
        reason: reason.to_string(),
        before_bytes: bytes.len() as u64,
        after_bytes: encoded.len() as u64,
    };
    meta.tier = target;
    meta.size_bytes = encoded.len() as u64;
    meta.updated_unix = env.now_unix();
    meta.fidelity_lost |= target.is_lossy();
    if meta.transition_log.len() >= HygieneEntryMeta::MAX_TRANSITION_LOG_ENTRIES {
        return Err(OpsError::invalid(
            "tier.transition_log",
            format!(
                "transition log already has {} entries; manual compaction required",
                meta.transition_log.len()
            ),
        ));
    }
    meta.transition_log.push(transition.clone());

    let mut batch = WriteBatch::default();
    batch.put_cf(handle, &entry_id.key, &encoded);
    batch.put_cf(
        cf(db, context_graph_mejepa_cf::CF_MEJEPA_PANEL_META)?,
        meta_key(entry_id),
        encode_cf_json(&meta)?,
    );
    db.write(batch)?;
    let readback = db
        .get_cf(handle, &entry_id.key)?
        .ok_or_else(|| OpsError::invalid("tier.readback", "missing row after tier transition"))?;
    if readback.as_slice() != encoded.as_slice() {
        return Err(OpsError::invalid(
            "tier.readback",
            "row bytes differ after tier transition",
        ));
    }
    let readback_meta = read_meta(db, entry_id)?.ok_or_else(|| {
        OpsError::invalid("tier.readback_meta", "missing meta after tier transition")
    })?;
    if readback_meta.tier != target {
        return Err(OpsError::invalid(
            "tier.readback_meta",
            "metadata tier does not match target after transition",
        ));
    }
    Ok(transition)
}
