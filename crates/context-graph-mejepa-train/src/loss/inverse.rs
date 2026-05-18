use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::Tensor;
use std::collections::HashMap;

use super::{finite_tensor, scalar, LossBreakdown};

const INV_TWO_LN_2: f64 = 1.0 / (2.0 * std::f64::consts::LN_2);

#[derive(Debug, Clone)]
pub struct InverseMapOutputs {
    pub predicted_input_panel: Tensor,
    pub predicted_action: Tensor,
}

#[derive(Debug, Clone)]
pub struct InverseMapTargets {
    pub input_panel: Tensor,
    pub action: Tensor,
}

#[derive(Debug, Clone)]
pub struct InverseMapLossBreakdown {
    pub loss: f32,
    pub loss_tensor: Tensor,
    pub input_panel_nll_bits: f64,
    pub action_nll_bits: f64,
    pub quality_bits: f64,
    pub components: HashMap<String, f32>,
}

pub fn inverse_map_loss(
    outputs: &InverseMapOutputs,
    targets: &InverseMapTargets,
) -> Result<InverseMapLossBreakdown, TrainerError> {
    ensure_same_shape(
        &outputs.predicted_input_panel,
        &targets.input_panel,
        "inverse_input_panel",
    )?;
    ensure_same_shape(&outputs.predicted_action, &targets.action, "inverse_action")?;
    finite_tensor(
        &outputs.predicted_input_panel,
        "inverse.predicted_input_panel",
    )?;
    finite_tensor(&targets.input_panel, "inverse.target_input_panel")?;
    finite_tensor(&outputs.predicted_action, "inverse.predicted_action")?;
    finite_tensor(&targets.action, "inverse.target_action")?;

    let input_mse_tensor = (&outputs.predicted_input_panel - &targets.input_panel)?
        .sqr()?
        .mean_all()?;
    let action_mse_tensor = (&outputs.predicted_action - &targets.action)?
        .sqr()?
        .mean_all()?;
    let loss_tensor = (&input_mse_tensor + &action_mse_tensor)?;
    let input_mse = scalar(&input_mse_tensor)?;
    let action_mse = scalar(&action_mse_tensor)?;
    let loss = scalar(&loss_tensor)?;
    if !loss.is_finite() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainLossNan,
            "inverse_map loss is non-finite",
        ));
    }
    let input_panel_nll_bits = unit_gaussian_nll_bits(input_mse)?;
    let action_nll_bits = unit_gaussian_nll_bits(action_mse)?;
    let quality_bits = input_panel_nll_bits + action_nll_bits;
    Ok(InverseMapLossBreakdown {
        loss,
        loss_tensor,
        input_panel_nll_bits,
        action_nll_bits,
        quality_bits,
        components: HashMap::from([
            ("inverse_input_panel_mse".to_string(), input_mse),
            ("inverse_action_mse".to_string(), action_mse),
        ]),
    })
}

pub fn compose_bidirectional_l_full(
    forward: &LossBreakdown,
    inverse: Option<&InverseMapLossBreakdown>,
    inverse_map_coefficient: f32,
) -> Result<LossBreakdown, TrainerError> {
    if !inverse_map_coefficient.is_finite() || inverse_map_coefficient < 0.0 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "inverse_map_coefficient must be non-negative finite",
        ));
    }
    if inverse_map_coefficient == 0.0 {
        return Ok(forward.clone());
    }
    let inverse = inverse.ok_or_else(|| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "inverse_map_coefficient > 0 requires inverse-map outputs",
        )
    })?;
    let weighted_inverse = inverse
        .loss_tensor
        .affine(inverse_map_coefficient as f64, 0.0)?;
    let total_tensor = (&forward.total_tensor + &weighted_inverse)?;
    let total = scalar(&total_tensor)?;
    if !total.is_finite() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainLossNan,
            "bidirectional L_full is non-finite",
        ));
    }
    let mut components = forward.components.clone();
    components.insert("inverse_map".to_string(), inverse.loss);
    components.insert(
        "inverse_map_weighted".to_string(),
        inverse.loss * inverse_map_coefficient,
    );
    Ok(LossBreakdown {
        total,
        total_tensor,
        components,
    })
}

pub fn unit_gaussian_nll_bits(mse: f32) -> Result<f64, TrainerError> {
    if !mse.is_finite() || mse < 0.0 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainLossNan,
            format!("inverse-map MSE must be finite and non-negative; got {mse}"),
        ));
    }
    Ok(0.5 * std::f64::consts::TAU.log2() + mse as f64 * INV_TWO_LN_2)
}

fn ensure_same_shape(a: &Tensor, b: &Tensor, field: &'static str) -> Result<(), TrainerError> {
    if a.dims() != b.dims() || a.elem_count() == 0 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!("{field}: shape {:?} != {:?} or empty", a.dims(), b.dims()),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};
    use std::collections::HashMap;

    #[test]
    fn inverse_loss_quality_bits_are_finite() {
        let device = Device::Cpu;
        let outputs = InverseMapOutputs {
            predicted_input_panel: Tensor::from_slice(&[0f32, 1., 2., 3.], (2, 2), &device)
                .unwrap(),
            predicted_action: Tensor::from_slice(&[1f32, 0.], (2, 1), &device).unwrap(),
        };
        let targets = InverseMapTargets {
            input_panel: Tensor::from_slice(&[0f32, 1., 1., 3.], (2, 2), &device).unwrap(),
            action: Tensor::from_slice(&[1f32, 1.], (2, 1), &device).unwrap(),
        };
        let loss = inverse_map_loss(&outputs, &targets).unwrap();
        assert!(loss.loss.is_finite());
        assert!(loss.quality_bits.is_finite());
        assert!(loss.quality_bits >= 0.0);
    }

    #[test]
    fn zero_coefficient_is_forward_compatible() {
        let device = Device::Cpu;
        let forward = LossBreakdown {
            total: 1.25,
            total_tensor: Tensor::new(1.25f32, &device).unwrap(),
            components: HashMap::from([("predict".to_string(), 1.25)]),
        };
        let composed = compose_bidirectional_l_full(&forward, None, 0.0).unwrap();
        assert_eq!(composed.total, forward.total);
        assert_eq!(composed.components, forward.components);
    }

    #[test]
    fn positive_coefficient_requires_inverse_outputs() {
        let device = Device::Cpu;
        let forward = LossBreakdown {
            total: 1.0,
            total_tensor: Tensor::new(1.0f32, &device).unwrap(),
            components: HashMap::new(),
        };
        let err = compose_bidirectional_l_full(&forward, None, 0.25).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }
}
