use serde::{Deserialize, Serialize};

use crate::heal::cf::{encode_plasticity_history_key, encode_value, CF_MEJEPA_PLASTICITY_HISTORY};
use crate::heal::errors::HealError;
use crate::heal::ewc::EWC_FISHER_DEGENERATE_EPSILON;
use crate::heal::pipeline::{sha_f32s, SelfHealingPipeline, StatusChange};
use crate::heal::plasticity_metrics::{
    measure_plasticity, push_gradient_window, select_reinit_candidates, validate_gradient,
};

pub const PLASTICITY_HISTORY_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_DORMANCY_ACTIVATION_THRESHOLD: f32 = 0.05;
pub const DEFAULT_DORMANCY_BATCH_FRACTION: f32 = 0.99;
pub const DEFAULT_DORMANCY_FRACTION_THRESHOLD: f32 = 0.15;
pub const DEFAULT_GRAD_RANK_THRESHOLD_FRACTION: f32 = 0.5;
pub const DEFAULT_COLLAPSE_DORMANCY_FRACTION: f32 = 0.99;
pub const DEFAULT_MIN_REINIT_RATE: f32 = 0.0001;
pub const DEFAULT_BASE_REINIT_RATE: f32 = 0.001;
pub const DEFAULT_MAX_REINIT_RATE: f32 = 0.01;
pub const DEFAULT_GRADIENT_WINDOW: usize = 128;
pub const DEFAULT_MIN_GRADIENT_SAMPLES_FOR_RANK: usize = 8;
pub const PLASTICITY_REGRESSION_ALERT: &str = "MEJEPA_PLASTICITY_REGRESSION";
pub const PLASTICITY_COLLAPSE_ALERT: &str = "MEJEPA_PLASTICITY_COLLAPSE";
pub const EWC_BLOCKED_REINIT_BUDGET_ALERT: &str = "EWC_BLOCKED_REINIT_BUDGET";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlasticityConfig {
    pub activation_threshold: f32,
    pub dormant_batch_fraction: f32,
    pub dormant_fraction_threshold: f32,
    pub grad_rank_threshold_fraction: f32,
    pub collapse_dormancy_fraction: f32,
    pub min_reinit_rate: f32,
    pub base_reinit_rate: f32,
    pub max_reinit_rate: f32,
    pub gradient_window_size: usize,
    pub min_gradient_samples_for_rank: usize,
}

impl Default for PlasticityConfig {
    fn default() -> Self {
        Self {
            activation_threshold: DEFAULT_DORMANCY_ACTIVATION_THRESHOLD,
            dormant_batch_fraction: DEFAULT_DORMANCY_BATCH_FRACTION,
            dormant_fraction_threshold: DEFAULT_DORMANCY_FRACTION_THRESHOLD,
            grad_rank_threshold_fraction: DEFAULT_GRAD_RANK_THRESHOLD_FRACTION,
            collapse_dormancy_fraction: DEFAULT_COLLAPSE_DORMANCY_FRACTION,
            min_reinit_rate: DEFAULT_MIN_REINIT_RATE,
            base_reinit_rate: DEFAULT_BASE_REINIT_RATE,
            max_reinit_rate: DEFAULT_MAX_REINIT_RATE,
            gradient_window_size: DEFAULT_GRADIENT_WINDOW,
            min_gradient_samples_for_rank: DEFAULT_MIN_GRADIENT_SAMPLES_FOR_RANK,
        }
    }
}

impl PlasticityConfig {
    pub fn validate(&self) -> Result<(), HealError> {
        validate_unit_interval("plasticity.activation_threshold", self.activation_threshold)?;
        validate_unit_interval(
            "plasticity.dormant_batch_fraction",
            self.dormant_batch_fraction,
        )?;
        validate_unit_interval(
            "plasticity.dormant_fraction_threshold",
            self.dormant_fraction_threshold,
        )?;
        validate_unit_interval(
            "plasticity.grad_rank_threshold_fraction",
            self.grad_rank_threshold_fraction,
        )?;
        validate_unit_interval(
            "plasticity.collapse_dormancy_fraction",
            self.collapse_dormancy_fraction,
        )?;
        if self.min_reinit_rate <= 0.0
            || self.min_reinit_rate < DEFAULT_MIN_REINIT_RATE
            || self.base_reinit_rate < self.min_reinit_rate
            || self.max_reinit_rate < self.base_reinit_rate
            || self.max_reinit_rate > DEFAULT_MAX_REINIT_RATE
            || !self.min_reinit_rate.is_finite()
            || !self.base_reinit_rate.is_finite()
            || !self.max_reinit_rate.is_finite()
        {
            return Err(HealError::invalid(
                "plasticity.reinit_rate",
                "reinit rates must be finite and clamped to [0.0001, 0.01]",
            ));
        }
        if self.gradient_window_size == 0 || self.min_gradient_samples_for_rank == 0 {
            return Err(HealError::invalid(
                "plasticity.gradient_window",
                "gradient window sizes must be > 0",
            ));
        }
        Ok(())
    }

    pub fn rank_threshold(&self, parameter_count: usize) -> f32 {
        parameter_count as f32 * self.grad_rank_threshold_fraction
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlasticityRuntimeState {
    pub consecutive_dormancy_crosses: u32,
    pub current_reinit_rate: f32,
    pub gradient_window: Vec<Vec<f32>>,
}

impl Default for PlasticityRuntimeState {
    fn default() -> Self {
        Self {
            consecutive_dormancy_crosses: 0,
            current_reinit_rate: DEFAULT_BASE_REINIT_RATE,
            gradient_window: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GsnrDistribution {
    pub min: f32,
    pub p50: f32,
    pub p90: f32,
    pub max: f32,
    pub low_signal_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlasticityHistoryRow {
    pub schema_version: u32,
    pub training_tick: u64,
    pub generated_at_unix_ms: i64,
    pub parameter_count: usize,
    pub sample_count: usize,
    pub gradient_sample_count: usize,
    pub dormancy_fraction: f32,
    pub dormant_unit_count: usize,
    pub dormant_units_sample: Vec<usize>,
    pub grad_cov_effective_rank: f32,
    pub grad_cov_rank_threshold: f32,
    pub gsnr_distribution: GsnrDistribution,
    pub reinit_rate: f32,
    pub reinit_count: usize,
    pub reinitialized_units: Vec<usize>,
    pub ewc_blocked_reinit_budget: usize,
    pub protected_unit_count: usize,
    pub consecutive_regression_ticks: u32,
    pub online_update_paused: bool,
    pub alert_codes: Vec<String>,
    pub weight_sha_before: [u8; 32],
    pub weight_sha_after: [u8; 32],
}

pub fn tick_predictor_plasticity(
    pipeline: &mut SelfHealingPipeline,
    gradient: &[f32],
) -> Result<PlasticityHistoryRow, HealError> {
    pipeline.plasticity_config.validate()?;
    validate_gradient(gradient, pipeline.predictor.weights.len())?;
    push_gradient_window(
        &mut pipeline.plasticity_state,
        gradient,
        pipeline.plasticity_config.gradient_window_size,
    );
    let params_before = pipeline.predictor.weights.clone();
    let metrics = measure_plasticity(
        &pipeline.dormant_activation_window,
        &pipeline.plasticity_state.gradient_window,
        &params_before,
        pipeline.plasticity_config,
    )?;
    let mut alert_codes = Vec::new();
    let dormancy_crossed =
        metrics.dormancy_fraction > pipeline.plasticity_config.dormant_fraction_threshold;
    if dormancy_crossed {
        pipeline.plasticity_state.consecutive_dormancy_crosses = pipeline
            .plasticity_state
            .consecutive_dormancy_crosses
            .saturating_add(1);
    } else {
        pipeline.plasticity_state.consecutive_dormancy_crosses = 0;
        pipeline.plasticity_state.current_reinit_rate = pipeline.plasticity_config.base_reinit_rate;
    }
    if pipeline.plasticity_state.consecutive_dormancy_crosses >= 3 {
        alert_codes.push(PLASTICITY_REGRESSION_ALERT.to_string());
        pipeline.plasticity_state.current_reinit_rate =
            (pipeline.plasticity_state.current_reinit_rate * 2.0).clamp(
                pipeline.plasticity_config.min_reinit_rate,
                pipeline.plasticity_config.max_reinit_rate,
            );
    }

    let collapse = metrics.dormancy_fraction
        >= pipeline.plasticity_config.collapse_dormancy_fraction
        || metrics.dormant_unit_count == params_before.len();
    let protected_units = pipeline
        .ewc
        .protected_parameter_indices(EWC_FISHER_DEGENERATE_EPSILON)?;
    let mut reinitialized_units = Vec::new();
    let mut ewc_blocked_reinit_budget = 0usize;
    let rank_trigger = metrics.gradient_sample_count
        >= pipeline.plasticity_config.min_gradient_samples_for_rank
        && metrics.grad_cov_effective_rank < metrics.grad_cov_rank_threshold;
    let should_reinit = !collapse && (dormancy_crossed || rank_trigger);
    if should_reinit {
        let selected = select_reinit_candidates(
            &metrics,
            &protected_units,
            pipeline.plasticity_state.current_reinit_rate,
            pipeline.plasticity_config,
        );
        ewc_blocked_reinit_budget = selected.ewc_blocked_reinit_budget;
        reinitialized_units = selected.reinit_units;
        if ewc_blocked_reinit_budget > 0 {
            alert_codes.push(EWC_BLOCKED_REINIT_BUDGET_ALERT.to_string());
        }
        if !reinitialized_units.is_empty() {
            let seed = pipeline.status.lock().unwrap().observation_counter
                ^ u64::from_be_bytes(params_before_sha_prefix(&params_before));
            pipeline
                .predictor
                .reinitialize_units_from_init_distribution(&reinitialized_units, seed)?;
            if let Some(counters) = pipeline.storage.system_cost_counters() {
                counters.record_dormant_units_reinit(reinitialized_units.len() as u64);
            }
        }
    }
    if collapse {
        alert_codes.push(PLASTICITY_COLLAPSE_ALERT.to_string());
        pipeline.status.lock().unwrap().status_change = StatusChange::Paused;
    }

    let params_after = pipeline.predictor.weights.clone();
    let training_tick = pipeline
        .status
        .lock()
        .unwrap()
        .observation_counter
        .saturating_add(1);
    let row = PlasticityHistoryRow {
        schema_version: PLASTICITY_HISTORY_SCHEMA_VERSION,
        training_tick,
        generated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        parameter_count: params_before.len(),
        sample_count: metrics.sample_count,
        gradient_sample_count: metrics.gradient_sample_count,
        dormancy_fraction: metrics.dormancy_fraction,
        dormant_unit_count: metrics.dormant_unit_count,
        dormant_units_sample: metrics.dormant_units.iter().copied().take(32).collect(),
        grad_cov_effective_rank: metrics.grad_cov_effective_rank,
        grad_cov_rank_threshold: metrics.grad_cov_rank_threshold,
        gsnr_distribution: metrics.gsnr_distribution,
        reinit_rate: pipeline.plasticity_state.current_reinit_rate,
        reinit_count: reinitialized_units.len(),
        reinitialized_units,
        ewc_blocked_reinit_budget,
        protected_unit_count: protected_units.len(),
        consecutive_regression_ticks: pipeline.plasticity_state.consecutive_dormancy_crosses,
        online_update_paused: collapse,
        alert_codes,
        weight_sha_before: sha_f32s(&params_before),
        weight_sha_after: sha_f32s(&params_after),
    };
    persist_plasticity_history(pipeline, &row)?;
    if collapse {
        return Err(HealError::PlasticityCollapse {
            training_tick,
            dormancy_fraction: row.dormancy_fraction,
            dormant_unit_count: row.dormant_unit_count,
            parameter_count: row.parameter_count,
        });
    }
    Ok(row)
}

pub fn persist_plasticity_history(
    pipeline: &SelfHealingPipeline,
    row: &PlasticityHistoryRow,
) -> Result<(), HealError> {
    row.validate()?;
    pipeline.storage.put_cf_readback(
        CF_MEJEPA_PLASTICITY_HISTORY,
        &encode_plasticity_history_key(row.training_tick),
        &encode_value(row)?,
    )
}

impl PlasticityHistoryRow {
    pub fn validate(&self) -> Result<(), HealError> {
        if self.schema_version != PLASTICITY_HISTORY_SCHEMA_VERSION {
            return Err(HealError::invalid(
                "plasticity_history.schema_version",
                format!(
                    "expected {PLASTICITY_HISTORY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            ));
        }
        if self.parameter_count == 0 {
            return Err(HealError::invalid(
                "plasticity_history.parameter_count",
                "parameter_count must be > 0",
            ));
        }
        for (name, value) in [
            ("dormancy_fraction", self.dormancy_fraction),
            ("grad_cov_effective_rank", self.grad_cov_effective_rank),
            ("grad_cov_rank_threshold", self.grad_cov_rank_threshold),
            ("reinit_rate", self.reinit_rate),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(HealError::invalid(
                    format!("plasticity_history.{name}"),
                    "metric must be finite and non-negative",
                ));
            }
        }
        if self.reinit_rate < DEFAULT_MIN_REINIT_RATE || self.reinit_rate > DEFAULT_MAX_REINIT_RATE
        {
            return Err(HealError::invalid(
                "plasticity_history.reinit_rate",
                "reinit_rate must stay clamped to [0.0001, 0.01]",
            ));
        }
        if self.reinit_count != self.reinitialized_units.len() {
            return Err(HealError::invalid(
                "plasticity_history.reinit_count",
                "reinit_count must match reinitialized_units length",
            ));
        }
        Ok(())
    }
}

fn validate_unit_interval(field: &str, value: f32) -> Result<(), HealError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(HealError::invalid(
            field,
            format!("value must be finite in [0,1], got {value}"),
        ));
    }
    Ok(())
}

fn params_before_sha_prefix(params: &[f32]) -> [u8; 8] {
    let sha = sha_f32s(params);
    sha[..8].try_into().unwrap_or([0; 8])
}
