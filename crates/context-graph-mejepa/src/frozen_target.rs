use std::collections::BTreeMap;

use candle_core::{DType, Device, Tensor};
use context_graph_mejepa_instruments::{
    frozen_hook::hash_f32s, Panel, PANEL_DIM as INSTR_PANEL_DIM,
};

use crate::config::PANEL_DIM;
use crate::data_models::{TargetPanel, TargetProvenance};
use crate::error::{NanSource, PredictorError};
use crate::grad_hook::{InstrumentGradHandle, NoOpGradHandle};
use crate::predictor::ensure_finite;

const _: () = assert!(PANEL_DIM == INSTR_PANEL_DIM);

pub struct FrozenTargetAdapter {
    provenance: TargetProvenance,
    grad_handles: Vec<Box<dyn InstrumentGradHandle + Send + Sync>>,
}

impl std::fmt::Debug for FrozenTargetAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrozenTargetAdapter")
            .field("provenance", &self.provenance)
            .field("grad_handle_count", &self.grad_handles.len())
            .finish()
    }
}

impl FrozenTargetAdapter {
    pub fn new(provenance: TargetProvenance) -> Self {
        Self {
            provenance,
            grad_handles: Vec::new(),
        }
    }

    pub fn empty_for_test() -> Self {
        let mut versions = BTreeMap::new();
        versions.insert("phase1-panel".to_string(), "deterministic-v1".to_string());
        Self::new(TargetProvenance::new(
            "phase2-empty-frozen-target",
            versions,
            0,
            None,
        ))
    }

    pub fn with_grad_handles(
        provenance: TargetProvenance,
        grad_handles: Vec<Box<dyn InstrumentGradHandle + Send + Sync>>,
    ) -> Self {
        Self {
            provenance,
            grad_handles,
        }
    }

    pub fn with_noop_labels(provenance: TargetProvenance, labels: &[&str]) -> Self {
        let handles = labels
            .iter()
            .map(|label| {
                Box::new(NoOpGradHandle::with_label(label))
                    as Box<dyn InstrumentGradHandle + Send + Sync>
            })
            .collect();
        Self::with_grad_handles(provenance, handles)
    }

    pub fn provenance(&self) -> &TargetProvenance {
        &self.provenance
    }

    pub fn grad_handles(&self) -> &[Box<dyn InstrumentGradHandle + Send + Sync>] {
        &self.grad_handles
    }

    pub fn parameters(&self) -> &'static [&'static candle_core::Var] {
        &[]
    }

    pub fn instrument_count(&self) -> usize {
        self.grad_handles.len()
    }

    pub fn encode_target(
        &self,
        panel_t2: &Panel,
        device: &Device,
        dtype: DType,
    ) -> Result<TargetPanel, PredictorError> {
        if panel_t2.data().len() != PANEL_DIM {
            return Err(PredictorError::DimMismatch {
                detail: format!(
                    "panel_t2 has {} dims, expected {PANEL_DIM}",
                    panel_t2.data().len()
                ),
                observed: serde_json::json!({ "panel_t2": panel_t2.data().len() }),
                expected_panel_dim: PANEL_DIM,
            });
        }
        if panel_t2.data().iter().any(|value| !value.is_finite()) {
            return Err(PredictorError::NanDetected {
                nan_source: NanSource::FrozenTarget,
                layer_id: None,
                tensor_name: Some("panel_t2".to_string()),
            });
        }
        let tensor = Tensor::from_slice(panel_t2.data(), (1, PANEL_DIM), device)?
            .to_dtype(dtype)?
            .detach();
        ensure_finite(&tensor, NanSource::FrozenTarget, None, "target_panel")?;
        Ok(TargetPanel {
            tensor,
            batch_size: 1,
            panel_dim: PANEL_DIM,
            dtype: format!("{dtype:?}"),
            provenance: self.provenance.clone(),
        })
    }

    pub fn target_sha256(&self, panel_t2: &Panel) -> String {
        hash_f32s(panel_t2.data())
    }
}
