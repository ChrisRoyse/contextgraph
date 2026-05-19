use anyhow::{ensure, Result};
use context_graph_mejepa::calibration::open_infer_rocksdb;
use context_graph_mejepa::cli::{build_deterministic_compiler, write_json_0600};
use context_graph_mejepa::conformal::{non_conformity_score, CalibrationExample};
use context_graph_mejepa::gates::{sha256_bytes, valid_witness_segment};
use context_graph_mejepa::{
    materialize_inference_panels, AstDiff, CalibrationStore, DiffHunk, Language, MeJepaInferConfig,
    PatchBundle, RocksDbInferStore, TaskContext, TaskEnvironment, TaskId, TestId, VerifyVerdict,
};
use context_graph_mejepa_instruments::InstrumentSlot;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-124-scenario-strings-inert-fsv";
const REPORT_NAME: &str = "scenario_strings_inert_fsv.json";

fn main() -> Result<()> {
    let root = std::env::var("CG_MEJEPA_TASK_PY_G_124_FSV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_FSV_ROOT));
    remove_if_exists(&root)?;
    fs::create_dir_all(&root)?;
    let repo_root = root.join("repo");
    fs::create_dir_all(repo_root.join("src"))?;
    let db_path = root.join("infer.rocksdb");

    let db = open_infer_rocksdb(&db_path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let examples = calibration_examples();
    let norm_sq = examples
        .iter()
        .map(|example| {
            let score =
                non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)?;
            Ok(score * score)
        })
        .collect::<Result<Vec<_>, context_graph_mejepa::MejepaInferError>>()?;
    calibration.calibrate(
        &examples,
        &norm_sq,
        0.10,
        30,
        0.30,
        [8; 32],
        BTreeMap::new(),
    )?;

    let store = Arc::new(RocksDbInferStore::new(db));
    let compiler = build_deterministic_compiler(
        repo_root.clone(),
        store,
        calibration,
        MeJepaInferConfig {
            outcome_set_max: 1,
            pause_state_path: None,
            ..MeJepaInferConfig::default()
        },
    )?;
    let patch = patch_bundle()?;
    let neutral_context = task_context(
        &repo_root,
        "task-scenario-neutral",
        "verify the answer increment changes by one",
    );
    let scenario_context = task_context(
        &repo_root,
        "task-scenario-magic-strings",
        "scenario: ood\nscenario: low_confidence\nscenario: constellation_reject",
    );

    let (_, neutral_t1, neutral_t2) = materialize_inference_panels(&patch, &neutral_context)?;
    let (_, scenario_t1, scenario_t2) = materialize_inference_panels(&patch, &scenario_context)?;
    let diff_t1_equal =
        neutral_t1.slot(InstrumentSlot::EDiff) == scenario_t1.slot(InstrumentSlot::EDiff);
    let diff_t2_equal =
        neutral_t2.slot(InstrumentSlot::EDiff) == scenario_t2.slot(InstrumentSlot::EDiff);
    let problem_slot_changed =
        neutral_t1.slot(InstrumentSlot::EProblem) != scenario_t1.slot(InstrumentSlot::EProblem);

    let neutral_verdict = verdict_name(compiler.verify(&patch, &neutral_context)?);
    let scenario_verdict = verdict_name(compiler.verify(&patch, &scenario_context)?);
    let source = fs::read_to_string("crates/context-graph-mejepa/src/compiler.rs")?;
    let production_scenario_contains = source.contains(".problem_statement")
        && source.contains("contains(\"scenario:")
        && source.contains("ConstellationGuard for DeterministicConstellationGuard");

    let all_passed = diff_t1_equal
        && diff_t2_equal
        && problem_slot_changed
        && neutral_verdict == scenario_verdict
        && !production_scenario_contains;
    let report_path = root.join(REPORT_NAME);
    let report = json!({
        "schema_version": 1,
        "task_id": "TASK-PY-G-124",
        "issue": 425,
        "source_of_truth": {
            "db_path": db_path,
            "report_path": report_path,
        },
        "panel_checks": {
            "diff_t1_equal": diff_t1_equal,
            "diff_t2_equal": diff_t2_equal,
            "problem_slot_changed_as_typed_evidence": problem_slot_changed,
        },
        "verdict_checks": {
            "neutral": neutral_verdict,
            "scenario_strings": scenario_verdict,
            "same": neutral_verdict == scenario_verdict,
        },
        "source_checks": {
            "production_scenario_contains": production_scenario_contains,
        },
        "all_passed": all_passed,
    });
    write_json_0600(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(readback == report, "FSV readback mismatch");
    ensure!(all_passed, "scenario string inertness FSV failed");
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn patch_bundle() -> Result<PatchBundle> {
    let before = "def answer(value: int) -> int:\n    return value + 1\n";
    let after = "def answer(value: int) -> int:\n    return value + 2\n";
    Ok(PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: PathBuf::from("src/reality.py"),
                pre_sha: sha256_bytes(before.as_bytes()),
                post_sha: sha256_bytes(after.as_bytes()),
                before: before.to_string(),
                after: after.to_string(),
            }],
        },
        valid_witness_segment(),
        "change answer increment".to_string(),
        sha256_bytes(b"scenario-inert-patch"),
    )?)
}

fn task_context(repo_root: &Path, task_id: &str, problem_statement: &str) -> TaskContext {
    TaskContext {
        task_id: TaskId(task_id.to_string()),
        session_id: [4; 16],
        language: Language::Python,
        problem_statement: problem_statement.to_string(),
        tests: vec![TestId("tests/test_reality.py::test_answer".to_string())],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: "linux".to_string(),
        },
        claim_graph: None,
        skill_citations: vec![],
    }
}

fn calibration_examples() -> Vec<CalibrationExample> {
    (0..30)
        .map(|idx| {
            let pass = idx % 3 != 0;
            CalibrationExample {
                language: Language::Python,
                predicted_test_pass: vec![if pass { 0.90 } else { 0.10 }],
                actual_test_pass: vec![if pass { 1.0 } else { 0.0 }],
            }
        })
        .collect()
}

fn verdict_name(verdict: VerifyVerdict) -> &'static str {
    match verdict {
        VerifyVerdict::Approve { .. } => "Approve",
        VerifyVerdict::EscalateToHuman { .. } => "EscalateToHuman",
    }
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    ensure!(
        path.starts_with("/zfs/archive/contextgraph/fsv/"),
        "refusing to remove path outside aiwonder FSV root: {}",
        path.display()
    );
    fs::remove_dir_all(path)?;
    Ok(())
}
