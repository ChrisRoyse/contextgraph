pub mod auxiliary;
pub mod constellation;
pub mod counterfactual;
pub mod entropy;
pub mod inverse;
pub mod predict;
pub mod sigreg;
pub mod vicreg;

use crate::config::{LossCoefficients, Q4_LOSS_COEFFICIENT_MAX};
use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::Tensor;
use serde_json::json;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct AuxOutputs {
    pub warnings: Tensor,
    pub runtime_log: Tensor,
    pub rss: Tensor,
    pub traceback_cluster_logits: Tensor,
    pub hunk_verdicts: Tensor,
    pub adjacent_outcomes: Tensor,
    pub reasoning_embedding: Tensor,
    pub operator_match: Tensor,
}

#[derive(Debug, Clone)]
pub struct BatchMetadata {
    pub actual_warnings: Tensor,
    pub actual_runtime_log: Tensor,
    pub actual_rss: Tensor,
    pub actual_traceback_cluster_id: Tensor,
    pub actual_hunk_verdicts: Tensor,
    pub actual_adjacent_outcomes: Tensor,
    pub actual_cot_cluster_id: Tensor,
    pub override_flags: Vec<bool>,
    pub override_gold_labels: Tensor,
    pub current_task_centroids: Option<Vec<Tensor>>,
}

#[derive(Debug, Clone)]
pub struct LossBreakdown {
    pub total: f32,
    pub total_tensor: Tensor,
    pub components: HashMap<String, f32>,
}

pub fn compose_l_full(
    predicted_latent: &Tensor,
    target_latent: &Tensor,
    aux_outputs: &AuxOutputs,
    batch_metadata: &BatchMetadata,
    coefficients: &LossCoefficients,
) -> Result<LossBreakdown, TrainerError> {
    let l_predict = predict::l_predict(predicted_latent, target_latent)?;
    let l_variance = vicreg::l_variance(predicted_latent)?;
    let l_covariance = vicreg::l_covariance(predicted_latent)?;
    let l_invariance = vicreg::l_invariance(predicted_latent, predicted_latent)?;
    let l_sigreg = sigreg::sigreg_loss(predicted_latent, sigreg::SigregConfig::default())?;
    let l_warnings = auxiliary::l_mse(&aux_outputs.warnings, &batch_metadata.actual_warnings)?;
    let l_runtime = auxiliary::l_mse(&aux_outputs.runtime_log, &batch_metadata.actual_runtime_log)?;
    let l_rss = auxiliary::l_mse(&aux_outputs.rss, &batch_metadata.actual_rss)?;
    let l_traceback = auxiliary::l_cluster_ce(
        &aux_outputs.traceback_cluster_logits,
        &batch_metadata.actual_traceback_cluster_id,
    )?;
    let l_hunk = auxiliary::l_binary_ce(
        &aux_outputs.hunk_verdicts,
        &batch_metadata.actual_hunk_verdicts,
    )?;
    let l_collateral = auxiliary::l_binary_ce(
        &aux_outputs.adjacent_outcomes,
        &batch_metadata.actual_adjacent_outcomes,
    )?;
    let l_reasoning = auxiliary::l_cluster_contrastive(
        &aux_outputs.reasoning_embedding,
        &batch_metadata.actual_cot_cluster_id,
    )?;
    let l_operator = auxiliary::l_operator_match(
        &aux_outputs.operator_match,
        &batch_metadata.override_gold_labels,
        &batch_metadata.override_flags,
    )?;
    let l_constellation = constellation::l_constellation_centroid(
        predicted_latent,
        batch_metadata.current_task_centroids.as_deref(),
    )?;
    let (l_entropy, entropy_report) = entropy::latent_entropy_loss(
        predicted_latent,
        entropy::LatentEntropyConfig {
            lambda: coefficients.lambda_entropy,
            ..entropy::LatentEntropyConfig::default()
        },
    )?;

    let effective_alpha_reasoning = coefficients
        .alpha_reasoning
        .clamp(0.0, Q4_LOSS_COEFFICIENT_MAX);
    let weighted = [
        ("predict", 1.0, &l_predict),
        ("variance", coefficients.lambda_var, &l_variance),
        ("covariance", coefficients.lambda_cov, &l_covariance),
        ("invariance", coefficients.lambda_inv, &l_invariance),
        ("sigreg", coefficients.lambda_sigreg, &l_sigreg),
        ("warnings", coefficients.alpha_warn, &l_warnings),
        ("runtime", coefficients.alpha_runtime, &l_runtime),
        ("rss", coefficients.alpha_rss, &l_rss),
        ("traceback", coefficients.alpha_trace, &l_traceback),
        ("hunk_attribution", coefficients.alpha_hunk, &l_hunk),
        ("collateral", coefficients.alpha_adjacent, &l_collateral),
        ("reasoning", effective_alpha_reasoning, &l_reasoning),
        ("operator_match", coefficients.alpha_overrides, &l_operator),
        (
            "constellation_centroid",
            coefficients.delta,
            &l_constellation,
        ),
        ("entropy", coefficients.lambda_entropy, &l_entropy),
    ];
    let mut total_tensor = Tensor::new(0f32, predicted_latent.device())?;
    let mut components = HashMap::new();
    for (name, coeff, tensor) in weighted {
        let scalar = scalar(tensor)?;
        components.insert(name.to_string(), scalar);
        components.insert(format!("{name}_weighted"), scalar * coeff);
        total_tensor = (&total_tensor + &tensor.affine(coeff as f64, 0.0)?)?;
    }
    components.insert("entropy_lambda".to_string(), entropy_report.lambda);
    let total = scalar(&total_tensor)?;
    if !total.is_finite() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainLossNan,
            format!("L_full is non-finite: {total}"),
        )
        .with_context(json!({"components": components})));
    }
    Ok(LossBreakdown {
        total,
        total_tensor,
        components,
    })
}

pub(crate) fn scalar(tensor: &Tensor) -> Result<f32, TrainerError> {
    Ok(tensor
        .to_dtype(candle_core::DType::F32)?
        .mean_all()?
        .to_scalar::<f32>()?)
}

#[allow(dead_code)]
pub(crate) fn finite_tensor(tensor: &Tensor, field: &'static str) -> Result<(), TrainerError> {
    let vals = tensor
        .to_dtype(candle_core::DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    if vals.iter().any(|v| !v.is_finite()) {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainLossNan,
            format!("{field} contains NaN or Inf"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};

    #[test]
    fn compose_has_all_components() {
        let device = Device::Cpu;
        let p = Tensor::from_slice(&[0f32, 1., 1., 0.], (2, 2), &device).unwrap();
        let t = p.clone();
        let aux = AuxOutputs {
            warnings: Tensor::zeros((2, 1), candle_core::DType::F32, &device).unwrap(),
            runtime_log: Tensor::zeros((2, 1), candle_core::DType::F32, &device).unwrap(),
            rss: Tensor::zeros((2, 1), candle_core::DType::F32, &device).unwrap(),
            traceback_cluster_logits: Tensor::zeros((2, 2), candle_core::DType::F32, &device)
                .unwrap(),
            hunk_verdicts: Tensor::zeros((2, 1), candle_core::DType::F32, &device).unwrap(),
            adjacent_outcomes: Tensor::zeros((2, 1), candle_core::DType::F32, &device).unwrap(),
            reasoning_embedding: p.clone(),
            operator_match: Tensor::zeros((2, 2), candle_core::DType::F32, &device).unwrap(),
        };
        let meta = BatchMetadata {
            actual_warnings: aux.warnings.clone(),
            actual_runtime_log: aux.runtime_log.clone(),
            actual_rss: aux.rss.clone(),
            actual_traceback_cluster_id: Tensor::from_slice(&[0u32, 1], 2, &device).unwrap(),
            actual_hunk_verdicts: aux.hunk_verdicts.clone(),
            actual_adjacent_outcomes: aux.adjacent_outcomes.clone(),
            actual_cot_cluster_id: Tensor::from_slice(&[0u32, 1], 2, &device).unwrap(),
            override_flags: vec![false, false],
            override_gold_labels: Tensor::zeros((2, 2), candle_core::DType::F32, &device).unwrap(),
            current_task_centroids: Some(vec![p.clone()]),
        };
        let loss = compose_l_full(&p, &t, &aux, &meta, &LossCoefficients::default()).unwrap();
        assert_eq!(loss.components.len(), 31);
        assert!(loss.components["sigreg"].is_finite());
        assert_eq!(loss.components["entropy"], 0.0);
        assert_eq!(loss.components["entropy_weighted"], 0.0);
        assert!(loss.total.is_finite());
    }

    #[test]
    fn direct_compose_clamps_q4_reasoning_loss_under_freeze() {
        let device = Device::Cpu;
        let p = Tensor::from_slice(&[1f32, 0., 1., 0.], (2, 2), &device).unwrap();
        let t = p.clone();
        let aux = AuxOutputs {
            warnings: Tensor::zeros((2, 1), candle_core::DType::F32, &device).unwrap(),
            runtime_log: Tensor::zeros((2, 1), candle_core::DType::F32, &device).unwrap(),
            rss: Tensor::zeros((2, 1), candle_core::DType::F32, &device).unwrap(),
            traceback_cluster_logits: Tensor::zeros((2, 2), candle_core::DType::F32, &device)
                .unwrap(),
            hunk_verdicts: Tensor::zeros((2, 1), candle_core::DType::F32, &device).unwrap(),
            adjacent_outcomes: Tensor::zeros((2, 1), candle_core::DType::F32, &device).unwrap(),
            reasoning_embedding: p.clone(),
            operator_match: Tensor::zeros((2, 2), candle_core::DType::F32, &device).unwrap(),
        };
        let meta = BatchMetadata {
            actual_warnings: aux.warnings.clone(),
            actual_runtime_log: aux.runtime_log.clone(),
            actual_rss: aux.rss.clone(),
            actual_traceback_cluster_id: Tensor::from_slice(&[0u32, 1], 2, &device).unwrap(),
            actual_hunk_verdicts: aux.hunk_verdicts.clone(),
            actual_adjacent_outcomes: aux.adjacent_outcomes.clone(),
            actual_cot_cluster_id: Tensor::from_slice(&[0u32, 1], 2, &device).unwrap(),
            override_flags: vec![false, false],
            override_gold_labels: Tensor::zeros((2, 2), candle_core::DType::F32, &device).unwrap(),
            current_task_centroids: Some(vec![p.clone()]),
        };
        let base = compose_l_full(&p, &t, &aux, &meta, &LossCoefficients::default()).unwrap();
        let forced_q4 = compose_l_full(
            &p,
            &t,
            &aux,
            &meta,
            &LossCoefficients {
                alpha_reasoning: 10.0,
                ..LossCoefficients::default()
            },
        )
        .unwrap();
        assert!(base.components["reasoning"] > 0.0);
        assert!((base.total - forced_q4.total).abs() < f32::EPSILON);
    }

    #[test]
    fn entropy_loss_adds_weighted_component_when_enabled() {
        let device = Device::Cpu;
        let p =
            Tensor::from_slice(&[0.5f32, 0.25, 2., 1., 4., 4., 6., 9.], (4, 2), &device).unwrap();
        let t = p.clone();
        let aux = AuxOutputs {
            warnings: Tensor::zeros((4, 1), candle_core::DType::F32, &device).unwrap(),
            runtime_log: Tensor::zeros((4, 1), candle_core::DType::F32, &device).unwrap(),
            rss: Tensor::zeros((4, 1), candle_core::DType::F32, &device).unwrap(),
            traceback_cluster_logits: Tensor::zeros((4, 2), candle_core::DType::F32, &device)
                .unwrap(),
            hunk_verdicts: Tensor::zeros((4, 1), candle_core::DType::F32, &device).unwrap(),
            adjacent_outcomes: Tensor::zeros((4, 1), candle_core::DType::F32, &device).unwrap(),
            reasoning_embedding: p.clone(),
            operator_match: Tensor::zeros((4, 2), candle_core::DType::F32, &device).unwrap(),
        };
        let meta = BatchMetadata {
            actual_warnings: aux.warnings.clone(),
            actual_runtime_log: aux.runtime_log.clone(),
            actual_rss: aux.rss.clone(),
            actual_traceback_cluster_id: Tensor::from_slice(&[0u32, 1, 0, 1], 4, &device).unwrap(),
            actual_hunk_verdicts: aux.hunk_verdicts.clone(),
            actual_adjacent_outcomes: aux.adjacent_outcomes.clone(),
            actual_cot_cluster_id: Tensor::from_slice(&[0u32, 1, 0, 1], 4, &device).unwrap(),
            override_flags: vec![false, false, false, false],
            override_gold_labels: Tensor::zeros((4, 2), candle_core::DType::F32, &device).unwrap(),
            current_task_centroids: Some(vec![p.clone()]),
        };
        let loss = compose_l_full(
            &p,
            &t,
            &aux,
            &meta,
            &LossCoefficients {
                lambda_entropy: 1e-4,
                ..LossCoefficients::default()
            },
        )
        .unwrap();
        assert!(loss.components["entropy"] > 0.0);
        assert!(loss.components["entropy_weighted"] > 0.0);
        assert_eq!(loss.components["entropy_lambda"], 1e-4);
    }
}
