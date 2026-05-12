use candle_core::Tensor;
use candle_nn::{linear, ops, Linear, Module, VarBuilder};

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
            failure_mode: project_head(
                HeadId::FailureMode,
                &self.failure_mode,
                predicted,
                FAILURE_MODE_CLASSES,
            )?,
            edge_case: project_head(
                HeadId::EdgeCase,
                &self.edge_case,
                predicted,
                EDGE_CASE_CLASSES,
            )?,
            tech_debt: project_head(
                HeadId::TechDebt,
                &self.tech_debt,
                predicted,
                TECH_DEBT_CLASSES,
            )?,
            perf: project_head(HeadId::Perf, &self.perf, predicted, PERF_CLASSES)?,
            security: project_head(
                HeadId::Security,
                &self.security,
                predicted,
                SECURITY_CLASSES,
            )?,
            accuracy: project_head(
                HeadId::Accuracy,
                &self.accuracy,
                predicted,
                ACCURACY_CLASSES,
            )?,
            cost: project_head(HeadId::Cost, &self.cost, predicted, COST_CLASSES)?,
            reasoning: project_head(
                HeadId::Reasoning,
                &self.reasoning,
                predicted,
                REASONING_CLASSES,
            )?,
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
