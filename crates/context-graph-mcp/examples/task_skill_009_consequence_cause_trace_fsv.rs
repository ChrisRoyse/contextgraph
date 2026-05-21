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
use context_graph_mejepa::eval::MutationCategory;
use context_graph_mejepa::{
    open_infer_rocksdb, AgentClaimGraph, ChunkId, ConformalInterval, ConformalMethod, ConformalSet,
    ConstellationIntelligenceEvidence, EdgeCaseClass, EmbedderId, ExemplarMatch, FailureModeClass,
    GuardViolation, Language, MejepaStore, OracleOutcome, PerSlotOodReason, PerSlotOodReasonKind,
    PredictedEdgeCase, PredictedFailureMode, PredictionLabelContext, PredictionProvenance,
    RealityPrediction, ReasoningClass, RocksDbInferStore, RootCauseClass, Severity,
    SlotAttributionEvidence, SlotAttributionPolarity, SlotAttributionSource, TaskId, TestDeltaKind,
    TestId, TestOutcome, Verdict, WitnessHash,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_MISTAKE_LOG, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
};
use context_graph_mejepa_instruments::OracleVerdict;
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use context_graph_storage::{BackgroundGraphBuilder, EdgeRepository, GraphBuilderConfig};
use rocksdb::IteratorMode;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::RwLock;

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-skill-009-consequence-cause-trace-fsv";
const REPORT_NAME: &str = "task_skill_009_consequence_cause_trace_fsv.json";
const CORPUS_CHUNKS_JSONL: &str =
    "/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl";
const LABEL_ROOT: &str =
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v2-live-split";
const TASK_560_REPORT: &str = "/zfs/archive/contextgraph/fsv/task-skill-008-split-legal-ece-repair-fsv/run-20260521T004021Z/task_py_g_392_temporal_trajectory_features_fsv.json";
const PASS_CHUNK_ID: &str =
    "sha256:01fe16fa3163dc4c73b7ac30c10d8019c37e12336547c1437a782362d2462ff6";
const FAIL_CHUNK_ID: &str =
    "sha256:1539cdcd9524da96a42f47633734e4149e55e8d25fa3c877260287f1acb0d0df";

fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT).join(format!(
        "run-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;

    let db_path = root.join("consequence-cause-trace.db");
    let source_jsonl = extract_corpus_source_index(&root, &[PASS_CHUNK_ID, FAIL_CHUNK_ID])?;
    let fail_prediction = fail_prediction()?;
    let pass_risk_prediction = pass_with_risk_prediction()?;
    let ood_prediction = ood_prediction()?;
    persist_predictions(
        &db_path,
        [&fail_prediction, &pass_risk_prediction, &ood_prediction],
    )?;

    let fail_trace = call_tool(
        &root,
        tool_names::MEJEPA_CONSEQUENCE_TRACE,
        json!({
            "dbPath": db_path,
            "predictionId": hex::encode(fail_prediction.prediction_id),
            "chunkSourceJsonl": source_jsonl,
            "requireSourceBytes": true
        }),
    )?;
    let pass_risk_trace = call_tool(
        &root,
        tool_names::MEJEPA_CONSEQUENCE_TRACE,
        json!({
            "dbPath": db_path,
            "predictionId": hex::encode(pass_risk_prediction.prediction_id),
            "chunkSourceJsonl": source_jsonl,
            "requireSourceBytes": true
        }),
    )?;
    let ood_trace = call_tool(
        &root,
        tool_names::MEJEPA_CONSEQUENCE_TRACE,
        json!({
            "dbPath": db_path,
            "predictionId": hex::encode(ood_prediction.prediction_id),
            "chunkSourceJsonl": source_jsonl,
            "requireSourceBytes": true
        }),
    )?;
    let reverse_chunk = call_tool(
        &root,
        tool_names::MEJEPA_EVIDENCE_TO_CONSEQUENCES,
        json!({
            "dbPath": db_path,
            "chunkId": FAIL_CHUNK_ID,
            "chunkSourceJsonl": source_jsonl,
            "requireSourceBytes": true,
            "limit": 16
        }),
    )?;
    let reverse_skill = call_tool(
        &root,
        tool_names::MEJEPA_EVIDENCE_TO_CONSEQUENCES,
        json!({
            "dbPath": db_path,
            "skillId": "skill:syntax_overlap_compile_break",
            "chunkSourceJsonl": source_jsonl,
            "requireSourceBytes": true,
            "limit": 16
        }),
    )?;
    let missing_source_case = call_tool_error(
        &root,
        tool_names::MEJEPA_CONSEQUENCE_TRACE,
        json!({
            "dbPath": db_path,
            "predictionId": hex::encode(fail_prediction.prediction_id),
            "requireSourceBytes": true
        }),
        "requireSourceBytes=true requires chunkSourceJsonl",
    )?;
    let calibration_blocker = task_560_blocker_trace()?;
    let readback = independent_readback(&db_path)?;
    let definitions = tool_definition_case();

    let pass = definitions["pass"].as_bool() == Some(true)
        && trace_has_kind(&fail_trace, "q2_verdict")
        && trace_has_source_bytes(&fail_trace, FAIL_CHUNK_ID)
        && trace_has_skill(&fail_trace, "skill:syntax_overlap_compile_break")
        && trace_has_kind(&pass_risk_trace, "q4_pass_with_edge_risk")
        && trace_has_source_bytes(&pass_risk_trace, PASS_CHUNK_ID)
        && trace_has_kind(&ood_trace, "q2_verdict")
        && ood_trace
            .pointer("/items/0/predictionContext/activeLearning/unknownCandidateId")
            .and_then(Value::as_str)
            .is_some()
        && reverse_chunk["matchCount"].as_u64().unwrap_or_default() >= 1
        && reverse_skill["matchCount"].as_u64().unwrap_or_default() >= 1
        && missing_source_case["pass"].as_bool() == Some(true)
        && calibration_blocker["pass"].as_bool() == Some(true)
        && readback["predictionRows"].as_u64() == Some(3);

    let report_path = root.join(REPORT_NAME);
    let report = json!({
        "schemaVersion": 1,
        "artifactKind": "task_skill_009_consequence_cause_trace_fsv",
        "issue": 561,
        "passes": pass,
        "fsvRoot": root,
        "sourceOfTruth": {
            "inferDbPath": db_path,
            "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS,
            "mistakeCf": CF_MEJEPA_MISTAKE_LOG,
            "skillLifecycleAuditCf": CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
            "corpusChunksJsonl": CORPUS_CHUNKS_JSONL,
            "labelRoot": LABEL_ROOT,
            "task560Report": TASK_560_REPORT,
            "reportPath": report_path
        },
        "definitionCase": definitions,
        "failTrace": fail_trace,
        "passWithRiskTrace": pass_risk_trace,
        "oodTrace": ood_trace,
        "reverseLookups": {
            "chunk": reverse_chunk,
            "skill": reverse_skill
        },
        "edgeCases": {
            "requireSourceBytesMissingSourceIndex": missing_source_case
        },
        "task560CalibrationBlockerTrace": calibration_blocker,
        "independentReadback": readback,
        "activeMntDReadsOrWrites": false,
        "innerLlmInvoked": false,
        "flatVectorConcatUsed": false
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let readback_report: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(
        readback_report["passes"].as_bool() == Some(pass)
            && readback_report["issue"].as_u64() == Some(561),
        "report readback mismatch"
    );
    ensure!(
        pass,
        "TASK-SKILL-009 FSV failed at {}",
        report_path.display()
    );
    println!("{}", report_path.display());
    Ok(())
}

fn fail_prediction() -> Result<RealityPrediction> {
    let chunk = ChunkId(FAIL_CHUNK_ID.to_string());
    let mut prediction = base_prediction([0x91; 16], chunk.clone(), Verdict::Fail, 0.03)?;
    prediction.label_context = label_context(
        "code_state:astropy__astropy-12907:mutation_compile_error",
        &[
            "slot:e2:family:temporal_coordinate",
            "source_site_relation:exact_patch_line_overlap",
        ],
        &["skill:syntax_overlap_compile_break"],
        &["ability:compile_break_localization"],
    );
    prediction.predicted_failure_modes = vec![PredictedFailureMode {
        failure_class: FailureModeClass::SyntaxError,
        chunk: chunk.clone(),
        line_range: (190, 205),
        confidence: 0.96,
        severity: Severity::Critical,
        explanation:
            "syntax-overlap compile break: source-site chunk carries oracle fail/error evidence"
                .to_string(),
        contributing_embedders: vec![EmbedderId("E2".to_string()), EmbedderId("E4".to_string())],
        root_cause_class: RootCauseClass::Other,
    }];
    prediction.guard_violations = vec![GuardViolation {
        embedder: EmbedderId("E4".to_string()),
        chunk: chunk.clone(),
        centroid_id: "centroid:compile_error:source_site_overlap".to_string(),
        cosine: 0.61,
        threshold_tau_m: 0.82,
        deficit: 0.21,
    }];
    prediction.predicted_failed_tests = vec![context_graph_mejepa::PredictedTestOutcome {
        test_id: TestId("swebench:astropy__astropy-12907:pass_to_pass".to_string()),
        current_outcome: TestOutcome::Pass,
        predicted_outcome: TestOutcome::Error,
        delta_kind: TestDeltaKind::PassToFail,
        confidence: 0.94,
        why: prediction.predicted_failure_modes[0].clone(),
    }];
    prediction.slot_attributions = vec![
        slot_attr(&chunk, SlotAttributionSource::FailureMode, "e4", 0.96),
        slot_attr(
            &chunk,
            SlotAttributionSource::ConstellationSkill,
            "e2/e4",
            0.91,
        ),
    ];
    prediction.constellation_intelligence = Some(
        ConstellationIntelligenceEvidence::neutral("task-skill-009-fsv")?
            .with_pattern_id("constellation:syntax_overlap_compile_break".to_string())?,
    );
    prediction.closest_exemplars = vec![exemplar(false)];
    RealityPrediction::try_new(prediction).context("validate fail prediction")
}

fn pass_with_risk_prediction() -> Result<RealityPrediction> {
    let chunk = ChunkId(PASS_CHUNK_ID.to_string());
    let mut prediction = base_prediction([0x92; 16], chunk.clone(), Verdict::Pass, 0.91)?;
    prediction.label_context = label_context(
        "code_state:astropy__astropy-12907:base",
        &[
            "slot:e2:family:temporal_coordinate",
            "ast_surface:return_value_surface",
        ],
        &["skill:stable_background_import_and_impl_scaffold"],
        &["ability:stable_scaffold_recognition"],
    );
    prediction.predicted_edge_cases = vec![PredictedEdgeCase {
        edge_class: EdgeCaseClass::BoundaryValue,
        chunk: chunk.clone(),
        line_range: (146, 147),
        triggering_input_description: "empty child list reaches return-value surface".to_string(),
        covered_by_test: false,
        confidence: 0.73,
    }];
    prediction.slot_attributions = vec![slot_attr(
        &chunk,
        SlotAttributionSource::ConstellationSkill,
        "e2/e3/e4",
        0.74,
    )];
    prediction.constellation_intelligence = Some(
        ConstellationIntelligenceEvidence::neutral("task-skill-009-fsv")?.with_pattern_id(
            "constellation:stable_background_import_and_impl_scaffold".to_string(),
        )?,
    );
    prediction.closest_exemplars = vec![exemplar(true)];
    RealityPrediction::try_new(prediction).context("validate pass-with-risk prediction")
}

fn ood_prediction() -> Result<RealityPrediction> {
    let chunk = ChunkId(PASS_CHUNK_ID.to_string());
    let mut prediction =
        base_prediction([0x93; 16], chunk.clone(), Verdict::OutOfDistribution, 0.42)?;
    prediction.label_context = label_context(
        "code_state:astropy__astropy-12907:unknown_ood",
        &["slot:e7:ood_margin_high"],
        &[],
        &[],
    );
    prediction.ood_score = 0.91;
    prediction.calibrated_confidence = 0.22;
    prediction.unknown_candidate_id = Some([0x44; 16]);
    prediction.per_slot_ood_reasons = vec![PerSlotOodReason {
        embedder: EmbedderId("E7".to_string()),
        chunk: Some(chunk.clone()),
        reason: PerSlotOodReasonKind::GtauViolation,
        observed_score: 0.58,
        threshold: 0.81,
        margin: 0.23,
        calibration_version: "task-skill-009-fsv-calibration".to_string(),
        evidence: "OOD branch carries candidate id and source-byte chunk but no known skill"
            .to_string(),
    }];
    prediction.slot_attributions = vec![slot_attr(
        &chunk,
        SlotAttributionSource::PerSlotOod,
        "e7",
        0.91,
    )];
    RealityPrediction::try_new(prediction).context("validate OOD prediction")
}

fn base_prediction(
    prediction_id: [u8; 16],
    chunk: ChunkId,
    verdict: Verdict,
    predicted_oracle_pass: f32,
) -> Result<RealityPrediction> {
    RealityPrediction::try_new(RealityPrediction {
        prediction_id,
        witness_hash: WitnessHash(id32(&format!("witness:{}", hex::encode(prediction_id)))),
        task_id: TaskId("task-skill-009-consequence-trace-fsv".to_string()),
        session_id: [0x21; 16],
        language: Language::Python,
        covered_chunks: vec![chunk],
        verdict,
        confidence_interval: ConformalInterval {
            lower: 0.10,
            upper: 0.90,
            method: ConformalMethod::SplitConformal,
            coverage_target: 0.90,
            empirical_coverage: 0.89,
        },
        predicted_oracle_pass,
        predicted_test_pass: vec![predicted_oracle_pass],
        predicted_runtime_trace: [0.0; 32],
        ood_score: 0.15,
        outcome_set: ConformalSet::try_new(vec![oracle_outcome_for_verdict(verdict)], 0.10, 0.20)?,
        calibrated_confidence: if predicted_oracle_pass >= 0.5 {
            predicted_oracle_pass
        } else {
            1.0 - predicted_oracle_pass
        },
        degraded_status: false,
        granger_attestations: BTreeMap::from([
            ("baseline:category_local_knn".to_string(), 0.61),
            ("calibration_bucket:evidence".to_string(), 0.78),
        ]),
        predicted_failure_modes: Vec::new(),
        predicted_failed_tests: Vec::new(),
        predicted_works: Vec::new(),
        predicted_uncovered_paths: Vec::new(),
        predicted_flaky_tests: Vec::new(),
        guard_violations: Vec::new(),
        per_slot_ood_reasons: Vec::new(),
        closest_exemplars: Vec::new(),
        predicted_edge_cases: Vec::new(),
        predicted_latent_bugs: Vec::new(),
        predicted_tech_debt_added: Vec::new(),
        predicted_dead_code: Vec::new(),
        predicted_redundant_code: Vec::new(),
        predicted_perf_regressions: Vec::new(),
        predicted_security_concerns: Vec::new(),
        predicted_accuracy_degradations: Vec::new(),
        predicted_cost_regressions: Vec::new(),
        predicted_reasoning_class: ReasoningClass::MostlyCorrect,
        agent_claim_graph: AgentClaimGraph::default(),
        claim_reconciliation: Vec::new(),
        reality_impact: None,
        provenance: PredictionProvenance {
            predictor_version: "task-skill-009-fsv-predictor".to_string(),
            constellation_version: "task-skill-009-fsv-constellation".to_string(),
            calibration_version: "task-skill-009-fsv-calibration".to_string(),
            active_pointer: hex::encode(prediction_id),
        },
        source_panel_sha: id32(&format!("panel:{}", hex::encode(prediction_id))),
        calibration_version: "task-skill-009-fsv-calibration".to_string(),
        created_at_unix_ms: 1_779_312_000_000,
        matched_fingerprint: None,
        unknown_candidate_id: None,
        constellation_intelligence: None,
        slot_attributions: Vec::new(),
        label_context: PredictionLabelContext::default(),
    })
    .context("validate base prediction")
}

fn label_context(
    code_state_key: &str,
    labels: &[&str],
    skills: &[&str],
    abilities: &[&str],
) -> PredictionLabelContext {
    let accepted_label_ids = labels.iter().map(|value| value.to_string()).collect();
    let active_skill_ids = skills.iter().map(|value| value.to_string()).collect();
    let active_higher_ability_ids = abilities.iter().map(|value| value.to_string()).collect();
    PredictionLabelContext {
        schema_version: context_graph_mejepa::PREDICTION_LABEL_CONTEXT_SCHEMA_VERSION,
        accepted_label_ids,
        code_state_key: Some(code_state_key.to_string()),
        failure_evidence_set_ids: vec!["failure_evidence:task-skill-009".to_string()],
        active_skill_ids,
        accepted_registry_sha256: Some("sha256:task-skill-009-accepted-registry".to_string()),
        usefulness_metrics_sha256: Some("sha256:task-skill-009-usefulness".to_string()),
        learning_bridge_manifest_sha256: Some("sha256:task-skill-009-learning-bridge".to_string()),
        label_signature_hash: Some("label-signature:task-skill-009".to_string()),
        skill_signature_hash: Some("skill-signature:task-skill-009".to_string()),
        active_higher_ability_ids,
        source_membership_keys: vec![format!("membership:{code_state_key}:task-skill-009")],
        ability_signature_hash: Some("ability-signature:task-skill-009".to_string()),
        membership_signature_hash: Some("membership-signature:task-skill-009".to_string()),
    }
}

fn slot_attr(
    chunk: &ChunkId,
    source: SlotAttributionSource,
    slot_id: &str,
    score: f32,
) -> SlotAttributionEvidence {
    SlotAttributionEvidence {
        schema_version: context_graph_mejepa::SLOT_ATTRIBUTION_SCHEMA_VERSION,
        slot_id: slot_id.to_string(),
        embedder: Some(EmbedderId(slot_id.to_string())),
        chunk: Some(chunk.clone()),
        polarity: SlotAttributionPolarity::Violating,
        source,
        score,
        threshold: Some(0.80),
        margin: Some((score - 0.80).max(0.0)),
        reason: "task-skill-009 consequence cause trace attribution".to_string(),
        relationship_slot_id: None,
        related_fingerprint_id: None,
        active_learning_candidate_id: None,
        q_head: Some("q2_or_q4_consequence".to_string()),
        impact_kind: None,
        evidence: "slot-preserving trace evidence; no flat vector concat".to_string(),
    }
}

fn exemplar(pass: bool) -> ExemplarMatch {
    ExemplarMatch {
        exemplar_prediction_id: Some(if pass { [0x51; 16] } else { [0x52; 16] }),
        task_id: TaskId("astropy__astropy-12907".to_string()),
        mutation_kind: if pass {
            MutationCategory::KnownGood
        } else {
            MutationCategory::CompileError
        },
        similarity_score: 0.87,
        diff_summary: if pass {
            "known-good source row from 300x8 corpus".to_string()
        } else {
            "compile-error source-site overlap from 300x8 corpus".to_string()
        },
        oracle_outcome: OracleVerdict {
            per_test: Vec::new(),
            exception: None,
            evidence_unavailable: false,
        },
        failure_mode_class: if pass {
            None
        } else {
            Some(FailureModeClass::SyntaxError)
        },
        evidence_path: Some(LABEL_ROOT.to_string()),
        witness_hash: WitnessHash(if pass { [0x61; 32] } else { [0x62; 32] }),
    }
}

fn oracle_outcome_for_verdict(verdict: Verdict) -> OracleOutcome {
    match verdict {
        Verdict::Pass => OracleOutcome::Pass,
        Verdict::Fail | Verdict::GuardRejected => OracleOutcome::Fail,
        Verdict::OutOfDistribution => OracleOutcome::OutOfDistribution,
        Verdict::Abstain => OracleOutcome::Abstain,
    }
}

fn persist_predictions<'a>(
    db_path: &Path,
    predictions: impl IntoIterator<Item = &'a RealityPrediction>,
) -> Result<()> {
    let db = open_infer_rocksdb(db_path)?;
    let store = RocksDbInferStore::new(db);
    for prediction in predictions {
        MejepaStore::write_live_prediction(&store, prediction)?;
    }
    Ok(())
}

fn extract_corpus_source_index(root: &Path, chunk_ids: &[&str]) -> Result<PathBuf> {
    let corpus = Path::new(CORPUS_CHUNKS_JSONL);
    let file = File::open(corpus).with_context(|| format!("open {}", corpus.display()))?;
    let target = chunk_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut found = BTreeSet::new();
    let output = root.join("corpus_source_index.jsonl");
    let mut writer = File::create(&output)?;
    for line in BufReader::new(file).lines() {
        let line = line?;
        let value: Value = serde_json::from_str(&line)?;
        let Some(chunk_id) = value.get("chunk_id").and_then(Value::as_str) else {
            continue;
        };
        if !target.contains(chunk_id) {
            continue;
        }
        let source_text = value
            .get("source_text")
            .and_then(Value::as_str)
            .context("corpus source row missing source_text")?;
        let row = json!({
            "chunk_id": chunk_id,
            "relative_path": value.get("relative_path").and_then(Value::as_str).unwrap_or("unknown.py"),
            "byte_span": [
                value.get("byte_start").and_then(Value::as_u64).unwrap_or(0),
                value.get("byte_end").and_then(Value::as_u64).unwrap_or(0)
            ],
            "source_text": source_text,
            "source_text_sha256": format!("sha256:{}", hex::encode(Sha256::digest(source_text.as_bytes()))),
            "row_key": value.get("subject").and_then(Value::as_str)
        });
        writeln!(writer, "{}", serde_json::to_string(&row)?)?;
        found.insert(chunk_id.to_string());
        if found.len() == target.len() {
            break;
        }
    }
    ensure!(
        found.len() == target.len(),
        "source index found {} of {} requested chunks",
        found.len(),
        target.len()
    );
    Ok(output)
}

fn call_tool(root: &Path, tool_name: &str, arguments: Value) -> Result<Value> {
    let runtime = tokio::runtime::Runtime::new()?;
    let handlers = build_protocol_handlers(root)?;
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(561)),
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

fn call_tool_error(
    root: &Path,
    tool_name: &str,
    arguments: Value,
    expected: &str,
) -> Result<Value> {
    let runtime = tokio::runtime::Runtime::new()?;
    let handlers = build_protocol_handlers(root)?;
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(562)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": tool_name,
            "arguments": arguments
        })),
    };
    let response = runtime.block_on(handlers.dispatch(request));
    let result = response.result.context("error case missing result")?;
    let pass = result.get("isError").and_then(Value::as_bool) == Some(true)
        && result.to_string().contains(expected);
    Ok(json!({
        "pass": pass,
        "expected": expected,
        "result": result
    }))
}

fn tool_definition_case() -> Value {
    let definitions = get_tool_definitions();
    let trace = definitions
        .iter()
        .find(|tool| tool.name == tool_names::MEJEPA_CONSEQUENCE_TRACE);
    let reverse = definitions
        .iter()
        .find(|tool| tool.name == tool_names::MEJEPA_EVIDENCE_TO_CONSEQUENCES);
    let pass = trace
        .and_then(|tool| tool.input_schema.pointer("/properties/chunkSourceJsonl"))
        .is_some()
        && trace
            .and_then(|tool| tool.input_schema.pointer("/properties/requireSourceBytes"))
            .is_some()
        && reverse
            .and_then(|tool| tool.input_schema.pointer("/properties/chunkSourceJsonl"))
            .is_some();
    json!({
        "pass": pass,
        "traceToolPresent": trace.is_some(),
        "reverseToolPresent": reverse.is_some()
    })
}

fn task_560_blocker_trace() -> Result<Value> {
    let path = Path::new(TASK_560_REPORT);
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let sha256 = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    let report: Value = serde_json::from_slice(&bytes)?;
    let blockers = report
        .pointer("/legal_ece_focused_mean_bias_calibration/blockers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let ece = report
        .pointer("/legal_ece_focused_mean_bias_calibration/holdout/metrics/ece")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let corr = report
        .pointer(
            "/legal_ece_focused_mean_bias_calibration/holdout/metrics/prediction_oracle_correlation",
        )
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let blocker_text = blockers
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(",");
    Ok(json!({
        "pass": blocker_text.contains("aggregate_ece_regressed") && corr > 0.86 && ece > 0.03,
        "path": path,
        "sha256": sha256,
        "causeTrace": {
            "consequence": "TASK_SKILL_008_ECE_BLOCKED",
            "whyBad": "targeted skill trajectory consequence signal exists, but aggregate ECE remains above the promotion baseline",
            "blockers": blockers,
            "holdoutPredictionOracleCorrelation": corr,
            "holdoutEce": ece,
            "nextEvidenceNeeded": "per-cell or per-constellation stable calibration, not another aggregate blanket skill expansion"
        }
    }))
}

fn independent_readback(db_path: &Path) -> Result<Value> {
    let db = open_infer_rocksdb(db_path)?;
    let cf = db
        .cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
        .context("CF_MEJEPA_LIVE_PREDICTIONS missing")?;
    let rows = db.iterator_cf(&cf, IteratorMode::Start).count() as u64;
    Ok(json!({
        "predictionRows": rows,
        "dbPath": db_path,
        "cf": CF_MEJEPA_LIVE_PREDICTIONS,
        "readbackVerified": rows == 3
    }))
}

fn trace_has_kind(trace: &Value, kind: &str) -> bool {
    trace["items"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["kind"] == kind))
}

fn trace_has_source_bytes(trace: &Value, chunk_id: &str) -> bool {
    trace["items"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item.pointer("/directEvidence/sourceStatus")
                .and_then(Value::as_str)
                == Some("source_bytes_verified")
                && item["directEvidence"]["sourceRows"]
                    .as_array()
                    .is_some_and(|rows| {
                        rows.iter().any(|row| {
                            row["chunkId"].as_str() == Some(chunk_id)
                                && row["sourceText"]
                                    .as_str()
                                    .is_some_and(|text| !text.is_empty())
                        })
                    })
        })
    })
}

fn trace_has_skill(trace: &Value, skill_id: &str) -> bool {
    trace["items"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item["directEvidence"]["activeSkillIds"]
                .as_array()
                .is_some_and(|skills| skills.iter().any(|value| value.as_str() == Some(skill_id)))
        })
    })
}

fn id32(input: &str) -> [u8; 32] {
    let digest = Sha256::digest(input.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
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
        "TASK-SKILL-009 FSV uses persisted ME-JEPA rows only; no graph embeddings are loaded."
            .to_string(),
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
            "TASK-SKILL-009 MCP FSV must not cross into graph embedding paths".to_string(),
        ))
    }

    async fn embed_batch_all(
        &self,
        _contents: &[String],
        _metadata: &[EmbeddingMetadata],
    ) -> CoreResult<Vec<MultiArrayEmbeddingOutput>> {
        Err(CoreError::Internal(
            "TASK-SKILL-009 MCP FSV must not batch graph embeddings".to_string(),
        ))
    }

    fn model_ids(&self) -> [&str; NUM_EMBEDDERS] {
        ["task-skill-009-fsv-fail-closed"; NUM_EMBEDDERS]
    }

    fn is_ready(&self) -> bool {
        false
    }

    fn health_status(&self) -> [bool; NUM_EMBEDDERS] {
        [false; NUM_EMBEDDERS]
    }
}
