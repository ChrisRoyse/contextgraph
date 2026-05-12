use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use context_graph_mejepa::calibration::{
    count_cf, CF_MEJEPA_CALIBRATION_HISTORY, CF_MEJEPA_LIVE_PREDICTIONS,
};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::conformal::{non_conformity_score, CalibrationExample};
use context_graph_mejepa::gates::{sha256_bytes, valid_witness_segment};
use context_graph_mejepa::{
    build_deterministic_compiler, open_infer_rocksdb, AstDiff, CalibrationStore, DiffHunk,
    Language, MeJepaInferConfig, MejepaStore, PatchBundle, RocksDbInferStore, TaskContext,
    TaskEnvironment, TaskId, TestId, VerifyVerdict,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

const FSV_DIR: &str = "/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PhaseBFsvEvidence {
    source_of_truth: SourceOfTruth,
    happy_path: HappyPathEvidence,
    edge_cases: Vec<EdgeCaseEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceOfTruth {
    db_path: String,
    live_prediction_cf: String,
    calibration_cf: String,
    physical_db_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HappyPathEvidence {
    before_live_prediction_count: u64,
    action: String,
    after_live_prediction_count: u64,
    persisted_prediction_id: String,
    persisted_verdict: String,
    covered_chunks: Vec<String>,
    security_concerns: usize,
    edge_cases: usize,
    tech_debt: usize,
    cost_regressions: usize,
    witness_hash: String,
    readback_task_id: String,
    readback_calibration_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeCaseEvidence {
    case_name: String,
    before_live_prediction_count: u64,
    action: String,
    expected: String,
    observed_error_code: String,
    after_live_prediction_count: u64,
    db_count_unchanged: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(FSV_DIR);
    let db_path = root.join("phase-b-rocksdb");
    let repo_root = root.join("repo");
    let _ = fs::remove_dir_all(&db_path);
    let _ = fs::remove_dir_all(&repo_root);
    fs::create_dir_all(&repo_root)?;
    fs::create_dir_all(&root)?;

    let db = open_infer_rocksdb(&db_path)?;
    let calibration_store = CalibrationStore::new(db.clone(), 30)?;
    let calibration_examples = calibration_examples();
    let norm_sq = calibration_examples
        .iter()
        .map(|example| {
            let score =
                non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)?;
            Ok(score * score)
        })
        .collect::<Result<Vec<_>, context_graph_mejepa::MejepaInferError>>()?;
    calibration_store.calibrate(
        &calibration_examples,
        &norm_sq,
        0.10,
        30,
        0.30,
        [7; 32],
        BTreeMap::new(),
    )?;

    let store = Arc::new(RocksDbInferStore::new(db.clone()));
    let compiler = build_deterministic_compiler(
        repo_root.clone(),
        store.clone(),
        calibration_store,
        MeJepaInferConfig {
            outcome_set_max: 1,
            ..MeJepaInferConfig::default()
        },
    )?;

    let before_live_prediction_count = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
    println!("happy_path before CF_MEJEPA_LIVE_PREDICTIONS={before_live_prediction_count}");
    let (patch, context) = happy_patch_context(&repo_root)?;
    let verdict = compiler.verify(&patch, &context)?;
    let persisted = match verdict {
        VerifyVerdict::Approve {
            reality_prediction, ..
        } => reality_prediction,
        other => return Err(format!("happy path did not approve: {other:?}").into()),
    };
    let loaded = store.read_live_predictions(context.session_id, 10)?;
    let readback = loaded
        .iter()
        .find(|row| row.prediction_id == persisted.prediction_id)
        .ok_or("persisted prediction not found by readback")?;
    let after_live_prediction_count = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
    println!(
        "happy_path action=compiler.verify persisted_id={}",
        hex::encode(readback.prediction_id)
    );
    println!("happy_path after CF_MEJEPA_LIVE_PREDICTIONS={after_live_prediction_count}");
    assert_eq!(
        after_live_prediction_count,
        before_live_prediction_count + 1
    );
    assert_eq!(readback, &persisted);
    assert_eq!(readback.covered_chunks.len(), 2);
    assert!(!readback.predicted_security_concerns.is_empty());
    assert!(!readback.predicted_edge_cases.is_empty());
    assert!(!readback.predicted_tech_debt_added.is_empty());
    assert!(!readback.predicted_cost_regressions.is_empty());

    let happy_path = HappyPathEvidence {
        before_live_prediction_count,
        action: "compiler.verify(candidate patch) then RocksDbInferStore.read_live_predictions"
            .to_string(),
        after_live_prediction_count,
        persisted_prediction_id: hex::encode(readback.prediction_id),
        persisted_verdict: format!("{:?}", readback.verdict),
        covered_chunks: readback
            .covered_chunks
            .iter()
            .map(|chunk| chunk.0.clone())
            .collect(),
        security_concerns: readback.predicted_security_concerns.len(),
        edge_cases: readback.predicted_edge_cases.len(),
        tech_debt: readback.predicted_tech_debt_added.len(),
        cost_regressions: readback.predicted_cost_regressions.len(),
        witness_hash: hex::encode(readback.witness_hash.0),
        readback_task_id: readback.task_id.0.clone(),
        readback_calibration_version: readback.calibration_version.clone(),
    };

    let edge_cases = vec![
        edge_case_empty_patch(db.as_ref())?,
        edge_case_path_traversal(db.as_ref())?,
        edge_case_bad_witness(db.as_ref())?,
    ];

    let source_of_truth = SourceOfTruth {
        db_path: db_path.display().to_string(),
        live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        calibration_cf: CF_MEJEPA_CALIBRATION_HISTORY.to_string(),
        physical_db_files: physical_db_files(&db_path)?,
    };
    let evidence = PhaseBFsvEvidence {
        source_of_truth,
        happy_path,
        edge_cases,
    };
    let evidence_path = root.join("phase-b-prediction-fsv.json");
    write_json_0600(&evidence_path, &evidence)?;
    let readback_evidence: PhaseBFsvEvidence = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    assert_eq!(readback_evidence, evidence);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "evidencePath": evidence_path,
            "persistedPredictionId": evidence.happy_path.persisted_prediction_id,
            "livePredictionCountAfter": evidence.happy_path.after_live_prediction_count,
            "edgeCases": evidence.edge_cases
        }))?
    );
    Ok(())
}

fn happy_patch_context(repo_root: &Path) -> Result<(PatchBundle, TaskContext), Box<dyn Error>> {
    let rel_src = PathBuf::from("src/phase_b_sample.py");
    let rel_manifest = PathBuf::from("requirements.txt");
    fs::create_dir_all(repo_root.join("src"))?;
    let src_after = "API_KEY = \"sk-phase-b-test\"\n# TODO: owner needed\n\
def normalize(values):\n    if values == [] or len(values) == 0:\n        return \"empty\"\n\
    digest = \"md5\"\n    return [v.clone() for v in values]\n";
    let manifest_after = "requests==2.32.3\n";
    fs::write(repo_root.join(&rel_src), src_after.as_bytes())?;
    fs::write(repo_root.join(&rel_manifest), manifest_after.as_bytes())?;
    let src_sha = sha256_bytes(src_after.as_bytes());
    let manifest_sha = sha256_bytes(manifest_after.as_bytes());
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![
                DiffHunk {
                    path: rel_src,
                    pre_sha: src_sha,
                    post_sha: src_sha,
                    before: String::new(),
                    after: src_after.to_string(),
                },
                DiffHunk {
                    path: rel_manifest,
                    pre_sha: manifest_sha,
                    post_sha: manifest_sha,
                    before: String::new(),
                    after: manifest_after.to_string(),
                },
            ],
        },
        valid_witness_segment(),
        "phase b fsv happy path".to_string(),
        sha256_bytes(b"phase-b-fsv-happy"),
    )?;
    let context = TaskContext {
        task_id: TaskId("phase-b-fsv-task".to_string()),
        session_id: [0x4b; 16],
        language: Language::Python,
        problem_statement: "scenario: approve; verify phase b surfaces".to_string(),
        tests: vec![
            TestId("test_empty_values".to_string()),
            TestId("test_secret_rejected".to_string()),
        ],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: vec![],
    };
    Ok((patch, context))
}

fn edge_case_empty_patch(db: &rocksdb::DB) -> Result<EdgeCaseEvidence, Box<dyn Error>> {
    let before = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    println!("edge empty_patch before={before}");
    let observed = PatchBundle::try_new(
        AstDiff { hunks: vec![] },
        valid_witness_segment(),
        "empty".to_string(),
        [1; 32],
    )
    .unwrap_err();
    let after = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    println!("edge empty_patch after={after} error={}", observed.code());
    Ok(EdgeCaseEvidence {
        case_name: "empty_patch".to_string(),
        before_live_prediction_count: before,
        action: "PatchBundle::try_new(AstDiff { hunks: [] })".to_string(),
        expected: "MEJEPA_INFER_INVALID_INPUT and no live prediction row".to_string(),
        observed_error_code: observed.code().to_string(),
        after_live_prediction_count: after,
        db_count_unchanged: before == after,
    })
}

fn edge_case_path_traversal(db: &rocksdb::DB) -> Result<EdgeCaseEvidence, Box<dyn Error>> {
    let before = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    println!("edge path_traversal before={before}");
    let observed = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: PathBuf::from("../escape.py"),
                pre_sha: [0; 32],
                post_sha: [0; 32],
                before: String::new(),
                after: "print('escape')".to_string(),
            }],
        },
        valid_witness_segment(),
        "path traversal".to_string(),
        [2; 32],
    )
    .unwrap_err();
    let after = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    println!(
        "edge path_traversal after={after} error={}",
        observed.code()
    );
    Ok(EdgeCaseEvidence {
        case_name: "path_traversal".to_string(),
        before_live_prediction_count: before,
        action: "PatchBundle::try_new(hunk path ../escape.py)".to_string(),
        expected: "MEJEPA_INFER_INVALID_INPUT and no live prediction row".to_string(),
        observed_error_code: observed.code().to_string(),
        after_live_prediction_count: after,
        db_count_unchanged: before == after,
    })
}

fn edge_case_bad_witness(db: &rocksdb::DB) -> Result<EdgeCaseEvidence, Box<dyn Error>> {
    let before = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    println!("edge bad_witness before={before}");
    let observed = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: PathBuf::from("valid.py"),
                pre_sha: [0; 32],
                post_sha: [0; 32],
                before: String::new(),
                after: "print('valid')".to_string(),
            }],
        },
        vec![1, 2, 3],
        "bad witness".to_string(),
        [3; 32],
    )
    .unwrap_err();
    let after = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    println!("edge bad_witness after={after} error={}", observed.code());
    Ok(EdgeCaseEvidence {
        case_name: "bad_witness_segment".to_string(),
        before_live_prediction_count: before,
        action: "PatchBundle::try_new(witness_chain_segment len 3)".to_string(),
        expected: "MEJEPA_INFER_DIM_MISMATCH and no live prediction row".to_string(),
        observed_error_code: observed.code().to_string(),
        after_live_prediction_count: after,
        db_count_unchanged: before == after,
    })
}

fn calibration_examples() -> Vec<CalibrationExample> {
    (0..100)
        .map(|idx| {
            let low = matches!(idx, 9 | 20 | 31 | 42 | 53 | 64 | 75 | 86 | 97);
            CalibrationExample {
                language: Language::Python,
                predicted_test_pass: vec![if low { 0.10 } else { 0.95 }],
                actual_test_pass: vec![1.0],
            }
        })
        .collect()
}

fn physical_db_files(db_path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut files = fs::read_dir(db_path)?
        .map(|entry| {
            let entry = entry?;
            Ok(entry.file_name().to_string_lossy().to_string())
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    files.sort();
    Ok(files)
}
