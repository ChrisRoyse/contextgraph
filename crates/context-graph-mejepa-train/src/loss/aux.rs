use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::{DType, Tensor};

pub fn l_mse(predicted: &Tensor, actual: &Tensor) -> Result<Tensor, TrainerError> {
    ensure_same_shape(predicted, actual, "mse")?;
    Ok((predicted - actual)?.sqr()?.mean_all()?)
}

pub fn l_binary_ce(logits: &Tensor, actual: &Tensor) -> Result<Tensor, TrainerError> {
    ensure_same_shape(logits, actual, "binary_ce")?;
    let device = logits.device().clone();
    let logit_vals = logits
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let actual_vals = actual
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let mut loss = 0.0f32;
    for (logit, y) in logit_vals.iter().zip(actual_vals.iter()) {
        if !logit.is_finite() || !y.is_finite() {
            return Err(non_finite("binary_ce"));
        }
        let max = logit.max(0.0);
        loss += max - logit * y + (1.0 + (-logit.abs()).exp()).ln();
    }
    Tensor::new(loss / logit_vals.len().max(1) as f32, &device).map_err(TrainerError::from)
}

pub fn l_cluster_ce(logits: &Tensor, class_ids: &Tensor) -> Result<Tensor, TrainerError> {
    let dims = logits.dims();
    if dims.len() != 2 {
        return Err(shape_error("cluster_ce", format!("logits rank {:?}", dims)));
    }
    let batch = dims[0];
    let classes = dims[1];
    let labels = class_ids
        .to_dtype(DType::U32)?
        .flatten_all()?
        .to_vec1::<u32>()?;
    if labels.len() != batch {
        return Err(shape_error(
            "cluster_ce",
            "label count does not match batch",
        ));
    }
    let vals = logits
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let mut total = 0.0f32;
    for row in 0..batch {
        let label = labels[row] as usize;
        if label >= classes {
            return Err(shape_error("cluster_ce", "label is out of class range"));
        }
        let slice = &vals[row * classes..(row + 1) * classes];
        total += ce_row(slice, label)?;
    }
    Tensor::new(total / batch.max(1) as f32, logits.device()).map_err(TrainerError::from)
}

pub fn l_cluster_contrastive(
    embedding: &Tensor,
    class_ids: &Tensor,
) -> Result<Tensor, TrainerError> {
    let dims = embedding.dims();
    if dims.len() != 2 {
        return Err(shape_error(
            "cluster_contrastive",
            format!("rank {:?}", dims),
        ));
    }
    let batch = dims[0];
    let dim = dims[1];
    let labels = class_ids
        .to_dtype(DType::U32)?
        .flatten_all()?
        .to_vec1::<u32>()?;
    if labels.len() != batch {
        return Err(shape_error(
            "cluster_contrastive",
            "label count does not match batch",
        ));
    }
    let vals = embedding
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let mut loss = 0.0f32;
    let mut pairs = 0usize;
    for i in 0..batch {
        for j in (i + 1)..batch {
            let cos = cosine(&vals[i * dim..(i + 1) * dim], &vals[j * dim..(j + 1) * dim]);
            if labels[i] == labels[j] {
                loss += 1.0 - cos;
            } else {
                loss += cos.max(0.0);
            }
            pairs += 1;
        }
    }
    Tensor::new(loss / pairs.max(1) as f32, embedding.device()).map_err(TrainerError::from)
}

pub fn l_operator_match(
    logits: &Tensor,
    gold_probs: &Tensor,
    override_flags: &[bool],
) -> Result<Tensor, TrainerError> {
    if !override_flags.iter().any(|flag| *flag) {
        return Tensor::new(0f32, logits.device()).map_err(TrainerError::from);
    }
    ensure_same_shape(logits, gold_probs, "operator_match")?;
    let dims = logits.dims();
    if dims.len() != 2 || dims[0] != override_flags.len() {
        return Err(shape_error("operator_match", "flags/logits batch mismatch"));
    }
    let batch = dims[0];
    let classes = dims[1];
    let logit_vals = logits
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let gold_vals = gold_probs
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let mut total = 0.0f32;
    let mut count = 0usize;
    for row in 0..batch {
        if !override_flags[row] {
            continue;
        }
        let p = softmax(&logit_vals[row * classes..(row + 1) * classes])?;
        let gold = &gold_vals[row * classes..(row + 1) * classes];
        for (g, pred) in gold.iter().zip(p.iter()) {
            if *g > 0.0 {
                total += g * (g.ln() - pred.max(1e-8).ln());
            }
        }
        count += 1;
    }
    Tensor::new(total / count.max(1) as f32, logits.device()).map_err(TrainerError::from)
}

fn ensure_same_shape(a: &Tensor, b: &Tensor, name: &'static str) -> Result<(), TrainerError> {
    if a.dims() != b.dims() {
        return Err(shape_error(
            name,
            format!("shape {:?} != {:?}", a.dims(), b.dims()),
        ));
    }
    Ok(())
}

fn ce_row(logits: &[f32], label: usize) -> Result<f32, TrainerError> {
    let probs = softmax(logits)?;
    Ok(-probs[label].max(1e-8).ln())
}

fn softmax(logits: &[f32]) -> Result<Vec<f32>, TrainerError> {
    if logits.is_empty() || logits.iter().any(|v| !v.is_finite()) {
        return Err(non_finite("softmax"));
    }
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps = logits.iter().map(|v| (*v - max).exp()).collect::<Vec<_>>();
    let sum = exps.iter().sum::<f32>();
    Ok(exps.iter().map(|v| v / sum.max(1e-8)).collect())
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

fn shape_error(name: &'static str, message: impl Into<String>) -> TrainerError {
    TrainerError::new(
        TrainerErrorCode::MejepaTrainConfigInvalid,
        format!("{name}: {}", message.into()),
    )
}

fn non_finite(name: &'static str) -> TrainerError {
    TrainerError::new(
        TrainerErrorCode::MejepaTrainLossNan,
        format!("{name} contains non-finite input"),
    )
}
