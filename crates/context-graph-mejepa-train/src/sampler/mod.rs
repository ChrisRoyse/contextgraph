pub mod cross_task;

use crate::config::TrainingConfig;
use crate::error::{TrainerError, TrainerErrorCode};
use crate::learning_signal::{
    sampling_weight_checked_with_operator_override_multiplier, should_drop, should_force_include,
    SamplingWeightComponents,
};
use crate::replay_buffer::{
    sample_replay_batch, ReplayBatchReport, ReplayBatchRequest, ReplayPriorityConfig,
};
use context_graph_mejepa::system_cost::SystemCostCounters;
use context_graph_mejepa::{
    operator_contribution_report_from_db, read_all_sampler_reward_signals,
    ActiveLearningQueueEntry, ChunkId, OperatorContributionError, OperatorOverride, PredictionId,
    RealityPrediction, SamplerRewardSignal, SamplerRewardStatus, TaskId,
    OPERATOR_OVERRIDE_SAMPLING_WEIGHT,
};
use context_graph_mejepa_corpus::prng::SplitMix64;
use rocksdb::{IteratorMode, DB};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchSimilarityGraph {
    pub neighbors: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchPlan {
    pub indices: Vec<usize>,
    pub force_count: usize,
    pub regular_count: usize,
    pub adversarial_count: usize,
    pub adversarial_example_indices: Vec<usize>,
    pub adversarial_fallback_count: usize,
    pub cross_task_indices: Vec<usize>,
    pub cross_task_fallback_count: usize,
    pub operator_override_sampler_applied_count: usize,
    pub operator_override_boost_audit: Vec<OperatorOverrideBoostAuditRow>,
    pub online_reward_signals_applied_count: usize,
    pub online_reward_boost_audit: Vec<SamplerRewardBoostAuditRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SamplerExampleSource {
    pub prediction_id: PredictionId,
    pub chunk_id: ChunkId,
}

#[derive(Debug, Clone)]
pub struct OperatorOverrideSamplerInput {
    pub config: TrainingConfig,
    pub l_steps: Vec<f32>,
    pub overrides: Vec<bool>,
    pub ages_days: Vec<f32>,
    pub task_ids: Vec<String>,
    pub example_sources: Vec<SamplerExampleSource>,
    pub patch_similarity_graph: PatchSimilarityGraph,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperatorOverrideSamplerReport {
    pub override_rows_scanned: usize,
    pub live_prediction_rows_scanned: usize,
    pub operator_quality_rows_scanned: usize,
    pub dismissed_override_count: usize,
    pub applied_count: usize,
    pub boosted_indices: Vec<usize>,
    pub index_weight_multiplier_micros: BTreeMap<usize, u32>,
    pub boost_audit: Vec<OperatorOverrideBoostAuditRow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperatorOverrideBoostAuditRow {
    pub example_index: usize,
    pub prediction_id_hex: String,
    pub chunk_id: String,
    pub operator_id: String,
    pub operator_quality_score_micros: u32,
    pub sampling_weight_multiplier_micros: u32,
    pub contribution_count: usize,
    pub downstream_linked_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SamplerRewardSamplerReport {
    pub reward_rows_scanned: usize,
    pub awaiting_oracle_count: usize,
    pub quarantined_non_finite_count: usize,
    pub stale_count: usize,
    pub applied_count: usize,
    pub boosted_indices: Vec<usize>,
    pub index_weight_multiplier_micros: BTreeMap<usize, u32>,
    pub boost_audit: Vec<SamplerRewardBoostAuditRow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SamplerRewardBoostAuditRow {
    pub example_index: usize,
    pub prediction_id_hex: String,
    pub chunk_id: String,
    pub cell_id: String,
    pub surprise_z_micros: u32,
    pub sampling_weight_multiplier_micros: u32,
    pub oracle_observed_at_unix_ms: i64,
    pub status_code: String,
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
    pub operator_override_weight_multipliers: Vec<f32>,
    pub online_reward_weight_multipliers: Vec<f32>,
    pub ages_days: Vec<f32>,
    pub l_steps: Vec<f32>,
    pub foundationality_scores: Vec<f32>,
    pub task_ids: Vec<String>,
    pub patch_similarity_graph: PatchSimilarityGraph,
    pub rng: SplitMix64,
    pub adversarial_indices: Vec<usize>,
    pub force_indices: Vec<usize>,
    pub drop_indices: Vec<usize>,
    pub regular_indices: Vec<usize>,
    pub force_cursor: usize,
    pub adversarial_cursor: usize,
    pub operator_override_sampler_applied_count: usize,
    pub online_reward_signals_applied_count: usize,
    adversarial_index_set: BTreeSet<usize>,
    operator_override_boosted_indices: BTreeSet<usize>,
    operator_override_boost_audit_by_index: BTreeMap<usize, Vec<OperatorOverrideBoostAuditRow>>,
    online_reward_boosted_indices: BTreeSet<usize>,
    online_reward_boost_audit_by_index: BTreeMap<usize, Vec<SamplerRewardBoostAuditRow>>,
    system_cost_counters: Option<Arc<SystemCostCounters>>,
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
        Self::new_with_adversarial_indices(
            config,
            l_steps,
            overrides,
            ages_days,
            task_ids,
            Vec::new(),
            patch_similarity_graph,
        )
    }

    pub fn new_with_adversarial_indices(
        config: TrainingConfig,
        l_steps: Vec<f32>,
        overrides: Vec<bool>,
        ages_days: Vec<f32>,
        task_ids: Vec<String>,
        adversarial_indices: Vec<usize>,
        patch_similarity_graph: PatchSimilarityGraph,
    ) -> Result<Self, TrainerError> {
        let foundationality_scores = vec![0.0; l_steps.len()];
        Self::new_with_foundationality_scores(
            config,
            l_steps,
            overrides,
            ages_days,
            task_ids,
            foundationality_scores,
            adversarial_indices,
            patch_similarity_graph,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_foundationality_scores(
        config: TrainingConfig,
        l_steps: Vec<f32>,
        overrides: Vec<bool>,
        ages_days: Vec<f32>,
        task_ids: Vec<String>,
        foundationality_scores: Vec<f32>,
        adversarial_indices: Vec<usize>,
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
            ("foundationality_scores", foundationality_scores.len()),
            (
                "patch_similarity_graph.neighbors",
                patch_similarity_graph.neighbors.len(),
            ),
        ] {
            if len != n {
                return Err(Self::config_error(
                    name,
                    format!("length {len} does not match l_steps length {n}"),
                ));
            }
        }
        let operator_override_weight_multipliers =
            default_operator_override_weight_multipliers(&overrides);
        let (adversarial_indices, adversarial_index_set) =
            Self::validate_adversarial_indices(n, adversarial_indices)?;
        let mut sampler = Self {
            rng: SplitMix64::new(config.random_seed),
            config,
            weights: vec![0.0; n],
            overrides,
            operator_override_weight_multipliers,
            online_reward_weight_multipliers: vec![1.0; n],
            ages_days,
            l_steps,
            foundationality_scores,
            task_ids,
            patch_similarity_graph,
            adversarial_indices,
            force_indices: Vec::new(),
            drop_indices: Vec::new(),
            regular_indices: Vec::new(),
            force_cursor: 0,
            adversarial_cursor: 0,
            operator_override_sampler_applied_count: 0,
            online_reward_signals_applied_count: 0,
            adversarial_index_set,
            operator_override_boosted_indices: BTreeSet::new(),
            operator_override_boost_audit_by_index: BTreeMap::new(),
            online_reward_boosted_indices: BTreeSet::new(),
            online_reward_boost_audit_by_index: BTreeMap::new(),
            system_cost_counters: None,
        };
        sampler.recompute_weights()?;
        Ok(sampler)
    }

    pub fn new_with_operator_override_db(
        mut input: OperatorOverrideSamplerInput,
        db: &DB,
        system_cost_counters: Option<Arc<SystemCostCounters>>,
    ) -> Result<(Self, OperatorOverrideSamplerReport), TrainerError> {
        validate_operator_override_input(&input)?;
        let mut report = apply_operator_override_rows(&mut input, db)?;
        let boosted_index_set = report
            .boosted_indices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let index_weight_multiplier_micros = report.index_weight_multiplier_micros.clone();
        let boost_audit_by_index = report.boost_audit.iter().cloned().fold(
            BTreeMap::<usize, Vec<OperatorOverrideBoostAuditRow>>::new(),
            |mut acc, row| {
                acc.entry(row.example_index).or_default().push(row);
                acc
            },
        );
        let mut sampler = Self::new(
            input.config,
            input.l_steps,
            input.overrides,
            input.ages_days,
            input.task_ids,
            input.patch_similarity_graph,
        )?;
        for (idx, micros) in index_weight_multiplier_micros {
            if idx < sampler.operator_override_weight_multipliers.len() {
                sampler.operator_override_weight_multipliers[idx] = micros_to_multiplier(micros);
            }
        }
        sampler.recompute_weights()?;
        report.boosted_indices.sort_unstable();
        sampler.operator_override_sampler_applied_count = report.applied_count;
        sampler.operator_override_boosted_indices = boosted_index_set;
        sampler.operator_override_boost_audit_by_index = boost_audit_by_index;
        sampler.system_cost_counters = system_cost_counters;
        Ok((sampler, report))
    }

    pub fn new_with_reward_signals(
        input: OperatorOverrideSamplerInput,
        db: &DB,
        last_consumed_oracle_ts: i64,
        system_cost_counters: Option<Arc<SystemCostCounters>>,
    ) -> Result<(Self, SamplerRewardSamplerReport), TrainerError> {
        validate_operator_override_input(&input)?;
        let mut report = apply_sampler_reward_rows(&input, db, last_consumed_oracle_ts)?;
        let boosted_index_set = report
            .boosted_indices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let index_weight_multiplier_micros = report.index_weight_multiplier_micros.clone();
        let boost_audit_by_index = report.boost_audit.iter().cloned().fold(
            BTreeMap::<usize, Vec<SamplerRewardBoostAuditRow>>::new(),
            |mut acc, row| {
                acc.entry(row.example_index).or_default().push(row);
                acc
            },
        );
        let mut sampler = Self::new(
            input.config,
            input.l_steps,
            input.overrides,
            input.ages_days,
            input.task_ids,
            input.patch_similarity_graph,
        )?;
        for (idx, micros) in index_weight_multiplier_micros {
            if idx < sampler.online_reward_weight_multipliers.len() {
                sampler.online_reward_weight_multipliers[idx] = micros_to_multiplier(micros);
            }
        }
        sampler.recompute_weights()?;
        report.boosted_indices.sort_unstable();
        sampler.online_reward_signals_applied_count = report.applied_count;
        sampler.online_reward_boosted_indices = boosted_index_set;
        sampler.online_reward_boost_audit_by_index = boost_audit_by_index;
        sampler.system_cost_counters = system_cost_counters;
        Ok((sampler, report))
    }

    pub fn recompute_weights(&mut self) -> Result<Vec<SamplerWarning>, TrainerError> {
        if self.operator_override_weight_multipliers.len() != self.l_steps.len() {
            return Err(Self::config_error(
                "operator_override_weight_multipliers",
                format!(
                    "length {} does not match l_steps length {}",
                    self.operator_override_weight_multipliers.len(),
                    self.l_steps.len()
                ),
            ));
        }
        if self.online_reward_weight_multipliers.len() != self.l_steps.len() {
            return Err(Self::config_error(
                "online_reward_weight_multipliers",
                format!(
                    "length {} does not match l_steps length {}",
                    self.online_reward_weight_multipliers.len(),
                    self.l_steps.len()
                ),
            ));
        }
        self.weights.clear();
        self.force_indices.clear();
        self.drop_indices.clear();
        self.regular_indices.clear();
        for i in 0..self.l_steps.len() {
            let online_reward_multiplier = self.online_reward_weight_multipliers[i];
            if !online_reward_multiplier.is_finite() || online_reward_multiplier < 1.0 {
                return Err(Self::config_error(
                    "online_reward_weight_multipliers",
                    format!(
                        "index {i}: multiplier must be finite and >= 1.0; got {online_reward_multiplier}"
                    ),
                ));
            }
            let base_weight = sampling_weight_checked_with_operator_override_multiplier(
                SamplingWeightComponents {
                    base_weight: 1.0,
                    l_step: self.l_steps[i],
                    operator_override: self.overrides[i],
                    age_days: self.ages_days[i],
                    age_decay: self.config.sampling_age_decay,
                    agent_surprise_severity_score: 0.0,
                    foundationality_score: self.foundationality_scores[i],
                    lambda_foundationality: self.config.sampling_foundationality_lambda,
                    curiosity_score: 0.0,
                    lambda_curiosity: 1.0,
                },
                self.operator_override_weight_multipliers[i],
            )
            .map_err(|err| Self::config_error("weights", format!("index {i}: {err}")))?;
            let weight = base_weight * online_reward_multiplier;
            if !weight.is_finite() || weight < 0.0 {
                return Err(Self::config_error(
                    "weights",
                    format!("index {i}: online reward multiplier produced invalid weight {weight}"),
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
        let mut used = plan.indices.iter().copied().collect::<BTreeSet<_>>();
        self.fill_adversarial_mix(&mut plan, batch_size, &mut used);
        let need = batch_size.saturating_sub(plan.indices.len());
        let mut pool = self.regular_pool_excluding(&used, true);
        for _ in 0..need.min(pool.len()) {
            let chosen = self.weighted_pick_from_pool(&pool)?;
            let pos = pool
                .iter()
                .position(|idx| *idx == chosen)
                .ok_or_else(|| Self::config_error("sampler", "chosen index absent from pool"))?;
            pool.swap_remove(pos);
            plan.indices.push(chosen);
            plan.regular_count += 1;
            used.insert(chosen);
        }
        self.finalize_batch_plan(&mut plan, batch_size);
        Ok(plan)
    }

    pub fn next_replay_batch(
        db: &DB,
        request: ReplayBatchRequest,
        priority_config: ReplayPriorityConfig,
    ) -> Result<ReplayBatchReport, TrainerError> {
        sample_replay_batch(db, request, priority_config)
    }

    pub(crate) fn force_prefix(&mut self, batch_size: usize) -> BatchPlan {
        let mut plan = BatchPlan {
            indices: Vec::with_capacity(batch_size),
            force_count: 0,
            regular_count: 0,
            adversarial_count: 0,
            adversarial_example_indices: Vec::new(),
            adversarial_fallback_count: 0,
            cross_task_indices: Vec::new(),
            cross_task_fallback_count: 0,
            operator_override_sampler_applied_count: 0,
            operator_override_boost_audit: Vec::new(),
            online_reward_signals_applied_count: 0,
            online_reward_boost_audit: Vec::new(),
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

    pub(crate) fn weighted_pick_from_regular_excluding(
        &mut self,
        used: &BTreeSet<usize>,
        exclude_adversarial: bool,
    ) -> Result<Option<usize>, TrainerError> {
        let pool = self.regular_pool_excluding(used, exclude_adversarial);
        if pool.is_empty() {
            return Ok(None);
        }
        self.weighted_pick_from_pool(&pool).map(Some)
    }

    pub(crate) fn weighted_pick_from_pool(
        &mut self,
        pool: &[usize],
    ) -> Result<usize, TrainerError> {
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

    pub(crate) fn record_operator_override_sampler_batch(&self, plan: &mut BatchPlan) {
        let count = plan
            .indices
            .iter()
            .filter(|idx| self.operator_override_boosted_indices.contains(idx))
            .count();
        plan.operator_override_sampler_applied_count = count;
        plan.operator_override_boost_audit = plan
            .indices
            .iter()
            .filter_map(|idx| self.operator_override_boost_audit_by_index.get(idx))
            .flat_map(|rows| rows.iter().cloned())
            .collect();
        if count > 0 {
            if let Some(counters) = &self.system_cost_counters {
                counters.record_operator_override_sampler_applied(count as u64);
            }
        }
    }

    pub(crate) fn record_online_reward_sampler_batch(&self, plan: &mut BatchPlan) {
        let count = plan
            .indices
            .iter()
            .filter(|idx| self.online_reward_boosted_indices.contains(idx))
            .count();
        plan.online_reward_signals_applied_count = count;
        plan.online_reward_boost_audit = plan
            .indices
            .iter()
            .filter_map(|idx| self.online_reward_boost_audit_by_index.get(idx))
            .flat_map(|rows| rows.iter().cloned())
            .collect();
        if count > 0 {
            if let Some(counters) = &self.system_cost_counters {
                counters.record_online_reward_signals_applied(count as u64);
            }
        }
    }

    pub(crate) fn finalize_batch_plan(&self, plan: &mut BatchPlan, requested_batch_size: usize) {
        self.record_adversarial_mix_batch(plan, requested_batch_size);
        self.record_operator_override_sampler_batch(plan);
        self.record_online_reward_sampler_batch(plan);
    }

    pub(crate) fn adversarial_quota(&self, batch_size: usize) -> usize {
        if batch_size == 0
            || self.adversarial_indices.is_empty()
            || self.config.adversarial_mix_ratio <= 0.0
        {
            return 0;
        }
        let raw = (batch_size as f32 * self.config.adversarial_mix_ratio).round() as usize;
        raw.max(1).min(batch_size)
    }

    pub(crate) fn fill_adversarial_mix(
        &mut self,
        plan: &mut BatchPlan,
        batch_size: usize,
        used: &mut BTreeSet<usize>,
    ) {
        let target = self.adversarial_quota(batch_size);
        if target == 0 {
            return;
        }
        let mut selected = plan
            .indices
            .iter()
            .filter(|idx| self.adversarial_index_set.contains(idx))
            .count();
        let mut scanned_without_pick = 0usize;
        while selected < target
            && plan.indices.len() < batch_size
            && scanned_without_pick < self.adversarial_indices.len()
        {
            let idx =
                self.adversarial_indices[self.adversarial_cursor % self.adversarial_indices.len()];
            self.adversarial_cursor =
                (self.adversarial_cursor + 1) % self.adversarial_indices.len();
            if used.insert(idx) {
                plan.indices.push(idx);
                selected += 1;
                scanned_without_pick = 0;
            } else {
                scanned_without_pick += 1;
            }
        }
    }

    pub(crate) fn regular_pool_excluding(
        &self,
        used: &BTreeSet<usize>,
        exclude_adversarial: bool,
    ) -> Vec<usize> {
        self.regular_indices
            .iter()
            .copied()
            .filter(|idx| !used.contains(idx))
            .filter(|idx| !exclude_adversarial || !self.adversarial_index_set.contains(idx))
            .collect()
    }

    fn record_adversarial_mix_batch(&self, plan: &mut BatchPlan, requested_batch_size: usize) {
        plan.adversarial_example_indices = plan
            .indices
            .iter()
            .copied()
            .filter(|idx| self.adversarial_index_set.contains(idx))
            .collect();
        plan.adversarial_count = plan.adversarial_example_indices.len();
        let quota = self
            .adversarial_quota(requested_batch_size)
            .min(plan.indices.len());
        plan.adversarial_fallback_count = quota.saturating_sub(plan.adversarial_count);
    }

    fn validate_adversarial_indices(
        n: usize,
        indices: Vec<usize>,
    ) -> Result<(Vec<usize>, BTreeSet<usize>), TrainerError> {
        let mut set = BTreeSet::new();
        for idx in indices {
            if idx >= n {
                return Err(Self::config_error(
                    "adversarial_indices",
                    format!("index {idx} outside sampler length {n}"),
                ));
            }
            if !set.insert(idx) {
                return Err(Self::config_error(
                    "adversarial_indices",
                    format!("duplicate adversarial index {idx}"),
                ));
            }
        }
        Ok((set.iter().copied().collect(), set))
    }

    fn config_error(field: &'static str, message: impl Into<String>) -> TrainerError {
        TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message).with_context(json!({
            "field": field,
            "file": "file:crates/context-graph-mejepa-train/src/sampler/mod.rs",
            "remediation": "fix sampler inputs; all vectors must be same length and all weights finite"
        }))
    }
}

fn validate_operator_override_input(
    input: &OperatorOverrideSamplerInput,
) -> Result<(), TrainerError> {
    let n = input.l_steps.len();
    if input.example_sources.len() != n {
        return Err(BatchSampler::config_error(
            "example_sources",
            format!(
                "length {} does not match l_steps length {n}",
                input.example_sources.len()
            ),
        ));
    }
    for (idx, source) in input.example_sources.iter().enumerate() {
        if source.prediction_id.0 == [0_u8; 16] {
            return Err(BatchSampler::config_error(
                "example_sources.prediction_id",
                format!("prediction_id at index {idx} must be non-zero"),
            ));
        }
        source
            .chunk_id
            .validate(&format!("example_sources[{idx}].chunk_id"))
            .map_err(|err| {
                BatchSampler::config_error(
                    "example_sources.chunk_id",
                    format!("invalid chunk_id at index {idx}: {err}"),
                )
            })?;
    }
    Ok(())
}

fn apply_operator_override_rows(
    input: &mut OperatorOverrideSamplerInput,
    db: &DB,
) -> Result<OperatorOverrideSamplerReport, TrainerError> {
    let overrides = load_operator_overrides(db)?;
    let live_predictions = load_live_prediction_rows(db)?;
    let operator_quality = load_operator_quality_rows(db)?;
    let mut index_weight_multiplier_micros: BTreeMap<usize, u32> = BTreeMap::new();
    let mut boost_audit = Vec::new();
    let mut boosted_indices = BTreeSet::new();
    let mut dismissed_override_count = 0_usize;

    for (prediction_id, override_record) in &overrides {
        validate_override_multiplier(override_record)?;
        let prediction = live_predictions.get(prediction_id).ok_or_else(|| {
            BatchSampler::config_error(
                "operator_overrides.prediction_id",
                format!(
                    "MEJEPA_OPERATOR_OVERRIDE_SAMPLER_PREDICTION_MISSING: override {} has no live prediction row",
                    hex::encode(prediction_id.0)
                ),
            )
        })?;
        if operator_override_dismissed(db, &prediction.task_id)? {
            dismissed_override_count += 1;
            continue;
        }
        let quality = operator_quality
            .get(&override_record.operator_id)
            .cloned()
            .unwrap_or_default();
        let multiplier = operator_quality_multiplier(
            input.config.operator_override_min_multiplier,
            input.config.operator_override_max_multiplier,
            quality.quality_score(),
        )?;
        let multiplier_micros = multiplier_to_micros(multiplier);
        let quality_score_micros = multiplier_to_micros(quality.quality_score());
        for (idx, source) in input.example_sources.iter().enumerate() {
            if source.prediction_id == *prediction_id
                && prediction
                    .covered_chunks
                    .iter()
                    .any(|chunk| chunk == &source.chunk_id)
            {
                input.overrides[idx] = true;
                index_weight_multiplier_micros
                    .entry(idx)
                    .and_modify(|current| *current = (*current).max(multiplier_micros))
                    .or_insert(multiplier_micros);
                boost_audit.push(OperatorOverrideBoostAuditRow {
                    example_index: idx,
                    prediction_id_hex: hex::encode(prediction_id.0),
                    chunk_id: source.chunk_id.0.clone(),
                    operator_id: override_record.operator_id.clone(),
                    operator_quality_score_micros: quality_score_micros,
                    sampling_weight_multiplier_micros: multiplier_micros,
                    contribution_count: quality.contribution_count,
                    downstream_linked_count: quality.downstream_linked_count,
                });
                boosted_indices.insert(idx);
            }
        }
    }

    let boosted_indices = boosted_indices.into_iter().collect::<Vec<_>>();
    Ok(OperatorOverrideSamplerReport {
        override_rows_scanned: overrides.len(),
        live_prediction_rows_scanned: live_predictions.len(),
        operator_quality_rows_scanned: operator_quality.len(),
        dismissed_override_count,
        applied_count: boosted_indices.len(),
        boosted_indices,
        index_weight_multiplier_micros,
        boost_audit,
    })
}

fn apply_sampler_reward_rows(
    input: &OperatorOverrideSamplerInput,
    db: &DB,
    last_consumed_oracle_ts: i64,
) -> Result<SamplerRewardSamplerReport, TrainerError> {
    let reward_rows = read_all_sampler_reward_signals(db).map_err(|err| {
        BatchSampler::config_error("sampler_rewards", format!("{}: {err}", err.code()))
    })?;
    let mut index_weight_multiplier_micros: BTreeMap<usize, u32> = BTreeMap::new();
    let mut boost_audit = Vec::new();
    let mut boosted_indices = BTreeSet::new();
    let mut awaiting_oracle_count = 0_usize;
    let mut quarantined_non_finite_count = 0_usize;
    let mut stale_count = 0_usize;

    for row in &reward_rows {
        match row.status {
            SamplerRewardStatus::AwaitingOracle => {
                awaiting_oracle_count += 1;
                continue;
            }
            SamplerRewardStatus::QuarantinedNonFinite => {
                quarantined_non_finite_count += 1;
                continue;
            }
            SamplerRewardStatus::Ready | SamplerRewardStatus::ClampedExtremeSurprise => {}
        }
        validate_sampler_reward_multiplier(row)?;
        if row.oracle_observed_at_unix_ms <= last_consumed_oracle_ts {
            stale_count += 1;
            continue;
        }
        let multiplier_micros = multiplier_to_micros(row.sampling_weight_multiplier);
        let surprise_z_micros = multiplier_to_micros(row.surprise_z);
        for (idx, source) in input.example_sources.iter().enumerate() {
            if source.prediction_id == row.prediction_id {
                index_weight_multiplier_micros
                    .entry(idx)
                    .and_modify(|current| *current = (*current).max(multiplier_micros))
                    .or_insert(multiplier_micros);
                boost_audit.push(SamplerRewardBoostAuditRow {
                    example_index: idx,
                    prediction_id_hex: hex::encode(row.prediction_id.0),
                    chunk_id: source.chunk_id.0.clone(),
                    cell_id: row.cell_id.clone(),
                    surprise_z_micros,
                    sampling_weight_multiplier_micros: multiplier_micros,
                    oracle_observed_at_unix_ms: row.oracle_observed_at_unix_ms,
                    status_code: row.status_code().to_string(),
                });
                boosted_indices.insert(idx);
            }
        }
    }

    let boosted_indices = boosted_indices.into_iter().collect::<Vec<_>>();
    Ok(SamplerRewardSamplerReport {
        reward_rows_scanned: reward_rows.len(),
        awaiting_oracle_count,
        quarantined_non_finite_count,
        stale_count,
        applied_count: boosted_indices.len(),
        boosted_indices,
        index_weight_multiplier_micros,
        boost_audit,
    })
}

#[derive(Debug, Clone, Copy, Default)]
struct OperatorQualitySnapshot {
    quality_score_micros: u32,
    contribution_count: usize,
    downstream_linked_count: usize,
}

impl OperatorQualitySnapshot {
    fn quality_score(self) -> f32 {
        micros_to_multiplier(self.quality_score_micros)
    }
}

fn load_operator_quality_rows(
    db: &DB,
) -> Result<BTreeMap<String, OperatorQualitySnapshot>, TrainerError> {
    let report = match operator_contribution_report_from_db(db, usize::MAX, None) {
        Ok(report) => report,
        Err(OperatorContributionError::CfMissing) => {
            return Ok(BTreeMap::new());
        }
        Err(err) => {
            return Err(BatchSampler::config_error(
                "operator_contributions",
                err.to_string(),
            ));
        }
    };
    Ok(report
        .quality_ranking
        .into_iter()
        .map(|row| {
            (
                row.operator_id,
                OperatorQualitySnapshot {
                    quality_score_micros: multiplier_to_micros(row.quality_score),
                    contribution_count: row.contribution_count,
                    downstream_linked_count: row.downstream_linked_count,
                },
            )
        })
        .collect())
}

fn operator_quality_multiplier(
    min: f32,
    max: f32,
    quality_score: f32,
) -> Result<f32, TrainerError> {
    if !min.is_finite() || !max.is_finite() || !quality_score.is_finite() {
        return Err(BatchSampler::config_error(
            "operator_override_quality_multiplier",
            "min, max, and quality_score must be finite",
        ));
    }
    if min < 1.0 || max < min {
        return Err(BatchSampler::config_error(
            "operator_override_quality_multiplier",
            "must satisfy 1 <= min <= max",
        ));
    }
    if !(0.0..=1.0).contains(&quality_score) {
        return Err(BatchSampler::config_error(
            "operator_override_quality_multiplier",
            "quality_score must be in [0,1]",
        ));
    }
    Ok(min + (max - min) * quality_score)
}

fn default_operator_override_weight_multipliers(overrides: &[bool]) -> Vec<f32> {
    overrides
        .iter()
        .map(|override_active| {
            if *override_active {
                OPERATOR_OVERRIDE_SAMPLING_WEIGHT
            } else {
                1.0
            }
        })
        .collect()
}

fn multiplier_to_micros(value: f32) -> u32 {
    (value * 1_000_000.0).round().clamp(0.0, u32::MAX as f32) as u32
}

fn micros_to_multiplier(value: u32) -> f32 {
    value as f32 / 1_000_000.0
}

fn load_operator_overrides(
    db: &DB,
) -> Result<BTreeMap<PredictionId, OperatorOverride>, TrainerError> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES)
        .ok_or_else(|| {
            BatchSampler::config_error(
                "operator_overrides.cf",
                "missing CF_MEJEPA_OPERATOR_OVERRIDES",
            )
        })?;
    let mut out = BTreeMap::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item.map_err(map_rocksdb_error)?;
        let record: OperatorOverride = bincode::deserialize(&value).map_err(map_bincode_error)?;
        validate_override_multiplier(&record)?;
        if key.as_ref() != record.prediction_id.as_slice() {
            return Err(BatchSampler::config_error(
                "operator_overrides.key",
                "CF_MEJEPA_OPERATOR_OVERRIDES key does not match payload prediction_id",
            ));
        }
        out.insert(PredictionId(record.prediction_id), record);
    }
    Ok(out)
}

fn load_live_prediction_rows(
    db: &DB,
) -> Result<BTreeMap<PredictionId, RealityPrediction>, TrainerError> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)
        .ok_or_else(|| {
            BatchSampler::config_error("live_predictions.cf", "missing CF_MEJEPA_LIVE_PREDICTIONS")
        })?;
    let mut out = BTreeMap::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item.map_err(map_rocksdb_error)?;
        if key.len() != 40 {
            return Err(BatchSampler::config_error(
                "live_predictions.key",
                format!("expected 40-byte live prediction key, got {}", key.len()),
            ));
        }
        let prediction: RealityPrediction =
            bincode::deserialize(&value).map_err(map_bincode_error)?;
        prediction
            .validate()
            .map_err(|err| BatchSampler::config_error("live_predictions.value", err.to_string()))?;
        if prediction.session_id.as_slice() != &key[0..16] {
            return Err(BatchSampler::config_error(
                "live_predictions.session_id",
                "key session prefix does not match prediction payload",
            ));
        }
        let mut timestamp = [0_u8; 8];
        timestamp.copy_from_slice(&key[16..24]);
        if prediction.created_at_unix_ms != i64::from_be_bytes(timestamp) {
            return Err(BatchSampler::config_error(
                "live_predictions.created_at_unix_ms",
                "key timestamp does not match prediction payload",
            ));
        }
        if prediction.prediction_id.as_slice() != &key[24..40] {
            return Err(BatchSampler::config_error(
                "live_predictions.prediction_id",
                "key prediction suffix does not match prediction payload",
            ));
        }
        out.insert(PredictionId(prediction.prediction_id), prediction);
    }
    Ok(out)
}

fn operator_override_dismissed(db: &DB, task_id: &TaskId) -> Result<bool, TrainerError> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS)
        .ok_or_else(|| {
            BatchSampler::config_error(
                "active_learning_evictions.cf",
                "missing CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS",
            )
        })?;
    let Some(value) = db
        .get_cf(cf, task_id.0.as_bytes())
        .map_err(map_rocksdb_error)?
    else {
        return Ok(false);
    };
    let entry: ActiveLearningQueueEntry =
        bincode::deserialize(&value).map_err(map_bincode_error)?;
    Ok(entry.reason.starts_with("operator_dismissed:"))
}

fn validate_override_multiplier(record: &OperatorOverride) -> Result<(), TrainerError> {
    if !record.sampling_weight_multiplier.is_finite()
        || (record.sampling_weight_multiplier - OPERATOR_OVERRIDE_SAMPLING_WEIGHT).abs()
            > f32::EPSILON
    {
        return Err(BatchSampler::config_error(
            "operator_overrides.sampling_weight_multiplier",
            "sampling_weight_multiplier must be exactly 6.0",
        ));
    }
    Ok(())
}

fn validate_sampler_reward_multiplier(row: &SamplerRewardSignal) -> Result<(), TrainerError> {
    if !row.sampling_weight_multiplier.is_finite() || row.sampling_weight_multiplier < 1.0 {
        return Err(BatchSampler::config_error(
            "sampler_rewards.sampling_weight_multiplier",
            "sampling_weight_multiplier must be finite and >= 1.0",
        ));
    }
    Ok(())
}

fn map_rocksdb_error(err: rocksdb::Error) -> TrainerError {
    BatchSampler::config_error("rocksdb", err.to_string())
}

fn map_bincode_error(err: Box<bincode::ErrorKind>) -> TrainerError {
    BatchSampler::config_error("bincode", err.to_string())
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
            assert_eq!(
                a.next_batch(None, 16).unwrap(),
                b.next_batch(None, 16).unwrap()
            );
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

    #[test]
    fn adversarial_mix_reserves_twenty_percent() {
        let cfg = TrainingConfig {
            random_seed: 19,
            batch_size: 10,
            adversarial_mix_ratio: 0.20,
            ..TrainingConfig::default()
        };
        let mut sampler = BatchSampler::new_with_adversarial_indices(
            cfg,
            vec![0.2; 50],
            vec![false; 50],
            vec![0.0; 50],
            (0..50).map(|idx| format!("task-{idx}")).collect(),
            (40..50).collect(),
            PatchSimilarityGraph {
                neighbors: vec![Vec::new(); 50],
            },
        )
        .unwrap();
        let plan = sampler.next_batch(None, 10).unwrap();
        assert_eq!(plan.indices.len(), 10);
        assert_eq!(plan.adversarial_count, 2);
        assert_eq!(plan.adversarial_fallback_count, 0);
        assert_eq!(plan.regular_count, 8);
        assert!(plan
            .adversarial_example_indices
            .iter()
            .all(|idx| (40..50).contains(idx)));
    }

    #[test]
    fn foundationality_scores_boost_sampler_weights() {
        let cfg = TrainingConfig {
            sampling_foundationality_lambda: 2.0,
            sampling_force_threshold: 1.0,
            ..TrainingConfig::default()
        };
        let sampler = BatchSampler::new_with_foundationality_scores(
            cfg,
            vec![0.2, 0.2],
            vec![false, false],
            vec![0.0, 0.0],
            vec!["leaf".to_string(), "bedrock".to_string()],
            vec![0.0, 1.0],
            Vec::new(),
            PatchSimilarityGraph {
                neighbors: vec![Vec::new(); 2],
            },
        )
        .unwrap();
        assert!((sampler.weights[0] - 0.2).abs() < 1e-6);
        assert!((sampler.weights[1] - 0.6).abs() < 1e-6);
        assert!(sampler.regular_indices.contains(&1));
    }

    #[test]
    fn duplicate_adversarial_index_fails_closed() {
        let cfg = TrainingConfig::default();
        let err = BatchSampler::new_with_adversarial_indices(
            cfg,
            vec![0.2; 3],
            vec![false; 3],
            vec![0.0; 3],
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec![1, 1],
            PatchSimilarityGraph {
                neighbors: vec![Vec::new(); 3],
            },
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
        assert!(err.to_string().contains("duplicate adversarial index"));
    }
}
