use super::error::{EvalError, EvalErrorCode};
use super::types::EvalObservation;
use crate::types::{OracleOutcome, TaskId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveLearningQueueEntry {
    pub task_id: TaskId,
    pub score: f32,
    pub outcome_set_len: usize,
    pub ood_score: f32,
    pub reason: String,
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
        let entry = ActiveLearningQueueEntry {
            task_id: observation.task_id.clone(),
            score: ((outcome_set_len as f32 / max_len) * (1.0 - ood_score)).clamp(0.0, 1.0),
            outcome_set_len,
            ood_score,
            reason: if ood_score >= ood_escalation_threshold {
                "ood_escalation".to_string()
            } else {
                "wide_conformal_set".to_string()
            },
        };
        if ood_score >= ood_escalation_threshold {
            self.ood_escalations.push(entry);
            return Ok(());
        }
        if outcome_set_len < uncertainty_outcome_len {
            return Ok(());
        }
        self.entries.insert(observation.task_id.clone(), entry);
        while self.entries.len() > self.capacity {
            let lowest = self
                .entries
                .iter()
                .min_by(|a, b| a.1.score.total_cmp(&b.1.score).then_with(|| a.0.cmp(b.0)))
                .map(|(task_id, _)| task_id.clone())
                .ok_or_else(|| EvalError::new(EvalErrorCode::Store, "queue unexpectedly empty"))?;
            if let Some(evicted) = self.entries.remove(&lowest) {
                self.evicted.push(evicted);
            }
        }
        Ok(())
    }
}
