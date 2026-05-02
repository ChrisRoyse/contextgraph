//! Full-state verification for `export dynamicjepa-episodes`.
//!
//! The test builds a real RocksDB with production DynamicJEPA record codecs,
//! exports it through the real CLI binary, then reads both the JSONL artifact
//! and RocksDB counts back through independent paths.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use context_graph_core::dynamicjepa::*;
use context_graph_storage::dynamicjepa::column_families::{
    CF_DJ_AUDIT_LOG, CF_DJ_GUARD_DECISIONS, CF_DJ_PLAN_TRACES, CF_DJ_PREDICTIONS,
    CF_DJ_VERIFICATION_RUNS,
};
use context_graph_storage::dynamicjepa::{
    put_guard_decision, put_plan_trace, put_prediction, put_verification_run, snapshot_dj_counts,
};
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use uuid::Uuid;

const TS0: i64 = 1_700_000_000_000;

fn context_graph_bin() -> &'static str {
    env!("CARGO_BIN_EXE_context-graph")
}

fn uuid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn domain_id() -> DomainPackId {
    DomainPackId::new("counter_world").expect("counter_world id validates")
}

fn hash32(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn header(record_id: Uuid, version: u8) -> DjRecordHeader {
    DjRecordHeader::new(
        record_id,
        version,
        domain_id(),
        "1.0.0",
        TS0,
        Some(uuid(900)),
    )
}

fn seal<R: DynamicJepaRecord>(mut record: R) -> R {
    record
        .refresh_content_hash()
        .expect("content hash computes");
    record.validate_record().expect("record validates");
    record
}

fn run_cli(args: &[&str]) -> (bool, Value) {
    let output = Command::new(context_graph_bin())
        .args(args)
        .env("CONTEXT_GRAPH_ENV", "development")
        .output()
        .expect("execute context-graph binary");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    assert!(
        !stdout.trim().is_empty(),
        "CLI produced empty stdout; stderr={stderr}"
    );
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|err| {
        panic!("stdout must be JSON: {err}\nstdout={stdout}\nstderr={stderr}")
    });
    (output.status.success(), value)
}

fn write_json(path: &Path, value: &Value) {
    fs::write(
        path,
        serde_json::to_string_pretty(value).expect("serialize JSON") + "\n",
    )
    .unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
}

fn make_bundle() -> (TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().expect("tempdir");
    let bundle = temp.path().join("bundle");
    let dbs = bundle.join("dbs");
    let evidence = bundle.join("fsv_evidence");
    fs::create_dir_all(&dbs).expect("create dbs");
    fs::create_dir_all(&evidence).expect("create evidence");

    let run_id = "bundle_export_test";
    let domain = "counter_world";
    let db_path = dbs.join(format!("{run_id}_{domain}_rocksdb"));
    let store = RocksDbTeleologicalStore::open(&db_path).expect("open real RocksDB");
    let db = store.dynamicjepa_db();
    let before_counts = snapshot_dj_counts(db).expect("snapshot before");

    let panel_id = PanelId::from(uuid(10));
    let action_id = ActionId::from(uuid(11));
    let artifact_id = ModelArtifactId::from(uuid(12));
    let prediction_id = PredictionId::from(uuid(13));
    let guard_id = GuardDecisionId::from(uuid(14));
    let plan_id = PlanTraceId::from(uuid(15));
    let skill_id = SkillId::from(uuid(16));
    let verification_id = VerificationRunId::from(uuid(17));
    let artifact_hash = hash32(7);

    let prediction = seal(PredictionRecord {
        header: header(prediction_id.0, PREDICTION_RECORD_VERSION),
        prediction_id,
        model_artifact_id: artifact_id,
        model_artifact_hash_at_inference: artifact_hash,
        input_panel_id: panel_id,
        candidate_action_id: action_id,
        predicted_next_panel_vec: vec![0.25, 0.5, 0.75],
        uncertainty: 0.125,
        objective_scores: BTreeMap::from([("predict_next_counter".to_string(), 0.875)]),
        created_at_unix_ms: TS0 + 1,
    });
    let guard = seal(GuardDecisionRecord {
        header: header(guard_id.0, GUARD_DECISION_RECORD_VERSION),
        guard_decision_id: guard_id,
        plan_trace_id: plan_id.0,
        guard_id: "bounds_check".to_string(),
        candidate_action_id: action_id,
        decision: GuardDecision::Allow,
        evidence_refs: vec![format!("prediction:{prediction_id}")],
        threshold_values: BTreeMap::from([("max_counter".to_string(), 1024.0)]),
        utility_score: Some(0.875),
        utility_decision: Some(GuardDecision::Allow),
        gtau_decision: None,
        constellation_uuid: None,
        cosine_to_centroid_per_modality: None,
        tau_per_modality: None,
        gtau_failed_modalities: None,
        created_at_unix_ms: TS0 + 2,
    });
    let plan = seal(PlanTraceRecord {
        header: header(plan_id.0, PLAN_TRACE_RECORD_VERSION),
        plan_trace_id: plan_id,
        domain_pack_id: domain_id(),
        current_panel_id: panel_id,
        model_artifact_id: artifact_id,
        model_artifact_hash_at_plan: artifact_hash,
        skill_policy_id: skill_id,
        candidate_action_ids: vec![action_id],
        prediction_ids: vec![prediction_id],
        guard_decision_ids: vec![guard_id.0],
        utility_scores: vec![0.875],
        selected_action_id: Some(action_id),
        no_accepted_candidate: false,
        constellation_uuid_used: None,
        status: PlanTraceStatus::Selected,
        created_at_unix_ms: TS0 + 3,
    });

    put_prediction(db, &prediction).expect("persist prediction");
    put_guard_decision(db, &guard).expect("persist guard");
    put_plan_trace(db, &plan).expect("persist plan");

    let mut after_counts = snapshot_dj_counts(db).expect("snapshot before verification");
    after_counts.insert(CF_DJ_VERIFICATION_RUNS.to_string(), 1);
    let verification = seal(VerificationRunRecord {
        header: header(verification_id.0, VERIFICATION_RUN_RECORD_VERSION),
        verification_run_id: verification_id,
        test_name: "bundle_export_real_rocksdb_fsv".to_string(),
        db_path_hash: sha256(db_path.display().to_string().as_bytes()),
        fixture_hashes: vec![hash32(8)],
        before_counts: before_counts.clone(),
        after_counts: after_counts.clone(),
        commands_executed: vec!["persist prediction/guard/plan/verification".to_string()],
        artifact_hash_checks: Vec::new(),
        decoded_record_excerpts: BTreeMap::new(),
        expected_results: BTreeMap::from([(
            "selected_action_id".to_string(),
            DjJsonValue::String(action_id.to_string()),
        )]),
        actual_results: BTreeMap::from([(
            "selected_action_id".to_string(),
            DjJsonValue::String(action_id.to_string()),
        )]),
        status: VerificationStatus::Passed,
        created_at_unix_ms: TS0 + 4,
    });
    put_verification_run(db, &verification).expect("persist verification");
    let final_counts = snapshot_dj_counts(db).expect("snapshot final");

    let edge_evidence = evidence.join("edge_00.json");
    write_json(
        &edge_evidence,
        &json!({"status":"error","error_code":"ADAPTER_FAILED"}),
    );
    let mut edge_after = before_counts.clone();
    edge_after.insert(CF_DJ_AUDIT_LOG.to_string(), 1);

    write_json(
        &bundle.join("run_manifest.json"),
        &json!({
            "bundle_schema_version": 2,
            "status": "passed",
            "run_id": run_id,
            "db_path": dbs.display().to_string(),
        }),
    );
    write_json(
        &bundle.join("fsv_report.json"),
        &json!({"status": "passed"}),
    );
    write_json(
        &bundle.join("db_counts_after.json"),
        &json!({ domain: final_counts }),
    );
    write_json(
        &bundle.join("edge_case_results.json"),
        &json!([{
            "domain": domain,
            "edge_index": 0,
            "evidence_file": edge_evidence.display().to_string(),
            "edge": {
                "name": "empty_input",
                "fixture": "counter_world_edge_empty_input.jsonl",
                "before_counts": before_counts,
                "after_counts": edge_after,
                "expected_delta": {
                    CF_DJ_AUDIT_LOG: 1,
                    CF_DJ_PREDICTIONS: 0,
                    CF_DJ_PLAN_TRACES: 0,
                    CF_DJ_GUARD_DECISIONS: 0
                },
                "error": {
                    "status": "error",
                    "error_code": "ADAPTER_FAILED",
                    "operation": "dynamicjepa_ingest_event",
                    "source_of_truth": [CF_DJ_AUDIT_LOG],
                    "count_delta": {
                        CF_DJ_AUDIT_LOG: 1,
                        CF_DJ_PREDICTIONS: 0,
                        CF_DJ_PLAN_TRACES: 0,
                        CF_DJ_GUARD_DECISIONS: 0
                    },
                    "created_ids": {},
                    "decoded_records": {},
                    "error_message": "adapter failed: empty input"
                }
            }
        }]),
    );
    drop(store);
    (temp, bundle, db_path)
}

#[test]
fn dynamicjepa_bundle_export_jsonl_fsv_real_rocksdb() {
    let (_temp, bundle, db_path) = make_bundle();
    let out = bundle.join("episodes.jsonl");
    let manifest = bundle.join("episodes.manifest.json");

    let before = run_cli(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        db_path.to_str().expect("db path str"),
        "--json",
    ]);
    assert!(before.0, "inspect before failed: {:#}", before.1);

    let (success, export) = run_cli(&[
        "export",
        "dynamicjepa-episodes",
        "--bundle",
        bundle.to_str().expect("bundle str"),
        "--out",
        out.to_str().expect("out str"),
        "--manifest-out",
        manifest.to_str().expect("manifest str"),
    ]);
    assert!(success, "export failed: {export:#}");
    assert_eq!(export["status"], "ok");
    assert_eq!(export["total_records"], 2);
    assert_eq!(export["plan_episode_count"], 1);
    assert_eq!(export["edge_case_episode_count"], 1);

    let lines = fs::read_to_string(&out).expect("read episodes.jsonl");
    let parsed = lines
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("parse JSONL line"))
        .collect::<Vec<_>>();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0]["record_kind"], "dynamicjepa_plan_episode");
    assert_eq!(parsed[0]["expected_persisted_delta"][CF_DJ_PLAN_TRACES], 1);
    assert_eq!(parsed[0]["actual_readback"]["linked_prediction_count"], 1);
    assert_eq!(
        parsed[0]["actual_readback"]["linked_guard_decision_count"],
        1
    );
    assert_eq!(parsed[1]["record_kind"], "dynamicjepa_edge_case_episode");
    assert_eq!(parsed[1]["error_code"], "ADAPTER_FAILED");

    let manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(&manifest).expect("read manifest"))
            .expect("parse manifest");
    assert_eq!(manifest_json["status"], "ok");
    assert_eq!(manifest_json["output"]["readback"]["line_count"], 2);
    assert_eq!(
        manifest_json["output"]["sha256"], export["output_sha256"],
        "manifest and CLI output must agree on artifact hash"
    );

    let after = run_cli(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        db_path.to_str().expect("db path str"),
        "--json",
    ]);
    assert!(after.0, "inspect after failed: {:#}", after.1);
    assert_eq!(
        before.1["counts"], after.1["counts"],
        "export must not mutate the source RocksDB"
    );
}
