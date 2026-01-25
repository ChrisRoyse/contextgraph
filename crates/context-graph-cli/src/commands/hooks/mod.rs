//! Hook types for Claude Code native integration
//!
//! # Architecture
//! This module defines the data types for hook input/output that match
//! Claude Code's native hook system specification.
//!
//! # Constitution References
//! - IDENTITY-002: IC thresholds and timeout requirements
//! - AP-25: N=13 (13 embedder spaces)
//! - AP-26: Exit codes (0=success, 1=error, 2=corruption)
//! - AP-50: NO internal hooks (use Claude Code native)
//! - AP-53: Hook logic in shell scripts calling CLI
//!
//! # NO BACKWARDS COMPATIBILITY
//! This module FAILS FAST on any error. Do not add fallback logic.

// Allow dead code in submodules - these types are scaffolded for future hook implementations
#[allow(dead_code)]
mod args;
#[allow(dead_code)]
mod error;
pub mod memory_cache;
#[allow(dead_code)]
pub mod post_tool_use;
#[allow(dead_code)]
pub mod pre_tool_use;
pub mod session_end;
pub mod session_start;
pub mod session_state;
#[allow(dead_code)]
mod types;
pub mod user_prompt_submit;

pub use args::HooksCommands;

use tracing::error;

/// Handle hooks subcommand dispatch
///
/// # Exit Codes
/// - 0: Success
/// - 1: General error
/// - 2: Timeout
/// - 3: Database error
/// - 4: Invalid input
pub async fn handle_hooks_command(cmd: HooksCommands) -> i32 {
    match cmd {
        HooksCommands::SessionStart(args) => {
            match session_start::execute(args).await {
                Ok(output) => {
                    // Output JSON to stdout
                    match serde_json::to_string(&output) {
                        Ok(json) => {
                            println!("{}", json);
                            0
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to serialize output");
                            1
                        }
                    }
                }
                Err(e) => {
                    // Output error JSON to stderr
                    let error_json = e.to_json_error();
                    eprintln!("{}", error_json);
                    e.exit_code()
                }
            }
        }
        HooksCommands::PreTool(args) => {
            match pre_tool_use::handle_pre_tool_use(&args) {
                Ok(output) => {
                    // Output JSON to stdout
                    match serde_json::to_string(&output) {
                        Ok(json) => {
                            println!("{}", json);
                            0
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to serialize output");
                            1
                        }
                    }
                }
                Err(e) => {
                    // Output error JSON to stderr
                    let error_json = e.to_json_error();
                    eprintln!("{}", error_json);
                    e.exit_code()
                }
            }
        }
        HooksCommands::PostTool(args) => match post_tool_use::execute(args).await {
            Ok(output) => match serde_json::to_string(&output) {
                Ok(json) => {
                    println!("{}", json);
                    0
                }
                Err(e) => {
                    error!(error = %e, "Failed to serialize output");
                    1
                }
            },
            Err(e) => {
                let error_json = e.to_json_error();
                eprintln!("{}", error_json);
                e.exit_code()
            }
        },
        HooksCommands::PromptSubmit(args) => match user_prompt_submit::execute(args).await {
            Ok(output) => match serde_json::to_string(&output) {
                Ok(json) => {
                    println!("{}", json);
                    0
                }
                Err(e) => {
                    error!(error = %e, "Failed to serialize output");
                    1
                }
            },
            Err(e) => {
                let error_json = e.to_json_error();
                eprintln!("{}", error_json);
                e.exit_code()
            }
        },
        HooksCommands::SessionEnd(args) => match session_end::execute(args).await {
            Ok(output) => match serde_json::to_string(&output) {
                Ok(json) => {
                    println!("{}", json);
                    0
                }
                Err(e) => {
                    error!(error = %e, "Failed to serialize output");
                    1
                }
            },
            Err(e) => {
                let error_json = e.to_json_error();
                eprintln!("{}", error_json);
                e.exit_code()
            }
        },
        HooksCommands::GenerateConfig(_args) => {
            error!("GenerateConfig not yet implemented");
            1
        }
    }
}
