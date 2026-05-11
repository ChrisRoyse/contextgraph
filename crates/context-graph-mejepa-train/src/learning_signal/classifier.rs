use super::{non_empty, validate_unit, LearningSignal, UtmlError, UtmlErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UtmlState {
    Boring,
    Confused,
    LatentCollapsing,
    Stuck,
    OptimizerDysregulated,
    Dissipating,
    Optimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intervention {
    Drop,
    Keep,
    ForceInclude,
    IncreaseExploration,
    RefreshOptimizer,
    ReinitializeDormantUnits,
    Consolidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassifierVerdict {
    pub state: UtmlState,
    pub intervention: Intervention,
    pub confidence: f32,
    pub matched_rules: Vec<String>,
    pub metrics: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LStepWindow {
    pub signals: Vec<LearningSignal>,
    pub stale_update_ratio: f32,
    pub optimizer_spike_ratio: f32,
    pub dissipating_ratio: f32,
}

impl LStepWindow {
    pub fn validate(&self) -> Result<(), UtmlError> {
        non_empty("signals", &self.signals)?;
        validate_unit("stale_update_ratio", self.stale_update_ratio)?;
        validate_unit("optimizer_spike_ratio", self.optimizer_spike_ratio)?;
        validate_unit("dissipating_ratio", self.dissipating_ratio)?;
        for (idx, signal) in self.signals.iter().enumerate() {
            signal.validate().map_err(|err| {
                UtmlError::new(
                    err.code,
                    format!("signals[{idx}] failed validation: {}", err.message),
                )
            })?;
        }
        Ok(())
    }

    pub fn mean_l_step(&self) -> f32 {
        self.signals.iter().map(|s| s.l_step).sum::<f32>() / self.signals.len() as f32
    }

    pub fn mean_delta_p(&self) -> f32 {
        self.signals.iter().map(|s| s.delta_p).sum::<f32>() / self.signals.len() as f32
    }

    pub fn mean_delta_k(&self) -> f32 {
        self.signals.iter().map(|s| s.delta_k).sum::<f32>() / self.signals.len() as f32
    }

    pub fn mean_delta_omega(&self) -> f32 {
        self.signals.iter().map(|s| s.delta_omega).sum::<f32>() / self.signals.len() as f32
    }

    pub fn mean_delta_xi(&self) -> f32 {
        self.signals.iter().map(|s| s.delta_xi).sum::<f32>() / self.signals.len() as f32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PruneDecision {
    pub drop: bool,
    pub force_include: bool,
    pub reason: String,
}

pub fn classify_utml_state(window: &LStepWindow) -> Result<ClassifierVerdict, UtmlError> {
    window.validate()?;
    let mut metrics = BTreeMap::new();
    let mean_l = window.mean_l_step();
    let mean_p = window.mean_delta_p();
    let mean_k = window.mean_delta_k();
    let mean_o = window.mean_delta_omega();
    let mean_x = window.mean_delta_xi();
    metrics.insert("mean_l_step".to_string(), mean_l);
    metrics.insert("mean_delta_p".to_string(), mean_p);
    metrics.insert("mean_delta_k".to_string(), mean_k);
    metrics.insert("mean_delta_omega".to_string(), mean_o);
    metrics.insert("mean_delta_xi".to_string(), mean_x);
    metrics.insert("stale_update_ratio".to_string(), window.stale_update_ratio);
    metrics.insert(
        "optimizer_spike_ratio".to_string(),
        window.optimizer_spike_ratio,
    );
    metrics.insert("dissipating_ratio".to_string(), window.dissipating_ratio);

    let mut matched = Vec::new();
    let state = if mean_x < 0.60 {
        matched.push("delta_xi_below_0_60".to_string());
        UtmlState::LatentCollapsing
    } else if window.optimizer_spike_ratio > 0.35 || mean_o < 0.30 {
        matched.push("optimizer_spike_or_delta_omega_low".to_string());
        UtmlState::OptimizerDysregulated
    } else if window.stale_update_ratio > 0.70 && mean_l < 0.20 {
        matched.push("stale_updates_high_and_l_step_low".to_string());
        UtmlState::Stuck
    } else if window.dissipating_ratio > 0.45 {
        matched.push("m_t_dissipating_ratio_high".to_string());
        UtmlState::Dissipating
    } else if mean_p > 0.65 && mean_k < 0.35 {
        matched.push("learning_pressure_high_but_knowledge_low".to_string());
        UtmlState::Confused
    } else if mean_l >= 0.35 && mean_p >= 0.35 && mean_k >= 0.35 && mean_o >= 0.35 {
        matched.push("balanced_learning_signal".to_string());
        UtmlState::Optimal
    } else {
        matched.push("low_learning_value".to_string());
        UtmlState::Boring
    };

    let confidence = match state {
        UtmlState::LatentCollapsing => 1.0 - mean_x,
        UtmlState::OptimizerDysregulated => window.optimizer_spike_ratio.max(1.0 - mean_o),
        UtmlState::Stuck => (window.stale_update_ratio + (1.0 - mean_l)) / 2.0,
        UtmlState::Dissipating => window.dissipating_ratio,
        UtmlState::Confused => (mean_p + (1.0 - mean_k)) / 2.0,
        UtmlState::Optimal => mean_l,
        UtmlState::Boring => 1.0 - mean_l,
    }
    .clamp(0.0, 1.0);

    if matched.is_empty() {
        return Err(UtmlError::new(
            UtmlErrorCode::AmbiguousClassifierState,
            "UTML classifier produced no rule match",
        ));
    }

    Ok(ClassifierVerdict {
        state,
        intervention: intervention_for_state(state),
        confidence,
        matched_rules: matched,
        metrics,
    })
}

pub fn prune_decision(
    signal: &LearningSignal,
    verdict: &ClassifierVerdict,
) -> Result<PruneDecision, UtmlError> {
    signal.validate()?;
    validate_unit("classifier.confidence", verdict.confidence)?;
    let decision = match verdict.state {
        UtmlState::LatentCollapsing | UtmlState::OptimizerDysregulated => PruneDecision {
            drop: false,
            force_include: true,
            reason: format!("force include for {:?}", verdict.state),
        },
        UtmlState::Boring if signal.l_step < 0.05 && verdict.confidence > 0.80 => PruneDecision {
            drop: true,
            force_include: false,
            reason: "drop low-value boring signal".to_string(),
        },
        UtmlState::Stuck if signal.l_step < 0.10 => PruneDecision {
            drop: false,
            force_include: true,
            reason: "force include stuck window for recovery pressure".to_string(),
        },
        _ => PruneDecision {
            drop: false,
            force_include: false,
            reason: "keep".to_string(),
        },
    };
    Ok(decision)
}

fn intervention_for_state(state: UtmlState) -> Intervention {
    match state {
        UtmlState::Boring => Intervention::Drop,
        UtmlState::Confused => Intervention::IncreaseExploration,
        UtmlState::LatentCollapsing => Intervention::ReinitializeDormantUnits,
        UtmlState::Stuck => Intervention::ForceInclude,
        UtmlState::OptimizerDysregulated => Intervention::RefreshOptimizer,
        UtmlState::Dissipating => Intervention::Consolidate,
        UtmlState::Optimal => Intervention::Keep,
    }
}
