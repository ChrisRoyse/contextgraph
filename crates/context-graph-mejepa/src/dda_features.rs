use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::MejepaInferError;
use crate::types::{validate_probability, ChunkId, DdaSignals};

pub const DDA_FEATURE_PROJECTION_SCHEMA: &str = "dda-feature-projection-v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DdaFeatureProjection {
    pub schema: String,
    pub row_count: usize,
    pub embedder_count: usize,
    pub pairwise_count_per_row: usize,
    pub feature_scalar_count: usize,
    pub mean_per_embedder_cosine_unit: f32,
    pub mean_pairwise_cosine_unit: f32,
    pub pairwise_mi_health: f32,
    pub blind_spot_health: f32,
}

impl DdaFeatureProjection {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema != DDA_FEATURE_PROJECTION_SCHEMA {
            return Err(MejepaInferError::InvalidInput {
                field: "dda_feature_projection.schema".to_string(),
                detail: format!(
                    "expected {DDA_FEATURE_PROJECTION_SCHEMA}, got {}",
                    self.schema
                ),
            });
        }
        if self.row_count == 0 {
            return Err(MejepaInferError::InvalidInput {
                field: "dda_feature_projection.row_count".to_string(),
                detail: "DDA feature projection requires at least one chunk row".to_string(),
            });
        }
        if self.embedder_count == 0 {
            return Err(MejepaInferError::DimMismatch {
                expected: 1,
                actual: 0,
                context: "DDA feature projection requires at least one embedder".to_string(),
            });
        }
        validate_probability(
            "dda_feature_projection.mean_per_embedder_cosine_unit",
            self.mean_per_embedder_cosine_unit,
        )?;
        validate_probability(
            "dda_feature_projection.mean_pairwise_cosine_unit",
            self.mean_pairwise_cosine_unit,
        )?;
        validate_probability(
            "dda_feature_projection.pairwise_mi_health",
            self.pairwise_mi_health,
        )?;
        validate_probability(
            "dda_feature_projection.blind_spot_health",
            self.blind_spot_health,
        )?;
        Ok(())
    }

    pub fn to_granger_attestations(&self) -> BTreeMap<String, f32> {
        BTreeMap::from([
            (
                format!(
                    "dda:{DDA_FEATURE_PROJECTION_SCHEMA}:rows={}:embedders={}:pairwise={}:shape_valid",
                    self.row_count, self.embedder_count, self.pairwise_count_per_row
                ),
                1.0,
            ),
            (
                format!("dda:{DDA_FEATURE_PROJECTION_SCHEMA}:row_coverage"),
                1.0,
            ),
            (
                format!("dda:{DDA_FEATURE_PROJECTION_SCHEMA}:per_embedder_cosine_mean"),
                self.mean_per_embedder_cosine_unit,
            ),
            (
                format!("dda:{DDA_FEATURE_PROJECTION_SCHEMA}:pairwise_cosine_mean"),
                self.mean_pairwise_cosine_unit,
            ),
            (
                format!("dda:{DDA_FEATURE_PROJECTION_SCHEMA}:pairwise_mi_health"),
                self.pairwise_mi_health,
            ),
            (
                format!("dda:{DDA_FEATURE_PROJECTION_SCHEMA}:blind_spot_health"),
                self.blind_spot_health,
            ),
        ])
    }
}

pub fn project_dda_features(
    rows: &[(ChunkId, DdaSignals)],
    expected_embedder_count: usize,
) -> Result<DdaFeatureProjection, MejepaInferError> {
    if rows.is_empty() {
        return Err(MejepaInferError::InvalidInput {
            field: "dda_rows".to_string(),
            detail: "prediction has no covered chunk DDA rows".to_string(),
        });
    }

    let mut row_count = 0usize;
    let mut embedder_count = None::<usize>;
    let mut pairwise_count_per_row = None::<usize>;
    let mut feature_scalar_count = 0usize;
    let mut per_embedder_sum = 0.0f64;
    let mut per_embedder_n = 0usize;
    let mut pairwise_sum = 0.0f64;
    let mut pairwise_n = 0usize;
    let mut mi_sum = 0.0f64;
    let mut mi_n = 0usize;
    let mut max_abs_z = 0.0f32;

    for (chunk_idx, (chunk_id, signals)) in rows.iter().enumerate() {
        chunk_id.validate(&format!("dda_rows[{chunk_idx}].chunk_id"))?;
        signals.validate()?;
        let n = signals.embedder_count();
        if n == 0 {
            return Err(MejepaInferError::DimMismatch {
                expected: 1,
                actual: 0,
                context: format!("DDA row {} has zero embedders", chunk_id.0),
            });
        }
        if expected_embedder_count > 0 && n != expected_embedder_count {
            return Err(MejepaInferError::DimMismatch {
                expected: expected_embedder_count,
                actual: n,
                context: format!(
                    "DDA row {} embedder_count does not match inference config",
                    chunk_id.0
                ),
            });
        }
        match embedder_count {
            Some(expected) if expected != n => {
                return Err(MejepaInferError::DimMismatch {
                    expected,
                    actual: n,
                    context: format!(
                        "DDA row {} embedder_count differs from first covered chunk",
                        chunk_id.0
                    ),
                });
            }
            None => embedder_count = Some(n),
            _ => {}
        }

        let expected_pairwise = n * (n - 1) / 2;
        if signals.pairwise_mi_upper.len() != expected_pairwise {
            return Err(MejepaInferError::DimMismatch {
                expected: expected_pairwise,
                actual: signals.pairwise_mi_upper.len(),
                context: format!(
                    "DDA row {} pairwise_mi_upper must be fully materialized for prediction consumption",
                    chunk_id.0
                ),
            });
        }
        match pairwise_count_per_row {
            Some(expected) if expected != expected_pairwise => {
                return Err(MejepaInferError::DimMismatch {
                    expected,
                    actual: expected_pairwise,
                    context: format!(
                        "DDA row {} pairwise width differs from first covered chunk",
                        chunk_id.0
                    ),
                });
            }
            None => pairwise_count_per_row = Some(expected_pairwise),
            _ => {}
        }

        row_count += 1;
        feature_scalar_count += signals.per_embedder_cosine.len()
            + signals.pairwise_cosine_upper.len()
            + signals.pairwise_mi_upper.len()
            + signals.blind_spot_z_scores.len();
        for value in &signals.per_embedder_cosine {
            per_embedder_sum += f64::from(*value);
            per_embedder_n += 1;
        }
        for value in &signals.pairwise_cosine_upper {
            pairwise_sum += f64::from(*value);
            pairwise_n += 1;
        }
        for value in &signals.pairwise_mi_upper {
            mi_sum += f64::from(*value);
            mi_n += 1;
        }
        for value in &signals.blind_spot_z_scores {
            max_abs_z = max_abs_z.max(value.abs());
        }
    }

    let projection = DdaFeatureProjection {
        schema: DDA_FEATURE_PROJECTION_SCHEMA.to_string(),
        row_count,
        embedder_count: embedder_count.unwrap_or(0),
        pairwise_count_per_row: pairwise_count_per_row.unwrap_or(0),
        feature_scalar_count,
        mean_per_embedder_cosine_unit: cosine_mean_to_unit(
            per_embedder_sum,
            per_embedder_n,
            "dda_features.mean_per_embedder_cosine_unit",
        )?,
        mean_pairwise_cosine_unit: cosine_mean_to_unit(
            pairwise_sum,
            pairwise_n,
            "dda_features.mean_pairwise_cosine_unit",
        )?,
        pairwise_mi_health: mi_mean_to_health(mi_sum, mi_n)?,
        blind_spot_health: 1.0 / (1.0 + max_abs_z),
    };
    projection.validate()?;
    Ok(projection)
}

/// Converts a sum of cosines (range `[-n, n]`) into a unit-interval mean in `[0, 1]`.
///
/// Returns `Err(MejepaInferError::CosineMeanUndefinedNoSamples)` when `n == 0`.
/// The original F-024 implementation silently substituted `0.5` ("neutral cosine") for
/// the empty-sample case, which conflated "vectors are on average orthogonal" with
/// "no input was observed." A DDA projection with zero per-embedder cosines or zero
/// pairwise cosines is an upstream invariant violation that must surface to the operator.
fn cosine_mean_to_unit(sum: f64, n: usize, context: &str) -> Result<f32, MejepaInferError> {
    if n == 0 {
        return Err(MejepaInferError::CosineMeanUndefinedNoSamples {
            context: context.to_string(),
        });
    }
    Ok((((sum / n as f64) + 1.0) / 2.0).clamp(0.0, 1.0) as f32)
}

fn mi_mean_to_health(sum: f64, n: usize) -> Result<f32, MejepaInferError> {
    if n == 0 {
        return Err(MejepaInferError::DimMismatch {
            expected: 1,
            actual: 0,
            context: "DDA feature projection requires pairwise MI values".to_string(),
        });
    }
    let mean = sum / n as f64;
    if !mean.is_finite() || mean < 0.0 {
        return Err(MejepaInferError::NanDetected {
            nan_source: "dda_feature_projection.pairwise_mi_upper".to_string(),
            detail: format!("pairwise MI mean must be finite and non-negative; got {mean}"),
        });
    }
    Ok((1.0 / (1.0 + mean)) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// F-024 regression: happy path on a non-degenerate sample maps `mean(cos)` into `[0, 1]`.
    #[test]
    fn cosine_mean_to_unit_maps_positive_mean_into_unit_interval() {
        // mean cos = 0.5 → unit = (0.5 + 1) / 2 = 0.75
        let v = cosine_mean_to_unit(1.0, 2, "dda_features.happy_path").expect("ok");
        assert!((v - 0.75).abs() < 1e-6, "expected ~0.75, got {v}");
    }

    /// F-024 regression: a perfectly anti-correlated sample maps to 0.0.
    #[test]
    fn cosine_mean_to_unit_maps_minus_one_to_zero() {
        let v = cosine_mean_to_unit(-1.0, 1, "dda_features.anti_correlated").expect("ok");
        assert!((v - 0.0).abs() < 1e-6, "expected 0.0, got {v}");
    }

    /// F-024 regression: a perfectly correlated sample maps to 1.0.
    #[test]
    fn cosine_mean_to_unit_maps_plus_one_to_one() {
        let v = cosine_mean_to_unit(1.0, 1, "dda_features.correlated").expect("ok");
        assert!((v - 1.0).abs() < 1e-6, "expected 1.0, got {v}");
    }

    /// F-024 regression: zero-sample input must fail closed, not silently return 0.5.
    /// Before the fix, `cosine_mean_to_unit(0.0, 0)` returned 0.5, which downstream consumed
    /// as "neutral cosine" — indistinguishable from "vectors are on average orthogonal."
    #[test]
    fn cosine_mean_to_unit_zero_samples_fails_closed() {
        let err = cosine_mean_to_unit(0.0, 0, "dda_features.empty_sample")
            .expect_err("zero samples must fail closed");
        assert_eq!(err.code(), "MEJEPA_INFER_COSINE_MEAN_UNDEFINED_NO_SAMPLES");
        match err {
            MejepaInferError::CosineMeanUndefinedNoSamples { context } => {
                assert_eq!(context, "dda_features.empty_sample");
            }
            other => panic!("expected CosineMeanUndefinedNoSamples, got: {other:?}"),
        }
    }

    /// F-024 regression: even a nonzero sum is still undefined when n=0
    /// (would otherwise NaN out via 0/0; but we caught it via the explicit Err).
    #[test]
    fn cosine_mean_to_unit_nonzero_sum_zero_n_fails_closed() {
        let err = cosine_mean_to_unit(3.7, 0, "dda_features.malformed")
            .expect_err("zero samples must fail closed regardless of sum");
        assert_eq!(err.code(), "MEJEPA_INFER_COSINE_MEAN_UNDEFINED_NO_SAMPLES");
    }
}
