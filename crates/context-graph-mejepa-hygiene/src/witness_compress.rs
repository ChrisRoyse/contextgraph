// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use context_graph_witness::{
    shake256_32, MerkleLeaf, MerkleTree, WitnessEntry, WITNESS_ENTRY_SIZE, ZERO_HASH,
};
use rocksdb::WriteBatch;
use sha2::{Digest, Sha256};

use crate::error::{OpsError, OpsErrorKind, OpsResult};
use crate::reports::{WitnessCompressionReport, WitnessIntegrityReport, WitnessSegmentMeta};
use crate::storage::{
    cf, decode_cf_json, encode_cf_json, open_exclusive_lock, operation_lock_path, scan_cf,
    HygieneEnv,
};

pub const WITNESS_TYPE_COMPRESSED_SEGMENT: u8 = 0x80;
type LiveWitnessRow = (Vec<u8>, Vec<u8>, WitnessEntry);
type LiveWitnessRows = Vec<LiveWitnessRow>;

pub fn witness_compress_old_segments(env: &HygieneEnv) -> OpsResult<WitnessCompressionReport> {
    let _lock = open_exclusive_lock(&operation_lock_path(
        &env.config.archive_root,
        "witness-compress",
    ))?;
    fs::create_dir_all(segment_dir(&env.config.archive_root)).map_err(|err| {
        OpsError::io("create_dir_all", segment_dir(&env.config.archive_root), err)
    })?;
    let before = live_rows(env)?;
    let mut metas = Vec::new();
    loop {
        let rows = live_rows(env)?;
        let Some(window) = first_eligible_window(env, &rows)? else {
            break;
        };
        let meta = compress_window(env, &window)?;
        metas.push(meta);
    }
    let after = live_rows(env)?;
    let archive_bytes = metas.iter().map(|m| m.archive_len_bytes).sum();
    let report = WitnessCompressionReport {
        segments_compressed: metas.len() as u64,
        entries_archived: metas.iter().map(|m| m.entry_count).sum(),
        before_live_entries: before.len() as u64,
        after_live_entries: after.len() as u64,
        archive_bytes,
        segment_metas: metas,
    };
    verify_witness_integrity(env)?;
    Ok(report)
}

pub fn verify_witness_integrity(env: &HygieneEnv) -> OpsResult<WitnessIntegrityReport> {
    let rows = live_rows(env)?;
    let mut expected_prev = ZERO_HASH;
    let mut compressed_entries = 0u64;
    let mut archive_verified_segments = 0u64;
    for (idx, (key, value, entry)) in rows.iter().enumerate() {
        if entry.prev_hash != expected_prev {
            return Err(OpsError::new(OpsErrorKind::WitnessChainBroken {
                offset: idx as u64,
                detail: format!(
                    "prev_hash mismatch expected={} actual={}",
                    hex::encode(expected_prev),
                    hex::encode(entry.prev_hash)
                ),
            }));
        }
        if entry.witness_type == WITNESS_TYPE_COMPRESSED_SEGMENT {
            compressed_entries += 1;
            verify_segment_archive_by_key(env, key, entry)?;
            archive_verified_segments += 1;
        } else if value.len() != WITNESS_ENTRY_SIZE {
            return Err(OpsError::new(OpsErrorKind::WitnessChainBroken {
                offset: idx as u64,
                detail: format!("entry length {} != {WITNESS_ENTRY_SIZE}", value.len()),
            }));
        }
        expected_prev = entry.chain_hash();
    }
    Ok(WitnessIntegrityReport {
        live_entries: rows.len() as u64,
        compressed_entries,
        last_chain_hash: expected_prev,
        archive_verified_segments,
    })
}

pub fn audit_verify_old_entry(
    env: &HygieneEnv,
    segment_start: u64,
    offset: usize,
) -> OpsResult<bool> {
    let meta = load_segment_meta(env, &segment_start.to_be_bytes())?;
    if offset >= meta.entry_count as usize {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: format!(
                "offset {offset} outside segment length {}",
                meta.entry_count
            ),
        }));
    }
    let archive = read_archive(&meta)?;
    let entries = archive_to_entries(&archive)?;
    let leaves = merkle_leaves(meta.segment_start, &entries);
    let tree = MerkleTree::build(leaves.clone()).map_err(|err| {
        OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: format!("Merkle tree build failed: {}: {err}", err.code()),
        })
    })?;
    if tree.root() != meta.merkle_root {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: "archive Merkle root does not match metadata".to_string(),
        }));
    }
    let proof = tree.proof_for(offset).map_err(|err| {
        OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: format!("Merkle proof failed: {}: {err}", err.code()),
        })
    })?;
    MerkleTree::verify_proof(&leaves[offset], &proof, &meta.merkle_root).map_err(|err| {
        OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path,
            detail: format!("Merkle proof rejected: {}: {err}", err.code()),
        })
    })?;
    Ok(true)
}

fn compress_window(env: &HygieneEnv, window: &[LiveWitnessRow]) -> OpsResult<WitnessSegmentMeta> {
    let first = window.first().ok_or_else(|| {
        OpsError::invalid(
            "witness_compress.window",
            "cannot compress an empty witness window",
        )
    })?;
    let last = window.last().ok_or_else(|| {
        OpsError::invalid(
            "witness_compress.window",
            "cannot compress an empty witness window",
        )
    })?;
    let segment_start = decode_u64_key(&first.0)?;
    let originals = window
        .iter()
        .map(|(_, bytes, _)| bytes.clone())
        .collect::<Vec<_>>();
    let archive_bytes = originals.concat();
    let leaves = merkle_leaves(segment_start, &originals);
    let tree = MerkleTree::build(leaves).map_err(|err| {
        OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: env.config.archive_root.clone(),
            detail: format!("Merkle tree build failed: {}: {err}", err.code()),
        })
    })?;
    let merkle_root = tree.root();
    let archive_path = segment_path(&env.config.archive_root, segment_start, window.len() as u64);
    write_synced_archive(&archive_path, &archive_bytes)?;
    let archive_sha256: [u8; 32] = Sha256::digest(&archive_bytes).into();
    let compressed = WitnessEntry::new(
        first.2.prev_hash,
        merkle_root,
        last.2.timestamp_ns,
        WITNESS_TYPE_COMPRESSED_SEGMENT,
    );
    let compressed_bytes = compressed.to_bytes();
    let rows = live_rows(env)?;
    let last_index = rows
        .iter()
        .position(|(key, _, _)| key == &last.0)
        .ok_or_else(|| {
            OpsError::invalid(
                "witness_compress.window",
                "last window row disappeared before DB write",
            )
        })?;
    let mut batch = WriteBatch::default();
    let chain_cf = cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_WITNESS_CHAIN,
    )?;
    batch.put_cf(chain_cf, &first.0, compressed_bytes);
    for (key, _, _) in window.iter().skip(1) {
        batch.delete_cf(chain_cf, key);
    }
    rechain_after_window(env, &mut batch, &rows, last_index, compressed.chain_hash())?;
    let meta = WitnessSegmentMeta {
        segment_start,
        entry_count: window.len() as u64,
        archive_path: archive_path.clone(),
        archive_sha256,
        merkle_root,
        compressed_entry_hash: compressed.chain_hash(),
        first_key: first.0.clone(),
        last_key: last.0.clone(),
        compressed_at_unix: env.now_unix(),
        archive_len_bytes: archive_bytes.len() as u64,
    };
    batch.put_cf(
        cf(
            &env.config.db,
            context_graph_mejepa_cf::CF_MEJEPA_WITNESS_SEGMENT_META,
        )?,
        segment_start.to_be_bytes(),
        encode_cf_json(&meta)?,
    );
    env.config.db.write(batch)?;
    let readback =
        env.config.db.get_cf(chain_cf, &first.0)?.ok_or_else(|| {
            OpsError::invalid("witness_compress.readback", "missing compressed row")
        })?;
    if readback.as_slice() != compressed_bytes.as_slice() {
        return Err(OpsError::invalid(
            "witness_compress.readback",
            "compressed row readback mismatch",
        ));
    }
    let meta_readback = load_segment_meta(env, &first.0)?;
    if meta_readback != meta {
        return Err(OpsError::invalid(
            "witness_compress.meta_readback",
            "segment metadata readback mismatch",
        ));
    }
    for (key, _, _) in window.iter().skip(1) {
        if env.config.db.get_cf(chain_cf, key)?.is_some() {
            return Err(OpsError::invalid(
                "witness_compress.delete_readback",
                format!("archived witness row {} still exists", hex::encode(key)),
            ));
        }
    }
    Ok(meta)
}

fn first_eligible_window(
    env: &HygieneEnv,
    rows: &[LiveWitnessRow],
) -> OpsResult<Option<LiveWitnessRows>> {
    let now = env.now_unix();
    if now < 0 {
        return Err(OpsError::invalid(
            "now_unix",
            format!("witness compression requires non-negative unix time, got {now}"),
        ));
    }
    let age_secs = i64::from(env.config.witness_min_age_days)
        .checked_mul(86_400)
        .ok_or_else(|| OpsError::invalid("witness_min_age_days", "age cutoff overflowed i64"))?;
    let min_ts = now.checked_sub(age_secs).ok_or_else(|| {
        OpsError::invalid(
            "witness_min_age_days",
            "age cutoff is before the Unix epoch",
        )
    })? as u64;
    let mut candidate = Vec::new();
    for row in rows {
        if row.2.witness_type == WITNESS_TYPE_COMPRESSED_SEGMENT {
            candidate.clear();
            continue;
        }
        if row.2.timestamp_ns / 1_000_000_000 > min_ts {
            candidate.clear();
            continue;
        }
        candidate.push(row.clone());
        if candidate.len() == env.config.witness_segment_size {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn rechain_after_window(
    env: &HygieneEnv,
    batch: &mut WriteBatch,
    rows: &[LiveWitnessRow],
    last_index: usize,
    mut expected_prev: [u8; 32],
) -> OpsResult<()> {
    let chain_cf = cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_WITNESS_CHAIN,
    )?;
    let meta_cf = cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_WITNESS_SEGMENT_META,
    )?;
    for (key, _bytes, entry) in rows.iter().skip(last_index + 1) {
        let mut rewritten = entry.clone();
        if rewritten.prev_hash != expected_prev {
            rewritten.prev_hash = expected_prev;
            batch.put_cf(chain_cf, key, rewritten.to_bytes());
            if rewritten.witness_type == WITNESS_TYPE_COMPRESSED_SEGMENT {
                let mut meta = load_segment_meta(env, key)?;
                meta.compressed_entry_hash = rewritten.chain_hash();
                validate_segment_meta(env, key, &meta)?;
                batch.put_cf(meta_cf, key, encode_cf_json(&meta)?);
            }
        }
        expected_prev = rewritten.chain_hash();
    }
    Ok(())
}

fn live_rows(env: &HygieneEnv) -> OpsResult<LiveWitnessRows> {
    let mut rows = Vec::new();
    for (key, value) in scan_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_WITNESS_CHAIN,
    )? {
        if key.len() != 8 {
            return Err(OpsError::new(OpsErrorKind::WitnessChainBroken {
                offset: rows.len() as u64,
                detail: format!(
                    "witness key must be 8-byte big-endian offset, got {}",
                    key.len()
                ),
            }));
        }
        let entry = WitnessEntry::from_bytes(&value).map_err(|err| {
            OpsError::new(OpsErrorKind::WitnessChainBroken {
                offset: rows.len() as u64,
                detail: format!("{}: {err}", err.code()),
            })
        })?;
        rows.push((key, value, entry));
    }
    Ok(rows)
}

fn verify_segment_archive_by_key(
    env: &HygieneEnv,
    key: &[u8],
    entry: &WitnessEntry,
) -> OpsResult<()> {
    let meta = load_segment_meta(env, key)?;
    if meta.merkle_root != entry.action_hash {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path,
            detail: "compressed entry root does not match segment metadata".to_string(),
        }));
    }
    if meta.compressed_entry_hash != entry.chain_hash() {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path,
            detail: "compressed entry chain hash does not match segment metadata".to_string(),
        }));
    }
    let archive = read_archive(&meta)?;
    let entries = archive_to_entries(&archive)?;
    let tree = MerkleTree::build(merkle_leaves(meta.segment_start, &entries)).map_err(|err| {
        OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: format!("Merkle tree build failed: {}: {err}", err.code()),
        })
    })?;
    if tree.root() != meta.merkle_root {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path,
            detail: "archive recomputed Merkle root does not match".to_string(),
        }));
    }
    Ok(())
}

fn load_segment_meta(env: &HygieneEnv, key: &[u8]) -> OpsResult<WitnessSegmentMeta> {
    let bytes = env
        .config
        .db
        .get_cf(
            cf(
                &env.config.db,
                context_graph_mejepa_cf::CF_MEJEPA_WITNESS_SEGMENT_META,
            )?,
            key,
        )?
        .ok_or_else(|| {
            OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
                path: env.config.archive_root.clone(),
                detail: format!("missing segment metadata for key {}", hex::encode(key)),
            })
        })?;
    let meta: WitnessSegmentMeta = decode_cf_json(&bytes)?;
    validate_segment_meta(env, key, &meta)?;
    Ok(meta)
}

fn validate_segment_meta(env: &HygieneEnv, key: &[u8], meta: &WitnessSegmentMeta) -> OpsResult<()> {
    let segment_start = decode_u64_key(key)?;
    if meta.segment_start != segment_start {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: format!(
                "metadata segment_start {} does not match key {}",
                meta.segment_start, segment_start
            ),
        }));
    }
    if meta.entry_count == 0 {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: "segment metadata entry_count must be > 0".to_string(),
        }));
    }
    let expected_len = meta
        .entry_count
        .checked_mul(WITNESS_ENTRY_SIZE as u64)
        .ok_or_else(|| {
            OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
                path: meta.archive_path.clone(),
                detail: "segment archive length overflowed u64".to_string(),
            })
        })?;
    if meta.archive_len_bytes != expected_len {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: format!(
                "archive_len_bytes {} != entry_count * witness_entry_size {expected_len}",
                meta.archive_len_bytes
            ),
        }));
    }
    if meta.first_key != meta.segment_start.to_be_bytes().to_vec() {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: "first_key does not match segment_start".to_string(),
        }));
    }
    let last_key = meta
        .segment_start
        .checked_add(meta.entry_count - 1)
        .ok_or_else(|| {
            OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
                path: meta.archive_path.clone(),
                detail: "last key overflowed u64".to_string(),
            })
        })?
        .to_be_bytes()
        .to_vec();
    if meta.last_key != last_key {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: "last_key does not match segment_start + entry_count - 1".to_string(),
        }));
    }
    let expected_path = segment_path(
        &env.config.archive_root,
        meta.segment_start,
        meta.entry_count,
    );
    if meta.archive_path != expected_path {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: format!(
                "archive path must be {}; got {}",
                expected_path.display(),
                meta.archive_path.display()
            ),
        }));
    }
    Ok(())
}

fn read_archive(meta: &WitnessSegmentMeta) -> OpsResult<Vec<u8>> {
    let archive_meta = fs::symlink_metadata(&meta.archive_path)
        .map_err(|err| OpsError::io("symlink_metadata", meta.archive_path.clone(), err))?;
    if archive_meta.file_type().is_symlink() {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: "archive path is a symlink".to_string(),
        }));
    }
    let bytes = fs::read(&meta.archive_path)
        .map_err(|err| OpsError::io("read", meta.archive_path.clone(), err))?;
    if bytes.len() as u64 != meta.archive_len_bytes {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: format!(
                "archive length {} != metadata {}",
                bytes.len(),
                meta.archive_len_bytes
            ),
        }));
    }
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    if digest != meta.archive_sha256 {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: meta.archive_path.clone(),
            detail: "archive sha256 mismatch".to_string(),
        }));
    }
    Ok(bytes)
}

fn archive_to_entries(bytes: &[u8]) -> OpsResult<Vec<Vec<u8>>> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(WITNESS_ENTRY_SIZE) {
        return Err(OpsError::new(OpsErrorKind::WitnessArchiveInvalid {
            path: PathBuf::new(),
            detail: format!("archive length must be non-zero multiple of {WITNESS_ENTRY_SIZE}"),
        }));
    }
    Ok(bytes
        .chunks_exact(WITNESS_ENTRY_SIZE)
        .map(|chunk| chunk.to_vec())
        .collect())
}

fn merkle_leaves(segment_start: u64, entries: &[Vec<u8>]) -> Vec<MerkleLeaf> {
    entries
        .iter()
        .enumerate()
        .map(|(idx, bytes)| {
            MerkleLeaf::new(
                format!("{}", segment_start + idx as u64),
                shake256_32(bytes),
            )
        })
        .collect()
}

fn write_synced_archive(path: &Path, bytes: &[u8]) -> OpsResult<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|err| OpsError::io("create_dir_all", parent, err))?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|err| OpsError::io("open", &tmp, err))?;
    file.write_all(bytes)
        .map_err(|err| OpsError::io("write", &tmp, err))?;
    file.sync_all()
        .map_err(|err| OpsError::io("sync_all", &tmp, err))?;
    fs::rename(&tmp, path).map_err(|err| OpsError::io("rename", path, err))?;
    sync_dir(parent)?;
    Ok(())
}

fn sync_dir(path: &Path) -> OpsResult<()> {
    let dir = File::open(path).map_err(|err| OpsError::io("open_dir", path, err))?;
    dir.sync_all()
        .map_err(|err| OpsError::io("sync_dir", path, err))
}

fn segment_dir(root: &Path) -> PathBuf {
    root.join("witness_segments")
}

fn segment_path(root: &Path, start: u64, count: u64) -> PathBuf {
    segment_dir(root).join(format!("segment-{start:020}-{count}.bin"))
}

fn decode_u64_key(key: &[u8]) -> OpsResult<u64> {
    if key.len() != 8 {
        return Err(OpsError::new(OpsErrorKind::WitnessChainBroken {
            offset: 0,
            detail: format!("witness key length must be 8, got {}", key.len()),
        }));
    }
    let mut raw = [0u8; 8];
    raw.copy_from_slice(key);
    Ok(u64::from_be_bytes(raw))
}
