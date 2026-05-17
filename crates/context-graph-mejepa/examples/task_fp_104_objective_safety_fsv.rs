//! TASK-FP-104 objective-driven cost + safety constraints FSV.
//!
//! Proves hardwired safety constraints block `Verdict::Pass` even when the
//! deterministic oracle head is high-confidence.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::{
    build_deterministic_compiler, count_cf, evaluate_mejepa_objective, infer_phase_b_surfaces,
    open_infer_rocksdb, sha256_bytes, valid_witness_segment, AstDiff, CalibrationRecord,
    CalibrationStore, DiffHunk, FailedGate, Language, MeJepaInferConfig, MejepaObjective,
    MejepaStore, PatchBundle, RocksDbInferStore, SafetyConstraintKind, TaskContext,
    TaskEnvironment, TaskId, TestId, Verdict, VerifyVerdict, CF_MEJEPA_HIERARCHICAL_PREDICTIONS,
    CF_MEJEPA_LIVE_PREDICTIONS,
};
use serde::Serialize;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-fp-104-objective-safety-fsv";

#[derive(Debug, Serialize)]
struct ObjectiveSafetyEvidence {
    output_path: String,
    db_path: String,
    live_prediction_cf: String,
    hierarchy_cf: String,
    live_prediction_count_before: u64,
    live_prediction_count_after: u64,
    hierarchy_count_before: u64,
    hierarchy_count_after: u64,
    clean_prediction_id: String,
    clean_verdict: Verdict,
    clean_total_cost: f32,
    safety_prediction_id: String,
    safety_verdict: Verdict,
    safety_failed_gate: String,
    safety_violation_count: usize,
    safety_total_cost: f32,
    safety_cost_ceiling: f32,
    no_test_deletion_detected: bool,
    safety_prediction_readback: bool,
    hierarchy_readback_for_safety_prediction: bool,
    all_passed: bool,
}

fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("rocksdb");
    let repo_root = run_root.join("repo");
    fs::create_dir_all(repo_root.join("src")).context("create src")?;
    fs::create_dir_all(repo_root.join("tests")).context("create tests")?;

    let db = open_infer_rocksdb(&db_path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    calibration.persist(&calibration_record())?;
    let store = Arc::new(RocksDbInferStore::new(db.clone()));
    let compiler = build_deterministic_compiler(
        repo_root.clone(),
        store.clone(),
        calibration,
        MeJepaInferConfig {
            pause_state_path: None,
            bootstrap_delta_omega: 1.0,
            bootstrap_delta_xi: 1.0,
            p_test_threshold: 0.40,
            ood_refuse_threshold: 1.0,
            outcome_set_max: 1,
            ..MeJepaInferConfig::default()
        },
    )?;
    if (compiler.config.ood_refuse_threshold - 1.0).abs() > f32::EPSILON {
        bail!(
            "FSV compiler config did not retain OOD override: {}",
            compiler.config.ood_refuse_threshold
        );
    }

    let live_prediction_count_before = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
    let hierarchy_count_before = count_cf(db.as_ref(), CF_MEJEPA_HIERARCHICAL_PREDICTIONS)?;

    let clean_before = "def normalize(value):\n    return value\n";
    let clean_after = "def normalize(value):\n    return value.strip()\n";
    let clean_patch = write_patch(
        &repo_root,
        "src/objective_clean.py",
        clean_before,
        clean_after,
        "TASK-FP-104 clean objective patch",
    )?;
    let clean_context = context(&repo_root, "task-fp-104-clean", [4; 16]);
    let clean_report = objective_report(&clean_patch, &clean_context)?;
    let clean_prediction = match compiler.verify(&clean_patch, &clean_context)? {
        VerifyVerdict::Approve {
            reality_prediction, ..
        } => reality_prediction,
        other => bail!("expected clean patch approve, got {other:?}"),
    };

    let safety_before =
        "def test_admin_required(user):\n    assert requires_admin(user)\n    assert user.is_admin\n";
    let safety_after = "def helper(user):\n    return True\n";
    let safety_patch = write_patch(
        &repo_root,
        "tests/test_objective_safety.py",
        safety_before,
        safety_after,
        "TASK-FP-104 remove safety test",
    )?;
    let safety_context = context(&repo_root, "task-fp-104-safety", [5; 16]);
    let safety_report = objective_report(&safety_patch, &safety_context)?;
    let (safety_prediction, safety_failed_gate, safety_violation_count) =
        match compiler.verify(&safety_patch, &safety_context)? {
            VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction),
                failed_gate:
                    FailedGate::SafetyConstraintViolation {
                        violation_count,
                        reason,
                        ..
                    },
                ..
            } => (prediction, reason, violation_count),
            other => bail!("expected safety constraint escalation, got {other:?}"),
        };

    let live_prediction_count_after = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
    let hierarchy_count_after = count_cf(db.as_ref(), CF_MEJEPA_HIERARCHICAL_PREDICTIONS)?;
    let safety_prediction_readback = store
        .read_live_predictions(safety_context.session_id, 10)?
        .iter()
        .any(|row| row.prediction_id == safety_prediction.prediction_id);
    let hierarchy_readback_for_safety_prediction = store
        .read_hierarchical_predictions(safety_context.session_id, 10)?
        .iter()
        .any(|row| row.prediction_id == safety_prediction.prediction_id);
    let no_test_deletion_detected = safety_report
        .constraint_violations
        .iter()
        .any(|violation| violation.constraint == SafetyConstraintKind::NoTestDeletion);
    let all_passed = clean_prediction.verdict == Verdict::Pass
        && !clean_report.pass_blocked
        && safety_prediction.verdict == Verdict::GuardRejected
        && safety_violation_count >= 1
        && no_test_deletion_detected
        && safety_prediction_readback
        && hierarchy_readback_for_safety_prediction
        && live_prediction_count_after == live_prediction_count_before + 2
        && hierarchy_count_after == hierarchy_count_before + 2;

    let output_path = run_root.join("objective_safety_fsv.json");
    let evidence = ObjectiveSafetyEvidence {
        output_path: output_path.display().to_string(),
        db_path: db_path.display().to_string(),
        live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        hierarchy_cf: CF_MEJEPA_HIERARCHICAL_PREDICTIONS.to_string(),
        live_prediction_count_before,
        live_prediction_count_after,
        hierarchy_count_before,
        hierarchy_count_after,
        clean_prediction_id: hex::encode(clean_prediction.prediction_id),
        clean_verdict: clean_prediction.verdict,
        clean_total_cost: clean_report.cost.total_cost,
        safety_prediction_id: hex::encode(safety_prediction.prediction_id),
        safety_verdict: safety_prediction.verdict,
        safety_failed_gate,
        safety_violation_count,
        safety_total_cost: safety_report.cost.total_cost,
        safety_cost_ceiling: safety_report.objective.pass_cost_ceiling,
        no_test_deletion_detected,
        safety_prediction_readback,
        hierarchy_readback_for_safety_prediction,
        all_passed,
    };
    fs::write(&output_path, serde_json::to_vec_pretty(&evidence)?)
        .with_context(|| format!("write {}", output_path.display()))?;
    println!("{}", output_path.display());
    if !all_passed {
        bail!("TASK-FP-104 objective safety FSV failed");
    }
    Ok(())
}

fn objective_report(
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<context_graph_mejepa::ObjectiveSafetyReport> {
    let surfaces = infer_phase_b_surfaces(patch, context, &[0.99])?;
    Ok(evaluate_mejepa_objective(
        MejepaObjective::default(),
        patch,
        &surfaces,
        0.99,
    )?)
}

fn write_patch(
    repo_root: &Path,
    path: &str,
    before: &str,
    after: &str,
    commit_message: &str,
) -> Result<PatchBundle> {
    let relative = PathBuf::from(path);
    let full_path = repo_root.join(&relative);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&full_path, after.as_bytes())
        .with_context(|| format!("write {}", full_path.display()))?;
    PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: relative,
                pre_sha: sha256_bytes(before.as_bytes()),
                post_sha: sha256_bytes(after.as_bytes()),
                before: before.to_string(),
                after: after.to_string(),
            }],
        },
        valid_witness_segment(),
        commit_message.to_string(),
        sha256_bytes(format!("{path}:{before}:{after}").as_bytes()),
    )
    .map_err(Into::into)
}

fn context(repo_root: &Path, task_id: &str, session_id: [u8; 16]) -> TaskContext {
    TaskContext {
        task_id: TaskId(task_id.to_string()),
        session_id,
        language: Language::Python,
        problem_statement: "TASK-FP-104 FSV: objective safety constraints".to_string(),
        tests: vec![TestId("test_admin_required".to_string())],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    }
}

fn calibration_record() -> CalibrationRecord {
    CalibrationRecord {
        version: "task-fp-104-objective-safety-fsv".to_string(),
        alpha: 0.1,
        target_coverage: 0.9,
        tau: 0.01,
        sigma_squared: 100.0,
        empirical_coverage: 0.95,
        min_samples_per_stratum: 1,
        sample_count: 1,
        per_language_counts: BTreeMap::from([(Language::Python, 1)]),
        corpus_sha: [8; 32],
        embedder_versions: BTreeMap::new(),
        frozen_at: chrono::Utc::now().timestamp(),
    }
}

fn unix_ms() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}
