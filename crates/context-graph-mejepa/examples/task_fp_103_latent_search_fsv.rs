//! TASK-FP-103 latent-space action search FSV.
//!
//! Proves candidate patches are ranked by an abstract action latent plus
//! predicted ME-JEPA outcome signals, without mutating referenced repo files.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::{
    build_deterministic_compiler, derive_latent_action_vector, open_infer_rocksdb,
    search_latent_actions, sha256_bytes, valid_witness_segment, AstDiff, CalibrationRecord,
    CalibrationStore, DiffHunk, Language, LatentActionCandidate, LatentActionObjectiveWeights,
    LatentActionSearchConfig, MeJepaInferConfig, PatchBundle, RocksDbInferStore, TaskContext,
    TaskEnvironment, TaskId, TestId, LATENT_ACTION_VECTOR_DIM,
};
use serde::Serialize;

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-fp-103-latent-search-fsv";
const SEARCH_TOOL_NAME: &str = "mcp__cgreality__mejepa_search_latent_actions";

#[derive(Debug, Serialize)]
struct LatentSearchEvidence {
    output_path: String,
    db_path: String,
    tool_name: String,
    query_id: String,
    candidate_count: usize,
    ranked_count: usize,
    selected_candidate_id: String,
    expected_candidate_id: String,
    selected_matches_expected: bool,
    ranks_are_contiguous: bool,
    action_latent_dim: usize,
    all_action_latents_finite: bool,
    selected_goal_alignment: f32,
    next_best_goal_alignment: f32,
    selected_objective_score: f32,
    next_best_objective_score: f32,
    disk_sha_before: BTreeMap<String, Option<String>>,
    disk_sha_after: BTreeMap<String, Option<String>>,
    no_file_mutation: bool,
    rejects_single_candidate: bool,
    rejects_duplicate_candidate_id: bool,
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
    let store = Arc::new(RocksDbInferStore::new(db));
    let compiler = build_deterministic_compiler(
        repo_root.clone(),
        store,
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

    let context = context(&repo_root);
    let candidates = vec![
        candidate(&repo_root, "candidate-low", b"task-fp-103-low")?,
        candidate(&repo_root, "candidate-goal", b"task-fp-103-goal")?,
        candidate(&repo_root, "candidate-alt", b"task-fp-103-alt")?,
    ];
    let expected_candidate_id = "candidate-goal".to_string();
    let goal_patch = candidates
        .iter()
        .find(|candidate| candidate.candidate_id == expected_candidate_id)
        .context("missing expected candidate")?
        .patch
        .clone();
    let goal_latent = derive_latent_action_vector(&goal_patch, &context)?;
    let config = LatentActionSearchConfig {
        goal_latent: Some(goal_latent),
        objective_weights: LatentActionObjectiveWeights {
            predicted_oracle_pass: 0.05,
            calibrated_confidence: 0.05,
            in_distribution: 0.05,
            hierarchy: 0.05,
            goal_alignment: 0.80,
        },
        ..LatentActionSearchConfig::default()
    };

    let disk_sha_before = file_sha_snapshot(&repo_root, &candidates)?;
    let result = search_latent_actions(&compiler, &context, candidates.clone(), config)?;
    let disk_sha_after = file_sha_snapshot(&repo_root, &candidates)?;

    let selected = result
        .ranked_candidates
        .first()
        .context("missing selected candidate")?;
    let next_best = result
        .ranked_candidates
        .get(1)
        .context("missing second-ranked candidate")?;
    let ranks_are_contiguous = result
        .ranked_candidates
        .iter()
        .enumerate()
        .all(|(idx, candidate)| candidate.rank == idx + 1);
    let all_action_latents_finite = result.ranked_candidates.iter().all(|candidate| {
        candidate.action_latent.len() == LATENT_ACTION_VECTOR_DIM
            && candidate
                .action_latent
                .iter()
                .all(|value| value.is_finite())
    });
    let no_file_mutation = disk_sha_before == disk_sha_after;
    let rejects_single_candidate = search_latent_actions(
        &compiler,
        &context,
        vec![candidates[0].clone()],
        LatentActionSearchConfig::default(),
    )
    .is_err();
    let rejects_duplicate_candidate_id = search_latent_actions(
        &compiler,
        &context,
        vec![
            candidates[0].clone(),
            LatentActionCandidate::try_new(
                candidates[0].candidate_id.clone(),
                candidates[1].patch.clone(),
            )?,
        ],
        LatentActionSearchConfig::default(),
    )
    .is_err();
    let selected_matches_expected = result.selected_candidate_id == expected_candidate_id;
    let all_passed = result.candidate_count == 3
        && result.ranked_candidates.len() == 3
        && selected_matches_expected
        && ranks_are_contiguous
        && all_action_latents_finite
        && selected.score_breakdown.goal_alignment >= next_best.score_breakdown.goal_alignment
        && selected.objective_score >= next_best.objective_score
        && no_file_mutation
        && rejects_single_candidate
        && rejects_duplicate_candidate_id;

    let output_path = run_root.join("latent_search_fsv.json");
    let evidence = LatentSearchEvidence {
        output_path: output_path.display().to_string(),
        db_path: db_path.display().to_string(),
        tool_name: SEARCH_TOOL_NAME.to_string(),
        query_id: hex::encode(result.query_id),
        candidate_count: result.candidate_count,
        ranked_count: result.ranked_candidates.len(),
        selected_candidate_id: result.selected_candidate_id.clone(),
        expected_candidate_id,
        selected_matches_expected,
        ranks_are_contiguous,
        action_latent_dim: LATENT_ACTION_VECTOR_DIM,
        all_action_latents_finite,
        selected_goal_alignment: selected.score_breakdown.goal_alignment,
        next_best_goal_alignment: next_best.score_breakdown.goal_alignment,
        selected_objective_score: selected.objective_score,
        next_best_objective_score: next_best.objective_score,
        disk_sha_before,
        disk_sha_after,
        no_file_mutation,
        rejects_single_candidate,
        rejects_duplicate_candidate_id,
        all_passed,
    };
    fs::write(&output_path, serde_json::to_vec_pretty(&evidence)?)
        .with_context(|| format!("write {}", output_path.display()))?;
    println!("{}", output_path.display());
    if !all_passed {
        bail!("TASK-FP-103 latent search FSV failed");
    }
    Ok(())
}

fn context(repo_root: &Path) -> TaskContext {
    TaskContext {
        task_id: TaskId("task-fp-103-latent-search-fsv".to_string()),
        session_id: sha16(b"task-fp-103-latent-search-fsv-session"),
        language: Language::Python,
        problem_statement: "TASK-FP-103 FSV: search candidate actions in latent space".to_string(),
        tests: vec![TestId("test_task_fp_103_latent_search".to_string())],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    }
}

fn candidate(repo_root: &Path, candidate_id: &str, seed: &[u8]) -> Result<LatentActionCandidate> {
    let module_name = candidate_id.replace('-', "_");
    let path = PathBuf::from(format!("src/{module_name}.py"));
    fs::create_dir_all(repo_root.join("src"))?;
    let contents = format!(
        "def {module_name}(value):\n    if value:\n        return '{}'\n    return 'fallback'\n",
        hex::encode(seed)
    );
    fs::write(repo_root.join(&path), contents.as_bytes())
        .with_context(|| format!("write {}", path.display()))?;
    let post_sha = sha256_bytes(contents.as_bytes());
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path,
                pre_sha: post_sha,
                post_sha,
                before: String::new(),
                after: contents,
            }],
        },
        valid_witness_segment(),
        format!("TASK-FP-103 latent search {candidate_id}"),
        sha256_bytes(seed),
    )?;
    Ok(LatentActionCandidate::try_new(candidate_id, patch)?)
}

fn file_sha_snapshot(
    repo_root: &Path,
    candidates: &[LatentActionCandidate],
) -> Result<BTreeMap<String, Option<String>>> {
    let mut paths = BTreeSet::new();
    for candidate in candidates {
        for hunk in &candidate.patch.ast_diff.hunks {
            paths.insert(hunk.path.clone());
        }
    }
    let mut out = BTreeMap::new();
    for path in paths {
        let full_path = repo_root.join(&path);
        let value = match fs::read(&full_path) {
            Ok(bytes) => Some(hex::encode(sha256_bytes(&bytes))),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(err).with_context(|| format!("read {}", full_path.display())),
        };
        out.insert(path.display().to_string(), value);
    }
    Ok(out)
}

fn calibration_record() -> CalibrationRecord {
    CalibrationRecord {
        version: "task-fp-103-latent-search-fsv".to_string(),
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
