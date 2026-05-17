use std::collections::BTreeSet;

use context_graph_mejepa_instruments::Panel;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::compiler::{materialize_inference_panels, MeJepaCompiler};
use crate::error::MejepaInferError;
use crate::hierarchical::build_hierarchical_prediction;
use crate::types::{
    validate_probability, HierarchicalPredictionRecord, PatchBundle, RealityPrediction, TaskContext,
};

pub const LATENT_ACTION_VECTOR_DIM: usize = 16;
pub const DEFAULT_LATENT_SEARCH_MAX_CANDIDATES: usize = 32;
pub const ABSOLUTE_LATENT_SEARCH_MAX_CANDIDATES: usize = 128;

fn default_max_candidates() -> usize {
    DEFAULT_LATENT_SEARCH_MAX_CANDIDATES
}

fn default_oracle_weight() -> f32 {
    0.40
}

fn default_confidence_weight() -> f32 {
    0.20
}

fn default_in_distribution_weight() -> f32 {
    0.20
}

fn default_hierarchy_weight() -> f32 {
    0.10
}

fn default_goal_alignment_weight() -> f32 {
    0.10
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LatentActionObjectiveWeights {
    #[serde(default = "default_oracle_weight")]
    pub predicted_oracle_pass: f32,
    #[serde(default = "default_confidence_weight")]
    pub calibrated_confidence: f32,
    #[serde(default = "default_in_distribution_weight")]
    pub in_distribution: f32,
    #[serde(default = "default_hierarchy_weight")]
    pub hierarchy: f32,
    #[serde(default = "default_goal_alignment_weight")]
    pub goal_alignment: f32,
}

impl Default for LatentActionObjectiveWeights {
    fn default() -> Self {
        Self {
            predicted_oracle_pass: default_oracle_weight(),
            calibrated_confidence: default_confidence_weight(),
            in_distribution: default_in_distribution_weight(),
            hierarchy: default_hierarchy_weight(),
            goal_alignment: default_goal_alignment_weight(),
        }
    }
}

impl LatentActionObjectiveWeights {
    fn validate(&self) -> Result<f32, MejepaInferError> {
        let mut sum = 0.0_f32;
        for (field, value) in [
            (
                "objective_weights.predicted_oracle_pass",
                self.predicted_oracle_pass,
            ),
            (
                "objective_weights.calibrated_confidence",
                self.calibrated_confidence,
            ),
            ("objective_weights.in_distribution", self.in_distribution),
            ("objective_weights.hierarchy", self.hierarchy),
            ("objective_weights.goal_alignment", self.goal_alignment),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(MejepaInferError::InvalidInput {
                    field: field.to_string(),
                    detail: format!("weight must be finite and non-negative; got {value}"),
                });
            }
            sum += value;
        }
        if sum <= f32::EPSILON {
            return Err(MejepaInferError::InvalidInput {
                field: "objective_weights".to_string(),
                detail: "at least one objective weight must be positive".to_string(),
            });
        }
        Ok(sum)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LatentActionSearchConfig {
    #[serde(default = "default_max_candidates")]
    pub max_candidates: usize,
    #[serde(default)]
    pub goal_latent: Option<Vec<f32>>,
    #[serde(default)]
    pub objective_weights: LatentActionObjectiveWeights,
}

impl Default for LatentActionSearchConfig {
    fn default() -> Self {
        Self {
            max_candidates: DEFAULT_LATENT_SEARCH_MAX_CANDIDATES,
            goal_latent: None,
            objective_weights: LatentActionObjectiveWeights::default(),
        }
    }
}

impl LatentActionSearchConfig {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if !(2..=ABSOLUTE_LATENT_SEARCH_MAX_CANDIDATES).contains(&self.max_candidates) {
            return Err(MejepaInferError::InvalidInput {
                field: "max_candidates".to_string(),
                detail: format!(
                    "max_candidates must be in [2, {ABSOLUTE_LATENT_SEARCH_MAX_CANDIDATES}]"
                ),
            });
        }
        self.objective_weights.validate()?;
        if let Some(goal) = &self.goal_latent {
            validate_latent_vector("goal_latent", goal)?;
            let norm = l2_norm(goal);
            if norm <= f32::EPSILON {
                return Err(MejepaInferError::InvalidInput {
                    field: "goal_latent".to_string(),
                    detail: "goal_latent must have non-zero norm".to_string(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LatentActionCandidate {
    pub candidate_id: String,
    pub patch: PatchBundle,
}

impl LatentActionCandidate {
    pub fn try_new(
        candidate_id: impl Into<String>,
        patch: PatchBundle,
    ) -> Result<Self, MejepaInferError> {
        let value = Self {
            candidate_id: candidate_id.into(),
            patch,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_candidate_id("candidate_id", &self.candidate_id)?;
        self.patch.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LatentActionScoreBreakdown {
    pub predicted_oracle_pass: f32,
    pub calibrated_confidence: f32,
    pub in_distribution: f32,
    pub hierarchy: f32,
    pub goal_alignment: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LatentActionEvaluation {
    pub rank: usize,
    pub candidate_id: String,
    pub prediction: RealityPrediction,
    pub hierarchy: HierarchicalPredictionRecord,
    pub action_latent: Vec<f32>,
    pub objective_score: f32,
    pub score_breakdown: LatentActionScoreBreakdown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LatentActionSearchResult {
    pub query_id: [u8; 16],
    pub task_id: String,
    pub session_id: [u8; 16],
    pub candidate_count: usize,
    pub selected_candidate_id: String,
    pub ranked_candidates: Vec<LatentActionEvaluation>,
    pub config: LatentActionSearchConfig,
}

pub fn derive_latent_action_vector(
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<Vec<f32>, MejepaInferError> {
    patch.validate()?;
    context.validate()?;
    let (panel_t0, panel_t1, _panel_t2) = materialize_inference_panels(patch, context)?;
    latent_delta_from_panels(&panel_t0, &panel_t1)
}

pub fn search_latent_actions(
    compiler: &MeJepaCompiler,
    context: &TaskContext,
    candidates: Vec<LatentActionCandidate>,
    config: LatentActionSearchConfig,
) -> Result<LatentActionSearchResult, MejepaInferError> {
    context.validate()?;
    config.validate()?;
    validate_candidates(&candidates, config.max_candidates)?;
    let weight_sum = config.objective_weights.validate()?;

    let mut evaluations = Vec::with_capacity(candidates.len());
    for candidate in &candidates {
        let action_latent = derive_latent_action_vector(&candidate.patch, context)?;
        let (prediction, predicted_panel) =
            compiler.compile_with_panel(&candidate.patch, context)?;
        let hierarchy =
            build_hierarchical_prediction(&prediction, &candidate.patch, &predicted_panel)?;
        let score_breakdown = score_breakdown(&prediction, &hierarchy, &action_latent, &config)?;
        let objective_score =
            weighted_objective(&score_breakdown, &config.objective_weights, weight_sum)?;
        evaluations.push(LatentActionEvaluation {
            rank: 0,
            candidate_id: candidate.candidate_id.clone(),
            prediction,
            hierarchy,
            action_latent,
            objective_score,
            score_breakdown,
        });
    }

    evaluations.sort_by(|left, right| {
        right
            .objective_score
            .total_cmp(&left.objective_score)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    for (idx, evaluation) in evaluations.iter_mut().enumerate() {
        evaluation.rank = idx + 1;
    }
    let selected_candidate_id = evaluations
        .first()
        .map(|candidate| candidate.candidate_id.clone())
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "candidates".to_string(),
            detail: "latent search requires at least one evaluated candidate".to_string(),
        })?;

    Ok(LatentActionSearchResult {
        query_id: query_id(context, &candidates, &config),
        task_id: context.task_id.0.clone(),
        session_id: context.session_id,
        candidate_count: candidates.len(),
        selected_candidate_id,
        ranked_candidates: evaluations,
        config,
    })
}

fn validate_candidates(
    candidates: &[LatentActionCandidate],
    max_candidates: usize,
) -> Result<(), MejepaInferError> {
    if candidates.len() < 2 {
        return Err(MejepaInferError::InvalidInput {
            field: "candidates".to_string(),
            detail: "latent action search requires at least two candidates".to_string(),
        });
    }
    if candidates.len() > max_candidates {
        return Err(MejepaInferError::DimMismatch {
            expected: max_candidates,
            actual: candidates.len(),
            context: "latent action candidates exceeds configured max_candidates".to_string(),
        });
    }
    let mut seen = BTreeSet::new();
    for (idx, candidate) in candidates.iter().enumerate() {
        candidate.validate()?;
        if !seen.insert(candidate.candidate_id.as_str()) {
            return Err(MejepaInferError::InvalidInput {
                field: format!("candidates[{idx}].candidate_id"),
                detail: format!("duplicate candidate id {}", candidate.candidate_id),
            });
        }
    }
    Ok(())
}

fn validate_candidate_id(field: &str, value: &str) -> Result<(), MejepaInferError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "candidate id must be non-empty".to_string(),
        });
    }
    if trimmed.len() > 128 {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "candidate id must be <= 128 bytes".to_string(),
        });
    }
    if trimmed
        .bytes()
        .any(|byte| byte < 0x20 || byte == 0x7f || byte == b'/')
    {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "candidate id contains an invalid control or slash character".to_string(),
        });
    }
    Ok(())
}

fn validate_latent_vector(field: &str, latent: &[f32]) -> Result<(), MejepaInferError> {
    if latent.len() != LATENT_ACTION_VECTOR_DIM {
        return Err(MejepaInferError::DimMismatch {
            expected: LATENT_ACTION_VECTOR_DIM,
            actual: latent.len(),
            context: format!("{field} must have the latent action vector dimension"),
        });
    }
    for (idx, value) in latent.iter().enumerate() {
        if !value.is_finite() {
            return Err(MejepaInferError::NanDetected {
                nan_source: field.to_string(),
                detail: format!("{field}[{idx}] is {value}"),
            });
        }
    }
    Ok(())
}

fn latent_delta_from_panels(
    panel_t0: &Panel,
    panel_t1: &Panel,
) -> Result<Vec<f32>, MejepaInferError> {
    if panel_t0.data().len() != panel_t1.data().len() {
        return Err(MejepaInferError::DimMismatch {
            expected: panel_t0.data().len(),
            actual: panel_t1.data().len(),
            context: "latent action panel delta length mismatch".to_string(),
        });
    }
    if panel_t0.data().is_empty() {
        return Err(MejepaInferError::DimMismatch {
            expected: 1,
            actual: 0,
            context: "latent action panels must be non-empty".to_string(),
        });
    }

    let mut latent = vec![0.0_f32; LATENT_ACTION_VECTOR_DIM];
    for (idx, (before, after)) in panel_t0.data().iter().zip(panel_t1.data()).enumerate() {
        let delta = after - before;
        if !delta.is_finite() {
            return Err(MejepaInferError::NanDetected {
                nan_source: "latent_action_delta".to_string(),
                detail: format!("panel delta at index {idx} is {delta}"),
            });
        }
        let bucket = idx % LATENT_ACTION_VECTOR_DIM;
        let sign = if ((idx / LATENT_ACTION_VECTOR_DIM) + bucket).is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        latent[bucket] += delta * sign;
    }

    let scale = (panel_t0.data().len() as f32).sqrt().max(1.0);
    for value in &mut latent {
        *value /= scale;
    }
    normalize_latent(&mut latent)?;
    Ok(latent)
}

fn normalize_latent(latent: &mut [f32]) -> Result<(), MejepaInferError> {
    let norm = l2_norm(latent);
    if !norm.is_finite() || norm <= f32::EPSILON {
        return Err(MejepaInferError::InvalidInput {
            field: "action_latent".to_string(),
            detail: "action latent must have finite non-zero norm".to_string(),
        });
    }
    for value in latent {
        *value /= norm;
    }
    Ok(())
}

fn score_breakdown(
    prediction: &RealityPrediction,
    hierarchy: &HierarchicalPredictionRecord,
    action_latent: &[f32],
    config: &LatentActionSearchConfig,
) -> Result<LatentActionScoreBreakdown, MejepaInferError> {
    prediction.validate()?;
    hierarchy.validate()?;
    validate_latent_vector("action_latent", action_latent)?;
    let in_distribution = (1.0 - prediction.ood_score).clamp(0.0, 1.0);
    validate_probability("latent_search.in_distribution", in_distribution)?;
    let hierarchy = hierarchy_score(hierarchy)?;
    let goal_alignment = match &config.goal_latent {
        Some(goal) => cosine_alignment(goal, action_latent)?,
        None => 0.5,
    };
    validate_probability("latent_search.goal_alignment", goal_alignment)?;
    Ok(LatentActionScoreBreakdown {
        predicted_oracle_pass: prediction.predicted_oracle_pass,
        calibrated_confidence: prediction.calibrated_confidence,
        in_distribution,
        hierarchy,
        goal_alignment,
    })
}

fn hierarchy_score(record: &HierarchicalPredictionRecord) -> Result<f32, MejepaInferError> {
    if record.levels.is_empty() {
        return Err(MejepaInferError::InvalidInput {
            field: "hierarchical_prediction.levels".to_string(),
            detail: "latent search requires hierarchy levels".to_string(),
        });
    }
    let mut sum = 0.0_f32;
    for level in &record.levels {
        let value =
            (level.predicted_oracle_pass + level.calibrated_confidence + (1.0 - level.ood_score))
                / 3.0;
        validate_probability("latent_search.hierarchy_level_score", value)?;
        sum += value;
    }
    let mean = sum / record.levels.len() as f32;
    validate_probability("latent_search.hierarchy_score", mean)?;
    Ok(mean)
}

fn cosine_alignment(goal: &[f32], actual: &[f32]) -> Result<f32, MejepaInferError> {
    validate_latent_vector("goal_latent", goal)?;
    validate_latent_vector("action_latent", actual)?;
    let goal_norm = l2_norm(goal);
    let actual_norm = l2_norm(actual);
    if goal_norm <= f32::EPSILON || actual_norm <= f32::EPSILON {
        return Err(MejepaInferError::InvalidInput {
            field: "latent_alignment".to_string(),
            detail: "goal and action latents must have non-zero norm".to_string(),
        });
    }
    let dot = goal
        .iter()
        .zip(actual)
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let cosine = (dot / (goal_norm * actual_norm)).clamp(-1.0, 1.0);
    let score = ((cosine + 1.0) * 0.5).clamp(0.0, 1.0);
    validate_probability("latent_search.cosine_alignment", score)?;
    Ok(score)
}

fn weighted_objective(
    scores: &LatentActionScoreBreakdown,
    weights: &LatentActionObjectiveWeights,
    weight_sum: f32,
) -> Result<f32, MejepaInferError> {
    let score = (scores.predicted_oracle_pass * weights.predicted_oracle_pass
        + scores.calibrated_confidence * weights.calibrated_confidence
        + scores.in_distribution * weights.in_distribution
        + scores.hierarchy * weights.hierarchy
        + scores.goal_alignment * weights.goal_alignment)
        / weight_sum;
    validate_probability("latent_search.objective_score", score)?;
    Ok(score)
}

fn l2_norm(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn query_id(
    context: &TaskContext,
    candidates: &[LatentActionCandidate],
    config: &LatentActionSearchConfig,
) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(context.task_id.0.as_bytes());
    hasher.update(context.session_id);
    hasher.update(context.problem_statement.as_bytes());
    hasher.update(config.max_candidates.to_le_bytes());
    for value in [
        config.objective_weights.predicted_oracle_pass,
        config.objective_weights.calibrated_confidence,
        config.objective_weights.in_distribution,
        config.objective_weights.hierarchy,
        config.objective_weights.goal_alignment,
    ] {
        hasher.update(value.to_le_bytes());
    }
    if let Some(goal) = &config.goal_latent {
        for value in goal {
            hasher.update(value.to_le_bytes());
        }
    }
    for candidate in candidates {
        hasher.update(candidate.candidate_id.as_bytes());
        hasher.update(candidate.patch.patch_sha);
        hasher.update(candidate.patch.commit_message.as_bytes());
        for hunk in &candidate.patch.ast_diff.hunks {
            hasher.update(hunk.path.to_string_lossy().as_bytes());
            hasher.update(hunk.before.as_bytes());
            hasher.update(hunk.after.as_bytes());
        }
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
        build_deterministic_compiler, open_infer_rocksdb, sha256_bytes, valid_witness_segment,
        AstDiff, CalibrationRecord, CalibrationStore, DiffHunk, Language, MeJepaInferConfig,
        RocksDbInferStore, TaskEnvironment, TaskId, TestId,
    };

    use super::*;

    #[test]
    fn latent_search_recovers_goal_matching_candidate() {
        let temp = tempfile::tempdir().unwrap();
        let repo_root = temp.path().join("repo");
        fs::create_dir_all(repo_root.join("src")).unwrap();
        let compiler = compiler_for(temp.path(), &repo_root);
        let context = context(&repo_root, "task-fp-103-unit");
        let candidates = vec![
            candidate(&repo_root, "candidate-a", b"candidate-a"),
            candidate(&repo_root, "candidate-b", b"candidate-b"),
            candidate(&repo_root, "candidate-c", b"candidate-c"),
        ];
        let goal_latent = derive_latent_action_vector(&candidates[1].patch, &context).unwrap();
        let result = search_latent_actions(
            &compiler,
            &context,
            candidates,
            LatentActionSearchConfig {
                goal_latent: Some(goal_latent),
                objective_weights: LatentActionObjectiveWeights {
                    predicted_oracle_pass: 0.05,
                    calibrated_confidence: 0.05,
                    in_distribution: 0.05,
                    hierarchy: 0.05,
                    goal_alignment: 0.80,
                },
                ..LatentActionSearchConfig::default()
            },
        )
        .unwrap();
        assert_eq!(result.selected_candidate_id, "candidate-b");
        assert_eq!(result.ranked_candidates[0].rank, 1);
        assert_eq!(result.ranked_candidates.len(), 3);
        assert_eq!(
            result.ranked_candidates[0].action_latent.len(),
            LATENT_ACTION_VECTOR_DIM
        );
        assert!(
            result.ranked_candidates[0].score_breakdown.goal_alignment
                >= result.ranked_candidates[1].score_breakdown.goal_alignment
        );
    }

    #[test]
    fn latent_search_rejects_too_few_candidates() {
        let temp = tempfile::tempdir().unwrap();
        let repo_root = temp.path().join("repo");
        fs::create_dir_all(repo_root.join("src")).unwrap();
        let compiler = compiler_for(temp.path(), &repo_root);
        let context = context(&repo_root, "task-fp-103-too-few");
        let err = search_latent_actions(
            &compiler,
            &context,
            vec![candidate(&repo_root, "candidate-a", b"candidate-a")],
            LatentActionSearchConfig::default(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("at least two candidates"));
    }

    #[test]
    fn latent_search_rejects_duplicate_candidate_ids() {
        let temp = tempfile::tempdir().unwrap();
        let repo_root = temp.path().join("repo");
        fs::create_dir_all(repo_root.join("src")).unwrap();
        let compiler = compiler_for(temp.path(), &repo_root);
        let context = context(&repo_root, "task-fp-103-duplicates");
        let err = search_latent_actions(
            &compiler,
            &context,
            vec![
                candidate(&repo_root, "candidate-a", b"candidate-a"),
                candidate(&repo_root, "candidate-a", b"candidate-b"),
            ],
            LatentActionSearchConfig::default(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("duplicate candidate id"));
    }

    fn compiler_for(root: &Path, repo_root: &Path) -> MeJepaCompiler {
        let db = open_infer_rocksdb(root.join("rocksdb")).unwrap();
        let calibration = CalibrationStore::new(db.clone(), 30).unwrap();
        calibration.persist(&calibration_record()).unwrap();
        let store = Arc::new(RocksDbInferStore::new(db));
        build_deterministic_compiler(
            repo_root.to_path_buf(),
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
        )
        .unwrap()
    }

    fn context(repo_root: &Path, task_id: &str) -> TaskContext {
        TaskContext {
            task_id: TaskId(task_id.to_string()),
            session_id: sha16(task_id.as_bytes()),
            language: Language::Python,
            problem_statement: "TASK-FP-103 unit: latent action search".to_string(),
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

    fn candidate(repo_root: &Path, candidate_id: &str, seed: &[u8]) -> LatentActionCandidate {
        let file_name = format!("src/{candidate_id}.py");
        let path = PathBuf::from(&file_name);
        let contents = format!(
            "def {candidate_id}():\n    return '{}'\n",
            hex::encode(seed)
        );
        fs::write(repo_root.join(&path), contents.as_bytes()).unwrap();
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
        )
        .unwrap();
        LatentActionCandidate::try_new(candidate_id, patch).unwrap()
    }

    fn calibration_record() -> CalibrationRecord {
        CalibrationRecord {
            version: "task-fp-103-latent-search-unit".to_string(),
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
}
