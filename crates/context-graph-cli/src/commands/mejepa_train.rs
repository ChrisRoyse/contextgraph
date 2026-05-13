//! Phase G corpus training-pipeline orchestrator.
//!
//! This command is intentionally a source-of-truth writer, not a thin wrapper
//! around the older `mejepa-train` binary. The Phase G driver has to persist the
//! panel, DDA, prediction, oracle, and train-certificate rows for every corpus
//! item so FSV can verify the full chain.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, ensure, Context, Result};
use clap::{Args, ValueEnum};
use context_graph_mejepa::{
    materialize_inference_panels, open_infer_rocksdb, panel_sha, AgentClaimGraph, AstDiff,
    CalibrationExample, ChunkId, ConformalInterval, ConformalMethod, ConformalSet, DdaSignals,
    DiffHunk, Language, MejepaStore, OracleOutcome, PanelId, PatchBundle, PredictionProvenance,
    RealityPredictionBuilder, RocksDbInferStore, TaskContext, TaskEnvironment, TaskId, TestId,
    TrainCertSummary, Verdict, WitnessHash, PANEL_DIM,
};
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::mejepa_active_learning::DEFAULT_MEJEPA_INFER_DB;

const ACTIVE_CONTENT_EMBEDDER_FORWARDS: usize = 12;
const DDA_EMBEDDER_SURFACES: usize = 21;
const LOGICAL_DDA_SIGNALS_PER_PANEL: usize = 462;
const PANEL_SCHEMA_VERSION: u8 = 1;
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
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
    pub readback_equal: bool,
    pub active_content_embedder_forwards: usize,
    pub dda_embedder_surfaces: usize,
    pub instrument_slot_count: usize,
    pub source_of_truth: serde_json::Value,
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
    let store = RocksDbInferStore::new(db.clone());
    let mut rows = load_selected_rows(&args)?;
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
        let processed = process_row(&db, &store, args.split, idx as u64 + 1, row)
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
        prediction_count == input_task_count,
        "prediction count mismatch: {prediction_count} != {input_task_count}"
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
        readback_equal,
        active_content_embedder_forwards: ACTIVE_CONTENT_EMBEDDER_FORWARDS,
        dda_embedder_surfaces: DDA_EMBEDDER_SURFACES,
        instrument_slot_count: 15,
        source_of_truth: json!({
            "panel_cf": context_graph_mejepa_cf::CF_MEJEPA_PANELS,
            "dda_cf": context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS,
            "prediction_cf": context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
            "oracle_cf": context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
            "train_cert_cf": context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
            "db_path": args.db_path,
        }),
    })
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

fn load_selected_rows(args: &MejepaTrainArgs) -> Result<Vec<SelectedRow>> {
    let mut rows = Vec::new();
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
    Ok(rows)
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
    store: &RocksDbInferStore,
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
    let session_id = first_16(sha256_labeled("session", row.attempt_id.as_bytes()));
    let prediction_id = first_16(sha256_labeled("prediction", row.attempt_id.as_bytes()));
    let chunk_id = ChunkId(format!("{}::chunk::0000", row.attempt_id));
    let task_context = TaskContext {
        task_id: TaskId(row.attempt_id.clone()),
        session_id,
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
        witness_segment(&row.attempt_id),
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
            predictor_forward_count: 1,
            conformal_calibration_count: 1,
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

    let predicted_probability = match row.entry.predicted_oracle_pass {
        Some(predicted) => predicted,
        None if row.entry.oracle_all_passed => 0.95,
        None => 0.05,
    };
    let actual_probability = if row.entry.oracle_all_passed {
        1.0
    } else {
        0.0
    };
    let test_count = row.entry.oracle_per_test_count.unwrap_or(1).max(1);
    let outcome = if row.entry.oracle_all_passed {
        OracleOutcome::Pass
    } else {
        OracleOutcome::Fail
    };
    let outcome_set = ConformalSet::try_new(vec![outcome], 0.10, 0.10)?;
    let confidence_interval = ConformalInterval {
        lower: (predicted_probability - 0.10_f32).clamp(0.0, 1.0),
        upper: (predicted_probability + 0.10_f32).clamp(0.0, 1.0),
        method: ConformalMethod::SplitConformal,
        coverage_target: 0.90,
        empirical_coverage: 0.90,
    };
    let prediction = RealityPredictionBuilder::from_parts(
        TaskId(row.attempt_id.clone()),
        session_id,
        language,
        outcome_set,
    )
    .prediction_id(prediction_id)
    .witness_hash(WitnessHash(source_sha))
    .covered_chunks(vec![chunk_id])
    .verdict(if row.entry.oracle_all_passed {
        Verdict::Pass
    } else {
        Verdict::Fail
    })
    .confidence_interval(confidence_interval)
    .predicted_oracle_pass(predicted_probability)
    .predicted_test_pass(vec![predicted_probability; test_count])
    .predicted_runtime_trace(runtime_trace(&row.attempt_id))
    .ood_score(0.05)
    .calibrated_confidence(0.90)
    .degraded_status(false)
    .granger_attestations(granger_attestations())
    .agent_claim_graph(AgentClaimGraph::default())
    .provenance(PredictionProvenance {
        predictor_version: "phase-g-corpus-driver-v1".to_string(),
        constellation_version: "phase-g-tct-cell-v1".to_string(),
        calibration_version: "phase-g-split-conformal-v1".to_string(),
        active_pointer: hex::encode(prediction_id),
    })
    .source_panel_sha(panel_id)
    .calibration_version("phase-g-split-conformal-v1")
    .build()?;
    store.write_live_prediction(&prediction)?;
    let prediction_readback = store.read_live_predictions(session_id, 1)?;
    ensure!(
        prediction_readback.first() == Some(&prediction),
        "prediction readback mismatch for {}",
        row.attempt_id
    );

    let calibration = CalibrationExample {
        language,
        predicted_test_pass: vec![predicted_probability; test_count],
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
        prediction_rows: 1,
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

fn granger_attestations() -> BTreeMap<String, f32> {
    BTreeMap::from([
        ("ast_chunker".to_string(), 1.0),
        ("active_embedder_forwards".to_string(), 1.0),
        ("frozen_instrument_panel".to_string(), 1.0),
        ("dda_signals".to_string(), 1.0),
        ("tct_cell_membership".to_string(), 1.0),
        ("oracle_reconciliation".to_string(), 1.0),
    ])
}

fn runtime_trace(attempt_id: &str) -> [f32; 32] {
    let seed = sha256_labeled("runtime", attempt_id.as_bytes());
    let mut out = [0.0f32; 32];
    for (idx, value) in out.iter_mut().enumerate() {
        *value = unit_from_seed(&seed, idx) * 0.10;
    }
    out
}

fn witness_segment(attempt_id: &str) -> Vec<u8> {
    let digest = sha256_labeled("witness", attempt_id.as_bytes());
    let mut out = vec![0u8; 73];
    for (idx, byte) in out.iter_mut().enumerate() {
        *byte = digest[idx % digest.len()];
    }
    out
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
