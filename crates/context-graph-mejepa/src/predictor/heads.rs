use candle_core::Tensor;
use candle_nn::{linear, ops, Linear, Module, VarBuilder};
use context_graph_mejepa_instruments::InstrumentSlot;
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
    failure_mode: SlotAwareHead,
    edge_case: SlotAwareHead,
    tech_debt: SlotAwareHead,
    perf: SlotAwareHead,
    security: SlotAwareHead,
    accuracy: SlotAwareHead,
    cost: SlotAwareHead,
    reasoning: SlotAwareHead,
}

#[derive(Debug)]
struct SlotAwareHead {
    head_id: HeadId,
    class_count: usize,
    slot_projections: Vec<SlotHeadProjection>,
    late_projection: Linear,
}

#[derive(Debug)]
struct SlotHeadProjection {
    slot: InstrumentSlot,
    projection: Linear,
}

#[derive(Debug, Clone)]
pub struct HeadPrediction {
    pub head_id: HeadId,
    pub logits: Tensor,
    pub probabilities: Tensor,
    pub class_count: usize,
    pub slot_logits: Vec<SlotLogitContribution>,
}

#[derive(Debug, Clone)]
pub struct SlotLogitContribution {
    pub slot_id: String,
    pub logits: Tensor,
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
            failure_mode: SlotAwareHead::new(
                HeadId::FailureMode,
                FAILURE_MODE_CLASSES,
                vb.pp("failure_mode"),
            )?,
            edge_case: SlotAwareHead::new(HeadId::EdgeCase, EDGE_CASE_CLASSES, vb.pp("edge_case"))?,
            tech_debt: SlotAwareHead::new(HeadId::TechDebt, TECH_DEBT_CLASSES, vb.pp("tech_debt"))?,
            perf: SlotAwareHead::new(HeadId::Perf, PERF_CLASSES, vb.pp("perf"))?,
            security: SlotAwareHead::new(HeadId::Security, SECURITY_CLASSES, vb.pp("security"))?,
            accuracy: SlotAwareHead::new(HeadId::Accuracy, ACCURACY_CLASSES, vb.pp("accuracy"))?,
            cost: SlotAwareHead::new(HeadId::Cost, COST_CLASSES, vb.pp("cost"))?,
            reasoning: SlotAwareHead::new(
                HeadId::Reasoning,
                REASONING_CLASSES,
                vb.pp("reasoning"),
            )?,
        })
    }

    pub fn forward(
        &self,
        predicted: &PredictedPanel,
    ) -> Result<AuxiliaryHeadOutputs, PredictorError> {
        validate_predicted_panel(predicted)?;
        Ok(AuxiliaryHeadOutputs {
            failure_mode: run_head_fail_closed(HeadId::FailureMode, || {
                self.failure_mode.forward(predicted)
            })?,
            edge_case: run_head_fail_closed(HeadId::EdgeCase, || {
                self.edge_case.forward(predicted)
            })?,
            tech_debt: run_head_fail_closed(HeadId::TechDebt, || {
                self.tech_debt.forward(predicted)
            })?,
            perf: run_head_fail_closed(HeadId::Perf, || self.perf.forward(predicted))?,
            security: run_head_fail_closed(HeadId::Security, || self.security.forward(predicted))?,
            accuracy: run_head_fail_closed(HeadId::Accuracy, || self.accuracy.forward(predicted))?,
            cost: run_head_fail_closed(HeadId::Cost, || self.cost.forward(predicted))?,
            reasoning: run_head_fail_closed(HeadId::Reasoning, || {
                self.reasoning.forward(predicted)
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

impl SlotAwareHead {
    fn new(head_id: HeadId, class_count: usize, vb: VarBuilder) -> Result<Self, PredictorError> {
        let mut slot_projections = Vec::with_capacity(InstrumentSlot::all().len());
        for slot in InstrumentSlot::all() {
            slot_projections.push(SlotHeadProjection {
                slot,
                projection: linear(
                    slot.dim(),
                    class_count,
                    vb.pp(format!("slot_{}", slot.slug())),
                )?,
            });
        }
        let late_projection = linear(
            class_count * InstrumentSlot::all().len(),
            class_count,
            vb.pp("late_projection"),
        )?;
        Ok(Self {
            head_id,
            class_count,
            slot_projections,
            late_projection,
        })
    }

    fn forward(&self, predicted: &PredictedPanel) -> Result<HeadPrediction, PredictorError> {
        let mut slot_logits = Vec::with_capacity(self.slot_projections.len());
        let mut slot_contributions = Vec::with_capacity(self.slot_projections.len());
        for slot_projection in &self.slot_projections {
            let (offset, dim) = slot_projection.slot.extent();
            let slot_tensor = predicted.tensor.narrow(1, offset, dim)?;
            let logits = slot_projection.projection.forward(&slot_tensor)?;
            if logits.dims() != [predicted.batch_size, self.class_count] {
                return Err(PredictorError::DimMismatch {
                    detail: format!(
                        "{} {} slot logits expected (B, {}); got {:?}",
                        self.head_id.as_str(),
                        slot_projection.slot.slug(),
                        self.class_count,
                        logits.dims()
                    ),
                    observed: serde_json::json!({
                        "head": self.head_id.as_str(),
                        "slot": slot_projection.slot.slug(),
                        "slot_logits": logits.dims()
                    }),
                    expected_panel_dim: PANEL_DIM,
                });
            }
            ensure_finite(
                &logits,
                NanSource::OracleHead,
                None,
                &format!(
                    "{}_{}_slot_logits",
                    self.head_id.as_str(),
                    slot_projection.slot.slug()
                ),
            )?;
            slot_contributions.push(SlotLogitContribution {
                slot_id: slot_projection.slot.slug().to_string(),
                logits: logits.clone(),
                class_count: self.class_count,
            });
            slot_logits.push(logits);
        }
        let slot_logit_refs = slot_logits.iter().collect::<Vec<_>>();
        let late_input = Tensor::cat(&slot_logit_refs, 1)?;
        let expected_late_dim = self.class_count * self.slot_projections.len();
        if late_input.dims() != [predicted.batch_size, expected_late_dim] {
            return Err(PredictorError::DimMismatch {
                detail: format!(
                    "{} late-head input expected (B, {expected_late_dim}); got {:?}",
                    self.head_id.as_str(),
                    late_input.dims()
                ),
                observed: serde_json::json!({
                    "head": self.head_id.as_str(),
                    "late_head_input": late_input.dims()
                }),
                expected_panel_dim: PANEL_DIM,
            });
        }
        let logits = self.late_projection.forward(&late_input)?;
        build_head_prediction(
            self.head_id,
            logits,
            predicted.batch_size,
            self.class_count,
            slot_contributions,
        )
    }
}

fn build_head_prediction(
    head_id: HeadId,
    logits: Tensor,
    batch_size: usize,
    class_count: usize,
    slot_logits: Vec<SlotLogitContribution>,
) -> Result<HeadPrediction, PredictorError> {
    if logits.dims() != [batch_size, class_count] {
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
        slot_logits,
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
