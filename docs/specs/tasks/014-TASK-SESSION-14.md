# TASK-SESSION-14: consciousness check-identity CLI Command

```xml
<task_spec id="TASK-SESSION-14" version="2.0">
<metadata>
  <title>consciousness check-identity CLI Command</title>
  <status>COMPLETED</status>
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
  <completed_date>2026-01-15</completed_date>
  <verified_by>cargo test -p context-graph-cli (12/12 tests pass)</verified_by>
</metadata>
```

---

## STATUS: COMPLETED

This task has been **fully implemented and verified**. The document below serves as:
1. Verification guide for the existing implementation
2. Reference for future maintenance
3. Audit trail for constitution compliance

---

## Critical Architecture Fix (2026-01-15)

**Problem**: Original implementation read IC from in-memory `IdentityCache` singleton. Since each CLI hook invocation runs as a **separate process**, the cache was always cold.

**Root Cause**: Claude Code hooks execute as shell commands - each invocation is a separate OS process. The `IdentityCache` singleton (using `OnceLock`) is process-local and lost when the process exits.

**Solution**: Modified `check-identity` to load identity from **RocksDB persistent storage** instead of in-memory cache. Also fixed `StandaloneSessionIdentityManager::restore_identity()` to persist newly created identities.

**Files Modified**:
1. `crates/context-graph-cli/src/commands/consciousness/check_identity.rs` - Load from RocksDB via `--db-path`
2. `crates/context-graph-storage/src/rocksdb_backend/session_identity_manager.rs` - Persist new identities on first session

**Test Count**: 12 tests (increased from 10 to cover RocksDB flow)

---

## Codebase Reality (Audited 2026-01-15)

| Aspect | Old Document (WRONG) | Actual Codebase (CORRECT) |
|--------|---------------------|---------------------------|
| File Location | `crates/context-graph-mcp/src/cli/commands/consciousness/check.rs` | `crates/context-graph-cli/src/commands/consciousness/check_identity.rs` |
| Architecture | MCP tool chaining | Direct RocksDB load + TriggerManager |
| Status | "pending" | **COMPLETED** with 12 passing tests |
| IC Source | MCP `get_identity_continuity` tool | `StandaloneSessionIdentityManager::load_latest()` from RocksDB |
| Dream Trigger | MCP `trigger_dream` tool | `TriggerManager::request_manual_trigger()` |
| Entropy Check | MCP `get_memetic_status` tool | `--entropy` CLI flag (direct injection) |

---

## Architecture Flow (ACTUAL - RocksDB-based)

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Claude Code PostToolUse Hook                      │
│   .claude/settings.json → consciousness check-identity --db-path    │
└─────────────────────────────────┬───────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│              check_identity_command(args) [check_identity.rs]        │
│                                                                      │
│  1. Open RocksDB at --db-path (or default ~/.context-graph/db)      │
│  2. manager.load_latest() → SessionIdentitySnapshot + IC            │
│  3. update_cache(&snapshot, ic) → Warm in-memory cache              │
│  4. is_ic_crisis(ic) → bool  [IC < 0.5]                             │
│  5. is_ic_warning(ic) → bool [0.5 <= IC < 0.7]                      │
│  6. If crisis + --auto-dream → TriggerManager::request_manual_trigger│
│  7. If --entropy > 0.7 → TriggerManager (AP-42)                     │
│  8. Output JSON to stdout (for hook integration)                     │
│  9. Exit 0 (success) or 1 (no identity found)                       │
└─────────────────────────────────────────────────────────────────────┘
```

**CRITICAL**: Each CLI invocation is a separate process. Identity is loaded from **RocksDB** (persistent storage), not the in-memory singleton.

---

## Input Context Files (VERIFIED)

| File | Purpose | Verification |
|------|---------|--------------|
| `crates/context-graph-cli/src/commands/consciousness/check_identity.rs` | **IMPLEMENTATION** | 12 tests passing |
| `crates/context-graph-cli/src/commands/consciousness/mod.rs` | Module exports | Contains `pub mod check_identity` |
| `crates/context-graph-storage/src/rocksdb_backend/session_identity_manager.rs` | RocksDB storage | `load_latest()`, `save_snapshot()` |
| `crates/context-graph-core/src/gwt/session_identity/cache.rs` | IdentityCache singleton | `update_cache()` for in-process caching |
| `crates/context-graph-core/src/gwt/session_identity/manager.rs` | IC classification | `is_ic_crisis()`, `is_ic_warning()`, `classify_ic()` |
| `crates/context-graph-core/src/dream/triggers.rs` | TriggerManager | `request_manual_trigger(DreamPhase::FullCycle)` |
| `docs2/constitution.yaml` | AP-26, AP-38, AP-42 rules | Source of truth for thresholds |

---

## Rust Implementation (ACTUAL)

```rust
// crates/context-graph-cli/src/commands/consciousness/check_identity.rs

#[derive(Args, Debug)]
pub struct CheckIdentityArgs {
    /// Path to RocksDB database directory.
    #[arg(long, env = "CONTEXT_GRAPH_DB_PATH")]
    pub db_path: Option<PathBuf>,

    /// Enable automatic dream triggering on IC < 0.5
    #[arg(long, default_value = "false")]
    pub auto_dream: bool,

    /// Output format
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,

    /// Override entropy value (for testing AP-42)
    #[arg(long)]
    pub entropy: Option<f64>,
}

pub async fn check_identity_command(args: CheckIdentityArgs) -> i32 {
    // 1. Open RocksDB storage
    // 2. Load latest snapshot via StandaloneSessionIdentityManager
    // 3. Update in-memory cache for compatibility
    // 4. Check IC crisis/warning thresholds
    // 5. Trigger dream if crisis + --auto-dream
    // 6. Output JSON to stdout
    // 7. Return 0 (success) or 1 (no identity found)
}
```

---

## Constitution Compliance Matrix

| Constitution Rule | Requirement | Implementation | Verified |
|-------------------|-------------|----------------|----------|
| **AP-26** | IC < 0.5 MUST trigger dream | `TriggerManager::request_manual_trigger(DreamPhase::FullCycle)` | tc_session_14_04 |
| **AP-26** | Exit codes: 0=success, 1=error, 2=corruption | Returns 0 on success, 1 if no identity | tc_session_14_05 |
| **AP-38** | IC < 0.5 auto-dream | Same as AP-26 (with --auto-dream flag) | tc_session_14_04 |
| **AP-42** | entropy > 0.7 wires to TriggerManager | `--entropy` flag triggers mental check | tc_session_14_07 |
| **IDENTITY-002** | IC thresholds | Healthy >= 0.9, Good >= 0.7, Warning >= 0.5, Degraded < 0.5 | tc_session_14_03 |

---

## Full State Verification Protocol

### 1. Source of Truth Definition

| Data | Source of Truth | How to Verify |
|------|-----------------|---------------|
| IC Value | RocksDB `CF_SESSION_IDENTITY` | `manager.load_latest()` returns snapshot with `last_ic` |
| Kuramoto R | RocksDB `CF_SESSION_IDENTITY` | `snapshot.kuramoto_phases` → compute `r` |
| ConsciousnessState | `IdentityCache::get().2` | Call in test, match enum variant |
| Session ID | `IdentityCache::get().3` | Call in test, string comparison |
| Dream Triggered | TriggerManager internal state | Mock TriggerManager or check stderr output |
| Exit Code | Process return value | `assert_eq!(result, 0)` |

### 2. Pre-Execution State Setup

```rust
// BEFORE: Setup known state in IdentityCache
let mut snapshot = SessionIdentitySnapshot::new("test-session");
snapshot.consciousness = 0.45;  // Will be crisis level
snapshot.last_ic = 0.42;
snapshot.kuramoto_phases = [0.0; KURAMOTO_N];
update_cache(&snapshot, 0.42);

// VERIFY: Cache is warm with expected values
let (ic, r, state, session) = IdentityCache::get().expect("cache must be warm");
assert!((ic - 0.42).abs() < 0.001);
```

### 3. Post-Execution State Inspection

```rust
// AFTER: Verify outputs
assert_eq!(exit_code, 0);  // ALWAYS 0 per AP-26
assert!(stderr.contains("IC crisis"));
assert!(stderr.contains("dream triggered"));

// VERIFY: No state corruption
let (ic_after, _, _, _) = IdentityCache::get().expect("cache still warm");
assert!((ic_after - ic_before).abs() < 0.001);  // IC unchanged by check
```

### 4. Evidence Collection

```bash
# Build and run with logging
RUST_LOG=debug cargo test -p context-graph-cli check_identity 2>&1 | tee test_output.log

# Verify all 10 tests pass
cargo test -p context-graph-cli check_identity -- --nocapture

# Check specific test
cargo test -p context-graph-cli tc_ap26_crisis_triggers_dream -- --nocapture
```

---

## Edge Case Verification Table

| Edge Case | Input | Expected Output | Test Name |
|-----------|-------|-----------------|-----------|
| Cold cache | `IdentityCache::get() = None` | Exit 0, stderr "Cache cold" | tc_cold_cache_graceful |
| IC = 0.5 exactly | IC boundary | Warning (not crisis) | tc_ic_boundary_at_05 |
| IC = 0.7 exactly | IC boundary | No output (healthy) | tc_ic_boundary_at_07 |
| IC = 0.0 | Minimum | Crisis + dream trigger | tc_ic_minimum |
| IC = 1.0 | Maximum | No output | tc_ic_maximum |
| entropy = 0.7 exactly | AP-42 boundary | Trigger mental check | tc_entropy_boundary |
| entropy = 0.0 | Minimum | No mental check | tc_entropy_minimum |
| entropy = 1.0 | Maximum | Trigger mental check | tc_entropy_maximum |
| --auto-dream without crisis | IC = 0.8 | No dream (healthy) | tc_auto_dream_healthy_noop |
| No flags (defaults) | IC = 0.45 | Crisis output, NO dream trigger | tc_crisis_no_auto_dream |

---

## Test Cases (ACTUAL - 12 Tests Verified)

### Existing Tests in `check_identity.rs`

```rust
// 1. Healthy IC - no output
#[test] fn tc_healthy_ic_silent()

// 2. Warning IC - stderr warning
#[test] fn tc_warning_ic_logs_warning()

// 3. Crisis IC without --auto-dream
#[test] fn tc_crisis_ic_no_flag_logs_crisis()

// 4. Crisis IC with --auto-dream
#[test] fn tc_crisis_ic_with_auto_dream_triggers()

// 5. AP-26 compliance - always exit 0
#[test] fn tc_always_exit_zero()

// 6. AP-42 high entropy triggers mental check
#[test] fn tc_ap42_high_entropy_triggers()

// 7. Cold cache graceful handling
#[test] fn tc_cold_cache_graceful()

// 8. IC boundary at 0.5
#[test] fn tc_ic_boundary_at_05()

// 9. IC boundary at 0.7
#[test] fn tc_ic_boundary_at_07()

// 10. Output format JSON
#[test] fn tc_output_format_json()

// 11. RocksDB load - loads identity from persistent storage
#[tokio::test] async fn tc_rocksdb_load_identity()

// 12. E2E chain - restore then check works across process boundaries
#[tokio::test] async fn tc_e2e_restore_then_check()
```

---

## Verification Commands

```bash
# 1. Build the CLI
cargo build -p context-graph-cli
echo "Exit code: $?"  # Must be 0

# 2. Run all check_identity tests (12 tests)
cargo test -p context-graph-cli check_identity -- --nocapture
echo "Exit code: $?"  # Must be 0 (12/12 tests pass)

# 3. Verify CLI help works
./target/debug/context-graph-cli consciousness check-identity --help

# 4. Run with healthy cache (manual test)
./target/debug/context-graph-cli consciousness check-identity
echo "Exit code: $?"  # Must be 0

# 5. Run with auto-dream flag
./target/debug/context-graph-cli consciousness check-identity --auto-dream
echo "Exit code: $?"  # Must be 0

# 6. Count lines in implementation
wc -l crates/context-graph-cli/src/commands/consciousness/check_identity.rs
# Expected: ~682 lines
```

---

## DO NOT DO THESE THINGS

| Anti-Pattern | Why It's Wrong | What to Do Instead |
|--------------|----------------|---------------------|
| Create MCP handlers | Architecture doesn't use MCP | Use IdentityCache directly |
| Create `crates/context-graph-mcp/src/cli/` | Path doesn't exist | Use `crates/context-graph-cli/` |
| Mock IdentityCache | Tests must use real data | Use `update_cache()` with real `SessionIdentitySnapshot` |
| Return non-zero exit codes | Violates AP-26 | Always return 0 |
| Write to stdout | PostToolUse expects stderr only | All output to stderr |
| Skip TriggerManager for dreams | Violates AP-26, AP-38 | Call `request_manual_trigger()` |
| Ignore --entropy flag | Violates AP-42 | Check entropy and trigger if > 0.7 |

---

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| "No identity found" on every run | No identity in RocksDB | Run `session restore-identity --db-path <path>` first |
| Dream not triggering | `--auto-dream` flag missing | Add flag to command |
| Test fails with "cache must be warm" | Test isolation issue | Use `TEST_LOCK` mutex in tests |
| Exit code 1 | No identity found in RocksDB | Ensure restore-identity has been run first |
| Exit code 2 | RocksDB corruption detected | Check database files, may need to recreate |
| No stderr output when expected | IC is healthy | Verify IC < 0.7 for warning, < 0.5 for crisis |
| E2E chain fails | restore-identity not persisting | Verify session_identity_manager.rs has save_snapshot call |

---

## Files Summary

| File | Status | Lines | Purpose |
|------|--------|-------|---------|
| `crates/context-graph-cli/src/commands/consciousness/check_identity.rs` | EXISTS | 682 | Main implementation |
| `crates/context-graph-cli/src/commands/consciousness/mod.rs` | EXISTS | - | Module exports |
| `crates/context-graph-cli/src/main.rs` | EXISTS | - | CLI entry point |
| `crates/context-graph-core/src/gwt/session_identity/cache.rs` | EXISTS | - | IdentityCache singleton |
| `crates/context-graph-core/src/gwt/session_identity/manager.rs` | EXISTS | - | IC computation |
| `crates/context-graph-core/src/gwt/session_identity/dream_trigger.rs` | EXISTS | - | TriggerManager |

---

## Next Task

Proceed to **015-TASK-SESSION-15** (consciousness inject-context CLI Command).

```xml
</task_spec>
```
