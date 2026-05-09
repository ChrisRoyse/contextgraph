// Inspired by ruvnet/RuVector crates/ruvector-core/src/advanced_features/conformal_prediction.rs
// at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.
//
// Algorithm reference:
//   Vovk, Gammerman & Shafer, "Algorithmic Learning in a Random World" (2005).
//   Inductive (split) conformal prediction: a calibration-set approach to
//   distribution-free prediction sets with finite-sample coverage guarantees.
//
//   Given a calibration sample of size n with non-conformity scores
//   { s_1, ..., s_n }, the prediction set at miscoverage α is
//     C(x) = { y : nonconformity(x, y) <= q_{ceil((1-α)(n+1))} }
//   where q_k is the k-th smallest score. Coverage:
//     P( y_test ∈ C(x_test) ) >= 1 - α
//   under exchangeability of (x_calib, y_calib) and (x_test, y_test).

use crate::error::{SolverError, SolverResult};
use serde::{Deserialize, Serialize};

/// Choice of non-conformity measure used to score `(query, candidate)` pairs
/// during calibration AND inference. The same measure MUST be used in both
/// phases — switching breaks the exchangeability assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NonconformityMeasure {
    /// Use the raw distance as the non-conformity score.
    /// Higher distance = more non-conforming (further from the truth).
    Distance,
    /// Use the candidate's 0-based rank in the descending-relevance ordering
    /// for the query. Rank 0 is most conforming; larger ranks are less
    /// conforming.
    Rank,
    /// Use `distance / (median_calibration_distance + epsilon)`. Robust to
    /// different distance scales across queries; recommended when calibration
    /// queries have heterogeneous distance distributions.
    NormalizedDistance,
}

/// Configuration for the split conformal predictor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConformalConfig {
    /// Target miscoverage rate. With α=0.1 the empirical coverage on
    /// calibration-distributed test points should be at least 90%.
    pub alpha: f64,
    /// Choice of nonconformity measure used in calibration AND inference.
    pub measure: NonconformityMeasure,
}

impl Default for ConformalConfig {
    fn default() -> Self {
        Self {
            alpha: 0.1,
            measure: NonconformityMeasure::Distance,
        }
    }
}

/// Result of the calibration step. Holds the threshold used to decide
/// inclusion at inference time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConformalReport {
    /// Number of calibration scores used to compute the threshold.
    pub calibration_size: usize,
    /// The non-conformity threshold τ. A candidate is in the prediction
    /// set iff `nonconformity(query, candidate) <= threshold`.
    pub threshold: f64,
    /// Empirical 1-α quantile rank used (1-based).
    pub quantile_rank: usize,
    /// Stated coverage `1 - α`.
    pub stated_coverage: f64,
}

/// Output of one inference call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionSet {
    /// 0-based indices into the candidate slice that make the cut.
    pub indices: Vec<usize>,
    /// The non-conformity scores of the included candidates, in the same
    /// order as `indices`. Useful for "which candidate was strongest?"
    /// downstream.
    pub scores: Vec<f64>,
    /// The threshold τ at the time of prediction (mirrors `report.threshold`).
    pub threshold: f64,
}

/// Inductive (split) conformal predictor. Calibration writes the threshold;
/// inference uses it. Calibration is required before any inference — the
/// predictor refuses to make uncalibrated predictions (fail-closed).
#[derive(Debug, Clone)]
pub struct ConformalPredictor {
    config: ConformalConfig,
    threshold: Option<f64>,
    calibration_size: Option<usize>,
    quantile_rank: Option<usize>,
    median_calibration_distance: Option<f64>,
}

impl ConformalPredictor {
    /// Create a new predictor with the given config. Validates α ∈ (0, 1).
    pub fn new(config: ConformalConfig) -> SolverResult<Self> {
        if !config.alpha.is_finite() || !(0.0..1.0).contains(&config.alpha) || config.alpha == 0.0 {
            return Err(SolverError::invalid(
                "alpha",
                format!(
                    "conformal alpha must be a finite value in (0.0, 1.0); got {}",
                    config.alpha
                ),
                "use a positive miscoverage rate strictly less than 1",
            ));
        }
        Ok(Self {
            config,
            threshold: None,
            calibration_size: None,
            quantile_rank: None,
            median_calibration_distance: None,
        })
    }

    /// Return the configuration used to construct this predictor.
    pub fn config(&self) -> ConformalConfig {
        self.config
    }

    /// Return `true` after a successful calibration call. Predictions are
    /// fail-closed before this returns `true`.
    pub fn is_calibrated(&self) -> bool {
        self.threshold.is_some()
    }

    /// Calibrate from raw distances (one per held-out calibration example).
    /// `Distance` and `NormalizedDistance` measures both consume distances;
    /// `Rank` requires calibration via `calibrate_with_ranks`.
    pub fn calibrate_distances(&mut self, distances: &[f64]) -> SolverResult<ConformalReport> {
        if distances.is_empty() {
            return Err(SolverError::invalid(
                "calibration",
                "calibration set must contain at least one score",
                "supply distances from a held-out calibration sample",
            ));
        }
        for (i, d) in distances.iter().enumerate() {
            if !d.is_finite() || *d < 0.0 {
                return Err(SolverError::invalid(
                    "calibration",
                    format!("distance at index {i} = {d} is not a non-negative finite value"),
                    "filter or clip non-finite or negative distances",
                ));
            }
        }
        let median = median_of(distances);
        let scores: Vec<f64> = match self.config.measure {
            NonconformityMeasure::Distance => distances.to_vec(),
            NonconformityMeasure::NormalizedDistance => distances
                .iter()
                .map(|d| d / (median + f64::EPSILON))
                .collect(),
            NonconformityMeasure::Rank => {
                return Err(SolverError::invalid(
                    "calibration",
                    "Rank measure requires calibration via calibrate_with_ranks",
                    "call calibrate_with_ranks(ranks) instead of calibrate_distances",
                ))
            }
        };
        self.median_calibration_distance = Some(median);
        self.finalize_calibration(scores)
    }

    /// Calibrate from per-example ranks (0-based, ascending = better).
    /// Required for `Rank` measure.
    pub fn calibrate_with_ranks(&mut self, ranks: &[usize]) -> SolverResult<ConformalReport> {
        if !matches!(self.config.measure, NonconformityMeasure::Rank) {
            return Err(SolverError::invalid(
                "calibration",
                format!(
                    "calibrate_with_ranks is only valid for Rank measure; got {:?}",
                    self.config.measure
                ),
                "call calibrate_distances or change the measure",
            ));
        }
        if ranks.is_empty() {
            return Err(SolverError::invalid(
                "calibration",
                "calibration set must contain at least one rank",
                "supply ranks from a held-out calibration sample",
            ));
        }
        let scores: Vec<f64> = ranks.iter().map(|rank| *rank as f64).collect();
        self.median_calibration_distance = None;
        self.finalize_calibration(scores)
    }

    fn finalize_calibration(&mut self, mut scores: Vec<f64>) -> SolverResult<ConformalReport> {
        scores.sort_by(|a, b| {
            a.partial_cmp(b)
                .expect("calibration scores must be comparable")
        });
        let n = scores.len();
        // 1-based quantile rank under finite-sample correction:
        //   k = ceil((n + 1) * (1 - alpha))
        // Clamp to [1, n].
        let one_minus_alpha = 1.0 - self.config.alpha;
        let raw = ((n as f64 + 1.0) * one_minus_alpha).ceil() as i64;
        let k = raw.clamp(1, n as i64) as usize;
        let threshold = scores[k - 1];
        self.threshold = Some(threshold);
        self.calibration_size = Some(n);
        self.quantile_rank = Some(k);
        Ok(ConformalReport {
            calibration_size: n,
            threshold,
            quantile_rank: k,
            stated_coverage: one_minus_alpha,
        })
    }

    /// Return the prediction set at the calibrated threshold for one query.
    /// `distances[i]` is the distance from the query to candidate `i`. Length
    /// must match `relevances` if `Rank` is used (relevances are used
    /// to derive rank order); for `Distance` and `NormalizedDistance`,
    /// `relevances` is ignored (pass `None`).
    pub fn predict_set(
        &self,
        distances: &[f64],
        relevances: Option<&[f64]>,
    ) -> SolverResult<PredictionSet> {
        let threshold = self.threshold.ok_or_else(|| {
            SolverError::invalid(
                "predict_set",
                "predictor not calibrated; call calibrate_distances or calibrate_with_ranks first",
                "calibrate before predicting (fail-closed)",
            )
        })?;
        for (i, d) in distances.iter().enumerate() {
            if !d.is_finite() || *d < 0.0 {
                return Err(SolverError::invalid(
                    "predict_set",
                    format!("distance at index {i} = {d} is not a non-negative finite value"),
                    "filter or clip non-finite or negative distances",
                ));
            }
        }

        let scores: Vec<f64> = match self.config.measure {
            NonconformityMeasure::Distance => distances.to_vec(),
            NonconformityMeasure::NormalizedDistance => {
                let median = self
                    .median_calibration_distance
                    .expect("median set during calibration");
                distances
                    .iter()
                    .map(|d| d / (median + f64::EPSILON))
                    .collect()
            }
            NonconformityMeasure::Rank => {
                let relevances = relevances.ok_or_else(|| {
                    SolverError::invalid(
                        "predict_set",
                        "Rank measure requires relevances at inference time",
                        "pass Some(&relevances) of the same length as distances",
                    )
                })?;
                if relevances.len() != distances.len() {
                    return Err(SolverError::invalid(
                        "predict_set",
                        format!(
                            "relevances len {} != distances len {}",
                            relevances.len(),
                            distances.len()
                        ),
                        "pass parallel slices of the same length",
                    ));
                }
                rank_scores(relevances)?
            }
        };

        let mut indices = Vec::new();
        let mut included_scores = Vec::new();
        for (i, score) in scores.iter().enumerate() {
            if *score <= threshold {
                indices.push(i);
                included_scores.push(*score);
            }
        }
        Ok(PredictionSet {
            indices,
            scores: included_scores,
            threshold,
        })
    }

    /// Empirical-coverage diagnostic: given a set of (distance-to-truth,
    /// per-example-distances) pairs, return what fraction of the time the
    /// truth ends up in the prediction set. Useful for FSV: empirical
    /// coverage should be within ±2% of stated coverage on a held-out sample.
    pub fn empirical_coverage(&self, truth_distances: &[f64]) -> SolverResult<f64> {
        let threshold = self.threshold.ok_or_else(|| {
            SolverError::invalid(
                "calibration",
                "predictor not calibrated; call calibrate_distances or calibrate_with_ranks first",
                "calibrate before measuring coverage",
            )
        })?;
        // Reject Rank measure up front: empirical_coverage consumes raw
        // truth distances and has no per-example rank to score against.
        // Lifting this check above the iteration makes the function shape
        // "validate, then iterate" instead of "iterate, validate per element."
        if matches!(self.config.measure, NonconformityMeasure::Rank) {
            return Err(SolverError::invalid(
                "measure",
                "Rank coverage requires per-example ranks; not supported here",
                "use Distance or NormalizedDistance for coverage diagnostics",
            ));
        }
        if truth_distances.is_empty() {
            return Err(SolverError::invalid(
                "truth_distances",
                "need at least one example to measure coverage",
                "supply at least one truth-distance sample",
            ));
        }
        let median = self.median_calibration_distance;
        let mut covered = 0usize;
        for d in truth_distances {
            if !d.is_finite() || *d < 0.0 {
                return Err(SolverError::invalid(
                    "truth_distances",
                    format!("truth distance {d} is not a non-negative finite value"),
                    "filter or clip non-finite or negative distances",
                ));
            }
            let score = match self.config.measure {
                NonconformityMeasure::Distance => *d,
                NonconformityMeasure::NormalizedDistance => {
                    let m = median.expect("median set during calibration");
                    d / (m + f64::EPSILON)
                }
                NonconformityMeasure::Rank => unreachable!("Rank rejected above"),
            };
            if score <= threshold {
                covered += 1;
            }
        }
        Ok(covered as f64 / truth_distances.len() as f64)
    }
}

fn rank_scores(relevances: &[f64]) -> SolverResult<Vec<f64>> {
    for (idx, score) in relevances.iter().enumerate() {
        if !score.is_finite() {
            return Err(SolverError::invalid(
                "relevances",
                format!("rank relevances must be finite (index {idx} = {score})"),
                "filter out NaN/Inf candidates before rank-based conformal prediction",
            ));
        }
    }
    // Sort indices by descending relevance for rank assignment; produce
    // 0-based ranks as nonconformity scores. Rank 0 is best / most
    // conforming, and larger ranks are progressively less conforming.
    let mut order: Vec<usize> = (0..relevances.len()).collect();
    order.sort_by(|a, b| {
        relevances[*b]
            .partial_cmp(&relevances[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ranks = vec![0usize; relevances.len()];
    for (rank, idx) in order.into_iter().enumerate() {
        ranks[idx] = rank;
    }
    Ok(ranks.into_iter().map(|rank| rank as f64).collect())
}

fn median_of(values: &[f64]) -> f64 {
    debug_assert!(!values.is_empty(), "median_of called with empty slice");
    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        0.5 * (sorted[n / 2 - 1] + sorted[n / 2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_alpha_zero() {
        let err = ConformalPredictor::new(ConformalConfig {
            alpha: 0.0,
            measure: NonconformityMeasure::Distance,
        })
        .expect_err("alpha=0 must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn rejects_alpha_one() {
        let err = ConformalPredictor::new(ConformalConfig {
            alpha: 1.0,
            measure: NonconformityMeasure::Distance,
        })
        .expect_err("alpha=1 must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn fail_closed_without_calibration() {
        let predictor = ConformalPredictor::new(ConformalConfig::default()).unwrap();
        let err = predictor
            .predict_set(&[0.1, 0.5, 0.9], None)
            .expect_err("uncalibrated predict_set must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn distance_threshold_matches_quantile() {
        // 10 calibration distances with α=0.1 means k = ceil(11 * 0.9) = 10.
        // The 10-th order statistic is the maximum: 1.0.
        let mut predictor = ConformalPredictor::new(ConformalConfig {
            alpha: 0.1,
            measure: NonconformityMeasure::Distance,
        })
        .unwrap();
        let calib = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let report = predictor.calibrate_distances(&calib).unwrap();
        assert_eq!(report.calibration_size, 10);
        assert_eq!(report.quantile_rank, 10);
        assert_eq!(report.threshold, 1.0);
        assert!((report.stated_coverage - 0.9).abs() < 1e-12);
    }

    #[test]
    fn distance_predict_set_includes_only_below_threshold() {
        let mut predictor = ConformalPredictor::new(ConformalConfig {
            alpha: 0.2,
            measure: NonconformityMeasure::Distance,
        })
        .unwrap();
        // Calibration of size 4. k = ceil(5 * 0.8) = 4 → threshold = max = 0.8.
        predictor
            .calibrate_distances(&[0.2, 0.4, 0.6, 0.8])
            .unwrap();
        let set = predictor
            .predict_set(&[0.1, 0.5, 0.85, 0.79, 1.5], None)
            .unwrap();
        assert_eq!(set.indices, vec![0, 1, 3]);
        assert_eq!(set.scores, vec![0.1, 0.5, 0.79]);
        assert_eq!(set.threshold, 0.8);
    }

    #[test]
    fn normalized_distance_handles_heterogeneous_scales() {
        let mut predictor = ConformalPredictor::new(ConformalConfig {
            alpha: 0.5,
            measure: NonconformityMeasure::NormalizedDistance,
        })
        .unwrap();
        let calib = vec![1.0, 2.0, 4.0, 8.0]; // median = 3.0
        let report = predictor.calibrate_distances(&calib).unwrap();
        // alpha=0.5, k = ceil(5 * 0.5) = 3; sorted normalized scores are
        // 1/3, 2/3, 4/3, 8/3 → 3rd order statistic ≈ 4/3.
        assert!((report.threshold - 4.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn rank_calibration_path() {
        let mut predictor = ConformalPredictor::new(ConformalConfig {
            alpha: 0.5,
            measure: NonconformityMeasure::Rank,
        })
        .unwrap();
        // Calibration ranks are already nonconformity scores: lower rank is
        // better. Sorted ascending: 0, 1, 4, 9.
        // alpha=0.5, k = ceil(5 * 0.5) = 3; threshold = 4.
        let report = predictor.calibrate_with_ranks(&[0, 1, 4, 9]).unwrap();
        assert!((report.threshold - 4.0).abs() < 1e-9);
    }

    #[test]
    fn rank_predict_uses_relevances_with_correct_polarity() {
        let mut predictor = ConformalPredictor::new(ConformalConfig {
            alpha: 0.5,
            measure: NonconformityMeasure::Rank,
        })
        .unwrap();
        predictor.calibrate_with_ranks(&[0, 1, 2, 3]).unwrap();
        // 4 candidates with these relevances → ranks 2,0,3,1 →
        // scores 2,0,3,1.
        let relevances = vec![0.3, 0.9, 0.1, 0.5];
        let set = predictor
            .predict_set(&[0.0, 0.0, 0.0, 0.0], Some(&relevances))
            .unwrap();
        // alpha=0.5, k = ceil(5 * 0.5) = 3; threshold = 2.
        // Include the three strongest candidates and exclude only the weakest.
        assert_eq!(set.indices, vec![0, 1, 3]);
        assert!(
            set.indices.contains(&1),
            "top-ranked candidate must be included"
        );
    }

    #[test]
    fn empirical_coverage_close_to_stated() {
        // Synthetic exchangeable sample: calibration and test both drawn i.i.d.
        // from Uniform[0,1]. Coverage at α=0.1 should hover near 0.9.
        let mut predictor = ConformalPredictor::new(ConformalConfig {
            alpha: 0.1,
            measure: NonconformityMeasure::Distance,
        })
        .unwrap();
        let calib: Vec<f64> = (0..200)
            .map(|i| ((i as f64 * 1234.5).sin().abs()).fract())
            .collect();
        predictor.calibrate_distances(&calib).unwrap();
        let test: Vec<f64> = (0..400)
            .map(|i| (((i + 200) as f64 * 1234.5).sin().abs()).fract())
            .collect();
        let coverage = predictor.empirical_coverage(&test).unwrap();
        // Generous tolerance (±0.05) — the sample is small but the
        // distribution-free guarantee holds in expectation.
        assert!(
            (coverage - 0.9).abs() < 0.05,
            "empirical_coverage={coverage} not within ±0.05 of stated 0.9"
        );
    }

    #[test]
    fn rejects_negative_distance() {
        let mut predictor = ConformalPredictor::new(ConformalConfig::default()).unwrap();
        let err = predictor
            .calibrate_distances(&[0.1, -0.2])
            .expect_err("negative distance must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn rejects_nan_distance_in_predict() {
        let mut predictor = ConformalPredictor::new(ConformalConfig::default()).unwrap();
        predictor.calibrate_distances(&[0.1, 0.2, 0.3]).unwrap();
        let err = predictor
            .predict_set(&[0.1, f64::NAN], None)
            .expect_err("NaN must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn empty_calibration_rejected() {
        let mut predictor = ConformalPredictor::new(ConformalConfig::default()).unwrap();
        let err = predictor
            .calibrate_distances(&[])
            .expect_err("empty calibration must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }
}
