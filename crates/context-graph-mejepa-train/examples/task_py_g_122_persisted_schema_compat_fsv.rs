use context_graph_mejepa::{
    decode_reality_prediction, AgentClaimGraph, ChunkId, ClaimReconciliation, ConformalInterval,
    ConformalSet, ExemplarMatch, FlakyTestCandidate, GuardViolation, Language,
    MatchedFingerprintEvidence, MejepaStore, OracleOutcome, PerSlotOodReason,
    PhaseBPredictionSurfaces, PredictedAccuracyDegradation, PredictedCostRegression,
    PredictedDeadCode, PredictedFailureMode, PredictedLatentBug, PredictedPerfRegression,
    PredictedRedundancy, PredictedSecurityConcern, PredictedTechDebt, PredictedTestOutcome,
    PredictedWorks, PredictionId, PredictionLabelContext, PredictionProvenance, RealityImpact,
    RealityPrediction, RealityPredictionBuilder, ReasoningClass, RocksDbInferStore, TaskId,
    UncoveredPath, Verdict, WitnessHash,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_MISTAKE_LOG, CF_MEJEPA_REPLAY_BUFFER,
    CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
};
use context_graph_mejepa_train::ability_resolver::open_ability_resolver_rocksdb;
use context_graph_mejepa_train::chunk_skill_membership::{
    read_all_skill_lifecycle_audit_rows, SkillLifecycleDecision,
};
use context_graph_mejepa_train::label_bridge::accepted_label_signature_hash;
use context_graph_mejepa_train::mistake_log::{
    mistake_id_from_parts, read_all_mistake_log_rows, MistakeTruthSource,
};
use context_graph_mejepa_train::replay_buffer::{
    label_aware_replay_cell_id, read_all_replay_rows, ReplayBufferSource, ReplayRetentionTier,
};
use rocksdb::{ColumnFamilyDescriptor, Options, WriteOptions, DB};
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const NOW: i64 = 1_779_360_000_000;

#[derive(Debug, Clone, Serialize)]
struct LegacyRealityPredictionNoLabelContext {
    prediction_id: [u8; 16],
    witness_hash: WitnessHash,
    task_id: TaskId,
    session_id: [u8; 16],
    language: Language,
    covered_chunks: Vec<ChunkId>,
    verdict: Verdict,
    confidence_interval: ConformalInterval,
    predicted_oracle_pass: f32,
    predicted_test_pass: Vec<f32>,
    predicted_runtime_trace: [f32; 32],
    ood_score: f32,
    outcome_set: ConformalSet,
    calibrated_confidence: f32,
    degraded_status: bool,
    granger_attestations: BTreeMap<String, f32>,
    predicted_failure_modes: Vec<PredictedFailureMode>,
    predicted_failed_tests: Vec<PredictedTestOutcome>,
    predicted_works: Vec<PredictedWorks>,
    predicted_uncovered_paths: Vec<UncoveredPath>,
    predicted_flaky_tests: Vec<FlakyTestCandidate>,
    guard_violations: Vec<GuardViolation>,
    per_slot_ood_reasons: Vec<PerSlotOodReason>,
    closest_exemplars: Vec<ExemplarMatch>,
    predicted_edge_cases: Vec<context_graph_mejepa::PredictedEdgeCase>,
    predicted_latent_bugs: Vec<PredictedLatentBug>,
    predicted_tech_debt_added: Vec<PredictedTechDebt>,
    predicted_dead_code: Vec<PredictedDeadCode>,
    predicted_redundant_code: Vec<PredictedRedundancy>,
    predicted_perf_regressions: Vec<PredictedPerfRegression>,
    predicted_security_concerns: Vec<PredictedSecurityConcern>,
    predicted_accuracy_degradations: Vec<PredictedAccuracyDegradation>,
    predicted_cost_regressions: Vec<PredictedCostRegression>,
    predicted_reasoning_class: ReasoningClass,
    agent_claim_graph: AgentClaimGraph,
    claim_reconciliation: Vec<ClaimReconciliation>,
    reality_impact: Option<RealityImpact>,
    provenance: PredictionProvenance,
    source_panel_sha: [u8; 32],
    calibration_version: String,
    created_at_unix_ms: i64,
    matched_fingerprint: Option<MatchedFingerprintEvidence>,
    unknown_candidate_id: Option<[u8; 16]>,
    constellation_intelligence: Option<context_graph_mejepa::ConstellationIntelligenceEvidence>,
    slot_attributions: Vec<context_graph_mejepa::SlotAttributionEvidence>,
}

impl From<&RealityPrediction> for LegacyRealityPredictionNoLabelContext {
    fn from(value: &RealityPrediction) -> Self {
        Self {
            prediction_id: value.prediction_id,
            witness_hash: value.witness_hash,
            task_id: value.task_id.clone(),
            session_id: value.session_id,
            language: value.language,
            covered_chunks: value.covered_chunks.clone(),
            verdict: value.verdict,
            confidence_interval: value.confidence_interval,
            predicted_oracle_pass: value.predicted_oracle_pass,
            predicted_test_pass: value.predicted_test_pass.clone(),
            predicted_runtime_trace: value.predicted_runtime_trace,
            ood_score: value.ood_score,
            outcome_set: value.outcome_set.clone(),
            calibrated_confidence: value.calibrated_confidence,
            degraded_status: value.degraded_status,
            granger_attestations: value.granger_attestations.clone(),
            predicted_failure_modes: value.predicted_failure_modes.clone(),
            predicted_failed_tests: value.predicted_failed_tests.clone(),
            predicted_works: value.predicted_works.clone(),
            predicted_uncovered_paths: value.predicted_uncovered_paths.clone(),
            predicted_flaky_tests: value.predicted_flaky_tests.clone(),
            guard_violations: value.guard_violations.clone(),
            per_slot_ood_reasons: value.per_slot_ood_reasons.clone(),
            closest_exemplars: value.closest_exemplars.clone(),
            predicted_edge_cases: value.predicted_edge_cases.clone(),
            predicted_latent_bugs: value.predicted_latent_bugs.clone(),
            predicted_tech_debt_added: value.predicted_tech_debt_added.clone(),
            predicted_dead_code: value.predicted_dead_code.clone(),
            predicted_redundant_code: value.predicted_redundant_code.clone(),
            predicted_perf_regressions: value.predicted_perf_regressions.clone(),
            predicted_security_concerns: value.predicted_security_concerns.clone(),
            predicted_accuracy_degradations: value.predicted_accuracy_degradations.clone(),
            predicted_cost_regressions: value.predicted_cost_regressions.clone(),
            predicted_reasoning_class: value.predicted_reasoning_class,
            agent_claim_graph: value.agent_claim_graph.clone(),
            claim_reconciliation: value.claim_reconciliation.clone(),
            reality_impact: value.reality_impact.clone(),
            provenance: value.provenance.clone(),
            source_panel_sha: value.source_panel_sha,
            calibration_version: value.calibration_version.clone(),
            created_at_unix_ms: value.created_at_unix_ms,
            matched_fingerprint: value.matched_fingerprint.clone(),
            unknown_candidate_id: value.unknown_candidate_id,
            constellation_intelligence: value.constellation_intelligence.clone(),
            slot_attributions: value.slot_attributions.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct LegacyPredictionLabelContextV1 {
    schema_version: u32,
    accepted_label_ids: Vec<String>,
    code_state_key: Option<String>,
    failure_evidence_set_ids: Vec<String>,
    active_skill_ids: Vec<String>,
    accepted_registry_sha256: Option<String>,
    usefulness_metrics_sha256: Option<String>,
    learning_bridge_manifest_sha256: Option<String>,
    label_signature_hash: Option<String>,
    skill_signature_hash: Option<String>,
}

impl From<&PredictionLabelContext> for LegacyPredictionLabelContextV1 {
    fn from(value: &PredictionLabelContext) -> Self {
        Self {
            schema_version: value.schema_version,
            accepted_label_ids: value.accepted_label_ids.clone(),
            code_state_key: value.code_state_key.clone(),
            failure_evidence_set_ids: value.failure_evidence_set_ids.clone(),
            active_skill_ids: value.active_skill_ids.clone(),
            accepted_registry_sha256: value.accepted_registry_sha256.clone(),
            usefulness_metrics_sha256: value.usefulness_metrics_sha256.clone(),
            learning_bridge_manifest_sha256: value.learning_bridge_manifest_sha256.clone(),
            label_signature_hash: value.label_signature_hash.clone(),
            skill_signature_hash: value.skill_signature_hash.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyMistakeLogRow {
    schema_version: u32,
    mistake_id: String,
    prediction_id: PredictionId,
    predicted_verdict: Verdict,
    ground_truth_verdict: Verdict,
    truth_source: MistakeTruthSource,
    code_state_key: String,
    named_failure_mode: Option<String>,
    accepted_label_ids: Vec<String>,
    active_skill_ids: Vec<String>,
    label_signature_hash: String,
    skill_signature_hash: Option<String>,
    failure_evidence_set_ids: Vec<String>,
    replay_row_key: String,
    accepted_registry_sha256: Option<String>,
    usefulness_metrics_sha256: Option<String>,
    learning_bridge_manifest_sha256: Option<String>,
    created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyReplayBufferRow {
    prediction_id: PredictionId,
    surprise_z: f32,
    cell_id: String,
    coverage_gap_score: f32,
    last_replayed_ts: Option<i64>,
    replay_count: u64,
    retention_weight: f32,
    protected: bool,
    retention_tier: ReplayRetentionTier,
    source: ReplayBufferSource,
    created_at_unix_ms: i64,
    updated_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacySkillLifecycleAuditRow {
    schema_version: u32,
    skill_audit_id: String,
    prediction_id: Option<String>,
    mistake_id: Option<String>,
    previous_skill_id: Option<String>,
    decision: SkillLifecycleDecision,
    candidate_skill_id: Option<String>,
    evidence_label_ids: Vec<String>,
    evidence_chunk_ids: Vec<String>,
    reason: String,
    created_at_unix_ms: i64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fsv_root = parse_fsv_root();
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&fsv_root)?;
    let before = tree_state(&fsv_root)?;

    let live_db_path = fsv_root.join("legacy-live-predictions.rocksdb");
    let ability_db_path = fsv_root.join("legacy-ability-rows.rocksdb");
    let prediction_current = prediction(0x42, NOW)?;
    let prediction_legacy_label = prediction(0x43, NOW + 1)?;
    let session_id = prediction_current.session_id;

    let live_db = open_live_db(&live_db_path)?;
    write_legacy_live_prediction_no_label_context(live_db.as_ref(), &prediction_current)?;
    write_legacy_live_prediction_with_legacy_label_context(
        live_db.as_ref(),
        &prediction_legacy_label,
    )?;
    let live_store = RocksDbInferStore::new(live_db.clone());
    let live_rows = live_store.read_live_predictions(session_id, 10)?;
    let decoded_direct = decode_reality_prediction(&bincode::serialize(
        &LegacyRealityPredictionNoLabelContext::from(&prediction_current),
    )?)?;
    drop(live_store);
    drop(live_db);

    let ability_db = open_ability_resolver_rocksdb(&ability_db_path, true)?;
    write_legacy_ability_rows(&ability_db, &prediction_current)?;
    ability_db.flush()?;
    drop(ability_db);

    let ability_readback = open_ability_resolver_rocksdb(&ability_db_path, false)?;
    let mistake_rows = read_all_mistake_log_rows(&ability_readback)?;
    let replay_rows = read_all_replay_rows(&ability_readback)?;
    let lifecycle_rows = read_all_skill_lifecycle_audit_rows(&ability_readback)?;
    drop(ability_readback);

    let live_current_defaulted = live_rows.iter().any(|row| {
        row.prediction_id == prediction_current.prediction_id
            && row.label_context == PredictionLabelContext::default()
    });
    let live_legacy_label_defaulted = live_rows.iter().any(|row| {
        row.prediction_id == prediction_legacy_label.prediction_id
            && row.label_context.accepted_label_ids
                == prediction_legacy_label.label_context.accepted_label_ids
            && row.label_context.active_skill_ids
                == prediction_legacy_label.label_context.active_skill_ids
            && row.label_context.active_higher_ability_ids.is_empty()
            && row.label_context.source_membership_keys.is_empty()
            && row.label_context.ability_signature_hash.is_none()
            && row.label_context.membership_signature_hash.is_none()
    });
    let ability_rows_defaulted = mistake_rows.len() == 1
        && replay_rows.len() == 1
        && lifecycle_rows.len() == 1
        && mistake_rows[0].active_higher_ability_ids.is_empty()
        && mistake_rows[0].source_membership_keys.is_empty()
        && mistake_rows[0].ability_signature_hash.is_none()
        && mistake_rows[0].membership_signature_hash.is_none()
        && replay_rows[0].accepted_label_ids.is_empty()
        && replay_rows[0].active_higher_ability_ids.is_empty()
        && replay_rows[0].source_membership_keys.is_empty()
        && lifecycle_rows[0].evidence_skill_ids.is_empty()
        && lifecycle_rows[0].evidence_higher_ability_ids.is_empty()
        && lifecycle_rows[0].source_membership_keys.is_empty();
    let corrupt_old_live_row_fails_closed = {
        let mut bytes = bincode::serialize(&LegacyRealityPredictionNoLabelContext::from(
            &prediction_current,
        ))?;
        bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        decode_reality_prediction(&bytes).is_err()
    };
    let direct_decode_defaulted = decoded_direct.label_context == PredictionLabelContext::default();
    let mntd_rejected =
        open_ability_resolver_rocksdb(Path::new("/mnt/d/contextgraph/bad"), true).is_err();

    let all_passed = live_rows.len() == 2
        && live_current_defaulted
        && live_legacy_label_defaulted
        && ability_rows_defaulted
        && direct_decode_defaulted
        && corrupt_old_live_row_fails_closed
        && mntd_rejected;

    let report_path = fsv_root.join("task_py_g_122_persisted_schema_compat_fsv.json");
    let report = json!({
        "artifact_kind": "task_py_g_122_persisted_schema_compat_fsv",
        "schema_version": 1,
        "task_id": "TASK-PY-G-122",
        "covered_github_issues": [432],
        "passes": all_passed,
        "before": before,
        "after": tree_state(&fsv_root)?,
        "source_of_truth": {
            "live_db_path": live_db_path,
            "ability_db_path": ability_db_path,
            "live_prediction_cf": CF_MEJEPA_LIVE_PREDICTIONS,
            "mistake_log_cf": CF_MEJEPA_MISTAKE_LOG,
            "replay_buffer_cf": CF_MEJEPA_REPLAY_BUFFER,
            "skill_lifecycle_audit_cf": CF_MEJEPA_SKILL_LIFECYCLE_AUDIT
        },
        "checks": {
            "live_prediction_legacy_no_label_context_defaults": live_current_defaulted,
            "live_prediction_legacy_v1_label_context_defaults_new_ability_fields": live_legacy_label_defaulted,
            "direct_decode_legacy_live_row_defaults_label_context": direct_decode_defaulted,
            "mistake_replay_lifecycle_legacy_rows_default_new_fields": ability_rows_defaulted,
            "corrupt_old_live_row_fails_closed": corrupt_old_live_row_fails_closed,
            "legacy_mnt_d_path_rejected": mntd_rejected
        },
        "readback": {
            "live_prediction_rows": live_rows,
            "mistake_rows": mistake_rows,
            "replay_rows": replay_rows,
            "lifecycle_rows": lifecycle_rows
        },
        "active_path_policy": {"mnt_d_active_reads_or_writes": false}
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if readback != report || !all_passed {
        return Err(format!("TASK-PY-G-122 FSV failed at {}", report_path.display()).into());
    }
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn prediction(
    byte: u8,
    created_at_unix_ms: i64,
) -> Result<RealityPrediction, Box<dyn std::error::Error>> {
    let label_context = PredictionLabelContext {
        accepted_label_ids: vec!["ast_surface:function".to_string()],
        code_state_key: Some("state:legacy".to_string()),
        active_skill_ids: vec!["skill:legacy".to_string()],
        label_signature_hash: Some("labels:73505a3043fe".to_string()),
        skill_signature_hash: Some("skills:legacy".to_string()),
        ..PredictionLabelContext::default()
    };
    Ok(RealityPredictionBuilder::from_parts(
        TaskId(format!("schema-compat-{byte}")),
        [0x11; 16],
        Language::Python,
        ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2)?,
    )
    .prediction_id([byte; 16])
    .witness_hash(WitnessHash([0x22; 32]))
    .verdict(Verdict::Pass)
    .confidence_interval(ConformalInterval {
        lower: 0.7,
        upper: 0.9,
        ..ConformalInterval::default()
    })
    .predicted_oracle_pass(0.8)
    .predicted_test_pass(vec![0.8])
    .ood_score(0.1)
    .calibrated_confidence(0.82)
    .granger_attestations(BTreeMap::from([("compat:unit".to_string(), 0.7)]))
    .provenance(PredictionProvenance {
        predictor_version: "schema-compat-fsv".to_string(),
        constellation_version: "schema-compat-fsv".to_string(),
        calibration_version: "schema-compat-fsv".to_string(),
        active_pointer: "schema-compat-fsv".to_string(),
    })
    .source_panel_sha([0x33; 32])
    .calibration_version("schema-compat-fsv")
    .created_at_unix_ms(created_at_unix_ms)
    .label_context(label_context)
    .phase_b_surfaces(PhaseBPredictionSurfaces::default())
    .build()?)
}

fn write_legacy_live_prediction_no_label_context(
    db: &DB,
    prediction: &RealityPrediction,
) -> Result<(), Box<dyn std::error::Error>> {
    put_live_bytes(
        db,
        prediction,
        &bincode::serialize(&LegacyRealityPredictionNoLabelContext::from(prediction))?,
    )
}

fn write_legacy_live_prediction_with_legacy_label_context(
    db: &DB,
    prediction: &RealityPrediction,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = bincode::serialize(&LegacyRealityPredictionNoLabelContext::from(prediction))?;
    bytes.extend(bincode::serialize(&LegacyPredictionLabelContextV1::from(
        &prediction.label_context,
    ))?);
    put_live_bytes(db, prediction, &bytes)
}

fn put_live_bytes(
    db: &DB,
    prediction: &RealityPrediction,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
        .ok_or("missing live predictions cf")?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, live_key(prediction), bytes, &opts)?;
    db.flush_cf(cf)?;
    Ok(())
}

fn write_legacy_ability_rows(
    db: &DB,
    prediction: &RealityPrediction,
) -> Result<(), Box<dyn std::error::Error>> {
    let prediction_id = PredictionId(prediction.prediction_id);
    let accepted_label_ids = vec!["ast_surface:function".to_string()];
    let label_signature_hash = accepted_label_signature_hash(&accepted_label_ids)?;
    let mistake_id = mistake_id_from_parts(
        prediction_id,
        "state:legacy",
        &label_signature_hash,
        None,
        Verdict::Fail,
    )?;
    let replay_row_key = hex::encode([0x55; 16]);
    let mistake = LegacyMistakeLogRow {
        schema_version: 1,
        mistake_id: mistake_id.clone(),
        prediction_id,
        predicted_verdict: Verdict::Pass,
        ground_truth_verdict: Verdict::Fail,
        truth_source: MistakeTruthSource::ShiftLogReplay,
        code_state_key: "state:legacy".to_string(),
        named_failure_mode: Some("failure:legacy".to_string()),
        accepted_label_ids,
        active_skill_ids: Vec::new(),
        label_signature_hash: label_signature_hash.clone(),
        skill_signature_hash: None,
        failure_evidence_set_ids: Vec::new(),
        replay_row_key: replay_row_key.clone(),
        accepted_registry_sha256: None,
        usefulness_metrics_sha256: None,
        learning_bridge_manifest_sha256: None,
        created_at_unix_ms: NOW,
    };
    let replay = LegacyReplayBufferRow {
        prediction_id,
        surprise_z: 1.25,
        cell_id: label_aware_replay_cell_id(
            "python",
            "legacy_cell",
            "state:legacy",
            "failure:legacy",
            &label_signature_hash,
            None,
        )?,
        coverage_gap_score: 0.25,
        last_replayed_ts: None,
        replay_count: 0,
        retention_weight: 1.0,
        protected: true,
        retention_tier: ReplayRetentionTier::Hot,
        source: ReplayBufferSource::AgentSurprise,
        created_at_unix_ms: NOW,
        updated_at_unix_ms: NOW,
    };
    let lifecycle = LegacySkillLifecycleAuditRow {
        schema_version: 1,
        skill_audit_id: "skill_audit:legacy".to_string(),
        prediction_id: Some(hex::encode(prediction.prediction_id)),
        mistake_id: Some(mistake_id),
        previous_skill_id: Some("skill:legacy".to_string()),
        decision: SkillLifecycleDecision::UpdateExistingSkill,
        candidate_skill_id: None,
        evidence_label_ids: vec!["ast_surface:function".to_string()],
        evidence_chunk_ids: vec!["chunk:legacy".to_string()],
        reason: "legacy_schema_compat".to_string(),
        created_at_unix_ms: NOW,
    };
    put_cf_bytes(
        db,
        CF_MEJEPA_MISTAKE_LOG,
        b"mistake:legacy",
        &bincode::serialize(&mistake)?,
    )?;
    put_cf_bytes(
        db,
        CF_MEJEPA_REPLAY_BUFFER,
        replay_row_key.as_bytes(),
        &bincode::serialize(&replay)?,
    )?;
    put_cf_bytes(
        db,
        CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
        b"skill_audit:legacy",
        &bincode::serialize(&lifecycle)?,
    )?;
    Ok(())
}

fn put_cf_bytes(
    db: &DB,
    cf_name: &str,
    key: &[u8],
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let cf = db.cf_handle(cf_name).ok_or("missing cf")?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, bytes, &opts)?;
    db.flush_cf(cf)?;
    Ok(())
}

fn open_live_db(path: &Path) -> Result<Arc<DB>, Box<dyn std::error::Error>> {
    if path.display().to_string().starts_with("/mnt/d/") {
        return Err("live prediction db path must not use /mnt/d".into());
    }
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let db = DB::open_cf_descriptors(
        &opts,
        path,
        vec![ColumnFamilyDescriptor::new(
            CF_MEJEPA_LIVE_PREDICTIONS,
            Options::default(),
        )],
    )?;
    Ok(Arc::new(db))
}

fn live_key(prediction: &RealityPrediction) -> Vec<u8> {
    let mut key = Vec::with_capacity(40);
    key.extend_from_slice(&prediction.session_id);
    key.extend_from_slice(&prediction.created_at_unix_ms.to_be_bytes());
    key.extend_from_slice(&prediction.prediction_id);
    key
}

fn parse_fsv_root() -> PathBuf {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--fsv-root" {
            if let Some(value) = args.next() {
                return PathBuf::from(value);
            }
        }
    }
    PathBuf::from("/tmp/contextgraph-task-py-g-122-schema-compat-fsv")
}

fn tree_state(path: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(json!({"exists": false}));
    }
    let mut files = 0_u64;
    let mut bytes = 0_u64;
    visit(path, &mut files, &mut bytes)?;
    Ok(json!({"exists": true, "files": files, "bytes": bytes}))
}

fn visit(path: &Path, files: &mut u64, bytes: &mut u64) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.is_dir() {
            visit(&entry.path(), files, bytes)?;
        } else if meta.is_file() {
            *files += 1;
            *bytes += meta.len();
        }
    }
    Ok(())
}
