use candle_core::{DType, Tensor};
use context_graph_mejepa_instruments::{InstrumentSlot, PANEL_DIM};
use serde::{Deserialize, Serialize};

use crate::error::MejepaInferError;

pub const NO_COMPENSATION_TRACE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoCompensationTrace {
    pub schema_version: u32,
    pub aggregate_l2_threshold: f32,
    pub slot_l2_threshold: f32,
    pub aggregate_l2: f32,
    pub aggregate_mse: f32,
    pub aggregate_passed: bool,
    pub strict_passed: bool,
    pub violated_slots: Vec<String>,
    pub slots: Vec<SlotResidualScore>,
    pub pairwise_scores_used: Vec<PairwiseResidualContrast>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotResidualScore {
    pub slot_id: String,
    pub offset: usize,
    pub dim: usize,
    pub l2: f32,
    pub mse: f32,
    pub max_abs: f32,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairwiseResidualContrast {
    pub left_slot_id: String,
    pub right_slot_id: String,
    pub normalized_l2_delta: f32,
    pub high_contrast: bool,
}

pub fn build_no_compensation_trace(
    predicted: &[f32],
    target: &[f32],
    aggregate_l2_threshold: f32,
    slot_l2_threshold: f32,
) -> Result<NoCompensationTrace, MejepaInferError> {
    validate_trace_inputs(predicted, target, aggregate_l2_threshold, slot_l2_threshold)?;
    let aggregate_sq = predicted
        .iter()
        .zip(target)
        .map(|(predicted, target)| {
            let delta = predicted - target;
            delta * delta
        })
        .sum::<f32>();
    let aggregate_l2 = aggregate_sq.sqrt();
    let aggregate_mse = aggregate_sq / PANEL_DIM as f32;
    let mut slots = Vec::with_capacity(InstrumentSlot::all().len());
    for slot in InstrumentSlot::all() {
        slots.push(score_slot(slot, predicted, target, slot_l2_threshold)?);
    }
    let violated_slots = slots
        .iter()
        .filter(|slot| !slot.passed)
        .map(|slot| slot.slot_id.clone())
        .collect::<Vec<_>>();
    let aggregate_passed = aggregate_l2 <= aggregate_l2_threshold;
    let strict_passed = aggregate_passed && violated_slots.is_empty();
    let pairwise_scores_used = pairwise_residual_contrasts(&slots, slot_l2_threshold);
    Ok(NoCompensationTrace {
        schema_version: NO_COMPENSATION_TRACE_SCHEMA_VERSION,
        aggregate_l2_threshold,
        slot_l2_threshold,
        aggregate_l2,
        aggregate_mse,
        aggregate_passed,
        strict_passed,
        violated_slots,
        slots,
        pairwise_scores_used,
    })
}

pub fn build_no_compensation_trace_from_tensors(
    predicted: &Tensor,
    target: &Tensor,
    batch_index: usize,
    aggregate_l2_threshold: f32,
    slot_l2_threshold: f32,
) -> Result<NoCompensationTrace, MejepaInferError> {
    validate_panel_tensor(predicted, "predicted")?;
    validate_panel_tensor(target, "target")?;
    if predicted.dims()[0] != target.dims()[0] {
        return Err(MejepaInferError::DimMismatch {
            expected: predicted.dims()[0],
            actual: target.dims()[0],
            context: "no-compensation predicted/target batch size".to_string(),
        });
    }
    if batch_index >= predicted.dims()[0] {
        return Err(MejepaInferError::DimMismatch {
            expected: predicted.dims()[0],
            actual: batch_index + 1,
            context: "no-compensation batch_index".to_string(),
        });
    }
    let predicted_values = predicted
        .to_dtype(DType::F32)
        .map_err(|err| tensor_conversion_error("predicted.to_dtype", err))?
        .flatten_all()
        .map_err(|err| tensor_conversion_error("predicted.flatten_all", err))?
        .to_vec1::<f32>()
        .map_err(|err| tensor_conversion_error("predicted.to_vec1", err))?;
    let target_values = target
        .to_dtype(DType::F32)
        .map_err(|err| tensor_conversion_error("target.to_dtype", err))?
        .flatten_all()
        .map_err(|err| tensor_conversion_error("target.flatten_all", err))?
        .to_vec1::<f32>()
        .map_err(|err| tensor_conversion_error("target.to_vec1", err))?;
    let start = batch_index * PANEL_DIM;
    let end = start + PANEL_DIM;
    build_no_compensation_trace(
        &predicted_values[start..end],
        &target_values[start..end],
        aggregate_l2_threshold,
        slot_l2_threshold,
    )
}

fn score_slot(
    slot: InstrumentSlot,
    predicted: &[f32],
    target: &[f32],
    slot_l2_threshold: f32,
) -> Result<SlotResidualScore, MejepaInferError> {
    let (offset, dim) = slot.extent();
    let predicted =
        predicted
            .get(offset..offset + dim)
            .ok_or_else(|| MejepaInferError::DimMismatch {
                expected: PANEL_DIM,
                actual: predicted.len(),
                context: format!("predicted slice for slot {}", slot.slug()),
            })?;
    let target = target
        .get(offset..offset + dim)
        .ok_or_else(|| MejepaInferError::DimMismatch {
            expected: PANEL_DIM,
            actual: target.len(),
            context: format!("target slice for slot {}", slot.slug()),
        })?;
    let mut sq = 0.0f32;
    let mut max_abs = 0.0f32;
    for (predicted, target) in predicted.iter().zip(target) {
        let delta = predicted - target;
        sq += delta * delta;
        max_abs = max_abs.max(delta.abs());
    }
    let l2 = sq.sqrt();
    Ok(SlotResidualScore {
        slot_id: slot.slug().to_string(),
        offset,
        dim,
        l2,
        mse: sq / dim as f32,
        max_abs,
        passed: l2 <= slot_l2_threshold,
    })
}

fn pairwise_residual_contrasts(
    slots: &[SlotResidualScore],
    slot_l2_threshold: f32,
) -> Vec<PairwiseResidualContrast> {
    let mut pairs =
        Vec::with_capacity(slots.len().saturating_mul(slots.len().saturating_sub(1)) / 2);
    for left_idx in 0..slots.len() {
        for right_idx in (left_idx + 1)..slots.len() {
            let left = &slots[left_idx];
            let right = &slots[right_idx];
            let left_norm = left.l2 / (left.dim as f32).sqrt().max(1.0);
            let right_norm = right.l2 / (right.dim as f32).sqrt().max(1.0);
            let normalized_l2_delta = (left_norm - right_norm).abs();
            pairs.push(PairwiseResidualContrast {
                left_slot_id: left.slot_id.clone(),
                right_slot_id: right.slot_id.clone(),
                normalized_l2_delta,
                high_contrast: normalized_l2_delta > slot_l2_threshold,
            });
        }
    }
    pairs
}

fn validate_trace_inputs(
    predicted: &[f32],
    target: &[f32],
    aggregate_l2_threshold: f32,
    slot_l2_threshold: f32,
) -> Result<(), MejepaInferError> {
    if predicted.len() != PANEL_DIM {
        return Err(MejepaInferError::DimMismatch {
            expected: PANEL_DIM,
            actual: predicted.len(),
            context: "no-compensation predicted panel".to_string(),
        });
    }
    if target.len() != PANEL_DIM {
        return Err(MejepaInferError::DimMismatch {
            expected: PANEL_DIM,
            actual: target.len(),
            context: "no-compensation target panel".to_string(),
        });
    }
    if !aggregate_l2_threshold.is_finite() || aggregate_l2_threshold <= 0.0 {
        return Err(MejepaInferError::InvalidInput {
            field: "aggregate_l2_threshold".to_string(),
            detail: "must be finite and > 0".to_string(),
        });
    }
    if !slot_l2_threshold.is_finite() || slot_l2_threshold <= 0.0 {
        return Err(MejepaInferError::InvalidInput {
            field: "slot_l2_threshold".to_string(),
            detail: "must be finite and > 0".to_string(),
        });
    }
    if predicted.iter().any(|value| !value.is_finite()) {
        return Err(MejepaInferError::NanDetected {
            nan_source: "no_compensation_trace.predicted".to_string(),
            detail: "predicted panel contains NaN or Inf".to_string(),
        });
    }
    if target.iter().any(|value| !value.is_finite()) {
        return Err(MejepaInferError::NanDetected {
            nan_source: "no_compensation_trace.target".to_string(),
            detail: "target panel contains NaN or Inf".to_string(),
        });
    }
    Ok(())
}

fn validate_panel_tensor(tensor: &Tensor, field: &str) -> Result<(), MejepaInferError> {
    let dims = tensor.dims();
    if dims.len() != 2 || dims[1] != PANEL_DIM {
        return Err(MejepaInferError::DimMismatch {
            expected: PANEL_DIM,
            actual: dims.get(1).copied().unwrap_or(0),
            context: format!("no-compensation {field} tensor"),
        });
    }
    if dims[0] == 0 {
        return Err(MejepaInferError::DimMismatch {
            expected: 1,
            actual: 0,
            context: format!("no-compensation {field} batch"),
        });
    }
    Ok(())
}

fn tensor_conversion_error(field: &str, err: candle_core::Error) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: format!("no_compensation_trace.{field}"),
        detail: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};

    #[test]
    fn one_bad_slot_cannot_hide_behind_passing_aggregate() {
        let target = vec![0.0f32; PANEL_DIM];
        let mut predicted = target.clone();
        let (offset, dim) = InstrumentSlot::EOracle.extent();
        for value in &mut predicted[offset..offset + dim] {
            *value = 0.10;
        }
        let trace = build_no_compensation_trace(&predicted, &target, 2.0, 0.5).unwrap();
        assert!(trace.aggregate_passed);
        assert!(!trace.strict_passed);
        assert_eq!(trace.violated_slots, vec!["e_oracle".to_string()]);
        assert_eq!(trace.slots.len(), InstrumentSlot::all().len());
        assert_eq!(
            trace.pairwise_scores_used.len(),
            InstrumentSlot::all().len() * (InstrumentSlot::all().len() - 1) / 2
        );
    }

    #[test]
    fn non_finite_panels_fail_closed() {
        let target = vec![0.0f32; PANEL_DIM];
        let mut predicted = target.clone();
        predicted[0] = f32::NAN;
        let err = build_no_compensation_trace(&predicted, &target, 2.0, 0.5).unwrap_err();
        assert!(err.to_string().contains("MEJEPA_INFER_NAN_DETECTED"));
    }

    #[test]
    fn tensor_trace_extracts_requested_batch() {
        let device = Device::Cpu;
        let target = vec![0.0f32; PANEL_DIM * 2];
        let mut predicted = target.clone();
        let (offset, dim) = InstrumentSlot::EOracle.extent();
        let batch_one_offset = PANEL_DIM + offset;
        for value in &mut predicted[batch_one_offset..batch_one_offset + dim] {
            *value = 0.10;
        }
        let predicted = Tensor::from_slice(&predicted, (2, PANEL_DIM), &device).unwrap();
        let target = Tensor::from_slice(&target, (2, PANEL_DIM), &device).unwrap();
        let trace =
            build_no_compensation_trace_from_tensors(&predicted, &target, 1, 2.0, 0.5).unwrap();
        assert!(trace.aggregate_passed);
        assert!(!trace.strict_passed);
        assert_eq!(trace.violated_slots, vec!["e_oracle".to_string()]);
    }
}
