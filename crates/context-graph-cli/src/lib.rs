//! Context Graph CLI — library surface.
//!
//! This crate ships a binary (`context-graph-cli`) and a small library
//! surface that integration tests can pull in. The library intentionally
//! re-exports only what tests / downstream callers need so the CLI remains
//! the canonical entry point.

pub mod commands;
pub mod error;
pub mod governed_edit;
pub mod mcp_client;
pub mod mcp_helpers;

pub use error::{exit_code_for_error, is_corruption_indicator, CliExitCode};
