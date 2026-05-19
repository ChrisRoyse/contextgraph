//! TASK-FP-102 hierarchical JEPA predictor FSV.
//!
//! Proves `compiler.verify()` emits a durable file/function/AST/chunk
//! hierarchy row beside the existing live `RealityPrediction` source of truth.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::{
    build_deterministic_compiler, count_cf, open_infer_rocksdb, sha256_bytes,
    valid_witness_segment, AstDiff, CalibrationRecord, CalibrationStore, DiffHunk,
    HierarchicalPredictionRecord, Language, MeJepaInferConfig, MejepaStore, PatchBundle,
    PredictionHierarchyLevel, RocksDbInferStore, TaskContext, TaskEnvironment, TaskId, TestId,
    VerifyVerdict, CF_MEJEPA_HIERARCHICAL_PREDICTIONS, CF_MEJEPA_LIVE_PREDICTIONS,
};
use serde::Serialize;

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-fp-102-hierarchical-jepa-fsv";

#[derive(Debug, Serialize)]
struct HierarchyEvidence {
    output_path: String,
    db_path: String,
    live_prediction_cf: String,
    hierarchy_cf: String,
    live_prediction_count_before: u64,
    live_prediction_count_after: u64,
    hierarchy_count_before: u64,
    hierarchy_count_after: u64,
    prediction_id: String,
    level_count: usize,
    level_kinds: Vec<String>,
    file_levels: usize,
    function_levels: usize,
    ast_levels: usize,
    chunk_levels: usize,
    parent_links_validated: bool,
    live_readback_equal: bool,
    hierarchy_readback_equal: bool,
    all_passed: bool,
}

fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("rocksdb");
    let repo_root = run_root.join("repo");
    fs::create_dir_all(&repo_root).context("create repo root")?;
    fs::create_dir_all(&run_root).context("create FSV root")?;

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
            p_test_threshold: 0.70,
            outcome_set_max: 1,
            ..MeJepaInferConfig::default()
        },
    )?;

    let (patch, context) = patch_context(&repo_root)?;
    let live_prediction_count_before = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
    let hierarchy_count_before = count_cf(db.as_ref(), CF_MEJEPA_HIERARCHICAL_PREDICTIONS)?;
    let verdict = compiler.verify(&patch, &context)?;
    let prediction = match verdict {
        VerifyVerdict::Approve {
            reality_prediction, ..
        } => reality_prediction,
        other => bail!("expected approve, got {other:?}"),
    };

    let live_readback = store
        .read_live_predictions(context.session_id, 10)?
        .into_iter()
        .find(|row| row.prediction_id == prediction.prediction_id)
        .context("missing live prediction readback")?;
    let hierarchy_readback = store
        .read_hierarchical_predictions(context.session_id, 10)?
        .into_iter()
        .find(|row| row.prediction_id == prediction.prediction_id)
        .context("missing hierarchical prediction readback")?;
    hierarchy_readback.validate()?;

    let live_prediction_count_after = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
    let hierarchy_count_after = count_cf(db.as_ref(), CF_MEJEPA_HIERARCHICAL_PREDICTIONS)?;
    let level_kinds = level_kinds(&hierarchy_readback);
    let file_levels = count_level(&hierarchy_readback, PredictionHierarchyLevel::File);
    let function_levels = count_level(&hierarchy_readback, PredictionHierarchyLevel::Function);
    let ast_levels = count_level(&hierarchy_readback, PredictionHierarchyLevel::AstNode);
    let chunk_levels = count_level(&hierarchy_readback, PredictionHierarchyLevel::Chunk);
    let required_levels_present =
        file_levels >= 1 && function_levels >= 1 && ast_levels >= 1 && chunk_levels >= 1;
    let parent_links_validated = hierarchy_readback.validate().is_ok();
    let live_readback_equal = live_readback == prediction;
    let hierarchy_readback_equal = hierarchy_readback.prediction_id == prediction.prediction_id
        && hierarchy_readback.source_panel_sha == prediction.source_panel_sha
        && hierarchy_readback.calibration_version == prediction.calibration_version;
    let all_passed = live_prediction_count_after == live_prediction_count_before + 1
        && hierarchy_count_after == hierarchy_count_before + 1
        && required_levels_present
        && parent_links_validated
        && live_readback_equal
        && hierarchy_readback_equal;

    let output_path = run_root.join("hierarchical_jepa_fsv.json");
    let evidence = HierarchyEvidence {
        output_path: output_path.display().to_string(),
        db_path: db_path.display().to_string(),
        live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        hierarchy_cf: CF_MEJEPA_HIERARCHICAL_PREDICTIONS.to_string(),
        live_prediction_count_before,
        live_prediction_count_after,
        hierarchy_count_before,
        hierarchy_count_after,
        prediction_id: hex::encode(prediction.prediction_id),
        level_count: hierarchy_readback.levels.len(),
        level_kinds,
        file_levels,
        function_levels,
        ast_levels,
        chunk_levels,
        parent_links_validated,
        live_readback_equal,
        hierarchy_readback_equal,
        all_passed,
    };
    fs::write(&output_path, serde_json::to_vec_pretty(&evidence)?)
        .with_context(|| format!("write {}", output_path.display()))?;
    println!("{}", output_path.display());
    if !all_passed {
        bail!("TASK-FP-102 hierarchical JEPA FSV failed");
    }
    Ok(())
}

fn patch_context(repo_root: &Path) -> Result<(PatchBundle, TaskContext)> {
    let files = [
        (
            PathBuf::from("src/task_fp_102_alpha.py"),
            "def normalize(values):\n    if not values:\n        return []\n    return [value.strip() for value in values]\n",
        ),
        (
            PathBuf::from("src/task_fp_102_beta.py"),
            "def summarize(rows):\n    total = 0\n    for row in rows:\n        total += row.get('value', 0)\n    return total\n",
        ),
    ];
    fs::create_dir_all(repo_root.join("src"))?;
    let mut hunks = Vec::with_capacity(files.len());
    for (path, contents) in files {
        let full_path = repo_root.join(&path);
        fs::write(&full_path, contents.as_bytes())
            .with_context(|| format!("write {}", full_path.display()))?;
        let post_sha = sha256_bytes(contents.as_bytes());
        hunks.push(DiffHunk {
            path,
            pre_sha: post_sha,
            post_sha,
            before: String::new(),
            after: contents.to_string(),
        });
    }
    let patch = PatchBundle::try_new(
        AstDiff { hunks },
        valid_witness_segment(),
        "TASK-FP-102 hierarchical JEPA FSV".to_string(),
        sha256_bytes(b"task-fp-102-hierarchical-jepa-fsv"),
    )?;
    let context = TaskContext {
        task_id: TaskId("task-fp-102-hierarchical-jepa-fsv".to_string()),
        session_id: sha16(b"task-fp-102-hierarchical-jepa-fsv-session"),
        language: Language::Python,
        problem_statement: "TASK-FP-102 FSV: emit file/function/AST/chunk hierarchy".to_string(),
        tests: vec![TestId("test_task_fp_102_hierarchy".to_string())],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    Ok((patch, context))
}

fn calibration_record() -> CalibrationRecord {
    CalibrationRecord {
        version: "task-fp-102-hierarchical-jepa-fsv".to_string(),
        alpha: 0.1,
        target_coverage: 0.9,
        tau: 0.20,
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

fn level_kinds(record: &HierarchicalPredictionRecord) -> Vec<String> {
    record
        .levels
        .iter()
        .map(|level| format!("{:?}", level.level))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn count_level(record: &HierarchicalPredictionRecord, level: PredictionHierarchyLevel) -> usize {
    record
        .levels
        .iter()
        .filter(|entry| entry.level == level)
        .count()
}

fn sha16(bytes: &[u8]) -> [u8; 16] {
    let digest = sha256_bytes(bytes);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn unix_ms() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}
