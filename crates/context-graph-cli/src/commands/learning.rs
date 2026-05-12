//! CLI inspection and JSONL export tools for `CF_LEARNING_EVENTS` and
//! `CF_LEARNER_TRAINING_DATASETS`.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand};
use context_graph_core::learner::sha256_json;
use context_graph_core::learner_training::{
    learning_event_feature_schema, learning_event_feature_vector, LearnerTrainingDataset,
};
use context_graph_core::learning::{
    LearningEvent, LearningOutcome, LearningOutcomeLabel, LearningStateSnapshot,
};
use context_graph_core::training::NUM_CROSS_CORRELATIONS;
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Subcommand, Debug)]
pub enum LearningCommands {
    /// Count rows in CF_LEARNING_EVENTS.
    Count(LearningStorageArgs),
    /// List rows in CF_LEARNING_EVENTS.
    List(LearningListArgs),
    /// Fetch one learning event by UUID.
    Get(LearningGetArgs),
    /// Export learning events as JSONL tensor rows.
    ExportEventsJsonl(LearningExportEventsJsonlArgs),
    /// Export persisted learner-training matrix rows as JSONL.
    ExportDatasetJsonl(LearningExportDatasetJsonlArgs),
    /// Store real LearningEvent rows from JSONL and read them back.
    RecordEventsJsonl(LearningRecordEventsJsonlArgs),
    /// Store a deterministic synthetic learning event and read it back.
    RecordSynthetic(LearningRecordSyntheticArgs),
}

#[derive(Args, Debug)]
pub struct LearningStorageArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,
}

#[derive(Args, Debug)]
pub struct LearningListArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Max rows to return.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,

    /// Rows to skip.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
}

#[derive(Args, Debug)]
pub struct LearningGetArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Learning event UUID.
    #[arg(long)]
    pub event_id: Uuid,

    /// Include query/context/response text.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub include_text: bool,
}

#[derive(Args, Debug)]
pub struct LearningExportEventsJsonlArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Output JSONL path. The parent directory must already exist.
    #[arg(long)]
    pub out: PathBuf,

    /// Max rows to export.
    #[arg(long)]
    pub limit: Option<usize>,

    /// Rows to skip.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,

    /// Include query/context/response text needed by learner DPO pair mining.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub include_text: bool,

    /// Allow overwriting an existing output file.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Args, Debug)]
pub struct LearningExportDatasetJsonlArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Output JSONL path. The parent directory must already exist.
    #[arg(long)]
    pub out: PathBuf,

    /// Export one learner-training dataset UUID. When omitted, exports all
    /// datasets currently stored in CF_LEARNER_TRAINING_DATASETS.
    #[arg(long)]
    pub dataset_id: Option<Uuid>,

    /// Max row records to export.
    #[arg(long)]
    pub limit: Option<usize>,

    /// Row records to skip.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,

    /// Allow overwriting an existing output file.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Args, Debug)]
pub struct LearningRecordEventsJsonlArgs {
    /// Path to the RocksDB data directory. Created by RocksDB when missing.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Input JSONL where each row is one LearningEvent payload.
    #[arg(long)]
    pub input: PathBuf,

    /// Delete existing learning events before importing rows.
    #[arg(long)]
    pub clear_existing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LearningEventJsonRow {
    event_id: Option<Uuid>,
    #[serde(default)]
    memory_ids: Vec<Uuid>,
    session_id: Option<String>,
    response_id: Option<String>,
    task_id: Option<String>,
    query: String,
    retrieved_context: String,
    assistant_response: String,
    before: LearningStateJsonRow,
    after: LearningStateJsonRow,
    outcome: LearningOutcomeJsonRow,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LearningStateJsonRow {
    topic_profile: [f32; NUM_EMBEDDERS],
    cross_correlations: Vec<f32>,
    retrieval_rank: Option<u32>,
    embedder_scores: Option<[f32; NUM_EMBEDDERS]>,
    #[serde(default)]
    contradiction_pressure: f32,
    #[serde(default)]
    integration_confidence: f32,
    #[serde(default)]
    recurrence_count: u32,
    #[serde(default)]
    stability_score: f32,
    domain: Option<String>,
    #[serde(default)]
    successful_transfer_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LearningOutcomeJsonRow {
    label: String,
    utility_delta: f32,
    #[serde(default)]
    correction_required: bool,
    #[serde(default)]
    reuse_observed: bool,
}

#[derive(Args, Debug)]
pub struct LearningRecordSyntheticArgs {
    /// Path to the RocksDB data directory. Created by RocksDB when missing.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Optional deterministic event UUID.
    #[arg(long)]
    pub event_id: Option<Uuid>,

    /// Delete existing learning events before writing the synthetic row.
    #[arg(long)]
    pub clear_existing: bool,

    /// Outcome label for the deterministic synthetic row.
    #[arg(
        long,
        default_value = "useful",
        value_parser = ["useful", "neutral", "harmful", "no_learning"]
    )]
    pub outcome_label: String,

    /// Signed utility delta in [-1, 1].
    #[arg(long, default_value_t = 0.4)]
    pub utility_delta: f32,
}

pub async fn handle_learning_command(action: LearningCommands) -> i32 {
    match run(action).await {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
            );
            0
        }
        Err(e) => {
            eprintln!("learning command FAILED: {:#}", e);
            1
        }
    }
}

async fn run(action: LearningCommands) -> Result<serde_json::Value> {
    match action {
        LearningCommands::Count(args) => {
            let store = open_store(&args.storage)?;
            let count = store.count_learning_events().await?;
            Ok(json!({
                "source_of_truth": source_of_truth(),
                "count": count,
            }))
        }
        LearningCommands::List(args) => {
            let store = open_store(&args.storage)?;
            let ids = store.list_learning_event_ids().await?;
            let total = ids.len();
            let page: Vec<_> = ids.into_iter().skip(args.offset).take(args.limit).collect();
            let mut events = Vec::with_capacity(page.len());
            for id in page {
                let event = store
                    .get_learning_event(id)
                    .await?
                    .with_context(|| format!("listed event {id} was not readable"))?;
                events.push(json!({
                    "event_id": id.to_string(),
                    "shape": event_shape(&event),
                    "features": feature_summary(&event),
                }));
            }
            Ok(json!({
                "source_of_truth": source_of_truth(),
                "total": total,
                "offset": args.offset,
                "limit": args.limit,
                "returned": events.len(),
                "events": events,
            }))
        }
        LearningCommands::Get(args) => {
            let store = open_store(&args.storage)?;
            let event = store.get_learning_event(args.event_id).await?;
            Ok(match event {
                Some(event) => render_event(&event, args.include_text),
                None => json!({
                    "source_of_truth": source_of_truth(),
                    "event_id": args.event_id.to_string(),
                    "found": false,
                }),
            })
        }
        LearningCommands::ExportEventsJsonl(args) => {
            let store = open_store(&args.storage)?;
            export_events_jsonl(&store, &args).await
        }
        LearningCommands::ExportDatasetJsonl(args) => {
            let store = open_store(&args.storage)?;
            export_dataset_jsonl(&store, &args).await
        }
        LearningCommands::RecordEventsJsonl(args) => {
            let store = open_store(&args.storage)?;
            record_events_jsonl(&store, &args).await
        }
        LearningCommands::RecordSynthetic(args) => {
            let store = open_store(&args.storage)?;
            let cleared = if args.clear_existing {
                store.clear_all_learning_events().await?
            } else {
                0
            };
            let event = synthetic_event(
                args.event_id.unwrap_or_else(Uuid::new_v4),
                parse_outcome_label(&args.outcome_label)?,
                args.utility_delta,
            )?;
            let before_count = store.count_learning_events().await?;
            store.store_learning_event(&event).await?;
            let after_count = store.count_learning_events().await?;
            let readback = store
                .get_learning_event(event.event_id)
                .await?
                .with_context(|| "synthetic event missing after write")?;
            Ok(json!({
                "source_of_truth": source_of_truth(),
                "cleared_before_write": cleared,
                "before_count": before_count,
                "after_count": after_count,
                "stored_event_id": event.event_id.to_string(),
                "readback": render_event(&readback, true),
            }))
        }
    }
}

fn open_store(storage: &Path) -> Result<RocksDbTeleologicalStore> {
    let storage = context_graph_paths::require_under_data_root(storage, "learning.storage")
        .map_err(|err| anyhow!(err.to_string()))?;
    RocksDbTeleologicalStore::open_with_config(&storage, TeleologicalStoreConfig::default())
        .with_context(|| format!("opening RocksDbTeleologicalStore at {}", storage.display()))
}

async fn export_events_jsonl(
    store: &RocksDbTeleologicalStore,
    args: &LearningExportEventsJsonlArgs,
) -> Result<serde_json::Value> {
    let out = context_graph_paths::require_under_data_root(&args.out, "learning.export.out")
        .map_err(|err| anyhow!(err.to_string()))?;
    let tmp_path = prepare_output_path(&out, args.overwrite)?;
    let ids = store
        .list_learning_event_ids()
        .await
        .context("listing CF_LEARNING_EVENTS ids")?;
    let total_available = ids.len();
    let selected = ids
        .into_iter()
        .skip(args.offset)
        .take(args.limit.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();

    let write_result: Result<usize> = async {
        let file = File::create(&tmp_path)
            .with_context(|| format!("creating temp output {}", tmp_path.display()))?;
        let mut writer = BufWriter::new(file);
        let schema = learning_event_feature_schema();
        for event_id in &selected {
            let event = store
                .get_learning_event(*event_id)
                .await?
                .with_context(|| format!("listed event {event_id} was not readable"))?;
            let tensor = learning_event_feature_vector(&event)
                .with_context(|| format!("featurizing learning event {event_id}"))?;
            let row = render_event_tensor_row(&event, &schema, tensor, args.include_text)?;
            serde_json::to_writer(&mut writer, &row)
                .with_context(|| format!("serializing learning event {event_id} to JSONL"))?;
            writer.write_all(b"\n")?;
        }
        writer.flush()?;
        Ok(selected.len())
    }
    .await;

    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }
    rename_or_cleanup(&tmp_path, &out)?;
    let bytes_written = std::fs::metadata(&out)?.len();
    Ok(json!({
        "status": "ok",
        "source_of_truth": source_of_truth(),
        "export": {
            "format": "jsonl",
            "path": out,
            "record_kind": "learning_event_tensor",
            "include_text": args.include_text,
            "feature_schema_len": learning_event_feature_schema().len(),
        },
        "total_available": total_available,
        "offset": args.offset,
        "limit": args.limit,
        "records_written": selected.len(),
        "bytes_written": bytes_written,
    }))
}

async fn export_dataset_jsonl(
    store: &RocksDbTeleologicalStore,
    args: &LearningExportDatasetJsonlArgs,
) -> Result<serde_json::Value> {
    let out = context_graph_paths::require_under_data_root(&args.out, "learning.export.out")
        .map_err(|err| anyhow!(err.to_string()))?;
    let tmp_path = prepare_output_path(&out, args.overwrite)?;
    let datasets = load_training_datasets(store, args.dataset_id).await?;
    let total_rows: usize = datasets.iter().map(|dataset| dataset.rows.len()).sum();

    let write_result: Result<usize> = (|| {
        let file = File::create(&tmp_path)
            .with_context(|| format!("creating temp output {}", tmp_path.display()))?;
        let mut writer = BufWriter::new(file);
        let mut seen = 0usize;
        let mut written = 0usize;
        for dataset in &datasets {
            let cols = dataset.cols_len as usize;
            for row_idx in 0..dataset.rows.len() {
                if seen < args.offset {
                    seen += 1;
                    continue;
                }
                if args.limit.is_some_and(|limit| written >= limit) {
                    writer.flush()?;
                    return Ok(written);
                }
                let start = row_idx * cols;
                let end = start + cols;
                let row = json!({
                    "record_kind": "learner_training_row",
                    "schema_version": context_graph_core::learner_training::LEARNER_TRAINING_DATASET_VERSION,
                    "dataset_id": dataset.dataset_id.to_string(),
                    "dataset_created_at": dataset.created_at,
                    "task": dataset.task.as_str(),
                    "row_index": row_idx,
                    "row": dataset.rows[row_idx],
                    "feature_schema": dataset.feature_schema,
                    "features": dataset.row_major[start..end].to_vec(),
                    "feature_tensor_len": cols,
                    "label_schema": dataset.label_schema,
                    "labels": {
                        "label_scalar": dataset.rows[row_idx].label_scalar,
                        "label_class": dataset.rows[row_idx].label_class,
                    },
                    "source_counts": dataset.source_counts,
                    "filters": dataset.filters,
                    "row_major_sha256": dataset.row_major_sha256,
                    "provenance_manifest_sha256": dataset.provenance_manifest_sha256,
                });
                serde_json::to_writer(&mut writer, &row).with_context(|| {
                    format!(
                        "serializing learner training dataset {} row {} to JSONL",
                        dataset.dataset_id, row_idx
                    )
                })?;
                writer.write_all(b"\n")?;
                seen += 1;
                written += 1;
            }
        }
        writer.flush()?;
        Ok(written)
    })();

    let records_written = match write_result {
        Ok(n) => n,
        Err(err) => {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(err);
        }
    };
    rename_or_cleanup(&tmp_path, &out)?;
    let bytes_written = std::fs::metadata(&out)?.len();
    Ok(json!({
        "status": "ok",
        "source_of_truth": {
            "backend": "rocksdb",
            "column_family": "learner_training_datasets",
            "format": "version_byte + bincode",
        },
        "export": {
            "format": "jsonl",
            "path": out,
            "record_kind": "learner_training_row",
        },
        "dataset_filter": args.dataset_id.map(|id| id.to_string()),
        "datasets_read": datasets.len(),
        "total_rows": total_rows,
        "offset": args.offset,
        "limit": args.limit,
        "records_written": records_written,
        "bytes_written": bytes_written,
    }))
}

async fn record_events_jsonl(
    store: &RocksDbTeleologicalStore,
    args: &LearningRecordEventsJsonlArgs,
) -> Result<serde_json::Value> {
    if !args.input.exists() {
        return Err(anyhow!(
            "input JSONL does not exist: {}",
            args.input.display()
        ));
    }
    let input = std::fs::read_to_string(&args.input)
        .with_context(|| format!("reading {}", args.input.display()))?;
    let before_count = store.count_learning_events().await?;
    let cleared = if args.clear_existing {
        store.clear_all_learning_events().await?
    } else {
        0
    };
    let after_clear_count = store.count_learning_events().await?;
    let mut imported = Vec::new();

    for (idx, line) in input.lines().enumerate() {
        let line_no = idx + 1;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: LearningEventJsonRow = serde_json::from_str(line)
            .with_context(|| format!("parsing {} line {line_no}", args.input.display()))?;
        let event = parsed
            .into_event()
            .with_context(|| format!("building LearningEvent from line {line_no}"))?;
        let event_id = event.event_id;
        store
            .store_learning_event(&event)
            .await
            .with_context(|| format!("storing LearningEvent {event_id} from line {line_no}"))?;
        let readback = store
            .get_learning_event(event_id)
            .await
            .with_context(|| format!("reading back LearningEvent {event_id}"))?
            .with_context(|| format!("LearningEvent {event_id} missing after write"))?;
        imported.push(json!({
            "line": line_no,
            "event_id": event_id.to_string(),
            "shape": event_shape(&readback),
            "features": feature_summary(&readback),
            "outcome": {
                "label": outcome_label_str(readback.outcome.label),
                "utility_delta": readback.outcome.utility_delta,
            },
        }));
    }

    if imported.is_empty() {
        return Err(anyhow!(
            "input JSONL contained zero LearningEvent rows: {}",
            args.input.display()
        ));
    }
    let after_count = store.count_learning_events().await?;
    Ok(json!({
        "status": "stored",
        "source_of_truth": source_of_truth(),
        "input": args.input,
        "before_count": before_count,
        "cleared_before_write": cleared,
        "after_clear_count": after_clear_count,
        "after_count": after_count,
        "imported": imported.len(),
        "events": imported,
    }))
}

async fn load_training_datasets(
    store: &RocksDbTeleologicalStore,
    dataset_id: Option<Uuid>,
) -> Result<Vec<LearnerTrainingDataset>> {
    if let Some(id) = dataset_id {
        let dataset = store
            .get_learner_training_dataset(id)
            .await
            .with_context(|| format!("reading learner training dataset {id}"))?
            .with_context(|| format!("learner training dataset {id} not found"))?;
        return Ok(vec![dataset]);
    }

    let ids = store
        .list_learner_training_dataset_ids()
        .await
        .context("listing CF_LEARNER_TRAINING_DATASETS ids")?;
    let mut datasets = Vec::with_capacity(ids.len());
    for id in ids {
        let dataset = store
            .get_learner_training_dataset(id)
            .await
            .with_context(|| format!("reading learner training dataset {id}"))?
            .with_context(|| format!("listed learner training dataset {id} was not readable"))?;
        datasets.push(dataset);
    }
    Ok(datasets)
}

fn prepare_output_path(out: &Path, overwrite: bool) -> Result<PathBuf> {
    let parent = out
        .parent()
        .ok_or_else(|| anyhow!("--out path has no parent directory: {}", out.display()))?;
    if !parent.as_os_str().is_empty() && !parent.exists() {
        return Err(anyhow!(
            "parent directory does not exist: {} (create it before export)",
            parent.display()
        ));
    }
    if out.exists() && !overwrite {
        return Err(anyhow!(
            "output file already exists: {} (pass --overwrite to replace it)",
            out.display()
        ));
    }
    let mut tmp = out.to_path_buf();
    match out.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => tmp.set_extension(format!("{ext}.tmp")),
        None => tmp.set_extension("tmp"),
    };
    if tmp.exists() {
        std::fs::remove_file(&tmp)
            .with_context(|| format!("removing stale temp output {}", tmp.display()))?;
    }
    Ok(tmp)
}

fn rename_or_cleanup(tmp_path: &PathBuf, out: &PathBuf) -> Result<()> {
    if let Err(err) = std::fs::rename(tmp_path, out) {
        let _ = std::fs::remove_file(tmp_path);
        return Err(err).with_context(|| {
            format!(
                "renaming temp output {} to {}",
                tmp_path.display(),
                out.display()
            )
        });
    }
    Ok(())
}

impl LearningEventJsonRow {
    fn into_event(self) -> Result<LearningEvent> {
        let event_id = self.event_id.unwrap_or_else(Uuid::new_v4);
        let before = self.before.into_state();
        let after = self.after.into_state();
        let outcome = self.outcome.into_outcome()?;
        Ok(LearningEvent::new(
            event_id,
            self.memory_ids,
            self.session_id,
            self.response_id,
            self.task_id,
            self.query,
            self.retrieved_context,
            self.assistant_response,
            before,
            after,
            outcome,
        )?)
    }
}

impl LearningStateJsonRow {
    fn into_state(self) -> LearningStateSnapshot {
        LearningStateSnapshot {
            topic_profile: self.topic_profile,
            cross_correlations: self.cross_correlations,
            retrieval_rank: self.retrieval_rank,
            embedder_scores: self.embedder_scores.unwrap_or([0.0; NUM_EMBEDDERS]),
            contradiction_pressure: self.contradiction_pressure,
            integration_confidence: self.integration_confidence,
            recurrence_count: self.recurrence_count,
            stability_score: self.stability_score,
            domain: self.domain,
            successful_transfer_count: self.successful_transfer_count,
        }
    }
}

impl LearningOutcomeJsonRow {
    fn into_outcome(self) -> Result<LearningOutcome> {
        Ok(LearningOutcome {
            label: parse_outcome_label(&self.label)?,
            utility_delta: self.utility_delta,
            correction_required: self.correction_required,
            reuse_observed: self.reuse_observed,
        })
    }
}

fn synthetic_event(
    event_id: Uuid,
    outcome_label: LearningOutcomeLabel,
    utility_delta: f32,
) -> Result<LearningEvent> {
    let mut before_profile = [0.0f32; NUM_EMBEDDERS];
    let mut after_profile = [0.0f32; NUM_EMBEDDERS];
    let mut before_scores = [0.0f32; NUM_EMBEDDERS];
    let mut after_scores = [0.0f32; NUM_EMBEDDERS];
    for idx in 0..NUM_EMBEDDERS {
        before_profile[idx] = (idx as f32 + 1.0) / 100.0;
        after_profile[idx] = before_profile[idx] + 0.1;
        before_scores[idx] = 0.2;
        after_scores[idx] = 0.2 + (idx as f32 / (NUM_EMBEDDERS - 1) as f32) * 0.7;
    }

    let before = LearningStateSnapshot {
        topic_profile: before_profile,
        cross_correlations: vec![0.05; NUM_CROSS_CORRELATIONS],
        retrieval_rank: Some(10),
        embedder_scores: before_scores,
        contradiction_pressure: 0.2,
        integration_confidence: 0.4,
        recurrence_count: 1,
        stability_score: 0.4,
        domain: Some("docs".into()),
        successful_transfer_count: 0,
    };
    let after = LearningStateSnapshot {
        topic_profile: after_profile,
        cross_correlations: vec![0.15; NUM_CROSS_CORRELATIONS],
        retrieval_rank: Some(2),
        embedder_scores: after_scores,
        contradiction_pressure: 0.1,
        integration_confidence: 0.8,
        recurrence_count: 8,
        stability_score: 0.9,
        domain: Some("code".into()),
        successful_transfer_count: 3,
    };
    let outcome = LearningOutcome {
        label: outcome_label,
        utility_delta,
        correction_required: utility_delta < 0.0,
        reuse_observed: utility_delta > 0.0,
    };
    Ok(LearningEvent::new(
        event_id,
        vec![Uuid::new_v4()],
        Some("synthetic-learning-fsv".into()),
        Some("synthetic-response".into()),
        Some("synthetic-task".into()),
        "How does the parser state move after adding a deterministic feature?".into(),
        "Synthetic context with known before and after profiles.".into(),
        format!(
            "Synthetic response with outcome={} utility_delta={utility_delta}.",
            outcome_label_str(outcome_label)
        ),
        before,
        after,
        outcome,
    )?)
}

fn source_of_truth() -> serde_json::Value {
    json!({
        "backend": "rocksdb",
        "column_family": "learning_events",
        "format": "version_byte + bincode",
    })
}

fn event_shape(event: &LearningEvent) -> serde_json::Value {
    json!({
        "before_topic_profile_len": event.before.topic_profile.len(),
        "after_topic_profile_len": event.after.topic_profile.len(),
        "before_cross_correlations_len": event.before.cross_correlations.len(),
        "after_cross_correlations_len": event.after.cross_correlations.len(),
        "delta_e_vector_len": event.features.delta_e_vector.len(),
        "signals": event.signals.iter().map(|s| json!({
            "signal_id": s.signal_id.as_str(),
            "vector_len": s.vector.len(),
            "scalar": s.scalar,
        })).collect::<Vec<_>>(),
    })
}

fn feature_summary(event: &LearningEvent) -> serde_json::Value {
    json!({
        "delta_e_scalar": event.features.delta_e_scalar,
        "retrieval_rank_shift": event.features.retrieval_rank_shift,
        "surprise_score": event.features.surprise_score,
        "coherence_delta": event.features.coherence_delta,
        "consolidation_readiness": event.features.consolidation_readiness,
        "transfer_score": event.features.transfer_score,
        "multi_utl_score": event.features.multi_utl_score,
    })
}

fn render_event_tensor_row(
    event: &LearningEvent,
    feature_schema: &[String],
    feature_tensor: Vec<f32>,
    include_text: bool,
) -> Result<serde_json::Value> {
    let provenance_sha256 = sha256_json(event)?;
    let mut value = json!({
        "record_kind": "learning_event_tensor",
        "schema_version": context_graph_core::learning::LEARNING_EVENT_VERSION,
        "source_of_truth": source_of_truth(),
        "event_id": event.event_id.to_string(),
        "created_at": event.created_at,
        "memory_ids": event.memory_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "session_id": event.session_id,
        "response_id": event.response_id,
        "task_id": event.task_id,
        "before": {
            "topic_profile": event.before.topic_profile,
            "cross_correlations": event.before.cross_correlations,
            "retrieval_rank": event.before.retrieval_rank,
            "embedder_scores": event.before.embedder_scores,
            "contradiction_pressure": event.before.contradiction_pressure,
            "integration_confidence": event.before.integration_confidence,
            "recurrence_count": event.before.recurrence_count,
            "stability_score": event.before.stability_score,
            "domain": event.before.domain,
            "successful_transfer_count": event.before.successful_transfer_count,
        },
        "after": {
            "topic_profile": event.after.topic_profile,
            "cross_correlations": event.after.cross_correlations,
            "retrieval_rank": event.after.retrieval_rank,
            "embedder_scores": event.after.embedder_scores,
            "contradiction_pressure": event.after.contradiction_pressure,
            "integration_confidence": event.after.integration_confidence,
            "recurrence_count": event.after.recurrence_count,
            "stability_score": event.after.stability_score,
            "domain": event.after.domain,
            "successful_transfer_count": event.after.successful_transfer_count,
        },
        "outcome": {
            "label": outcome_label_str(event.outcome.label),
            "utility_delta": event.outcome.utility_delta,
            "correction_required": event.outcome.correction_required,
            "reuse_observed": event.outcome.reuse_observed,
        },
        "feature_summary": feature_summary(event),
        "feature_schema": feature_schema,
        "feature_tensor": feature_tensor,
        "feature_tensor_len": feature_schema.len(),
        "signals": event.signals.iter().map(|s| json!({
            "signal_id": s.signal_id.as_str(),
            "vector": s.vector,
            "scalar": s.scalar,
            "label": s.label,
        })).collect::<Vec<_>>(),
        "provenance_sha256": provenance_sha256,
    });

    if include_text {
        value["query"] = json!(event.query);
        value["retrieved_context"] = json!(event.retrieved_context);
        value["assistant_response"] = json!(event.assistant_response);
    }
    Ok(value)
}

fn render_event(event: &LearningEvent, include_text: bool) -> serde_json::Value {
    let mut value = json!({
        "source_of_truth": source_of_truth(),
        "event_id": event.event_id.to_string(),
        "found": true,
        "created_at": event.created_at,
        "session_id": event.session_id,
        "response_id": event.response_id,
        "task_id": event.task_id,
        "memory_ids": event.memory_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "before": {
            "topic_profile": event.before.topic_profile,
            "cross_correlations_len": event.before.cross_correlations.len(),
            "retrieval_rank": event.before.retrieval_rank,
            "embedder_scores": event.before.embedder_scores,
            "contradiction_pressure": event.before.contradiction_pressure,
            "integration_confidence": event.before.integration_confidence,
            "recurrence_count": event.before.recurrence_count,
            "stability_score": event.before.stability_score,
            "domain": event.before.domain,
            "successful_transfer_count": event.before.successful_transfer_count,
        },
        "after": {
            "topic_profile": event.after.topic_profile,
            "cross_correlations_len": event.after.cross_correlations.len(),
            "retrieval_rank": event.after.retrieval_rank,
            "embedder_scores": event.after.embedder_scores,
            "contradiction_pressure": event.after.contradiction_pressure,
            "integration_confidence": event.after.integration_confidence,
            "recurrence_count": event.after.recurrence_count,
            "stability_score": event.after.stability_score,
            "domain": event.after.domain,
            "successful_transfer_count": event.after.successful_transfer_count,
        },
        "outcome": {
            "label": format!("{:?}", event.outcome.label),
            "utility_delta": event.outcome.utility_delta,
            "correction_required": event.outcome.correction_required,
            "reuse_observed": event.outcome.reuse_observed,
        },
        "shape": event_shape(event),
        "features": feature_summary(event),
        "signals": event.signals.iter().map(|s| json!({
            "signal_id": s.signal_id.as_str(),
            "vector": s.vector,
            "scalar": s.scalar,
            "label": s.label,
        })).collect::<Vec<_>>(),
    });

    if include_text {
        value["query"] = json!(event.query);
        value["retrieved_context"] = json!(event.retrieved_context);
        value["assistant_response"] = json!(event.assistant_response);
    }
    value
}

fn outcome_label_str(label: LearningOutcomeLabel) -> &'static str {
    match label {
        LearningOutcomeLabel::Useful => "useful",
        LearningOutcomeLabel::Neutral => "neutral",
        LearningOutcomeLabel::Harmful => "harmful",
        LearningOutcomeLabel::NoLearning => "no_learning",
    }
}

fn parse_outcome_label(value: &str) -> Result<LearningOutcomeLabel> {
    match value {
        "useful" => Ok(LearningOutcomeLabel::Useful),
        "neutral" => Ok(LearningOutcomeLabel::Neutral),
        "harmful" => Ok(LearningOutcomeLabel::Harmful),
        "no_learning" => Ok(LearningOutcomeLabel::NoLearning),
        other => Err(anyhow!("unknown outcome label: {other}")),
    }
}
