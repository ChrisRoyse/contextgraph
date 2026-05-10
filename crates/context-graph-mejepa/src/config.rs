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

pub const INFER_DEFAULT_ALPHA: f32 = 0.10;
pub const INFER_DEFAULT_P_THRESHOLD: f32 = 0.01;
pub const INFER_DEFAULT_MAX_CALIBRATION_AGE_DAYS: u32 = 30;
pub const INFER_DEFAULT_MAX_CONSTELLATION_AGE_DAYS: u32 = 90;
pub const INFER_DEFAULT_P_TEST_THRESHOLD: f32 = 0.80;
pub const INFER_DEFAULT_OOD_REFUSE_THRESHOLD: f32 = 0.50;
pub const INFER_DEFAULT_OUTCOME_SET_MAX: usize = 2;
pub const INFER_DEFAULT_BOOTSTRAP_DELTA_OMEGA: f32 = 0.50;
pub const INFER_DEFAULT_BOOTSTRAP_DELTA_XI: f32 = 0.50;
pub const INFER_DEFAULT_TRAIN_CERT_WINDOW_STEPS: usize = 128;
pub const INFER_DEFAULT_INSTRUMENT_CACHE_CAPACITY: usize = 4096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeJepaInferConfig {
    pub alpha: f32,
    pub p_threshold: f32,
    pub max_calibration_age_days: u32,
    pub max_constellation_age_days: u32,
    pub p_test_threshold: f32,
    pub ood_refuse_threshold: f32,
    pub outcome_set_max: usize,
    pub bootstrap_delta_omega: f32,
    pub bootstrap_delta_xi: f32,
    pub train_cert_window_steps: usize,
    pub instrument_cache_capacity: usize,
}

impl Default for MeJepaInferConfig {
    fn default() -> Self {
        Self {
            alpha: INFER_DEFAULT_ALPHA,
            p_threshold: INFER_DEFAULT_P_THRESHOLD,
            max_calibration_age_days: INFER_DEFAULT_MAX_CALIBRATION_AGE_DAYS,
            max_constellation_age_days: INFER_DEFAULT_MAX_CONSTELLATION_AGE_DAYS,
            p_test_threshold: INFER_DEFAULT_P_TEST_THRESHOLD,
            ood_refuse_threshold: INFER_DEFAULT_OOD_REFUSE_THRESHOLD,
            outcome_set_max: INFER_DEFAULT_OUTCOME_SET_MAX,
            bootstrap_delta_omega: INFER_DEFAULT_BOOTSTRAP_DELTA_OMEGA,
            bootstrap_delta_xi: INFER_DEFAULT_BOOTSTRAP_DELTA_XI,
            train_cert_window_steps: INFER_DEFAULT_TRAIN_CERT_WINDOW_STEPS,
            instrument_cache_capacity: INFER_DEFAULT_INSTRUMENT_CACHE_CAPACITY,
        }
    }
}

impl MeJepaInferConfig {
    pub fn validate(&self) -> Result<(), PredictorError> {
        for (name, value) in [
            ("alpha", self.alpha),
            ("p_threshold", self.p_threshold),
            ("p_test_threshold", self.p_test_threshold),
            ("ood_refuse_threshold", self.ood_refuse_threshold),
            ("bootstrap_delta_omega", self.bootstrap_delta_omega),
            ("bootstrap_delta_xi", self.bootstrap_delta_xi),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(PredictorError::ConfigInvalid {
                    detail: format!("{name} must be finite and in [0, 1]; got {value}"),
                });
            }
        }
        if self.alpha <= 0.0 || self.alpha >= 1.0 {
            return Err(PredictorError::ConfigInvalid {
                detail: format!("alpha must be strictly inside (0, 1); got {}", self.alpha),
            });
        }
        if self.max_calibration_age_days == 0 {
            return Err(PredictorError::ConfigInvalid {
                detail: "max_calibration_age_days must be >= 1".to_string(),
            });
        }
        if self.max_constellation_age_days == 0 {
            return Err(PredictorError::ConfigInvalid {
                detail: "max_constellation_age_days must be >= 1".to_string(),
            });
        }
        if self.outcome_set_max == 0 {
            return Err(PredictorError::ConfigInvalid {
                detail: "outcome_set_max must be >= 1".to_string(),
            });
        }
        if self.train_cert_window_steps == 0 {
            return Err(PredictorError::ConfigInvalid {
                detail: "train_cert_window_steps must be >= 1".to_string(),
            });
        }
        if self.instrument_cache_capacity == 0 {
            return Err(PredictorError::ConfigInvalid {
                detail: "instrument_cache_capacity must be >= 1".to_string(),
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
