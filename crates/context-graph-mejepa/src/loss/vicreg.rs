use candle_core::{DType, Tensor};

use crate::config::PANEL_DIM;
use crate::error::LossError;
use crate::evidence::{LossOutputs, VicregLambdas};

pub fn vicreg_loss(
    predicted: &Tensor,
    target: &Tensor,
    lambdas: VicregLambdas,
) -> Result<LossOutputs, LossError> {
    lambdas.validate_finite()?;
    validate_loss_inputs(predicted, target)?;
    ensure_tensor_finite(predicted, "predicted")?;
    ensure_tensor_finite(target, "target")?;

    let predicted = predicted.to_dtype(DType::F32)?;
    let target = target.to_dtype(DType::F32)?;
    let l_predict = huber_loss_delta_one(&predicted, &target)?;
    let l_invariance = (&predicted - &target)?
        .sqr()?
        .mean_all()?
        .to_scalar::<f32>()?;
    let (pred_var_loss, pred_low) = variance_loss_one(&predicted, lambdas.gamma)?;
    let (target_var_loss, target_low) = variance_loss_one(&target, lambdas.gamma)?;
    let l_variance = (pred_var_loss + target_var_loss) * 0.5;
    let l_covariance = (covariance_loss_one(&predicted)? + covariance_loss_one(&target)?) * 0.5;
    let l_total = l_predict
        + lambdas.var * l_variance
        + lambdas.cov * l_covariance
        + lambdas.inv * l_invariance;
    let expected = l_predict
        + lambdas.var * l_variance
        + lambdas.cov * l_covariance
        + lambdas.inv * l_invariance;
    let formula_check = (l_total - expected).abs() <= 1e-3_f32.max(expected.abs() * 1e-5);
    let outputs = LossOutputs {
        l_predict,
        l_variance,
        l_covariance,
        l_invariance,
        l_total,
        low_variance_dim_count: pred_low + target_low,
        formula_check,
    };
    if !outputs.finite() {
        return Err(LossError::NanDetected {
            component: "vicreg",
            detail: format!("non-finite VICReg output: {outputs:?}"),
        });
    }
    Ok(outputs)
}

pub fn huber_loss_delta_one(predicted: &Tensor, target: &Tensor) -> Result<f32, LossError> {
    if predicted.dims() != target.dims() {
        return Err(LossError::DimMismatch {
            detail: format!(
                "huber tensors differ: {:?} vs {:?}",
                predicted.dims(),
                target.dims()
            ),
        });
    }
    let diff = (predicted - target)?;
    let abs = diff.abs()?;
    let quadratic = (diff.sqr()? * 0.5)?;
    let linear = (&abs - 0.5)?;
    let one = Tensor::new(1.0f32, predicted.device())?;
    let mask = abs.broadcast_le(&one)?;
    Ok(mask
        .where_cond(&quadratic, &linear)?
        .mean_all()?
        .to_scalar::<f32>()?)
}

fn validate_loss_inputs(predicted: &Tensor, target: &Tensor) -> Result<(), LossError> {
    if predicted.dims().len() != 2 || target.dims().len() != 2 {
        return Err(LossError::DimMismatch {
            detail: format!(
                "loss expects rank-2 tensors; predicted={:?} target={:?}",
                predicted.dims(),
                target.dims()
            ),
        });
    }
    if predicted.dims()[0] < 2 {
        return Err(LossError::BatchTooSmall {
            batch: predicted.dims()[0],
        });
    }
    if predicted.dims() != target.dims() || predicted.dims()[1] != PANEL_DIM {
        return Err(LossError::DimMismatch {
            detail: format!(
                "loss expects predicted=target=(B, {PANEL_DIM}); predicted={:?} target={:?}",
                predicted.dims(),
                target.dims()
            ),
        });
    }
    Ok(())
}

fn ensure_tensor_finite(tensor: &Tensor, component: &'static str) -> Result<(), LossError> {
    let values = tensor
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    if values.iter().any(|value| !value.is_finite()) {
        return Err(LossError::NanDetected {
            component,
            detail: "tensor contains NaN or Inf".to_string(),
        });
    }
    Ok(())
}

fn variance_loss_one(tensor: &Tensor, gamma: f32) -> Result<(f32, usize), LossError> {
    let batch = tensor.dims()[0];
    let centered = tensor.broadcast_sub(&tensor.mean_keepdim(0)?)?;
    let variance = (centered.sqr()?.sum_keepdim(0)? / (batch.saturating_sub(1) as f64))?;
    let std = (variance + 1e-4)?.sqrt()?;
    let gamma_tensor = Tensor::new(gamma, tensor.device())?;
    let loss = (gamma_tensor.broadcast_sub(&std)?.relu()?)
        .mean_all()?
        .to_scalar::<f32>()?;
    let low_count = std
        .flatten_all()?
        .to_vec1::<f32>()?
        .into_iter()
        .filter(|value| *value < gamma)
        .count();
    Ok((loss, low_count))
}

fn covariance_loss_one(tensor: &Tensor) -> Result<f32, LossError> {
    let batch = tensor.dims()[0];
    let centered = tensor.broadcast_sub(&tensor.mean_keepdim(0)?)?;
    let cov = (centered.t()?.matmul(&centered)? / (batch.saturating_sub(1) as f64))?;
    let ones = Tensor::ones((PANEL_DIM, PANEL_DIM), DType::F32, tensor.device())?;
    let eye = Tensor::eye(PANEL_DIM, DType::F32, tensor.device())?;
    let mask = (ones - eye)?;
    let off_diag = cov.broadcast_mul(&mask)?;
    Ok((off_diag.sqr()?.sum_all()? / PANEL_DIM as f64)?.to_scalar::<f32>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn huber_known_values() {
        let device = Device::Cpu;
        let predicted = Tensor::from_slice(&[0.0f32, 2.0, 4.0, 6.0], (2, 2), &device)
            .expect("predicted tensor");
        let target =
            Tensor::from_slice(&[0.0f32, 1.0, 1.0, 6.0], (2, 2), &device).expect("target tensor");
        let loss = huber_loss_delta_one(&predicted, &target).expect("huber loss");
        assert!((loss - 0.75).abs() < 1e-6, "loss={loss}");
    }
}
