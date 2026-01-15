# TASK-SESSION-08: Create dream_trigger Module (MCP-Integrated)

```xml
<task_spec id="TASK-SESSION-08" version="1.0">
<metadata>
  <title>Create dream_trigger Module (MCP-Integrated)</title>
  <status>pending</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-08</requirement_ref>
    <constitution_ref>AP-26</constitution_ref>
    <constitution_ref>AP-38</constitution_ref>
    <constitution_ref>AP-42</constitution_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-07</task_ref>
  </depends_on>
  <estimated_hours>1.5</estimated_hours>
</metadata>
```

## Objective

Implement auto-dream trigger per AP-26/AP-38 constitution requirements via MCP `trigger_dream` tool integration. Fire-and-forget async pattern for IC < 0.5.

## MCP Integration

| Condition | MCP Tool | Parameters |
|-----------|----------|------------|
| IC < 0.5 | `trigger_dream` | phase="full_cycle", rationale="IC crisis: {ic}" |
| entropy > 0.7 | `trigger_mental_check` | reason="High entropy: {entropy}" |

## Context

When identity continuity drops below 0.5, the system must automatically trigger dream consolidation to restore coherence. This is a fire-and-forget operation - we spawn the task and continue without blocking.

## Implementation Steps

1. Create `dream_trigger.rs` in session_identity module
2. Implement trigger_dream_via_mcp() calling MCP tool
3. Implement check_and_trigger_dream() for CLI integration
4. Add stderr logging "IC crisis (X.XX), dream triggered via MCP"
5. Implement check_entropy_and_trigger() for AP-42 compliance

## Input Context Files

```xml
<input_context_files>
  <file purpose="classify_ic">crates/context-graph-core/src/gwt/session_identity/manager.rs</file>
  <file purpose="mcp_context">crates/context-graph-mcp/src/context.rs</file>
  <file purpose="dream_handler">crates/context-graph-mcp/src/handlers/dream.rs</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-core/src/gwt/session_identity/dream_trigger.rs` | Dream trigger module |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/gwt/session_identity/mod.rs` | Export dream_trigger module |

## Rust Signatures

```rust
// crates/context-graph-core/src/gwt/session_identity/dream_trigger.rs

use crate::mcp::McpContext;

/// Trigger dream via MCP tool (fire-and-forget).
/// Logs to stderr: "IC crisis (X.XX), dream triggered via MCP"
pub async fn trigger_dream_via_mcp(mcp: &McpContext, ic: f32) -> Result<(), String>;

/// Check IC and trigger dream if below threshold.
/// Returns true if dream was triggered.
pub async fn check_and_trigger_dream(mcp: &McpContext, ic: f32, auto_dream: bool) -> bool;

/// Check entropy and trigger mental_check if > 0.7 (AP-42).
/// Returns true if mental_check was triggered.
pub async fn check_entropy_and_trigger(mcp: &McpContext, entropy: f64) -> bool;

/// Synchronous check for IC crisis, returning dream rationale if needed.
pub fn should_trigger_dream(ic: f32) -> Option<String>;
```

## Constitution Requirements

| Requirement | Description |
|-------------|-------------|
| AP-26 | Exit code 2 only for blocking failures (not for dream trigger) |
| AP-38 | IC < 0.5 triggers `trigger_dream` MCP tool automatically |
| AP-42 | entropy > 0.7 triggers `trigger_mental_check` MCP tool |

## Definition of Done

### Acceptance Criteria

- [ ] trigger_dream_via_mcp calls MCP `trigger_dream` tool
- [ ] Passes rationale "IC crisis: {ic}" to MCP tool
- [ ] check_and_trigger_dream returns true when dream triggered
- [ ] check_and_trigger_dream returns false when auto_dream=false or IC >= 0.5
- [ ] Prints "IC crisis (X.XX), dream triggered via MCP" to stderr
- [ ] check_entropy_and_trigger calls `trigger_mental_check` when entropy > 0.7
- [ ] Fire-and-forget does not block caller (async spawn)
- [ ] Test case TC-SESSION-10 passes (auto-dream trigger)

### Constraints

- Must be fire-and-forget (tokio::spawn)
- Output goes to stderr (not stdout)
- Dream only triggered when --auto-dream flag is set
- Must not block caller

### Verification Commands

```bash
cargo build -p context-graph-core
cargo test -p context-graph-core dream_trigger
```

## Test Cases

### TC-SESSION-10: Auto-Dream Trigger
```rust
#[tokio::test]
async fn test_auto_dream_trigger() {
    let mcp = mock_mcp_context();

    // IC < 0.5, auto_dream = true -> should trigger
    let triggered = check_and_trigger_dream(&mcp, 0.45, true).await;
    assert!(triggered);

    // IC >= 0.5, auto_dream = true -> should not trigger
    let triggered = check_and_trigger_dream(&mcp, 0.50, true).await;
    assert!(!triggered);

    // IC < 0.5, auto_dream = false -> should not trigger
    let triggered = check_and_trigger_dream(&mcp, 0.45, false).await;
    assert!(!triggered);
}
```

### TC-SESSION-10b: Entropy Trigger
```rust
#[tokio::test]
async fn test_entropy_trigger() {
    let mcp = mock_mcp_context();

    // entropy > 0.7 -> should trigger mental_check
    let triggered = check_entropy_and_trigger(&mcp, 0.75).await;
    assert!(triggered);

    // entropy <= 0.7 -> should not trigger
    let triggered = check_entropy_and_trigger(&mcp, 0.70).await;
    assert!(!triggered);
}
```

## Exit Conditions

- **Success**: Dream triggered via MCP on IC < 0.5 with --auto-dream
- **Failure**: Blocking behavior, missing stderr output, MCP call failure - error out with detailed logging

## Next Task

After completion, proceed to **009-TASK-SESSION-09** (format_brief() Performance).

```xml
</task_spec>
```
