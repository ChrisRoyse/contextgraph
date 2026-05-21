use anyhow::{ensure, Context, Result};
use async_trait::async_trait;
use context_graph_core::traits::{
    EmbeddingMetadata, MultiArrayEmbeddingOutput, MultiArrayEmbeddingProvider,
    TeleologicalMemoryStore,
};
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_core::{CoreError, CoreResult};
use context_graph_mcp::handlers::Handlers;
use context_graph_mcp::monitoring::LiveLayerStatusProvider;
use context_graph_mcp::protocol::{JsonRpcId, JsonRpcRequest};
use context_graph_mcp::tools::{get_tool_definitions, names as tool_names};
use context_graph_mejepa::{PredictionId, Verdict};
use context_graph_mejepa_cf::{
    CF_MEJEPA_MISTAKE_LOG, CF_MEJEPA_ONLINE_HEAD_STATE, CF_MEJEPA_REPLAY_BUFFER,
    CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
};
use context_graph_mejepa_train::label_bridge::{
    ability_signature_hash, accepted_label_signature_hash, membership_signature_hash,
    skill_signature_hash,
};
use context_graph_mejepa_train::{
    ability_aware_replay_cell_id, count_cf_rows, lifecycle_audit_id_from_parts,
    mistake_id_from_evidence_parts, open_ability_resolver_rocksdb, read_all_mistake_log_rows,
    read_all_replay_rows, read_all_skill_lifecycle_audit_rows, read_online_head_state_row,
    read_repeat_metric_row, repeat_metric_key, write_skill_lifecycle_audit_row_sync_readback,
    MistakeLogRow, MistakeTruthSource, OnlineHeadRepeatMetricRow, OnlineHeadStateRow,
    ReplayBufferRow, ReplayBufferSource, ReplayRetentionTier, SkillLifecycleAuditRow,
    SkillLifecycleDecision,
};
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use context_graph_storage::{BackgroundGraphBuilder, EdgeRepository, GraphBuilderConfig};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::RwLock;

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-112b-mistake-loop-mcp-tools-fsv";
const REPORT_NAME: &str = "task_py_g_112b_mistake_loop_mcp_tools_fsv.json";

fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT).join(format!(
        "run-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;

    let db_path = root.join("mistake-loop.db");
    let seed = seed_rows()?;
    let db = open_ability_resolver_rocksdb(&db_path, true)?;
    write_skill_lifecycle_audit_row_sync_readback(&db, &seed.lifecycle_audit)?;
    drop(db);

    let definitions_case = definitions_case()?;
    let happy_record = call_tool(
        &root,
        tool_names::MEJEPA_RECORD_MISTAKE,
        json!({
            "dbPath": db_path,
            "panelSignatureHash": seed.panel_signature_hash,
            "mistakeRow": seed.mistake_row,
            "replayRow": seed.replay_row,
            "baseVerdictBeforeUpdate": "pass",
            "nowUnixMs": seed.now_unix_ms
        }),
    )?;
    let history = call_tool(
        &root,
        tool_names::MEJEPA_MISTAKE_HISTORY,
        json!({
            "dbPath": db_path,
            "mistakeId": seed.mistake_row.mistake_id,
            "labelSignatureHash": seed.mistake_row.label_signature_hash,
            "skillSignatureHash": seed.mistake_row.skill_signature_hash,
            "abilitySignatureHash": seed.mistake_row.ability_signature_hash,
            "membershipSignatureHash": seed.mistake_row.membership_signature_hash,
            "includeReplayRows": true,
            "includeLifecycleAudits": true
        }),
    )?;
    let status = call_tool(
        &root,
        tool_names::MEJEPA_MISTAKE_LOOP_STATUS,
        json!({
            "dbPath": db_path,
            "panelSignatureHash": seed.panel_signature_hash,
            "repeatMetricLimit": 10
        }),
    )?;
    let readback = independent_readback(&db_path, &seed)?;

    let edge_missing_cfs = missing_cf_case(&root, &seed.panel_signature_hash)?;
    let edge_invalid_panel = tool_error_case(
        &root,
        tool_names::MEJEPA_MISTAKE_LOOP_STATUS,
        json!({
            "dbPath": db_path,
            "panelSignatureHash": "bad\npanel"
        }),
        "MEJEPA_MISTAKE_LOOP_STATUS_FAILED",
    )?;
    let edge_record_invalid_panel =
        record_invalid_panel_no_partial_write_case(&root, &db_path, &seed)?;
    let edge_target_label = target_label_case(&root, &db_path, &seed)?;

    let all_passed = definitions_case["pass"].as_bool() == Some(true)
        && happy_record["onlineHeadUpdate"]["samePanelSignatureCorrected"].as_bool() == Some(true)
        && history["returned"].as_u64() == Some(1)
        && !history["entries"][0]["skillLifecycleAudits"]
            .as_array()
            .unwrap_or(&Vec::new())
            .is_empty()
        && status["matchingOnlineHeadStateExists"].as_bool() == Some(true)
        && readback["pass"].as_bool() == Some(true)
        && edge_missing_cfs["pass"].as_bool() == Some(true)
        && edge_invalid_panel["pass"].as_bool() == Some(true)
        && edge_record_invalid_panel["pass"].as_bool() == Some(true)
        && edge_target_label["pass"].as_bool() == Some(true);

    let report = json!({
        "artifactKind": "task_py_g_112b_mistake_loop_mcp_tools_fsv",
        "issue": 523,
        "passes": all_passed,
        "fsvRoot": root,
        "dbPath": db_path,
        "tools": {
            "recordMistake": tool_names::MEJEPA_RECORD_MISTAKE,
            "mistakeHistory": tool_names::MEJEPA_MISTAKE_HISTORY,
            "mistakeLoopStatus": tool_names::MEJEPA_MISTAKE_LOOP_STATUS
        },
        "sourceOfTruthCfs": [
            CF_MEJEPA_MISTAKE_LOG,
            CF_MEJEPA_REPLAY_BUFFER,
            CF_MEJEPA_ONLINE_HEAD_STATE,
            CF_MEJEPA_SKILL_LIFECYCLE_AUDIT
        ],
        "definitionCase": definitions_case,
        "happyRecord": happy_record,
        "history": history,
        "status": status,
        "independentReadback": readback,
        "edgeCases": {
            "missingCfs": edge_missing_cfs,
            "invalidPanelSignature": edge_invalid_panel,
            "recordInvalidPanelNoPartialWrite": edge_record_invalid_panel,
            "targetSideLabelsAsLiveInputs": edge_target_label
        },
        "noCachedJsonSummary": true
    });
    let report_path = root.join(REPORT_NAME);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    ensure!(
        all_passed,
        "FSV failed; report at {}",
        report_path.display()
    );
    println!("{}", report_path.display());
    Ok(())
}

#[derive(Clone)]
struct SeedRows {
    panel_signature_hash: String,
    mistake_row: MistakeLogRow,
    replay_row: ReplayBufferRow,
    lifecycle_audit: SkillLifecycleAuditRow,
    repeat_metric_key: String,
    now_unix_ms: i64,
}

fn seed_rows() -> Result<SeedRows> {
    let prediction_id = PredictionId([0x42; 16]);
    let accepted_label_ids = vec![
        "slot:e_ast:api_contract_shift".to_string(),
        "pair:e_ast:e_typegraph:contract_disagreement".to_string(),
    ];
    let active_skill_ids = vec!["skill:api_contract_mismatch".to_string()];
    let active_higher_ability_ids = vec!["ability:call_contract_repair".to_string()];
    let source_membership_keys =
        vec!["membership:chunk-17:code-state-demo:skill:api_contract_mismatch".to_string()];
    let label_signature_hash = accepted_label_signature_hash(&accepted_label_ids)?;
    let skill_signature_hash = skill_signature_hash(&active_skill_ids)?;
    let ability_signature_hash = ability_signature_hash(&active_higher_ability_ids)?;
    let membership_signature_hash = membership_signature_hash(&source_membership_keys)?;
    let code_state_key = "code-state-demo".to_string();
    let named_failure_mode = "api_contract_mismatch".to_string();
    let replay_cell_id = ability_aware_replay_cell_id(
        "python",
        "live_patch",
        &code_state_key,
        &named_failure_mode,
        &label_signature_hash,
        Some(&skill_signature_hash),
        Some(&ability_signature_hash),
        Some(&membership_signature_hash),
    )?;
    let mistake_id = mistake_id_from_evidence_parts(
        prediction_id,
        &code_state_key,
        &label_signature_hash,
        Some(&skill_signature_hash),
        Some(&ability_signature_hash),
        Some(&membership_signature_hash),
        Verdict::Fail,
    )?;
    let now_unix_ms = 1_779_280_000_000_i64;
    let replay_row_key = hex::encode(prediction_id.0);
    let mistake_row = MistakeLogRow {
        schema_version: 1,
        mistake_id: mistake_id.clone(),
        prediction_id,
        predicted_verdict: Verdict::Pass,
        ground_truth_verdict: Verdict::Fail,
        truth_source: MistakeTruthSource::ShiftLogReplay,
        code_state_key: code_state_key.clone(),
        named_failure_mode: Some(named_failure_mode),
        accepted_label_ids: accepted_label_ids.clone(),
        active_skill_ids: active_skill_ids.clone(),
        label_signature_hash: label_signature_hash.clone(),
        skill_signature_hash: Some(skill_signature_hash.clone()),
        failure_evidence_set_ids: vec!["failure-evidence:demo".to_string()],
        replay_row_key,
        accepted_registry_sha256: Some("a".repeat(64)),
        usefulness_metrics_sha256: Some("b".repeat(64)),
        learning_bridge_manifest_sha256: Some("c".repeat(64)),
        created_at_unix_ms: now_unix_ms,
        active_higher_ability_ids: active_higher_ability_ids.clone(),
        source_membership_keys: source_membership_keys.clone(),
        ability_signature_hash: Some(ability_signature_hash.clone()),
        membership_signature_hash: Some(membership_signature_hash.clone()),
    };
    let replay_row = ReplayBufferRow {
        prediction_id,
        surprise_z: 1.0,
        cell_id: replay_cell_id.clone(),
        coverage_gap_score: 0.25,
        last_replayed_ts: None,
        replay_count: 0,
        retention_weight: 1.0,
        protected: true,
        retention_tier: ReplayRetentionTier::Hot,
        source: ReplayBufferSource::ConstellationSkillMistake,
        created_at_unix_ms: now_unix_ms,
        updated_at_unix_ms: now_unix_ms,
        accepted_label_ids,
        active_skill_ids: active_skill_ids.clone(),
        active_higher_ability_ids: active_higher_ability_ids.clone(),
        source_membership_keys: source_membership_keys.clone(),
        label_signature_hash: Some(label_signature_hash.clone()),
        skill_signature_hash: Some(skill_signature_hash.clone()),
        ability_signature_hash: Some(ability_signature_hash.clone()),
        membership_signature_hash: Some(membership_signature_hash.clone()),
    };
    let lifecycle_audit = SkillLifecycleAuditRow {
        schema_version: 1,
        skill_audit_id: lifecycle_audit_id_from_parts(
            Some(&active_skill_ids[0]),
            SkillLifecycleDecision::UpdateExistingSkill,
            now_unix_ms,
        )?,
        prediction_id: Some(hex::encode(prediction_id.0)),
        mistake_id: Some(mistake_id),
        previous_skill_id: Some(active_skill_ids[0].clone()),
        decision: SkillLifecycleDecision::UpdateExistingSkill,
        candidate_skill_id: None,
        evidence_label_ids: mistake_row.accepted_label_ids.clone(),
        evidence_chunk_ids: vec!["chunk-17".to_string()],
        reason: "mcp_fsv_refuted_prediction_updates_existing_skill".to_string(),
        created_at_unix_ms: now_unix_ms,
        evidence_skill_ids: active_skill_ids,
        evidence_higher_ability_ids: active_higher_ability_ids,
        source_membership_keys,
    };
    let repeat_metric_key = repeat_metric_key(
        &replay_cell_id,
        &label_signature_hash,
        Some(&skill_signature_hash),
        Some(&ability_signature_hash),
        Some(&membership_signature_hash),
    )?;
    Ok(SeedRows {
        panel_signature_hash: "panel:demo-112b".to_string(),
        mistake_row,
        replay_row,
        lifecycle_audit,
        repeat_metric_key,
        now_unix_ms,
    })
}

fn definitions_case() -> Result<Value> {
    let names = get_tool_definitions()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<std::collections::BTreeSet<_>>();
    Ok(json!({
        "pass": names.contains(tool_names::MEJEPA_RECORD_MISTAKE)
            && names.contains(tool_names::MEJEPA_MISTAKE_HISTORY)
            && names.contains(tool_names::MEJEPA_MISTAKE_LOOP_STATUS),
        "toolCount": names.len()
    }))
}

fn independent_readback(db_path: &Path, seed: &SeedRows) -> Result<Value> {
    let db = open_ability_resolver_rocksdb(db_path, false)?;
    let mistakes = read_all_mistake_log_rows(&db)?;
    let replay_rows = read_all_replay_rows(&db)?;
    let lifecycle_audits = read_all_skill_lifecycle_audit_rows(&db)?;
    let head_key = context_graph_mejepa_train::online_head_key(&seed.panel_signature_hash)?;
    let head_state = read_online_head_state_row(&db, &head_key)?;
    let repeat_metric = read_repeat_metric_row(&db, &seed.repeat_metric_key)?;
    let joined_evidence = joined_evidence_readback(
        seed,
        &mistakes,
        &replay_rows,
        head_state.as_ref(),
        repeat_metric.as_ref(),
        &lifecycle_audits,
    );
    let pass = mistakes.len() == 1
        && replay_rows.len() == 1
        && head_state
            .as_ref()
            .map(|row| row.corrected_verdict == Verdict::Fail)
            .unwrap_or(false)
        && repeat_metric
            .as_ref()
            .map(|row| row.mistake_repeat_rate == 0.0)
            .unwrap_or(false)
        && count_cf_rows(&db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)? == 1
        && joined_evidence["pass"].as_bool() == Some(true);
    Ok(json!({
        "pass": pass,
        "mistakeRows": mistakes.len(),
        "replayRows": replay_rows.len(),
        "onlineHeadState": head_state,
        "repeatMetric": repeat_metric,
        "joinedEvidence": joined_evidence,
        "cfCounts": {
            CF_MEJEPA_MISTAKE_LOG: count_cf_rows(&db, CF_MEJEPA_MISTAKE_LOG)?,
            CF_MEJEPA_REPLAY_BUFFER: count_cf_rows(&db, CF_MEJEPA_REPLAY_BUFFER)?,
            CF_MEJEPA_ONLINE_HEAD_STATE: count_cf_rows(&db, CF_MEJEPA_ONLINE_HEAD_STATE)?,
            CF_MEJEPA_SKILL_LIFECYCLE_AUDIT: count_cf_rows(&db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)?
        }
    }))
}

fn joined_evidence_readback(
    seed: &SeedRows,
    mistakes: &[MistakeLogRow],
    replay_rows: &[ReplayBufferRow],
    head_state: Option<&OnlineHeadStateRow>,
    repeat_metric: Option<&OnlineHeadRepeatMetricRow>,
    lifecycle_audits: &[SkillLifecycleAuditRow],
) -> Value {
    let mistake = mistakes
        .iter()
        .find(|row| row.mistake_id == seed.mistake_row.mistake_id);
    let replay = replay_rows
        .iter()
        .find(|row| row.prediction_id == seed.replay_row.prediction_id);
    let lifecycle = lifecycle_audits
        .iter()
        .find(|row| row.mistake_id.as_deref() == Some(seed.mistake_row.mistake_id.as_str()));
    let mistake_replay_match = mistake
        .zip(replay)
        .map(|(mistake, replay)| {
            mistake.accepted_label_ids == replay.accepted_label_ids
                && mistake.active_skill_ids == replay.active_skill_ids
                && mistake.active_higher_ability_ids == replay.active_higher_ability_ids
                && mistake.source_membership_keys == replay.source_membership_keys
                && Some(mistake.label_signature_hash.clone()) == replay.label_signature_hash
                && mistake.skill_signature_hash == replay.skill_signature_hash
                && mistake.ability_signature_hash == replay.ability_signature_hash
                && mistake.membership_signature_hash == replay.membership_signature_hash
                && replay.cell_id == seed.replay_row.cell_id
        })
        .unwrap_or(false);
    let lifecycle_match = lifecycle
        .map(|row| {
            row.evidence_label_ids == seed.mistake_row.accepted_label_ids
                && row.evidence_skill_ids == seed.mistake_row.active_skill_ids
                && row.evidence_higher_ability_ids == seed.mistake_row.active_higher_ability_ids
                && row.source_membership_keys == seed.mistake_row.source_membership_keys
        })
        .unwrap_or(false);
    let online_head_match = head_state
        .map(|row| {
            row.replay_cell_id == seed.replay_row.cell_id
                && row.label_signature_hash == seed.mistake_row.label_signature_hash
                && row.skill_signature_hash == seed.mistake_row.skill_signature_hash
                && row.ability_signature_hash == seed.mistake_row.ability_signature_hash
                && row.membership_signature_hash == seed.mistake_row.membership_signature_hash
                && row.last_mistake_id == seed.mistake_row.mistake_id
                && row.last_replay_row_key == seed.mistake_row.replay_row_key
        })
        .unwrap_or(false);
    let repeat_metric_match = repeat_metric
        .map(|row| {
            row.replay_cell_id == seed.replay_row.cell_id
                && row.label_signature_hash == seed.mistake_row.label_signature_hash
                && row.skill_signature_hash == seed.mistake_row.skill_signature_hash
                && row.ability_signature_hash == seed.mistake_row.ability_signature_hash
                && row.membership_signature_hash == seed.mistake_row.membership_signature_hash
        })
        .unwrap_or(false);
    json!({
        "pass": mistake_replay_match && lifecycle_match && online_head_match && repeat_metric_match,
        "mistakeReplayMatch": mistake_replay_match,
        "lifecycleMatch": lifecycle_match,
        "onlineHeadMatch": online_head_match,
        "repeatMetricMatch": repeat_metric_match
    })
}

fn independent_counts(db_path: &Path) -> Result<Value> {
    let db = open_ability_resolver_rocksdb(db_path, false)?;
    Ok(json!({
        CF_MEJEPA_MISTAKE_LOG: count_cf_rows(&db, CF_MEJEPA_MISTAKE_LOG)?,
        CF_MEJEPA_REPLAY_BUFFER: count_cf_rows(&db, CF_MEJEPA_REPLAY_BUFFER)?,
        CF_MEJEPA_ONLINE_HEAD_STATE: count_cf_rows(&db, CF_MEJEPA_ONLINE_HEAD_STATE)?,
        CF_MEJEPA_SKILL_LIFECYCLE_AUDIT: count_cf_rows(&db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)?
    }))
}

fn missing_cf_case(root: &Path, panel_signature_hash: &str) -> Result<Value> {
    let path = root.join("missing-cfs.db");
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let descriptors = vec![
        ColumnFamilyDescriptor::new("default", Options::default()),
        ColumnFamilyDescriptor::new(CF_MEJEPA_MISTAKE_LOG, Options::default()),
    ];
    drop(DB::open_cf_descriptors(&opts, &path, descriptors)?);
    tool_error_case(
        root,
        tool_names::MEJEPA_MISTAKE_LOOP_STATUS,
        json!({
            "dbPath": path,
            "panelSignatureHash": panel_signature_hash
        }),
        "MEJEPA_MISTAKE_LOOP_STATUS_FAILED",
    )
}

fn target_label_case(root: &Path, db_path: &Path, seed: &SeedRows) -> Result<Value> {
    let mut bad_mistake = seed.mistake_row.clone();
    bad_mistake.mistake_id = "mistake:target-label-rejected".to_string();
    bad_mistake.accepted_label_ids = vec!["oracle:fail".to_string()];
    bad_mistake.label_signature_hash =
        accepted_label_signature_hash(&bad_mistake.accepted_label_ids)?;
    let mut bad_replay = seed.replay_row.clone();
    bad_replay.accepted_label_ids = bad_mistake.accepted_label_ids.clone();
    bad_replay.label_signature_hash = Some(bad_mistake.label_signature_hash.clone());
    tool_error_case(
        root,
        tool_names::MEJEPA_RECORD_MISTAKE,
        json!({
            "dbPath": db_path,
            "panelSignatureHash": seed.panel_signature_hash,
            "mistakeRow": bad_mistake,
            "replayRow": bad_replay,
            "baseVerdictBeforeUpdate": "pass",
            "nowUnixMs": seed.now_unix_ms + 1
        }),
        "MEJEPA_RECORD_MISTAKE_FAILED",
    )
}

fn record_invalid_panel_no_partial_write_case(
    root: &Path,
    db_path: &Path,
    seed: &SeedRows,
) -> Result<Value> {
    let before = independent_counts(db_path)?;
    let prediction_id = PredictionId([0x43; 16]);
    let mut bad_mistake = seed.mistake_row.clone();
    bad_mistake.prediction_id = prediction_id;
    bad_mistake.replay_row_key = hex::encode(prediction_id.0);
    bad_mistake.mistake_id = mistake_id_from_evidence_parts(
        prediction_id,
        &bad_mistake.code_state_key,
        &bad_mistake.label_signature_hash,
        bad_mistake.skill_signature_hash.as_deref(),
        bad_mistake.ability_signature_hash.as_deref(),
        bad_mistake.membership_signature_hash.as_deref(),
        bad_mistake.ground_truth_verdict,
    )?;
    let mut bad_replay = seed.replay_row.clone();
    bad_replay.prediction_id = prediction_id;
    let error = tool_error_case(
        root,
        tool_names::MEJEPA_RECORD_MISTAKE,
        json!({
            "dbPath": db_path,
            "panelSignatureHash": "bad\npanel",
            "mistakeRow": bad_mistake,
            "replayRow": bad_replay,
            "baseVerdictBeforeUpdate": "pass",
            "nowUnixMs": seed.now_unix_ms + 2
        }),
        "MEJEPA_RECORD_MISTAKE_FAILED",
    )?;
    let after = independent_counts(db_path)?;
    Ok(json!({
        "pass": error["pass"].as_bool() == Some(true) && before == after,
        "error": error,
        "countsBefore": before,
        "countsAfter": after
    }))
}

fn call_tool(root: &Path, tool_name: &str, arguments: Value) -> Result<Value> {
    let runtime = tokio::runtime::Runtime::new()?;
    let handlers = build_protocol_handlers(root)?;
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(112)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": tool_name,
            "arguments": arguments
        })),
    };
    let response = runtime.block_on(handlers.dispatch(request));
    ensure!(
        response.error.is_none(),
        "tools/call returned JSON-RPC error: {:?}",
        response.error
    );
    let result = response.result.context("tools/call missing result")?;
    ensure!(
        result.get("isError").and_then(Value::as_bool) == Some(false),
        "tools/call returned MCP isError for {tool_name}: {result}"
    );
    result
        .get("structuredContent")
        .cloned()
        .context("tools/call missing structuredContent")
}

fn tool_error_case(
    root: &Path,
    tool_name: &str,
    arguments: Value,
    expected: &str,
) -> Result<Value> {
    let runtime = tokio::runtime::Runtime::new()?;
    let handlers = build_protocol_handlers(root)?;
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(113)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": tool_name,
            "arguments": arguments
        })),
    };
    let response = runtime.block_on(handlers.dispatch(request));
    let result = response.result.context("error case missing result")?;
    let is_error = result.get("isError").and_then(Value::as_bool) == Some(true);
    let text = result.to_string();
    Ok(json!({
        "pass": is_error && text.contains(expected),
        "expected": expected,
        "result": result
    }))
}

fn build_protocol_handlers(root: &Path) -> Result<Handlers> {
    let db_path = root.join(format!(
        "protocol-handler-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)?;
    let db_arc = rocksdb_store.db_arc();
    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);
    let provider: Arc<dyn MultiArrayEmbeddingProvider> = Arc::new(FailClosedEmbeddingProvider);
    let edge_repository = EdgeRepository::new(db_arc);
    let graph_builder = Arc::new(BackgroundGraphBuilder::new(
        edge_repository.clone(),
        Arc::clone(&teleological_store),
        Arc::clone(&provider),
        GraphBuilderConfig::default(),
    ));
    let models_loading = Arc::new(AtomicBool::new(false));
    let models_failed = Arc::new(RwLock::new(Some(
        "FSV harness uses FailClosedEmbeddingProvider; no graph embeddings are loaded.".to_string(),
    )));
    let layer_status_provider = Arc::new(LiveLayerStatusProvider::new(
        models_loading,
        models_failed,
        Arc::clone(&teleological_store),
    ));
    Handlers::without_llm(
        teleological_store,
        provider,
        layer_status_provider,
        edge_repository,
        graph_builder,
        Arc::new(context_graph_embeddings::provider::NoOpCausalHintProvider),
    )
    .context("build protocol handlers")
}

struct FailClosedEmbeddingProvider;

#[async_trait]
impl MultiArrayEmbeddingProvider for FailClosedEmbeddingProvider {
    async fn embed_all(&self, _content: &str) -> CoreResult<MultiArrayEmbeddingOutput> {
        Err(CoreError::Internal(
            "mistake-loop MCP FSV must not cross into graph embedding paths".to_string(),
        ))
    }

    async fn embed_batch_all(
        &self,
        _contents: &[String],
        _metadata: &[EmbeddingMetadata],
    ) -> CoreResult<Vec<MultiArrayEmbeddingOutput>> {
        Err(CoreError::Internal(
            "mistake-loop MCP FSV must not batch graph embeddings".to_string(),
        ))
    }

    fn model_ids(&self) -> [&str; NUM_EMBEDDERS] {
        ["mistake-loop-fsv-fail-closed"; NUM_EMBEDDERS]
    }

    fn is_ready(&self) -> bool {
        false
    }

    fn health_status(&self) -> [bool; NUM_EMBEDDERS] {
        [false; NUM_EMBEDDERS]
    }
}
