pub mod heads;
pub mod transformer_layer;

use candle_core::{DType, Device, DeviceLocation, Tensor, Var};
use candle_nn::{linear, Linear, Module, VarBuilder, VarMap};
use serde::Serialize;

pub use transformer_layer::TransformerLayer;

use crate::config::{PredictorConfig, CONCAT_INPUT_DIM, PANEL_DIM};
use crate::data_models::PredictedPanel;
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
    pub panel_dim: usize,
    pub concat_input_dim: usize,
    pub gradient_checkpointing: bool,
}

pub struct MeJepaPredictor {
    pub(crate) config: PredictorConfig,
    pub(crate) varmap: VarMap,
    pub(crate) input_proj: Linear,
    pub(crate) layers: Vec<TransformerLayer>,
    pub(crate) output_proj: Linear,
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
            .field("frozen_target_adapter", &self.frozen_target_adapter)
            .field("device", &self.device.location())
            .field("dtype", &self.dtype)
            .finish()
    }
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
        let input_proj = linear(CONCAT_INPUT_DIM, hidden_dim, vb.pp("input_proj"))?;
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
        let output_proj = linear(hidden_dim, PANEL_DIM, vb.pp("output_proj"))?;
        let oracle_head = OracleHead::new(num_tests, vb.pp("oracle_head"))?;
        let auxiliary_heads = AuxiliaryHeads::new(vb.pp("auxiliary_heads"))?;
        Ok(Self {
            config,
            varmap,
            input_proj,
            layers,
            output_proj,
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
        validate_panel_pair(panel_t0, panel_t1)?;
        if validate_finite {
            ensure_finite(panel_t0, NanSource::Input, None, "panel_t0")?;
            ensure_finite(panel_t1, NanSource::Input, None, "panel_t1")?;
        }
        let batch = panel_t0.dims()[0];
        let t0 = panel_t0.to_dtype(self.dtype)?;
        let t1 = panel_t1.to_dtype(self.dtype)?;
        let x = Tensor::cat(&[&t0, &t1], 1)?;
        if x.dims() != [batch, CONCAT_INPUT_DIM] {
            return Err(PredictorError::DimMismatch {
                detail: format!(
                    "concat expected (B, {CONCAT_INPUT_DIM}); got {:?}",
                    x.dims()
                ),
                observed: serde_json::json!({ "concat": x.dims() }),
                expected_panel_dim: PANEL_DIM,
            });
        }
        let mut hidden = self.input_proj.forward(&x)?.to_dtype(self.dtype)?;
        for (idx, layer) in self.layers.iter().enumerate() {
            hidden = layer.forward_with_layer_idx(&hidden, idx as u8, validate_finite)?;
        }
        let predicted = (self.output_proj.forward(&hidden)? / 4.0)?.to_dtype(self.dtype)?;
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

    pub fn trainable_parameters(&self) -> Vec<Var> {
        self.varmap.all_vars()
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
            panel_dim: PANEL_DIM,
            concat_input_dim: CONCAT_INPUT_DIM,
            gradient_checkpointing: self.config.gradient_checkpointing,
        }
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
