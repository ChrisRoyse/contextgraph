//! Operator CLI for ME-JEPA active-learning queue triage.
//!
//! Source of truth is the inference RocksDB active-learning CF set:
//! `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`, `CF_MEJEPA_ACTIVE_LEARNING_LABELS`,
//! and `CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use context_graph_mejepa::{
    bedrock_consistency_for_patch_diff, mejepa_mincut_panel, open_infer_rocksdb,
    open_mincut_rocksdb, operator_contribution_report_from_db, read_library_foundationality_report,
    read_mincut_report, render_operator_contributions_weekly_section, review_ood_harvest,
    write_mincut_report_sync_readback, ActiveLearningLabel, ActiveLearningQueueEntry,
    ActiveLearningRankBy, BedrockConsistencyReport, LabelMethod, LibraryFoundationalityQueryReport,
    LibraryId, MincutOptions, MincutReport, OodHarvestReviewRow, OperatorContributionReport,
    OracleOutcome, PanelGraphSource, RocksDbEvalStore, TaskId,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub(crate) const DEFAULT_MEJEPA_INFER_DB: &str = "/mnt/d/contextgraph/storage/contextgraph-rocksdb";
pub(crate) const DEFAULT_MEJEPA_TRAIN_DB: &str = "/mnt/d/contextgraph/storage/contextgraph-rocksdb";

#[derive(Subcommand, Debug)]
pub enum MejepaCommands {
    /// Active-learning queue operator commands.
    #[command(name = "active-learning")]
    ActiveLearning {
        #[command(subcommand)]
        action: ActiveLearningCommands,
    },
    /// Live-session OOD harvest review commands.
    #[command(name = "ood-harvest")]
    OodHarvest {
        #[command(subcommand)]
        action: OodHarvestCommands,
    },
    /// Replay a persisted ME-JEPA prediction byte-for-byte from RocksDB.
    Replay(crate::commands::mejepa_replay::ReplayArgs),
    /// End-to-end Phase G corpus training-pipeline orchestrator.
    Train(crate::commands::mejepa_train::MejepaTrainArgs),
    /// Read Schmidhuberian compression progress from CF_MEJEPA_TRAIN_CERTS.
    #[command(name = "compression-progress")]
    CompressionProgress(CompressionProgressArgs),
    /// Build and persist a TASK-EK-003 structural-hole mincut panel.
    #[command(name = "mincut-panel")]
    MincutPanel(MincutPanelArgs),
    /// TASK-EK-016 bedrock verifier over a patch diff and persisted foundationality scores.
    #[command(name = "check-bedrock")]
    CheckBedrock(CheckBedrockArgs),
    /// TASK-EK-017 per-library and cross-library foundationality readback.
    #[command(name = "library-foundationality")]
    LibraryFoundationality(LibraryFoundationalityArgs),
    /// TASK-EK-014 operator-contribution report and weekly section renderer.
    #[command(name = "operator-contributions")]
    OperatorContributions(OperatorContributionsArgs),
    /// Bind PostToolUse Bash test outcomes back to live RealityPrediction rows.
    #[command(name = "bind-test-outcomes")]
    BindTestOutcomes(crate::commands::mejepa_prediction_verification::BindTestOutcomesArgs),
    /// Stop-hook gate: list non-Abstain predictions missing Confirmed/Refuted verification.
    #[command(name = "stop-self-verify")]
    StopSelfVerify(crate::commands::mejepa_prediction_verification::StopSelfVerifyArgs),
    /// Checkpointed Python train/calibration/holdout cycle with eval readback.
    #[command(name = "checkpointed-train")]
    CheckpointedTrain(crate::commands::mejepa_train::MejepaCheckpointedTrainArgs),
    /// Phase G cross-validation against a pre-scraped public CI corpus.
    #[command(name = "cross-validate-public-ci")]
    CrossValidatePublicCi(
        crate::commands::mejepa_public_ci_cross_validate::PublicCiCrossValidationArgs,
    ),
    /// Phase G oracle determinism audit that writes corpus quarantine config.
    #[command(name = "oracle-flakiness-audit")]
    OracleFlakinessAudit(crate::commands::mejepa_oracle_flakiness::OracleFlakinessAuditArgs),
    /// Phase G per-cell convergence status from CF_MEJEPA_EVAL_REPORTS.
    #[command(name = "cell-status")]
    CellStatus(crate::commands::mejepa_runbook::CellStatusArgs),
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
    /// Operator runbook: hygiene cold-audit / aggressive-cold-tier-down.
    Hygiene(crate::commands::mejepa_runbook::HygieneArgs),
    /// Operator runbook: integrity-check the inference RocksDB.
    #[command(name = "storage-verify")]
    StorageVerify(crate::commands::mejepa_runbook::StorageVerifyArgs),
    /// Operator runbook: storage verification, CF restore, and CF migration commands.
    Storage {
        #[command(subcommand)]
        action: crate::commands::mejepa_runbook::StorageCommands,
    },
    /// Operator runbook: delete all rows for a closed session.
    #[command(name = "session-cleanup")]
    SessionCleanup(crate::commands::mejepa_runbook::SessionCleanupArgs),
    /// Operator runbook: pause predictions for a duration (writes pause state file).
    Pause(crate::commands::mejepa_runbook::PauseArgs),
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

#[derive(Subcommand, Debug)]
pub enum OodHarvestCommands {
    /// List top harvested OOD rows with bytes-on-disk anchors.
    Review(OodHarvestReviewArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ActiveLearningListArgs {
    /// Inference RocksDB path containing CF_MEJEPA_ACTIVE_LEARNING_QUEUE.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Maximum queue entries to show.
    #[arg(long, default_value_t = 10)]
    pub top_n: usize,

    /// Queue ordering to use.
    #[arg(long, value_enum, default_value_t = ActiveLearningRankByArg::SchedulerPriority)]
    pub ranked_by: ActiveLearningRankByArg,
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

#[derive(Args, Debug, Clone)]
pub struct OodHarvestReviewArgs {
    /// Inference RocksDB path containing CF_MEJEPA_OOD_HARVEST.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Maximum harvested rows to show.
    #[arg(long, default_value_t = 10)]
    pub top_n: usize,
}

#[derive(Args, Debug, Clone)]
pub struct CompressionProgressArgs {
    /// RocksDB path containing CF_MEJEPA_TRAIN_CERTS.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_TRAIN_DB", default_value = DEFAULT_MEJEPA_TRAIN_DB)]
    pub db_path: PathBuf,

    /// Number of recent training certificates to inspect.
    #[arg(long, default_value_t = context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_WINDOW)]
    pub window: u64,

    /// Allowed floating-point decrease in running mean CP before it is called regressing.
    #[arg(long, default_value_t = context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS)]
    pub epsilon_bits: f64,
}

#[derive(Args, Debug, Clone)]
pub struct MincutPanelArgs {
    /// ME-JEPA RocksDB path containing the graph-source CFs and CF_MEJEPA_MINCUT_REPORTS.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// JSON file containing graphSource plus optional options, or a top-level PanelGraphSource.
    #[arg(long)]
    pub input: PathBuf,

    /// Persist the resulting MincutReport to CF_MEJEPA_MINCUT_REPORTS.
    #[arg(long, default_value_t = true)]
    pub persist: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CheckBedrockArgs {
    /// Inference RocksDB path containing CF_MEJEPA_CHUNK_FOUNDATIONALITY.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Unified diff file to check against persisted foundationality scores.
    #[arg(long)]
    pub patch: PathBuf,

    /// Foundationality threshold at or above which a touched chunk is considered bedrock.
    #[arg(long, default_value_t = 0.75)]
    pub threshold: f32,

    /// Maximum touched chunks to return.
    #[arg(long, default_value_t = 5)]
    pub top_k: usize,
}

#[derive(Args, Debug, Clone)]
pub struct LibraryFoundationalityArgs {
    /// Inference RocksDB path containing TASK-EK-017 library foundationality CFs.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Optional library slug: python-swe-bench-lite, non-python-fixtures, shakespeare-canon, santa-training-video, customer-service-transcripts, or custom:<name>.
    #[arg(long)]
    pub library_id: Option<String>,

    /// Maximum rows to return in each ranking.
    #[arg(long, default_value_t = 10)]
    pub top_k: usize,
}

#[derive(Args, Debug, Clone)]
pub struct OperatorContributionsArgs {
    /// Inference RocksDB path containing CF_MEJEPA_OPERATOR_CONTRIBUTIONS.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Number of latest contribution rows to include after filtering.
    #[arg(long)]
    pub window: usize,

    /// Optional operator id filter.
    #[arg(long)]
    pub operator: Option<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OperatorContributionFormatArg::Json)]
    pub format: OperatorContributionFormatArg,
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

#[derive(Clone, Copy, Debug, ValueEnum, Serialize)]
#[clap(rename_all = "kebab_case")]
#[serde(rename_all = "snake_case")]
pub enum ActiveLearningRankByArg {
    SchedulerPriority,
    Curiosity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Serialize)]
#[clap(rename_all = "kebab_case")]
#[serde(rename_all = "snake_case")]
pub enum OperatorContributionFormatArg {
    Json,
    Markdown,
}

impl ActiveLearningRankByArg {
    fn into_rank_by(self) -> ActiveLearningRankBy {
        match self {
            Self::SchedulerPriority => ActiveLearningRankBy::SchedulerPriority,
            Self::Curiosity => ActiveLearningRankBy::Curiosity,
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
    pub curiosity_score: f32,
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
    pub ranked_by: ActiveLearningRankByArg,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OodHarvestReviewOutput {
    pub tool: &'static str,
    pub top_n: usize,
    pub rows: Vec<OodHarvestReviewRow>,
    pub source_of_truth: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionProgressCliOutput {
    pub tool: &'static str,
    pub source_of_truth: serde_json::Value,
    pub report: context_graph_mejepa_train::CompressionProgressReport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MincutPanelCliOutput {
    pub tool: &'static str,
    pub report: MincutReport,
    pub persisted_readback_equal: bool,
    pub source_of_truth: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckBedrockCliOutput {
    pub tool: &'static str,
    pub report: BedrockConsistencyReport,
    pub source_of_truth: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFoundationalityCliOutput {
    pub tool: &'static str,
    pub report: LibraryFoundationalityQueryReport,
    pub source_of_truth: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatorContributionsCliOutput {
    pub tool: &'static str,
    pub format: OperatorContributionFormatArg,
    pub report: OperatorContributionReport,
    pub markdown: Option<String>,
    pub source_of_truth: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MincutPanelInputEnvelope {
    #[serde(default)]
    graph_source: Option<PanelGraphSource>,
    #[serde(default)]
    options: Option<MincutOptions>,
}

pub async fn handle_mejepa_command(action: MejepaCommands) -> i32 {
    let action = match action {
        MejepaCommands::CellStatus(args) => {
            return match crate::commands::mejepa_runbook::run_cell_status(args) {
                Ok(output) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&output)
                            .unwrap_or_else(|_| serde_json::to_string(&output).unwrap_or_default())
                    );
                    if output.passed {
                        0
                    } else {
                        1
                    }
                }
                Err(err) => {
                    eprintln!("mejepa command FAILED: {err:#}");
                    1
                }
            };
        }
        MejepaCommands::OperatorContributions(args)
            if args.format == OperatorContributionFormatArg::Markdown =>
        {
            return match operator_contributions_cli(args) {
                Ok(output) => {
                    if let Some(markdown) = output.markdown {
                        println!("{markdown}");
                    } else {
                        eprintln!("mejepa command FAILED: markdown output missing");
                        return 1;
                    }
                    0
                }
                Err(err) => {
                    eprintln!("mejepa command FAILED: {err:#}");
                    1
                }
            };
        }
        other => other,
    };
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
        MejepaCommands::OodHarvest { action } => match action {
            OodHarvestCommands::Review(args) => serde_json::to_value(review_ood_harvest_cli(args)?)
                .context("serialize ood-harvest review"),
        },
        MejepaCommands::Replay(args) => {
            serde_json::to_value(crate::commands::mejepa_replay::replay_prediction_cli(args).await?)
                .context("serialize replay")
        }
        MejepaCommands::Train(args) => {
            serde_json::to_value(crate::commands::mejepa_train::run_mejepa_train(args)?)
                .context("serialize train")
        }
        MejepaCommands::CompressionProgress(args) => {
            serde_json::to_value(compression_progress(args)?)
                .context("serialize compression-progress")
        }
        MejepaCommands::MincutPanel(args) => {
            serde_json::to_value(mincut_panel_cli(args)?).context("serialize mincut-panel")
        }
        MejepaCommands::CheckBedrock(args) => {
            serde_json::to_value(check_bedrock_cli(args)?).context("serialize check-bedrock")
        }
        MejepaCommands::LibraryFoundationality(args) => {
            serde_json::to_value(library_foundationality_cli(args)?)
                .context("serialize library-foundationality")
        }
        MejepaCommands::OperatorContributions(args) => {
            serde_json::to_value(operator_contributions_cli(args)?)
                .context("serialize operator-contributions")
        }
        MejepaCommands::BindTestOutcomes(args) => serde_json::to_value(
            crate::commands::mejepa_prediction_verification::bind_test_outcomes(args)?,
        )
        .context("serialize bind-test-outcomes"),
        MejepaCommands::StopSelfVerify(args) => serde_json::to_value(
            crate::commands::mejepa_prediction_verification::stop_self_verify(args)?,
        )
        .context("serialize stop-self-verify"),
        MejepaCommands::CheckpointedTrain(args) => serde_json::to_value(
            crate::commands::mejepa_train::run_checkpointed_python_train(args)?,
        )
        .context("serialize checkpointed-train"),
        MejepaCommands::CrossValidatePublicCi(args) => serde_json::to_value(
            crate::commands::mejepa_public_ci_cross_validate::run_public_ci_cross_validation(args)?,
        )
        .context("serialize cross-validate-public-ci"),
        MejepaCommands::OracleFlakinessAudit(args) => serde_json::to_value(
            crate::commands::mejepa_oracle_flakiness::run_oracle_flakiness_audit(args)?,
        )
        .context("serialize oracle-flakiness-audit"),
        MejepaCommands::CellStatus(args) => {
            serde_json::to_value(crate::commands::mejepa_runbook::run_cell_status(args)?)
                .context("serialize cell-status")
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
        MejepaCommands::Hygiene(args) => {
            serde_json::to_value(crate::commands::mejepa_runbook::run_hygiene(args)?)
                .context("serialize hygiene")
        }
        MejepaCommands::StorageVerify(args) => {
            serde_json::to_value(crate::commands::mejepa_runbook::run_storage_verify(args)?)
                .context("serialize storage-verify")
        }
        MejepaCommands::Storage { action } => serde_json::to_value(
            crate::commands::mejepa_runbook::run_storage_command(action)?,
        )
        .context("serialize storage"),
        MejepaCommands::SessionCleanup(args) => {
            serde_json::to_value(crate::commands::mejepa_runbook::run_session_cleanup(args)?)
                .context("serialize session-cleanup")
        }
        MejepaCommands::Pause(args) => {
            serde_json::to_value(crate::commands::mejepa_runbook::run_pause(args)?)
                .context("serialize pause")
        }
    }
}

pub fn compression_progress(args: CompressionProgressArgs) -> Result<CompressionProgressCliOutput> {
    let report = context_graph_mejepa_train::compression_progress_report_from_path(
        &args.db_path,
        args.window,
        args.epsilon_bits,
    )
    .with_context(|| {
        format!(
            "MEJEPA_COMPRESSION_PROGRESS_FAILED: db_path={} window={}",
            args.db_path.display(),
            args.window
        )
    })?;
    Ok(CompressionProgressCliOutput {
        tool: "context-graph-cli mejepa compression-progress",
        source_of_truth: json!({
            "dbPath": args.db_path,
            "trainCertCf": context_graph_mejepa_train::CF_MEJEPA_TRAIN_CERTS,
        }),
        report,
    })
}

pub fn mincut_panel_cli(args: MincutPanelArgs) -> Result<MincutPanelCliOutput> {
    let bytes = fs::read(&args.input)
        .with_context(|| format!("read mincut-panel input {}", args.input.display()))?;
    let (graph_source, options) =
        if let Ok(graph_source) = serde_json::from_slice::<PanelGraphSource>(&bytes) {
            (graph_source, MincutOptions::default())
        } else {
            let envelope: MincutPanelInputEnvelope =
                serde_json::from_slice(&bytes).context("parse mincut-panel JSON input")?;
            let graph_source = envelope
                .graph_source
                .ok_or_else(|| anyhow!("mincut-panel input must contain graphSource"))?;
            (graph_source, envelope.options.unwrap_or_default())
        };
    let created_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let db = open_mincut_rocksdb(&args.db_path)
        .with_context(|| format!("open ME-JEPA RocksDB {}", args.db_path.display()))?;
    let report = mejepa_mincut_panel(Some(db.as_ref()), graph_source, options, created_at_unix_ms)
        .with_context(|| format!("MEJEPA_MINCUT_PANEL_FAILED: input={}", args.input.display()))?;
    if args.persist {
        write_mincut_report_sync_readback(db.as_ref(), &report)
            .context("write CF_MEJEPA_MINCUT_REPORTS")?;
    }
    let persisted_readback_equal = if args.persist {
        read_mincut_report(db.as_ref(), &report.report_id)
            .context("read CF_MEJEPA_MINCUT_REPORTS")?
            .map(|row| row == report)
            .unwrap_or(false)
    } else {
        false
    };
    if args.persist && !persisted_readback_equal {
        return Err(anyhow!(
            "MEJEPA_MINCUT_REPORT_READBACK_MISMATCH: report_id={}",
            report.report_id
        ));
    }
    Ok(MincutPanelCliOutput {
        tool: "context-graph-cli mejepa mincut-panel",
        report,
        persisted_readback_equal,
        source_of_truth: json!({
            "dbPath": args.db_path,
            "inputPath": args.input,
            "mincutReportCf": context_graph_mejepa_cf::CF_MEJEPA_MINCUT_REPORTS,
            "persisted": args.persist,
            "innerLlmInvoked": false
        }),
    })
}

pub fn check_bedrock_cli(args: CheckBedrockArgs) -> Result<CheckBedrockCliOutput> {
    let patch = fs::read_to_string(&args.patch)
        .with_context(|| format!("read patch diff {}", args.patch.display()))?;
    let db = open_infer_rocksdb(&args.db_path)
        .with_context(|| format!("open ME-JEPA RocksDB {}", args.db_path.display()))?;
    let report =
        bedrock_consistency_for_patch_diff(db.as_ref(), &patch, args.threshold, args.top_k)
            .with_context(|| {
                format!(
                    "MEJEPA_CHECK_BEDROCK_FAILED: patch={}",
                    args.patch.display()
                )
            })?;
    Ok(CheckBedrockCliOutput {
        tool: "context-graph-cli mejepa check-bedrock",
        report,
        source_of_truth: json!({
            "dbPath": args.db_path,
            "patch": args.patch,
            "foundationalityCf": context_graph_mejepa_cf::CF_MEJEPA_CHUNK_FOUNDATIONALITY,
            "innerLlmInvoked": false
        }),
    })
}

pub fn library_foundationality_cli(
    args: LibraryFoundationalityArgs,
) -> Result<LibraryFoundationalityCliOutput> {
    let db = open_infer_rocksdb(&args.db_path)
        .with_context(|| format!("open ME-JEPA RocksDB {}", args.db_path.display()))?;
    let library_id = args
        .library_id
        .as_deref()
        .map(LibraryId::parse_slug)
        .transpose()
        .context("parse library id")?;
    let report = read_library_foundationality_report(db.as_ref(), library_id.as_ref(), args.top_k)
        .context("MEJEPA_LIBRARY_FOUNDATIONALITY_FAILED")?;
    Ok(LibraryFoundationalityCliOutput {
        tool: "context-graph-cli mejepa library-foundationality",
        report,
        source_of_truth: json!({
            "dbPath": args.db_path,
            "libraryId": args.library_id,
            "libraryRegistryCf": context_graph_mejepa_cf::CF_MEJEPA_LIBRARY_REGISTRY,
            "libraryFoundationalityCf": context_graph_mejepa_cf::CF_MEJEPA_LIBRARY_FOUNDATIONALITY,
            "crossLibraryReferencesCf": context_graph_mejepa_cf::CF_MEJEPA_CROSS_LIBRARY_REFERENCES,
            "innerLlmInvoked": false
        }),
    })
}

pub fn operator_contributions_cli(
    args: OperatorContributionsArgs,
) -> Result<OperatorContributionsCliOutput> {
    let db = open_infer_rocksdb(&args.db_path)
        .with_context(|| format!("open ME-JEPA RocksDB {}", args.db_path.display()))?;
    let report =
        operator_contribution_report_from_db(db.as_ref(), args.window, args.operator.as_deref())
            .context("MEJEPA_OPERATOR_CONTRIBUTIONS_FAILED")?;
    let markdown = if args.format == OperatorContributionFormatArg::Markdown {
        Some(
            render_operator_contributions_weekly_section(&report)
                .context("render operator-contributions markdown")?,
        )
    } else {
        None
    };
    Ok(OperatorContributionsCliOutput {
        tool: "context-graph-cli mejepa operator-contributions",
        format: args.format,
        report,
        markdown,
        source_of_truth: json!({
            "dbPath": args.db_path,
            "operatorContributionCf": context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_CONTRIBUTIONS,
            "operatorFilter": args.operator,
            "innerLlmInvoked": false
        }),
    })
}

pub fn review_ood_harvest_cli(args: OodHarvestReviewArgs) -> Result<OodHarvestReviewOutput> {
    validate_top_n(args.top_n)?;
    let db = open_infer_rocksdb(&args.db_path)
        .with_context(|| format!("open ME-JEPA RocksDB {}", args.db_path.display()))?;
    let rows = review_ood_harvest(db.as_ref(), Some(&args.db_path), args.top_n)
        .context("MEJEPA_OOD_HARVEST_REVIEW_FAILED")?;
    Ok(OodHarvestReviewOutput {
        tool: "context-graph-cli mejepa ood-harvest review",
        top_n: args.top_n,
        rows,
        source_of_truth: json!({
            "dbPath": args.db_path,
            "harvestCf": context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST,
            "calibrationCf": context_graph_mejepa_cf::CF_MEJEPA_OOD_CALIBRATIONS,
            "innerLlmInvoked": false
        }),
    })
}

pub async fn list_active_learning(
    args: ActiveLearningListArgs,
) -> Result<ActiveLearningListOutput> {
    validate_top_n(args.top_n)?;
    let store = open_store(&args.db_path)?;
    let queue = load_required_queue(&store)?;
    let entries = queue
        .ranked_entries(args.ranked_by.into_rank_by())
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
        ranked_by: args.ranked_by,
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

fn entry_view(entry: &ActiveLearningQueueEntry) -> Result<ActiveLearningEntryView> {
    Ok(ActiveLearningEntryView {
        task_id: entry.task_id.0.clone(),
        score: entry.score,
        outcome_set_len: entry.outcome_set_len,
        ood_score: entry.ood_score,
        curiosity_score: entry.curiosity_score,
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
    use context_graph_mejepa::{
        compute_chunk_foundationality, persist_chunk_foundationality_report_sync_readback,
        ActiveLearningKind, ActiveLearningQueueState, ChunkDependencyEdge,
        ChunkFoundationalityConfig, PredictionId,
    };

    fn entry(task_id: &str, score: f32, kind: ActiveLearningKind) -> ActiveLearningQueueEntry {
        ActiveLearningQueueEntry {
            task_id: TaskId(task_id.to_string()),
            score,
            outcome_set_len: 2,
            ood_score: 0.1,
            curiosity_score: 0.0,
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
        let rows = queue.ranked_entries(ActiveLearningRankBy::SchedulerPriority);
        assert_eq!(rows[0].task_id.0, "surprise-low");
        assert_eq!(rows[1].task_id.0, "uncertain-high");
    }

    #[test]
    fn curiosity_ranking_orders_by_curiosity_score() {
        let mut queue = ActiveLearningQueueState::new(8).unwrap();
        queue.entries.insert(
            TaskId("scheduler-high".to_string()),
            entry("scheduler-high", 0.99, ActiveLearningKind::Uncertainty),
        );
        queue.entries.insert(
            TaskId("curiosity-high".to_string()),
            entry("curiosity-high", 0.10, ActiveLearningKind::Uncertainty)
                .with_curiosity_score(0.88)
                .unwrap(),
        );
        let rows = queue.ranked_entries(ActiveLearningRankBy::Curiosity);
        assert_eq!(rows[0].task_id.0, "curiosity-high");
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

    #[test]
    fn check_bedrock_cli_reads_foundationality_scores() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("infer-rocksdb");
        let patch_path = tmp.path().join("patch.diff");
        let db = open_infer_rocksdb(&db_path).unwrap();
        let edges = vec![
            ChunkDependencyEdge::new("app.py::handler", "pkg/core.py::Base", "call", 1.0, "test"),
            ChunkDependencyEdge::new(
                "tests/test_core.py::test_base",
                "pkg/core.py::Base",
                "test_verifies",
                1.0,
                "test",
            ),
        ];
        let report =
            compute_chunk_foundationality(&edges, 1, ChunkFoundationalityConfig::default())
                .unwrap();
        persist_chunk_foundationality_report_sync_readback(db.as_ref(), &edges, &report).unwrap();
        drop(db);
        fs::write(
            &patch_path,
            "diff --git a/pkg/core.py b/pkg/core.py\n--- a/pkg/core.py\n+++ b/pkg/core.py\n",
        )
        .unwrap();
        let output = check_bedrock_cli(CheckBedrockArgs {
            db_path,
            patch: patch_path,
            threshold: 0.75,
            top_k: 5,
        })
        .unwrap();
        assert!(output.report.bedrock_touched);
        assert_eq!(
            output.report.top_touched_chunks[0].chunk_id,
            "pkg/core.py::Base"
        );
    }
}
