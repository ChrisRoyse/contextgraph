//! Setup command for initializing context-graph hooks for Claude Code.
//!
//! This command creates `.claude/settings.json` and `.claude/hooks/` directory
//! with all required hook scripts for context-graph integration.
//!
//! # Constitution Reference
//! - ARCH-07: Native Claude Code hooks via .claude/settings.json
//! - AP-14: No .unwrap() - use map_err, ok_or
//! - AP-26: Exit codes: 0=success, 1=error
//! - AP-50: Native hooks via settings.json ONLY
//! - AP-53: Hook logic in shell scripts calling CLI
//!
//! # Usage
//! ```bash
//! context-graph-cli setup           # Setup in current directory
//! context-graph-cli setup --force   # Overwrite existing configuration
//! context-graph-cli setup --target-dir /path/to/project
//! ```

use clap::Args;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info};

/// Log error and print to stderr, returning exit code 1.
fn fail(context: &str, err: impl std::fmt::Display) -> i32 {
    error!("{}: {}", context, err);
    eprintln!("Error: {}: {}", context, err);
    1
}

/// Arguments for the setup command.
#[derive(Args, Debug)]
pub struct SetupArgs {
    /// Force overwrite existing configuration.
    #[arg(long, short = 'f')]
    pub force: bool,

    /// Target directory (default: current working directory).
    #[arg(long)]
    pub target_dir: Option<PathBuf>,

    /// Skip making scripts executable (for testing on non-Unix systems).
    #[arg(long, hide = true)]
    pub skip_chmod: bool,
}

/// Handle the setup command.
///
/// Creates .claude/settings.json and .claude/hooks/ with all required scripts.
///
/// # Returns
/// - 0: Success
/// - 1: Error (hooks already configured without --force, write failure, etc.)
pub async fn handle_setup(args: SetupArgs) -> i32 {
    let target_dir = match args.target_dir {
        Some(dir) => dir,
        None => match std::env::current_dir() {
            Ok(cwd) => cwd,
            Err(e) => return fail("Failed to get current working directory", e),
        },
    };

    info!("Setting up context-graph hooks in: {:?}", target_dir);

    if !target_dir.exists() {
        error!("Target directory does not exist: {:?}", target_dir);
        eprintln!("Error: Target directory does not exist: {:?}", target_dir);
        return 1;
    }

    let claude_dir = target_dir.join(".claude");
    let hooks_dir = claude_dir.join("hooks");
    let settings_path = claude_dir.join("settings.json");

    // Check if settings.json exists and has hooks key (block without --force)
    if settings_path.exists() && !args.force {
        if let Err(code) = check_existing_hooks(&settings_path) {
            return code;
        }
    }

    // Create directories
    if let Err(e) = fs::create_dir_all(&hooks_dir) {
        return fail("Failed to create hooks directory", e);
    }
    info!("Ensured directories exist: {:?}", hooks_dir);

    // Write settings.json (merge with existing if present)
    if let Err(code) = write_settings_json(&settings_path) {
        return code;
    }

    // Write all hook scripts
    let scripts: &[(&str, &str)] = &[
        ("session_start.sh", SESSION_START_SCRIPT),
        ("pre_tool_use.sh", PRE_TOOL_USE_SCRIPT),
        ("post_tool_use.sh", POST_TOOL_USE_SCRIPT),
        ("user_prompt_submit.sh", USER_PROMPT_SUBMIT_SCRIPT),
        ("session_end.sh", SESSION_END_SCRIPT),
    ];

    for (name, content) in scripts {
        let script_path = hooks_dir.join(name);
        if let Err(e) = fs::write(&script_path, content) {
            return fail(&format!("Failed to write {}", name), e);
        }
        info!("Created script: {:?}", script_path);

        #[cfg(unix)]
        if !args.skip_chmod {
            use std::os::unix::fs::PermissionsExt;
            if let Err(e) = fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)) {
                return fail(&format!("Failed to set permissions on {}", name), e);
            }
        }
    }

    // Print success summary
    println!("Context-graph hooks configured successfully!");
    println!();
    println!("Created files:");
    println!("  {}", settings_path.display());
    for (name, _) in scripts {
        println!("  {}", hooks_dir.join(name).display());
    }
    println!();
    println!("The hooks are now active for Claude Code sessions in this directory.");

    0
}

/// Check if existing settings.json has hooks configured.
/// Returns Err(1) if hooks exist and should block setup.
fn check_existing_hooks(settings_path: &Path) -> Result<(), i32> {
    let content = fs::read_to_string(settings_path)
        .map_err(|e| fail("Failed to read existing settings.json", e))?;

    let json: Value = serde_json::from_str(&content)
        .map_err(|e| fail("Failed to parse existing settings.json", e))?;

    if json.get("hooks").is_some() {
        error!(
            "Hooks already configured in {:?}. Use --force to overwrite.",
            settings_path
        );
        eprintln!(
            "Error: Hooks already configured in {:?}. Use --force to overwrite.",
            settings_path
        );
        return Err(1);
    }

    Ok(())
}

/// Write settings.json, merging with existing non-hook settings if present.
fn write_settings_json(settings_path: &Path) -> Result<(), i32> {
    let hooks_config: Value = serde_json::from_str(SETTINGS_JSON_TEMPLATE).map_err(|e| {
        error!("Internal error: Failed to parse SETTINGS_JSON_TEMPLATE: {}", e);
        eprintln!("Internal error: Failed to parse SETTINGS_JSON_TEMPLATE: {}", e);
        1
    })?;

    // If file exists, merge hooks into existing settings; otherwise use template directly
    let final_config = if settings_path.exists() {
        let existing_content = fs::read_to_string(settings_path)
            .map_err(|e| fail("Failed to read existing settings.json", e))?;

        let mut existing: Value = serde_json::from_str(&existing_content)
            .map_err(|e| fail("Failed to parse existing settings.json", e))?;

        // Merge hooks config into existing (preserves other keys, overwrites hooks)
        if let (Some(existing_obj), Some(hooks_obj)) =
            (existing.as_object_mut(), hooks_config.as_object())
        {
            for (key, value) in hooks_obj {
                existing_obj.insert(key.clone(), value.clone());
            }
        }
        existing
    } else {
        hooks_config
    };

    let pretty = serde_json::to_string_pretty(&final_config)
        .map_err(|e| fail("Failed to serialize settings.json", e))?;

    fs::write(settings_path, format!("{}\n", pretty))
        .map_err(|e| fail("Failed to write settings.json", e))?;

    info!("Wrote settings.json: {:?}", settings_path);
    Ok(())
}

// =============================================================================
// TEMPLATE CONSTANTS
// =============================================================================

/// Settings.json template with hook configuration.
const SETTINGS_JSON_TEMPLATE: &str = r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/session_start.sh",
            "timeout": 5000
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/session_end.sh",
            "timeout": 30000
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/pre_tool_use.sh",
            "timeout": 100
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/post_tool_use.sh",
            "timeout": 3000
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/user_prompt_submit.sh",
            "timeout": 2000
          }
        ]
      }
    ]
  }
}"#;

/// session_start.sh template.
const SESSION_START_SCRIPT: &str = r#"#!/bin/bash
# Claude Code Hook: SessionStart
# Timeout: 5000ms
#
# Constitution: AP-50, AP-26
# Exit Codes: 0=success, 1=error, 2=timeout, 3=db_error (CLI passthrough), 4=invalid_input
#
# Input from Claude Code: {"session_id":"...", "timestamp":"..."}
# Transforms to HookInput format expected by CLI

set -euo pipefail

INPUT=$(cat)
if [ -z "$INPUT" ]; then
    echo '{"success":false,"error":"Empty stdin","exit_code":4}' >&2
    exit 4
fi

# Validate JSON input
if ! echo "$INPUT" | jq empty 2>/dev/null; then
    echo '{"success":false,"error":"Invalid JSON input","exit_code":4}' >&2
    exit 4
fi

# Find CLI binary
CONTEXT_GRAPH_CLI="${CONTEXT_GRAPH_CLI:-context-graph-cli}"
if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null; then
    for candidate in \
        "./target/release/context-graph-cli" \
        "./target/debug/context-graph-cli" \
        "$HOME/.cargo/bin/context-graph-cli" \
    ; do
        if [ -x "$candidate" ]; then
            CONTEXT_GRAPH_CLI="$candidate"
            break
        fi
    done
fi

if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null && [ ! -x "$CONTEXT_GRAPH_CLI" ]; then
    echo '{"success":false,"error":"CLI binary not found","exit_code":1}' >&2
    exit 1
fi

# Parse input JSON using jq
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
PREVIOUS_SESSION_ID=$(echo "$INPUT" | jq -r '.previous_session_id // empty')
TIMESTAMP_MS=$(date +%s%3N)
CWD=$(pwd)

# Generate session ID if not provided (better than bare $RANDOM)
if [ -z "$SESSION_ID" ]; then
    SESSION_ID="session-$(uuidgen 2>/dev/null || echo "$(date +%s)-$$-$RANDOM")"
fi

# Build HookInput JSON with snake_case and tagged enum format
# Note: payload uses tag="type" content="data" format per serde config
if [ -n "$PREVIOUS_SESSION_ID" ]; then
    HOOK_INPUT=$(cat <<EOF
{
    "hook_type": "session_start",
    "session_id": "$SESSION_ID",
    "timestamp_ms": $TIMESTAMP_MS,
    "payload": {
        "type": "session_start",
        "data": {
            "cwd": "$CWD",
            "source": "cli",
            "previous_session_id": "$PREVIOUS_SESSION_ID"
        }
    }
}
EOF
)
else
    HOOK_INPUT=$(cat <<EOF
{
    "hook_type": "session_start",
    "session_id": "$SESSION_ID",
    "timestamp_ms": $TIMESTAMP_MS,
    "payload": {
        "type": "session_start",
        "data": {
            "cwd": "$CWD",
            "source": "cli"
        }
    }
}
EOF
)
fi

# Execute CLI with 5s timeout
# Note: CLI exit codes pass through (including exit code 3 for db_error)
echo "$HOOK_INPUT" | timeout 5s "$CONTEXT_GRAPH_CLI" hooks session-start --stdin --format json
exit_code=$?

if [ $exit_code -eq 124 ]; then
    echo '{"success":false,"error":"Timeout after 5000ms","exit_code":2}' >&2
    exit 2
fi
exit $exit_code
"#;

/// pre_tool_use.sh template.
const PRE_TOOL_USE_SCRIPT: &str = r#"#!/bin/bash
# Claude Code Hook: PreToolUse
# Wrapper Timeout: 500ms (CLI internal: <100ms, process overhead: ~200-400ms)
#
# Constitution: AP-50, AP-26
# Exit Codes: 0=success, 1=error, 2=timeout, 4=invalid_input
#
# CRITICAL: CLI logic must complete in <100ms. No database operations allowed.
# The 500ms wrapper timeout accounts for bash/process startup overhead.
# Input from Claude Code: {"tool_name":"...", "tool_input":{...}}

set -euo pipefail

INPUT=$(cat)
if [ -z "$INPUT" ]; then
    echo '{"success":false,"error":"Empty stdin","exit_code":4}' >&2
    exit 4
fi

# Validate JSON input
if ! echo "$INPUT" | jq empty 2>/dev/null; then
    echo '{"success":false,"error":"Invalid JSON input","exit_code":4}' >&2
    exit 4
fi

# Find CLI binary
CONTEXT_GRAPH_CLI="${CONTEXT_GRAPH_CLI:-context-graph-cli}"
if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null; then
    for candidate in \
        "./target/release/context-graph-cli" \
        "./target/debug/context-graph-cli" \
        "$HOME/.cargo/bin/context-graph-cli" \
    ; do
        if [ -x "$candidate" ]; then
            CONTEXT_GRAPH_CLI="$candidate"
            break
        fi
    done
fi

if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null && [ ! -x "$CONTEXT_GRAPH_CLI" ]; then
    echo '{"success":false,"error":"CLI binary not found","exit_code":1}' >&2
    exit 1
fi

# Parse input - extract session_id and tool info
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // "default-session"')
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // "unknown"')

# Execute CLI - FAST PATH (no DB)
# Note: The 100ms budget is for the overall hook. Process startup adds overhead.
# We allow 500ms for the shell wrapper (100ms CLI + 400ms process overhead).
# The CLI itself MUST complete in <100ms - no database access allowed.
timeout 0.5s "$CONTEXT_GRAPH_CLI" hooks pre-tool \
    --session-id "$SESSION_ID" \
    --tool-name "$TOOL_NAME" \
    --fast-path true \
    --format json
exit_code=$?

if [ $exit_code -eq 124 ]; then
    echo '{"success":false,"error":"Timeout after 500ms - process startup exceeded budget","exit_code":2}' >&2
    exit 2
fi
exit $exit_code
"#;

/// post_tool_use.sh template.
const POST_TOOL_USE_SCRIPT: &str = r#"#!/bin/bash
# Claude Code Hook: PostToolUse
# Timeout: 3000ms (async allowed)
#
# Constitution: AP-50, AP-26
# Exit Codes: 0=success, 1=error, 2=timeout, 3=db_error, 4=invalid_input
#
# Input from Claude Code: {"tool_name":"...", "tool_result":"...", "success":bool}

set -euo pipefail

INPUT=$(cat)
if [ -z "$INPUT" ]; then
    echo '{"success":false,"error":"Empty stdin","exit_code":4}' >&2
    exit 4
fi

# Validate JSON input
if ! echo "$INPUT" | jq empty 2>/dev/null; then
    echo '{"success":false,"error":"Invalid JSON input","exit_code":4}' >&2
    exit 4
fi

# Find CLI binary
CONTEXT_GRAPH_CLI="${CONTEXT_GRAPH_CLI:-context-graph-cli}"
if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null; then
    for candidate in \
        "./target/release/context-graph-cli" \
        "./target/debug/context-graph-cli" \
        "$HOME/.cargo/bin/context-graph-cli" \
    ; do
        if [ -x "$candidate" ]; then
            CONTEXT_GRAPH_CLI="$candidate"
            break
        fi
    done
fi

if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null && [ ! -x "$CONTEXT_GRAPH_CLI" ]; then
    echo '{"success":false,"error":"CLI binary not found","exit_code":1}' >&2
    exit 1
fi

# Parse input
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // "default-session"')
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // "unknown"')
SUCCESS=$(echo "$INPUT" | jq -r '.success // "true"')

# Execute CLI with 3s timeout
timeout 3s "$CONTEXT_GRAPH_CLI" hooks post-tool \
    --session-id "$SESSION_ID" \
    --tool-name "$TOOL_NAME" \
    --success "$SUCCESS" \
    --format json
exit_code=$?

if [ $exit_code -eq 124 ]; then
    echo '{"success":false,"error":"Timeout after 3000ms","exit_code":2}' >&2
    exit 2
fi
exit $exit_code
"#;

/// user_prompt_submit.sh template.
const USER_PROMPT_SUBMIT_SCRIPT: &str = r#"#!/bin/bash
# Claude Code Hook: UserPromptSubmit
# Timeout: 2000ms
#
# Constitution: AP-50, AP-26
# Exit Codes: 0=success, 1=error, 2=timeout, 3=db_error (CLI passthrough), 4=invalid_input
#
# Input from Claude Code: {"prompt":"...", "session_id":"..."}
# Uses stdin approach to avoid shell injection with user-controlled prompt text

set -euo pipefail

INPUT=$(cat)
if [ -z "$INPUT" ]; then
    echo '{"success":false,"error":"Empty stdin","exit_code":4}' >&2
    exit 4
fi

# Validate JSON input
if ! echo "$INPUT" | jq empty 2>/dev/null; then
    echo '{"success":false,"error":"Invalid JSON input","exit_code":4}' >&2
    exit 4
fi

# Find CLI binary
CONTEXT_GRAPH_CLI="${CONTEXT_GRAPH_CLI:-context-graph-cli}"
if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null; then
    for candidate in \
        "./target/release/context-graph-cli" \
        "./target/debug/context-graph-cli" \
        "$HOME/.cargo/bin/context-graph-cli" \
    ; do
        if [ -x "$candidate" ]; then
            CONTEXT_GRAPH_CLI="$candidate"
            break
        fi
    done
fi

if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null && [ ! -x "$CONTEXT_GRAPH_CLI" ]; then
    echo '{"success":false,"error":"CLI binary not found","exit_code":1}' >&2
    exit 1
fi

# Parse input - extract needed fields
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // "default-session"')
TIMESTAMP_MS=$(date +%s%3N)

# Extract prompt safely via jq (handles special chars, newlines)
PROMPT=$(echo "$INPUT" | jq -r '.prompt // ""')

# Build HookInput JSON - using jq to safely embed the prompt
HOOK_INPUT=$(jq -n \
    --arg hook_type "user_prompt_submit" \
    --arg session_id "$SESSION_ID" \
    --argjson timestamp_ms "$TIMESTAMP_MS" \
    --arg prompt "$PROMPT" \
    '{
        hook_type: $hook_type,
        session_id: $session_id,
        timestamp_ms: $timestamp_ms,
        payload: {
            type: "user_prompt_submit",
            data: {
                prompt: $prompt,
                context: []
            }
        }
    }')

# Execute CLI with 2s timeout using stdin (avoids shell injection)
# Note: CLI exit codes pass through (including exit code 3 for db_error)
# Note: --session-id is required even when using --stdin
echo "$HOOK_INPUT" | timeout 2s "$CONTEXT_GRAPH_CLI" hooks prompt-submit \
    --session-id "$SESSION_ID" \
    --stdin true \
    --format json
exit_code=$?

if [ $exit_code -eq 124 ]; then
    echo '{"success":false,"error":"Timeout after 2000ms","exit_code":2}' >&2
    exit 2
fi
exit $exit_code
"#;

/// session_end.sh template.
const SESSION_END_SCRIPT: &str = r#"#!/bin/bash
# Claude Code Hook: SessionEnd
# Timeout: 30000ms (30 seconds for full persistence)
#
# Constitution: AP-50, AP-26
# Exit Codes: 0=success, 1=error, 2=timeout, 3=db_error, 4=invalid_input
#
# Input from Claude Code: {"session_id":"...", "reason":"...", "stats":{...}}

set -euo pipefail

INPUT=$(cat)
if [ -z "$INPUT" ]; then
    echo '{"success":false,"error":"Empty stdin","exit_code":4}' >&2
    exit 4
fi

# Validate JSON input
if ! echo "$INPUT" | jq empty 2>/dev/null; then
    echo '{"success":false,"error":"Invalid JSON input","exit_code":4}' >&2
    exit 4
fi

# Find CLI binary
CONTEXT_GRAPH_CLI="${CONTEXT_GRAPH_CLI:-context-graph-cli}"
if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null; then
    for candidate in \
        "./target/release/context-graph-cli" \
        "./target/debug/context-graph-cli" \
        "$HOME/.cargo/bin/context-graph-cli" \
    ; do
        if [ -x "$candidate" ]; then
            CONTEXT_GRAPH_CLI="$candidate"
            break
        fi
    done
fi

if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null && [ ! -x "$CONTEXT_GRAPH_CLI" ]; then
    echo '{"success":false,"error":"CLI binary not found","exit_code":1}' >&2
    exit 1
fi

# Parse input
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // "default-session"')
DURATION_MS=$(echo "$INPUT" | jq -r '.stats.duration_ms // "0"')

# Execute CLI with 30s timeout
timeout 30s "$CONTEXT_GRAPH_CLI" hooks session-end \
    --session-id "$SESSION_ID" \
    --duration-ms "$DURATION_MS" \
    --generate-summary true \
    --format json
exit_code=$?

if [ $exit_code -eq 124 ]; then
    echo '{"success":false,"error":"Timeout after 30000ms","exit_code":2}' >&2
    exit 2
fi
exit $exit_code
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // =============================================================================
    // TC-SETUP-01: Fresh setup creates all files
    // =============================================================================
    #[tokio::test]
    async fn tc_setup_01_fresh_setup_creates_all_files() {
        println!("\n=== TC-SETUP-01: Fresh Setup Creates All Files ===");
        println!("SOURCE OF TRUTH: File system in temp directory");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let target = temp_dir.path().to_path_buf();

        println!("BEFORE: Empty directory at {:?}", target);
        assert!(!target.join(".claude").exists());

        // Action
        let args = SetupArgs {
            force: false,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;

        println!("AFTER: exit_code={}", exit_code);
        assert_eq!(exit_code, 0, "Setup should succeed with exit code 0");

        // Verify files exist
        let claude_dir = target.join(".claude");
        let hooks_dir = claude_dir.join("hooks");
        let settings_path = claude_dir.join("settings.json");

        assert!(claude_dir.exists(), ".claude directory must exist");
        assert!(hooks_dir.exists(), ".claude/hooks directory must exist");
        assert!(settings_path.exists(), "settings.json must exist");

        let expected_scripts = [
            "session_start.sh",
            "pre_tool_use.sh",
            "post_tool_use.sh",
            "user_prompt_submit.sh",
            "session_end.sh",
        ];

        for script in &expected_scripts {
            let script_path = hooks_dir.join(script);
            assert!(script_path.exists(), "{} must exist", script);
            println!("  {} exists: true", script);
        }

        // Verify settings.json is valid JSON with hooks
        let settings_content = fs::read_to_string(&settings_path).expect("Read settings.json");
        let settings: Value = serde_json::from_str(&settings_content).expect("Parse settings.json");
        assert!(settings.get("hooks").is_some(), "settings.json must have hooks key");

        println!("EVIDENCE: All 5 scripts + settings.json created");
        println!("RESULT: PASS - Fresh setup creates all files");
    }

    // =============================================================================
    // TC-SETUP-02: Existing hooks without --force returns error
    // =============================================================================
    #[tokio::test]
    async fn tc_setup_02_existing_hooks_no_force_fails() {
        println!("\n=== TC-SETUP-02: Existing Hooks Without --force Fails ===");
        println!("SOURCE OF TRUTH: Exit code");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let target = temp_dir.path().to_path_buf();

        // First setup
        let args = SetupArgs {
            force: false,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;
        assert_eq!(exit_code, 0, "First setup should succeed");

        println!("BEFORE: Hooks already configured");

        // Second setup without --force
        let args = SetupArgs {
            force: false,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;

        println!("AFTER: exit_code={}", exit_code);
        assert_eq!(exit_code, 1, "Second setup without --force should fail with exit code 1");

        println!("EVIDENCE: Exit code 1 returned");
        println!("RESULT: PASS - Existing hooks without --force fails");
    }

    // =============================================================================
    // TC-SETUP-03: Existing hooks with --force succeeds
    // =============================================================================
    #[tokio::test]
    async fn tc_setup_03_existing_hooks_with_force_succeeds() {
        println!("\n=== TC-SETUP-03: Existing Hooks With --force Succeeds ===");
        println!("SOURCE OF TRUTH: Exit code and file modification");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let target = temp_dir.path().to_path_buf();

        // First setup
        let args = SetupArgs {
            force: false,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;
        assert_eq!(exit_code, 0, "First setup should succeed");

        println!("BEFORE: Hooks already configured");

        // Second setup with --force
        let args = SetupArgs {
            force: true,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;

        println!("AFTER: exit_code={}", exit_code);
        assert_eq!(exit_code, 0, "Setup with --force should succeed");

        println!("EVIDENCE: Exit code 0 returned");
        println!("RESULT: PASS - Existing hooks with --force succeeds");
    }

    // =============================================================================
    // TC-SETUP-04: Merge preserves existing non-hook settings
    // =============================================================================
    #[tokio::test]
    async fn tc_setup_04_merge_preserves_existing_settings() {
        println!("\n=== TC-SETUP-04: Merge Preserves Existing Settings ===");
        println!("SOURCE OF TRUTH: settings.json content");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let target = temp_dir.path().to_path_buf();
        let claude_dir = target.join(".claude");
        let settings_path = claude_dir.join("settings.json");

        // Create .claude directory with custom settings (no hooks)
        fs::create_dir_all(&claude_dir).expect("Create .claude dir");
        let custom_settings = r#"{"customSetting": "value123", "theme": "dark"}"#;
        fs::write(&settings_path, custom_settings).expect("Write custom settings");

        println!("BEFORE: Custom settings without hooks");
        println!("  customSetting: value123");
        println!("  theme: dark");

        // Run setup
        let args = SetupArgs {
            force: false,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;

        println!("AFTER: exit_code={}", exit_code);
        assert_eq!(exit_code, 0, "Setup should succeed");

        // Verify settings preserved
        let settings_content = fs::read_to_string(&settings_path).expect("Read settings.json");
        let settings: Value = serde_json::from_str(&settings_content).expect("Parse settings.json");

        assert_eq!(
            settings.get("customSetting").and_then(|v| v.as_str()),
            Some("value123"),
            "customSetting must be preserved"
        );
        assert_eq!(
            settings.get("theme").and_then(|v| v.as_str()),
            Some("dark"),
            "theme must be preserved"
        );
        assert!(settings.get("hooks").is_some(), "hooks must be added");

        println!("EVIDENCE: customSetting=value123, theme=dark, hooks=present");
        println!("RESULT: PASS - Merge preserves existing settings");
    }

    // =============================================================================
    // TC-SETUP-05: Scripts are executable (Unix only)
    // =============================================================================
    #[cfg(unix)]
    #[tokio::test]
    async fn tc_setup_05_scripts_are_executable() {
        use std::os::unix::fs::PermissionsExt;

        println!("\n=== TC-SETUP-05: Scripts Are Executable ===");
        println!("SOURCE OF TRUTH: File permissions");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let target = temp_dir.path().to_path_buf();

        let args = SetupArgs {
            force: false,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;
        assert_eq!(exit_code, 0, "Setup should succeed");

        let hooks_dir = target.join(".claude").join("hooks");
        let expected_scripts = [
            "session_start.sh",
            "pre_tool_use.sh",
            "post_tool_use.sh",
            "user_prompt_submit.sh",
            "session_end.sh",
        ];

        for script in &expected_scripts {
            let script_path = hooks_dir.join(script);
            let metadata = fs::metadata(&script_path).expect("Get metadata");
            let mode = metadata.permissions().mode();
            let is_executable = mode & 0o111 != 0;
            println!("  {}: mode={:o}, executable={}", script, mode & 0o777, is_executable);
            assert!(is_executable, "{} must be executable", script);
        }

        println!("EVIDENCE: All scripts have executable bit set");
        println!("RESULT: PASS - Scripts are executable");
    }

    // =============================================================================
    // TC-SETUP-06: settings.json structure matches spec
    // =============================================================================
    #[tokio::test]
    async fn tc_setup_06_settings_structure_matches_spec() {
        println!("\n=== TC-SETUP-06: settings.json Structure Matches Spec ===");
        println!("SOURCE OF TRUTH: settings.json content");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let target = temp_dir.path().to_path_buf();

        let args = SetupArgs {
            force: false,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;
        assert_eq!(exit_code, 0, "Setup should succeed");

        let settings_path = target.join(".claude").join("settings.json");
        let settings_content = fs::read_to_string(&settings_path).expect("Read settings.json");
        let settings: Value = serde_json::from_str(&settings_content).expect("Parse settings.json");

        let hooks = settings.get("hooks").expect("hooks key must exist");

        // Check all hook types exist
        let expected_hooks = ["SessionStart", "SessionEnd", "PreToolUse", "PostToolUse", "UserPromptSubmit"];
        for hook_type in &expected_hooks {
            assert!(hooks.get(*hook_type).is_some(), "{} must exist", hook_type);
            println!("  {}: present", hook_type);
        }

        // Verify SessionStart structure
        let session_start = &hooks["SessionStart"][0]["hooks"][0];
        assert_eq!(session_start["type"].as_str(), Some("command"));
        assert_eq!(session_start["command"].as_str(), Some(".claude/hooks/session_start.sh"));
        assert_eq!(session_start["timeout"].as_i64(), Some(5000));

        // Verify PreToolUse has matcher
        let pre_tool = &hooks["PreToolUse"][0];
        assert_eq!(pre_tool["matcher"].as_str(), Some(".*"));

        println!("EVIDENCE: All hook types present with correct structure");
        println!("RESULT: PASS - settings.json structure matches spec");
    }

    // =============================================================================
    // TC-SETUP-07: Script content starts with shebang
    // =============================================================================
    #[tokio::test]
    async fn tc_setup_07_script_content_has_shebang() {
        println!("\n=== TC-SETUP-07: Script Content Has Shebang ===");
        println!("SOURCE OF TRUTH: First line of each script");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let target = temp_dir.path().to_path_buf();

        let args = SetupArgs {
            force: false,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;
        assert_eq!(exit_code, 0, "Setup should succeed");

        let hooks_dir = target.join(".claude").join("hooks");
        let expected_scripts = [
            "session_start.sh",
            "pre_tool_use.sh",
            "post_tool_use.sh",
            "user_prompt_submit.sh",
            "session_end.sh",
        ];

        for script in &expected_scripts {
            let script_path = hooks_dir.join(script);
            let content = fs::read_to_string(&script_path).expect("Read script");
            let first_line = content.lines().next().expect("Script must have content");
            assert_eq!(first_line, "#!/bin/bash", "{} must start with #!/bin/bash", script);
            println!("  {}: #!/bin/bash", script);
        }

        println!("EVIDENCE: All scripts start with #!/bin/bash");
        println!("RESULT: PASS - Script content has shebang");
    }

    // =============================================================================
    // TC-SETUP-08: Non-existent target directory fails
    // =============================================================================
    #[tokio::test]
    async fn tc_setup_08_nonexistent_target_fails() {
        println!("\n=== TC-SETUP-08: Non-existent Target Directory Fails ===");
        println!("SOURCE OF TRUTH: Exit code");

        let target = PathBuf::from("/this/path/definitely/does/not/exist/12345");

        println!("BEFORE: Target directory does not exist");

        let args = SetupArgs {
            force: false,
            target_dir: Some(target),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;

        println!("AFTER: exit_code={}", exit_code);
        assert_eq!(exit_code, 1, "Setup with non-existent target should fail");

        println!("EVIDENCE: Exit code 1 returned");
        println!("RESULT: PASS - Non-existent target directory fails");
    }

    // =============================================================================
    // EDGE CASE 1: Empty target directory
    // =============================================================================
    #[tokio::test]
    async fn edge_case_empty_target_directory() {
        println!("\n=== EDGE CASE 1: Empty Target Directory ===");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let target = temp_dir.path().to_path_buf();

        println!("BEFORE: Empty directory at {:?}", target);
        let entries: Vec<_> = fs::read_dir(&target).unwrap().collect();
        assert!(entries.is_empty(), "Directory should be empty");

        let args = SetupArgs {
            force: false,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;

        println!("AFTER: exit_code={}", exit_code);
        assert_eq!(exit_code, 0);

        // Count created files
        let claude_entries: Vec<_> = fs::read_dir(target.join(".claude")).unwrap().collect();
        println!("  .claude/ entries: {}", claude_entries.len());
        assert_eq!(claude_entries.len(), 2); // hooks dir + settings.json

        println!("RESULT: PASS - Empty target directory handled");
    }

    // =============================================================================
    // EDGE CASE 2: settings.json with invalid JSON
    // =============================================================================
    #[tokio::test]
    async fn edge_case_invalid_settings_json() {
        println!("\n=== EDGE CASE 2: Invalid settings.json ===");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let target = temp_dir.path().to_path_buf();
        let claude_dir = target.join(".claude");
        let settings_path = claude_dir.join("settings.json");

        fs::create_dir_all(&claude_dir).expect("Create .claude dir");
        fs::write(&settings_path, "{ invalid json }").expect("Write invalid JSON");

        println!("BEFORE: Invalid JSON in settings.json");

        let args = SetupArgs {
            force: false,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;

        println!("AFTER: exit_code={}", exit_code);
        assert_eq!(exit_code, 1, "Should fail with invalid JSON");

        println!("RESULT: PASS - Invalid settings.json handled");
    }

    // =============================================================================
    // EDGE CASE 3: Force overwrite with custom settings preserved
    // =============================================================================
    #[tokio::test]
    async fn edge_case_force_preserves_custom_settings() {
        println!("\n=== EDGE CASE 3: Force Overwrite Preserves Custom Settings ===");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let target = temp_dir.path().to_path_buf();
        let claude_dir = target.join(".claude");
        let settings_path = claude_dir.join("settings.json");

        // First setup
        let args = SetupArgs {
            force: false,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        handle_setup(args).await;

        // Add custom setting to existing settings.json
        let mut settings: Value = serde_json::from_str(
            &fs::read_to_string(&settings_path).unwrap()
        ).unwrap();
        settings.as_object_mut().unwrap().insert(
            "myCustomSetting".to_string(),
            Value::String("preserved".to_string())
        );
        fs::write(&settings_path, serde_json::to_string_pretty(&settings).unwrap()).unwrap();

        println!("BEFORE: settings.json with myCustomSetting=preserved");

        // Force overwrite
        let args = SetupArgs {
            force: true,
            target_dir: Some(target.clone()),
            skip_chmod: false,
        };
        let exit_code = handle_setup(args).await;

        println!("AFTER: exit_code={}", exit_code);
        assert_eq!(exit_code, 0);

        // Verify custom setting preserved
        let settings: Value = serde_json::from_str(
            &fs::read_to_string(&settings_path).unwrap()
        ).unwrap();
        assert_eq!(
            settings.get("myCustomSetting").and_then(|v| v.as_str()),
            Some("preserved"),
            "Custom setting must be preserved after --force"
        );

        println!("EVIDENCE: myCustomSetting=preserved after --force");
        println!("RESULT: PASS - Force overwrite preserves custom settings");
    }
}
