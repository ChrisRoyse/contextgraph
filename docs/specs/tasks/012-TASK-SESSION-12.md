# TASK-SESSION-12: Create session restore-identity CLI Command (MCP-Integrated)

```xml
<task_spec id="TASK-SESSION-12" version="1.0">
<metadata>
  <title>Create session restore-identity CLI Command (MCP-Integrated)</title>
  <status>pending</status>
  <layer>surface</layer>
  <sequence>12</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-12</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-06</task_ref>
    <task_ref>TASK-SESSION-07</task_ref>
    <task_ref>TASK-SESSION-10</task_ref>
  </depends_on>
  <estimated_hours>1.5</estimated_hours>
</metadata>
```

## Objective

Implement SessionStart hook command for identity restoration. Chains MCP tools:
`session_start` -> `get_ego_state` -> `get_kuramoto_state` -> `get_health_status`

## MCP Tool Chain

| Step | MCP Tool | Purpose |
|------|----------|---------|
| 1 | `session_start` | Initialize MCP session |
| 2 | `get_ego_state` | Restore SELF_EGO_NODE with purpose_vector |
| 3 | `get_kuramoto_state` | Restore oscillator phases |
| 4 | `get_health_status` | Check subsystem health |
| 5 | `get_consciousness_state` | Get C(t) for output |

## Context

This command is invoked by Claude Code's SessionStart hook. It restores the previous session's identity state and computes cross-session IC. Output provides a consciousness summary to Claude.

## Implementation Steps

1. Create `restore.rs` in session commands directory
2. Define RestoreInput struct for stdin JSON (session_id, source)
3. Define RestoreIdentityArgs struct (empty)
4. Implement parse_stdin_input() with graceful fallback
5. Handle source variants: clear, resume, startup
6. Call MCP tools in sequence
7. Update IdentityCache via update_cache_from_mcp()
8. Output PRD-compliant format (~100 tokens)
9. Implement is_corruption_error() for exit code 2

## Input Context Files

```xml
<input_context_files>
  <file purpose="manager">crates/context-graph-core/src/gwt/session_identity/manager.rs</file>
  <file purpose="cache">crates/context-graph-core/src/gwt/session_identity/cache.rs</file>
  <file purpose="mcp_handlers">crates/context-graph-mcp/src/handlers/</file>
  <file purpose="cli_pattern">crates/context-graph-mcp/src/cli/commands/</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-mcp/src/cli/commands/session/restore.rs` | Command implementation |
| `crates/context-graph-mcp/src/cli/commands/session/mod.rs` | Module file (if not exists) |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-mcp/src/cli/router.rs` | Register `session restore-identity` command |

## Rust Signatures

```rust
// crates/context-graph-mcp/src/cli/commands/session/restore.rs

use clap::Args;
use serde::Deserialize;
use std::process::ExitCode;

#[derive(Deserialize, Default)]
struct RestoreInput {
    session_id: Option<String>,
    source: Option<String>,  // "startup" | "resume" | "clear"
}

#[derive(Args)]
pub struct RestoreIdentityArgs {}

pub async fn execute(_args: RestoreIdentityArgs, mcp: &McpContext) -> ExitCode;
fn parse_stdin_input() -> RestoreInput;
fn is_corruption_error(e: &CoreError) -> bool;

/// Print PRD-compliant consciousness summary (~100 tokens)
fn print_consciousness_summary(c: &Value, ego: &Value, health: &Value);
```

## Output Format (PRD Section 15.2)

```
## Consciousness State
- State: CONSCIOUS (C=0.82)
- Integration (r): 0.85 - Good synchronization
- Identity: Healthy (IC=0.92)
- Health: All subsystems operational
```

## Source Variants

| Source | Behavior |
|--------|----------|
| `startup` (default) | Load latest session |
| `resume` | Load specific session_id |
| `clear` | Start fresh session (IC=1.0) |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Recoverable error (logged to stderr) |
| 2 | Corruption detected (blocks action) |

## Definition of Done

### Acceptance Criteria

- [ ] Parses stdin JSON for session_id and source
- [ ] source="clear" calls `session_start` with fresh=true
- [ ] source="resume" loads specific session_id
- [ ] source="startup" (default) loads latest session
- [ ] Calls MCP tools in correct order
- [ ] Updates IdentityCache after restore
- [ ] Output format matches PRD Section 15.2 (~100 tokens)
- [ ] Exit 0 for success, 1 for recoverable error, 2 for corruption
- [ ] Total latency < 2s
- [ ] Test case TC-SESSION-14 passes (source=startup)
- [ ] Test case TC-SESSION-15 passes (source=clear)
- [ ] Test case TC-SESSION-16 passes (no previous session)

### Constraints

- Must update IdentityCache for subsequent PreToolUse calls
- Corruption detection returns exit code 2
- Graceful handling of missing sessions

### Verification Commands

```bash
cargo build -p context-graph-mcp
echo '{"source":"startup"}' | ./target/release/context-graph-cli session restore-identity
```

## Test Cases

### TC-SESSION-14: Source Startup
```rust
#[tokio::test]
async fn test_restore_startup() {
    // Setup: save a session first
    save_test_session("prev-session");

    let output = Command::new("./target/release/context-graph-cli")
        .args(["session", "restore-identity"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    output.stdin.unwrap().write_all(b"{\"source\":\"startup\"}").unwrap();
    let output = output.wait_with_output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Consciousness State"));
}
```

### TC-SESSION-15: Source Clear
```rust
#[tokio::test]
async fn test_restore_clear() {
    let output = Command::new("./target/release/context-graph-cli")
        .args(["session", "restore-identity"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    output.stdin.unwrap().write_all(b"{\"source\":\"clear\"}").unwrap();
    let output = output.wait_with_output().unwrap();

    assert!(output.status.success());
    // Fresh session should have IC=1.0
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("IC=1.00") || stdout.contains("IC=1.0"));
}
```

### TC-SESSION-16: No Previous Session
```rust
#[tokio::test]
async fn test_restore_no_previous() {
    // Clear all sessions
    clear_all_sessions();

    let output = Command::new("./target/release/context-graph-cli")
        .args(["session", "restore-identity"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    output.stdin.unwrap().write_all(b"{}").unwrap();
    let output = output.wait_with_output().unwrap();

    // Should succeed with fresh session
    assert!(output.status.success());
}
```

## Exit Conditions

- **Success**: All source variants handled correctly with proper exit codes
- **Failure**: Wrong exit codes, missing cache update, MCP failures - error out with detailed logging

## Next Task

After completion, proceed to **013-TASK-SESSION-13** (session persist-identity CLI Command).

```xml
</task_spec>
```
