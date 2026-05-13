//! Operator CLI for ME-JEPA active-learning queue triage.
//!
//! Source of truth is the inference RocksDB active-learning CF set:
//! `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`, `CF_MEJEPA_ACTIVE_LEARNING_LABELS`,
//! and `CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS`.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use context_graph_mejepa::{
    open_infer_rocksdb, ActiveLearningLabel, ActiveLearningQueueEntry, LabelMethod, OracleOutcome,
    RocksDbEvalStore, TaskId,
};
use serde::Serialize;
use serde_json::json;

const DEFAULT_MEJEPA_INFER_DB: &str = "/mnt/d/contextgraph/storage/contextgraph-rocksdb";

#[derive(Subcommand, Debug)]
pub enum MejepaCommands {
    /// Active-learning queue operator commands.
    #[command(name = "active-learning")]
    ActiveLearning {
        #[command(subcommand)]
        action: ActiveLearningCommands,
    },
    /// Replay a persisted ME-JEPA prediction byte-for-byte from RocksDB.
    Replay(crate::commands::mejepa_replay::ReplayArgs),
    /// Operator runbook: read the self-optimization scheduler status file.
    #[command(name = "daemon-status")]
    DaemonStatus(crate::commands::mejepa_runbook::DaemonStatusArgs),
    /// Operator runbook: read the weekly EvalReport for a specific date.
    #[command(name = "report-weekly")]
    ReportWeekly(crate::commands::mejepa_runbook::ReportWeeklyArgs),
    /// Operator runbook: roll the active weights back to a witness-chain offset.
    Rollback(crate::commands::mejepa_runbook::RollbackArgs),
    /// Operator runbook: read ship-gate status from the latest EvalReport.
    #[command(name = "eval-ship-gate")]
    EvalShipGate(crate::commands::mejepa_runbook::EvalShipGateArgs),
}

#[derive(Subcommand, Debug)]
pub enum ActiveLearningCommands {
    /// List queued active-learning items, sorted by scheduler priority.
    List(ActiveLearningListArgs),
    /// Label and remove one queued item.
    Label(ActiveLearningLabelArgs),
    /// Dismiss and remove one queued item.
    Dismiss(ActiveLearningDismissArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ActiveLearningListArgs {
    /// Inference RocksDB path containing CF_MEJEPA_ACTIVE_LEARNING_QUEUE.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Maximum queue entries to show.
    #[arg(long, default_value_t = 10)]
    pub top_n: usize,
}

#[derive(Args, Debug, Clone)]
pub struct ActiveLearningLabelArgs {
    /// Inference RocksDB path containing active-learning CFs.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Task id to label.
    pub task_id: String,

    /// Oracle outcome label to persist.
    #[arg(value_enum)]
    pub label: OracleOutcomeArg,

    /// Operator reason for the label.
    pub reason: String,
}

#[derive(Args, Debug, Clone)]
pub struct ActiveLearningDismissArgs {
    /// Inference RocksDB path containing active-learning CFs.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Task id to dismiss.
    pub task_id: String,

    /// Operator reason for dismissal.
    pub reason: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum OracleOutcomeArg {
    Pass,
    Fail,
    OutOfDistribution,
    Abstain,
}

impl OracleOutcomeArg {
    fn into_outcome(self) -> OracleOutcome {
        match self {
            Self::Pass => OracleOutcome::Pass,
            Self::Fail => OracleOutcome::Fail,
            Self::OutOfDistribution => OracleOutcome::OutOfDistribution,
            Self::Abstain => OracleOutcome::Abstain,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveLearningEntryView {
    pub task_id: String,
    pub score: f32,
    pub outcome_set_len: usize,
    pub ood_score: f32,
    pub reason: String,
    pub kind: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveLearningListOutput {
    pub capacity: usize,
    pub queued_count: usize,
    pub evicted_count: usize,
    pub ood_escalation_count: usize,
    pub top_n: usize,
    pub entries: Vec<ActiveLearningEntryView>,
    pub source_of_truth: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveLearningLabelOutput {
    pub task_id: String,
    pub label: OracleOutcome,
    pub reason: String,
    pub queue_count_before: usize,
    pub queue_count_after: usize,
    pub persisted_label_readback_equal: bool,
    pub source_of_truth: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveLearningDismissOutput {
    pub task_id: String,
    pub reason: String,
    pub queue_count_before: usize,
    pub queue_count_after: usize,
    pub evicted_count_after: usize,
    pub persisted_eviction_readback_equal: bool,
    pub source_of_truth: serde_json::Value,
}

pub async fn handle_mejepa_command(action: MejepaCommands) -> i32 {
    match run(action).await {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
            );
            0
        }
        Err(err) => {
            eprintln!("mejepa command FAILED: {err:#}");
            1
        }
    }
}

async fn run(action: MejepaCommands) -> Result<serde_json::Value> {
    match action {
        MejepaCommands::ActiveLearning { action } => match action {
            ActiveLearningCommands::List(args) => {
                serde_json::to_value(list_active_learning(args).await?).context("serialize list")
            }
            ActiveLearningCommands::Label(args) => {
                serde_json::to_value(label_active_learning(args).await?).context("serialize label")
            }
            ActiveLearningCommands::Dismiss(args) => {
                serde_json::to_value(dismiss_active_learning(args).await?)
                    .context("serialize dismiss")
            }
        },
        MejepaCommands::Replay(args) => {
            serde_json::to_value(crate::commands::mejepa_replay::replay_prediction_cli(args).await?)
                .context("serialize replay")
        }
        MejepaCommands::DaemonStatus(args) => {
            serde_json::to_value(crate::commands::mejepa_runbook::run_daemon_status(args)?)
                .context("serialize daemon-status")
        }
        MejepaCommands::ReportWeekly(args) => {
            serde_json::to_value(crate::commands::mejepa_runbook::run_report_weekly(args)?)
                .context("serialize report-weekly")
        }
        MejepaCommands::Rollback(args) => {
            serde_json::to_value(crate::commands::mejepa_runbook::run_rollback(args)?)
                .context("serialize rollback")
        }
        MejepaCommands::EvalShipGate(args) => {
            serde_json::to_value(crate::commands::mejepa_runbook::run_eval_ship_gate(args)?)
                .context("serialize eval-ship-gate")
        }
    }
}

pub async fn list_active_learning(
    args: ActiveLearningListArgs,
) -> Result<ActiveLearningListOutput> {
    validate_top_n(args.top_n)?;
    let store = open_store(&args.db_path)?;
    let queue = load_required_queue(&store)?;
    let entries = sorted_entries(&queue.entries)
        .into_iter()
        .take(args.top_n)
        .map(entry_view)
        .collect::<Result<Vec<_>>>()?;
    Ok(ActiveLearningListOutput {
        capacity: queue.capacity,
        queued_count: queue.entries.len(),
        evicted_count: queue.evicted.len(),
        ood_escalation_count: queue.ood_escalations.len(),
        top_n: args.top_n,
        entries,
        source_of_truth: source_of_truth(&args.db_path, "CF_MEJEPA_ACTIVE_LEARNING_QUEUE"),
    })
}

pub async fn label_active_learning(
    args: ActiveLearningLabelArgs,
) -> Result<ActiveLearningLabelOutput> {
    validate_reason(&args.reason)?;
    let task_id = task_id_from_arg(&args.task_id)?;
    let store = open_store(&args.db_path)?;
    let mut queue = load_required_queue(&store)?;
    let queue_count_before = queue.entries.len();
    let _entry = queue.entries.remove(&task_id).ok_or_else(|| {
        anyhow!(
            "MEJEPA_ACTIVE_LEARNING_ENTRY_NOT_FOUND: task_id={} source_of_truth=CF_MEJEPA_ACTIVE_LEARNING_QUEUE",
            task_id.0
        )
    })?;
    let label = ActiveLearningLabel {
        task_id: task_id.clone(),
        oracle_outcome: args.label.into_outcome(),
        method: LabelMethod::Human,
        labeled_at_unix_ms: chrono::Utc::now().timestamp_millis(),
    };
    store.persist_label(&label)?;
    store.persist_queue(&queue)?;
    let readback = store.load_label(&task_id)?.ok_or_else(|| {
        anyhow!(
            "MEJEPA_ACTIVE_LEARNING_LABEL_READBACK_MISSING: task_id={}",
            task_id.0
        )
    })?;
    let queue_readback = load_required_queue(&store)?;
    let readback_equal = readback.task_id == label.task_id
        && readback.oracle_outcome == label.oracle_outcome
        && readback.method == label.method
        && readback.labeled_at_unix_ms == label.labeled_at_unix_ms;
    if !readback_equal {
        return Err(anyhow!(
            "MEJEPA_ACTIVE_LEARNING_LABEL_READBACK_MISMATCH: task_id={}",
            task_id.0
        ));
    }
    if queue_readback.entries.contains_key(&task_id) {
        return Err(anyhow!(
            "MEJEPA_ACTIVE_LEARNING_QUEUE_READBACK_MISMATCH: labeled task still queued: {}",
            task_id.0
        ));
    }
    Ok(ActiveLearningLabelOutput {
        task_id: task_id.0.clone(),
        label: label.oracle_outcome,
        reason: args.reason,
        queue_count_before,
        queue_count_after: queue_readback.entries.len(),
        persisted_label_readback_equal: true,
        source_of_truth: json!({
            "queue_cf": "CF_MEJEPA_ACTIVE_LEARNING_QUEUE",
            "label_cf": "CF_MEJEPA_ACTIVE_LEARNING_LABELS",
            "db_path": args.db_path,
        }),
    })
}

pub async fn dismiss_active_learning(
    args: ActiveLearningDismissArgs,
) -> Result<ActiveLearningDismissOutput> {
    validate_reason(&args.reason)?;
    let task_id = task_id_from_arg(&args.task_id)?;
    let store = open_store(&args.db_path)?;
    let mut queue = load_required_queue(&store)?;
    let queue_count_before = queue.entries.len();
    let mut dismissed = queue.entries.remove(&task_id).ok_or_else(|| {
        anyhow!(
            "MEJEPA_ACTIVE_LEARNING_ENTRY_NOT_FOUND: task_id={} source_of_truth=CF_MEJEPA_ACTIVE_LEARNING_QUEUE",
            task_id.0
        )
    })?;
    dismissed.reason = format!("operator_dismissed:{}", args.reason);
    queue.evicted.push(dismissed.clone());
    store.persist_queue(&queue)?;
    let queue_readback = load_required_queue(&store)?;
    let eviction_readback = store.load_evicted_entry(&task_id)?.ok_or_else(|| {
        anyhow!(
            "MEJEPA_ACTIVE_LEARNING_EVICTION_READBACK_MISSING: task_id={}",
            task_id.0
        )
    })?;
    let readback_equal = eviction_readback.task_id == dismissed.task_id
        && eviction_readback.reason == dismissed.reason
        && (eviction_readback.score - dismissed.score).abs() < f32::EPSILON
        && eviction_readback.kind == dismissed.kind;
    if !readback_equal {
        return Err(anyhow!(
            "MEJEPA_ACTIVE_LEARNING_EVICTION_READBACK_MISMATCH: task_id={}",
            task_id.0
        ));
    }
    if queue_readback.entries.contains_key(&task_id) {
        return Err(anyhow!(
            "MEJEPA_ACTIVE_LEARNING_QUEUE_READBACK_MISMATCH: dismissed task still queued: {}",
            task_id.0
        ));
    }
    Ok(ActiveLearningDismissOutput {
        task_id: task_id.0.clone(),
        reason: args.reason,
        queue_count_before,
        queue_count_after: queue_readback.entries.len(),
        evicted_count_after: queue_readback.evicted.len(),
        persisted_eviction_readback_equal: true,
        source_of_truth: json!({
            "queue_cf": "CF_MEJEPA_ACTIVE_LEARNING_QUEUE",
            "eviction_cf": "CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS",
            "db_path": args.db_path,
        }),
    })
}

fn open_store(db_path: &Path) -> Result<RocksDbEvalStore> {
    let db = open_infer_rocksdb(db_path).with_context(|| {
        format!(
            "MEJEPA_ACTIVE_LEARNING_DB_OPEN_FAILED: {}",
            db_path.display()
        )
    })?;
    RocksDbEvalStore::new(db).context("MEJEPA_ACTIVE_LEARNING_STORE_OPEN_FAILED")
}

fn load_required_queue(
    store: &RocksDbEvalStore,
) -> Result<context_graph_mejepa::ActiveLearningQueueState> {
    store.load_queue()?.ok_or_else(|| {
        anyhow!(
            "MEJEPA_ACTIVE_LEARNING_QUEUE_MISSING: source_of_truth=CF_MEJEPA_ACTIVE_LEARNING_QUEUE"
        )
    })
}

fn sorted_entries(
    entries: &std::collections::BTreeMap<TaskId, ActiveLearningQueueEntry>,
) -> Vec<&ActiveLearningQueueEntry> {
    let mut rows = entries.values().collect::<Vec<_>>();
    rows.sort_by(|lhs, rhs| {
        lhs.kind
            .fast_track_rank()
            .cmp(&rhs.kind.fast_track_rank())
            .then_with(|| rhs.score.total_cmp(&lhs.score))
            .then_with(|| lhs.task_id.cmp(&rhs.task_id))
    });
    rows
}

fn entry_view(entry: &ActiveLearningQueueEntry) -> Result<ActiveLearningEntryView> {
    Ok(ActiveLearningEntryView {
        task_id: entry.task_id.0.clone(),
        score: entry.score,
        outcome_set_len: entry.outcome_set_len,
        ood_score: entry.ood_score,
        reason: entry.reason.clone(),
        kind: serde_json::to_value(&entry.kind).context("serialize active-learning kind")?,
    })
}

fn validate_top_n(top_n: usize) -> Result<()> {
    if top_n == 0 {
        return Err(anyhow!(
            "MEJEPA_ACTIVE_LEARNING_TOP_N_INVALID: --top-n must be > 0"
        ));
    }
    Ok(())
}

fn validate_reason(reason: &str) -> Result<()> {
    if reason.trim().is_empty() {
        return Err(anyhow!(
            "MEJEPA_ACTIVE_LEARNING_REASON_EMPTY: reason must be non-empty"
        ));
    }
    Ok(())
}

fn task_id_from_arg(raw: &str) -> Result<TaskId> {
    let task_id = TaskId(raw.trim().to_string());
    task_id
        .validate("active_learning.task_id")
        .context("MEJEPA_ACTIVE_LEARNING_TASK_ID_INVALID")?;
    Ok(task_id)
}

fn source_of_truth(db_path: &Path, cf_name: &str) -> serde_json::Value {
    json!({
        "db_path": db_path,
        "column_family": cf_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_mejepa::{ActiveLearningKind, ActiveLearningQueueState, PredictionId};

    fn entry(task_id: &str, score: f32, kind: ActiveLearningKind) -> ActiveLearningQueueEntry {
        ActiveLearningQueueEntry {
            task_id: TaskId(task_id.to_string()),
            score,
            outcome_set_len: 2,
            ood_score: 0.1,
            reason: "test".to_string(),
            kind,
        }
    }

    #[test]
    fn sorted_entries_fast_tracks_agent_surprise_before_score() {
        let mut queue = ActiveLearningQueueState::new(8).unwrap();
        queue.entries.insert(
            TaskId("uncertain-high".to_string()),
            entry("uncertain-high", 0.99, ActiveLearningKind::Uncertainty),
        );
        queue.entries.insert(
            TaskId("surprise-low".to_string()),
            entry(
                "surprise-low",
                0.20,
                ActiveLearningKind::AgentSurprise {
                    prediction_id: PredictionId([0x11; 16]),
                    severity_score: 0.2,
                },
            ),
        );
        let rows = sorted_entries(&queue.entries);
        assert_eq!(rows[0].task_id.0, "surprise-low");
        assert_eq!(rows[1].task_id.0, "uncertain-high");
    }

    #[test]
    fn empty_reason_fails_closed() {
        let err = validate_reason("  ").unwrap_err().to_string();
        assert!(err.contains("MEJEPA_ACTIVE_LEARNING_REASON_EMPTY"));
    }

    #[test]
    fn zero_top_n_fails_closed() {
        let err = validate_top_n(0).unwrap_err().to_string();
        assert!(err.contains("MEJEPA_ACTIVE_LEARNING_TOP_N_INVALID"));
    }
}
