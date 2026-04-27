//! `context-graph-cli backfill-e14` — backfill E14 BGE-M3 Dense vectors.
//!
//! Iterates every fingerprint in the teleological store; for any record whose
//! `e14_bge_m3_dense` field is empty (pre-Phase-A or pre-BGE-M3 record),
//! invokes the native `BgeM3DenseModel` on the original content stored under
//! `CF_CONTENT` and writes the updated fingerprint back.
//!
//! # Operation modes
//! - `--dry-run`: iterate and report how many fingerprints need E14 backfill —
//!   no model load, no writes.
//! - Live mode: loads the model from `--models-dir/bge-m3-dense/` and backfills.
//!
//! # Resumability
//! The tool is idempotent: on re-run it skips fingerprints whose E14 field is
//! already populated. If interrupted mid-run, re-invocation picks up where it
//! left off without double-writing.
//!
//! # FAIL FAST
//! Any RocksDB error, any missing content entry, any embedding error aborts
//! the tool with a non-zero exit code. No partial writes — each fingerprint
//! update goes through the store's atomic `update()` path.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Args;
use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::fingerprint::{TeleologicalFingerprint, E14_DIM};
use context_graph_embeddings::config::GpuConfig;
use context_graph_embeddings::models::DefaultModelFactory;
use context_graph_embeddings::traits::{EmbeddingModel, ModelFactory, SingleModelConfig};
use context_graph_embeddings::types::{ModelId, ModelInput};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use tracing::{info, warn};

/// CLI arguments for `backfill-e14`.
#[derive(Args, Debug)]
pub struct BackfillE14Args {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Directory holding model snapshots (expects `bge-m3-dense/` subdir).
    #[arg(long, default_value = "./models")]
    pub models_dir: PathBuf,

    /// Report what would be done without calling the model or writing anything.
    #[arg(long)]
    pub dry_run: bool,

    /// Cap on total fingerprints processed (default: no cap).
    #[arg(long)]
    pub limit: Option<usize>,

    /// Upper bound on fingerprints fetched per `list_fingerprints_unbiased`
    /// call — keeps memory bounded even on very large databases.
    #[arg(long, default_value_t = 2000)]
    pub scan_batch: usize,

    /// Log every N backfilled updates for progress tracking.
    #[arg(long, default_value_t = 100)]
    pub log_every: usize,
}

/// Run the backfill. Returns exit code:
/// - 0 on full success (dry-run or live)
/// - 1 on any error
pub async fn run(args: BackfillE14Args) -> i32 {
    match run_inner(args).await {
        Ok(summary) => {
            info!(
                target: "context_graph_cli::backfill_e14",
                total_seen = summary.total_seen,
                already_populated = summary.already_populated,
                backfilled = summary.backfilled,
                skipped_missing_content = summary.skipped_missing_content,
                duration_ms = summary.duration_ms,
                "backfill-e14 complete"
            );
            eprintln!(
                "backfill-e14: {} seen, {} already had E14, {} backfilled, {} skipped (missing content), {}ms",
                summary.total_seen,
                summary.already_populated,
                summary.backfilled,
                summary.skipped_missing_content,
                summary.duration_ms
            );
            0
        }
        Err(e) => {
            eprintln!("backfill-e14 FAILED: {:#}", e);
            1
        }
    }
}

/// Summary returned by `run_inner`. Public within the crate for testing.
#[derive(Debug, Default)]
pub struct BackfillSummary {
    pub total_seen: usize,
    pub already_populated: usize,
    pub backfilled: usize,
    pub skipped_missing_content: usize,
    pub duration_ms: u128,
}

async fn run_inner(args: BackfillE14Args) -> Result<BackfillSummary> {
    let start = Instant::now();
    let mut summary = BackfillSummary::default();

    if !args.storage.exists() {
        anyhow::bail!(
            "Storage directory does not exist: {}",
            args.storage.display()
        );
    }

    // Open the store read/write. The store signature is
    // `open_with_config(path, config)`.
    let store_cfg = TeleologicalStoreConfig::default();
    let store = Arc::new(
        RocksDbTeleologicalStore::open_with_config(&args.storage, store_cfg)
            .context("opening RocksDbTeleologicalStore for backfill")?,
    );

    info!(
        target: "context_graph_cli::backfill_e14",
        storage = %args.storage.display(),
        dry_run = args.dry_run,
        "starting E14 backfill scan"
    );

    // Load the model only in live mode.
    let model: Option<Box<dyn EmbeddingModel>> = if args.dry_run {
        None
    } else {
        let factory = DefaultModelFactory::new(args.models_dir.clone(), GpuConfig::default());
        let m = factory
            .create_model(ModelId::BgeM3Dense, &SingleModelConfig::default())
            .context("creating BgeM3Dense model (check ./models/bge-m3-dense/ snapshot)")?;
        m.load()
            .await
            .context("loading BgeM3Dense weights (check tokenizer.json + model.safetensors)")?;
        info!(
            target: "context_graph_cli::backfill_e14",
            "BgeM3Dense model loaded"
        );
        Some(m)
    };

    // Fetch fingerprints in one unbiased batch — the store's
    // `list_fingerprints_unbiased` honours the `limit` argument and skips
    // soft-deleted rows. For >scan_batch databases, the operator can re-run
    // with progressively higher `--limit` values once the first run catches up.
    let store_ref: &dyn TeleologicalMemoryStore = store.as_ref();
    let fingerprints = store_ref
        .list_fingerprints_unbiased(args.scan_batch)
        .await
        .context("list_fingerprints_unbiased failed")?;

    for fp in fingerprints.into_iter() {
        if let Some(cap) = args.limit {
            if summary.total_seen >= cap {
                break;
            }
        }
        summary.total_seen += 1;

        if !fp.semantic.e14_bge_m3_dense.is_empty() {
            summary.already_populated += 1;
            continue;
        }

        if args.dry_run {
            summary.backfilled += 1; // Would be backfilled.
            continue;
        }

        let id = fp.id;
        let content = match store_ref.get_content(id).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                summary.skipped_missing_content += 1;
                warn!(
                    target: "context_graph_cli::backfill_e14",
                    memory_id = %id,
                    "skipping: no content row in CF_CONTENT"
                );
                continue;
            }
            Err(e) => {
                summary.skipped_missing_content += 1;
                warn!(
                    target: "context_graph_cli::backfill_e14",
                    memory_id = %id,
                    error = %e,
                    "skipping: get_content failed"
                );
                continue;
            }
        };

        // `model` is always `Some` in live mode (see construction above); the
        // `ok_or_else` keeps the invariant explicit without a panic path.
        let loaded_model = model.as_ref().ok_or_else(|| {
            anyhow::anyhow!("internal invariant violated: model missing in live mode")
        })?;
        let embedding = loaded_model
            .embed(&ModelInput::Text {
                content,
                instruction: None,
            })
            .await
            .with_context(|| format!("embedding E14 for {}", id))?;

        if embedding.vector.len() != E14_DIM {
            anyhow::bail!(
                "BgeM3Dense returned {}-D vector for {}; expected {}",
                embedding.vector.len(),
                id,
                E14_DIM
            );
        }

        let mut updated: TeleologicalFingerprint = fp;
        updated.semantic.e14_bge_m3_dense = embedding.vector;
        let ok = store_ref
            .update(updated)
            .await
            .with_context(|| format!("updating fingerprint {} with E14 vector", id))?;
        if !ok {
            warn!(
                target: "context_graph_cli::backfill_e14",
                memory_id = %id,
                "update returned false; fingerprint disappeared mid-run"
            );
            continue;
        }
        summary.backfilled += 1;

        if args.log_every > 0 && summary.backfilled % args.log_every == 0 {
            info!(
                target: "context_graph_cli::backfill_e14",
                backfilled = summary.backfilled,
                already_populated = summary.already_populated,
                "progress"
            );
        }
    }

    summary.duration_ms = start.elapsed().as_millis();
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backfill_args_parsing() {
        use clap::Parser;

        #[derive(clap::Parser)]
        struct Cli {
            #[command(flatten)]
            args: BackfillE14Args,
        }

        let cli = Cli::try_parse_from([
            "prog",
            "--storage",
            "/tmp/db",
            "--dry-run",
            "--limit",
            "100",
        ])
        .expect("parse");
        assert!(cli.args.dry_run);
        assert_eq!(cli.args.limit, Some(100));
        assert_eq!(cli.args.storage, PathBuf::from("/tmp/db"));
        assert_eq!(cli.args.models_dir, PathBuf::from("./models"));
        assert_eq!(cli.args.scan_batch, 2000);
        assert_eq!(cli.args.log_every, 100);
    }
}
