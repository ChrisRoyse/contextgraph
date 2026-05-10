use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::{DType, Tensor};

pub fn l_variance(predicted: &Tensor) -> Result<Tensor, TrainerError> {
    validate_rank2(predicted)?;
    let x = predicted.to_dtype(DType::F32)?;
    let batch = x.dims()[0].max(2);
    let centered = x.broadcast_sub(&x.mean_keepdim(0)?)?;
    let variance = (centered.sqr()?.sum_keepdim(0)? / (batch.saturating_sub(1) as f64))?;
    let std = (variance + 1e-4)?.sqrt()?;
    let gamma = Tensor::new(1.0f32, x.device())?;
    Ok(gamma.broadcast_sub(&std)?.relu()?.mean_all()?)
}

pub fn l_covariance(predicted: &Tensor) -> Result<Tensor, TrainerError> {
    validate_rank2(predicted)?;
    let x = predicted.to_dtype(DType::F32)?;
    let batch = x.dims()[0].max(2);
    let dim = x.dims()[1];
    let centered = x.broadcast_sub(&x.mean_keepdim(0)?)?;
    let cov = (centered.t()?.matmul(&centered)? / (batch.saturating_sub(1) as f64))?;
    let ones = Tensor::ones((dim, dim), DType::F32, x.device())?;
    let eye = Tensor::eye(dim, DType::F32, x.device())?;
    let off = cov.broadcast_mul(&(ones - eye)?)?;
    Ok((off.sqr()?.sum_all()? / dim as f64)?)
}

pub fn l_invariance(p1: &Tensor, p2: &Tensor) -> Result<Tensor, TrainerError> {
    if p1.dims() != p2.dims() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "VICReg invariance tensors differ in shape",
        ));
    }
    Ok((p1 - p2)?.sqr()?.mean_all()?)
}

fn validate_rank2(tensor: &Tensor) -> Result<(), TrainerError> {
    if tensor.dims().len() != 2 || tensor.dims()[0] < 2 || tensor.dims()[1] == 0 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!(
                "VICReg expects rank-2 tensor with batch>=2, got {:?}",
                tensor.dims()
            ),
        ));
    }
    Ok(())
}
