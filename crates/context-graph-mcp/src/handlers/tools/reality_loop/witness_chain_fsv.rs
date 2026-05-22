use super::witness_chain::{
    WitnessOpType, append_witness_entry_for_run, verify_witness_chain_for_run,
};
use context_graph_witness::WITNESS_ENTRY_SIZE;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn seed_runtime() -> (TempDir, PathBuf, String) {
    let tmp = TempDir::new().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let run_id = "run-witness-basic".to_string();
    fs::create_dir_all(runtime_root.join(&run_id).join("claude-code-optimizer"))
        .expect("witness dir");
    (tmp, runtime_root, run_id)
}

fn witness_path(runtime_root: &Path, run_id: &str) -> PathBuf {
    runtime_root
        .join(run_id)
        .join("claude-code-optimizer")
        .join("witness-chain.bin")
}

fn content_hash(byte: u8) -> String {
    format!("sha256:{}", hex::encode([byte; 32]))
}

#[test]
fn missing_chain_fails_closed() {
    let (_tmp, runtime_root, run_id) = seed_runtime();
    let err = verify_witness_chain_for_run(&runtime_root, &run_id).expect_err("must fail");
    assert_eq!(err.error_code, "CCREALITY_WITNESS_CHAIN_ABSENT");
}

#[test]
fn missing_format_manifest_fails_closed() {
    let (_tmp, runtime_root, run_id) = seed_runtime();
    append_witness_entry_for_run(
        &runtime_root,
        &run_id,
        WitnessOpType::Decision,
        &content_hash(1),
    )
    .expect("append");
    let path = witness_path(&runtime_root, &run_id);
    fs::remove_file(path.with_file_name("witness-chain.format.json")).expect("remove manifest");
    let err = verify_witness_chain_for_run(&runtime_root, &run_id).expect_err("must fail");
    assert_eq!(err.error_code, "CCREALITY_WITNESS_FORMAT_MANIFEST_ABSENT");
}

#[test]
fn append_and_verify_chain_reads_physical_bytes() {
    let (_tmp, runtime_root, run_id) = seed_runtime();
    append_witness_entry_for_run(
        &runtime_root,
        &run_id,
        WitnessOpType::Decision,
        &content_hash(1),
    )
    .expect("append1");
    append_witness_entry_for_run(
        &runtime_root,
        &run_id,
        WitnessOpType::Recommendation,
        &content_hash(2),
    )
    .expect("append2");
    let path = witness_path(&runtime_root, &run_id);
    let bytes = fs::read(&path).expect("physical chain");
    assert_eq!(bytes.len(), WITNESS_ENTRY_SIZE * 2);
    let verification = verify_witness_chain_for_run(&runtime_root, &run_id).expect("verify");
    assert_eq!(verification["entries"], 2);
    assert_eq!(verification["valid"], true);
    assert_eq!(
        verification["format_manifest"]["status"]
            .as_str()
            .expect("format status"),
        "valid"
    );
}

#[test]
fn truncated_chain_errors() {
    let (_tmp, runtime_root, run_id) = seed_runtime();
    let path = witness_path(&runtime_root, &run_id);
    fs::write(&path, [1u8; 7]).expect("write corrupt");
    let err = verify_witness_chain_for_run(&runtime_root, &run_id).expect_err("must fail");
    assert_eq!(err.error_code, "CCREALITY_WITNESS_LENGTH_INVALID");
}

#[test]
fn tampered_prev_hash_errors() {
    let (_tmp, runtime_root, run_id) = seed_runtime();
    append_witness_entry_for_run(
        &runtime_root,
        &run_id,
        WitnessOpType::Decision,
        &content_hash(1),
    )
    .expect("append1");
    append_witness_entry_for_run(
        &runtime_root,
        &run_id,
        WitnessOpType::Recommendation,
        &content_hash(2),
    )
    .expect("append2");
    let path = witness_path(&runtime_root, &run_id);
    let mut bytes = fs::read(&path).expect("read");
    bytes[WITNESS_ENTRY_SIZE] ^= 0x7f;
    fs::write(&path, bytes).expect("tamper");
    let err = verify_witness_chain_for_run(&runtime_root, &run_id).expect_err("must fail");
    assert_eq!(err.error_code, "CCREALITY_WITNESS_PREV_HASH_MISMATCH");
}
