use super::helpers::{file_arg_to_path, file_sot, read_json, sha256_file, write_json_checked};
use super::recommendations::recall_recommendations_for_run;
use context_graph_witness::{verify_chain_bytes_with_type_validator, WITNESS_ENTRY_SIZE};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn seed_runtime() -> (TempDir, PathBuf, String) {
    let tmp = TempDir::new().expect("tempdir");
    let runtime_root = tmp.path().join("runtime-root");
    let run_id = "run-recall-001".to_string();
    fs::create_dir_all(runtime_root.join(&run_id).join("reality-optimizer"))
        .expect("optimizer dir");
    (tmp, runtime_root, run_id)
}

fn write_recommendation(runtime_root: &Path, run_id: &str, task: &str, turn: u64, body: Value) {
    let path = runtime_root
        .join(run_id)
        .join(task)
        .join("attempt-1")
        .join("claude-code-optimizer")
        .join(format!("recommendation-turn-{turn:02}.json"));
    write_json_checked(&path, &body).expect("write recommendation");
}

fn valid_recommendation(run_id: &str, attempt: u64, reason: &str, uplift: f64) -> Value {
    json!({
        "schema_version": 1,
        "record_kind": "ccreality_optimizer_recommendation",
        "run_id": run_id,
        "attempt": attempt,
        "turn_number": attempt,
        "status": "changed",
        "reason": reason,
        "diagnosis_summary": {
            "failure_class": "import-error",
            "root_cause_hypothesis": reason,
            "intervention_surface": "tool_surface"
        },
        "uplift_against_baseline": uplift,
        "source_of_truth_readbacks": []
    })
}

pub(super) fn audit_count(runtime_root: &Path, run_id: &str) -> usize {
    let dir = runtime_root
        .join(run_id)
        .join("reality-optimizer")
        .join("recommendation-recall");
    if !dir.is_dir() {
        return 0;
    }
    fs::read_dir(dir)
        .expect("read audit dir")
        .filter_map(std::result::Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name.starts_with("recall-") && name.ends_with(".json"))
                .unwrap_or(false)
        })
        .count()
}

#[test]
fn recall_happy_path_mmr_writes_audit_and_witness() {
    let (_tmp, runtime_root, run_id) = seed_runtime();
    write_recommendation(
        &runtime_root,
        &run_id,
        "task-a",
        1,
        valid_recommendation(&run_id, 1, "missing import module loader generic fix", 0.1),
    );
    write_recommendation(
        &runtime_root,
        &run_id,
        "task-b",
        2,
        valid_recommendation(
            &run_id,
            1,
            "missing import module loader generic fix duplicate",
            0.1,
        ),
    );
    write_recommendation(
        &runtime_root,
        &run_id,
        "task-c",
        3,
        valid_recommendation(
            &run_id,
            1,
            "timeout process watchdog runtime generic fix",
            0.0,
        ),
    );

    let before = audit_count(&runtime_root, &run_id);
    let result = recall_recommendations_for_run(
        &runtime_root,
        &run_id,
        &json!({
            "failure_summary": "missing import module timeout process",
            "k": 2,
            "alpha": 1.0,
            "beta": 0.0,
            "gamma": 0.0,
            "lambda": 0.30
        }),
    )
    .expect("recall");
    let after = audit_count(&runtime_root, &run_id);
    println!("RECALL_HAPPY_STATE before_audits={before} after_audits={after}");
    assert_eq!(before, 0);
    assert_eq!(after, 1);

    let audit_path = file_arg_to_path(
        result["source_of_truth"]["recommendation_recall_audit"]
            .as_str()
            .expect("audit path"),
    );
    let audit = read_json(&audit_path).expect("read audit");
    let selected_paths = audit["selected_recommendations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row["recommendation_relative_path"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    println!("RECALL_HAPPY_SELECTED {selected_paths:?}");
    assert!(selected_paths.iter().any(|path| path.contains("task-a/")));
    assert!(selected_paths.iter().any(|path| path.contains("task-c/")));
    assert!(!selected_paths.iter().any(|path| path.contains("task-b/")));
    assert_eq!(
        result["recall_audit_sha256"],
        json!(sha256_file(&audit_path).unwrap())
    );

    let witness_path = runtime_root
        .join(&run_id)
        .join("claude-code-optimizer")
        .join("witness-chain.bin");
    let witness_bytes = fs::read(&witness_path).expect("read witness");
    assert_eq!(witness_bytes.len(), WITNESS_ENTRY_SIZE);
    let verification =
        verify_chain_bytes_with_type_validator(&witness_bytes, |ty| ty == 7).expect("witness");
    println!(
        "RECALL_HAPPY_WITNESS entries={} chain_hash={}",
        verification.entries,
        hex::encode(verification.last_chain_hash)
    );
    assert_eq!(verification.entries, 1);
}

#[test]
fn recall_edge_cases_fail_closed_without_audit() {
    let (_tmp, runtime_root, run_id) = seed_runtime();
    let before = audit_count(&runtime_root, &run_id);
    let err = recall_recommendations_for_run(
        &runtime_root,
        &run_id,
        &json!({"failure_summary": "import", "k": 1}),
    )
    .expect_err("missing recommendations");
    let after = audit_count(&runtime_root, &run_id);
    println!(
        "RECALL_EDGE_MISSING before_audits={before} after_audits={after} error_code={}",
        err.error_code
    );
    assert_eq!(err.error_code, "CCREALITY_RECALL_RECOMMENDATIONS_MISSING");
    assert_eq!(after, 0);

    write_recommendation(
        &runtime_root,
        &run_id,
        "task-empty-text",
        1,
        json!({"schema_version": 1, "attempt": 1, "turn_number": 1}),
    );
    let before = audit_count(&runtime_root, &run_id);
    let err = recall_recommendations_for_run(
        &runtime_root,
        &run_id,
        &json!({"failure_summary": "import", "k": 1}),
    )
    .expect_err("empty text");
    let after = audit_count(&runtime_root, &run_id);
    println!(
        "RECALL_EDGE_EMPTY_TEXT before_audits={before} after_audits={after} error_code={}",
        err.error_code
    );
    assert_eq!(err.error_code, "CCREALITY_RECALL_RECOMMENDATION_TEXT_EMPTY");
    assert_eq!(after, 0);

    let (_tmp2, runtime_root2, run_id2) = seed_runtime();
    write_recommendation(
        &runtime_root2,
        &run_id2,
        "task-a",
        1,
        valid_recommendation(&run_id2, 1, "missing import module loader generic fix", 0.1),
    );
    let before = audit_count(&runtime_root2, &run_id2);
    let err = recall_recommendations_for_run(
        &runtime_root2,
        &run_id2,
        &json!({"failure_summary": "import", "k": 1, "lambda": 1.5}),
    )
    .expect_err("invalid lambda");
    let after = audit_count(&runtime_root2, &run_id2);
    println!(
        "RECALL_EDGE_INVALID_LAMBDA before_audits={before} after_audits={after} error_code={}",
        err.error_code
    );
    assert_eq!(err.error_code, "CCREALITY_RECALL_LAMBDA_OUT_OF_RANGE");
    assert_eq!(after, 0);

    let (_tmp3, runtime_root3, run_id3) = seed_runtime();
    write_recommendation(
        &runtime_root3,
        &run_id3,
        "task-a",
        1,
        valid_recommendation(&run_id3, 1, "missing import module loader generic fix", 0.1),
    );
    let witness_path = runtime_root3
        .join(&run_id3)
        .join("claude-code-optimizer")
        .join("witness-chain.bin");
    fs::create_dir_all(witness_path.parent().unwrap()).expect("witness parent");
    let mut corrupt = vec![0u8; WITNESS_ENTRY_SIZE];
    corrupt[0] = 1;
    fs::write(&witness_path, corrupt).expect("write corrupt witness");
    let before = audit_count(&runtime_root3, &run_id3);
    let err = recall_recommendations_for_run(
        &runtime_root3,
        &run_id3,
        &json!({"failure_summary": "import", "k": 1}),
    )
    .expect_err("corrupt witness preflight");
    let after = audit_count(&runtime_root3, &run_id3);
    println!(
        "RECALL_EDGE_CORRUPT_WITNESS before_audits={before} after_audits={after} error_code={}",
        err.error_code
    );
    assert_eq!(err.error_code, "CCREALITY_WITNESS_PREV_HASH_MISMATCH");
    assert_eq!(after, 0);
}

#[test]
#[ignore = "writes persistent manual FSV evidence under target/; run explicitly"]
fn recall_manual_fsv_persists_target_evidence() {
    let evidence_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/context-graph-recall-fsv");
    if evidence_root.exists() {
        fs::remove_dir_all(&evidence_root).expect("clear previous recall fsv evidence");
    }
    let runtime_root = evidence_root.join("runtime-root");
    let run_id = "run-recall-fsv";
    fs::create_dir_all(runtime_root.join(run_id).join("reality-optimizer")).expect("optimizer dir");
    write_recommendation(
        &runtime_root,
        run_id,
        "task-a",
        1,
        valid_recommendation(run_id, 1, "missing import module loader generic fix", 0.1),
    );
    write_recommendation(
        &runtime_root,
        run_id,
        "task-b",
        2,
        valid_recommendation(
            run_id,
            1,
            "missing import module loader generic fix duplicate",
            0.1,
        ),
    );
    write_recommendation(
        &runtime_root,
        run_id,
        "task-c",
        3,
        valid_recommendation(
            run_id,
            1,
            "timeout process watchdog runtime generic fix",
            0.0,
        ),
    );
    let happy_before = audit_count(&runtime_root, run_id);
    let happy_result = recall_recommendations_for_run(
        &runtime_root,
        run_id,
        &json!({"failure_summary":"missing import module timeout process","k":2,"alpha":1.0,"beta":0.0,"gamma":0.0,"lambda":0.30}),
    )
    .expect("happy recall");
    let happy_after = audit_count(&runtime_root, run_id);
    let audit_path = file_arg_to_path(
        happy_result["source_of_truth"]["recommendation_recall_audit"]
            .as_str()
            .unwrap(),
    );
    let audit = read_json(&audit_path).expect("audit readback");
    let witness_path = runtime_root
        .join(run_id)
        .join("claude-code-optimizer")
        .join("witness-chain.bin");
    let witness_bytes = fs::read(&witness_path).expect("witness readback");
    let witness = verify_chain_bytes_with_type_validator(&witness_bytes, |ty| ty == 7)
        .expect("witness verifies");

    let missing_run = "run-missing";
    fs::create_dir_all(runtime_root.join(missing_run).join("reality-optimizer"))
        .expect("missing optimizer dir");
    let missing_before = audit_count(&runtime_root, missing_run);
    let missing_err = recall_recommendations_for_run(
        &runtime_root,
        missing_run,
        &json!({"failure_summary": "import", "k": 1}),
    )
    .expect_err("missing recommendations edge");
    let missing_after = audit_count(&runtime_root, missing_run);

    let invalid_lambda_run = "run-invalid-lambda";
    fs::create_dir_all(
        runtime_root
            .join(invalid_lambda_run)
            .join("reality-optimizer"),
    )
    .expect("invalid lambda optimizer dir");
    write_recommendation(
        &runtime_root,
        invalid_lambda_run,
        "task-a",
        1,
        valid_recommendation(
            invalid_lambda_run,
            1,
            "missing import module loader generic fix",
            0.1,
        ),
    );
    let invalid_lambda_before = audit_count(&runtime_root, invalid_lambda_run);
    let invalid_lambda_err = recall_recommendations_for_run(
        &runtime_root,
        invalid_lambda_run,
        &json!({"failure_summary": "import", "k": 1, "lambda": 1.5}),
    )
    .expect_err("invalid lambda edge");
    let invalid_lambda_after = audit_count(&runtime_root, invalid_lambda_run);

    let corrupt_witness_run = "run-corrupt-witness";
    fs::create_dir_all(
        runtime_root
            .join(corrupt_witness_run)
            .join("reality-optimizer"),
    )
    .expect("corrupt witness optimizer dir");
    write_recommendation(
        &runtime_root,
        corrupt_witness_run,
        "task-a",
        1,
        valid_recommendation(
            corrupt_witness_run,
            1,
            "missing import module loader generic fix",
            0.1,
        ),
    );
    let corrupt_witness_path = runtime_root
        .join(corrupt_witness_run)
        .join("claude-code-optimizer")
        .join("witness-chain.bin");
    fs::create_dir_all(corrupt_witness_path.parent().unwrap()).expect("witness parent");
    let mut corrupt = vec![0u8; WITNESS_ENTRY_SIZE];
    corrupt[0] = 1;
    fs::write(&corrupt_witness_path, corrupt).expect("write corrupt witness");
    let corrupt_witness_before = audit_count(&runtime_root, corrupt_witness_run);
    let corrupt_witness_err = recall_recommendations_for_run(
        &runtime_root,
        corrupt_witness_run,
        &json!({"failure_summary": "import", "k": 1}),
    )
    .expect_err("corrupt witness edge");
    let corrupt_witness_after = audit_count(&runtime_root, corrupt_witness_run);

    let evidence = json!({
        "schema_version": 1,
        "record_kind": "ccreality_recommendation_recall_manual_fsv_evidence",
        "source_of_truth": {
            "runtime_root": file_sot(&runtime_root),
            "happy_audit": file_sot(&audit_path),
            "happy_witness_chain": file_sot(&witness_path)
        },
        "happy_path": {
            "before": {"audit_count": happy_before},
            "after": {
                "audit_count": happy_after,
                "audit_sha256": sha256_file(&audit_path).unwrap(),
                "witness_bytes": witness_bytes.len(),
                "witness_entries": witness.entries,
                "selected_recommendations": audit["selected_recommendations"],
                "candidate_count": audit["candidate_count"]
            }
        },
        "edge_cases": [
            {
                "name": "missing_recommendations",
                "before": {"audit_count": missing_before},
                "after": {"audit_count": missing_after, "error_code": missing_err.error_code}
            },
            {
                "name": "invalid_lambda",
                "before": {"audit_count": invalid_lambda_before},
                "after": {"audit_count": invalid_lambda_after, "error_code": invalid_lambda_err.error_code}
            },
            {
                "name": "corrupt_witness_preflight",
                "before": {"audit_count": corrupt_witness_before},
                "after": {"audit_count": corrupt_witness_after, "error_code": corrupt_witness_err.error_code, "source_of_truth": corrupt_witness_err.source_of_truth}
            }
        ]
    });
    let evidence_path = evidence_root.join("manual-evidence.json");
    write_json_checked(&evidence_path, &evidence).expect("write manual evidence");
    let readback = read_json(&evidence_path).expect("read manual evidence");
    assert_eq!(readback, evidence);
    println!(
        "RECALL_MANUAL_FSV_SOURCE_OF_TRUTH {}",
        file_sot(&evidence_path)
    );
    println!(
        "RECALL_MANUAL_FSV_HAPPY after_audits={} witness_entries={} selected_count={}",
        happy_after,
        witness.entries,
        readback["happy_path"]["after"]["selected_recommendations"]
            .as_array()
            .unwrap()
            .len()
    );
    for edge in readback["edge_cases"].as_array().unwrap() {
        println!(
            "RECALL_MANUAL_FSV_EDGE name={} before={} after={} error_code={}",
            edge["name"].as_str().unwrap(),
            edge["before"]["audit_count"],
            edge["after"]["audit_count"],
            edge["after"]["error_code"].as_str().unwrap()
        );
        assert_eq!(edge["before"]["audit_count"], json!(0));
        assert_eq!(edge["after"]["audit_count"], json!(0));
    }
}
