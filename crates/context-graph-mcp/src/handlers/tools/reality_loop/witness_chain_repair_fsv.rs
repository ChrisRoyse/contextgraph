use super::helpers::{
    file_arg_to_path, file_sot, latest_run_id, read_json, sha256_file, write_json_checked,
};
use super::witness_chain::{
    append_witness_entry_for_run, verify_witness_chain_for_run, WitnessOpType,
};
use super::witness_chain_io::read_chain_bytes;
use super::witness_chain_repair::repair_legacy_chain_for_run;
use context_graph_witness::{shake256_32, WITNESS_ENTRY_SIZE, ZERO_HASH};
use serde_json::{json, Value};
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

fn active_runtime_for_fsv(test_name: &str) -> (PathBuf, String) {
    let active_path =
        context_graph_paths::cgreality_cache_file("active_runtime_root").expect("active path");
    let raw = fs::read_to_string(&active_path).unwrap_or_else(|err| {
        panic!(
            "{test_name}: failed to read active runtime root at {}: {err}",
            active_path.display()
        )
    });
    let runtime_root = PathBuf::from(raw.trim());
    let run_id = latest_run_id(&runtime_root).unwrap_or_else(|err| {
        panic!(
            "{test_name}: failed to scan active runtime root {} from {}: {err}",
            runtime_root.display(),
            active_path.display()
        )
    });
    let Some(run_id) = run_id else {
        panic!(
            "{test_name}: active runtime root {} from {} contains no run directories; entries={}",
            runtime_root.display(),
            active_path.display(),
            runtime_entries_for_debug(&runtime_root)
        );
    };
    (runtime_root, run_id)
}

fn runtime_entries_for_debug(runtime_root: &Path) -> String {
    let entries = match fs::read_dir(runtime_root) {
        Ok(entries) => entries,
        Err(err) => return format!("<read_dir failed: {err}>"),
    };
    let mut names = entries
        .filter_map(std::result::Result::ok)
        .take(32)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    names.sort();
    format!("{names:?}")
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

#[test]
#[ignore = "writes persistent manual FSV evidence under target/; run explicitly"]
fn witness_repair_manual_fsv_persists_target_evidence() {
    let target = PathBuf::from("target/context-graph-witness-repair-fsv");
    let _ = fs::remove_dir_all(&target);
    fs::create_dir_all(&target).expect("target dir");

    let runtime_root = target.join("runtime-root");
    let run_id = "run-witness-repair-fsv";
    fs::create_dir_all(runtime_root.join(run_id).join("claude-code-optimizer"))
        .expect("witness dir");
    let path = witness_path(&runtime_root, run_id);
    write_legacy_chain(&path, &[(1_778_200_000, 1, 0x11), (1_778_200_001, 3, 0x22)]);

    let before_state = json!({
        "witness_chain": file_sot(&path),
        "sha256": sha256_file(&path).unwrap(),
        "bytes": fs::metadata(&path).unwrap().len(),
        "repair_audits": witness_repair_audit_count(&runtime_root, run_id),
        "verify_error": verify_witness_chain_for_run(&runtime_root, run_id).unwrap_err().into_value(),
    });
    let repair = repair_legacy_chain_for_run(
        &runtime_root,
        run_id,
        before_state["sha256"].as_str().unwrap(),
    )
    .expect("manual repair");
    let after_verify = verify_witness_chain_for_run(&runtime_root, run_id).expect("after verify");
    let happy_after = json!({
        "sha256": sha256_file(&path).unwrap(),
        "bytes": fs::metadata(&path).unwrap().len(),
        "repair_audits": witness_repair_audit_count(&runtime_root, run_id),
        "verify": after_verify,
        "repair": repair,
    });
    println!("WITNESS_FSV_HAPPY_BEFORE {before_state}");
    println!("WITNESS_FSV_HAPPY_AFTER {happy_after}");

    let edge_report = run_manual_edge_cases(&target);
    let evidence = json!({
        "schema_version": 1,
        "record_kind": "ccreality_witness_repair_manual_fsv",
        "source_of_truth": {
            "runtime_root": file_sot(&runtime_root),
            "witness_chain": file_sot(&path),
            "manual_evidence": file_sot(&target.join("manual-evidence.json")),
        },
        "happy_path": {
            "before": before_state,
            "after": happy_after,
        },
        "edge_cases": edge_report,
    });
    let evidence_path = target.join("manual-evidence.json");
    write_json_checked(&evidence_path, &evidence).expect("write evidence");
    let readback: Value = read_json(&evidence_path).expect("read evidence");
    assert_eq!(readback, evidence);
    println!(
        "WITNESS_FSV_EVIDENCE path={} sha256={}",
        file_sot(&evidence_path),
        sha256_file(&evidence_path).unwrap()
    );
}

fn run_manual_edge_cases(target: &Path) -> Value {
    let mut cases = Vec::new();
    for case in ["empty", "corrupt_legacy_prev", "already_canonical"] {
        let runtime_root = target.join(format!("edge-{case}"));
        let run_id = format!("run-{case}");
        fs::create_dir_all(runtime_root.join(&run_id).join("claude-code-optimizer"))
            .expect("edge dir");
        let path = witness_path(&runtime_root, &run_id);
        match case {
            "empty" => fs::write(&path, []).expect("empty"),
            "corrupt_legacy_prev" => {
                let mut bytes = write_legacy_chain(
                    &path,
                    &[(1_778_200_000, 1, 0x11), (1_778_200_001, 3, 0x22)],
                );
                bytes[WITNESS_ENTRY_SIZE + 41] ^= 0x7f;
                fs::write(&path, bytes).expect("corrupt");
            }
            "already_canonical" => {
                append_witness_entry_for_run(
                    &runtime_root,
                    &run_id,
                    WitnessOpType::Decision,
                    "sha256:2222222222222222222222222222222222222222222222222222222222222222",
                )
                .expect("canonical append");
            }
            _ => unreachable!(),
        }
        let before = json!({
            "sha256": sha256_file(&path).unwrap(),
            "bytes": fs::metadata(&path).unwrap().len(),
            "repair_audits": witness_repair_audit_count(&runtime_root, &run_id),
        });
        let err =
            repair_legacy_chain_for_run(&runtime_root, &run_id, before["sha256"].as_str().unwrap())
                .expect_err("edge case must fail");
        let after = json!({
            "sha256": sha256_file(&path).unwrap(),
            "bytes": fs::metadata(&path).unwrap().len(),
            "repair_audits": witness_repair_audit_count(&runtime_root, &run_id),
            "error": err.into_value(),
        });
        println!("WITNESS_FSV_EDGE case={case} before={before} after={after}");
        assert_eq!(before["sha256"], after["sha256"]);
        cases.push(json!({"case": case, "before": before, "after": after}));
    }
    json!(cases)
}

#[test]
#[ignore = "repairs the active ccreality runtime; requires CG_EXPECTED_LEGACY_WITNESS_SHA256"]
fn witness_repair_active_runtime_legacy_chain() {
    let expected_sha =
        std::env::var("CG_EXPECTED_LEGACY_WITNESS_SHA256").expect("expected legacy sha env");
    let (runtime_root, run_id) =
        active_runtime_for_fsv("witness_repair_active_runtime_legacy_chain");
    let path = witness_path(&runtime_root, &run_id);
    let before_sha = sha256_file(&path).expect("before sha");
    assert_eq!(before_sha, expected_sha);
    let before = json!({
        "run_id": run_id,
        "witness_chain": file_sot(&path),
        "sha256": before_sha,
        "bytes": fs::metadata(&path).unwrap().len(),
        "repair_audits": witness_repair_audit_count(&runtime_root, &run_id),
        "verify_error": verify_witness_chain_for_run(&runtime_root, &run_id).unwrap_err().into_value(),
    });
    let repair =
        repair_legacy_chain_for_run(&runtime_root, &run_id, expected_sha.as_str()).expect("repair");
    let after_verify = verify_witness_chain_for_run(&runtime_root, &run_id).expect("after verify");
    let after = json!({
        "sha256": sha256_file(&path).unwrap(),
        "bytes": fs::metadata(&path).unwrap().len(),
        "repair_audits": witness_repair_audit_count(&runtime_root, &run_id),
        "verify": after_verify,
        "repair": repair,
    });
    let target = PathBuf::from("target/context-graph-witness-repair-active");
    fs::create_dir_all(&target).expect("active target");
    let evidence = json!({
        "schema_version": 1,
        "record_kind": "ccreality_active_witness_repair_fsv",
        "before": before,
        "after": after,
    });
    let evidence_path = target.join("manual-evidence.json");
    write_json_checked(&evidence_path, &evidence).expect("write active evidence");
    let readback: Value = read_json(&evidence_path).expect("read active evidence");
    assert_eq!(readback, evidence);
    println!("WITNESS_ACTIVE_REPAIR_BEFORE {}", evidence["before"]);
    println!("WITNESS_ACTIVE_REPAIR_AFTER {}", evidence["after"]);
    println!(
        "WITNESS_ACTIVE_REPAIR_EVIDENCE path={} sha256={}",
        file_sot(&evidence_path),
        sha256_file(&evidence_path).unwrap()
    );
}

#[test]
#[ignore = "reads active ccreality runtime witness chain and writes verification evidence"]
fn witness_active_runtime_chain_verifies() {
    let (runtime_root, run_id) = active_runtime_for_fsv("witness_active_runtime_chain_verifies");
    let path = witness_path(&runtime_root, &run_id);
    let verification = verify_witness_chain_for_run(&runtime_root, &run_id).expect("active verify");
    let evidence = json!({
        "schema_version": 1,
        "record_kind": "ccreality_active_witness_verify_fsv",
        "run_id": run_id,
        "source_of_truth": file_sot(&path),
        "sha256": sha256_file(&path).unwrap(),
        "bytes": fs::metadata(&path).unwrap().len(),
        "verification": verification,
    });
    let target = PathBuf::from("target/context-graph-witness-repair-active");
    fs::create_dir_all(&target).expect("active target");
    let evidence_path = target.join("verify-evidence.json");
    write_json_checked(&evidence_path, &evidence).expect("write active verify");
    let readback: Value = read_json(&evidence_path).expect("read active verify");
    assert_eq!(readback, evidence);
    println!("WITNESS_ACTIVE_VERIFY {evidence}");
    println!(
        "WITNESS_ACTIVE_VERIFY_EVIDENCE path={} sha256={}",
        file_sot(&evidence_path),
        sha256_file(&evidence_path).unwrap()
    );
}
