//! TASK-FP-105 counterfactual candidate ranking FSV.
//!
//! Proves the formal `mejepa_rank_candidates` surface can rank 5+ candidate
//! diffs atomically by predicted pass in-distribution score while consuming the
//! TASK-FP-104 objective/safety report for hard-blocked repairs.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::{
    build_deterministic_compiler, open_infer_rocksdb, rank_counterfactual_candidates, sha256_bytes,
    valid_witness_segment, AstDiff, CalibrationRecord, CalibrationStore,
    CounterfactualCandidateRankingConfig, DiffHunk, Language, LatentActionCandidate,
    MeJepaInferConfig, PatchBundle, RocksDbInferStore, TaskContext, TaskEnvironment, TaskId,
    TestId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-fp-105-rank-candidates-fsv";
const RANK_TOOL_NAME: &str = "mcp__cgreality__mejepa_rank_candidates";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RankCandidateSummary {
    rank: usize,
    candidate_id: String,
    pass_in_distribution_score: f32,
    objective_total_cost: f32,
    pass_blocked: bool,
    safety_violation_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RankCandidatesEvidence {
    output_path: String,
    db_path: String,
    tool_name: String,
    query_id: String,
    latent_search_query_id: String,
    candidate_count: usize,
    ranked_count: usize,
    selected_candidate_id: String,
    expected_selected_candidate_id: String,
    selected_matches_expected: bool,
    ranks_are_contiguous: bool,
    unblocked_pass_scores_monotonic: bool,
    clean_candidate_before_risky_candidate: bool,
    blocked_candidate_last: bool,
    blocked_candidate_id: String,
    blocked_candidate_safety_violations: usize,
    ranked_summaries: Vec<RankCandidateSummary>,
    disk_sha_before: BTreeMap<String, Option<String>>,
    disk_sha_after: BTreeMap<String, Option<String>>,
    no_file_mutation: bool,
    rejects_single_candidate: bool,
    rejects_duplicate_candidate_id: bool,
    readback_equal: bool,
    all_passed: bool,
}

fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("rocksdb");
    let repo_root = run_root.join("repo");
    fs::create_dir_all(repo_root.join("src")).context("create repo src")?;
    fs::create_dir_all(repo_root.join("tests")).context("create repo tests")?;
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
            p_test_threshold: 0.20,
            ood_refuse_threshold: 1.0,
            outcome_set_max: 1,
            ..MeJepaInferConfig::default()
        },
    )?;

    let context = context(&repo_root);
    let candidates = vec![
        candidate(
            &repo_root,
            "candidate-00-clean",
            "src/candidate_clean.py",
            "",
            "def normalize(value):\n    return value.strip()\n",
        )?,
        candidate(
            &repo_root,
            "candidate-01-todo-perf",
            "src/candidate_todo_perf.py",
            "",
            "# TODO: add owner\ndef normalize(values):\n    return [v.clone() for v in values]\n",
        )?,
        candidate(
            &repo_root,
            "candidate-02-empty-edge",
            "src/candidate_empty_edge.py",
            "",
            "def normalize(values):\n    if len(values) == 0:\n        return []\n    return list(values)\n",
        )?,
        candidate(
            &repo_root,
            "candidate-03-accuracy-touch",
            "src/candidate_accuracy.py",
            "",
            "def update_metrics(row):\n    accuracy = row.get('accuracy', 0.0)\n    return accuracy\n",
        )?,
        candidate(
            &repo_root,
            "candidate-04-dependency",
            "requirements.txt",
            "",
            "numpy==2.0.0\n",
        )?,
        candidate(
            &repo_root,
            "candidate-99-safety-blocked",
            "tests/test_auth.py",
            "def test_auth_required():\n    assert requires_auth(user)\n",
            "def helper():\n    return True\n",
        )?,
    ];
    let expected_selected_candidate_id = "candidate-00-clean".to_string();
    let blocked_candidate_id = "candidate-99-safety-blocked".to_string();

    let disk_sha_before = file_sha_snapshot(&repo_root, &candidates)?;
    let result = rank_counterfactual_candidates(
        &compiler,
        &context,
        candidates.clone(),
        CounterfactualCandidateRankingConfig::default(),
    )?;
    let disk_sha_after = file_sha_snapshot(&repo_root, &candidates)?;

    let ranks_are_contiguous = result
        .ranked_candidates
        .iter()
        .enumerate()
        .all(|(idx, candidate)| candidate.rank == idx + 1);
    let unblocked_scores = result
        .ranked_candidates
        .iter()
        .filter(|candidate| !candidate.pass_blocked)
        .map(|candidate| candidate.pass_in_distribution_score)
        .collect::<Vec<_>>();
    let unblocked_pass_scores_monotonic = unblocked_scores
        .windows(2)
        .all(|pair| pair[0] + f32::EPSILON >= pair[1]);
    let selected_matches_expected = result.selected_candidate_id == expected_selected_candidate_id;
    let clean_rank = rank_for(&result.ranked_candidates, &expected_selected_candidate_id)?;
    let risky_rank = rank_for(&result.ranked_candidates, "candidate-01-todo-perf")?;
    let blocked = result
        .ranked_candidates
        .iter()
        .find(|candidate| candidate.candidate_id == blocked_candidate_id)
        .context("missing blocked candidate")?;
    let blocked_candidate_last = blocked.rank == result.ranked_candidates.len();
    let no_file_mutation = disk_sha_before == disk_sha_after;
    let rejects_single_candidate = rank_counterfactual_candidates(
        &compiler,
        &context,
        vec![candidates[0].clone()],
        CounterfactualCandidateRankingConfig::default(),
    )
    .is_err();
    let rejects_duplicate_candidate_id = rank_counterfactual_candidates(
        &compiler,
        &context,
        vec![
            candidates[0].clone(),
            LatentActionCandidate::try_new(
                candidates[0].candidate_id.clone(),
                candidates[1].patch.clone(),
            )?,
        ],
        CounterfactualCandidateRankingConfig::default(),
    )
    .is_err();
    let ranked_summaries = result
        .ranked_candidates
        .iter()
        .map(|candidate| RankCandidateSummary {
            rank: candidate.rank,
            candidate_id: candidate.candidate_id.clone(),
            pass_in_distribution_score: candidate.pass_in_distribution_score,
            objective_total_cost: candidate.objective_total_cost,
            pass_blocked: candidate.pass_blocked,
            safety_violation_count: candidate.safety_violation_count,
        })
        .collect::<Vec<_>>();
    let all_passed = result.candidate_count >= 5
        && result.ranked_candidates.len() == result.candidate_count
        && selected_matches_expected
        && ranks_are_contiguous
        && unblocked_pass_scores_monotonic
        && clean_rank < risky_rank
        && blocked_candidate_last
        && blocked.pass_blocked
        && blocked.safety_violation_count > 0
        && no_file_mutation
        && rejects_single_candidate
        && rejects_duplicate_candidate_id;

    let output_path = run_root.join("rank_candidates_fsv.json");
    let mut evidence = RankCandidatesEvidence {
        output_path: output_path.display().to_string(),
        db_path: db_path.display().to_string(),
        tool_name: RANK_TOOL_NAME.to_string(),
        query_id: hex::encode(result.query_id),
        latent_search_query_id: hex::encode(result.latent_search_query_id),
        candidate_count: result.candidate_count,
        ranked_count: result.ranked_candidates.len(),
        selected_candidate_id: result.selected_candidate_id.clone(),
        expected_selected_candidate_id,
        selected_matches_expected,
        ranks_are_contiguous,
        unblocked_pass_scores_monotonic,
        clean_candidate_before_risky_candidate: clean_rank < risky_rank,
        blocked_candidate_last,
        blocked_candidate_id,
        blocked_candidate_safety_violations: blocked.safety_violation_count,
        ranked_summaries,
        disk_sha_before,
        disk_sha_after,
        no_file_mutation,
        rejects_single_candidate,
        rejects_duplicate_candidate_id,
        readback_equal: false,
        all_passed,
    };
    write_json_readback(&output_path, &mut evidence)?;
    println!("{}", output_path.display());
    if !evidence.all_passed || !evidence.readback_equal {
        bail!("TASK-FP-105 rank candidates FSV failed");
    }
    Ok(())
}

fn context(repo_root: &Path) -> TaskContext {
    TaskContext {
        task_id: TaskId("task-fp-105-rank-candidates-fsv".to_string()),
        session_id: sha16(b"task-fp-105-rank-candidates-fsv-session"),
        language: Language::Python,
        problem_statement: "TASK-FP-105 FSV: rank counterfactual repair candidates".to_string(),
        tests: vec![TestId("test_task_fp_105_rank_candidates".to_string())],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    }
}

fn candidate(
    repo_root: &Path,
    candidate_id: &str,
    rel_path: &str,
    before: &str,
    after: &str,
) -> Result<LatentActionCandidate> {
    let path = PathBuf::from(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(repo_root.join(parent))?;
    }
    fs::write(repo_root.join(&path), after.as_bytes())
        .with_context(|| format!("write {}", path.display()))?;
    let pre_sha = sha256_bytes(before.as_bytes());
    let post_sha = sha256_bytes(after.as_bytes());
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path,
                pre_sha,
                post_sha,
                before: before.to_string(),
                after: after.to_string(),
            }],
        },
        valid_witness_segment(),
        format!("TASK-FP-105 rank {candidate_id}"),
        sha256_bytes(candidate_id.as_bytes()),
    )?;
    Ok(LatentActionCandidate::try_new(candidate_id, patch)?)
}

fn rank_for(
    candidates: &[context_graph_mejepa::CounterfactualCandidateRank],
    id: &str,
) -> Result<usize> {
    candidates
        .iter()
        .find(|candidate| candidate.candidate_id == id)
        .map(|candidate| candidate.rank)
        .with_context(|| format!("missing rank for {id}"))
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

fn write_json_readback(path: &Path, evidence: &mut RankCandidatesEvidence) -> Result<()> {
    evidence.readback_equal = false;
    let first_bytes = serde_json::to_vec_pretty(evidence)?;
    fs::write(path, &first_bytes).with_context(|| format!("write {}", path.display()))?;
    let readback: Value = serde_json::from_slice(&fs::read(path)?)?;
    let expected: Value = serde_json::from_slice(&first_bytes)?;
    evidence.readback_equal = readback == expected;
    let final_bytes = serde_json::to_vec_pretty(evidence)?;
    fs::write(path, &final_bytes).with_context(|| format!("write {}", path.display()))?;
    let final_readback: RankCandidatesEvidence = serde_json::from_slice(&fs::read(path)?)?;
    if final_readback != *evidence {
        bail!("rank candidate FSV readback mismatch");
    }
    Ok(())
}

fn calibration_record() -> CalibrationRecord {
    CalibrationRecord {
        version: "task-fp-105-rank-candidates-fsv".to_string(),
        alpha: 0.1,
        target_coverage: 0.9,
        tau: 0.20,
        sigma_squared: 1_000_000_000.0,
        empirical_coverage: 0.95,
        min_samples_per_stratum: 1,
        sample_count: 1,
        per_language_counts: BTreeMap::from([(Language::Python, 1)]),
        corpus_sha: [11; 32],
        embedder_versions: BTreeMap::new(),
        frozen_at: chrono::Utc::now().timestamp(),
    }
}

fn unix_ms() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before UNIX epoch")?
        .as_millis())
}

fn sha16(bytes: &[u8]) -> [u8; 16] {
    let digest = sha256_bytes(bytes);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}
