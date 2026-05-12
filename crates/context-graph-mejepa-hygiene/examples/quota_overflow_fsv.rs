// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::fs;
use std::path::Path;

use context_graph_mejepa_hygiene::{
    count_and_bytes_cf, open_hygiene_rocksdb, put_readback, quota_check_and_evict, quota_status,
    runtime_config, write_meta, EntryId, HygieneEntryMeta, HygieneEnv, StorageCategory, Tier,
};
use serde::Serialize;

const ROOT: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-hygiene-quota-fsv";
const MIB: usize = 1024 * 1024;

#[derive(Debug, Serialize)]
struct Evidence {
    source_of_truth: String,
    happy_path: serde_json::Value,
    edge_cases: Vec<serde_json::Value>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("CG_MEJEPA_DISK_QUOTA_GB", "1");
    let root = Path::new(ROOT);
    reset(root)?;
    let happy = happy_path(&root.join("happy"))?;
    let empty = empty_case(&root.join("edge-empty"))?;
    let invalid_env = invalid_env_case();
    std::env::set_var("CG_MEJEPA_DISK_QUOTA_GB", "1");
    let gold = all_gold_case(&root.join("edge-all-gold"))?;
    let unknown = unknown_rows_case(&root.join("edge-unknown-rows"))?;
    let evidence = Evidence {
        source_of_truth: format!(
            "{} scanned directly, with unrecoverable state in {}",
            context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
            context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE
        ),
        happy_path: happy,
        edge_cases: vec![empty, invalid_env, gold, unknown],
    };
    write_json_0600(&root.join("aggregate-evidence.json"), &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn happy_path(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let env = env(root)?;
    seed_bytes(&env, 6, false)?;
    let before = quota_status(&env)?;
    let report = quota_check_and_evict(&env)?;
    let after = quota_status(&env)?;
    let rows = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
    )?;
    assert!(before.categories.iter().any(|row| row.over_budget));
    assert!(!after.categories.iter().any(|row| row.over_budget));
    assert!(report.evicted.len() >= 2);
    Ok(
        serde_json::json!({ "before": before, "evictionReport": report, "after": after, "shiftCfRowsBytes": rows }),
    )
}

fn empty_case(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let env = env(root)?;
    let before = quota_status(&env)?;
    let report = quota_check_and_evict(&env)?;
    let after = quota_status(&env)?;
    assert_eq!(before.total_used_bytes, after.total_used_bytes);
    assert!(report.evicted.is_empty());
    Ok(serde_json::json!({ "name": "empty-db", "before": before, "after": after }))
}

fn invalid_env_case() -> serde_json::Value {
    std::env::set_var("CG_MEJEPA_DISK_QUOTA_GB", "0");
    let err = context_graph_mejepa_hygiene::quota_bytes_from_env()
        .expect_err("zero quota must fail closed");
    serde_json::json!({ "name": "invalid-quota-env", "errorCode": err.code, "errorKind": err.kind })
}

fn all_gold_case(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let env = env(root)?;
    seed_bytes(&env, 6, true)?;
    let before = quota_status(&env)?;
    let err = quota_check_and_evict(&env).expect_err("all-gold overflow must be unrecoverable");
    let state_rows = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
    )?;
    assert!(state_rows.0 >= 1);
    Ok(
        serde_json::json!({ "name": "all-gold-overflow", "before": before, "errorCode": err.code, "errorKind": err.kind, "quotaStateRowsBytes": state_rows }),
    )
}

fn unknown_rows_case(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let env = env(root)?;
    let payload = vec![0xD6u8; MIB];
    for idx in 0..6u64 {
        put_readback(
            &env.config.db,
            context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
            &idx.to_be_bytes(),
            &payload,
        )?;
    }
    let before = quota_status(&env)?;
    let err = quota_check_and_evict(&env).expect_err("unknown rows must fail closed");
    let after_rows = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
    )?;
    assert_eq!(after_rows, (6, (6 * MIB) as u64));
    Ok(
        serde_json::json!({ "name": "unknown-rows-fail-closed", "before": before, "errorCode": err.code, "errorKind": err.kind, "shiftCfRowsBytesAfter": after_rows }),
    )
}

fn env(root: &Path) -> Result<HygieneEnv, Box<dyn std::error::Error>> {
    let db = open_hygiene_rocksdb(root.join("rocksdb"))?;
    let mut config = runtime_config(db, root.join("archives"))?;
    config.now_unix = Some(2_000_000_000);
    Ok(HygieneEnv::try_new(config)?)
}

fn seed_bytes(env: &HygieneEnv, rows: u64, gold: bool) -> Result<(), Box<dyn std::error::Error>> {
    let payload = vec![0xA5u8; MIB];
    for idx in 0..rows {
        let key = idx.to_be_bytes();
        put_readback(
            &env.config.db,
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
        write_meta(&env.config.db, &meta)?;
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
