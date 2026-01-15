# TASK-SESSION-07: Implement classify_ic() Function

```xml
<task_spec id="TASK-SESSION-07" version="1.0">
<metadata>
  <title>Implement classify_ic() Function</title>
  <status>pending</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-07</requirement_ref>
    <constitution_ref>IDENTITY-002</constitution_ref>
  </implements>
  <depends_on><!-- None --></depends_on>
  <estimated_hours>0.5</estimated_hours>
</metadata>
```

## Objective

Implement IC classification function with IDENTITY-002 constitution thresholds:
- **Healthy**: IC >= 0.9
- **Good**: IC >= 0.7
- **Warning**: IC >= 0.5
- **Degraded**: IC < 0.5

## Context

The IC (Identity Continuity) value is a key metric that determines system behavior:
- IC < 0.5 triggers automatic dream consolidation (AP-26, AP-38)
- IC < 0.7 triggers warnings
- Classification is used in CLI output and decision making

## Implementation Steps

1. Add classify_ic() function in manager.rs
2. Add is_ic_crisis() helper (IC < 0.5)
3. Add is_ic_warning() helper (0.5 <= IC < 0.7)
4. Add classify_sync() for Kuramoto r classification
5. Add unit tests for boundary values

## Input Context Files

```xml
<input_context_files>
  <file purpose="add_to">crates/context-graph-core/src/gwt/session_identity/manager.rs</file>
  <file purpose="constitution">docs2/constitution.yaml</file>
</input_context_files>
```

## Files to Create

None.

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/gwt/session_identity/manager.rs` | Add classification functions |

## Rust Signatures

```rust
// crates/context-graph-core/src/gwt/session_identity/manager.rs

/// Classify IC value per IDENTITY-002 constitution thresholds.
#[inline]
pub fn classify_ic(ic: f32) -> &'static str {
    match ic {
        ic if ic >= 0.9 => "Healthy",
        ic if ic >= 0.7 => "Good",
        ic if ic >= 0.5 => "Warning",
        _ => "Degraded",
    }
}

/// Returns true if IC indicates identity crisis (< 0.5).
/// Triggers automatic dream per AP-26, AP-38.
#[inline]
pub fn is_ic_crisis(ic: f32) -> bool {
    ic < 0.5
}

/// Returns true if IC is in warning range (0.5 <= IC < 0.7).
#[inline]
pub fn is_ic_warning(ic: f32) -> bool {
    ic >= 0.5 && ic < 0.7
}

/// Classify Kuramoto order parameter r.
#[inline]
pub fn classify_sync(r: f64) -> &'static str {
    match r {
        r if r >= 0.8 => "Good synchronization",
        r if r >= 0.5 => "Partial synchronization",
        _ => "Fragmented",
    }
}
```

## IC Threshold Table (IDENTITY-002)

| IC Range | Classification | Action |
|----------|---------------|--------|
| >= 0.9 | Healthy | Normal operation |
| 0.7 - 0.89 | Good | Normal operation |
| 0.5 - 0.69 | Warning | Log warning |
| < 0.5 | Degraded | Auto-dream trigger |

## Definition of Done

### Acceptance Criteria

- [ ] classify_ic(0.91) returns "Healthy"
- [ ] classify_ic(0.90) returns "Healthy"
- [ ] classify_ic(0.89) returns "Good"
- [ ] classify_ic(0.70) returns "Good"
- [ ] classify_ic(0.69) returns "Warning"
- [ ] classify_ic(0.50) returns "Warning"
- [ ] classify_ic(0.49) returns "Degraded"
- [ ] is_ic_crisis returns true for IC < 0.5
- [ ] is_ic_warning returns true for 0.5 <= IC < 0.7
- [ ] Test case TC-SESSION-09 passes (threshold boundaries)

### Constraints

- All functions must be #[inline]
- Return &'static str (no allocation)
- Boundary conditions must be exact per IDENTITY-002

### Verification Commands

```bash
cargo build -p context-graph-core
cargo test -p context-graph-core classify_ic
```

## Test Cases

### TC-SESSION-09: Threshold Boundaries
```rust
#[test]
fn test_classify_ic_boundaries() {
    // Healthy boundary
    assert_eq!(classify_ic(0.90), "Healthy");
    assert_eq!(classify_ic(0.899), "Good");

    // Good boundary
    assert_eq!(classify_ic(0.70), "Good");
    assert_eq!(classify_ic(0.699), "Warning");

    // Warning boundary
    assert_eq!(classify_ic(0.50), "Warning");
    assert_eq!(classify_ic(0.499), "Degraded");

    // Edge cases
    assert_eq!(classify_ic(1.0), "Healthy");
    assert_eq!(classify_ic(0.0), "Degraded");
    assert_eq!(classify_ic(-0.1), "Degraded");
}

#[test]
fn test_is_ic_crisis() {
    assert!(!is_ic_crisis(0.5));
    assert!(is_ic_crisis(0.49));
    assert!(is_ic_crisis(0.0));
}

#[test]
fn test_is_ic_warning() {
    assert!(!is_ic_warning(0.7));
    assert!(is_ic_warning(0.69));
    assert!(is_ic_warning(0.5));
    assert!(!is_ic_warning(0.49));
}
```

## Exit Conditions

- **Success**: All threshold boundaries correct per IDENTITY-002
- **Failure**: Wrong classification at boundaries - error out with detailed logging

## Next Task

After completion, proceed to **008-TASK-SESSION-08** (dream_trigger Module).

```xml
</task_spec>
```
