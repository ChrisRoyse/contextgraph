use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::time::Duration;

use anyhow::{anyhow, bail, ensure, Context, Result};
use context_graph_mejepa::calibration::count_cf;
use context_graph_mejepa::{
    build_fixture_deterministic_compiler, open_infer_rocksdb, valid_witness_segment, ActualOutcome,
    AgentId, AstDiff, CalibrationExample, CalibrationStore, DiffHunk, FeedbackId, FeedbackKind,
    Language, MeJepaInferConfig, MejepaStore, OracleOutcome, PatchBundle, PredictionId,
    RealityPrediction, RocksDbInferStore, SurpriseEvent, SurpriseSeverity, TaskContext,
    TaskEnvironment, TaskId, TestId, TrainCertSummary, VerifyVerdict, WitnessHash,
    MEJEPA_INFER_CFS,
};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const TASK_ID: &str = "TASK-TEST-003";
const AGENT_COUNT: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MultiAgentFsvReport {
    pub fsv_root: String,
    pub task_id: String,
    pub build_release_sha: String,
    pub source_of_truth: MultiAgentSourceOfTruth,
    pub happy_path: Vec<StepEvidence>,
    pub boundary_cases: Vec<BoundaryEvidence>,
    pub cf_counts_before: BTreeMap<String, u64>,
    pub cf_counts_after: BTreeMap<String, u64>,
    pub physical_artifacts: Vec<FileArtifact>,
    pub readback_equal: bool,
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MultiAgentSourceOfTruth {
    pub infer_db_path: String,
    pub repo_root: String,
    pub live_prediction_cf: String,
    pub feedback_cf: String,
    pub report_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StepEvidence {
    pub step: String,
    pub trigger: String,
    pub source_of_truth: String,
    pub before: Value,
    pub after: Value,
    pub expected: Value,
    pub actual: Value,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BoundaryEvidence {
    pub case: String,
    pub trigger: String,
    pub expected_error_code: String,
    pub actual_error_code: String,
    pub before: Value,
    pub after: Value,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FileArtifact {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone)]
struct AgentCase {
    agent_id: AgentId,
    session_id: [u8; 16],
    session_hex: String,
    task_id: String,
    patch: PatchBundle,
    context: TaskContext,
    feedback_ts_millis: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct PredictionArtifact {
    agent_id: String,
    session_hex: String,
    task_id: String,
    prediction_id_hex: String,
    created_at_unix_ms: i64,
    verdict: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct FeedbackArtifact {
    agent_id: String,
    session_hex: String,
    prediction_id_hex: String,
    feedback_id_hex: String,
    feedback_key_hex: String,
    ts_millis: i64,
}

pub fn default_fsv_root() -> PathBuf {
    PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-multi-agent-fsv")
}

pub fn default_report_path(fsv_root: &Path) -> PathBuf {
    fsv_root.join("multi_agent_fsv.json")
}

pub fn run_multi_agent_fsv(fsv_root: &Path) -> Result<MultiAgentFsvReport> {
    remove_existing_fsv_root(fsv_root)?;
    fs::create_dir_all(fsv_root)?;
    let repo_root = fsv_root.join("repo");
    let db_path = fsv_root.join("infer-rocksdb");
    fs::create_dir_all(&repo_root)?;
    let cases = build_agent_cases(&repo_root)?;
    let report_path = default_report_path(fsv_root);
    let build_release_sha = git_head_short();

    let cf_counts_before = {
        let db = open_infer_rocksdb(&db_path)?;
        seed_infer_db(db.clone())?;
        count_relevant_cfs(db.as_ref())?
    };

    let (happy_path, boundary_cases, cf_counts_after) = {
        let db = open_infer_rocksdb(&db_path)?;
        let store = Arc::new(RocksDbInferStore::new(db.clone()));
        let calibration = CalibrationStore::new(db.clone(), 30)?;
        let mut happy_path = Vec::new();
        let mut boundary_cases = Vec::new();

        let predictions_before = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
        )?;
        let prediction_artifacts = run_concurrent_predictions(
            &repo_root,
            db.clone(),
            store.clone(),
            calibration.clone(),
            cases.clone(),
        )?;
        let predictions_after = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
        )?;
        let per_session_counts = per_session_prediction_counts(store.as_ref(), &cases)?;
        let prediction_isolation = prediction_artifacts.len() == AGENT_COUNT
            && predictions_after == predictions_before + AGENT_COUNT as u64
            && per_session_counts.values().all(|count| *count == 1)
            && distinct_prediction_count(&prediction_artifacts) == AGENT_COUNT;
        happy_path.push(StepEvidence {
            step: "concurrent_predictions_isolated".to_string(),
            trigger: "three std::thread workers call MeJepaCompiler::verify on shared RocksDB"
                .to_string(),
            source_of_truth: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            before: json!({"rows": predictions_before}),
            after: json!({
                "rows": predictions_after,
                "perSessionCounts": per_session_counts,
                "predictions": prediction_artifacts
            }),
            expected: json!({
                "rows": predictions_before + AGENT_COUNT as u64,
                "perSessionRows": 1,
                "distinctPredictions": AGENT_COUNT
            }),
            actual: json!({
                "rows": predictions_after,
                "perSessionCounts": per_session_counts,
                "distinctPredictions": distinct_prediction_count(&prediction_artifacts)
            }),
            pass: prediction_isolation,
        });

        let feedback_before = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
        )?;
        let feedback_artifacts = run_concurrent_feedback_writes(db.clone(), &cases)?;
        let feedback_after = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
        )?;
        let feedback_isolation = feedback_artifacts.len() == AGENT_COUNT
            && feedback_after == feedback_before + AGENT_COUNT as u64
            && feedback_session_map(db.as_ref())? == expected_feedback_session_map(&cases);
        happy_path.push(StepEvidence {
            step: "concurrent_feedback_isolated".to_string(),
            trigger: "three std::thread workers persist SurpriseEvent rows keyed by prediction+agent+timestamp".to_string(),
            source_of_truth: context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK.to_string(),
            before: json!({"rows": feedback_before}),
            after: json!({
                "rows": feedback_after,
                "feedback": feedback_artifacts,
                "feedbackSessionMap": feedback_session_map(db.as_ref())?
            }),
            expected: json!({
                "rows": feedback_before + AGENT_COUNT as u64,
                "feedbackRowsPerAgent": 1
            }),
            actual: json!({
                "rows": feedback_after,
                "feedbackSessionMap": feedback_session_map(db.as_ref())?
            }),
            pass: feedback_isolation,
        });

        boundary_cases.push(boundary_zero_session_context_rejected(
            db.as_ref(),
            &repo_root,
        )?);
        boundary_cases.push(boundary_unknown_prediction_feedback_rejected(
            db.as_ref(),
            &cases[0],
        )?);
        boundary_cases.push(boundary_duplicate_feedback_key_rejected(
            db.as_ref(),
            &cases[0],
        )?);
        flush_infer_db(db.as_ref())?;
        (happy_path, boundary_cases, count_relevant_cfs(db.as_ref())?)
    };

    let reopened_counts = {
        let db = open_infer_rocksdb(&db_path)?;
        count_relevant_cfs(db.as_ref())?
    };
    let readback_equal = reopened_counts == cf_counts_after;
    let mut happy_path = happy_path;
    happy_path.push(StepEvidence {
        step: "independent_reopen_readback".to_string(),
        trigger: "drop handles, reopen RocksDB, recount live predictions and feedback rows"
            .to_string(),
        source_of_truth: format!(
            "{} + {}",
            context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
            context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK
        ),
        before: json!({"cfCounts": cf_counts_after}),
        after: json!({"cfCounts": reopened_counts}),
        expected: json!({"countsStableAfterReopen": true}),
        actual: json!({"countsStableAfterReopen": readback_equal}),
        pass: readback_equal,
    });

    let all_passed =
        happy_path.iter().all(|step| step.pass) && boundary_cases.iter().all(|case| case.pass);
    ensure!(all_passed, "multi-agent FSV contains failed evidence");
    ensure!(readback_equal, "multi-agent FSV reopen counts changed");

    Ok(MultiAgentFsvReport {
        fsv_root: fsv_root.display().to_string(),
        task_id: TASK_ID.to_string(),
        build_release_sha,
        source_of_truth: MultiAgentSourceOfTruth {
            infer_db_path: db_path.display().to_string(),
            repo_root: repo_root.display().to_string(),
            live_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            feedback_cf: context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK.to_string(),
            report_path: report_path.display().to_string(),
        },
        happy_path,
        boundary_cases,
        cf_counts_before,
        cf_counts_after: reopened_counts,
        physical_artifacts: collect_file_artifacts(fsv_root)?,
        readback_equal,
        all_passed,
    })
}

pub fn write_report_with_readback(report: &MultiAgentFsvReport, path: &Path) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    write_bytes_readback(path, &bytes)?;
    let readback: MultiAgentFsvReport = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(readback == *report, "multi-agent FSV readback mismatch");
    Ok(())
}

fn run_concurrent_predictions(
    repo_root: &Path,
    db: Arc<DB>,
    store: Arc<RocksDbInferStore>,
    calibration: CalibrationStore,
    cases: Vec<AgentCase>,
) -> Result<Vec<PredictionArtifact>> {
    let barrier = Arc::new(Barrier::new(cases.len()));
    let mut handles = Vec::new();
    for case in cases {
        let barrier = barrier.clone();
        let repo_root = repo_root.to_path_buf();
        let db = db.clone();
        let store = store.clone();
        let calibration = calibration.clone();
        handles.push(std::thread::spawn(move || -> Result<PredictionArtifact> {
            let compiler = build_fixture_deterministic_compiler(
                repo_root,
                Arc::new(RocksDbInferStore::new(db)),
                calibration,
                MeJepaInferConfig {
                    outcome_set_max: 4,
                    ..MeJepaInferConfig::default()
                },
            )?;
            barrier.wait();
            let prediction =
                verify_approved(&compiler, store.as_ref(), &case.patch, &case.context)?;
            Ok(PredictionArtifact {
                agent_id: case.agent_id.0,
                session_hex: case.session_hex,
                task_id: case.task_id,
                prediction_id_hex: hex::encode(prediction.prediction_id),
                created_at_unix_ms: prediction.created_at_unix_ms,
                verdict: format!("{:?}", prediction.verdict),
            })
        }));
    }
    let mut out = Vec::new();
    for handle in handles {
        out.push(
            handle
                .join()
                .map_err(|_| anyhow!("MEJEPA_MULTI_AGENT_PREDICTION_THREAD_PANIC"))??,
        );
    }
    out.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
    Ok(out)
}

fn run_concurrent_feedback_writes(
    db: Arc<DB>,
    cases: &[AgentCase],
) -> Result<Vec<FeedbackArtifact>> {
    let barrier = Arc::new(Barrier::new(cases.len()));
    let mut handles = Vec::new();
    for case in cases.iter().cloned() {
        let barrier = barrier.clone();
        let db = db.clone();
        handles.push(std::thread::spawn(move || -> Result<FeedbackArtifact> {
            let store = RocksDbInferStore::new(db.clone());
            let prediction = store
                .read_live_predictions(case.session_id, 10)?
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("MEJEPA_MULTI_AGENT_SESSION_PREDICTION_MISSING"))?;
            barrier.wait();
            persist_feedback_for_prediction(&db, &prediction, &case)
        }));
    }
    let mut out = Vec::new();
    for handle in handles {
        out.push(
            handle
                .join()
                .map_err(|_| anyhow!("MEJEPA_MULTI_AGENT_FEEDBACK_THREAD_PANIC"))??,
        );
    }
    out.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
    Ok(out)
}

fn verify_approved(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    store: &RocksDbInferStore,
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<RealityPrediction> {
    match compiler.verify(patch, context)? {
        VerifyVerdict::Approve {
            reality_prediction, ..
        } => {
            let readback = store.read_live_predictions(context.session_id, 20)?;
            let found = readback
                .into_iter()
                .find(|row| row.prediction_id == reality_prediction.prediction_id)
                .ok_or_else(|| anyhow!("MEJEPA_MULTI_AGENT_PREDICTION_READBACK_MISSING"))?;
            ensure!(found == reality_prediction, "prediction readback mismatch");
            Ok(reality_prediction)
        }
        other => bail!("MEJEPA_MULTI_AGENT_VERIFY_NOT_APPROVED: {other:?}"),
    }
}

fn persist_feedback_for_prediction(
    db: &DB,
    prediction: &RealityPrediction,
    case: &AgentCase,
) -> Result<FeedbackArtifact> {
    let prediction_id = PredictionId(prediction.prediction_id);
    let feedback_id = feedback_id_for(prediction_id, &case.agent_id, case.feedback_ts_millis);
    let key = encode_feedback_key(&prediction_id, &case.agent_id, case.feedback_ts_millis)?;
    persist_feedback_unique(
        db,
        &key,
        &SurpriseEvent::try_new(SurpriseEvent {
            feedback_id,
            prediction_id,
            agent_id: case.agent_id.clone(),
            ts_millis: case.feedback_ts_millis,
            feedback_kind: FeedbackKind::Confirmed,
            agent_explanation: format!("{} confirmed isolated prediction", case.agent_id.0),
            actual_outcome: Some(ActualOutcome {
                oracle_outcome: OracleOutcome::Pass,
                failed_tests: Vec::new(),
                runtime_ms: Some(25 + case.feedback_ts_millis as u64 % 10),
                notes: "Phase F multi-agent isolation evidence".to_string(),
            }),
            severity: SurpriseSeverity::Low,
            extra_structured_data: json!({
                "sessionHex": case.session_hex,
                "taskId": case.task_id
            }),
            witness_hash: WitnessHash(prediction.source_panel_sha),
        })?,
    )?;
    Ok(FeedbackArtifact {
        agent_id: case.agent_id.0.clone(),
        session_hex: case.session_hex.clone(),
        prediction_id_hex: hex::encode(prediction_id.0),
        feedback_id_hex: hex::encode(feedback_id.0),
        feedback_key_hex: hex::encode(key),
        ts_millis: case.feedback_ts_millis,
    })
}

fn persist_feedback_unique(db: &DB, key: &[u8], event: &SurpriseEvent) -> Result<()> {
    if !prediction_exists(db, event.prediction_id)? {
        bail!("MEJEPA_MULTI_AGENT_PREDICTION_NOT_FOUND");
    }
    let cf = cf_handle(db, context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)?;
    if db.get_cf(cf, key)?.is_some() {
        bail!("MEJEPA_MULTI_AGENT_DUPLICATE_FEEDBACK_KEY");
    }
    let bytes = serde_json::to_vec(event)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &bytes, &opts)?;
    let readback = db
        .get_cf(cf, key)?
        .ok_or_else(|| anyhow!("MEJEPA_MULTI_AGENT_FEEDBACK_READBACK_MISSING"))?;
    ensure!(readback == bytes, "feedback readback bytes differ");
    let decoded: SurpriseEvent = serde_json::from_slice(&readback)?;
    decoded.validate()?;
    ensure!(decoded == *event, "feedback decoded readback differs");
    Ok(())
}

fn boundary_zero_session_context_rejected(db: &DB, repo_root: &Path) -> Result<BoundaryEvidence> {
    let before = count_relevant_cfs(db)?;
    let context = TaskContext {
        task_id: TaskId("phase-f-multi-agent-zero-session".to_string()),
        session_id: [0; 16],
        language: Language::Python,
        problem_statement: "zero session boundary".to_string(),
        tests: vec![TestId("test_zero_session".to_string())],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    let err = context.validate().expect_err("zero session must fail");
    let after = count_relevant_cfs(db)?;
    Ok(BoundaryEvidence {
        case: "zero_session_context_rejected".to_string(),
        trigger: "TaskContext::validate(session_id=[0;16])".to_string(),
        expected_error_code: "MEJEPA_INFER_INVALID_INPUT".to_string(),
        actual_error_code: err.code().to_string(),
        before: json!(before),
        after: json!(after),
        pass: err.code() == "MEJEPA_INFER_INVALID_INPUT" && before == after,
    })
}

fn boundary_unknown_prediction_feedback_rejected(
    db: &DB,
    case: &AgentCase,
) -> Result<BoundaryEvidence> {
    let before = count_relevant_cfs(db)?;
    let prediction_id = PredictionId([0xEF; 16]);
    let feedback_id = feedback_id_for(prediction_id, &case.agent_id, case.feedback_ts_millis + 99);
    let key = encode_feedback_key(&prediction_id, &case.agent_id, case.feedback_ts_millis + 99)?;
    let event = SurpriseEvent::try_new(SurpriseEvent {
        feedback_id,
        prediction_id,
        agent_id: case.agent_id.clone(),
        ts_millis: case.feedback_ts_millis + 99,
        feedback_kind: FeedbackKind::Confirmed,
        agent_explanation: "unknown prediction boundary".to_string(),
        actual_outcome: None,
        severity: SurpriseSeverity::Low,
        extra_structured_data: json!({}),
        witness_hash: WitnessHash([0; 32]),
    })?;
    let err = persist_feedback_unique(db, &key, &event)
        .expect_err("unknown prediction feedback must fail closed");
    let after = count_relevant_cfs(db)?;
    Ok(BoundaryEvidence {
        case: "unknown_prediction_feedback_rejected".to_string(),
        trigger: "persist feedback for absent prediction id".to_string(),
        expected_error_code: "MEJEPA_MULTI_AGENT_PREDICTION_NOT_FOUND".to_string(),
        actual_error_code: err.to_string(),
        before: json!(before),
        after: json!(after),
        pass: err.to_string() == "MEJEPA_MULTI_AGENT_PREDICTION_NOT_FOUND" && before == after,
    })
}

fn boundary_duplicate_feedback_key_rejected(db: &DB, case: &AgentCase) -> Result<BoundaryEvidence> {
    let before = count_relevant_cfs(db)?;
    let prediction = scan_predictions(db)?
        .into_iter()
        .find(|prediction| prediction.session_id == case.session_id)
        .ok_or_else(|| anyhow!("MEJEPA_MULTI_AGENT_DUPLICATE_BOUNDARY_PREDICTION_MISSING"))?;
    let prediction_id = PredictionId(prediction.prediction_id);
    let feedback_id = feedback_id_for(prediction_id, &case.agent_id, case.feedback_ts_millis);
    let key = encode_feedback_key(&prediction_id, &case.agent_id, case.feedback_ts_millis)?;
    let event = SurpriseEvent::try_new(SurpriseEvent {
        feedback_id,
        prediction_id,
        agent_id: case.agent_id.clone(),
        ts_millis: case.feedback_ts_millis,
        feedback_kind: FeedbackKind::Confirmed,
        agent_explanation: "duplicate feedback boundary".to_string(),
        actual_outcome: None,
        severity: SurpriseSeverity::Low,
        extra_structured_data: json!({}),
        witness_hash: WitnessHash(prediction.source_panel_sha),
    })?;
    let err = persist_feedback_unique(db, &key, &event)
        .expect_err("duplicate feedback key must fail closed");
    let after = count_relevant_cfs(db)?;
    Ok(BoundaryEvidence {
        case: "duplicate_feedback_key_rejected".to_string(),
        trigger: "persist feedback with an existing prediction+agent+timestamp key".to_string(),
        expected_error_code: "MEJEPA_MULTI_AGENT_DUPLICATE_FEEDBACK_KEY".to_string(),
        actual_error_code: err.to_string(),
        before: json!(before),
        after: json!(after),
        pass: err.to_string() == "MEJEPA_MULTI_AGENT_DUPLICATE_FEEDBACK_KEY" && before == after,
    })
}

fn build_agent_cases(repo_root: &Path) -> Result<Vec<AgentCase>> {
    let mut cases = Vec::new();
    for idx in 0..AGENT_COUNT {
        let session_byte = u8::try_from(idx + 1)?;
        let session_id = [session_byte; 16];
        let session_hex = hex::encode(session_id);
        let path = format!("agent_{idx}_change.py");
        let before = format!("def agent_{idx}_value():\n    return {idx}\n");
        let after = format!("def agent_{idx}_value():\n    return {}\n", idx + 10);
        let repo_file = repo_root.join(&path);
        write_bytes_readback(&repo_file, after.as_bytes())?;
        let patch = PatchBundle::try_new(
            AstDiff {
                hunks: vec![DiffHunk {
                    path: PathBuf::from(&path),
                    pre_sha: context_graph_mejepa::sha256_bytes(before.as_bytes()),
                    post_sha: context_graph_mejepa::sha256_bytes(after.as_bytes()),
                    before,
                    after: after.clone(),
                }],
            },
            valid_witness_segment(),
            format!("Phase F multi-agent edit {idx}"),
            context_graph_mejepa::sha256_bytes(after.as_bytes()),
        )?;
        let task_id = format!("phase-f-multi-agent-task-{idx}");
        let context = TaskContext {
            task_id: TaskId(task_id.clone()),
            session_id,
            language: Language::Python,
            problem_statement: format!("multi-agent isolated prediction {idx}"),
            tests: vec![TestId(format!("test_agent_{idx}_value"))],
            environment: TaskEnvironment {
                repo_root: repo_root.to_path_buf(),
                python_version: Some("3.11".to_string()),
                os: std::env::consts::OS.to_string(),
            },
            claim_graph: None,
            skill_citations: Vec::new(),
        };
        context.validate()?;
        cases.push(AgentCase {
            agent_id: AgentId::try_new(format!("phase-f-agent-{idx}"))?,
            session_id,
            session_hex,
            task_id,
            patch,
            context,
            feedback_ts_millis: 2_000_000_000_000 + i64::try_from(idx)?,
        });
    }
    Ok(cases)
}

fn seed_infer_db(db: Arc<DB>) -> Result<()> {
    let calibration_store = CalibrationStore::new(db.clone(), 30)?;
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![if idx % 10 == 0 { 0.2 } else { 0.95 }],
            actual_test_pass: vec![if idx % 10 == 0 { 0.0 } else { 1.0 }],
        })
        .collect::<Vec<_>>();
    let norm_sq = vec![0.01; examples.len()];
    calibration_store.calibrate(
        &examples,
        &norm_sq,
        0.10,
        30,
        0.30,
        [8; 32],
        BTreeMap::new(),
    )?;
    let cf = cf_handle(db.as_ref(), context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)?;
    let cert = TrainCertSummary {
        step: 1,
        delta_omega: 0.8,
        delta_xi: 0.8,
        witness_offset: 44,
    };
    let value = bincode::serialize(&cert)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, b"cert:multi-agent:0001", &value, &opts)?;
    let readback = db
        .get_cf(cf, b"cert:multi-agent:0001")?
        .ok_or_else(|| anyhow!("MEJEPA_MULTI_AGENT_TRAIN_CERT_READBACK_MISSING"))?;
    ensure!(readback == value, "train cert readback mismatch");
    flush_infer_db(db.as_ref())
}

fn per_session_prediction_counts(
    store: &RocksDbInferStore,
    cases: &[AgentCase],
) -> Result<BTreeMap<String, usize>> {
    let mut out = BTreeMap::new();
    for case in cases {
        out.insert(
            case.session_hex.clone(),
            store.read_live_predictions(case.session_id, 1000)?.len(),
        );
    }
    Ok(out)
}

fn feedback_session_map(db: &DB) -> Result<BTreeMap<String, String>> {
    let predictions = scan_predictions(db)?
        .into_iter()
        .map(|prediction| (prediction.prediction_id, hex::encode(prediction.session_id)))
        .collect::<BTreeMap<_, _>>();
    let mut out = BTreeMap::new();
    for event in scan_feedback(db)? {
        let session = predictions
            .get(&event.prediction_id.0)
            .ok_or_else(|| anyhow!("MEJEPA_MULTI_AGENT_FEEDBACK_ORPHANED_PREDICTION"))?;
        out.insert(event.agent_id.0, session.clone());
    }
    Ok(out)
}

fn expected_feedback_session_map(cases: &[AgentCase]) -> BTreeMap<String, String> {
    cases
        .iter()
        .map(|case| (case.agent_id.0.clone(), case.session_hex.clone()))
        .collect()
}

fn distinct_prediction_count(predictions: &[PredictionArtifact]) -> usize {
    predictions
        .iter()
        .map(|prediction| prediction.prediction_id_hex.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

fn prediction_exists(db: &DB, prediction_id: PredictionId) -> Result<bool> {
    Ok(scan_predictions(db)?
        .into_iter()
        .any(|prediction| prediction.prediction_id == prediction_id.0))
}

fn scan_predictions(db: &DB) -> Result<Vec<RealityPrediction>> {
    let cf = cf_handle(db, context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let prediction: RealityPrediction = bincode::deserialize(&value)?;
        prediction.validate()?;
        out.push(prediction);
    }
    Ok(out)
}

fn scan_feedback(db: &DB) -> Result<Vec<SurpriseEvent>> {
    let cf = cf_handle(db, context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let event: SurpriseEvent = serde_json::from_slice(&value)?;
        event.validate()?;
        out.push(event);
    }
    Ok(out)
}

fn feedback_id_for(prediction_id: PredictionId, agent_id: &AgentId, ts_millis: i64) -> FeedbackId {
    let mut hasher = Sha256::new();
    hasher.update(prediction_id.0);
    hasher.update(agent_id.0.as_bytes());
    hasher.update(ts_millis.to_be_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    FeedbackId(out)
}

fn encode_feedback_key(
    prediction_id: &PredictionId,
    agent_id: &AgentId,
    ts_millis: i64,
) -> Result<Vec<u8>> {
    bincode::serialize(&(prediction_id, agent_id, ts_millis)).context("encode feedback key")
}

fn count_relevant_cfs(db: &DB) -> Result<BTreeMap<String, u64>> {
    let mut out = BTreeMap::new();
    for cf_name in [
        context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
        context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
        context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
        context_graph_mejepa_cf::CF_MEJEPA_CALIBRATION_HISTORY,
    ] {
        out.insert(cf_name.to_string(), count_cf(db, cf_name)?);
    }
    Ok(out)
}

fn cf_handle<'a>(db: &'a DB, name: &str) -> Result<&'a rocksdb::ColumnFamily> {
    db.cf_handle(name)
        .ok_or_else(|| anyhow!("MEJEPA_MULTI_AGENT_MISSING_CF:{name}"))
}

fn flush_infer_db(db: &DB) -> Result<()> {
    for cf_name in MEJEPA_INFER_CFS {
        if let Some(cf) = db.cf_handle(cf_name) {
            db.flush_cf(cf)?;
        }
    }
    Ok(())
}

fn write_bytes_readback(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    let mut file = options
        .open(path)
        .with_context(|| format!("open write {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let readback = fs::read(path)?;
    ensure!(
        readback == bytes,
        "file readback mismatch for {}",
        path.display()
    );
    Ok(())
}

fn remove_existing_fsv_root(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let temp_dir = std::env::temp_dir();
    let allowed = path.starts_with("/mnt/d/contextgraph/fsv/") || path.starts_with(&temp_dir);
    ensure!(
        allowed,
        "refusing to remove existing multi-agent FSV root outside /mnt/d/contextgraph/fsv or temp dir: {}",
        path.display()
    );
    let mut last_err = None;
    for attempt in 0..5 {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                std::thread::sleep(Duration::from_millis(50 * (attempt + 1) as u64));
            }
        }
    }
    if !path.exists() {
        return Ok(());
    }
    Err(last_err.expect("remove_dir_all attempted at least once"))
        .with_context(|| format!("remove {}", path.display()))
}

fn collect_file_artifacts(root: &Path) -> Result<Vec<FileArtifact>> {
    let mut out = Vec::new();
    collect_file_artifacts_inner(root, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn collect_file_artifacts_inner(path: &Path, out: &mut Vec<FileArtifact>) -> Result<()> {
    if path.is_file() {
        let bytes = fs::read(path)?;
        out.push(FileArtifact {
            path: path.display().to_string(),
            bytes: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(&bytes)),
        });
        return Ok(());
    }
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            collect_file_artifacts_inner(&entry?.path(), out)?;
        }
    }
    Ok(())
}

fn git_head_short() -> String {
    match Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_test_003_multi_agent_sessions_and_feedback_are_isolated() {
        let fsv_root = default_fsv_root();
        let report_path = default_report_path(&fsv_root);
        let report = run_multi_agent_fsv(&fsv_root).expect("multi-agent FSV");
        write_report_with_readback(&report, &report_path).expect("write multi-agent report");
        assert!(report.all_passed);
        assert!(report.readback_equal);
        assert!(report.happy_path.len() >= 3);
        assert!(report.boundary_cases.len() >= 3);
        assert_eq!(
            report
                .cf_counts_after
                .get(context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)
                .copied(),
            Some(3)
        );
        assert_eq!(
            report
                .cf_counts_after
                .get(context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)
                .copied(),
            Some(3)
        );
    }
}
