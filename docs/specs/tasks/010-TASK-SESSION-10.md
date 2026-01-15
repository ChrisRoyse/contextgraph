# TASK-SESSION-10: Implement update_cache() Function

```xml
<task_spec id="TASK-SESSION-10" version="1.0">
<metadata>
  <title>Implement update_cache() Function</title>
  <status>pending</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-10</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-02</task_ref>
  </depends_on>
  <estimated_hours>1.5</estimated_hours>
</metadata>
```

## Objective

Implement atomic cache update function that safely updates IdentityCache after identity restoration or MCP tool responses.

## Context

The cache must be updated:
1. After `restore_identity` loads from storage
2. After `check_identity` gets IC from MCP
3. After any MCP tool returns consciousness state

Updates must be atomic - either all fields update or none do. The cache is accessed by multiple threads (PreToolUse hot path).

## Implementation Steps

1. Implement update_cache() in cache.rs
2. Compute ConsciousnessState from snapshot.consciousness
3. Compute Kuramoto r from snapshot.kuramoto_phases
4. Acquire write lock and update all fields atomically
5. Verify thread safety under concurrent access
6. Add update_cache_from_mcp() for MCP response parsing

## Input Context Files

```xml
<input_context_files>
  <file purpose="cache_struct">crates/context-graph-core/src/gwt/session_identity/cache.rs</file>
  <file purpose="snapshot_type">crates/context-graph-core/src/gwt/session_identity/types.rs</file>
  <file purpose="consciousness_state">crates/context-graph-core/src/gwt/state_machine/types.rs</file>
</input_context_files>
```

## Files to Create

None.

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/gwt/session_identity/cache.rs` | Complete update_cache implementation |

## Rust Signatures

```rust
// crates/context-graph-core/src/gwt/session_identity/cache.rs

/// Update cache from SessionIdentitySnapshot.
/// Atomically updates all 4 cache fields.
pub fn update_cache(snapshot: &SessionIdentitySnapshot, ic: f32) {
    let cache = IDENTITY_CACHE.get_or_init(|| RwLock::new(None));
    let consciousness_state = ConsciousnessState::from_level(snapshot.consciousness);
    let r = compute_kuramoto_r(&snapshot.kuramoto_phases);

    if let Ok(mut guard) = cache.write() {
        *guard = Some(IdentityCacheInner {
            current_ic: ic,
            kuramoto_r: r,
            consciousness_state,
            session_id: snapshot.session_id.clone(),
        });
    }
}

/// Update cache from MCP tool responses.
/// Parses JSON from get_ego_state, get_kuramoto_state, get_consciousness_state.
pub fn update_cache_from_mcp(
    ego: &serde_json::Value,
    kuramoto: &serde_json::Value,
    consciousness: &serde_json::Value
) {
    let cache = IDENTITY_CACHE.get_or_init(|| RwLock::new(None));

    // Extract IC from consciousness response
    let ic = consciousness.get("ic")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0) as f32;

    // Extract r from kuramoto response
    let r = kuramoto.get("order_parameter")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;

    // Extract state from consciousness response
    let state_str = consciousness.get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("dormant");
    let consciousness_state = ConsciousnessState::from_str(state_str);

    // Extract session_id from ego response
    let session_id = ego.get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    if let Ok(mut guard) = cache.write() {
        *guard = Some(IdentityCacheInner {
            current_ic: ic,
            kuramoto_r: r,
            consciousness_state,
            session_id,
        });
    }
}

/// Compute Kuramoto order parameter r from phases.
/// r = |1/N * sum(e^(i*theta_j))| where i is imaginary unit.
fn compute_kuramoto_r(phases: &[f64; KURAMOTO_N]) -> f32 {
    let (sum_cos, sum_sin) = phases.iter()
        .fold((0.0, 0.0), |(c, s), &theta| {
            (c + theta.cos(), s + theta.sin())
        });
    let n = KURAMOTO_N as f64;
    let r = ((sum_cos / n).powi(2) + (sum_sin / n).powi(2)).sqrt();
    r as f32
}
```

## Definition of Done

### Acceptance Criteria

- [ ] Write lock acquired successfully
- [ ] All 4 cache fields updated atomically
- [ ] ConsciousnessState computed from consciousness value
- [ ] Kuramoto r computed from phases using order parameter formula
- [ ] No data races under concurrent updates
- [ ] Subsequent get() returns updated values
- [ ] update_cache_from_mcp() correctly parses MCP JSON responses
- [ ] Thread safety verified with concurrent test

### Constraints

- Must be atomic (all-or-nothing)
- Write lock held for minimum duration
- Graceful fallback on JSON parse errors
- compute_kuramoto_r must match standard formula

### Verification Commands

```bash
cargo build -p context-graph-core
cargo test -p context-graph-core update_cache
```

## Test Cases

### TC-SESSION-UPDATE-01: Atomic Update
```rust
#[test]
fn test_update_cache_atomic() {
    clear_cache();

    let mut snapshot = SessionIdentitySnapshot::new("test-session");
    snapshot.consciousness = 0.85;
    snapshot.kuramoto_phases = [0.0; KURAMOTO_N]; // All aligned = r = 1.0

    update_cache(&snapshot, 0.92);

    let (ic, r, state, session_id) = IdentityCache::get().unwrap();
    assert_eq!(session_id, "test-session");
    assert!((ic - 0.92).abs() < 0.01);
    assert!((r - 1.0).abs() < 0.01);
}
```

### TC-SESSION-UPDATE-02: Concurrent Safety
```rust
#[test]
fn test_update_cache_concurrent() {
    use std::thread;

    clear_cache();

    let handles: Vec<_> = (0..10).map(|i| {
        thread::spawn(move || {
            let snapshot = SessionIdentitySnapshot::new(format!("session-{}", i));
            update_cache(&snapshot, i as f32 / 10.0);
            IdentityCache::format_brief()
        })
    }).collect();

    // All threads should complete without deadlock
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result.starts_with("[C:"));
    }
}
```

### TC-SESSION-UPDATE-03: MCP Response Parsing
```rust
#[test]
fn test_update_cache_from_mcp() {
    clear_cache();

    let ego = serde_json::json!({"session_id": "mcp-session"});
    let kuramoto = serde_json::json!({"order_parameter": 0.85});
    let consciousness = serde_json::json!({"ic": 0.92, "state": "conscious"});

    update_cache_from_mcp(&ego, &kuramoto, &consciousness);

    let (ic, r, state, session_id) = IdentityCache::get().unwrap();
    assert_eq!(session_id, "mcp-session");
    assert!((ic - 0.92).abs() < 0.01);
    assert!((r - 0.85).abs() < 0.01);
    assert_eq!(state, ConsciousnessState::Conscious);
}
```

## Exit Conditions

- **Success**: Atomic updates work correctly under concurrent access
- **Failure**: Data races, partial updates - error out with detailed logging

## Next Task

After completion, proceed to **011-TASK-SESSION-11** (consciousness brief CLI Command).

```xml
</task_spec>
```
