# TASK-SESSION-11: Create consciousness brief CLI Command (<50ms)

```xml
<task_spec id="TASK-SESSION-11" version="1.0">
<metadata>
  <title>Create consciousness brief CLI Command (&lt;50ms)</title>
  <status>pending</status>
  <layer>surface</layer>
  <sequence>11</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-11</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-02</task_ref>
    <task_ref>TASK-SESSION-03</task_ref>
    <task_ref>TASK-SESSION-09</task_ref>
  </depends_on>
  <estimated_hours>1.0</estimated_hours>
</metadata>
```

## Objective

Implement PreToolUse hot path command with cache-only access, NO stdin parsing, NO disk I/O. Target: <50ms p95.

## Key Optimizations

- No stdin JSON parsing
- No RocksDB read
- Static format string
- No allocations in hot path (beyond single output string)

## Context

This command is invoked by Claude Code's PreToolUse hook on every tool call. It has a hard 100ms timeout. Our target is <50ms to leave buffer for process startup.

## Implementation Steps

1. Create `brief.rs` in consciousness commands directory
2. Define BriefArgs struct (empty - no arguments)
3. Implement execute() function using IdentityCache.format_brief()
4. Output to stdout, always exit 0
5. Verify no stdin reads or disk I/O
6. Register command in CLI router

## Input Context Files

```xml
<input_context_files>
  <file purpose="cache">crates/context-graph-core/src/gwt/session_identity/cache.rs</file>
  <file purpose="cli_pattern">crates/context-graph-mcp/src/cli/commands/</file>
  <file purpose="router">crates/context-graph-mcp/src/cli/router.rs</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-mcp/src/cli/commands/consciousness/brief.rs` | Command implementation |
| `crates/context-graph-mcp/src/cli/commands/consciousness/mod.rs` | Module file (if not exists) |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-mcp/src/cli/router.rs` | Register `consciousness brief` command |
| `crates/context-graph-mcp/src/cli/commands/mod.rs` | Export consciousness module |

## Rust Signatures

```rust
// crates/context-graph-mcp/src/cli/commands/consciousness/brief.rs

use clap::Args;
use std::process::ExitCode;

#[derive(Args)]
pub struct BriefArgs {}

/// Ultra-fast consciousness brief for PreToolUse hook.
/// No stdin parsing, no disk I/O (cache only).
pub fn execute(_args: BriefArgs) -> ExitCode {
    use context_graph_core::gwt::session_identity::cache::IdentityCache;
    let brief = IdentityCache::format_brief();
    println!("{}", brief);
    ExitCode::SUCCESS
}
```

## Output Format

| Cache State | Output |
|-------------|--------|
| Warm | `[C:CON r=0.85 IC=0.92]` |
| Cold | `[C:? r=? IC=?]` |

## Definition of Done

### Acceptance Criteria

- [ ] No stdin JSON parsing
- [ ] No RocksDB disk I/O
- [ ] Uses IdentityCache.format_brief() only
- [ ] Output format: "[C:STATE r=X.XX IC=X.XX]"
- [ ] Cold start fallback: "[C:? r=? IC=?]"
- [ ] Always exits with code 0
- [ ] Total latency < 50ms p95
- [ ] Test case TC-SESSION-12 passes (warm cache)
- [ ] Test case TC-SESSION-13 passes (cold cache)

### Constraints

- NO stdin reads
- NO disk I/O
- Exit 0 always (never block)
- Single println! call

### Verification Commands

```bash
cargo build -p context-graph-mcp --release
time ./target/release/context-graph-cli consciousness brief
```

## Test Cases

### TC-SESSION-12: Warm Cache Output
```rust
#[test]
fn test_brief_warm_cache() {
    // Setup warm cache
    let snapshot = SessionIdentitySnapshot::default();
    update_cache(&snapshot, 0.85);

    let output = Command::new("./target/release/context-graph-cli")
        .args(["consciousness", "brief"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[C:"));
    assert!(stdout.contains("r="));
    assert!(stdout.contains("IC="));
}
```

### TC-SESSION-13: Cold Cache Output
```rust
#[test]
fn test_brief_cold_cache() {
    clear_cache();

    let output = Command::new("./target/release/context-graph-cli")
        .args(["consciousness", "brief"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "[C:? r=? IC=?]");
}
```

### TC-SESSION-LATENCY: Performance Test
```rust
#[test]
fn test_brief_latency() {
    let start = std::time::Instant::now();

    for _ in 0..10 {
        let output = Command::new("./target/release/context-graph-cli")
            .args(["consciousness", "brief"])
            .output()
            .unwrap();
        assert!(output.status.success());
    }

    let elapsed = start.elapsed();
    // 10 invocations in < 500ms = < 50ms each
    assert!(elapsed.as_millis() < 500, "Too slow: {:?}", elapsed);
}
```

## Exit Conditions

- **Success**: Command completes in under 50ms p95 with correct output
- **Failure**: Disk I/O detected, latency exceeded - error out with detailed logging

## Next Task

After completion, proceed to **012-TASK-SESSION-12** (session restore-identity CLI Command).

```xml
</task_spec>
```
