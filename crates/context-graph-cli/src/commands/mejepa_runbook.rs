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
use clap::Args;
use context_graph_mejepa::eval::store::CF_MEJEPA_EVAL_REPORTS;
use context_graph_mejepa::eval::types::EvalReport;
use context_graph_mejepa::heal::{
    AbcPromoter, HealRocksStore, PromotionGate, PromotionLockState, RollbackEvidence,
    WitnessChainAppender,
};
use context_graph_mejepa::{open_infer_rocksdb, RocksDbEvalStore};
use rocksdb::IteratorMode;
use serde::Serialize;
use serde_json::{json, Value};

const DEFAULT_MEJEPA_INFER_DB: &str = "/mnt/d/contextgraph/storage/contextgraph-rocksdb";
const DEFAULT_MEJEPA_HEAL_DB: &str = "/mnt/d/contextgraph/storage/mejepa-heal";
const DEFAULT_SCHEDULER_STATE_ROOT: &str = "/mnt/d/contextgraph/state/schedulers";
const DEFAULT_WITNESS_CHAIN_PATH: &str =
    "/mnt/d/contextgraph/state/cgreality/mejepa-witness-chain.bin";
const DEFAULT_HYGIENE_ARCHIVE_ROOT: &str = "/mnt/d/contextgraph/storage/hygiene-archive";

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
        anyhow!("MEJEPA_RUNBOOK_REPORT_NOT_FOUND: no CF_MEJEPA_EVAL_REPORTS row with report_date={date}")
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
}
