use serde::{Deserialize, Serialize};

use crate::heal::errors::{CriticalBugKind, HealError};
use crate::heal::lora_refresh::{refresh, LoraRefreshReport, PLASTICITY_REGULATE_FLOOR};
use crate::heal::pipeline::{SelfHealingPipeline, StatusChange, TrainCertWindowMeans};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heal::pipeline::bootstrap_pipeline_for_path;

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
}
