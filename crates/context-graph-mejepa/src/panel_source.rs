use std::fs;
use std::path::Path;

use candle_core::{DType, Device, Tensor};
use context_graph_mejepa_instruments::{Panel, PANEL_DIM};
use sha2::{Digest, Sha256};

use crate::error::PredictorError;

pub const DEFAULT_PHASE1_PANEL_PATH: &str =
    "/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/panel-all-slots.json";

#[derive(Debug, Clone, Copy)]
pub enum PanelBatchView {
    T0,
    T1,
    T2,
}

pub fn read_phase1_panel(path: &Path) -> Result<Panel, PredictorError> {
    let text = fs::read_to_string(path).map_err(|err| PredictorError::DeviceUnavailable {
        detail: format!(
            "required real Phase 1 panel source is missing or unreadable at {}: {err}; run `cargo run -p context-graph-mejepa-instruments --example phase1_extended_fsv` first",
            path.display()
        ),
    })?;
    let panel: Panel = serde_json::from_str(&text)?;
    if panel.data().len() != PANEL_DIM {
        return Err(PredictorError::DimMismatch {
            detail: format!(
                "Phase 1 panel has {} dims, expected {PANEL_DIM}",
                panel.data().len()
            ),
            observed: serde_json::json!({ "panel_path": path.display().to_string(), "dim": panel.data().len() }),
            expected_panel_dim: PANEL_DIM,
        });
    }
    Ok(panel)
}

pub fn panel_batch_tensor(
    panel: &Panel,
    batch_size: usize,
    view: PanelBatchView,
    device: &Device,
    dtype: DType,
) -> Result<Tensor, PredictorError> {
    if batch_size == 0 {
        return Err(PredictorError::DimMismatch {
            detail: "batch_size must be >= 1".to_string(),
            observed: serde_json::json!({ "batch_size": 0 }),
            expected_panel_dim: PANEL_DIM,
        });
    }
    let mut values = Vec::with_capacity(batch_size * PANEL_DIM);
    for row in 0..batch_size {
        let row_phase = (row as f32 + 1.0) / (batch_size as f32 + 1.0);
        for (col, base) in panel.data().iter().enumerate() {
            let col_phase = (col % 97) as f32 / 97.0;
            let modifier = match view {
                PanelBatchView::T0 => -0.012 + row_phase * 0.006 + col_phase * 0.0007,
                PanelBatchView::T1 => -0.004 + row_phase * 0.009 - col_phase * 0.0005,
                PanelBatchView::T2 => row_phase * 0.004 + col_phase * 0.0003,
            };
            let value = base.mul_add(1.0 + modifier, modifier * 0.01);
            if !value.is_finite() {
                return Err(PredictorError::NanDetected {
                    nan_source: crate::error::NanSource::Input,
                    layer_id: None,
                    tensor_name: Some("panel_batch_tensor".to_string()),
                });
            }
            values.push(value);
        }
    }
    Ok(Tensor::from_slice(&values, (batch_size, PANEL_DIM), device)?.to_dtype(dtype)?)
}

pub fn tensor_sha256_f32(tensor: &Tensor) -> Result<String, PredictorError> {
    let values = tensor
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn panel_first_values(panel: &Panel, count: usize) -> Vec<f32> {
    panel.data().iter().take(count).copied().collect()
}
