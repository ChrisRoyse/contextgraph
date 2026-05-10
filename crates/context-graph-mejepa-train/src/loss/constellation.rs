use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::Tensor;

pub fn l_constellation_centroid(
    predicted: &Tensor,
    centroids: Option<&[Tensor]>,
) -> Result<Tensor, TrainerError> {
    let Some(centroids) = centroids else {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "constellation centroid loss requires real current_task_centroids; refusing zero-loss fallback",
        ));
    };
    if centroids.is_empty() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "constellation centroid list was Some(empty)",
        ));
    }
    let pred = predicted
        .to_dtype(candle_core::DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let mut best = f32::INFINITY;
    for centroid in centroids {
        let vals = centroid
            .to_dtype(candle_core::DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        if vals.len() != pred.len() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                "constellation centroid dim mismatch",
            ));
        }
        best = best.min(1.0 - cosine(&pred, &vals)?);
    }
    Tensor::new(best.max(0.0), predicted.device()).map_err(TrainerError::from)
}

fn cosine(a: &[f32], b: &[f32]) -> Result<f32, TrainerError> {
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for (idx, (x, y)) in a.iter().zip(b).enumerate() {
        if !x.is_finite() || !y.is_finite() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainLossNan,
                format!("constellation centroid cosine saw non-finite value at dim {idx}"),
            ));
        }
        dot += *x as f64 * *y as f64;
        na += *x as f64 * *x as f64;
        nb += *y as f64 * *y as f64;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom < 1.0e-12 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "constellation centroid cosine saw zero-norm predicted or centroid vector",
        ));
    }
    Ok((dot / denom).clamp(-1.0, 1.0) as f32)
}
