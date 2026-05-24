use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::compiler::MeJepaCompiler;
use crate::error::MejepaInferError;
use crate::latent_search::{
    search_latent_actions, LatentActionCandidate, LatentActionSearchConfig,
    DEFAULT_LATENT_SEARCH_MAX_CANDIDATES,
};
use crate::objective_safety::{objective_report_for_prediction, ObjectiveSafetyReport};
use crate::types::{
    validate_probability, HierarchicalPredictionRecord, PatchBundle, RealityPrediction, TaskContext,
};

pub const DEFAULT_COUNTERFACTUAL_RANK_MAX_CANDIDATES: usize = DEFAULT_LATENT_SEARCH_MAX_CANDIDATES;

fn default_max_candidates() -> usize {
    DEFAULT_COUNTERFACTUAL_RANK_MAX_CANDIDATES
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CounterfactualCandidateRankingConfig {
    #[serde(default = "default_max_candidates")]
    pub max_candidates: usize,
    #[serde(default)]
    pub goal_latent: Option<Vec<f32>>,
}

impl Default for CounterfactualCandidateRankingConfig {
    fn default() -> Self {
        Self {
            max_candidates: DEFAULT_COUNTERFACTUAL_RANK_MAX_CANDIDATES,
            goal_latent: None,
        }
    }
}

impl CounterfactualCandidateRankingConfig {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.as_latent_search_config()?.validate()
    }

    fn as_latent_search_config(&self) -> Result<LatentActionSearchConfig, MejepaInferError> {
        let config = LatentActionSearchConfig {
            max_candidates: self.max_candidates,
            goal_latent: self.goal_latent.clone(),
            ..LatentActionSearchConfig::default()
        };
        config.validate()?;
        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CounterfactualCandidateRank {
    pub rank: usize,
    pub latent_search_rank: usize,
    pub candidate_id: String,
    pub prediction: RealityPrediction,
    pub hierarchy: HierarchicalPredictionRecord,
    pub action_latent: Vec<f32>,
    pub pass_in_distribution_score: f32,
    pub latent_objective_score: f32,
    pub objective_safety: ObjectiveSafetyReport,
    pub pass_blocked: bool,
    pub safety_violation_count: usize,
    pub objective_total_cost: f32,
    pub objective_cost_ceiling: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CounterfactualCandidateRankingResult {
    pub query_id: [u8; 16],
    pub latent_search_query_id: [u8; 16],
    pub task_id: String,
    pub session_id: [u8; 16],
    pub candidate_count: usize,
    pub selected_candidate_id: String,
    pub ranked_candidates: Vec<CounterfactualCandidateRank>,
    pub config: CounterfactualCandidateRankingConfig,
}

pub fn rank_counterfactual_candidates(
    compiler: &MeJepaCompiler,
    context: &TaskContext,
    candidates: Vec<LatentActionCandidate>,
    config: CounterfactualCandidateRankingConfig,
) -> Result<CounterfactualCandidateRankingResult, MejepaInferError> {
    context.validate()?;
    config.validate()?;
    let patches_by_id = candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.clone(), candidate.patch.clone()))
        .collect::<BTreeMap<_, _>>();
    let latent_config = config.as_latent_search_config()?;
    let latent_result = search_latent_actions(compiler, context, candidates, latent_config)?;

    let mut ranked = Vec::with_capacity(latent_result.ranked_candidates.len());
    for evaluation in latent_result.ranked_candidates {
        let patch = patches_by_id.get(&evaluation.candidate_id).ok_or_else(|| {
            MejepaInferError::InvalidInput {
                field: "candidate_id".to_string(),
                detail: format!("missing patch for candidate {}", evaluation.candidate_id),
            }
        })?;
        let objective_safety = objective_report_for_prediction(patch, &evaluation.prediction)?;
        let pass_in_distribution_score = pass_in_distribution_score(
            evaluation.prediction.predicted_oracle_pass,
            evaluation.prediction.ood_score,
        )?;
        ranked.push(CounterfactualCandidateRank {
            rank: 0,
            latent_search_rank: evaluation.rank,
            candidate_id: evaluation.candidate_id,
            prediction: evaluation.prediction,
            hierarchy: evaluation.hierarchy,
            action_latent: evaluation.action_latent,
            pass_in_distribution_score,
            latent_objective_score: evaluation.objective_score,
            pass_blocked: objective_safety.pass_blocked,
            safety_violation_count: objective_safety.constraint_violations.len(),
            objective_total_cost: objective_safety.cost.total_cost,
            objective_cost_ceiling: objective_safety.objective.pass_cost_ceiling,
            objective_safety,
        });
    }

    ranked.sort_by(compare_counterfactual_candidates);
    for (idx, candidate) in ranked.iter_mut().enumerate() {
        candidate.rank = idx + 1;
    }
    let selected_candidate_id = ranked
        .first()
        .map(|candidate| candidate.candidate_id.clone())
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "candidates".to_string(),
            detail: "counterfactual ranking requires at least one evaluated candidate".to_string(),
        })?;

    let query_id = ranking_query_id(context, &patches_by_id, &config, latent_result.query_id);
    Ok(CounterfactualCandidateRankingResult {
        query_id,
        latent_search_query_id: latent_result.query_id,
        task_id: context.task_id.0.clone(),
        session_id: context.session_id,
        candidate_count: ranked.len(),
        selected_candidate_id,
        ranked_candidates: ranked,
        config,
    })
}

pub fn pass_in_distribution_score(
    predicted_oracle_pass: f32,
    ood_score: f32,
) -> Result<f32, MejepaInferError> {
    validate_probability(
        "counterfactual_rank.predicted_oracle_pass",
        predicted_oracle_pass,
    )?;
    validate_probability("counterfactual_rank.ood_score", ood_score)?;
    let score = predicted_oracle_pass * (1.0 - ood_score);
    validate_probability("counterfactual_rank.pass_in_distribution_score", score)?;
    Ok(score)
}

fn compare_counterfactual_candidates(
    left: &CounterfactualCandidateRank,
    right: &CounterfactualCandidateRank,
) -> std::cmp::Ordering {
    left.pass_blocked
        .cmp(&right.pass_blocked)
        .then_with(|| {
            right
                .pass_in_distribution_score
                .total_cmp(&left.pass_in_distribution_score)
        })
        .then_with(|| {
            left.objective_total_cost
                .total_cmp(&right.objective_total_cost)
        })
        .then_with(|| {
            right
                .latent_objective_score
                .total_cmp(&left.latent_objective_score)
        })
        .then_with(|| left.candidate_id.cmp(&right.candidate_id))
}

fn ranking_query_id(
    context: &TaskContext,
    patches_by_id: &BTreeMap<String, PatchBundle>,
    config: &CounterfactualCandidateRankingConfig,
    latent_search_query_id: [u8; 16],
) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_COUNTERFACTUAL_RANKING_V1");
    hasher.update(latent_search_query_id);
    hasher.update(context.task_id.0.as_bytes());
    hasher.update(context.session_id);
    hasher.update(config.max_candidates.to_le_bytes());
    if let Some(goal) = &config.goal_latent {
        for value in goal {
            hasher.update(value.to_le_bytes());
        }
    }
    for (candidate_id, patch) in patches_by_id {
        hasher.update(candidate_id.as_bytes());
        hasher.update(patch.patch_sha);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use crate::{
        build_fixture_deterministic_compiler, complete_per_slot_sigma_squared, open_infer_rocksdb,
        sha256_bytes, valid_witness_segment, AstDiff, CalibrationRecord, CalibrationStore,
        DiffHunk, Language, MeJepaInferConfig, RocksDbInferStore, TaskEnvironment, TaskId, TestId,
    };

    use super::*;

    #[test]
    fn pass_in_distribution_score_is_predicted_pass_times_not_ood() {
        let score = pass_in_distribution_score(0.8, 0.25).unwrap();
        assert!((score - 0.6).abs() < f32::EPSILON);
        assert!(pass_in_distribution_score(f32::NAN, 0.25).is_err());
        assert!(pass_in_distribution_score(0.8, 1.25).is_err());
    }

    #[test]
    fn counterfactual_ranking_moves_hard_safety_blocks_after_safe_candidates() {
        let temp = tempfile::tempdir().unwrap();
        let repo_root = temp.path().join("repo");
        fs::create_dir_all(repo_root.join("src")).unwrap();
        fs::create_dir_all(repo_root.join("tests")).unwrap();
        let compiler = compiler_for(temp.path(), &repo_root);
        let context = context(&repo_root, "task-fp-105-unit");
        // Production `panel_source_text` rejects empty `before`; provide
        // minimal real prior content for the safe/risky candidates so they
        // satisfy the panel-materialization contract. The ranking logic
        // under test depends on path patterns and patch shape, not the
        // specific content of `before`.
        let candidates = vec![
            candidate(
                &repo_root,
                "candidate-safe",
                "src/safe.py",
                "def normalize(value):\n    return value\n",
                "def normalize(value):\n    return value.strip()\n",
            ),
            candidate(
                &repo_root,
                "candidate-risky",
                "src/risky.py",
                "def normalize(values):\n    return values\n",
                "# TODO: add owner\ndef normalize(values):\n    return [v.clone() for v in values]\n",
            ),
            candidate(
                &repo_root,
                "candidate-blocked",
                "tests/test_auth.py",
                "def test_auth_required():\n    assert requires_auth(user)\n",
                "def helper():\n    return True\n",
            ),
        ];
        let result = rank_counterfactual_candidates(
            &compiler,
            &context,
            candidates,
            CounterfactualCandidateRankingConfig::default(),
        )
        .unwrap();
        assert_eq!(result.ranked_candidates.len(), 3);
        assert!(!result.ranked_candidates[..2]
            .iter()
            .any(|candidate| candidate.pass_blocked));
        assert!(result.ranked_candidates[0].pass_in_distribution_score >= 0.0);
        assert_eq!(
            result.ranked_candidates.last().unwrap().candidate_id,
            "candidate-blocked"
        );
        assert!(result.ranked_candidates.last().unwrap().pass_blocked);
        assert!(
            result
                .ranked_candidates
                .last()
                .unwrap()
                .safety_violation_count
                > 0
        );
    }

    fn compiler_for(root: &Path, repo_root: &Path) -> MeJepaCompiler {
        let db = open_infer_rocksdb(root.join("rocksdb")).unwrap();
        let calibration = CalibrationStore::new(db.clone(), 30).unwrap();
        calibration.persist(&calibration_record()).unwrap();
        let store = Arc::new(RocksDbInferStore::new(db));
        build_fixture_deterministic_compiler(
            repo_root.to_path_buf(),
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
        )
        .unwrap()
    }

    fn context(repo_root: &Path, task_id: &str) -> TaskContext {
        TaskContext {
            task_id: TaskId(task_id.to_string()),
            session_id: sha16(task_id.as_bytes()),
            language: Language::Python,
            problem_statement: "TASK-FP-105 unit: rank counterfactual candidates".to_string(),
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
    ) -> LatentActionCandidate {
        let path = PathBuf::from(rel_path);
        fs::write(repo_root.join(&path), after.as_bytes()).unwrap();
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
        )
        .unwrap();
        LatentActionCandidate::try_new(candidate_id, patch).unwrap()
    }

    fn calibration_record() -> CalibrationRecord {
        CalibrationRecord {
            version: "task-fp-105-rank-candidates-unit".to_string(),
            alpha: 0.1,
            target_coverage: 0.9,
            tau: 0.20,
            sigma_squared: 1_000_000_000.0,
            empirical_coverage: 0.95,
            min_samples_per_stratum: 1,
            sample_count: 1,
            per_language_counts: BTreeMap::from([(Language::Python, 1)]),
            per_slot_sigma_squared: Some(complete_per_slot_sigma_squared(1_000_000_000.0)),
            corpus_sha: [10; 32],
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
}
