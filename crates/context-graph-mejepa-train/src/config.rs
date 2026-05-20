use crate::error::{TrainerError, TrainerErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;

pub const Q4_LOSS_COEFFICIENT_MAX: f32 = 0.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LossCoefficients {
    pub lambda_var: f32,
    pub lambda_cov: f32,
    pub lambda_inv: f32,
    pub lambda_sigreg: f32,
    pub lambda_entropy: f32,
    pub alpha_warn: f32,
    pub alpha_runtime: f32,
    pub alpha_rss: f32,
    pub alpha_trace: f32,
    pub alpha_hunk: f32,
    pub alpha_adjacent: f32,
    pub alpha_reasoning: f32,
    pub alpha_overrides: f32,
    pub alpha_counterfactual: f32,
    pub delta: f32,
}

impl Default for LossCoefficients {
    fn default() -> Self {
        Self {
            lambda_var: 25.0,
            lambda_cov: 1.0,
            lambda_inv: 25.0,
            lambda_sigreg: 1.0,
            lambda_entropy: 0.0,
            alpha_warn: 0.5,
            alpha_runtime: 0.3,
            alpha_rss: 0.2,
            alpha_trace: 0.5,
            alpha_hunk: 1.0,
            alpha_adjacent: 0.5,
            alpha_reasoning: 0.0,
            alpha_overrides: 5.0,
            alpha_counterfactual: 0.2,
            delta: 2.0,
        }
    }
}

impl LossCoefficients {
    pub fn q4_loss_coefficient_max(&self) -> f32 {
        self.alpha_reasoning.max(0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TrainingConfig {
    pub lr: f64,
    pub weight_decay: f64,
    pub batch_size: usize,
    pub epochs: u32,
    pub warmup_steps: u32,
    pub max_grad_norm: f32,
    pub mixed_precision: bool,
    pub lora_rank: usize,
    pub lora_alpha: usize,
    pub lora_dropout: f32,
    pub full_finetune: bool,
    pub loss_coefficients: LossCoefficients,
    pub inverse_map_coefficient: f32,
    pub checkpoint_interval_steps: u64,
    pub holdout_eval_interval_steps: u64,
    pub counterfactual_interval_steps: u64,
    pub counterfactual_warmup_steps: u64,
    pub distillation_interval_steps: u64,
    pub cross_task_transfer_probability: f32,
    pub cross_task_cosine_threshold: f32,
    pub adversarial_mix_ratio: f32,
    pub default_library_id: context_graph_mejepa::LibraryId,
    pub sampling_foundationality_lambda: f32,
    pub sampling_drop_threshold: f32,
    pub sampling_force_threshold: f32,
    pub sampling_age_decay: f32,
    pub operator_override_min_multiplier: f32,
    pub operator_override_max_multiplier: f32,
    pub holdout_promotion_threshold: f32,
    pub holdout_regression_threshold: f32,
    pub phase3_dod_min_agreement: f32,
    pub random_seed: u64,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            lr: 5e-4,
            weight_decay: 0.04,
            batch_size: 64,
            epochs: 10,
            warmup_steps: 500,
            max_grad_norm: 1.0,
            mixed_precision: true,
            lora_rank: 32,
            lora_alpha: 16,
            lora_dropout: 0.05,
            full_finetune: false,
            loss_coefficients: LossCoefficients::default(),
            inverse_map_coefficient: 0.0,
            checkpoint_interval_steps: 2_000,
            holdout_eval_interval_steps: 1_000,
            counterfactual_interval_steps: 5,
            counterfactual_warmup_steps: 1_000,
            distillation_interval_steps: 1_000,
            cross_task_transfer_probability: 0.3,
            cross_task_cosine_threshold: 0.7,
            adversarial_mix_ratio: 0.20,
            default_library_id: context_graph_mejepa::LibraryId::PythonSweBenchLite,
            sampling_foundationality_lambda: 1.0,
            sampling_drop_threshold: 0.05,
            sampling_force_threshold: 1.0,
            sampling_age_decay: 0.995,
            operator_override_min_multiplier: 4.0,
            operator_override_max_multiplier: 12.0,
            holdout_promotion_threshold: 0.005,
            holdout_regression_threshold: 0.02,
            phase3_dod_min_agreement: 0.75,
            random_seed: 0,
        }
    }
}

impl TrainingConfig {
    pub fn validate(&self) -> Result<(), TrainerError> {
        let bad = |field: &'static str, why: &'static str| {
            Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("{field}: {why}"),
            )
            .with_context(json!({
                "file": "file:crates/context-graph-mejepa-train/src/config.rs",
                "field": field,
                "remediation": "fix the invalid training config field and restart training"
            })))
        };
        if !self.lr.is_finite() || self.lr <= 0.0 {
            return bad("lr", "must be positive finite");
        }
        if !self.weight_decay.is_finite() || self.weight_decay < 0.0 {
            return bad("weight_decay", "must be non-negative finite");
        }
        if self.batch_size == 0 {
            return bad("batch_size", "must be positive");
        }
        if self.epochs == 0 {
            return bad("epochs", "must be positive");
        }
        if !self.max_grad_norm.is_finite() || self.max_grad_norm <= 0.0 {
            return bad("max_grad_norm", "must be positive finite");
        }
        if !(0.0..=0.5).contains(&self.lora_dropout) || !self.lora_dropout.is_finite() {
            return bad("lora_dropout", "must be in [0, 0.5]");
        }
        if self.lora_rank == 0 || self.lora_alpha == 0 {
            return bad("lora_rank/lora_alpha", "must be positive");
        }
        for (field, value) in [
            (
                "cross_task_transfer_probability",
                self.cross_task_transfer_probability,
            ),
            (
                "cross_task_cosine_threshold",
                self.cross_task_cosine_threshold,
            ),
            ("adversarial_mix_ratio", self.adversarial_mix_ratio),
            ("sampling_force_threshold", self.sampling_force_threshold),
            ("sampling_age_decay", self.sampling_age_decay),
            (
                "holdout_promotion_threshold",
                self.holdout_promotion_threshold,
            ),
            (
                "holdout_regression_threshold",
                self.holdout_regression_threshold,
            ),
            ("phase3_dod_min_agreement", self.phase3_dod_min_agreement),
        ] {
            if !(0.0..=1.0).contains(&value) || !value.is_finite() {
                return bad(field, "must be in [0, 1]");
            }
        }
        if !self.sampling_drop_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.sampling_drop_threshold)
        {
            return bad("sampling_drop_threshold", "must be in [0, 1]");
        }
        if let Err(err) = self.default_library_id.validate("default_library_id") {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("default_library_id: {err}"),
            )
            .with_context(json!({
                "file": "file:crates/context-graph-mejepa-train/src/config.rs",
                "field": "default_library_id",
                "remediation": "use a built-in library id or a non-empty custom library slug"
            })));
        }
        if !self.sampling_foundationality_lambda.is_finite()
            || self.sampling_foundationality_lambda < 0.0
        {
            return bad(
                "sampling_foundationality_lambda",
                "must be finite and non-negative",
            );
        }
        if !self.operator_override_min_multiplier.is_finite()
            || !self.operator_override_max_multiplier.is_finite()
            || self.operator_override_min_multiplier < 1.0
            || self.operator_override_max_multiplier < self.operator_override_min_multiplier
        {
            return bad(
                "operator_override_min_multiplier/operator_override_max_multiplier",
                "must satisfy 1 <= min <= max",
            );
        }
        if self.sampling_force_threshold <= self.sampling_drop_threshold {
            return bad(
                "sampling_force_threshold",
                "must be greater than sampling_drop_threshold",
            );
        }
        if !(0.0..=1.0).contains(&self.inverse_map_coefficient)
            || !self.inverse_map_coefficient.is_finite()
        {
            return bad("inverse_map_coefficient", "must be finite and in [0, 1]");
        }
        for (field, value) in [
            ("checkpoint_interval_steps", self.checkpoint_interval_steps),
            (
                "holdout_eval_interval_steps",
                self.holdout_eval_interval_steps,
            ),
            (
                "counterfactual_interval_steps",
                self.counterfactual_interval_steps,
            ),
            (
                "distillation_interval_steps",
                self.distillation_interval_steps,
            ),
        ] {
            if value == 0 {
                return bad(field, "must be positive");
            }
        }
        for (field, value) in [
            ("lambda_var", self.loss_coefficients.lambda_var),
            ("lambda_cov", self.loss_coefficients.lambda_cov),
            ("lambda_inv", self.loss_coefficients.lambda_inv),
            ("lambda_sigreg", self.loss_coefficients.lambda_sigreg),
            ("lambda_entropy", self.loss_coefficients.lambda_entropy),
            ("alpha_warn", self.loss_coefficients.alpha_warn),
            ("alpha_runtime", self.loss_coefficients.alpha_runtime),
            ("alpha_rss", self.loss_coefficients.alpha_rss),
            ("alpha_trace", self.loss_coefficients.alpha_trace),
            ("alpha_hunk", self.loss_coefficients.alpha_hunk),
            ("alpha_adjacent", self.loss_coefficients.alpha_adjacent),
            ("alpha_reasoning", self.loss_coefficients.alpha_reasoning),
            ("alpha_overrides", self.loss_coefficients.alpha_overrides),
            (
                "alpha_counterfactual",
                self.loss_coefficients.alpha_counterfactual,
            ),
            ("delta", self.loss_coefficients.delta),
        ] {
            if !value.is_finite() || value < 0.0 {
                return bad(field, "must be non-negative finite");
            }
        }
        if self.loss_coefficients.q4_loss_coefficient_max() != Q4_LOSS_COEFFICIENT_MAX {
            return bad(
                "loss_coefficients.alpha_reasoning",
                "Q4 doctrine freeze requires q4_loss_coefficient_max == 0.0",
            );
        }
        Ok(())
    }

    pub fn load_from_toml(path: &Path) -> Result<Self, TrainerError> {
        let bytes = std::fs::read(path).map_err(|err| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("failed to read config {}: {err}", path.display()),
            )
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|err| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("TOML config is not UTF-8: {err}"),
            )
        })?;
        let cfg: Self = toml::from_str(text)?;
        cfg.validate()?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate() {
        TrainingConfig::default().validate().unwrap();
    }

    #[test]
    fn invalid_lr_fails_closed() {
        let cfg = TrainingConfig {
            lr: -1.0,
            ..TrainingConfig::default()
        };
        let err = cfg.validate().unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }

    #[test]
    fn invalid_inverse_map_coefficient_fails_closed() {
        let cfg = TrainingConfig {
            inverse_map_coefficient: 1.25,
            ..TrainingConfig::default()
        };
        let err = cfg.validate().unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }

    #[test]
    fn q4_loss_coefficient_max_is_frozen_zero() {
        let cfg = TrainingConfig::default();
        assert_eq!(
            cfg.loss_coefficients.q4_loss_coefficient_max(),
            Q4_LOSS_COEFFICIENT_MAX
        );
        cfg.validate().unwrap();
    }

    #[test]
    fn nonzero_q4_loss_coefficient_fails_closed() {
        let cfg = TrainingConfig {
            loss_coefficients: LossCoefficients {
                alpha_reasoning: 0.3,
                ..LossCoefficients::default()
            },
            ..TrainingConfig::default()
        };
        let err = cfg.validate().unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
        assert!(err.to_string().contains("q4_loss_coefficient_max"));
    }

    #[test]
    fn toml_round_trip() {
        let cfg = TrainingConfig::default();
        let text = toml::to_string(&cfg).unwrap();
        let decoded: TrainingConfig = toml::from_str(&text).unwrap();
        assert_eq!(cfg, decoded);
    }
}
