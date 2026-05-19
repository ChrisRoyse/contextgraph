//! CLI command handlers
//!
//! # Modules
//!
//! - `session`: Session persistence commands
//! - `hooks`: Hook types for Claude Code native integration (TASK-HOOKS-001)
//! - `memory`: Memory capture and context injection commands (TASK-P6-003)
//! - `warmup`: Pre-load embedding models into VRAM (TASK-EMB-WARMUP)
//! - `topic`: Topic portfolio and stability commands
//! - `divergence`: Divergence detection commands

pub mod backfill_e14;
pub mod ccreality;
pub mod divergence;
pub mod dynamicjepa;
pub mod export_dynamicjepa;
pub mod export_training;
pub mod hooks;
pub mod learning;
pub mod mejepa_active_learning;
pub mod mejepa_oracle_flakiness;
pub mod mejepa_prediction_verification;
pub mod mejepa_public_ci_cross_validate;
pub mod mejepa_replay;
pub mod mejepa_runbook;
pub mod mejepa_train;
pub mod memory;
pub mod session;
pub mod setup;
pub mod topic;
pub mod utl;
pub mod warmup;
pub mod watch;

/// Test utilities for CLI tests
#[cfg(test)]
pub mod test_utils {
    use std::sync::Mutex;

    /// Global test lock for serializing tests that access shared state.
    pub static GLOBAL_IDENTITY_LOCK: Mutex<()> = Mutex::new(());
}
