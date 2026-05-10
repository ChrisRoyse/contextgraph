pub mod cross_task;

use crate::config::TrainingConfig;
use crate::error::{TrainerError, TrainerErrorCode};
use crate::learning_signal::{sampling_weight, should_drop, should_force_include};
use context_graph_mejepa_corpus::prng::SplitMix64;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchSimilarityGraph {
    pub neighbors: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchPlan {
    pub indices: Vec<usize>,
    pub force_count: usize,
    pub regular_count: usize,
    pub cross_task_indices: Vec<usize>,
    pub cross_task_fallback_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SamplerWarning {
    AllWeightsBelowDropThreshold,
    ForceCountExceedsHalfBatch {
        force_count: usize,
        batch_size: usize,
    },
}

#[derive(Debug, Clone)]
pub struct BatchSampler {
    pub config: TrainingConfig,
    pub weights: Vec<f32>,
    pub overrides: Vec<bool>,
    pub ages_days: Vec<f32>,
    pub l_steps: Vec<f32>,
    pub task_ids: Vec<String>,
    pub patch_similarity_graph: PatchSimilarityGraph,
    pub rng: SplitMix64,
    pub force_indices: Vec<usize>,
    pub drop_indices: Vec<usize>,
    pub regular_indices: Vec<usize>,
    pub force_cursor: usize,
}

impl BatchSampler {
    pub fn new(
        config: TrainingConfig,
        l_steps: Vec<f32>,
        overrides: Vec<bool>,
        ages_days: Vec<f32>,
        task_ids: Vec<String>,
        patch_similarity_graph: PatchSimilarityGraph,
    ) -> Result<Self, TrainerError> {
        config.validate()?;
        let n = l_steps.len();
        if n == 0 {
            return Err(Self::config_error("l_steps", "sampler cannot be empty"));
        }
        for (name, len) in [
            ("overrides", overrides.len()),
            ("ages_days", ages_days.len()),
            ("task_ids", task_ids.len()),
            ("patch_similarity_graph.neighbors", patch_similarity_graph.neighbors.len()),
        ] {
            if len != n {
                return Err(Self::config_error(
                    name,
                    format!("length {len} does not match l_steps length {n}"),
                ));
            }
        }
        let mut sampler = Self {
            rng: SplitMix64::new(config.random_seed),
            config,
            weights: vec![0.0; n],
            overrides,
            ages_days,
            l_steps,
            task_ids,
            patch_similarity_graph,
            force_indices: Vec::new(),
            drop_indices: Vec::new(),
            regular_indices: Vec::new(),
            force_cursor: 0,
        };
        sampler.recompute_weights()?;
        Ok(sampler)
    }

    pub fn recompute_weights(&mut self) -> Result<Vec<SamplerWarning>, TrainerError> {
        self.weights.clear();
        self.force_indices.clear();
        self.drop_indices.clear();
        self.regular_indices.clear();
        for i in 0..self.l_steps.len() {
            let weight = sampling_weight(
                self.l_steps[i],
                self.overrides[i],
                self.ages_days[i],
                self.config.sampling_age_decay,
            );
            if !weight.is_finite() {
                return Err(Self::config_error(
                    "weights",
                    format!("non-finite sampling weight at index {i}"),
                ));
            }
            self.weights.push(weight);
            if should_drop(weight, self.config.sampling_drop_threshold) {
                self.drop_indices.push(i);
            } else if should_force_include(weight, self.config.sampling_force_threshold) {
                self.force_indices.push(i);
            } else {
                self.regular_indices.push(i);
            }
        }
        let mut warnings = Vec::new();
        if self.regular_indices.is_empty() && self.force_indices.is_empty() {
            warnings.push(SamplerWarning::AllWeightsBelowDropThreshold);
        }
        if self.force_indices.len() > self.config.batch_size / 2 {
            warnings.push(SamplerWarning::ForceCountExceedsHalfBatch {
                force_count: self.force_indices.len(),
                batch_size: self.config.batch_size,
            });
        }
        Ok(warnings)
    }

    pub fn next_batch(
        &mut self,
        _current_task_idx: Option<usize>,
        batch_size: usize,
    ) -> Result<BatchPlan, TrainerError> {
        if batch_size == 0 {
            return Err(Self::config_error("batch_size", "must be positive"));
        }
        let mut plan = self.force_prefix(batch_size);
        let need = batch_size.saturating_sub(plan.force_count);
        let mut pool = self.regular_indices.clone();
        for _ in 0..need.min(pool.len()) {
            let chosen = self.weighted_pick_from_pool(&pool)?;
            let pos = pool
                .iter()
                .position(|idx| *idx == chosen)
                .ok_or_else(|| Self::config_error("sampler", "chosen index absent from pool"))?;
            pool.swap_remove(pos);
            plan.indices.push(chosen);
            plan.regular_count += 1;
        }
        Ok(plan)
    }

    pub(crate) fn force_prefix(&mut self, batch_size: usize) -> BatchPlan {
        let mut plan = BatchPlan {
            indices: Vec::with_capacity(batch_size),
            force_count: 0,
            regular_count: 0,
            cross_task_indices: Vec::new(),
            cross_task_fallback_count: 0,
        };
        if self.force_indices.is_empty() {
            return plan;
        }
        for _ in 0..batch_size.min(self.force_indices.len()) {
            let idx = self.force_indices[self.force_cursor % self.force_indices.len()];
            self.force_cursor = (self.force_cursor + 1) % self.force_indices.len();
            plan.indices.push(idx);
            plan.force_count += 1;
        }
        plan
    }

    pub(crate) fn weighted_pick_from_regular(&mut self) -> Result<Option<usize>, TrainerError> {
        if self.regular_indices.is_empty() {
            return Ok(None);
        }
        let pool = self.regular_indices.clone();
        self.weighted_pick_from_pool(&pool).map(Some)
    }

    pub(crate) fn weighted_pick_from_pool(&mut self, pool: &[usize]) -> Result<usize, TrainerError> {
        if pool.is_empty() {
            return Err(Self::config_error("pool", "weighted pool is empty"));
        }
        let total = pool
            .iter()
            .map(|idx| self.weights[*idx].max(0.0))
            .sum::<f32>();
        if total <= 0.0 || !total.is_finite() {
            return Err(Self::config_error(
                "weights",
                "weighted pool has no positive finite mass",
            ));
        }
        let target = self.rng.next_unit_f32() * total;
        let mut acc = 0.0;
        for idx in pool {
            acc += self.weights[*idx].max(0.0);
            if target <= acc {
                return Ok(*idx);
            }
        }
        Ok(*pool.last().expect("pool non-empty checked"))
    }

    pub fn save_rng_state(&self) -> u64 {
        self.rng.state()
    }

    pub fn restore_rng_state(&mut self, state: u64) {
        self.rng = SplitMix64::from_state(state);
    }

    pub fn n_examples(&self) -> usize {
        self.l_steps.len()
    }

    fn config_error(field: &'static str, message: impl Into<String>) -> TrainerError {
        TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message).with_context(json!({
            "field": field,
            "file": "file:crates/context-graph-mejepa-train/src/sampler/mod.rs",
            "remediation": "fix sampler inputs; all vectors must be same length and all weights finite"
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sampler() -> BatchSampler {
        let cfg = TrainingConfig {
            random_seed: 7,
            batch_size: 16,
            ..TrainingConfig::default()
        };
        let mut l_steps = vec![0.01; 10];
        l_steps.extend(vec![0.6; 30]);
        l_steps.extend(vec![0.3; 50]);
        l_steps.extend(vec![0.2; 10]);
        let mut overrides = vec![false; 90];
        overrides.extend(vec![true; 10]);
        let ages = vec![0.0; 100];
        let ids = (0..100).map(|i| format!("t{i}")).collect();
        let graph = PatchSimilarityGraph {
            neighbors: vec![Vec::new(); 100],
        };
        BatchSampler::new(cfg, l_steps, overrides, ages, ids, graph).unwrap()
    }

    #[test]
    fn partitions_match_weights() {
        let s = sampler();
        assert_eq!(s.drop_indices.len(), 10);
        assert_eq!(s.force_indices.len(), 10);
        assert_eq!(s.regular_indices.len(), 80);
    }

    #[test]
    fn deterministic_sequence() {
        let mut a = sampler();
        let mut b = sampler();
        for _ in 0..10 {
            assert_eq!(a.next_batch(None, 16).unwrap(), b.next_batch(None, 16).unwrap());
        }
    }

    #[test]
    fn rng_state_restores() {
        let mut a = sampler();
        let _ = a.next_batch(None, 16).unwrap();
        let state = a.save_rng_state();
        let expected = a.next_batch(None, 16).unwrap();
        a.restore_rng_state(state);
        assert_eq!(expected, a.next_batch(None, 16).unwrap());
    }
}

