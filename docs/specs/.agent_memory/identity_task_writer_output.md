# Identity Task Spec Writer Output

## Session: 2026-01-12
## Agent: Identity Task Spec Writer
## Status: COMPLETE

---

## Files Created

| File | Absolute Path | Purpose |
|------|---------------|---------|
| TASK-IDENTITY-P0-001.md | `/home/cabdru/contextgraph/docs/specs/tasks/TASK-IDENTITY-P0-001.md` | Fix Dual Monitor Desync |
| Agent Memory | `/home/cabdru/contextgraph/docs/specs/.agent_memory/identity_task_writer_output.md` | This file - coordination with next agent |

---

## Task Summary

### TASK-IDENTITY-P0-001: Fix Dual Monitor Desync

**Key Insight**: Two separate `IdentityContinuityMonitor` instances exist - MCP tools read from the WRONG one.

**Current State**:
- `IdentityContinuityListener` (identity.rs:65) creates its own monitor
- `GwtSystemProviderImpl` (gwt_providers.rs:149) creates SEPARATE monitor
- Listener's monitor receives events and computes IC correctly
- Provider's monitor is NEVER updated - MCP reads default IC=0.0

**Fix Required**:
1. Change `GwtSystemProviderImpl.identity_monitor` from `TokioRwLock<Monitor>` to `Arc<TokioRwLock<Monitor>>`
2. Add `with_shared_monitor()` constructor accepting shared reference
3. Add factory function `create_gwt_provider_with_listener()` for wiring
4. Add warning log to `new()` about isolated monitor usage

**Files to Modify**:
- `crates/context-graph-mcp/src/handlers/gwt_providers.rs`
- `crates/context-graph-mcp/src/handlers/mod.rs`

**Constitution Rules**: AP-40, IDENTITY-007

---

## Key Implementation Insights

### 1. Monitor Sharing Pattern

The fix uses Arc for shared ownership:
```rust
// Before: Isolated monitor
identity_monitor: TokioRwLock<IdentityContinuityMonitor>

// After: Shared monitor
identity_monitor: Arc<TokioRwLock<IdentityContinuityMonitor>>
```

Both components already use `tokio::sync::RwLock`, so no lock type conversion needed.

### 2. Listener Already Exposes Monitor

`IdentityContinuityListener` already has a `monitor()` method (line 131):
```rust
pub fn monitor(&self) -> Arc<RwLock<IdentityContinuityMonitor>> {
    Arc::clone(&self.monitor)
}
```

This makes sharing straightforward - the provider just needs to accept this Arc.

### 3. Backward Compatibility

The `new()` constructor remains for existing unit tests but logs a warning:
```rust
pub fn new() -> Self {
    tracing::warn!(
        "GwtSystemProviderImpl::new() creates isolated monitor - \
         use with_shared_monitor() for production"
    );
    // ... creates isolated monitor
}
```

Production code should use `with_shared_monitor()` via factory function.

### 4. Constitution Compliance Points

| Rule | Description | Status After Fix |
|------|-------------|------------------|
| AP-40 | MCP must read from correct monitor instance | COMPLIANT |
| IDENTITY-007 | IC < 0.5 auto-trigger dream | Already working (via listener) |
| IDENTITY-001 | IC = cos(PV_t, PV_{t-1}) x r(t) | Already working |
| IDENTITY-002 | Thresholds correct | Already working |
| IDENTITY-003 | FIFO eviction (max 1000) | Already working |
| IDENTITY-004 | IdentityContinuityMonitor struct | Already exists |
| IDENTITY-005 | cosine_similarity_13d public | Already public |
| IDENTITY-006 | Listener subscribes to events | Already working |

---

## What's NOT in Scope

This task modifies ONLY the provider to share the existing monitor. It does NOT change:
- `IdentityContinuityListener` implementation (working correctly)
- `IdentityContinuityMonitor` implementation (working correctly)
- MCP handler implementation (uses trait correctly)
- Crisis protocol (working correctly)
- Persistence layer

---

## Blockers Discovered

### No Major Blockers

The task is **READY** with no dependencies:
- IdentityContinuityListener already exposes `monitor()` method
- Both components use same lock type (tokio::sync::RwLock)
- Trait implementation methods already read from `self.identity_monitor`

### Minor Integration Note

The factory function `create_gwt_provider_with_listener()` requires access to both crates:
- `context-graph-core` (for listener)
- `context-graph-mcp` (for provider)

The function should be placed in `context-graph-mcp` which already depends on `context-graph-core`.

---

## Recommended Execution Order

```
1. TASK-IDENTITY-P0-001 (Dual Monitor Desync) - No dependencies
```

This is a standalone fix that doesn't require other tasks to complete first.

---

## Verification Commands

```bash
# After TASK-IDENTITY-P0-001
cargo test -p context-graph-mcp --lib handlers::gwt_providers -- --nocapture
cargo test -p context-graph-core --lib gwt::listeners::identity -- --nocapture

# Verify no duplicate monitor creation in production code
grep -r "IdentityContinuityMonitor::new()" crates/context-graph-mcp/src/ | grep -v "test" | wc -l

# Integration test
cargo test -p context-graph-mcp --test integration -- identity --nocapture
```

---

## Reference Documents Used

- `/home/cabdru/contextgraph/docs/sherlock-identity-continuity-report.md` - Investigation findings
- `/home/cabdru/contextgraph/docs/specs/_index.md` - Gap inventory
- `/home/cabdru/contextgraph/docs/specs/_traceability.md` - Requirements mapping
- `/home/cabdru/contextgraph/docs/specs/.agent_memory/architect_output.md` - Architecture context
- `/home/cabdru/contextgraph/docs/specs/.agent_memory/dream_task_writer_output.md` - Task format reference
- Source files in `crates/context-graph-core/src/gwt/` and `crates/context-graph-mcp/src/handlers/`

---

## Sherlock Investigation Summary

From `SHERLOCK-IDENTITY-2026-01-12`:

```
THE CRIME: Incomplete Identity Continuity Loop integration
THE CRIMINAL: GwtSystemProviderImpl.identity_monitor (line 149)
THE MOTIVE: Parallel implementation without coordination
THE METHOD: Two monitors created independently; MCP reads stale data
THE SENTENCE: GwtSystemProviderImpl must reference IdentityContinuityListener's monitor
```

This task implements the sentence: share the monitor instance via Arc.

---

*Identity Task Spec Writer Agent Complete*
*Format: prdtospec.md v1.0*
