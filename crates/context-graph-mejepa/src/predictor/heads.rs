use candle_core::Tensor;
use candle_nn::{linear, ops, Linear, Module, VarBuilder};
use std::panic::AssertUnwindSafe;

use crate::config::PANEL_DIM;
use crate::data_models::PredictedPanel;
use crate::error::{NanSource, PredictorError};
use crate::predictor::ensure_finite;
use crate::types::HeadId;

pub const FAILURE_MODE_CLASSES: usize = 36;
pub const EDGE_CASE_CLASSES: usize = 23;
pub const TECH_DEBT_CLASSES: usize = 21;
pub const PERF_CLASSES: usize = 17;
pub const SECURITY_CLASSES: usize = 20;
pub const ACCURACY_CLASSES: usize = 13;
pub const COST_CLASSES: usize = 11;
pub const REASONING_CLASSES: usize = 8;

#[derive(Debug)]
pub struct AuxiliaryHeads {
    failure_mode: Linear,
    edge_case: Linear,
    tech_debt: Linear,
    perf: Linear,
    security: Linear,
    accuracy: Linear,
    cost: Linear,
    reasoning: Linear,
}

#[derive(Debug, Clone)]
pub struct HeadPrediction {
    pub head_id: HeadId,
    pub logits: Tensor,
    pub probabilities: Tensor,
    pub class_count: usize,
}

#[derive(Debug, Clone)]
pub struct AllHeadOutputs {
    pub predicted_panel: PredictedPanel,
    pub failure_mode: HeadPrediction,
    pub edge_case: HeadPrediction,
    pub tech_debt: HeadPrediction,
    pub perf: HeadPrediction,
    pub security: HeadPrediction,
    pub accuracy: HeadPrediction,
    pub cost: HeadPrediction,
    pub reasoning: HeadPrediction,
}

impl AuxiliaryHeads {
    pub fn new(vb: VarBuilder) -> Result<Self, PredictorError> {
        Ok(Self {
            failure_mode: linear(PANEL_DIM, FAILURE_MODE_CLASSES, vb.pp("failure_mode"))?,
            edge_case: linear(PANEL_DIM, EDGE_CASE_CLASSES, vb.pp("edge_case"))?,
            tech_debt: linear(PANEL_DIM, TECH_DEBT_CLASSES, vb.pp("tech_debt"))?,
            perf: linear(PANEL_DIM, PERF_CLASSES, vb.pp("perf"))?,
            security: linear(PANEL_DIM, SECURITY_CLASSES, vb.pp("security"))?,
            accuracy: linear(PANEL_DIM, ACCURACY_CLASSES, vb.pp("accuracy"))?,
            cost: linear(PANEL_DIM, COST_CLASSES, vb.pp("cost"))?,
            reasoning: linear(PANEL_DIM, REASONING_CLASSES, vb.pp("reasoning"))?,
        })
    }

    pub fn forward(
        &self,
        predicted: &PredictedPanel,
    ) -> Result<AuxiliaryHeadOutputs, PredictorError> {
        validate_predicted_panel(predicted)?;
        Ok(AuxiliaryHeadOutputs {
            failure_mode: run_head_fail_closed(HeadId::FailureMode, || {
                project_head(
                    HeadId::FailureMode,
                    &self.failure_mode,
                    predicted,
                    FAILURE_MODE_CLASSES,
                )
            })?,
            edge_case: run_head_fail_closed(HeadId::EdgeCase, || {
                project_head(HeadId::EdgeCase, &self.edge_case, predicted, EDGE_CASE_CLASSES)
            })?,
            tech_debt: run_head_fail_closed(HeadId::TechDebt, || {
                project_head(HeadId::TechDebt, &self.tech_debt, predicted, TECH_DEBT_CLASSES)
            })?,
            perf: run_head_fail_closed(HeadId::Perf, || {
                project_head(HeadId::Perf, &self.perf, predicted, PERF_CLASSES)
            })?,
            security: run_head_fail_closed(HeadId::Security, || {
                project_head(HeadId::Security, &self.security, predicted, SECURITY_CLASSES)
            })?,
            accuracy: run_head_fail_closed(HeadId::Accuracy, || {
                project_head(HeadId::Accuracy, &self.accuracy, predicted, ACCURACY_CLASSES)
            })?,
            cost: run_head_fail_closed(HeadId::Cost, || {
                project_head(HeadId::Cost, &self.cost, predicted, COST_CLASSES)
            })?,
            reasoning: run_head_fail_closed(HeadId::Reasoning, || {
                project_head(HeadId::Reasoning, &self.reasoning, predicted, REASONING_CLASSES)
            })?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AuxiliaryHeadOutputs {
    pub failure_mode: HeadPrediction,
    pub edge_case: HeadPrediction,
    pub tech_debt: HeadPrediction,
    pub perf: HeadPrediction,
    pub security: HeadPrediction,
    pub accuracy: HeadPrediction,
    pub cost: HeadPrediction,
    pub reasoning: HeadPrediction,
}

fn project_head(
    head_id: HeadId,
    projection: &Linear,
    predicted: &PredictedPanel,
    class_count: usize,
) -> Result<HeadPrediction, PredictorError> {
    let logits = projection.forward(&predicted.tensor)?;
    if logits.dims() != [predicted.batch_size, class_count] {
        return Err(PredictorError::DimMismatch {
            detail: format!(
                "{} logits expected (B, {class_count}); got {:?}",
                head_id.as_str(),
                logits.dims()
            ),
            observed: serde_json::json!({
                "head": head_id.as_str(),
                "logits": logits.dims()
            }),
            expected_panel_dim: PANEL_DIM,
        });
    }
    ensure_finite(
        &logits,
        NanSource::OracleHead,
        None,
        &format!("{}_logits", head_id.as_str()),
    )?;
    let probabilities = ops::softmax(&logits, 1)?;
    ensure_finite(
        &probabilities,
        NanSource::OracleHead,
        None,
        &format!("{}_probabilities", head_id.as_str()),
    )?;
    Ok(HeadPrediction {
        head_id,
        logits,
        probabilities,
        class_count,
    })
}

fn run_head_fail_closed<F>(head_id: HeadId, run: F) -> Result<HeadPrediction, PredictorError>
where
    F: FnOnce() -> Result<HeadPrediction, PredictorError>,
{
    match std::panic::catch_unwind(AssertUnwindSafe(run)) {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(err)) => Err(PredictorError::HeadFailure {
            head: head_id.as_str().to_string(),
            code: err.code().to_string(),
            detail: err.to_string(),
        }),
        Err(payload) => Err(PredictorError::HeadFailure {
            head: head_id.as_str().to_string(),
            code: "MEJEPA_HEAD_PANIC".to_string(),
            detail: panic_payload_to_string(payload),
        }),
    }
}

fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "head panicked with non-string payload".to_string()
    }
}

fn validate_predicted_panel(predicted: &PredictedPanel) -> Result<(), PredictorError> {
    if predicted.batch_size == 0
        || predicted.panel_dim != PANEL_DIM
        || predicted.tensor.dims() != [predicted.batch_size, PANEL_DIM]
    {
        return Err(PredictorError::DimMismatch {
            detail: format!(
                "auxiliary heads expect predicted panel shape (B, {PANEL_DIM}); got {:?}",
                predicted.tensor.dims()
            ),
            observed: serde_json::json!({ "predicted_panel": predicted.tensor.dims() }),
            expected_panel_dim: PANEL_DIM,
        });
    }
    Ok(())
}
