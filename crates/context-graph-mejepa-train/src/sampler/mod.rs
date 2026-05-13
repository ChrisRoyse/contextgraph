pub mod cross_task;

use crate::config::TrainingConfig;
use crate::error::{TrainerError, TrainerErrorCode};
use crate::learning_signal::{sampling_weight, should_drop, should_force_include};
use context_graph_mejepa::system_cost::SystemCostCounters;
use context_graph_mejepa::{
    ActiveLearningQueueEntry, ChunkId, OperatorOverride, PredictionId, RealityPrediction, TaskId,
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
    pub cross_task_indices: Vec<usize>,
    pub cross_task_fallback_count: usize,
    pub operator_override_sampler_applied_count: usize,
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
    pub dismissed_override_count: usize,
    pub applied_count: usize,
    pub boosted_indices: Vec<usize>,
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
    pub operator_override_sampler_applied_count: usize,
    operator_override_boosted_indices: BTreeSet<usize>,
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
        config.validate()?;
        let n = l_steps.len();
        if n == 0 {
            return Err(Self::config_error("l_steps", "sampler cannot be empty"));
        }
        for (name, len) in [
            ("overrides", overrides.len()),
            ("ages_days", ages_days.len()),
            ("task_ids", task_ids.len()),
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
            operator_override_sampler_applied_count: 0,
            operator_override_boosted_indices: BTreeSet::new(),
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
        let mut sampler = Self::new(
            input.config,
            input.l_steps,
            input.overrides,
            input.ages_days,
            input.task_ids,
            input.patch_similarity_graph,
        )?;
        report.boosted_indices.sort_unstable();
        sampler.operator_override_sampler_applied_count = report.applied_count;
        sampler.operator_override_boosted_indices = boosted_index_set;
        sampler.system_cost_counters = system_cost_counters;
        Ok((sampler, report))
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
        self.record_operator_override_sampler_batch(&mut plan);
        Ok(plan)
    }

    pub(crate) fn force_prefix(&mut self, batch_size: usize) -> BatchPlan {
        let mut plan = BatchPlan {
            indices: Vec::with_capacity(batch_size),
            force_count: 0,
            regular_count: 0,
            cross_task_indices: Vec::new(),
            cross_task_fallback_count: 0,
            operator_override_sampler_applied_count: 0,
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
        if count > 0 {
            if let Some(counters) = &self.system_cost_counters {
                counters.record_operator_override_sampler_applied(count as u64);
            }
        }
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
        for (idx, source) in input.example_sources.iter().enumerate() {
            if source.prediction_id == *prediction_id
                && prediction
                    .covered_chunks
                    .iter()
                    .any(|chunk| chunk == &source.chunk_id)
            {
                input.overrides[idx] = true;
                boosted_indices.insert(idx);
            }
        }
    }

    let boosted_indices = boosted_indices.into_iter().collect::<Vec<_>>();
    Ok(OperatorOverrideSamplerReport {
        override_rows_scanned: overrides.len(),
        live_prediction_rows_scanned: live_predictions.len(),
        dismissed_override_count,
        applied_count: boosted_indices.len(),
        boosted_indices,
    })
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
}
