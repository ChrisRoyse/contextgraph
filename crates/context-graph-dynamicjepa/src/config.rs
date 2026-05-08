use context_graph_core::dynamicjepa::{DynamicJepaError, DynamicJepaResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainConfig {
    pub model: ModelConfig,
    pub loss: LossConfig,
    pub optim: OptimConfig,
    pub schedule: ScheduleConfig,
    pub stopping: StoppingConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality_gates: Vec<MetricQualityGate>,
    pub seed: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelConfig {
    pub encoder: MlpConfig,
    pub predictor: PredictorConfig,
    #[serde(default = "default_target_architecture")]
    pub target_architecture: TargetArchitecture,
    pub ema_momentum: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetArchitecture {
    EmaEncoder,
    FrozenInstrumentProjection,
}

fn default_target_architecture() -> TargetArchitecture {
    TargetArchitecture::EmaEncoder
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MlpConfig {
    pub kind: String,
    pub hidden: Vec<usize>,
    pub out_dim: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictorConfig {
    pub kind: String,
    pub hidden: Vec<usize>,
    pub in_action_dim: usize,
    pub out_dim: usize,
    #[serde(default)]
    pub ignore_action: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LossConfig {
    pub latent_mse_weight: f64,
    pub vicreg_variance_weight: f64,
    pub vicreg_covariance_weight: f64,
    pub vicreg_target_std: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptimConfig {
    pub kind: String,
    pub lr: f64,
    pub weight_decay: f64,
    pub warmup_steps: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleConfig {
    pub epochs: usize,
    pub batch_size: usize,
    pub device: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoppingConfig {
    pub metric: String,
    pub target: f64,
    pub max_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricQualityGate {
    pub metric: String,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
}

impl TrainConfig {
    pub fn from_path(path: &Path) -> DynamicJepaResult<Self> {
        let bytes = fs::read(path).map_err(|err| {
            DynamicJepaError::validation(
                "train.config",
                format!("failed to read training config {}: {err}", path.display()),
                "provide an existing JSON training config file",
            )
        })?;
        let config: Self = serde_json::from_slice(&bytes).map_err(|err| {
            DynamicJepaError::validation(
                "train.config",
                format!("failed to parse strict training config JSON: {err}"),
                "fix the JSON file; unknown keys and missing required fields are rejected",
            )
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn canonical_bytes(&self) -> DynamicJepaResult<Vec<u8>> {
        serde_json::to_vec(self).map_err(|err| {
            DynamicJepaError::validation(
                "train.config",
                format!("failed to serialize canonical training config: {err}"),
                "training config must remain JSON serializable",
            )
        })
    }

    pub fn validate(&self) -> DynamicJepaResult<()> {
        validate_mlp(
            "model.encoder",
            &self.model.encoder.kind,
            &self.model.encoder.hidden,
            self.model.encoder.out_dim,
        )?;
        validate_mlp(
            "model.predictor",
            &self.model.predictor.kind,
            &self.model.predictor.hidden,
            self.model.predictor.out_dim,
        )?;
        if matches!(
            self.model.target_architecture,
            TargetArchitecture::EmaEncoder
        ) && self.model.predictor.out_dim != self.model.encoder.out_dim
        {
            return Err(DynamicJepaError::validation(
                "model.predictor.out_dim",
                format!(
                    "predictor out_dim {} must equal encoder out_dim {} when target_architecture=ema_encoder",
                    self.model.predictor.out_dim, self.model.encoder.out_dim
                ),
                "use matching latent dimensions for EMA encoder targets; frozen instrument targets are checked against the dataset target dimension at train time",
            ));
        }
        if !(0.0..1.0).contains(&self.model.ema_momentum) {
            return Err(DynamicJepaError::validation(
                "model.ema_momentum",
                format!(
                    "ema_momentum must be in [0,1), got {}",
                    self.model.ema_momentum
                ),
                "set the EMA momentum from the checked-in tiny train config",
            ));
        }
        if matches!(
            self.model.target_architecture,
            TargetArchitecture::FrozenInstrumentProjection
        ) && self.model.ema_momentum != 0.0
        {
            return Err(DynamicJepaError::validation(
                "model.ema_momentum",
                format!(
                    "frozen_instrument_projection requires ema_momentum 0.0, got {}",
                    self.model.ema_momentum
                ),
                "set ema_momentum to 0.0 when target_architecture is frozen_instrument_projection",
            ));
        }
        for (field, value) in [
            ("loss.latent_mse_weight", self.loss.latent_mse_weight),
            (
                "loss.vicreg_variance_weight",
                self.loss.vicreg_variance_weight,
            ),
            (
                "loss.vicreg_covariance_weight",
                self.loss.vicreg_covariance_weight,
            ),
            ("loss.vicreg_target_std", self.loss.vicreg_target_std),
            ("optim.lr", self.optim.lr),
            ("optim.weight_decay", self.optim.weight_decay),
            ("stopping.target", self.stopping.target),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(DynamicJepaError::validation(
                    field,
                    format!("{field} must be finite and non-negative, got {value}"),
                    "use the checked-in tiny train config values",
                ));
            }
        }
        if self.optim.kind != "adamw" {
            return Err(DynamicJepaError::validation(
                "optim.kind",
                format!("unsupported optimizer {:?}", self.optim.kind),
                "Phase 7 supports adamw only",
            ));
        }
        if self.schedule.epochs == 0 || self.schedule.batch_size == 0 {
            return Err(DynamicJepaError::validation(
                "schedule",
                "epochs and batch_size must be positive",
                "use positive schedule values",
            ));
        }
        if self.schedule.device != "cuda" {
            return Err(DynamicJepaError::TrainingFailed {
                training_run_id: uuid::Uuid::nil(),
                message: format!("unsupported training device {:?}", self.schedule.device),
                remediation:
                    "Phase 7 5090 demo requires schedule.device=\"cuda\"; do not fall back to CPU"
                        .to_string(),
            });
        }
        if self.stopping.max_seconds == 0 {
            return Err(DynamicJepaError::validation(
                "stopping.max_seconds",
                "max_seconds must be positive",
                "set an explicit training time budget",
            ));
        }
        for (idx, gate) in self.quality_gates.iter().enumerate() {
            gate.validate(idx)?;
        }
        Ok(())
    }
}

impl MetricQualityGate {
    fn validate(&self, idx: usize) -> DynamicJepaResult<()> {
        let field = format!("quality_gates[{idx}]");
        if self.metric.is_empty()
            || !self
                .metric
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
            || !self
                .metric
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_lowercase())
        {
            return Err(DynamicJepaError::validation(
                format!("{field}.metric"),
                format!("quality gate metric {:?} is not a known metric-key shape", self.metric),
                "use a flattened metric key such as val_latent_mse or vicreg_covariance_off_diag_rms",
            ));
        }
        if self.min.is_none() && self.max.is_none() {
            return Err(DynamicJepaError::validation(
                field,
                "quality gate must declare at least one of min or max",
                "declare the physical artifact metric bound that must pass before registration",
            ));
        }
        for (bound, value) in [("min", self.min), ("max", self.max)] {
            if let Some(value) = value {
                if !value.is_finite() {
                    return Err(DynamicJepaError::validation(
                        format!("{field}.{bound}"),
                        format!("quality gate {bound} must be finite, got {value}"),
                        "use finite numeric bounds for artifact quality gates",
                    ));
                }
            }
        }
        if let (Some(min), Some(max)) = (self.min, self.max) {
            if min > max {
                return Err(DynamicJepaError::validation(
                    field,
                    format!("quality gate min {min} exceeds max {max}"),
                    "set min <= max or split the checks into separate gates",
                ));
            }
        }
        Ok(())
    }
}

fn validate_mlp(
    field: &str,
    kind: &str,
    hidden: &[usize],
    out_dim: usize,
) -> DynamicJepaResult<()> {
    if kind != "mlp" {
        return Err(DynamicJepaError::validation(
            format!("{field}.kind"),
            format!("unsupported MLP kind {kind:?}"),
            "Phase 7 supports kind=\"mlp\" only",
        ));
    }
    if hidden.is_empty() || hidden.contains(&0) || out_dim == 0 {
        return Err(DynamicJepaError::validation(
            field,
            "hidden dimensions and out_dim must be positive",
            "use the checked-in tiny train config",
        ));
    }
    Ok(())
}
