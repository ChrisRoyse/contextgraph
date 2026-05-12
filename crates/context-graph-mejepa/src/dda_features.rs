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
        mean_per_embedder_cosine_unit: cosine_mean_to_unit(per_embedder_sum, per_embedder_n),
        mean_pairwise_cosine_unit: cosine_mean_to_unit(pairwise_sum, pairwise_n),
        pairwise_mi_health: mi_mean_to_health(mi_sum, mi_n)?,
        blind_spot_health: 1.0 / (1.0 + max_abs_z),
    };
    projection.validate()?;
    Ok(projection)
}

fn cosine_mean_to_unit(sum: f64, n: usize) -> f32 {
    if n == 0 {
        0.5
    } else {
        (((sum / n as f64) + 1.0) / 2.0).clamp(0.0, 1.0) as f32
    }
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
