//! Context Graph CLI
//!
//! CLI tools for Context Graph memory management and hooks integration.
//!
//! # Commands
//!
//! - `session restore-identity`: Restore session state from storage
//! - `session persist-identity`: Persist session state to storage
//! - `hooks`: Claude Code native hooks commands
//! - `memory`: Memory capture and context injection commands
//! - `warmup`: Pre-load embedding models into VRAM
//!
//! This CLI provides hooks integration for Claude Code via .claude/settings.json.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! # Constitution Reference
//! - ARCH-07: Native Claude Code hooks
//! - ARCH-08: CUDA GPU required for production
//! - AP-26: Exit code 1 on error, 2 on corruption

use clap::{Parser, Subcommand};
use context_graph_cli::commands;
use tracing_subscriber::{fmt, EnvFilter};

/// Context Graph CLI - Memory Management and Hooks Integration
#[derive(Parser)]
#[command(name = "context-graph-cli")]
#[command(author = "Context Graph Team")]
#[command(version = "0.1.0")]
#[command(about = "CLI tools for Context Graph memory management and hooks integration")]
#[command(propagate_version = true)]
struct Cli {
    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Session persistence commands
    Session {
        #[command(subcommand)]
        action: commands::session::SessionCommands,
    },
    /// Claude Code native hooks commands
    Hooks {
        #[command(subcommand)]
        action: commands::hooks::HooksCommands,
    },
    /// Memory capture and context injection commands
    Memory {
        #[command(subcommand)]
        action: commands::memory::MemoryCommands,
    },
    /// Topic portfolio and stability commands
    ///
    /// Explore emergent topics and stability metrics.
    /// Topics emerge from weighted multi-space clustering (threshold >= 2.5).
    Topic {
        #[command(subcommand)]
        action: commands::topic::TopicCommands,
    },
    /// Divergence detection commands
    ///
    /// Check for divergence from recent activity patterns.
    /// Uses SEMANTIC embedders only (E1, E5-E7, E10, E12, E13).
    /// Temporal embedders (E2-E4) are excluded per AP-62, AP-63.
    Divergence {
        #[command(subcommand)]
        action: commands::divergence::DivergenceCommands,
    },
    /// DynamicJEPA registry, adapter, panel, binding, trajectory, dataset, and source-of-truth commands
    #[command(name = "dynamicjepa")]
    DynamicJepa {
        #[command(subcommand)]
        action: Box<commands::dynamicjepa::DynamicJepaCommands>,
    },
    /// Learning-as-UTL event commands
    Learning {
        #[command(subcommand)]
        action: commands::learning::LearningCommands,
    },
    /// UTL learner-state computation and storage commands
    Utl {
        #[command(subcommand)]
        action: commands::utl::UtlCommands,
    },
    /// Initialize context-graph hooks for Claude Code
    ///
    /// Creates .claude/settings.json and .claude/hooks/ directory with
    /// all required hook scripts for context-graph integration.
    Setup(commands::setup::SetupArgs),
    /// Pre-load embedding models into VRAM
    ///
    /// Loads all 13 embedding models into GPU VRAM before the MCP server
    /// starts. This ensures embedding operations are available immediately.
    ///
    /// Run this before starting the MCP server:
    ///   context-graph-cli warmup
    ///   context-graph-mcp
    ///
    /// Takes approximately 20-30 seconds on RTX 5090 (32GB VRAM).
    Warmup(commands::warmup::WarmupArgs),
    /// Watch a directory for markdown file changes
    ///
    /// Monitors the specified directory for .md file changes and automatically
    /// chunks and stores them as memories with source metadata.
    ///
    /// Example:
    ///   context-graph-cli watch --path ./docs --session-id my-session
    Watch(commands::watch::WatchArgs),
    /// File-format exports of persisted artifacts (Phase 6)
    ///
    /// Dumps rows from RocksDB column families into flat files for
    /// downstream consumers (HuggingFace `datasets`, DuckDB, etc.).
    Export {
        #[command(subcommand)]
        sub: ExportSubcommand,
    },
    /// Backfill E14 BGE-M3 Dense vectors on existing fingerprints.
    ///
    /// Iterates every fingerprint; for records whose `e14_bge_m3_dense` field
    /// is empty (pre-Phase-A / pre-BGE-M3), re-embeds the original content
    /// through the native `BgeM3DenseModel` and persists the updated
    /// fingerprint. Supports `--dry-run` to count candidates without touching
    /// the model or writing anything.
    ///
    /// Expects the BAAI/bge-m3 snapshot under
    /// `<models_dir>/bge-m3-dense/` (defaults to `./models/bge-m3-dense/`).
    #[command(name = "backfill-e14")]
    BackfillE14(commands::backfill_e14::BackfillE14Args),
    /// ccreality hook support commands.
    Ccreality {
        #[command(subcommand)]
        action: commands::ccreality::CCRealityCommands,
    },
    /// ME-JEPA operator commands.
    Mejepa {
        #[command(subcommand)]
        action: commands::mejepa_active_learning::MejepaCommands,
    },
}

/// `context-graph-cli export <sub>` targets.
///
/// Phase 6 shipped the `training-corpus` target (Parquet). The
/// DynamicJEPA episode exporter turns verified bundles into JSONL process
/// episodes for downstream model training and audit replay.
#[derive(Subcommand)]
enum ExportSubcommand {
    /// Export every row in `CF_TRAINING_RECORDS` to a Parquet file.
    ///
    /// Schema: `(memory_id: Utf8, record_bytes: Binary)`. The `record_bytes`
    /// column is the bincode-with-version-byte payload identical to what
    /// lives in RocksDB. Decode in Rust via
    /// `context_graph_storage::teleological::decode_training_record` or in
    /// Python via the `TRAINING_RECORD_VERSION` + bincode wire contract.
    #[command(name = "training-corpus")]
    TrainingCorpus(commands::export_training::ExportTrainingArgs),
    /// Export DynamicJEPA bundle plan and edge-case evidence to JSONL.
    #[command(name = "dynamicjepa-episodes")]
    DynamicJepaEpisodes(commands::export_dynamicjepa::ExportDynamicJepaEpisodesArgs),
}

#[tokio::main]
pub async fn main() {
    let cli = Cli::parse();

    // Setup logging based on verbosity
    let filter = match cli.verbose {
        0 => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        1 => EnvFilter::new("info"),
        2 => EnvFilter::new("debug"),
        _ => EnvFilter::new("trace"),
    };

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_level(true)
        .with_writer(std::io::stderr)
        .init();

    // Dispatch to command handlers
    let exit_code = match cli.command {
        Commands::Session { action } => commands::session::handle_session_command(action).await,
        Commands::Hooks { action } => commands::hooks::handle_hooks_command(action).await,
        Commands::Memory { action } => commands::memory::handle_memory_command(action).await,
        Commands::Topic { action } => commands::topic::handle_topic_command(action).await,
        Commands::Divergence { action } => {
            commands::divergence::handle_divergence_command(action).await
        }
        Commands::DynamicJepa { action } => {
            commands::dynamicjepa::handle_dynamicjepa_command(*action).await
        }
        Commands::Learning { action } => commands::learning::handle_learning_command(action).await,
        Commands::Utl { action } => commands::utl::handle_utl_command(action).await,
        Commands::Setup(args) => commands::setup::handle_setup(args).await,
        Commands::Warmup(args) => commands::warmup::handle_warmup(args).await,
        Commands::Watch(args) => commands::watch::handle_watch(args).await,
        Commands::Export { sub } => match sub {
            ExportSubcommand::TrainingCorpus(args) => {
                let result = commands::export_training::run(args).await;
                commands::export_training::summary_to_exit_code(result)
            }
            ExportSubcommand::DynamicJepaEpisodes(args) => {
                let result = commands::export_dynamicjepa::run(args).await;
                commands::export_dynamicjepa::summary_to_exit_code(result)
            }
        },
        Commands::BackfillE14(args) => commands::backfill_e14::run(args).await,
        Commands::Ccreality { action } => {
            commands::ccreality::handle_ccreality_command(action).await
        }
        Commands::Mejepa { action } => {
            commands::mejepa_active_learning::handle_mejepa_command(action).await
        }
    };

    std::process::exit(exit_code);
}
