# TASK-SESSION-09: Implement format_brief() Performance

```xml
<task_spec id="TASK-SESSION-09" version="1.0">
<metadata>
  <title>Implement format_brief() Performance Optimization</title>
  <status>pending</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-09</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-02</task_ref>
  </depends_on>
  <estimated_hours>1.0</estimated_hours>
</metadata>
```

## Objective

Ensure IdentityCache.format_brief() completes in under 1ms with inline annotation and minimal allocations. This is the PreToolUse hot path - must be <50ms total.

## Performance Budget

```
50ms total budget:
  - Binary startup (precompiled): ~15ms
  - RocksDB cache hit: ~5ms (warm cache: 0ms)
  - Format output: ~2ms
  - Buffer: ~28ms
```

**format_brief() target: <100 microseconds p95**

## Context

The PreToolUse hook has a hard 100ms timeout from Claude Code. Our target is <50ms. The format_brief() function is called on every tool invocation and must be extremely fast:
- No disk I/O
- No network calls
- Minimal allocations
- Inline everything possible

## Implementation Steps

1. Verify #[inline] annotation on format_brief()
2. Use static format string pattern
3. Minimize heap allocations (single String output ~30 bytes)
4. Add benchmark test measuring latency
5. Verify p95 latency < 100 microseconds
6. Add criterion benchmark for continuous monitoring

## Input Context Files

```xml
<input_context_files>
  <file purpose="cache_impl">crates/context-graph-core/src/gwt/session_identity/cache.rs</file>
  <file purpose="short_name">crates/context-graph-core/src/gwt/state_machine/types.rs</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-core/benches/session_identity.rs` | Criterion benchmark |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/gwt/session_identity/cache.rs` | Optimize format_brief |
| `crates/context-graph-core/Cargo.toml` | Add criterion dev-dependency if missing |

## Optimized Implementation

```rust
impl IdentityCache {
    /// Format brief output for PreToolUse hook.
    /// Target: <15 tokens, <1ms.
    #[inline]
    pub fn format_brief() -> String {
        // Fast path: use in-memory cache
        if let Some(cache) = IDENTITY_CACHE.get() {
            if let Ok(guard) = cache.read() {
                if let Some(inner) = guard.as_ref() {
                    return format!(
                        "[C:{} r={:.2} IC={:.2}]",
                        inner.consciousness_state.short_name(),
                        inner.kuramoto_r,
                        inner.current_ic
                    );
                }
            }
        }
        // Cold start fallback
        "[C:? r=? IC=?]".to_string()
    }
}
```

## Benchmark Setup

```rust
// crates/context-graph-core/benches/session_identity.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_format_brief(c: &mut Criterion) {
    // Warm the cache first
    let snapshot = SessionIdentitySnapshot::default();
    update_cache(&snapshot, 0.85);

    c.bench_function("format_brief_warm", |b| {
        b.iter(|| {
            black_box(IdentityCache::format_brief())
        })
    });
}

fn bench_format_brief_cold(c: &mut Criterion) {
    clear_cache();

    c.bench_function("format_brief_cold", |b| {
        b.iter(|| {
            black_box(IdentityCache::format_brief())
        })
    });
}

criterion_group!(benches, bench_format_brief, bench_format_brief_cold);
criterion_main!(benches);
```

## Definition of Done

### Acceptance Criteria

- [ ] Method marked #[inline]
- [ ] Single String allocation per call (~30 bytes)
- [ ] RwLock read lock held for minimum duration
- [ ] No disk I/O in hot path
- [ ] 10,000 iterations complete in under 10ms total
- [ ] p95 latency < 100 microseconds
- [ ] p99 latency < 500 microseconds
- [ ] Test case TC-SESSION-11 passes (latency benchmark)
- [ ] Criterion benchmark added and passes

### Constraints

- NO disk I/O
- NO network calls
- Single allocation per call
- Read lock held briefly

### Verification Commands

```bash
cargo build -p context-graph-core --release
cargo bench -p context-graph-core format_brief
cargo test -p context-graph-core format_brief_performance
```

## Test Cases

### TC-SESSION-11: Latency Benchmark
```rust
#[test]
fn test_format_brief_latency() {
    // Warm the cache
    let snapshot = SessionIdentitySnapshot::default();
    update_cache(&snapshot, 0.85);

    let start = std::time::Instant::now();
    for _ in 0..10_000 {
        let _ = IdentityCache::format_brief();
    }
    let elapsed = start.elapsed();

    // 10,000 iterations in < 10ms = < 1us average
    assert!(elapsed.as_millis() < 10, "format_brief too slow: {:?}", elapsed);
}

#[test]
fn test_format_brief_cold_cache() {
    clear_cache();

    let start = std::time::Instant::now();
    let result = IdentityCache::format_brief();
    let elapsed = start.elapsed();

    assert_eq!(result, "[C:? r=? IC=?]");
    assert!(elapsed.as_micros() < 100, "cold cache too slow: {:?}", elapsed);
}
```

## Exit Conditions

- **Success**: format_brief() consistently under 1ms
- **Failure**: Latency exceeds 1ms at p95 - error out with detailed logging

## Next Task

After completion, proceed to **010-TASK-SESSION-10** (update_cache() Function).

```xml
</task_spec>
```
