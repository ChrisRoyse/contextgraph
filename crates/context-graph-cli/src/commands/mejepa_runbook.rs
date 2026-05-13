//! Operator runbook CLI commands wrapping ME-JEPA durable state.
//!
//! Each subcommand reads or writes only its declared Source of Truth and
//! refuses to proceed when the SoT is missing or malformed
//! (`MEJEPA_RUNBOOK_*` fail-closed error codes). The commands are thin
//! Rust wrappers over the same public APIs the MCP handlers use; they
//! deliberately do not go over JSON-RPC to keep operator tooling fast
//! and reproducible.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use chrono::NaiveDate;
use clap::{Args, Subcommand};
use context_graph_mejepa::eval::store::CF_MEJEPA_EVAL_REPORTS;
use context_graph_mejepa::eval::types::EvalReport;
use context_graph_mejepa::eval::{
    language_slug, required_active_python_ship_gate_cells, validate_active_python_ship_gate_report,
    ACTIVE_PYTHON_SHIP_GATE_GRID, ACTIVE_PYTHON_SHIP_GATE_NAME,
};
use context_graph_mejepa::heal::{
    AbcPromoter, HealRocksStore, PromotionGate, PromotionLockState, RollbackEvidence,
    WitnessChainAppender,
};
use context_graph_mejepa::{open_infer_rocksdb, RocksDbEvalStore};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteBatch};
use serde::Serialize;
use serde_json::{json, Value};

const DEFAULT_MEJEPA_INFER_DB: &str = "/mnt/d/contextgraph/storage/contextgraph-rocksdb";
const DEFAULT_MEJEPA_HEAL_DB: &str = "/mnt/d/contextgraph/storage/mejepa-heal";
const DEFAULT_SCHEDULER_STATE_ROOT: &str = "/mnt/d/contextgraph/state/schedulers";
const DEFAULT_WITNESS_CHAIN_PATH: &str =
    "/mnt/d/contextgraph/state/cgreality/mejepa-witness-chain.bin";
const DEFAULT_HYGIENE_ARCHIVE_ROOT: &str = "/mnt/d/contextgraph/storage/hygiene-archive";
const DEFAULT_PAUSE_STATE_PATH: &str =
    "/mnt/d/contextgraph/state/cgreality/predictions_paused_until.json";

#[derive(Args, Debug, Clone)]
pub struct PauseArgs {
    /// Pause-state file path (JSON, single key `paused_until_unix_ms`).
    #[arg(
        long,
        env = "CONTEXTGRAPH_MEJEPA_PAUSE_PATH",
        default_value = DEFAULT_PAUSE_STATE_PATH
    )]
    pub state_path: PathBuf,

    /// Pause duration in minutes (> 0). Predictions resume `now + duration`.
    #[arg(long)]
    pub duration_mins: u64,

    /// Operator reason persisted alongside the pause timestamp.
    #[arg(long, default_value = "manual pause")]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PauseState {
    pub paused_until_unix_ms: i64,
    pub set_at_unix_ms: i64,
    pub reason: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseOutput {
    pub state_path: PathBuf,
    pub paused_until_unix_ms: i64,
    pub set_at_unix_ms: i64,
    pub duration_mins: u64,
    pub reason: String,
    pub readback_equal: bool,
    pub source_of_truth: Value,
}

pub fn run_pause(args: PauseArgs) -> Result<PauseOutput> {
    validate_path_non_empty(&args.state_path, "pause-state-path")?;
    if args.duration_mins == 0 {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_PAUSE_DURATION_ZERO: --duration-mins must be > 0"
        ));
    }
    if args.duration_mins > 7 * 24 * 60 {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_PAUSE_DURATION_EXCEEDS_WEEK: --duration-mins must be <= 10080 (one week); got {}",
            args.duration_mins
        ));
    }
    if args.reason.trim().is_empty() {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_PAUSE_REASON_EMPTY: --reason must be non-empty"
        ));
    }
    if args.reason.chars().any(char::is_control) {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_PAUSE_REASON_INVALID: --reason must contain no control characters"
        ));
    }
    if let Some(parent) = args.state_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "MEJEPA_RUNBOOK_PAUSE_PARENT_CREATE_FAILED: {}",
                parent.display()
            )
        })?;
    }
    let now_ms = chrono::Utc::now().timestamp_millis();
    let duration_ms = args
        .duration_mins
        .checked_mul(60_000)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| anyhow!("MEJEPA_RUNBOOK_PAUSE_OVERFLOW: duration calculation overflowed"))?;
    let paused_until_ms = now_ms.checked_add(duration_ms).ok_or_else(|| {
        anyhow!("MEJEPA_RUNBOOK_PAUSE_OVERFLOW: paused_until calculation overflowed i64")
    })?;
    let state = PauseState {
        paused_until_unix_ms: paused_until_ms,
        set_at_unix_ms: now_ms,
        reason: args.reason.clone(),
        source: "context-graph-cli mejepa pause".to_string(),
    };
    let bytes = serde_json::to_vec_pretty(&state)
        .with_context(|| "MEJEPA_RUNBOOK_PAUSE_SERIALIZE_FAILED".to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&args.state_path)
        .with_context(|| {
            format!(
                "MEJEPA_RUNBOOK_PAUSE_WRITE_OPEN_FAILED: {}",
                args.state_path.display()
            )
        })?;
    std::io::Write::write_all(&mut file, &bytes).with_context(|| {
        format!(
            "MEJEPA_RUNBOOK_PAUSE_WRITE_FAILED: {}",
            args.state_path.display()
        )
    })?;
    file.sync_all().with_context(|| {
        format!(
            "MEJEPA_RUNBOOK_PAUSE_SYNC_FAILED: {}",
            args.state_path.display()
        )
    })?;
    drop(file);
    let readback_bytes = std::fs::read(&args.state_path).with_context(|| {
        format!(
            "MEJEPA_RUNBOOK_PAUSE_READBACK_FAILED: {}",
            args.state_path.display()
        )
    })?;
    if readback_bytes != bytes {
        return Err(anyhow!("MEJEPA_RUNBOOK_PAUSE_READBACK_BYTES_MISMATCH"));
    }
    let readback: PauseState = serde_json::from_slice(&readback_bytes)
        .with_context(|| "MEJEPA_RUNBOOK_PAUSE_READBACK_DESERIALIZE_FAILED".to_string())?;
    let readback_equal = readback == state;
    if !readback_equal {
        return Err(anyhow!("MEJEPA_RUNBOOK_PAUSE_READBACK_MISMATCH"));
    }
    Ok(PauseOutput {
        state_path: args.state_path.clone(),
        paused_until_unix_ms: state.paused_until_unix_ms,
        set_at_unix_ms: state.set_at_unix_ms,
        duration_mins: args.duration_mins,
        reason: args.reason,
        readback_equal,
        source_of_truth: json!({
            "kind": "file",
            "path": args.state_path,
            "writer": "context_graph_cli::commands::mejepa_runbook::run_pause",
            "consumer": "ME-JEPA prediction handlers read this on startup of each verify cycle",
        }),
    })
}

#[derive(Args, Debug, Clone)]
pub struct StorageVerifyArgs {
    /// Inference RocksDB path to integrity-check.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageVerifyOutput {
    pub db_path: PathBuf,
    pub paranoid_open_ok: bool,
    pub cf_count: usize,
    pub cf_row_counts: std::collections::BTreeMap<String, u64>,
    pub source_of_truth: Value,
}

pub fn run_storage_verify(args: StorageVerifyArgs) -> Result<StorageVerifyOutput> {
    validate_dir(&args.db_path, "storage-db-path")?;
    let db = open_infer_rocksdb(&args.db_path).with_context(|| {
        format!(
            "MEJEPA_RUNBOOK_STORAGE_VERIFY_OPEN_FAILED: {}",
            args.db_path.display()
        )
    })?;
    let mut counts = std::collections::BTreeMap::new();
    for cf_name in context_graph_mejepa_cf::INFER_CFS {
        let count = context_graph_mejepa::count_cf(db.as_ref(), cf_name)
            .with_context(|| format!("MEJEPA_RUNBOOK_STORAGE_VERIFY_CF_FAILED: {cf_name}"))?;
        counts.insert((*cf_name).to_string(), count);
    }
    Ok(StorageVerifyOutput {
        db_path: args.db_path.clone(),
        paranoid_open_ok: true,
        cf_count: counts.len(),
        cf_row_counts: counts,
        source_of_truth: json!({
            "writer": "rocksdb DB::open_cf_descriptors with set_paranoid_checks(true)",
            "verification": "count_cf walks every iterator key — surfaces SST corruption",
        }),
    })
}

#[derive(Subcommand, Debug, Clone)]
pub enum StorageCommands {
    /// Integrity-check the inference RocksDB.
    Verify(StorageVerifyArgs),
    /// Restore one column family from a source RocksDB snapshot into the target DB.
    #[command(name = "restore-cf")]
    RestoreCf(StorageRestoreCfArgs),
    /// Copy rows from one column family to another in the same target DB.
    #[command(name = "migrate-cf")]
    MigrateCf(StorageMigrateCfArgs),
}

#[derive(Args, Debug, Clone)]
pub struct StorageRestoreCfArgs {
    /// Target ME-JEPA RocksDB path to restore into.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Source RocksDB snapshot path containing the same column family.
    #[arg(long)]
    pub source: PathBuf,

    /// Column family name to restore.
    pub cf_name: String,
}

#[derive(Args, Debug, Clone)]
pub struct StorageMigrateCfArgs {
    /// Target ME-JEPA RocksDB path containing both column families.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Source column family name.
    pub old_cf_name: String,

    /// Destination column family name.
    pub new_cf_name: String,

    /// Inspect source/destination counts without writing.
    #[arg(long)]
    pub dry_run: bool,

    /// Permit replacing an already-populated destination CF.
    #[arg(long)]
    pub allow_overwrite: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageRestoreCfOutput {
    pub db_path: PathBuf,
    pub source_path: PathBuf,
    pub cf_name: String,
    pub source_rows: u64,
    pub source_value_bytes: u64,
    pub target_rows_before: u64,
    pub target_rows_after: u64,
    pub replaced_target_rows: u64,
    pub readback_equal: bool,
    pub reopened_readback_equal: bool,
    pub source_of_truth: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageMigrateCfOutput {
    pub db_path: PathBuf,
    pub old_cf_name: String,
    pub new_cf_name: String,
    pub dry_run: bool,
    pub allow_overwrite: bool,
    pub source_rows: u64,
    pub source_value_bytes: u64,
    pub target_rows_before: u64,
    pub target_rows_after: u64,
    pub migrated_rows: u64,
    pub readback_equal: bool,
    pub reopened_readback_equal: bool,
    pub source_of_truth: Value,
}

pub fn run_storage_command(action: StorageCommands) -> Result<Value> {
    match action {
        StorageCommands::Verify(args) => {
            serde_json::to_value(run_storage_verify(args)?).context("serialize storage verify")
        }
        StorageCommands::RestoreCf(args) => serde_json::to_value(run_storage_restore_cf(args)?)
            .context("serialize storage restore-cf"),
        StorageCommands::MigrateCf(args) => serde_json::to_value(run_storage_migrate_cf(args)?)
            .context("serialize storage migrate-cf"),
    }
}

pub fn run_storage_restore_cf(args: StorageRestoreCfArgs) -> Result<StorageRestoreCfOutput> {
    let cf_name = validate_hygiene_cf_name(&args.cf_name, "restore-cf")?;
    validate_dir(&args.db_path, "storage-db-path")?;
    validate_dir(&args.source, "storage-source-path")?;
    ensure_distinct_dirs(
        &args.db_path,
        &args.source,
        "MEJEPA_RUNBOOK_STORAGE_RESTORE_SAME_DB",
    )?;

    let source_db = open_hygiene_rocksdb_read_only(&args.source).with_context(|| {
        format!(
            "MEJEPA_RUNBOOK_STORAGE_RESTORE_SOURCE_OPEN_FAILED: {}",
            args.source.display()
        )
    })?;
    let source_rows = collect_cf_rows(&source_db, &cf_name)
        .with_context(|| format!("MEJEPA_RUNBOOK_STORAGE_RESTORE_SOURCE_READ_FAILED: {cf_name}"))?;
    if source_rows.is_empty() {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_STORAGE_RESTORE_SOURCE_EMPTY: cf_name={cf_name} source={}",
            args.source.display()
        ));
    }
    let source_value_bytes = rows_value_bytes(&source_rows);
    drop(source_db);

    let target_db = context_graph_mejepa_hygiene::open_hygiene_rocksdb(&args.db_path)
        .map_err(|err| anyhow!("MEJEPA_RUNBOOK_STORAGE_RESTORE_TARGET_OPEN_FAILED: {err}"))?;
    let target_rows_before = collect_cf_rows(target_db.as_ref(), &cf_name)
        .with_context(|| format!("MEJEPA_RUNBOOK_STORAGE_RESTORE_TARGET_READ_FAILED: {cf_name}"))?;
    replace_cf_rows(
        target_db.as_ref(),
        &cf_name,
        &target_rows_before,
        &source_rows,
    )
    .with_context(|| format!("MEJEPA_RUNBOOK_STORAGE_RESTORE_WRITE_FAILED: {cf_name}"))?;
    let target_rows_after = collect_cf_rows(target_db.as_ref(), &cf_name)
        .with_context(|| format!("MEJEPA_RUNBOOK_STORAGE_RESTORE_READBACK_FAILED: {cf_name}"))?;
    let readback_equal = target_rows_after == source_rows;
    if !readback_equal {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_STORAGE_RESTORE_READBACK_MISMATCH: cf_name={cf_name}"
        ));
    }
    drop(target_db);

    let reopened = open_hygiene_rocksdb_read_only(&args.db_path).with_context(|| {
        format!(
            "MEJEPA_RUNBOOK_STORAGE_RESTORE_REOPEN_FAILED: {}",
            args.db_path.display()
        )
    })?;
    let reopened_rows = collect_cf_rows(&reopened, &cf_name)
        .with_context(|| format!("MEJEPA_RUNBOOK_STORAGE_RESTORE_REOPEN_READ_FAILED: {cf_name}"))?;
    let reopened_readback_equal = reopened_rows == source_rows;
    if !reopened_readback_equal {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_STORAGE_RESTORE_REOPEN_MISMATCH: cf_name={cf_name}"
        ));
    }

    Ok(StorageRestoreCfOutput {
        db_path: args.db_path.clone(),
        source_path: args.source.clone(),
        cf_name: cf_name.clone(),
        source_rows: source_rows.len() as u64,
        source_value_bytes,
        target_rows_before: target_rows_before.len() as u64,
        target_rows_after: target_rows_after.len() as u64,
        replaced_target_rows: target_rows_before.len() as u64,
        readback_equal,
        reopened_readback_equal,
        source_of_truth: json!({
            "target_db_path": args.db_path,
            "source_db_path": args.source,
            "column_family": cf_name,
            "process": "source read-only DB iterator -> target WriteBatch delete+put -> sync write -> flush -> reopen readback",
        }),
    })
}

pub fn run_storage_migrate_cf(args: StorageMigrateCfArgs) -> Result<StorageMigrateCfOutput> {
    let old_cf_name = validate_hygiene_cf_name(&args.old_cf_name, "migrate-cf-old")?;
    let new_cf_name = validate_hygiene_cf_name(&args.new_cf_name, "migrate-cf-new")?;
    if old_cf_name == new_cf_name {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_STORAGE_MIGRATE_SAME_CF: old_cf_name and new_cf_name must differ"
        ));
    }
    validate_dir(&args.db_path, "storage-db-path")?;

    let db = context_graph_mejepa_hygiene::open_hygiene_rocksdb(&args.db_path)
        .map_err(|err| anyhow!("MEJEPA_RUNBOOK_STORAGE_MIGRATE_OPEN_FAILED: {err}"))?;
    let source_rows = collect_cf_rows(db.as_ref(), &old_cf_name).with_context(|| {
        format!("MEJEPA_RUNBOOK_STORAGE_MIGRATE_SOURCE_READ_FAILED: {old_cf_name}")
    })?;
    let target_rows_before = collect_cf_rows(db.as_ref(), &new_cf_name).with_context(|| {
        format!("MEJEPA_RUNBOOK_STORAGE_MIGRATE_TARGET_READ_FAILED: {new_cf_name}")
    })?;
    let source_value_bytes = rows_value_bytes(&source_rows);

    if args.dry_run {
        return Ok(StorageMigrateCfOutput {
            db_path: args.db_path.clone(),
            old_cf_name,
            new_cf_name,
            dry_run: true,
            allow_overwrite: args.allow_overwrite,
            source_rows: source_rows.len() as u64,
            source_value_bytes,
            target_rows_before: target_rows_before.len() as u64,
            target_rows_after: target_rows_before.len() as u64,
            migrated_rows: 0,
            readback_equal: true,
            reopened_readback_equal: true,
            source_of_truth: json!({
                "db_path": args.db_path,
                "old_column_family": args.old_cf_name,
                "new_column_family": args.new_cf_name,
                "process": "dry-run iterator read only; no WriteBatch executed",
            }),
        });
    }

    if !target_rows_before.is_empty() && !args.allow_overwrite {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_STORAGE_MIGRATE_TARGET_NOT_EMPTY: new_cf_name={new_cf_name} rows={}",
            target_rows_before.len()
        ));
    }

    replace_cf_rows(db.as_ref(), &new_cf_name, &target_rows_before, &source_rows)
        .with_context(|| format!("MEJEPA_RUNBOOK_STORAGE_MIGRATE_WRITE_FAILED: {new_cf_name}"))?;
    let target_rows_after = collect_cf_rows(db.as_ref(), &new_cf_name).with_context(|| {
        format!("MEJEPA_RUNBOOK_STORAGE_MIGRATE_READBACK_FAILED: {new_cf_name}")
    })?;
    let readback_equal = target_rows_after == source_rows;
    if !readback_equal {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_STORAGE_MIGRATE_READBACK_MISMATCH: new_cf_name={new_cf_name}"
        ));
    }
    drop(db);

    let reopened = open_hygiene_rocksdb_read_only(&args.db_path).with_context(|| {
        format!(
            "MEJEPA_RUNBOOK_STORAGE_MIGRATE_REOPEN_FAILED: {}",
            args.db_path.display()
        )
    })?;
    let reopened_rows = collect_cf_rows(&reopened, &new_cf_name).with_context(|| {
        format!("MEJEPA_RUNBOOK_STORAGE_MIGRATE_REOPEN_READ_FAILED: {new_cf_name}")
    })?;
    let reopened_readback_equal = reopened_rows == source_rows;
    if !reopened_readback_equal {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_STORAGE_MIGRATE_REOPEN_MISMATCH: new_cf_name={new_cf_name}"
        ));
    }

    Ok(StorageMigrateCfOutput {
        db_path: args.db_path.clone(),
        old_cf_name: old_cf_name.clone(),
        new_cf_name: new_cf_name.clone(),
        dry_run: false,
        allow_overwrite: args.allow_overwrite,
        source_rows: source_rows.len() as u64,
        source_value_bytes,
        target_rows_before: target_rows_before.len() as u64,
        target_rows_after: target_rows_after.len() as u64,
        migrated_rows: source_rows.len() as u64,
        readback_equal,
        reopened_readback_equal,
        source_of_truth: json!({
            "db_path": args.db_path,
            "old_column_family": old_cf_name,
            "new_column_family": new_cf_name,
            "transform": "identity bytes",
            "process": "source iterator -> destination WriteBatch put -> sync write -> flush -> reopen readback",
        }),
    })
}

#[derive(Args, Debug, Clone)]
pub struct SessionCleanupArgs {
    /// Inference RocksDB path containing CF_MEJEPA_LIVE_PREDICTIONS + CF_MEJEPA_SHIFT_WATERMARK.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Hygiene archive root used for quota readback.
    #[arg(
        long,
        env = "CONTEXTGRAPH_MEJEPA_HYGIENE_ARCHIVE_ROOT",
        default_value = DEFAULT_HYGIENE_ARCHIVE_ROOT
    )]
    pub archive_root: PathBuf,

    /// 32-character lowercase hexadecimal session id.
    #[arg(long)]
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCleanupOutput {
    pub session_id_hex: String,
    pub live_predictions_deleted: u64,
    pub shift_watermark_deleted: bool,
    pub deleted_live_prediction_bytes: u64,
    pub deleted_shift_watermark_bytes: u64,
    pub deleted_total_bytes: u64,
    pub quota_before_total_used_bytes: u64,
    pub quota_after_total_used_bytes: u64,
    pub quota_state_key_hex: String,
    pub gc_event_key_hex: String,
    pub quota_report_readback_equal: bool,
    pub gc_event_readback_equal: bool,
    pub readback_equal: bool,
    pub source_of_truth: Value,
}

pub fn run_session_cleanup(args: SessionCleanupArgs) -> Result<SessionCleanupOutput> {
    let session_id = parse_session_hex(&args.session_id)?;
    validate_dir(&args.db_path, "session-db-path")?;
    validate_path_non_empty(&args.archive_root, "archiveRoot")?;
    let db =
        context_graph_mejepa_hygiene::open_hygiene_rocksdb(&args.db_path).with_context(|| {
            format!(
                "MEJEPA_RUNBOOK_SESSION_CLEANUP_OPEN_FAILED: {}",
                args.db_path.display()
            )
        })?;
    let env = context_graph_mejepa_hygiene::HygieneEnv::try_new(
        context_graph_mejepa_hygiene::runtime_config(db.clone(), args.archive_root.clone())
            .map_err(|err| anyhow!("MEJEPA_RUNBOOK_SESSION_CLEANUP_QUOTA_ENV_FAILED: {err}"))?,
    )
    .map_err(|err| anyhow!("MEJEPA_RUNBOOK_SESSION_CLEANUP_QUOTA_ENV_FAILED: {err}"))?;
    let quota_before = context_graph_mejepa_hygiene::quota_status(&env)
        .map_err(|err| anyhow!("MEJEPA_RUNBOOK_SESSION_CLEANUP_QUOTA_BEFORE_FAILED: {err}"))?;

    let live_cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)
        .ok_or_else(|| {
            anyhow!("MEJEPA_RUNBOOK_SESSION_CLEANUP_LIVE_CF_MISSING: CF_MEJEPA_LIVE_PREDICTIONS")
        })?;
    let mut keys_to_delete = Vec::new();
    let mut deleted_live_prediction_bytes = 0u64;
    for item in db.iterator_cf(live_cf, rocksdb::IteratorMode::Start) {
        let (key, value) =
            item.with_context(|| "MEJEPA_RUNBOOK_SESSION_CLEANUP_ITER_FAILED".to_string())?;
        if key.starts_with(session_id.as_slice()) {
            deleted_live_prediction_bytes =
                deleted_live_prediction_bytes.saturating_add(value.len() as u64);
            keys_to_delete.push(key.to_vec());
        }
    }
    let mut write_opts = rocksdb::WriteOptions::default();
    write_opts.set_sync(true);
    let live_predictions_deleted = keys_to_delete.len() as u64;
    for key in &keys_to_delete {
        db.delete_cf_opt(live_cf, key, &write_opts)
            .with_context(|| {
                format!(
                    "MEJEPA_RUNBOOK_SESSION_CLEANUP_DELETE_FAILED: key_len={}",
                    key.len()
                )
            })?;
    }
    db.flush_cf(live_cf)
        .with_context(|| "MEJEPA_RUNBOOK_SESSION_CLEANUP_FLUSH_FAILED".to_string())?;

    let watermark_cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)
        .ok_or_else(|| {
            anyhow!(
                "MEJEPA_RUNBOOK_SESSION_CLEANUP_WATERMARK_CF_MISSING: CF_MEJEPA_SHIFT_WATERMARK"
            )
        })?;
    let watermark_existed = db
        .get_cf(watermark_cf, session_id)
        .with_context(|| "MEJEPA_RUNBOOK_SESSION_CLEANUP_WATERMARK_READ_FAILED".to_string())?
        .map(|value| value.len() as u64);
    let deleted_shift_watermark_bytes = watermark_existed.unwrap_or(0);
    let watermark_existed = watermark_existed.is_some();
    if watermark_existed {
        db.delete_cf_opt(watermark_cf, session_id, &write_opts)
            .with_context(|| {
                "MEJEPA_RUNBOOK_SESSION_CLEANUP_WATERMARK_DELETE_FAILED".to_string()
            })?;
        db.flush_cf(watermark_cf)
            .with_context(|| "MEJEPA_RUNBOOK_SESSION_CLEANUP_WATERMARK_FLUSH_FAILED".to_string())?;
    }

    // Readback: nothing matching `session_id` should remain in either CF.
    let mut residual_live = 0u64;
    for item in db.iterator_cf(live_cf, rocksdb::IteratorMode::Start) {
        let (key, _value) =
            item.with_context(|| "MEJEPA_RUNBOOK_SESSION_CLEANUP_READBACK_FAILED".to_string())?;
        if key.starts_with(session_id.as_slice()) {
            residual_live += 1;
        }
    }
    let residual_watermark = db
        .get_cf(watermark_cf, session_id)
        .with_context(|| "MEJEPA_RUNBOOK_SESSION_CLEANUP_READBACK_WATERMARK_FAILED".to_string())?
        .is_some();
    let readback_equal = residual_live == 0 && !residual_watermark;
    if !readback_equal {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_SESSION_CLEANUP_READBACK_MISMATCH: residual_live={residual_live} residual_watermark={residual_watermark}"
        ));
    }

    let quota_after = context_graph_mejepa_hygiene::quota_status(&env)
        .map_err(|err| anyhow!("MEJEPA_RUNBOOK_SESSION_CLEANUP_QUOTA_AFTER_FAILED: {err}"))?;
    let deleted_total_bytes =
        deleted_live_prediction_bytes.saturating_add(deleted_shift_watermark_bytes);
    let quota_delta = quota_before
        .total_used_bytes
        .checked_sub(quota_after.total_used_bytes)
        .ok_or_else(|| {
            anyhow!(
                "MEJEPA_RUNBOOK_SESSION_CLEANUP_QUOTA_INCREASED: before={} after={}",
                quota_before.total_used_bytes,
                quota_after.total_used_bytes
            )
        })?;
    if quota_delta != deleted_total_bytes {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_SESSION_CLEANUP_QUOTA_MISMATCH: deleted_total_bytes={deleted_total_bytes} quota_delta={quota_delta}"
        ));
    }

    let quota_report = context_graph_mejepa_hygiene::QuotaEvictionReport {
        before: quota_before.clone(),
        after: quota_after.clone(),
        evicted: Vec::new(),
    };
    let quota_report_bytes = serde_json::to_vec(&quota_report).with_context(|| {
        "MEJEPA_RUNBOOK_SESSION_CLEANUP_QUOTA_REPORT_SERIALIZE_FAILED".to_string()
    })?;
    put_cf_sync_readback(
        db.as_ref(),
        context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
        context_graph_mejepa_hygiene::QUOTA_LAST_REPORT_KEY,
        &quota_report_bytes,
        "MEJEPA_RUNBOOK_SESSION_CLEANUP_QUOTA_REPORT",
    )?;
    let quota_report_readback_equal = true;

    let occurred_unix_ms = chrono::Utc::now().timestamp_millis();
    let session_id_hex = args.session_id.to_ascii_lowercase();
    let gc_event_key = format!("session_cleanup:{occurred_unix_ms}:{session_id_hex}");
    let gc_event_key_bytes = gc_event_key.as_bytes();
    let gc_event = context_graph_mejepa_hygiene::GcEvent::SessionCleanup {
        session_id_hex: session_id_hex.clone(),
        occurred_unix_ms,
        live_predictions_deleted,
        shift_watermark_deleted: watermark_existed,
        deleted_live_prediction_bytes,
        deleted_shift_watermark_bytes,
        deleted_total_bytes,
        quota_category: context_graph_mejepa_hygiene::StorageCategory::ShiftLogSubscriberState,
        quota_before_total_used_bytes: quota_before.total_used_bytes,
        quota_after_total_used_bytes: quota_after.total_used_bytes,
        source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY.to_string(),
        report_key_hex: hex::encode(gc_event_key_bytes),
    };
    let gc_event_bytes = serde_json::to_vec(&gc_event)
        .with_context(|| "MEJEPA_RUNBOOK_SESSION_CLEANUP_GC_EVENT_SERIALIZE_FAILED".to_string())?;
    put_cf_sync_readback(
        db.as_ref(),
        context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY,
        gc_event_key_bytes,
        &gc_event_bytes,
        "MEJEPA_RUNBOOK_SESSION_CLEANUP_GC_EVENT",
    )?;
    let gc_event_readback_equal = true;

    Ok(SessionCleanupOutput {
        session_id_hex,
        live_predictions_deleted,
        shift_watermark_deleted: watermark_existed,
        deleted_live_prediction_bytes,
        deleted_shift_watermark_bytes,
        deleted_total_bytes,
        quota_before_total_used_bytes: quota_before.total_used_bytes,
        quota_after_total_used_bytes: quota_after.total_used_bytes,
        quota_state_key_hex: hex::encode(context_graph_mejepa_hygiene::QUOTA_LAST_REPORT_KEY),
        gc_event_key_hex: hex::encode(gc_event_key_bytes),
        quota_report_readback_equal,
        gc_event_readback_equal,
        readback_equal,
        source_of_truth: json!({
            "writer": "rocksdb DB::delete_cf with WriteOptions::set_sync(true)",
            "column_families": [
                context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
                context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
                context_graph_mejepa_cf::CF_MEJEPA_GC_HISTORY,
                context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
            ],
            "archive_root": args.archive_root,
            "quota_category": context_graph_mejepa_hygiene::StorageCategory::ShiftLogSubscriberState,
        }),
    })
}

fn put_cf_sync_readback(
    db: &rocksdb::DB,
    cf_name: &str,
    key: &[u8],
    value: &[u8],
    error_prefix: &str,
) -> Result<()> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| anyhow!("{error_prefix}_CF_MISSING: {cf_name}"))?;
    let mut write_opts = rocksdb::WriteOptions::default();
    write_opts.set_sync(true);
    db.put_cf_opt(cf, key, value, &write_opts)
        .with_context(|| format!("{error_prefix}_WRITE_FAILED: {cf_name}"))?;
    db.flush_cf(cf)
        .with_context(|| format!("{error_prefix}_FLUSH_FAILED: {cf_name}"))?;
    let readback = db
        .get_cf(cf, key)
        .with_context(|| format!("{error_prefix}_READBACK_FAILED: {cf_name}"))?
        .ok_or_else(|| anyhow!("{error_prefix}_READBACK_MISSING: {cf_name}"))?;
    if readback.as_slice() != value {
        return Err(anyhow!("{error_prefix}_READBACK_MISMATCH: {cf_name}"));
    }
    Ok(())
}

fn open_hygiene_rocksdb_read_only(path: &Path) -> Result<rocksdb::DB> {
    validate_dir(path, "storage-db-path")?;
    let mut opts = Options::default();
    opts.set_paranoid_checks(true);
    let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
        .into_iter()
        .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
        .collect::<Vec<_>>();
    rocksdb::DB::open_cf_descriptors_read_only(&opts, path, descriptors, false).with_context(|| {
        format!(
            "MEJEPA_RUNBOOK_STORAGE_READ_ONLY_OPEN_FAILED: {}",
            path.display()
        )
    })
}

fn validate_hygiene_cf_name(raw: &str, field: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_STORAGE_CF_EMPTY: {field} must be a non-empty CF name"
        ));
    }
    if !context_graph_mejepa_cf::all_hygiene_referenced_cfs().contains(&trimmed) {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_STORAGE_CF_UNKNOWN: {field} {trimmed} is not in the ME-JEPA hygiene CF registry"
        ));
    }
    Ok(trimmed.to_string())
}

fn ensure_distinct_dirs(left: &Path, right: &Path, error_code: &str) -> Result<()> {
    let left_real = std::fs::canonicalize(left)
        .with_context(|| format!("{error_code}_CANONICALIZE_LEFT_FAILED: {}", left.display()))?;
    let right_real = std::fs::canonicalize(right).with_context(|| {
        format!(
            "{error_code}_CANONICALIZE_RIGHT_FAILED: {}",
            right.display()
        )
    })?;
    if left_real == right_real {
        return Err(anyhow!(
            "{error_code}: source and target DB paths must differ: {}",
            left_real.display()
        ));
    }
    Ok(())
}

fn collect_cf_rows(db: &rocksdb::DB, cf_name: &str) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| anyhow!("MEJEPA_RUNBOOK_STORAGE_CF_MISSING: {cf_name}"))?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) =
            item.with_context(|| format!("MEJEPA_RUNBOOK_STORAGE_CF_ITER_FAILED: {cf_name}"))?;
        rows.push((key.to_vec(), value.to_vec()));
    }
    Ok(rows)
}

fn rows_value_bytes(rows: &[(Vec<u8>, Vec<u8>)]) -> u64 {
    rows.iter().map(|(_, value)| value.len() as u64).sum()
}

fn replace_cf_rows(
    db: &rocksdb::DB,
    cf_name: &str,
    rows_to_delete: &[(Vec<u8>, Vec<u8>)],
    rows_to_put: &[(Vec<u8>, Vec<u8>)],
) -> Result<()> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| anyhow!("MEJEPA_RUNBOOK_STORAGE_CF_MISSING: {cf_name}"))?;
    let mut batch = WriteBatch::default();
    for (key, _) in rows_to_delete {
        batch.delete_cf(cf, key);
    }
    for (key, value) in rows_to_put {
        batch.put_cf(cf, key, value);
    }
    let mut write_opts = rocksdb::WriteOptions::default();
    write_opts.set_sync(true);
    db.write_opt(batch, &write_opts)
        .with_context(|| format!("MEJEPA_RUNBOOK_STORAGE_CF_BATCH_WRITE_FAILED: {cf_name}"))?;
    db.flush_cf(cf)
        .with_context(|| format!("MEJEPA_RUNBOOK_STORAGE_CF_FLUSH_FAILED: {cf_name}"))?;
    Ok(())
}

fn parse_session_hex(raw: &str) -> Result<[u8; 16]> {
    let trimmed = raw.trim();
    if trimmed.len() != 32 {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_SESSION_ID_INVALID: must be 32 lowercase hex chars; got {} chars",
            trimmed.len()
        ));
    }
    let mut bytes = [0u8; 16];
    hex::decode_to_slice(trimmed, &mut bytes)
        .map_err(|err| anyhow!("MEJEPA_RUNBOOK_SESSION_ID_INVALID: {err}"))?;
    Ok(bytes)
}

#[derive(Args, Debug, Clone)]
pub struct HygieneArgs {
    /// Hygiene RocksDB path containing the hygiene CFs.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_HYGIENE_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Hygiene archive root (cold-tier-down destination).
    #[arg(
        long,
        env = "CONTEXTGRAPH_MEJEPA_HYGIENE_ARCHIVE_ROOT",
        default_value = DEFAULT_HYGIENE_ARCHIVE_ROOT
    )]
    pub archive_root: PathBuf,

    #[command(subcommand)]
    pub action: HygieneAction,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum HygieneAction {
    /// Read-only quota / cold-audit status against the hygiene CFs.
    #[command(name = "cold-audit")]
    ColdAudit,
    /// Run nightly GC: cold-tier compression + emergency eviction if needed.
    #[command(name = "aggressive-cold-tier-down")]
    AggressiveColdTierDown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HygieneOutput {
    pub action: String,
    pub result: Value,
    pub source_of_truth: Value,
}

#[derive(Args, Debug, Clone)]
pub struct DaemonStatusArgs {
    /// Scheduler state root containing `self_optimization_status.json`.
    #[arg(
        long,
        env = "CONTEXTGRAPH_MEJEPA_SCHEDULER_ROOT",
        default_value = DEFAULT_SCHEDULER_STATE_ROOT
    )]
    pub state_root: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct ReportWeeklyArgs {
    /// Inference RocksDB path containing CF_MEJEPA_EVAL_REPORTS.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Report date as `YYYY-MM-DD`. Selects the latest report whose
    /// `report_date` field equals this value.
    #[arg(long)]
    pub date: String,
}

#[derive(Args, Debug, Clone)]
pub struct RollbackArgs {
    /// Heal RocksDB path containing CF_MEJEPA_HEAL_REPORTS and weight blobs.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_HEAL_DB", default_value = DEFAULT_MEJEPA_HEAL_DB)]
    pub heal_db_path: PathBuf,

    /// Witness-chain ledger path used by the heal scheduler.
    #[arg(
        long,
        env = "CONTEXTGRAPH_MEJEPA_WITNESS_CHAIN",
        default_value = DEFAULT_WITNESS_CHAIN_PATH
    )]
    pub witness_chain_path: PathBuf,

    /// Witness-chain offset to roll back to.
    #[arg(long)]
    pub target_witness_offset: u64,
}

#[derive(Args, Debug, Clone)]
pub struct EvalShipGateArgs {
    /// Inference RocksDB path containing CF_MEJEPA_EVAL_REPORTS.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct CellStatusArgs {
    /// Inference RocksDB path containing CF_MEJEPA_EVAL_REPORTS.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Per-cell prediction/oracle correlation threshold.
    #[arg(long, default_value_t = 0.95)]
    pub threshold: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonStatusOutput {
    pub state_root: PathBuf,
    pub status_path: PathBuf,
    pub scheduler_status: Value,
    pub source_of_truth: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportWeeklyOutput {
    pub date: String,
    pub generated_at_unix_ms: i64,
    pub overall_correlation: Option<f32>,
    pub ship_gate_passed: bool,
    pub ship_gate_failures: Vec<String>,
    pub holdout_count: usize,
    pub per_language_correlation: Value,
    pub per_category_correlation: Value,
    pub determinism_hash: String,
    pub source_of_truth: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackOutput {
    pub target_witness_offset: u64,
    pub new_witness_offset: u64,
    pub rolled_back_to_hex: String,
    pub heal_db_path: PathBuf,
    pub witness_chain_path: PathBuf,
    pub source_of_truth: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalShipGateOutput {
    pub latest_report_date: String,
    pub generated_at_unix_ms: i64,
    pub overall_correlation: Option<f32>,
    pub ship_gate_passed: bool,
    pub ship_gate_failures: Vec<String>,
    pub holdout_count: usize,
    pub per_cell_passing_count: usize,
    pub per_cell_failing_count: usize,
    pub source_of_truth: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellStatusRow {
    pub cell: String,
    pub mutation_category: String,
    pub language: String,
    pub correlation: Option<f32>,
    pub convergence_eta: Option<context_graph_mejepa::CellConvergenceEta>,
    pub holdout_count: usize,
    pub distance_to_threshold: Option<f32>,
    pub last_update_unix_ms: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellStatusOutput {
    pub latest_report_date: String,
    pub generated_at_unix_ms: i64,
    pub threshold: f32,
    pub required_cell_count: usize,
    pub observed_cell_count: usize,
    pub passing_cell_count: usize,
    pub below_threshold_cell_count: usize,
    pub missing_cell_count: usize,
    pub failing_cell_count: usize,
    pub passed: bool,
    pub cells: Vec<CellStatusRow>,
    pub source_of_truth: Value,
}

pub fn run_daemon_status(args: DaemonStatusArgs) -> Result<DaemonStatusOutput> {
    validate_dir(&args.state_root, "stateRoot")?;
    let status_path = args.state_root.join("self_optimization_status.json");
    if !status_path.exists() {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_DAEMON_STATUS_MISSING: {} not found",
            status_path.display()
        ));
    }
    let bytes = std::fs::read(&status_path).with_context(|| {
        format!(
            "MEJEPA_RUNBOOK_DAEMON_STATUS_READ_FAILED: {}",
            status_path.display()
        )
    })?;
    let scheduler_status: Value = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "MEJEPA_RUNBOOK_DAEMON_STATUS_JSON_INVALID: {}",
            status_path.display()
        )
    })?;
    Ok(DaemonStatusOutput {
        state_root: args.state_root.clone(),
        status_path: status_path.clone(),
        scheduler_status,
        source_of_truth: json!({
            "kind": "file",
            "path": status_path,
            "writer": "context_graph_mejepa::heal::scheduler::run_self_optimization_scheduler",
        }),
    })
}

pub fn run_report_weekly(args: ReportWeeklyArgs) -> Result<ReportWeeklyOutput> {
    let date = validate_iso_date(&args.date)?;
    let report = load_report_for_date(&args.db_path, &date)?;
    let determinism_hash = report
        .determinism_hash()
        .map_err(|err| anyhow!("MEJEPA_RUNBOOK_REPORT_HASH_FAILED: {err}"))?;
    Ok(ReportWeeklyOutput {
        date,
        generated_at_unix_ms: report.generated_at_unix_ms,
        overall_correlation: report.overall_correlation,
        ship_gate_passed: report.ship_gate_passed,
        ship_gate_failures: report.ship_gate_failures.clone(),
        holdout_count: report.holdout_count,
        per_language_correlation: serde_json::to_value(&report.per_language_correlation)
            .map_err(|err| anyhow!("MEJEPA_RUNBOOK_REPORT_SERIALIZE_FAILED: {err}"))?,
        per_category_correlation: serde_json::to_value(&report.per_category_correlation)
            .map_err(|err| anyhow!("MEJEPA_RUNBOOK_REPORT_SERIALIZE_FAILED: {err}"))?,
        determinism_hash,
        source_of_truth: json!({
            "db_path": args.db_path,
            "column_family": CF_MEJEPA_EVAL_REPORTS,
            "selector": format!("report_date == {}", args.date),
        }),
    })
}

pub fn run_rollback(args: RollbackArgs) -> Result<RollbackOutput> {
    validate_dir(&args.heal_db_path, "heal-db-path")?;
    validate_path_non_empty(&args.witness_chain_path, "witnessChainPath")?;
    let evidence = perform_rollback(
        &args.heal_db_path,
        args.witness_chain_path.clone(),
        args.target_witness_offset,
    )?;
    Ok(RollbackOutput {
        target_witness_offset: evidence.target_witness_chain_offset,
        new_witness_offset: evidence.new_witness_chain_offset,
        rolled_back_to_hex: hex::encode(evidence.rolled_back_to),
        heal_db_path: args.heal_db_path,
        witness_chain_path: args.witness_chain_path,
        source_of_truth: json!({
            "writer": "context_graph_mejepa::heal::AbcPromoter::rollback_to",
            "verification": "witness chain offset advance + CF_MEJEPA_ACTIVE_POINTERS updated",
        }),
    })
}

pub fn run_hygiene(args: HygieneArgs) -> Result<HygieneOutput> {
    use context_graph_mejepa_hygiene::mcp::{mcp_gc_run, mcp_quota_status, HygieneMcpRequest};
    validate_dir(&args.db_path, "hygiene-db-path")?;
    validate_path_non_empty(&args.archive_root, "archiveRoot")?;
    let request = HygieneMcpRequest {
        db_path: args.db_path.clone(),
        archive_root: args.archive_root.clone(),
    };
    let (action_name, result) = match args.action {
        HygieneAction::ColdAudit => (
            "cold-audit",
            mcp_quota_status(request)
                .map_err(|err| anyhow!("MEJEPA_RUNBOOK_HYGIENE_COLD_AUDIT_FAILED: {err}"))?,
        ),
        HygieneAction::AggressiveColdTierDown => (
            "aggressive-cold-tier-down",
            mcp_gc_run(request)
                .map_err(|err| anyhow!("MEJEPA_RUNBOOK_HYGIENE_GC_FAILED: {err}"))?,
        ),
    };
    Ok(HygieneOutput {
        action: action_name.to_string(),
        result,
        source_of_truth: json!({
            "writer": "context_graph_mejepa_hygiene::mcp",
            "db_path": args.db_path,
            "archive_root": args.archive_root,
        }),
    })
}

pub fn run_eval_ship_gate(args: EvalShipGateArgs) -> Result<EvalShipGateOutput> {
    let report = load_latest_report(&args.db_path)?;
    validate_active_python_ship_gate_report(&report)
        .map_err(|err| anyhow!("MEJEPA_RUNBOOK_STALE_SHIP_GATE_GRID: {err}"))?;
    let per_cell_passing_count = report
        .per_cell_correlation
        .values()
        .filter(|value| value.map(|x| x >= 0.95).unwrap_or(false))
        .count();
    let per_cell_failing_count = report.per_cell_correlation.len() - per_cell_passing_count;
    Ok(EvalShipGateOutput {
        latest_report_date: report.report_date.clone(),
        generated_at_unix_ms: report.generated_at_unix_ms,
        overall_correlation: report.overall_correlation,
        ship_gate_passed: report.ship_gate_passed,
        ship_gate_failures: report.ship_gate_failures.clone(),
        holdout_count: report.holdout_count,
        per_cell_passing_count,
        per_cell_failing_count,
        source_of_truth: json!({
            "db_path": args.db_path,
            "column_family": CF_MEJEPA_EVAL_REPORTS,
            "selector": "load_latest_report()",
            "ship_gate_threshold": 0.95,
            "active_gate": ACTIVE_PYTHON_SHIP_GATE_NAME,
            "required_grid": ACTIVE_PYTHON_SHIP_GATE_GRID,
        }),
    })
}

pub fn run_cell_status(args: CellStatusArgs) -> Result<CellStatusOutput> {
    if !args.threshold.is_finite() || !(-1.0..=1.0).contains(&args.threshold) {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_CELL_STATUS_THRESHOLD_INVALID: --threshold must be finite in [-1,1], got {}",
            args.threshold
        ));
    }
    let report = load_latest_report(&args.db_path)?;
    validate_active_python_ship_gate_report(&report)
        .map_err(|err| anyhow!("MEJEPA_RUNBOOK_STALE_SHIP_GATE_GRID: {err}"))?;
    let required = required_active_python_ship_gate_cells();
    let mut rows = Vec::with_capacity(required.len());
    let mut observed_cell_count = 0usize;
    let mut passing_cell_count = 0usize;
    let mut below_threshold_cell_count = 0usize;
    let mut missing_cell_count = 0usize;

    for (category, language, key) in required {
        let correlation = report.per_cell_correlation.get(&key).copied().flatten();
        let status = match correlation {
            Some(value) if value >= args.threshold => {
                observed_cell_count += 1;
                passing_cell_count += 1;
                "pass"
            }
            Some(_) => {
                observed_cell_count += 1;
                below_threshold_cell_count += 1;
                "below_threshold"
            }
            None => {
                missing_cell_count += 1;
                "missing"
            }
        };
        let convergence_eta = report.per_cell_convergence_eta.get(&key).cloned();
        rows.push(CellStatusRow {
            cell: key,
            mutation_category: category.slug().to_string(),
            language: language_slug(language).to_string(),
            correlation,
            convergence_eta,
            holdout_count: report.holdout_count,
            distance_to_threshold: correlation.map(|value| (args.threshold - value).max(0.0)),
            last_update_unix_ms: report.generated_at_unix_ms,
            status: status.to_string(),
        });
    }

    let failing_cell_count = below_threshold_cell_count + missing_cell_count;
    Ok(CellStatusOutput {
        latest_report_date: report.report_date.clone(),
        generated_at_unix_ms: report.generated_at_unix_ms,
        threshold: args.threshold,
        required_cell_count: rows.len(),
        observed_cell_count,
        passing_cell_count,
        below_threshold_cell_count,
        missing_cell_count,
        failing_cell_count,
        passed: failing_cell_count == 0,
        cells: rows,
        source_of_truth: json!({
            "db_path": args.db_path,
            "column_family": CF_MEJEPA_EVAL_REPORTS,
            "selector": "load_latest_report()",
            "required_grid": ACTIVE_PYTHON_SHIP_GATE_GRID,
            "active_gate": ACTIVE_PYTHON_SHIP_GATE_NAME,
            "threshold": args.threshold,
        }),
    })
}

fn load_report_for_date(db_path: &Path, date: &str) -> Result<EvalReport> {
    let store = open_eval_store(db_path)?;
    let db = store.db();
    let cf_handle = db.cf_handle(CF_MEJEPA_EVAL_REPORTS).ok_or_else(|| {
        anyhow!(
            "MEJEPA_RUNBOOK_REPORT_CF_MISSING: column family {CF_MEJEPA_EVAL_REPORTS} not present"
        )
    })?;
    let prefix = format!("{date}::");
    let mut latest: Option<EvalReport> = None;
    for item in db.iterator_cf(cf_handle, IteratorMode::Start) {
        let (key, value) = item.with_context(|| "MEJEPA_RUNBOOK_REPORT_ITER_FAILED".to_string())?;
        let key_str = std::str::from_utf8(&key).unwrap_or("");
        if !key_str.starts_with(&prefix) {
            continue;
        }
        let report: EvalReport = bincode::deserialize(&value)
            .with_context(|| format!("MEJEPA_RUNBOOK_REPORT_DESERIALIZE_FAILED: key={key_str}"))?;
        let take = match latest.as_ref() {
            Some(prev) => report.generated_at_unix_ms >= prev.generated_at_unix_ms,
            None => true,
        };
        if take {
            latest = Some(report);
        }
    }
    latest.ok_or_else(|| {
        anyhow!(
            "MEJEPA_RUNBOOK_REPORT_NOT_FOUND: no CF_MEJEPA_EVAL_REPORTS row with report_date={date}"
        )
    })
}

fn load_latest_report(db_path: &Path) -> Result<EvalReport> {
    let store = open_eval_store(db_path)?;
    let report = store
        .load_latest_report()
        .map_err(|err| anyhow!("MEJEPA_RUNBOOK_LATEST_REPORT_READ_FAILED: {err}"))?;
    report.ok_or_else(|| {
        anyhow!("MEJEPA_RUNBOOK_LATEST_REPORT_MISSING: CF_MEJEPA_EVAL_REPORTS is empty")
    })
}

fn perform_rollback(
    heal_db_path: &Path,
    witness_chain_path: PathBuf,
    target_offset: u64,
) -> Result<RollbackEvidence> {
    let storage = HealRocksStore::open(heal_db_path).map_err(|err| {
        anyhow!(
            "MEJEPA_RUNBOOK_ROLLBACK_HEAL_DB_OPEN_FAILED: {}: {err}",
            heal_db_path.display()
        )
    })?;
    let mut chain = WitnessChainAppender::new(witness_chain_path.clone()).map_err(|err| {
        anyhow!(
            "MEJEPA_RUNBOOK_ROLLBACK_WITNESS_CHAIN_OPEN_FAILED: {}: {err}",
            witness_chain_path.display()
        )
    })?;
    let lock = Arc::new(Mutex::new(PromotionLockState::default()));
    let mut promoter = AbcPromoter::try_new(0.1, PromotionGate::default())
        .map_err(|err| anyhow!("MEJEPA_RUNBOOK_ROLLBACK_PROMOTER_INIT_FAILED: {err}"))?;
    promoter
        .rollback_to(target_offset, storage, &mut chain, lock)
        .map_err(|err| anyhow!("MEJEPA_RUNBOOK_ROLLBACK_FAILED: {err}"))
}

fn open_eval_store(db_path: &Path) -> Result<RocksDbEvalStore> {
    validate_path_non_empty(db_path, "dbPath")?;
    let db = open_infer_rocksdb(db_path)
        .with_context(|| format!("MEJEPA_RUNBOOK_INFER_DB_OPEN_FAILED: {}", db_path.display()))?;
    RocksDbEvalStore::new(db).map_err(|err| anyhow!("MEJEPA_RUNBOOK_EVAL_STORE_OPEN_FAILED: {err}"))
}

fn validate_iso_date(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_DATE_EMPTY: --date must be non-empty"
        ));
    }
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map_err(|err| {
        anyhow!("MEJEPA_RUNBOOK_DATE_INVALID: --date must be YYYY-MM-DD (got {trimmed:?}): {err}")
    })?;
    Ok(trimmed.to_string())
}

fn validate_path_non_empty(path: &Path, field: &str) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_{}_EMPTY: {field} must be a non-empty path",
            field.to_uppercase().replace('-', "_")
        ));
    }
    Ok(())
}

fn validate_dir(path: &Path, field: &str) -> Result<()> {
    validate_path_non_empty(path, field)?;
    if !path.exists() {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_{}_MISSING: {field} {} does not exist",
            field.to_uppercase().replace('-', "_"),
            path.display()
        ));
    }
    if !path.is_dir() {
        return Err(anyhow!(
            "MEJEPA_RUNBOOK_{}_NOT_DIR: {field} {} is not a directory",
            field.to_uppercase().replace('-', "_"),
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_iso_date_accepts_well_formed() {
        assert_eq!(validate_iso_date("2026-05-13").unwrap(), "2026-05-13");
    }

    #[test]
    fn validate_iso_date_rejects_empty() {
        let err = validate_iso_date("   ").unwrap_err().to_string();
        assert!(err.contains("MEJEPA_RUNBOOK_DATE_EMPTY"));
    }

    #[test]
    fn validate_iso_date_rejects_malformed() {
        let err = validate_iso_date("not-a-date").unwrap_err().to_string();
        assert!(err.contains("MEJEPA_RUNBOOK_DATE_INVALID"));
    }

    #[test]
    fn validate_dir_rejects_missing() {
        let err = validate_dir(Path::new("/nope/this/does/not/exist"), "stateRoot")
            .unwrap_err()
            .to_string();
        assert!(err.contains("MEJEPA_RUNBOOK_STATEROOT_MISSING"));
    }

    #[test]
    fn run_pause_writes_and_reads_back_state_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let state_path = temp.path().join("pause-state.json");
        let output = run_pause(PauseArgs {
            state_path: state_path.clone(),
            duration_mins: 5,
            reason: "unit-test".to_string(),
        })
        .unwrap();
        assert!(output.readback_equal);
        assert!(output.paused_until_unix_ms > output.set_at_unix_ms);
        let readback: PauseState =
            serde_json::from_slice(&std::fs::read(state_path).unwrap()).unwrap();
        assert_eq!(readback.reason, "unit-test");
        assert_eq!(readback.paused_until_unix_ms, output.paused_until_unix_ms);
    }

    #[test]
    fn run_pause_rejects_invalid_boundaries() {
        let temp = tempfile::TempDir::new().unwrap();
        let zero = run_pause(PauseArgs {
            state_path: temp.path().join("zero.json"),
            duration_mins: 0,
            reason: "unit-test".to_string(),
        })
        .unwrap_err()
        .to_string();
        assert!(zero.contains("MEJEPA_RUNBOOK_PAUSE_DURATION_ZERO"));

        let invalid_reason = run_pause(PauseArgs {
            state_path: temp.path().join("reason.json"),
            duration_mins: 1,
            reason: "bad\nreason".to_string(),
        })
        .unwrap_err()
        .to_string();
        assert!(invalid_reason.contains("MEJEPA_RUNBOOK_PAUSE_REASON_INVALID"));

        let empty_path = run_pause(PauseArgs {
            state_path: PathBuf::new(),
            duration_mins: 1,
            reason: "unit-test".to_string(),
        })
        .unwrap_err()
        .to_string();
        assert!(empty_path.contains("MEJEPA_RUNBOOK_PAUSE_STATE_PATH_EMPTY"));
    }
}
