# TASK-SESSION-13: Create session persist-identity CLI Command (MCP-Integrated)

```xml
<task_spec id="TASK-SESSION-13" version="1.0">
<metadata>
  <title>Create session persist-identity CLI Command (MCP-Integrated)</title>
  <status>pending</status>
  <layer>surface</layer>
  <sequence>13</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-13</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-05</task_ref>
    <task_ref>TASK-SESSION-06</task_ref>
  </depends_on>
  <estimated_hours>1.0</estimated_hours>
</metadata>
```

## Objective

Implement SessionEnd hook command for persisting current session identity via MCP `session_end` tool. Silent success.

## Context

This command is invoked by Claude Code's SessionEnd hook when the user ends a session. It persists the current session state so it can be restored on next startup.

**Key behavior**: Silent on success (no stdout), logs errors to stderr, never blocks (exit 1, not 2).

## Implementation Steps

1. Create `persist.rs` in session commands directory
2. Define PersistInput struct for stdin JSON (session_id, reason)
3. Define PersistIdentityArgs struct (empty)
4. Parse stdin JSON for session_id and reason
5. Call `session_end` MCP tool (handles persistence internally)
6. Silent success (no stdout), exit 0
7. Non-blocking exit 1 on errors

## Input Context Files

```xml
<input_context_files>
  <file purpose="storage">crates/context-graph-storage/src/session_identity.rs</file>
  <file purpose="manager">crates/context-graph-core/src/gwt/session_identity/manager.rs</file>
  <file purpose="mcp_session">crates/context-graph-mcp/src/handlers/session.rs</file>
  <file purpose="cli_pattern">crates/context-graph-mcp/src/cli/commands/session/</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-mcp/src/cli/commands/session/persist.rs` | Command implementation |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-mcp/src/cli/commands/session/mod.rs` | Export persist module |
| `crates/context-graph-mcp/src/cli/router.rs` | Register `session persist-identity` command |

## Rust Signatures

```rust
// crates/context-graph-mcp/src/cli/commands/session/persist.rs

use clap::Args;
use serde::Deserialize;
use std::process::ExitCode;

#[derive(Deserialize, Default)]
struct PersistInput {
    session_id: Option<String>,
    reason: Option<String>,  // "exit" | "clear" | "logout" | "prompt_input_exit" | "other"
}

#[derive(Args)]
pub struct PersistIdentityArgs {}

pub async fn execute(_args: PersistIdentityArgs, mcp: &McpContext) -> ExitCode;
fn parse_stdin_input() -> PersistInput;
```

## Session End Reasons

| Reason | Description |
|--------|-------------|
| `exit` | User typed exit/quit |
| `clear` | User cleared conversation |
| `logout` | User logged out |
| `prompt_input_exit` | User cancelled prompt |
| `other` | Unknown reason |

## Exit Code Behavior

| Outcome | Exit Code | Stdout | Stderr |
|---------|-----------|--------|--------|
| Success | 0 | (empty) | (empty) |
| Error | 1 | (empty) | Error message |
| Never | 2 | N/A | N/A |

**Important**: Never exit 2 for persist failures per Claude Code semantics (non-blocking hook).

## Definition of Done

### Acceptance Criteria

- [ ] Parses stdin JSON for session_id and reason
- [ ] Calls `session_end` MCP tool with session_id and reason
- [ ] No stdout output (silent success)
- [ ] Exit 0 on success
- [ ] Exit 1 on errors (non-blocking)
- [ ] Never exit 2 (persist failures are non-blocking per Claude Code semantics)
- [ ] Total latency < 3s
- [ ] Test case TC-SESSION-17 passes (success path)

### Constraints

- Silent success (no stdout)
- Errors to stderr only
- Never exit 2 (non-blocking hook)
- Must capture current state before persisting

### Verification Commands

```bash
cargo build -p context-graph-mcp
echo '{"reason":"exit"}' | ./target/release/context-graph-cli session persist-identity
echo $?  # Should be 0
```

## Test Cases

### TC-SESSION-17: Success Path
```rust
#[tokio::test]
async fn test_persist_success() {
    // Setup: ensure session exists
    restore_test_session();

    let output = Command::new("./target/release/context-graph-cli")
        .args(["session", "persist-identity"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    output.stdin.unwrap().write_all(b"{\"reason\":\"exit\"}").unwrap();
    let output = output.wait_with_output().unwrap();

    assert!(output.status.success());
    assert!(output.stdout.is_empty()); // Silent success
    assert!(output.stderr.is_empty()); // No errors
}
```

### TC-SESSION-17b: Error Path (Non-Blocking)
```rust
#[tokio::test]
async fn test_persist_error_nonblocking() {
    // Simulate error condition (e.g., invalid session)
    let output = Command::new("./target/release/context-graph-cli")
        .args(["session", "persist-identity"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    output.stdin.unwrap().write_all(b"{\"session_id\":\"nonexistent\"}").unwrap();
    let output = output.wait_with_output().unwrap();

    // Should be exit 1 (not 2) even on error
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    // Error logged to stderr
    assert!(!output.stderr.is_empty());
}
```

## Exit Conditions

- **Success**: Session persisted via MCP, silent success
- **Failure**: Silent failure without logging - error out with detailed logging

## Next Task

After completion, proceed to **014-TASK-SESSION-14** (consciousness check-identity CLI Command).

```xml
</task_spec>
```
