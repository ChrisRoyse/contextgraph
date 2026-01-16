# TASK-HOOKS-004: Create CLI Argument Types for Hooks Commands

```xml
<task_spec id="TASK-HOOKS-004" version="2.0">
<metadata>
  <title>Create CLI Argument Types for Hooks Commands</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <requirement_ref>REQ-HOOKS-17</requirement_ref>
    <requirement_ref>REQ-HOOKS-18</requirement_ref>
    <requirement_ref>REQ-HOOKS-19</requirement_ref>
    <requirement_ref>REQ-HOOKS-20</requirement_ref>
    <requirement_ref>REQ-HOOKS-21</requirement_ref>
    <requirement_ref>REQ-HOOKS-22</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="COMPLETE">TASK-HOOKS-001</task_ref>
    <task_ref status="COMPLETE">TASK-HOOKS-002</task_ref>
    <task_ref status="COMPLETE">TASK-HOOKS-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_hours>1.0</estimated_hours>
</metadata>

<current_state_audit date="2026-01-15">
  <already_implemented>
    <file path="crates/context-graph-cli/src/commands/hooks/types.rs">
      <type name="HookEventType" status="COMPLETE">5 variants with timeout_ms(), is_fast_path(), cli_command(), all()</type>
      <type name="ICLevel" status="COMPLETE">4 variants with from_value(), is_crisis(), needs_attention()</type>
      <type name="JohariQuadrant" status="COMPLETE">4 variants with classify(), HIGH_THRESHOLD=0.7</type>
      <type name="ConsciousnessState" status="COMPLETE">6 fields including johari_quadrant</type>
      <type name="ICClassification" status="COMPLETE">DEFAULT_CRISIS_THRESHOLD=0.5, new(), from_value()</type>
      <type name="SessionEndStatus" status="COMPLETE">6 variants: Normal, Timeout, Error, UserAbort, Clear, Logout</type>
      <type name="ConversationMessage" status="COMPLETE">role + content fields</type>
      <type name="HookPayload" status="COMPLETE">5 typed variants with internally tagged JSON</type>
      <type name="HookInput" status="COMPLETE">hook_type, session_id, timestamp_ms, payload + validate()</type>
      <type name="HookOutput" status="COMPLETE">success, error, consciousness_state, ic_classification, context_injection, execution_time_ms + builders</type>
    </file>
    <file path="crates/context-graph-cli/src/commands/hooks/mod.rs">
      <status>COMPLETE - exports all types from types.rs</status>
    </file>
  </already_implemented>

  <not_yet_implemented>
    <file path="crates/context-graph-cli/src/commands/hooks/args.rs">
      <status>MISSING - THIS IS THE TARGET FILE FOR THIS TASK</status>
      <required_types>
        <type>HooksCommands enum (6 subcommands)</type>
        <type>SessionStartArgs struct</type>
        <type>PreToolArgs struct (fast_path flag)</type>
        <type>PostToolArgs struct</type>
        <type>PromptSubmitArgs struct</type>
        <type>SessionEndArgs struct</type>
        <type>GenerateConfigArgs struct</type>
        <type>OutputFormat enum (Json, JsonCompact, Text)</type>
        <type>HookType enum (for generate-config)</type>
        <type>ShellType enum</type>
      </required_types>
    </file>
  </not_yet_implemented>

  <compiler_warnings>
    <warning file="crates/context-graph-cli/src/commands/hooks/mod.rs" lines="20-29">
      unused imports: ConsciousnessState, ConversationMessage, HookEventType, HookInput, HookOutput, HookPayload, ICClassification, ICLevel, JohariQuadrant, SessionEndStatus
    </warning>
    <note>These warnings exist because the types are defined but not yet used - they will be used by args.rs handlers</note>
  </compiler_warnings>
</current_state_audit>

<constitution_references>
  <ref id="AP-26">Exit codes: 0=success, 1=error, 2=corruption</ref>
  <ref id="AP-50">NO internal hooks - use Claude Code native</ref>
  <ref id="AP-53">Hook logic in shell scripts calling CLI</ref>
  <ref id="IDENTITY-002">IC thresholds and timeout requirements</ref>
  <ref id="AP-25">Kuramoto N=13</ref>
</constitution_references>

<timeout_budget source="TECH-HOOKS.md Section 2.2">
  <hook name="PreToolUse" timeout_ms="100" category="FAST_PATH">NO DATABASE ACCESS</hook>
  <hook name="UserPromptSubmit" timeout_ms="2000" category="normal"/>
  <hook name="PostToolUse" timeout_ms="3000" category="normal"/>
  <hook name="SessionStart" timeout_ms="5000" category="normal"/>
  <hook name="SessionEnd" timeout_ms="30000" category="extended"/>
</timeout_budget>

<context>
This task creates the clap argument definitions for all hooks CLI subcommands.
These define the command-line interface that shell scripts use to invoke context-graph-cli.
Each hook command has specific arguments matching Claude Code's hook data.

The types.rs file already contains all the core types (HookEventType, HookInput, HookOutput, etc.).
This task creates the CLI argument structs that WRAP these types for command-line parsing.

Commands to be defined:
- hooks session-start: Initialize session identity
- hooks pre-tool: Fast path consciousness check (100ms timeout - NO DB ACCESS)
- hooks post-tool: Update IC after tool execution
- hooks prompt-submit: Inject context for user prompt
- hooks session-end: Persist final session state
- hooks generate-config: Generate hook configuration files
</context>

<input_context_files>
  <file purpose="types_already_implemented" MUST_READ="true">crates/context-graph-cli/src/commands/hooks/types.rs</file>
  <file purpose="module_exports">crates/context-graph-cli/src/commands/hooks/mod.rs</file>
  <file purpose="technical_spec">docs/specs/technical/TECH-HOOKS.md</file>
  <file purpose="claude_code_hooks_reference">docs2/claudehooks.md</file>
  <file purpose="constitution">docs2/constitution.yaml</file>
</input_context_files>

<prerequisites>
  <check status="PASS">TASK-HOOKS-001 completed (HookEventType exists in types.rs)</check>
  <check status="PASS">TASK-HOOKS-002 completed (HookInput/HookOutput exist in types.rs)</check>
  <check status="PASS">TASK-HOOKS-003 completed (HookPayload typed variants exist in types.rs)</check>
  <check status="VERIFY">clap is a workspace dependency (run: grep clap Cargo.toml)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create args.rs file with clap imports
    - Create HooksCommands enum with 6 subcommands
    - Create SessionStartArgs struct
    - Create PreToolArgs struct (with fast_path flag defaulting to true)
    - Create PostToolArgs struct
    - Create PromptSubmitArgs struct
    - Create SessionEndArgs struct
    - Create GenerateConfigArgs struct
    - Create OutputFormat enum (Json, JsonCompact, Text)
    - Create HookType enum (for generate-config - mirrors HookEventType)
    - Create ShellType enum (Bash, Zsh, Fish, Powershell)
    - Add mod args; and pub use args::*; to mod.rs
  </in_scope>
  <out_of_scope>
    - Command handler implementations (TASK-HOOKS-012 through 017)
    - Module registration with main CLI (TASK-HOOKS-011)
    - The core types in types.rs (already complete)
  </out_of_scope>
</scope>

<no_backwards_compatibility>
  <rule>FAIL FAST on any error - do not add fallback logic</rule>
  <rule>Do not add "Other" or "Unknown" variants to enums</rule>
  <rule>All argument parsing must use strict validation</rule>
  <rule>Exit code 2 for corruption per AP-26</rule>
</no_backwards_compatibility>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-cli/src/commands/hooks/args.rs">
use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Hook commands for Claude Code native integration
/// Constitution: AP-50, AP-53
#[derive(Subcommand, Debug)]
pub enum HooksCommands {
    #[command(name = "session-start")]
    SessionStart(SessionStartArgs),
    #[command(name = "pre-tool")]
    PreTool(PreToolArgs),
    #[command(name = "post-tool")]
    PostTool(PostToolArgs),
    #[command(name = "prompt-submit")]
    PromptSubmit(PromptSubmitArgs),
    #[command(name = "session-end")]
    SessionEnd(SessionEndArgs),
    #[command(name = "generate-config")]
    GenerateConfig(GenerateConfigArgs),
}

#[derive(Args, Debug)]
pub struct SessionStartArgs {
    #[arg(long, env = "CONTEXT_GRAPH_DB_PATH")]
    pub db_path: Option&lt;PathBuf&gt;,
    #[arg(long)]
    pub session_id: Option&lt;String&gt;,
    #[arg(long)]
    pub previous_session_id: Option&lt;String&gt;,
    #[arg(long, default_value = "false")]
    pub stdin: bool,
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

/// FAST PATH - 100ms timeout - NO DATABASE ACCESS
#[derive(Args, Debug)]
pub struct PreToolArgs {
    #[arg(long)]
    pub session_id: String,
    #[arg(long)]
    pub tool_name: Option&lt;String&gt;,
    #[arg(long, default_value = "false")]
    pub stdin: bool,
    #[arg(long, default_value = "true")]
    pub fast_path: bool,
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum OutputFormat {
    #[default]
    Json,
    JsonCompact,
    Text,
}
    </signature>
  </signatures>

  <constraints>
    - All commands MUST use kebab-case names (session-start, NOT session_start)
    - db_path MUST support CONTEXT_GRAPH_DB_PATH env variable
    - PreToolArgs MUST have fast_path defaulting to true (NO DB ACCESS per 100ms timeout)
    - stdin flag MUST default to false
    - format MUST default to json
    - All timeout values MUST match TECH-HOOKS.md (100, 2000, 3000, 5000, 30000 ms)
  </constraints>

  <verification>
    <command>cargo build --package context-graph-cli 2>&amp;1 | grep -E "(error|warning.*args)"</command>
    <command>cargo test --package context-graph-cli -- args --nocapture</command>
    <manual_check>Verify --help output for each subcommand shows correct defaults</manual_check>
  </verification>
</definition_of_done>

<full_state_verification>
  <source_of_truth>
    <primary>crates/context-graph-cli/src/commands/hooks/types.rs - Core types already implemented</primary>
    <primary>docs/specs/technical/TECH-HOOKS.md Section 2.2 - Timeout values</primary>
    <primary>docs2/constitution.yaml - AP-26 exit codes, IDENTITY-002 thresholds</primary>
    <secondary>docs2/claudehooks.md - Claude Code native hook JSON format</secondary>
  </source_of_truth>

  <execute_and_inspect>
    <step>1. Read types.rs to understand existing HookEventType.timeout_ms() values</step>
    <step>2. Create args.rs with all argument structs</step>
    <step>3. Run cargo build and verify no errors</step>
    <step>4. Run cargo test for argument parsing tests</step>
    <step>5. Manually verify --help shows correct defaults</step>
  </execute_and_inspect>

  <boundary_edge_cases count="3">
    <edge_case id="1" description="Empty session_id">
      PreToolArgs requires session_id - empty string should be accepted by clap but rejected by handler
    </edge_case>
    <edge_case id="2" description="fast_path=false with 100ms timeout">
      When fast_path=false on PreToolArgs, handler MUST still respect 100ms timeout but MAY access cache
    </edge_case>
    <edge_case id="3" description="Multiple hook types in generate-config">
      GenerateConfigArgs.hooks accepts comma-separated list via value_delimiter
    </edge_case>
  </boundary_edge_cases>

  <evidence_of_success>
    <log>cargo build --package context-graph-cli exits with code 0</log>
    <log>cargo test args shows all tests passing</log>
    <log>context-graph-cli hooks --help shows 6 subcommands</log>
    <log>context-graph-cli hooks pre-tool --help shows fast_path=true default</log>
  </evidence_of_success>
</full_state_verification>

<test_requirements>
  <rule>NO MOCK DATA - use real values from types.rs and constitution</rule>
  <rule>Test case IDs: TC-HOOKS-ARGS-001 through TC-HOOKS-ARGS-0XX</rule>
  <rule>Each test MUST print SOURCE OF TRUTH reference</rule>
  <rule>Each test MUST print RESULT: PASS or FAIL</rule>
</test_requirements>

<pseudo_code>
1. Create args.rs file with clap imports:
   use clap::{Args, Subcommand, ValueEnum};
   use std::path::PathBuf;

2. Create OutputFormat enum (for CLI output, different from HookOutput):
   - Json (default, pretty-printed)
   - JsonCompact (single line, no whitespace)
   - Text (human readable for debugging)

3. Create HookType enum for generate-config (mirrors HookEventType names):
   - SessionStart
   - PreToolUse
   - PostToolUse
   - UserPromptSubmit
   - SessionEnd
   Note: This is separate from HookEventType because clap ValueEnum needs different derive

4. Create ShellType enum:
   - Bash (default)
   - Zsh
   - Fish
   - Powershell

5. Create SessionStartArgs (timeout: 5000ms):
   - db_path: Option&lt;PathBuf&gt; with CONTEXT_GRAPH_DB_PATH env
   - session_id: Option&lt;String&gt; (auto-generated if not provided)
   - previous_session_id: Option&lt;String&gt; (for continuity linking)
   - stdin: bool (default false) - read HookInput from stdin
   - format: OutputFormat (default json)

6. Create PreToolArgs (timeout: 100ms - FAST PATH):
   - session_id: String (REQUIRED)
   - tool_name: Option&lt;String&gt;
   - stdin: bool (default false)
   - fast_path: bool (default TRUE - NO DATABASE ACCESS)
   - format: OutputFormat
   CRITICAL: fast_path=true means use IdentityCache only, no DB/disk access

7. Create PostToolArgs (timeout: 3000ms):
   - db_path: Option&lt;PathBuf&gt; with env
   - session_id: String (required)
   - tool_name: Option&lt;String&gt;
   - success: Option&lt;bool&gt; (tool execution result)
   - stdin: bool (default false)
   - format: OutputFormat

8. Create PromptSubmitArgs (timeout: 2000ms):
   - db_path: Option&lt;PathBuf&gt; with env
   - session_id: String (required)
   - prompt: Option&lt;String&gt; (can be passed via --prompt or stdin)
   - stdin: bool (default false)
   - format: OutputFormat

9. Create SessionEndArgs (timeout: 30000ms):
   - db_path: Option&lt;PathBuf&gt; with env
   - session_id: String (required)
   - duration_ms: Option&lt;u64&gt; (session duration)
   - stdin: bool (default false)
   - generate_summary: bool (default true)
   - format: OutputFormat

10. Create GenerateConfigArgs:
    - output_dir: PathBuf (default ".claude/hooks")
    - force: bool (default false - overwrite existing)
    - hooks: Option&lt;Vec&lt;HookType&gt;&gt; (comma-separated, all if not specified)
    - shell: ShellType (default bash)

11. Create HooksCommands enum with all 6 subcommands:
    - session-start -&gt; SessionStart(SessionStartArgs)
    - pre-tool -&gt; PreTool(PreToolArgs)
    - post-tool -&gt; PostTool(PostToolArgs)
    - prompt-submit -&gt; PromptSubmit(PromptSubmitArgs)
    - session-end -&gt; SessionEnd(SessionEndArgs)
    - generate-config -&gt; GenerateConfig(GenerateConfigArgs)

12. Add to mod.rs:
    mod args;
    pub use args::*;

13. Write tests in args.rs #[cfg(test)] module:
    - TC-HOOKS-ARGS-001: Parse session-start with all options
    - TC-HOOKS-ARGS-002: Parse pre-tool with default fast_path=true
    - TC-HOOKS-ARGS-003: Parse generate-config with multiple hooks
    - TC-HOOKS-ARGS-004: Verify OutputFormat default is Json
    - TC-HOOKS-ARGS-005: Verify env var CONTEXT_GRAPH_DB_PATH works
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-cli/src/commands/hooks/args.rs">
    CLI argument definitions for hooks commands
    Contains: HooksCommands, *Args structs, OutputFormat, HookType, ShellType
    Tests: TC-HOOKS-ARGS-* with NO MOCK DATA
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-cli/src/commands/hooks/mod.rs">
    Add: mod args; and pub use args::*;
    After types module declaration
  </file>
</files_to_modify>

<test_commands>
  <command description="Build CLI package">cargo build --package context-graph-cli</command>
  <command description="Run args tests">cargo test --package context-graph-cli -- args --nocapture</command>
  <command description="Verify help output">cargo run --package context-graph-cli -- hooks --help</command>
  <command description="Verify pre-tool defaults">cargo run --package context-graph-cli -- hooks pre-tool --help</command>
</test_commands>

<git_history_context>
  <recent_commits>
    <commit hash="f1b2080">feat(cli): implement HookInput/HookOutput types for Claude Code hooks (TASK-HOOKS-002)</commit>
    <commit hash="83611c4">feat(cli): implement HookEventType enum for Claude Code hooks (TASK-HOOKS-001)</commit>
    <commit hash="7433bef">docs: add HOOKS/SKILLS specs, remove completed SESSION tasks</commit>
  </recent_commits>
  <note>TASK-HOOKS-001, 002, 003 are complete. This task (004) creates the CLI argument layer.</note>
</git_history_context>
</task_spec>
```

## Implementation

### Create args.rs

```rust
// crates/context-graph-cli/src/commands/hooks/args.rs
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
//! # Timeout Budget (per TECH-HOOKS.md Section 2.2)
//! - PreToolUse: 100ms (FAST PATH - NO DB ACCESS)
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
    /// Pre-tool execution hook (timeout: 100ms - FAST PATH)
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
    /// Database path for session storage
    /// Reads from CONTEXT_GRAPH_DB_PATH environment variable if not provided
    #[arg(long, env = "CONTEXT_GRAPH_DB_PATH")]
    pub db_path: Option<PathBuf>,

    /// Session ID (auto-generated if not provided)
    #[arg(long)]
    pub session_id: Option<String>,

    /// Previous session ID for identity continuity linking
    #[arg(long)]
    pub previous_session_id: Option<String>,

    /// Read HookInput JSON from stdin
    #[arg(long, default_value = "false")]
    pub stdin: bool,

    /// Output format for response
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

// ============================================================================
// Pre-Tool Arguments (timeout: 100ms - FAST PATH)
// ============================================================================

/// Pre-tool command arguments (FAST PATH - 100ms timeout)
/// Implements REQ-HOOKS-18
///
/// # Performance Critical
/// This command MUST complete within 100ms.
/// When `fast_path` is true (default), NO database access occurs.
/// Uses IdentityCache only for consciousness brief.
#[derive(Args, Debug, Clone)]
pub struct PreToolArgs {
    /// Session ID (REQUIRED)
    #[arg(long)]
    pub session_id: String,

    /// Tool name being invoked (from Claude Code)
    #[arg(long)]
    pub tool_name: Option<String>,

    /// Read HookInput JSON from stdin
    #[arg(long, default_value = "false")]
    pub stdin: bool,

    /// Skip database access for faster response (default: true)
    /// When true, uses IdentityCache only - NO disk/DB access
    /// MUST remain true to meet 100ms timeout requirement
    #[arg(long, default_value = "true")]
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
    /// Database path
    #[arg(long, env = "CONTEXT_GRAPH_DB_PATH")]
    pub db_path: Option<PathBuf>,

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
    #[arg(long, default_value = "false")]
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
    /// Database path
    #[arg(long, env = "CONTEXT_GRAPH_DB_PATH")]
    pub db_path: Option<PathBuf>,

    /// Session ID (REQUIRED)
    #[arg(long)]
    pub session_id: String,

    /// User prompt text (alternative to stdin)
    #[arg(long)]
    pub prompt: Option<String>,

    /// Read HookInput JSON from stdin
    #[arg(long, default_value = "false")]
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
    /// Database path
    #[arg(long, env = "CONTEXT_GRAPH_DB_PATH")]
    pub db_path: Option<PathBuf>,

    /// Session ID (REQUIRED)
    #[arg(long)]
    pub session_id: String,

    /// Session duration in milliseconds
    #[arg(long)]
    pub duration_ms: Option<u64>,

    /// Read HookInput JSON from stdin
    #[arg(long, default_value = "false")]
    pub stdin: bool,

    /// Generate session summary on end
    #[arg(long, default_value = "true")]
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
    #[arg(long, default_value = "false")]
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
    /// Timeout: 100ms - NO DATABASE ACCESS
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
            "--session-id", "session-12345",
            "--previous-session-id", "prev-session-98765",
            "--stdin",
            "--format", "json-compact",
        ]);

        if let HooksCommands::SessionStart(args) = cli.command {
            assert_eq!(args.session_id, Some("session-12345".to_string()));
            assert_eq!(args.previous_session_id, Some("prev-session-98765".to_string()));
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
    // SOURCE OF TRUTH: TECH-HOOKS.md 100ms timeout, NO DB ACCESS
    // =========================================================================
    #[test]
    fn tc_hooks_args_002_pre_tool_defaults() {
        println!("\n=== TC-HOOKS-ARGS-002: Pre-Tool Default fast_path=true ===");
        println!("SOURCE: TECH-HOOKS.md 100ms timeout - MUST use cache only");

        let cli = TestCli::parse_from([
            "test",
            "pre-tool",
            "--session-id", "session-abc",
        ]);

        if let HooksCommands::PreTool(args) = cli.command {
            assert_eq!(args.session_id, "session-abc");
            assert!(args.fast_path, "FAIL: fast_path MUST default to true for 100ms timeout");
            assert!(!args.stdin, "FAIL: stdin MUST default to false");
            assert_eq!(args.format, OutputFormat::Json, "FAIL: format MUST default to json");
            println!("  session_id: {}", args.session_id);
            println!("  fast_path: {} (default=true for 100ms timeout)", args.fast_path);
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
            "--session-id", "session-xyz",
            "--fast-path", "false",
        ]);

        if let HooksCommands::PreTool(args) = cli.command {
            assert!(!args.fast_path, "fast_path should be false when explicitly set");
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
            "--session-id", "session-post",
            "--tool-name", "Read",
            "--success", "true",
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
            "--output-dir", "/custom/hooks",
            "--force",
            "--hooks", "session-start,pre-tool-use,session-end",
            "--shell", "zsh",
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

        let cli = TestCli::parse_from([
            "test",
            "session-end",
            "--session-id", "session-end-test",
        ]);

        if let HooksCommands::SessionEnd(args) = cli.command {
            assert_eq!(args.format, OutputFormat::Json, "FAIL: format MUST default to Json");
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
            assert_eq!(args.shell, ShellType::Bash, "FAIL: shell MUST default to Bash");
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
            "--session-id", "session-prompt",
            "--prompt", "Help me fix this bug",
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
}
```

### Update mod.rs

Add after the existing `mod types;` line:

```rust
mod args;

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
```

## Verification Checklist

- [ ] args.rs created with all structs and enums
- [ ] All 6 subcommands defined with kebab-case names
- [ ] SessionStartArgs has db_path with CONTEXT_GRAPH_DB_PATH env support
- [ ] PreToolArgs.fast_path defaults to true (for 100ms timeout)
- [ ] All args.stdin defaults to false
- [ ] All args.format defaults to json
- [ ] GenerateConfigArgs.output_dir defaults to ".claude/hooks"
- [ ] GenerateConfigArgs.hooks accepts comma-separated values
- [ ] mod.rs updated with mod args; and pub use args::*;
- [ ] cargo build --package context-graph-cli succeeds
- [ ] cargo test --package context-graph-cli -- args passes
- [ ] All tests print SOURCE OF TRUTH and RESULT

## Evidence of Success

After implementation, run these commands and capture output:

```bash
# Build verification
cargo build --package context-graph-cli 2>&1 | tee /tmp/hooks-args-build.log
echo "Exit code: $?"

# Test verification
cargo test --package context-graph-cli -- args --nocapture 2>&1 | tee /tmp/hooks-args-test.log

# Help output verification
cargo run --package context-graph-cli -- hooks --help
cargo run --package context-graph-cli -- hooks pre-tool --help
```
