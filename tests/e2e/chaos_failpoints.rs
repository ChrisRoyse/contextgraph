use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, ensure, Context, Result};
use context_graph_mejepa::calibration::count_cf;
use context_graph_mejepa::{
    build_fixture_deterministic_compiler, open_infer_rocksdb, sha256_bytes, valid_witness_segment,
    ActualOutcome, AgentId, AstDiff, CalibrationExample, CalibrationStore, DiffHunk, FeedbackId,
    FeedbackKind, Language, MeJepaInferConfig, MejepaStore, OracleOutcome, PatchBundle,
    PredictionId, RealityPrediction, RocksDbInferStore, SurpriseEvent, SurpriseSeverity,
    TaskContext, TaskEnvironment, TaskId, TestId, TrainCertSummary, VerifyVerdict, WitnessHash,
    MEJEPA_INFER_CFS,
};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const TASK_ID: &str = "TASK-TEST-006";
const FP_PREDICTION_WRITE: &str = "phase_f_chaos_prediction_write";
const FP_FEEDBACK_WRITE: &str = "phase_f_chaos_feedback_write";
const FP_FLUSH: &str = "phase_f_chaos_flush";
const FAILPOINT_ACTION: &str = "return";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChaosFailpointsFsvReport {
    pub fsv_root: String,
    pub task_id: String,
    pub build_release_sha: String,
    pub source_of_truth: ChaosSourceOfTruth,
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
pub struct ChaosSourceOfTruth {
    pub infer_db_path: String,
    pub repo_root: String,
    pub live_prediction_cf: String,
    pub feedback_cf: String,
    pub report_path: String,
    pub failpoints: Vec<String>,
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

pub fn default_fsv_root() -> PathBuf {
    PathBuf::from("/zfs/archive/contextgraph/fsv/phase-f-chaos-failpoints-fsv")
}

pub fn default_report_path(fsv_root: &Path) -> PathBuf {
    fsv_root.join("chaos_failpoints_fsv.json")
}

pub fn run_chaos_failpoints_fsv(fsv_root: &Path) -> Result<ChaosFailpointsFsvReport> {
    clear_all_failpoints();
    remove_existing_fsv_root(fsv_root)?;
    fs::create_dir_all(fsv_root)?;
    let repo_root = fsv_root.join("repo");
    let db_path = fsv_root.join("infer-rocksdb");
    fs::create_dir_all(&repo_root)?;
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
        let (patch, context) = build_patch_and_context(&repo_root)?;
        let mut happy_path = Vec::new();
        let mut boundary_cases = Vec::new();

        let predictions_before = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
        )?;
        let compiler = build_fixture_deterministic_compiler(
            repo_root.clone(),
            store.clone(),
            calibration,
            MeJepaInferConfig {
                outcome_set_max: 4,
                ..MeJepaInferConfig::default()
            },
        )?;
        let baseline_prediction = verify_approved(&compiler, store.as_ref(), &patch, &context)?;
        let predictions_after = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
        )?;
        happy_path.push(StepEvidence {
            step: "baseline_prediction_persists_before_chaos".to_string(),
            trigger: "MeJepaCompiler::verify without failpoint".to_string(),
            source_of_truth: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            before: json!({"rows": predictions_before}),
            after: json!({
                "rows": predictions_after,
                "predictionId": hex::encode(baseline_prediction.prediction_id)
            }),
            expected: json!({"rows": predictions_before + 1}),
            actual: json!({"rows": predictions_after}),
            pass: predictions_after == predictions_before + 1,
        });

        let prediction_fail_before = count_relevant_cfs(db.as_ref())?;
        let blocked_prediction = prediction_variant(&baseline_prediction, [0x31; 16], 1)?;
        configure_failpoint(FP_PREDICTION_WRITE, FAILPOINT_ACTION)?;
        let prediction_err = chaos_write_live_prediction(store.as_ref(), &blocked_prediction)
            .expect_err("prediction write failpoint must fail closed");
        clear_failpoint(FP_PREDICTION_WRITE);
        let prediction_fail_after = count_relevant_cfs(db.as_ref())?;
        happy_path.push(StepEvidence {
            step: "prediction_write_failpoint_blocks_sot_change".to_string(),
            trigger: format!("{FP_PREDICTION_WRITE}=return before write_live_prediction"),
            source_of_truth: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            before: json!(prediction_fail_before),
            after: json!(prediction_fail_after),
            expected: json!({
                "error": "MEJEPA_CHAOS_PREDICTION_WRITE_FAILPOINT",
                "countsUnchanged": true
            }),
            actual: json!({
                "error": prediction_err.to_string(),
                "countsUnchanged": prediction_fail_after == prediction_fail_before
            }),
            pass: prediction_err.to_string() == "MEJEPA_CHAOS_PREDICTION_WRITE_FAILPOINT"
                && prediction_fail_after == prediction_fail_before,
        });

        let feedback_before = count_relevant_cfs(db.as_ref())?;
        let event = feedback_for_prediction(&baseline_prediction, 2_100_000_000_000)?;
        let feedback_key =
            encode_feedback_key(&event.prediction_id, &event.agent_id, event.ts_millis)?;
        configure_failpoint(FP_FEEDBACK_WRITE, FAILPOINT_ACTION)?;
        let feedback_err = chaos_write_feedback(db.as_ref(), &feedback_key, &event)
            .expect_err("feedback write failpoint must fail closed");
        clear_failpoint(FP_FEEDBACK_WRITE);
        let feedback_after = count_relevant_cfs(db.as_ref())?;
        happy_path.push(StepEvidence {
            step: "feedback_write_failpoint_blocks_sot_change".to_string(),
            trigger: format!("{FP_FEEDBACK_WRITE}=return before CF_MEJEPA_AGENT_FEEDBACK write"),
            source_of_truth: context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK.to_string(),
            before: json!(feedback_before),
            after: json!(feedback_after),
            expected: json!({
                "error": "MEJEPA_CHAOS_FEEDBACK_WRITE_FAILPOINT",
                "countsUnchanged": true
            }),
            actual: json!({
                "error": feedback_err.to_string(),
                "countsUnchanged": feedback_after == feedback_before
            }),
            pass: feedback_err.to_string() == "MEJEPA_CHAOS_FEEDBACK_WRITE_FAILPOINT"
                && feedback_after == feedback_before,
        });

        let feedback_persist_before = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
        )?;
        chaos_write_feedback(db.as_ref(), &feedback_key, &event)?;
        let feedback_persist_after = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
        )?;
        happy_path.push(StepEvidence {
            step: "feedback_persists_after_failpoint_cleared".to_string(),
            trigger: "chaos_write_feedback after fail::remove".to_string(),
            source_of_truth: context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK.to_string(),
            before: json!({"rows": feedback_persist_before}),
            after: json!({
                "rows": feedback_persist_after,
                "feedbackId": hex::encode(event.feedback_id.0)
            }),
            expected: json!({"rows": feedback_persist_before + 1}),
            actual: json!({"rows": feedback_persist_after}),
            pass: feedback_persist_after == feedback_persist_before + 1,
        });

        let flush_before = count_relevant_cfs(db.as_ref())?;
        configure_failpoint(FP_FLUSH, FAILPOINT_ACTION)?;
        let flush_err = chaos_flush_infer_db(db.as_ref())
            .expect_err("flush failpoint must fail before durability claim");
        clear_failpoint(FP_FLUSH);
        let flush_after = count_relevant_cfs(db.as_ref())?;
        happy_path.push(StepEvidence {
            step: "flush_failpoint_blocks_durability_claim".to_string(),
            trigger: format!("{FP_FLUSH}=return before RocksDB flush loop"),
            source_of_truth: "MEJEPA_INFER_CFS flush loop".to_string(),
            before: json!(flush_before),
            after: json!(flush_after),
            expected: json!({
                "error": "MEJEPA_CHAOS_FLUSH_FAILPOINT",
                "countsUnchanged": true
            }),
            actual: json!({
                "error": flush_err.to_string(),
                "countsUnchanged": flush_after == flush_before
            }),
            pass: flush_err.to_string() == "MEJEPA_CHAOS_FLUSH_FAILPOINT"
                && flush_after == flush_before,
        });

        boundary_cases.push(boundary_unknown_failpoint_rejected(db.as_ref())?);
        boundary_cases.push(boundary_invalid_failpoint_action_rejected(db.as_ref())?);
        boundary_cases.push(boundary_duplicate_feedback_key_rejected(
            db.as_ref(),
            &feedback_key,
            &event,
        )?);

        chaos_flush_infer_db(db.as_ref())?;
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
    ensure!(all_passed, "chaos failpoints FSV contains failed evidence");
    ensure!(readback_equal, "chaos failpoints FSV reopen counts changed");
    clear_all_failpoints();

    Ok(ChaosFailpointsFsvReport {
        fsv_root: fsv_root.display().to_string(),
        task_id: TASK_ID.to_string(),
        build_release_sha,
        source_of_truth: ChaosSourceOfTruth {
            infer_db_path: db_path.display().to_string(),
            repo_root: repo_root.display().to_string(),
            live_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            feedback_cf: context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK.to_string(),
            report_path: report_path.display().to_string(),
            failpoints: allowed_failpoints()
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
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

pub fn write_report_with_readback(report: &ChaosFailpointsFsvReport, path: &Path) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    write_bytes_readback(path, &bytes)?;
    let readback: ChaosFailpointsFsvReport = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        readback == *report,
        "chaos failpoints FSV readback mismatch"
    );
    Ok(())
}

fn chaos_write_live_prediction(
    store: &RocksDbInferStore,
    prediction: &RealityPrediction,
) -> Result<()> {
    fail::fail_point!(FP_PREDICTION_WRITE, |_| {
        Err::<(), anyhow::Error>(anyhow!("MEJEPA_CHAOS_PREDICTION_WRITE_FAILPOINT"))
    });
    store.write_live_prediction(prediction)?;
    Ok(())
}

fn chaos_write_feedback(db: &DB, key: &[u8], event: &SurpriseEvent) -> Result<()> {
    fail::fail_point!(FP_FEEDBACK_WRITE, |_| {
        Err::<(), anyhow::Error>(anyhow!("MEJEPA_CHAOS_FEEDBACK_WRITE_FAILPOINT"))
    });
    ensure!(
        prediction_exists(db, event.prediction_id)?,
        "MEJEPA_CHAOS_PREDICTION_NOT_FOUND"
    );
    let cf = cf_handle(db, context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)?;
    if db.get_cf(cf, key)?.is_some() {
        bail!("MEJEPA_CHAOS_DUPLICATE_FEEDBACK_KEY");
    }
    let bytes = serde_json::to_vec(event)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &bytes, &opts)?;
    let readback = db
        .get_cf(cf, key)?
        .ok_or_else(|| anyhow!("MEJEPA_CHAOS_FEEDBACK_READBACK_MISSING"))?;
    ensure!(readback == bytes, "feedback readback bytes differ");
    let decoded: SurpriseEvent = serde_json::from_slice(&readback)?;
    decoded.validate()?;
    ensure!(decoded == *event, "feedback decoded readback differs");
    Ok(())
}

fn chaos_flush_infer_db(db: &DB) -> Result<()> {
    fail::fail_point!(FP_FLUSH, |_| {
        Err::<(), anyhow::Error>(anyhow!("MEJEPA_CHAOS_FLUSH_FAILPOINT"))
    });
    for cf_name in MEJEPA_INFER_CFS {
        if let Some(cf) = db.cf_handle(cf_name) {
            db.flush_cf(cf)?;
        }
    }
    Ok(())
}

fn configure_failpoint(name: &str, action: &str) -> Result<()> {
    if !allowed_failpoints().contains(&name) {
        bail!("MEJEPA_CHAOS_UNKNOWN_FAILPOINT");
    }
    if action != FAILPOINT_ACTION {
        bail!("MEJEPA_CHAOS_INVALID_FAILPOINT_ACTION");
    }
    fail::cfg(name, action).map_err(|err| anyhow!("MEJEPA_CHAOS_FAILPOINT_CONFIG:{err}"))
}

fn clear_failpoint(name: &str) {
    fail::remove(name);
}

fn clear_all_failpoints() {
    for name in allowed_failpoints() {
        clear_failpoint(name);
    }
}

fn allowed_failpoints() -> [&'static str; 3] {
    [FP_PREDICTION_WRITE, FP_FEEDBACK_WRITE, FP_FLUSH]
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
                .ok_or_else(|| anyhow!("MEJEPA_CHAOS_PREDICTION_READBACK_MISSING"))?;
            ensure!(found == reality_prediction, "prediction readback mismatch");
            Ok(reality_prediction)
        }
        other => bail!("MEJEPA_CHAOS_VERIFY_NOT_APPROVED:{other:?}"),
    }
}

fn boundary_unknown_failpoint_rejected(db: &DB) -> Result<BoundaryEvidence> {
    let before = count_relevant_cfs(db)?;
    let err = configure_failpoint("phase_f_chaos_unknown", FAILPOINT_ACTION)
        .expect_err("unknown failpoint must fail closed");
    let after = count_relevant_cfs(db)?;
    Ok(BoundaryEvidence {
        case: "unknown_failpoint_rejected".to_string(),
        trigger: "configure_failpoint(unknown, return)".to_string(),
        expected_error_code: "MEJEPA_CHAOS_UNKNOWN_FAILPOINT".to_string(),
        actual_error_code: err.to_string(),
        before: json!(before),
        after: json!(after),
        pass: err.to_string() == "MEJEPA_CHAOS_UNKNOWN_FAILPOINT" && before == after,
    })
}

fn boundary_invalid_failpoint_action_rejected(db: &DB) -> Result<BoundaryEvidence> {
    let before = count_relevant_cfs(db)?;
    let err = configure_failpoint(FP_PREDICTION_WRITE, "panic")
        .expect_err("invalid failpoint action must fail closed");
    let after = count_relevant_cfs(db)?;
    Ok(BoundaryEvidence {
        case: "invalid_failpoint_action_rejected".to_string(),
        trigger: "configure_failpoint(prediction_write, panic)".to_string(),
        expected_error_code: "MEJEPA_CHAOS_INVALID_FAILPOINT_ACTION".to_string(),
        actual_error_code: err.to_string(),
        before: json!(before),
        after: json!(after),
        pass: err.to_string() == "MEJEPA_CHAOS_INVALID_FAILPOINT_ACTION" && before == after,
    })
}

fn boundary_duplicate_feedback_key_rejected(
    db: &DB,
    key: &[u8],
    event: &SurpriseEvent,
) -> Result<BoundaryEvidence> {
    let before = count_relevant_cfs(db)?;
    let err =
        chaos_write_feedback(db, key, event).expect_err("duplicate feedback key must fail closed");
    let after = count_relevant_cfs(db)?;
    Ok(BoundaryEvidence {
        case: "duplicate_feedback_key_rejected".to_string(),
        trigger: "chaos_write_feedback with existing feedback key".to_string(),
        expected_error_code: "MEJEPA_CHAOS_DUPLICATE_FEEDBACK_KEY".to_string(),
        actual_error_code: err.to_string(),
        before: json!(before),
        after: json!(after),
        pass: err.to_string() == "MEJEPA_CHAOS_DUPLICATE_FEEDBACK_KEY" && before == after,
    })
}

fn build_patch_and_context(repo_root: &Path) -> Result<(PatchBundle, TaskContext)> {
    let before = "def chaos_value():\n    return 1\n".to_string();
    let after = "def chaos_value():\n    return 2\n".to_string();
    let path = PathBuf::from("chaos_change.py");
    write_bytes_readback(&repo_root.join(&path), after.as_bytes())?;
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path,
                pre_sha: sha256_bytes(before.as_bytes()),
                post_sha: sha256_bytes(after.as_bytes()),
                before,
                after: after.clone(),
            }],
        },
        valid_witness_segment(),
        "Phase F chaos failpoint edit".to_string(),
        sha256_bytes(after.as_bytes()),
    )?;
    let context = TaskContext {
        task_id: TaskId("phase-f-chaos-failpoint-task".to_string()),
        session_id: [0xC6; 16],
        language: Language::Python,
        problem_statement: "chaos failpoint baseline prediction".to_string(),
        tests: vec![TestId("test_chaos_value".to_string())],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    context.validate()?;
    Ok((patch, context))
}

fn prediction_variant(
    prediction: &RealityPrediction,
    session_id: [u8; 16],
    millis_delta: i64,
) -> Result<RealityPrediction> {
    let mut variant = prediction.clone();
    variant.session_id = session_id;
    variant.created_at_unix_ms = variant
        .created_at_unix_ms
        .checked_add(millis_delta)
        .ok_or_else(|| anyhow!("MEJEPA_CHAOS_PREDICTION_TS_OVERFLOW"))?;
    variant.prediction_id = prediction_id_for(session_id, variant.created_at_unix_ms);
    variant.task_id = TaskId("phase-f-chaos-blocked-prediction".to_string());
    variant.validate()?;
    Ok(variant)
}

fn feedback_for_prediction(
    prediction: &RealityPrediction,
    ts_millis: i64,
) -> Result<SurpriseEvent> {
    let prediction_id = PredictionId(prediction.prediction_id);
    let agent_id = AgentId::try_new("phase-f-chaos-agent")?;
    let feedback_id = feedback_id_for(prediction_id, &agent_id, ts_millis);
    SurpriseEvent::try_new(SurpriseEvent {
        feedback_id,
        prediction_id,
        agent_id,
        ts_millis,
        feedback_kind: FeedbackKind::Confirmed,
        agent_explanation: "Phase F chaos failpoint confirmed readback".to_string(),
        actual_outcome: Some(ActualOutcome {
            oracle_outcome: OracleOutcome::Pass,
            failed_tests: Vec::new(),
            runtime_ms: Some(42),
            notes: "failpoint cleared after failed feedback attempt".to_string(),
        }),
        severity: SurpriseSeverity::Low,
        extra_structured_data: json!({"taskId": TASK_ID}),
        witness_hash: WitnessHash(prediction.source_panel_sha),
    })
    .map_err(Into::into)
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
        [0xC6; 32],
        BTreeMap::new(),
    )?;
    let cf = cf_handle(db.as_ref(), context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)?;
    let cert = TrainCertSummary {
        step: 1,
        delta_omega: 0.8,
        delta_xi: 0.8,
        witness_offset: 46,
    };
    let value = bincode::serialize(&cert)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, b"cert:chaos-failpoints:0001", &value, &opts)?;
    let readback = db
        .get_cf(cf, b"cert:chaos-failpoints:0001")?
        .ok_or_else(|| anyhow!("MEJEPA_CHAOS_TRAIN_CERT_READBACK_MISSING"))?;
    ensure!(readback == value, "train cert readback mismatch");
    chaos_flush_infer_db(db.as_ref())
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

fn prediction_id_for(session_id: [u8; 16], created_at_unix_ms: i64) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(session_id);
    hasher.update(created_at_unix_ms.to_be_bytes());
    hasher.update(TASK_ID.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
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
        .ok_or_else(|| anyhow!("MEJEPA_CHAOS_MISSING_CF:{name}"))
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
    let allowed = path.starts_with("/zfs/archive/contextgraph/fsv/") || path.starts_with(&temp_dir);
    ensure!(
        allowed,
        "refusing to remove existing chaos FSV root outside /zfs/archive/contextgraph/fsv or temp dir: {}",
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
    fn task_test_006_chaos_failpoints_fail_closed_with_sot_readback() {
        let fsv_root = default_fsv_root();
        let report_path = default_report_path(&fsv_root);
        let report = run_chaos_failpoints_fsv(&fsv_root).expect("chaos failpoints FSV");
        write_report_with_readback(&report, &report_path).expect("write chaos failpoints report");
        assert!(report.all_passed);
        assert!(report.readback_equal);
        assert!(report.happy_path.len() >= 5);
        assert!(report.boundary_cases.len() >= 3);
        assert_eq!(
            report
                .cf_counts_after
                .get(context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)
                .copied(),
            Some(1)
        );
        assert_eq!(
            report
                .cf_counts_after
                .get(context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)
                .copied(),
            Some(1)
        );
        let failpoint_names = report
            .source_of_truth
            .failpoints
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(failpoint_names.len(), 3);
    }
}
