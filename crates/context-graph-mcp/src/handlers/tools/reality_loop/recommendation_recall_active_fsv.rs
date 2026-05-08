use super::helpers::{file_arg_to_path, file_sot, latest_run_id, read_json, sha256_file};
use super::recommendation_recall_fsv::audit_count;
use super::recommendations::recall_recommendations_for_run;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

#[test]
#[ignore = "appends a recall audit to the active ccreality runtime root; run manually"]
fn recall_manual_active_runtime_reads_real_recommendations() {
    let active_path = dirs::home_dir()
        .expect("home")
        .join(".cache/cgreality/active_runtime_root");
    let raw = fs::read_to_string(&active_path).expect("active runtime root");
    let runtime_root = PathBuf::from(raw.trim());
    let run_id = latest_run_id(&runtime_root)
        .expect("latest run")
        .expect("active run id");
    let before = audit_count(&runtime_root, &run_id);
    let outcome = recall_recommendations_for_run(
        &runtime_root,
        &run_id,
        &json!({
            "failure_summary": "MCP error -32000 connection closed fire and forget engine spawn timeout structured error logs",
            "k": 2,
            "lambda": 0.70
        }),
    );
    let after = audit_count(&runtime_root, &run_id);
    let result = match outcome {
        Ok(result) => result,
        Err(err) if err.error_code == "CCREALITY_WITNESS_PREV_HASH_MISMATCH" => {
            println!(
                "RECALL_REAL_RUNTIME_FAIL_CLOSED run_id={run_id} before_audits={before} after_audits={after} error_code={} source_of_truth={:?}",
                err.error_code, err.source_of_truth
            );
            assert_eq!(after, before);
            return;
        }
        Err(err) => panic!("unexpected real active recall error: {err:?}"),
    };
    let audit_path = file_arg_to_path(
        result["source_of_truth"]["recommendation_recall_audit"]
            .as_str()
            .expect("audit path"),
    );
    let audit = read_json(&audit_path).expect("audit readback");
    println!(
        "RECALL_REAL_RUNTIME_STATE run_id={run_id} before_audits={before} after_audits={after} audit={} selected_count={} candidate_count={}",
        file_sot(&audit_path),
        audit["selected_count"],
        audit["candidate_count"]
    );
    assert_eq!(after, before + 1);
    assert_eq!(
        result["recall_audit_sha256"],
        json!(sha256_file(&audit_path).unwrap())
    );
    assert!(audit["selected_recommendations"].as_array().unwrap().len() >= 1);
}
