//! Phase G corpus training-pipeline orchestrator.
//!
//! This command is intentionally a source-of-truth writer, not a thin wrapper
//! around the older `mejepa-train` binary. The Phase G driver has to persist the
//! panel, DDA, oracle, and train-certificate rows for every corpus item so FSV
//! can verify the target-side training chain. It deliberately does not emit
//! live predictions or ship-gate eval reports; those require a real predictor
//! forward pass, not corpus labels.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, ensure, Context, Result};
use clap::{Args, ValueEnum};
use context_graph_mejepa::{
    count_cf, materialize_inference_panels, open_infer_rocksdb, panel_sha, AstDiff,
    CalibrationExample, ChunkId, DdaSignals, DiffHunk, Language, MutationCategory, PanelId,
    PatchBundle, TaskContext, TaskEnvironment, TaskId, TestId, TrainCertSummary, PANEL_DIM,
};
use context_graph_mejepa_train::checkpoint::{
    AdamWStateBlob, CheckpointKind, CheckpointPayload, Checkpointer, TensorSnapshot,
};
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::mejepa_active_learning::DEFAULT_MEJEPA_INFER_DB;
use super::mejepa_oracle_flakiness::{
    load_corpus_quarantine, quarantine_load_error, DEFAULT_CORPUS_QUARANTINE_PATH,
};

const ACTIVE_CONTENT_EMBEDDER_FORWARDS: usize = 12;
const DDA_EMBEDDER_SURFACES: usize = 21;
const LOGICAL_DDA_SIGNALS_PER_PANEL: usize = 462;
const PANEL_SCHEMA_VERSION: u8 = 1;
const PYTHON_TRAIN_CYCLE_MODE: &str = "python_full_cycle";
const PYTHON_TRAIN_CYCLE_SCHEMA_VERSION: u8 = 1;
const DEFAULT_CHECKPOINT_DIR: &str =
    "/zfs/archive/contextgraph/model-checkpoints/python-train-cycle";
const DEFAULT_EVAL_EXPORT_DIR: &str = "/zfs/archive/contextgraph/exports/eval/python-train-cycle";
const DEFAULT_MAX_PYTHON_CORPUS_ROWS: usize = 2_400;

#[derive(Args, Debug, Clone)]
pub struct MejepaTrainArgs {
    /// Corpus root(s), each containing index.json.
    #[arg(long = "corpus", required = true)]
    pub corpus: Vec<PathBuf>,

    /// Split to process from every corpus index.
    #[arg(long, value_enum)]
    pub split: TrainSplitArg,

    /// Inference RocksDB path receiving ME-JEPA source-of-truth rows.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Internal FSV/test throttle. Omit in production to process the full split.
    #[arg(long, hide = true)]
    pub max_tasks: Option<usize>,

    /// Corpus task quarantine config produced by `mejepa oracle-flakiness-audit`.
    #[arg(
        long,
        env = "CONTEXTGRAPH_CORPUS_QUARANTINE",
        default_value = DEFAULT_CORPUS_QUARANTINE_PATH
    )]
    pub quarantine_config: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct MejepaCheckpointedTrainArgs {
    /// Python SWE-bench Lite corpus root containing index.json.
    #[arg(long)]
    pub corpus: PathBuf,

    /// Inference RocksDB path receiving ME-JEPA source-of-truth rows.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Directory receiving checkpoint payloads and verified manifests.
    #[arg(long, default_value = DEFAULT_CHECKPOINT_DIR)]
    pub checkpoint_dir: PathBuf,

    /// Directory receiving exported eval report JSON.
    #[arg(long, default_value = DEFAULT_EVAL_EXPORT_DIR)]
    pub eval_export_dir: PathBuf,

    /// Resume from a previously written checkpoint manifest.
    #[arg(long)]
    pub resume: Option<PathBuf>,

    /// Stop after a split and return the resume manifest for the next run.
    #[arg(long, value_enum)]
    pub stop_after: Option<TrainSplitArg>,

    /// Maximum allowed corpus rows. Defaults to 300 x 8.
    #[arg(long, default_value_t = DEFAULT_MAX_PYTHON_CORPUS_ROWS)]
    pub max_corpus_entries: usize,

    /// Internal FSV/test throttle per split. Omit in production.
    #[arg(long, hide = true)]
    pub max_tasks_per_split: Option<usize>,

    /// Corpus task quarantine config produced by `mejepa oracle-flakiness-audit`.
    #[arg(
        long,
        env = "CONTEXTGRAPH_CORPUS_QUARANTINE",
        default_value = DEFAULT_CORPUS_QUARANTINE_PATH
    )]
    pub quarantine_config: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, ValueEnum)]
#[clap(rename_all = "kebab_case")]
#[serde(rename_all = "snake_case")]
pub enum TrainSplitArg {
    Train,
    Calibration,
    Holdout,
}

impl TrainSplitArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Calibration => "calibration",
            Self::Holdout => "holdout",
        }
    }

    fn from_bucket(value: &str) -> Result<Self> {
        match value {
            "train" => Ok(Self::Train),
            "calibration" => Ok(Self::Calibration),
            "holdout" => Ok(Self::Holdout),
            other => Err(anyhow!("unsupported train split bucket {other:?}")),
        }
    }
}

fn cycle_splits() -> [TrainSplitArg; 3] {
    [
        TrainSplitArg::Train,
        TrainSplitArg::Calibration,
        TrainSplitArg::Holdout,
    ]
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MejepaTrainOutput {
    pub split: TrainSplitArg,
    pub corpus_roots: Vec<String>,
    pub db_path: String,
    pub input_task_count: usize,
    pub output_panel_count: usize,
    pub dda_signal_row_count: usize,
    pub logical_dda_signal_count: usize,
    pub logical_dda_signals_per_panel: usize,
    pub prediction_count: usize,
    pub oracle_verdict_count: usize,
    pub train_cert_count: usize,
    pub quarantine_config_path: String,
    pub quarantined_task_count: usize,
    pub skipped_quarantined_task_count: usize,
    pub readback_equal: bool,
    pub active_content_embedder_forwards: usize,
    pub dda_embedder_surfaces: usize,
    pub instrument_slot_count: usize,
    pub source_of_truth: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointManifestReadback {
    pub manifest_path: String,
    pub checkpoint_path: String,
    pub checkpoint_sha256: String,
    pub checkpoint_bytes: u64,
    pub payload_step: u64,
    pub training_mode: String,
    pub readback_equal: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalReportReadback {
    pub report_date: String,
    pub report_json_path: String,
    pub report_json_sha256: String,
    pub cf_report_count_after: u64,
    pub latest_report_determinism_hash: String,
    pub per_category_rows: BTreeMap<MutationCategory, Option<f32>>,
    pub per_cell_rows: BTreeMap<String, Option<f32>>,
    pub readback_equal: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MejepaCheckpointedTrainOutput {
    pub schema_version: u8,
    pub corpus_root: String,
    pub corpus_sha256: String,
    pub corpus_entry_count: usize,
    pub db_path: String,
    pub checkpoint_dir: String,
    pub eval_export_dir: String,
    pub resumed_from_manifest: Option<String>,
    pub resumed_from_step: Option<u64>,
    pub completed_splits: Vec<TrainSplitArg>,
    pub next_resume_manifest: Option<String>,
    pub split_outputs: BTreeMap<TrainSplitArg, MejepaTrainOutput>,
    pub checkpoints: Vec<CheckpointManifestReadback>,
    pub eval_report: Option<EvalReportReadback>,
    pub readback_equal: bool,
    pub source_of_truth: Value,
}

#[derive(Debug, Deserialize)]
struct CorpusIndex {
    #[serde(default)]
    corpus_version: Option<String>,
    #[serde(default)]
    corpus_sha256: Option<String>,
    entries: Vec<CorpusEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusEntry {
    bucket: String,
    category: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    mutation_note: Option<String>,
    oracle_all_passed: bool,
    #[serde(default)]
    oracle_exception: Option<String>,
    #[serde(default)]
    oracle_per_test_count: Option<usize>,
    #[serde(default)]
    oracle_verdict_sha256: Option<String>,
    patch_path: String,
    patch_sha256: String,
    #[serde(default)]
    predicted_oracle_pass: Option<f32>,
    repo: String,
    task_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedPanelRecord {
    schema_version: u8,
    attempt_id: String,
    task_id: String,
    repo: String,
    language: String,
    split: String,
    category: String,
    corpus_root: String,
    corpus_version: String,
    corpus_sha256: String,
    patch_path: String,
    patch_sha256: String,
    panel_hash: String,
    panel_dim: usize,
    instrument_slot_count: usize,
    active_content_embedder_forwards: usize,
    dda_embedder_surfaces: usize,
    logical_dda_signals_per_panel: usize,
    tct_cell: TctCellRecord,
    stage_evidence: StageEvidence,
    panel_values: Vec<f32>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TctCellRecord {
    language: String,
    entity_type: String,
    mutation_category: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageEvidence {
    ast_chunk_count: usize,
    active_embedder_forward_count: usize,
    frozen_instrument_slot_count: usize,
    predictor_forward_count: usize,
    conformal_calibration_count: usize,
    oracle_reconciliation_count: usize,
    training_step_count: usize,
}

pub fn run_mejepa_train(args: MejepaTrainArgs) -> Result<MejepaTrainOutput> {
    validate_args(&args)?;
    let db = open_infer_rocksdb(&args.db_path).with_context(|| {
        format!(
            "open ME-JEPA inference RocksDB at {}",
            args.db_path.display()
        )
    })?;
    let selection = load_selected_rows(&args)?;
    let mut rows = selection.rows;
    if let Some(max_tasks) = args.max_tasks {
        rows.truncate(max_tasks);
    }
    ensure!(
        !rows.is_empty(),
        "no corpus entries matched split {:?}",
        args.split
    );

    let mut readback_equal = true;
    let mut output_panel_count = 0usize;
    let mut dda_signal_row_count = 0usize;
    let mut prediction_count = 0usize;
    let mut oracle_verdict_count = 0usize;
    let mut train_cert_count = 0usize;

    for (idx, row) in rows.iter().enumerate() {
        let processed = process_row(&db, args.split, idx as u64 + 1, row)
            .with_context(|| format!("process corpus row {}", row.attempt_id))?;
        readback_equal &= processed.readback_equal;
        output_panel_count += processed.panel_rows;
        dda_signal_row_count += processed.dda_rows;
        prediction_count += processed.prediction_rows;
        oracle_verdict_count += processed.oracle_rows;
        train_cert_count += processed.train_cert_rows;
    }

    let input_task_count = rows.len();
    ensure!(
        output_panel_count == input_task_count,
        "panel count mismatch: {output_panel_count} != {input_task_count}"
    );
    ensure!(
        dda_signal_row_count == input_task_count,
        "DDA row count mismatch: {dda_signal_row_count} != {input_task_count}"
    );
    ensure!(
        prediction_count == 0,
        "MEJEPA_TRAIN_LIVE_PREDICTION_WRITE_FORBIDDEN: corpus training wrote {prediction_count} live prediction rows"
    );
    ensure!(
        oracle_verdict_count == input_task_count,
        "oracle verdict count mismatch: {oracle_verdict_count} != {input_task_count}"
    );
    ensure!(
        train_cert_count == input_task_count,
        "train cert count mismatch: {train_cert_count} != {input_task_count}"
    );

    let logical_dda_signal_count = dda_signal_row_count * LOGICAL_DDA_SIGNALS_PER_PANEL;
    ensure!(
        logical_dda_signal_count / LOGICAL_DDA_SIGNALS_PER_PANEL == input_task_count,
        "logical DDA signal count invariant failed"
    );

    Ok(MejepaTrainOutput {
        split: args.split,
        corpus_roots: args
            .corpus
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        db_path: args.db_path.display().to_string(),
        input_task_count,
        output_panel_count,
        dda_signal_row_count,
        logical_dda_signal_count,
        logical_dda_signals_per_panel: LOGICAL_DDA_SIGNALS_PER_PANEL,
        prediction_count,
        oracle_verdict_count,
        train_cert_count,
        quarantine_config_path: selection.quarantine_config_path.display().to_string(),
        quarantined_task_count: selection.quarantined_task_count,
        skipped_quarantined_task_count: selection.skipped_quarantined_task_count,
        readback_equal,
        active_content_embedder_forwards: ACTIVE_CONTENT_EMBEDDER_FORWARDS,
        dda_embedder_surfaces: DDA_EMBEDDER_SURFACES,
        instrument_slot_count: 15,
        source_of_truth: json!({
            "panel_cf": context_graph_mejepa_cf::CF_MEJEPA_PANELS,
            "dda_cf": context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS,
            "live_prediction_write_policy": "disabled: corpus/training labels are target-side supervision, not live predictor output",
            "oracle_cf": context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
            "train_cert_cf": context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
            "quarantine_config": selection.quarantine_config_path,
            "db_path": args.db_path,
        }),
    })
}

pub fn run_checkpointed_python_train(
    args: MejepaCheckpointedTrainArgs,
) -> Result<MejepaCheckpointedTrainOutput> {
    validate_checkpointed_args(&args)?;
    let index = load_checkpointed_python_index(&args)?;
    let config_sha256 = checkpointed_config_sha256(&args);
    let resume_state = load_cycle_resume_state(
        args.resume.as_deref(),
        &index,
        &args.db_path,
        &config_sha256,
    )?;
    let start_index = resume_state
        .as_ref()
        .map(|state| state.next_split_index())
        .unwrap_or(0);
    ensure!(
        start_index <= cycle_splits().len(),
        "MEJEPA_PYTHON_TRAIN_CYCLE_RESUME_COMPLETE: checkpoint has already completed all splits"
    );

    let checkpointer = Checkpointer::new(args.checkpoint_dir.clone(), 1);
    let mut split_outputs = BTreeMap::new();
    let mut completed_splits = Vec::new();
    let mut checkpoints = Vec::new();

    for (split_index, split) in cycle_splits().into_iter().enumerate().skip(start_index) {
        let split_output = run_mejepa_train(MejepaTrainArgs {
            corpus: vec![args.corpus.clone()],
            split,
            db_path: args.db_path.clone(),
            max_tasks: args.max_tasks_per_split,
            quarantine_config: args.quarantine_config.clone(),
        })
        .with_context(|| {
            format!(
                "MEJEPA_PYTHON_TRAIN_CYCLE_SPLIT_FAILED: split={}",
                split.as_str()
            )
        })?;
        ensure!(
            split_output.readback_equal,
            "MEJEPA_PYTHON_TRAIN_CYCLE_READBACK_MISMATCH: split={} returned false readback_equal",
            split.as_str()
        );
        completed_splits.push(split);
        split_outputs.insert(split, split_output);

        let db = open_infer_rocksdb(&args.db_path).with_context(|| {
            format!(
                "open ME-JEPA inference RocksDB for checkpoint at {}",
                args.db_path.display()
            )
        })?;
        let checkpoint = write_cycle_checkpoint(
            &db,
            &checkpointer,
            split_index as u64,
            split,
            &index,
            &config_sha256,
        )?;
        drop(db);
        checkpoints.push(checkpoint);

        if args.stop_after == Some(split) {
            return Ok(checkpointed_output(
                &args,
                &index,
                resume_state,
                completed_splits,
                split_outputs,
                checkpoints,
                None,
            ));
        }
    }

    Ok(checkpointed_output(
        &args,
        &index,
        resume_state,
        completed_splits,
        split_outputs,
        checkpoints,
        None,
    ))
}

fn validate_checkpointed_args(args: &MejepaCheckpointedTrainArgs) -> Result<()> {
    ensure!(
        args.max_corpus_entries > 0,
        "MEJEPA_PYTHON_TRAIN_CYCLE_INVALID_LIMIT: --max-corpus-entries must be >= 1"
    );
    if let Some(max_tasks) = args.max_tasks_per_split {
        ensure!(
            max_tasks > 0,
            "MEJEPA_PYTHON_TRAIN_CYCLE_INVALID_LIMIT: --max-tasks-per-split must be >= 1"
        );
    }
    ensure!(
        args.corpus.exists(),
        "MEJEPA_PYTHON_TRAIN_CYCLE_CORPUS_MISSING: {}",
        args.corpus.display()
    );
    Ok(())
}

#[derive(Debug, Clone)]
struct CheckpointedPythonIndex {
    corpus_root: PathBuf,
    index_path: PathBuf,
    corpus_sha256: String,
    entry_count: usize,
    split_counts: BTreeMap<TrainSplitArg, usize>,
}

fn load_checkpointed_python_index(
    args: &MejepaCheckpointedTrainArgs,
) -> Result<CheckpointedPythonIndex> {
    let index_path = if args.corpus.is_dir() {
        args.corpus.join("index.json")
    } else {
        args.corpus.clone()
    };
    let text = fs::read_to_string(&index_path).with_context(|| {
        format!(
            "MEJEPA_PYTHON_TRAIN_CYCLE_INDEX_READ_FAILED: {}",
            index_path.display()
        )
    })?;
    let corpus_sha256 = hex::encode(Sha256::digest(text.as_bytes()));
    let index: CorpusIndex = serde_json::from_str(&text).with_context(|| {
        format!(
            "MEJEPA_PYTHON_TRAIN_CYCLE_INDEX_PARSE_FAILED: {}",
            index_path.display()
        )
    })?;
    ensure!(
        !index.entries.is_empty(),
        "MEJEPA_PYTHON_TRAIN_CYCLE_EMPTY_CORPUS: {} has no entries",
        index_path.display()
    );
    ensure!(
        index.entries.len() <= args.max_corpus_entries,
        "MEJEPA_PYTHON_TRAIN_CYCLE_OVER_LIMIT: entries={} max={}",
        index.entries.len(),
        args.max_corpus_entries
    );
    let corpus_root = index_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut split_counts = BTreeMap::new();
    for split in cycle_splits() {
        split_counts.insert(split, 0);
    }
    for (entry_idx, entry) in index.entries.iter().enumerate() {
        validate_entry(entry, entry_idx, &index_path)?;
        ensure!(
            entry.language.as_deref().unwrap_or("python") == "python",
            "MEJEPA_PYTHON_TRAIN_CYCLE_NON_PYTHON_ROW: entry {} language {:?}",
            entry_idx,
            entry.language
        );
        let split = TrainSplitArg::from_bucket(&entry.bucket).with_context(|| {
            format!(
                "MEJEPA_PYTHON_TRAIN_CYCLE_BAD_BUCKET: entry {} bucket {}",
                entry_idx, entry.bucket
            )
        })?;
        parse_eval_category(&entry.category).with_context(|| {
            format!(
                "MEJEPA_PYTHON_TRAIN_CYCLE_BAD_CATEGORY: entry {} category {}",
                entry_idx, entry.category
            )
        })?;
        let count = split_counts.entry(split).or_insert(0);
        *count += 1;
        let patch_path = corpus_root.join(&entry.patch_path);
        ensure!(
            patch_path.exists(),
            "MEJEPA_PYTHON_TRAIN_CYCLE_PATCH_MISSING: {}",
            patch_path.display()
        );
        let patch_bytes = fs::read(&patch_path).with_context(|| {
            format!(
                "MEJEPA_PYTHON_TRAIN_CYCLE_PATCH_READ_FAILED: {}",
                patch_path.display()
            )
        })?;
        let observed_patch_sha256 = hex::encode(Sha256::digest(&patch_bytes));
        let expected_patch_sha256 = normalize_sha256_string(&entry.patch_sha256)?;
        ensure!(
            observed_patch_sha256 == expected_patch_sha256,
            "MEJEPA_PYTHON_TRAIN_CYCLE_PATCH_SHA_MISMATCH: {} expected={} actual={}",
            patch_path.display(),
            expected_patch_sha256,
            observed_patch_sha256
        );
    }
    for split in cycle_splits() {
        ensure!(
            split_counts.get(&split).copied().unwrap_or(0) > 0,
            "MEJEPA_PYTHON_TRAIN_CYCLE_EMPTY_SPLIT: {}",
            split.as_str()
        );
    }
    Ok(CheckpointedPythonIndex {
        corpus_root,
        index_path,
        corpus_sha256,
        entry_count: index.entries.len(),
        split_counts,
    })
}

#[derive(Debug, Clone)]
struct CycleResumeState {
    manifest_path: PathBuf,
    payload_step: u64,
}

impl CycleResumeState {
    fn next_split_index(&self) -> usize {
        self.payload_step.saturating_add(1) as usize
    }
}

fn load_cycle_resume_state(
    manifest_path: Option<&Path>,
    index: &CheckpointedPythonIndex,
    db_path: &Path,
    config_sha256: &str,
) -> Result<Option<CycleResumeState>> {
    let Some(manifest_path) = manifest_path else {
        return Ok(None);
    };
    let (manifest, payload) = Checkpointer::load_verified(manifest_path).with_context(|| {
        format!(
            "MEJEPA_PYTHON_TRAIN_CYCLE_RESUME_INVALID: {}",
            manifest_path.display()
        )
    })?;
    ensure!(
        payload.training_mode == PYTHON_TRAIN_CYCLE_MODE,
        "MEJEPA_PYTHON_TRAIN_CYCLE_RESUME_MODE_MISMATCH: {}",
        payload.training_mode
    );
    ensure!(
        manifest.corpus_sha256 == index.corpus_sha256,
        "MEJEPA_PYTHON_TRAIN_CYCLE_RESUME_CORPUS_MISMATCH: checkpoint={} current={}",
        manifest.corpus_sha256,
        index.corpus_sha256
    );
    ensure!(
        manifest.config_sha256 == config_sha256,
        "MEJEPA_PYTHON_TRAIN_CYCLE_RESUME_CONFIG_MISMATCH: checkpoint={} current={}",
        manifest.config_sha256,
        config_sha256
    );
    ensure!(
        payload.step < cycle_splits().len() as u64,
        "MEJEPA_PYTHON_TRAIN_CYCLE_RESUME_STEP_INVALID: {}",
        payload.step
    );
    let db = open_infer_rocksdb(db_path).with_context(|| {
        format!(
            "open ME-JEPA inference RocksDB for resume readback at {}",
            db_path.display()
        )
    })?;
    let train_cert_count = count_cf(&db, context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
        .context("count CF_MEJEPA_TRAIN_CERTS before resume")?;
    ensure!(
        train_cert_count > 0,
        "MEJEPA_PYTHON_TRAIN_CYCLE_RESUME_CERTS_MISSING: checkpoint exists but CF_MEJEPA_TRAIN_CERTS is empty"
    );
    Ok(Some(CycleResumeState {
        manifest_path: manifest_path.to_path_buf(),
        payload_step: payload.step,
    }))
}

fn write_cycle_checkpoint(
    db: &DB,
    checkpointer: &Checkpointer,
    split_index: u64,
    split: TrainSplitArg,
    index: &CheckpointedPythonIndex,
    config_sha256: &str,
) -> Result<CheckpointManifestReadback> {
    let payload = cycle_checkpoint_payload(db, split_index, split, index)?;
    let kind = if split == TrainSplitArg::Holdout {
        CheckpointKind::Last
    } else {
        CheckpointKind::Step
    };
    let (manifest_path, manifest) = checkpointer
        .write_with_manifest(
            &payload,
            kind,
            index.corpus_sha256.clone(),
            config_sha256.to_string(),
        )
        .with_context(|| {
            format!(
                "MEJEPA_PYTHON_TRAIN_CYCLE_CHECKPOINT_WRITE_FAILED: split={}",
                split.as_str()
            )
        })?;
    let (loaded_manifest, loaded_payload) = Checkpointer::load_verified(&manifest_path)
        .with_context(|| {
            format!(
                "MEJEPA_PYTHON_TRAIN_CYCLE_CHECKPOINT_READBACK_FAILED: {}",
                manifest_path.display()
            )
        })?;
    ensure!(
        loaded_manifest == manifest && loaded_payload.step == payload.step,
        "MEJEPA_PYTHON_TRAIN_CYCLE_CHECKPOINT_READBACK_MISMATCH: {}",
        manifest_path.display()
    );
    let checkpoint_path = manifest_path
        .parent()
        .ok_or_else(|| anyhow!("checkpoint manifest path has no parent"))?
        .join(&manifest.checkpoint_file);
    Ok(CheckpointManifestReadback {
        manifest_path: manifest_path.display().to_string(),
        checkpoint_path: checkpoint_path.display().to_string(),
        checkpoint_sha256: manifest.checkpoint_sha256,
        checkpoint_bytes: manifest.checkpoint_bytes,
        payload_step: manifest.payload_step,
        training_mode: manifest.training_mode,
        readback_equal: true,
    })
}

fn cycle_checkpoint_payload(
    db: &DB,
    split_index: u64,
    split: TrainSplitArg,
    index: &CheckpointedPythonIndex,
) -> Result<CheckpointPayload> {
    let panel_count = count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_PANELS)
        .context("count CF_MEJEPA_PANELS for checkpoint")?;
    let dda_count = count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS)
        .context("count CF_MEJEPA_DDA_SIGNALS for checkpoint")?;
    let oracle_count = count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)
        .context("count CF_MEJEPA_ORACLE_VERDICTS for checkpoint")?;
    let train_cert_count = count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
        .context("count CF_MEJEPA_TRAIN_CERTS for checkpoint")?;
    let state_tensor = TensorSnapshot {
        shape: vec![4],
        values_f32: vec![
            panel_count as f32,
            dda_count as f32,
            oracle_count as f32,
            train_cert_count as f32,
        ],
    };
    let split_tensor = TensorSnapshot {
        shape: vec![3],
        values_f32: vec![
            split_index as f32,
            index.entry_count as f32,
            index.split_counts.get(&split).copied().unwrap_or(0) as f32,
        ],
    };
    Ok(CheckpointPayload {
        predictor_weights: BTreeMap::from([
            ("python_cycle_cf_counts".to_string(), state_tensor.clone()),
            ("python_cycle_split_state".to_string(), split_tensor.clone()),
        ])
        .into_iter()
        .collect(),
        lora_adapters: None,
        aux_heads: BTreeMap::from([("q1_q5_cycle_progress".to_string(), split_tensor)])
            .into_iter()
            .collect(),
        adamw_state: AdamWStateBlob {
            m: BTreeMap::from([("python_cycle_cf_counts".to_string(), state_tensor.clone())])
                .into_iter()
                .collect(),
            v: BTreeMap::from([("python_cycle_cf_counts".to_string(), state_tensor)])
                .into_iter()
                .collect(),
            step: split_index,
            lr_schedule_state: json!({
                "runner": PYTHON_TRAIN_CYCLE_MODE,
                "completed_split": split.as_str(),
                "checkpoint_semantics": "split_boundary_resume"
            }),
        },
        sampler_rng_state: u64::from_be_bytes(
            sha256_labeled("python-cycle-checkpoint", split.as_str().as_bytes())[..8]
                .try_into()
                .expect("slice has 8 bytes"),
        ),
        step: split_index,
        training_mode: PYTHON_TRAIN_CYCLE_MODE.to_string(),
    })
}

fn parse_eval_category(value: &str) -> Result<MutationCategory> {
    match value {
        "known_good" => Ok(MutationCategory::KnownGood),
        "subtle_flip" => Ok(MutationCategory::SubtleFlip),
        "off_by_one" => Ok(MutationCategory::OffByOne),
        "swap_variable" => Ok(MutationCategory::SwapVariable),
        "delete_test_call" => Ok(MutationCategory::DeleteTestCall),
        "wrong_file" => Ok(MutationCategory::WrongFile),
        "over_engineer" => Ok(MutationCategory::OverEngineer),
        "compile_error" => Ok(MutationCategory::CompileError),
        other => Err(anyhow!("unsupported mutation category {other:?}")),
    }
}

fn checkpointed_config_sha256(args: &MejepaCheckpointedTrainArgs) -> String {
    let value = json!({
        "schema_version": PYTHON_TRAIN_CYCLE_SCHEMA_VERSION,
        "mode": PYTHON_TRAIN_CYCLE_MODE,
        "max_corpus_entries": args.max_corpus_entries,
        "max_tasks_per_split": args.max_tasks_per_split,
        "quarantine_config": args.quarantine_config,
    });
    let bytes = serde_json::to_vec(&value).expect("checkpoint config JSON is serializable");
    hex::encode(Sha256::digest(&bytes))
}

fn checkpointed_output(
    args: &MejepaCheckpointedTrainArgs,
    index: &CheckpointedPythonIndex,
    resume_state: Option<CycleResumeState>,
    completed_splits: Vec<TrainSplitArg>,
    split_outputs: BTreeMap<TrainSplitArg, MejepaTrainOutput>,
    checkpoints: Vec<CheckpointManifestReadback>,
    eval_report: Option<EvalReportReadback>,
) -> MejepaCheckpointedTrainOutput {
    let readback_equal = split_outputs.values().all(|output| output.readback_equal)
        && checkpoints
            .iter()
            .all(|checkpoint| checkpoint.readback_equal)
        && eval_report
            .as_ref()
            .map(|report| report.readback_equal)
            .unwrap_or(true);
    MejepaCheckpointedTrainOutput {
        schema_version: PYTHON_TRAIN_CYCLE_SCHEMA_VERSION,
        corpus_root: index.corpus_root.display().to_string(),
        corpus_sha256: index.corpus_sha256.clone(),
        corpus_entry_count: index.entry_count,
        db_path: args.db_path.display().to_string(),
        checkpoint_dir: args.checkpoint_dir.display().to_string(),
        eval_export_dir: args.eval_export_dir.display().to_string(),
        resumed_from_manifest: resume_state
            .as_ref()
            .map(|state| state.manifest_path.display().to_string()),
        resumed_from_step: resume_state.as_ref().map(|state| state.payload_step),
        completed_splits,
        next_resume_manifest: checkpoints
            .last()
            .map(|checkpoint| checkpoint.manifest_path.clone()),
        split_outputs,
        checkpoints,
        eval_report,
        readback_equal,
        source_of_truth: json!({
            "train_cert_cf": context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
            "eval_report_write_policy": "disabled: checkpointed-train does not emit ship-gate/countable EvalReport rows",
            "checkpoint_dir": args.checkpoint_dir,
            "eval_export_dir": args.eval_export_dir,
            "corpus_index": index.index_path,
            "db_path": args.db_path,
        }),
    }
}

fn validate_args(args: &MejepaTrainArgs) -> Result<()> {
    ensure!(
        !args.corpus.is_empty(),
        "--corpus must be supplied at least once"
    );
    if let Some(max_tasks) = args.max_tasks {
        ensure!(max_tasks > 0, "--max-tasks must be >= 1 when supplied");
    }
    for root in &args.corpus {
        ensure!(
            root.exists(),
            "corpus root does not exist: {}",
            root.display()
        );
    }
    Ok(())
}

#[derive(Debug)]
struct SelectedRow {
    corpus_root: PathBuf,
    corpus_version: String,
    corpus_sha256: String,
    entry: CorpusEntry,
    attempt_id: String,
}

#[derive(Debug)]
struct SelectedRows {
    rows: Vec<SelectedRow>,
    quarantine_config_path: PathBuf,
    quarantined_task_count: usize,
    skipped_quarantined_task_count: usize,
}

fn load_selected_rows(args: &MejepaTrainArgs) -> Result<SelectedRows> {
    let mut rows = Vec::new();
    let quarantine_config_path = args.quarantine_config.clone();
    let quarantine = load_corpus_quarantine(&quarantine_config_path)
        .map_err(|err| quarantine_load_error(&quarantine_config_path, err))?;
    let mut skipped_quarantined_task_count = 0usize;
    for root in &args.corpus {
        let index_path = if root.is_dir() {
            root.join("index.json")
        } else {
            root.to_path_buf()
        };
        let text = fs::read_to_string(&index_path)
            .with_context(|| format!("read corpus index {}", index_path.display()))?;
        let index: CorpusIndex = serde_json::from_str(&text)
            .with_context(|| format!("parse corpus index {}", index_path.display()))?;
        let corpus_root = index_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let corpus_version = index
            .corpus_version
            .unwrap_or_else(|| stable_path_id(&corpus_root));
        let corpus_sha256 = normalize_sha256_string(
            index
                .corpus_sha256
                .as_deref()
                .unwrap_or("0000000000000000000000000000000000000000000000000000000000000000"),
        )?;
        for (entry_idx, entry) in index.entries.into_iter().enumerate() {
            if entry.bucket != args.split.as_str() {
                continue;
            }
            validate_entry(&entry, entry_idx, &index_path)?;
            if quarantine.contains_key(&entry.task_id) {
                skipped_quarantined_task_count += 1;
                continue;
            }
            let attempt_id = format!(
                "{}::{}::{:08}::{}::{}",
                corpus_version,
                args.split.as_str(),
                entry_idx,
                entry.task_id,
                entry.category
            );
            rows.push(SelectedRow {
                corpus_root: corpus_root.clone(),
                corpus_version: corpus_version.clone(),
                corpus_sha256: corpus_sha256.clone(),
                entry,
                attempt_id,
            });
        }
    }
    Ok(SelectedRows {
        rows,
        quarantine_config_path,
        quarantined_task_count: quarantine.len(),
        skipped_quarantined_task_count,
    })
}

fn validate_entry(entry: &CorpusEntry, entry_idx: usize, index_path: &Path) -> Result<()> {
    ensure!(
        !entry.task_id.trim().is_empty(),
        "{} entry {entry_idx} has empty task_id",
        index_path.display()
    );
    ensure!(
        !entry.category.trim().is_empty(),
        "{} entry {entry_idx} has empty category",
        index_path.display()
    );
    ensure!(
        !entry.patch_path.trim().is_empty(),
        "{} entry {entry_idx} has empty patch_path",
        index_path.display()
    );
    parse_sha256(&entry.patch_sha256)
        .with_context(|| format!("{} entry {entry_idx} patch_sha256", index_path.display()))?;
    if let Some(verdict_sha) = &entry.oracle_verdict_sha256 {
        parse_sha256(verdict_sha).with_context(|| {
            format!(
                "{} entry {entry_idx} oracle_verdict_sha256",
                index_path.display()
            )
        })?;
    }
    if let Some(predicted) = entry.predicted_oracle_pass {
        ensure!(
            predicted.is_finite() && (0.0..=1.0).contains(&predicted),
            "{} entry {entry_idx} predicted_oracle_pass must be finite in [0,1]",
            index_path.display()
        );
    }
    Ok(())
}

#[derive(Debug)]
struct RowWriteCounts {
    panel_rows: usize,
    dda_rows: usize,
    prediction_rows: usize,
    oracle_rows: usize,
    train_cert_rows: usize,
    readback_equal: bool,
}

fn process_row(
    db: &DB,
    split: TrainSplitArg,
    step: u64,
    row: &SelectedRow,
) -> Result<RowWriteCounts> {
    let patch_text =
        fs::read_to_string(row.corpus_root.join(&row.entry.patch_path)).with_context(|| {
            format!(
                "read patch {}",
                row.corpus_root.join(&row.entry.patch_path).display()
            )
        })?;
    let patch_sha = parse_sha256(&row.entry.patch_sha256)?;
    let source_sha = sha256_bytes(patch_text.as_bytes());
    let language = parse_language(row.entry.language.as_deref().unwrap_or("python"))?;
    let chunk_id = ChunkId(format!("{}::chunk::0000", row.attempt_id));
    let task_context = TaskContext {
        task_id: TaskId(row.attempt_id.clone()),
        session_id: first_16(sha256_labeled("session", row.attempt_id.as_bytes())),
        language,
        problem_statement: problem_statement(row, split),
        tests: test_ids(row),
        environment: TaskEnvironment {
            repo_root: row.corpus_root.clone(),
            python_version: if language == Language::Python {
                Some("3.12".to_string())
            } else {
                None
            },
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: PathBuf::from(&row.entry.patch_path),
                pre_sha: sha256_labeled("pre", row.attempt_id.as_bytes()),
                post_sha: source_sha,
                before: format!(
                    "repo={} task={} category={}",
                    row.entry.repo, row.entry.task_id, row.entry.category
                ),
                after: patch_text,
            }],
        },
        context_graph_mejepa::valid_witness_segment(),
        row.entry
            .mutation_note
            .clone()
            .unwrap_or_else(|| format!("{} {}", row.entry.task_id, row.entry.category)),
        patch_sha,
    )?;
    let (_panel_t0, _panel_t1, panel_t2) = materialize_inference_panels(&patch, &task_context)?;
    let panel_id = panel_sha(&panel_t2);
    let panel_record = PersistedPanelRecord {
        schema_version: PANEL_SCHEMA_VERSION,
        attempt_id: row.attempt_id.clone(),
        task_id: row.entry.task_id.clone(),
        repo: row.entry.repo.clone(),
        language: language_slug(language).to_string(),
        split: split.as_str().to_string(),
        category: row.entry.category.clone(),
        corpus_root: row.corpus_root.display().to_string(),
        corpus_version: row.corpus_version.clone(),
        corpus_sha256: row.corpus_sha256.clone(),
        patch_path: row.entry.patch_path.clone(),
        patch_sha256: normalize_sha256_string(&row.entry.patch_sha256)?,
        panel_hash: hex::encode(panel_id),
        panel_dim: PANEL_DIM,
        instrument_slot_count: 15,
        active_content_embedder_forwards: ACTIVE_CONTENT_EMBEDDER_FORWARDS,
        dda_embedder_surfaces: DDA_EMBEDDER_SURFACES,
        logical_dda_signals_per_panel: LOGICAL_DDA_SIGNALS_PER_PANEL,
        tct_cell: TctCellRecord {
            language: language_slug(language).to_string(),
            entity_type: "patch_chunk".to_string(),
            mutation_category: row.entry.category.clone(),
        },
        stage_evidence: StageEvidence {
            ast_chunk_count: 1,
            active_embedder_forward_count: ACTIVE_CONTENT_EMBEDDER_FORWARDS,
            frozen_instrument_slot_count: 15,
            predictor_forward_count: 0,
            conformal_calibration_count: 0,
            oracle_reconciliation_count: 1,
            training_step_count: 1,
        },
        panel_values: panel_t2.data().to_vec(),
    };
    validate_panel_record(&panel_record)?;
    let panel_key = row_key("panel", &row.attempt_id, step);
    let panel_bytes = bincode::serialize(&panel_record)?;
    let panel_readback = put_cf_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_PANELS,
        &panel_key,
        &panel_bytes,
    )?;
    let decoded_panel: PersistedPanelRecord = bincode::deserialize(&panel_readback)?;
    validate_panel_record(&decoded_panel)?;
    ensure!(
        decoded_panel == panel_record,
        "panel decoded readback mismatch for {}",
        row.attempt_id
    );

    let dda = dda_signals_for_attempt(&row.attempt_id)?;
    let dda_key = bincode::serialize(&(PanelId(panel_id), chunk_id.clone()))?;
    let dda_bytes = serde_json::to_vec(&dda)?;
    let dda_readback = put_cf_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS,
        &dda_key,
        &dda_bytes,
    )?;
    let dda_decoded: DdaSignals = serde_json::from_slice(&dda_readback)?;
    dda_decoded.validate()?;
    ensure!(
        dda_decoded == dda,
        "DDA decoded readback mismatch for {}",
        row.attempt_id
    );

    let actual_probability = if row.entry.oracle_all_passed {
        1.0
    } else {
        0.0
    };
    let test_count = row.entry.oracle_per_test_count.unwrap_or(1).max(1);

    let calibration = CalibrationExample {
        language,
        predicted_test_pass: vec![0.5; test_count],
        actual_test_pass: vec![actual_probability; test_count],
    };
    let oracle_key = row_key("oracle", &row.attempt_id, step);
    let oracle_bytes = bincode::serialize(&calibration)?;
    let oracle_readback = put_cf_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
        &oracle_key,
        &oracle_bytes,
    )?;
    let oracle_decoded: CalibrationExample = bincode::deserialize(&oracle_readback)?;
    ensure!(
        oracle_decoded == calibration,
        "oracle decoded readback mismatch for {}",
        row.attempt_id
    );

    let cert = TrainCertSummary {
        step,
        delta_omega: if row.entry.oracle_all_passed {
            0.95
        } else {
            0.85
        },
        delta_xi: 0.90,
        witness_offset: step,
        // #699: mejepa-train CLI currently writes certs as part of the
        // diagnostic-only training loop (#683). Keep this at 0 so
        // compute_train_health's gate fires DiagnosticCertificateOnlyNeutral
        // and confidence is not scaled by these pseudo-values. Wire to a
        // real predictor-update count once #683 closes.
        predictor_parameter_update_count: 0,
    };
    cert.validate()?;
    let cert_key = row_key("train-cert", &row.attempt_id, step);
    let cert_bytes = bincode::serialize(&cert)?;
    let cert_readback = put_cf_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
        &cert_key,
        &cert_bytes,
    )?;
    let cert_decoded: TrainCertSummary = bincode::deserialize(&cert_readback)?;
    ensure!(
        cert_decoded == cert,
        "train cert decoded readback mismatch for {}",
        row.attempt_id
    );

    Ok(RowWriteCounts {
        panel_rows: 1,
        dda_rows: 1,
        prediction_rows: 0,
        oracle_rows: 1,
        train_cert_rows: 1,
        readback_equal: true,
    })
}

fn validate_panel_record(record: &PersistedPanelRecord) -> Result<()> {
    ensure!(
        record.schema_version == PANEL_SCHEMA_VERSION,
        "unexpected panel schema version {}",
        record.schema_version
    );
    ensure!(
        record.panel_dim == PANEL_DIM,
        "panel dim {} != {PANEL_DIM}",
        record.panel_dim
    );
    ensure!(
        record.panel_values.len() == PANEL_DIM,
        "panel value count {} != {PANEL_DIM}",
        record.panel_values.len()
    );
    ensure!(
        record.panel_values.iter().all(|value| value.is_finite()),
        "panel contains non-finite values"
    );
    ensure!(
        record.active_content_embedder_forwards == ACTIVE_CONTENT_EMBEDDER_FORWARDS,
        "active content embedder count mismatch"
    );
    ensure!(
        record.dda_embedder_surfaces == DDA_EMBEDDER_SURFACES,
        "DDA embedder surface count mismatch"
    );
    ensure!(
        record.logical_dda_signals_per_panel == LOGICAL_DDA_SIGNALS_PER_PANEL,
        "DDA logical signal budget mismatch"
    );
    let mut hasher = Sha256::new();
    for value in &record.panel_values {
        hasher.update(value.to_le_bytes());
    }
    let actual = hex::encode(hasher.finalize());
    ensure!(
        actual == record.panel_hash,
        "panel hash mismatch: {} != {}",
        actual,
        record.panel_hash
    );
    Ok(())
}

fn dda_signals_for_attempt(attempt_id: &str) -> Result<DdaSignals> {
    let seed = sha256_labeled("dda", attempt_id.as_bytes());
    let mut per_embedder_cosine = Vec::with_capacity(DDA_EMBEDDER_SURFACES);
    for idx in 0..DDA_EMBEDDER_SURFACES {
        per_embedder_cosine.push(0.80 + unit_from_seed(&seed, idx) * 0.19);
    }
    let pairwise_count = DDA_EMBEDDER_SURFACES * (DDA_EMBEDDER_SURFACES - 1) / 2;
    let mut pairwise_cosine_upper = Vec::with_capacity(pairwise_count);
    let mut pairwise_mi_upper = Vec::with_capacity(pairwise_count);
    let mut blind_spot_z_scores = Vec::with_capacity(pairwise_count);
    for idx in 0..pairwise_count {
        pairwise_cosine_upper.push(-0.20 + unit_from_seed(&seed, idx + 101) * 1.10);
        pairwise_mi_upper.push(unit_from_seed(&seed, idx + 401) * 0.25);
        blind_spot_z_scores.push(-2.0 + unit_from_seed(&seed, idx + 701) * 4.0);
    }
    Ok(DdaSignals::try_new(DdaSignals {
        per_embedder_cosine,
        pairwise_cosine_upper,
        pairwise_mi_upper,
        blind_spot_z_scores,
    })?)
}

fn test_ids(row: &SelectedRow) -> Vec<TestId> {
    let count = row.entry.oracle_per_test_count.unwrap_or(1).max(1);
    (0..count)
        .map(|idx| TestId(format!("{}::test::{idx:04}", row.attempt_id)))
        .collect()
}

fn problem_statement(row: &SelectedRow, split: TrainSplitArg) -> String {
    format!(
        "Phase G corpus orchestrator row. split={} repo={} task={} category={} oracle_exception={}",
        split.as_str(),
        row.entry.repo,
        row.entry.task_id,
        row.entry.category,
        row.entry.oracle_exception.as_deref().unwrap_or("none")
    )
}

fn parse_language(value: &str) -> Result<Language> {
    match value {
        "python" => Ok(Language::Python),
        "rust" => Ok(Language::Rust),
        "java_script" | "javascript" => Ok(Language::Javascript),
        "type_script" | "typescript" => Ok(Language::Typescript),
        "go" => Ok(Language::Go),
        "java" => Ok(Language::Java),
        "c" => Ok(Language::C),
        "cpp" | "c_plus_plus" => Ok(Language::Cpp),
        "c_sharp" | "csharp" => Ok(Language::CSharp),
        "ruby" => Ok(Language::Ruby),
        "php" => Ok(Language::Php),
        other => Err(anyhow!("unsupported corpus language {other:?}")),
    }
}

fn language_slug(language: Language) -> &'static str {
    match language {
        Language::Rust => "rust",
        Language::Python => "python",
        Language::Javascript => "javascript",
        Language::Typescript => "typescript",
        Language::Go => "go",
        Language::Java => "java",
        Language::C => "c",
        Language::Cpp => "cpp",
        Language::CSharp => "csharp",
        Language::Ruby => "ruby",
        Language::Php => "php",
    }
}

fn parse_sha256(value: &str) -> Result<[u8; 32]> {
    let normalized = normalize_sha256_string(value)?;
    let bytes = hex::decode(&normalized)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn normalize_sha256_string(value: &str) -> Result<String> {
    let raw = value.strip_prefix("sha256:").unwrap_or(value);
    ensure!(
        raw.len() == 64 && raw.bytes().all(|b| b.is_ascii_hexdigit()),
        "expected sha256 hex digest, got {value:?}"
    );
    Ok(raw.to_ascii_lowercase())
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn sha256_labeled(label: &str, bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    hasher.finalize().into()
}

fn first_16(bytes: [u8; 32]) -> [u8; 16] {
    let mut out = [0u8; 16];
    out.copy_from_slice(&bytes[..16]);
    out
}

fn unit_from_seed(seed: &[u8; 32], idx: usize) -> f32 {
    let a = seed[idx % 32] as u32;
    let b = seed[(idx.wrapping_mul(7).wrapping_add(13)) % 32] as u32;
    ((a << 8) | b) as f32 / 65_535.0
}

fn stable_path_id(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("corpus")
        .to_string()
}

fn row_key(kind: &str, attempt_id: &str, step: u64) -> Vec<u8> {
    let mut key = Vec::new();
    key.extend_from_slice(b"phase-g-corpus-train-v1/");
    key.extend_from_slice(kind.as_bytes());
    key.extend_from_slice(b"/");
    key.extend_from_slice(&step.to_be_bytes());
    key.extend_from_slice(b"/");
    key.extend_from_slice(attempt_id.as_bytes());
    key
}

fn put_cf_readback(db: &DB, cf_name: &str, key: &[u8], value: &[u8]) -> Result<Vec<u8>> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| anyhow!("missing RocksDB column family {cf_name}"))?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, value, &opts)?;
    let readback = db
        .get_cf(cf, key)?
        .ok_or_else(|| anyhow!("missing readback row in {cf_name}"))?;
    ensure!(
        readback.as_slice() == value,
        "readback byte mismatch in {cf_name}"
    );
    Ok(readback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpointed_train_stop_resume_writes_only_training_artifacts() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let corpus = temp.path().join("python-corpus");
        write_checkpointed_test_corpus(&corpus)?;
        let db_path = temp.path().join("infer-rocksdb");
        let checkpoint_dir = temp.path().join("checkpoints");
        let eval_export_dir = temp.path().join("exports");
        let quarantine_config = temp.path().join("empty-quarantine.toml");

        let first = run_checkpointed_python_train(MejepaCheckpointedTrainArgs {
            corpus: corpus.clone(),
            db_path: db_path.clone(),
            checkpoint_dir: checkpoint_dir.clone(),
            eval_export_dir: eval_export_dir.clone(),
            resume: None,
            stop_after: Some(TrainSplitArg::Train),
            max_corpus_entries: 4,
            max_tasks_per_split: None,
            quarantine_config: quarantine_config.clone(),
        })?;
        assert!(first.readback_equal);
        assert_eq!(first.completed_splits, vec![TrainSplitArg::Train]);
        assert!(first.eval_report.is_none());
        assert_eq!(
            first
                .split_outputs
                .get(&TrainSplitArg::Train)
                .map(|output| output.prediction_count),
            Some(0)
        );
        let resume_manifest = first
            .next_resume_manifest
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("missing resume manifest"))?;
        assert!(resume_manifest.exists());
        assert!(PathBuf::from(&first.checkpoints[0].checkpoint_path).exists());

        let resumed = run_checkpointed_python_train(MejepaCheckpointedTrainArgs {
            corpus,
            db_path: db_path.clone(),
            checkpoint_dir,
            eval_export_dir: eval_export_dir.clone(),
            resume: Some(resume_manifest.clone()),
            stop_after: None,
            max_corpus_entries: 4,
            max_tasks_per_split: None,
            quarantine_config,
        })?;
        assert!(resumed.readback_equal);
        assert_eq!(
            resumed.resumed_from_manifest,
            Some(resume_manifest.display().to_string())
        );
        assert_eq!(resumed.resumed_from_step, Some(0));
        assert_eq!(
            resumed.completed_splits,
            vec![TrainSplitArg::Calibration, TrainSplitArg::Holdout]
        );
        assert!(resumed.eval_report.is_none());
        assert!(resumed
            .split_outputs
            .values()
            .all(|output| output.prediction_count == 0));
        assert_eq!(
            resumed.source_of_truth["eval_report_write_policy"],
            "disabled: checkpointed-train does not emit ship-gate/countable EvalReport rows"
        );

        let db = open_infer_rocksdb(&db_path)?;
        assert_eq!(
            count_cf(&db, context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)?,
            4
        );
        assert_eq!(
            count_cf(&db, context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS)?,
            0
        );
        assert_eq!(
            count_cf(&db, context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)?,
            0
        );
        let oracle_cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)
            .ok_or_else(|| anyhow!("missing oracle verdict CF"))?;
        let mut oracle_rows = 0usize;
        for item in db.iterator_cf(oracle_cf, rocksdb::IteratorMode::Start) {
            let (_key, value) = item?;
            let decoded: CalibrationExample = bincode::deserialize(&value)?;
            assert_eq!(decoded.predicted_test_pass, vec![0.5]);
            assert_eq!(
                decoded.predicted_test_pass.len(),
                decoded.actual_test_pass.len()
            );
            oracle_rows += 1;
        }
        assert_eq!(oracle_rows, 4);
        let eval_exports_present = eval_export_dir.exists()
            && fs::read_dir(&eval_export_dir)?
                .next()
                .transpose()?
                .is_some();
        assert!(!eval_exports_present);
        Ok(())
    }

    fn write_checkpointed_test_corpus(root: &Path) -> Result<()> {
        fs::create_dir_all(root.join("mutations"))?;
        let rows = [
            (
                "train",
                "known_good",
                true,
                "train_known_good",
                "diff --git a/app.py b/app.py\n+print('train pass')\n",
            ),
            (
                "calibration",
                "compile_error",
                false,
                "calibration_compile_error",
                "diff --git a/app.py b/app.py\n+print(undefined_name)\n",
            ),
            (
                "holdout",
                "known_good",
                true,
                "holdout_known_good",
                "diff --git a/app.py b/app.py\n+print('holdout pass')\n",
            ),
            (
                "holdout",
                "compile_error",
                false,
                "holdout_compile_error",
                "diff --git a/app.py b/app.py\n+print(undefined_name)\n",
            ),
        ];
        let mut entries = Vec::new();
        for (bucket, category, oracle_all_passed, task_id, patch_text) in rows {
            let patch_path = format!("mutations/{task_id}.patch");
            fs::write(root.join(&patch_path), patch_text)?;
            entries.push(json!({
                "bucket": bucket,
                "category": category,
                "language": "python",
                "mutation_note": format!("{task_id} synthetic regression row"),
                "oracle_all_passed": oracle_all_passed,
                "oracle_exception": null,
                "oracle_per_test_count": 1,
                "oracle_verdict_sha256": format!("sha256:{}", hex::encode(Sha256::digest(format!("{task_id}:{oracle_all_passed}").as_bytes()))),
                "patch_path": patch_path,
                "patch_sha256": format!("sha256:{}", hex::encode(Sha256::digest(patch_text.as_bytes()))),
                "repo": "synthetic/python",
                "task_id": task_id
            }));
        }
        let index = json!({
            "schema_version": 1,
            "corpus_version": "checkpointed-train-regression",
            "corpus_sha256": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "entries": entries
        });
        fs::write(root.join("index.json"), serde_json::to_vec_pretty(&index)?)?;
        Ok(())
    }
}
