use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::config::PANEL_DIM;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NanSource {
    Input,
    Layer,
    Output,
    Loss,
    OracleHead,
    FrozenTarget,
}

#[derive(Debug, Error)]
pub enum PredictorError {
    #[error("MEJEPA_PRED_CONFIG_INVALID: {detail}")]
    ConfigInvalid { detail: String },
    #[error("MEJEPA_PRED_DEVICE_UNAVAILABLE: {detail}")]
    DeviceUnavailable { detail: String },
    #[error("MEJEPA_PRED_DIM_MISMATCH: {detail}; observed={observed}; expected_panel_dim={expected_panel_dim}")]
    DimMismatch {
        detail: String,
        observed: Value,
        expected_panel_dim: usize,
    },
    #[error(
        "MEJEPA_PRED_NAN_DETECTED: source={nan_source:?} layer_id={layer_id:?} tensor={tensor_name:?}"
    )]
    NanDetected {
        nan_source: NanSource,
        layer_id: Option<u8>,
        tensor_name: Option<String>,
    },
    #[error("MEJEPA_PRED_VRAM_EXCEEDED: vram_resident_bytes={vram_resident_bytes} threshold_bytes={threshold_bytes}")]
    VramExceeded {
        vram_resident_bytes: u64,
        threshold_bytes: u64,
    },
    #[error("MEJEPA_PRED_FROZEN_TARGET_GRAD: instrument_id={instrument_id} grad_norm={grad_norm:.6e} threshold={threshold:.6e} fix_at={fix_at}")]
    FrozenTargetGrad {
        instrument_id: String,
        grad_norm: f32,
        threshold: f32,
        fix_at: String,
    },
    #[error("MEJEPA_PRED_VICREG_DEGENERATE: low_variance_dim_count={low_variance_dim_count} total_dims={total_dims} gamma={gamma} consecutive_passes={consecutive_passes}")]
    VicregDegenerate {
        low_variance_dim_count: usize,
        total_dims: usize,
        gamma: f32,
        consecutive_passes: u8,
    },
    #[error("MEJEPA_PRED_CANDLE: {0}")]
    Candle(#[from] candle_core::Error),
    #[error("MEJEPA_PRED_IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("MEJEPA_PRED_JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl PredictorError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ConfigInvalid { .. } => "MEJEPA_PRED_CONFIG_INVALID",
            Self::DeviceUnavailable { .. } => "MEJEPA_PRED_DEVICE_UNAVAILABLE",
            Self::DimMismatch { .. } => "MEJEPA_PRED_DIM_MISMATCH",
            Self::NanDetected { .. } => "MEJEPA_PRED_NAN_DETECTED",
            Self::VramExceeded { .. } => "MEJEPA_PRED_VRAM_EXCEEDED",
            Self::FrozenTargetGrad { .. } => "MEJEPA_PRED_FROZEN_TARGET_GRAD",
            Self::VicregDegenerate { .. } => "MEJEPA_PRED_VICREG_DEGENERATE",
            Self::Candle(_) => "MEJEPA_PRED_CANDLE",
            Self::Io(_) => "MEJEPA_PRED_IO",
            Self::Json(_) => "MEJEPA_PRED_JSON",
        }
    }

    pub fn is_critical(&self) -> bool {
        matches!(self, Self::FrozenTargetGrad { .. })
    }
}

#[derive(Debug, Error)]
pub enum LossError {
    #[error("MEJEPA_PRED_DIM_MISMATCH (loss): {detail}")]
    DimMismatch { detail: String },
    #[error("MEJEPA_PRED_NAN_DETECTED (loss component={component}): {detail}")]
    NanDetected {
        component: &'static str,
        detail: String,
    },
    #[error("MEJEPA_PRED_BATCH_TOO_SMALL: batch={batch} minimum=2")]
    BatchTooSmall { batch: usize },
    #[error("MEJEPA_PRED_NON_FINITE_LAMBDA: lambdas={lambdas_dump}")]
    NonFiniteLambda { lambdas_dump: String },
    #[error("MEJEPA_PRED_CANDLE: {0}")]
    Candle(#[from] candle_core::Error),
}

impl LossError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::DimMismatch { .. } => "MEJEPA_PRED_DIM_MISMATCH",
            Self::NanDetected { .. } => "MEJEPA_PRED_NAN_DETECTED",
            Self::BatchTooSmall { .. } => "MEJEPA_PRED_BATCH_TOO_SMALL",
            Self::NonFiniteLambda { .. } => "MEJEPA_PRED_NON_FINITE_LAMBDA",
            Self::Candle(_) => "MEJEPA_PRED_CANDLE",
        }
    }
}

impl From<LossError> for PredictorError {
    fn from(value: LossError) -> Self {
        match value {
            LossError::DimMismatch { detail } => PredictorError::DimMismatch {
                detail,
                observed: Value::Null,
                expected_panel_dim: PANEL_DIM,
            },
            LossError::NanDetected { component, .. } => PredictorError::NanDetected {
                nan_source: NanSource::Loss,
                layer_id: None,
                tensor_name: Some(component.to_string()),
            },
            LossError::BatchTooSmall { batch } => PredictorError::DimMismatch {
                detail: format!("loss batch must be at least 2; got {batch}"),
                observed: serde_json::json!({ "batch": batch }),
                expected_panel_dim: PANEL_DIM,
            },
            LossError::NonFiniteLambda { lambdas_dump } => PredictorError::ConfigInvalid {
                detail: format!("VICReg lambdas must be finite: {lambdas_dump}"),
            },
            LossError::Candle(err) => PredictorError::Candle(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        let err = PredictorError::FrozenTargetGrad {
            instrument_id: "e_ast".to_string(),
            grad_norm: 1.0,
            threshold: 1e-12,
            fix_at: "file:crates/context-graph-mejepa/src/frozen_target.rs".to_string(),
        };
        assert_eq!(err.code(), "MEJEPA_PRED_FROZEN_TARGET_GRAD");
        assert!(err.is_critical());
    }
}
