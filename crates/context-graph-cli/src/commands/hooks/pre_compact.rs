//! R5: PreCompact hook handler
//!
//! Captures a session summary before Claude Code compresses the context window.
//! This ensures the most important memories from the current session survive
//! compaction and are retrievable in the continued conversation.
//!
//! # Performance Requirements
//! - Timeout: 20000ms (constitution.yaml hooks.timeout_ms.pre_compact)
//! - Database access: ALLOWED (via MCP)
//!
//! # NO BACKWARDS COMPATIBILITY - FAIL FAST

use std::io::{self, BufRead};
use std::time::Instant;

use tracing::{debug, error, info};

use super::args::PreCompactArgs;
use super::error::{HookError, HookResult};
use super::types::{HookInput, HookOutput, HookPayload};
use crate::mcp_client::McpClient;

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of recent user prompts to include in session summary.
const MAX_PROMPTS_FOR_SUMMARY: usize = 5;

/// Importance for session summary memory (high — should survive consolidation).
const SESSION_SUMMARY_IMPORTANCE: f64 = 0.8;

/// Maximum summary content length.
const MAX_SUMMARY_CONTENT_LEN: usize = 2000;

// ============================================================================
// Handler
// ============================================================================

/// Execute pre-compact hook.
///
/// # Flow
/// 1. Parse input (stdin JSON or CLI args)
/// 2. Extract recent user prompts from conversation context
/// 3. Build a session summary string
/// 4. Store the summary via MCP with importance=0.8
/// 5. Return HookOutput
///
/// # Exit Codes
/// - 0: Success
/// - 4: Invalid input
pub async fn execute(args: PreCompactArgs) -> HookResult<HookOutput> {
    let start = Instant::now();

    info!(
        session_id = %args.session_id,
        stdin = args.stdin,
        "PRE_COMPACT: R5 execute starting"
    );

    // 1. Parse input to extract conversation context
    let (trigger, user_prompts) = if args.stdin {
        let input = parse_stdin()?;
        extract_compact_info(&input)?
    } else {
        ("manual".to_string(), Vec::new())
    };

    debug!(
        trigger = %trigger,
        prompt_count = user_prompts.len(),
        "PRE_COMPACT: parsed input"
    );

    // 2. Build session summary from recent user prompts
    let summary = build_session_summary(&args.session_id, &trigger, &user_prompts);

    // 3. Store the summary via MCP
    let client = McpClient::new();
    let mcp_available = match client.is_server_running().await {
        Ok(running) => running,
        Err(e) => {
            error!(error = %e, "PRE_COMPACT: MCP server check failed");
            false
        }
    };

    // MED-10 FIX: Return HookOutput::error when MCP operations fail.
    // Reporting success when the MCP server is down or storage fails is misleading —
    // the hook's purpose (preserving session context) was NOT accomplished.
    if mcp_available {
        let rationale = format!(
            "R5: Session summary before compaction (trigger: {})",
            trigger
        );
        match client
            .inject_context(
                &summary,
                &rationale,
                SESSION_SUMMARY_IMPORTANCE,
                Some(&args.session_id),
            )
            .await
        {
            Ok(result) => {
                let fingerprint_id = result
                    .get("fingerprintId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let execution_time_ms = start.elapsed().as_millis() as u64;
                info!(
                    fingerprint_id,
                    summary_len = summary.len(),
                    execution_time_ms,
                    "PRE_COMPACT: R5 session summary stored successfully"
                );
                Ok(HookOutput::success(execution_time_ms))
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                error!(
                    error = %e,
                    execution_time_ms,
                    "PRE_COMPACT: R5 FAILED to store session summary. \
                     Context may be lost after compaction."
                );
                Ok(HookOutput::error(
                    format!("Failed to store session summary: {}", e),
                    execution_time_ms,
                ))
            }
        }
    } else {
        let execution_time_ms = start.elapsed().as_millis() as u64;
        error!(
            execution_time_ms,
            "PRE_COMPACT: R5 MCP server not available. \
             Cannot store session summary — context will be lost after compaction."
        );
        Ok(HookOutput::error(
            "MCP server not available — session summary could not be stored".to_string(),
            execution_time_ms,
        ))
    }
}

// ============================================================================
// Input Parsing
// ============================================================================

/// Parse stdin JSON into HookInput.
fn parse_stdin() -> HookResult<HookInput> {
    let stdin = io::stdin();
    let mut input_str = String::new();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| {
            error!(error = %e, "PRE_COMPACT: stdin read failed");
            HookError::invalid_input(format!("stdin read failed: {}", e))
        })?;
        input_str.push_str(&line);
    }

    if input_str.is_empty() {
        error!("PRE_COMPACT: stdin is empty");
        return Err(HookError::invalid_input("stdin is empty - expected JSON"));
    }

    serde_json::from_str(&input_str).map_err(|e| {
        error!(error = %e, "PRE_COMPACT: JSON parse failed");
        HookError::invalid_input(format!("JSON parse failed: {}", e))
    })
}

/// Extract trigger and user prompts from HookInput.
fn extract_compact_info(input: &HookInput) -> HookResult<(String, Vec<String>)> {
    match &input.payload {
        HookPayload::PreCompact {
            trigger,
            conversation_summary,
        } => {
            let prompts = conversation_summary
                .as_ref()
                .map(|s| vec![s.clone()])
                .unwrap_or_default();
            Ok((trigger.clone(), prompts))
        }
        _ => {
            // Accept any payload — extract what we can
            debug!("PRE_COMPACT: Non-PreCompact payload, using defaults");
            Ok(("auto".to_string(), Vec::new()))
        }
    }
}

/// Build a session summary string from recent user prompts.
fn build_session_summary(session_id: &str, trigger: &str, user_prompts: &[String]) -> String {
    let mut summary = format!(
        "[SessionSummary: {} (trigger: {})]\n",
        session_id, trigger
    );

    if user_prompts.is_empty() {
        summary.push_str("Pre-compaction checkpoint — no conversation context available.\n");
    } else {
        summary.push_str("Recent topics:\n");
        for (i, prompt) in user_prompts.iter().take(MAX_PROMPTS_FOR_SUMMARY).enumerate() {
            let truncated = if prompt.len() > 200 {
                format!("{}...", &prompt[..prompt.floor_char_boundary(200)])
            } else {
                prompt.clone()
            };
            summary.push_str(&format!("{}. {}\n", i + 1, truncated));
        }
    }

    // Enforce max length
    if summary.len() > MAX_SUMMARY_CONTENT_LEN {
        summary.truncate(summary.floor_char_boundary(MAX_SUMMARY_CONTENT_LEN));
        summary.push_str("...\n");
    }

    summary
}
