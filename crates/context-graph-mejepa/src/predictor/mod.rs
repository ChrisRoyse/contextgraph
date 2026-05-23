pub mod heads;
pub mod slot_trace;
pub mod transformer_layer;

use candle_core::{DType, Device, DeviceLocation, Tensor, Var};
use candle_nn::{linear, Linear, Module, VarBuilder, VarMap};
use context_graph_mejepa_instruments::InstrumentSlot;
use serde::Serialize;

pub use slot_trace::{
    build_no_compensation_trace, build_no_compensation_trace_from_tensors, NoCompensationTrace,
    PairwiseResidualContrast, SlotResidualScore,
};
pub use transformer_layer::TransformerLayer;

use crate::config::{PredictorConfig, CONCAT_INPUT_DIM, INVERSE_ACTION_DIM, PANEL_DIM};
use crate::data_models::{PredictedInverseMap, PredictedPanel};
use crate::error::{NanSource, PredictorError};
use crate::frozen_target::FrozenTargetAdapter;
use crate::grad_hook::{run_gradient_hook, GradStore, GradientHookReport};
use crate::loss::VarianceFloorHistory;
use crate::oracle_head::OracleHead;
use crate::predictor::heads::{AllHeadOutputs, AuxiliaryHeads};
use crate::vram::{check_vram_steady_state, VramReport};

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureSummary {
    pub num_layers: u8,
    pub hidden_dim: u32,
    pub num_heads: u8,
    pub ff_expansion: u8,
    pub activation: &'static str,
    pub residual: bool,
    pub layer_norm: bool,
    pub slot_preserving_late_fusion: bool,
    pub slot_encoder_count: usize,
    pub per_slot_decoder_count: usize,
    pub slot_projection_summary: Vec<SlotProjectionSummary>,
    pub panel_dim: usize,
    pub concat_input_dim: usize,
    pub bidirectional_inverse_head: bool,
    pub inverse_slot_preserving_late_fusion: bool,
    pub inverse_slot_encoder_count: usize,
    pub inverse_per_slot_decoder_count: usize,
    pub inverse_action_dim: usize,
    pub gradient_checkpointing: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlotProjectionSummary {
    pub slot: &'static str,
    pub dim: usize,
    pub forward_input_dim: usize,
    pub forward_output_dim: usize,
    pub inverse_target_input_dim: usize,
    pub inverse_input_output_dim: usize,
    pub hidden_dim: u32,
}

pub struct MeJepaPredictor {
    pub(crate) config: PredictorConfig,
    pub(crate) varmap: VarMap,
    pub(crate) slot_blocks: Vec<SlotPredictorBlock>,
    pub(crate) late_fusion_proj: Linear,
    pub(crate) layers: Vec<TransformerLayer>,
    pub(crate) inverse_action_proj: Linear,
    pub(crate) oracle_head: OracleHead,
    pub(crate) auxiliary_heads: AuxiliaryHeads,
    pub(crate) frozen_target_adapter: FrozenTargetAdapter,
    pub(crate) device: Device,
    pub(crate) dtype: DType,
    pub(crate) variance_floor_history: VarianceFloorHistory,
}

impl std::fmt::Debug for MeJepaPredictor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeJepaPredictor")
            .field("config", &self.config)
            .field("layers", &self.layers.len())
            .field("oracle_head", &self.oracle_head)
            .field("auxiliary_heads", &self.auxiliary_heads)
            .field("inverse_action_dim", &INVERSE_ACTION_DIM)
            .field("frozen_target_adapter", &self.frozen_target_adapter)
            .field("device", &self.device.location())
            .field("dtype", &self.dtype)
            .finish()
    }
}

#[derive(Debug)]
pub(crate) struct SlotPredictorBlock {
    slot: InstrumentSlot,
    input_proj: Linear,
    output_proj: Linear,
    inverse_target_proj: Linear,
    inverse_input_proj: Linear,
}

struct LateFusionLatent {
    hidden: Tensor,
    slot_hidden: Vec<Tensor>,
    batch: usize,
}

impl MeJepaPredictor {
    pub fn new(
        config: PredictorConfig,
        frozen_target_adapter: FrozenTargetAdapter,
        device: Device,
        num_tests: usize,
    ) -> Result<Self, PredictorError> {
        config.validate()?;
        match device.location() {
            DeviceLocation::Cuda { gpu_id: 0 } => {}
            other => {
                return Err(PredictorError::DeviceUnavailable {
                    detail: format!(
                        "Phase 2 predictor requires CUDA device 0; got {other:?}; no CPU fallback"
                    ),
                });
            }
        }
        if !frozen_target_adapter.parameters().is_empty() {
            return Err(PredictorError::FrozenTargetGrad {
                instrument_id: "frozen_target_adapter".to_string(),
                grad_norm: 1.0,
                threshold: 0.0,
                fix_at: "file:crates/context-graph-mejepa/src/frozen_target.rs".to_string(),
            });
        }

        let dtype = DType::BF16;
        let hidden_dim = config.hidden_dim as usize;
        let eps = config.layer_norm_eps_value()?;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, dtype, &device);
        let mut slot_blocks = Vec::with_capacity(InstrumentSlot::all().len());
        for slot in InstrumentSlot::all() {
            let slot_vb = vb.pp(format!("slot_{}", slot.slug()));
            slot_blocks.push(SlotPredictorBlock {
                slot,
                input_proj: linear(slot.dim() * 2, hidden_dim, slot_vb.pp("input_proj"))?,
                output_proj: linear(hidden_dim, slot.dim(), slot_vb.pp("output_proj"))?,
                inverse_target_proj: linear(
                    slot.dim(),
                    hidden_dim,
                    slot_vb.pp("inverse_target_proj"),
                )?,
                inverse_input_proj: linear(
                    hidden_dim,
                    slot.dim(),
                    slot_vb.pp("inverse_input_proj"),
                )?,
            });
        }
        let late_fusion_proj = linear(
            hidden_dim * InstrumentSlot::all().len(),
            hidden_dim,
            vb.pp("late_fusion_proj"),
        )?;
        let mut layers = Vec::with_capacity(config.num_layers as usize);
        for layer_idx in 0..config.num_layers {
            layers.push(TransformerLayer::new(
                hidden_dim,
                config.num_heads,
                config.ff_expansion,
                eps,
                vb.pp(format!("layer_{layer_idx}")),
            )?);
        }
        let inverse_action_proj =
            linear(hidden_dim, INVERSE_ACTION_DIM, vb.pp("inverse_action_proj"))?;
        let oracle_head = OracleHead::new(num_tests, vb.pp("oracle_head"))?;
        let auxiliary_heads = AuxiliaryHeads::new(vb.pp("auxiliary_heads"))?;
        Ok(Self {
            config,
            varmap,
            slot_blocks,
            late_fusion_proj,
            layers,
            inverse_action_proj,
            oracle_head,
            auxiliary_heads,
            frozen_target_adapter,
            device,
            dtype,
            variance_floor_history: VarianceFloorHistory::new(3)?,
        })
    }

    pub fn forward(
        &self,
        panel_t0: &Tensor,
        panel_t1: &Tensor,
    ) -> Result<PredictedPanel, PredictorError> {
        self.forward_inner(panel_t0, panel_t1, false)
    }

    pub fn forward_dryrun(
        &self,
        panel_t0: &Tensor,
        panel_t1: &Tensor,
    ) -> Result<PredictedPanel, PredictorError> {
        self.forward_inner(panel_t0, panel_t1, true)
    }

    pub fn forward_late_fused_latent(
        &self,
        panel_t0: &Tensor,
        panel_t1: &Tensor,
    ) -> Result<Tensor, PredictorError> {
        Ok(self
            .encode_late_fused_latent(panel_t0, panel_t1, false)?
            .hidden)
    }

    pub fn forward_late_fused_latent_dryrun(
        &self,
        panel_t0: &Tensor,
        panel_t1: &Tensor,
    ) -> Result<Tensor, PredictorError> {
        Ok(self
            .encode_late_fused_latent(panel_t0, panel_t1, true)?
            .hidden)
    }

    pub fn forward_inverse(
        &self,
        target_panel: &Tensor,
    ) -> Result<PredictedInverseMap, PredictorError> {
        self.forward_inverse_inner(target_panel, false)
    }

    pub fn forward_inverse_dryrun(
        &self,
        target_panel: &Tensor,
    ) -> Result<PredictedInverseMap, PredictorError> {
        self.forward_inverse_inner(target_panel, true)
    }

    pub fn forward_all_heads(
        &self,
        panel_t0: &Tensor,
        panel_t1: &Tensor,
    ) -> Result<AllHeadOutputs, PredictorError> {
        let predicted_panel = self.forward_inner(panel_t0, panel_t1, true)?;
        let auxiliary = self.auxiliary_heads.forward(&predicted_panel)?;
        Ok(AllHeadOutputs {
            predicted_panel,
            failure_mode: auxiliary.failure_mode,
            edge_case: auxiliary.edge_case,
            tech_debt: auxiliary.tech_debt,
            perf: auxiliary.perf,
            security: auxiliary.security,
            accuracy: auxiliary.accuracy,
            cost: auxiliary.cost,
            reasoning: auxiliary.reasoning,
        })
    }

    fn forward_inner(
        &self,
        panel_t0: &Tensor,
        panel_t1: &Tensor,
        validate_finite: bool,
    ) -> Result<PredictedPanel, PredictorError> {
        let latent = self.encode_late_fused_latent(panel_t0, panel_t1, validate_finite)?;
        let batch = latent.batch;
        let hidden = latent.hidden;
        let slot_hidden = latent.slot_hidden;
        let mut predicted_slots = Vec::with_capacity(self.slot_blocks.len());
        for (block, slot_hidden) in self.slot_blocks.iter().zip(&slot_hidden) {
            let slot_context = (slot_hidden + &hidden)?.to_dtype(self.dtype)?;
            let slot_predicted =
                (block.output_proj.forward(&slot_context)? / 4.0)?.to_dtype(self.dtype)?;
            if slot_predicted.dims() != [batch, block.slot.dim()] {
                return Err(PredictorError::DimMismatch {
                    detail: format!(
                        "{} slot output expected (B, {}); got {:?}",
                        block.slot.slug(),
                        block.slot.dim(),
                        slot_predicted.dims()
                    ),
                    observed: serde_json::json!({
                        "slot": block.slot.slug(),
                        "slot_output": slot_predicted.dims()
                    }),
                    expected_panel_dim: PANEL_DIM,
                });
            }
            if validate_finite {
                ensure_finite(
                    &slot_predicted,
                    NanSource::Output,
                    None,
                    &format!("{}_slot_predicted", block.slot.slug()),
                )?;
            }
            predicted_slots.push(slot_predicted);
        }
        let predicted_slot_refs = predicted_slots.iter().collect::<Vec<_>>();
        let predicted = Tensor::cat(&predicted_slot_refs, 1)?;
        if predicted.dims() != [batch, PANEL_DIM] {
            return Err(PredictorError::DimMismatch {
                detail: format!(
                    "output expected (B, {PANEL_DIM}); got {:?}",
                    predicted.dims()
                ),
                observed: serde_json::json!({ "predicted": predicted.dims() }),
                expected_panel_dim: PANEL_DIM,
            });
        }
        if validate_finite {
            ensure_finite(&predicted, NanSource::Output, None, "predicted_panel")?;
        }
        Ok(PredictedPanel {
            tensor: predicted,
            batch_size: batch,
            panel_dim: PANEL_DIM,
            dtype: format!("{:?}", self.dtype),
        })
    }

    fn encode_late_fused_latent(
        &self,
        panel_t0: &Tensor,
        panel_t1: &Tensor,
        validate_finite: bool,
    ) -> Result<LateFusionLatent, PredictorError> {
        validate_panel_pair(panel_t0, panel_t1)?;
        if validate_finite {
            ensure_finite(panel_t0, NanSource::Input, None, "panel_t0")?;
            ensure_finite(panel_t1, NanSource::Input, None, "panel_t1")?;
        }
        let batch = panel_t0.dims()[0];
        let t0 = panel_t0.to_dtype(self.dtype)?;
        let t1 = panel_t1.to_dtype(self.dtype)?;
        let mut slot_hidden = Vec::with_capacity(self.slot_blocks.len());
        for block in &self.slot_blocks {
            let (offset, dim) = block.slot.extent();
            let t0_slot = t0.narrow(1, offset, dim)?;
            let t1_slot = t1.narrow(1, offset, dim)?;
            let slot_input = Tensor::cat(&[&t0_slot, &t1_slot], 1)?;
            if slot_input.dims() != [batch, dim * 2] {
                return Err(PredictorError::DimMismatch {
                    detail: format!(
                        "{} slot input expected (B, {}); got {:?}",
                        block.slot.slug(),
                        dim * 2,
                        slot_input.dims()
                    ),
                    observed: serde_json::json!({
                        "slot": block.slot.slug(),
                        "slot_input": slot_input.dims()
                    }),
                    expected_panel_dim: PANEL_DIM,
                });
            }
            let encoded = block
                .input_proj
                .forward(&slot_input)?
                .to_dtype(self.dtype)?;
            if encoded.dims() != [batch, self.config.hidden_dim as usize] {
                return Err(PredictorError::DimMismatch {
                    detail: format!(
                        "{} slot hidden expected (B, {}); got {:?}",
                        block.slot.slug(),
                        self.config.hidden_dim,
                        encoded.dims()
                    ),
                    observed: serde_json::json!({
                        "slot": block.slot.slug(),
                        "slot_hidden": encoded.dims()
                    }),
                    expected_panel_dim: PANEL_DIM,
                });
            }
            if validate_finite {
                ensure_finite(
                    &encoded,
                    NanSource::Input,
                    None,
                    &format!("{}_slot_hidden", block.slot.slug()),
                )?;
            }
            slot_hidden.push(encoded);
        }
        let slot_hidden_refs = slot_hidden.iter().collect::<Vec<_>>();
        let fusion_input = Tensor::cat(&slot_hidden_refs, 1)?;
        let expected_fusion_dim = self.config.hidden_dim as usize * self.slot_blocks.len();
        if fusion_input.dims() != [batch, expected_fusion_dim] {
            return Err(PredictorError::DimMismatch {
                detail: format!(
                    "late-fusion input expected (B, {expected_fusion_dim}); got {:?}",
                    fusion_input.dims()
                ),
                observed: serde_json::json!({ "late_fusion_input": fusion_input.dims() }),
                expected_panel_dim: PANEL_DIM,
            });
        }
        let hidden = self
            .late_fusion_proj
            .forward(&fusion_input)?
            .to_dtype(self.dtype)?;
        let hidden = self.forward_shared_trunk(hidden, validate_finite)?;
        if validate_finite {
            ensure_finite(&hidden, NanSource::Output, None, "late_fused_latent")?;
        }
        Ok(LateFusionLatent {
            hidden,
            slot_hidden,
            batch,
        })
    }

    fn forward_inverse_inner(
        &self,
        target_panel: &Tensor,
        validate_finite: bool,
    ) -> Result<PredictedInverseMap, PredictorError> {
        validate_panel_tensor(target_panel, "target_panel")?;
        if validate_finite {
            ensure_finite(target_panel, NanSource::Input, None, "target_panel")?;
        }
        let batch = target_panel.dims()[0];
        let target = target_panel.to_dtype(self.dtype)?;
        let mut inverse_slot_hidden = Vec::with_capacity(self.slot_blocks.len());
        for block in &self.slot_blocks {
            let (offset, dim) = block.slot.extent();
            let target_slot = target.narrow(1, offset, dim)?;
            let encoded = block
                .inverse_target_proj
                .forward(&target_slot)?
                .to_dtype(self.dtype)?;
            if encoded.dims() != [batch, self.config.hidden_dim as usize] {
                return Err(PredictorError::DimMismatch {
                    detail: format!(
                        "{} inverse target hidden expected (B, {}); got {:?}",
                        block.slot.slug(),
                        self.config.hidden_dim,
                        encoded.dims()
                    ),
                    observed: serde_json::json!({
                        "slot": block.slot.slug(),
                        "inverse_target_hidden": encoded.dims()
                    }),
                    expected_panel_dim: PANEL_DIM,
                });
            }
            if validate_finite {
                ensure_finite(
                    &encoded,
                    NanSource::Input,
                    None,
                    &format!("{}_inverse_target_hidden", block.slot.slug()),
                )?;
            }
            inverse_slot_hidden.push(encoded);
        }
        let inverse_slot_refs = inverse_slot_hidden.iter().collect::<Vec<_>>();
        let inverse_fusion_input = Tensor::cat(&inverse_slot_refs, 1)?;
        let expected_fusion_dim = self.config.hidden_dim as usize * self.slot_blocks.len();
        if inverse_fusion_input.dims() != [batch, expected_fusion_dim] {
            return Err(PredictorError::DimMismatch {
                detail: format!(
                    "inverse late-fusion input expected (B, {expected_fusion_dim}); got {:?}",
                    inverse_fusion_input.dims()
                ),
                observed: serde_json::json!({
                    "inverse_late_fusion_input": inverse_fusion_input.dims()
                }),
                expected_panel_dim: PANEL_DIM,
            });
        }
        let hidden = self
            .late_fusion_proj
            .forward(&inverse_fusion_input)?
            .to_dtype(self.dtype)?;
        let hidden = self.forward_shared_trunk(hidden, validate_finite)?;
        let mut predicted_input_slots = Vec::with_capacity(self.slot_blocks.len());
        for (block, slot_hidden) in self.slot_blocks.iter().zip(&inverse_slot_hidden) {
            let slot_context = (slot_hidden + &hidden)?.to_dtype(self.dtype)?;
            let slot_predicted =
                (block.inverse_input_proj.forward(&slot_context)? / 4.0)?.to_dtype(self.dtype)?;
            if slot_predicted.dims() != [batch, block.slot.dim()] {
                return Err(PredictorError::DimMismatch {
                    detail: format!(
                        "{} inverse input slot expected (B, {}); got {:?}",
                        block.slot.slug(),
                        block.slot.dim(),
                        slot_predicted.dims()
                    ),
                    observed: serde_json::json!({
                        "slot": block.slot.slug(),
                        "inverse_input_slot": slot_predicted.dims()
                    }),
                    expected_panel_dim: PANEL_DIM,
                });
            }
            if validate_finite {
                ensure_finite(
                    &slot_predicted,
                    NanSource::Output,
                    None,
                    &format!("{}_inverse_input_slot", block.slot.slug()),
                )?;
            }
            predicted_input_slots.push(slot_predicted);
        }
        let predicted_input_refs = predicted_input_slots.iter().collect::<Vec<_>>();
        let predicted_input = Tensor::cat(&predicted_input_refs, 1)?;
        if predicted_input.dims() != [batch, PANEL_DIM] {
            return Err(PredictorError::DimMismatch {
                detail: format!(
                    "inverse input-panel output expected (B, {PANEL_DIM}); got {:?}",
                    predicted_input.dims()
                ),
                observed: serde_json::json!({ "predicted_input_panel": predicted_input.dims() }),
                expected_panel_dim: PANEL_DIM,
            });
        }
        let predicted_action =
            (self.inverse_action_proj.forward(&hidden)? / 4.0)?.to_dtype(self.dtype)?;
        if predicted_action.dims() != [batch, INVERSE_ACTION_DIM] {
            return Err(PredictorError::DimMismatch {
                detail: format!(
                    "inverse action output expected (B, {INVERSE_ACTION_DIM}); got {:?}",
                    predicted_action.dims()
                ),
                observed: serde_json::json!({ "predicted_action": predicted_action.dims() }),
                expected_panel_dim: PANEL_DIM,
            });
        }
        if validate_finite {
            ensure_finite(
                &predicted_input,
                NanSource::Output,
                None,
                "inverse_predicted_input_panel",
            )?;
            ensure_finite(
                &predicted_action,
                NanSource::Output,
                None,
                "inverse_predicted_action",
            )?;
        }
        Ok(PredictedInverseMap {
            predicted_input_panel: PredictedPanel {
                tensor: predicted_input,
                batch_size: batch,
                panel_dim: PANEL_DIM,
                dtype: format!("{:?}", self.dtype),
            },
            predicted_action,
            action_dim: INVERSE_ACTION_DIM,
            dtype: format!("{:?}", self.dtype),
        })
    }

    fn forward_shared_trunk(
        &self,
        mut hidden: Tensor,
        validate_finite: bool,
    ) -> Result<Tensor, PredictorError> {
        for (idx, layer) in self.layers.iter().enumerate() {
            hidden = layer.forward_with_layer_idx(&hidden, idx as u8, validate_finite)?;
        }
        Ok(hidden)
    }

    pub fn trainable_parameters(&self) -> Vec<Var> {
        self.varmap.all_vars()
    }

    pub fn config(&self) -> &PredictorConfig {
        &self.config
    }

    pub fn architecture_summary(&self) -> ArchitectureSummary {
        ArchitectureSummary {
            num_layers: self.config.num_layers,
            hidden_dim: self.config.hidden_dim,
            num_heads: self.config.num_heads,
            ff_expansion: self.config.ff_expansion,
            activation: "GeLU",
            residual: true,
            layer_norm: true,
            slot_preserving_late_fusion: true,
            slot_encoder_count: InstrumentSlot::all().len(),
            per_slot_decoder_count: InstrumentSlot::all().len(),
            slot_projection_summary: self.slot_projection_summary(),
            panel_dim: PANEL_DIM,
            concat_input_dim: CONCAT_INPUT_DIM,
            bidirectional_inverse_head: true,
            inverse_slot_preserving_late_fusion: true,
            inverse_slot_encoder_count: InstrumentSlot::all().len(),
            inverse_per_slot_decoder_count: InstrumentSlot::all().len(),
            inverse_action_dim: INVERSE_ACTION_DIM,
            gradient_checkpointing: self.config.gradient_checkpointing,
        }
    }

    fn slot_projection_summary(&self) -> Vec<SlotProjectionSummary> {
        slot_projection_summary_for_hidden_dim(self.config.hidden_dim)
    }

    pub fn oracle_head(&self) -> &OracleHead {
        &self.oracle_head
    }

    pub fn frozen_target_adapter(&self) -> &FrozenTargetAdapter {
        &self.frozen_target_adapter
    }

    pub fn dtype(&self) -> DType {
        self.dtype
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn check_vram_steady_state(&self) -> Result<VramReport, PredictorError> {
        check_vram_steady_state(&self.device)
    }

    pub fn run_gradient_hook(
        &self,
        grads: &GradStore,
    ) -> Result<GradientHookReport, PredictorError> {
        run_gradient_hook(&self.frozen_target_adapter, grads)
    }

    pub fn variance_floor_history(&mut self) -> &mut VarianceFloorHistory {
        &mut self.variance_floor_history
    }
}

fn slot_projection_summary_for_hidden_dim(hidden_dim: u32) -> Vec<SlotProjectionSummary> {
    InstrumentSlot::all()
        .iter()
        .map(|slot| SlotProjectionSummary {
            slot: slot.slug(),
            dim: slot.dim(),
            forward_input_dim: slot.dim() * 2,
            forward_output_dim: slot.dim(),
            inverse_target_input_dim: slot.dim(),
            inverse_input_output_dim: slot.dim(),
            hidden_dim,
        })
        .collect()
}

pub(crate) fn validate_panel_pair(
    panel_t0: &Tensor,
    panel_t1: &Tensor,
) -> Result<(), PredictorError> {
    let t0 = panel_t0.dims();
    let t1 = panel_t1.dims();
    if t0.len() != 2 || t1.len() != 2 || t0[1] != PANEL_DIM || t1[1] != PANEL_DIM || t0[0] != t1[0]
    {
        return Err(PredictorError::DimMismatch {
            detail: format!("panel_t0={t0:?} panel_t1={t1:?} expected (B, {PANEL_DIM})"),
            observed: serde_json::json!({ "panel_t0": t0, "panel_t1": t1 }),
            expected_panel_dim: PANEL_DIM,
        });
    }
    if t0[0] == 0 {
        return Err(PredictorError::DimMismatch {
            detail: "batch size must be >= 1".to_string(),
            observed: serde_json::json!({ "batch": 0 }),
            expected_panel_dim: PANEL_DIM,
        });
    }
    if !panel_t0.device().same_device(panel_t1.device()) {
        return Err(PredictorError::DeviceUnavailable {
            detail: format!(
                "panel_t0 device {:?} differs from panel_t1 device {:?}",
                panel_t0.device().location(),
                panel_t1.device().location()
            ),
        });
    }
    Ok(())
}

pub(crate) fn validate_panel_tensor(
    panel: &Tensor,
    tensor_name: &str,
) -> Result<(), PredictorError> {
    let dims = panel.dims();
    if dims.len() != 2 || dims[1] != PANEL_DIM {
        return Err(PredictorError::DimMismatch {
            detail: format!("{tensor_name}={dims:?} expected (B, {PANEL_DIM})"),
            observed: serde_json::json!({ tensor_name: dims }),
            expected_panel_dim: PANEL_DIM,
        });
    }
    if dims[0] == 0 {
        return Err(PredictorError::DimMismatch {
            detail: format!("{tensor_name} batch size must be >= 1"),
            observed: serde_json::json!({ "batch": 0 }),
            expected_panel_dim: PANEL_DIM,
        });
    }
    Ok(())
}

pub(crate) fn ensure_finite(
    tensor: &Tensor,
    nan_source: NanSource,
    layer_id: Option<u8>,
    tensor_name: &str,
) -> Result<(), PredictorError> {
    let values = tensor
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    if values.iter().any(|value| !value.is_finite()) {
        return Err(PredictorError::NanDetected {
            nan_source,
            layer_id,
            tensor_name: Some(tensor_name.to_string()),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_summary_declares_inverse_slot_preserving_dims() {
        let hidden_dim = 17;
        let summary = slot_projection_summary_for_hidden_dim(hidden_dim);

        assert_eq!(summary.len(), InstrumentSlot::all().len());
        assert_eq!(
            summary.iter().map(|slot| slot.dim).sum::<usize>(),
            PANEL_DIM
        );
        for slot in &summary {
            assert_eq!(slot.forward_input_dim, slot.dim * 2);
            assert_eq!(slot.forward_output_dim, slot.dim);
            assert_eq!(slot.inverse_target_input_dim, slot.dim);
            assert_eq!(slot.inverse_input_output_dim, slot.dim);
            assert_eq!(slot.hidden_dim, hidden_dim);
            assert_ne!(slot.inverse_target_input_dim, PANEL_DIM);
            assert_ne!(slot.inverse_input_output_dim, PANEL_DIM);
        }
    }
}
