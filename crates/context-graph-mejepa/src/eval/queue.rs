use super::error::{EvalError, EvalErrorCode};
use super::types::EvalObservation;
use crate::pause_state::prediction_is_operator_paused;
use crate::types::{OracleOutcome, PredictionId, RealityPrediction, SurpriseSeverity, TaskId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// REQ-FLYWHEEL-11 — discriminates how an entry landed on the active-learning
/// queue. The Uncertainty / OutOfDistribution variants are the existing
/// eval-runner-driven paths; AgentSurprise is the agent-feedback path
/// (TASK-PREDICT-002) that fast-tracks observe() for predictions the agent
/// flagged as wrong. The `severity_score` on AgentSurprise feeds the
/// REQ-FLYWHEEL-12 sampling-weight bump (`1 + 2 * severity_score`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveLearningKind {
    Uncertainty,
    OutOfDistribution,
    AgentSurprise {
        prediction_id: PredictionId,
        severity_score: f32,
    },
    ColdCellTargetedCorpus {
        cell_id: String,
        abstain_count: u32,
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
            Self::ColdCellTargetedCorpus { .. } => 1,
            Self::OutOfDistribution => 2,
            Self::Uncertainty => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveLearningQueueEntry {
    pub task_id: TaskId,
    pub score: f32,
    pub outcome_set_len: usize,
    pub ood_score: f32,
    pub reason: String,
    pub kind: ActiveLearningKind,
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
}
