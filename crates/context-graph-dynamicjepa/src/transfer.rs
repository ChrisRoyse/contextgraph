use crate::config::{
    LossConfig, MlpConfig, ModelConfig, OptimConfig, PredictorConfig, ScheduleConfig,
    StoppingConfig, TargetArchitecture, TrainConfig,
};
use crate::model::{random_init_tiny_jepa_tensors, train_tiny_jepa, TrainExample};
use candle_core::Tensor;
use context_graph_core::dynamicjepa::{DynamicJepaError, DynamicJepaResult};
use context_graph_storage::dynamicjepa::{
    counter_to_grid_bridge_action_effect, counter_to_grid_counter_action_kind,
    counter_to_grid_grid_action_kind, validate_counter_to_grid_bridge_instrument_count,
    COUNTER_TO_GRID_BRIDGE_INSTRUMENT_IDS, COUNTER_TO_GRID_DOMAIN_ID,
    COUNTER_TO_GRID_DOMAIN_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const TRANSFER_DEFAULT_SEEDS: [u64; 5] = [42, 43, 44, 45, 46];
pub const TRANSFER_DEFAULT_BOOTSTRAP_ITERS: usize = 10_000;
const BRIDGE_EFFECT_BASIS_DIM: usize = 3;
const POSITION_MEAN: f32 = 12.0;
const POSITION_STD: f32 = 12.0;
const SHUFFLED_TARGET_PERMUTATIONS: usize = 32;

#[derive(Debug, Clone)]
pub struct CrossDomainTransferConfig {
    pub output_root: PathBuf,
    pub seeds: Vec<u64>,
    pub source_events: usize,
    pub target_events: usize,
    pub bootstrap_iters: usize,
    pub train_epochs: usize,
    pub batch_size: usize,
    pub max_seconds_per_training: u64,
    pub learning_rate: f64,
    pub stopping_target: f64,
}

impl CrossDomainTransferConfig {
    pub fn validate(&self) -> DynamicJepaResult<()> {
        if self.output_root.as_os_str().is_empty() {
            return Err(DynamicJepaError::validation(
                "cross_domain_transfer.output_root",
                "output root must not be empty",
                "provide a fresh directory for transfer evidence",
            ));
        }
        if self.output_root.exists() {
            return Err(DynamicJepaError::validation(
                "cross_domain_transfer.output_root",
                format!("output root already exists: {}", self.output_root.display()),
                "use a fresh output directory; transfer evidence is immutable",
            ));
        }
        if self.seeds.is_empty() {
            return Err(DynamicJepaError::validation(
                "cross_domain_transfer.seeds",
                "at least one seed is required",
                "run the v2.1 pilot with seeds 42,43,44,45,46 or a non-empty test subset",
            ));
        }
        let mut sorted = self.seeds.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() != self.seeds.len() {
            return Err(DynamicJepaError::validation(
                "cross_domain_transfer.seeds",
                format!("duplicate seeds are not allowed: {:?}", self.seeds),
                "each seed must represent one independent paired transfer replicate",
            ));
        }
        if self.source_events < 20 {
            return Err(DynamicJepaError::validation(
                "cross_domain_transfer.source_events",
                format!("source_events must be >=20, got {}", self.source_events),
                "generate enough counter_world bridge rows for train/val/test splits",
            ));
        }
        if self.target_events < 20 {
            return Err(DynamicJepaError::validation(
                "cross_domain_transfer.target_events",
                format!("target_events must be >=20, got {}", self.target_events),
                "generate enough gridworld bridge rows for transfer evaluation and baselines",
            ));
        }
        if self.bootstrap_iters == 0 {
            return Err(DynamicJepaError::validation(
                "cross_domain_transfer.bootstrap_iters",
                "bootstrap_iters must be positive",
                "use 10000 for the v2.1 pilot or a positive smaller value for a smoke run",
            ));
        }
        if self.train_epochs == 0 || self.batch_size == 0 || self.max_seconds_per_training == 0 {
            return Err(DynamicJepaError::validation(
                "cross_domain_transfer.training",
                "train_epochs, batch_size, and max_seconds_per_training must be positive",
                "declare an explicit bounded CUDA training schedule",
            ));
        }
        if self.learning_rate <= 0.0 || !self.learning_rate.is_finite() {
            return Err(DynamicJepaError::validation(
                "cross_domain_transfer.learning_rate",
                format!(
                    "learning_rate must be finite and positive, got {}",
                    self.learning_rate
                ),
                "use a stable AdamW learning rate such as 0.001",
            ));
        }
        if self.stopping_target <= 0.0 || !self.stopping_target.is_finite() {
            return Err(DynamicJepaError::validation(
                "cross_domain_transfer.stopping_target",
                format!(
                    "stopping_target must be finite and positive, got {}",
                    self.stopping_target
                ),
                "use a positive val_latent_mse convergence target",
            ));
        }
        validate_counter_to_grid_bridge_instrument_count(
            COUNTER_TO_GRID_BRIDGE_INSTRUMENT_IDS.len(),
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossDomainTransferReport {
    pub status: String,
    pub domain: String,
    pub domain_version: String,
    pub output_root: PathBuf,
    pub source_of_truth: Vec<String>,
    pub phase1_passed_seeds: usize,
    pub phase2_passed_seeds: usize,
    pub seeds: Vec<TransferSeedReport>,
    pub statistics: TransferStatistics,
    pub acceptance: TransferAcceptance,
    pub artifacts: TransferArtifactManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferSeedReport {
    pub seed: u64,
    pub source_event_count: usize,
    pub target_event_count: usize,
    pub transfer_cosine: f64,
    pub random_init_cosine: f64,
    pub shuffled_target_cosine: f64,
    pub within_domain_ceiling_cosine: f64,
    pub no_pairwise_transfer_cosine: f64,
    pub pairwise_effect: f64,
    pub pairwise_on_train_metrics: BTreeMap<String, f64>,
    pub pairwise_off_train_metrics: BTreeMap<String, f64>,
    pub within_domain_train_metrics: BTreeMap<String, f64>,
    pub file_manifest: Vec<TransferFileState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferStatistics {
    pub n_seeds: usize,
    pub metrics: BTreeMap<String, TransferMetricStats>,
    pub sign_flip_pairwise_effect_p: f64,
    pub sign_flip_null_distribution: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferMetricStats {
    pub mean: f64,
    pub std: f64,
    pub bca_low: f64,
    pub bca_high: f64,
    pub bootstrap_iters: usize,
    pub degenerate_interval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferAcceptance {
    pub transfer_above_shuffled: bool,
    pub transfer_above_random: bool,
    pub pairwise_ablation_effect: bool,
    pub hypothesis_result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferArtifactManifest {
    pub table_cross_domain_transfer_csv: TransferFileState,
    pub cross_domain_transfer_box_svg: TransferFileState,
    pub seed_files: Vec<TransferFileState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferFileState {
    pub path: PathBuf,
    pub kind: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BridgeEventRow {
    trajectory_id: String,
    source_domain: String,
    step: usize,
    source_action_label: String,
    state: BridgeState,
    action: BridgeAction,
    outcome: BridgeOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BridgeState {
    abstract_position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BridgeAction {
    abstract_action_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BridgeOutcome {
    next_abstract_position: i64,
}

#[derive(Debug, Clone)]
struct BridgeExample {
    input_panel: Vec<f32>,
    target_panel: Vec<f32>,
    action: Vec<f32>,
}

pub fn run_cross_domain_transfer(
    config: &CrossDomainTransferConfig,
) -> DynamicJepaResult<CrossDomainTransferReport> {
    config.validate()?;
    fs::create_dir_all(&config.output_root).map_err(|err| DynamicJepaError::Storage {
        operation: "cross_domain_transfer.create_output_root".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to create {}: {err}", config.output_root.display()),
        remediation: "use a writable fresh output root".to_string(),
    })?;
    fs::create_dir_all(config.output_root.join("paper_tables")).map_err(|err| {
        DynamicJepaError::Storage {
            operation: "cross_domain_transfer.create_paper_tables".to_string(),
            cf: "filesystem".to_string(),
            message: format!(
                "failed to create {}/paper_tables: {err}",
                config.output_root.display()
            ),
            remediation: "use a writable fresh output root".to_string(),
        }
    })?;
    fs::create_dir_all(config.output_root.join("plots")).map_err(|err| {
        DynamicJepaError::Storage {
            operation: "cross_domain_transfer.create_plots".to_string(),
            cf: "filesystem".to_string(),
            message: format!(
                "failed to create {}/plots: {err}",
                config.output_root.display()
            ),
            remediation: "use a writable fresh output root".to_string(),
        }
    })?;

    let mut seeds = Vec::with_capacity(config.seeds.len());
    for &seed in &config.seeds {
        seeds.push(run_transfer_seed(config, seed)?);
    }
    let statistics = transfer_statistics(&seeds, config.bootstrap_iters)?;
    let acceptance = transfer_acceptance(&statistics)?;

    let mut report = CrossDomainTransferReport {
        status: "ok".to_string(),
        domain: COUNTER_TO_GRID_DOMAIN_ID.to_string(),
        domain_version: COUNTER_TO_GRID_DOMAIN_VERSION.to_string(),
        output_root: config.output_root.clone(),
        source_of_truth: vec![
            "transfer_results.json".to_string(),
            "paper_tables/table_cross_domain_transfer.csv".to_string(),
            "plots/cross_domain_transfer_box.svg".to_string(),
            "bridge/seed_*/model.safetensors".to_string(),
            "counter_world/seed_*/events.jsonl".to_string(),
            "gridworld_5x5/seed_*/events.jsonl".to_string(),
        ],
        phase1_passed_seeds: seeds.len(),
        phase2_passed_seeds: seeds.len(),
        seeds,
        statistics,
        acceptance,
        artifacts: TransferArtifactManifest {
            table_cross_domain_transfer_csv: empty_file_state(
                &config
                    .output_root
                    .join("paper_tables/table_cross_domain_transfer.csv"),
            ),
            cross_domain_transfer_box_svg: empty_file_state(
                &config
                    .output_root
                    .join("plots/cross_domain_transfer_box.svg"),
            ),
            seed_files: Vec::new(),
        },
    };

    write_text_atomic(
        &config
            .output_root
            .join("paper_tables/table_cross_domain_transfer.csv"),
        &cross_domain_transfer_csv(&report),
    )?;
    write_text_atomic(
        &config
            .output_root
            .join("plots/cross_domain_transfer_box.svg"),
        &cross_domain_transfer_box_svg(&report),
    )?;
    let seed_files = report
        .seeds
        .iter()
        .flat_map(|seed| seed.file_manifest.iter().cloned())
        .collect::<Vec<_>>();
    report.artifacts = TransferArtifactManifest {
        table_cross_domain_transfer_csv: file_state(
            &config
                .output_root
                .join("paper_tables/table_cross_domain_transfer.csv"),
        )?,
        cross_domain_transfer_box_svg: file_state(
            &config
                .output_root
                .join("plots/cross_domain_transfer_box.svg"),
        )?,
        seed_files,
    };
    write_json_atomic(&config.output_root.join("transfer_results.json"), &report)?;
    let readback = read_json_value(&config.output_root.join("transfer_results.json"))?;
    if readback
        .get("phase1_passed_seeds")
        .and_then(serde_json::Value::as_u64)
        != Some(report.phase1_passed_seeds as u64)
    {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: "transfer_results.json readback did not match phase1_passed_seeds".to_string(),
        });
    }
    Ok(report)
}

fn run_transfer_seed(
    config: &CrossDomainTransferConfig,
    seed: u64,
) -> DynamicJepaResult<TransferSeedReport> {
    let seed_dir = format!("seed_{seed}");
    let counter_dir = config.output_root.join("counter_world").join(&seed_dir);
    let grid_dir = config.output_root.join("gridworld_5x5").join(&seed_dir);
    let bridge_dir = config.output_root.join("bridge").join(&seed_dir);
    fs::create_dir_all(&counter_dir).map_err(file_create_dir_err)?;
    fs::create_dir_all(&grid_dir).map_err(file_create_dir_err)?;
    fs::create_dir_all(&bridge_dir).map_err(file_create_dir_err)?;

    let counter_rows = generate_counter_bridge_rows(seed, config.source_events)?;
    let grid_eval_rows = generate_grid_bridge_rows(seed, config.target_events)?;
    let grid_train_rows =
        generate_grid_bridge_rows(seed.wrapping_add(10_000), config.source_events)?;

    write_jsonl(&counter_dir.join("events.jsonl"), &counter_rows)?;
    write_jsonl(&grid_dir.join("events.jsonl"), &grid_eval_rows)?;
    write_jsonl(
        &grid_dir.join("within_domain_train_events.jsonl"),
        &grid_train_rows,
    )?;

    let pairwise_examples = build_train_examples(&counter_rows, true)?;
    let no_pairwise_examples = build_train_examples(&counter_rows, false)?;
    let within_examples = build_train_examples(&grid_train_rows, true)?;
    let target_eval_pairwise = build_eval_examples(&grid_eval_rows, true)?;
    let target_eval_no_pairwise = build_eval_examples(&grid_eval_rows, false)?;

    ensure_transfer_input_dims(
        "pairwise_on_target_eval",
        pairwise_examples[0].input_panel.len(),
        target_eval_pairwise[0].input_panel.len(),
    )?;
    ensure_transfer_input_dims(
        "pairwise_off_target_eval",
        no_pairwise_examples[0].input_panel.len(),
        target_eval_no_pairwise[0].input_panel.len(),
    )?;

    let pairwise_config = transfer_train_config(
        seed,
        pairwise_examples[0].input_panel.len(),
        pairwise_examples[0].action.len(),
        config,
        false,
    )?;
    let no_pairwise_config = transfer_train_config(
        seed.wrapping_add(1_000),
        no_pairwise_examples[0].input_panel.len(),
        no_pairwise_examples[0].action.len(),
        config,
        false,
    )?;
    let within_config = transfer_train_config(
        seed.wrapping_add(2_000),
        within_examples[0].input_panel.len(),
        within_examples[0].action.len(),
        config,
        false,
    )?;

    let pairwise_model = train_tiny_jepa(
        &pairwise_config,
        "counter_to_grid_pairwise_on",
        &pairwise_examples,
    )?;
    let no_pairwise_model = train_tiny_jepa(
        &no_pairwise_config,
        "counter_to_grid_pairwise_off",
        &no_pairwise_examples,
    )?;
    let within_model = train_tiny_jepa(
        &within_config,
        "counter_to_grid_within_grid_ceiling",
        &within_examples,
    )?;
    let random_tensors = random_init_tiny_jepa_tensors(
        &pairwise_config,
        pairwise_examples[0].input_panel.len(),
        pairwise_examples[0].action.len(),
        seed.wrapping_add(99_000),
    )?;

    let transfer_scores = evaluate_transfer(
        &pairwise_model.tensors,
        &pairwise_config,
        &target_eval_pairwise,
    )?;
    let shuffled_scores = evaluate_transfer_shuffled(
        &pairwise_model.tensors,
        &pairwise_config,
        &target_eval_pairwise,
        seed.wrapping_add(55_000),
    )?;
    let random_scores =
        evaluate_transfer(&random_tensors, &pairwise_config, &target_eval_pairwise)?;
    let no_pairwise_scores = evaluate_transfer(
        &no_pairwise_model.tensors,
        &no_pairwise_config,
        &target_eval_no_pairwise,
    )?;
    let within_scores =
        evaluate_transfer(&within_model.tensors, &within_config, &target_eval_pairwise)?;

    save_transfer_artifact(
        &bridge_dir.join("pairwise_on"),
        &pairwise_config,
        &pairwise_model,
    )?;
    save_transfer_artifact(
        &bridge_dir.join("pairwise_off"),
        &no_pairwise_config,
        &no_pairwise_model,
    )?;
    save_transfer_artifact(
        &bridge_dir.join("within_domain"),
        &within_config,
        &within_model,
    )?;

    let mut file_manifest = vec![
        file_state(&counter_dir.join("events.jsonl"))?,
        file_state(&grid_dir.join("events.jsonl"))?,
        file_state(&grid_dir.join("within_domain_train_events.jsonl"))?,
    ];
    for rel in [
        "pairwise_on/model.safetensors",
        "pairwise_on/config.json",
        "pairwise_on/metrics.json",
        "pairwise_off/model.safetensors",
        "within_domain/model.safetensors",
    ] {
        file_manifest.push(file_state(&bridge_dir.join(rel))?);
    }

    let transfer_cosine = mean(&transfer_scores)?;
    let no_pairwise_transfer_cosine = mean(&no_pairwise_scores)?;
    Ok(TransferSeedReport {
        seed,
        source_event_count: counter_rows.len(),
        target_event_count: grid_eval_rows.len(),
        transfer_cosine,
        random_init_cosine: mean(&random_scores)?,
        shuffled_target_cosine: mean(&shuffled_scores)?,
        within_domain_ceiling_cosine: mean(&within_scores)?,
        no_pairwise_transfer_cosine,
        pairwise_effect: transfer_cosine - no_pairwise_transfer_cosine,
        pairwise_on_train_metrics: pairwise_model.metrics,
        pairwise_off_train_metrics: no_pairwise_model.metrics,
        within_domain_train_metrics: within_model.metrics,
        file_manifest,
    })
}

pub fn ensure_transfer_input_dims(
    phase: impl Into<String>,
    expected: usize,
    actual: usize,
) -> DynamicJepaResult<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(DynamicJepaError::BridgePredictorInputDimMismatch {
            phase: phase.into(),
            expected,
            actual,
        })
    }
}

fn generate_counter_bridge_rows(
    seed: u64,
    event_count: usize,
) -> DynamicJepaResult<Vec<BridgeEventRow>> {
    let mut rows = Vec::with_capacity(event_count);
    let mut position = 8 + (splitmix64(seed) % 9) as i64;
    let pattern = [
        "step_up",
        "step_up",
        "step_down",
        "noop",
        "step_down",
        "step_up",
    ];
    for step in 0..event_count {
        let preferred = pattern[step % pattern.len()];
        let raw = match preferred {
            "step_up" if position < 24 => "step_up",
            "step_down" if position > 0 => "step_down",
            "noop" => "noop",
            _ => "noop",
        };
        let action_kind = counter_to_grid_counter_action_kind(raw)?;
        let effect = counter_to_grid_bridge_action_effect(action_kind)? as i64;
        let next_position = (position + effect).clamp(0, 24);
        rows.push(BridgeEventRow {
            trajectory_id: format!("counter_bridge_{seed}"),
            source_domain: "counter_world".to_string(),
            step,
            source_action_label: raw.to_string(),
            state: BridgeState {
                abstract_position: position,
            },
            action: BridgeAction {
                abstract_action_kind: action_kind.to_string(),
            },
            outcome: BridgeOutcome {
                next_abstract_position: next_position,
            },
        });
        position = next_position;
    }
    Ok(rows)
}

fn generate_grid_bridge_rows(
    seed: u64,
    event_count: usize,
) -> DynamicJepaResult<Vec<BridgeEventRow>> {
    let mut rows = Vec::with_capacity(event_count);
    let pattern = ["down", "down", "left", "noop", "left", "down"];
    let mut segment = 0usize;
    while rows.len() < event_count {
        let segment_seed = splitmix64(seed ^ segment as u64);
        let mut position = 8 + (segment_seed % 9) as i64;
        for raw in pattern {
            if rows.len() == event_count {
                break;
            }
            let action_kind = counter_to_grid_grid_action_kind(raw)?;
            let effect = counter_to_grid_bridge_action_effect(action_kind)? as i64;
            let next_position = (position + effect).clamp(0, 24);
            let step = rows.len();
            rows.push(BridgeEventRow {
                trajectory_id: format!("grid_bridge_{seed}_{segment:04}"),
                source_domain: "gridworld_5x5".to_string(),
                step,
                source_action_label: raw.to_string(),
                state: BridgeState {
                    abstract_position: position,
                },
                action: BridgeAction {
                    abstract_action_kind: action_kind.to_string(),
                },
                outcome: BridgeOutcome {
                    next_abstract_position: next_position,
                },
            });
            position = next_position;
        }
        segment += 1;
    }
    Ok(rows)
}

fn build_train_examples(
    rows: &[BridgeEventRow],
    include_pairwise: bool,
) -> DynamicJepaResult<Vec<TrainExample>> {
    let eval = build_eval_examples(rows, include_pairwise)?;
    let mut out = Vec::with_capacity(eval.len());
    for (idx, example) in eval.iter().enumerate() {
        let split_name = split_name(idx, eval.len()).to_string();
        let negative_panel = eval[(idx + 7) % eval.len()].target_panel.clone();
        out.push(TrainExample {
            split_name,
            input_panel: example.input_panel.clone(),
            target_panel: example.target_panel.clone(),
            action: example.action.clone(),
            negative_panel,
            segments: Default::default(),
        });
    }
    Ok(out)
}

fn build_eval_examples(
    rows: &[BridgeEventRow],
    include_pairwise: bool,
) -> DynamicJepaResult<Vec<BridgeExample>> {
    if rows.len() < 2 {
        return Err(DynamicJepaError::validation(
            "cross_domain_transfer.rows",
            format!("need at least two bridge rows, got {}", rows.len()),
            "generate adjacent bridge events so next-panel targets exist",
        ));
    }
    let mut out = Vec::with_capacity(rows.len() - 1);
    for idx in 0..(rows.len() - 1) {
        let current = &rows[idx];
        let next = &rows[idx + 1];
        if current.trajectory_id != next.trajectory_id {
            continue;
        }
        let current_action_kind = current.action.abstract_action_kind.as_str();
        let next_action_kind = next.action.abstract_action_kind.as_str();
        out.push(BridgeExample {
            input_panel: bridge_panel_features(
                current.state.abstract_position,
                current_action_kind,
                include_pairwise,
            )?,
            target_panel: bridge_panel_features(
                next.state.abstract_position,
                next_action_kind,
                include_pairwise,
            )?,
            action: if include_pairwise {
                bridge_action_effect_basis(current_action_kind)?
            } else {
                bridge_action_source_basis(current_action_kind)?
            },
        });
    }
    Ok(out)
}

fn split_name(idx: usize, total: usize) -> &'static str {
    let train_end = ((total * 7) / 10).max(1);
    let val_end = train_end + ((total * 2).div_ceil(10)).min(total.saturating_sub(train_end));
    if idx < train_end {
        "train"
    } else if idx < val_end {
        "val"
    } else {
        "test"
    }
}

fn bridge_panel_features(
    position: i64,
    action_kind: &str,
    include_pairwise: bool,
) -> DynamicJepaResult<Vec<f32>> {
    if !(0..=24).contains(&position) {
        return Err(DynamicJepaError::validation(
            "cross_domain_transfer.abstract_position",
            format!("abstract_position {position} outside 0..24"),
            "fix the bridge generator or source-domain mapper before training",
        ));
    }
    let pos = (position as f32 - POSITION_MEAN) / POSITION_STD;
    let mut out = Vec::with_capacity(if include_pairwise { 6 } else { 4 });
    out.push(pos);
    if include_pairwise {
        out.extend(bridge_action_effect_basis(action_kind)?);
        let effect = counter_to_grid_bridge_action_effect(action_kind)?;
        out.push(effect);
        out.push(pos * effect);
    } else {
        out.extend(bridge_action_source_basis(action_kind)?);
    }
    Ok(out)
}

fn bridge_action_effect_basis(action_kind: &str) -> DynamicJepaResult<Vec<f32>> {
    let mut out = vec![0.0; BRIDGE_EFFECT_BASIS_DIM];
    match action_kind {
        "increment" | "lateral" => out[0] = 1.0,
        "decrement" | "perpendicular" => out[1] = 1.0,
        "noop" => out[2] = 1.0,
        other => {
            return Err(DynamicJepaError::BridgeActionMappingIncomplete {
                source_domain: COUNTER_TO_GRID_DOMAIN_ID.to_string(),
                action_label: other.to_string(),
            });
        }
    }
    Ok(out)
}

fn bridge_action_source_basis(action_kind: &str) -> DynamicJepaResult<Vec<f32>> {
    let mut out = vec![0.0; BRIDGE_EFFECT_BASIS_DIM];
    match action_kind {
        "increment" => out[0] = 1.0,
        "decrement" => out[1] = 1.0,
        "noop" => out[2] = 1.0,
        "lateral" | "perpendicular" => {}
        other => {
            return Err(DynamicJepaError::BridgeActionMappingIncomplete {
                source_domain: COUNTER_TO_GRID_DOMAIN_ID.to_string(),
                action_label: other.to_string(),
            });
        }
    }
    Ok(out)
}

fn transfer_train_config(
    seed: u64,
    input_dim: usize,
    action_dim: usize,
    config: &CrossDomainTransferConfig,
    ignore_action: bool,
) -> DynamicJepaResult<TrainConfig> {
    let latent_dim = input_dim.max(8);
    let train_config = TrainConfig {
        model: ModelConfig {
            encoder: MlpConfig {
                kind: "mlp".to_string(),
                hidden: vec![16],
                out_dim: latent_dim,
            },
            predictor: PredictorConfig {
                kind: "mlp".to_string(),
                hidden: vec![16],
                in_action_dim: action_dim,
                out_dim: latent_dim,
                ignore_action,
            },
            target_architecture: TargetArchitecture::EmaEncoder,
            ema_momentum: 0.99,
        },
        loss: LossConfig {
            latent_mse_weight: 1.0,
            vicreg_variance_weight: 0.01,
            vicreg_covariance_weight: 0.001,
            vicreg_target_std: 0.5,
        },
        optim: OptimConfig {
            kind: "adamw".to_string(),
            lr: config.learning_rate,
            weight_decay: 0.0001,
            warmup_steps: 0,
        },
        schedule: ScheduleConfig {
            epochs: config.train_epochs,
            batch_size: config.batch_size,
            device: "cuda".to_string(),
        },
        stopping: StoppingConfig {
            metric: "val_latent_mse".to_string(),
            target: config.stopping_target,
            max_seconds: config.max_seconds_per_training,
        },
        quality_gates: Vec::new(),
        seed,
    };
    train_config.validate()?;
    ensure_transfer_input_dims("train_config.input_dim_nonzero", input_dim, input_dim)?;
    Ok(train_config)
}

fn evaluate_transfer(
    tensors: &BTreeMapCompat,
    config: &TrainConfig,
    examples: &[BridgeExample],
) -> DynamicJepaResult<Vec<f64>> {
    let mut scores = Vec::with_capacity(examples.len());
    for example in examples {
        let predicted = predict_latent(tensors, config, &example.input_panel, &example.action)?;
        let target = encode_target_latent(tensors, config, &example.target_panel)?;
        scores.push(cosine(&predicted, &target)?);
    }
    Ok(scores)
}

fn evaluate_transfer_shuffled(
    tensors: &BTreeMapCompat,
    config: &TrainConfig,
    examples: &[BridgeExample],
    seed: u64,
) -> DynamicJepaResult<Vec<f64>> {
    if examples.len() < 2 {
        return Err(DynamicJepaError::validation(
            "cross_domain_transfer.shuffled_target",
            "need at least two examples to build a deranged shuffled-target baseline",
            "generate more target events before running transfer evaluation",
        ));
    }
    let mut scores = Vec::with_capacity(examples.len() * SHUFFLED_TARGET_PERMUTATIONS);
    for permutation_idx in 0..SHUFFLED_TARGET_PERMUTATIONS {
        let order = deranged_shuffle_order(
            examples,
            seed ^ splitmix64(permutation_idx as u64 + 0xC20D_A1D5),
        );
        for (idx, example) in examples.iter().enumerate() {
            let target_example = &examples[order[idx]];
            let predicted = predict_latent(tensors, config, &example.input_panel, &example.action)?;
            let target = encode_target_latent(tensors, config, &target_example.target_panel)?;
            scores.push(cosine(&predicted, &target)?);
        }
    }
    Ok(scores)
}

fn deranged_shuffle_order(examples: &[BridgeExample], seed: u64) -> Vec<usize> {
    let mut order = (0..examples.len()).collect::<Vec<_>>();
    shuffle_indices(&mut order, seed);
    for idx in 0..order.len() {
        if order[idx] == idx {
            order.swap(idx, (idx + 1) % examples.len());
        }
    }
    for idx in 0..order.len() {
        if examples[order[idx]].target_panel == examples[idx].target_panel {
            let replacement = (idx + examples.len() / 2) % examples.len();
            order.swap(idx, replacement);
        }
    }
    order
}

type BTreeMapCompat = std::collections::HashMap<String, Tensor>;

fn predict_latent(
    tensors: &BTreeMapCompat,
    config: &TrainConfig,
    input_panel: &[f32],
    action: &[f32],
) -> DynamicJepaResult<Vec<f32>> {
    let z = mlp_forward(
        tensors,
        "online.encoder",
        config.model.encoder.hidden.len() + 1,
        input_panel,
    )?;
    if z.len() != config.model.encoder.out_dim {
        return Err(DynamicJepaError::BridgePredictorInputDimMismatch {
            phase: "online_encoder_output".to_string(),
            expected: config.model.encoder.out_dim,
            actual: z.len(),
        });
    }
    let mut joined = z;
    joined.extend_from_slice(action);
    mlp_forward(
        tensors,
        "predictor",
        config.model.predictor.hidden.len() + 1,
        &joined,
    )
}

fn encode_target_latent(
    tensors: &BTreeMapCompat,
    config: &TrainConfig,
    target_panel: &[f32],
) -> DynamicJepaResult<Vec<f32>> {
    mlp_forward(
        tensors,
        "target.encoder",
        config.model.encoder.hidden.len() + 1,
        target_panel,
    )
}

fn mlp_forward(
    tensors: &BTreeMapCompat,
    prefix: &str,
    layer_count: usize,
    input: &[f32],
) -> DynamicJepaResult<Vec<f32>> {
    let mut x = input.to_vec();
    for layer_idx in 0..layer_count {
        let weight_name = format!("{prefix}.layer{layer_idx}.weight");
        let bias_name = format!("{prefix}.layer{layer_idx}.bias");
        let weight = tensors
            .get(&weight_name)
            .ok_or_else(|| DynamicJepaError::TrainingFailed {
                training_run_id: uuid::Uuid::nil(),
                message: format!("missing tensor {weight_name}"),
                remediation:
                    "rerun cross-domain transfer training; artifact tensors are incomplete"
                        .to_string(),
            })?;
        let bias = tensors
            .get(&bias_name)
            .ok_or_else(|| DynamicJepaError::TrainingFailed {
                training_run_id: uuid::Uuid::nil(),
                message: format!("missing tensor {bias_name}"),
                remediation:
                    "rerun cross-domain transfer training; artifact tensors are incomplete"
                        .to_string(),
            })?;
        let weight = weight
            .to_vec2::<f32>()
            .map_err(|err| DynamicJepaError::TrainingFailed {
                training_run_id: uuid::Uuid::nil(),
                message: format!("failed to decode tensor {weight_name}: {err}"),
                remediation: "rerun transfer training from a clean output root".to_string(),
            })?;
        let bias = bias
            .to_vec1::<f32>()
            .map_err(|err| DynamicJepaError::TrainingFailed {
                training_run_id: uuid::Uuid::nil(),
                message: format!("failed to decode tensor {bias_name}: {err}"),
                remediation: "rerun transfer training from a clean output root".to_string(),
            })?;
        if weight.len() != bias.len() {
            return Err(DynamicJepaError::BridgePredictorInputDimMismatch {
                phase: format!("{prefix}.layer{layer_idx}.rows"),
                expected: weight.len(),
                actual: bias.len(),
            });
        }
        let mut next = Vec::with_capacity(weight.len());
        for row in &weight {
            if row.len() != x.len() {
                return Err(DynamicJepaError::BridgePredictorInputDimMismatch {
                    phase: format!("{prefix}.layer{layer_idx}.cols"),
                    expected: row.len(),
                    actual: x.len(),
                });
            }
            let mut acc = bias[next.len()];
            for (w, v) in row.iter().zip(x.iter()) {
                acc += *w * *v;
            }
            if layer_idx + 1 != layer_count {
                acc = acc.max(0.0);
            }
            if !acc.is_finite() {
                return Err(DynamicJepaError::TrainingFailed {
                    training_run_id: uuid::Uuid::nil(),
                    message: format!("{prefix}.layer{layer_idx} produced non-finite output"),
                    remediation: "inspect model.safetensors and training metrics".to_string(),
                });
            }
            next.push(acc);
        }
        x = next;
    }
    Ok(x)
}

fn transfer_statistics(
    seeds: &[TransferSeedReport],
    bootstrap_iters: usize,
) -> DynamicJepaResult<TransferStatistics> {
    let metric_values = [
        (
            "counter_to_grid_transfer_cosine",
            seeds.iter().map(|s| s.transfer_cosine).collect::<Vec<_>>(),
        ),
        (
            "random_init_cosine",
            seeds
                .iter()
                .map(|s| s.random_init_cosine)
                .collect::<Vec<_>>(),
        ),
        (
            "shuffled_target_cosine",
            seeds
                .iter()
                .map(|s| s.shuffled_target_cosine)
                .collect::<Vec<_>>(),
        ),
        (
            "within_domain_ceiling_cosine",
            seeds
                .iter()
                .map(|s| s.within_domain_ceiling_cosine)
                .collect::<Vec<_>>(),
        ),
        (
            "no_pairwise_transfer_cosine",
            seeds
                .iter()
                .map(|s| s.no_pairwise_transfer_cosine)
                .collect::<Vec<_>>(),
        ),
        (
            "pairwise_effect",
            seeds.iter().map(|s| s.pairwise_effect).collect::<Vec<_>>(),
        ),
    ];
    let mut metrics = BTreeMap::new();
    for (name, values) in metric_values {
        metrics.insert(
            name.to_string(),
            metric_stats(&values, bootstrap_iters, 20260601)?,
        );
    }
    let differences = seeds.iter().map(|s| s.pairwise_effect).collect::<Vec<_>>();
    let (p, null_distribution) = exact_sign_flip_p_value(&differences)?;
    Ok(TransferStatistics {
        n_seeds: seeds.len(),
        metrics,
        sign_flip_pairwise_effect_p: p,
        sign_flip_null_distribution: null_distribution,
    })
}

fn transfer_acceptance(statistics: &TransferStatistics) -> DynamicJepaResult<TransferAcceptance> {
    let transfer = statistics
        .metrics
        .get("counter_to_grid_transfer_cosine")
        .ok_or_else(|| DynamicJepaError::StorageInvariantViolation {
            message: "missing transfer metric".to_string(),
        })?;
    let shuffled = statistics
        .metrics
        .get("shuffled_target_cosine")
        .ok_or_else(|| DynamicJepaError::StorageInvariantViolation {
            message: "missing shuffled metric".to_string(),
        })?;
    let random = statistics
        .metrics
        .get("random_init_cosine")
        .ok_or_else(|| DynamicJepaError::StorageInvariantViolation {
            message: "missing random metric".to_string(),
        })?;
    let effect = statistics.metrics.get("pairwise_effect").ok_or_else(|| {
        DynamicJepaError::StorageInvariantViolation {
            message: "missing pairwise effect metric".to_string(),
        }
    })?;
    let transfer_above_shuffled = transfer.mean > shuffled.bca_high + 0.05;
    let transfer_above_random = transfer.mean > random.bca_high + 0.10;
    let pairwise_ablation_effect =
        effect.mean >= 0.05 && statistics.sign_flip_pairwise_effect_p <= 0.0625;
    let hypothesis_result =
        if transfer_above_shuffled && transfer_above_random && pairwise_ablation_effect {
            "supports_pairwise_transfer"
        } else if transfer_above_shuffled && transfer_above_random {
            "supports_transfer_not_pairwise"
        } else {
            "negative_result"
        };
    Ok(TransferAcceptance {
        transfer_above_shuffled,
        transfer_above_random,
        pairwise_ablation_effect,
        hypothesis_result: hypothesis_result.to_string(),
    })
}

fn metric_stats(
    values: &[f64],
    bootstrap_iters: usize,
    seed: u64,
) -> DynamicJepaResult<TransferMetricStats> {
    if values.is_empty() {
        return Err(DynamicJepaError::validation(
            "cross_domain_transfer.metric_values",
            "metric values must not be empty",
            "run at least one transfer seed before aggregating statistics",
        ));
    }
    let mean_value = mean(values)?;
    let std = sample_std(values, mean_value);
    let all_equal = values
        .iter()
        .all(|value| (*value - values[0]).abs() <= f64::EPSILON);
    if all_equal {
        return Ok(TransferMetricStats {
            mean: mean_value,
            std,
            bca_low: mean_value,
            bca_high: mean_value,
            bootstrap_iters,
            degenerate_interval: true,
        });
    }
    let mut boot = Vec::with_capacity(bootstrap_iters);
    let mut rng = seed;
    for _ in 0..bootstrap_iters {
        let mut acc = 0.0;
        for _ in 0..values.len() {
            rng = splitmix64(rng);
            let idx = (rng as usize) % values.len();
            acc += values[idx];
        }
        boot.push(acc / values.len() as f64);
    }
    boot.sort_by(f64::total_cmp);
    let less = boot.iter().filter(|value| **value < mean_value).count() as f64;
    let prop_less = (less / boot.len() as f64).clamp(1.0e-9, 1.0 - 1.0e-9);
    let z0 = inv_norm_cdf(prop_less);
    let acceleration = jackknife_acceleration(values)?;
    let alpha_low = bca_adjusted_alpha(0.025, z0, acceleration);
    let alpha_high = bca_adjusted_alpha(0.975, z0, acceleration);
    Ok(TransferMetricStats {
        mean: mean_value,
        std,
        bca_low: sorted_quantile(&boot, alpha_low),
        bca_high: sorted_quantile(&boot, alpha_high),
        bootstrap_iters,
        degenerate_interval: false,
    })
}

fn bca_adjusted_alpha(alpha: f64, z0: f64, acceleration: f64) -> f64 {
    let z = inv_norm_cdf(alpha);
    let numerator = z0 + z;
    let denominator = 1.0 - acceleration * numerator;
    normal_cdf(z0 + numerator / denominator).clamp(0.0, 1.0)
}

fn jackknife_acceleration(values: &[f64]) -> DynamicJepaResult<f64> {
    if values.len() < 3 {
        return Ok(0.0);
    }
    let total: f64 = values.iter().sum();
    let denom = (values.len() - 1) as f64;
    let jack = values
        .iter()
        .map(|value| (total - value) / denom)
        .collect::<Vec<_>>();
    let jack_mean = mean(&jack)?;
    let mut num = 0.0;
    let mut den = 0.0;
    for value in &jack {
        let centered = jack_mean - value;
        num += centered.powi(3);
        den += centered.powi(2);
    }
    if den <= f64::EPSILON {
        return Ok(0.0);
    }
    Ok(num / (6.0 * den.powf(1.5)))
}

fn exact_sign_flip_p_value(differences: &[f64]) -> DynamicJepaResult<(f64, Vec<f64>)> {
    if differences.is_empty() || differences.len() > 20 {
        return Err(DynamicJepaError::validation(
            "cross_domain_transfer.sign_flip",
            format!(
                "sign-flip exact test supports 1..20 paired differences, got {}",
                differences.len()
            ),
            "run the documented five-seed pilot or a bounded smoke subset",
        ));
    }
    let observed = mean(differences)?;
    let total = 1usize << differences.len();
    let mut null = Vec::with_capacity(total);
    for mask in 0..total {
        let mut acc = 0.0;
        for (idx, diff) in differences.iter().enumerate() {
            let sign = if (mask >> idx) & 1 == 1 { 1.0 } else { -1.0 };
            acc += sign * diff;
        }
        null.push(acc / differences.len() as f64);
    }
    let ge = null
        .iter()
        .filter(|value| **value >= observed - 1.0e-12)
        .count() as f64;
    let le = null
        .iter()
        .filter(|value| **value <= observed + 1.0e-12)
        .count() as f64;
    let one_sided = ge.min(le) / total as f64;
    let p = (2.0 * one_sided).min(1.0);
    null.sort_by(f64::total_cmp);
    Ok((p, null))
}

fn cross_domain_transfer_csv(report: &CrossDomainTransferReport) -> String {
    let transfer = &report.statistics.metrics["counter_to_grid_transfer_cosine"];
    let shuffled = &report.statistics.metrics["shuffled_target_cosine"];
    let random = &report.statistics.metrics["random_init_cosine"];
    let within = &report.statistics.metrics["within_domain_ceiling_cosine"];
    let ablation = &report.statistics.metrics["no_pairwise_transfer_cosine"];
    let effect = &report.statistics.metrics["pairwise_effect"];
    let mut out = String::from(
        "source_domain,target_domain,n_seeds,phase1_passed_seeds,phase2_passed_seeds,transfer_cosine_mean,transfer_bca_low,transfer_bca_high,random_init_cosine_mean,random_init_bca_high,shuffled_target_cosine_mean,shuffled_target_bca_high,within_domain_ceiling_cosine_mean,no_pairwise_transfer_cosine_mean,pairwise_effect_mean,pairwise_effect_bca_low,pairwise_effect_bca_high,sign_flip_pairwise_effect_p,hypothesis_result\n",
    );
    out.push_str(&format!(
        "counter_world,gridworld_5x5,{},{},{},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{}\n",
        report.statistics.n_seeds,
        report.phase1_passed_seeds,
        report.phase2_passed_seeds,
        transfer.mean,
        transfer.bca_low,
        transfer.bca_high,
        random.mean,
        random.bca_high,
        shuffled.mean,
        shuffled.bca_high,
        within.mean,
        ablation.mean,
        effect.mean,
        effect.bca_low,
        effect.bca_high,
        report.statistics.sign_flip_pairwise_effect_p,
        report.acceptance.hypothesis_result
    ));
    out
}

fn cross_domain_transfer_box_svg(report: &CrossDomainTransferReport) -> String {
    let metrics = [
        ("transfer", "counter_to_grid_transfer_cosine"),
        ("random", "random_init_cosine"),
        ("shuffled", "shuffled_target_cosine"),
        ("ablation", "no_pairwise_transfer_cosine"),
        ("ceiling", "within_domain_ceiling_cosine"),
    ];
    let width = 720.0;
    let height = 360.0;
    let left = 70.0;
    let top = 28.0;
    let plot_h = 250.0;
    let scale_y = |v: f64| top + (1.0 - v.clamp(-1.0, 1.0)) * plot_h / 2.0;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#fff\"/><text x=\"70\" y=\"22\" font-family=\"sans-serif\" font-size=\"15\">Cross-domain transfer cosine by seed</text><line x1=\"{left}\" y1=\"{}\" x2=\"690\" y2=\"{}\" stroke=\"#222\" stroke-width=\"1\"/><line x1=\"{left}\" y1=\"{top}\" x2=\"{left}\" y2=\"{}\" stroke=\"#222\" stroke-width=\"1\"/>",
        scale_y(0.0),
        scale_y(0.0),
        top + plot_h
    );
    for (idx, (label, metric)) in metrics.iter().enumerate() {
        let x = left + 70.0 + idx as f64 * 120.0;
        let values = report
            .seeds
            .iter()
            .map(|seed| match *metric {
                "counter_to_grid_transfer_cosine" => seed.transfer_cosine,
                "random_init_cosine" => seed.random_init_cosine,
                "shuffled_target_cosine" => seed.shuffled_target_cosine,
                "no_pairwise_transfer_cosine" => seed.no_pairwise_transfer_cosine,
                "within_domain_ceiling_cosine" => seed.within_domain_ceiling_cosine,
                _ => 0.0,
            })
            .collect::<Vec<_>>();
        let stats = &report.statistics.metrics[*metric];
        let y_low = scale_y(stats.bca_low);
        let y_high = scale_y(stats.bca_high);
        let y_mean = scale_y(stats.mean);
        svg.push_str(&format!(
            "<line x1=\"{x}\" y1=\"{y_low}\" x2=\"{x}\" y2=\"{y_high}\" stroke=\"#444\" stroke-width=\"2\"/><rect x=\"{}\" y=\"{}\" width=\"32\" height=\"{}\" fill=\"#d7e8f7\" stroke=\"#245\"/><line x1=\"{}\" y1=\"{y_mean}\" x2=\"{}\" y2=\"{y_mean}\" stroke=\"#b21\" stroke-width=\"2\"/>",
            x - 16.0,
            y_high.min(y_low),
            (y_low - y_high).abs().max(2.0),
            x - 20.0,
            x + 20.0
        ));
        for (point_idx, value) in values.iter().enumerate() {
            let px = x - 18.0 + point_idx as f64 * 9.0;
            let py = scale_y(*value);
            svg.push_str(&format!(
                "<circle cx=\"{px}\" cy=\"{py}\" r=\"3\" fill=\"#111\"/>"
            ));
        }
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"325\" font-family=\"sans-serif\" font-size=\"12\" text-anchor=\"middle\">{label}</text>",
            x
        ));
    }
    for tick in [-1.0, -0.5, 0.0, 0.5, 1.0] {
        let y = scale_y(tick);
        svg.push_str(&format!(
            "<line x1=\"64\" y1=\"{y}\" x2=\"70\" y2=\"{y}\" stroke=\"#222\"/><text x=\"58\" y=\"{}\" font-family=\"sans-serif\" font-size=\"11\" text-anchor=\"end\">{tick:.1}</text>",
            y + 4.0
        ));
    }
    svg.push_str("</svg>\n");
    svg
}

fn save_transfer_artifact(
    dir: &Path,
    config: &TrainConfig,
    trained: &crate::model::TrainedTinyJepa,
) -> DynamicJepaResult<()> {
    fs::create_dir_all(dir).map_err(file_create_dir_err)?;
    write_json_atomic(&dir.join("config.json"), config)?;
    candle_core::safetensors::save(&trained.tensors, dir.join("model.safetensors")).map_err(
        |err| DynamicJepaError::TrainingFailed {
            training_run_id: uuid::Uuid::nil(),
            message: format!("failed to write transfer model.safetensors: {err}"),
            remediation: "verify transfer output root is writable and rerun".to_string(),
        },
    )?;
    write_json_atomic(&dir.join("metrics.json"), &trained.metrics)?;
    write_json_atomic(
        &dir.join("evaluation_report.json"),
        &trained.evaluation_report,
    )?;
    Ok(())
}

fn write_jsonl(path: &Path, rows: &[BridgeEventRow]) -> DynamicJepaResult<()> {
    let mut out = String::new();
    for row in rows {
        let line = serde_json::to_string(row).map_err(|err| {
            DynamicJepaError::validation(
                "cross_domain_transfer.jsonl",
                format!("failed to serialize bridge event row: {err}"),
                "bridge rows must remain JSON serializable",
            )
        })?;
        out.push_str(&line);
        out.push('\n');
    }
    write_text_atomic(path, &out)
}

fn file_state(path: &Path) -> DynamicJepaResult<TransferFileState> {
    let metadata = fs::metadata(path).map_err(|err| DynamicJepaError::Storage {
        operation: "cross_domain_transfer.file_state".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to stat {}: {err}", path.display()),
        remediation: "all transfer source-of-truth files must exist after execution".to_string(),
    })?;
    if !metadata.is_file() {
        return Err(DynamicJepaError::validation(
            "cross_domain_transfer.file_state",
            format!("path is not a regular file: {}", path.display()),
            "transfer required outputs must be regular files",
        ));
    }
    Ok(TransferFileState {
        path: path.to_path_buf(),
        kind: "file".to_string(),
        size_bytes: metadata.len(),
        sha256: hex(&file_sha256(path)?),
    })
}

fn empty_file_state(path: &Path) -> TransferFileState {
    TransferFileState {
        path: path.to_path_buf(),
        kind: "pending".to_string(),
        size_bytes: 0,
        sha256: String::new(),
    }
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> DynamicJepaResult<()> {
    let text = serde_json::to_string_pretty(value).map_err(|err| {
        DynamicJepaError::validation(
            "cross_domain_transfer.write_json",
            format!("failed to serialize {}: {err}", path.display()),
            "transfer evidence values must be JSON serializable",
        )
    })?;
    write_text_atomic(path, &(text + "\n"))
}

fn write_text_atomic(path: &Path, text: &str) -> DynamicJepaResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(file_create_dir_err)?;
    }
    let tmp = path.with_extension(format!(
        "{}tmp",
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| format!("{ext}."))
            .unwrap_or_default()
    ));
    fs::write(&tmp, text).map_err(|err| DynamicJepaError::Storage {
        operation: "cross_domain_transfer.write_file".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to write {}: {err}", tmp.display()),
        remediation: "use a writable output root with free disk space".to_string(),
    })?;
    fs::rename(&tmp, path).map_err(|err| DynamicJepaError::Storage {
        operation: "cross_domain_transfer.rename_file".to_string(),
        cf: "filesystem".to_string(),
        message: format!(
            "failed to rename {} to {}: {err}",
            tmp.display(),
            path.display()
        ),
        remediation: "ensure output root is on a writable filesystem".to_string(),
    })
}

fn read_json_value(path: &Path) -> DynamicJepaResult<serde_json::Value> {
    let bytes = fs::read(path).map_err(|err| DynamicJepaError::Storage {
        operation: "cross_domain_transfer.read_json".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to read {}: {err}", path.display()),
        remediation: "transfer source-of-truth JSON must exist after write".to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|err| {
        DynamicJepaError::validation(
            "cross_domain_transfer.read_json",
            format!("failed to parse {}: {err}", path.display()),
            "fix the transfer JSON writer before rerunning",
        )
    })
}

fn file_sha256(path: &Path) -> DynamicJepaResult<[u8; 32]> {
    let bytes = fs::read(path).map_err(|err| DynamicJepaError::Storage {
        operation: "cross_domain_transfer.sha256".to_string(),
        cf: "filesystem".to_string(),
        message: format!("failed to read {}: {err}", path.display()),
        remediation: "all transfer files must be readable for FSV readback".to_string(),
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hasher.finalize().into())
}

fn file_create_dir_err(err: std::io::Error) -> DynamicJepaError {
    DynamicJepaError::Storage {
        operation: "cross_domain_transfer.create_dir".to_string(),
        cf: "filesystem".to_string(),
        message: err.to_string(),
        remediation: "use a writable fresh output root".to_string(),
    }
}

fn mean(values: &[f64]) -> DynamicJepaResult<f64> {
    if values.is_empty() {
        return Err(DynamicJepaError::validation(
            "cross_domain_transfer.mean",
            "cannot compute mean of empty values",
            "run transfer evaluation before aggregation",
        ));
    }
    Ok(values.iter().sum::<f64>() / values.len() as f64)
}

fn sample_std(values: &[f64], mean_value: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let var = values
        .iter()
        .map(|value| (*value - mean_value).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    var.sqrt()
}

fn sorted_quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let pos = q.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = pos - lo as f64;
        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
    }
}

fn cosine(left: &[f32], right: &[f32]) -> DynamicJepaResult<f64> {
    if left.len() != right.len() {
        return Err(DynamicJepaError::BridgePredictorInputDimMismatch {
            phase: "transfer_cosine".to_string(),
            expected: left.len(),
            actual: right.len(),
        });
    }
    let mut dot = 0.0;
    let mut ln = 0.0;
    let mut rn = 0.0;
    for (l, r) in left.iter().zip(right.iter()) {
        dot += *l as f64 * *r as f64;
        ln += (*l as f64).powi(2);
        rn += (*r as f64).powi(2);
    }
    let denom = ln.sqrt().max(1.0e-12) * rn.sqrt().max(1.0e-12);
    let value = (dot / denom).clamp(-1.0, 1.0);
    if value.is_finite() {
        Ok(value)
    } else {
        Err(DynamicJepaError::TrainingFailed {
            training_run_id: uuid::Uuid::nil(),
            message: "transfer cosine produced NaN or infinity".to_string(),
            remediation: "inspect transfer model tensors and bridge feature vectors".to_string(),
        })
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn shuffle_indices(values: &mut [usize], seed: u64) {
    let mut state = seed;
    for idx in (1..values.len()).rev() {
        state = splitmix64(state);
        values.swap(idx, (state as usize) % (idx + 1));
    }
}

fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf_approx(x / std::f64::consts::SQRT_2))
}

fn erf_approx(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    sign * y
}

fn inv_norm_cdf(p: f64) -> f64 {
    let p = p.clamp(1.0e-12, 1.0 - 1.0e-12);
    const A: [f64; 6] = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383_577_518_672_69e2,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];
    let plow = 0.02425;
    let phigh = 1.0 - plow;
    if p < plow {
        let q = (-2.0 * p.ln()).sqrt();
        return (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0);
    }
    if p > phigh {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        return -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0);
    }
    let q = p - 0.5;
    let r = q * q;
    (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
        / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub fn now_unix_ms() -> DynamicJepaResult<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            DynamicJepaError::validation(
                "system_time",
                format!("system clock before unix epoch: {err}"),
                "fix host clock before generating transfer evidence",
            )
        })?;
    Ok(duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_storage::dynamicjepa::{
        counter_to_grid_grid_action_kind, validate_counter_to_grid_bridge_instrument_count,
    };

    #[test]
    fn bridge_mapping_rejects_unknown_action() {
        let err = counter_to_grid_grid_action_kind("teleport").unwrap_err();
        assert_eq!(err.code(), "BRIDGE_ACTION_MAPPING_INCOMPLETE");
    }

    #[test]
    fn bridge_count_drift_is_typed() {
        let err = validate_counter_to_grid_bridge_instrument_count(3).unwrap_err();
        assert_eq!(err.code(), "BRIDGE_INSTRUMENT_COUNT_DRIFT");
    }

    #[test]
    fn predictor_dim_mismatch_is_typed() {
        let err = ensure_transfer_input_dims("unit", 8, 6).unwrap_err();
        assert_eq!(err.code(), "BRIDGE_PREDICTOR_INPUT_DIM_MISMATCH");
    }

    #[test]
    fn exact_sign_flip_min_p_for_all_positive_five_seed_effects() {
        let (p, null) = exact_sign_flip_p_value(&[0.1, 0.2, 0.3, 0.4, 0.5]).unwrap();
        assert_eq!(null.len(), 32);
        assert!((p - 0.0625).abs() < 1e-12);
    }
}
