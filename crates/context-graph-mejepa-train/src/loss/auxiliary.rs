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
            let cos = cosine(
                &vals[i * dim..(i + 1) * dim],
                &vals[j * dim..(j + 1) * dim],
                "l_cluster_contrastive",
            )?;
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

/// Fail-closed cosine for auxiliary loss (F-009). Mirrors the gold-standard
/// `cosine_same_dim` in `dda.rs:384` and the trainer-side helper in
/// `sampler::cross_task::cosine_same_dim`.
///
/// Returns:
/// * dim mismatch / empty input → `MejepaTrainPatchSimilarityDimMismatch`
/// * zero or non-finite norm    → `MejepaTrainPatchSimilarityZeroNormVector`
/// * NaN/Inf product            → `MejepaTrainPatchSimilarityZeroNormVector`
///
/// Zero-norm input means a degenerate gradient signal. The auxiliary loss
/// must distinguish "vectors are orthogonal" from "input was malformed", per
/// CLAUDE.md §6.2.
fn cosine(a: &[f32], b: &[f32], context: &str) -> Result<f32, TrainerError> {
    if a.len() != b.len() || a.is_empty() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainPatchSimilarityDimMismatch,
            format!(
                "aux cosine dimension mismatch for {context}: {} vs {} (empty={})",
                a.len(),
                b.len(),
                a.is_empty()
            ),
        )
        .with_context(serde_json::json!({
            "context": context,
            "left_dim": a.len(),
            "right_dim": b.len(),
            "remediation": "ensure both aux-loss row vectors share the same (>0) dimension per CLAUDE.md §6.2"
        })));
    }
    let dot = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || !na.is_finite() || nb == 0.0 || !nb.is_finite() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainPatchSimilarityZeroNormVector,
            format!("aux cosine received zero or non-finite norm vector for {context}"),
        )
        .with_context(serde_json::json!({
            "context": context,
            "left_norm": na,
            "right_norm": nb,
            "remediation": "reject the malformed aux-loss row upstream; do not synthesize a zero-norm embedding"
        })));
    }
    let raw = dot / (na * nb);
    if !raw.is_finite() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainPatchSimilarityZeroNormVector,
            format!("aux cosine produced non-finite value for {context}"),
        )
        .with_context(serde_json::json!({
            "context": context,
            "dot": dot,
            "left_norm": na,
            "right_norm": nb,
            "remediation": "inspect upstream embedding pipeline for NaN/Inf contamination"
        })));
    }
    Ok(raw.clamp(-1.0, 1.0))
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

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn aux_cosine_orthogonal_unit_vectors_real_data() {
        // Real-data check: two real unit vectors that are orthogonal must
        // return cosine = 0.0 (NOT a sentinel — the actual orthogonal value).
        let v = cosine(&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0], "test").expect("orthogonal");
        assert!(v.abs() < 1e-6);
    }

    #[test]
    fn aux_cosine_identical_unit_vectors_real_data() {
        let v = cosine(&[0.6, 0.8], &[0.6, 0.8], "test").expect("identical");
        assert!((v - 1.0).abs() < 1e-6);
    }

    #[test]
    fn aux_cosine_fails_closed_on_dim_mismatch() {
        let err = cosine(&[1.0, 0.0, 0.5], &[0.0, 1.0], "test").expect_err("dim mismatch");
        assert_eq!(err.code(), "MEJEPA_TRAIN_PATCH_SIMILARITY_DIM_MISMATCH");
        assert_eq!(err.context["left_dim"], 3);
        assert_eq!(err.context["right_dim"], 2);
    }

    #[test]
    fn aux_cosine_fails_closed_on_empty_input() {
        let err = cosine(&[], &[], "test").expect_err("empty must fail");
        assert_eq!(err.code(), "MEJEPA_TRAIN_PATCH_SIMILARITY_DIM_MISMATCH");
    }

    #[test]
    fn aux_cosine_fails_closed_on_zero_norm_vector() {
        let err = cosine(&[0.0, 0.0, 0.0], &[1.0, 0.0, 0.0], "test").expect_err("zero norm");
        assert_eq!(err.code(), "MEJEPA_TRAIN_PATCH_SIMILARITY_ZERO_NORM_VECTOR");
    }

    #[test]
    fn aux_cosine_fails_closed_on_nan_input() {
        let err = cosine(&[f32::NAN, 0.0], &[1.0, 0.0], "test").expect_err("NaN must fail");
        assert_eq!(err.code(), "MEJEPA_TRAIN_PATCH_SIMILARITY_ZERO_NORM_VECTOR");
    }

    #[test]
    fn l_cluster_contrastive_propagates_zero_norm_error() {
        // Two-row batch where row 1 is a zero vector — l_cluster_contrastive
        // must NOT silently treat that as orthogonal.
        let embedding = Tensor::from_vec(
            vec![1.0_f32, 0.0, 0.0, 0.0_f32, 0.0, 0.0],
            (2, 3),
            &Device::Cpu,
        )
        .expect("build tensor");
        let labels = Tensor::from_vec(vec![0_u32, 0_u32], 2, &Device::Cpu).expect("labels");
        let err = l_cluster_contrastive(&embedding, &labels)
            .expect_err("zero-norm row must surface as structured error");
        assert_eq!(err.code(), "MEJEPA_TRAIN_PATCH_SIMILARITY_ZERO_NORM_VECTOR");
    }

    #[test]
    fn l_cluster_contrastive_real_data_happy_path() {
        // Two real rows, same class, non-degenerate. Loss must be the
        // (1 - cos_sim) average for the only pair.
        let v1 = [1.0_f32, 0.0, 0.0];
        let v2 = [0.6_f32, 0.8, 0.0];
        let expected_cos = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum::<f32>()
            / (v1.iter().map(|a| a * a).sum::<f32>().sqrt()
                * v2.iter().map(|a| a * a).sum::<f32>().sqrt());
        let expected_loss = 1.0 - expected_cos;
        let embedding = Tensor::from_vec(
            vec![v1[0], v1[1], v1[2], v2[0], v2[1], v2[2]],
            (2, 3),
            &Device::Cpu,
        )
        .expect("tensor");
        let labels = Tensor::from_vec(vec![0_u32, 0_u32], 2, &Device::Cpu).expect("labels");
        let loss = l_cluster_contrastive(&embedding, &labels).expect("real data");
        let actual = loss.to_scalar::<f32>().expect("scalar");
        assert!(
            (actual - expected_loss).abs() < 1e-6,
            "expected {expected_loss}, got {actual}"
        );
    }

    #[test]
    fn regression_silent_zero_returns_no_longer_happen() {
        // F-009 regression: if `cosine` silently returned 0.0 on zero-norm
        // input, l_cluster_contrastive would compute loss = (1 - 0.0) = 1.0
        // for same-class rows. The new fail-closed contract MUST surface an
        // error instead — proving the silent path is dead.
        let embedding = Tensor::from_vec(
            vec![0.0_f32, 0.0, 0.0, 0.0_f32, 0.0, 0.0],
            (2, 3),
            &Device::Cpu,
        )
        .expect("tensor");
        let labels = Tensor::from_vec(vec![1_u32, 1_u32], 2, &Device::Cpu).expect("labels");
        let outcome = l_cluster_contrastive(&embedding, &labels);
        assert!(outcome.is_err(), "silent 0.0 path must be dead");
        let err = outcome.unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TRAIN_PATCH_SIMILARITY_ZERO_NORM_VECTOR");
    }
}
