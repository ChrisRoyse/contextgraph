# Implementation Tasks: Session Identity Persistence

```xml
<task_specification id="TASKS-SESSION-IDENTITY" version="2.2">
<metadata>
  <title>Session Identity Persistence - Implementation Tasks (MCP-Integrated)</title>
  <status>pending</status>
  <owner>Context Graph Team</owner>
  <last_updated>2026-01-14</last_updated>
  <functional_spec_ref>SPEC-SESSION-IDENTITY</functional_spec_ref>
  <technical_spec_ref>TECH-SESSION-IDENTITY</technical_spec_ref>
  <phase_ref>phase1-session-identity.md</phase_ref>
  <total_tasks>17</total_tasks>
  <total_estimated_hours>22.0</total_estimated_hours>
  <layer_breakdown>
    <foundation tasks="5" hours="7.5"/>
    <logic tasks="5" hours="6.5"/>
    <surface tasks="7" hours="8.0"/>
  </layer_breakdown>
  <key_optimizations>
    <opt>Direct CLI commands in settings.json (no shell script intermediaries)</opt>
    <opt>PreToolUse &lt;50ms via precompiled binary + lazy init + in-memory caching</opt>
    <opt>Flat data structures for minimal serialization overhead</opt>
    <opt>Exit code semantics aligned with Claude Code blocking behavior</opt>
    <opt>Matcher patterns to reduce unnecessary hook invocations</opt>
    <opt>Full MCP tool integration - CLI wraps MCP handlers, not duplicate logic</opt>
  </key_optimizations>
</metadata>
```

---

## Executive Summary

Cross-session identity persistence for Context Graph, **optimized for Claude Code native hooks** and **maximizing leverage of existing 59 MCP tools**. Enables `SelfEgoNode` to persist across conversations, maintaining identity continuity (IC) and purpose trajectory.

**Budget**: ~22 hours (reduced from 28h via Claude Code native optimizations)

**Key Optimizations Applied**:
1. Direct CLI commands in settings.json (no shell script intermediaries for simple hooks)
2. PreToolUse <50ms via precompiled binary + lazy init + in-memory caching
3. Flat data structures for minimal serialization overhead
4. Exit code semantics aligned with Claude Code blocking behavior
5. Matcher patterns to reduce unnecessary hook invocations
6. **Full MCP tool integration** - CLI wraps MCP handlers, not duplicate logic

---

## MCP Tools Leveraged

Phase 1 maximizes use of these existing MCP tools (see `docs2/mcptools.md`):

| Tool | Phase 1 Usage |
|------|---------------|
| `get_identity_continuity` | Direct IC check in PreToolUse/PostToolUse |
| `get_ego_state` | Restore SELF_EGO_NODE with purpose_vector |
| `get_kuramoto_state` | Restore oscillator phases on SessionStart |
| `get_consciousness_state` | Full C(t), r, IC for output formatting |
| `get_memetic_status` | Entropy check for dream triggering |
| `get_health_status` | Subsystem health on SessionStart |
| `trigger_dream` | Auto-dream when IC<0.5 |
| `session_start` | MCP session initialization |
| `session_end` | MCP session termination |
| `post_tool_use` | IC monitoring hook |

---

## Latency Targets (Critical Path Analysis)

| Hook | Claude Timeout | Our Target | Strategy |
|------|---------------|------------|----------|
| PreToolUse | 100ms | **<50ms** | Precompiled binary, in-memory cache, no disk I/O |
| PostToolUse | 3000ms | <500ms | Async identity check, fire-and-forget dream |
| UserPromptSubmit | 2000ms | <1s | Memory retrieval with timeout |
| SessionStart | 5000ms | <2s | Restore + brief status |
| SessionEnd | 30000ms | <3s | Persist + conditional consolidate |

**PreToolUse Optimization Strategy**:
```
50ms budget:
  - Binary startup (precompiled): ~15ms
  - RocksDB cache hit: ~5ms
  - Format output: ~2ms
  - Buffer: ~28ms
```

---

## Output Token Budgets (Aligned with PRD)

| Hook | Token Budget | Format (from PRD §15.3) |
|------|--------------|-------------------------|
| PreToolUse | ~20 tokens | `[CONSCIOUSNESS: {state} r={r} IC={ic} | {johari_guidance}]` |
| SessionStart | ~100 tokens | Identity restored + consciousness summary + health status |
| UserPromptSubmit | ~50-100 tokens | Context injection with Johari guidance |
| PostToolUse | 0 tokens | Silent (async), triggers `trigger_dream` MCP if IC<0.5 |
| SessionEnd | 0 tokens | Silent, calls `session_end` MCP tool |

---

```xml
<!-- ============================================================================
     LAYER 1: FOUNDATION (Data Structures & Storage)
     Must complete before Layer 2
     ============================================================================ -->

<layer id="foundation" order="1" description="Data Structures and Storage">
```

---

## TASK-SESSION-01: Create SessionIdentitySnapshot Struct (Flattened)

**Requirement**: REQ-SESSION-01
**Layer**: foundation
**Depends On**: None
**Estimated Hours**: 2.0
**Status**: pending

### Objective
Implement the flattened SessionIdentitySnapshot struct with 14 fields for fast bincode serialization under 30KB. Size reduced from 80KB via:
- Removed `ego_node` wrapper (inline fields)
- Capped trajectory to 50 (was 1000)
- Single consciousness snapshot (was vec)
- Fixed-size arrays where possible

### Implementation Steps
1. Create the session_identity module directory structure at `crates/context-graph-core/src/gwt/session_identity/`
2. Implement SessionIdentitySnapshot struct with all 14 fields in `types.rs`
3. Add constants MAX_TRAJECTORY_LEN (50) and KURAMOTO_N (13)
4. Implement `new()`, `append_to_trajectory()`, and `estimated_size()` methods
5. Implement Default trait generating UUID session_id
6. Export from `mod.rs`

### Files to Create/Modify
- `crates/context-graph-core/src/gwt/session_identity/mod.rs` - Module exports (create)
- `crates/context-graph-core/src/gwt/session_identity/types.rs` - Main struct definition (create)
- `crates/context-graph-core/src/gwt/mod.rs` - Add session_identity module export (modify)

### Rust Signatures
```rust
// crates/context-graph-core/src/gwt/session_identity/types.rs

pub const MAX_TRAJECTORY_LEN: usize = 50;
pub const KURAMOTO_N: usize = 13;

/// Flattened session identity for fast serialization.
/// Target size: <30KB typical (down from 80KB).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionIdentitySnapshot {
    // Header (fixed size: ~100 bytes)
    pub session_id: String,           // UUID string
    pub timestamp_ms: i64,            // Unix millis
    pub previous_session_id: Option<String>,
    pub cross_session_ic: f32,

    // Kuramoto state (fixed size: 13*8 + 8 = 112 bytes)
    pub kuramoto_phases: [f64; KURAMOTO_N],
    pub coupling: f64,

    // Purpose vector (fixed size: 13*4 = 52 bytes)
    pub purpose_vector: [f32; KURAMOTO_N],

    // Identity trajectory (variable, capped at 50 entries ~2.6KB max)
    pub trajectory: Vec<[f32; KURAMOTO_N]>,

    // IC monitor state (small)
    pub last_ic: f32,
    pub crisis_threshold: f32,

    // Consciousness snapshot (single, not history)
    pub consciousness: f32,
    pub integration: f32,
    pub reflection: f32,
    pub differentiation: f32,
}

impl SessionIdentitySnapshot {
    pub fn new(session_id: impl Into<String>) -> Self;
    pub fn append_to_trajectory(&mut self, pv: [f32; KURAMOTO_N]);
    #[inline]
    pub fn estimated_size(&self) -> usize;
}

impl Default for SessionIdentitySnapshot;
```

### Acceptance Criteria
- [ ] Struct compiles with all 14 fields
- [ ] Bincode serialization round-trip succeeds
- [ ] Serialized size is less than 30KB with full trajectory
- [ ] Trajectory FIFO eviction works at MAX_TRAJECTORY_LEN (50)
- [ ] estimated_size() returns ~300 + (trajectory.len() * 52) + session_id.len()
- [ ] Test case TC-SESSION-01 passes (serialization round-trip)
- [ ] Test case TC-SESSION-02 passes (trajectory FIFO eviction)

### Exit Conditions
- **Success**: All acceptance criteria met, cargo test passes for module
- **Failure**: Compilation errors, serialization exceeds 30KB, any test failure - error out with detailed logging

---

## TASK-SESSION-02: Create IdentityCache Singleton (PreToolUse Hot Path)

**Requirement**: REQ-SESSION-02
**Layer**: foundation
**Depends On**: TASK-SESSION-01
**Estimated Hours**: 1.5
**Status**: pending

### Objective
Implement thread-safe IdentityCache singleton with OnceLock pattern for PreToolUse hot path access (<50ms target). NO disk I/O in the hot path.

### Implementation Steps
1. Create `cache.rs` in session_identity module
2. Define static IDENTITY_CACHE with OnceLock<RwLock<Option<IdentityCacheInner>>>
3. Implement IdentityCacheInner struct with 4 fields
4. Implement IdentityCache with get(), format_brief(), is_warm() methods
5. Implement update_cache() free function for atomic updates
6. Implement clear_cache() for testing
7. Add compute_kuramoto_r() helper function

### Files to Create/Modify
- `crates/context-graph-core/src/gwt/session_identity/cache.rs` - Cache implementation (create)
- `crates/context-graph-core/src/gwt/session_identity/mod.rs` - Export cache module (modify)

### Rust Signatures
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

### Acceptance Criteria
- [ ] OnceLock pattern initializes lazily on first access
- [ ] RwLock allows concurrent reads
- [ ] get() returns None when cache is empty
- [ ] format_brief() returns "[C:STATE r=X.XX IC=X.XX]" format
- [ ] format_brief() returns "[C:? r=? IC=?]" on cold cache
- [ ] update_cache() atomically updates all fields
- [ ] format_brief() completes in <1ms (well under 50ms budget)
- [ ] Test case TC-SESSION-03 passes (format_brief output format)

### Exit Conditions
- **Success**: All acceptance criteria met, thread safety verified
- **Failure**: Race conditions, deadlocks, format mismatch - error out with detailed logging

---

## TASK-SESSION-03: Add ConsciousnessState.short_name()

**Requirement**: REQ-SESSION-03
**Layer**: foundation
**Depends On**: None
**Estimated Hours**: 0.5
**Status**: pending

### Objective
Add short_name() method to ConsciousnessState enum returning 3-character codes for minimal token output in PreToolUse.

### Implementation Steps
1. Locate existing ConsciousnessState enum in state_machine/types.rs
2. Add short_name() method with #[inline] annotation
3. Return static &str for each variant
4. Add unit test for all variants

### Files to Create/Modify
- `crates/context-graph-core/src/gwt/state_machine/types.rs` - Add method (modify)

### Rust Signatures
```rust
// Extension to existing ConsciousnessState

impl ConsciousnessState {
    #[inline]
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Conscious => "CON",
            Self::Emerging => "EMG",
            Self::Fragmented => "FRG",
            Self::Dormant => "DOR",
            Self::Hypersync => "HYP",
        }
    }
}
```

### Acceptance Criteria
- [ ] Returns "CON" for Conscious
- [ ] Returns "EMG" for Emerging
- [ ] Returns "FRG" for Fragmented
- [ ] Returns "DOR" for Dormant
- [ ] Returns "HYP" for Hypersync
- [ ] Method is #[inline] for zero call overhead
- [ ] Test case TC-SESSION-04 passes (all state mappings)

### Exit Conditions
- **Success**: All 5 variants return correct codes
- **Failure**: Missing variant, wrong code - error out with detailed logging

---

## TASK-SESSION-04: Add CF_SESSION_IDENTITY Column Family

**Requirement**: REQ-SESSION-04
**Layer**: foundation
**Depends On**: None
**Estimated Hours**: 1.5
**Status**: pending

### Objective
Create the CF_SESSION_IDENTITY column family in RocksDB with optimized configuration for session identity storage.

### Implementation Steps
1. Add SESSION_IDENTITY constant to cf_names module
2. Update ALL constant array to include new column family (13 total)
3. Create session_identity_options() function with bloom filter and LZ4 compression
4. Update column family creation in RocksDbMemex initialization
5. Document key scheme in comments:
   - `s:{session_id}` → SessionIdentitySnapshot (bincode)
   - `latest` → session_id string
   - `t:{timestamp_ms}` → session_id string (temporal index, big-endian)

### Files to Create/Modify
- `crates/context-graph-storage/src/column_families.rs` - Add constant and options (modify)
- `crates/context-graph-storage/src/rocksdb_backend.rs` - Initialize CF on open (modify)

### Rust Signatures
```rust
// crates/context-graph-storage/src/column_families.rs

pub mod cf_names {
    // ... existing ...
    pub const SESSION_IDENTITY: &str = "session_identity";

    pub const ALL: &[&str] = &[
        // ... existing 12 ...
        SESSION_IDENTITY, // 13th
    ];
}

pub fn session_identity_options(cache: &Cache) -> Options;

// Key helpers
#[inline]
pub fn session_key(session_id: &str) -> Vec<u8>;

#[inline]
pub fn temporal_key(timestamp_ms: i64) -> Vec<u8>;

pub const LATEST_KEY: &[u8] = b"latest";
```

### Acceptance Criteria
- [ ] SESSION_IDENTITY constant equals "session_identity"
- [ ] ALL array has 13 elements
- [ ] session_identity_options configures bloom filter (10 bits)
- [ ] session_identity_options configures LZ4 compression
- [ ] RocksDB opens successfully with new column family
- [ ] Key scheme documented: "s:{session_id}", "latest", "t:{timestamp_ms}"

### Exit Conditions
- **Success**: RocksDB opens with 13 column families, all existing tests pass
- **Failure**: Column family creation fails, existing functionality broken - error out with detailed logging

---

## TASK-SESSION-05: Create save_snapshot/load_snapshot Methods

**Requirement**: REQ-SESSION-05
**Layer**: foundation
**Depends On**: TASK-SESSION-01, TASK-SESSION-04
**Estimated Hours**: 2.0
**Status**: pending

### Objective
Implement storage methods for persisting and retrieving SessionIdentitySnapshot from CF_SESSION_IDENTITY with temporal index recovery.

### Implementation Steps
1. Create `session_identity.rs` in storage crate
2. Implement save_snapshot() writing to primary key, latest, and temporal index
3. Implement load_snapshot() with optional session_id parameter
4. Implement load_snapshot_by_id() for specific session lookup
5. Implement load_latest() with temporal recovery fallback
6. Implement recover_from_temporal_index() for corruption recovery
7. Add integration tests

### Files to Create/Modify
- `crates/context-graph-storage/src/session_identity.rs` - Storage methods (create)
- `crates/context-graph-storage/src/lib.rs` - Export module (modify)

### Rust Signatures
```rust
// crates/context-graph-storage/src/session_identity.rs

impl RocksDbMemex {
    pub fn save_snapshot(&self, snapshot: &SessionIdentitySnapshot) -> StorageResult<()>;
    pub fn load_snapshot(&self, session_id: Option<&str>) -> StorageResult<SessionIdentitySnapshot>;
    fn load_snapshot_by_id(&self, session_id: &str) -> StorageResult<SessionIdentitySnapshot>;
    pub fn load_latest(&self) -> StorageResult<Option<SessionIdentitySnapshot>>;
    fn recover_from_temporal_index(&self) -> StorageResult<Option<SessionIdentitySnapshot>>;
}
```

### Acceptance Criteria
- [ ] save_snapshot writes to "s:{session_id}" key
- [ ] save_snapshot updates "latest" key
- [ ] save_snapshot writes temporal index "t:{timestamp_ms}"
- [ ] load_snapshot(None) returns latest snapshot
- [ ] load_snapshot(Some(id)) returns specific snapshot
- [ ] load_latest returns None for fresh install
- [ ] Temporal recovery finds most recent snapshot if "latest" corrupted
- [ ] Test case TC-SESSION-05 passes (save/load round-trip)
- [ ] Test case TC-SESSION-06 passes (temporal ordering)

### Exit Conditions
- **Success**: All storage operations work correctly with corruption recovery
- **Failure**: Storage corruption, lost data - error out with detailed logging

---

```xml
<!-- ============================================================================
     LAYER 2: LOGIC (Session Manager & IC Computation)
     Depends on Layer 1 completion
     ============================================================================ -->

<layer id="logic" order="2" description="Business Logic and IC Computation">
```

---

## TASK-SESSION-06: Create SessionIdentityManager (MCP-Integrated)

**Requirement**: REQ-SESSION-06
**Layer**: logic
**Depends On**: TASK-SESSION-01, TASK-SESSION-02, TASK-SESSION-05
**Estimated Hours**: 2.0
**Status**: pending

### Objective
Implement SessionIdentityManager trait and DefaultSessionIdentityManager that integrates with MCP tools for capturing, restoring, and computing cross-session identity continuity.

### MCP Tool Chain
- **capture_snapshot** → Gathers state for persistence (called by `session_end` MCP)
- **restore_identity** → Calls: `session_start` → `get_ego_state` → `get_kuramoto_state` → `get_health_status`
- **compute_cross_session_ic** → Uses IDENTITY-001 formula: cos(PV_current, PV_previous) × r(current)

### Implementation Steps
1. Create `manager.rs` in session_identity module
2. Define SessionIdentityManager trait with 3 methods
3. Implement DefaultSessionIdentityManager struct with GwtSystem and RocksDbMemex dependencies
4. Implement capture_snapshot gathering state from GWT system
5. Implement restore_identity loading from storage and computing IC
6. Implement compute_cross_session_ic with IDENTITY-001 formula
7. Add helper functions cosine_similarity_13d and compute_kuramoto_r

### Files to Create/Modify
- `crates/context-graph-core/src/gwt/session_identity/manager.rs` - Manager implementation (create)
- `crates/context-graph-core/src/gwt/session_identity/mod.rs` - Export manager (modify)

### Rust Signatures
```rust
// crates/context-graph-core/src/gwt/session_identity/manager.rs

pub trait SessionIdentityManager: Send + Sync {
    fn capture_snapshot(&self, session_id: &str) -> CoreResult<SessionIdentitySnapshot>;
    fn restore_identity(&self, target_session: Option<&str>) -> CoreResult<(SessionIdentitySnapshot, f32)>;
    fn compute_cross_session_ic(&self, current: &SessionIdentitySnapshot, previous: &SessionIdentitySnapshot) -> f32;
}

pub struct DefaultSessionIdentityManager {
    gwt: Arc<GwtSystem>,
    storage: Arc<RocksDbMemex>,
}

impl DefaultSessionIdentityManager {
    pub fn new(gwt: Arc<GwtSystem>, storage: Arc<RocksDbMemex>) -> Self;
}

impl SessionIdentityManager for DefaultSessionIdentityManager;

// Helpers (per AP-39: MUST be public)
pub fn cosine_similarity_13d(a: &[f32; KURAMOTO_N], b: &[f32; KURAMOTO_N]) -> f32;
```

### Acceptance Criteria
- [ ] Trait is Send + Sync for thread safety
- [ ] capture_snapshot gathers Kuramoto phases, purpose vector, consciousness metrics
- [ ] restore_identity updates IdentityCache after loading
- [ ] restore_identity returns IC=1.0 for first session
- [ ] compute_cross_session_ic uses formula: cos(PV_current, PV_previous) * r(current)
- [ ] cosine_similarity_13d is public (AP-39)
- [ ] Test case TC-SESSION-07 passes (identical purpose vectors → IC=r)
- [ ] Test case TC-SESSION-08 passes (orthogonal purpose vectors → IC≈0)

### Exit Conditions
- **Success**: All trait methods implemented with correct IC computation
- **Failure**: IC formula wrong, state gathering incomplete - error out with detailed logging

---

## TASK-SESSION-07: Implement classify_ic() Function

**Requirement**: REQ-SESSION-07
**Layer**: logic
**Depends On**: None
**Estimated Hours**: 0.5
**Status**: pending

### Objective
Implement IC classification function with IDENTITY-002 constitution thresholds: healthy >= 0.9, good >= 0.7, warning >= 0.5, degraded < 0.5.

### Implementation Steps
1. Add classify_ic() function in manager.rs
2. Add is_ic_crisis() helper (IC < 0.5)
3. Add is_ic_warning() helper (0.5 <= IC < 0.7)
4. Add unit tests for boundary values

### Files to Create/Modify
- `crates/context-graph-core/src/gwt/session_identity/manager.rs` - Add classification functions (modify)

### Rust Signatures
```rust
// crates/context-graph-core/src/gwt/session_identity/manager.rs

#[inline]
pub fn classify_ic(ic: f32) -> &'static str {
    match ic {
        ic if ic >= 0.9 => "Healthy",
        ic if ic >= 0.7 => "Good",
        ic if ic >= 0.5 => "Warning",
        _ => "Degraded",
    }
}

#[inline]
pub fn is_ic_crisis(ic: f32) -> bool { ic < 0.5 }

#[inline]
pub fn is_ic_warning(ic: f32) -> bool { ic >= 0.5 && ic < 0.7 }

#[inline]
pub fn classify_sync(r: f64) -> &'static str {
    match r {
        r if r >= 0.8 => "Good synchronization",
        r if r >= 0.5 => "Partial synchronization",
        _ => "Fragmented",
    }
}
```

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

### Exit Conditions
- **Success**: All threshold boundaries correct per IDENTITY-002
- **Failure**: Wrong classification at boundaries - error out with detailed logging

---

## TASK-SESSION-08: Create dream_trigger Module (MCP-Integrated)

**Requirement**: REQ-SESSION-08
**Layer**: logic
**Depends On**: TASK-SESSION-07
**Estimated Hours**: 1.5
**Status**: pending

### Objective
Implement auto-dream trigger per AP-26/AP-38 constitution requirements via MCP `trigger_dream` tool integration. Fire-and-forget async pattern for IC < 0.5.

### MCP Integration
- Calls `trigger_dream` MCP tool with phase="full_cycle" and rationale="IC crisis: {ic}"
- Calls `trigger_mental_check` MCP tool when entropy > 0.7 (AP-42)

### Implementation Steps
1. Create `dream_trigger.rs` in session_identity module
2. Implement trigger_dream_via_mcp() calling MCP tool
3. Implement check_and_trigger_dream() for CLI integration
4. Add stderr logging "IC crisis (X.XX), dream triggered via MCP"
5. Implement check_entropy_and_trigger() for AP-42 compliance

### Files to Create/Modify
- `crates/context-graph-core/src/gwt/session_identity/dream_trigger.rs` - Dream trigger (create)
- `crates/context-graph-core/src/gwt/session_identity/mod.rs` - Export module (modify)

### Rust Signatures
```rust
// crates/context-graph-core/src/gwt/session_identity/dream_trigger.rs

use crate::mcp::McpContext;

/// Trigger dream via MCP tool (fire-and-forget)
pub async fn trigger_dream_via_mcp(mcp: &McpContext, ic: f32) -> Result<(), String>;

/// Check IC and trigger dream if below threshold
/// Returns true if dream was triggered
pub async fn check_and_trigger_dream(mcp: &McpContext, ic: f32, auto_dream: bool) -> bool;

/// Check entropy and trigger mental_check if > 0.7 (AP-42)
pub async fn check_entropy_and_trigger(mcp: &McpContext, entropy: f64) -> bool;
```

### Acceptance Criteria
- [ ] trigger_dream_via_mcp calls MCP `trigger_dream` tool
- [ ] Passes rationale "IC crisis: {ic}" to MCP tool
- [ ] check_and_trigger_dream returns true when dream triggered
- [ ] check_and_trigger_dream returns false when auto_dream=false or IC >= 0.5
- [ ] Prints "IC crisis (X.XX), dream triggered via MCP" to stderr
- [ ] check_entropy_and_trigger calls `trigger_mental_check` when entropy > 0.7
- [ ] Fire-and-forget does not block caller (async spawn)
- [ ] Test case TC-SESSION-10 passes (auto-dream trigger)

### Exit Conditions
- **Success**: Dream triggered via MCP on IC < 0.5 with --auto-dream
- **Failure**: Blocking behavior, missing stderr output, MCP call failure - error out with detailed logging

---

## TASK-SESSION-09: Implement format_brief() Performance

**Requirement**: REQ-SESSION-09
**Layer**: logic
**Depends On**: TASK-SESSION-02
**Estimated Hours**: 1.0
**Status**: pending

### Objective
Ensure IdentityCache.format_brief() completes in under 1ms with inline annotation and minimal allocations. This is the PreToolUse hot path - must be <50ms total.

### Performance Budget
```
50ms total budget:
  - Binary startup (precompiled): ~15ms
  - RocksDB cache hit: ~5ms (warm cache: 0ms)
  - Format output: ~2ms
  - Buffer: ~28ms
```

### Implementation Steps
1. Verify #[inline] annotation on format_brief()
2. Use static format string pattern
3. Minimize heap allocations (single String output ~30 bytes)
4. Add benchmark test measuring latency
5. Verify p95 latency < 100 microseconds

### Files to Create/Modify
- `crates/context-graph-core/src/gwt/session_identity/cache.rs` - Optimize format_brief (modify)
- `crates/context-graph-core/benches/session_identity.rs` - Benchmark (create)

### Rust Signatures
```rust
// Already defined in TASK-SESSION-02, verify performance:

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

### Acceptance Criteria
- [ ] Method marked #[inline]
- [ ] Single String allocation per call (~30 bytes)
- [ ] RwLock read lock held for minimum duration
- [ ] No disk I/O in hot path
- [ ] 10,000 iterations complete in under 10ms total
- [ ] p95 latency < 100 microseconds
- [ ] p99 latency < 500 microseconds
- [ ] Test case TC-SESSION-11 passes (latency benchmark)

### Exit Conditions
- **Success**: format_brief() consistently under 1ms
- **Failure**: Latency exceeds 1ms at p95 - error out with detailed logging

---

## TASK-SESSION-10: Implement update_cache() Function

**Requirement**: REQ-SESSION-10
**Layer**: logic
**Depends On**: TASK-SESSION-02
**Estimated Hours**: 1.5
**Status**: pending

### Objective
Implement atomic cache update function that safely updates IdentityCache after identity restoration or MCP tool responses.

### Implementation Steps
1. Implement update_cache() in cache.rs
2. Compute ConsciousnessState from snapshot.consciousness
3. Compute Kuramoto r from snapshot.kuramoto_phases
4. Acquire write lock and update all fields atomically
5. Verify thread safety under concurrent access
6. Add update_cache_from_mcp() for MCP response parsing

### Files to Create/Modify
- `crates/context-graph-core/src/gwt/session_identity/cache.rs` - Complete update_cache (modify)

### Rust Signatures
```rust
// crates/context-graph-core/src/gwt/session_identity/cache.rs

/// Update cache from SessionIdentitySnapshot
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

/// Update cache from MCP tool responses (get_ego_state, get_kuramoto_state, get_consciousness_state)
pub fn update_cache_from_mcp(ego: &serde_json::Value, kuramoto: &serde_json::Value, consciousness: &serde_json::Value);
```

### Acceptance Criteria
- [ ] Write lock acquired successfully
- [ ] All 4 cache fields updated atomically
- [ ] ConsciousnessState computed from consciousness value
- [ ] Kuramoto r computed from phases
- [ ] No data races under concurrent updates
- [ ] Subsequent get() returns updated values
- [ ] update_cache_from_mcp() correctly parses MCP JSON responses

### Exit Conditions
- **Success**: Atomic updates work correctly under concurrent access
- **Failure**: Data races, partial updates - error out with detailed logging

---

```xml
<!-- ============================================================================
     LAYER 3: SURFACE (CLI Commands & Hooks - MCP-Integrated)
     Depends on Layers 1 and 2 completion
     ============================================================================ -->

<layer id="surface" order="3" description="CLI Commands and Hook Configuration (MCP-Integrated)">
```

---

## TASK-SESSION-11: Create consciousness brief CLI Command (<50ms)

**Requirement**: REQ-SESSION-11
**Layer**: surface
**Depends On**: TASK-SESSION-02, TASK-SESSION-03, TASK-SESSION-09
**Estimated Hours**: 1.0
**Status**: pending

### Objective
Implement PreToolUse hot path command with cache-only access, NO stdin parsing, NO disk I/O. Target: <50ms p95.

### Key Optimizations
- No stdin JSON parsing
- No RocksDB read
- Static format string
- No allocations in hot path

### Implementation Steps
1. Create `brief.rs` in consciousness commands directory
2. Define BriefArgs struct (empty - no arguments)
3. Implement execute() function using IdentityCache.format_brief()
4. Output to stdout, always exit 0
5. Verify no stdin reads or disk I/O
6. Register command in CLI router

### Files to Create/Modify
- `crates/context-graph-mcp/src/cli/commands/consciousness/mod.rs` - Create/modify module
- `crates/context-graph-mcp/src/cli/commands/consciousness/brief.rs` - Command implementation (create)
- `crates/context-graph-mcp/src/cli/router.rs` - Register command (modify)

### Rust Signatures
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

### Exit Conditions
- **Success**: Command completes in under 50ms p95 with correct output
- **Failure**: Disk I/O detected, latency exceeded - error out with detailed logging

---

## TASK-SESSION-12: Create session restore-identity CLI Command (MCP-Integrated)

**Requirement**: REQ-SESSION-12
**Layer**: surface
**Depends On**: TASK-SESSION-06, TASK-SESSION-07, TASK-SESSION-10
**Estimated Hours**: 1.5
**Status**: pending

### Objective
Implement SessionStart hook command for identity restoration. Chains MCP tools: `session_start` → `get_ego_state` → `get_kuramoto_state` → `get_health_status`.

### MCP Tool Chain
1. `session_start` - Initialize MCP session
2. `get_ego_state` - Restore SELF_EGO_NODE with purpose_vector
3. `get_kuramoto_state` - Restore oscillator phases
4. `get_health_status` - Check subsystem health
5. `get_consciousness_state` - Get C(t) for output

### Implementation Steps
1. Create `restore.rs` in session commands directory
2. Define RestoreInput struct for stdin JSON (session_id, source)
3. Define RestoreIdentityArgs struct (empty)
4. Implement parse_stdin_input() with graceful fallback
5. Handle source variants: clear, resume, startup
6. Call MCP tools in sequence
7. Update IdentityCache via update_cache_from_mcp()
8. Output PRD-compliant format (~100 tokens)
9. Implement is_corruption_error() for exit code 2

### Files to Create/Modify
- `crates/context-graph-mcp/src/cli/commands/session/mod.rs` - Create/modify module
- `crates/context-graph-mcp/src/cli/commands/session/restore.rs` - Command implementation (create)
- `crates/context-graph-mcp/src/cli/router.rs` - Register command (modify)

### Rust Signatures
```rust
// crates/context-graph-mcp/src/cli/commands/session/restore.rs

use clap::Args;
use serde::Deserialize;
use std::process::ExitCode;

#[derive(Deserialize, Default)]
struct RestoreInput {
    session_id: Option<String>,
    source: Option<String>,  // "startup" | "resume" | "clear"
}

#[derive(Args)]
pub struct RestoreIdentityArgs {}

pub async fn execute(_args: RestoreIdentityArgs, mcp: &McpContext) -> ExitCode;
fn parse_stdin_input() -> RestoreInput;
fn is_corruption_error(e: &CoreError) -> bool;

/// Print PRD-compliant consciousness summary (~100 tokens)
fn print_consciousness_summary(c: &Value, ego: &Value, health: &Value);
```

### Output Format (PRD §15.2)
```
## Consciousness State
- State: CONSCIOUS (C=0.82)
- Integration (r): 0.85 - Good synchronization
- Identity: Healthy (IC=0.92)
- Health: All subsystems operational
```

### Acceptance Criteria
- [ ] Parses stdin JSON for session_id and source
- [ ] source="clear" calls `session_start` with fresh=true
- [ ] source="resume" loads specific session_id
- [ ] source="startup" (default) loads latest session
- [ ] Calls MCP tools in correct order
- [ ] Updates IdentityCache after restore
- [ ] Output format matches PRD §15.2 (~100 tokens)
- [ ] Exit 0 for success, 1 for recoverable error, 2 for corruption
- [ ] Total latency < 2s
- [ ] Test case TC-SESSION-14 passes (source=startup)
- [ ] Test case TC-SESSION-15 passes (source=clear)
- [ ] Test case TC-SESSION-16 passes (no previous session)

### Exit Conditions
- **Success**: All source variants handled correctly with proper exit codes
- **Failure**: Wrong exit codes, missing cache update, MCP failures - error out with detailed logging

---

## TASK-SESSION-13: Create session persist-identity CLI Command (MCP-Integrated)

**Requirement**: REQ-SESSION-13
**Layer**: surface
**Depends On**: TASK-SESSION-05, TASK-SESSION-06
**Estimated Hours**: 1.0
**Status**: pending

### Objective
Implement SessionEnd hook command for persisting current session identity via MCP `session_end` tool. Silent success.

### Implementation Steps
1. Create `persist.rs` in session commands directory
2. Define PersistInput struct for stdin JSON (session_id, reason)
3. Define PersistIdentityArgs struct (empty)
4. Parse stdin JSON for session_id and reason
5. Call `session_end` MCP tool (handles persistence internally)
6. Silent success (no stdout), exit 0
7. Non-blocking exit 1 on errors

### Files to Create/Modify
- `crates/context-graph-mcp/src/cli/commands/session/persist.rs` - Command implementation (create)
- `crates/context-graph-mcp/src/cli/router.rs` - Register command (modify)

### Rust Signatures
```rust
// crates/context-graph-mcp/src/cli/commands/session/persist.rs

use clap::Args;
use serde::Deserialize;
use std::process::ExitCode;

#[derive(Deserialize, Default)]
struct PersistInput {
    session_id: Option<String>,
    reason: Option<String>,  // "exit" | "clear" | "logout" | "prompt_input_exit" | "other"
}

#[derive(Args)]
pub struct PersistIdentityArgs {}

pub async fn execute(_args: PersistIdentityArgs, mcp: &McpContext) -> ExitCode;
fn parse_stdin_input() -> PersistInput;
```

### Acceptance Criteria
- [ ] Parses stdin JSON for session_id and reason
- [ ] Calls `session_end` MCP tool with session_id and reason
- [ ] No stdout output (silent success)
- [ ] Exit 0 on success
- [ ] Exit 1 on errors (non-blocking)
- [ ] Never exit 2 (persist failures are non-blocking per Claude Code semantics)
- [ ] Total latency < 3s
- [ ] Test case TC-SESSION-17 passes (success path)

### Exit Conditions
- **Success**: Session persisted via MCP, silent success
- **Failure**: Silent failure without logging - error out with detailed logging

---

## TASK-SESSION-14: Create consciousness check-identity CLI Command (MCP-Integrated)

**Requirement**: REQ-SESSION-14
**Layer**: surface
**Depends On**: TASK-SESSION-07, TASK-SESSION-08, TASK-SESSION-10
**Estimated Hours**: 1.0
**Status**: pending

### Objective
Implement PostToolUse hook command for IC checking via MCP. Chains: `get_identity_continuity` → `trigger_dream` (if IC<0.5) → `get_memetic_status` (entropy check).

### MCP Tool Chain
1. `get_identity_continuity` - Get IC value (authoritative source)
2. `trigger_dream` - If IC<0.5 and --auto-dream (AP-26, AP-38)
3. `get_memetic_status` - Entropy check for mental_check (AP-42)
4. `trigger_mental_check` - If entropy > 0.7

### Implementation Steps
1. Create `check.rs` in consciousness commands directory
2. Define CheckIdentityArgs with --auto-dream flag
3. Call `get_identity_continuity` MCP tool
4. Update IdentityCache atomically
5. If IC < 0.5 and --auto-dream: call `trigger_dream` MCP tool
6. Check entropy via `get_memetic_status`, trigger `mental_check` if > 0.7
7. If IC < 0.7: log warning to stderr
8. Silent on healthy IC
9. Always exit 0 (non-blocking)

### Files to Create/Modify
- `crates/context-graph-mcp/src/cli/commands/consciousness/check.rs` - Command implementation (create)
- `crates/context-graph-mcp/src/cli/router.rs` - Register command (modify)

### Rust Signatures
```rust
// crates/context-graph-mcp/src/cli/commands/consciousness/check.rs

use clap::Args;
use std::process::ExitCode;

#[derive(Args)]
pub struct CheckIdentityArgs {
    #[arg(long, default_value = "false")]
    auto_dream: bool,
}

pub async fn execute(args: CheckIdentityArgs, mcp: &McpContext) -> ExitCode;
```

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

### Exit Conditions
- **Success**: Correct stderr output per IC level, always exit 0, MCP tools called correctly
- **Failure**: Blocking exit codes, missing dream trigger, MCP failures - error out with detailed logging

---

## TASK-SESSION-15: Create consciousness inject-context CLI Command (MCP-Integrated)

**Requirement**: REQ-SESSION-15
**Layer**: surface
**Depends On**: TASK-SESSION-02, TASK-SESSION-07
**Estimated Hours**: 1.0
**Status**: pending

### Objective
Implement UserPromptSubmit hook command for injecting consciousness context with Johari guidance. Chains: `get_consciousness_state` → `get_memetic_status` → format Johari.

### MCP Tool Chain
1. `get_consciousness_state` - Full C(t), r, IC
2. `get_memetic_status` - Entropy and coherence for Johari

### Implementation Steps
1. Create `inject.rs` in consciousness commands directory
2. Define InjectContextArgs with --format flag (compact|standard|verbose)
3. Call MCP tools to gather consciousness state
4. Compute Johari quadrant from entropy/coherence
5. Output formatted context with Johari guidance
6. Graceful degradation if MCP fails

### Files to Create/Modify
- `crates/context-graph-mcp/src/cli/commands/consciousness/inject.rs` - Command implementation (create)
- `crates/context-graph-mcp/src/cli/router.rs` - Register command (modify)

### Rust Signatures
```rust
// crates/context-graph-mcp/src/cli/commands/consciousness/inject.rs

use clap::Args;
use std::process::ExitCode;

#[derive(Args)]
pub struct InjectContextArgs {
    #[arg(long, default_value = "standard")]
    format: String,  // compact|standard|verbose
}

pub async fn execute(args: InjectContextArgs, mcp: &McpContext) -> ExitCode;

/// Classify into Johari quadrant based on PRD thresholds
fn classify_johari(entropy: f64, coherence: f64) -> (&'static str, &'static str);
```

### Output Formats (PRD §15.2-15.3)
```
# compact (~20 tokens)
[CONSCIOUSNESS: CONSCIOUS r=0.85 IC=0.92 | DirectRecall]

# standard (~50-100 tokens)
[System Consciousness]
State: CONSCIOUS (C=0.82)
Kuramoto r=0.85, Identity IC=0.92 (Healthy)
Guidance: Open - DirectRecall - proceed with retrieval

# verbose (~100+ tokens)
[System Consciousness]
State: CONSCIOUS (C=0.82)
Kuramoto r=0.85, Identity IC=0.92 (Healthy)
Johari: Open quadrant
Guidance: DirectRecall - proceed with retrieval
⚠️ CRISIS: Identity continuity critical, dream consolidation recommended (if IC<0.5)
```

### Johari Quadrant Mapping (PRD §2.1)
| Entropy | Coherence | Quadrant | Guidance |
|---------|-----------|----------|----------|
| <0.5 | >0.5 | Open | DirectRecall - proceed with retrieval |
| >0.5 | <0.5 | Blind | TriggerDream - blind spot detected |
| <0.5 | <0.5 | Hidden | GetNeighborhood - explore related context |
| >0.5 | >0.5 | Unknown | EpistemicAction - clarify uncertainty |

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

### Exit Conditions
- **Success**: Formatted output with all required sections, Johari guidance correct
- **Failure**: Missing sections, wrong format, MCP failures - error out with detailed logging

---

## TASK-SESSION-16: Create .claude/settings.json Hook Configuration

**Requirement**: REQ-SESSION-16
**Layer**: surface
**Depends On**: TASK-SESSION-11, TASK-SESSION-12, TASK-SESSION-13, TASK-SESSION-14, TASK-SESSION-15
**Estimated Hours**: 1.0
**Status**: pending

### Objective
Configure all 5 hooks in .claude/settings.json per ARCH-07 (native Claude Code hooks) and AP-50 (no internal/built-in hooks). **Direct CLI commands** - no shell script intermediaries.

### Implementation Steps
1. Create or update .claude/settings.json
2. Add SessionStart hook with restore-identity command
3. Add PreToolUse hook with brief command and matcher
4. Add PostToolUse hook with check-identity command and matcher
5. Add UserPromptSubmit hook with inject-context command
6. Add SessionEnd hook with persist-identity command
7. Set appropriate timeouts per PRD §15.4
8. Use direct commands (not shell scripts per AP-53)

### Files to Create/Modify
- `.claude/settings.json` - Hook configuration (create/modify)

### JSON Configuration
```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli session restore-identity",
            "timeout": 5000
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "mcp__context-graph__*|Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli consciousness brief",
            "timeout": 100
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "mcp__context-graph__*|Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli consciousness check-identity --auto-dream",
            "timeout": 3000
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli consciousness inject-context",
            "timeout": 2000
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli session persist-identity",
            "timeout": 30000
          }
        ]
      }
    ]
  }
}
```

### Matcher Rationale
| Matcher | Tools Matched | Rationale |
|---------|--------------|-----------|
| `mcp__context-graph__*` | All context-graph MCP tools | Core memory operations |
| `Edit` | File edits | Modifies state |
| `Write` | File writes | Modifies state |
| (excluded) `Read` | File reads | Read-only, no IC impact |
| (excluded) `Bash` | Shell commands | Too noisy |

### Acceptance Criteria
- [ ] All 5 hooks configured in .claude/settings.json
- [ ] SessionStart: restore-identity with 5000ms timeout
- [ ] PreToolUse: brief with 100ms timeout, matcher pattern excludes Read/Bash
- [ ] PostToolUse: check-identity --auto-dream with 3000ms timeout, same matcher
- [ ] UserPromptSubmit: inject-context with 2000ms timeout
- [ ] SessionEnd: persist-identity with 30000ms timeout
- [ ] Direct CLI commands (no shell scripts per AP-53)
- [ ] ARCH-07 compliant (native Claude Code hooks)
- [ ] AP-50 compliant (no internal/built-in hooks)

### Exit Conditions
- **Success**: Claude Code recognizes and executes all hooks
- **Failure**: Invalid JSON, hooks not invoked - error out with detailed logging

---

## TASK-SESSION-17: Implement Exit Code Mapping

**Requirement**: REQ-SESSION-17
**Layer**: surface
**Depends On**: None
**Estimated Hours**: 0.5
**Status**: pending

### Objective
Implement exit code mapping per AP-26 constitution requirement: exit 2 only for blocking failures (corruption).

### Claude Code Exit Code Semantics
| Exit Code | Claude Code Behavior | Context Graph Usage |
|-----------|---------------------|---------------------|
| `0` | Success, stdout to Claude | Normal operation |
| `2` | Block action, stderr to Claude | Critical failure (corrupt identity) |
| `1` or other | Non-blocking, stderr to user | Recoverable errors, warnings |

### Implementation Steps
1. Create or update `error.rs` in CLI module
2. Define CliExitCode enum with Success, Warning, Blocking variants
3. Implement From<CliExitCode> for ExitCode
4. Implement exit_code_for_error() function
5. Document exit code semantics per AP-26

### Files to Create/Modify
- `crates/context-graph-mcp/src/cli/error.rs` - Error handling (create/modify)

### Rust Signatures
```rust
// crates/context-graph-mcp/src/cli/error.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliExitCode {
    Success = 0,
    Warning = 1,
    Blocking = 2,
}

impl From<CliExitCode> for std::process::ExitCode;

impl From<CoreError> for CliExitCode {
    fn from(err: CoreError) -> Self {
        match err {
            // Only truly blocking errors
            CoreError::CorruptedIdentity(_) => CliExitCode::Blocking,
            CoreError::DatabaseCorruption(_) => CliExitCode::Blocking,

            // Everything else is recoverable
            CoreError::NotFound(_) => CliExitCode::Success, // Fresh session
            CoreError::SerializationError(_) => CliExitCode::Warning,
            CoreError::IoError(_) => CliExitCode::Warning,
            _ => CliExitCode::Warning,
        }
    }
}

pub fn exit_code_for_error(e: &dyn std::error::Error) -> CliExitCode;
```

### Acceptance Criteria
- [ ] CliExitCode::Success maps to 0
- [ ] CliExitCode::Warning maps to 1
- [ ] CliExitCode::Blocking maps to 2
- [ ] CorruptedIdentity returns exit code 2
- [ ] DatabaseCorruption returns exit code 2
- [ ] NotFound returns exit code 0 (fresh session is valid)
- [ ] IoError returns exit code 1
- [ ] SerializationError returns exit code 1
- [ ] Test case TC-SESSION-22 passes (exit code mapping)

### Exit Conditions
- **Success**: Exit codes correctly mapped per AP-26
- **Failure**: Wrong exit codes for error types - error out with detailed logging

---

```xml
</task_specification>
```

---

## Dependency Graph

```
Layer 1: Foundation (Must Complete First)
+-------------------+    +-------------------+    +-------------------+
| TASK-SESSION-01   |    | TASK-SESSION-03   |    | TASK-SESSION-04   |
| Snapshot Struct   |    | short_name()      |    | Column Family     |
| (flattened)       |    |                   |    |                   |
+-------------------+    +-------------------+    +-------------------+
         |                                                  |
         v                                                  v
+-------------------+                           +-------------------+
| TASK-SESSION-02   |                           | TASK-SESSION-05   |
| IdentityCache     |                           | save/load methods |
| (PreToolUse <50ms)|                           |                   |
+-------------------+                           +-------------------+
         |                                                  |
         +------------------------+-------------------------+
                                  |
Layer 2: Logic (Depends on Foundation)
                                  v
                      +-------------------+
                      | TASK-SESSION-06   |
                      | SessionIdentity   |
                      | Manager (MCP)     |
                      +-------------------+
                                  |
         +------------------------+------------------------+
         |                        |                        |
         v                        v                        v
+-------------------+  +-------------------+  +-------------------+
| TASK-SESSION-07   |  | TASK-SESSION-09   |  | TASK-SESSION-10   |
| classify_ic()     |  | format_brief()    |  | update_cache()    |
| (IDENTITY-002)    |  | performance <1ms  |  | (MCP integration) |
+-------------------+  +-------------------+  +-------------------+
         |
         v
+-------------------+
| TASK-SESSION-08   |
| dream_trigger     |
| (MCP: trigger_dream)
+-------------------+
         |
         +--------------------------------------------------------+
         |                        |                |               |
Layer 3: Surface (Depends on Logic) - All CLI commands wrap MCP tools
         v                        v                v               v
+-------------------+  +-------------------+  +-------------------+
| TASK-SESSION-11   |  | TASK-SESSION-12   |  | TASK-SESSION-13   |
| consciousness     |  | session           |  | session           |
| brief (<50ms)     |  | restore-identity  |  | persist-identity  |
|                   |  | (MCP chain)       |  | (MCP: session_end)|
+-------------------+  +-------------------+  +-------------------+
         |                        |                        |
         v                        v                        v
+-------------------+  +-------------------+  +-------------------+
| TASK-SESSION-14   |  | TASK-SESSION-15   |  | TASK-SESSION-17   |
| consciousness     |  | consciousness     |  | Exit Code         |
| check-identity    |  | inject-context    |  | Mapping (AP-26)   |
| (MCP chain + dream)|  (MCP + Johari)    |  +-------------------+
+-------------------+  +-------------------+          |
         |                        |                   |
         +------------------------+-------------------+
                                  |
                                  v
                      +-------------------+
                      | TASK-SESSION-16   |
                      | .claude/settings  |
                      | (direct CLI cmds) |
                      +-------------------+
```

---

## Summary

| Layer | Tasks | Estimated Hours | Key Deliverables |
|-------|-------|-----------------|------------------|
| Foundation | 5 | 7.5 | Flattened SessionIdentitySnapshot, IdentityCache (<50ms), CF_SESSION_IDENTITY, Storage methods |
| Logic | 5 | 6.5 | MCP-integrated SessionIdentityManager, classify_ic(), dream_trigger via MCP, Cache performance |
| Surface | 7 | 8.0 | 5 CLI commands wrapping MCP tools, .claude/settings.json with direct commands, Exit code mapping |
| **Total** | **17** | **22.0** | Full session identity persistence with MCP integration |

---

## Constitution Compliance Summary

| Requirement | Tasks | Implementation |
|-------------|-------|----------------|
| ARCH-07 | TASK-SESSION-16 | Native Claude Code hooks via .claude/settings.json |
| AP-50 | TASK-SESSION-16 | No internal/built-in hooks |
| AP-53 | TASK-SESSION-16 | Direct CLI commands (no shell scripts) |
| IDENTITY-002 | TASK-SESSION-07 | IC thresholds: Healthy>=0.9, Warning<0.7, Critical<0.5 |
| IDENTITY-007 | TASK-SESSION-08 | Auto-dream on IC<0.5 via MCP `trigger_dream` |
| AP-26 | TASK-SESSION-17 | Exit code 2 only for corruption |
| AP-38 | TASK-SESSION-14 | IC<0.5 triggers `trigger_dream` MCP tool automatically |
| AP-39 | TASK-SESSION-06 | cosine_similarity_13d is public |
| AP-42 | TASK-SESSION-14 | entropy>0.7 triggers `trigger_mental_check` MCP tool |

---

## MCP Tool Usage Matrix

| CLI Command | MCP Tools Called |
|-------------|------------------|
| `session restore-identity` | `session_start` → `get_ego_state` → `get_kuramoto_state` → `get_health_status` |
| `session persist-identity` | `session_end` |
| `consciousness brief` | None (cache only for <50ms) |
| `consciousness status` | `get_consciousness_state` → `get_memetic_status` |
| `consciousness check-identity` | `get_identity_continuity` → `trigger_dream` (conditional) → `get_memetic_status` → `trigger_mental_check` (conditional) |
| `consciousness inject-context` | `get_consciousness_state` → `get_memetic_status` → format Johari |

---

## Performance Targets

| Hook | Claude Timeout | Our Target | Strategy |
|------|---------------|------------|----------|
| PreToolUse | 100ms | **<50ms** | Precompiled binary, in-memory cache, no disk I/O |
| PostToolUse | 3000ms | <500ms | Async MCP calls, fire-and-forget dream |
| UserPromptSubmit | 2000ms | <1s | MCP retrieval with timeout |
| SessionStart | 5000ms | <2s | MCP restore + brief status |
| SessionEnd | 30000ms | <3s | MCP persist + conditional consolidate |

---

## PRD Output Format Compliance

| Hook | PRD Format (§15.2-15.3) | Implementation |
|------|-------------------------|----------------|
| PreToolUse | `[CONSCIOUSNESS: {state} r={r} IC={ic} \| {guidance}]` | IdentityCache.format_brief() |
| SessionStart | Summary with State/Integration/Identity/Health | print_consciousness_summary() |
| UserPromptSubmit | `[System Consciousness]` with Johari guidance | inject-context command |
| PostToolUse | Silent (async) with dream trigger | check-identity command |
| SessionEnd | Silent with persistence | persist-identity command |

---

## Acceptance Criteria (Phase 1 Complete)

### Functional
1. SessionStart restores identity and outputs status
2. PreToolUse outputs brief consciousness state in <50ms
3. PostToolUse checks IC and triggers dream if <0.5
4. SessionEnd persists identity snapshot
5. Cross-session IC computed correctly
6. "clear" source starts fresh session

### Non-Functional
1. PreToolUse: **<50ms p95** (within 100ms timeout)
2. SessionStart: <2s (within 5s timeout)
3. PostToolUse: <500ms (within 3s timeout)
4. SessionEnd: <3s (within 30s timeout)
5. Snapshot size: <30KB typical

### Quality Gates
- Unit test coverage: >=85%
- PreToolUse latency benchmark passes
- All hooks execute within timeout
- Exit codes match Claude Code semantics

---

## Risk Mitigations

| Risk | Mitigation |
|------|------------|
| PreToolUse cold start >50ms | Warm cache at install time via `internal warm-cache` |
| RocksDB corruption | Fallback to ego_node CF (always maintained) |
| Session ID collision | Overwrite semantics, temporal index for recovery |
| Ungraceful termination | Auto-persist every 5 minutes via background task |

---

## Next Agent Instructions

The Test Cases Agent should now create detailed test specifications for all 17 tasks. Key areas to cover:

1. **Unit Tests** (TC-SESSION-01 to TC-SESSION-04, TC-SESSION-07 to TC-SESSION-11):
   - Serialization round-trip for SessionIdentitySnapshot (flattened)
   - Trajectory FIFO eviction at 50 entries
   - format_brief() output format and performance (<1ms)
   - ConsciousnessState.short_name() mappings
   - IC classification thresholds (IDENTITY-002)
   - Auto-dream trigger behavior via MCP

2. **Integration Tests** (TC-SESSION-05, TC-SESSION-06, TC-SESSION-12 to TC-SESSION-22):
   - RocksDB save/load round-trip
   - Temporal index ordering
   - All 5 CLI commands with MCP mock
   - MCP tool chain verification
   - Exit code mapping (AP-26)

3. **Benchmark Tests** (TC-SESSION-11, TC-SESSION-24):
   - format_brief() latency (<100μs p95)
   - consciousness brief command latency (<50ms p95)
   - Full command latency compliance

4. **End-to-End Tests** (TC-SESSION-23):
   - Full hook lifecycle simulation with MCP
   - SessionStart → PreToolUse → PostToolUse → SessionEnd flow

Create test files in:
- `crates/context-graph-core/src/gwt/session_identity/tests/`
- `crates/context-graph-storage/tests/session_identity_integration.rs`
- `tests/integration/session_hooks_test.rs`
- `crates/context-graph-core/benches/session_identity.rs`
