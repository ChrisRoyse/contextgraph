//! Memory inject-context and inject-brief command implementations.
//!
//! - `inject-context`: Called by UserPromptSubmit and SessionStart hooks
//! - `inject-brief`: Called by PreToolUse hook for quick context (<200 tokens)
//!
//! # Task 14: Connect CLI to MCP Server
//!
//! This module uses the MCP client to connect to the running MCP server
//! for search and context injection operations. This eliminates local model
//! loading and ensures all operations use the MCP server's warm-loaded GPU models.
//!
//! # Constitution Compliance
//!
//! - ARCH-06: All memory ops through MCP tools
//! - ARCH-08: CUDA GPU required (MCP server uses GPU, not CLI)
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - ARCH-10: Divergence uses SEMANTIC embedders only
//! - AP-06: No direct DB access - MCP tools only
//! - AP-11: Check existing utils before creating helpers (uses mcp_helpers)
//! - AP-14: No .unwrap() in library code
//! - AP-26: Exit code 1 on error, 2 on corruption

use clap::Args;
use tracing::{debug, info};

use crate::commands::hooks::memory_cache::get_cached_memories;
use crate::error::CliExitCode;
use crate::mcp_client::McpClient;
use crate::mcp_helpers::{mcp_error_to_exit_code, require_mcp_server, resolve_session_id};

// =============================================================================
// Constants (AP-12: No magic numbers)
// =============================================================================

/// Default token budget for inject-context (per constitution injection.priorities).
pub const DEFAULT_CONTEXT_BUDGET: u32 = 1200;

/// Brief context budget (per constitution).
pub const BRIEF_BUDGET: u32 = 200;

/// Characters per token estimate (conservative).
const CHARS_PER_TOKEN: u32 = 4;

// =============================================================================
// Argument Structs
// =============================================================================

/// Arguments for inject-context command.
///
/// Supports both CLI arguments and environment variable fallbacks
/// for seamless hook integration.
///
/// # Environment Variables
///
/// - `USER_PROMPT`: Query text source
/// - `CLAUDE_SESSION_ID`: Session identifier
/// - `CONTEXT_GRAPH_MCP_HOST`: MCP server host (default: 127.0.0.1)
/// - `CONTEXT_GRAPH_MCP_PORT`: MCP server port (default: 3100)
#[derive(Args)]
pub struct InjectContextArgs {
    /// Query text for context retrieval (or use USER_PROMPT env var)
    pub query: Option<String>,

    /// Session ID (or use CLAUDE_SESSION_ID env var)
    #[arg(long)]
    pub session_id: Option<String>,

    /// Token budget for context (default: 1200 per constitution)
    #[arg(long, default_value_t = DEFAULT_CONTEXT_BUDGET)]
    pub budget: u32,

    /// Maximum number of results to retrieve (default: 10)
    #[arg(long, default_value = "10")]
    pub top_k: u32,
}

/// Arguments for inject-brief command.
///
/// Called by PreToolUse hook for quick context injection (<200 tokens).
/// Reads TOOL_DESCRIPTION or TOOL_NAME from environment for query.
///
/// NOTE: No --budget argument. Brief ALWAYS uses 200 tokens (BRIEF_BUDGET constant).
///
/// # Environment Variables
///
/// - `TOOL_DESCRIPTION`: Primary query source
/// - `TOOL_NAME`: Fallback query source
/// - `CLAUDE_SESSION_ID`: Session identifier
/// - `CONTEXT_GRAPH_MCP_HOST`: MCP server host (default: 127.0.0.1)
/// - `CONTEXT_GRAPH_MCP_PORT`: MCP server port (default: 3100)
#[derive(Args)]
pub struct InjectBriefArgs {
    /// Query text (or use TOOL_DESCRIPTION/TOOL_NAME env var)
    pub query: Option<String>,

    /// Session ID (or use CLAUDE_SESSION_ID env var)
    #[arg(long)]
    pub session_id: Option<String>,

    /// Maximum number of results to retrieve (default: 5 for brief)
    #[arg(long, default_value = "5")]
    pub top_k: u32,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Resolve query from CLI argument or environment variables.
///
/// Returns None if no valid query is available.
fn resolve_query(
    arg: Option<String>,
    primary_env: &str,
    fallback_env: Option<&str>,
) -> Option<String> {
    arg.or_else(|| std::env::var(primary_env).ok())
        .or_else(|| fallback_env.and_then(|env| std::env::var(env).ok()))
        .filter(|q| !q.trim().is_empty())
}

/// Format search results as context output.
///
/// Converts MCP search_graph results to markdown for context injection.
/// Respects token budget by estimating ~4 chars per token.
fn format_search_results(
    results: &serde_json::Value,
    budget: u32,
    session_id: &str,
) -> (String, usize, u32) {
    let mut output = String::new();
    let mut memory_count = 0usize;
    let max_chars = (budget * CHARS_PER_TOKEN) as usize;

    // Get results array from either "results" or "memories" key
    let results_array = results
        .get("results")
        .or_else(|| results.get("memories"))
        .and_then(|v| v.as_array());

    if let Some(memories) = results_array {
        let mut header_added = false;

        for (i, memory) in memories.iter().enumerate() {
            let content = memory
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if content.is_empty() {
                continue;
            }

            // Check budget (with overhead for formatting)
            let content_chars = content.len() + 50;
            if output.len() + content_chars > max_chars {
                debug!(
                    memory_count,
                    budget, "Budget exhausted, stopping at {} memories", memory_count
                );
                break;
            }

            // Extract metadata
            let similarity = memory
                .get("similarity")
                .or_else(|| memory.get("score"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let id = memory.get("id").and_then(|v| v.as_str()).unwrap_or("?");

            // Add header on first memory
            if !header_added {
                output.push_str("## Relevant Context\n\n");
                header_added = true;
            }

            // Format memory entry
            output.push_str(&format!(
                "### Memory {} (similarity: {:.2})\n",
                i + 1,
                similarity
            ));
            output.push_str(&format!("ID: {}\n\n", id));
            output.push_str(content);
            output.push_str("\n\n---\n\n");

            memory_count += 1;
        }
    }

    if memory_count == 0 {
        debug!(session_id, "No relevant memories found");
    }

    let tokens_used = (output.len() / (CHARS_PER_TOKEN as usize)) as u32;
    (output, memory_count, tokens_used)
}

// =============================================================================
// Command Handlers
// =============================================================================

/// Handle inject-context command.
///
/// Searches for relevant memories via MCP server and outputs formatted context.
/// Empty result = empty stdout (not an error).
///
/// # Exit Codes
/// - 0: Success (including empty result)
/// - 1: Error (MCP server not running, timeout, or tool failure)
/// - 2: Corruption (missing memory, stale index)
pub async fn handle_inject_context(args: InjectContextArgs) -> i32 {
    let query = resolve_query(args.query, "USER_PROMPT", None);

    let Some(query) = query else {
        debug!("No query provided, returning empty context");
        return CliExitCode::Success as i32;
    };

    let session_id = resolve_session_id(args.session_id);

    info!(
        query_len = query.len(),
        session_id = %session_id,
        budget = args.budget,
        top_k = args.top_k,
        "Injecting memory context via MCP"
    );

    let client = McpClient::new();
    if let Err(exit_code) = require_mcp_server(&client).await {
        return exit_code;
    }

    match client.search_graph(&query, Some(args.top_k)).await {
        Ok(results) => {
            let (output, memory_count, tokens_used) =
                format_search_results(&results, args.budget, &session_id);

            if memory_count > 0 {
                info!(
                    memories = memory_count,
                    tokens = tokens_used,
                    "Context generated via MCP"
                );
                print!("{}", output);
            } else {
                debug!("No relevant context found");
            }

            CliExitCode::Success as i32
        }
        Err(e) => {
            eprintln!("ERROR: Failed to search for context via MCP: {}", e);
            mcp_error_to_exit_code(&e)
        }
    }
}

/// Handle inject-brief command.
///
/// Generates brief context for PreToolUse hook (<200 tokens).
/// Reads query from TOOL_DESCRIPTION, then TOOL_NAME, then CLI arg.
///
/// # Exit Codes
/// - 0: Success (including empty result)
/// - 1: Error (MCP server not running, timeout, or tool failure)
/// - 2: Corruption (missing memory, stale index)
pub async fn handle_inject_brief(args: InjectBriefArgs) -> i32 {
    let query = resolve_query(args.query, "TOOL_DESCRIPTION", Some("TOOL_NAME"));

    let Some(query) = query else {
        debug!("No query provided, returning empty brief context");
        return CliExitCode::Success as i32;
    };

    let session_id = resolve_session_id(args.session_id);

    info!(
        query_len = query.len(),
        session_id = %session_id,
        top_k = args.top_k,
        "Generating brief context"
    );

    // Check memory cache first (populated by user_prompt_submit hook).
    // This gives subagents instant access to context without MCP round-trip.
    let cached = get_cached_memories(&session_id);
    if !cached.is_empty() {
        let max_chars = (BRIEF_BUDGET * CHARS_PER_TOKEN) as usize;
        let mut output = String::from("## Relevant Context\n\n");
        let mut count = 0usize;

        for (i, mem) in cached.iter().enumerate() {
            if mem.content.is_empty() {
                continue;
            }
            let entry = format!(
                "### Memory {} (similarity: {:.2})\n{}\n\n---\n\n",
                i + 1,
                mem.similarity,
                mem.content
            );
            if output.len() + entry.len() > max_chars {
                break;
            }
            output.push_str(&entry);
            count += 1;
        }

        if count > 0 {
            let tokens_used = output.len() / (CHARS_PER_TOKEN as usize);
            info!(
                memories = count,
                tokens = tokens_used,
                "Brief context from cache (no MCP round-trip)"
            );
            print!("{}", output);
            return CliExitCode::Success as i32;
        }
    }

    // Cache empty — fall back to MCP search
    debug!("Cache miss, falling back to MCP search");

    let client = McpClient::new();
    if let Err(exit_code) = require_mcp_server(&client).await {
        return exit_code;
    }

    match client.search_graph(&query, Some(args.top_k)).await {
        Ok(results) => {
            let (output, memory_count, tokens_used) =
                format_search_results(&results, BRIEF_BUDGET, &session_id);

            if memory_count > 0 {
                info!(
                    memories = memory_count,
                    tokens = tokens_used,
                    "Brief context generated via MCP"
                );
                print!("{}", output);
            } else {
                debug!("No relevant brief context found");
            }

            CliExitCode::Success as i32
        }
        Err(e) => {
            eprintln!("ERROR: Failed to search for brief context via MCP: {}", e);
            mcp_error_to_exit_code(&e)
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_utils::GLOBAL_IDENTITY_LOCK;
    use crate::mcp_client::McpClientError;

    // =========================================================================
    // Constant Tests
    // =========================================================================

    #[test]
    fn test_constants_defined() {
        assert_eq!(DEFAULT_CONTEXT_BUDGET, 1200);
        assert_eq!(BRIEF_BUDGET, 200);
        assert_eq!(CHARS_PER_TOKEN, 4);
    }

    // =========================================================================
    // Exit Code Tests
    // =========================================================================

    #[test]
    fn test_exit_code_values() {
        assert_eq!(CliExitCode::Success as i32, 0, "Success must be 0");
        assert_eq!(CliExitCode::Warning as i32, 1, "Warning/Error must be 1");
        assert_eq!(
            CliExitCode::Blocking as i32,
            2,
            "Blocking/Corruption must be 2"
        );
    }

    #[test]
    fn test_mcp_error_to_exit_code() {
        let server_not_running = McpClientError::ServerNotRunning {
            host: "127.0.0.1".to_string(),
            port: 3100,
            source: std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused"),
        };
        assert_eq!(mcp_error_to_exit_code(&server_not_running), 1);

        let timeout = McpClientError::ConnectionTimeout {
            host: "127.0.0.1".to_string(),
            port: 3100,
            timeout_ms: 5000,
        };
        assert_eq!(mcp_error_to_exit_code(&timeout), 1);

        let corruption_error = McpClientError::McpError {
            code: -32000,
            message: "stale index detected".to_string(),
        };
        assert_eq!(mcp_error_to_exit_code(&corruption_error), 2);

        let mcp_error = McpClientError::McpError {
            code: -32600,
            message: "Invalid request".to_string(),
        };
        assert_eq!(mcp_error_to_exit_code(&mcp_error), 1);
    }

    // =========================================================================
    // Format Search Results Tests
    // =========================================================================

    #[test]
    fn test_format_search_results_empty() {
        let empty_results = serde_json::json!({"results": []});
        let (output, count, tokens) = format_search_results(&empty_results, 1200, "test-session");
        assert!(output.is_empty());
        assert_eq!(count, 0);
        assert_eq!(tokens, 0);
    }

    #[test]
    fn test_format_search_results_with_memories() {
        let results = serde_json::json!({
            "results": [
                {"id": "abc123", "content": "Test memory content", "similarity": 0.85},
                {"id": "def456", "content": "Another memory", "similarity": 0.75}
            ]
        });
        let (output, count, tokens) = format_search_results(&results, 1200, "test-session");
        assert!(output.contains("Test memory content"));
        assert!(output.contains("Another memory"));
        assert!(output.contains("similarity: 0.85"));
        assert_eq!(count, 2);
        assert!(tokens > 0);
    }

    #[test]
    fn test_format_search_results_budget_limit() {
        let mut memories = Vec::new();
        for i in 0..100 {
            memories.push(serde_json::json!({
                "id": format!("id-{}", i),
                "content": "x".repeat(500),
                "similarity": 0.9 - (i as f64 * 0.01)
            }));
        }
        let results = serde_json::json!({"results": memories});

        let (output, count, _tokens) = format_search_results(&results, 200, "test-session");
        assert!(count < 100, "Budget should limit memory count");
        assert!(output.len() < 1000, "Output should be limited by budget");
    }

    #[test]
    fn test_format_search_results_alternate_key() {
        // Test "memories" key instead of "results"
        let results = serde_json::json!({
            "memories": [
                {"id": "abc123", "content": "Test content", "score": 0.9}
            ]
        });
        let (output, count, _) = format_search_results(&results, 1200, "test");
        assert_eq!(count, 1);
        assert!(output.contains("Test content"));
    }

    // =========================================================================
    // Query Resolution Tests
    // =========================================================================

    #[test]
    fn test_resolve_query_from_arg() {
        let query = resolve_query(Some("arg query".to_string()), "NOT_SET", None);
        assert_eq!(query, Some("arg query".to_string()));
    }

    #[test]
    fn test_resolve_query_from_env() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();
        std::env::set_var("TEST_QUERY_ENV", "env query");

        let query = resolve_query(None, "TEST_QUERY_ENV", None);
        assert_eq!(query, Some("env query".to_string()));

        std::env::remove_var("TEST_QUERY_ENV");
    }

    #[test]
    fn test_resolve_query_from_fallback() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();
        std::env::remove_var("TEST_PRIMARY");
        std::env::set_var("TEST_FALLBACK", "fallback query");

        let query = resolve_query(None, "TEST_PRIMARY", Some("TEST_FALLBACK"));
        assert_eq!(query, Some("fallback query".to_string()));

        std::env::remove_var("TEST_FALLBACK");
    }

    #[test]
    fn test_resolve_query_whitespace_is_none() {
        let query = resolve_query(Some("   ".to_string()), "NOT_SET", None);
        assert!(query.is_none());
    }

    // =========================================================================
    // Empty Query Tests (No MCP Server Required)
    // =========================================================================

    #[tokio::test]
    async fn test_inject_context_empty_query() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();
        std::env::remove_var("USER_PROMPT");

        let args = InjectContextArgs {
            query: None,
            session_id: Some("test".to_string()),
            budget: DEFAULT_CONTEXT_BUDGET,
            top_k: 10,
        };

        let exit_code = handle_inject_context(args).await;
        assert_eq!(exit_code, 0, "Empty query should return exit 0");
    }

    #[tokio::test]
    async fn test_inject_context_whitespace_query() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();
        std::env::remove_var("USER_PROMPT");

        let args = InjectContextArgs {
            query: Some("   \n\t  ".to_string()),
            session_id: Some("test".to_string()),
            budget: DEFAULT_CONTEXT_BUDGET,
            top_k: 10,
        };

        let exit_code = handle_inject_context(args).await;
        assert_eq!(exit_code, 0, "Whitespace query should return exit 0");
    }

    #[tokio::test]
    async fn test_inject_brief_empty_query() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();
        std::env::remove_var("TOOL_DESCRIPTION");
        std::env::remove_var("TOOL_NAME");

        let args = InjectBriefArgs {
            query: None,
            session_id: Some("test".to_string()),
            top_k: 5,
        };

        let exit_code = handle_inject_brief(args).await;
        assert_eq!(exit_code, 0, "Empty query should return exit 0");
    }

    // =========================================================================
    // Environment Variable Integration Tests
    // =========================================================================

    #[test]
    fn test_inject_context_env_fallback() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();
        std::env::set_var("USER_PROMPT", "test query from env");

        let query = resolve_query(None, "USER_PROMPT", None);
        assert_eq!(query, Some("test query from env".to_string()));

        std::env::remove_var("USER_PROMPT");
    }

    #[test]
    fn test_inject_context_arg_overrides_env() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();
        std::env::set_var("USER_PROMPT", "env query");

        let query = resolve_query(Some("arg query".to_string()), "USER_PROMPT", None);
        assert_eq!(query, Some("arg query".to_string()));

        std::env::remove_var("USER_PROMPT");
    }

    #[test]
    fn test_inject_brief_tool_description_env() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();
        std::env::set_var("TOOL_DESCRIPTION", "Running cargo test");
        std::env::remove_var("TOOL_NAME");

        let query = resolve_query(None, "TOOL_DESCRIPTION", Some("TOOL_NAME"));
        assert_eq!(query, Some("Running cargo test".to_string()));

        std::env::remove_var("TOOL_DESCRIPTION");
    }

    #[test]
    fn test_inject_brief_tool_name_fallback() {
        let _lock = GLOBAL_IDENTITY_LOCK.lock();
        std::env::remove_var("TOOL_DESCRIPTION");
        std::env::set_var("TOOL_NAME", "Bash");

        let query = resolve_query(None, "TOOL_DESCRIPTION", Some("TOOL_NAME"));
        assert_eq!(query, Some("Bash".to_string()));

        std::env::remove_var("TOOL_NAME");
    }
}
