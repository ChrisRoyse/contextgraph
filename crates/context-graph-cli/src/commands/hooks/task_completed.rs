//! R6: TaskCompleted hook handler
//!
//! Extracts learnings from completed tasks and stores them as memories.
//! Captures task subject, ID, and result for future retrieval.
//!
//! # Performance Requirements
//! - Timeout: 20000ms (constitution.yaml hooks.timeout_ms.task_completed)
//! - Database access: ALLOWED (via MCP)
//!
//! # NO BACKWARDS COMPATIBILITY - FAIL FAST

use std::io::{self, BufRead};
use std::time::Instant;

use tracing::{debug, error, info};

use super::args::TaskCompletedArgs;
use super::error::{HookError, HookResult};
use super::types::{HookInput, HookOutput, HookPayload};
use crate::mcp_client::McpClient;

// ============================================================================
// Constants
// ============================================================================

/// Importance for task completion memories (moderate-high — task learnings).
const TASK_MEMORY_IMPORTANCE: f64 = 0.6;

/// Maximum task result content length.
const MAX_TASK_RESULT_LEN: usize = 1500;

// ============================================================================
// Handler
// ============================================================================

/// Execute task-completed hook.
///
/// # Flow
/// 1. Parse input (stdin JSON or CLI args)
/// 2. Extract task subject, ID, and result
/// 3. Build a task summary string
/// 4. Store the summary via MCP with importance=0.6
/// 5. Return HookOutput
///
/// # Exit Codes
/// - 0: Success
/// - 4: Invalid input
pub async fn execute(args: TaskCompletedArgs) -> HookResult<HookOutput> {
    let start = Instant::now();

    info!(
        session_id = %args.session_id,
        stdin = args.stdin,
        "TASK_COMPLETED: R6 execute starting"
    );

    // 1. Parse input to extract task info
    let (task_subject, task_id, task_result) = if args.stdin {
        let input = parse_stdin()?;
        extract_task_info(&input)?
    } else {
        ("unknown task".to_string(), "unknown".to_string(), None)
    };

    debug!(
        task_subject = %task_subject,
        task_id = %task_id,
        has_result = task_result.is_some(),
        "TASK_COMPLETED: parsed input"
    );

    // 2. Build task summary
    let summary = build_task_summary(&args.session_id, &task_subject, &task_id, task_result.as_deref());

    // 3. Store the summary via MCP
    let client = McpClient::new();
    let mcp_available = match client.is_server_running().await {
        Ok(running) => running,
        Err(e) => {
            error!(error = %e, "TASK_COMPLETED: MCP server check failed");
            false
        }
    };

    // MED-10 FIX: Return HookOutput::error when MCP operations fail.
    // Reporting success when the MCP server is down or storage fails is misleading —
    // the hook's purpose (storing task learnings) was NOT accomplished.
    if mcp_available {
        let rationale = format!(
            "R6: Task completion learning (task: {}, id: {})",
            task_subject, task_id
        );
        match client
            .inject_context(
                &summary,
                &rationale,
                TASK_MEMORY_IMPORTANCE,
                Some(&args.session_id),
                Some("text"),
                Some(&["task-completion".to_string(), "learning".to_string()]),
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
                    task_id = %task_id,
                    summary_len = summary.len(),
                    execution_time_ms,
                    "TASK_COMPLETED: R6 task learning stored successfully"
                );
                Ok(HookOutput::success(execution_time_ms))
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                error!(
                    error = %e,
                    task_id = %task_id,
                    execution_time_ms,
                    "TASK_COMPLETED: R6 FAILED to store task learning."
                );
                Ok(HookOutput::error(
                    format!("Failed to store task learning: {}", e),
                    execution_time_ms,
                ))
            }
        }
    } else {
        let execution_time_ms = start.elapsed().as_millis() as u64;
        error!(
            execution_time_ms,
            "TASK_COMPLETED: R6 MCP server not available. \
             Cannot store task learning."
        );
        Ok(HookOutput::error(
            "MCP server not available — task learning could not be stored".to_string(),
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
            error!(error = %e, "TASK_COMPLETED: stdin read failed");
            HookError::invalid_input(format!("stdin read failed: {}", e))
        })?;
        input_str.push_str(&line);
    }

    if input_str.is_empty() {
        error!("TASK_COMPLETED: stdin is empty");
        return Err(HookError::invalid_input("stdin is empty - expected JSON"));
    }

    serde_json::from_str(&input_str).map_err(|e| {
        error!(error = %e, "TASK_COMPLETED: JSON parse failed");
        HookError::invalid_input(format!("JSON parse failed: {}", e))
    })
}

/// Extract task info from HookInput.
fn extract_task_info(input: &HookInput) -> HookResult<(String, String, Option<String>)> {
    match &input.payload {
        HookPayload::TaskCompleted {
            task_subject,
            task_id,
            task_result,
        } => Ok((task_subject.clone(), task_id.clone(), task_result.clone())),
        _ => {
            // Accept any payload — extract what we can
            debug!("TASK_COMPLETED: Non-TaskCompleted payload, using defaults");
            Ok(("unknown task".to_string(), "unknown".to_string(), None))
        }
    }
}

/// Build a task summary string.
fn build_task_summary(
    session_id: &str,
    task_subject: &str,
    task_id: &str,
    task_result: Option<&str>,
) -> String {
    let mut summary = format!(
        "[TaskCompleted: {} (session: {})]\n",
        task_id, session_id
    );

    summary.push_str(&format!("Subject: {}\n", task_subject));

    if let Some(result) = task_result {
        let truncated = if result.len() > MAX_TASK_RESULT_LEN {
            format!("{}...", &result[..result.floor_char_boundary(MAX_TASK_RESULT_LEN)])
        } else {
            result.to_string()
        };
        summary.push_str(&format!("Result: {}\n", truncated));
    }

    summary
}
