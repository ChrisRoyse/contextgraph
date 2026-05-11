// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::fs;

use context_graph_mejepa_hygiene::{
    gc_run_nightly, open_hygiene_rocksdb, put_readback, runtime_config, write_meta, EntryId,
    HygieneEntryMeta, HygieneEnv, StorageCategory, Tier,
};
use context_graph_witness::{WitnessEntry, HASH_SIZE, ZERO_HASH};

pub(super) fn run_stress(
    args: super::StressArgs,
) -> Result<serde_json::Value, context_graph_mejepa_hygiene::OpsError> {
    if args.entries < 1_024 {
        return Err(context_graph_mejepa_hygiene::OpsError::invalid(
            "entries",
            "stress requires at least 1024 entries so witness compression is exercised",
        ));
    }
    if args.gc_passes == 0 {
        return Err(context_graph_mejepa_hygiene::OpsError::invalid(
            "gc_passes",
            "gc_passes must be >= 1",
        ));
    }
    if args.output_fsv.exists() {
        fs::remove_dir_all(&args.output_fsv).map_err(|err| {
            context_graph_mejepa_hygiene::OpsError::io("remove_dir_all", &args.output_fsv, err)
        })?;
    }
    fs::create_dir_all(&args.output_fsv).map_err(|err| {
        context_graph_mejepa_hygiene::OpsError::io("create_dir_all", &args.output_fsv, err)
    })?;
    let db_path = args.output_fsv.join("hygiene-rocksdb");
    let archive_root = args.output_fsv.join("archives");
    let db = open_hygiene_rocksdb(&db_path)?;
    let base_now = 2_000_000_000i64;
    let mut config = runtime_config(db.clone(), archive_root.clone())?;
    config.witness_segment_size = 1_024;
    config.witness_min_age_days = 1;
    config.now_unix = Some(base_now);
    let env = HygieneEnv::try_new(config.clone())?;
    seed_hygiene_stress(&env, args.entries, base_now - 100 * 86_400)?;

    let mut reports = Vec::new();
    for pass in 0..args.gc_passes {
        config.now_unix = Some(base_now + pass as i64 * 86_400);
        let pass_env = HygieneEnv::try_new(config.clone())?;
        reports.push(gc_run_nightly(&pass_env)?);
    }
    let final_env = HygieneEnv::try_new(config)?;
    let quota = context_graph_mejepa_hygiene::quota_status(&final_env)?;
    let witness = context_graph_mejepa_hygiene::verify_witness_integrity(&final_env)?;
    let (_, gc_rows) = context_graph_mejepa_hygiene::count_and_bytes_cf(
        &db,
        context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY,
    )?;
    Ok(serde_json::json!({
        "sourceOfTruth": {
            "rocksdb": db_path,
            "archiveRoot": archive_root,
            "gcHistoryCf": context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY
        },
        "stressEntries": args.entries,
        "gcPassesCompleted": reports.len(),
        "witnessChainCompressed": witness.compressed_entries > 0,
        "quotaViolations": quota.categories.iter().filter(|row| row.over_budget).count(),
        "totalDiskUsedGb": quota.total_used_bytes as f64 / 1024.0 / 1024.0 / 1024.0,
        "totalDiskUsedBytes": quota.total_used_bytes,
        "gcHistoryBytes": gc_rows,
        "witnessAfter": witness,
        "quotaAfter": quota
    }))
}

fn seed_hygiene_stress(
    env: &HygieneEnv,
    entries: u64,
    old_unix: i64,
) -> Result<(), context_graph_mejepa_hygiene::OpsError> {
    let vector = [0.125f32, -0.25, 0.5, -1.0, 2.0, -4.0, 8.0, -16.0];
    let value = context_graph_mejepa_hygiene::encode_for_tier(&vector, Tier::Hot)?;
    for idx in 0..entries {
        let key = idx.to_be_bytes();
        put_readback(
            &env.config.db,
            context_graph_mejepa_cf::CF_MEJEPA_PANEL_CACHE,
            &key,
            &value,
        )?;
        let mut meta = HygieneEntryMeta::new(
            EntryId::new(context_graph_mejepa_cf::CF_MEJEPA_PANEL_CACHE, key),
            StorageCategory::LivePanelCacheRam,
            Tier::Hot,
            value.len() as u64,
            old_unix,
        );
        meta.frequency.score = 0.1;
        write_meta(&env.config.db, &meta)?;
    }
    let mut prev = ZERO_HASH;
    let witness_entries = entries.clamp(2_048, 10_240);
    for idx in 0..witness_entries {
        let mut action = [0u8; HASH_SIZE];
        action[..8].copy_from_slice(&idx.to_be_bytes());
        let entry = WitnessEntry::new(prev, action, (old_unix as u64) * 1_000_000_000 + idx, 7);
        put_readback(
            &env.config.db,
            context_graph_mejepa_cf::CF_MEJEPA_WITNESS_CHAIN,
            &idx.to_be_bytes(),
            &entry.to_bytes(),
        )?;
        prev = entry.chain_hash();
    }
    Ok(())
}
