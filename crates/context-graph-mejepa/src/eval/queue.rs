use super::error::{EvalError, EvalErrorCode};
use super::novel_pattern::{
    admit_novel_pattern_clusters, ConstellationCentroid, NovelPatternCandidate,
    NovelPatternClusterAdmission, NovelPatternDetectorConfig,
};
use super::types::EvalObservation;
use crate::failure_fingerprint::{
    FingerprintCandidateScore, FingerprintClassification, FingerprintDecisionReason,
};
use crate::pause_state::prediction_is_operator_paused;
use crate::types::{
    EmbedderId, OracleOutcome, PredictionId, RealityPrediction, SurpriseSeverity, TaskId, Verdict,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_UNKNOWN_FINGERPRINT_EMBEDDERS: usize = 32;
const MAX_UNKNOWN_FINGERPRINT_VECTOR_DIMS: usize = 65_536;
const DEFAULT_UNKNOWN_FINGERPRINT_NEAR_MATCH_MARGIN: f32 = -0.02;
pub const CURIOSITY_TELEMETRY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnknownFingerprintCandidate {
    pub candidate_id: [u8; 16],
    pub prediction_id: PredictionId,
    pub task_id: TaskId,
    pub session_id: [u8; 16],
    pub observed_at_unix_ms: i64,
    pub ood_score: f32,
    pub embedder_disagreement_score: f32,
    pub active_learning_priority: u8,
    pub observation_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
    pub nearest_fingerprints: Vec<FingerprintCandidateScore>,
}

impl UnknownFingerprintCandidate {
    pub fn validate(&self) -> Result<(), EvalError> {
        self.task_id
            .validate("unknown_fingerprint_candidate.task_id")
            .map_err(EvalError::from)?;
        if self.candidate_id.iter().all(|byte| *byte == 0) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "unknown fingerprint candidate_id must be non-zero",
            ));
        }
        if self.prediction_id.0.iter().all(|byte| *byte == 0) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "unknown fingerprint prediction_id must be non-zero",
            ));
        }
        if self.session_id.iter().all(|byte| *byte == 0) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "unknown fingerprint session_id must be non-zero",
            ));
        }
        if self.observed_at_unix_ms <= 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "unknown fingerprint observed_at_unix_ms must be positive",
            ));
        }
        validate_probability("unknown_fingerprint_candidate.ood_score", self.ood_score)?;
        validate_nonnegative_finite(
            "unknown_fingerprint_candidate.embedder_disagreement_score",
            self.embedder_disagreement_score,
        )?;
        if !(1..=5).contains(&self.active_learning_priority) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "unknown fingerprint active_learning_priority must be in 1..=5",
            ));
        }
        validate_observation_map(&self.observation_by_embedder)?;
        for candidate in &self.nearest_fingerprints {
            candidate.validate().map_err(EvalError::from)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnknownFingerprintClusterSuggestion {
    pub cluster_id: [u8; 16],
    pub suggested_name: String,
    pub member_candidate_ids: Vec<[u8; 16]>,
    pub member_task_ids: Vec<TaskId>,
    pub centroid_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
    pub mean_ood_score: f32,
    pub mean_embedder_disagreement_score: f32,
    pub active_learning_priority: u8,
}

impl UnknownFingerprintClusterSuggestion {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.cluster_id.iter().all(|byte| *byte == 0) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "unknown fingerprint cluster_id must be non-zero",
            ));
        }
        validate_cell_id(&self.suggested_name)?;
        if self.member_candidate_ids.is_empty() || self.member_task_ids.is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "unknown fingerprint cluster must include at least one member",
            ));
        }
        if self.member_candidate_ids.len() != self.member_task_ids.len() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "unknown fingerprint cluster member id/task count mismatch",
            ));
        }
        validate_observation_map(&self.centroid_by_embedder)?;
        validate_probability(
            "unknown_fingerprint_cluster.mean_ood_score",
            self.mean_ood_score,
        )?;
        validate_nonnegative_finite(
            "unknown_fingerprint_cluster.mean_embedder_disagreement_score",
            self.mean_embedder_disagreement_score,
        )?;
        if !(1..=5).contains(&self.active_learning_priority) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "unknown fingerprint cluster active_learning_priority must be in 1..=5",
            ));
        }
        Ok(())
    }
}

/// REQ-FLYWHEEL-11 — discriminates how an entry landed on the active-learning
/// queue. The Uncertainty / OutOfDistribution variants are the existing
/// eval-runner-driven paths; AgentSurprise is the agent-feedback path
/// (TASK-PREDICT-002) that fast-tracks observe() for predictions the agent
/// flagged as wrong. The `severity_score` on AgentSurprise feeds the
/// REQ-FLYWHEEL-12 sampling-weight bump (`1 + 2 * severity_score`).
///
/// Additive variants must stay appended: bincode stores enum variant indexes,
/// and production queue rows must keep decoding after schema extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveLearningKind {
    Uncertainty,
    OutOfDistribution,
    AgentSurprise {
        prediction_id: PredictionId,
        severity_score: f32,
    },
    EwcProtectionViolation {
        violation_id: String,
        cell_id: String,
        projected_fisher_displacement: f32,
        budget: f32,
    },
    ColdCellTargetedCorpus {
        cell_id: String,
        abstain_count: u32,
    },
    UnknownFingerprint {
        candidate: Box<UnknownFingerprintCandidate>,
    },
    NovelCluster {
        candidate: Box<NovelPatternCandidate>,
    },
    OodHarvest {
        prediction_id: PredictionId,
        priority_weight: f32,
    },
    ConstellationDisagreement {
        pattern_id: String,
        contradiction_score: f32,
        novelty_score: f32,
        slot_pair_count: u32,
    },
}

impl ActiveLearningKind {
    /// Convenience constructor that maps a [`SurpriseSeverity`] enum into the
    /// (0.2 / 0.5 / 0.8 / 1.0) score the AgentSurprise variant carries.
    pub fn agent_surprise(prediction_id: PredictionId, severity: SurpriseSeverity) -> Self {
        Self::AgentSurprise {
            prediction_id,
            severity_score: severity.severity_score(),
        }
    }

    /// Fast-track ordering key: AgentSurprise > OutOfDistribution > Uncertainty.
    /// The scheduler's `tick_active_learning` consumes entries in this order so
    /// agent-flagged surprises are drained before any uncertainty backlog.
    pub fn fast_track_rank(&self) -> u8 {
        match self {
            Self::AgentSurprise { .. } => 0,
            Self::EwcProtectionViolation { .. } => 1,
            Self::OodHarvest { .. } => 2,
            Self::ConstellationDisagreement { .. } => 3,
            Self::NovelCluster { .. } => 4,
            Self::UnknownFingerprint { .. } => 5,
            Self::ColdCellTargetedCorpus { .. } => 6,
            Self::OutOfDistribution => 7,
            Self::Uncertainty => 8,
        }
    }

    pub fn validate(&self) -> Result<(), EvalError> {
        match self {
            Self::Uncertainty | Self::OutOfDistribution => Ok(()),
            Self::AgentSurprise {
                prediction_id,
                severity_score,
            } => {
                if prediction_id.0.iter().all(|byte| *byte == 0) {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        "agent surprise prediction_id must be non-zero",
                    ));
                }
                validate_probability(
                    "active_learning.agent_surprise.severity_score",
                    *severity_score,
                )
            }
            Self::EwcProtectionViolation {
                violation_id,
                cell_id,
                projected_fisher_displacement,
                budget,
            } => {
                if violation_id.trim().is_empty() {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        "EWC violation_id must be non-empty",
                    ));
                }
                validate_cell_id(cell_id)?;
                validate_nonnegative_finite(
                    "active_learning.ewc.projected_fisher_displacement",
                    *projected_fisher_displacement,
                )?;
                if !budget.is_finite() || *budget <= 0.0 {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        "EWC budget must be finite and positive",
                    ));
                }
                Ok(())
            }
            Self::ColdCellTargetedCorpus {
                cell_id,
                abstain_count,
            } => {
                validate_cell_id(cell_id)?;
                if *abstain_count == 0 {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        "cold-cell abstain_count must be greater than zero",
                    ));
                }
                Ok(())
            }
            Self::UnknownFingerprint { candidate } => candidate.validate(),
            Self::NovelCluster { candidate } => candidate.validate(),
            Self::OodHarvest {
                prediction_id,
                priority_weight,
            } => {
                if prediction_id.0.iter().all(|byte| *byte == 0) {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        "OOD harvest prediction_id must be non-zero",
                    ));
                }
                if !priority_weight.is_finite() || *priority_weight <= 0.0 {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        "OOD harvest priority_weight must be finite and positive",
                    ));
                }
                Ok(())
            }
            Self::ConstellationDisagreement {
                pattern_id,
                contradiction_score,
                novelty_score,
                slot_pair_count,
            } => {
                validate_cell_id(pattern_id)?;
                validate_probability(
                    "active_learning.constellation_disagreement.contradiction_score",
                    *contradiction_score,
                )?;
                validate_probability(
                    "active_learning.constellation_disagreement.novelty_score",
                    *novelty_score,
                )?;
                if *slot_pair_count == 0 {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        "constellation disagreement slot_pair_count must be non-zero",
                    ));
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveLearningRankBy {
    #[default]
    SchedulerPriority,
    Curiosity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveLearningQueueEntry {
    pub task_id: TaskId,
    pub score: f32,
    pub outcome_set_len: usize,
    pub ood_score: f32,
    pub curiosity_score: f32,
    pub reason: String,
    pub kind: ActiveLearningKind,
}

impl ActiveLearningQueueEntry {
    pub fn recompute_curiosity_score(&mut self, support_gap: f32) -> Result<(), EvalError> {
        self.curiosity_score = curiosity_score_from_proxy(
            conformal_width_proxy(self.outcome_set_len),
            self.ood_score,
            support_gap,
        )?;
        Ok(())
    }

    pub fn with_curiosity_score(mut self, curiosity_score: f32) -> Result<Self, EvalError> {
        validate_probability("active_learning.curiosity_score", curiosity_score)?;
        self.curiosity_score = curiosity_score;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), EvalError> {
        self.task_id
            .validate("active_learning.task_id")
            .map_err(EvalError::from)?;
        validate_nonnegative_finite("active_learning.score", self.score)?;
        validate_probability("active_learning.ood_score", self.ood_score)?;
        validate_probability("active_learning.curiosity_score", self.curiosity_score)?;
        if self.reason.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "active-learning reason must be non-empty",
            ));
        }
        self.kind.validate()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CuriosityDistribution {
    pub count: usize,
    pub min: f32,
    pub mean: f32,
    pub p50: f32,
    pub p90: f32,
    pub max: f32,
}

impl CuriosityDistribution {
    pub fn from_scores(scores: impl IntoIterator<Item = f32>) -> Result<Self, EvalError> {
        let mut scores = scores.into_iter().collect::<Vec<_>>();
        for score in &scores {
            validate_probability("curiosity_distribution.score", *score)?;
        }
        if scores.is_empty() {
            return Ok(Self {
                count: 0,
                min: 0.0,
                mean: 0.0,
                p50: 0.0,
                p90: 0.0,
                max: 0.0,
            });
        }
        scores.sort_by(|left, right| left.total_cmp(right));
        let count = scores.len();
        let mean = scores.iter().sum::<f32>() / count as f32;
        let p50 = scores[count / 2];
        let p90 = scores[((count - 1) as f32 * 0.90).ceil() as usize];
        Ok(Self {
            count,
            min: scores[0],
            mean,
            p50,
            p90,
            max: scores[count - 1],
        })
    }

    pub fn validate(&self) -> Result<(), EvalError> {
        validate_probability("curiosity_distribution.min", self.min)?;
        validate_probability("curiosity_distribution.mean", self.mean)?;
        validate_probability("curiosity_distribution.p50", self.p50)?;
        validate_probability("curiosity_distribution.p90", self.p90)?;
        validate_probability("curiosity_distribution.max", self.max)?;
        if self.min > self.mean || self.mean > self.max {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "curiosity distribution must satisfy min <= mean <= max",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CuriosityRankedEntry {
    pub rank: usize,
    pub task_id: TaskId,
    pub curiosity_score: f32,
    pub scheduler_score: f32,
    pub ood_score: f32,
    pub outcome_set_len: usize,
    pub reason: String,
}

impl CuriosityRankedEntry {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.rank == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "curiosity ranked entry rank must be one-based",
            ));
        }
        self.task_id
            .validate("curiosity_ranked_entry.task_id")
            .map_err(EvalError::from)?;
        validate_probability(
            "curiosity_ranked_entry.curiosity_score",
            self.curiosity_score,
        )?;
        validate_nonnegative_finite(
            "curiosity_ranked_entry.scheduler_score",
            self.scheduler_score,
        )?;
        validate_probability("curiosity_ranked_entry.ood_score", self.ood_score)?;
        if self.reason.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "curiosity ranked entry reason must be non-empty",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CuriosityCalibrationEvidence {
    pub task_id: TaskId,
    pub predicted_delta_cp: f32,
    pub actual_delta_cp: f32,
    pub tolerance: f32,
    pub within_tolerance: bool,
}

impl CuriosityCalibrationEvidence {
    pub fn new(
        task_id: TaskId,
        predicted_delta_cp: f32,
        actual_delta_cp: f32,
        tolerance: f32,
    ) -> Result<Self, EvalError> {
        validate_probability(
            "curiosity_calibration.predicted_delta_cp",
            predicted_delta_cp,
        )?;
        validate_probability("curiosity_calibration.actual_delta_cp", actual_delta_cp)?;
        validate_probability("curiosity_calibration.tolerance", tolerance)?;
        let within_tolerance =
            curiosity_delta_matches(predicted_delta_cp, actual_delta_cp, tolerance)?;
        Ok(Self {
            task_id,
            predicted_delta_cp,
            actual_delta_cp,
            tolerance,
            within_tolerance,
        })
    }

    pub fn validate(&self) -> Result<(), EvalError> {
        self.task_id
            .validate("curiosity_calibration.task_id")
            .map_err(EvalError::from)?;
        validate_probability(
            "curiosity_calibration.predicted_delta_cp",
            self.predicted_delta_cp,
        )?;
        validate_probability(
            "curiosity_calibration.actual_delta_cp",
            self.actual_delta_cp,
        )?;
        validate_probability("curiosity_calibration.tolerance", self.tolerance)?;
        if self.within_tolerance
            != curiosity_delta_matches(
                self.predicted_delta_cp,
                self.actual_delta_cp,
                self.tolerance,
            )?
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "curiosity calibration within_tolerance does not match predicted/actual delta",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CuriosityTelemetryWindow {
    pub schema_version: u32,
    pub window_id: String,
    pub generated_at_unix_ms: i64,
    pub entry_count: usize,
    pub curiosity_distribution: CuriosityDistribution,
    pub top_entries: Vec<CuriosityRankedEntry>,
    pub calibration_evidence: Vec<CuriosityCalibrationEvidence>,
    pub source_queue_cf: String,
    pub source_label_cf: String,
}

impl CuriosityTelemetryWindow {
    pub fn from_queue(
        window_id: impl Into<String>,
        generated_at_unix_ms: i64,
        queue: &ActiveLearningQueueState,
        top_n: usize,
    ) -> Result<Self, EvalError> {
        if top_n == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "curiosity telemetry top_n must be greater than zero",
            ));
        }
        let window_id = window_id.into();
        if window_id.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "curiosity telemetry window_id must be non-empty",
            ));
        }
        if generated_at_unix_ms <= 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "curiosity telemetry generated_at_unix_ms must be positive",
            ));
        }
        for entry in queue.entries.values() {
            entry.validate()?;
        }
        let curiosity_distribution = CuriosityDistribution::from_scores(
            queue.entries.values().map(|entry| entry.curiosity_score),
        )?;
        let top_entries = queue
            .ranked_entries(ActiveLearningRankBy::Curiosity)
            .into_iter()
            .take(top_n)
            .enumerate()
            .map(|(idx, entry)| CuriosityRankedEntry {
                rank: idx + 1,
                task_id: entry.task_id.clone(),
                curiosity_score: entry.curiosity_score,
                scheduler_score: entry.score,
                ood_score: entry.ood_score,
                outcome_set_len: entry.outcome_set_len,
                reason: entry.reason.clone(),
            })
            .collect::<Vec<_>>();
        let window = Self {
            schema_version: CURIOSITY_TELEMETRY_SCHEMA_VERSION,
            window_id,
            generated_at_unix_ms,
            entry_count: queue.entries.len(),
            curiosity_distribution,
            top_entries,
            calibration_evidence: Vec::new(),
            source_queue_cf: context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE.to_string(),
            source_label_cf: context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_LABELS.to_string(),
        };
        window.validate()?;
        Ok(window)
    }

    pub fn with_calibration_evidence(
        mut self,
        evidence: Vec<CuriosityCalibrationEvidence>,
    ) -> Result<Self, EvalError> {
        self.calibration_evidence = evidence;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), EvalError> {
        if self.schema_version != CURIOSITY_TELEMETRY_SCHEMA_VERSION {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "curiosity telemetry schema_version must be {CURIOSITY_TELEMETRY_SCHEMA_VERSION}"
                ),
            ));
        }
        if self.window_id.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "curiosity telemetry window_id must be non-empty",
            ));
        }
        if self.generated_at_unix_ms <= 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "curiosity telemetry generated_at_unix_ms must be positive",
            ));
        }
        self.curiosity_distribution.validate()?;
        if self.entry_count != self.curiosity_distribution.count {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "curiosity telemetry entry_count must match distribution count",
            ));
        }
        let mut previous_score = f32::INFINITY;
        for entry in &self.top_entries {
            entry.validate()?;
            if entry.curiosity_score > previous_score {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    "curiosity telemetry top_entries must be sorted descending",
                ));
            }
            previous_score = entry.curiosity_score;
        }
        for evidence in &self.calibration_evidence {
            evidence.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelMethod {
    Human,
    OracleReplay,
    VerifiedWitness,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveLearningLabel {
    pub task_id: TaskId,
    pub oracle_outcome: OracleOutcome,
    pub method: LabelMethod,
    pub labeled_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ActiveLearningQueueState {
    pub capacity: usize,
    pub entries: BTreeMap<TaskId, ActiveLearningQueueEntry>,
    pub evicted: Vec<ActiveLearningQueueEntry>,
    pub ood_escalations: Vec<ActiveLearningQueueEntry>,
}

impl ActiveLearningQueueState {
    pub fn new(capacity: usize) -> Result<Self, EvalError> {
        if capacity == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "active learning queue capacity must be greater than zero",
            ));
        }
        Ok(Self {
            capacity,
            entries: BTreeMap::new(),
            evicted: Vec::new(),
            ood_escalations: Vec::new(),
        })
    }

    pub fn enqueue_uncertain(
        &mut self,
        observation: &EvalObservation,
        uncertainty_outcome_len: usize,
        ood_escalation_threshold: f32,
    ) -> Result<(), EvalError> {
        if prediction_is_operator_paused(&observation.prediction) {
            return Ok(());
        }
        if uncertainty_outcome_len < 2 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "uncertainty_outcome_len must be at least two",
            ));
        }
        if !ood_escalation_threshold.is_finite() || !(0.0..=1.0).contains(&ood_escalation_threshold)
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "ood_escalation_threshold must be in [0,1]",
            ));
        }
        let outcome_set_len = observation.prediction.outcome_set.outcomes.len();
        let ood_score = observation.prediction.ood_score;
        let max_len = outcome_set_len.max(uncertainty_outcome_len) as f32;
        let is_ood = ood_score >= ood_escalation_threshold;
        let entry = ActiveLearningQueueEntry {
            task_id: observation.task_id.clone(),
            score: ((outcome_set_len as f32 / max_len) * (1.0 - ood_score)).clamp(0.0, 1.0),
            outcome_set_len,
            ood_score,
            curiosity_score: curiosity_score_from_proxy(
                conformal_width_proxy(outcome_set_len),
                ood_score,
                1.0,
            )?,
            reason: if is_ood {
                "ood_escalation".to_string()
            } else {
                "wide_conformal_set".to_string()
            },
            kind: if is_ood {
                ActiveLearningKind::OutOfDistribution
            } else {
                ActiveLearningKind::Uncertainty
            },
        };
        if is_ood {
            self.ood_escalations.push(entry);
            return Ok(());
        }
        if outcome_set_len < uncertainty_outcome_len {
            return Ok(());
        }
        self.entries.insert(observation.task_id.clone(), entry);
        self.evict_to_capacity()?;
        Ok(())
    }

    pub fn enqueue_agent_surprise_for_prediction(
        &mut self,
        prediction: &RealityPrediction,
        severity: SurpriseSeverity,
    ) -> Result<bool, EvalError> {
        if prediction_is_operator_paused(prediction) {
            return Ok(false);
        }
        self.enqueue_agent_surprise(
            prediction.task_id.clone(),
            PredictionId(prediction.prediction_id),
            severity,
        )?;
        Ok(true)
    }

    /// REQ-FLYWHEEL-11 / TASK-FLYWHEEL-005 — enqueue an agent-flagged surprise.
    /// Fast-tracked ahead of Uncertainty / OOD entries via
    /// [`ActiveLearningKind::fast_track_rank`]; the score is pinned to the
    /// severity (so Catastrophic surprises always sort to the top regardless
    /// of the queue's other ordering criteria).
    pub fn enqueue_agent_surprise(
        &mut self,
        task_id: TaskId,
        prediction_id: PredictionId,
        severity: SurpriseSeverity,
    ) -> Result<(), EvalError> {
        if task_id.0.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "agent surprise task_id must be non-empty",
            ));
        }
        let severity_score = severity.severity_score();
        let entry = ActiveLearningQueueEntry {
            task_id: task_id.clone(),
            score: severity_score,
            outcome_set_len: 0,
            ood_score: 0.0,
            curiosity_score: curiosity_score_from_proxy(0.0, 0.0, 1.0)?,
            reason: format!("agent_surprise:{severity:?}"),
            kind: ActiveLearningKind::AgentSurprise {
                prediction_id,
                severity_score,
            },
        };
        self.entries.insert(task_id, entry);
        self.evict_to_capacity()?;
        Ok(())
    }

    pub fn enqueue_ewc_protection_violation(
        &mut self,
        violation_id: String,
        cell_id: String,
        projected_fisher_displacement: f32,
        budget: f32,
    ) -> Result<(), EvalError> {
        if violation_id.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "EWC violation_id must be non-empty",
            ));
        }
        validate_cell_id(&cell_id)?;
        validate_nonnegative_finite(
            "active_learning.ewc.projected_fisher_displacement",
            projected_fisher_displacement,
        )?;
        if !budget.is_finite() || budget <= 0.0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "EWC budget must be finite and positive",
            ));
        }
        let digest = Sha256::digest(violation_id.as_bytes());
        let task_id = TaskId(format!("ewc-protection-{}", hex::encode(&digest[..8])));
        let severity_score = (projected_fisher_displacement / budget).clamp(0.0, 1.0);
        let entry = ActiveLearningQueueEntry {
            task_id: task_id.clone(),
            score: severity_score,
            outcome_set_len: 0,
            ood_score: 0.0,
            curiosity_score: curiosity_score_from_proxy(0.0, 0.0, 1.0)?,
            reason: format!("ewc_protection_violation:{violation_id}"),
            kind: ActiveLearningKind::EwcProtectionViolation {
                violation_id,
                cell_id,
                projected_fisher_displacement,
                budget,
            },
        };
        entry.validate()?;
        self.entries.insert(task_id, entry);
        self.evict_to_capacity()?;
        Ok(())
    }

    pub fn enqueue_cold_cell_targeted_corpus(
        &mut self,
        cell_id: String,
        abstain_count: u32,
    ) -> Result<(), EvalError> {
        validate_cell_id(&cell_id)?;
        if abstain_count == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "cold-cell abstain_count must be greater than zero",
            ));
        }
        let digest = Sha256::digest(cell_id.as_bytes());
        let task_id = TaskId(format!(
            "cold-cell-targeted-corpus-{}",
            hex::encode(&digest[..8])
        ));
        let entry = ActiveLearningQueueEntry {
            task_id: task_id.clone(),
            score: 1.0,
            outcome_set_len: 1,
            ood_score: 0.0,
            curiosity_score: curiosity_score_from_proxy(0.0, 0.0, 1.0)?,
            reason: format!("cold_cell_targeted_corpus:{cell_id}"),
            kind: ActiveLearningKind::ColdCellTargetedCorpus {
                cell_id,
                abstain_count,
            },
        };
        self.entries.insert(task_id, entry);
        self.evict_to_capacity()?;
        Ok(())
    }

    pub fn enqueue_unknown_fingerprint_candidate(
        &mut self,
        prediction: &RealityPrediction,
        classification: &FingerprintClassification,
        observation_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
        observed_at_unix_ms: i64,
    ) -> Result<bool, EvalError> {
        if prediction_is_operator_paused(prediction) {
            return Ok(false);
        }
        prediction.validate().map_err(EvalError::from)?;
        classification.validate().map_err(EvalError::from)?;
        if classification.verdict != Verdict::OutOfDistribution
            || classification.reason != FingerprintDecisionReason::NoKnownMatch
        {
            return Ok(false);
        }
        validate_observation_map(&observation_by_embedder)?;
        if observed_at_unix_ms <= 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "unknown fingerprint observed_at_unix_ms must be positive",
            ));
        }
        let embedder_disagreement_score = embedder_disagreement_score(classification)?;
        let active_learning_priority = unknown_fingerprint_priority(
            prediction.ood_score,
            embedder_disagreement_score,
            classification,
        )?;
        let candidate_id = unknown_candidate_id(
            &prediction.task_id,
            PredictionId(prediction.prediction_id),
            observed_at_unix_ms,
            &observation_by_embedder,
        );
        let candidate = UnknownFingerprintCandidate {
            candidate_id,
            prediction_id: PredictionId(prediction.prediction_id),
            task_id: prediction.task_id.clone(),
            session_id: prediction.session_id,
            observed_at_unix_ms,
            ood_score: prediction.ood_score,
            embedder_disagreement_score,
            active_learning_priority,
            observation_by_embedder,
            nearest_fingerprints: classification.ranked_matches.clone(),
        };
        candidate.validate()?;
        let entry = ActiveLearningQueueEntry {
            task_id: prediction.task_id.clone(),
            score: active_learning_priority_score(active_learning_priority),
            outcome_set_len: prediction.outcome_set.outcomes.len(),
            ood_score: prediction.ood_score,
            curiosity_score: curiosity_score_from_proxy(
                conformal_width_proxy(prediction.outcome_set.outcomes.len()),
                prediction.ood_score,
                active_learning_priority_score(active_learning_priority),
            )?,
            reason: "fingerprint_unknown_ood".to_string(),
            kind: ActiveLearningKind::UnknownFingerprint {
                candidate: Box::new(candidate),
            },
        };
        self.entries.insert(prediction.task_id.clone(), entry);
        self.evict_to_capacity()?;
        Ok(true)
    }

    pub fn enqueue_ood_harvest(
        &mut self,
        harvest: &crate::ood_harvest::OodHarvestRow,
    ) -> Result<bool, EvalError> {
        harvest.validate()?;
        let task_id =
            crate::ood_harvest::ood_harvest_active_learning_task_id(harvest.prediction_id);
        let entry = ActiveLearningQueueEntry {
            task_id: task_id.clone(),
            score: crate::ood_harvest::OOD_HARVEST_ACTIVE_LEARNING_WEIGHT,
            outcome_set_len: harvest.affected_chunk_ids.len().max(1),
            ood_score: harvest.ood_score,
            curiosity_score: 1.0,
            reason: format!("ood_harvest:{}", hex::encode(harvest.prediction_id.0)),
            kind: ActiveLearningKind::OodHarvest {
                prediction_id: harvest.prediction_id,
                priority_weight: crate::ood_harvest::OOD_HARVEST_ACTIVE_LEARNING_WEIGHT,
            },
        };
        entry.validate()?;
        self.entries.insert(task_id, entry);
        self.evict_to_capacity()?;
        Ok(true)
    }

    pub fn enqueue_constellation_disagreement(
        &mut self,
        pattern_id: String,
        contradiction_score: f32,
        novelty_score: f32,
        slot_pair_count: u32,
    ) -> Result<(), EvalError> {
        validate_cell_id(&pattern_id)?;
        validate_probability(
            "active_learning.constellation_disagreement.contradiction_score",
            contradiction_score,
        )?;
        validate_probability(
            "active_learning.constellation_disagreement.novelty_score",
            novelty_score,
        )?;
        if slot_pair_count == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "constellation disagreement slot_pair_count must be non-zero",
            ));
        }
        let digest = Sha256::digest(pattern_id.as_bytes());
        let task_id = TaskId(format!(
            "constellation-disagreement-{}",
            hex::encode(&digest[..8])
        ));
        let support_gap = contradiction_score.max(novelty_score).clamp(0.0, 1.0);
        let entry = ActiveLearningQueueEntry {
            task_id: task_id.clone(),
            score: support_gap,
            outcome_set_len: slot_pair_count as usize,
            ood_score: contradiction_score,
            curiosity_score: curiosity_score_from_proxy(0.0, contradiction_score, support_gap)?,
            reason:
                crate::constellation_intelligence::CONSTELLATION_DISAGREEMENT_ACTIVE_LEARNING_REASON
                    .to_string(),
            kind: ActiveLearningKind::ConstellationDisagreement {
                pattern_id,
                contradiction_score,
                novelty_score,
                slot_pair_count,
            },
        };
        entry.validate()?;
        self.entries.insert(task_id, entry);
        self.evict_to_capacity()?;
        Ok(())
    }

    pub fn enqueue_novel_pattern_candidate(
        &mut self,
        prediction: &RealityPrediction,
        observation_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
        existing_cells: &[ConstellationCentroid],
        observed_at_unix_ms: i64,
        config: NovelPatternDetectorConfig,
    ) -> Result<bool, EvalError> {
        if prediction_is_operator_paused(prediction) {
            return Ok(false);
        }
        let Some(candidate) = NovelPatternCandidate::from_prediction(
            prediction,
            observation_by_embedder,
            existing_cells,
            observed_at_unix_ms,
            config,
        )?
        else {
            return Ok(false);
        };
        let entry = ActiveLearningQueueEntry {
            task_id: prediction.task_id.clone(),
            score: active_learning_priority_score(candidate.active_learning_priority),
            outcome_set_len: prediction.outcome_set.outcomes.len(),
            ood_score: prediction.ood_score,
            curiosity_score: curiosity_score_from_proxy(
                conformal_width_proxy(prediction.outcome_set.outcomes.len()),
                prediction.ood_score,
                active_learning_priority_score(candidate.active_learning_priority),
            )?,
            reason: "novel_pattern".to_string(),
            kind: ActiveLearningKind::NovelCluster {
                candidate: Box::new(candidate),
            },
        };
        self.entries.insert(prediction.task_id.clone(), entry);
        self.evict_to_capacity()?;
        Ok(true)
    }

    pub fn admit_novel_pattern_clusters(
        &self,
        existing_cells: &[ConstellationCentroid],
        config: NovelPatternDetectorConfig,
    ) -> Result<Vec<NovelPatternClusterAdmission>, EvalError> {
        let candidates = self
            .entries
            .values()
            .filter_map(|entry| match &entry.kind {
                ActiveLearningKind::NovelCluster { candidate } => Some((**candidate).clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        admit_novel_pattern_clusters(&candidates, existing_cells, config)
    }

    pub fn suggest_unknown_fingerprint_clusters(
        &self,
        min_cluster_size: usize,
        cosine_threshold: f32,
    ) -> Result<Vec<UnknownFingerprintClusterSuggestion>, EvalError> {
        if min_cluster_size == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "unknown fingerprint min_cluster_size must be greater than zero",
            ));
        }
        validate_probability(
            "unknown_fingerprint_cluster.cosine_threshold",
            cosine_threshold,
        )?;
        let candidates = self
            .entries
            .values()
            .filter_map(|entry| match &entry.kind {
                ActiveLearningKind::UnknownFingerprint { candidate } => Some(candidate.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut assigned = BTreeSet::new();
        let mut suggestions = Vec::new();
        for seed in &candidates {
            seed.validate()?;
            if assigned.contains(&seed.candidate_id) {
                continue;
            }
            let mut cluster = vec![*seed];
            for candidate in &candidates {
                candidate.validate()?;
                if candidate.candidate_id == seed.candidate_id
                    || assigned.contains(&candidate.candidate_id)
                {
                    continue;
                }
                let similarity = mean_observation_cosine(
                    &seed.observation_by_embedder,
                    &candidate.observation_by_embedder,
                )?;
                if similarity >= cosine_threshold {
                    cluster.push(*candidate);
                }
            }
            if cluster.len() < min_cluster_size {
                continue;
            }
            for candidate in &cluster {
                assigned.insert(candidate.candidate_id);
            }
            suggestions.push(build_unknown_cluster_suggestion(&cluster)?);
        }
        Ok(suggestions)
    }

    pub fn ranked_entries(
        &self,
        ranked_by: ActiveLearningRankBy,
    ) -> Vec<&ActiveLearningQueueEntry> {
        let mut rows = self.entries.values().collect::<Vec<_>>();
        match ranked_by {
            ActiveLearningRankBy::SchedulerPriority => rows.sort_by(|left, right| {
                left.kind
                    .fast_track_rank()
                    .cmp(&right.kind.fast_track_rank())
                    .then_with(|| right.score.total_cmp(&left.score))
                    .then_with(|| left.task_id.cmp(&right.task_id))
            }),
            ActiveLearningRankBy::Curiosity => rows.sort_by(|left, right| {
                right
                    .curiosity_score
                    .total_cmp(&left.curiosity_score)
                    .then_with(|| {
                        left.kind
                            .fast_track_rank()
                            .cmp(&right.kind.fast_track_rank())
                    })
                    .then_with(|| right.score.total_cmp(&left.score))
                    .then_with(|| left.task_id.cmp(&right.task_id))
            }),
        }
        rows
    }

    fn evict_to_capacity(&mut self) -> Result<(), EvalError> {
        while self.entries.len() > self.capacity {
            let lowest = self
                .entries
                .iter()
                .min_by(|a, b| {
                    // Fast-tracked kinds (lower rank) MUST never be evicted in
                    // favor of uncertainty rows. Compare rank first, score
                    // second so AgentSurprise wins ties.
                    let lhs_rank = a.1.kind.fast_track_rank();
                    let rhs_rank = b.1.kind.fast_track_rank();
                    rhs_rank
                        .cmp(&lhs_rank)
                        .then_with(|| a.1.score.total_cmp(&b.1.score))
                        .then_with(|| a.0.cmp(b.0))
                })
                .map(|(task_id, _)| task_id.clone())
                .ok_or_else(|| EvalError::new(EvalErrorCode::Store, "queue unexpectedly empty"))?;
            if let Some(evicted) = self.entries.remove(&lowest) {
                self.evicted.push(evicted);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyActiveLearningQueueEntry {
    pub task_id: TaskId,
    pub score: f32,
    pub outcome_set_len: usize,
    pub ood_score: f32,
    pub reason: String,
    pub kind: ActiveLearningKind,
}

impl From<LegacyActiveLearningQueueEntry> for ActiveLearningQueueEntry {
    fn from(entry: LegacyActiveLearningQueueEntry) -> Self {
        let support_gap = match &entry.kind {
            ActiveLearningKind::EwcProtectionViolation { .. } => 1.0,
            ActiveLearningKind::UnknownFingerprint { candidate } => {
                active_learning_priority_score(candidate.active_learning_priority)
            }
            ActiveLearningKind::NovelCluster { candidate } => {
                active_learning_priority_score(candidate.active_learning_priority)
            }
            ActiveLearningKind::OodHarvest {
                priority_weight, ..
            } => *priority_weight,
            ActiveLearningKind::ConstellationDisagreement {
                contradiction_score,
                novelty_score,
                ..
            } => contradiction_score.max(*novelty_score),
            _ => 1.0,
        };
        let curiosity_score = curiosity_score_from_proxy(
            conformal_width_proxy(entry.outcome_set_len),
            entry.ood_score,
            support_gap,
        )
        .unwrap_or(0.0);
        Self {
            task_id: entry.task_id,
            score: entry.score,
            outcome_set_len: entry.outcome_set_len,
            ood_score: entry.ood_score,
            curiosity_score,
            reason: entry.reason,
            kind: entry.kind,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyActiveLearningQueueState {
    pub capacity: usize,
    pub entries: BTreeMap<TaskId, LegacyActiveLearningQueueEntry>,
    pub evicted: Vec<LegacyActiveLearningQueueEntry>,
    pub ood_escalations: Vec<LegacyActiveLearningQueueEntry>,
}

impl From<LegacyActiveLearningQueueState> for ActiveLearningQueueState {
    fn from(state: LegacyActiveLearningQueueState) -> Self {
        Self {
            capacity: state.capacity,
            entries: state
                .entries
                .into_iter()
                .map(|(task_id, entry)| (task_id, entry.into()))
                .collect(),
            evicted: state.evicted.into_iter().map(Into::into).collect(),
            ood_escalations: state.ood_escalations.into_iter().map(Into::into).collect(),
        }
    }
}

pub fn conformal_width_proxy(outcome_set_len: usize) -> f32 {
    if outcome_set_len <= 1 {
        0.0
    } else {
        ((outcome_set_len - 1) as f32 / outcome_set_len as f32).clamp(0.0, 1.0)
    }
}

pub fn curiosity_score_from_proxy(
    conformal_interval_width: f32,
    ood_score: f32,
    support_gap: f32,
) -> Result<f32, EvalError> {
    validate_probability(
        "curiosity.conformal_interval_width",
        conformal_interval_width,
    )?;
    validate_probability("curiosity.ood_score", ood_score)?;
    validate_probability("curiosity.support_gap", support_gap)?;
    Ok((conformal_interval_width * ood_score * support_gap).clamp(0.0, 1.0))
}

pub fn curiosity_delta_matches(
    predicted_delta_cp: f32,
    actual_delta_cp: f32,
    tolerance: f32,
) -> Result<bool, EvalError> {
    validate_probability("curiosity.predicted_delta_cp", predicted_delta_cp)?;
    validate_probability("curiosity.actual_delta_cp", actual_delta_cp)?;
    validate_probability("curiosity.tolerance", tolerance)?;
    Ok((predicted_delta_cp - actual_delta_cp).abs() <= tolerance)
}

pub fn curiosity_telemetry_window_key(
    window: &CuriosityTelemetryWindow,
) -> Result<Vec<u8>, EvalError> {
    window.validate()?;
    Ok(format!("{:020}::{}", window.generated_at_unix_ms, window.window_id).into_bytes())
}

pub fn render_curiosity_ranking_weekly_section(
    queue: &ActiveLearningQueueState,
    top_n: usize,
) -> Result<String, EvalError> {
    if top_n == 0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "curiosity weekly top_n must be greater than zero",
        ));
    }
    let rows = queue
        .ranked_entries(ActiveLearningRankBy::Curiosity)
        .into_iter()
        .take(top_n)
        .enumerate()
        .map(|(idx, entry)| {
            entry.validate()?;
            Ok(format!(
                "| {} | {} | {:.6} | {:.6} | {:.6} | {} |",
                idx + 1,
                entry.task_id.0,
                entry.curiosity_score,
                entry.score,
                entry.ood_score,
                entry.reason.replace('|', "/")
            ))
        })
        .collect::<Result<Vec<_>, EvalError>>()?;
    let mut section = String::from(
        "## Curiosity Ranking\n\n| rank | task_id | curiosity_score | scheduler_score | ood_score | reason |\n| --- | --- | ---: | ---: | ---: | --- |\n",
    );
    if rows.is_empty() {
        section.push_str("| 0 | none | 0.000000 | 0.000000 | 0.000000 | empty_queue |\n");
    } else {
        section.push_str(&rows.join("\n"));
        section.push('\n');
    }
    section.push_str(
        "\n- source_of_truth: CF_MEJEPA_CURIOSITY_TELEMETRY / CF_MEJEPA_ACTIVE_LEARNING_QUEUE\n",
    );
    Ok(section)
}

fn validate_cell_id(cell_id: &str) -> Result<(), EvalError> {
    if cell_id.trim().is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidConfig,
            "cold-cell cell_id must be non-empty",
        ));
    }
    if cell_id.len() > 512 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidConfig,
            "cold-cell cell_id exceeds 512 bytes",
        ));
    }
    if cell_id.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidConfig,
            "cold-cell cell_id contains a control character",
        ));
    }
    Ok(())
}

fn validate_probability(field: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{field} must be finite and in [0,1]; got {value}"),
        ));
    }
    Ok(())
}

fn validate_nonnegative_finite(field: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || value < 0.0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{field} must be finite and non-negative; got {value}"),
        ));
    }
    Ok(())
}

fn validate_observation_map(
    observation_by_embedder: &BTreeMap<EmbedderId, Vec<f32>>,
) -> Result<(), EvalError> {
    if observation_by_embedder.is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "unknown fingerprint observation map must be non-empty",
        ));
    }
    if observation_by_embedder.len() > MAX_UNKNOWN_FINGERPRINT_EMBEDDERS {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!(
                "unknown fingerprint observation has {} embedders; max {MAX_UNKNOWN_FINGERPRINT_EMBEDDERS}",
                observation_by_embedder.len()
            ),
        ));
    }
    for (embedder, vector) in observation_by_embedder {
        embedder
            .validate("unknown_fingerprint.embedder")
            .map_err(EvalError::from)?;
        if vector.is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("unknown fingerprint vector for {} is empty", embedder.0),
            ));
        }
        if vector.len() > MAX_UNKNOWN_FINGERPRINT_VECTOR_DIMS {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "unknown fingerprint vector for {} exceeds {MAX_UNKNOWN_FINGERPRINT_VECTOR_DIMS} dims",
                    embedder.0
                ),
            ));
        }
        let mut norm = 0.0_f64;
        for value in vector {
            if !value.is_finite() {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "unknown fingerprint vector for {} contains {value}",
                        embedder.0
                    ),
                ));
            }
            norm += f64::from(*value) * f64::from(*value);
        }
        if norm <= f64::EPSILON {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "unknown fingerprint vector for {} has zero norm",
                    embedder.0
                ),
            ));
        }
    }
    Ok(())
}

fn embedder_disagreement_score(
    classification: &FingerprintClassification,
) -> Result<f32, EvalError> {
    let Some(nearest) = classification.ranked_matches.first() else {
        return Ok(0.0);
    };
    if nearest.embedder_scores.is_empty() {
        return Ok(0.0);
    }
    let mean = nearest
        .embedder_scores
        .iter()
        .map(|score| score.cosine)
        .sum::<f32>()
        / nearest.embedder_scores.len() as f32;
    let variance = nearest
        .embedder_scores
        .iter()
        .map(|score| {
            let delta = score.cosine - mean;
            delta * delta
        })
        .sum::<f32>()
        / nearest.embedder_scores.len() as f32;
    let score = variance.sqrt().clamp(0.0, 1.0);
    validate_probability("unknown_fingerprint.embedder_disagreement_score", score)?;
    Ok(score)
}

fn unknown_fingerprint_priority(
    ood_score: f32,
    embedder_disagreement_score: f32,
    classification: &FingerprintClassification,
) -> Result<u8, EvalError> {
    validate_probability("unknown_fingerprint.ood_score", ood_score)?;
    validate_probability(
        "unknown_fingerprint.embedder_disagreement_score",
        embedder_disagreement_score,
    )?;
    let near_match = classification
        .ranked_matches
        .first()
        .is_some_and(|candidate| {
            candidate.min_margin >= DEFAULT_UNKNOWN_FINGERPRINT_NEAR_MATCH_MARGIN
        });
    let priority = if near_match {
        4
    } else if ood_score >= 0.90 || embedder_disagreement_score >= 0.25 {
        2
    } else {
        3
    };
    Ok(priority)
}

fn active_learning_priority_score(priority: u8) -> f32 {
    match priority {
        1 => 1.0,
        2 => 0.75,
        3 => 0.50,
        4 => 0.25,
        _ => 0.0,
    }
}

fn unknown_candidate_id(
    task_id: &TaskId,
    prediction_id: PredictionId,
    observed_at_unix_ms: i64,
    observation_by_embedder: &BTreeMap<EmbedderId, Vec<f32>>,
) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_UNKNOWN_FINGERPRINT_CANDIDATE_V1");
    hasher.update(task_id.0.as_bytes());
    hasher.update(prediction_id.0);
    hasher.update(observed_at_unix_ms.to_be_bytes());
    for (embedder, vector) in observation_by_embedder {
        hasher.update(embedder.0.as_bytes());
        hasher.update((vector.len() as u64).to_be_bytes());
        for value in vector {
            hasher.update(value.to_bits().to_be_bytes());
        }
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn mean_observation_cosine(
    left: &BTreeMap<EmbedderId, Vec<f32>>,
    right: &BTreeMap<EmbedderId, Vec<f32>>,
) -> Result<f32, EvalError> {
    if left.keys().collect::<Vec<_>>() != right.keys().collect::<Vec<_>>() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "unknown fingerprint observations must have identical embedder sets to cluster",
        ));
    }
    let mut total = 0.0_f32;
    for (embedder, left_vector) in left {
        let right_vector = right.get(embedder).ok_or_else(|| {
            EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("missing right observation for embedder {}", embedder.0),
            )
        })?;
        total += vector_cosine(left_vector, right_vector, &embedder.0)?;
    }
    Ok(total / left.len() as f32)
}

fn vector_cosine(left: &[f32], right: &[f32], context: &str) -> Result<f32, EvalError> {
    if left.len() != right.len() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!(
                "unknown fingerprint vector dim mismatch for {context}: {} != {}",
                left.len(),
                right.len()
            ),
        ));
    }
    let mut dot = 0.0_f64;
    let mut left_norm = 0.0_f64;
    let mut right_norm = 0.0_f64;
    for (left_value, right_value) in left.iter().zip(right.iter()) {
        if !left_value.is_finite() || !right_value.is_finite() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("unknown fingerprint vector for {context} contains a non-finite value"),
            ));
        }
        let left_f64 = f64::from(*left_value);
        let right_f64 = f64::from(*right_value);
        dot += left_f64 * right_f64;
        left_norm += left_f64 * left_f64;
        right_norm += right_f64 * right_f64;
    }
    if left_norm <= f64::EPSILON || right_norm <= f64::EPSILON {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("unknown fingerprint vector for {context} has zero norm"),
        ));
    }
    Ok((dot / (left_norm.sqrt() * right_norm.sqrt())).clamp(-1.0, 1.0) as f32)
}

fn build_unknown_cluster_suggestion(
    cluster: &[&UnknownFingerprintCandidate],
) -> Result<UnknownFingerprintClusterSuggestion, EvalError> {
    let first = cluster.first().ok_or_else(|| {
        EvalError::new(
            EvalErrorCode::InvalidInput,
            "unknown fingerprint cluster cannot be empty",
        )
    })?;
    let mut sums = first
        .observation_by_embedder
        .iter()
        .map(|(embedder, vector)| (embedder.clone(), vec![0.0_f64; vector.len()]))
        .collect::<BTreeMap<_, _>>();
    let mut member_candidate_ids = Vec::with_capacity(cluster.len());
    let mut member_task_ids = Vec::with_capacity(cluster.len());
    let mut mean_ood_score = 0.0_f32;
    let mut mean_embedder_disagreement_score = 0.0_f32;
    let mut min_priority = 5_u8;
    for candidate in cluster {
        candidate.validate()?;
        if candidate.observation_by_embedder.keys().collect::<Vec<_>>()
            != first.observation_by_embedder.keys().collect::<Vec<_>>()
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "unknown fingerprint cluster members have different embedder sets",
            ));
        }
        member_candidate_ids.push(candidate.candidate_id);
        member_task_ids.push(candidate.task_id.clone());
        mean_ood_score += candidate.ood_score;
        mean_embedder_disagreement_score += candidate.embedder_disagreement_score;
        min_priority = min_priority.min(candidate.active_learning_priority);
        for (embedder, vector) in &candidate.observation_by_embedder {
            let Some(sum) = sums.get_mut(embedder) else {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "unknown fingerprint cluster missing embedder {}",
                        embedder.0
                    ),
                ));
            };
            if sum.len() != vector.len() {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "unknown fingerprint cluster dim mismatch for {}",
                        embedder.0
                    ),
                ));
            }
            for (idx, value) in vector.iter().enumerate() {
                sum[idx] += f64::from(*value);
            }
        }
    }
    let centroid_by_embedder = sums
        .into_iter()
        .map(|(embedder, mut vector)| {
            let norm = vector.iter().map(|value| value * value).sum::<f64>().sqrt();
            if norm <= f64::EPSILON {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "unknown fingerprint cluster centroid for {} has zero norm",
                        embedder.0
                    ),
                ));
            }
            let centroid = vector
                .drain(..)
                .map(|value| (value / norm) as f32)
                .collect::<Vec<_>>();
            Ok((embedder, centroid))
        })
        .collect::<Result<BTreeMap<_, _>, EvalError>>()?;
    let cluster_id = unknown_cluster_id(&member_candidate_ids);
    let priority = if cluster.len() >= 10 { 1 } else { min_priority };
    let suggestion = UnknownFingerprintClusterSuggestion {
        cluster_id,
        suggested_name: format!("unknown-fingerprint-{}", hex::encode(&cluster_id[..8])),
        member_candidate_ids,
        member_task_ids,
        centroid_by_embedder,
        mean_ood_score: mean_ood_score / cluster.len() as f32,
        mean_embedder_disagreement_score: mean_embedder_disagreement_score / cluster.len() as f32,
        active_learning_priority: priority,
    };
    suggestion.validate()?;
    Ok(suggestion)
}

fn unknown_cluster_id(member_candidate_ids: &[[u8; 16]]) -> [u8; 16] {
    let mut ids = member_candidate_ids.to_vec();
    ids.sort();
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_UNKNOWN_FINGERPRINT_CLUSTER_V1");
    for id in ids {
        hasher.update(id);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pred(id: u8) -> PredictionId {
        PredictionId([id; 16])
    }

    #[test]
    fn enqueue_agent_surprise_uses_agent_surprise_kind_and_severity_score() {
        let mut queue = ActiveLearningQueueState::new(8).expect("queue");
        queue
            .enqueue_agent_surprise(
                TaskId("synthetic-task-1".to_string()),
                pred(0x11),
                SurpriseSeverity::High,
            )
            .expect("enqueue surprise");
        let entry = queue
            .entries
            .get(&TaskId("synthetic-task-1".to_string()))
            .expect("entry present");
        match &entry.kind {
            ActiveLearningKind::AgentSurprise {
                prediction_id,
                severity_score,
            } => {
                assert_eq!(prediction_id, &pred(0x11));
                assert!((severity_score - 0.8).abs() < f32::EPSILON);
            }
            other => panic!("expected AgentSurprise variant, got {other:?}"),
        }
        assert!((entry.score - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn agent_surprise_survives_eviction_when_uncertainty_rows_would_lose_first() {
        let mut queue = ActiveLearningQueueState::new(1).expect("queue");
        queue
            .enqueue_agent_surprise(
                TaskId("agent-surprise".to_string()),
                pred(0x22),
                SurpriseSeverity::Low,
            )
            .expect("enqueue surprise");
        // Force-evict by inserting an Uncertainty entry with a higher score:
        // the queue is capacity=1, but AgentSurprise must out-rank by kind
        // regardless of score.
        let task_id = TaskId("uncertain".to_string());
        queue.entries.insert(
            task_id.clone(),
            ActiveLearningQueueEntry {
                task_id: task_id.clone(),
                score: 0.99,
                outcome_set_len: 4,
                ood_score: 0.1,
                curiosity_score: 0.075,
                reason: "wide_conformal_set".to_string(),
                kind: ActiveLearningKind::Uncertainty,
            },
        );
        queue.evict_to_capacity().expect("evict");
        assert!(queue
            .entries
            .contains_key(&TaskId("agent-surprise".to_string())));
        assert!(!queue.entries.contains_key(&task_id));
    }

    #[test]
    fn empty_task_id_rejected() {
        let mut queue = ActiveLearningQueueState::new(8).expect("queue");
        let err = queue
            .enqueue_agent_surprise(TaskId(String::new()), pred(0x33), SurpriseSeverity::Medium)
            .expect_err("empty task_id must reject");
        assert!(err.to_string().contains("task_id"));
    }

    #[test]
    fn fast_track_rank_orders_agent_surprise_first() {
        assert!(
            ActiveLearningKind::AgentSurprise {
                prediction_id: pred(0),
                severity_score: 0.5,
            }
            .fast_track_rank()
                < ActiveLearningKind::NovelCluster {
                    candidate: Box::new(novel_candidate(0, vec![0.0, 1.0])),
                }
                .fast_track_rank()
        );
        assert!(
            ActiveLearningKind::NovelCluster {
                candidate: Box::new(novel_candidate(0, vec![0.0, 1.0])),
            }
            .fast_track_rank()
                < ActiveLearningKind::UnknownFingerprint {
                    candidate: Box::new(unknown_candidate(0, vec![1.0, 0.0])),
                }
                .fast_track_rank()
        );
        assert!(
            ActiveLearningKind::UnknownFingerprint {
                candidate: Box::new(unknown_candidate(0, vec![1.0, 0.0])),
            }
            .fast_track_rank()
                < ActiveLearningKind::ColdCellTargetedCorpus {
                    cell_id: "cell-a".to_string(),
                    abstain_count: 101,
                }
                .fast_track_rank()
        );
        assert!(
            ActiveLearningKind::ColdCellTargetedCorpus {
                cell_id: "cell-a".to_string(),
                abstain_count: 101,
            }
            .fast_track_rank()
                < ActiveLearningKind::OutOfDistribution.fast_track_rank()
        );
        assert!(
            ActiveLearningKind::OutOfDistribution.fast_track_rank()
                < ActiveLearningKind::Uncertainty.fast_track_rank()
        );
    }

    #[test]
    fn enqueue_cold_cell_targeted_corpus_uses_deterministic_task_id() {
        let mut queue = ActiveLearningQueueState::new(8).expect("queue");
        queue
            .enqueue_cold_cell_targeted_corpus("cell-a".to_string(), 101)
            .expect("enqueue cold cell");
        let entry = queue
            .entries
            .values()
            .find(|entry| {
                matches!(
                    entry.kind,
                    ActiveLearningKind::ColdCellTargetedCorpus { .. }
                )
            })
            .expect("cold cell entry present");
        assert!(entry.task_id.0.starts_with("cold-cell-targeted-corpus-"));
        assert_eq!(entry.score, 1.0);
        match &entry.kind {
            ActiveLearningKind::ColdCellTargetedCorpus {
                cell_id,
                abstain_count,
            } => {
                assert_eq!(cell_id, "cell-a");
                assert_eq!(*abstain_count, 101);
            }
            other => panic!("expected ColdCellTargetedCorpus variant, got {other:?}"),
        }
    }

    #[test]
    fn enqueue_unknown_fingerprint_candidate_persists_clusterable_entry() {
        let mut queue = ActiveLearningQueueState::new(8).expect("queue");
        let prediction = prediction("unknown-task-1", 0x44, 0.91);
        let enqueued = queue
            .enqueue_unknown_fingerprint_candidate(
                &prediction,
                &ood_classification(),
                vectors([("e1", vec![1.0, 0.0]), ("e2", vec![0.0, 1.0])]),
                1,
            )
            .expect("enqueue unknown");
        assert!(enqueued);
        let entry = queue.entries.get(&prediction.task_id).expect("entry");
        assert_eq!(entry.reason, "fingerprint_unknown_ood");
        assert_eq!(entry.score, 0.75);
        match &entry.kind {
            ActiveLearningKind::UnknownFingerprint { candidate } => {
                assert_eq!(candidate.task_id, prediction.task_id);
                assert_eq!(
                    candidate.prediction_id,
                    PredictionId(prediction.prediction_id)
                );
                assert_eq!(candidate.active_learning_priority, 2);
                assert_eq!(candidate.observation_by_embedder.len(), 2);
                candidate.validate().expect("candidate validates");
            }
            other => panic!("expected UnknownFingerprint variant, got {other:?}"),
        }
    }

    #[test]
    fn unknown_fingerprint_cluster_suggestion_groups_similar_candidates() {
        let mut queue = ActiveLearningQueueState::new(8).expect("queue");
        for (idx, vector) in [vec![1.0, 0.0], vec![0.999, 0.010], vec![0.998, 0.020]]
            .into_iter()
            .enumerate()
        {
            let prediction = prediction(
                &format!("unknown-cluster-task-{idx}"),
                0x50 + idx as u8,
                0.91,
            );
            queue
                .enqueue_unknown_fingerprint_candidate(
                    &prediction,
                    &ood_classification(),
                    vectors([("e1", vector), ("e2", vec![0.0, 1.0])]),
                    10 + idx as i64,
                )
                .expect("enqueue unknown cluster candidate");
        }
        let suggestions = queue
            .suggest_unknown_fingerprint_clusters(3, 0.98)
            .expect("cluster suggestions");
        assert_eq!(suggestions.len(), 1);
        let suggestion = &suggestions[0];
        suggestion.validate().expect("suggestion validates");
        assert_eq!(suggestion.member_candidate_ids.len(), 3);
        assert_eq!(suggestion.member_task_ids.len(), 3);
        assert_eq!(suggestion.active_learning_priority, 2);
        assert!(suggestion
            .centroid_by_embedder
            .contains_key(&EmbedderId("e1".to_string())));
    }

    #[test]
    fn novel_pattern_candidate_enqueues_and_admits_cluster() {
        let mut queue = ActiveLearningQueueState::new(8).expect("queue");
        let config = NovelPatternDetectorConfig::default();
        let existing_cells = vec![ConstellationCentroid {
            cell_id: "known-cell-a".to_string(),
            centroid_by_embedder: vectors([("e1", vec![1.0, 0.0]), ("e2", vec![1.0, 0.0])]),
            variance_by_embedder: BTreeMap::from([
                (EmbedderId("e1".to_string()), 0.01),
                (EmbedderId("e2".to_string()), 0.01),
            ]),
        }];
        for idx in 0..3 {
            let prediction = prediction(&format!("novel-task-{idx}"), 0x90 + idx as u8, 0.88);
            let enqueued = queue
                .enqueue_novel_pattern_candidate(
                    &prediction,
                    vectors([("e1", vec![0.0, 1.0]), ("e2", vec![0.0, 1.0])]),
                    &existing_cells,
                    100 + idx,
                    config,
                )
                .expect("enqueue novel pattern");
            assert!(enqueued);
        }
        let admissions = queue
            .admit_novel_pattern_clusters(&existing_cells, config)
            .expect("admit novel cluster");
        assert_eq!(admissions.len(), 1);
        assert_eq!(admissions[0].member_candidate_ids.len(), 3);
        assert!(admissions[0].min_distance_to_existing_centroid >= config.tau_far);
        assert!(admissions[0].intra_cluster_min_cosine >= config.tau_intra);
    }

    #[test]
    fn non_ood_fingerprint_classification_is_not_enqueued() {
        let mut queue = ActiveLearningQueueState::new(8).expect("queue");
        let prediction = prediction("known-good-task", 0x60, 0.10);
        let enqueued = queue
            .enqueue_unknown_fingerprint_candidate(
                &prediction,
                &known_good_classification(),
                vectors([("e1", vec![1.0, 0.0])]),
                1,
            )
            .expect("non-ood classification");
        assert!(!enqueued);
        assert!(queue.entries.is_empty());
    }

    #[test]
    fn curiosity_proxy_matches_expected_delta_and_zero_width() {
        let expected = 0.75 * 0.80 * 0.50;
        let observed = curiosity_score_from_proxy(0.75, 0.80, 0.50).expect("curiosity score");
        assert!((observed - expected).abs() <= 0.000001);
        assert_eq!(
            curiosity_score_from_proxy(0.0, 1.0, 1.0).expect("zero width"),
            0.0
        );
    }

    #[test]
    fn curiosity_ranking_uses_curiosity_before_scheduler_score() {
        let mut queue = ActiveLearningQueueState::new(8).expect("queue");
        queue.entries.insert(
            TaskId("scheduler-high".to_string()),
            synthetic_entry("scheduler-high", 0.99, 0.10),
        );
        queue.entries.insert(
            TaskId("curiosity-high".to_string()),
            synthetic_entry("curiosity-high", 0.20, 0.90),
        );
        let rows = queue.ranked_entries(ActiveLearningRankBy::Curiosity);
        assert_eq!(rows[0].task_id.0, "curiosity-high");
        assert_eq!(rows[1].task_id.0, "scheduler-high");
    }

    #[test]
    fn curiosity_telemetry_window_captures_distribution_and_calibration() {
        let mut queue = ActiveLearningQueueState::new(8).expect("queue");
        queue.entries.insert(
            TaskId("low".to_string()),
            synthetic_entry("low", 0.90, 0.10),
        );
        queue.entries.insert(
            TaskId("high".to_string()),
            synthetic_entry("high", 0.20, 0.80),
        );
        let evidence =
            CuriosityCalibrationEvidence::new(TaskId("high".to_string()), 0.80, 0.78, 0.05)
                .expect("calibration evidence");
        let window = CuriosityTelemetryWindow::from_queue("window-1", 1, &queue, 10)
            .expect("telemetry")
            .with_calibration_evidence(vec![evidence])
            .expect("calibrated telemetry");
        assert_eq!(window.entry_count, 2);
        assert_eq!(window.curiosity_distribution.count, 2);
        assert_eq!(window.top_entries[0].task_id.0, "high");
        assert!(window.calibration_evidence[0].within_tolerance);
        assert!(curiosity_telemetry_window_key(&window)
            .expect("key")
            .starts_with(b"00000000000000000001::window-1"));
    }

    fn synthetic_entry(
        task_id: &str,
        scheduler_score: f32,
        curiosity_score: f32,
    ) -> ActiveLearningQueueEntry {
        ActiveLearningQueueEntry {
            task_id: TaskId(task_id.to_string()),
            score: scheduler_score,
            outcome_set_len: 4,
            ood_score: 0.80,
            curiosity_score,
            reason: "synthetic_curiosity".to_string(),
            kind: ActiveLearningKind::Uncertainty,
        }
    }

    fn prediction(task_id: &str, prediction_id: u8, ood_score: f32) -> RealityPrediction {
        use crate::types::{ConformalSet, Language, RealityPredictionBuilder};

        let outcome_set = ConformalSet::try_new(vec![OracleOutcome::OutOfDistribution], 0.1, 0.5)
            .expect("outcome set");
        RealityPredictionBuilder::from_parts(
            TaskId(task_id.to_string()),
            [0x7a; 16],
            Language::Python,
            outcome_set,
        )
        .prediction_id([prediction_id; 16])
        .verdict(Verdict::OutOfDistribution)
        .predicted_oracle_pass(0.5)
        .predicted_test_pass(vec![0.5])
        .ood_score(ood_score)
        .calibrated_confidence(0.5)
        .calibration_version("queue-test")
        .build()
        .expect("prediction")
    }

    fn ood_classification() -> FingerprintClassification {
        let classification = FingerprintClassification {
            verdict: Verdict::OutOfDistribution,
            reason: FingerprintDecisionReason::NoKnownMatch,
            primary_match: None,
            ranked_matches: vec![candidate_score(false, 0x77, -0.40)],
            matched_known_good_count: 0,
            matched_known_bad_count: 0,
            matched_unknown_count: 0,
            scored_fingerprint_count: 1,
        };
        classification.validate().expect("ood classification");
        classification
    }

    fn known_good_classification() -> FingerprintClassification {
        let primary = candidate_score(true, 0x78, 0.10);
        let classification = FingerprintClassification {
            verdict: Verdict::Pass,
            reason: FingerprintDecisionReason::KnownGoodOnly,
            primary_match: Some(primary.clone()),
            ranked_matches: vec![primary],
            matched_known_good_count: 1,
            matched_known_bad_count: 0,
            matched_unknown_count: 0,
            scored_fingerprint_count: 1,
        };
        classification
            .validate()
            .expect("known-good classification");
        classification
    }

    fn candidate_score(matched: bool, id: u8, min_margin: f32) -> FingerprintCandidateScore {
        use crate::failure_fingerprint::{
            FingerprintCalibrationState, FingerprintEmbedderScore, FingerprintId, FingerprintKind,
        };
        let kind = if matched {
            FingerprintKind::KnownGood {
                repo: Some("django__django".to_string()),
                gold_patch_count: 2,
            }
        } else {
            FingerprintKind::KnownBad {
                repo: "django__django".to_string(),
                mutation_category: crate::MutationCategory::OffByOne,
                failure_mode: crate::FailureModeClass::OffByOne,
                exception_class: None,
            }
        };
        let score = FingerprintCandidateScore {
            fingerprint_id: FingerprintId([id; 32]),
            name: format!("candidate-{id}"),
            kind,
            oracle_outcome: if matched {
                Some(OracleOutcome::Pass)
            } else {
                Some(OracleOutcome::Fail)
            },
            matched,
            mean_cosine: if matched { 0.95 } else { 0.50 },
            min_margin,
            embedder_scores: vec![
                FingerprintEmbedderScore {
                    embedder: EmbedderId("e1".to_string()),
                    cosine: if matched { 0.95 } else { 0.20 },
                    tau: 0.90,
                    margin: if matched { 0.05 } else { -0.70 },
                    matched,
                },
                FingerprintEmbedderScore {
                    embedder: EmbedderId("e2".to_string()),
                    cosine: if matched { 0.95 } else { 0.80 },
                    tau: 0.90,
                    margin: if matched { 0.05 } else { -0.10 },
                    matched,
                },
            ],
            n_references: 1,
            calibration_state: FingerprintCalibrationState::Calibrated,
        };
        score.validate().expect("candidate score");
        score
    }

    fn unknown_candidate(idx: u8, vector: Vec<f32>) -> UnknownFingerprintCandidate {
        let prediction = prediction(&format!("unknown-candidate-{idx}"), idx.max(1), 0.91);
        let observation_by_embedder = vectors([("e1", vector)]);
        UnknownFingerprintCandidate {
            candidate_id: unknown_candidate_id(
                &prediction.task_id,
                PredictionId(prediction.prediction_id),
                i64::from(idx) + 1,
                &observation_by_embedder,
            ),
            prediction_id: PredictionId(prediction.prediction_id),
            task_id: prediction.task_id,
            session_id: prediction.session_id,
            observed_at_unix_ms: i64::from(idx) + 1,
            ood_score: prediction.ood_score,
            embedder_disagreement_score: 0.1,
            active_learning_priority: 2,
            observation_by_embedder,
            nearest_fingerprints: vec![candidate_score(false, 0x79, -0.40)],
        }
    }

    fn novel_candidate(idx: u8, vector: Vec<f32>) -> NovelPatternCandidate {
        let prediction = prediction(&format!("novel-candidate-{idx}"), idx.max(1), 0.91);
        let observation_by_embedder = vectors([("e1", vector)]);
        NovelPatternCandidate {
            candidate_id: [idx.max(1); 16],
            prediction_id: PredictionId(prediction.prediction_id),
            task_id: prediction.task_id,
            session_id: prediction.session_id,
            observed_at_unix_ms: i64::from(idx) + 1,
            novelty_score: 3.0,
            nearest_existing_cell_id: "known-cell".to_string(),
            nearest_existing_distance: 0.9,
            active_learning_priority: 2,
            observation_by_embedder,
        }
    }

    fn vectors<const N: usize>(items: [(&str, Vec<f32>); N]) -> BTreeMap<EmbedderId, Vec<f32>> {
        items
            .into_iter()
            .map(|(embedder, vector)| (EmbedderId(embedder.to_string()), vector))
            .collect()
    }
}
