use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::Tensor;

pub fn l_predict(predicted: &Tensor, target_detached: &Tensor) -> Result<Tensor, TrainerError> {
    if predicted.dims() != target_detached.dims() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!(
                "L_predict dim mismatch: predicted={:?} target={:?}",
                predicted.dims(),
                target_detached.dims()
            ),
        ));
    }
    let target = target_detached.detach();
    Ok((predicted - &target)?.sqr()?.mean_all()?)
}
