# TASK-SESSION-01: Create SessionIdentitySnapshot Struct (Flattened)

```xml
<task_spec id="TASK-SESSION-01" version="1.0">
<metadata>
  <title>Create SessionIdentitySnapshot Struct (Flattened)</title>
  <status>pending</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-01</requirement_ref>
  </implements>
  <depends_on><!-- None - First task --></depends_on>
  <estimated_hours>2.0</estimated_hours>
  <functional_spec_ref>SPEC-SESSION-IDENTITY</functional_spec_ref>
  <technical_spec_ref>TECH-SESSION-IDENTITY</technical_spec_ref>
</metadata>
```

## Objective

Implement the flattened SessionIdentitySnapshot struct with 14 fields for fast bincode serialization under 30KB. Size reduced from 80KB via:
- Removed `ego_node` wrapper (inline fields)
- Capped trajectory to 50 (was 1000)
- Single consciousness snapshot (was vec)
- Fixed-size arrays where possible

## Context

Foundation task for the session identity persistence system. This struct is the core data structure that will be serialized to RocksDB for cross-session identity continuity. All subsequent session identity tasks depend on this struct.

## Implementation Steps

1. Create the session_identity module directory structure at `crates/context-graph-core/src/gwt/session_identity/`
2. Implement SessionIdentitySnapshot struct with all 14 fields in `types.rs`
3. Add constants MAX_TRAJECTORY_LEN (50) and KURAMOTO_N (13)
4. Implement `new()`, `append_to_trajectory()`, and `estimated_size()` methods
5. Implement Default trait generating UUID session_id
6. Export from `mod.rs`

## Input Context Files

```xml
<input_context_files>
  <file purpose="module_location">crates/context-graph-core/src/gwt/mod.rs</file>
  <file purpose="existing_gwt_types">crates/context-graph-core/src/gwt/types.rs</file>
  <file purpose="serialization_pattern">crates/context-graph-core/src/lib.rs</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-core/src/gwt/session_identity/mod.rs` | Module exports |
| `crates/context-graph-core/src/gwt/session_identity/types.rs` | Main struct definition |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/gwt/mod.rs` | Add `pub mod session_identity;` export |

## Rust Signatures

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

## Definition of Done

### Acceptance Criteria

- [ ] Struct compiles with all 14 fields
- [ ] Bincode serialization round-trip succeeds
- [ ] Serialized size is less than 30KB with full trajectory
- [ ] Trajectory FIFO eviction works at MAX_TRAJECTORY_LEN (50)
- [ ] `estimated_size()` returns ~300 + (trajectory.len() * 52) + session_id.len()
- [ ] Test case TC-SESSION-01 passes (serialization round-trip)
- [ ] Test case TC-SESSION-02 passes (trajectory FIFO eviction)

### Constraints

- UUID for session_id (not auto-increment)
- All 14 fields exactly as specified
- NO 'any' type anywhere
- Follow constitution naming conventions
- Must be Send + Sync compatible

### Verification Commands

```bash
cargo build -p context-graph-core
cargo test -p context-graph-core session_identity
```

## Test Cases

### TC-SESSION-01: Serialization Round-Trip
```rust
#[test]
fn test_snapshot_serialization_roundtrip() {
    let snapshot = SessionIdentitySnapshot::default();
    let bytes = bincode::serialize(&snapshot).unwrap();
    let restored: SessionIdentitySnapshot = bincode::deserialize(&bytes).unwrap();
    assert_eq!(snapshot, restored);
    assert!(bytes.len() < 30_000); // <30KB
}
```

### TC-SESSION-02: Trajectory FIFO Eviction
```rust
#[test]
fn test_trajectory_fifo_eviction() {
    let mut snapshot = SessionIdentitySnapshot::default();
    for i in 0..100 {
        snapshot.append_to_trajectory([i as f32; KURAMOTO_N]);
    }
    assert_eq!(snapshot.trajectory.len(), MAX_TRAJECTORY_LEN);
    assert_eq!(snapshot.trajectory[0][0], 50.0); // First 50 evicted
}
```

## Exit Conditions

- **Success**: All acceptance criteria met, cargo test passes for module
- **Failure**: Compilation errors, serialization exceeds 30KB, any test failure - error out with detailed logging

## Next Task

After completion, proceed to **002-TASK-SESSION-02** (IdentityCache Singleton).

```xml
</task_spec>
```
