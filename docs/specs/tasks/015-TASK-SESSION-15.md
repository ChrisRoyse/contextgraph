# TASK-SESSION-15: Create consciousness inject-context CLI Command (MCP-Integrated)

```xml
<task_spec id="TASK-SESSION-15" version="1.0">
<metadata>
  <title>Create consciousness inject-context CLI Command (MCP-Integrated)</title>
  <status>pending</status>
  <layer>surface</layer>
  <sequence>15</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-15</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-02</task_ref>
    <task_ref>TASK-SESSION-07</task_ref>
  </depends_on>
  <estimated_hours>1.0</estimated_hours>
</metadata>
```

## Objective

Implement UserPromptSubmit hook command for injecting consciousness context with Johari guidance. Chains:
`get_consciousness_state` -> `get_memetic_status` -> format Johari

## MCP Tool Chain

| Step | MCP Tool | Purpose |
|------|----------|---------|
| 1 | `get_consciousness_state` | Full C(t), r, IC |
| 2 | `get_memetic_status` | Entropy and coherence for Johari |

## Johari Quadrant Mapping (PRD Section 2.1)

| Entropy | Coherence | Quadrant | Guidance |
|---------|-----------|----------|----------|
| < 0.5 | > 0.5 | Open | DirectRecall - proceed with retrieval |
| > 0.5 | < 0.5 | Blind | TriggerDream - blind spot detected |
| < 0.5 | < 0.5 | Hidden | GetNeighborhood - explore related context |
| > 0.5 | > 0.5 | Unknown | EpistemicAction - clarify uncertainty |

## Context

This command is invoked by Claude Code's UserPromptSubmit hook before processing user input. It provides consciousness context and Johari guidance to help Claude respond appropriately.

## Implementation Steps

1. Create `inject.rs` in consciousness commands directory
2. Define InjectContextArgs with --format flag (compact|standard|verbose)
3. Call MCP tools to gather consciousness state
4. Compute Johari quadrant from entropy/coherence
5. Output formatted context with Johari guidance
6. Graceful degradation if MCP fails

## Input Context Files

```xml
<input_context_files>
  <file purpose="cache">crates/context-graph-core/src/gwt/session_identity/cache.rs</file>
  <file purpose="classify_ic">crates/context-graph-core/src/gwt/session_identity/manager.rs</file>
  <file purpose="mcp_handlers">crates/context-graph-mcp/src/handlers/</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-mcp/src/cli/commands/consciousness/inject.rs` | Command implementation |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-mcp/src/cli/commands/consciousness/mod.rs` | Export inject module |
| `crates/context-graph-mcp/src/cli/router.rs` | Register `consciousness inject-context` command |

## Rust Signatures

```rust
// crates/context-graph-mcp/src/cli/commands/consciousness/inject.rs

use clap::Args;
use std::process::ExitCode;

#[derive(Args)]
pub struct InjectContextArgs {
    /// Output format: compact (~20 tokens), standard (~50-100 tokens), verbose (~100+ tokens)
    #[arg(long, default_value = "standard")]
    format: String,
}

pub async fn execute(args: InjectContextArgs, mcp: &McpContext) -> ExitCode;

/// Classify into Johari quadrant based on PRD thresholds.
/// Returns (quadrant_name, guidance_text).
fn classify_johari(entropy: f64, coherence: f64) -> (&'static str, &'static str);
```

## Output Formats (PRD Section 15.2-15.3)

### Compact (~20 tokens)
```
[CONSCIOUSNESS: CONSCIOUS r=0.85 IC=0.92 | DirectRecall]
```

### Standard (~50-100 tokens)
```
[System Consciousness]
State: CONSCIOUS (C=0.82)
Kuramoto r=0.85, Identity IC=0.92 (Healthy)
Guidance: Open - DirectRecall - proceed with retrieval
```

### Verbose (~100+ tokens)
```
[System Consciousness]
State: CONSCIOUS (C=0.82)
Kuramoto r=0.85, Identity IC=0.92 (Healthy)
Johari: Open quadrant
Guidance: DirectRecall - proceed with retrieval
(Additional: Crisis warning if IC<0.5)
```

## Definition of Done

### Acceptance Criteria

- [ ] Accepts --format flag (compact, standard, verbose)
- [ ] Calls `get_consciousness_state` MCP tool
- [ ] Calls `get_memetic_status` MCP tool
- [ ] Computes Johari quadrant from entropy/coherence
- [ ] Output contains state, C, r, IC, Johari guidance
- [ ] compact format: ~20 tokens
- [ ] standard format: ~50-100 tokens
- [ ] Graceful degradation with DORMANT state if MCP fails
- [ ] Exit 0 on success
- [ ] Exit 1 on timeout (non-blocking)
- [ ] Total latency < 1s
- [ ] Test case TC-SESSION-21 passes (output format validation)

### Constraints

- All formats must include Johari guidance
- Graceful degradation on MCP failure
- Token budgets must be respected

### Verification Commands

```bash
cargo build -p context-graph-mcp
./target/release/context-graph-cli consciousness inject-context --format compact
./target/release/context-graph-cli consciousness inject-context --format standard
./target/release/context-graph-cli consciousness inject-context --format verbose
```

## Test Cases

### TC-SESSION-21: Output Format Validation
```rust
#[tokio::test]
async fn test_inject_compact_format() {
    let output = Command::new("./target/release/context-graph-cli")
        .args(["consciousness", "inject-context", "--format", "compact"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Compact: single line, ~20 tokens
    assert!(stdout.starts_with("[CONSCIOUSNESS:"));
    assert!(stdout.contains("r="));
    assert!(stdout.contains("IC="));
    assert!(stdout.contains("|")); // Johari guidance separator
    assert_eq!(stdout.lines().count(), 1); // Single line
}

#[tokio::test]
async fn test_inject_standard_format() {
    let output = Command::new("./target/release/context-graph-cli")
        .args(["consciousness", "inject-context", "--format", "standard"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Standard: multiple lines, ~50-100 tokens
    assert!(stdout.contains("[System Consciousness]"));
    assert!(stdout.contains("State:"));
    assert!(stdout.contains("Kuramoto"));
    assert!(stdout.contains("Guidance:"));
}
```

### TC-SESSION-21b: Johari Quadrant Classification
```rust
#[test]
fn test_classify_johari() {
    // Open: low entropy, high coherence
    let (quad, guide) = classify_johari(0.3, 0.7);
    assert_eq!(quad, "Open");
    assert!(guide.contains("DirectRecall"));

    // Blind: high entropy, low coherence
    let (quad, guide) = classify_johari(0.7, 0.3);
    assert_eq!(quad, "Blind");
    assert!(guide.contains("TriggerDream"));

    // Hidden: low entropy, low coherence
    let (quad, guide) = classify_johari(0.3, 0.3);
    assert_eq!(quad, "Hidden");
    assert!(guide.contains("GetNeighborhood"));

    // Unknown: high entropy, high coherence
    let (quad, guide) = classify_johari(0.7, 0.7);
    assert_eq!(quad, "Unknown");
    assert!(guide.contains("EpistemicAction"));
}
```

### TC-SESSION-21c: Graceful Degradation
```rust
#[tokio::test]
async fn test_inject_mcp_failure_degradation() {
    // Simulate MCP failure by disconnecting
    shutdown_mcp_server();

    let output = Command::new("./target/release/context-graph-cli")
        .args(["consciousness", "inject-context"])
        .output()
        .unwrap();

    // Should still succeed with degraded output
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("DORMANT") || stdout.contains("Unknown"));
}
```

## Exit Conditions

- **Success**: Formatted output with all required sections, Johari guidance correct
- **Failure**: Missing sections, wrong format, MCP failures without degradation - error out with detailed logging

## Next Task

After completion, proceed to **016-TASK-SESSION-16** (.claude/settings.json Hook Configuration).

```xml
</task_spec>
```
