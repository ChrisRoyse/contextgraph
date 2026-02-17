//! CLI argument definitions for hooks commands
//!
//! # Architecture
//! This module defines clap argument types that wrap the core types
//! from types.rs for command-line parsing.
//!
//! # Constitution References
//! - AP-26: Exit codes (0=success, 1=error, 2=corruption)
//! - AP-50: NO internal hooks (use Claude Code native)
//! - AP-53: Hook logic in shell scripts calling CLI
//!
//! # Timeout Budget (per constitution.yaml)
//! - PreToolUse: 500ms total (FAST PATH - NO DB ACCESS, CLI logic ~100ms)
//! - UserPromptSubmit: 2000ms
//! - PostToolUse: 3000ms
//! - SessionStart: 5000ms
//! - SessionEnd: 30000ms
//!
//! # NO BACKWARDS COMPATIBILITY - FAIL FAST

use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

// ============================================================================
// Output Format
// ============================================================================

/// Output format for hook responses
/// Different from HookOutput - this controls CLI output formatting
#[derive(Debug, Clone, Copy, ValueEnum, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// JSON format with pretty printing (default for hook integration)
    #[default]
    Json,
    /// Compact JSON (single line, minimal whitespace)
    JsonCompact,
    /// Human-readable text (for debugging)
    Text,
}

// ============================================================================
// Hook Type (for generate-config)
// ============================================================================

/// Hook types for configuration generation
/// Mirrors HookEventType but with clap ValueEnum derive
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum HookType {
    /// Session initialization hook (timeout: 5000ms)
    SessionStart,
    /// Pre-tool execution hook (timeout: 500ms total - FAST PATH)
    PreToolUse,
    /// Post-tool execution hook (timeout: 3000ms)
    PostToolUse,
    /// User prompt submission hook (timeout: 2000ms)
    UserPromptSubmit,
    /// Session termination hook (timeout: 30000ms)
    SessionEnd,
}

impl HookType {
    /// Get all hook types
    pub const fn all() -> [Self; 5] {
        [
            Self::SessionStart,
            Self::PreToolUse,
            Self::PostToolUse,
            Self::UserPromptSubmit,
            Self::SessionEnd,
        ]
    }
}

// ============================================================================
// Shell Type
// ============================================================================

/// Shell type for script generation
#[derive(Debug, Clone, Copy, ValueEnum, Default, PartialEq, Eq)]
pub enum ShellType {
    /// Bash shell (default, most common)
    #[default]
    Bash,
    /// Zsh shell
    Zsh,
    /// Fish shell
    Fish,
    /// PowerShell (Windows)
    Powershell,
}

// ============================================================================
// Session Start Arguments (timeout: 5000ms)
// ============================================================================

/// Session start command arguments
/// Implements REQ-HOOKS-17
/// Timeout: 5000ms per TECH-HOOKS.md
#[derive(Args, Debug, Clone)]
pub struct SessionStartArgs {
    // LOW-13: Removed dead `db_path` field — no handler reads it.
    // Session storage uses in-memory SessionCache per PRD v6 Section 14.

    /// Session ID (auto-generated if not provided)
    #[arg(long)]
    pub session_id: Option<String>,

    /// Previous session ID for session linking
    #[arg(long)]
    pub previous_session_id: Option<String>,

    /// Read HookInput JSON from stdin
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub stdin: bool,

    /// Output format for response
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

// ============================================================================
// Pre-Tool Arguments (timeout: 500ms total - FAST PATH)
// ============================================================================

/// Pre-tool command arguments (FAST PATH - 500ms total timeout)
/// Implements REQ-HOOKS-18
///
/// # Performance Critical
/// This command has a 500ms total budget (CLI logic ~100ms + process overhead ~300ms).
/// When `fast_path` is true (default), NO database access occurs.
/// Uses SessionCache only for coherence brief.
#[derive(Args, Debug, Clone)]
pub struct PreToolArgs {
    /// Session ID (REQUIRED)
    #[arg(long)]
    pub session_id: String,

    /// Tool name being invoked (from Claude Code)
    #[arg(long)]
    pub tool_name: Option<String>,

    /// Read HookInput JSON from stdin
    #[arg(long, action = clap::ArgAction::Set, default_value = "false")]
    pub stdin: bool,

    /// Skip database access for faster response (default: true)
    /// When true, uses SessionCache only - NO disk/DB access
    /// MUST remain true to meet 500ms timeout requirement
    #[arg(long, action = clap::ArgAction::Set, default_value = "true")]
    pub fast_path: bool,

    /// Output format for response
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

// ============================================================================
// Post-Tool Arguments (timeout: 3000ms)
// ============================================================================

/// Post-tool command arguments
/// Implements REQ-HOOKS-19
/// Timeout: 3000ms per TECH-HOOKS.md
#[derive(Args, Debug, Clone)]
pub struct PostToolArgs {
    // LOW-13: Removed dead `db_path` field — no handler reads it.

    /// Session ID (REQUIRED)
    #[arg(long)]
    pub session_id: String,

    /// Tool name that was executed
    #[arg(long)]
    pub tool_name: Option<String>,

    /// Whether tool execution succeeded
    #[arg(long)]
    pub success: Option<bool>,

    /// Read HookInput JSON from stdin
    #[arg(long, action = clap::ArgAction::Set, default_value = "false")]
    pub stdin: bool,

    /// Output format for response
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

// ============================================================================
// Prompt Submit Arguments (timeout: 2000ms)
// ============================================================================

/// Prompt submit command arguments
/// Implements REQ-HOOKS-20
/// Timeout: 2000ms per TECH-HOOKS.md
#[derive(Args, Debug, Clone)]
pub struct PromptSubmitArgs {
    // LOW-13: Removed dead `db_path` field — no handler reads it.

    /// Session ID (REQUIRED)
    #[arg(long)]
    pub session_id: String,

    /// User prompt text (alternative to stdin)
    #[arg(long)]
    pub prompt: Option<String>,

    /// Read HookInput JSON from stdin
    #[arg(long, action = clap::ArgAction::Set, default_value = "false")]
    pub stdin: bool,

    /// Output format for response
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

// ============================================================================
// Session End Arguments (timeout: 30000ms)
// ============================================================================

/// Session end command arguments
/// Implements REQ-HOOKS-21
/// Timeout: 30000ms per TECH-HOOKS.md
#[derive(Args, Debug, Clone)]
pub struct SessionEndArgs {
    // LOW-13: Removed dead `db_path` field — no handler reads it.

    /// Session ID (REQUIRED)
    #[arg(long)]
    pub session_id: String,

    /// Session duration in milliseconds
    #[arg(long)]
    pub duration_ms: Option<u64>,

    /// Read HookInput JSON from stdin
    #[arg(long, action = clap::ArgAction::Set, default_value = "false")]
    pub stdin: bool,

    /// Generate session summary on end
    #[arg(long, action = clap::ArgAction::Set, default_value = "true")]
    pub generate_summary: bool,

    /// Output format for response
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

// ============================================================================
// Generate Config Arguments
// ============================================================================

/// Generate config command arguments
/// Implements REQ-HOOKS-22
/// Creates .claude/hooks/*.sh scripts for Claude Code integration
#[derive(Args, Debug, Clone)]
pub struct GenerateConfigArgs {
    /// Output directory for hook scripts
    #[arg(long, default_value = ".claude/hooks")]
    pub output_dir: PathBuf,

    /// Overwrite existing files
    #[arg(long, action = clap::ArgAction::Set, default_value = "false")]
    pub force: bool,

    /// Hook types to generate (all if not specified)
    /// Comma-separated list: session-start,pre-tool-use,post-tool-use,user-prompt-submit,session-end
    #[arg(long, value_delimiter = ',')]
    pub hooks: Option<Vec<HookType>>,

    /// Shell to target for script generation
    #[arg(long, value_enum, default_value = "bash")]
    pub shell: ShellType,
}

// ============================================================================
// Pre-Compact Arguments (timeout: 20000ms)
// ============================================================================

/// Pre-compact command arguments
/// R5: Captures session summary before context window compression.
/// Timeout: 20000ms per settings.json
#[derive(Args, Debug, Clone)]
pub struct PreCompactArgs {
    /// Session ID (REQUIRED)
    #[arg(long)]
    pub session_id: String,

    /// Read HookInput JSON from stdin
    #[arg(long, action = clap::ArgAction::Set, default_value = "false")]
    pub stdin: bool,

    /// Output format for response
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

// ============================================================================
// Task Completed Arguments (timeout: 20000ms)
// ============================================================================

/// Task completed command arguments
/// R6: Extracts learnings from completed tasks.
/// Timeout: 20000ms per settings.json
#[derive(Args, Debug, Clone)]
pub struct TaskCompletedArgs {
    /// Session ID (REQUIRED)
    #[arg(long)]
    pub session_id: String,

    /// Read HookInput JSON from stdin
    #[arg(long, action = clap::ArgAction::Set, default_value = "false")]
    pub stdin: bool,

    /// Output format for response
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

// ============================================================================
// Hooks Commands Enum
// ============================================================================

/// Hook commands for Claude Code native integration
/// Constitution: AP-50 (NO internal hooks), AP-53 (shell scripts calling CLI)
/// Implements REQ-HOOKS-17 through REQ-HOOKS-22
#[derive(Subcommand, Debug, Clone)]
pub enum HooksCommands {
    /// Handle session start event
    /// Timeout: 5000ms - Initializes session identity
    /// CLI: context-graph-cli hooks session-start
    #[command(name = "session-start")]
    SessionStart(SessionStartArgs),

    /// Handle pre-tool-use event (FAST PATH)
    /// Timeout: 500ms total - NO DATABASE ACCESS
    /// CLI: context-graph-cli hooks pre-tool
    #[command(name = "pre-tool")]
    PreTool(PreToolArgs),

    /// Handle post-tool-use event
    /// Timeout: 3000ms - Updates IC and trajectory
    /// CLI: context-graph-cli hooks post-tool
    #[command(name = "post-tool")]
    PostTool(PostToolArgs),

    /// Handle user prompt submit event
    /// Timeout: 2000ms - Injects context
    /// CLI: context-graph-cli hooks prompt-submit
    #[command(name = "prompt-submit")]
    PromptSubmit(PromptSubmitArgs),

    /// Handle session end event
    /// Timeout: 30000ms - Persists final state
    /// CLI: context-graph-cli hooks session-end
    #[command(name = "session-end")]
    SessionEnd(SessionEndArgs),

    /// R5: Handle pre-compact event (saves context before compression)
    /// Timeout: 20000ms - Stores session summary
    /// CLI: context-graph-cli hooks pre-compact
    #[command(name = "pre-compact")]
    PreCompact(PreCompactArgs),

    /// R6: Handle task-completed event (extracts learnings)
    /// Timeout: 20000ms - Stores task summary
    /// CLI: context-graph-cli hooks task-completed
    #[command(name = "task-completed")]
    TaskCompleted(TaskCompletedArgs),

    /// Generate hook configuration files
    /// Creates shell scripts for .claude/hooks/
    #[command(name = "generate-config")]
    GenerateConfig(GenerateConfigArgs),
}

// ============================================================================
// Tests - NO MOCK DATA - REAL VALUES ONLY
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // Test CLI wrapper for parsing
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: HooksCommands,
    }

    // =========================================================================
    // TC-HOOKS-ARGS-001: Session Start Argument Parsing
    // SOURCE OF TRUTH: REQ-HOOKS-17, TECH-HOOKS.md timeout 5000ms
    // =========================================================================
    #[test]
    fn tc_hooks_args_001_session_start_parsing() {
        println!("\n=== TC-HOOKS-ARGS-001: Session Start Argument Parsing ===");
        println!("SOURCE: REQ-HOOKS-17, TECH-HOOKS.md timeout=5000ms");

        let cli = TestCli::parse_from([
            "test",
            "session-start",
            "--session-id",
            "session-12345",
            "--previous-session-id",
            "prev-session-98765",
            "--stdin",
            "--format",
            "json-compact",
        ]);

        if let HooksCommands::SessionStart(args) = cli.command {
            assert_eq!(args.session_id, Some("session-12345".to_string()));
            assert_eq!(
                args.previous_session_id,
                Some("prev-session-98765".to_string())
            );
            assert!(args.stdin);
            assert_eq!(args.format, OutputFormat::JsonCompact);
            println!("  session_id: {:?}", args.session_id);
            println!("  previous_session_id: {:?}", args.previous_session_id);
            println!("  stdin: {}", args.stdin);
            println!("  format: {:?}", args.format);
        } else {
            panic!("Expected SessionStart command");
        }

        println!("RESULT: PASS - SessionStart arguments parse correctly");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-002: Pre-Tool Default fast_path=true
    // SOURCE OF TRUTH: constitution.yaml 500ms timeout, NO DB ACCESS
    // =========================================================================
    #[test]
    fn tc_hooks_args_002_pre_tool_defaults() {
        println!("\n=== TC-HOOKS-ARGS-002: Pre-Tool Default fast_path=true ===");
        println!("SOURCE: constitution.yaml 500ms total timeout - MUST use cache only");

        let cli = TestCli::parse_from(["test", "pre-tool", "--session-id", "session-abc"]);

        if let HooksCommands::PreTool(args) = cli.command {
            assert_eq!(args.session_id, "session-abc");
            assert!(
                args.fast_path,
                "FAIL: fast_path MUST default to true for 500ms timeout"
            );
            assert!(!args.stdin, "FAIL: stdin MUST default to false");
            assert_eq!(
                args.format,
                OutputFormat::Json,
                "FAIL: format MUST default to json"
            );
            println!("  session_id: {}", args.session_id);
            println!(
                "  fast_path: {} (default=true for 500ms timeout)",
                args.fast_path
            );
            println!("  stdin: {} (default=false)", args.stdin);
            println!("  format: {:?} (default=json)", args.format);
        } else {
            panic!("Expected PreTool command");
        }

        println!("RESULT: PASS - PreTool defaults are correct for fast path");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-003: Pre-Tool fast_path=false Override
    // SOURCE OF TRUTH: TECH-HOOKS.md
    // =========================================================================
    #[test]
    fn tc_hooks_args_003_pre_tool_fast_path_override() {
        println!("\n=== TC-HOOKS-ARGS-003: Pre-Tool fast_path=false Override ===");

        let cli = TestCli::parse_from([
            "test",
            "pre-tool",
            "--session-id",
            "session-xyz",
            "--fast-path",
            "false",
        ]);

        if let HooksCommands::PreTool(args) = cli.command {
            assert!(
                !args.fast_path,
                "fast_path should be false when explicitly set"
            );
            println!("  fast_path: {} (explicitly set to false)", args.fast_path);
        } else {
            panic!("Expected PreTool command");
        }

        println!("RESULT: PASS - fast_path can be overridden to false");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-004: Post-Tool Arguments
    // SOURCE OF TRUTH: REQ-HOOKS-19, TECH-HOOKS.md timeout 3000ms
    // =========================================================================
    #[test]
    fn tc_hooks_args_004_post_tool_parsing() {
        println!("\n=== TC-HOOKS-ARGS-004: Post-Tool Arguments ===");
        println!("SOURCE: REQ-HOOKS-19, TECH-HOOKS.md timeout=3000ms");

        let cli = TestCli::parse_from([
            "test",
            "post-tool",
            "--session-id",
            "session-post",
            "--tool-name",
            "Read",
            "--success",
            "true",
        ]);

        if let HooksCommands::PostTool(args) = cli.command {
            assert_eq!(args.session_id, "session-post");
            assert_eq!(args.tool_name, Some("Read".to_string()));
            assert_eq!(args.success, Some(true));
            println!("  session_id: {}", args.session_id);
            println!("  tool_name: {:?}", args.tool_name);
            println!("  success: {:?}", args.success);
        } else {
            panic!("Expected PostTool command");
        }

        println!("RESULT: PASS - PostTool arguments parse correctly");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-005: Generate Config with Multiple Hooks
    // SOURCE OF TRUTH: REQ-HOOKS-22
    // =========================================================================
    #[test]
    fn tc_hooks_args_005_generate_config_multiple_hooks() {
        println!("\n=== TC-HOOKS-ARGS-005: Generate Config with Multiple Hooks ===");
        println!("SOURCE: REQ-HOOKS-22");

        let cli = TestCli::parse_from([
            "test",
            "generate-config",
            "--output-dir",
            "/custom/hooks",
            "--force",
            "true",
            "--hooks",
            "session-start,pre-tool-use,session-end",
            "--shell",
            "zsh",
        ]);

        if let HooksCommands::GenerateConfig(args) = cli.command {
            assert_eq!(args.output_dir, PathBuf::from("/custom/hooks"));
            assert!(args.force);
            let hooks = args.hooks.expect("hooks should be Some");
            assert_eq!(hooks.len(), 3);
            assert!(hooks.contains(&HookType::SessionStart));
            assert!(hooks.contains(&HookType::PreToolUse));
            assert!(hooks.contains(&HookType::SessionEnd));
            assert_eq!(args.shell, ShellType::Zsh);
            println!("  output_dir: {:?}", args.output_dir);
            println!("  force: {}", args.force);
            println!("  hooks: {:?}", hooks);
            println!("  shell: {:?}", args.shell);
        } else {
            panic!("Expected GenerateConfig command");
        }

        println!("RESULT: PASS - GenerateConfig parses comma-separated hooks");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-006: OutputFormat Default is Json
    // SOURCE OF TRUTH: Hook integration requires JSON
    // =========================================================================
    #[test]
    fn tc_hooks_args_006_output_format_default() {
        println!("\n=== TC-HOOKS-ARGS-006: OutputFormat Default is Json ===");
        println!("SOURCE: Claude Code hook integration requires JSON");

        let cli = TestCli::parse_from(["test", "session-end", "--session-id", "session-end-test"]);

        if let HooksCommands::SessionEnd(args) = cli.command {
            assert_eq!(
                args.format,
                OutputFormat::Json,
                "FAIL: format MUST default to Json"
            );
            println!("  format: {:?} (default)", args.format);
        } else {
            panic!("Expected SessionEnd command");
        }

        println!("RESULT: PASS - OutputFormat defaults to Json");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-007: All Subcommands Exist
    // SOURCE OF TRUTH: REQ-HOOKS-17 through REQ-HOOKS-22
    // =========================================================================
    #[test]
    fn tc_hooks_args_007_all_subcommands_exist() {
        println!("\n=== TC-HOOKS-ARGS-007: All 6 Subcommands Exist ===");
        println!("SOURCE: REQ-HOOKS-17 through REQ-HOOKS-22");

        let commands = [
            ("session-start", "--session-id", "s1"),
            ("pre-tool", "--session-id", "s2"),
            ("post-tool", "--session-id", "s3"),
            ("prompt-submit", "--session-id", "s4"),
            ("session-end", "--session-id", "s5"),
        ];

        for (cmd_name, flag, value) in commands {
            let result = TestCli::try_parse_from(["test", cmd_name, flag, value]);
            assert!(result.is_ok(), "FAIL: {} command MUST exist", cmd_name);
            println!("  {}: exists", cmd_name);
        }

        // generate-config doesn't require session-id
        let result = TestCli::try_parse_from(["test", "generate-config"]);
        assert!(result.is_ok(), "FAIL: generate-config command MUST exist");
        println!("  generate-config: exists");

        println!("RESULT: PASS - All 6 subcommands exist");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-008: HookType All Variants
    // SOURCE OF TRUTH: Matches HookEventType from types.rs
    // =========================================================================
    #[test]
    fn tc_hooks_args_008_hook_type_all_variants() {
        println!("\n=== TC-HOOKS-ARGS-008: HookType All Variants ===");
        println!("SOURCE: Must match HookEventType from types.rs");

        let all = HookType::all();
        assert_eq!(all.len(), 5, "FAIL: Must have exactly 5 hook types");

        let expected = [
            HookType::SessionStart,
            HookType::PreToolUse,
            HookType::PostToolUse,
            HookType::UserPromptSubmit,
            HookType::SessionEnd,
        ];

        for (i, hook) in all.iter().enumerate() {
            assert_eq!(*hook, expected[i]);
            println!("  {:?}", hook);
        }

        println!("RESULT: PASS - All 5 hook types exist");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-009: ShellType Default is Bash
    // =========================================================================
    #[test]
    fn tc_hooks_args_009_shell_type_default() {
        println!("\n=== TC-HOOKS-ARGS-009: ShellType Default is Bash ===");

        let cli = TestCli::parse_from(["test", "generate-config"]);

        if let HooksCommands::GenerateConfig(args) = cli.command {
            assert_eq!(
                args.shell,
                ShellType::Bash,
                "FAIL: shell MUST default to Bash"
            );
            println!("  shell: {:?} (default)", args.shell);
        } else {
            panic!("Expected GenerateConfig command");
        }

        println!("RESULT: PASS - ShellType defaults to Bash");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-010: Prompt Submit Arguments
    // SOURCE OF TRUTH: REQ-HOOKS-20, TECH-HOOKS.md timeout 2000ms
    // =========================================================================
    #[test]
    fn tc_hooks_args_010_prompt_submit_parsing() {
        println!("\n=== TC-HOOKS-ARGS-010: Prompt Submit Arguments ===");
        println!("SOURCE: REQ-HOOKS-20, TECH-HOOKS.md timeout=2000ms");

        let cli = TestCli::parse_from([
            "test",
            "prompt-submit",
            "--session-id",
            "session-prompt",
            "--prompt",
            "Help me fix this bug",
        ]);

        if let HooksCommands::PromptSubmit(args) = cli.command {
            assert_eq!(args.session_id, "session-prompt");
            assert_eq!(args.prompt, Some("Help me fix this bug".to_string()));
            println!("  session_id: {}", args.session_id);
            println!("  prompt: {:?}", args.prompt);
        } else {
            panic!("Expected PromptSubmit command");
        }

        println!("RESULT: PASS - PromptSubmit arguments parse correctly");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-011: Session End with generate_summary=true Default
    // SOURCE OF TRUTH: REQ-HOOKS-21
    // =========================================================================
    #[test]
    fn tc_hooks_args_011_session_end_generate_summary_default() {
        println!("\n=== TC-HOOKS-ARGS-011: Session End generate_summary Default ===");
        println!("SOURCE: REQ-HOOKS-21");

        let cli =
            TestCli::parse_from(["test", "session-end", "--session-id", "session-end-summary"]);

        if let HooksCommands::SessionEnd(args) = cli.command {
            assert!(
                args.generate_summary,
                "FAIL: generate_summary MUST default to true"
            );
            println!(
                "  generate_summary: {} (default=true)",
                args.generate_summary
            );
        } else {
            panic!("Expected SessionEnd command");
        }

        println!("RESULT: PASS - generate_summary defaults to true");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-012: Session End with duration_ms
    // SOURCE OF TRUTH: REQ-HOOKS-21
    // =========================================================================
    #[test]
    fn tc_hooks_args_012_session_end_duration() {
        println!("\n=== TC-HOOKS-ARGS-012: Session End with duration_ms ===");
        println!("SOURCE: REQ-HOOKS-21, TECH-HOOKS.md timeout=30000ms");

        let cli = TestCli::parse_from([
            "test",
            "session-end",
            "--session-id",
            "session-end-duration",
            "--duration-ms",
            "3600000",
            "--generate-summary",
            "false",
        ]);

        if let HooksCommands::SessionEnd(args) = cli.command {
            assert_eq!(args.session_id, "session-end-duration");
            assert_eq!(args.duration_ms, Some(3600000));
            assert!(
                !args.generate_summary,
                "generate_summary should be false when explicitly set"
            );
            println!("  session_id: {}", args.session_id);
            println!("  duration_ms: {:?}", args.duration_ms);
            println!("  generate_summary: {}", args.generate_summary);
        } else {
            panic!("Expected SessionEnd command");
        }

        println!("RESULT: PASS - SessionEnd parses duration_ms correctly");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-013: GenerateConfig output_dir Default
    // SOURCE OF TRUTH: REQ-HOOKS-22
    // =========================================================================
    #[test]
    fn tc_hooks_args_013_generate_config_output_dir_default() {
        println!("\n=== TC-HOOKS-ARGS-013: GenerateConfig output_dir Default ===");
        println!("SOURCE: REQ-HOOKS-22");

        let cli = TestCli::parse_from(["test", "generate-config"]);

        if let HooksCommands::GenerateConfig(args) = cli.command {
            assert_eq!(
                args.output_dir,
                PathBuf::from(".claude/hooks"),
                "FAIL: output_dir MUST default to .claude/hooks"
            );
            assert!(!args.force, "FAIL: force MUST default to false");
            println!(
                "  output_dir: {:?} (default=.claude/hooks)",
                args.output_dir
            );
            println!("  force: {} (default=false)", args.force);
        } else {
            panic!("Expected GenerateConfig command");
        }

        println!("RESULT: PASS - GenerateConfig defaults are correct");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-014: PreTool Missing session_id Fails
    // SOURCE OF TRUTH: session_id is REQUIRED for PreTool
    // =========================================================================
    #[test]
    fn tc_hooks_args_014_pre_tool_missing_session_id() {
        println!("\n=== TC-HOOKS-ARGS-014: PreTool Missing session_id Fails ===");
        println!("SOURCE: PreToolArgs.session_id is REQUIRED");

        let result = TestCli::try_parse_from(["test", "pre-tool"]);
        assert!(
            result.is_err(),
            "FAIL: pre-tool without session_id MUST fail"
        );
        println!("  pre-tool without --session-id: Err (expected)");

        println!("RESULT: PASS - Missing session_id fails fast");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-015: All Shell Types Parse
    // SOURCE OF TRUTH: ShellType enum
    // =========================================================================
    #[test]
    fn tc_hooks_args_015_all_shell_types() {
        println!("\n=== TC-HOOKS-ARGS-015: All Shell Types Parse ===");

        let shells = ["bash", "zsh", "fish", "powershell"];
        let expected = [
            ShellType::Bash,
            ShellType::Zsh,
            ShellType::Fish,
            ShellType::Powershell,
        ];

        for (shell_str, expected_shell) in shells.iter().zip(expected.iter()) {
            let cli = TestCli::parse_from(["test", "generate-config", "--shell", shell_str]);

            if let HooksCommands::GenerateConfig(args) = cli.command {
                assert_eq!(
                    args.shell, *expected_shell,
                    "FAIL: shell {} must parse to {:?}",
                    shell_str, expected_shell
                );
                println!("  {} -> {:?}", shell_str, args.shell);
            } else {
                panic!("Expected GenerateConfig command");
            }
        }

        println!("RESULT: PASS - All shell types parse correctly");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-016: All Output Formats Parse
    // SOURCE OF TRUTH: OutputFormat enum
    // =========================================================================
    #[test]
    fn tc_hooks_args_016_all_output_formats() {
        println!("\n=== TC-HOOKS-ARGS-016: All Output Formats Parse ===");

        let formats = ["json", "json-compact", "text"];
        let expected = [
            OutputFormat::Json,
            OutputFormat::JsonCompact,
            OutputFormat::Text,
        ];

        for (format_str, expected_format) in formats.iter().zip(expected.iter()) {
            let cli = TestCli::parse_from(["test", "session-start", "--format", format_str]);

            if let HooksCommands::SessionStart(args) = cli.command {
                assert_eq!(
                    args.format, *expected_format,
                    "FAIL: format {} must parse to {:?}",
                    format_str, expected_format
                );
                println!("  {} -> {:?}", format_str, args.format);
            } else {
                panic!("Expected SessionStart command");
            }
        }

        println!("RESULT: PASS - All output formats parse correctly");
    }

    // =========================================================================
    // TC-HOOKS-ARGS-017: PostTool success=false Parses
    // SOURCE OF TRUTH: REQ-HOOKS-19
    // =========================================================================
    #[test]
    fn tc_hooks_args_017_post_tool_success_false() {
        println!("\n=== TC-HOOKS-ARGS-017: PostTool success=false Parses ===");

        let cli = TestCli::parse_from([
            "test",
            "post-tool",
            "--session-id",
            "session-fail",
            "--success",
            "false",
        ]);

        if let HooksCommands::PostTool(args) = cli.command {
            assert_eq!(
                args.success,
                Some(false),
                "FAIL: success=false must parse correctly"
            );
            println!("  success: {:?}", args.success);
        } else {
            panic!("Expected PostTool command");
        }

        println!("RESULT: PASS - PostTool success=false parses correctly");
    }
}
