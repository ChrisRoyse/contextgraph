// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::fs;
use std::path::Path;

use context_graph_mejepa_hygiene::{
    audit_verify_old_entry, count_and_bytes_cf, open_hygiene_rocksdb, put_readback, runtime_config,
    verify_witness_integrity, witness_compress_old_segments, HygieneEnv,
};
use context_graph_witness::{WitnessEntry, HASH_SIZE, ZERO_HASH};
use serde::Serialize;

const ROOT: &str = "/zfs/archive/contextgraph/fsv/contextgraph-mejepa-hygiene-merkle-fsv";

#[derive(Debug, Serialize)]
struct CaseEvidence {
    name: String,
    before_rows: u64,
    after_rows: u64,
    outcome: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct Evidence {
    source_of_truth: String,
    happy_path: CaseEvidence,
    edge_cases: Vec<CaseEvidence>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(ROOT);
    reset(root)?;
    let happy = run_case(&root.join("happy"), 2048, 1024, true)?;
    let empty = run_case(&root.join("edge-empty"), 0, 1024, false)?;
    let partial = run_case(&root.join("edge-partial"), 500, 1024, false)?;
    let trailing = run_case(&root.join("edge-trailing"), 1500, 1024, true)?;
    let tampered = run_tamper_case(&root.join("edge-tamper"))?;
    let evidence = Evidence {
        source_of_truth: format!(
            "{} + {} + archive files under {ROOT}",
            context_graph_mejepa_cf::CF_MEJEPA_WITNESS_CHAIN,
            context_graph_mejepa_cf::CF_MEJEPA_WITNESS_SEGMENT_META
        ),
        happy_path: happy,
        edge_cases: vec![empty, partial, trailing, tampered],
    };
    write_json_0600(&root.join("aggregate-evidence.json"), &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn run_case(
    root: &Path,
    entries: u64,
    segment_size: usize,
    expect_compressed: bool,
) -> Result<CaseEvidence, Box<dyn std::error::Error>> {
    let env = env(root, segment_size)?;
    seed_witness(&env, entries)?;
    let before = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_WITNESS_CHAIN,
    )?
    .0;
    let report = witness_compress_old_segments(&env)?;
    let after = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_WITNESS_CHAIN,
    )?
    .0;
    let integrity = verify_witness_integrity(&env)?;
    if expect_compressed {
        let expected_after = (entries / segment_size as u64) + (entries % segment_size as u64);
        assert_eq!(after, expected_after);
        assert!(audit_verify_old_entry(&env, 0, 17)?);
        assert!(integrity.compressed_entries > 0);
    } else {
        assert_eq!(before, after);
        assert_eq!(report.segments_compressed, 0);
    }
    Ok(CaseEvidence {
        name: format!("entries-{entries}-segment-{segment_size}"),
        before_rows: before,
        after_rows: after,
        outcome: serde_json::json!({ "compression": report, "integrity": integrity }),
    })
}

fn run_tamper_case(root: &Path) -> Result<CaseEvidence, Box<dyn std::error::Error>> {
    let env = env(root, 1024)?;
    seed_witness(&env, 1024)?;
    let before = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_WITNESS_CHAIN,
    )?
    .0;
    let report = witness_compress_old_segments(&env)?;
    let path = report.segment_metas[0].archive_path.clone();
    let mut bytes = fs::read(&path)?;
    bytes[0] ^= 0x7f;
    fs::write(&path, &bytes)?;
    let err = audit_verify_old_entry(&env, 0, 0).expect_err("tampered archive must fail");
    let after = count_and_bytes_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_WITNESS_CHAIN,
    )?
    .0;
    Ok(CaseEvidence {
        name: "tampered-archive".to_string(),
        before_rows: before,
        after_rows: after,
        outcome: serde_json::json!({ "errorCode": err.code, "errorKind": err.kind }),
    })
}

fn env(root: &Path, segment_size: usize) -> Result<HygieneEnv, Box<dyn std::error::Error>> {
    let db = open_hygiene_rocksdb(root.join("rocksdb"))?;
    let mut config = runtime_config(db, root.join("archives"))?;
    config.witness_segment_size = segment_size;
    config.witness_min_age_days = 1;
    config.now_unix = Some(2_000_000_000);
    Ok(HygieneEnv::try_new(config)?)
}

fn seed_witness(env: &HygieneEnv, entries: u64) -> Result<(), Box<dyn std::error::Error>> {
    let mut prev = ZERO_HASH;
    for idx in 0..entries {
        let mut action = [0u8; HASH_SIZE];
        action[..8].copy_from_slice(&idx.to_be_bytes());
        let entry = WitnessEntry::new(prev, action, 1_900_000_000_000_000_000 + idx, 7);
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
