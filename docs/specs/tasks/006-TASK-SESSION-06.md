# TASK-SESSION-06: Create SessionIdentityManager (MCP-Integrated)

```xml
<task_spec id="TASK-SESSION-06" version="1.0">
<metadata>
  <title>Create SessionIdentityManager (MCP-Integrated)</title>
  <status>pending</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-06</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-01</task_ref>
    <task_ref>TASK-SESSION-02</task_ref>
    <task_ref>TASK-SESSION-05</task_ref>
  </depends_on>
  <estimated_hours>2.0</estimated_hours>
</metadata>
```

## Objective

Implement SessionIdentityManager trait and DefaultSessionIdentityManager that integrates with MCP tools for capturing, restoring, and computing cross-session identity continuity.

## MCP Tool Chain

- **capture_snapshot** - Gathers state for persistence (called by `session_end` MCP)
- **restore_identity** - Calls: `session_start` -> `get_ego_state` -> `get_kuramoto_state` -> `get_health_status`
- **compute_cross_session_ic** - Uses IDENTITY-001 formula: `cos(PV_current, PV_previous) * r(current)`

## Implementation Steps

1. Create `manager.rs` in session_identity module
2. Define SessionIdentityManager trait with 3 methods
3. Implement DefaultSessionIdentityManager struct with GwtSystem and RocksDbMemex dependencies
4. Implement capture_snapshot gathering state from GWT system
5. Implement restore_identity loading from storage and computing IC
6. Implement compute_cross_session_ic with IDENTITY-001 formula
7. Add helper functions cosine_similarity_13d and compute_kuramoto_r

## Input Context Files

```xml
<input_context_files>
  <file purpose="snapshot_type">crates/context-graph-core/src/gwt/session_identity/types.rs</file>
  <file purpose="cache">crates/context-graph-core/src/gwt/session_identity/cache.rs</file>
  <file purpose="storage">crates/context-graph-storage/src/session_identity.rs</file>
  <file purpose="gwt_system">crates/context-graph-core/src/gwt/system.rs</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-core/src/gwt/session_identity/manager.rs` | Manager implementation |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/gwt/session_identity/mod.rs` | Export manager module |

## Rust Signatures

```rust
// crates/context-graph-core/src/gwt/session_identity/manager.rs

pub trait SessionIdentityManager: Send + Sync {
    /// Capture current state into a snapshot.
    fn capture_snapshot(&self, session_id: &str) -> CoreResult<SessionIdentitySnapshot>;

    /// Restore identity from storage, compute IC.
    /// Returns (snapshot, cross_session_ic).
    fn restore_identity(&self, target_session: Option<&str>) -> CoreResult<(SessionIdentitySnapshot, f32)>;

    /// Compute cross-session IC using IDENTITY-001 formula.
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

## IC Formula (IDENTITY-001)

```
IC = cos(PV_current, PV_previous) * r(current)

where:
  - PV = purpose_vector [f32; 13]
  - r = Kuramoto order parameter (0.0 to 1.0)
  - cos = cosine similarity
```

## Definition of Done

### Acceptance Criteria

- [ ] Trait is Send + Sync for thread safety
- [ ] capture_snapshot gathers Kuramoto phases, purpose vector, consciousness metrics
- [ ] restore_identity updates IdentityCache after loading
- [ ] restore_identity returns IC=1.0 for first session
- [ ] compute_cross_session_ic uses formula: `cos(PV_current, PV_previous) * r(current)`
- [ ] cosine_similarity_13d is public (AP-39)
- [ ] Test case TC-SESSION-MGR-01 passes (identical purpose vectors -> IC=r)
- [ ] Test case TC-SESSION-MGR-02 passes (orthogonal purpose vectors -> IC~0)

### Constraints

- Must follow AP-39: cosine_similarity_13d must be public
- First session returns IC=1.0 (no previous to compare)
- All state gathering is synchronous

### Verification Commands

```bash
cargo build -p context-graph-core
cargo test -p context-graph-core manager
```

## Test Cases

### TC-SESSION-MGR-01: Identical Purpose Vectors
```rust
#[test]
fn test_ic_identical_vectors() {
    let pv = [1.0_f32; KURAMOTO_N];
    let mut s1 = SessionIdentitySnapshot::default();
    let mut s2 = SessionIdentitySnapshot::default();
    s1.purpose_vector = pv;
    s2.purpose_vector = pv;
    s2.kuramoto_phases = [0.0; KURAMOTO_N]; // r = 1.0

    let manager = test_manager();
    let ic = manager.compute_cross_session_ic(&s2, &s1);

    // cos(identical) = 1.0, r = 1.0, so IC = 1.0
    assert!((ic - 1.0).abs() < 0.01);
}
```

### TC-SESSION-MGR-02: Orthogonal Purpose Vectors
```rust
#[test]
fn test_ic_orthogonal_vectors() {
    let mut s1 = SessionIdentitySnapshot::default();
    let mut s2 = SessionIdentitySnapshot::default();
    // Create orthogonal vectors
    s1.purpose_vector = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    s2.purpose_vector = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

    let manager = test_manager();
    let ic = manager.compute_cross_session_ic(&s2, &s1);

    // cos(orthogonal) = 0, so IC ~ 0
    assert!(ic.abs() < 0.01);
}
```

## Exit Conditions

- **Success**: All trait methods implemented with correct IC computation
- **Failure**: IC formula wrong, state gathering incomplete - error out with detailed logging

## Next Task

After completion, proceed to **007-TASK-SESSION-07** (classify_ic() Function).

```xml
</task_spec>
```
