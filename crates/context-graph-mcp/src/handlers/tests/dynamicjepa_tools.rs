//! DynamicJEPA MCP Phase 9 full-state verification.
//!
//! This test exercises the MCP `tools/call` dispatch layer, real DynamicJEPA
//! command handlers, real RocksDB storage, real CUDA training, and decoded
//! source-of-truth reads after writes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use context_graph_core::dynamicjepa::DjRecordHeader;
use context_graph_core::dynamicjepa::{
    AdapterId, AdapterRunRecord, AdapterRunStatus, DomainPackId, DynamicJepaRecord, EventId,
    PayloadFormat, RawDomainEvent, SourceKind, ADAPTER_RUN_RECORD_VERSION,
    RAW_DOMAIN_EVENT_RECORD_VERSION,
};
use context_graph_storage::dynamicjepa::column_families::{
    CF_DJ_ACTIONS, CF_DJ_ADAPTER_RUNS, CF_DJ_AUDIT_LOG, CF_DJ_BINDINGS, CF_DJ_DATASET_SHARDS,
    CF_DJ_DOMAIN_PACKS, CF_DJ_GUARD_DECISIONS, CF_DJ_LATENT_PANELS, CF_DJ_MODEL_ARTIFACTS,
    CF_DJ_OUTCOMES, CF_DJ_PAIRWISE_READINGS, CF_DJ_PLAN_TRACES, CF_DJ_PREDICTIONS,
    CF_DJ_RAW_EVENTS, CF_DJ_SKILL_POLICIES, CF_DJ_SURPRISE_EVENTS, CF_DJ_TRAINING_RUNS,
    CF_DJ_TRAJECTORIES, CF_DJ_TRANSITIONS, DJ_CF_COUNT,
};
use context_graph_storage::dynamicjepa::{
    inspect_cf, inspect_cf_key, put_raw_event_and_adapter_run_started, snapshot_dj_counts,
    AuditStatus, DjAuditRecord,
};
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::handlers::Handlers;
use crate::protocol::JsonRpcId;
use crate::tools::{get_tool_definitions, names as tool_names};

use super::{create_protocol_handlers_from_store, create_protocol_test_handlers, make_request};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn write_counter_fixture(temp: &Path) -> PathBuf {
    let mut lines = Vec::new();
    for counter in 0..60 {
        let delta = [-2, -1, 0, 1, 2][counter % 5];
        let next_counter = counter as i64 + delta;
        lines.push(format!(
            r#"{{"state":{{"counter":{counter}}},"action":{{"delta":{delta}}},"outcome":{{"next_counter":{next_counter}}},"ts":{}}}"#,
            1_700_000_000_000i64 + counter as i64 * 1_000
        ));
    }
    let path = temp.join("counter_world_phase9_mcp_generated_60.jsonl");
    fs::write(&path, lines.join("\n") + "\n").expect("write counter fixture");
    path
}

fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn count(value: &Value, cf: &str) -> u64 {
    value["counts"][cf]
        .as_u64()
        .unwrap_or_else(|| panic!("missing count for {cf} in {value:#}"))
}

fn delta(before: &Value, after: &Value, cf: &str) -> i64 {
    count(after, cf) as i64 - count(before, cf) as i64
}

async fn call_tool_result(handlers: &Handlers, name: &str, arguments: Value) -> Value {
    let request = make_request(
        "tools/call",
        Some(JsonRpcId::Number(9)),
        Some(json!({
            "name": name,
            "arguments": arguments,
        })),
    );
    let response = handlers.dispatch(request).await;
    assert!(
        response.error.is_none(),
        "tools/call returned JSON-RPC error for {name}: {:?}",
        response.error
    );
    response
        .result
        .unwrap_or_else(|| panic!("{name} must return an MCP result"))
        .as_ref()
        .clone()
}

async fn call_tool_ok(handlers: &Handlers, name: &str, arguments: Value) -> Value {
    let result = call_tool_result(handlers, name, arguments).await;
    assert_eq!(
        result.get("isError").and_then(Value::as_bool),
        Some(false),
        "{name} returned MCP tool error: {result:#}"
    );
    let structured = result
        .get("structuredContent")
        .unwrap_or_else(|| panic!("{name} missing structuredContent"))
        .clone();
    let text = result["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{name} missing MCP text content"));
    let parsed_text: Value =
        serde_json::from_str(text).unwrap_or_else(|err| panic!("{name} text JSON invalid: {err}"));
    if let Some(operation) = structured.get("operation") {
        assert_eq!(parsed_text.get("operation"), Some(operation));
    }
    if let Some(status) = structured.get("status") {
        assert_eq!(parsed_text.get("status"), Some(status));
    }
    structured
}

async fn call_tool_error(handlers: &Handlers, name: &str, arguments: Value) -> Value {
    let result = call_tool_result(handlers, name, arguments).await;
    assert_eq!(
        result.get("isError").and_then(Value::as_bool),
        Some(true),
        "{name} expected MCP tool error but returned success: {result:#}"
    );
    result
}

async fn inspect_counts_mcp(handlers: &Handlers, db_path: &Path) -> Value {
    call_tool_ok(
        handlers,
        tool_names::DYNAMICJEPA_INSPECT_COUNTS,
        json!({ "dbPath": db_path }),
    )
    .await
}

async fn inspect_cf_mcp(handlers: &Handlers, db_path: &Path, cf: &str, limit: usize) -> Value {
    call_tool_ok(
        handlers,
        tool_names::DYNAMICJEPA_INSPECT_CF,
        json!({ "dbPath": db_path, "cf": cf, "limit": limit }),
    )
    .await
}

async fn inspect_cf_key_mcp(handlers: &Handlers, db_path: &Path, cf: &str, key_hex: &str) -> Value {
    call_tool_ok(
        handlers,
        tool_names::DYNAMICJEPA_INSPECT_CF,
        json!({ "dbPath": db_path, "cf": cf, "keyHex": key_hex }),
    )
    .await
}

fn decoded_rows(value: &Value) -> &[Value] {
    value["decoded_records"]["rows"]
        .as_array()
        .expect("decoded_records.rows")
}

fn decoded_key_row(value: &Value) -> &Value {
    &value["decoded_records"]["row"]["decoded"]
}

fn first_key_hex(inspect_value: &Value) -> String {
    decoded_rows(inspect_value)[0]["key_hex"]
        .as_str()
        .expect("row key_hex")
        .to_string()
}

fn uuid_key_hex(id: &str) -> String {
    id.chars().filter(|ch| *ch != '-').collect()
}

fn uuid_key_bytes(id: &str) -> Vec<u8> {
    Uuid::parse_str(id)
        .unwrap_or_else(|err| panic!("invalid UUID key {id}: {err}"))
        .as_bytes()
        .to_vec()
}

fn string_array(value: &Value, field: &str) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{field} must be an array: {value:#}"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("{field} item must be string: {item:#}"))
                .to_string()
        })
        .collect()
}

fn action_delta(action: &Value) -> i64 {
    action["fields"]["delta"]["I64"]
        .as_i64()
        .unwrap_or_else(|| panic!("candidate action missing delta field: {action:#}"))
}

fn ldb_count(db_path: &Path, cf: &str) -> u64 {
    let output = Command::new("/usr/bin/ldb")
        .args([
            format!("--db={}", db_path.display()),
            format!("--column_family={cf}"),
            "dump".to_string(),
            "--count_only".to_string(),
        ])
        .output()
        .unwrap_or_else(|err| panic!("run ldb count for {cf}: {err}"));
    let stdout = String::from_utf8(output.stdout).expect("ldb stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("ldb stderr is UTF-8");
    assert!(
        output.status.success(),
        "ldb count failed for {cf}: stdout={stdout}\nstderr={stderr}"
    );
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("Keys in range:"))
        .unwrap_or_else(|| panic!("ldb count output missing key count for {cf}: {stdout}"))
        .trim()
        .parse::<u64>()
        .unwrap_or_else(|err| panic!("parse ldb count for {cf}: {err}; stdout={stdout}"))
}

fn seed_started_adapter_run(db_path: &Path) -> String {
    let store = RocksDbTeleologicalStore::open(db_path).expect("open DynamicJEPA DB for seed");
    let db = store.dynamicjepa_db();
    let domain_pack_id = DomainPackId::new("counter_world").expect("valid domain id");
    let adapter_id = AdapterId::new("json_event").expect("valid adapter id");
    let event_id = EventId::new_v4();
    let adapter_run_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let now_ms = 1_700_100_000_000i64;
    let payload = br#"{"state":{"counter":900},"action":{"delta":1},"outcome":{"next_counter":901},"ts":1700100000000}"#;

    let mut raw_event = RawDomainEvent {
        header: DjRecordHeader::new(
            event_id.0,
            RAW_DOMAIN_EVENT_RECORD_VERSION,
            domain_pack_id.clone(),
            "1.0.0",
            now_ms,
            Some(run_id),
        ),
        event_id,
        domain_pack_id: domain_pack_id.clone(),
        adapter_id: adapter_id.clone(),
        source_kind: SourceKind::JsonlFixture,
        source_uri: "mcp_phase9_seed_started_adapter_run.jsonl".to_string(),
        source_offset: 9_999,
        payload_format: PayloadFormat::Json,
        payload_bytes: payload.to_vec(),
        payload_hash: hash_bytes(payload),
        received_at_unix_ms: now_ms,
    };
    raw_event
        .refresh_content_hash()
        .expect("refresh raw event hash");

    let mut adapter_run = AdapterRunRecord {
        header: DjRecordHeader::new(
            adapter_run_id,
            ADAPTER_RUN_RECORD_VERSION,
            domain_pack_id.clone(),
            "1.0.0",
            now_ms,
            Some(run_id),
        ),
        adapter_run_id,
        adapter_id,
        domain_pack_id,
        event_id,
        started_at_unix_ms: now_ms,
        finished_at_unix_ms: None,
        status: AdapterRunStatus::Started,
        error_code: None,
        error_message: None,
        field_path: None,
        expected_kind: None,
        actual_kind: None,
        output_state_id: None,
        output_action_id: None,
        output_outcome_id: None,
        output_transition_id: None,
    };
    adapter_run
        .refresh_content_hash()
        .expect("refresh adapter run hash");

    let audit = DjAuditRecord {
        audit_id: Uuid::new_v4(),
        timestamp_unix_nanos: 1_700_100_000_000_000_000,
        operation: "ingest_event".to_string(),
        actor: "mcp_phase9_fsv_test".to_string(),
        input_ids: vec![raw_event.source_uri.clone()],
        output_ids: vec![event_id.to_string(), adapter_run_id.to_string()],
        cfs_touched: vec![
            CF_DJ_RAW_EVENTS.to_string(),
            CF_DJ_ADAPTER_RUNS.to_string(),
            CF_DJ_AUDIT_LOG.to_string(),
        ],
        content_hashes: vec![
            raw_event.header.content_hash,
            adapter_run.header.content_hash,
        ],
        status: AuditStatus::Ok,
        verification_run_id: None,
        signal_yield: 1,
    };
    put_raw_event_and_adapter_run_started(db, &raw_event, &adapter_run, &audit)
        .expect("persist seeded started adapter run");
    event_id.to_string()
}

fn assert_dynamicjepa_tools_listed() {
    let tools = get_tool_definitions();
    let dynamic_names = tools
        .iter()
        .filter(|tool| tool.name.starts_with("dynamicjepa_"))
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(dynamic_names.len(), 39);
    for name in [
        tool_names::DYNAMICJEPA_REGISTER_DOMAIN_PACK,
        tool_names::DYNAMICJEPA_LIST_DOMAIN_PACKS,
        tool_names::DYNAMICJEPA_GET_DOMAIN_PACK,
        tool_names::DYNAMICJEPA_INGEST_EVENT,
        tool_names::DYNAMICJEPA_RUN_ADAPTER,
        tool_names::DYNAMICJEPA_MATERIALIZE_PANEL,
        tool_names::DYNAMICJEPA_GET_PANEL,
        tool_names::DYNAMICJEPA_LIST_INSTRUMENT_READINGS,
        tool_names::DYNAMICJEPA_CREATE_BINDING,
        tool_names::DYNAMICJEPA_LIST_BINDINGS,
        tool_names::DYNAMICJEPA_COMPILE_TRAJECTORIES,
        tool_names::DYNAMICJEPA_GET_TRAJECTORY,
        tool_names::DYNAMICJEPA_LIST_TRAJECTORIES,
        tool_names::DYNAMICJEPA_COMPILE_DATASET,
        tool_names::DYNAMICJEPA_GET_DATASET_SHARD,
        tool_names::DYNAMICJEPA_INSPECT_DATASET_ROW,
        tool_names::DYNAMICJEPA_TRAIN,
        tool_names::DYNAMICJEPA_GET_TRAINING_RUN,
        tool_names::DYNAMICJEPA_GET_ARTIFACT,
        tool_names::DYNAMICJEPA_PREDICT,
        tool_names::DYNAMICJEPA_PLAN,
        tool_names::DYNAMICJEPA_RECORD_SURPRISE,
        tool_names::DYNAMICJEPA_BUILD_CONSTELLATION,
        tool_names::DYNAMICJEPA_LIST_CONSTELLATIONS,
        tool_names::DYNAMICJEPA_GET_CONSTELLATION,
        tool_names::DYNAMICJEPA_CALIBRATE_THRESHOLD,
        tool_names::DYNAMICJEPA_RECALIBRATE_THRESHOLD,
        tool_names::DYNAMICJEPA_COMPUTE_MC_RATIO,
        tool_names::DYNAMICJEPA_AUDIT_PAIRWISE_MI,
        tool_names::DYNAMICJEPA_CROSS_DOMAIN_TRANSFER,
        tool_names::DYNAMICJEPA_BUILD_SEMANTIC_INDEX,
        tool_names::DYNAMICJEPA_VALIDATE_CORPUS_DIVERSITY,
        tool_names::DYNAMICJEPA_ATTRIBUTE_TEST_DELTA,
        tool_names::DYNAMICJEPA_COMPARE_SHADOW_UTILITY,
        tool_names::DYNAMICJEPA_GET_PREDICTION,
        tool_names::DYNAMICJEPA_GET_PLAN_TRACE,
        tool_names::DYNAMICJEPA_GET_SURPRISE,
        tool_names::DYNAMICJEPA_INSPECT_COUNTS,
        tool_names::DYNAMICJEPA_INSPECT_CF,
    ] {
        assert!(
            dynamic_names.contains(&name),
            "{name} missing from tool definitions"
        );
    }
}

#[test]
fn test_dynamicjepa_phase9_mcp_full_state_verification() {
    std::thread::Builder::new()
        .name("dynamicjepa_phase9_mcp_fsv".to_string())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build Phase 9 MCP FSV runtime");
            runtime.block_on(dynamicjepa_phase9_mcp_full_state_verification_inner());
        })
        .expect("spawn large-stack Phase 9 MCP FSV test thread")
        .join()
        .expect("Phase 9 MCP FSV test thread must not panic");
}

async fn dynamicjepa_phase9_mcp_full_state_verification_inner() {
    assert_dynamicjepa_tools_listed();

    let root = repo_root();
    let evidence_dir = root.join("tmp/5090jepa_evidence/phase9");
    fs::create_dir_all(&evidence_dir).expect("create Phase 9 evidence dir");
    let run_root = evidence_dir.join(format!(
        "mcp_phase9_fsv_runtime_{}_{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    fs::create_dir_all(&run_root).expect("create Phase 9 runtime evidence dir");

    let db_path = run_root.join("dynamicjepa_phase9_mcp_rocksdb");
    let artifact_root = run_root.join("dynamicjepa_phase9_artifacts");
    let fixture = write_counter_fixture(&run_root);
    let domain_pack = root.join("configs/dynamicjepa/domain_packs/counter_world.v1.toml");
    let config = root.join("configs/dynamicjepa/verification/tiny_train_config.json");
    let skill_id = "10000000-0000-0000-0000-000000000001";
    let mut evidence = Vec::new();

    let (handlers, _handler_tempdir) = create_protocol_test_handlers().await;

    let initial = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(count(&initial, CF_DJ_DOMAIN_PACKS), 0);
    evidence.push(json!({"step": "source_of_truth_initial_counts", "output": initial}));

    let empty_before = inspect_counts_mcp(&handlers, &db_path).await;
    let empty_error = call_tool_error(
        &handlers,
        tool_names::DYNAMICJEPA_GET_DOMAIN_PACK,
        json!({ "dbPath": db_path, "id": "" }),
    )
    .await;
    let empty_after = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(empty_before["counts"], empty_after["counts"]);
    println!(
        "PHASE9_EDGE_EMPTY_INPUT before={} error={} after={}",
        empty_before, empty_error, empty_after
    );
    evidence.push(json!({
        "step": "edge_empty_input_rejected_before_storage",
        "before": empty_before,
        "error": empty_error,
        "after": empty_after
    }));

    let before_register = inspect_counts_mcp(&handlers, &db_path).await;
    let register = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_REGISTER_DOMAIN_PACK,
        json!({ "dbPath": db_path, "file": domain_pack }),
    )
    .await;
    assert_eq!(
        register["created_ids"]["domain_pack_id"],
        json!("counter_world")
    );
    let after_register = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(
        delta(&before_register, &after_register, CF_DJ_DOMAIN_PACKS),
        1
    );
    evidence.push(json!({
        "step": "happy_register_domain_pack",
        "before": before_register,
        "output": register,
        "after": after_register
    }));

    let list_domains = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_LIST_DOMAIN_PACKS,
        json!({ "dbPath": db_path, "limit": 10 }),
    )
    .await;
    assert_eq!(
        list_domains["decoded_records"]["domains"]
            .as_array()
            .expect("domains array")
            .len(),
        1
    );
    let get_domain = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_GET_DOMAIN_PACK,
        json!({ "dbPath": db_path, "id": "counter_world", "domainVersion": "1.0.0" }),
    )
    .await;
    assert_eq!(
        get_domain["decoded_records"]["domain_pack"]["id"],
        "counter_world"
    );
    let domain_cf = inspect_cf_mcp(&handlers, &db_path, CF_DJ_DOMAIN_PACKS, 10).await;
    assert_eq!(decoded_rows(&domain_cf).len(), 1);
    evidence.push(json!({
        "step": "readback_domain_pack_tools",
        "list": list_domains,
        "get": get_domain,
        "inspect_cf": domain_cf
    }));

    let duplicate_before = inspect_counts_mcp(&handlers, &db_path).await;
    let duplicate_error = call_tool_error(
        &handlers,
        tool_names::DYNAMICJEPA_REGISTER_DOMAIN_PACK,
        json!({ "dbPath": db_path, "file": root.join("configs/dynamicjepa/domain_packs/counter_world.v1.toml") }),
    )
    .await;
    let duplicate_after = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(duplicate_before["counts"], duplicate_after["counts"]);
    assert_eq!(
        duplicate_error["structuredContent"]["error_code"],
        json!("VALIDATION")
    );
    println!(
        "PHASE9_EDGE_DUPLICATE_REGISTER before={} error={} after={}",
        duplicate_before, duplicate_error, duplicate_after
    );
    evidence.push(json!({
        "step": "edge_duplicate_register_no_write",
        "before": duplicate_before,
        "error": duplicate_error,
        "after": duplicate_after
    }));

    let invalid_cf_before = inspect_counts_mcp(&handlers, &db_path).await;
    let invalid_cf_error = call_tool_error(
        &handlers,
        tool_names::DYNAMICJEPA_INSPECT_CF,
        json!({ "dbPath": db_path, "cf": "dj_not_a_real_cf", "limit": 1 }),
    )
    .await;
    let invalid_cf_after = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(invalid_cf_before["counts"], invalid_cf_after["counts"]);
    println!(
        "PHASE9_EDGE_INVALID_CF before={} error={} after={}",
        invalid_cf_before, invalid_cf_error, invalid_cf_after
    );
    evidence.push(json!({
        "step": "edge_invalid_cf_no_write",
        "before": invalid_cf_before,
        "error": invalid_cf_error,
        "after": invalid_cf_after
    }));

    let before_ingest = inspect_counts_mcp(&handlers, &db_path).await;
    let ingest = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_INGEST_EVENT,
        json!({
            "dbPath": db_path,
            "domain": "counter_world",
            "adapter": "json_event",
            "file": fixture
        }),
    )
    .await;
    let after_ingest = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(delta(&before_ingest, &after_ingest, CF_DJ_RAW_EVENTS), 60);
    assert_eq!(delta(&before_ingest, &after_ingest, CF_DJ_TRANSITIONS), 60);
    let first_event_id = ingest["created_ids"]["raw_event_ids"][0]
        .as_str()
        .expect("first raw event id")
        .to_string();
    evidence.push(json!({
        "step": "happy_ingest_event",
        "before": before_ingest,
        "output": ingest,
        "after": after_ingest
    }));

    let before_panels = inspect_counts_mcp(&handlers, &db_path).await;
    let panels = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_MATERIALIZE_PANEL,
        json!({ "dbPath": db_path, "allPending": true, "domain": "counter_world" }),
    )
    .await;
    let after_panels = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(
        delta(&before_panels, &after_panels, CF_DJ_LATENT_PANELS),
        60
    );
    let first_panel_id = panels["created_ids"]["panel_ids"][0]
        .as_str()
        .expect("first panel id")
        .to_string();
    let first_panel = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_GET_PANEL,
        json!({ "dbPath": db_path, "panelId": first_panel_id, "includeReadings": true }),
    )
    .await;
    assert_eq!(
        first_panel["decoded_records"]["instrument_readings"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let readings = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_LIST_INSTRUMENT_READINGS,
        json!({ "dbPath": db_path, "eventId": first_event_id }),
    )
    .await;
    assert_eq!(
        readings["decoded_records"]["instrument_readings"]
            .as_array()
            .expect("instrument readings")
            .len(),
        2
    );
    evidence.push(json!({
        "step": "happy_materialize_and_get_panel",
        "before": before_panels,
        "output": panels,
        "after": after_panels,
        "get_panel": first_panel,
        "list_instrument_readings": readings
    }));

    let before_traj = inspect_counts_mcp(&handlers, &db_path).await;
    let trajectories = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_COMPILE_TRAJECTORIES,
        json!({ "dbPath": db_path, "domain": "counter_world", "policy": "by_domain_session" }),
    )
    .await;
    let after_traj = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(delta(&before_traj, &after_traj, CF_DJ_TRAJECTORIES), 1);
    let trajectory_id = trajectories["created_ids"]["trajectory_ids"][0]
        .as_str()
        .expect("trajectory id")
        .to_string();
    let get_trajectory = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_GET_TRAJECTORY,
        json!({ "dbPath": db_path, "id": trajectory_id }),
    )
    .await;
    let list_trajectories = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_LIST_TRAJECTORIES,
        json!({ "dbPath": db_path, "domain": "counter_world", "limit": 10 }),
    )
    .await;
    assert_eq!(
        list_trajectories["decoded_records"]["total_for_domain"],
        json!(1)
    );
    evidence.push(json!({
        "step": "happy_compile_and_read_trajectories",
        "before": before_traj,
        "output": trajectories,
        "after": after_traj,
        "get": get_trajectory,
        "list": list_trajectories
    }));

    let raw_cf = inspect_cf_mcp(&handlers, &db_path, CF_DJ_RAW_EVENTS, 1).await;
    let traj_cf = inspect_cf_mcp(&handlers, &db_path, CF_DJ_TRAJECTORIES, 1).await;
    let before_binding = inspect_counts_mcp(&handlers, &db_path).await;
    let binding = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_CREATE_BINDING,
        json!({
            "dbPath": db_path,
            "leftCf": CF_DJ_RAW_EVENTS,
            "leftKey": first_key_hex(&raw_cf),
            "rightCf": CF_DJ_TRAJECTORIES,
            "rightKey": first_key_hex(&traj_cf),
            "method": "explicit_mapping",
            "kind": "event_to_trajectory",
            "score": 1.0
        }),
    )
    .await;
    let after_binding = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(delta(&before_binding, &after_binding, CF_DJ_BINDINGS), 1);
    let list_bindings = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_LIST_BINDINGS,
        json!({ "dbPath": db_path, "limit": 10 }),
    )
    .await;
    assert_eq!(
        list_bindings["decoded_records"]["total_after_filter"],
        json!(1)
    );
    evidence.push(json!({
        "step": "happy_create_and_list_bindings",
        "before": before_binding,
        "output": binding,
        "after": after_binding,
        "list": list_bindings
    }));

    let before_train_ds = inspect_counts_mcp(&handlers, &db_path).await;
    let train_ds = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_COMPILE_DATASET,
        json!({ "dbPath": db_path, "domain": "counter_world", "policy": "one_step", "split": "train" }),
    )
    .await;
    let dataset_id = train_ds["created_ids"]["dataset_id"]
        .as_str()
        .expect("dataset id")
        .to_string();
    let train_shard_id = train_ds["created_ids"]["shard_id"]
        .as_str()
        .expect("train shard id")
        .to_string();
    assert_eq!(train_ds["summary"]["row_count"], 41);
    let val_ds = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_COMPILE_DATASET,
        json!({ "dbPath": db_path, "domain": "counter_world", "policy": "one_step", "split": "val" }),
    )
    .await;
    let val_shard_id = val_ds["created_ids"]["shard_id"]
        .as_str()
        .expect("val shard id")
        .to_string();
    assert_eq!(val_ds["created_ids"]["dataset_id"], dataset_id);
    assert_eq!(val_ds["summary"]["row_count"], 12);
    let test_ds = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_COMPILE_DATASET,
        json!({ "dbPath": db_path, "domain": "counter_world", "policy": "one_step", "split": "test" }),
    )
    .await;
    assert_eq!(test_ds["created_ids"]["dataset_id"], dataset_id);
    assert_eq!(test_ds["summary"]["row_count"], 6);
    let after_train_ds = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(
        delta(&before_train_ds, &after_train_ds, CF_DJ_DATASET_SHARDS),
        3
    );
    let get_shard = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_GET_DATASET_SHARD,
        json!({ "dbPath": db_path, "datasetId": dataset_id, "shardId": train_shard_id }),
    )
    .await;
    let row0 = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_INSPECT_DATASET_ROW,
        json!({ "dbPath": db_path, "datasetId": dataset_id, "shardId": train_shard_id, "row": 0 }),
    )
    .await;
    assert_eq!(
        get_shard["decoded_records"]["dataset_shard"]["row_count"],
        41
    );
    evidence.push(json!({
        "step": "happy_compile_and_read_datasets",
        "before": before_train_ds,
        "train": train_ds,
        "val": val_ds,
        "test": test_ds,
        "after": after_train_ds,
        "get_shard": get_shard,
        "inspect_row": row0
    }));

    let input_panel_id = row0["decoded_records"]["row"]["input_panel_id"]
        .as_str()
        .expect("input panel id")
        .to_string();
    let action_id = row0["decoded_records"]["row"]["input_action_id"]
        .as_str()
        .expect("action id")
        .to_string();

    let missing_predict_before = inspect_counts_mcp(&handlers, &db_path).await;
    let missing_predict = call_tool_error(
        &handlers,
        tool_names::DYNAMICJEPA_PREDICT,
        json!({
            "dbPath": db_path,
            "artifactId": "00000000-0000-0000-0000-000000000123",
            "panelId": input_panel_id,
            "actionId": action_id
        }),
    )
    .await;
    let missing_predict_after = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(
        delta(
            &missing_predict_before,
            &missing_predict_after,
            CF_DJ_PREDICTIONS
        ),
        0
    );
    println!(
        "PHASE9_EDGE_MISSING_ARTIFACT before={} error={} after={}",
        missing_predict_before, missing_predict, missing_predict_after
    );
    evidence.push(json!({
        "step": "edge_missing_artifact_no_prediction_write",
        "before": missing_predict_before,
        "error": missing_predict,
        "after": missing_predict_after
    }));

    let before_train = inspect_counts_mcp(&handlers, &db_path).await;
    let train = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_TRAIN,
        json!({
            "dbPath": db_path,
            "datasetId": dataset_id,
            "config": config,
            "artifactRoot": artifact_root
        }),
    )
    .await;
    let after_train = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(delta(&before_train, &after_train, CF_DJ_TRAINING_RUNS), 1);
    assert_eq!(delta(&before_train, &after_train, CF_DJ_MODEL_ARTIFACTS), 1);
    let training_run_id = train["created_ids"]["training_run_id"]
        .as_str()
        .expect("training run id")
        .to_string();
    let artifact_id = train["created_ids"]["artifact_id"]
        .as_str()
        .expect("artifact id")
        .to_string();
    assert!(train["artifact_hashes"]
        .as_array()
        .expect("artifact hashes")
        .iter()
        .all(|file| file["equal"].as_bool() == Some(true)));
    let get_training = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_GET_TRAINING_RUN,
        json!({ "dbPath": db_path, "id": training_run_id }),
    )
    .await;
    let get_artifact = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_GET_ARTIFACT,
        json!({ "dbPath": db_path, "id": artifact_id, "verifyFiles": true }),
    )
    .await;
    assert!(get_artifact["artifact_hashes"]
        .as_array()
        .expect("verified hashes")
        .iter()
        .all(|file| file["equal"].as_bool() == Some(true)));
    evidence.push(json!({
        "step": "happy_train_and_artifact_readback",
        "before": before_train,
        "output": train,
        "after": after_train,
        "get_training": get_training,
        "get_artifact": get_artifact
    }));

    let before_predict = inspect_counts_mcp(&handlers, &db_path).await;
    let predict = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_PREDICT,
        json!({
            "dbPath": db_path,
            "artifactId": artifact_id,
            "panelId": input_panel_id,
            "actionId": action_id
        }),
    )
    .await;
    let after_predict = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(delta(&before_predict, &after_predict, CF_DJ_PREDICTIONS), 1);
    let prediction_id = predict["created_ids"]["prediction_id"]
        .as_str()
        .expect("prediction id")
        .to_string();
    let get_prediction = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_GET_PREDICTION,
        json!({ "dbPath": db_path, "id": prediction_id }),
    )
    .await;
    assert_eq!(
        get_prediction["decoded_records"]["prediction"]["prediction_id"],
        prediction_id
    );
    evidence.push(json!({
        "step": "happy_predict_and_readback",
        "before": before_predict,
        "output": predict,
        "after": after_predict,
        "get": get_prediction
    }));

    let prediction_cf_key = inspect_cf_key_mcp(
        &handlers,
        &db_path,
        CF_DJ_PREDICTIONS,
        &uuid_key_hex(&prediction_id),
    )
    .await;
    let prediction_cf_record = decoded_key_row(&prediction_cf_key);
    assert_eq!(
        prediction_cf_record["prediction_id"].as_str(),
        Some(prediction_id.as_str())
    );
    assert_eq!(
        prediction_cf_record["model_artifact_id"].as_str(),
        Some(artifact_id.as_str())
    );
    assert_eq!(
        prediction_cf_record["input_panel_id"].as_str(),
        Some(input_panel_id.as_str())
    );
    assert_eq!(
        prediction_cf_record["candidate_action_id"].as_str(),
        Some(action_id.as_str())
    );
    evidence.push(json!({
        "step": "mcp_inspect_prediction_cf_key_readback",
        "output": prediction_cf_key
    }));

    let before_plan = inspect_counts_mcp(&handlers, &db_path).await;
    let plan = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_PLAN,
        json!({
            "dbPath": db_path,
            "artifactId": artifact_id,
            "panelId": input_panel_id,
            "skillId": skill_id
        }),
    )
    .await;
    let after_plan = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(delta(&before_plan, &after_plan, CF_DJ_ACTIONS), 5);
    assert_eq!(delta(&before_plan, &after_plan, CF_DJ_PREDICTIONS), 5);
    assert_eq!(delta(&before_plan, &after_plan, CF_DJ_GUARD_DECISIONS), 5);
    assert_eq!(plan["summary"]["status"], "Selected");
    let plan_trace_id = plan["created_ids"]["plan_trace_id"]
        .as_str()
        .expect("plan trace id")
        .to_string();
    let get_plan = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_GET_PLAN_TRACE,
        json!({
            "dbPath": db_path,
            "id": plan_trace_id,
            "includePredictions": true,
            "includeGuards": true
        }),
    )
    .await;
    assert_eq!(
        get_plan["decoded_records"]["predictions"]
            .as_array()
            .expect("plan predictions")
            .len(),
        5
    );
    evidence.push(json!({
        "step": "happy_plan_and_readback",
        "before": before_plan,
        "output": plan,
        "after": after_plan,
        "get": get_plan
    }));

    let candidate_action_ids = string_array(
        &plan["created_ids"]["candidate_action_ids"],
        "candidate_action_ids",
    );
    let plan_prediction_ids =
        string_array(&plan["created_ids"]["prediction_ids"], "prediction_ids");
    let guard_decision_ids = string_array(
        &plan["created_ids"]["guard_decision_ids"],
        "guard_decision_ids",
    );
    let selected_action_id = plan["created_ids"]["selected_action_id"]
        .as_str()
        .expect("selected action id")
        .to_string();
    let plan_trace_cf_key = inspect_cf_key_mcp(
        &handlers,
        &db_path,
        CF_DJ_PLAN_TRACES,
        &uuid_key_hex(&plan_trace_id),
    )
    .await;
    let plan_trace_cf_record = decoded_key_row(&plan_trace_cf_key);
    assert_eq!(
        plan_trace_cf_record["plan_trace_id"].as_str(),
        Some(plan_trace_id.as_str())
    );
    assert_eq!(
        string_array(
            &plan_trace_cf_record["candidate_action_ids"],
            "plan_trace.candidate_action_ids"
        ),
        candidate_action_ids
    );
    assert_eq!(
        string_array(
            &plan_trace_cf_record["prediction_ids"],
            "plan_trace.prediction_ids"
        ),
        plan_prediction_ids
    );
    assert_eq!(
        string_array(
            &plan_trace_cf_record["guard_decision_ids"],
            "plan_trace.guard_decision_ids"
        ),
        guard_decision_ids
    );
    assert_eq!(
        plan_trace_cf_record["selected_action_id"].as_str(),
        Some(selected_action_id.as_str())
    );

    let mut action_cf_key_readbacks = Vec::new();
    for id in &candidate_action_ids {
        action_cf_key_readbacks
            .push(inspect_cf_key_mcp(&handlers, &db_path, CF_DJ_ACTIONS, &uuid_key_hex(id)).await);
    }
    let persisted_deltas = action_cf_key_readbacks
        .iter()
        .map(|row| action_delta(decoded_key_row(row)))
        .collect::<Vec<_>>();
    assert_eq!(persisted_deltas, vec![-2, -1, 0, 1, 2]);
    for (idx, row) in action_cf_key_readbacks.iter().enumerate() {
        let decoded = decoded_key_row(row);
        assert_eq!(
            decoded["action_id"].as_str(),
            Some(candidate_action_ids[idx].as_str())
        );
        assert_eq!(decoded["action_origin"].as_str(), Some("Hypothetical"));
    }

    let mut plan_prediction_cf_key_readbacks = Vec::new();
    for id in &plan_prediction_ids {
        plan_prediction_cf_key_readbacks.push(
            inspect_cf_key_mcp(&handlers, &db_path, CF_DJ_PREDICTIONS, &uuid_key_hex(id)).await,
        );
    }
    for (idx, row) in plan_prediction_cf_key_readbacks.iter().enumerate() {
        let decoded = decoded_key_row(row);
        assert_eq!(
            decoded["prediction_id"].as_str(),
            Some(plan_prediction_ids[idx].as_str())
        );
        assert_eq!(
            decoded["input_panel_id"].as_str(),
            Some(input_panel_id.as_str())
        );
        assert_eq!(
            decoded["candidate_action_id"].as_str(),
            Some(candidate_action_ids[idx].as_str())
        );
        assert_eq!(
            decoded["header"]["source_run_id"].as_str(),
            Some(plan_trace_id.as_str())
        );
    }

    let mut guard_cf_key_readbacks = Vec::new();
    for id in &guard_decision_ids {
        guard_cf_key_readbacks.push(
            inspect_cf_key_mcp(
                &handlers,
                &db_path,
                CF_DJ_GUARD_DECISIONS,
                &uuid_key_hex(id),
            )
            .await,
        );
    }
    for row in &guard_cf_key_readbacks {
        let decoded = decoded_key_row(row);
        assert_eq!(
            decoded["plan_trace_id"].as_str(),
            Some(plan_trace_id.as_str())
        );
        let guard_action_id = decoded["candidate_action_id"]
            .as_str()
            .expect("guard candidate_action_id");
        assert!(
            candidate_action_ids.iter().any(|id| id == guard_action_id),
            "guard action {guard_action_id} must reference a persisted candidate"
        );
    }

    let skill_policy_cf_key = inspect_cf_key_mcp(
        &handlers,
        &db_path,
        CF_DJ_SKILL_POLICIES,
        &uuid_key_hex(skill_id),
    )
    .await;
    assert_eq!(
        decoded_key_row(&skill_policy_cf_key)["skill_id"].as_str(),
        Some(skill_id)
    );
    evidence.push(json!({
        "step": "mcp_inspect_plan_cf_key_readbacks",
        "plan_trace": plan_trace_cf_key,
        "candidate_actions": action_cf_key_readbacks,
        "predictions": plan_prediction_cf_key_readbacks,
        "guard_decisions": guard_cf_key_readbacks,
        "skill_policy": skill_policy_cf_key
    }));

    let val_row = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_INSPECT_DATASET_ROW,
        json!({ "dbPath": db_path, "datasetId": dataset_id, "shardId": val_shard_id, "row": 0 }),
    )
    .await;
    let far_target_panel_id = val_row["decoded_records"]["row"]["target_panel_id"]
        .as_str()
        .expect("far target panel id")
        .to_string();
    let far_target_panel = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_GET_PANEL,
        json!({ "dbPath": db_path, "panelId": far_target_panel_id, "includeReadings": true }),
    )
    .await;
    let far_outcome_id = far_target_panel["decoded_records"]["panel"]["outcome_id"]
        .as_str()
        .expect("far target outcome id")
        .to_string();
    let before_surprise = inspect_counts_mcp(&handlers, &db_path).await;
    let surprise = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_RECORD_SURPRISE,
        json!({
            "dbPath": db_path,
            "predictionId": prediction_id,
            "observedOutcomeId": far_outcome_id,
            "observedPanelId": far_target_panel_id
        }),
    )
    .await;
    let after_surprise = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(
        delta(&before_surprise, &after_surprise, CF_DJ_SURPRISE_EVENTS),
        1
    );
    let surprise_id = surprise["created_ids"]["surprise_event_id"]
        .as_str()
        .expect("surprise event id")
        .to_string();
    let get_surprise = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_GET_SURPRISE,
        json!({ "dbPath": db_path, "id": surprise_id }),
    )
    .await;
    assert_eq!(
        get_surprise["decoded_records"]["surprise_event"]["surprise_event_id"],
        surprise_id
    );
    evidence.push(json!({
        "step": "happy_record_surprise_and_readback",
        "before": before_surprise,
        "output": surprise,
        "after": after_surprise,
        "get": get_surprise
    }));

    let surprise_cf_key = inspect_cf_key_mcp(
        &handlers,
        &db_path,
        CF_DJ_SURPRISE_EVENTS,
        &uuid_key_hex(&surprise_id),
    )
    .await;
    let surprise_cf_record = decoded_key_row(&surprise_cf_key);
    assert_eq!(
        surprise_cf_record["surprise_event_id"].as_str(),
        Some(surprise_id.as_str())
    );
    assert_eq!(
        surprise_cf_record["prediction_id"].as_str(),
        Some(prediction_id.as_str())
    );
    assert_eq!(
        surprise_cf_record["observed_outcome_id"].as_str(),
        Some(far_outcome_id.as_str())
    );
    assert_eq!(
        surprise_cf_record["observed_panel_id"].as_str(),
        Some(far_target_panel_id.as_str())
    );
    evidence.push(json!({
        "step": "mcp_inspect_surprise_cf_key_readback",
        "output": surprise_cf_key
    }));

    let mc_ratio_root = run_root.join("dynamicjepa_phase9_mc_ratio_out");
    let before_mc_ratio = inspect_counts_mcp(&handlers, &db_path).await;
    let mc_ratio = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_COMPUTE_MC_RATIO,
        json!({
            "dbPath": db_path,
            "domain": "counter_world",
            "outputDir": mc_ratio_root
        }),
    )
    .await;
    let after_mc_ratio = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(before_mc_ratio["counts"], after_mc_ratio["counts"]);
    assert_eq!(mc_ratio["summary"]["domain"], json!("counter_world"));
    assert!(run_root
        .join("dynamicjepa_phase9_mc_ratio_out/paper_tables/table_mc_ratio.csv")
        .is_file());
    assert!(run_root
        .join("dynamicjepa_phase9_mc_ratio_out/mc_ratio_per_stage.json")
        .is_file());
    assert!(run_root
        .join("dynamicjepa_phase9_mc_ratio_out/plots/signal_density_per_event_distribution.svg")
        .is_file());
    evidence.push(json!({
        "step": "happy_compute_mc_ratio_mcp_file_readback",
        "before": before_mc_ratio,
        "output": mc_ratio,
        "after": after_mc_ratio
    }));

    let mi_root = run_root.join("dynamicjepa_phase9_pairwise_mi_out");
    let before_pairwise_mi = inspect_counts_mcp(&handlers, &db_path).await;
    let pairwise_mi = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_AUDIT_PAIRWISE_MI,
        json!({
            "dbPath": db_path,
            "domain": "counter_world",
            "sampleSize": 50,
            "ksgK": 3,
            "bootstrapIters": 8,
            "seed": 20260501,
            "outputDir": mi_root
        }),
    )
    .await;
    let after_pairwise_mi = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(
        delta(&before_pairwise_mi, &after_pairwise_mi, CF_DJ_AUDIT_LOG),
        1
    );
    assert_eq!(pairwise_mi["summary"]["domain"], json!("counter_world"));
    assert_eq!(pairwise_mi["summary"]["n_pairs"], json!(1));
    assert!(run_root
        .join("dynamicjepa_phase9_pairwise_mi_out/pairwise_mi_matrix.csv")
        .is_file());
    assert!(run_root
        .join("dynamicjepa_phase9_pairwise_mi_out/paper_tables/table_pairwise_mi_summary.csv")
        .is_file());
    assert!(run_root
        .join("dynamicjepa_phase9_pairwise_mi_out/pairwise_mi_heatmap.svg")
        .is_file());
    assert!(run_root
        .join("dynamicjepa_phase9_pairwise_mi_out/pairwise_mi_audit.json")
        .is_file());
    evidence.push(json!({
        "step": "happy_audit_pairwise_mi_mcp_file_and_audit_log_readback",
        "before": before_pairwise_mi,
        "output": pairwise_mi,
        "after": after_pairwise_mi
    }));

    let transfer_existing_root = run_root.join("dynamicjepa_phase9_existing_transfer_root");
    fs::create_dir_all(&transfer_existing_root).expect("create existing transfer root");
    let transfer_edge_before = inspect_counts_mcp(&handlers, &db_path).await;
    let transfer_existing_root_error = call_tool_error(
        &handlers,
        tool_names::DYNAMICJEPA_CROSS_DOMAIN_TRANSFER,
        json!({
            "outputRoot": transfer_existing_root,
            "seeds": [42],
            "sourceEvents": 40,
            "targetEvents": 24,
            "bootstrapIters": 1,
            "trainEpochs": 1,
            "batchSize": 1,
            "maxSecondsPerTraining": 1,
            "stoppingTarget": 0.50
        }),
    )
    .await;
    let transfer_edge_after = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(
        transfer_edge_before["counts"],
        transfer_edge_after["counts"]
    );
    assert_eq!(
        transfer_existing_root_error["structuredContent"]["status"],
        json!("error")
    );
    println!(
        "PHASE9_EDGE_CROSS_DOMAIN_EXISTING_OUTPUT before={} error={} after={}",
        transfer_edge_before, transfer_existing_root_error, transfer_edge_after
    );
    evidence.push(json!({
        "step": "edge_cross_domain_transfer_existing_output_no_db_write",
        "before": transfer_edge_before,
        "error": transfer_existing_root_error,
        "after": transfer_edge_after
    }));

    let seed_before = inspect_counts_mcp(&handlers, &db_path).await;
    let seeded_event_id = seed_started_adapter_run(&db_path);
    let seed_after = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(delta(&seed_before, &seed_after, CF_DJ_RAW_EVENTS), 1);
    assert_eq!(delta(&seed_before, &seed_after, CF_DJ_ADAPTER_RUNS), 1);
    let before_run_adapter = inspect_counts_mcp(&handlers, &db_path).await;
    let run_adapter = call_tool_ok(
        &handlers,
        tool_names::DYNAMICJEPA_RUN_ADAPTER,
        json!({ "dbPath": db_path, "eventId": seeded_event_id }),
    )
    .await;
    let after_run_adapter = inspect_counts_mcp(&handlers, &db_path).await;
    assert_eq!(
        delta(&before_run_adapter, &after_run_adapter, CF_DJ_TRANSITIONS),
        1
    );
    evidence.push(json!({
        "step": "happy_seeded_run_adapter",
        "seed_before": seed_before,
        "seed_after": seed_after,
        "before": before_run_adapter,
        "output": run_adapter,
        "after": after_run_adapter
    }));

    let final_counts = inspect_counts_mcp(&handlers, &db_path).await;
    println!("PHASE9_FINAL_MCP_COUNTS={}", final_counts);
    evidence.push(json!({"step": "final_mcp_counts", "output": final_counts}));
    drop(handlers);

    let capability_store =
        RocksDbTeleologicalStore::open(&db_path).expect("open populated DB for capability matrix");
    let (capability_handlers, capability_store_arc) =
        create_protocol_handlers_from_store(capability_store).await;
    let capability = call_tool_ok(
        &capability_handlers,
        tool_names::GET_CAPABILITY_MATRIX,
        json!({ "includeRuntimeState": true, "includeToolSchemas": true }),
    )
    .await;
    assert_eq!(capability["mcp"]["toolCount"], get_tool_definitions().len());
    assert_eq!(
        capability["runtime"]["dynamicjepa"]["counts"][CF_DJ_DOMAIN_PACKS],
        1
    );
    assert_eq!(
        capability["runtime"]["dynamicjepa"]["counts"][CF_DJ_MODEL_ARTIFACTS],
        1
    );
    assert_eq!(
        capability["sourceOfTruth"]["dynamicjepa"]
            .as_array()
            .expect("dynamic source of truth")
            .len(),
        DJ_CF_COUNT
    );
    evidence.push(json!({"step": "capability_matrix_dynamicjepa_runtime", "output": capability}));
    drop(capability_handlers);
    drop(capability_store_arc);

    let direct_store =
        RocksDbTeleologicalStore::open(&db_path).expect("open populated DB for direct FSV");
    let direct_db = direct_store.dynamicjepa_db();
    let direct_counts = snapshot_dj_counts(direct_db).expect("direct snapshot counts");
    assert_eq!(direct_counts[CF_DJ_DOMAIN_PACKS], 1);
    assert_eq!(direct_counts[CF_DJ_RAW_EVENTS], 61);
    assert_eq!(direct_counts[CF_DJ_LATENT_PANELS], 60);
    assert_eq!(direct_counts[CF_DJ_TRAJECTORIES], 1);
    assert_eq!(direct_counts[CF_DJ_DATASET_SHARDS], 3);
    assert_eq!(direct_counts[CF_DJ_TRAINING_RUNS], 1);
    assert_eq!(direct_counts[CF_DJ_MODEL_ARTIFACTS], 1);
    assert_eq!(direct_counts[CF_DJ_PREDICTIONS], 6);
    assert_eq!(direct_counts[CF_DJ_GUARD_DECISIONS], 5);
    assert_eq!(direct_counts[CF_DJ_SURPRISE_EVENTS], 1);
    let direct_predictions =
        inspect_cf(direct_db, CF_DJ_PREDICTIONS, 10, 0).expect("direct predictions inspect");
    let direct_surprises =
        inspect_cf(direct_db, CF_DJ_SURPRISE_EVENTS, 10, 0).expect("direct surprises inspect");
    let direct_artifacts =
        inspect_cf(direct_db, CF_DJ_MODEL_ARTIFACTS, 10, 0).expect("direct artifacts inspect");
    let direct_prediction_key = inspect_cf_key(
        direct_db,
        CF_DJ_PREDICTIONS,
        &uuid_key_bytes(&prediction_id),
    )
    .expect("direct prediction key inspect")
    .expect("direct prediction key exists");
    let direct_plan_key = inspect_cf_key(
        direct_db,
        CF_DJ_PLAN_TRACES,
        &uuid_key_bytes(&plan_trace_id),
    )
    .expect("direct plan key inspect")
    .expect("direct plan key exists");
    let direct_surprise_key = inspect_cf_key(
        direct_db,
        CF_DJ_SURPRISE_EVENTS,
        &uuid_key_bytes(&surprise_id),
    )
    .expect("direct surprise key inspect")
    .expect("direct surprise key exists");
    let direct_artifact_key = inspect_cf_key(
        direct_db,
        CF_DJ_MODEL_ARTIFACTS,
        &uuid_key_bytes(&artifact_id),
    )
    .expect("direct artifact key inspect")
    .expect("direct artifact key exists");
    println!(
        "PHASE9_DIRECT_SOURCE_OF_TRUTH counts={:?} predictions={} surprises={} artifacts={}",
        direct_counts,
        serde_json::to_string(&direct_predictions).expect("predictions JSON"),
        serde_json::to_string(&direct_surprises).expect("surprises JSON"),
        serde_json::to_string(&direct_artifacts).expect("artifacts JSON")
    );
    evidence.push(json!({
        "step": "direct_rocksdb_source_of_truth",
        "counts": direct_counts.clone(),
        "predictions": direct_predictions,
        "surprises": direct_surprises,
        "artifacts": direct_artifacts,
        "exact_keys": {
            "prediction": direct_prediction_key,
            "plan_trace": direct_plan_key,
            "surprise": direct_surprise_key,
            "artifact": direct_artifact_key
        }
    }));
    drop(direct_store);

    let physical_count_cfs = [
        CF_DJ_DOMAIN_PACKS,
        CF_DJ_RAW_EVENTS,
        CF_DJ_ADAPTER_RUNS,
        CF_DJ_LATENT_PANELS,
        CF_DJ_OUTCOMES,
        CF_DJ_PAIRWISE_READINGS,
        CF_DJ_TRAJECTORIES,
        CF_DJ_DATASET_SHARDS,
        CF_DJ_TRAINING_RUNS,
        CF_DJ_MODEL_ARTIFACTS,
        CF_DJ_ACTIONS,
        CF_DJ_PREDICTIONS,
        CF_DJ_SKILL_POLICIES,
        CF_DJ_GUARD_DECISIONS,
        CF_DJ_PLAN_TRACES,
        CF_DJ_SURPRISE_EVENTS,
        CF_DJ_AUDIT_LOG,
    ];
    let mut physical_counts = Map::new();
    for cf in physical_count_cfs {
        let physical_count = ldb_count(&db_path, cf);
        assert_eq!(
            physical_count, direct_counts[cf],
            "physical ldb count for {cf} must match direct RocksDB count"
        );
        physical_counts.insert(cf.to_string(), json!(physical_count));
    }
    evidence.push(json!({
        "step": "physical_ldb_count_proof",
        "ldb_counts": physical_counts
    }));

    fs::write(
        evidence_dir.join("mcp_phase9_fsv.json"),
        serde_json::to_vec_pretty(&json!({
            "phase": 9,
            "source_of_truth": {
                "rocksdb_path": db_path,
                "artifact_root": run_root.join("dynamicjepa_phase9_artifacts"),
                "evidence_file": evidence_dir.join("mcp_phase9_fsv.json")
            },
            "tool_count": {
                "dynamicjepa_tools": 35,
                "total_tools": get_tool_definitions().len()
            },
            "evidence": evidence
        }))
        .expect("serialize Phase 9 evidence"),
    )
    .expect("write Phase 9 evidence");
}
