// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::fs;
use std::path::Path;

use context_graph_mejepa_hygiene::{
    gc_run_nightly, open_hygiene_rocksdb, put_readback, quota_status, runtime_config,
    verify_witness_integrity, write_meta, EntryId, HygieneEntryMeta, HygieneEnv, StorageCategory,
    Tier,
};
use context_graph_witness::{WitnessEntry, HASH_SIZE, ZERO_HASH};
use serde::Serialize;

const ROOT: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-hygiene-gc-stress-fsv";

#[derive(Debug, Serialize)]
struct Evidence {
    source_of_truth: String,
    happy_path: serde_json::Value,
    edge_cases: Vec<serde_json::Value>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(ROOT);
    reset(root)?;
    let happy = stress(&root.join("happy"), None, 10_000, 7)?;
    let empty = stress(&root.join("edge-empty"), Some("empty-db"), 0, 1)?;
    let invalid_config = invalid_config_case();
    let corrupt = corrupt_tier_case(&root.join("edge-corrupt-tier"))?;
    let evidence = Evidence {
        source_of_truth: format!(
            "{}, {}, witness archives, and {}",
            context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY,
            context_graph_mejepa_cf::CF_MEJEPA_PANEL_CACHE,
            context_graph_mejepa_cf::CF_MEJEPA_WITNESS_SEGMENT_META
        ),
        happy_path: happy,
        edge_cases: vec![empty, invalid_config, corrupt],
    };
    write_json_0600(&root.join("aggregate-evidence.json"), &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn stress(
    root: &Path,
    name: Option<&str>,
    entries: u64,
    passes: u32,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let db = open_hygiene_rocksdb(root.join("rocksdb"))?;
    let mut config = runtime_config(db.clone(), root.join("archives"))?;
    config.witness_segment_size = 1024;
    config.witness_min_age_days = 1;
    config.now_unix = Some(2_000_000_000);
    let env = HygieneEnv::try_new(config.clone())?;
    seed(&env, entries)?;
    let before_quota = quota_status(&env)?;
    let mut reports = Vec::new();
    for pass in 0..passes {
        config.now_unix = Some(2_000_000_000 + pass as i64 * 86_400);
        let env = HygieneEnv::try_new(config.clone())?;
        reports.push(gc_run_nightly(&env)?);
    }
    let env = HygieneEnv::try_new(config)?;
    let after_quota = quota_status(&env)?;
    let witness = verify_witness_integrity(&env)?;
    if entries >= 1024 {
        assert!(witness.compressed_entries > 0);
    }
    assert!(!after_quota.categories.iter().any(|row| row.over_budget));
    let mut report = serde_json::json!({
        "entries": entries,
        "gcPassesCompleted": reports.len(),
        "beforeQuota": before_quota,
        "afterQuota": after_quota,
        "witnessAfter": witness,
        "latestReport": reports.last()
    });
    if let Some(name) = name {
        report["name"] = serde_json::json!(name);
    }
    Ok(report)
}

fn invalid_config_case() -> serde_json::Value {
    let db = match open_hygiene_rocksdb("/mnt/d/contextgraph/fsv/contextgraph-mejepa-hygiene-gc-invalid-db") {
        Ok(db) => db,
        Err(err) => {
            return serde_json::json!({
                "name": "invalid-config",
                "setupErrorCode": err.code,
                "setupErrorKind": err.kind
            });
        }
    };
    let mut config = match runtime_config(
        db,
        "/mnt/d/contextgraph/fsv/contextgraph-mejepa-hygiene-gc-invalid-archives".into(),
    ) {
        Ok(config) => config,
        Err(err) => {
            return serde_json::json!({
                "name": "invalid-config",
                "setupErrorCode": err.code,
                "setupErrorKind": err.kind
            });
        }
    };
    config.total_quota_bytes = 0;
    let err = match HygieneEnv::try_new(config) {
        Ok(_) => {
            return serde_json::json!({
                "name": "invalid-config",
                "error": "quota under 1GiB unexpectedly succeeded"
            });
        }
        Err(err) => err,
    };
    serde_json::json!({ "name": "invalid-config", "errorCode": err.code, "errorKind": err.kind })
}

fn corrupt_tier_case(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let db = open_hygiene_rocksdb(root.join("rocksdb"))?;
    let mut config = runtime_config(db.clone(), root.join("archives"))?;
    config.now_unix = Some(2_000_000_000);
    let env = HygieneEnv::try_new(config)?;
    put_readback(
        &db,
        context_graph_mejepa_cf::CF_MEJEPA_PANEL_CACHE,
        b"bad",
        b"\x01\x02",
    )?;
    let mut meta = HygieneEntryMeta::new(
        EntryId::new(
            context_graph_mejepa_cf::CF_MEJEPA_PANEL_CACHE,
            b"bad".to_vec(),
        ),
        StorageCategory::LivePanelCacheRam,
        Tier::Hot,
        2,
        env.now_unix(),
    );
    meta.frequency.score = 0.0;
    write_meta(&db, &meta)?;
    let report = context_graph_mejepa_hygiene::tier_demote_all(&env)?;
    let readback = context_graph_mejepa_hygiene::read_meta(
        &db,
        &EntryId::new(
            context_graph_mejepa_cf::CF_MEJEPA_PANEL_CACHE,
            b"bad".to_vec(),
        ),
    )?
    .ok_or("corrupt-tier-blob metadata disappeared after demotion attempt")?;
    assert!(readback.corrupt);
    Ok(serde_json::json!({ "name": "corrupt-tier-blob", "report": report, "metaAfter": readback }))
}

fn seed(env: &HygieneEnv, entries: u64) -> Result<(), Box<dyn std::error::Error>> {
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
            env.now_unix() - 100 * 86_400,
        );
        meta.frequency.score = 0.1;
        write_meta(&env.config.db, &meta)?;
    }
    let mut prev = ZERO_HASH;
    let witness_entries = entries.min(2048);
    for idx in 0..witness_entries {
        let mut action = [0u8; HASH_SIZE];
        action[..8].copy_from_slice(&idx.to_be_bytes());
        let entry = WitnessEntry::new(prev, action, 1_900_000_000_000_000_000 + idx, 9);
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

fn reset(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)
}

fn write_json_0600<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    std::io::Write::write_all(&mut options.open(path)?, &bytes)
}
