use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, ensure, Context, Result};
use context_graph_mejepa::calibration::count_cf;
use context_graph_mejepa::conformal::{non_conformity_score, CalibrationExample};
use context_graph_mejepa::eval::{ActiveLearningQueueState, EvalObservation, MutationCategory};
use context_graph_mejepa::gates::{sha256_bytes, valid_witness_segment};
use context_graph_mejepa::{
    build_deterministic_compiler, current_unix_ms, open_infer_rocksdb, read_pause_state, AstDiff,
    CalibrationStore, DiffHunk, FailedGate, Language, MeJepaInferConfig, OracleOutcome,
    PatchBundle, PauseReadOutcome, PauseState, PredictionId, RocksDbEvalStore, RocksDbInferStore,
    SurpriseSeverity, TaskContext, TaskEnvironment, TaskId, TestId, VerifyVerdict,
    MEJEPA_INFER_CFS,
};
use context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const TASK_ID: &str = "TASK-FLYWHEEL-012";
const GOOD_SOURCE: &str =
    "def normalize(values):\n    return [value.strip() for value in values]\n";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PauseActiveLearningFsvReport {
    pub fsv_root: String,
    pub task_id: String,
    pub build_release_sha: String,
    pub source_of_truth: SourceOfTruth,
    pub happy_path: Vec<CaseEvidence>,
    pub boundary_cases: Vec<CaseEvidence>,
    pub cf_counts_before: BTreeMap<String, u64>,
    pub cf_counts_after: BTreeMap<String, u64>,
    pub queue_entries_before: usize,
    pub queue_entries_after: usize,
    pub physical_artifacts: Vec<FileArtifact>,
    pub readback_equal: bool,
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SourceOfTruth {
    pub pause_state_path: String,
    pub infer_db_path: String,
    pub active_learning_queue_cf: String,
    pub active_learning_queue_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CaseEvidence {
    pub case: String,
    pub trigger: String,
    pub expected: Value,
    pub actual: Value,
    pub queue_entries_before: usize,
    pub queue_entries_after: usize,
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
    PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-pause-active-learning-fsv")
}

pub fn default_report_path(fsv_root: &Path) -> PathBuf {
    fsv_root.join("pause_active_learning_fsv.json")
}

pub fn run_pause_active_learning_fsv(fsv_root: &Path) -> Result<PauseActiveLearningFsvReport> {
    remove_existing_fsv_root(fsv_root)?;
    fs::create_dir_all(fsv_root)?;
    let repo_root = fsv_root.join("repo");
    let db_path = fsv_root.join("infer-rocksdb");
    let pause_path = fsv_root.join("state/predictions_paused_until.json");
    fs::create_dir_all(repo_root.join("src"))?;
    let patch = build_patch(&repo_root, "pause_active_learning.py", GOOD_SOURCE)?;

    let cf_counts_before = {
        let db = open_infer_rocksdb(&db_path)?;
        count_queue_cf(db.as_ref())?
    };

    let (happy_path, boundary_cases, queue_entries_before, queue_entries_after, cf_counts_after) = {
        let db = open_infer_rocksdb(&db_path)?;
        let eval_store = RocksDbEvalStore::new(db.clone())?;
        seed_queue(&eval_store)?;
        let queue_entries_before = queue_entry_count(&eval_store)?;
        let compiler = build_compiler(&repo_root, &pause_path, db.clone())?;

        let happy_path = vec![active_pause_suppresses_queue(
            &compiler,
            &eval_store,
            &pause_path,
            &repo_root,
            &patch,
        )?];
        let boundary_cases = vec![
            expired_pause_allows_queue(&compiler, &eval_store, &pause_path, &repo_root, &patch)?,
            missing_pause_allows_queue(&compiler, &eval_store, &pause_path, &repo_root, &patch)?,
            malformed_pause_allows_queue(&compiler, &eval_store, &pause_path, &repo_root, &patch)?,
        ];
        flush_infer_db(db.as_ref())?;
        (
            happy_path,
            boundary_cases,
            queue_entries_before,
            queue_entry_count(&eval_store)?,
            count_queue_cf(db.as_ref())?,
        )
    };

    let (reopened_queue_entries, reopened_cf_counts) = {
        let db = open_infer_rocksdb(&db_path)?;
        let eval_store = RocksDbEvalStore::new(db.clone())?;
        (
            queue_entry_count(&eval_store)?,
            count_queue_cf(db.as_ref())?,
        )
    };
    ensure!(
        reopened_queue_entries == queue_entries_after,
        "queue entry count changed after reopen"
    );
    ensure!(
        reopened_cf_counts == cf_counts_after,
        "active-learning queue CF count changed after reopen"
    );

    let all_passed =
        happy_path.iter().all(|case| case.pass) && boundary_cases.iter().all(|case| case.pass);
    ensure!(
        all_passed,
        "pause active-learning FSV contains failed evidence"
    );

    Ok(PauseActiveLearningFsvReport {
        fsv_root: fsv_root.display().to_string(),
        task_id: TASK_ID.to_string(),
        build_release_sha: git_head_short(),
        source_of_truth: SourceOfTruth {
            pause_state_path: pause_path.display().to_string(),
            infer_db_path: db_path.display().to_string(),
            active_learning_queue_cf: CF_MEJEPA_ACTIVE_LEARNING_QUEUE.to_string(),
            active_learning_queue_key: hex::encode(b"active"),
        },
        happy_path,
        boundary_cases,
        cf_counts_before,
        cf_counts_after: reopened_cf_counts,
        queue_entries_before,
        queue_entries_after: reopened_queue_entries,
        physical_artifacts: collect_file_artifacts(fsv_root)?,
        readback_equal: true,
        all_passed,
    })
}

pub fn write_report_with_readback(
    report: &PauseActiveLearningFsvReport,
    path: &Path,
) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    write_bytes_readback(path, &bytes)?;
    let readback: PauseActiveLearningFsvReport = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        readback == *report,
        "pause active-learning FSV readback mismatch"
    );
    Ok(())
}

fn active_pause_suppresses_queue(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    eval_store: &RocksDbEvalStore,
    pause_path: &Path,
    repo_root: &Path,
    patch: &PatchBundle,
) -> Result<CaseEvidence> {
    write_pause_state_readback(
        pause_path,
        &pause_state(
            current_unix_ms() - 1_000,
            current_unix_ms() + 900_000,
            "active",
        ),
    )?;
    let before = queue_entry_count(eval_store)?;
    let mut queue = eval_store
        .load_queue()?
        .ok_or_else(|| anyhow!("active-learning queue missing before paused verifies"))?;
    let mut paused_verifies = 0usize;
    let mut suppressed_surprises = 0usize;
    let mut suppressed_uncertainty = 0usize;

    for idx in 0..100 {
        let context = build_context(repo_root, &format!("paused-{idx}"))?;
        let verdict = compiler.verify(patch, &context)?;
        let prediction = match verdict {
            VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction),
                failed_gate: FailedGate::PredictionPaused { .. },
                ..
            } => prediction,
            other => return Err(anyhow!("expected paused verify, got {other:?}")),
        };
        paused_verifies += 1;
        if !queue
            .enqueue_agent_surprise_for_prediction(&prediction, SurpriseSeverity::Catastrophic)?
        {
            suppressed_surprises += 1;
        }
        let obs = EvalObservation {
            task_id: context.task_id.clone(),
            mutation_category: MutationCategory::SubtleFlip,
            language: Language::Python,
            actual_oracle: OracleOutcome::Fail,
            actual_failure_modes: vec![context_graph_mejepa::FailureModeClass::WrongAlgorithm],
            prediction,
            gtau_passed: false,
            approved: false,
            live_prediction_readback: false,
            latency_ms: 0.0,
        };
        let len_before = queue.entries.len();
        queue.enqueue_uncertain(&obs, 2, 0.0)?;
        if queue.entries.len() == len_before {
            suppressed_uncertainty += 1;
        }
    }
    eval_store.persist_queue(&queue)?;
    let after = queue_entry_count(eval_store)?;
    Ok(CaseEvidence {
        case: "active_pause_100_verify_calls_queue_unchanged".to_string(),
        trigger:
            "100 active paused verify calls plus non-zero surprise/uncertainty enqueue attempts"
                .to_string(),
        expected: json!({
            "pausedVerifies": 100,
            "suppressedSurprises": 100,
            "suppressedUncertainty": 100,
            "queueEntriesDelta": 0
        }),
        actual: json!({
            "pausedVerifies": paused_verifies,
            "suppressedSurprises": suppressed_surprises,
            "suppressedUncertainty": suppressed_uncertainty,
            "queueEntriesDelta": after.saturating_sub(before)
        }),
        queue_entries_before: before,
        queue_entries_after: after,
        pass: before == after
            && paused_verifies == 100
            && suppressed_surprises == 100
            && suppressed_uncertainty == 100,
    })
}

fn expired_pause_allows_queue(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    eval_store: &RocksDbEvalStore,
    pause_path: &Path,
    repo_root: &Path,
    patch: &PatchBundle,
) -> Result<CaseEvidence> {
    write_pause_state_readback(
        pause_path,
        &pause_state(
            current_unix_ms() - 900_000,
            current_unix_ms() - 1_000,
            "expired",
        ),
    )?;
    normal_verify_enqueues(
        "expired_pause_allows_agent_surprise_enqueue",
        compiler,
        eval_store,
        repo_root,
        patch,
        "expired",
    )
}

fn missing_pause_allows_queue(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    eval_store: &RocksDbEvalStore,
    pause_path: &Path,
    repo_root: &Path,
    patch: &PatchBundle,
) -> Result<CaseEvidence> {
    if pause_path.exists() {
        fs::remove_file(pause_path)?;
    }
    normal_verify_enqueues(
        "missing_pause_allows_agent_surprise_enqueue",
        compiler,
        eval_store,
        repo_root,
        patch,
        "missing",
    )
}

fn malformed_pause_allows_queue(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    eval_store: &RocksDbEvalStore,
    pause_path: &Path,
    repo_root: &Path,
    patch: &PatchBundle,
) -> Result<CaseEvidence> {
    write_bytes_readback(pause_path, b"{")?;
    let warning_code = match read_pause_state(pause_path, current_unix_ms()) {
        PauseReadOutcome::IgnoredInvalid { warning } => warning.code,
        other => return Err(anyhow!("expected malformed pause warning, got {other:?}")),
    };
    let mut case = normal_verify_enqueues(
        "malformed_pause_warns_and_allows_agent_surprise_enqueue",
        compiler,
        eval_store,
        repo_root,
        patch,
        "malformed",
    )?;
    case.actual["warningCode"] = json!(warning_code);
    case.expected["warningCode"] = json!("MEJEPA_PAUSE_STATE_DESERIALIZE_FAILED");
    case.pass = case.pass && case.actual["warningCode"] == case.expected["warningCode"];
    Ok(case)
}

fn normal_verify_enqueues(
    case: &str,
    compiler: &context_graph_mejepa::MeJepaCompiler,
    eval_store: &RocksDbEvalStore,
    repo_root: &Path,
    patch: &PatchBundle,
    suffix: &str,
) -> Result<CaseEvidence> {
    let before = queue_entry_count(eval_store)?;
    let context = build_context(repo_root, suffix)?;
    let verdict = compiler.verify(patch, &context)?;
    let prediction = match verdict {
        VerifyVerdict::Approve {
            reality_prediction, ..
        } => reality_prediction,
        other => return Err(anyhow!("expected approved verify, got {other:?}")),
    };
    let mut queue = eval_store
        .load_queue()?
        .ok_or_else(|| anyhow!("active-learning queue missing before normal enqueue"))?;
    let queued =
        queue.enqueue_agent_surprise_for_prediction(&prediction, SurpriseSeverity::High)?;
    eval_store.persist_queue(&queue)?;
    let after = queue_entry_count(eval_store)?;
    Ok(CaseEvidence {
        case: case.to_string(),
        trigger: "normal verify followed by agent surprise enqueue".to_string(),
        expected: json!({"queued": true, "queueEntriesDelta": 1}),
        actual: json!({"queued": queued, "queueEntriesDelta": after.saturating_sub(before)}),
        queue_entries_before: before,
        queue_entries_after: after,
        pass: queued && after == before + 1,
    })
}

fn seed_queue(eval_store: &RocksDbEvalStore) -> Result<()> {
    let mut queue = ActiveLearningQueueState::new(4096)?;
    queue.enqueue_agent_surprise(
        TaskId("phase-f-pause-active-learning-sentinel".to_string()),
        PredictionId([0x96; 16]),
        SurpriseSeverity::Low,
    )?;
    eval_store.persist_queue(&queue)?;
    Ok(())
}

fn build_compiler(
    repo_root: &Path,
    pause_path: &Path,
    db: Arc<DB>,
) -> Result<context_graph_mejepa::MeJepaCompiler> {
    let calibration = seed_calibration(db.clone())?;
    let store = Arc::new(RocksDbInferStore::new(db));
    Ok(build_deterministic_compiler(
        repo_root.to_path_buf(),
        store,
        calibration,
        MeJepaInferConfig {
            outcome_set_max: 4,
            pause_state_path: Some(pause_path.to_path_buf()),
            ..MeJepaInferConfig::default()
        },
    )?)
}

fn seed_calibration(db: Arc<DB>) -> Result<CalibrationStore> {
    let calibration_store = CalibrationStore::new(db, 30)?;
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![0.50],
            actual_test_pass: vec![if idx % 2 == 0 { 1.0 } else { 0.0 }],
        })
        .collect::<Vec<_>>();
    let norm_sq = examples
        .iter()
        .map(|example| {
            let score =
                non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)?;
            Ok(score * score)
        })
        .collect::<Result<Vec<_>, context_graph_mejepa::MejepaInferError>>()?;
    calibration_store.calibrate(
        &examples,
        &norm_sq,
        0.10,
        30,
        0.30,
        [96; 32],
        BTreeMap::new(),
    )?;
    Ok(calibration_store)
}

fn build_patch(repo_root: &Path, file_name: &str, source: &str) -> Result<PatchBundle> {
    let rel_path = PathBuf::from("src").join(file_name);
    let abs_path = repo_root.join(&rel_path);
    write_bytes_readback(&abs_path, source.as_bytes())?;
    PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: rel_path,
                pre_sha: [0; 32],
                post_sha: sha256_bytes(source.as_bytes()),
                before: String::new(),
                after: source.to_string(),
            }],
        },
        valid_witness_segment(),
        "Phase F pause active-learning suppression".to_string(),
        sha256_bytes(source.as_bytes()),
    )
    .map_err(Into::into)
}

fn build_context(repo_root: &Path, suffix: &str) -> Result<TaskContext> {
    let context = TaskContext {
        task_id: TaskId(format!("phase-f-pause-active-learning-{suffix}")),
        session_id: [0x96; 16],
        language: Language::Python,
        problem_statement: "Phase F pause suppresses active-learning queue noise".to_string(),
        tests: vec![TestId("test_pause_active_learning".to_string())],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    context.validate()?;
    Ok(context)
}

fn queue_entry_count(eval_store: &RocksDbEvalStore) -> Result<usize> {
    Ok(eval_store
        .load_queue()?
        .map(|queue| queue.entries.len())
        .unwrap_or(0))
}

fn count_queue_cf(db: &DB) -> Result<BTreeMap<String, u64>> {
    let mut counts = BTreeMap::new();
    counts.insert(
        CF_MEJEPA_ACTIVE_LEARNING_QUEUE.to_string(),
        count_cf(db, CF_MEJEPA_ACTIVE_LEARNING_QUEUE)?,
    );
    Ok(counts)
}

fn pause_state(set_at_unix_ms: i64, paused_until_unix_ms: i64, reason: &str) -> PauseState {
    PauseState {
        paused_until_unix_ms,
        set_at_unix_ms,
        reason: reason.to_string(),
        source: "phase_f_pause_active_learning_fsv".to_string(),
    }
}

fn write_pause_state_readback(path: &Path, state: &PauseState) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(state)?;
    write_bytes_readback(path, &bytes)?;
    let readback: PauseState = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(readback == *state, "pause-state readback mismatch");
    Ok(())
}

fn write_bytes_readback(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
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

fn flush_infer_db(db: &DB) -> Result<()> {
    for cf_name in MEJEPA_INFER_CFS {
        if let Some(cf) = db.cf_handle(cf_name) {
            db.flush_cf(cf)?;
        }
    }
    Ok(())
}

fn remove_existing_fsv_root(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let allowed =
        path.starts_with("/mnt/d/contextgraph/fsv/") || path.starts_with(std::env::temp_dir());
    ensure!(allowed, "refusing to remove FSV root {}", path.display());
    fs::remove_dir_all(path).with_context(|| format!("remove {}", path.display()))
}

fn collect_file_artifacts(path: &Path) -> Result<Vec<FileArtifact>> {
    let mut out = Vec::new();
    collect_file_artifacts_inner(path, &mut out)?;
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
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_flywheel_012_pause_active_learning_fsv_passes() {
        let fsv_root = default_fsv_root();
        let report_path = default_report_path(&fsv_root);
        let report = run_pause_active_learning_fsv(&fsv_root).expect("pause active-learning FSV");
        write_report_with_readback(&report, &report_path).expect("write pause active-learning FSV");
        assert!(report.all_passed);
        assert!(report.readback_equal);
        assert_eq!(report.happy_path.len(), 1);
        assert!(report.boundary_cases.len() >= 3);
    }
}
