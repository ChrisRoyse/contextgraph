use super::helpers::{file_arg_to_path, read_json, sha256_file};
use super::witness_chain::{
    append_witness_entry_for_run, verify_witness_chain_for_run, WitnessOpType,
};
use super::witness_chain_io::read_chain_bytes;
use super::witness_chain_repair::repair_legacy_chain_for_run;
use context_graph_witness::{shake256_32, WITNESS_ENTRY_SIZE, ZERO_HASH};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn seed_runtime(run_id: &str) -> (TempDir, PathBuf, String) {
    let tmp = TempDir::new().expect("tempdir");
    let runtime_root = tmp.path().join("runtime-root");
    fs::create_dir_all(runtime_root.join(run_id).join("claude-code-optimizer"))
        .expect("witness dir");
    (tmp, runtime_root, run_id.to_string())
}

fn write_legacy_chain(path: &Path, entries: &[(u64, u8, u8)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut prev = ZERO_HASH;
    for (timestamp_unix, op_type, action_byte) in entries {
        let mut entry = [0u8; WITNESS_ENTRY_SIZE];
        entry[0..8].copy_from_slice(&timestamp_unix.to_be_bytes());
        entry[8] = *op_type;
        entry[9..41].copy_from_slice(&[*action_byte; 32]);
        entry[41..73].copy_from_slice(&prev);
        prev = shake256_32(&entry);
        bytes.extend_from_slice(&entry);
    }
    fs::write(path, &bytes).expect("write legacy chain");
    bytes
}

fn witness_path(runtime_root: &Path, run_id: &str) -> PathBuf {
    runtime_root
        .join(run_id)
        .join("claude-code-optimizer")
        .join("witness-chain.bin")
}

fn witness_repair_audit_count(runtime_root: &Path, run_id: &str) -> usize {
    let dir = runtime_root
        .join(run_id)
        .join("reality-optimizer")
        .join("witness-repair");
    if !dir.is_dir() {
        return 0;
    }
    fs::read_dir(dir)
        .expect("read repair dir")
        .filter_map(std::result::Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name.starts_with("repair-") && name.ends_with("-claim.json"))
                .unwrap_or(false)
        })
        .count()
}

#[test]
fn legacy_layout_is_reported_explicitly() {
    let (_tmp, runtime_root, run_id) = seed_runtime("run-legacy-detect");
    let path = witness_path(&runtime_root, &run_id);
    write_legacy_chain(&path, &[(1_778_200_000, 1, 0x11), (1_778_200_001, 3, 0x22)]);
    let err = verify_witness_chain_for_run(&runtime_root, &run_id).expect_err("legacy must fail");
    println!(
        "WITNESS_LEGACY_DETECT error_code={} source_of_truth={:?}",
        err.error_code, err.source_of_truth
    );
    assert_eq!(err.error_code, "CCREALITY_WITNESS_LEGACY_LAYOUT_DETECTED");
}

#[test]
fn repair_legacy_chain_preserves_bytes_and_writes_canonical_chain() {
    let (_tmp, runtime_root, run_id) = seed_runtime("run-legacy-repair");
    let path = witness_path(&runtime_root, &run_id);
    let before_bytes =
        write_legacy_chain(&path, &[(1_778_200_000, 1, 0x11), (1_778_200_001, 3, 0x22)]);
    let before_sha = sha256_file(&path).expect("before sha");
    let before_repairs = witness_repair_audit_count(&runtime_root, &run_id);
    let result =
        repair_legacy_chain_for_run(&runtime_root, &run_id, &before_sha).expect("repair legacy");
    let after_repairs = witness_repair_audit_count(&runtime_root, &run_id);
    println!(
        "WITNESS_REPAIR_STATE before_repairs={before_repairs} after_repairs={after_repairs} result={}",
        serde_json::to_string(&result).unwrap()
    );
    assert_eq!(before_repairs, 0);
    assert_eq!(after_repairs, 1);

    let backup_path = file_arg_to_path(
        result["source_of_truth"]["legacy_backup"]
            .as_str()
            .expect("backup"),
    );
    assert_eq!(fs::read(&backup_path).expect("backup read"), before_bytes);
    let claim_path = file_arg_to_path(
        result["source_of_truth"]["repair_claim"]
            .as_str()
            .expect("claim"),
    );
    let claim = read_json(&claim_path).expect("claim read");
    assert_eq!(claim["legacy_chain"]["sha256"], json!(before_sha));

    let verify = verify_witness_chain_for_run(&runtime_root, &run_id).expect("verify after");
    assert_eq!(verify["valid"], json!(true));
    assert_eq!(verify["entries"], json!(3));
    assert_eq!(
        verify["last_op"]["op_type"],
        json!(WitnessOpType::WitnessRepair.as_str())
    );
    assert_eq!(
        read_chain_bytes(&path).unwrap().len(),
        WITNESS_ENTRY_SIZE * 3
    );
}

#[test]
fn repair_edge_cases_fail_closed_without_mutation() {
    let (_tmp_empty, runtime_empty, run_empty) = seed_runtime("run-empty");
    let empty_path = witness_path(&runtime_empty, &run_empty);
    fs::write(&empty_path, []).expect("empty chain");
    let before = fs::read(&empty_path).expect("read empty before");
    let err = repair_legacy_chain_for_run(
        &runtime_empty,
        &run_empty,
        &sha256_file(&empty_path).unwrap(),
    )
    .expect_err("empty repair must fail");
    let after = fs::read(&empty_path).expect("read empty after");
    println!(
        "WITNESS_EDGE_EMPTY before_bytes={} after_bytes={} error_code={}",
        before.len(),
        after.len(),
        err.error_code
    );
    assert_eq!(err.error_code, "CCREALITY_WITNESS_REPAIR_EMPTY_CHAIN");
    assert_eq!(before, after);

    let (_tmp_corrupt, runtime_corrupt, run_corrupt) = seed_runtime("run-corrupt");
    let corrupt_path = witness_path(&runtime_corrupt, &run_corrupt);
    let mut corrupt = write_legacy_chain(
        &corrupt_path,
        &[(1_778_200_000, 1, 0x11), (1_778_200_001, 3, 0x22)],
    );
    corrupt[WITNESS_ENTRY_SIZE + 41] ^= 0x7f;
    fs::write(&corrupt_path, &corrupt).expect("write corrupt");
    let before = fs::read(&corrupt_path).expect("corrupt before");
    let err = repair_legacy_chain_for_run(
        &runtime_corrupt,
        &run_corrupt,
        &sha256_file(&corrupt_path).unwrap(),
    )
    .expect_err("corrupt repair must fail");
    let after = fs::read(&corrupt_path).expect("corrupt after");
    println!(
        "WITNESS_EDGE_CORRUPT before_repairs={} after_repairs={} error_code={}",
        witness_repair_audit_count(&runtime_corrupt, &run_corrupt),
        witness_repair_audit_count(&runtime_corrupt, &run_corrupt),
        err.error_code
    );
    assert_eq!(
        err.error_code,
        "CCREALITY_WITNESS_REPAIR_LEGACY_REPLAY_FAILED"
    );
    assert_eq!(before, after);

    let (_tmp_canonical, runtime_canonical, run_canonical) = seed_runtime("run-canonical");
    append_witness_entry_for_run(
        &runtime_canonical,
        &run_canonical,
        WitnessOpType::Decision,
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
    )
    .expect("append canonical");
    let canonical_path = witness_path(&runtime_canonical, &run_canonical);
    let before = fs::read(&canonical_path).expect("canonical before");
    let err = repair_legacy_chain_for_run(
        &runtime_canonical,
        &run_canonical,
        &sha256_file(&canonical_path).unwrap(),
    )
    .expect_err("canonical repair must fail");
    let after = fs::read(&canonical_path).expect("canonical after");
    println!(
        "WITNESS_EDGE_CANONICAL before_bytes={} after_bytes={} error_code={}",
        before.len(),
        after.len(),
        err.error_code
    );
    assert_eq!(err.error_code, "CCREALITY_WITNESS_REPAIR_NOT_NEEDED");
    assert_eq!(before, after);
}
