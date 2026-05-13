use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, ensure, Context, Result};
use context_graph_mejepa::calibration::count_cf;
use context_graph_mejepa::conformal::non_conformity_score;
use context_graph_mejepa::gates::{sha256_bytes, valid_witness_segment};
use context_graph_mejepa::{
    build_deterministic_compiler, current_unix_ms, open_infer_rocksdb, read_pause_state, AstDiff,
    CalibrationExample, CalibrationStore, DiffHunk, FailedGate, Language, MeJepaInferConfig,
    OracleOutcome, PatchBundle, PauseReadOutcome, PauseState, RocksDbInferStore, TaskContext,
    TaskEnvironment, TaskId, TestId, VerifyVerdict, CF_MEJEPA_LIVE_PREDICTIONS, MEJEPA_INFER_CFS,
    PAUSE_REASON_CODE,
};
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const TASK_ID: &str = "TASK-PREDICT-013";
const GOOD_SOURCE: &str =
    "def normalize(values):\n    return [value.strip() for value in values]\n";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PauseConsumerFsvReport {
    pub fsv_root: String,
    pub task_id: String,
    pub build_release_sha: String,
    pub source_of_truth: PauseConsumerSourceOfTruth,
    pub happy_path: Vec<PauseCaseEvidence>,
    pub boundary_cases: Vec<PauseCaseEvidence>,
    pub cf_counts_before: BTreeMap<String, u64>,
    pub cf_counts_after: BTreeMap<String, u64>,
    pub physical_artifacts: Vec<FileArtifact>,
    pub readback_equal: bool,
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PauseConsumerSourceOfTruth {
    pub pause_state_path: String,
    pub infer_db_path: String,
    pub live_prediction_cf: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PauseCaseEvidence {
    pub case: String,
    pub pause_file_before: Value,
    pub trigger: String,
    pub expected: Value,
    pub actual: Value,
    pub pause_file_after: Value,
    pub live_rows_before: u64,
    pub live_rows_after: u64,
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
    PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-pause-consumer-fsv")
}

pub fn default_report_path(fsv_root: &Path) -> PathBuf {
    fsv_root.join("pause_consumer_fsv.json")
}

pub fn run_pause_consumer_fsv(fsv_root: &Path) -> Result<PauseConsumerFsvReport> {
    remove_existing_fsv_root(fsv_root)?;
    fs::create_dir_all(fsv_root)?;
    let repo_root = fsv_root.join("repo");
    let db_path = fsv_root.join("infer-rocksdb");
    let pause_path = fsv_root.join("state/predictions_paused_until.json");
    fs::create_dir_all(repo_root.join("src"))?;
    let patch = build_patch(&repo_root, "pause_consumer.py", GOOD_SOURCE)?;
    let context = build_context(&repo_root, "happy")?;

    let cf_counts_before = {
        let db = open_infer_rocksdb(&db_path)?;
        count_live_predictions(db.as_ref())?
    };

    let (happy_path, boundary_cases, cf_counts_after) = {
        let db = open_infer_rocksdb(&db_path)?;
        let compiler = build_compiler(&repo_root, &pause_path, db.clone())?;
        let mut happy_path = Vec::new();
        let mut boundary_cases = Vec::new();

        happy_path.push(active_pause_rejects(
            &compiler,
            db.as_ref(),
            &pause_path,
            &patch,
            &context,
        )?);
        boundary_cases.push(expired_pause_allows_prediction(
            &compiler,
            db.as_ref(),
            &pause_path,
            &patch,
            &context,
        )?);
        boundary_cases.push(missing_pause_allows_prediction(
            &compiler,
            db.as_ref(),
            &pause_path,
            &patch,
            &context,
        )?);
        boundary_cases.push(malformed_pause_allows_prediction(
            &compiler,
            db.as_ref(),
            &pause_path,
            &patch,
            &context,
        )?);

        flush_infer_db(db.as_ref())?;
        (
            happy_path,
            boundary_cases,
            count_live_predictions(db.as_ref())?,
        )
    };

    let reopened_counts = {
        let db = open_infer_rocksdb(&db_path)?;
        count_live_predictions(db.as_ref())?
    };
    ensure!(
        reopened_counts == cf_counts_after,
        "pause consumer live prediction count changed after reopen"
    );

    let all_passed =
        happy_path.iter().all(|case| case.pass) && boundary_cases.iter().all(|case| case.pass);
    ensure!(
        all_passed,
        "pause consumer FSV contains failed case evidence"
    );

    Ok(PauseConsumerFsvReport {
        fsv_root: fsv_root.display().to_string(),
        task_id: TASK_ID.to_string(),
        build_release_sha: git_head_short(),
        source_of_truth: PauseConsumerSourceOfTruth {
            pause_state_path: pause_path.display().to_string(),
            infer_db_path: db_path.display().to_string(),
            live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        },
        happy_path,
        boundary_cases,
        cf_counts_before,
        cf_counts_after: reopened_counts,
        physical_artifacts: collect_file_artifacts(fsv_root)?,
        readback_equal: true,
        all_passed,
    })
}

pub fn write_report_with_readback(report: &PauseConsumerFsvReport, path: &Path) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    write_bytes_readback(path, &bytes)?;
    let readback: PauseConsumerFsvReport = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        readback == *report,
        "pause consumer FSV report readback mismatch"
    );
    Ok(())
}

fn active_pause_rejects(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    db: &DB,
    pause_path: &Path,
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<PauseCaseEvidence> {
    let rows_before = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    let state = pause_state(
        current_unix_ms() - 1_000,
        current_unix_ms() + 600_000,
        "active",
    );
    write_pause_state_readback(pause_path, &state)?;
    let before = pause_file_value(pause_path)?;
    let verdict = compiler.verify(patch, context)?;
    let rows_after = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    let (paused_until, prediction_verdict) = match &verdict {
        VerifyVerdict::EscalateToHuman {
            reality_prediction: Some(reality_prediction),
            failed_gate:
                FailedGate::PredictionPaused {
                    paused_until_unix_ms,
                    reason,
                },
            ..
        } => {
            ensure!(reason == PAUSE_REASON_CODE, "pause reason code mismatch");
            (*paused_until_unix_ms, reality_prediction.verdict)
        }
        other => return Err(anyhow!("expected PredictionPaused, got {other:?}")),
    };
    Ok(PauseCaseEvidence {
        case: "active_pause_abstains_before_prediction".to_string(),
        pause_file_before: before,
        trigger: "MeJepaCompiler::verify with active pause file".to_string(),
        expected: json!({
            "failedGate": "PredictionPaused",
            "predictionVerdict": "Abstain",
            "pausedUntilUnixMs": state.paused_until_unix_ms,
            "liveRowsDelta": 0
        }),
        actual: json!({
            "failedGate": "PredictionPaused",
            "predictionVerdict": format!("{prediction_verdict:?}"),
            "pausedUntilUnixMs": paused_until,
            "liveRowsDelta": rows_after.saturating_sub(rows_before)
        }),
        pause_file_after: pause_file_value(pause_path)?,
        live_rows_before: rows_before,
        live_rows_after: rows_after,
        pass: paused_until == state.paused_until_unix_ms
            && prediction_verdict == context_graph_mejepa::Verdict::Abstain
            && rows_after == rows_before,
    })
}

fn expired_pause_allows_prediction(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    db: &DB,
    pause_path: &Path,
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<PauseCaseEvidence> {
    let rows_before = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    let state = pause_state(
        current_unix_ms() - 600_000,
        current_unix_ms() - 1_000,
        "expired",
    );
    write_pause_state_readback(pause_path, &state)?;
    let before = pause_file_value(pause_path)?;
    let verdict = compiler.verify(patch, context)?;
    let rows_after = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    let approved = matches!(verdict, VerifyVerdict::Approve { .. });
    Ok(PauseCaseEvidence {
        case: "expired_pause_allows_normal_verdict".to_string(),
        pause_file_before: before,
        trigger: "MeJepaCompiler::verify with expired pause file".to_string(),
        expected: json!({"approved": true, "liveRowsDelta": 1}),
        actual: json!({"approved": approved, "liveRowsDelta": rows_after.saturating_sub(rows_before)}),
        pause_file_after: pause_file_value(pause_path)?,
        live_rows_before: rows_before,
        live_rows_after: rows_after,
        pass: approved && rows_after == rows_before + 1,
    })
}

fn missing_pause_allows_prediction(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    db: &DB,
    pause_path: &Path,
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<PauseCaseEvidence> {
    let rows_before = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    if pause_path.exists() {
        fs::remove_file(pause_path)?;
    }
    let verdict = compiler.verify(patch, context)?;
    let rows_after = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    let approved = matches!(verdict, VerifyVerdict::Approve { .. });
    Ok(PauseCaseEvidence {
        case: "missing_pause_file_allows_normal_verdict".to_string(),
        pause_file_before: json!({"exists": false}),
        trigger: "MeJepaCompiler::verify with no pause file".to_string(),
        expected: json!({"approved": true, "liveRowsDelta": 1}),
        actual: json!({"approved": approved, "liveRowsDelta": rows_after.saturating_sub(rows_before)}),
        pause_file_after: json!({"exists": pause_path.exists()}),
        live_rows_before: rows_before,
        live_rows_after: rows_after,
        pass: approved && rows_after == rows_before + 1,
    })
}

fn malformed_pause_allows_prediction(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    db: &DB,
    pause_path: &Path,
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<PauseCaseEvidence> {
    let rows_before = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    write_bytes_readback(pause_path, b"{")?;
    let before = pause_file_value(pause_path)?;
    let warning = match read_pause_state(pause_path, current_unix_ms()) {
        PauseReadOutcome::IgnoredInvalid { warning } => warning,
        other => return Err(anyhow!("expected malformed pause warning, got {other:?}")),
    };
    let verdict = compiler.verify(patch, context)?;
    let rows_after = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    let approved = matches!(verdict, VerifyVerdict::Approve { .. });
    Ok(PauseCaseEvidence {
        case: "malformed_pause_file_warns_and_allows_normal_verdict".to_string(),
        pause_file_before: before,
        trigger: "MeJepaCompiler::verify with malformed pause file".to_string(),
        expected: json!({
            "warningCode": "MEJEPA_PAUSE_STATE_DESERIALIZE_FAILED",
            "approved": true,
            "liveRowsDelta": 1
        }),
        actual: json!({
            "warningCode": warning.code,
            "approved": approved,
            "liveRowsDelta": rows_after.saturating_sub(rows_before)
        }),
        pause_file_after: pause_file_value(pause_path)?,
        live_rows_before: rows_before,
        live_rows_after: rows_after,
        pass: warning.code == "MEJEPA_PAUSE_STATE_DESERIALIZE_FAILED"
            && approved
            && rows_after == rows_before + 1,
    })
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
        [13; 32],
        BTreeMap::new(),
    )?;
    Ok(calibration_store)
}

fn build_patch(repo_root: &Path, file_name: &str, source: &str) -> Result<PatchBundle> {
    let rel_path = PathBuf::from("src").join(file_name);
    let abs_path = repo_root.join(&rel_path);
    write_bytes_readback(&abs_path, source.as_bytes())?;
    let post_sha = sha256_bytes(source.as_bytes());
    PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: rel_path,
                pre_sha: [0; 32],
                post_sha,
                before: String::new(),
                after: source.to_string(),
            }],
        },
        valid_witness_segment(),
        "Phase F pause consumer".to_string(),
        sha256_bytes(source.as_bytes()),
    )
    .map_err(Into::into)
}

fn build_context(repo_root: &Path, suffix: &str) -> Result<TaskContext> {
    let context = TaskContext {
        task_id: TaskId(format!("phase-f-pause-consumer-{suffix}")),
        session_id: [0x82; 16],
        language: Language::Python,
        problem_statement: "Phase F pause-state consumer verifies operator pause".to_string(),
        tests: vec![TestId("test_pause_consumer".to_string())],
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

fn pause_state(set_at_unix_ms: i64, paused_until_unix_ms: i64, reason: &str) -> PauseState {
    PauseState {
        paused_until_unix_ms,
        set_at_unix_ms,
        reason: reason.to_string(),
        source: "phase_f_pause_consumer_fsv".to_string(),
    }
}

fn write_pause_state_readback(path: &Path, state: &PauseState) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(state)?;
    write_bytes_readback(path, &bytes)?;
    let readback: PauseState = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(readback == *state, "pause-state readback mismatch");
    Ok(())
}

fn pause_file_value(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({"exists": false}));
    }
    let bytes = fs::read(path)?;
    Ok(json!({
        "exists": true,
        "bytes": bytes.len(),
        "sha256": hex::encode(Sha256::digest(&bytes)),
        "json": serde_json::from_slice::<Value>(&bytes).ok()
    }))
}

fn count_live_predictions(db: &DB) -> Result<BTreeMap<String, u64>> {
    let mut counts = BTreeMap::new();
    counts.insert(
        CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?,
    );
    Ok(counts)
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

fn remove_existing_fsv_root(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let temp_dir = std::env::temp_dir();
    let allowed = path.starts_with("/mnt/d/contextgraph/fsv/") || path.starts_with(&temp_dir);
    ensure!(
        allowed,
        "refusing to remove pause consumer FSV root outside /mnt/d/contextgraph/fsv or temp dir: {}",
        path.display()
    );
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
    fn task_predict_013_pause_consumer_fsv_passes() {
        let fsv_root = default_fsv_root();
        let report_path = default_report_path(&fsv_root);
        let report = run_pause_consumer_fsv(&fsv_root).expect("pause consumer FSV");
        write_report_with_readback(&report, &report_path).expect("write pause consumer FSV");
        assert!(report.all_passed);
        assert!(report.readback_equal);
        assert_eq!(report.happy_path.len(), 1);
        assert!(report.boundary_cases.len() >= 3);
    }
}
