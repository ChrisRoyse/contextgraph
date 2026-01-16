//! Hook types for Claude Code native integration
//!
//! # Architecture
//! This module defines the data types for hook input/output that match
//! Claude Code's native hook system specification.
//!
//! # Constitution References
//! - IDENTITY-002: IC thresholds and timeout requirements
//! - AP-25: Kuramoto N=13
//! - AP-26: Exit codes (0=success, 1=error, 2=corruption)
//! - AP-50: NO internal hooks (use Claude Code native)
//! - AP-53: Hook logic in shell scripts calling CLI
//!
//! # NO BACKWARDS COMPATIBILITY
//! This module FAILS FAST on any error. Do not add fallback logic.

mod args;
mod error;
pub mod post_tool_use;
pub mod pre_tool_use;
pub mod session_start;
mod types;
pub mod user_prompt_submit;

pub use args::{
    GenerateConfigArgs,
    HookType,
    HooksCommands,
    OutputFormat,
    PostToolArgs,
    PreToolArgs,
    PromptSubmitArgs,
    SessionEndArgs,
    SessionStartArgs,
    ShellType,
};

pub use error::{HookError, HookResult};

pub use types::{
    ConsciousnessState,
    ConversationMessage,
    HookEventType,
    HookInput,
    HookOutput,
    HookPayload,
    ICClassification,
    ICLevel,
    JohariQuadrant,
    SessionEndStatus,
};

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
        HooksCommands::PostTool(args) => {
            match post_tool_use::execute(args).await {
                Ok(output) => {
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
                    let error_json = e.to_json_error();
                    eprintln!("{}", error_json);
                    e.exit_code()
                }
            }
        }
        HooksCommands::PromptSubmit(args) => {
            match user_prompt_submit::execute(args).await {
                Ok(output) => {
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
                    let error_json = e.to_json_error();
                    eprintln!("{}", error_json);
                    e.exit_code()
                }
            }
        }
        HooksCommands::SessionEnd(_args) => {
            error!("SessionEnd hook not yet implemented");
            1
        }
        HooksCommands::GenerateConfig(_args) => {
            error!("GenerateConfig not yet implemented");
            1
        }
    }
}
