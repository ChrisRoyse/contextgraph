use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, ensure, Context, Result};
use context_graph_mejepa::{
    build_deterministic_compiler, count_cf, fixture_patch_context, open_infer_rocksdb,
    ActualOutcome, AgentId, CalibrationExample, CalibrationStore, FeedbackId, FeedbackKind,
    Language, MeJepaInferConfig, MejepaStore, OracleOutcome, PatchBundle, PredictionId,
    RealityPrediction, RocksDbInferStore, SurpriseEvent, SurpriseSeverity, TaskContext, TaskId,
    TrainCertSummary, VerifyVerdict, WitnessHash, MEJEPA_INFER_CFS,
};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const TASK_ID: &str = "TASK-TEST-007";
const DEFAULT_AGENT_COUNT: usize = 6;
const DEFAULT_EDITS_PER_AGENT: usize = 4;
const DEFAULT_P99_BUDGET_MS: u64 = 30_000;
const MAX_AGENT_COUNT: usize = 64;
const MAX_EDITS_PER_AGENT: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LoadFsvReport {
    pub fsv_root: String,
    pub task_id: String,
    pub build_release_sha: String,
    pub source_of_truth: LoadSourceOfTruth,
    pub profile: LoadProfile,
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
pub struct LoadSourceOfTruth {
    pub infer_db_path: String,
    pub repo_root: String,
    pub metrics_export_path: String,
    pub report_path: String,
    pub live_prediction_cf: String,
    pub feedback_cf: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LoadProfile {
    pub concurrent_agents: usize,
    pub edits_per_agent: usize,
    pub p99_latency_budget_ms: u64,
}

impl LoadProfile {
    fn try_new(
        concurrent_agents: usize,
        edits_per_agent: usize,
        p99_latency_budget_ms: u64,
    ) -> Result<Self> {
        if concurrent_agents == 0 {
            bail!("MEJEPA_LOAD_INVALID_INPUT: concurrent_agents must be >= 1");
        }
        if concurrent_agents > MAX_AGENT_COUNT {
            bail!("MEJEPA_LOAD_INVALID_INPUT: concurrent_agents must be <= {MAX_AGENT_COUNT}");
        }
        if edits_per_agent == 0 {
            bail!("MEJEPA_LOAD_INVALID_INPUT: edits_per_agent must be >= 1");
        }
        if edits_per_agent > MAX_EDITS_PER_AGENT {
            bail!("MEJEPA_LOAD_INVALID_INPUT: edits_per_agent must be <= {MAX_EDITS_PER_AGENT}");
        }
        if p99_latency_budget_ms == 0 {
            bail!("MEJEPA_LOAD_INVALID_INPUT: p99_latency_budget_ms must be >= 1");
        }
        Ok(Self {
            concurrent_agents,
            edits_per_agent,
            p99_latency_budget_ms,
        })
    }

    fn total_predictions(self) -> usize {
        self.concurrent_agents * self.edits_per_agent
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LoadMetrics {
    pub total_predictions: usize,
    pub total_feedback_events: usize,
    pub prediction_latency_ms: LatencySummary,
    pub feedback_latency_ms: LatencySummary,
    pub prediction_throughput_per_sec: f64,
    pub feedback_throughput_per_sec: f64,
    pub per_agent_prediction_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LatencySummary {
    pub min: u64,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub max: u64,
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
struct LoadCase {
    agent_id: AgentId,
    session_id: [u8; 16],
    session_hex: String,
    patch: PatchBundle,
    context: TaskContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct PredictionSample {
    agent_id: String,
    session_hex: String,
    prediction_id_hex: String,
    latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct FeedbackSample {
    agent_id: String,
    session_hex: String,
    prediction_id_hex: String,
    feedback_id_hex: String,
    latency_ms: u64,
}

pub fn default_fsv_root() -> PathBuf {
    process_thread_scoped_fsv_root("phase-f-load-test-fsv")
}

pub fn process_thread_scoped_fsv_root(task_dir: &str) -> PathBuf {
    let thread_id = sanitize_path_segment(&format!("{:?}", std::thread::current().id()));
    PathBuf::from("/mnt/d/contextgraph/fsv")
        .join(task_dir)
        .join("runs")
        .join(format!("pid-{}-{thread_id}", std::process::id()))
}

fn sanitize_path_segment(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

pub fn default_report_path(fsv_root: &Path) -> PathBuf {
    fsv_root.join("load_test_fsv.json")
}

pub fn run_load_fsv(fsv_root: &Path) -> Result<LoadFsvReport> {
    remove_existing_fsv_root(fsv_root)?;
    fs::create_dir_all(fsv_root)?;
    let repo_root = fsv_root.join("repo");
    let db_path = fsv_root.join("infer-rocksdb");
    let metrics_export_path = fsv_root.join("exports/load-tests/phase-f-load.json");
    let report_path = default_report_path(fsv_root);
    let profile = LoadProfile::try_new(
        DEFAULT_AGENT_COUNT,
        DEFAULT_EDITS_PER_AGENT,
        DEFAULT_P99_BUDGET_MS,
    )?;
    let build_release_sha = git_head_short();

    let cf_counts_before = {
        let db = open_infer_rocksdb(&db_path)?;
        seed_infer_db(db.clone())?;
        count_relevant_cfs(db.as_ref())?
    };

    let (mut happy_path, boundary_cases, cf_counts_after) = {
        let db = open_infer_rocksdb(&db_path)?;
        let store = Arc::new(RocksDbInferStore::new(db.clone()));
        let calibration = CalibrationStore::new(db.clone(), 30)?;
        let cases = build_load_cases(&repo_root, profile)?;
        let predictions_before = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
        )?;
        let feedback_before = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
        )?;

        let prediction_started = Instant::now();
        let prediction_samples = run_prediction_load(
            &repo_root,
            db.clone(),
            store.clone(),
            calibration,
            cases.clone(),
            profile,
        )?;
        let prediction_elapsed = prediction_started.elapsed();
        let predictions_after = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
        )?;
        let per_agent_counts = per_agent_prediction_counts(store.as_ref(), &cases)?;
        let prediction_latencies = prediction_samples
            .iter()
            .map(|sample| sample.latency_ms)
            .collect::<Vec<_>>();
        let prediction_latency = summarize_latencies(&prediction_latencies)?;
        let predictions_pass = prediction_samples.len() == profile.total_predictions()
            && predictions_after == predictions_before + profile.total_predictions() as u64
            && per_agent_counts
                .values()
                .all(|count| *count == profile.edits_per_agent)
            && prediction_latency.p99 <= profile.p99_latency_budget_ms;

        let feedback_started = Instant::now();
        let feedback_samples = run_feedback_load(db.clone(), &prediction_samples)?;
        let feedback_elapsed = feedback_started.elapsed();
        let feedback_after = count_cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
        )?;
        let feedback_latencies = feedback_samples
            .iter()
            .map(|sample| sample.latency_ms)
            .collect::<Vec<_>>();
        let feedback_latency = summarize_latencies(&feedback_latencies)?;
        let feedback_pass = feedback_samples.len() == profile.total_predictions()
            && feedback_after == feedback_before + profile.total_predictions() as u64;

        let metrics = LoadMetrics {
            total_predictions: prediction_samples.len(),
            total_feedback_events: feedback_samples.len(),
            prediction_latency_ms: prediction_latency,
            feedback_latency_ms: feedback_latency,
            prediction_throughput_per_sec: throughput(prediction_samples.len(), prediction_elapsed),
            feedback_throughput_per_sec: throughput(feedback_samples.len(), feedback_elapsed),
            per_agent_prediction_counts: per_agent_counts.clone(),
        };
        write_json_readback(&metrics_export_path, &metrics)?;
        let metrics_readback: LoadMetrics =
            serde_json::from_slice(&fs::read(&metrics_export_path)?)?;
        flush_infer_db(db.as_ref())?;

        let happy_path = vec![
            StepEvidence {
                step: "concurrent_prediction_load".to_string(),
                trigger: format!(
                    "{} agents x {} edits call MeJepaCompiler::verify concurrently",
                    profile.concurrent_agents, profile.edits_per_agent
                ),
                source_of_truth: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
                before: json!({"rows": predictions_before}),
                after: json!({
                    "rows": predictions_after,
                    "samples": prediction_samples.len(),
                    "latency": prediction_latency,
                    "perAgentCounts": per_agent_counts
                }),
                expected: json!({
                    "rows": predictions_before + profile.total_predictions() as u64,
                    "samples": profile.total_predictions(),
                    "p99LatencyUnderBudgetMs": profile.p99_latency_budget_ms
                }),
                actual: json!({
                    "rows": predictions_after,
                    "samples": prediction_samples.len(),
                    "p99LatencyMs": prediction_latency.p99
                }),
                pass: predictions_pass,
            },
            StepEvidence {
                step: "concurrent_feedback_load".to_string(),
                trigger: "persist one SurpriseEvent per prediction under concurrent load"
                    .to_string(),
                source_of_truth: context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK.to_string(),
                before: json!({"rows": feedback_before}),
                after: json!({
                    "rows": feedback_after,
                    "samples": feedback_samples.len(),
                    "latency": feedback_latency
                }),
                expected: json!({
                    "rows": feedback_before + profile.total_predictions() as u64,
                    "samples": profile.total_predictions()
                }),
                actual: json!({"rows": feedback_after, "samples": feedback_samples.len()}),
                pass: feedback_pass,
            },
            StepEvidence {
                step: "load_metrics_export_readback".to_string(),
                trigger: "write load metrics JSON and read it back".to_string(),
                source_of_truth: metrics_export_path.display().to_string(),
                before: json!(null),
                after: json!({"metrics": metrics_readback}),
                expected: json!({"readbackMatches": true}),
                actual: json!({"readbackMatches": metrics_readback == metrics}),
                pass: metrics_readback == metrics,
            },
        ];
        let boundary_cases = vec![
            boundary_invalid_profile(
                "zero_agents_rejected",
                || LoadProfile::try_new(0, 1, DEFAULT_P99_BUDGET_MS),
                db.as_ref(),
            )?,
            boundary_invalid_profile(
                "zero_edits_rejected",
                || LoadProfile::try_new(1, 0, DEFAULT_P99_BUDGET_MS),
                db.as_ref(),
            )?,
            boundary_invalid_profile(
                "zero_latency_budget_rejected",
                || LoadProfile::try_new(1, 1, 0),
                db.as_ref(),
            )?,
            boundary_invalid_profile(
                "agent_count_over_max_rejected",
                || LoadProfile::try_new(MAX_AGENT_COUNT + 1, 1, DEFAULT_P99_BUDGET_MS),
                db.as_ref(),
            )?,
        ];
        (happy_path, boundary_cases, count_relevant_cfs(db.as_ref())?)
    };

    let reopened_counts = {
        let db = open_infer_rocksdb(&db_path)?;
        count_relevant_cfs(db.as_ref())?
    };
    let readback_equal = reopened_counts == cf_counts_after;
    happy_path.push(StepEvidence {
        step: "independent_reopen_readback".to_string(),
        trigger: "drop handles, reopen RocksDB, recount load-test CFs".to_string(),
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
    if !all_passed {
        let failed = json!({
            "failedSteps": happy_path
                .iter()
                .filter(|step| !step.pass)
                .map(|step| json!({
                    "step": step.step,
                    "expected": step.expected,
                    "actual": step.actual
                }))
                .collect::<Vec<_>>(),
            "failedBoundaries": boundary_cases
                .iter()
                .filter(|case| !case.pass)
                .map(|case| json!({
                    "case": case.case,
                    "expectedErrorCode": case.expected_error_code,
                    "actualErrorCode": case.actual_error_code
                }))
                .collect::<Vec<_>>()
        });
        bail!("load FSV contains failed evidence: {failed}");
    }
    ensure!(readback_equal, "load FSV reopen counts changed");

    Ok(LoadFsvReport {
        fsv_root: fsv_root.display().to_string(),
        task_id: TASK_ID.to_string(),
        build_release_sha,
        source_of_truth: LoadSourceOfTruth {
            infer_db_path: db_path.display().to_string(),
            repo_root: repo_root.display().to_string(),
            metrics_export_path: metrics_export_path.display().to_string(),
            report_path: report_path.display().to_string(),
            live_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            feedback_cf: context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK.to_string(),
        },
        profile,
        happy_path,
        boundary_cases,
        cf_counts_before,
        cf_counts_after: reopened_counts,
        physical_artifacts: collect_file_artifacts(fsv_root)?,
        readback_equal,
        all_passed,
    })
}

pub fn write_report_with_readback(report: &LoadFsvReport, path: &Path) -> Result<()> {
    write_json_readback(path, report)?;
    let readback: LoadFsvReport = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(readback == *report, "load FSV readback mismatch");
    Ok(())
}

fn run_prediction_load(
    repo_root: &Path,
    db: Arc<DB>,
    store: Arc<RocksDbInferStore>,
    calibration: CalibrationStore,
    cases: Vec<Vec<LoadCase>>,
    profile: LoadProfile,
) -> Result<Vec<PredictionSample>> {
    let barrier = Arc::new(Barrier::new(profile.concurrent_agents));
    let mut handles = Vec::new();
    for agent_cases in cases {
        let barrier = barrier.clone();
        let repo_root = repo_root.to_path_buf();
        let db = db.clone();
        let store = store.clone();
        let calibration = calibration.clone();
        handles.push(std::thread::spawn(
            move || -> Result<Vec<PredictionSample>> {
                let compiler = build_deterministic_compiler(
                    repo_root,
                    Arc::new(RocksDbInferStore::new(db)),
                    calibration,
                    MeJepaInferConfig {
                        outcome_set_max: 4,
                        ..MeJepaInferConfig::default()
                    },
                )?;
                barrier.wait();
                let mut samples = Vec::new();
                for case in agent_cases {
                    let started = Instant::now();
                    let prediction =
                        verify_approved(&compiler, store.as_ref(), &case.patch, &case.context)?;
                    samples.push(PredictionSample {
                        agent_id: case.agent_id.0,
                        session_hex: case.session_hex,
                        prediction_id_hex: hex::encode(prediction.prediction_id),
                        latency_ms: millis_u64(started.elapsed()),
                    });
                }
                Ok(samples)
            },
        ));
    }

    let mut out = Vec::new();
    for handle in handles {
        out.extend(
            handle
                .join()
                .map_err(|_| anyhow!("MEJEPA_LOAD_PREDICTION_THREAD_PANIC"))??,
        );
    }
    out.sort_by(|a, b| {
        a.agent_id
            .cmp(&b.agent_id)
            .then_with(|| a.prediction_id_hex.cmp(&b.prediction_id_hex))
    });
    Ok(out)
}

fn run_feedback_load(
    db: Arc<DB>,
    prediction_samples: &[PredictionSample],
) -> Result<Vec<FeedbackSample>> {
    let predictions = scan_predictions(db.as_ref())?
        .into_iter()
        .map(|prediction| (hex::encode(prediction.prediction_id), prediction))
        .collect::<BTreeMap<_, _>>();
    let barrier = Arc::new(Barrier::new(prediction_samples.len()));
    let mut handles = Vec::new();
    for (idx, sample) in prediction_samples.iter().cloned().enumerate() {
        let db = db.clone();
        let barrier = barrier.clone();
        let prediction = predictions
            .get(&sample.prediction_id_hex)
            .cloned()
            .ok_or_else(|| anyhow!("MEJEPA_LOAD_PREDICTION_SAMPLE_READBACK_MISSING"))?;
        handles.push(std::thread::spawn(move || -> Result<FeedbackSample> {
            barrier.wait();
            let started = Instant::now();
            let agent_id = AgentId::try_new(sample.agent_id.clone())?;
            let prediction_id = PredictionId(prediction.prediction_id);
            let ts_millis = 2_100_000_000_000 + i64::try_from(idx)?;
            let feedback_id = feedback_id_for(prediction_id, &agent_id, ts_millis);
            let key = encode_feedback_key(&prediction_id, &agent_id, ts_millis)?;
            let event = SurpriseEvent::try_new(SurpriseEvent {
                feedback_id,
                prediction_id,
                agent_id: agent_id.clone(),
                ts_millis,
                feedback_kind: FeedbackKind::Confirmed,
                agent_explanation: format!("{} confirmed prediction under load", agent_id.0),
                actual_outcome: Some(ActualOutcome {
                    oracle_outcome: OracleOutcome::Pass,
                    failed_tests: Vec::new(),
                    runtime_ms: Some(25),
                    notes: "Phase F load feedback evidence".to_string(),
                }),
                severity: SurpriseSeverity::Low,
                extra_structured_data: json!({"sessionHex": sample.session_hex}),
                witness_hash: WitnessHash(prediction.source_panel_sha),
            })?;
            persist_feedback_unique(db.as_ref(), &key, &event)?;
            Ok(FeedbackSample {
                agent_id: agent_id.0,
                session_hex: sample.session_hex,
                prediction_id_hex: sample.prediction_id_hex,
                feedback_id_hex: hex::encode(feedback_id.0),
                latency_ms: millis_u64(started.elapsed()),
            })
        }));
    }
    let mut out = Vec::new();
    for handle in handles {
        out.push(
            handle
                .join()
                .map_err(|_| anyhow!("MEJEPA_LOAD_FEEDBACK_THREAD_PANIC"))??,
        );
    }
    out.sort_by(|a, b| {
        a.agent_id
            .cmp(&b.agent_id)
            .then_with(|| a.feedback_id_hex.cmp(&b.feedback_id_hex))
    });
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
            let found = store
                .read_live_predictions(context.session_id, 1000)?
                .into_iter()
                .find(|row| row.prediction_id == reality_prediction.prediction_id)
                .ok_or_else(|| anyhow!("MEJEPA_LOAD_PREDICTION_READBACK_MISSING"))?;
            ensure!(found == reality_prediction, "prediction readback mismatch");
            Ok(reality_prediction)
        }
        other => bail!("MEJEPA_LOAD_VERIFY_NOT_APPROVED: {other:?}"),
    }
}

fn persist_feedback_unique(db: &DB, key: &[u8], event: &SurpriseEvent) -> Result<()> {
    if !prediction_exists(db, event.prediction_id)? {
        bail!("MEJEPA_LOAD_PREDICTION_NOT_FOUND");
    }
    let cf = cf_handle(db, context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)?;
    if db.get_cf(cf, key)?.is_some() {
        bail!("MEJEPA_LOAD_DUPLICATE_FEEDBACK_KEY");
    }
    let bytes = serde_json::to_vec(event)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &bytes, &opts)?;
    let readback = db
        .get_cf(cf, key)?
        .ok_or_else(|| anyhow!("MEJEPA_LOAD_FEEDBACK_READBACK_MISSING"))?;
    ensure!(readback == bytes, "feedback readback bytes differ");
    let decoded: SurpriseEvent = serde_json::from_slice(&readback)?;
    decoded.validate()?;
    ensure!(decoded == *event, "feedback decoded readback differs");
    Ok(())
}

fn boundary_invalid_profile<F>(case: &str, action: F, db: &DB) -> Result<BoundaryEvidence>
where
    F: FnOnce() -> Result<LoadProfile>,
{
    let before = count_relevant_cfs(db)?;
    let err = action().expect_err("invalid load profile must fail");
    let after = count_relevant_cfs(db)?;
    let actual_error_code = err
        .to_string()
        .split(':')
        .next()
        .unwrap_or_default()
        .to_string();
    Ok(BoundaryEvidence {
        case: case.to_string(),
        trigger: format!("LoadProfile::try_new boundary {case}"),
        expected_error_code: "MEJEPA_LOAD_INVALID_INPUT".to_string(),
        actual_error_code: actual_error_code.clone(),
        before: json!(before),
        after: json!(after),
        pass: actual_error_code == "MEJEPA_LOAD_INVALID_INPUT" && before == after,
    })
}

fn build_load_cases(repo_root: &Path, profile: LoadProfile) -> Result<Vec<Vec<LoadCase>>> {
    let mut grouped = Vec::new();
    for agent_idx in 0..profile.concurrent_agents {
        let session_byte = u8::try_from(agent_idx + 1)?;
        let session_id = [session_byte; 16];
        let session_hex = hex::encode(session_id);
        let agent_id = AgentId::try_new(format!("phase-f-load-agent-{agent_idx}"))?;
        let mut cases = Vec::new();
        for edit_idx in 0..profile.edits_per_agent {
            let scenario = format!("load-agent-{agent_idx}-edit-{edit_idx}");
            let (patch, mut context) = fixture_patch_context(repo_root, &scenario)?;
            context.session_id = session_id;
            context.task_id = TaskId(format!("phase-f-load-task-{agent_idx}-{edit_idx}"));
            context.problem_statement = format!("load profile {agent_idx}/{edit_idx}");
            context.validate()?;
            cases.push(LoadCase {
                agent_id: agent_id.clone(),
                session_id,
                session_hex: session_hex.clone(),
                patch,
                context,
            });
        }
        grouped.push(cases);
    }
    Ok(grouped)
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
        [7; 32],
        BTreeMap::new(),
    )?;
    let cf = cf_handle(db.as_ref(), context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)?;
    let cert = TrainCertSummary {
        step: 1,
        delta_omega: 0.8,
        delta_xi: 0.8,
        witness_offset: 77,
    };
    let value = bincode::serialize(&cert)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, b"cert:load:0001", &value, &opts)?;
    let readback = db
        .get_cf(cf, b"cert:load:0001")?
        .ok_or_else(|| anyhow!("MEJEPA_LOAD_TRAIN_CERT_READBACK_MISSING"))?;
    ensure!(readback == value, "train cert readback mismatch");
    flush_infer_db(db.as_ref())
}

fn per_agent_prediction_counts(
    store: &RocksDbInferStore,
    cases: &[Vec<LoadCase>],
) -> Result<BTreeMap<String, usize>> {
    let mut out = BTreeMap::new();
    for agent_cases in cases {
        let first = agent_cases
            .first()
            .ok_or_else(|| anyhow!("MEJEPA_LOAD_EMPTY_AGENT_CASES"))?;
        out.insert(
            first.agent_id.0.clone(),
            store.read_live_predictions(first.session_id, 1000)?.len(),
        );
    }
    Ok(out)
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

fn summarize_latencies(values: &[u64]) -> Result<LatencySummary> {
    ensure!(
        !values.is_empty(),
        "MEJEPA_LOAD_INVALID_INPUT: empty latency set"
    );
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    Ok(LatencySummary {
        min: sorted[0],
        p50: percentile(&sorted, 0.50),
        p95: percentile(&sorted, 0.95),
        p99: percentile(&sorted, 0.99),
        max: *sorted.last().expect("non-empty"),
    })
}

fn percentile(sorted: &[u64], q: f64) -> u64 {
    let idx = ((sorted.len() - 1) as f64 * q).ceil() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn throughput(samples: usize, elapsed: Duration) -> f64 {
    let seconds = elapsed.as_secs_f64();
    if seconds <= f64::EPSILON {
        return samples as f64;
    }
    samples as f64 / seconds
}

fn millis_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
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
        .ok_or_else(|| anyhow!("MEJEPA_LOAD_MISSING_CF:{name}"))
}

fn flush_infer_db(db: &DB) -> Result<()> {
    for cf_name in MEJEPA_INFER_CFS {
        if let Some(cf) = db.cf_handle(cf_name) {
            db.flush_cf(cf)?;
        }
    }
    Ok(())
}

fn write_json_readback<T>(path: &Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value)?;
    write_bytes_readback(path, &bytes)
}

fn write_bytes_readback(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
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
        "refusing to remove existing load FSV root outside /mnt/d/contextgraph/fsv or temp dir: {}",
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
