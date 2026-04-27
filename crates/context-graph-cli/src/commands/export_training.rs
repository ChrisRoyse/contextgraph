//! `context-graph-cli export training-corpus --format parquet --out <path>`
//!
//! Phase 6 — File-format export (Parquet only).
//!
//! Reads every row from `CF_TRAINING_RECORDS` via
//! [`RocksDbTeleologicalStore`] and writes it to a Parquet file using the
//! minimal `(memory_id: Utf8, record_bytes: Binary)` schema recommended in
//! `docs/TRAINING_DATA_EXPORT_PLAN.md` §8 and `tasks/phase4_summary.md`.
//!
//! Downstream consumers decode `record_bytes` back into a `TrainingRecord`
//! via [`context_graph_storage::teleological::decode_training_record`].
//! The bytes written here are **byte-for-byte identical** to what lives in
//! RocksDB — we call the production
//! [`context_graph_storage::teleological::encode_training_record`] helper
//! instead of re-serialising locally.
//!
//! # FAIL FAST
//!
//! - Missing parent directory for `--out` → `anyhow::Error`.
//! - Any RocksDB error from the store propagates unmodified.
//! - Any Parquet writer error aborts the export; partial files are removed.
//! - Version-byte mismatches on read are surfaced via the decoder, not
//!   silently dropped.

use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use arrow_array::{ArrayRef, BinaryArray, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use clap::Args;
use context_graph_storage::teleological::{
    encode_training_record, RocksDbTeleologicalStore, TeleologicalStoreConfig,
};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use tracing::{info, warn};

/// CLI arguments for `export training-corpus`.
#[derive(Args, Debug)]
pub struct ExportTrainingArgs {
    /// Export target. Currently only `training-corpus` is supported.
    #[arg(long, default_value = "training-corpus", value_parser = ["training-corpus"])]
    pub kind: String,

    /// Output format. Currently only `parquet`.
    #[arg(long, default_value = "parquet", value_parser = ["parquet"])]
    pub format: String,

    /// Absolute path of the output file (Parquet). The parent directory must
    /// already exist.
    #[arg(long)]
    pub out: PathBuf,

    /// Path to the RocksDB data directory (the same one
    /// `CF_TRAINING_RECORDS` lives in).
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Batch size for reads and row groups. Larger batches reduce Parquet
    /// overhead but hold more records in RAM at once.
    #[arg(long, default_value_t = 500)]
    pub batch_size: usize,

    /// Optional cap on total records exported. Default: no cap.
    #[arg(long)]
    pub limit: Option<usize>,
}

/// Summary of a successful export run.
#[derive(Debug, Clone)]
pub struct ExportSummary {
    /// Number of training records successfully written.
    pub total_records: usize,
    /// On-disk size of the produced Parquet file in bytes.
    pub bytes_written: u64,
    /// Number of Parquet row groups produced (one per batch).
    pub row_groups: usize,
    /// Wall-clock time for the full export.
    pub elapsed_ms: u64,
}

/// Execute the export. Returns an [`ExportSummary`] on success.
///
/// On any failure, the partially-written Parquet file (if any) is removed so
/// a retried run does not start from a corrupted state.
pub async fn run(args: ExportTrainingArgs) -> Result<ExportSummary> {
    // --- Guardrails ---
    if args.kind != "training-corpus" {
        return Err(anyhow!(
            "unsupported export kind: {} (only 'training-corpus' is implemented in Phase 6)",
            args.kind
        ));
    }
    if args.format != "parquet" {
        return Err(anyhow!(
            "unsupported export format: {} (only 'parquet' is implemented in Phase 6)",
            args.format
        ));
    }
    if args.batch_size == 0 {
        return Err(anyhow!("--batch-size must be > 0"));
    }

    let parent = args
        .out
        .parent()
        .ok_or_else(|| anyhow!("--out path has no parent directory: {}", args.out.display()))?;
    if !parent.as_os_str().is_empty() && !parent.exists() {
        return Err(anyhow!(
            "parent directory does not exist: {} (create it before running export)",
            parent.display()
        ));
    }

    if !args.storage.exists() {
        return Err(anyhow!(
            "--storage directory does not exist: {}",
            args.storage.display()
        ));
    }

    let start = Instant::now();

    // --- Open the production store (no mocks) ---
    let store = RocksDbTeleologicalStore::open_with_config(
        &args.storage,
        TeleologicalStoreConfig::default(),
    )
    .with_context(|| {
        format!(
            "failed to open RocksDbTeleologicalStore at {}",
            args.storage.display()
        )
    })?;

    // --- Enumerate IDs, apply limit ---
    let mut all_ids = store
        .list_training_record_ids()
        .await
        .context("failed to list training record ids from CF_TRAINING_RECORDS")?;
    if let Some(cap) = args.limit {
        if all_ids.len() > cap {
            all_ids.truncate(cap);
        }
    }
    info!(
        storage = %args.storage.display(),
        out = %args.out.display(),
        total_candidates = all_ids.len(),
        batch_size = args.batch_size,
        "beginning Parquet export of training corpus"
    );

    // --- Build Arrow schema: (memory_id Utf8, record_bytes Binary) ---
    let schema = Arc::new(Schema::new(vec![
        Field::new("memory_id", DataType::Utf8, false),
        Field::new("record_bytes", DataType::Binary, false),
    ]));

    // --- Open Parquet writer ---
    // Zstd + SNAPPY fallback are both in the default-features=false feature
    // set we explicitly enabled; zstd gives better compression ratios for the
    // repetitive bincode payloads.
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .build();

    let file = File::create(&args.out)
        .with_context(|| format!("failed to create output file: {}", args.out.display()))?;

    let mut writer = ArrowWriter::try_new(file, Arc::clone(&schema), Some(props))
        .with_context(|| format!("failed to construct ArrowWriter for {}", args.out.display()))?;

    // --- Stream batches ---
    let mut total_records: usize = 0;
    let mut row_groups: usize = 0;

    // On any error below, wipe the half-written file to fail fast.
    let out_path = args.out.clone();
    let export_result: Result<()> = async {
        for chunk in all_ids.chunks(args.batch_size) {
            let records = store
                .multi_get_training_records(chunk)
                .await
                .context("multi_get_training_records failed during export")?;

            // Build the two columns. We only emit rows whose record decoded
            // successfully (Some(_)); missing or undecodable rows are logged
            // and skipped so the downstream consumer never sees orphan UUIDs.
            let mut id_strings: Vec<String> = Vec::with_capacity(chunk.len());
            let mut payloads: Vec<Vec<u8>> = Vec::with_capacity(chunk.len());

            for (idx, slot) in records.iter().enumerate() {
                let id = chunk[idx];
                match slot {
                    Some(record) => {
                        let bytes = encode_training_record(record)
                            .with_context(|| format!("encode_training_record failed for {}", id))?;
                        id_strings.push(id.to_string());
                        payloads.push(bytes);
                    }
                    None => {
                        warn!(id = %id, "training record missing or undecodable; skipping");
                    }
                }
            }

            if id_strings.is_empty() {
                continue;
            }

            let id_array: ArrayRef = Arc::new(StringArray::from(id_strings));
            let payload_refs: Vec<&[u8]> = payloads.iter().map(|v| v.as_slice()).collect();
            let payload_array: ArrayRef = Arc::new(BinaryArray::from_vec(payload_refs));

            let batch = RecordBatch::try_new(Arc::clone(&schema), vec![id_array, payload_array])
                .context("RecordBatch::try_new failed")?;

            total_records += batch.num_rows();
            writer.write(&batch).context("ArrowWriter::write failed")?;

            // Force one row group per batch so FSV can assert the
            // batch_size => row_groups contract.
            writer
                .flush()
                .context("ArrowWriter::flush failed between row groups")?;
            row_groups += 1;
        }

        // finalise footer
        writer.close().context("ArrowWriter::close failed")?;
        Ok(())
    }
    .await;

    if let Err(e) = export_result {
        // Best-effort cleanup of a half-written file.
        let _ = std::fs::remove_file(&out_path);
        return Err(e);
    }

    let bytes_written = std::fs::metadata(&args.out)
        .with_context(|| {
            format!(
                "failed to stat output file after writer close: {}",
                args.out.display()
            )
        })?
        .len();

    let elapsed_ms = start.elapsed().as_millis() as u64;
    info!(
        total_records,
        row_groups, bytes_written, elapsed_ms, "Parquet export finished"
    );

    Ok(ExportSummary {
        total_records,
        bytes_written,
        row_groups,
        elapsed_ms,
    })
}

/// Convert an [`ExportSummary`] into a process exit code (`0` success, `1`
/// on failure). The CLI entry point maps `Result<ExportSummary>` to this.
pub fn summary_to_exit_code(result: Result<ExportSummary>) -> i32 {
    match result {
        Ok(summary) => {
            info!(
                total_records = summary.total_records,
                bytes_written = summary.bytes_written,
                row_groups = summary.row_groups,
                elapsed_ms = summary.elapsed_ms,
                "export training-corpus: SUCCESS"
            );
            println!(
                "{{\"status\":\"ok\",\"total_records\":{},\"bytes_written\":{},\"row_groups\":{},\"elapsed_ms\":{}}}",
                summary.total_records,
                summary.bytes_written,
                summary.row_groups,
                summary.elapsed_ms,
            );
            0
        }
        Err(err) => {
            tracing::error!(error = %err, "export training-corpus: FAILED");
            eprintln!("error: {:#}", err);
            1
        }
    }
}
