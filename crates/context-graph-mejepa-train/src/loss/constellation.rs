use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::Tensor;

pub fn l_constellation_centroid(
    predicted: &Tensor,
    centroids: Option<&[Tensor]>,
) -> Result<Tensor, TrainerError> {
    let Some(centroids) = centroids else {
        return Tensor::new(0f32, predicted.device()).map_err(TrainerError::from);
    };
    if centroids.is_empty() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "constellation centroid list was Some(empty)",
        ));
    }
    let pred = predicted.to_dtype(candle_core::DType::F32)?.flatten_all()?.to_vec1::<f32>()?;
    let mut best = f32::INFINITY;
    for centroid in centroids {
        let vals = centroid.to_dtype(candle_core::DType::F32)?.flatten_all()?.to_vec1::<f32>()?;
        if vals.len() != pred.len() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                "constellation centroid dim mismatch",
            ));
        }
        best = best.min(1.0 - cosine(&pred, &vals));
    }
    Tensor::new(best.max(0.0), predicted.device()).map_err(TrainerError::from)
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}
