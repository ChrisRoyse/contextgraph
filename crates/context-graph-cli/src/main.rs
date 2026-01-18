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
//!
//! This CLI provides hooks integration for Claude Code via .claude/settings.json.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! # Constitution Reference
//! - ARCH-07: Native Claude Code hooks
//! - AP-26: Exit code 1 on error, 2 on corruption

use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};

mod commands;
mod error;

pub use error::{exit_code_for_error, is_corruption_indicator, CliExitCode};

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
    /// Initialize context-graph hooks for Claude Code
    ///
    /// Creates .claude/settings.json and .claude/hooks/ directory with
    /// all required hook scripts for context-graph integration.
    Setup(commands::setup::SetupArgs),
}

#[tokio::main]
async fn main() {
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
        Commands::Setup(args) => commands::setup::handle_setup(args).await,
    };

    std::process::exit(exit_code);
}
