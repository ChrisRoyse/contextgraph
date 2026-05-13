use super::error::{EvalError, EvalErrorCode};
use super::types::{
    validate_correlation, validate_optional_correlation, validate_optional_unit, validate_unit,
    EvalReport,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_CONVERGENCE_TARGET: f32 = 0.95;
pub const DEFAULT_CONVERGENCE_HISTORY_WINDOWS: usize = 8;
pub const DEFAULT_CONVERGENCE_MIN_POINTS: usize = 3;

const LINEAR_MODEL: &str = "linear_least_squares";
const CONFIDENCE_LEVEL: f32 = 0.95;
const CONFIDENCE_Z: f64 = 1.96;
const MAX_ESTIMATED_WINDOWS: usize = 10_000;

#[derive(Debug, Clone, Copy)]
struct LinearFitMetrics {
    slope: f64,
    intercept: f64,
    r_squared: f64,
    residual_std_error: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceEtaStatus {
    AlreadyPassing,
    TrendingToTarget,
    NotConverging,
    InsufficientHistory,
    InsufficientSamples,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConvergenceEtaConfidenceInterval {
    pub lower_window: usize,
    pub upper_window: usize,
    pub confidence_level: f32,
}

impl ConvergenceEtaConfidenceInterval {
    pub fn validate(&self, name: &str) -> Result<(), EvalError> {
        validate_unit(&format!("{name}.confidence_level"), self.confidence_level)?;
        if self.lower_window > self.upper_window {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "{name}.lower_window {} exceeds upper_window {}",
                    self.lower_window, self.upper_window
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellConvergenceEta {
    pub cell: String,
    pub model: String,
    pub target_correlation: f32,
    pub history_window_count: usize,
    pub valid_observation_count: usize,
    pub latest_correlation: Option<f32>,
    pub slope_per_window: Option<f32>,
    pub intercept: Option<f32>,
    pub r_squared: Option<f32>,
    pub residual_std_error: Option<f32>,
    pub estimated_passing_window: Option<usize>,
    pub confidence_interval: Option<ConvergenceEtaConfidenceInterval>,
    pub status: ConvergenceEtaStatus,
}

impl CellConvergenceEta {
    pub fn validate(&self, expected_cell: &str) -> Result<(), EvalError> {
        if self.cell != expected_cell {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "per_cell_convergence_eta key {expected_cell} does not match payload cell {}",
                    self.cell
                ),
            ));
        }
        if self.model.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("per_cell_convergence_eta.{expected_cell}.model must be non-empty"),
            ));
        }
        validate_correlation(
            &format!("per_cell_convergence_eta.{expected_cell}.target_correlation"),
            self.target_correlation,
        )?;
        validate_optional_correlation(
            &format!("per_cell_convergence_eta.{expected_cell}.latest_correlation"),
            self.latest_correlation,
        )?;
        validate_optional_finite(
            &format!("per_cell_convergence_eta.{expected_cell}.slope_per_window"),
            self.slope_per_window,
        )?;
        validate_optional_finite(
            &format!("per_cell_convergence_eta.{expected_cell}.intercept"),
            self.intercept,
        )?;
        validate_optional_unit(
            &format!("per_cell_convergence_eta.{expected_cell}.r_squared"),
            self.r_squared,
        )?;
        validate_optional_unit(
            &format!("per_cell_convergence_eta.{expected_cell}.residual_std_error"),
            self.residual_std_error,
        )?;
        if self.history_window_count == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("per_cell_convergence_eta.{expected_cell}.history_window_count is zero"),
            ));
        }
        if self.valid_observation_count > self.history_window_count {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "per_cell_convergence_eta.{expected_cell}.valid_observation_count exceeds history_window_count"
                ),
            ));
        }
        if let Some(interval) = &self.confidence_interval {
            interval.validate(&format!(
                "per_cell_convergence_eta.{expected_cell}.confidence_interval"
            ))?;
            if let Some(estimate) = self.estimated_passing_window {
                if estimate < interval.lower_window || estimate > interval.upper_window {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        format!(
                            "per_cell_convergence_eta.{expected_cell}.estimated_passing_window outside confidence interval"
                        ),
                    ));
                }
            }
        }
        match self.status {
            ConvergenceEtaStatus::AlreadyPassing | ConvergenceEtaStatus::TrendingToTarget => {
                if self.estimated_passing_window.is_none() || self.confidence_interval.is_none() {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        format!(
                            "per_cell_convergence_eta.{expected_cell}.{status:?} requires ETA and confidence interval",
                            status = self.status
                        ),
                    ));
                }
            }
            ConvergenceEtaStatus::NotConverging
            | ConvergenceEtaStatus::InsufficientHistory
            | ConvergenceEtaStatus::InsufficientSamples => {
                if self.estimated_passing_window.is_some() || self.confidence_interval.is_some() {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        format!(
                            "per_cell_convergence_eta.{expected_cell}.{status:?} cannot report ETA",
                            status = self.status
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

pub fn baseline_convergence_eta_for_cells(
    per_cell: &BTreeMap<String, Option<f32>>,
    target_correlation: f32,
) -> BTreeMap<String, CellConvergenceEta> {
    per_cell
        .iter()
        .map(|(cell, latest)| {
            (
                cell.clone(),
                baseline_eta(
                    cell,
                    *latest,
                    target_correlation,
                    1,
                    latest.is_some() as usize,
                ),
            )
        })
        .collect()
}

pub fn compute_convergence_eta_from_reports(
    reports: &[EvalReport],
    target_correlation: f32,
    max_history_windows: usize,
    min_points: usize,
) -> Result<BTreeMap<String, CellConvergenceEta>, EvalError> {
    validate_correlation("convergence_target_correlation", target_correlation)?;
    if max_history_windows == 0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidConfig,
            "max_history_windows must be greater than zero",
        ));
    }
    if min_points < 2 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidConfig,
            "min_points must be at least 2 for linear convergence ETA",
        ));
    }
    if reports.is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "convergence ETA requires at least one eval report",
        ));
    }

    let mut ordered = reports.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.generated_at_unix_ms
            .cmp(&right.generated_at_unix_ms)
            .then_with(|| left.report_date.cmp(&right.report_date))
    });
    let start = ordered.len().saturating_sub(max_history_windows);
    let windows = &ordered[start..];
    let latest = windows.last().expect("non-empty windows");
    let history_window_count = windows.len();

    let mut out = BTreeMap::new();
    for (cell, latest_correlation) in &latest.per_cell_correlation {
        let latest_correlation = *latest_correlation;
        let points = windows
            .iter()
            .enumerate()
            .filter_map(|(index, report)| {
                report
                    .per_cell_correlation
                    .get(cell)
                    .copied()
                    .flatten()
                    .map(|value| (index as f64, value as f64))
            })
            .collect::<Vec<_>>();
        let valid_observation_count = points.len();
        let eta = match latest_correlation {
            None => baseline_eta(
                cell,
                None,
                target_correlation,
                history_window_count,
                valid_observation_count,
            ),
            Some(value) if value >= target_correlation => already_passing_eta(
                cell,
                value,
                target_correlation,
                history_window_count,
                valid_observation_count,
            ),
            Some(value) if valid_observation_count < min_points => CellConvergenceEta {
                cell: cell.clone(),
                model: LINEAR_MODEL.to_string(),
                target_correlation,
                history_window_count,
                valid_observation_count,
                latest_correlation: Some(value),
                slope_per_window: None,
                intercept: None,
                r_squared: None,
                residual_std_error: None,
                estimated_passing_window: None,
                confidence_interval: None,
                status: ConvergenceEtaStatus::InsufficientHistory,
            },
            Some(value) => fit_eta(
                cell,
                value,
                target_correlation,
                history_window_count,
                valid_observation_count,
                &points,
            )?,
        };
        eta.validate(cell)?;
        out.insert(cell.clone(), eta);
    }
    Ok(out)
}

fn baseline_eta(
    cell: &str,
    latest_correlation: Option<f32>,
    target_correlation: f32,
    history_window_count: usize,
    valid_observation_count: usize,
) -> CellConvergenceEta {
    match latest_correlation {
        Some(value) if value >= target_correlation => already_passing_eta(
            cell,
            value,
            target_correlation,
            history_window_count,
            valid_observation_count,
        ),
        Some(value) => CellConvergenceEta {
            cell: cell.to_string(),
            model: LINEAR_MODEL.to_string(),
            target_correlation,
            history_window_count,
            valid_observation_count,
            latest_correlation: Some(value),
            slope_per_window: None,
            intercept: None,
            r_squared: None,
            residual_std_error: None,
            estimated_passing_window: None,
            confidence_interval: None,
            status: ConvergenceEtaStatus::InsufficientHistory,
        },
        None => CellConvergenceEta {
            cell: cell.to_string(),
            model: LINEAR_MODEL.to_string(),
            target_correlation,
            history_window_count,
            valid_observation_count,
            latest_correlation: None,
            slope_per_window: None,
            intercept: None,
            r_squared: None,
            residual_std_error: None,
            estimated_passing_window: None,
            confidence_interval: None,
            status: ConvergenceEtaStatus::InsufficientSamples,
        },
    }
}

fn already_passing_eta(
    cell: &str,
    latest_correlation: f32,
    target_correlation: f32,
    history_window_count: usize,
    valid_observation_count: usize,
) -> CellConvergenceEta {
    CellConvergenceEta {
        cell: cell.to_string(),
        model: LINEAR_MODEL.to_string(),
        target_correlation,
        history_window_count,
        valid_observation_count,
        latest_correlation: Some(latest_correlation),
        slope_per_window: None,
        intercept: None,
        r_squared: None,
        residual_std_error: None,
        estimated_passing_window: Some(0),
        confidence_interval: Some(ConvergenceEtaConfidenceInterval {
            lower_window: 0,
            upper_window: 0,
            confidence_level: CONFIDENCE_LEVEL,
        }),
        status: ConvergenceEtaStatus::AlreadyPassing,
    }
}

fn fit_eta(
    cell: &str,
    latest_correlation: f32,
    target_correlation: f32,
    history_window_count: usize,
    valid_observation_count: usize,
    points: &[(f64, f64)],
) -> Result<CellConvergenceEta, EvalError> {
    let n = points.len() as f64;
    let mean_x = points.iter().map(|(x, _)| *x).sum::<f64>() / n;
    let mean_y = points.iter().map(|(_, y)| *y).sum::<f64>() / n;
    let denom = points
        .iter()
        .map(|(x, _)| (*x - mean_x).powi(2))
        .sum::<f64>();
    if denom == 0.0 {
        return Ok(not_converging_eta(
            cell,
            latest_correlation,
            target_correlation,
            history_window_count,
            valid_observation_count,
            LinearFitMetrics {
                slope: 0.0,
                intercept: mean_y,
                r_squared: 0.0,
                residual_std_error: 0.0,
            },
        ));
    }
    let slope = points
        .iter()
        .map(|(x, y)| (*x - mean_x) * (*y - mean_y))
        .sum::<f64>()
        / denom;
    let intercept = mean_y - slope * mean_x;
    let residuals = points
        .iter()
        .map(|(x, y)| {
            let predicted = intercept + slope * *x;
            (*y - predicted).powi(2)
        })
        .collect::<Vec<_>>();
    let sse = residuals.iter().sum::<f64>();
    let sst = points
        .iter()
        .map(|(_, y)| (*y - mean_y).powi(2))
        .sum::<f64>();
    let r_squared = if sst <= f64::EPSILON {
        if sse <= f64::EPSILON {
            1.0
        } else {
            0.0
        }
    } else {
        (1.0 - (sse / sst)).clamp(0.0, 1.0)
    };
    let residual_std_error = (sse / (n - 2.0).max(1.0)).sqrt().clamp(0.0, 1.0);
    if slope <= 0.0 {
        return Ok(not_converging_eta(
            cell,
            latest_correlation,
            target_correlation,
            history_window_count,
            valid_observation_count,
            LinearFitMetrics {
                slope,
                intercept,
                r_squared,
                residual_std_error,
            },
        ));
    }

    let gap = (target_correlation as f64 - latest_correlation as f64).max(0.0);
    let estimated = ceil_windows(gap / slope).max(1);
    let ci_band = CONFIDENCE_Z * residual_std_error;
    let lower = ceil_windows(((gap - ci_band).max(0.0)) / slope);
    let upper = ceil_windows((gap + ci_band) / slope).max(estimated);
    Ok(CellConvergenceEta {
        cell: cell.to_string(),
        model: LINEAR_MODEL.to_string(),
        target_correlation,
        history_window_count,
        valid_observation_count,
        latest_correlation: Some(latest_correlation),
        slope_per_window: Some(slope as f32),
        intercept: Some(intercept as f32),
        r_squared: Some(r_squared as f32),
        residual_std_error: Some(residual_std_error as f32),
        estimated_passing_window: Some(estimated),
        confidence_interval: Some(ConvergenceEtaConfidenceInterval {
            lower_window: lower,
            upper_window: upper,
            confidence_level: CONFIDENCE_LEVEL,
        }),
        status: ConvergenceEtaStatus::TrendingToTarget,
    })
}

fn not_converging_eta(
    cell: &str,
    latest_correlation: f32,
    target_correlation: f32,
    history_window_count: usize,
    valid_observation_count: usize,
    metrics: LinearFitMetrics,
) -> CellConvergenceEta {
    CellConvergenceEta {
        cell: cell.to_string(),
        model: LINEAR_MODEL.to_string(),
        target_correlation,
        history_window_count,
        valid_observation_count,
        latest_correlation: Some(latest_correlation),
        slope_per_window: Some(metrics.slope as f32),
        intercept: Some(metrics.intercept as f32),
        r_squared: Some(metrics.r_squared as f32),
        residual_std_error: Some(metrics.residual_std_error as f32),
        estimated_passing_window: None,
        confidence_interval: None,
        status: ConvergenceEtaStatus::NotConverging,
    }
}

fn ceil_windows(value: f64) -> usize {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    value.ceil().min(MAX_ESTIMATED_WINDOWS as f64) as usize
}

fn validate_optional_finite(name: &str, value: Option<f32>) -> Result<(), EvalError> {
    if let Some(value) = value {
        if !value.is_finite() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("{name} must be finite; got {value}"),
            ));
        }
    }
    Ok(())
}
