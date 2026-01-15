//! Context Graph CLI
//!
//! CLI tools for Context Graph session identity and consciousness management.
//!
//! # Commands
//!
//! - `consciousness check-identity`: Check identity continuity and trigger dream if crisis
//! - `consciousness brief`: Quick consciousness status for PreToolUse hook
//! - `session restore-identity`: Restore identity from storage
//! - `session persist-identity`: Persist identity to storage
//!
//! # TASK-SESSION-08: CLI Dream Trigger Integration
//!
//! This CLI provides hooks integration for Claude Code via .claude/settings.json.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! # Constitution Reference
//! - ARCH-07: Native Claude Code hooks
//! - AP-26: Exit code 1 on error, 2 on corruption
//! - AP-38: IC<0.5 MUST auto-trigger dream
//! - AP-42: entropy>0.7 MUST wire to TriggerManager

use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};

mod commands;

/// Context Graph CLI - Session Identity and Consciousness Management
#[derive(Parser)]
#[command(name = "context-graph-cli")]
#[command(author = "Context Graph Team")]
#[command(version = "0.1.0")]
#[command(about = "CLI tools for Context Graph session identity and consciousness management")]
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
    /// Consciousness state management commands
    Consciousness {
        #[command(subcommand)]
        action: commands::consciousness::ConsciousnessCommands,
    },
    /// Session identity persistence commands
    Session {
        #[command(subcommand)]
        action: commands::session::SessionCommands,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Setup logging based on verbosity
    let filter = match cli.verbose {
        0 => EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("warn")),
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
        Commands::Consciousness { action } => {
            commands::consciousness::handle_consciousness_command(action).await
        }
        Commands::Session { action } => {
            commands::session::handle_session_command(action).await
        }
    };

    std::process::exit(exit_code);
}
