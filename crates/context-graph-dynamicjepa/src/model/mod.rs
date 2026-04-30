use crate::config::{MlpConfig, PredictorConfig, TrainConfig};
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
    pub split_metrics: std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>>,
    pub skipped_row_count: u32,
    pub skipped_reasons: std::collections::BTreeMap<String, u32>,
    pub random_init_baseline: std::collections::BTreeMap<String, f64>,
    pub shuffled_target_baseline: std::collections::BTreeMap<String, f64>,
    pub vicreg_variance_per_dim_min: f64,
    pub vicreg_variance_per_dim_mean: f64,
    pub vicreg_covariance_off_diag_frobenius: f64,
    pub epoch_metrics: Vec<EpochMetric>,
    pub parameter_count_trainable: usize,
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
    fn new(in_dim: usize, out_dim: usize, device: &Device) -> candle_core::Result<Self> {
        let scale = (2.0f32 / in_dim.max(1) as f32).sqrt();
        Ok(Self {
            weight: Var::randn(0f32, scale, (out_dim, in_dim), device)?,
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
    ) -> candle_core::Result<Self> {
        if dims.len() < 2 {
            candle_core::bail!("MLP requires at least input and output dimensions")
        }
        let layers = dims
            .windows(2)
            .map(|pair| LinearLayer::new(pair[0], pair[1], device))
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
    ) -> candle_core::Result<Self> {
        let mut dims = Vec::with_capacity(config.hidden.len() + 2);
        dims.push(input_dim);
        dims.extend(config.hidden.iter().copied());
        dims.push(config.out_dim);
        Self::from_dims(tensor_prefix, &dims, device)
    }

    fn from_predictor_config(
        tensor_prefix: &'static str,
        latent_dim: usize,
        action_dim: usize,
        config: &PredictorConfig,
        device: &Device,
    ) -> candle_core::Result<Self> {
        let mut dims = Vec::with_capacity(config.hidden.len() + 2);
        dims.push(latent_dim + action_dim);
        dims.extend(config.hidden.iter().copied());
        dims.push(config.out_dim);
        Self::from_dims(tensor_prefix, &dims, device)
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
    target_encoder: MlpNetwork,
    predictor: MlpNetwork,
}

impl TinyMlpJepa {
    fn new(
        config: &TrainConfig,
        input_dim: usize,
        action_dim: usize,
        device: &Device,
    ) -> candle_core::Result<Self> {
        let online_encoder = MlpNetwork::from_encoder_config(
            "online.encoder",
            input_dim,
            &config.model.encoder,
            device,
        )?;
        let target_encoder = MlpNetwork::from_encoder_config(
            "target.encoder",
            input_dim,
            &config.model.encoder,
            device,
        )?;
        let predictor = MlpNetwork::from_predictor_config(
            "predictor",
            config.model.encoder.out_dim,
            action_dim,
            &config.model.predictor,
            device,
        )?;
        target_encoder.set_from(&online_encoder)?;
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

    fn encode_target(&self, target: &Tensor) -> candle_core::Result<Tensor> {
        self.target_encoder.forward(target)
    }

    fn predict(&self, z: &Tensor, action: &Tensor) -> candle_core::Result<Tensor> {
        let joined = Tensor::cat(&[z, action], 1)?;
        self.predictor.forward(&joined)
    }

    fn ema_update(&self, momentum: f64) -> candle_core::Result<()> {
        self.target_encoder
            .ema_update(&self.online_encoder, momentum)
    }

    fn tensors(&self) -> HashMap<String, Tensor> {
        let mut tensors = self.online_encoder.tensors();
        tensors.extend(self.target_encoder.tensors());
        tensors.extend(self.predictor.tensors());
        tensors
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

    let model = TinyMlpJepa::new(config, input_dim, action_dim, &device)
        .map_err(|err| training_err(format!("failed to initialize tiny JEPA MLP: {err}")))?;
    let baseline_metrics = evaluate_model(
        &model,
        examples,
        &eval_input,
        &eval_target,
        &eval_action,
        &eval_negative,
    )?;

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
    let mut converged = false;
    for epoch in 0..config.schedule.epochs {
        let epoch_indices = shuffled_epoch_indices(&train_indices, config.seed, epoch as u64);
        let mut epoch_loss = 0.0;
        let mut epoch_mse = 0.0;
        let mut epoch_var = 0.0;
        let mut epoch_cov = 0.0;
        for step in 0..steps_per_epoch {
            global_step += 1;
            apply_lr_schedule(&mut opt, config, global_step);
            let batch = build_batch_tensors(
                examples,
                &epoch_indices,
                step * config.schedule.batch_size,
                config.schedule.batch_size,
                input_dim,
                target_dim,
                action_dim,
                &device,
            )?;
            let z_online = model
                .encode_online(&batch.input)
                .map_err(|err| training_err(format!("encoder forward failed: {err}")))?;
            let z_target = model
                .encode_target(&batch.target)
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
                .ema_update(config.model.ema_momentum)
                .map_err(|err| training_err(format!("EMA update failed: {err}")))?;
            device
                .synchronize()
                .map_err(|err| training_err(format!("CUDA synchronize failed: {err}")))?;
            epoch_loss += scalar_f64(&loss)?;
            epoch_mse += scalar_f64(&latent_mse)?;
            epoch_var += scalar_f64(&vic_var)?;
            epoch_cov += scalar_f64(&vic_cov)?;
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
        epoch_metrics.push(EpochMetric {
            epoch,
            loss: epoch_loss / denom,
            latent_mse: epoch_mse / denom,
            vicreg_variance: epoch_var / denom,
            vicreg_covariance: epoch_cov / denom,
        });
        let epoch_eval = evaluate_model(
            &model,
            examples,
            &eval_input,
            &eval_target,
            &eval_action,
            &eval_negative,
        )?;
        let stopping_value = metric_value(&epoch_eval, &config.stopping.metric)?;
        if stopping_value <= config.stopping.target {
            converged = true;
            break;
        }
    }

    let mut evaluation = evaluate_model(
        &model,
        examples,
        &eval_input,
        &eval_target,
        &eval_action,
        &eval_negative,
    )?;
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
    evaluation.random_init_baseline = std::collections::BTreeMap::from([(
        "cosine".to_string(),
        first_split_metric(&baseline_metrics, "latent_cosine").unwrap_or(0.0),
    )]);
    evaluation.epoch_metrics = epoch_metrics;
    evaluation.parameter_count_trainable = model.parameter_count_trainable();
    let metrics = flatten_metrics(&evaluation);
    Ok(TrainedTinyJepa {
        tensors: model.tensors(),
        metrics,
        evaluation_report: evaluation,
    })
}

struct BatchTensors {
    input: Tensor,
    target: Tensor,
    action: Tensor,
}

fn build_batch_tensors(
    examples: &[TrainExample],
    train_indices: &[usize],
    start: usize,
    batch_size: usize,
    input_dim: usize,
    target_dim: usize,
    action_dim: usize,
    device: &Device,
) -> DynamicJepaResult<BatchTensors> {
    if train_indices.is_empty() {
        return Err(training_err(
            "cannot build a batch with zero train rows".to_string(),
        ));
    }
    let mut inputs = Vec::with_capacity(batch_size * input_dim);
    let mut targets = Vec::with_capacity(batch_size * target_dim);
    let mut actions = Vec::with_capacity(batch_size * action_dim);
    for offset in 0..batch_size {
        let idx = train_indices[(start + offset) % train_indices.len()];
        inputs.extend_from_slice(&examples[idx].input_panel);
        targets.extend_from_slice(&examples[idx].target_panel);
        actions.extend_from_slice(&examples[idx].action);
    }
    Ok(BatchTensors {
        input: Tensor::from_vec(inputs, (batch_size, input_dim), device)
            .map_err(|err| training_err(format!("failed to create batch input tensor: {err}")))?,
        target: Tensor::from_vec(targets, (batch_size, target_dim), device)
            .map_err(|err| training_err(format!("failed to create batch target tensor: {err}")))?,
        action: Tensor::from_vec(actions, (batch_size, action_dim), device)
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

fn evaluate_model(
    model: &TinyMlpJepa,
    examples: &[TrainExample],
    input: &Tensor,
    target: &Tensor,
    action: &Tensor,
    negative: &Tensor,
) -> DynamicJepaResult<EvaluationReport> {
    let z_online = model
        .encode_online(input)
        .map_err(|err| training_err(format!("evaluation encoder forward failed: {err}")))?;
    let z_target = model
        .encode_target(target)
        .map_err(|err| training_err(format!("evaluation target forward failed: {err}")))?;
    let z_negative = model
        .encode_target(negative)
        .map_err(|err| training_err(format!("evaluation negative forward failed: {err}")))?;
    let z_hat = model
        .predict(&z_online, action)
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
    let mut shuffled_targets = Vec::with_capacity(examples.len());
    for idx in 0..examples.len() {
        let shuffled = if examples.len() > 1 {
            (idx + 1) % examples.len()
        } else {
            idx
        };
        shuffled_targets.push(shuffled);
    }
    for (split, idxs) in &by_split {
        let mut mse = 0.0;
        let mut cosine_sum = 0.0;
        let mut contrast_hits = 0u64;
        let mut shuffled_cosine_sum = 0.0;
        for idx in idxs {
            mse += mse_vec(&pred[*idx], &target[*idx]);
            cosine_sum += cosine(&pred[*idx], &target[*idx]);
            let pos = cosine(&pred[*idx], &target[*idx]);
            let neg = cosine(&pred[*idx], &negative[*idx]);
            if pos > neg {
                contrast_hits += 1;
            }
            let shuffled_idx = shuffled_targets[*idx];
            let shuffled_target = if shuffled_idx == *idx {
                &negative[*idx]
            } else {
                &target[shuffled_idx]
            };
            shuffled_cosine_sum += cosine(&pred[*idx], shuffled_target);
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
                    shuffled_cosine_sum / denom,
                ),
            ]),
        );
    }
    let (var_min, var_mean, cov_off_diag) = variance_covariance_metrics(&online);
    let shuffled_overall = split_metrics
        .values()
        .map(|metrics| metrics["shuffled_target_cosine"])
        .sum::<f64>()
        / split_metrics.len().max(1) as f64;
    Ok(EvaluationReport {
        objective_id: String::new(),
        split_metrics,
        skipped_row_count: 0,
        skipped_reasons: std::collections::BTreeMap::new(),
        random_init_baseline: std::collections::BTreeMap::new(),
        shuffled_target_baseline: std::collections::BTreeMap::from([(
            "cosine".to_string(),
            shuffled_overall,
        )]),
        vicreg_variance_per_dim_min: var_min,
        vicreg_variance_per_dim_mean: var_mean,
        vicreg_covariance_off_diag_frobenius: cov_off_diag,
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

fn first_split_metric(report: &EvaluationReport, metric: &str) -> Option<f64> {
    report
        .split_metrics
        .values()
        .next()
        .and_then(|metrics| metrics.get(metric))
        .copied()
}

fn flatten_metrics(report: &EvaluationReport) -> std::collections::BTreeMap<String, f64> {
    let mut out = std::collections::BTreeMap::new();
    for (split, metrics) in &report.split_metrics {
        for (metric, value) in metrics {
            out.insert(format!("{split}_{metric}"), *value);
        }
    }
    out.insert(
        "shuffled_target_cosine".to_string(),
        *report
            .shuffled_target_baseline
            .get("cosine")
            .unwrap_or(&0.0),
    );
    out.insert(
        "random_init_cosine".to_string(),
        *report.random_init_baseline.get("cosine").unwrap_or(&0.0),
    );
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
        "parameter_count_trainable".to_string(),
        report.parameter_count_trainable as f64,
    );
    out
}

fn variance_covariance_metrics(rows: &[Vec<f32>]) -> (f64, f64, f64) {
    if rows.is_empty() || rows[0].is_empty() {
        return (0.0, 0.0, 0.0);
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
    let mut off_diag = 0.0;
    if n > 1 {
        for i in 0..d {
            for j in 0..d {
                if i == j {
                    continue;
                }
                let mut cov = 0.0;
                for row in rows {
                    cov += (row[i] as f64 - means[i]) * (row[j] as f64 - means[j]);
                }
                cov /= (n - 1) as f64;
                off_diag += cov * cov;
            }
        }
    }
    (var_min, var_mean, off_diag.sqrt())
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
