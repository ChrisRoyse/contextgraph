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
pub mod pre_compact;
#[allow(dead_code)]
pub mod pre_tool_use;
pub mod session_end;
pub mod session_start;
pub mod session_state;
#[allow(dead_code)]
pub mod task_completed;
#[allow(dead_code)]
mod types;
pub mod user_prompt_submit;

pub use args::HooksCommands;

use error::HookResult;
use tracing::error;
use types::HookOutput;

/// Convert a hook result to an exit code, printing output JSON to stdout
/// on success or error JSON to stderr on failure.
fn emit_hook_result(result: HookResult<HookOutput>) -> i32 {
    match result {
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
            eprintln!("{}", e.to_json_error());
            e.exit_code()
        }
    }
}

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
            emit_hook_result(session_start::execute(args).await)
        }
        HooksCommands::PreTool(args) => {
            emit_hook_result(pre_tool_use::handle_pre_tool_use(&args))
        }
        HooksCommands::PostTool(args) => {
            emit_hook_result(post_tool_use::execute(args).await)
        }
        HooksCommands::PromptSubmit(args) => {
            emit_hook_result(user_prompt_submit::execute(args).await)
        }
        HooksCommands::SessionEnd(args) => {
            emit_hook_result(session_end::execute(args).await)
        }
        HooksCommands::PreCompact(args) => {
            emit_hook_result(pre_compact::execute(args).await)
        }
        HooksCommands::TaskCompleted(args) => {
            emit_hook_result(task_completed::execute(args).await)
        }
        HooksCommands::GenerateConfig(_args) => {
            error!(
                "GenerateConfig is not implemented. Hook scripts are generated \
                 automatically by the `setup` command. Run: \
                 context-graph-cli setup --generate-hooks"
            );
            1
        }
    }
}
