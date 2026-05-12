use std::collections::BTreeMap;

use context_graph_mejepa::HeadId;
use serde::{Deserialize, Serialize};

use super::{
    clamp01, compute_l_step, validate_unit, DeltaKComponents, DeltaOmegaComponents,
    DeltaPComponents, DeltaXiComponents, LearningSignal, UtmlError, UtmlErrorCode,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadSignalInput {
    pub delta_p: DeltaPComponents,
    pub delta_k: DeltaKComponents,
    pub delta_omega: DeltaOmegaComponents,
    pub delta_xi: DeltaXiComponents,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerHeadLearningSignal {
    pub per_head: BTreeMap<HeadId, LearningSignal>,
    pub delta_xi_global_min: f32,
    pub mean_l_step: f32,
}

impl PerHeadLearningSignal {
    pub fn l_step_map(&self) -> BTreeMap<String, f32> {
        self.per_head
            .iter()
            .map(|(head, signal)| (head.as_str().to_string(), signal.l_step))
            .collect()
    }

    pub fn validate(&self) -> Result<(), UtmlError> {
        validate_required_heads(&self.per_head)?;
        validate_unit("per_head.delta_xi_global_min", self.delta_xi_global_min)?;
        validate_unit("per_head.mean_l_step", self.mean_l_step)?;
        let observed_min = self
            .per_head
            .values()
            .map(|signal| signal.delta_xi)
            .fold(1.0_f32, f32::min);
        if (observed_min - self.delta_xi_global_min).abs() > 1e-6 {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!(
                    "per-head global ΔΞ {} does not match observed minimum {}",
                    self.delta_xi_global_min, observed_min
                ),
            ));
        }
        let observed_mean = self
            .per_head
            .values()
            .map(|signal| signal.l_step)
            .sum::<f32>()
            / self.per_head.len() as f32;
        if (observed_mean - self.mean_l_step).abs() > 1e-6 {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!(
                    "per-head mean L-step {} does not match observed mean {}",
                    self.mean_l_step, observed_mean
                ),
            ));
        }
        Ok(())
    }
}

pub fn compute_per_head_learning_signal(
    inputs: BTreeMap<HeadId, HeadSignalInput>,
) -> Result<PerHeadLearningSignal, UtmlError> {
    validate_required_inputs(&inputs)?;
    let mut per_head = BTreeMap::new();
    for (head, input) in inputs {
        let signal = compute_l_step(
            input.delta_p,
            input.delta_k,
            input.delta_omega,
            input.delta_xi,
        )?;
        per_head.insert(head, signal);
    }
    let delta_xi_global_min = per_head
        .values()
        .map(|signal| signal.delta_xi)
        .fold(1.0_f32, f32::min);
    let mean_l_step =
        clamp01(per_head.values().map(|signal| signal.l_step).sum::<f32>() / per_head.len() as f32);
    let value = PerHeadLearningSignal {
        per_head,
        delta_xi_global_min,
        mean_l_step,
    };
    value.validate()?;
    Ok(value)
}

fn validate_required_inputs(inputs: &BTreeMap<HeadId, HeadSignalInput>) -> Result<(), UtmlError> {
    for head in HeadId::ALL {
        if !inputs.contains_key(&head) {
            return Err(UtmlError::new(
                UtmlErrorCode::MissingSourceOfTruth,
                format!(
                    "missing per-head learning signal input for {}",
                    head.as_str()
                ),
            ));
        }
    }
    if inputs.len() != HeadId::ALL.len() {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "per-head learning signal requires exactly {} heads, got {}",
                HeadId::ALL.len(),
                inputs.len()
            ),
        ));
    }
    Ok(())
}

fn validate_required_heads(signals: &BTreeMap<HeadId, LearningSignal>) -> Result<(), UtmlError> {
    for head in HeadId::ALL {
        let signal = signals.get(&head).ok_or_else(|| {
            UtmlError::new(
                UtmlErrorCode::MissingSourceOfTruth,
                format!("missing per-head learning signal for {}", head.as_str()),
            )
        })?;
        signal.validate()?;
    }
    if signals.len() != HeadId::ALL.len() {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "per-head learning signal requires exactly {} heads, got {}",
                HeadId::ALL.len(),
                signals.len()
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning_signal::DeltaPAggregator;

    fn input(delta_xi_redundancy: f32) -> HeadSignalInput {
        HeadSignalInput {
            delta_p: DeltaPComponents {
                delta_p_real: 0.5,
                delta_p_imagined: None,
                snr: 1.0,
                exploration_bonus: 0.0,
                gamma: 0.7,
                aggregator: DeltaPAggregator::Mean,
                per_chunk_values: vec![0.5],
            },
            delta_k: DeltaKComponents::default(),
            delta_omega: DeltaOmegaComponents::default(),
            delta_xi: DeltaXiComponents {
                target_collapse: 0.0,
                predictor_redundancy: delta_xi_redundancy,
                constellation_violation_rate: 0.0,
            },
        }
    }

    #[test]
    fn global_delta_xi_is_minimum_per_head_value() {
        let mut inputs = BTreeMap::new();
        for head in HeadId::ALL {
            inputs.insert(
                head,
                input(if head == HeadId::Security { 0.9 } else { 0.0 }),
            );
        }
        let signal = compute_per_head_learning_signal(inputs).unwrap();
        let security = signal.per_head.get(&HeadId::Security).unwrap().delta_xi;
        assert_eq!(signal.delta_xi_global_min, security);
    }

    #[test]
    fn missing_head_fails_closed() {
        let inputs = BTreeMap::new();
        let err = compute_per_head_learning_signal(inputs).unwrap_err();
        assert_eq!(err.code(), "UTML_MISSING_SOURCE_OF_TRUTH");
    }
}
