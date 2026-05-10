use std::collections::BTreeMap;

use context_graph_mejepa_instruments::Panel;
use serde::{Deserialize, Serialize};

use crate::constellation::{TctConstellation, Thresholds};
use crate::error::TctError;
use crate::gtau::cosine_similarity;
use crate::panel_slots::panel_slice_for_embedder;
use crate::types::{EmbedderId, EntityType, Language, MutationCategory};

pub const TARGET_FPR: f32 = 0.05;
pub const MIN_SAMPLES_PANEL_LEVEL: usize = 30;
pub const MIN_SAMPLES_CHUNK_TYPE: usize = 50;

#[derive(Debug, Clone)]
pub struct HeldOutValidation {
    pub knowngood_samples: Vec<HeldOutSample>,
}

#[derive(Debug, Clone)]
pub struct HeldOutSample {
    pub language: Language,
    pub entity_type: EntityType,
    pub mutation: MutationCategory,
    pub panel: Panel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationDecision {
    pub cell: String,
    pub observed_fpr: f32,
    pub chosen_tau: f32,
    pub sample_count: usize,
    pub strict_enforcement: bool,
}

pub fn calibrate(
    centroids: &TctConstellation,
    validation: &HeldOutValidation,
) -> Result<(Thresholds, Vec<CalibrationDecision>), TctError> {
    if validation.knowngood_samples.is_empty() {
        return Err(TctError::InsufficientSamples {
            cell: "heldout_known_good".to_string(),
            observed: 0,
            required: MIN_SAMPLES_PANEL_LEVEL,
        });
    }
    for sample in &validation.knowngood_samples {
        if sample.mutation != MutationCategory::KnownGood {
            return Err(TctError::invalid(
                "HeldOutSample.mutation",
                format!(
                    "calibration requires KnownGood samples, got {:?}",
                    sample.mutation
                ),
            ));
        }
    }

    let mut panel_level = BTreeMap::new();
    let mut decisions = Vec::new();
    for embedder in EmbedderId::all() {
        let mut cosines = Vec::new();
        for sample in &validation.knowngood_samples {
            let slot = panel_slice_for_embedder(&sample.panel, embedder)?;
            let (centroid, _origin, _n) = centroids
                .lookup_centroid(
                    sample.mutation,
                    sample.language,
                    sample.entity_type,
                    embedder,
                )
                .ok_or_else(|| TctError::MissingCentroid {
                    detail: format!(
                        "missing calibration centroid for {:?}/{:?}/{:?}/{embedder}",
                        sample.mutation, sample.language, sample.entity_type
                    ),
                })?;
            cosines.push(cosine_similarity(slot, centroid)?);
        }
        if cosines.len() < MIN_SAMPLES_PANEL_LEVEL {
            return Err(TctError::InsufficientSamples {
                cell: format!("panel/{embedder}"),
                observed: cosines.len(),
                required: MIN_SAMPLES_PANEL_LEVEL,
            });
        }
        let (tau, fpr) = choose_threshold(&cosines, TARGET_FPR)?;
        if fpr > TARGET_FPR {
            return Err(TctError::ThresholdCalibrationFail {
                detail: format!("panel/{embedder} FPR {fpr} exceeded target {TARGET_FPR}"),
            });
        }
        panel_level.insert(embedder, tau);
        decisions.push(CalibrationDecision {
            cell: format!("panel/{embedder}"),
            observed_fpr: fpr,
            chosen_tau: tau,
            sample_count: cosines.len(),
            strict_enforcement: true,
        });
    }

    let mut per_chunk_type = BTreeMap::new();
    for entity_type in EntityType::all() {
        let entity_samples = validation
            .knowngood_samples
            .iter()
            .filter(|sample| sample.entity_type == entity_type)
            .collect::<Vec<_>>();
        if entity_samples.is_empty() {
            continue;
        }
        for embedder in EmbedderId::all() {
            if entity_samples.len() < MIN_SAMPLES_CHUNK_TYPE {
                decisions.push(CalibrationDecision {
                    cell: format!("chunk/{entity_type:?}/{embedder}"),
                    observed_fpr: 0.0,
                    chosen_tau: 0.0,
                    sample_count: entity_samples.len(),
                    strict_enforcement: false,
                });
                continue;
            }
            let mut cosines = Vec::new();
            for sample in &entity_samples {
                let slot = panel_slice_for_embedder(&sample.panel, embedder)?;
                let (centroid, _origin, _n) = centroids
                    .lookup_centroid(
                        sample.mutation,
                        sample.language,
                        sample.entity_type,
                        embedder,
                    )
                    .ok_or_else(|| TctError::MissingCentroid {
                        detail: format!(
                            "missing chunk calibration centroid for {:?}/{:?}/{:?}/{embedder}",
                            sample.mutation, sample.language, sample.entity_type
                        ),
                    })?;
                cosines.push(cosine_similarity(slot, centroid)?);
            }
            let (tau, fpr) = choose_threshold(&cosines, TARGET_FPR)?;
            let panel_tau =
                panel_level
                    .get(&embedder)
                    .ok_or_else(|| TctError::MissingCentroid {
                        detail: format!("missing panel threshold for {embedder}"),
                    })?;
            if tau < *panel_tau {
                return Err(TctError::ThresholdCalibrationFail {
                    detail: format!(
                        "chunk threshold {tau} for {entity_type:?}/{embedder} is looser than panel threshold {panel_tau}"
                    ),
                });
            }
            if fpr > TARGET_FPR {
                return Err(TctError::ThresholdCalibrationFail {
                    detail: format!(
                        "chunk/{entity_type:?}/{embedder} FPR {fpr} exceeded target {TARGET_FPR}"
                    ),
                });
            }
            per_chunk_type.insert((entity_type, embedder), tau);
            decisions.push(CalibrationDecision {
                cell: format!("chunk/{entity_type:?}/{embedder}"),
                observed_fpr: fpr,
                chosen_tau: tau,
                sample_count: entity_samples.len(),
                strict_enforcement: true,
            });
        }
    }

    Ok((Thresholds::try_new(panel_level, per_chunk_type)?, decisions))
}

pub fn choose_threshold(cosines: &[f32], target_fpr: f32) -> Result<(f32, f32), TctError> {
    if cosines.is_empty() {
        return Err(TctError::InsufficientSamples {
            cell: "threshold_cosines".to_string(),
            observed: 0,
            required: 1,
        });
    }
    if !target_fpr.is_finite() || !(0.0..=1.0).contains(&target_fpr) {
        return Err(TctError::invalid(
            "target_fpr",
            format!("target_fpr must be finite in [0, 1], got {target_fpr}"),
        ));
    }
    let mut sorted = cosines.to_vec();
    for (idx, value) in sorted.iter().enumerate() {
        if !value.is_finite() || !(-1.0..=1.0).contains(value) {
            return Err(TctError::nan(
                "cosines",
                format!("cosines[{idx}] must be finite in [-1,1], got {value}"),
            ));
        }
    }
    sorted.sort_by(f32::total_cmp);
    let mut chosen = sorted[0];
    let mut chosen_fpr = 0.0f32;
    for candidate in &sorted {
        let rejected = sorted.iter().filter(|score| **score < *candidate).count();
        let fpr = rejected as f32 / sorted.len() as f32;
        if fpr <= target_fpr {
            chosen = *candidate;
            chosen_fpr = fpr;
        } else {
            break;
        }
    }
    Ok((chosen, chosen_fpr))
}
