# TASK-SESSION-02: Create IdentityCache Singleton (PreToolUse Hot Path)

```xml
<task_spec id="TASK-SESSION-02" version="1.0">
<metadata>
  <title>Create IdentityCache Singleton (PreToolUse Hot Path)</title>
  <status>pending</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-02</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-01</task_ref>
  </depends_on>
  <estimated_hours>1.5</estimated_hours>
</metadata>
```

## Objective

Implement thread-safe IdentityCache singleton with OnceLock pattern for PreToolUse hot path access (<50ms target). NO disk I/O in the hot path.

## Context

This cache is critical for the PreToolUse hook which has a 100ms timeout from Claude Code. Our target is <50ms. The cache stores the current consciousness state, IC value, and Kuramoto r so that `format_brief()` can return immediately without disk access.

## Performance Budget

```
50ms total budget:
  - Binary startup (precompiled): ~15ms
  - RocksDB cache hit: ~5ms (warm cache: 0ms)
  - Format output: ~2ms
  - Buffer: ~28ms
```

## Implementation Steps

1. Create `cache.rs` in session_identity module
2. Define static IDENTITY_CACHE with OnceLock<RwLock<Option<IdentityCacheInner>>>
3. Implement IdentityCacheInner struct with 4 fields
4. Implement IdentityCache with get(), format_brief(), is_warm() methods
5. Implement update_cache() free function for atomic updates
6. Implement clear_cache() for testing
7. Add compute_kuramoto_r() helper function

## Input Context Files

```xml
<input_context_files>
  <file purpose="snapshot_struct">crates/context-graph-core/src/gwt/session_identity/types.rs</file>
  <file purpose="consciousness_state">crates/context-graph-core/src/gwt/state_machine/types.rs</file>
  <file purpose="module_export">crates/context-graph-core/src/gwt/session_identity/mod.rs</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-core/src/gwt/session_identity/cache.rs` | Cache implementation |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/gwt/session_identity/mod.rs` | Export cache module |

## Rust Signatures

```rust
// crates/context-graph-core/src/gwt/session_identity/cache.rs

use std::sync::{OnceLock, RwLock};

static IDENTITY_CACHE: OnceLock<RwLock<Option<IdentityCacheInner>>> = OnceLock::new();

#[derive(Debug, Clone)]
struct IdentityCacheInner {
    current_ic: f32,
    kuramoto_r: f32,
    consciousness_state: ConsciousnessState,
    session_id: String,
}

pub struct IdentityCache;

impl IdentityCache {
    pub fn get() -> Option<(f32, f32, ConsciousnessState, String)>;

    /// Format brief output for PreToolUse hook.
    /// Target: <15 tokens, <5ms.
    #[inline]
    pub fn format_brief() -> String;

    #[inline]
    pub fn is_warm() -> bool;
}

pub fn update_cache(snapshot: &SessionIdentitySnapshot, ic: f32);
pub fn clear_cache();
fn compute_kuramoto_r(phases: &[f64; KURAMOTO_N]) -> f32;
```

## Definition of Done

### Acceptance Criteria

- [ ] OnceLock pattern initializes lazily on first access
- [ ] RwLock allows concurrent reads
- [ ] get() returns None when cache is empty
- [ ] format_brief() returns "[C:STATE r=X.XX IC=X.XX]" format
- [ ] format_brief() returns "[C:? r=? IC=?]" on cold cache
- [ ] update_cache() atomically updates all fields
- [ ] format_brief() completes in <1ms (well under 50ms budget)
- [ ] Test case TC-SESSION-03 passes (format_brief output format)

### Constraints

- NO disk I/O in any hot path method
- Thread-safe for concurrent access
- Single String allocation per format_brief() call (~30 bytes)
- RwLock read lock held for minimum duration

### Verification Commands

```bash
cargo build -p context-graph-core
cargo test -p context-graph-core cache
```

## Test Cases

### TC-SESSION-03: format_brief Output Format
```rust
#[test]
fn test_format_brief_output() {
    // Cold cache
    assert_eq!(IdentityCache::format_brief(), "[C:? r=? IC=?]");

    // Warm cache
    let snapshot = SessionIdentitySnapshot::default();
    update_cache(&snapshot, 0.85);
    let brief = IdentityCache::format_brief();
    assert!(brief.starts_with("[C:"));
    assert!(brief.contains("r="));
    assert!(brief.contains("IC="));
}
```

## Exit Conditions

- **Success**: All acceptance criteria met, thread safety verified
- **Failure**: Race conditions, deadlocks, format mismatch - error out with detailed logging

## Next Task

After completion, proceed to **003-TASK-SESSION-03** (ConsciousnessState.short_name()).

```xml
</task_spec>
```
