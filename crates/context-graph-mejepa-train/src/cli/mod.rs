use crate::cert::{open_train_rocksdb, verify_chain, CF_MEJEPA_TRAIN_CERTS};
use crate::config::TrainingConfig;
use crate::error::{TrainerError, TrainerErrorCode};
use crate::eval::holdout::{HoldoutExample, TrainSplit};
use crate::eval::{Lang, MutationCategory};
use crate::trainer::{Trainer, TrainingDataset};
use candle_core::{Device, Tensor};
use clap::Args;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Args, Debug, Clone)]
pub struct TrainArgs {
    #[arg(long)]
    pub corpus: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub epochs: Option<u32>,
    #[arg(long, name = "batch-size")]
    pub batch_size: Option<usize>,
    #[arg(long)]
    pub lr: Option<f64>,
    #[arg(long, name = "weight-decay")]
    pub weight_decay: Option<f64>,
    #[arg(long, name = "warmup-steps")]
    pub warmup_steps: Option<u32>,
    #[arg(long, name = "max-grad-norm")]
    pub max_grad_norm: Option<f32>,
    #[arg(long, name = "no-mixed-precision")]
    pub no_mixed_precision: bool,
    #[arg(long, name = "full-finetune")]
    pub full_finetune: bool,
    #[arg(long, name = "lora-rank")]
    pub lora_rank: Option<usize>,
    #[arg(long, name = "lora-alpha")]
    pub lora_alpha: Option<usize>,
    #[arg(long, name = "lora-dropout")]
    pub lora_dropout: Option<f32>,
    #[arg(long, name = "checkpoint-interval-steps")]
    pub checkpoint_interval_steps: Option<u64>,
    #[arg(long, name = "holdout-eval-interval-steps")]
    pub holdout_eval_interval_steps: Option<u64>,
    #[arg(long, name = "counterfactual-interval-steps")]
    pub counterfactual_interval_steps: Option<u64>,
    #[arg(long, name = "counterfactual-warmup-steps")]
    pub counterfactual_warmup_steps: Option<u64>,
    #[arg(long, name = "distillation-interval-steps")]
    pub distillation_interval_steps: Option<u64>,
    #[arg(long, name = "cross-task-transfer-probability")]
    pub cross_task_transfer_probability: Option<f32>,
    #[arg(long, name = "cross-task-cosine-threshold")]
    pub cross_task_cosine_threshold: Option<f32>,
    #[arg(long, name = "adversarial-mix-ratio")]
    pub adversarial_mix_ratio: Option<f32>,
    #[arg(long, name = "random-seed")]
    pub random_seed: u64,
    #[arg(long)]
    pub resume: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct VerifyChainArgs {
    #[arg(long)]
    pub rocksdb: PathBuf,
    #[arg(long, name = "from-step")]
    pub from_step: Option<u64>,
    #[arg(long, name = "to-step")]
    pub to_step: Option<u64>,
}

#[derive(Args, Debug, Clone)]
pub struct EvalHoldoutArgs {
    #[arg(long)]
    pub weights: PathBuf,
    #[arg(long)]
    pub holdout: PathBuf,
    #[arg(long)]
    pub constellation: Option<PathBuf>,
    #[arg(long, default_value_t = 0.10)]
    pub alpha: f32,
}

pub fn run_train(args: TrainArgs) -> ExitCode {
    match run_train_inner(args) {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string(&value).expect("serialize result")
            );
            ExitCode::from(0)
        }
        Err(err) => exit_for_error(err),
    }
}

pub fn run_verify_chain(args: VerifyChainArgs) -> ExitCode {
    match open_train_rocksdb(&args.rocksdb).and_then(|db| {
        let from = args.from_step.unwrap_or(0);
        let to = args.to_step.unwrap_or(from);
        verify_chain(&db, CF_MEJEPA_TRAIN_CERTS, from, to)
    }) {
        Ok(report) if report.broken_at.is_none() => {
            println!(
                "{}",
                serde_json::to_string(&report).expect("serialize report")
            );
            ExitCode::from(0)
        }
        Ok(report) => {
            eprintln!(
                "{}",
                json!({"code":"MEJEPA_TRAIN_CERT_CHAIN_BROKEN","report": report})
            );
            ExitCode::from(1)
        }
        Err(err) => exit_for_error(err),
    }
}

pub fn run_eval_holdout(args: EvalHoldoutArgs) -> ExitCode {
    if !args.weights.exists() || !args.holdout.exists() {
        let err = TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "eval-holdout requires existing --weights and --holdout paths",
        );
        return exit_for_error(err);
    }
    println!(
        "{}",
        json!({"status":"validated_inputs","weights":args.weights,"holdout":args.holdout,"alpha":args.alpha})
    );
    ExitCode::from(0)
}

fn run_train_inner(args: TrainArgs) -> Result<serde_json::Value, TrainerError> {
    let mut config = match &args.config {
        Some(path) => TrainingConfig::load_from_toml(path)?,
        None => TrainingConfig::default(),
    };
    apply_overrides(&mut config, &args);
    config.random_seed = args.random_seed;
    config.validate()?;
    let dataset = load_corpus_index_dataset(&args.corpus)?;
    let db = open_train_rocksdb(&args.output)?;
    let mut trainer = Trainer::new(config, db, Device::Cpu)?;
    let result = trainer.run_full_training(&dataset)?;
    Ok(serde_json::to_value(result)?)
}

fn apply_overrides(config: &mut TrainingConfig, args: &TrainArgs) {
    if let Some(v) = args.epochs {
        config.epochs = v;
    }
    if let Some(v) = args.batch_size {
        config.batch_size = v;
    }
    if let Some(v) = args.lr {
        config.lr = v;
    }
    if let Some(v) = args.weight_decay {
        config.weight_decay = v;
    }
    if let Some(v) = args.warmup_steps {
        config.warmup_steps = v;
    }
    if let Some(v) = args.max_grad_norm {
        config.max_grad_norm = v;
    }
    if args.no_mixed_precision {
        config.mixed_precision = false;
    }
    if args.full_finetune {
        config.full_finetune = true;
    }
    if let Some(v) = args.lora_rank {
        config.lora_rank = v;
    }
    if let Some(v) = args.lora_alpha {
        config.lora_alpha = v;
    }
    if let Some(v) = args.lora_dropout {
        config.lora_dropout = v;
    }
    if let Some(v) = args.checkpoint_interval_steps {
        config.checkpoint_interval_steps = v;
    }
    if let Some(v) = args.holdout_eval_interval_steps {
        config.holdout_eval_interval_steps = v;
    }
    if let Some(v) = args.counterfactual_interval_steps {
        config.counterfactual_interval_steps = v;
    }
    if let Some(v) = args.counterfactual_warmup_steps {
        config.counterfactual_warmup_steps = v;
    }
    if let Some(v) = args.distillation_interval_steps {
        config.distillation_interval_steps = v;
    }
    if let Some(v) = args.cross_task_transfer_probability {
        config.cross_task_transfer_probability = v;
    }
    if let Some(v) = args.cross_task_cosine_threshold {
        config.cross_task_cosine_threshold = v;
    }
    if let Some(v) = args.adversarial_mix_ratio {
        config.adversarial_mix_ratio = v;
    }
}

pub fn load_corpus_index_dataset(corpus: &Path) -> Result<TrainingDataset, TrainerError> {
    let index = if corpus.is_dir() {
        corpus.join("index.json")
    } else {
        corpus.to_path_buf()
    };
    let bytes = std::fs::read(&index).map_err(|err| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!("failed to read corpus index {}: {err}", index.display()),
        )
        .with_context(json!({
            "index": index,
            "remediation": "pass the verified corpus index.json path or a directory containing index.json"
        }))
    })?;
    let root: serde_json::Value = serde_json::from_slice(&bytes)?;
    let entries = root
        .get("entries")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                "corpus-index.json missing entries array",
            )
        })?;
    let mut train = Vec::new();
    let mut calibration = Vec::new();
    let mut holdout = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        let task_id = entry
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| corpus_field(i, "task_id"))?
            .to_string();
        let cat = parse_category(
            entry
                .get("category")
                .and_then(|v| v.as_str())
                .ok_or_else(|| corpus_field(i, "category"))?,
        )?;
        let actual = entry
            .get("oracle_all_passed")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| corpus_field(i, "oracle_all_passed"))?;
        let adversarial = parse_optional_adversarial_flag(i, entry)?;
        let foundationality_score = parse_optional_foundationality_score(i, entry)?;
        let bucket = parse_bucket(
            i,
            entry
                .get("bucket")
                .and_then(|v| v.as_str())
                .ok_or_else(|| corpus_field(i, "bucket"))?,
        )?;
        let panel = corpus_feature_panel(&task_id, cat)?;
        let example = HoldoutExample {
            task_id,
            category: cat,
            language: Lang::Python,
            panel_t01: panel.clone(),
            panel_t2: panel,
            actual_oracle_pass: actual,
            adversarial,
            foundationality_score,
        };
        match bucket {
            CorpusBucket::Train => train.push(example),
            CorpusBucket::Calibration => calibration.push(example),
            CorpusBucket::Holdout => holdout.push(example),
        }
    }
    validate_dataset_splits(
        entries.len(),
        train.len(),
        calibration.len(),
        holdout.len(),
        &index,
    )?;
    Ok(TrainingDataset {
        train: TrainSplit { examples: train },
        calibration: crate::eval::holdout::CalibrationDataset {
            examples: calibration,
        },
        holdout: crate::eval::holdout::HoldoutDataset { examples: holdout },
    })
}

pub fn corpus_feature_panel(
    task_id: &str,
    category: MutationCategory,
) -> Result<Tensor, TrainerError> {
    if task_id.trim().is_empty() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "corpus task_id must not be empty",
        ));
    }
    let digest = Sha256::digest(task_id.as_bytes());
    let task_hash =
        u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) as f32 / u32::MAX as f32;
    let category_code = category_feature_code(category);
    Ok(Tensor::from_slice(
        &[task_hash, category_code],
        2,
        &Device::Cpu,
    )?)
}

fn parse_optional_foundationality_score(
    index: usize,
    entry: &serde_json::Value,
) -> Result<f32, TrainerError> {
    let Some(value) = entry.get("foundationality_score") else {
        return Ok(0.0);
    };
    let Some(score) = value.as_f64() else {
        return Err(corpus_field(index, "foundationality_score"));
    };
    if !score.is_finite() || !(0.0..=1.0).contains(&score) {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!("corpus entry {index} foundationality_score must be finite in [0, 1]"),
        )
        .with_context(json!({
            "index": index,
            "foundationality_score": score,
            "remediation": "materialize chunk foundationality from CF_MEJEPA_CHUNK_FOUNDATIONALITY before enabling lambda_found"
        })));
    }
    Ok(score as f32)
}

fn validate_dataset_splits(
    total_entries: usize,
    train: usize,
    calibration: usize,
    holdout: usize,
    index: &Path,
) -> Result<(), TrainerError> {
    let observed_total = train + calibration + holdout;
    if observed_total != total_entries {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!(
                "corpus split count mismatch for {}: observed {observed_total}, entries {total_entries}",
                index.display()
            ),
        )
        .with_context(json!({
            "index": index,
            "entries": total_entries,
            "train": train,
            "calibration": calibration,
            "holdout": holdout
        })));
    }
    let missing = [
        ("train", train),
        ("calibration", calibration),
        ("holdout", holdout),
    ]
    .into_iter()
    .filter_map(|(name, count)| if count == 0 { Some(name) } else { None })
    .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!(
                "corpus index {} has empty required split(s): {}",
                index.display(),
                missing.join(", ")
            ),
        )
        .with_context(json!({
            "index": index,
            "entries": total_entries,
            "train": train,
            "calibration": calibration,
            "holdout": holdout,
            "missing_splits": missing
        })));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CorpusBucket {
    Train,
    Calibration,
    Holdout,
}

fn parse_bucket(row: usize, value: &str) -> Result<CorpusBucket, TrainerError> {
    match value {
        "train" => Ok(CorpusBucket::Train),
        "calibration" => Ok(CorpusBucket::Calibration),
        "holdout" => Ok(CorpusBucket::Holdout),
        other => Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!("corpus entry {row} has unknown bucket {other}"),
        )
        .with_context(json!({
            "row": row,
            "bucket": other,
            "allowed": ["train", "calibration", "holdout"]
        }))),
    }
}

fn parse_optional_adversarial_flag(
    row: usize,
    entry: &serde_json::Value,
) -> Result<bool, TrainerError> {
    match entry.get("adversarial") {
        Some(value) => value.as_bool().ok_or_else(|| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("corpus entry {row} adversarial field must be boolean"),
            )
            .with_context(json!({
                "row": row,
                "field": "adversarial",
                "remediation": "set adversarial to true only for TASK-TEST-010 adversarial training rows"
            }))
        }),
        None => Ok(false),
    }
}

fn category_feature_code(category: MutationCategory) -> f32 {
    let idx = match category {
        MutationCategory::KnownGood => 0,
        MutationCategory::SubtleFlip => 1,
        MutationCategory::OffByOne => 2,
        MutationCategory::SwapVariable => 3,
        MutationCategory::DeleteTestCall => 4,
        MutationCategory::WrongFile => 5,
        MutationCategory::OverEngineer => 6,
        MutationCategory::CompileError => 7,
    };
    idx as f32 / (MutationCategory::ALL.len() - 1) as f32
}

fn parse_category(value: &str) -> Result<MutationCategory, TrainerError> {
    match value {
        "known_good" => Ok(MutationCategory::KnownGood),
        "subtle_flip" => Ok(MutationCategory::SubtleFlip),
        "off_by_one" => Ok(MutationCategory::OffByOne),
        "swap_variable" => Ok(MutationCategory::SwapVariable),
        "delete_test_call" => Ok(MutationCategory::DeleteTestCall),
        "wrong_file" => Ok(MutationCategory::WrongFile),
        "over_engineer" => Ok(MutationCategory::OverEngineer),
        "compile_error" => Ok(MutationCategory::CompileError),
        other => Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!("unknown mutation category in corpus index: {other}"),
        )),
    }
}

fn corpus_field(row: usize, field: &'static str) -> TrainerError {
    TrainerError::new(
        TrainerErrorCode::MejepaTrainConfigInvalid,
        format!("corpus entry {row} missing required field {field}"),
    )
}

fn exit_for_error(err: TrainerError) -> ExitCode {
    eprintln!("{}", serde_json::to_string(&err).expect("serialize error"));
    match err.code {
        TrainerErrorCode::MejepaTrainDodFailed => ExitCode::from(1),
        TrainerErrorCode::MejepaTrainConfigInvalid
        | TrainerErrorCode::MejepaTrainEmbedderDigestMismatch => ExitCode::from(3),
        _ => ExitCode::from(2),
    }
}
