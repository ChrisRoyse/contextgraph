use super::helpers::sha256_file;
use super::optimizer::{optimizer_record_decision, optimizer_record_harness_transition};
use context_graph_witness::WITNESS_ENTRY_SIZE;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;
use tempfile::TempDir;

struct TestRuntime {
    _tmp: TempDir,
    _orig_active: Option<String>,
    _orig_target: Option<String>,
    _guard: MutexGuard<'static, ()>,
    runtime_root: PathBuf,
    run_id: String,
    task_id: String,
}

fn setup_runtime() -> TestRuntime {
    let guard = super::TEST_RUNTIME_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().expect("tmpdir");
    let runtime_root = tmp.path().join("runtime-root");
    let run_id = "run-optimizer".to_string();
    let task_id = "psf__requests-2317".to_string();
    fs::create_dir_all(runtime_root.join(&run_id).join(&task_id).join("attempt-1"))
        .expect("attempt dir");
    let active_path =
        context_graph_paths::cgreality_cache_file("active_runtime_root").expect("active path");
    let target_path =
        context_graph_paths::cgreality_cache_file("active_target_instance").expect("target path");
    let orig_active = fs::read_to_string(&active_path).ok();
    let orig_target = fs::read_to_string(&target_path).ok();
    fs::write(&active_path, runtime_root.to_string_lossy().as_bytes()).expect("active");
    fs::write(&target_path, task_id.as_bytes()).expect("target");
    TestRuntime {
        _tmp: tmp,
        _orig_active: orig_active,
        _orig_target: orig_target,
        _guard: guard,
        runtime_root,
        run_id,
        task_id,
    }
}

impl Drop for TestRuntime {
    fn drop(&mut self) {
        let active_path =
            context_graph_paths::cgreality_cache_file("active_runtime_root").expect("active path");
        let target_path = context_graph_paths::cgreality_cache_file("active_target_instance")
            .expect("target path");
        if let Some(orig) = &self._orig_active {
            fs::write(&active_path, orig).ok();
        } else {
            fs::remove_file(&active_path).ok();
        }
        if let Some(orig) = &self._orig_target {
            fs::write(&target_path, orig).ok();
        } else {
            fs::remove_file(&target_path).ok();
        }
    }
}

fn attempt_dir(rt: &TestRuntime) -> PathBuf {
    rt.runtime_root
        .join(&rt.run_id)
        .join(&rt.task_id)
        .join("attempt-1")
}

fn write_corrupt_witness_chain(rt: &TestRuntime) -> PathBuf {
    let path = rt
        .runtime_root
        .join(&rt.run_id)
        .join("claude-code-optimizer")
        .join("witness-chain.bin");
    fs::create_dir_all(path.parent().expect("witness parent")).expect("witness parent");
    let mut bytes = vec![0u8; WITNESS_ENTRY_SIZE];
    bytes[0] = 0x7f;
    fs::write(&path, bytes).expect("write corrupt witness");
    path
}

#[tokio::test]
async fn decision_corrupt_witness_preflight_fails_without_optimizer_artifacts() {
    let rt = setup_runtime();
    let witness_path = write_corrupt_witness_chain(&rt);
    let optimizer_dir = attempt_dir(&rt).join("claude-code-optimizer");
    let trigger_path = attempt_dir(&rt).join("trigger-decision.json");
    let before_optimizer_dir_exists = optimizer_dir.exists();
    let before_trigger_exists = trigger_path.exists();
    let before_witness_sha = sha256_file(&witness_path).expect("before witness sha");

    let err = optimizer_record_decision(json!({
        "run_id": rt.run_id.clone(),
        "attempt": 1,
        "claude_session_id": "optimizer-test-session",
        "policy": "continue",
        "should_run": true,
        "reasons": ["synthetic preflight test"]
    }))
    .await
    .expect_err("corrupt witness must fail before decision write");

    let after_optimizer_dir_exists = optimizer_dir.exists();
    let after_trigger_exists = trigger_path.exists();
    let after_witness_sha = sha256_file(&witness_path).expect("after witness sha");
    println!(
        "OPTIMIZER_EDGE_CORRUPT_DECISION before_optimizer_dir_exists={before_optimizer_dir_exists} after_optimizer_dir_exists={after_optimizer_dir_exists} before_trigger_exists={before_trigger_exists} after_trigger_exists={after_trigger_exists} before_witness_sha={before_witness_sha} after_witness_sha={after_witness_sha} error_code={}",
        err.error_code
    );
    assert_eq!(err.error_code, "CCREALITY_WITNESS_PREV_HASH_MISMATCH");
    assert!(!before_optimizer_dir_exists);
    assert!(!after_optimizer_dir_exists);
    assert!(!before_trigger_exists);
    assert!(!after_trigger_exists);
    assert_eq!(before_witness_sha, after_witness_sha);
}

#[tokio::test]
async fn harness_transition_corrupt_witness_preflight_fails_without_transition_file() {
    let rt = setup_runtime();
    let witness_path = write_corrupt_witness_chain(&rt);
    let transitions_dir = attempt_dir(&rt).join("harness-transitions");
    let before_transition_count = count_json_files(&transitions_dir);
    let before_witness_sha = sha256_file(&witness_path).expect("before witness sha");

    let err = optimizer_record_harness_transition(
        json!({
            "run_id": rt.run_id.clone(),
            "attempt": 1,
            "claude_session_id": "optimizer-test-session",
            "outcome": "recorded",
            "summary": "synthetic preflight test"
        }),
        false,
    )
    .await
    .expect_err("corrupt witness must fail before transition write");

    let after_transition_count = count_json_files(&transitions_dir);
    let after_witness_sha = sha256_file(&witness_path).expect("after witness sha");
    println!(
        "OPTIMIZER_EDGE_CORRUPT_TRANSITION before_transition_count={before_transition_count} after_transition_count={after_transition_count} before_witness_sha={before_witness_sha} after_witness_sha={after_witness_sha} error_code={}",
        err.error_code
    );
    assert_eq!(err.error_code, "CCREALITY_WITNESS_PREV_HASH_MISMATCH");
    assert_eq!(before_transition_count, 0);
    assert_eq!(after_transition_count, 0);
    assert_eq!(before_witness_sha, after_witness_sha);
}

fn count_json_files(dir: &Path) -> usize {
    if !dir.is_dir() {
        return 0;
    }
    fs::read_dir(dir)
        .expect("read dir")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
        .count()
}
