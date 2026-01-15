# TASK-SESSION-14: Create consciousness check-identity CLI Command (MCP-Integrated)

```xml
<task_spec id="TASK-SESSION-14" version="1.0">
<metadata>
  <title>Create consciousness check-identity CLI Command (MCP-Integrated)</title>
  <status>pending</status>
  <layer>surface</layer>
  <sequence>14</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-14</requirement_ref>
    <constitution_ref>AP-26</constitution_ref>
    <constitution_ref>AP-38</constitution_ref>
    <constitution_ref>AP-42</constitution_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-07</task_ref>
    <task_ref>TASK-SESSION-08</task_ref>
    <task_ref>TASK-SESSION-10</task_ref>
  </depends_on>
  <estimated_hours>1.0</estimated_hours>
</metadata>
```

## Objective

Implement PostToolUse hook command for IC checking via MCP. Chains:
`get_identity_continuity` -> `trigger_dream` (if IC<0.5) -> `get_memetic_status` (entropy check)

## MCP Tool Chain

| Step | Condition | MCP Tool | Purpose |
|------|-----------|----------|---------|
| 1 | Always | `get_identity_continuity` | Get IC value |
| 2 | IC < 0.5 && --auto-dream | `trigger_dream` | Auto-dream (AP-26, AP-38) |
| 3 | Always | `get_memetic_status` | Entropy check |
| 4 | entropy > 0.7 | `trigger_mental_check` | Mental check (AP-42) |

## Context

This command is invoked by Claude Code's PostToolUse hook after significant operations. It monitors identity continuity and triggers corrective actions when needed.

## Implementation Steps

1. Create `check.rs` in consciousness commands directory
2. Define CheckIdentityArgs with --auto-dream flag
3. Call `get_identity_continuity` MCP tool
4. Update IdentityCache atomically
5. If IC < 0.5 and --auto-dream: call `trigger_dream` MCP tool
6. Check entropy via `get_memetic_status`, trigger `mental_check` if > 0.7
7. If IC < 0.7: log warning to stderr
8. Silent on healthy IC
9. Always exit 0 (non-blocking)

## Input Context Files

```xml
<input_context_files>
  <file purpose="classify_ic">crates/context-graph-core/src/gwt/session_identity/manager.rs</file>
  <file purpose="dream_trigger">crates/context-graph-core/src/gwt/session_identity/dream_trigger.rs</file>
  <file purpose="cache">crates/context-graph-core/src/gwt/session_identity/cache.rs</file>
  <file purpose="mcp_handlers">crates/context-graph-mcp/src/handlers/</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-mcp/src/cli/commands/consciousness/check.rs` | Command implementation |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-mcp/src/cli/commands/consciousness/mod.rs` | Export check module |
| `crates/context-graph-mcp/src/cli/router.rs` | Register `consciousness check-identity` command |

## Rust Signatures

```rust
// crates/context-graph-mcp/src/cli/commands/consciousness/check.rs

use clap::Args;
use std::process::ExitCode;

#[derive(Args)]
pub struct CheckIdentityArgs {
    /// Enable automatic dream triggering on IC < 0.5
    #[arg(long, default_value = "false")]
    auto_dream: bool,
}

pub async fn execute(args: CheckIdentityArgs, mcp: &McpContext) -> ExitCode;
```

## Output Behavior

| IC Range | Stdout | Stderr | Action |
|----------|--------|--------|--------|
| >= 0.7 | (empty) | (empty) | None |
| 0.5 - 0.69 | (empty) | "IC warning: X.XX" | None |
| < 0.5 (no flag) | (empty) | "IC crisis: X.XX" | None |
| < 0.5 (--auto-dream) | (empty) | "IC crisis (X.XX), dream triggered via MCP" | trigger_dream |

## Constitution Requirements

| Requirement | Implementation |
|-------------|---------------|
| AP-26 | Always exit 0 (non-blocking) |
| AP-38 | IC < 0.5 triggers `trigger_dream` MCP tool |
| AP-42 | entropy > 0.7 triggers `trigger_mental_check` MCP tool |

## Definition of Done

### Acceptance Criteria

- [ ] Accepts --auto-dream flag
- [ ] Calls `get_identity_continuity` MCP tool
- [ ] Updates IdentityCache atomically from MCP response
- [ ] IC < 0.5 with --auto-dream calls `trigger_dream` MCP tool (AP-26, AP-38)
- [ ] IC < 0.5 outputs "IC crisis (X.XX), dream triggered via MCP" to stderr
- [ ] Checks entropy via `get_memetic_status` MCP tool
- [ ] Entropy > 0.7 calls `trigger_mental_check` MCP tool (AP-42)
- [ ] 0.5 <= IC < 0.7 outputs "IC warning: X.XX" to stderr
- [ ] IC >= 0.7 produces no output
- [ ] Always exits 0 (never blocks)
- [ ] Total latency < 500ms
- [ ] Test case TC-SESSION-18 passes (healthy IC)
- [ ] Test case TC-SESSION-19 passes (warning IC)
- [ ] Test case TC-SESSION-20 passes (crisis IC with --auto-dream)

### Constraints

- Always exit 0 (PostToolUse is non-blocking)
- No stdout output (all to stderr)
- Fire-and-forget dream trigger

### Verification Commands

```bash
cargo build -p context-graph-mcp
./target/release/context-graph-cli consciousness check-identity --auto-dream
echo $?  # Should always be 0
```

## Test Cases

### TC-SESSION-18: Healthy IC
```rust
#[tokio::test]
async fn test_check_healthy_ic() {
    // Setup: cache with healthy IC
    update_cache_with_ic(0.92);

    let output = Command::new("./target/release/context-graph-cli")
        .args(["consciousness", "check-identity"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty()); // No warnings
}
```

### TC-SESSION-19: Warning IC
```rust
#[tokio::test]
async fn test_check_warning_ic() {
    // Setup: cache with warning IC
    update_cache_with_ic(0.65);

    let output = Command::new("./target/release/context-graph-cli")
        .args(["consciousness", "check-identity"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("IC warning"));
}
```

### TC-SESSION-20: Crisis IC with Auto-Dream
```rust
#[tokio::test]
async fn test_check_crisis_ic_auto_dream() {
    // Setup: cache with crisis IC
    update_cache_with_ic(0.45);

    let output = Command::new("./target/release/context-graph-cli")
        .args(["consciousness", "check-identity", "--auto-dream"])
        .output()
        .unwrap();

    assert!(output.status.success()); // Always exit 0
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("IC crisis"));
    assert!(stderr.contains("dream triggered"));
}
```

## Exit Conditions

- **Success**: Correct stderr output per IC level, always exit 0, MCP tools called correctly
- **Failure**: Blocking exit codes, missing dream trigger, MCP failures - error out with detailed logging

## Next Task

After completion, proceed to **015-TASK-SESSION-15** (consciousness inject-context CLI Command).

```xml
</task_spec>
```
