use crate::config::{
    MetricQualityGate, MlpConfig, PredictorConfig, TargetArchitecture, TrainConfig,
};
use candle_core::{DType, Device, Tensor, Var};
use candle_nn::{AdamW, Optimizer, ParamsAdamW};
use context_graph_core::dynamicjepa::{DynamicJepaError, DynamicJepaResult};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct TrainExample {
    pub split_name: String,
    pub input_panel: Vec<f32>,
    pub target_panel: Vec<f32>,
    pub action: Vec<f32>,
    pub negative_panel: Vec<f32>,
    pub segments: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EpochMetric {
    pub epoch: usize,
    pub loss: f64,
    pub latent_mse: f64,
    pub vicreg_variance: f64,
    pub vicreg_covariance: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EvaluationReport {
    pub objective_id: String,
    pub target_architecture: TargetArchitecture,
    pub surprise_calibration: SurpriseCalibrationReport,
    pub surprise_segment_calibrations:
        std::collections::BTreeMap<String, SurpriseCalibrationReport>,
    pub split_metrics: std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>>,
    pub skipped_row_count: u32,
    pub skipped_reasons: std::collections::BTreeMap<String, u32>,
    pub random_init_baseline: std::collections::BTreeMap<String, f64>,
    pub shuffled_target_baseline: std::collections::BTreeMap<String, f64>,
    pub vicreg_variance_per_dim_min: f64,
    pub vicreg_variance_per_dim_mean: f64,
    pub vicreg_covariance_off_diag_frobenius: f64,
    pub vicreg_covariance_off_diag_rms: f64,
    pub vicreg_covariance_loss_scale: f64,
    pub collapse_diagnostics: CollapseDiagnostics,
    pub epoch_metrics: Vec<EpochMetric>,
    pub parameter_count_trainable: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SurpriseCalibrationReport {
    pub status: String,
    pub split: String,
    pub threshold_cosine: f64,
    pub percentile: f64,
    pub calibration_set_count: usize,
    pub cosine_min: f64,
    pub cosine_p10: f64,
    pub cosine_median: f64,
    pub cosine_max: f64,
    pub false_positive_budget: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CollapseDiagnostics {
    pub computed: bool,
    pub val_var_min: f64,
    pub val_var_p25: f64,
    pub val_var_median: f64,
    pub val_var_p75: f64,
    pub val_var_max: f64,
    pub covar_frob: f64,
    pub covar_rms: f64,
    pub covar_loss_scale: f64,
    pub eff_rank: f64,
    pub eff_rank_ratio: f64,
    pub alignment: f64,
    pub uniformity: f64,
    pub sigreg_kl: f64,
    pub sigreg_kl_clamped: bool,
    pub latent_collapsed: bool,
}

impl CollapseDiagnostics {
    fn not_computed(variance_metrics: &VarianceCovarianceMetrics) -> Self {
        let mut variances = variance_metrics.variances.clone();
        variances.sort_by(f64::total_cmp);
        Self {
            computed: false,
            val_var_min: variance_metrics.var_min,
            val_var_p25: sorted_quantile(&variances, 0.25),
            val_var_median: sorted_quantile(&variances, 0.50),
            val_var_p75: sorted_quantile(&variances, 0.75),
            val_var_max: *variances.last().unwrap_or(&0.0),
            covar_frob: variance_metrics.cov_off_diag_frobenius,
            covar_rms: variance_metrics.cov_off_diag_rms,
            covar_loss_scale: variance_metrics.cov_loss_scale,
            eff_rank: f64::NAN,
            eff_rank_ratio: f64::NAN,
            alignment: f64::NAN,
            uniformity: f64::NAN,
            sigreg_kl: f64::NAN,
            sigreg_kl_clamped: false,
            latent_collapsed: false,
        }
    }
}

#[derive(Debug)]
pub struct TrainedTinyJepa {
    pub tensors: HashMap<String, Tensor>,
    pub metrics: std::collections::BTreeMap<String, f64>,
    pub evaluation_report: EvaluationReport,
}

struct LinearLayer {
    weight: Var,
    bias: Var,
}

impl LinearLayer {
    fn new(
        in_dim: usize,
        out_dim: usize,
        device: &Device,
        rng: &mut DeterministicInitRng,
    ) -> candle_core::Result<Self> {
        let scale = (2.0f32 / in_dim.max(1) as f32).sqrt();
        let weight_values = (0..out_dim * in_dim)
            .map(|_| rng.normal_f32(0.0, scale))
            .collect::<Vec<_>>();
        let weight = Tensor::from_vec(weight_values, (out_dim, in_dim), device)?;
        Ok(Self {
            weight: Var::from_tensor(&weight)?,
            bias: Var::zeros(out_dim, DType::F32, device)?,
        })
    }

    fn forward(&self, input: &Tensor) -> candle_core::Result<Tensor> {
        input
            .matmul(&self.weight.as_tensor().t()?)?
            .broadcast_add(self.bias.as_tensor())
    }

    fn set_from(&self, source: &Self) -> candle_core::Result<()> {
        self.weight.set(source.weight.as_tensor())?;
        self.bias.set(source.bias.as_tensor())?;
        Ok(())
    }

    fn ema_update(&self, source: &Self, momentum: f64) -> candle_core::Result<()> {
        let next_weight = (self.weight.as_tensor() * momentum)?
            .add(&(source.weight.as_tensor().detach() * (1.0 - momentum))?)?;
        let next_bias = (self.bias.as_tensor() * momentum)?
            .add(&(source.bias.as_tensor().detach() * (1.0 - momentum))?)?;
        self.weight.set(&next_weight)?;
        self.bias.set(&next_bias)?;
        Ok(())
    }

    fn params(&self) -> Vec<Var> {
        vec![self.weight.clone(), self.bias.clone()]
    }

    fn parameter_count(&self) -> usize {
        self.weight.elem_count() + self.bias.elem_count()
    }
}

struct MlpNetwork {
    layers: Vec<LinearLayer>,
    tensor_prefix: &'static str,
}

impl MlpNetwork {
    fn from_dims(
        tensor_prefix: &'static str,
        dims: &[usize],
        device: &Device,
        rng: &mut DeterministicInitRng,
    ) -> candle_core::Result<Self> {
        if dims.len() < 2 {
            candle_core::bail!("MLP requires at least input and output dimensions")
        }
        let layers = dims
            .windows(2)
            .map(|pair| LinearLayer::new(pair[0], pair[1], device, rng))
            .collect::<candle_core::Result<Vec<_>>>()?;
        Ok(Self {
            layers,
            tensor_prefix,
        })
    }

    fn from_encoder_config(
        tensor_prefix: &'static str,
        input_dim: usize,
        config: &MlpConfig,
        device: &Device,
        rng: &mut DeterministicInitRng,
    ) -> candle_core::Result<Self> {
        let mut dims = Vec::with_capacity(config.hidden.len() + 2);
        dims.push(input_dim);
        dims.extend(config.hidden.iter().copied());
        dims.push(config.out_dim);
        Self::from_dims(tensor_prefix, &dims, device, rng)
    }

    fn from_predictor_config(
        tensor_prefix: &'static str,
        latent_dim: usize,
        action_dim: usize,
        config: &PredictorConfig,
        device: &Device,
        rng: &mut DeterministicInitRng,
    ) -> candle_core::Result<Self> {
        let mut dims = Vec::with_capacity(config.hidden.len() + 2);
        dims.push(latent_dim + action_dim);
        dims.extend(config.hidden.iter().copied());
        dims.push(config.out_dim);
        Self::from_dims(tensor_prefix, &dims, device, rng)
    }

    fn forward(&self, input: &Tensor) -> candle_core::Result<Tensor> {
        let mut x = input.clone();
        for (idx, layer) in self.layers.iter().enumerate() {
            x = layer.forward(&x)?;
            if idx + 1 != self.layers.len() {
                x = x.relu()?;
            }
        }
        Ok(x)
    }

    fn set_from(&self, source: &Self) -> candle_core::Result<()> {
        if self.layers.len() != source.layers.len() {
            candle_core::bail!("cannot clone MLP with different layer count")
        }
        for (target, source) in self.layers.iter().zip(source.layers.iter()) {
            target.set_from(source)?;
        }
        Ok(())
    }

    fn ema_update(&self, source: &Self, momentum: f64) -> candle_core::Result<()> {
        if self.layers.len() != source.layers.len() {
            candle_core::bail!("cannot EMA MLP with different layer count")
        }
        for (target, source) in self.layers.iter().zip(source.layers.iter()) {
            target.ema_update(source, momentum)?;
        }
        Ok(())
    }

    fn params(&self) -> Vec<Var> {
        self.layers
            .iter()
            .flat_map(LinearLayer::params)
            .collect::<Vec<_>>()
    }

    fn parameter_count(&self) -> usize {
        self.layers.iter().map(LinearLayer::parameter_count).sum()
    }

    fn tensors(&self) -> HashMap<String, Tensor> {
        let mut tensors = HashMap::new();
        for (idx, layer) in self.layers.iter().enumerate() {
            tensors.insert(
                format!("{}.layer{}.weight", self.tensor_prefix, idx),
                layer.weight.as_detached_tensor(),
            );
            tensors.insert(
                format!("{}.layer{}.bias", self.tensor_prefix, idx),
                layer.bias.as_detached_tensor(),
            );
        }
        tensors
    }
}

struct TinyMlpJepa {
    online_encoder: MlpNetwork,
    target_encoder: Option<MlpNetwork>,
    predictor: MlpNetwork,
}

impl TinyMlpJepa {
    fn new(
        config: &TrainConfig,
        input_dim: usize,
        action_dim: usize,
        device: &Device,
        init_seed: u64,
    ) -> candle_core::Result<Self> {
        let mut rng = DeterministicInitRng::new(init_seed);
        let online_encoder = MlpNetwork::from_encoder_config(
            "online.encoder",
            input_dim,
            &config.model.encoder,
            device,
            &mut rng,
        )?;
        let predictor = MlpNetwork::from_predictor_config(
            "predictor",
            config.model.encoder.out_dim,
            action_dim,
            &config.model.predictor,
            device,
            &mut rng,
        )?;
        let target_encoder = match config.model.target_architecture {
            TargetArchitecture::EmaEncoder => {
                let target_encoder = MlpNetwork::from_encoder_config(
                    "target.encoder",
                    input_dim,
                    &config.model.encoder,
                    device,
                    &mut rng,
                )?;
                target_encoder.set_from(&online_encoder)?;
                Some(target_encoder)
            }
            TargetArchitecture::FrozenInstrumentProjection => None,
        };
        Ok(Self {
            online_encoder,
            target_encoder,
            predictor,
        })
    }

    fn params(&self) -> Vec<Var> {
        let mut params = self.online_encoder.params();
        params.extend(self.predictor.params());
        params
    }

    fn parameter_count_trainable(&self) -> usize {
        self.online_encoder.parameter_count() + self.predictor.parameter_count()
    }

    fn encode_online(&self, input: &Tensor) -> candle_core::Result<Tensor> {
        self.online_encoder.forward(input)
    }

    fn encode_target(&self, config: &TrainConfig, target: &Tensor) -> candle_core::Result<Tensor> {
        match config.model.target_architecture {
            TargetArchitecture::EmaEncoder => {
                let Some(target_encoder) = self.target_encoder.as_ref() else {
                    candle_core::bail!("EMA target encoder is missing");
                };
                target_encoder.forward(target)
            }
            TargetArchitecture::FrozenInstrumentProjection => Ok(target.clone()),
        }
    }

    fn predict(&self, z: &Tensor, action: &Tensor) -> candle_core::Result<Tensor> {
        let joined = Tensor::cat(&[z, action], 1)?;
        self.predictor.forward(&joined)
    }

    fn ema_update(&self, momentum: f64) -> candle_core::Result<()> {
        let Some(target_encoder) = self.target_encoder.as_ref() else {
            candle_core::bail!("EMA target encoder is missing");
        };
        target_encoder.ema_update(&self.online_encoder, momentum)
    }

    fn update_target(&self, config: &TrainConfig) -> candle_core::Result<()> {
        match config.model.target_architecture {
            TargetArchitecture::EmaEncoder => self.ema_update(config.model.ema_momentum),
            TargetArchitecture::FrozenInstrumentProjection => Ok(()),
        }
    }

    fn tensors(&self) -> HashMap<String, Tensor> {
        let mut tensors = self.online_encoder.tensors();
        if let Some(target_encoder) = &self.target_encoder {
            tensors.extend(target_encoder.tensors());
        }
        tensors.extend(self.predictor.tensors());
        tensors
    }
}

struct DeterministicInitRng {
    state: u64,
    spare_normal: Option<f32>,
}

impl DeterministicInitRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0xD1B5_4A32_D192_ED03,
            spare_normal: None,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn uniform_open01(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / ((1u64 << 53) as f64)
    }

    fn normal_f32(&mut self, mean: f32, std: f32) -> f32 {
        if let Some(value) = self.spare_normal.take() {
            return mean + std * value;
        }
        let u1 = self.uniform_open01().max(f64::MIN_POSITIVE);
        let u2 = self.uniform_open01();
        let radius = (-2.0 * u1.ln()).sqrt();
        let theta = std::f64::consts::TAU * u2;
        let z0 = radius * theta.cos();
        let z1 = radius * theta.sin();
        self.spare_normal = Some(z1 as f32);
        mean + std * z0 as f32
    }
}

pub fn train_tiny_jepa(
    config: &TrainConfig,
    objective_id: &str,
    examples: &[TrainExample],
) -> DynamicJepaResult<TrainedTinyJepa> {
    if examples.is_empty() {
        return Err(DynamicJepaError::TrainingFailed {
            training_run_id: uuid::Uuid::nil(),
            message: "training examples are empty".to_string(),
            remediation: "compile non-empty dataset shards before training".to_string(),
        });
    }
    let input_dim = examples[0].input_panel.len();
    let target_dim = examples[0].target_panel.len();
    let action_dim = examples[0].action.len();
    if target_dim != input_dim {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: "dynamicjepa_train".to_string(),
            expected: vec![input_dim],
            actual: vec![target_dim],
        });
    }
    if config.model.predictor.in_action_dim != action_dim {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: "dynamicjepa_train".to_string(),
            expected: vec![config.model.predictor.in_action_dim],
            actual: vec![action_dim],
        });
    }
    if matches!(
        config.model.target_architecture,
        TargetArchitecture::FrozenInstrumentProjection
    ) && config.model.predictor.out_dim != target_dim
    {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: "dynamicjepa_train.frozen_instrument_projection".to_string(),
            expected: vec![target_dim],
            actual: vec![config.model.predictor.out_dim],
        });
    }
    validate_examples(examples, input_dim, target_dim, action_dim)?;
    let zeroed_action_examples = if config.model.predictor.ignore_action {
        Some(
            examples
                .iter()
                .cloned()
                .map(|mut example| {
                    example.action.fill(0.0);
                    example
                })
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };
    let examples = zeroed_action_examples.as_deref().unwrap_or(examples);
    let train_indices = split_indices(examples, "train");
    if train_indices.is_empty() {
        return Err(DynamicJepaError::TrainingFailed {
            training_run_id: uuid::Uuid::nil(),
            message: "dataset does not contain a train split".to_string(),
            remediation: "compile a train dataset shard before training".to_string(),
        });
    }
    ensure_stopping_split_available(examples, &config.stopping.metric)?;

    let device = Device::new_cuda(0).map_err(|err| DynamicJepaError::TrainingFailed {
        training_run_id: uuid::Uuid::nil(),
        message: format!("failed to initialize CUDA device 0: {err}"),
        remediation:
            "confirm nvidia-smi reports compute_cap 12.0 and CUDA_COMPUTE_CAP=120 was used at build time"
                .to_string(),
    })?;
    device
        .set_seed(config.seed)
        .map_err(|err| training_err(format!("failed to seed CUDA RNG: {err}")))?;
    cuda_warmup(&device)?;

    let eval_input = tensor2(
        examples.iter().map(|ex| ex.input_panel.as_slice()),
        examples.len(),
        input_dim,
        &device,
        "eval_input_panels",
    )?;
    let eval_target = tensor2(
        examples.iter().map(|ex| ex.target_panel.as_slice()),
        examples.len(),
        target_dim,
        &device,
        "eval_target_panels",
    )?;
    let eval_action = tensor2(
        examples.iter().map(|ex| ex.action.as_slice()),
        examples.len(),
        action_dim,
        &device,
        "eval_actions",
    )?;
    let eval_negative = tensor2(
        examples.iter().map(|ex| ex.negative_panel.as_slice()),
        examples.len(),
        target_dim,
        &device,
        "eval_negative_panels",
    )?;

    let model = TinyMlpJepa::new(config, input_dim, action_dim, &device, config.seed)
        .map_err(|err| training_err(format!("failed to initialize tiny JEPA MLP: {err}")))?;
    let eval_tensors = EvalTensors {
        input: &eval_input,
        target: &eval_target,
        action: &eval_action,
        negative: &eval_negative,
    };
    let baseline_metrics = evaluate_model(config, &model, examples, eval_tensors, false)?;

    let mut opt = AdamW::new(
        model.params(),
        ParamsAdamW {
            lr: config.optim.lr,
            weight_decay: config.optim.weight_decay,
            ..ParamsAdamW::default()
        },
    )
    .map_err(|err| training_err(format!("failed to initialize AdamW: {err}")))?;

    let start = Instant::now();
    let mut epoch_metrics = Vec::new();
    let mut global_step = 0u64;
    let steps_per_epoch = train_indices
        .len()
        .div_ceil(config.schedule.batch_size)
        .max(1);
    let eval_interval_epochs = stopping_eval_interval(config.schedule.epochs);
    let mut converged = false;
    for epoch in 0..config.schedule.epochs {
        let epoch_indices = shuffled_epoch_indices(&train_indices, config.seed, epoch as u64);
        let mut epoch_loss = None;
        let mut epoch_mse = None;
        let mut epoch_var = None;
        let mut epoch_cov = None;
        for step in 0..steps_per_epoch {
            global_step += 1;
            apply_lr_schedule(&mut opt, config, global_step);
            let batch = build_batch_tensors(BatchTensorRequest {
                examples,
                train_indices: &epoch_indices,
                start: step * config.schedule.batch_size,
                batch_size: config.schedule.batch_size,
                input_dim,
                target_dim,
                action_dim,
                device: &device,
            })?;
            let z_online = model
                .encode_online(&batch.input)
                .map_err(|err| training_err(format!("encoder forward failed: {err}")))?;
            let z_target = model
                .encode_target(config, &batch.target)
                .map_err(|err| training_err(format!("target encoder forward failed: {err}")))?
                .detach();
            let z_hat = model
                .predict(&z_online, &batch.action)
                .map_err(|err| training_err(format!("predictor forward failed: {err}")))?;
            let latent_mse = mse_tensor(&z_hat, &z_target)
                .map_err(|err| training_err(format!("latent MSE failed: {err}")))?;
            let vic_var = vicreg_variance(&z_online, config.loss.vicreg_target_std)
                .map_err(|err| training_err(format!("VICReg variance failed: {err}")))?;
            let vic_cov = vicreg_covariance(&z_online)
                .map_err(|err| training_err(format!("VICReg covariance failed: {err}")))?;
            let mse_loss = (&latent_mse * config.loss.latent_mse_weight)
                .map_err(|err| training_err(format!("latent MSE weighting failed: {err}")))?;
            let var_loss = (&vic_var * config.loss.vicreg_variance_weight)
                .map_err(|err| training_err(format!("VICReg variance weighting failed: {err}")))?;
            let cov_loss = (&vic_cov * config.loss.vicreg_covariance_weight).map_err(|err| {
                training_err(format!("VICReg covariance weighting failed: {err}"))
            })?;
            let loss = mse_loss
                .add(&var_loss)
                .and_then(|loss| loss.add(&cov_loss))
                .map_err(|err| training_err(format!("loss assembly failed: {err}")))?;
            opt.backward_step(&loss)
                .map_err(|err| training_err(format!("AdamW backward_step failed: {err}")))?;
            model
                .update_target(config)
                .map_err(|err| training_err(format!("target branch update failed: {err}")))?;
            device
                .synchronize()
                .map_err(|err| training_err(format!("CUDA synchronize failed: {err}")))?;
            epoch_loss = accumulate_epoch_scalar(epoch_loss, &loss, "loss")?;
            epoch_mse = accumulate_epoch_scalar(epoch_mse, &latent_mse, "latent_mse")?;
            epoch_var = accumulate_epoch_scalar(epoch_var, &vic_var, "vicreg_variance")?;
            epoch_cov = accumulate_epoch_scalar(epoch_cov, &vic_cov, "vicreg_covariance")?;
            if start.elapsed().as_secs_f64() > config.stopping.max_seconds as f64 {
                return Err(DynamicJepaError::TrainingFailed {
                    training_run_id: uuid::Uuid::nil(),
                    message: format!(
                        "max_seconds budget {} exceeded before convergence at epoch {epoch} step {step}",
                        config.stopping.max_seconds
                    ),
                    remediation:
                        "check CUDA utilization and inspect metrics.json for collapse or divergence"
                            .to_string(),
                });
            }
        }
        let denom = steps_per_epoch as f64;
        let epoch_loss = mean_epoch_scalar(epoch_loss, denom, "loss").map_err(training_err)?;
        let epoch_mse = mean_epoch_scalar(epoch_mse, denom, "latent_mse").map_err(training_err)?;
        let epoch_var =
            mean_epoch_scalar(epoch_var, denom, "vicreg_variance").map_err(training_err)?;
        let epoch_cov =
            mean_epoch_scalar(epoch_cov, denom, "vicreg_covariance").map_err(training_err)?;
        epoch_metrics.push(EpochMetric {
            epoch,
            loss: epoch_loss,
            latent_mse: epoch_mse,
            vicreg_variance: epoch_var,
            vicreg_covariance: epoch_cov,
        });
        let completed_epoch = epoch + 1;
        let should_eval_stopping = completed_epoch % eval_interval_epochs == 0
            || completed_epoch == config.schedule.epochs;
        if should_eval_stopping {
            let epoch_eval = evaluate_model(config, &model, examples, eval_tensors, false)?;
            if start.elapsed().as_secs_f64() > config.stopping.max_seconds as f64 {
                return Err(DynamicJepaError::TrainingFailed {
                    training_run_id: uuid::Uuid::nil(),
                    message: format!(
                        "max_seconds budget {} exceeded before convergence after epoch {epoch} evaluation",
                        config.stopping.max_seconds
                    ),
                    remediation: "check CUDA utilization and inspect metrics.json for collapse or divergence"
                        .to_string(),
                });
            }
            let stopping_value = metric_value(&epoch_eval, &config.stopping.metric)?;
            if stopping_value <= config.stopping.target
                && quality_gates_satisfied(&epoch_eval, &config.quality_gates)?
            {
                converged = true;
                break;
            }
        }
    }

    let mut evaluation = evaluate_model(config, &model, examples, eval_tensors, true)?;
    let final_stopping_value = metric_value(&evaluation, &config.stopping.metric)?;
    if !converged && final_stopping_value > config.stopping.target {
        return Err(DynamicJepaError::TrainingFailed {
            training_run_id: uuid::Uuid::nil(),
            message: format!(
                "stopping metric {} did not converge: final={final_stopping_value:.6} target={:.6}",
                config.stopping.metric, config.stopping.target
            ),
            remediation:
                "inspect evaluation_report.json and either fix the model/data issue or choose a justified config"
                    .to_string(),
        });
    }
    evaluation.objective_id = objective_id.to_string();
    evaluation.target_architecture = config.model.target_architecture.clone();
    let random_init_cosine =
        first_split_metric(&baseline_metrics, "latent_cosine").ok_or_else(|| {
            DynamicJepaError::TrainingFailed {
                training_run_id: uuid::Uuid::nil(),
                message: "random-init baseline did not produce latent_cosine".to_string(),
                remediation:
                    "inspect evaluation split metrics; missing baseline metrics must not be treated as zero"
                        .to_string(),
            }
        })?;
    evaluation.random_init_baseline =
        std::collections::BTreeMap::from([("cosine".to_string(), random_init_cosine)]);
    evaluation.epoch_metrics = epoch_metrics;
    evaluation.parameter_count_trainable = model.parameter_count_trainable();
    let metrics = flatten_metrics(&evaluation)?;
    enforce_quality_gates(&metrics, &config.quality_gates)?;
    Ok(TrainedTinyJepa {
        tensors: model.tensors(),
        metrics,
        evaluation_report: evaluation,
    })
}

pub fn random_init_tiny_jepa_tensors(
    config: &TrainConfig,
    input_dim: usize,
    action_dim: usize,
    seed: u64,
) -> DynamicJepaResult<HashMap<String, Tensor>> {
    config.validate()?;
    if config.model.predictor.in_action_dim != action_dim {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: "dynamicjepa_random_init".to_string(),
            expected: vec![config.model.predictor.in_action_dim],
            actual: vec![action_dim],
        });
    }
    let device = Device::new_cuda(0).map_err(|err| DynamicJepaError::TrainingFailed {
        training_run_id: uuid::Uuid::nil(),
        message: format!("failed to initialize CUDA device 0 for random-init baseline: {err}"),
        remediation:
            "confirm nvidia-smi reports compute_cap 12.0 and CUDA_COMPUTE_CAP=120 was used at build time"
                .to_string(),
    })?;
    device
        .set_seed(seed)
        .map_err(|err| training_err(format!("failed to seed CUDA RNG: {err}")))?;
    cuda_warmup(&device)?;
    let model = TinyMlpJepa::new(config, input_dim, action_dim, &device, seed).map_err(|err| {
        training_err(format!(
            "failed to initialize random-init tiny JEPA MLP: {err}"
        ))
    })?;
    Ok(model.tensors())
}

struct BatchTensors {
    input: Tensor,
    target: Tensor,
    action: Tensor,
}

struct BatchTensorRequest<'a> {
    examples: &'a [TrainExample],
    train_indices: &'a [usize],
    start: usize,
    batch_size: usize,
    input_dim: usize,
    target_dim: usize,
    action_dim: usize,
    device: &'a Device,
}

fn build_batch_tensors(request: BatchTensorRequest<'_>) -> DynamicJepaResult<BatchTensors> {
    if request.train_indices.is_empty() {
        return Err(training_err(
            "cannot build a batch with zero train rows".to_string(),
        ));
    }
    let mut inputs = Vec::with_capacity(request.batch_size * request.input_dim);
    let mut targets = Vec::with_capacity(request.batch_size * request.target_dim);
    let mut actions = Vec::with_capacity(request.batch_size * request.action_dim);
    for offset in 0..request.batch_size {
        let idx = request.train_indices[(request.start + offset) % request.train_indices.len()];
        inputs.extend_from_slice(&request.examples[idx].input_panel);
        targets.extend_from_slice(&request.examples[idx].target_panel);
        actions.extend_from_slice(&request.examples[idx].action);
    }
    Ok(BatchTensors {
        input: Tensor::from_vec(
            inputs,
            (request.batch_size, request.input_dim),
            request.device,
        )
        .map_err(|err| training_err(format!("failed to create batch input tensor: {err}")))?,
        target: Tensor::from_vec(
            targets,
            (request.batch_size, request.target_dim),
            request.device,
        )
        .map_err(|err| training_err(format!("failed to create batch target tensor: {err}")))?,
        action: Tensor::from_vec(
            actions,
            (request.batch_size, request.action_dim),
            request.device,
        )
        .map_err(|err| training_err(format!("failed to create batch action tensor: {err}")))?,
    })
}

fn apply_lr_schedule(opt: &mut AdamW, config: &TrainConfig, global_step: u64) {
    if config.optim.warmup_steps == 0 {
        opt.set_learning_rate(config.optim.lr);
        return;
    }
    let warmup = config.optim.warmup_steps as f64;
    let scale = (global_step as f64 / warmup).clamp(0.0, 1.0);
    opt.set_learning_rate(config.optim.lr * scale);
}

fn stopping_eval_interval(epochs: usize) -> usize {
    epochs.div_ceil(50).max(1)
}

fn accumulate_epoch_scalar(
    acc: Option<Tensor>,
    value: &Tensor,
    name: &'static str,
) -> DynamicJepaResult<Option<Tensor>> {
    let value = value.detach();
    let next = match acc {
        Some(acc) => acc
            .add(&value)
            .map_err(|err| training_err(format!("failed to accumulate {name}: {err}")))?,
        None => value,
    };
    Ok(Some(next))
}

fn mean_epoch_scalar(acc: Option<Tensor>, denom: f64, name: &'static str) -> Result<f64, String> {
    let acc = acc.ok_or_else(|| format!("epoch did not accumulate {name}"))?;
    let mean = (&acc / denom).map_err(|err| format!("failed to average {name}: {err}"))?;
    scalar_f64(&mean).map_err(|err| format!("failed to read {name}: {err}"))
}

fn validate_examples(
    examples: &[TrainExample],
    input_dim: usize,
    target_dim: usize,
    action_dim: usize,
) -> DynamicJepaResult<()> {
    for (idx, example) in examples.iter().enumerate() {
        if example.input_panel.len() != input_dim
            || example.target_panel.len() != target_dim
            || example.negative_panel.len() != target_dim
            || example.action.len() != action_dim
        {
            return Err(DynamicJepaError::PanelShapeMismatch {
                domain_pack_id: "dynamicjepa_train".to_string(),
                expected: vec![input_dim, target_dim, target_dim, action_dim],
                actual: vec![
                    example.input_panel.len(),
                    example.target_panel.len(),
                    example.negative_panel.len(),
                    example.action.len(),
                ],
            });
        }
        for (field, values) in [
            ("input_panel", &example.input_panel),
            ("target_panel", &example.target_panel),
            ("negative_panel", &example.negative_panel),
            ("action", &example.action),
        ] {
            if values.iter().any(|value| !value.is_finite()) {
                return Err(DynamicJepaError::TrainingFailed {
                    training_run_id: uuid::Uuid::nil(),
                    message: format!("row {idx} {field} contains NaN or infinity"),
                    remediation: "inspect the persisted panel/action source-of-truth rows"
                        .to_string(),
                });
            }
        }
        if example.target_panel == example.negative_panel {
            return Err(DynamicJepaError::TrainingFailed {
                training_run_id: uuid::Uuid::nil(),
                message: format!(
                    "row {idx} target_panel and negative_panel are identical; negative-control audit would be invalid"
                ),
                remediation:
                    "recompile the dataset and inspect negative_panel_ids for same-outcome leakage"
                        .to_string(),
            });
        }
    }
    Ok(())
}

fn split_indices(examples: &[TrainExample], split: &str) -> Vec<usize> {
    examples
        .iter()
        .enumerate()
        .filter_map(|(idx, example)| (example.split_name == split).then_some(idx))
        .collect()
}

fn shuffled_epoch_indices(indices: &[usize], seed: u64, epoch: u64) -> Vec<usize> {
    let mut out = indices.to_vec();
    if out.len() <= 1 {
        return out;
    }
    let mut state = seed ^ epoch.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for idx in (1..out.len()).rev() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let swap_idx = (state as usize) % (idx + 1);
        out.swap(idx, swap_idx);
    }
    out
}

fn ensure_stopping_split_available(
    examples: &[TrainExample],
    metric: &str,
) -> DynamicJepaResult<()> {
    let (split, _) = metric.split_once('_').ok_or_else(|| {
        DynamicJepaError::validation(
            "stopping.metric",
            format!("metric {metric:?} must be formatted as <split>_<metric_name>"),
            "use a metric such as val_latent_mse",
        )
    })?;
    if examples.iter().any(|example| example.split_name == split) {
        return Ok(());
    }
    Err(DynamicJepaError::TrainingFailed {
        training_run_id: uuid::Uuid::nil(),
        message: format!("dataset does not contain stopping metric split {split:?}"),
        remediation: format!("compile a {split} dataset shard or change stopping.metric"),
    })
}

fn cuda_warmup(device: &Device) -> DynamicJepaResult<()> {
    let a = Tensor::ones((2, 2), DType::F32, device)
        .map_err(|err| training_err(format!("CUDA warmup allocation failed: {err}")))?;
    let _ = a
        .matmul(&a)
        .map_err(|err| training_err(format!("CUDA warmup matmul failed: {err}")))?;
    device
        .synchronize()
        .map_err(|err| training_err(format!("CUDA warmup synchronize failed: {err}")))?;
    Ok(())
}

fn tensor2<'a, I>(
    rows: I,
    row_count: usize,
    dim: usize,
    device: &Device,
    name: &'static str,
) -> DynamicJepaResult<Tensor>
where
    I: Iterator<Item = &'a [f32]>,
{
    let mut flat = Vec::with_capacity(row_count * dim);
    for row in rows {
        flat.extend_from_slice(row);
    }
    Tensor::from_vec(flat, (row_count, dim), device)
        .map_err(|err| training_err(format!("failed to create {name} tensor: {err}")))
}

fn mse_tensor(lhs: &Tensor, rhs: &Tensor) -> candle_core::Result<Tensor> {
    (lhs - rhs)?.sqr()?.mean_all()
}

fn vicreg_variance(z: &Tensor, target_std: f64) -> candle_core::Result<Tensor> {
    let variance = z.var(0)?;
    (Tensor::from_vec(vec![target_std as f32], 1, z.device())?
        .broadcast_sub(&(variance + 1e-4)?.sqrt()?)?)
    .relu()?
    .mean_all()
}

fn vicreg_covariance(z: &Tensor) -> candle_core::Result<Tensor> {
    let dims = z.dims();
    let batch = dims[0];
    let latent = dims[1];
    if batch <= 1 {
        return Tensor::zeros((), DType::F32, z.device());
    }
    let centered = z.broadcast_sub(&z.mean_keepdim(0)?)?;
    let cov = (centered.t()?.matmul(&centered)? / ((batch - 1) as f64))?;
    let eye = Tensor::eye(latent, DType::F32, z.device())?;
    let mask = (Tensor::ones((latent, latent), DType::F32, z.device())? - eye)?;
    let off_diag = (cov.sqr()? * mask)?.sum_all()?;
    off_diag / (latent as f64)
}

#[derive(Clone, Copy)]
struct EvalTensors<'a> {
    input: &'a Tensor,
    target: &'a Tensor,
    action: &'a Tensor,
    negative: &'a Tensor,
}

fn evaluate_model(
    config: &TrainConfig,
    model: &TinyMlpJepa,
    examples: &[TrainExample],
    tensors: EvalTensors<'_>,
    compute_collapse_diagnostics: bool,
) -> DynamicJepaResult<EvaluationReport> {
    let z_online = model
        .encode_online(tensors.input)
        .map_err(|err| training_err(format!("evaluation encoder forward failed: {err}")))?;
    let z_target = model
        .encode_target(config, tensors.target)
        .map_err(|err| training_err(format!("evaluation target forward failed: {err}")))?;
    let z_negative = model
        .encode_target(config, tensors.negative)
        .map_err(|err| training_err(format!("evaluation negative forward failed: {err}")))?;
    let z_hat = model
        .predict(&z_online, tensors.action)
        .map_err(|err| training_err(format!("evaluation predictor forward failed: {err}")))?;
    let pred = z_hat
        .to_vec2::<f32>()
        .map_err(|err| training_err(format!("evaluation prediction readback failed: {err}")))?;
    let target = z_target
        .to_vec2::<f32>()
        .map_err(|err| training_err(format!("evaluation target readback failed: {err}")))?;
    let negative = z_negative
        .to_vec2::<f32>()
        .map_err(|err| training_err(format!("evaluation negative readback failed: {err}")))?;
    let online = z_online
        .to_vec2::<f32>()
        .map_err(|err| training_err(format!("evaluation online readback failed: {err}")))?;

    let mut by_split: std::collections::BTreeMap<String, Vec<usize>> =
        std::collections::BTreeMap::new();
    for (idx, example) in examples.iter().enumerate() {
        by_split
            .entry(example.split_name.clone())
            .or_default()
            .push(idx);
    }
    let mut split_metrics = std::collections::BTreeMap::new();
    for (split, idxs) in &by_split {
        let mut mse = 0.0;
        let mut cosine_sum = 0.0;
        let mut contrast_hits = 0u64;
        let mut mismatched_cosine_sum = 0.0;
        let target_mean = mean_vector(idxs.iter().map(|idx| target[*idx].as_slice()));
        for idx in idxs {
            mse += mse_vec(&pred[*idx], &target[*idx]);
            cosine_sum += cosine(&pred[*idx], &target[*idx]);
            let pos = cosine(&pred[*idx], &target[*idx]);
            let neg = cosine(&pred[*idx], &negative[*idx]);
            if pos > neg {
                contrast_hits += 1;
            }
            let centered_pred = subtract_mean(&pred[*idx], &target_mean);
            let centered_negative = subtract_mean(&negative[*idx], &target_mean);
            mismatched_cosine_sum += cosine(&centered_pred, &centered_negative);
        }
        let denom = idxs.len() as f64;
        split_metrics.insert(
            split.clone(),
            std::collections::BTreeMap::from([
                ("latent_mse".to_string(), mse / denom),
                ("latent_cosine".to_string(), cosine_sum / denom),
                (
                    "action_contrast_acc".to_string(),
                    contrast_hits as f64 / denom,
                ),
                (
                    "shuffled_target_cosine".to_string(),
                    mismatched_cosine_sum / denom,
                ),
            ]),
        );
    }
    let variance_metrics = variance_covariance_metrics(&online);
    let collapse_diagnostics = if compute_collapse_diagnostics {
        collapse_diagnostics(&pred, &target, &online, &variance_metrics)
    } else {
        CollapseDiagnostics::not_computed(&variance_metrics)
    };
    let shuffled_overall = split_metrics
        .values()
        .map(|metrics| metrics["shuffled_target_cosine"])
        .sum::<f64>()
        / split_metrics.len().max(1) as f64;
    Ok(EvaluationReport {
        objective_id: String::new(),
        target_architecture: TargetArchitecture::EmaEncoder,
        surprise_calibration: surprise_calibration_report(examples, &pred, &target),
        surprise_segment_calibrations: surprise_segment_calibration_reports(
            examples, &pred, &target,
        ),
        split_metrics,
        skipped_row_count: 0,
        skipped_reasons: std::collections::BTreeMap::new(),
        random_init_baseline: std::collections::BTreeMap::new(),
        shuffled_target_baseline: std::collections::BTreeMap::from([(
            "cosine".to_string(),
            shuffled_overall,
        )]),
        vicreg_variance_per_dim_min: variance_metrics.var_min,
        vicreg_variance_per_dim_mean: variance_metrics.var_mean,
        vicreg_covariance_off_diag_frobenius: variance_metrics.cov_off_diag_frobenius,
        vicreg_covariance_off_diag_rms: variance_metrics.cov_off_diag_rms,
        vicreg_covariance_loss_scale: variance_metrics.cov_loss_scale,
        collapse_diagnostics,
        epoch_metrics: Vec::new(),
        parameter_count_trainable: 0,
    })
}

fn metric_value(report: &EvaluationReport, metric: &str) -> DynamicJepaResult<f64> {
    let (split, name) = metric.split_once('_').ok_or_else(|| {
        DynamicJepaError::validation(
            "stopping.metric",
            format!("metric {metric:?} must be formatted as <split>_<metric_name>"),
            "use a metric such as val_latent_mse",
        )
    })?;
    report
        .split_metrics
        .get(split)
        .and_then(|metrics| metrics.get(name))
        .copied()
        .ok_or_else(|| DynamicJepaError::TrainingFailed {
            training_run_id: uuid::Uuid::nil(),
            message: format!("stopping metric {metric:?} was not produced by evaluation"),
            remediation: "compile the requested split and use a known metric name".to_string(),
        })
}

fn quality_gates_satisfied(
    report: &EvaluationReport,
    gates: &[MetricQualityGate],
) -> DynamicJepaResult<bool> {
    for gate in gates {
        let value = report_metric_value(report, &gate.metric)?;
        if !metric_gate_passes(value, gate) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn enforce_quality_gates(
    metrics: &std::collections::BTreeMap<String, f64>,
    gates: &[MetricQualityGate],
) -> DynamicJepaResult<()> {
    let mut failures = Vec::new();
    for gate in gates {
        let value =
            metrics
                .get(&gate.metric)
                .copied()
                .ok_or_else(|| {
                    DynamicJepaError::TrainingFailed {
                training_run_id: uuid::Uuid::nil(),
                message: format!(
                    "quality gate metric {:?} was not produced by final evaluation",
                    gate.metric
                ),
                remediation:
                    "fix the training metric name or add the required metric to evaluation output"
                        .to_string(),
            }
                })?;
        if !value.is_finite() {
            return Err(DynamicJepaError::TrainingFailed {
                training_run_id: uuid::Uuid::nil(),
                message: format!("quality gate metric {} is not finite: {value}", gate.metric),
                remediation:
                    "inspect evaluation_report.json for NaN/Inf and fix the training data or loss"
                        .to_string(),
            });
        }
        if !metric_gate_passes(value, gate) {
            failures.push(format!(
                "{} value={value:.12} min={} max={}",
                gate.metric,
                format_optional_bound(gate.min),
                format_optional_bound(gate.max)
            ));
        }
    }
    if failures.is_empty() {
        return Ok(());
    }
    Err(DynamicJepaError::TrainingFailed {
        training_run_id: uuid::Uuid::nil(),
        message: format!(
            "quality gates failed: {}; final_metrics={}",
            failures.join("; "),
            format_metric_snapshot(metrics)
        ),
        remediation:
            "do not register the artifact; fix the training config or data until every declared quality gate passes"
                .to_string(),
    })
}

fn format_metric_snapshot(metrics: &std::collections::BTreeMap<String, f64>) -> String {
    metrics
        .iter()
        .map(|(key, value)| format!("{key}={value:.12}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn report_metric_value(report: &EvaluationReport, metric: &str) -> DynamicJepaResult<f64> {
    let value = match metric {
        "shuffled_target_cosine" => report
            .shuffled_target_baseline
            .get("cosine")
            .copied()
            .ok_or_else(|| DynamicJepaError::TrainingFailed {
                training_run_id: uuid::Uuid::nil(),
                message: "evaluation report is missing shuffled_target_baseline.cosine".to_string(),
                remediation: "fix evaluation_model output; missing baseline metrics must fail"
                    .to_string(),
            })?,
        "vicreg_variance_per_dim_min" => report.vicreg_variance_per_dim_min,
        "vicreg_variance_per_dim_mean" => report.vicreg_variance_per_dim_mean,
        "vicreg_covariance_off_diag_frobenius" => report.vicreg_covariance_off_diag_frobenius,
        "vicreg_covariance_off_diag_rms" => report.vicreg_covariance_off_diag_rms,
        "vicreg_covariance_loss_scale" => report.vicreg_covariance_loss_scale,
        "collapse_diagnostics_computed" => {
            if report.collapse_diagnostics.computed {
                1.0
            } else {
                0.0
            }
        }
        "collapse_val_var_min" => report.collapse_diagnostics.val_var_min,
        "collapse_val_var_p25" => report.collapse_diagnostics.val_var_p25,
        "collapse_val_var_median" => report.collapse_diagnostics.val_var_median,
        "collapse_val_var_p75" => report.collapse_diagnostics.val_var_p75,
        "collapse_val_var_max" => report.collapse_diagnostics.val_var_max,
        "collapse_covar_frob" => report.collapse_diagnostics.covar_frob,
        "collapse_covar_rms" => report.collapse_diagnostics.covar_rms,
        "collapse_covar_loss_scale" => report.collapse_diagnostics.covar_loss_scale,
        "collapse_eff_rank" => report.collapse_diagnostics.eff_rank,
        "collapse_eff_rank_ratio" => report.collapse_diagnostics.eff_rank_ratio,
        "collapse_alignment" => report.collapse_diagnostics.alignment,
        "collapse_uniformity" => report.collapse_diagnostics.uniformity,
        "collapse_sigreg_kl" => report.collapse_diagnostics.sigreg_kl,
        "collapse_sigreg_kl_clamped" => {
            if report.collapse_diagnostics.sigreg_kl_clamped {
                1.0
            } else {
                0.0
            }
        }
        "collapse_latent_collapsed" => {
            if report.collapse_diagnostics.latent_collapsed {
                1.0
            } else {
                0.0
            }
        }
        "parameter_count_trainable" => report.parameter_count_trainable as f64,
        _ => {
            let (split, name) = metric.split_once('_').ok_or_else(|| {
                DynamicJepaError::TrainingFailed {
                    training_run_id: uuid::Uuid::nil(),
                    message: format!(
                        "quality gate metric {metric:?} must be formatted as <split>_<metric_name> or be a known global metric"
                    ),
                    remediation:
                        "use a produced metric such as val_latent_mse or vicreg_covariance_off_diag_rms"
                            .to_string(),
                }
            })?;
            report
                .split_metrics
                .get(split)
                .and_then(|metrics| metrics.get(name))
                .copied()
                .ok_or_else(|| DynamicJepaError::TrainingFailed {
                    training_run_id: uuid::Uuid::nil(),
                    message: format!(
                        "quality gate metric {metric:?} was not produced by evaluation"
                    ),
                    remediation: "compile the requested split and use a known metric name"
                        .to_string(),
                })?
        }
    };
    if !value.is_finite() {
        return Err(DynamicJepaError::TrainingFailed {
            training_run_id: uuid::Uuid::nil(),
            message: format!("quality gate metric {metric} is not finite: {value}"),
            remediation:
                "inspect evaluation_report.json for NaN/Inf and fix the training data or loss"
                    .to_string(),
        });
    }
    Ok(value)
}

fn metric_gate_passes(value: f64, gate: &MetricQualityGate) -> bool {
    gate.min.is_none_or(|min| value >= min) && gate.max.is_none_or(|max| value <= max)
}

fn format_optional_bound(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.12}"))
        .unwrap_or_else(|| "unset".to_string())
}

fn first_split_metric(report: &EvaluationReport, metric: &str) -> Option<f64> {
    report
        .split_metrics
        .values()
        .next()
        .and_then(|metrics| metrics.get(metric))
        .copied()
}

fn flatten_metrics(
    report: &EvaluationReport,
) -> DynamicJepaResult<std::collections::BTreeMap<String, f64>> {
    let mut out = std::collections::BTreeMap::new();
    for (split, metrics) in &report.split_metrics {
        for (metric, value) in metrics {
            out.insert(format!("{split}_{metric}"), *value);
        }
    }
    let shuffled_target_cosine = report
        .shuffled_target_baseline
        .get("cosine")
        .copied()
        .ok_or_else(|| DynamicJepaError::TrainingFailed {
            training_run_id: uuid::Uuid::nil(),
            message: "evaluation report is missing shuffled_target_baseline.cosine".to_string(),
            remediation: "fix evaluation_model output; missing baseline metrics must fail"
                .to_string(),
        })?;
    let random_init_cosine = report
        .random_init_baseline
        .get("cosine")
        .copied()
        .ok_or_else(|| DynamicJepaError::TrainingFailed {
            training_run_id: uuid::Uuid::nil(),
            message: "evaluation report is missing random_init_baseline.cosine".to_string(),
            remediation: "fix training baseline evaluation; missing baseline metrics must fail"
                .to_string(),
        })?;
    out.insert("shuffled_target_cosine".to_string(), shuffled_target_cosine);
    out.insert("random_init_cosine".to_string(), random_init_cosine);
    out.insert(
        "vicreg_variance_per_dim_min".to_string(),
        report.vicreg_variance_per_dim_min,
    );
    out.insert(
        "vicreg_variance_per_dim_mean".to_string(),
        report.vicreg_variance_per_dim_mean,
    );
    out.insert(
        "vicreg_covariance_off_diag_frobenius".to_string(),
        report.vicreg_covariance_off_diag_frobenius,
    );
    out.insert(
        "vicreg_covariance_off_diag_rms".to_string(),
        report.vicreg_covariance_off_diag_rms,
    );
    out.insert(
        "vicreg_covariance_loss_scale".to_string(),
        report.vicreg_covariance_loss_scale,
    );
    out.insert(
        "collapse_val_var_min".to_string(),
        report.collapse_diagnostics.val_var_min,
    );
    out.insert(
        "collapse_diagnostics_computed".to_string(),
        if report.collapse_diagnostics.computed {
            1.0
        } else {
            0.0
        },
    );
    out.insert(
        "collapse_val_var_p25".to_string(),
        report.collapse_diagnostics.val_var_p25,
    );
    out.insert(
        "collapse_val_var_median".to_string(),
        report.collapse_diagnostics.val_var_median,
    );
    out.insert(
        "collapse_val_var_p75".to_string(),
        report.collapse_diagnostics.val_var_p75,
    );
    out.insert(
        "collapse_val_var_max".to_string(),
        report.collapse_diagnostics.val_var_max,
    );
    out.insert(
        "collapse_covar_frob".to_string(),
        report.collapse_diagnostics.covar_frob,
    );
    out.insert(
        "collapse_covar_rms".to_string(),
        report.collapse_diagnostics.covar_rms,
    );
    out.insert(
        "collapse_covar_loss_scale".to_string(),
        report.collapse_diagnostics.covar_loss_scale,
    );
    out.insert(
        "collapse_eff_rank".to_string(),
        report.collapse_diagnostics.eff_rank,
    );
    out.insert(
        "collapse_eff_rank_ratio".to_string(),
        report.collapse_diagnostics.eff_rank_ratio,
    );
    out.insert(
        "collapse_alignment".to_string(),
        report.collapse_diagnostics.alignment,
    );
    out.insert(
        "collapse_uniformity".to_string(),
        report.collapse_diagnostics.uniformity,
    );
    out.insert(
        "collapse_sigreg_kl".to_string(),
        report.collapse_diagnostics.sigreg_kl,
    );
    out.insert(
        "collapse_sigreg_kl_clamped".to_string(),
        if report.collapse_diagnostics.sigreg_kl_clamped {
            1.0
        } else {
            0.0
        },
    );
    out.insert(
        "collapse_latent_collapsed".to_string(),
        if report.collapse_diagnostics.latent_collapsed {
            1.0
        } else {
            0.0
        },
    );
    out.insert(
        "parameter_count_trainable".to_string(),
        report.parameter_count_trainable as f64,
    );
    if report.surprise_calibration.status == "calibrated" {
        out.insert(
            "surprise_threshold_cosine".to_string(),
            report.surprise_calibration.threshold_cosine,
        );
        out.insert(
            "surprise_calibration_set_count".to_string(),
            report.surprise_calibration.calibration_set_count as f64,
        );
    }
    out.insert(
        "surprise_segment_calibration_count".to_string(),
        report
            .surprise_segment_calibrations
            .values()
            .filter(|report| report.status == "calibrated")
            .count() as f64,
    );
    Ok(out)
}

#[derive(Debug, Clone)]
struct VarianceCovarianceMetrics {
    means: Vec<f64>,
    variances: Vec<f64>,
    covariance: Vec<Vec<f64>>,
    var_min: f64,
    var_mean: f64,
    cov_off_diag_frobenius: f64,
    cov_off_diag_rms: f64,
    cov_loss_scale: f64,
}

fn variance_covariance_metrics(rows: &[Vec<f32>]) -> VarianceCovarianceMetrics {
    if rows.is_empty() || rows[0].is_empty() {
        return VarianceCovarianceMetrics {
            means: Vec::new(),
            variances: Vec::new(),
            covariance: Vec::new(),
            var_min: 0.0,
            var_mean: 0.0,
            cov_off_diag_frobenius: 0.0,
            cov_off_diag_rms: 0.0,
            cov_loss_scale: 0.0,
        };
    }
    let n = rows.len();
    let d = rows[0].len();
    let mut means = vec![0.0f64; d];
    for row in rows {
        for (idx, value) in row.iter().enumerate() {
            means[idx] += *value as f64;
        }
    }
    for mean in &mut means {
        *mean /= n as f64;
    }
    let mut vars = vec![0.0f64; d];
    for row in rows {
        for (idx, value) in row.iter().enumerate() {
            let diff = *value as f64 - means[idx];
            vars[idx] += diff * diff;
        }
    }
    for var in &mut vars {
        *var /= n.max(1) as f64;
    }
    let var_min = vars.iter().copied().fold(f64::INFINITY, f64::min);
    let var_mean = vars.iter().sum::<f64>() / d as f64;
    let mut covariance = vec![vec![0.0f64; d]; d];
    let mut off_diag = 0.0;
    if n > 1 {
        for i in 0..d {
            for j in 0..d {
                let mut cov = 0.0;
                for row in rows {
                    cov += (row[i] as f64 - means[i]) * (row[j] as f64 - means[j]);
                }
                cov /= (n - 1) as f64;
                covariance[i][j] = cov;
                if i != j {
                    off_diag += cov * cov;
                }
            }
        }
    }
    VarianceCovarianceMetrics {
        means,
        variances: vars,
        covariance,
        var_min,
        var_mean,
        cov_off_diag_frobenius: off_diag.sqrt(),
        cov_off_diag_rms: (off_diag / d.saturating_mul(d.saturating_sub(1)).max(1) as f64).sqrt(),
        cov_loss_scale: off_diag / d.max(1) as f64,
    }
}

fn collapse_diagnostics(
    pred: &[Vec<f32>],
    target: &[Vec<f32>],
    online: &[Vec<f32>],
    variance_metrics: &VarianceCovarianceMetrics,
) -> CollapseDiagnostics {
    if online.is_empty() || online[0].is_empty() {
        return CollapseDiagnostics {
            computed: true,
            val_var_min: 0.0,
            val_var_p25: 0.0,
            val_var_median: 0.0,
            val_var_p75: 0.0,
            val_var_max: 0.0,
            covar_frob: 0.0,
            covar_rms: 0.0,
            covar_loss_scale: 0.0,
            eff_rank: 0.0,
            eff_rank_ratio: 0.0,
            alignment: 0.0,
            uniformity: 0.0,
            sigreg_kl: 0.0,
            sigreg_kl_clamped: false,
            latent_collapsed: true,
        };
    }
    let mut variances = variance_metrics.variances.clone();
    variances.sort_by(f64::total_cmp);
    let val_var_min = *variances.first().unwrap_or(&0.0);
    let val_var_p25 = sorted_quantile(&variances, 0.25);
    let val_var_median = sorted_quantile(&variances, 0.50);
    let val_var_p75 = sorted_quantile(&variances, 0.75);
    let val_var_max = *variances.last().unwrap_or(&0.0);
    let eigenvalues = jacobi_symmetric_eigenvalues(variance_metrics.covariance.clone());
    let eff_rank = effective_rank_from_eigenvalues(&eigenvalues);
    let eff_rank_ratio = eff_rank / online[0].len().max(1) as f64;
    let alignment = alignment_metric(pred, target);
    let uniformity = uniformity_metric(pred);
    let (sigreg_kl, sigreg_kl_clamped) = sigreg_diagonal_kl(
        &variance_metrics.means,
        &variance_metrics.variances,
        1.0e-12,
        1.0e12,
    );
    let latent_collapsed = val_var_min < 1.0e-8
        || eff_rank_ratio < 0.10
        || !alignment.is_finite()
        || !uniformity.is_finite()
        || !sigreg_kl.is_finite();
    CollapseDiagnostics {
        computed: true,
        val_var_min,
        val_var_p25,
        val_var_median,
        val_var_p75,
        val_var_max,
        covar_frob: variance_metrics.cov_off_diag_frobenius,
        covar_rms: variance_metrics.cov_off_diag_rms,
        covar_loss_scale: variance_metrics.cov_loss_scale,
        eff_rank,
        eff_rank_ratio,
        alignment,
        uniformity,
        sigreg_kl,
        sigreg_kl_clamped,
        latent_collapsed,
    }
}

fn sorted_quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let q = q.clamp(0.0, 1.0);
    let pos = q * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let weight = pos - lo as f64;
        sorted[lo] * (1.0 - weight) + sorted[hi] * weight
    }
}

fn surprise_calibration_report(
    examples: &[TrainExample],
    pred: &[Vec<f32>],
    target: &[Vec<f32>],
) -> SurpriseCalibrationReport {
    let mut cosines = examples
        .iter()
        .enumerate()
        .filter(|(_, example)| example.split_name == "val")
        .map(|(idx, _)| cosine(&pred[idx], &target[idx]))
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    cosines.sort_by(f64::total_cmp);
    if cosines.is_empty() {
        return SurpriseCalibrationReport {
            status: "uncalibrated".to_string(),
            split: "val".to_string(),
            threshold_cosine: 0.0,
            percentile: 10.0,
            calibration_set_count: 0,
            cosine_min: 0.0,
            cosine_p10: 0.0,
            cosine_median: 0.0,
            cosine_max: 0.0,
            false_positive_budget: 0.10,
        };
    }
    let p10 = sorted_quantile(&cosines, 0.10);
    SurpriseCalibrationReport {
        status: "calibrated".to_string(),
        split: "val".to_string(),
        threshold_cosine: p10,
        percentile: 10.0,
        calibration_set_count: cosines.len(),
        cosine_min: *cosines.first().unwrap_or(&f64::NAN),
        cosine_p10: p10,
        cosine_median: sorted_quantile(&cosines, 0.50),
        cosine_max: *cosines.last().unwrap_or(&f64::NAN),
        false_positive_budget: 0.10,
    }
}

fn surprise_segment_calibration_reports(
    examples: &[TrainExample],
    pred: &[Vec<f32>],
    target: &[Vec<f32>],
) -> std::collections::BTreeMap<String, SurpriseCalibrationReport> {
    let mut by_segment: std::collections::BTreeMap<String, Vec<f64>> =
        std::collections::BTreeMap::new();
    for (idx, example) in examples.iter().enumerate() {
        if example.split_name != "val" {
            continue;
        }
        let cosine = cosine(&pred[idx], &target[idx]);
        if !cosine.is_finite() {
            continue;
        }
        for (field, value) in &example.segments {
            by_segment
                .entry(format!("{field}={value}"))
                .or_default()
                .push(cosine);
        }
    }
    let mut reports = std::collections::BTreeMap::new();
    for (segment, mut cosines) in by_segment {
        cosines.sort_by(f64::total_cmp);
        let report = if cosines.len() < 2 {
            SurpriseCalibrationReport {
                status: "insufficient_segment_data".to_string(),
                split: "val".to_string(),
                threshold_cosine: 0.0,
                percentile: 10.0,
                calibration_set_count: cosines.len(),
                cosine_min: *cosines.first().unwrap_or(&0.0),
                cosine_p10: 0.0,
                cosine_median: *cosines.first().unwrap_or(&0.0),
                cosine_max: *cosines.last().unwrap_or(&0.0),
                false_positive_budget: 0.10,
            }
        } else {
            let p10 = sorted_quantile(&cosines, 0.10);
            SurpriseCalibrationReport {
                status: "calibrated".to_string(),
                split: "val".to_string(),
                threshold_cosine: p10,
                percentile: 10.0,
                calibration_set_count: cosines.len(),
                cosine_min: *cosines.first().unwrap_or(&f64::NAN),
                cosine_p10: p10,
                cosine_median: sorted_quantile(&cosines, 0.50),
                cosine_max: *cosines.last().unwrap_or(&f64::NAN),
                false_positive_budget: 0.10,
            }
        };
        reports.insert(segment, report);
    }
    reports
}

fn effective_rank_from_eigenvalues(eigenvalues: &[f64]) -> f64 {
    let values = eigenvalues
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > 1.0e-12)
        .collect::<Vec<_>>();
    let sum = values.iter().sum::<f64>();
    if sum <= f64::EPSILON {
        return 0.0;
    }
    let entropy = values
        .iter()
        .map(|value| {
            let p = *value / sum;
            -p * p.ln()
        })
        .sum::<f64>();
    entropy.exp()
}

fn jacobi_symmetric_eigenvalues(mut matrix: Vec<Vec<f64>>) -> Vec<f64> {
    let n = matrix.len();
    if n == 0 {
        return Vec::new();
    }
    if matrix.iter().any(|row| row.len() != n) {
        return Vec::new();
    }
    let max_iter = (n * n * 32).max(64);
    for _ in 0..max_iter {
        let mut p = 0usize;
        let mut q = 0usize;
        let mut max_off = 0.0f64;
        let mut i = 0usize;
        while i < n {
            let mut j = i + 1;
            while j < n {
                let value = matrix[i][j].abs();
                if value > max_off {
                    max_off = value;
                    p = i;
                    q = j;
                }
                j += 1;
            }
            i += 1;
        }
        if max_off < 1.0e-12 {
            break;
        }
        let app = matrix[p][p];
        let aqq = matrix[q][q];
        let apq = matrix[p][q];
        if apq.abs() < 1.0e-18 {
            continue;
        }
        let tau = (aqq - app) / (2.0 * apq);
        let t = if tau >= 0.0 {
            1.0 / (tau + (1.0 + tau * tau).sqrt())
        } else {
            -1.0 / (-tau + (1.0 + tau * tau).sqrt())
        };
        let c = 1.0 / (1.0 + t * t).sqrt();
        let s = t * c;
        let mut k = 0usize;
        while k < n {
            if k != p && k != q {
                let akp = matrix[k][p];
                let akq = matrix[k][q];
                let next_kp = c * akp - s * akq;
                let next_kq = s * akp + c * akq;
                matrix[k][p] = next_kp;
                matrix[p][k] = next_kp;
                matrix[k][q] = next_kq;
                matrix[q][k] = next_kq;
            }
            k += 1;
        }
        matrix[p][p] = c * c * app - 2.0 * s * c * apq + s * s * aqq;
        matrix[q][q] = s * s * app + 2.0 * s * c * apq + c * c * aqq;
        matrix[p][q] = 0.0;
        matrix[q][p] = 0.0;
    }
    matrix
        .iter()
        .enumerate()
        .map(|(idx, row)| row[idx].max(0.0))
        .collect()
}

fn alignment_metric(pred: &[Vec<f32>], target: &[Vec<f32>]) -> f64 {
    if pred.is_empty() || target.is_empty() {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut count = 0usize;
    for (left, right) in pred.iter().zip(target.iter()) {
        let left = normalize_vec(left);
        let right = normalize_vec(right);
        let dist_sq = left
            .iter()
            .zip(right.iter())
            .map(|(l, r)| {
                let diff = l - r;
                diff * diff
            })
            .sum::<f64>();
        if dist_sq.is_finite() {
            sum += dist_sq;
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

fn uniformity_metric(rows: &[Vec<f32>]) -> f64 {
    if rows.len() < 2 {
        return 0.0;
    }
    let normalized = rows
        .iter()
        .map(|row| normalize_vec(row))
        .collect::<Vec<_>>();
    let mut terms = Vec::with_capacity(rows.len() * (rows.len() - 1) / 2);
    for i in 0..normalized.len() {
        for j in (i + 1)..normalized.len() {
            let dist_sq = normalized[i]
                .iter()
                .zip(normalized[j].iter())
                .map(|(l, r)| {
                    let diff = l - r;
                    diff * diff
                })
                .sum::<f64>();
            terms.push(-2.0 * dist_sq);
        }
    }
    log_mean_exp(&terms)
}

fn normalize_vec(row: &[f32]) -> Vec<f64> {
    let norm = row
        .iter()
        .map(|value| {
            let value = *value as f64;
            value * value
        })
        .sum::<f64>()
        .sqrt();
    if norm <= f64::EPSILON {
        vec![0.0; row.len()]
    } else {
        row.iter().map(|value| *value as f64 / norm).collect()
    }
}

fn log_mean_exp(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let max = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .fold(f64::NEG_INFINITY, f64::max);
    if !max.is_finite() {
        return 0.0;
    }
    let sum = values
        .iter()
        .filter(|value| value.is_finite())
        .map(|value| (*value - max).exp())
        .sum::<f64>();
    if sum <= f64::EPSILON {
        0.0
    } else {
        max + (sum / values.len() as f64).ln()
    }
}

fn sigreg_diagonal_kl(
    means: &[f64],
    variances: &[f64],
    min_variance: f64,
    max_kl: f64,
) -> (f64, bool) {
    let mut clamped = false;
    let mut kl = 0.0;
    for (mean, variance) in means.iter().zip(variances.iter()) {
        let mut var = *variance;
        if !var.is_finite() || var < min_variance {
            var = min_variance;
            clamped = true;
        }
        let mean = if mean.is_finite() { *mean } else { 0.0 };
        let term = var + mean * mean - 1.0 - var.ln();
        if !term.is_finite() {
            clamped = true;
            continue;
        }
        kl += 0.5 * term;
        if kl > max_kl {
            return (max_kl, true);
        }
    }
    (kl, clamped)
}

fn mse_vec(lhs: &[f32], rhs: &[f32]) -> f64 {
    lhs.iter()
        .zip(rhs.iter())
        .map(|(l, r)| {
            let diff = *l as f64 - *r as f64;
            diff * diff
        })
        .sum::<f64>()
        / lhs.len().max(1) as f64
}

fn mean_vector<'a>(rows: impl Iterator<Item = &'a [f32]>) -> Vec<f64> {
    let mut count = 0usize;
    let mut mean = Vec::new();
    for row in rows {
        if mean.is_empty() {
            mean.resize(row.len(), 0.0);
        }
        count += 1;
        for (idx, value) in row.iter().enumerate() {
            mean[idx] += *value as f64;
        }
    }
    if count > 0 {
        for value in &mut mean {
            *value /= count as f64;
        }
    }
    mean
}

fn subtract_mean(row: &[f32], mean: &[f64]) -> Vec<f32> {
    row.iter()
        .zip(mean.iter())
        .map(|(value, mean)| (*value as f64 - *mean) as f32)
        .collect()
}

fn cosine(lhs: &[f32], rhs: &[f32]) -> f64 {
    let mut dot = 0.0;
    let mut ln = 0.0;
    let mut rn = 0.0;
    for (l, r) in lhs.iter().zip(rhs.iter()) {
        let l = *l as f64;
        let r = *r as f64;
        dot += l * r;
        ln += l * l;
        rn += r * r;
    }
    let denom = ln.sqrt() * rn.sqrt();
    if denom <= f64::EPSILON {
        0.0
    } else {
        dot / denom
    }
}

fn scalar_f64(tensor: &Tensor) -> DynamicJepaResult<f64> {
    tensor
        .to_scalar::<f32>()
        .map(|value| value as f64)
        .map_err(|err| training_err(format!("failed to read scalar tensor: {err}")))
}

fn training_err(message: String) -> DynamicJepaError {
    DynamicJepaError::TrainingFailed {
        training_run_id: uuid::Uuid::nil(),
        message,
        remediation: "inspect CUDA availability, dataset source rows, and the metrics artifact"
            .to_string(),
    }
}
