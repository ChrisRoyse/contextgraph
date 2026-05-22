// Inspired by ruvnet/RuVector crates/sona/src/ewc.rs at HEAD ef5274c2
// (clean-room reimplementation; no code copied).

use std::collections::BTreeMap;

use context_graph_mejepa_instruments::Panel;
use serde::{Deserialize, Serialize};

use crate::heal::errors::HealError;
use crate::types::OracleOutcome;

pub const DEFAULT_INITIAL_LAMBDA: f32 = 1000.0;
pub const DEFAULT_MAX_LAMBDA: f32 = 15000.0;
pub const DEFAULT_BOUNDARY_THRESHOLD_Z: f32 = 3.0;
pub const DEFAULT_FISHER_EMA_DECAY: f32 = 0.01;
pub const PER_EMBEDDER_SNAPSHOT_CAP_BASE_PLUS_DELTAS: usize = 5;
pub const EWC_SNAPSHOT_INTERVAL_UPDATES: u64 = 1000;
pub const DEFAULT_EWC_FISHER_BUDGET: f32 = 1.0e-3;
pub const EWC_VALIDATION_REGRESSION_DECAY_THRESHOLD: f32 = 0.02;
pub const EWC_LAMBDA_DECAY_FACTOR: f32 = 0.5;
pub const EWC_FISHER_DEGENERATE_EPSILON: f32 = 1.0e-12;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FisherMatrix {
    pub diagonal: Vec<f32>,
    pub rank: usize,
    pub step_count: u64,
}

impl FisherMatrix {
    pub fn try_new_zero(dim: usize) -> Result<Self, HealError> {
        if dim == 0 {
            return Err(HealError::invalid("fisher.dim", "dim must be > 0"));
        }
        Ok(Self {
            diagonal: vec![0.0; dim],
            rank: 0,
            step_count: 0,
        })
    }

    pub fn try_from_diagonal(diagonal: Vec<f32>, step_count: u64) -> Result<Self, HealError> {
        if diagonal.is_empty() {
            return Err(HealError::invalid("fisher.diagonal", "must be non-empty"));
        }
        if diagonal
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(HealError::invalid(
                "fisher.diagonal",
                "all entries must be finite and non-negative",
            ));
        }
        let rank = diagonal.iter().filter(|value| **value > 1e-12).count();
        Ok(Self {
            diagonal,
            rank,
            step_count,
        })
    }

    pub fn recompute_rank(&self) -> usize {
        self.diagonal.iter().filter(|value| **value > 1e-12).count()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TaskSnapshot {
    pub theta_star: Vec<f32>,
    pub fisher_diag_at_boundary: Vec<f32>,
    pub boundary_step: u64,
    pub corpus_sha: [u8; 32],
    pub frozen_at: i64,
}

impl TaskSnapshot {
    pub fn try_new(
        theta_star: Vec<f32>,
        fisher_diag_at_boundary: Vec<f32>,
        boundary_step: u64,
        corpus_sha: [u8; 32],
        frozen_at: i64,
    ) -> Result<Self, HealError> {
        if theta_star.len() != fisher_diag_at_boundary.len() {
            return Err(HealError::invalid(
                "task_snapshot.dim",
                format!(
                    "theta len {} != fisher len {}",
                    theta_star.len(),
                    fisher_diag_at_boundary.len()
                ),
            ));
        }
        if theta_star.is_empty() {
            return Err(HealError::invalid(
                "task_snapshot.theta",
                "theta must be non-empty",
            ));
        }
        if theta_star.iter().any(|value| !value.is_finite()) {
            return Err(HealError::invalid(
                "task_snapshot.theta",
                "theta contains NaN/Inf",
            ));
        }
        if fisher_diag_at_boundary
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(HealError::invalid(
                "task_snapshot.fisher",
                "fisher contains non-finite or negative values",
            ));
        }
        Ok(Self {
            theta_star,
            fisher_diag_at_boundary,
            boundary_step,
            corpus_sha,
            frozen_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HessianTraceEstimator {
    pub last_estimate: f32,
    pub hutchinson_samples: u32,
    pub last_update_step: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BoundaryDetectorState {
    pub loss_running_mean: f32,
    pub loss_running_var: f32,
    pub hessian_trace_estimator: HessianTraceEstimator,
    pub boundary_threshold: f32,
    pub consecutive_above_threshold: u32,
}

impl Default for BoundaryDetectorState {
    fn default() -> Self {
        Self {
            loss_running_mean: 0.0,
            loss_running_var: 1.0,
            hessian_trace_estimator: HessianTraceEstimator {
                last_estimate: 0.0,
                hutchinson_samples: 0,
                last_update_step: 0,
            },
            boundary_threshold: DEFAULT_BOUNDARY_THRESHOLD_Z,
            consecutive_above_threshold: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EwcCellLambdaState {
    pub cell_id: String,
    pub lambda: f32,
    pub validation_loss_before: f32,
    pub validation_loss_after: f32,
    pub regression_fraction: f32,
    pub decay_count: u32,
    pub updated_at_unix_ms: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EwcGuardDecision {
    pub projected_fisher_displacement: f32,
    pub budget: f32,
    pub cold_start_exemption: bool,
    pub violation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EwcPlusPlus {
    pub fisher_matrix: FisherMatrix,
    pub lambda: f32,
    pub task_snapshots: Vec<TaskSnapshot>,
    pub boundary_detector: BoundaryDetectorState,
    pub initial_lambda: f32,
    pub max_lambda: f32,
    pub predictor_dim: usize,
    pub cell_lambda_states: BTreeMap<String, EwcCellLambdaState>,
}

impl EwcPlusPlus {
    pub fn try_new(
        predictor_dim: usize,
        initial_lambda: f32,
        max_lambda: f32,
    ) -> Result<Self, HealError> {
        if predictor_dim == 0 {
            return Err(HealError::invalid("ewc.predictor_dim", "must be > 0"));
        }
        if !initial_lambda.is_finite() || !max_lambda.is_finite() {
            return Err(HealError::invalid(
                "ewc.lambda",
                "lambda values must be finite",
            ));
        }
        if initial_lambda < 1.0 || max_lambda < initial_lambda {
            return Err(HealError::invalid(
                "ewc.lambda",
                "initial_lambda must be >= 1 and <= max_lambda",
            ));
        }
        Ok(Self {
            fisher_matrix: FisherMatrix::try_new_zero(predictor_dim)?,
            lambda: initial_lambda,
            task_snapshots: Vec::new(),
            boundary_detector: BoundaryDetectorState::default(),
            initial_lambda,
            max_lambda,
            predictor_dim,
            cell_lambda_states: BTreeMap::new(),
        })
    }

    pub fn update_fisher_online<P: PredictorGradAccessor>(
        &mut self,
        panel: &Panel,
        oracle_outcome: &OracleOutcome,
        predictor: &P,
    ) -> Result<FisherUpdate, HealError> {
        if predictor.param_count() != self.predictor_dim {
            return Err(HealError::invalid(
                "ewc.predictor_dim",
                format!(
                    "predictor param count {} != ewc dim {}",
                    predictor.param_count(),
                    self.predictor_dim
                ),
            ));
        }
        let grad = predictor.gradients_for(panel, oracle_outcome)?;
        validate_finite_vec("ewc.gradient", &grad)?;
        if grad.len() != self.predictor_dim {
            return Err(HealError::invalid(
                "ewc.gradient",
                format!("gradient len {} != dim {}", grad.len(), self.predictor_dim),
            ));
        }
        let mut delta_sq_sum = 0.0f32;
        for (fisher, g) in self.fisher_matrix.diagonal.iter_mut().zip(grad) {
            let next =
                (1.0 - DEFAULT_FISHER_EMA_DECAY) * *fisher + DEFAULT_FISHER_EMA_DECAY * g * g;
            delta_sq_sum += (next - *fisher).powi(2);
            *fisher = next;
        }
        self.fisher_matrix.step_count += 1;
        self.fisher_matrix.rank = self.fisher_matrix.recompute_rank();
        Ok(FisherUpdate {
            delta_norm: delta_sq_sum.sqrt(),
            current_rank: self.fisher_matrix.rank,
        })
    }

    pub fn detect_task_boundary<H: HessianTraceProbe>(
        &mut self,
        current_loss: f32,
        predictor: &H,
    ) -> Result<bool, HealError> {
        if !current_loss.is_finite() {
            return Err(HealError::BatchNan {
                component: "ewc.current_loss".to_string(),
                witness_chain_offset: self.fisher_matrix.step_count,
            });
        }
        let curvature = predictor.hessian_trace_estimate()?;
        if !curvature.is_finite() {
            return Err(HealError::BatchNan {
                component: "ewc.curvature".to_string(),
                witness_chain_offset: self.fisher_matrix.step_count,
            });
        }
        let decay = 0.99f32;
        let old_mean = self.boundary_detector.loss_running_mean;
        if self.fisher_matrix.step_count == 0 {
            self.boundary_detector.loss_running_mean = current_loss;
            self.boundary_detector.loss_running_var = 1.0;
        } else {
            let diff = current_loss - old_mean;
            self.boundary_detector.loss_running_mean =
                decay * old_mean + (1.0 - decay) * current_loss;
            self.boundary_detector.loss_running_var =
                (decay * self.boundary_detector.loss_running_var + (1.0 - decay) * diff * diff)
                    .max(1e-6);
        }
        let z = ((current_loss - self.boundary_detector.loss_running_mean).abs()
            + curvature.abs() * 0.01)
            / self.boundary_detector.loss_running_var.sqrt();
        self.boundary_detector.hessian_trace_estimator.last_estimate = curvature;
        self.boundary_detector
            .hessian_trace_estimator
            .last_update_step = self.fisher_matrix.step_count;
        if z >= self.boundary_detector.boundary_threshold {
            self.boundary_detector.consecutive_above_threshold += 1;
        } else {
            self.boundary_detector.consecutive_above_threshold = 0;
        }
        Ok(self.boundary_detector.consecutive_above_threshold >= 1)
    }

    pub fn snapshot_current_task<S: SnapshotStore>(
        &mut self,
        predictor_params: &[f32],
        corpus_sha: [u8; 32],
        store: &S,
    ) -> Result<TaskSnapshot, HealError> {
        validate_finite_vec("ewc.predictor_params", predictor_params)?;
        if predictor_params.len() != self.predictor_dim {
            return Err(HealError::invalid(
                "ewc.predictor_params",
                format!(
                    "params len {} != dim {}",
                    predictor_params.len(),
                    self.predictor_dim
                ),
            ));
        }
        if self.task_snapshots.len() >= 9 && self.fisher_rank() < self.predictor_dim {
            return Err(HealError::FisherRankDeficient {
                rank: self.fisher_rank(),
                dim: self.predictor_dim,
            });
        }
        self.ensure_non_degenerate()?;
        let snapshot = TaskSnapshot::try_new(
            predictor_params.to_vec(),
            self.fisher_matrix.diagonal.clone(),
            self.fisher_matrix.step_count,
            corpus_sha,
            chrono::Utc::now().timestamp(),
        )?;
        self.task_snapshots.push(snapshot.clone());
        self.merge_snapshots_on_overflow()?;
        self.lambda = (self.initial_lambda * (1.0 + self.task_snapshots.len() as f32 * 0.25))
            .min(self.max_lambda);
        store.replace_task_snapshots(&self.task_snapshots)?;
        Ok(snapshot)
    }

    pub fn current_lambda(&self) -> f32 {
        self.lambda.clamp(0.0, self.max_lambda)
    }

    pub fn current_lambda_for_cell(&self, cell_id: &str) -> Result<f32, HealError> {
        validate_cell_id(cell_id)?;
        Ok(self
            .cell_lambda_states
            .get(cell_id)
            .map(|state| state.lambda)
            .unwrap_or(self.initial_lambda)
            .clamp(0.0, self.max_lambda))
    }

    pub fn ewc_penalty(&self, theta_flat: &[f32]) -> Result<f32, HealError> {
        validate_finite_vec("ewc.theta_flat", theta_flat)?;
        if theta_flat.len() != self.predictor_dim {
            return Err(HealError::invalid(
                "ewc.theta_flat",
                format!(
                    "theta len {} != dim {}",
                    theta_flat.len(),
                    self.predictor_dim
                ),
            ));
        }
        let mut penalty = 0.0f32;
        for snapshot in &self.task_snapshots {
            for ((theta, theta_star), fisher) in theta_flat
                .iter()
                .zip(&snapshot.theta_star)
                .zip(&snapshot.fisher_diag_at_boundary)
            {
                penalty += *fisher * (*theta - *theta_star).powi(2);
            }
        }
        if !penalty.is_finite() {
            return Err(HealError::BatchNan {
                component: "ewc.penalty".to_string(),
                witness_chain_offset: self.fisher_matrix.step_count,
            });
        }
        Ok(penalty)
    }

    pub fn ewc_gradient(&self, theta_flat: &[f32]) -> Result<Vec<f32>, HealError> {
        validate_finite_vec("ewc.theta_flat", theta_flat)?;
        if theta_flat.len() != self.predictor_dim {
            return Err(HealError::invalid(
                "ewc.theta_flat",
                format!(
                    "theta len {} != dim {}",
                    theta_flat.len(),
                    self.predictor_dim
                ),
            ));
        }
        let mut out = vec![0.0f32; theta_flat.len()];
        for snapshot in &self.task_snapshots {
            for (((accum, theta), theta_star), fisher) in out
                .iter_mut()
                .zip(theta_flat)
                .zip(&snapshot.theta_star)
                .zip(&snapshot.fisher_diag_at_boundary)
            {
                *accum += 2.0 * *fisher * (*theta - *theta_star);
            }
        }
        validate_finite_vec("ewc.gradient_penalty", &out)?;
        Ok(out)
    }

    pub fn guard_projected_update(
        &self,
        gradient: &[f32],
        learning_rate: f32,
        budget: f32,
    ) -> Result<EwcGuardDecision, HealError> {
        validate_finite_vec("ewc.projected_gradient", gradient)?;
        if gradient.len() != self.predictor_dim {
            return Err(HealError::invalid(
                "ewc.projected_gradient",
                format!(
                    "gradient len {} != dim {}",
                    gradient.len(),
                    self.predictor_dim
                ),
            ));
        }
        if !learning_rate.is_finite() || learning_rate <= 0.0 {
            return Err(HealError::invalid(
                "ewc.learning_rate",
                "learning rate must be finite and positive",
            ));
        }
        if !budget.is_finite() || budget <= 0.0 {
            return Err(HealError::invalid(
                "ewc.fisher_budget",
                "budget must be finite and positive",
            ));
        }
        if self.task_snapshots.is_empty() {
            return Ok(EwcGuardDecision {
                projected_fisher_displacement: 0.0,
                budget,
                cold_start_exemption: true,
                violation: false,
            });
        }
        self.ensure_non_degenerate()?;
        let mut projected = 0.0f32;
        for snapshot in &self.task_snapshots {
            for (grad, fisher) in gradient.iter().zip(&snapshot.fisher_diag_at_boundary) {
                let delta = learning_rate * *grad;
                projected += *fisher * delta * delta;
            }
        }
        if !projected.is_finite() {
            return Err(HealError::BatchNan {
                component: "ewc.projected_fisher_displacement".to_string(),
                witness_chain_offset: self.fisher_matrix.step_count,
            });
        }
        Ok(EwcGuardDecision {
            projected_fisher_displacement: projected,
            budget,
            cold_start_exemption: false,
            violation: projected > budget,
        })
    }

    pub fn auto_tune_lambda_for_cell(
        &mut self,
        cell_id: &str,
        validation_loss_before: f32,
        validation_loss_after: f32,
    ) -> Result<EwcCellLambdaState, HealError> {
        validate_cell_id(cell_id)?;
        validate_nonnegative_finite("ewc.validation_loss_before", validation_loss_before)?;
        validate_nonnegative_finite("ewc.validation_loss_after", validation_loss_after)?;
        if !self.task_snapshots.is_empty() {
            self.ensure_non_degenerate()?;
        }
        let previous = self
            .cell_lambda_states
            .get(cell_id)
            .map(|state| state.lambda)
            .unwrap_or(self.initial_lambda);
        let regression_fraction = if validation_loss_before <= f32::EPSILON {
            if validation_loss_after > validation_loss_before {
                1.0
            } else {
                0.0
            }
        } else {
            ((validation_loss_after - validation_loss_before) / validation_loss_before).max(0.0)
        };
        let should_decay = regression_fraction > EWC_VALIDATION_REGRESSION_DECAY_THRESHOLD;
        let lambda = if should_decay {
            (previous * EWC_LAMBDA_DECAY_FACTOR).max(0.0)
        } else {
            previous
        };
        let decay_count = self
            .cell_lambda_states
            .get(cell_id)
            .map(|state| state.decay_count)
            .unwrap_or(0)
            + u32::from(should_decay);
        let state = EwcCellLambdaState {
            cell_id: cell_id.to_string(),
            lambda,
            validation_loss_before,
            validation_loss_after,
            regression_fraction,
            decay_count,
            updated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        };
        self.cell_lambda_states
            .insert(cell_id.to_string(), state.clone());
        self.lambda = lambda.clamp(0.0, self.max_lambda);
        Ok(state)
    }

    pub fn snapshot_current_task_if_due<S: SnapshotStore>(
        &mut self,
        predictor_params: &[f32],
        corpus_sha: [u8; 32],
        store: &S,
    ) -> Result<Option<TaskSnapshot>, HealError> {
        if self.fisher_matrix.step_count == 0
            || !self
                .fisher_matrix
                .step_count
                .is_multiple_of(EWC_SNAPSHOT_INTERVAL_UPDATES)
        {
            return Ok(None);
        }
        if self
            .task_snapshots
            .last()
            .is_some_and(|snapshot| snapshot.boundary_step == self.fisher_matrix.step_count)
        {
            return Ok(None);
        }
        self.snapshot_current_task(predictor_params, corpus_sha, store)
            .map(Some)
    }

    pub fn protected_parameter_indices(&self, min_fisher: f32) -> Result<Vec<usize>, HealError> {
        if !min_fisher.is_finite() || min_fisher < 0.0 {
            return Err(HealError::invalid(
                "ewc.protected_min_fisher",
                "min_fisher must be finite and non-negative",
            ));
        }
        let mut protected = vec![false; self.predictor_dim];
        for (idx, value) in self.fisher_matrix.diagonal.iter().enumerate() {
            if *value > min_fisher {
                protected[idx] = true;
            }
        }
        for snapshot in &self.task_snapshots {
            if snapshot.fisher_diag_at_boundary.len() != self.predictor_dim {
                return Err(HealError::invalid(
                    "ewc.snapshot_fisher_dim",
                    format!(
                        "snapshot fisher len {} != dim {}",
                        snapshot.fisher_diag_at_boundary.len(),
                        self.predictor_dim
                    ),
                ));
            }
            for (idx, value) in snapshot.fisher_diag_at_boundary.iter().enumerate() {
                if *value > min_fisher {
                    protected[idx] = true;
                }
            }
        }
        Ok(protected
            .into_iter()
            .enumerate()
            .filter_map(|(idx, is_protected)| is_protected.then_some(idx))
            .collect())
    }

    pub fn fisher_rank(&self) -> usize {
        self.fisher_matrix.rank
    }

    pub fn readback_snapshot(&self) -> EwcReadbackEvidence {
        EwcReadbackEvidence {
            boundary_step_trajectory: self
                .task_snapshots
                .iter()
                .map(|snapshot| snapshot.boundary_step)
                .collect(),
            lambda_trajectory: vec![self.current_lambda()],
            fisher_rank_per_boundary: self
                .task_snapshots
                .iter()
                .map(|_| self.fisher_rank())
                .collect(),
            fisher_full_rank_after_boundary_10: self.task_snapshots.len() >= 10
                && self.fisher_rank() == self.predictor_dim,
            task_snapshots_count: self.task_snapshots.len(),
            predictor_dim: self.predictor_dim,
        }
    }

    fn merge_snapshots_on_overflow(&mut self) -> Result<(), HealError> {
        if self.task_snapshots.len() <= PER_EMBEDDER_SNAPSHOT_CAP_BASE_PLUS_DELTAS {
            return Ok(());
        }
        let first = self.task_snapshots.remove(0);
        let second = self.task_snapshots.remove(0);
        let theta = first
            .theta_star
            .iter()
            .zip(&second.theta_star)
            .map(|(a, b)| (a + b) * 0.5)
            .collect::<Vec<_>>();
        let fisher = first
            .fisher_diag_at_boundary
            .iter()
            .zip(&second.fisher_diag_at_boundary)
            .map(|(a, b)| a.max(*b))
            .collect::<Vec<_>>();
        let merged = TaskSnapshot::try_new(
            theta,
            fisher,
            second.boundary_step,
            second.corpus_sha,
            second.frozen_at,
        )?;
        self.task_snapshots.insert(0, merged);
        Ok(())
    }

    fn ensure_non_degenerate(&self) -> Result<(), HealError> {
        let max_fisher = self
            .task_snapshots
            .iter()
            .flat_map(|snapshot| snapshot.fisher_diag_at_boundary.iter().copied())
            .fold(0.0f32, f32::max)
            .max(
                self.fisher_matrix
                    .diagonal
                    .iter()
                    .copied()
                    .fold(0.0f32, f32::max),
            );
        if max_fisher <= EWC_FISHER_DEGENERATE_EPSILON {
            return Err(HealError::EwcFisherDegenerate {
                rank: self.fisher_rank(),
                dim: self.predictor_dim,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FisherUpdate {
    pub delta_norm: f32,
    pub current_rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EwcReadbackEvidence {
    pub boundary_step_trajectory: Vec<u64>,
    pub lambda_trajectory: Vec<f32>,
    pub fisher_rank_per_boundary: Vec<usize>,
    pub fisher_full_rank_after_boundary_10: bool,
    pub task_snapshots_count: usize,
    pub predictor_dim: usize,
}

pub trait PredictorGradAccessor {
    fn param_count(&self) -> usize;
    fn parameters_flat(&self) -> Vec<f32>;
    fn gradients_for(
        &self,
        panel: &Panel,
        oracle_outcome: &OracleOutcome,
    ) -> Result<Vec<f32>, HealError>;
    fn apply_gradient(&mut self, gradient: &[f32], learning_rate: f32) -> Result<(), HealError>;
}

pub trait HessianTraceProbe {
    fn hessian_trace_estimate(&self) -> Result<f32, HealError>;
}

pub trait SnapshotStore {
    fn replace_task_snapshots(&self, snapshots: &[TaskSnapshot]) -> Result<(), HealError>;
}

#[derive(Debug, Clone, Default)]
pub struct MemorySnapshotStore {
    pub snapshots: std::sync::Arc<std::sync::Mutex<Vec<TaskSnapshot>>>,
}

impl SnapshotStore for MemorySnapshotStore {
    fn replace_task_snapshots(&self, snapshots: &[TaskSnapshot]) -> Result<(), HealError> {
        *self.snapshots.lock().unwrap() = snapshots.to_vec();
        Ok(())
    }
}

fn validate_finite_vec(field: &str, values: &[f32]) -> Result<(), HealError> {
    if values.is_empty() {
        return Err(HealError::invalid(field, "vector must be non-empty"));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(HealError::BatchNan {
            component: field.to_string(),
            witness_chain_offset: 0,
        });
    }
    Ok(())
}

fn validate_nonnegative_finite(field: &str, value: f32) -> Result<(), HealError> {
    if !value.is_finite() || value < 0.0 {
        return Err(HealError::invalid(
            field,
            format!("value must be finite and non-negative, got {value}"),
        ));
    }
    Ok(())
}

fn validate_cell_id(cell_id: &str) -> Result<(), HealError> {
    if cell_id.trim().is_empty() {
        return Err(HealError::invalid(
            "ewc.cell_id",
            "cell id must be non-empty",
        ));
    }
    if cell_id.len() > 512 {
        return Err(HealError::invalid(
            "ewc.cell_id",
            "cell id exceeds 512 bytes",
        ));
    }
    if cell_id.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return Err(HealError::invalid(
            "ewc.cell_id",
            "cell id contains a control character",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_mejepa_instruments::{Panel, PANEL_DIM};

    struct TestPredictor {
        params: Vec<f32>,
        grad_nan: bool,
    }

    impl PredictorGradAccessor for TestPredictor {
        fn param_count(&self) -> usize {
            self.params.len()
        }

        fn parameters_flat(&self) -> Vec<f32> {
            self.params.clone()
        }

        fn gradients_for(
            &self,
            _panel: &Panel,
            _oracle_outcome: &OracleOutcome,
        ) -> Result<Vec<f32>, HealError> {
            if self.grad_nan {
                Ok(vec![f32::NAN; self.params.len()])
            } else {
                Ok((0..self.params.len())
                    .map(|idx| 0.1 + idx as f32 * 0.001)
                    .collect())
            }
        }

        fn apply_gradient(
            &mut self,
            gradient: &[f32],
            learning_rate: f32,
        ) -> Result<(), HealError> {
            for (p, g) in self.params.iter_mut().zip(gradient) {
                *p -= learning_rate * *g;
            }
            Ok(())
        }
    }

    impl HessianTraceProbe for TestPredictor {
        fn hessian_trace_estimate(&self) -> Result<f32, HealError> {
            Ok(1000.0)
        }
    }

    fn panel() -> Panel {
        Panel::try_new(vec![0.1; PANEL_DIM], (1u16 << 15) - 1).unwrap()
    }

    #[test]
    fn fisher_matrix_zero_constructor_full_zeros_and_rank_zero() {
        let fisher = FisherMatrix::try_new_zero(4).unwrap();
        assert_eq!(fisher.diagonal, vec![0.0; 4]);
        assert_eq!(fisher.rank, 0);
    }

    #[test]
    fn task_snapshot_rejects_length_mismatch() {
        assert!(TaskSnapshot::try_new(vec![1.0], vec![1.0, 2.0], 0, [0; 32], 0).is_err());
    }

    #[test]
    fn ewc_plus_plus_rejects_bad_lambda_or_dim() {
        assert!(EwcPlusPlus::try_new(0, DEFAULT_INITIAL_LAMBDA, DEFAULT_MAX_LAMBDA).is_err());
        assert!(EwcPlusPlus::try_new(4, 20.0, 10.0).is_err());
    }

    #[test]
    fn update_fisher_online_rejects_non_finite_grad() {
        let mut ewc = EwcPlusPlus::try_new(4, DEFAULT_INITIAL_LAMBDA, DEFAULT_MAX_LAMBDA).unwrap();
        let pred = TestPredictor {
            params: vec![0.0; 4],
            grad_nan: true,
        };
        assert_eq!(
            ewc.update_fisher_online(&panel(), &OracleOutcome::Pass, &pred)
                .unwrap_err()
                .code(),
            "MEJEPA_OBSERVE_BATCH_NAN"
        );
    }

    #[test]
    fn ten_boundaries_full_rank() {
        let mut ewc = EwcPlusPlus::try_new(16, DEFAULT_INITIAL_LAMBDA, DEFAULT_MAX_LAMBDA).unwrap();
        let pred = TestPredictor {
            params: vec![0.1; 16],
            grad_nan: false,
        };
        let store = MemorySnapshotStore::default();
        for idx in 0..10 {
            ewc.update_fisher_online(&panel(), &OracleOutcome::Pass, &pred)
                .unwrap();
            assert!(ewc.detect_task_boundary(10.0 + idx as f32, &pred).unwrap());
            ewc.snapshot_current_task(&pred.parameters_flat(), [idx as u8; 32], &store)
                .unwrap();
        }
        assert_eq!(ewc.fisher_rank(), 16);
        assert!(ewc.current_lambda() <= DEFAULT_MAX_LAMBDA);
        assert!(ewc.task_snapshots.len() <= PER_EMBEDDER_SNAPSHOT_CAP_BASE_PLUS_DELTAS);
    }
}
