use serde::{Deserialize, Serialize};

use crate::error::PredictorError;

pub const PANEL_DIM: usize = context_graph_mejepa_instruments::PANEL_DIM;
pub const CONCAT_INPUT_DIM: usize = PANEL_DIM * 2;

pub const DEFAULT_NUM_LAYERS: u8 = 6;
pub const DEFAULT_HIDDEN_DIM: u32 = 1_024;
pub const DEFAULT_NUM_HEADS: u8 = 8;
pub const DEFAULT_FF_EXPANSION: u8 = 4;
pub const DEFAULT_LAYER_NORM_EPS: f64 = 1e-5;

pub const VICREG_VAR_LAMBDA: f32 = 25.0;
pub const VICREG_COV_LAMBDA: f32 = 1.0;
pub const VICREG_INV_LAMBDA: f32 = 25.0;
pub const VICREG_GAMMA: f32 = 1.0;

pub const GRAD_NORM_NOISE_FLOOR: f32 = 1e-12;
pub const VRAM_STEADY_STATE_TARGET_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const VRAM_WARN_THRESHOLD_BYTES: u64 = 9 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictorConfig {
    pub num_layers: u8,
    pub hidden_dim: u32,
    pub num_heads: u8,
    pub ff_expansion: u8,
    pub layer_norm_eps: String,
    pub activation_dtype: String,
    pub gradient_checkpointing: bool,
}

impl Default for PredictorConfig {
    fn default() -> Self {
        Self {
            num_layers: DEFAULT_NUM_LAYERS,
            hidden_dim: DEFAULT_HIDDEN_DIM,
            num_heads: DEFAULT_NUM_HEADS,
            ff_expansion: DEFAULT_FF_EXPANSION,
            layer_norm_eps: DEFAULT_LAYER_NORM_EPS.to_string(),
            activation_dtype: "bf16".to_string(),
            gradient_checkpointing: false,
        }
    }
}

impl PredictorConfig {
    pub fn layer_norm_eps_value(&self) -> Result<f64, PredictorError> {
        self.layer_norm_eps
            .parse::<f64>()
            .map_err(|err| PredictorError::ConfigInvalid {
                detail: format!(
                    "layer_norm_eps must parse as f64; got {:?}: {err}",
                    self.layer_norm_eps
                ),
            })
    }

    pub fn validate(&self) -> Result<(), PredictorError> {
        if self.num_layers == 0 {
            return Err(PredictorError::ConfigInvalid {
                detail: "num_layers must be >= 1".to_string(),
            });
        }
        if self.hidden_dim == 0 {
            return Err(PredictorError::ConfigInvalid {
                detail: "hidden_dim must be >= 1".to_string(),
            });
        }
        if self.num_heads == 0 {
            return Err(PredictorError::ConfigInvalid {
                detail: "num_heads must be >= 1".to_string(),
            });
        }
        if !self.hidden_dim.is_multiple_of(self.num_heads as u32) {
            return Err(PredictorError::ConfigInvalid {
                detail: format!(
                    "hidden_dim {} must be divisible by num_heads {}",
                    self.hidden_dim, self.num_heads
                ),
            });
        }
        if self.ff_expansion == 0 {
            return Err(PredictorError::ConfigInvalid {
                detail: "ff_expansion must be >= 1".to_string(),
            });
        }
        let eps = self.layer_norm_eps_value()?;
        if !eps.is_finite() || eps <= 0.0 {
            return Err(PredictorError::ConfigInvalid {
                detail: format!("layer_norm_eps must be finite and positive; got {eps}"),
            });
        }
        if self.activation_dtype != "bf16" {
            return Err(PredictorError::ConfigInvalid {
                detail: format!(
                    "activation_dtype must be bf16 for Phase 2 CUDA predictor; got {:?}",
                    self.activation_dtype
                ),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_phase2_contract() {
        let config = PredictorConfig::default();
        assert_eq!(PANEL_DIM, 5_120);
        assert_eq!(CONCAT_INPUT_DIM, 10_240);
        assert_eq!(config.num_layers, 6);
        assert_eq!(config.hidden_dim, 1_024);
        assert_eq!(config.num_heads, 8);
        config.validate().expect("default config must validate");
    }

    #[test]
    fn validate_rejects_non_divisible_heads() {
        let config = PredictorConfig {
            hidden_dim: 1_025,
            ..PredictorConfig::default()
        };
        let err = config
            .validate()
            .expect_err("non-divisible heads must fail");
        assert_eq!(err.code(), "MEJEPA_PRED_CONFIG_INVALID");
    }
}
