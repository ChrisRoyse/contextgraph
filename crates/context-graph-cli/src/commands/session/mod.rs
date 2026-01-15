//! Session identity persistence commands
//!
//! # Commands
//!
//! - `restore-identity`: Restore identity from storage (TASK-SESSION-12)
//! - `persist-identity`: Persist identity to storage (TASK-SESSION-13)
//!
//! # Constitution Reference
//! - IDENTITY-001: IC formula: IC = cos(PV_current, PV_previous) * r(current)
//! - IDENTITY-002: IC thresholds (Healthy >= 0.9, Good >= 0.7, Warning >= 0.5, Degraded < 0.5)
//! - AP-26: Exit codes (0=success, 1=error, 2=corruption)
//! - GWT-003: Identity continuity across sessions
//! - ARCH-07: Native Claude Code hooks
//!
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.

mod persist;
mod restore;

pub use persist::{persist_identity_command, PersistIdentityArgs};
pub use restore::{restore_identity_command, RestoreIdentityArgs};

use clap::Subcommand;

/// Session subcommands
#[derive(Subcommand, Debug)]
pub enum SessionCommands {
    /// Restore identity from storage
    RestoreIdentity(RestoreIdentityArgs),
    /// Persist identity to storage
    PersistIdentity(PersistIdentityArgs),
}

/// Handle session command dispatch
pub async fn handle_session_command(cmd: SessionCommands) -> i32 {
    match cmd {
        SessionCommands::RestoreIdentity(args) => restore_identity_command(args).await,
        SessionCommands::PersistIdentity(args) => persist_identity_command(args).await,
    }
}
