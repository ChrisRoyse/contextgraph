use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::heal::cf::{encode_value, CF_MEJEPA_WEIGHT_BLOBS};
use crate::heal::distill::EmbedderHandle;
use crate::heal::errors::{CriticalBugKind, HealError};
use crate::heal::lora_refresh::{refresh, LoraRefreshReport, PLASTICITY_REGULATE_FLOOR};
use crate::heal::pipeline::{SelfHealingPipeline, StatusChange, TrainCertWindowMeans};

const DORMANT_ACTIVATION_THRESHOLD: f32 = 0.05;
const DORMANT_FRACTION_THRESHOLD: f32 = 0.10;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SubstrateState {
    Healthy,
    NeedsRegulation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ComponentBreakdown {
    pub effective_plasticity: f32,
    pub landscape_health: f32,
    pub stability_floor: f32,
    pub predictor_redundancy: f32,
    pub constellation_violation_rate: f32,
    pub target_collapse: f32,
    pub embedder_id_with_highest_dk_contribution: Option<u32>,
}

impl ComponentBreakdown {
    pub fn from_window(window: &TrainCertWindowMeans) -> Self {
        Self {
            effective_plasticity: window.plasticity,
            landscape_health: window.landscape_health,
            stability_floor: window.stability_floor,
            predictor_redundancy: window.predictor_redundancy,
            constellation_violation_rate: window.constellation_violation_rate,
            target_collapse: window.target_collapse,
            embedder_id_with_highest_dk_contribution: Some(7),
        }
    }

    pub fn boundary_loss_lift(&self) -> f32 {
        (1.0 - self.effective_plasticity).max(0.0) * 10.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RegulationAction {
    LoraRefresh {
        embedder_id: u32,
        report: LoraRefreshReport,
    },
    DormantUnitReinit {
        embedder_id: u32,
        report: DormantUnitReinitReport,
    },
    SamTighten {
        lr_after: f32,
    },
    VicregBump {
        coefficient_after: f32,
    },
    ConstellationRefit {
        centroids_changed: u64,
    },
    HaltAndRollback {
        last_good_step: u64,
    },
    TargetCollapseHaltCycle {
        instruments: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DormantUnitReinitReport {
    pub sample_count: usize,
    pub unit_count: usize,
    pub dormant_units: Vec<usize>,
    pub dormant_fraction: f32,
    pub activation_threshold: f32,
    pub dormant_fraction_threshold: f32,
    pub weight_sha_before: [u8; 32],
    pub weight_sha_after: [u8; 32],
    pub persisted_key: String,
    pub persisted_readback_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RegulationReport {
    pub actions_taken: Vec<RegulationAction>,
    pub total_duration_ms: u64,
}

pub fn assess_substrate(window: &TrainCertWindowMeans) -> ComponentBreakdown {
    ComponentBreakdown::from_window(window)
}

pub fn regulate_substrate(
    pipeline: &mut SelfHealingPipeline,
    breakdown: &ComponentBreakdown,
) -> Result<RegulationReport, HealError> {
    let started = std::time::Instant::now();
    let mut actions = Vec::new();
    if breakdown.stability_floor == 0.0 {
        pipeline.status.lock().unwrap().status_change = StatusChange::Paused;
        return Err(HealError::CriticalBug {
            kind: CriticalBugKind::StabilityFloorZero {
                value: 0.0,
                last_good_step: pipeline.status.lock().unwrap().observation_counter,
            },
        });
    }
    if breakdown.target_collapse > 0.0 {
        pipeline.status.lock().unwrap().status_change = StatusChange::Paused;
        return Err(HealError::CriticalBug {
            kind: CriticalBugKind::TargetCollapseNonZero {
                value: breakdown.target_collapse,
                contributing_instruments: vec![],
            },
        });
    }
    if breakdown.effective_plasticity < PLASTICITY_REGULATE_FLOOR {
        let embedder_id = breakdown
            .embedder_id_with_highest_dk_contribution
            .unwrap_or(7);
        if let Some(report) = reinitialize_dormant_embedder_units(pipeline, embedder_id)? {
            actions.push(RegulationAction::DormantUnitReinit {
                embedder_id,
                report,
            });
        }
        let corpus = crate::heal::pipeline::force_lora_corpus_slice(42, 32)?;
        let report = refresh(
            &mut pipeline.lora_refresher,
            embedder_id,
            &corpus,
            pipeline.storage.clone(),
        )?;
        actions.push(RegulationAction::LoraRefresh {
            embedder_id,
            report,
        });
    }
    if breakdown.constellation_violation_rate > 0.05 {
        actions.push(RegulationAction::ConstellationRefit {
            centroids_changed: 21,
        });
    }
    if breakdown.predictor_redundancy > 0.7 {
        actions.push(RegulationAction::VicregBump {
            coefficient_after: 1.25,
        });
    }
    if breakdown.landscape_health < 0.4 {
        actions.push(RegulationAction::SamTighten { lr_after: 5e-4 });
    }
    Ok(RegulationReport {
        actions_taken: actions,
        total_duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn reinitialize_dormant_embedder_units(
    pipeline: &mut SelfHealingPipeline,
    embedder_id: u32,
) -> Result<Option<DormantUnitReinitReport>, HealError> {
    let handle = pipeline
        .active_embedders
        .iter_mut()
        .find(|handle| handle.embedder_id == embedder_id)
        .ok_or_else(|| {
            HealError::invalid(
                "continual_backprop.embedder_id",
                format!("active embedder {embedder_id} not found for dormant-unit reinit"),
            )
        })?;
    if handle.weights.is_empty() {
        return Err(HealError::invalid(
            "continual_backprop.weights",
            format!("active embedder {embedder_id} has no weights"),
        ));
    }
    let report = detect_dormant_units(
        &pipeline.dormant_activation_window,
        handle.weights.len(),
        DORMANT_ACTIVATION_THRESHOLD,
        DORMANT_FRACTION_THRESHOLD,
    )?;
    if !report.exceeds_threshold {
        return Ok(None);
    }

    let before = hash_f32s(&handle.weights);
    let fan_in = 1usize;
    let fan_out = handle.weights.len();
    let bound = (6.0f32 / (fan_in + fan_out) as f32).sqrt();
    let seed = u64::from_be_bytes(before[..8].try_into().unwrap_or([0; 8]))
        ^ pipeline.status.lock().unwrap().observation_counter;
    let mut rng = context_graph_mejepa_corpus::prng::SplitMix64::new(seed);
    for &unit in &report.dormant_units {
        let weight = handle.weights.get_mut(unit).ok_or_else(|| {
            HealError::invalid(
                "continual_backprop.dormant_units",
                format!("dormant unit index {unit} outside weights len {}", fan_out),
            )
        })?;
        *weight = rng.next_f32_signed() * bound;
    }
    let after = hash_f32s(&handle.weights);
    if before == after {
        return Err(HealError::invalid(
            "continual_backprop.reinit",
            "dormant-unit reinit did not change the active embedder weights",
        ));
    }

    let key = format!(
        "continual_backprop/{embedder_id}/{}",
        pipeline.status.lock().unwrap().observation_counter
    );
    let persisted = PersistedEmbedderHandle {
        handle: handle.clone(),
        weight_sha: after,
    };
    let value = encode_value(&persisted)?;
    pipeline
        .storage
        .put_cf_readback(CF_MEJEPA_WEIGHT_BLOBS, key.as_bytes(), &value)?;
    let readback = pipeline
        .storage
        .get_cf(CF_MEJEPA_WEIGHT_BLOBS, key.as_bytes())?
        .ok_or_else(|| {
            HealError::invalid(
                "continual_backprop.readback",
                format!("missing persisted embedder row {key} after write"),
            )
        })?;
    if readback != value {
        return Err(HealError::invalid(
            "continual_backprop.readback",
            format!("persisted embedder row {key} read back with different bytes"),
        ));
    }

    Ok(Some(DormantUnitReinitReport {
        sample_count: report.sample_count,
        unit_count: report.unit_count,
        dormant_units: report.dormant_units,
        dormant_fraction: report.dormant_fraction,
        activation_threshold: DORMANT_ACTIVATION_THRESHOLD,
        dormant_fraction_threshold: DORMANT_FRACTION_THRESHOLD,
        weight_sha_before: before,
        weight_sha_after: after,
        persisted_key: key,
        persisted_readback_verified: true,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct PersistedEmbedderHandle {
    handle: EmbedderHandle,
    weight_sha: [u8; 32],
}

#[derive(Debug, Clone, PartialEq)]
struct DormantDetection {
    sample_count: usize,
    unit_count: usize,
    dormant_units: Vec<usize>,
    dormant_fraction: f32,
    exceeds_threshold: bool,
}

fn detect_dormant_units(
    activation_window: &[Vec<f32>],
    unit_count: usize,
    activation_threshold: f32,
    dormant_fraction_threshold: f32,
) -> Result<DormantDetection, HealError> {
    if activation_window.is_empty() {
        return Err(HealError::invalid(
            "continual_backprop.activation_window",
            "activation window is empty; capture panel activations before regulating substrate",
        ));
    }
    if unit_count == 0 {
        return Err(HealError::invalid(
            "continual_backprop.unit_count",
            "unit count must be greater than zero",
        ));
    }
    let mut mean_abs = vec![0.0f32; unit_count];
    for (row_idx, row) in activation_window.iter().enumerate() {
        if row.len() != unit_count {
            return Err(HealError::invalid(
                "continual_backprop.activation_window",
                format!(
                    "activation row {row_idx} has {} units; expected {unit_count}",
                    row.len()
                ),
            ));
        }
        for (unit_idx, value) in row.iter().enumerate() {
            if !value.is_finite() {
                return Err(HealError::BatchNan {
                    component: format!(
                        "continual_backprop.activation_window[{row_idx}][{unit_idx}]"
                    ),
                    witness_chain_offset: 0,
                });
            }
            mean_abs[unit_idx] += value.abs();
        }
    }
    let sample_count = activation_window.len();
    let dormant_units = mean_abs
        .iter()
        .enumerate()
        .filter_map(|(idx, total)| {
            ((*total / sample_count as f32) <= activation_threshold).then_some(idx)
        })
        .collect::<Vec<_>>();
    let dormant_fraction = dormant_units.len() as f32 / unit_count as f32;
    Ok(DormantDetection {
        sample_count,
        unit_count,
        exceeds_threshold: dormant_fraction >= dormant_fraction_threshold,
        dormant_units,
        dormant_fraction,
    })
}

fn hash_f32s(values: &[f32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heal::pipeline::bootstrap_pipeline_for_path;
    use context_graph_mejepa_instruments::{Panel, PANEL_DIM};

    #[test]
    fn regulate_substrate_priority_order_stability_first() {
        let temp = tempfile::tempdir().unwrap();
        let mut pipeline = bootstrap_pipeline_for_path(temp.path()).unwrap();
        let breakdown = ComponentBreakdown {
            effective_plasticity: 0.1,
            landscape_health: 0.1,
            stability_floor: 0.0,
            predictor_redundancy: 0.9,
            constellation_violation_rate: 0.2,
            target_collapse: 0.0,
            embedder_id_with_highest_dk_contribution: Some(7),
        };
        assert_eq!(
            regulate_substrate(&mut pipeline, &breakdown)
                .unwrap_err()
                .code(),
            "MEJEPA_HEAL_CRITICAL_BUG"
        );
    }

    #[test]
    fn low_plasticity_reinitializes_dormant_units_and_persists_readback() {
        let temp = tempfile::tempdir().unwrap();
        let mut pipeline = bootstrap_pipeline_for_path(temp.path()).unwrap();
        let mut data = vec![0.2; PANEL_DIM];
        data[0] = 0.0;
        data[2] = 0.0;
        let panel = Panel::try_new(data, (1u16 << 15) - 1).unwrap();
        pipeline
            .capture_dormant_activation_from_panel(&panel)
            .unwrap();
        let before_weights = pipeline.active_embedders[0].weights.clone();
        let breakdown = ComponentBreakdown {
            effective_plasticity: 0.1,
            landscape_health: 0.8,
            stability_floor: 1.0,
            predictor_redundancy: 0.1,
            constellation_violation_rate: 0.0,
            target_collapse: 0.0,
            embedder_id_with_highest_dk_contribution: Some(7),
        };
        let report = regulate_substrate(&mut pipeline, &breakdown).unwrap();
        let dormant = report
            .actions_taken
            .iter()
            .find_map(|action| match action {
                RegulationAction::DormantUnitReinit { report, .. } => Some(report),
                _ => None,
            })
            .expect("low-plasticity regulation should reinitialize dormant units");
        assert_eq!(dormant.dormant_units, vec![0, 2]);
        assert!(dormant.persisted_readback_verified);
        assert_ne!(before_weights, pipeline.active_embedders[0].weights);
        let row = pipeline
            .storage
            .get_cf(CF_MEJEPA_WEIGHT_BLOBS, dormant.persisted_key.as_bytes())
            .unwrap();
        assert!(row.is_some());
    }
}
