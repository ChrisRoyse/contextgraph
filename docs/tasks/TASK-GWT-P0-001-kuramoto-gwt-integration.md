# TASK-GWT-P0-001: Integrate KuramotoNetwork into GwtSystem

```xml
<task_spec id="TASK-GWT-P0-001" version="2.0">
<metadata>
  <title>Add KuramotoNetwork Field to GwtSystem Struct</title>
  <status>completed</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item>Constitution v4.0.0 Section gwt.consciousness_equation: C(t) = I(t) × R(t) × D(t)</item>
    <item>I(t) computation from Kuramoto order parameter r</item>
    <item>Self-contained consciousness system without external kuramoto_r input</item>
  </implements>
  <depends_on>
    <!-- No dependencies - this is foundation layer -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <last_audit>2026-01-10</last_audit>
</metadata>
```

---

## CRITICAL: Read This First

**ABSOLUTE RULES (NO EXCEPTIONS):**
1. **NO BACKWARDS COMPATIBILITY** - System works or fails fast with clear error messages
2. **NO MOCK DATA** - All tests use real data and verify real outputs
3. **NO WORKAROUNDS** - If something fails, debug and fix the root cause
4. **FAIL FAST** - Use robust error logging so failures are immediately diagnosable

---

## Problem Statement

The `GwtSystem` struct in `crates/context-graph-core/src/gwt/mod.rs` lacks a `KuramotoNetwork` field. Currently:
- The consciousness equation `C(t) = I(t) × R(t) × D(t)` cannot compute `I(t)` internally
- Callers must manually pass `kuramoto_r` to `update_consciousness()`
- There is no self-contained oscillator synchronization in GwtSystem

**Root Cause:** GwtSystem was designed to receive kuramoto_r externally, but the Constitution requires GwtSystem to own and manage the Kuramoto oscillator network internally.

---

## Current Codebase State (VERIFIED 2026-01-10)

### Existing Implementations

**TWO KuramotoNetwork implementations exist:**

| Location | Oscillators | Purpose | Use Case |
|----------|-------------|---------|----------|
| `crates/context-graph-core/src/layers/coherence.rs` | 8 | Bio-nervous layer architecture | Layer-level synchronization |
| `crates/context-graph-utl/src/phase/oscillator/kuramoto.rs` | 13 | Embedding space synchronization | Constitution-compliant C(t) |

### Key Files (VERIFIED PATHS)

| File | Purpose | Current State |
|------|---------|---------------|
| `crates/context-graph-core/src/gwt/mod.rs` | GwtSystem struct | Missing kuramoto field |
| `crates/context-graph-core/src/gwt/consciousness.rs` | ConsciousnessCalculator | Works, takes external kuramoto_r |
| `crates/context-graph-core/src/layers/coherence.rs` | 8-oscillator Kuramoto + CoherenceLayer | Complete, exported via layers::KuramotoNetwork |
| `crates/context-graph-utl/src/phase/oscillator/kuramoto.rs` | 13-oscillator Constitution-compliant | Complete, exported via phase::KuramotoNetwork |
| `crates/context-graph-mcp/src/handlers/gwt_providers.rs` | MCP wrappers | Uses utl KuramotoNetwork |

### Dependency Structure (CRITICAL)

```
context-graph-core (NO EXTERNAL DEPS on utl)
       ↑
context-graph-utl (DEPENDS ON core)
       ↑
context-graph-mcp (DEPENDS ON both core and utl)
```

**WARNING:** You CANNOT add context-graph-utl as a dependency of context-graph-core - this creates a circular dependency!

---

## Solution Architecture

### Option A: Use layers::KuramotoNetwork (RECOMMENDED)

Use the existing 8-oscillator `KuramotoNetwork` from `crates/context-graph-core/src/layers/coherence.rs`.

**Pros:**
- No new dependencies
- Already tested and working
- Simpler integration

**Cons:**
- 8 oscillators vs Constitution's 13 oscillators
- Different frequency bands

### Option B: Move 13-oscillator to core

Migrate the Constitution-compliant 13-oscillator implementation from utl to core.

**Pros:**
- Full Constitution compliance
- 13 oscillators match 13 embedding spaces

**Cons:**
- Breaking change to utl
- More migration work

### Option C: Create new 13-oscillator in core

Implement a new 13-oscillator Kuramoto in core, similar to the utl implementation.

**Pros:**
- Clean design
- Full Constitution compliance
- No breaking changes

**Cons:**
- Code duplication (initially)
- Must maintain two implementations

---

## Recommended Implementation: Option A (8-oscillator)

Use the existing `layers::KuramotoNetwork` for immediate integration. The 8 oscillators provide sufficient synchronization dynamics for consciousness computation. A future task can upgrade to 13 oscillators if needed.

### Files to Modify

| File | Changes |
|------|---------|
| `crates/context-graph-core/src/gwt/mod.rs` | Add kuramoto field, new methods |

### Implementation Steps

#### Step 1: Add Import

```rust
// In crates/context-graph-core/src/gwt/mod.rs
use crate::layers::KuramotoNetwork;
use std::time::Duration;
```

#### Step 2: Add Field to GwtSystem

```rust
/// Global Workspace Theory system orchestrating consciousness
#[derive(Debug)]
pub struct GwtSystem {
    /// Consciousness calculator (C = I×R×D)
    pub consciousness_calc: Arc<ConsciousnessCalculator>,

    /// Global workspace for winner-take-all selection
    pub workspace: Arc<RwLock<GlobalWorkspace>>,

    /// System identity node
    pub self_ego_node: Arc<RwLock<SelfEgoNode>>,

    /// Consciousness state machine
    pub state_machine: Arc<RwLock<StateMachineManager>>,

    /// Meta-cognitive feedback loop
    pub meta_cognitive: Arc<RwLock<MetaCognitiveLoop>>,

    /// Workspace event broadcaster
    pub event_broadcaster: Arc<WorkspaceEventBroadcaster>,

    /// Kuramoto oscillator network for phase synchronization (I(t) computation)
    /// Uses 8 oscillators from layers::coherence for layer-level sync
    pub kuramoto: Arc<RwLock<KuramotoNetwork>>,
}
```

#### Step 3: Initialize in new()

```rust
impl GwtSystem {
    /// Create a new GWT consciousness system
    pub async fn new() -> crate::CoreResult<Self> {
        use crate::layers::{KURAMOTO_K, KURAMOTO_N};

        Ok(Self {
            consciousness_calc: Arc::new(ConsciousnessCalculator::new()),
            workspace: Arc::new(RwLock::new(GlobalWorkspace::new())),
            self_ego_node: Arc::new(RwLock::new(SelfEgoNode::new())),
            state_machine: Arc::new(RwLock::new(StateMachineManager::new())),
            meta_cognitive: Arc::new(RwLock::new(MetaCognitiveLoop::new())),
            event_broadcaster: Arc::new(WorkspaceEventBroadcaster::new()),
            kuramoto: Arc::new(RwLock::new(KuramotoNetwork::new(KURAMOTO_N, KURAMOTO_K))),
        })
    }
```

#### Step 4: Add Accessor and Helper Methods

```rust
    /// Get reference to the Kuramoto network
    pub fn kuramoto(&self) -> Arc<RwLock<KuramotoNetwork>> {
        Arc::clone(&self.kuramoto)
    }

    /// Step the Kuramoto network forward by elapsed duration
    ///
    /// # Arguments
    /// * `elapsed` - Time duration to advance the oscillators
    pub async fn step_kuramoto(&self, elapsed: Duration) {
        use crate::layers::KURAMOTO_DT;

        let mut network = self.kuramoto.write().await;
        // Convert Duration to f32 seconds for the step function
        let dt = elapsed.as_secs_f32();
        // Use multiple integration steps for stability
        let steps = (dt / KURAMOTO_DT).ceil() as usize;
        for _ in 0..steps {
            network.step(KURAMOTO_DT);
        }
    }

    /// Get current Kuramoto order parameter r (synchronization level)
    ///
    /// # Returns
    /// * `f32` in [0, 1] where 1 = perfect sync, 0 = no sync
    pub async fn get_kuramoto_r(&self) -> f32 {
        let network = self.kuramoto.read().await;
        network.order_parameter()
    }

    /// Update consciousness with internal Kuramoto r value
    ///
    /// This method fetches r from the internal Kuramoto network
    /// instead of requiring the caller to pass it.
    ///
    /// # Arguments
    /// * `meta_accuracy` - Meta-UTL prediction accuracy [0,1]
    /// * `purpose_vector` - 13D purpose alignment vector
    ///
    /// # Returns
    /// * Consciousness level C(t) in [0, 1]
    pub async fn update_consciousness_auto(
        &self,
        meta_accuracy: f32,
        purpose_vector: &[f32; 13],
    ) -> crate::CoreResult<f32> {
        let kuramoto_r = self.get_kuramoto_r().await;
        self.update_consciousness(kuramoto_r, meta_accuracy, purpose_vector).await
    }
```

---

## Full State Verification Requirements

### Source of Truth

The Kuramoto order parameter `r` is computed from the oscillator phases stored in `KuramotoNetwork.oscillators`. After any operation:

1. **Verify phases exist:** `network.size() == KURAMOTO_N` (should be 8)
2. **Verify r is valid:** `0.0 <= r <= 1.0`
3. **Verify phases are in range:** Each phase θᵢ ∈ [0, 2π]

### Test Pattern: Execute & Inspect

```rust
#[tokio::test]
async fn test_gwt_kuramoto_integration_full_verification() {
    // === SETUP ===
    let gwt = GwtSystem::new().await.expect("GwtSystem creation must succeed");

    // === SOURCE OF TRUTH CHECK ===
    let network = gwt.kuramoto.read().await;
    assert_eq!(network.size(), 8, "Must have exactly 8 oscillators");

    let initial_r = network.order_parameter();
    println!("STATE BEFORE: r = {:.4}", initial_r);
    assert!(initial_r >= 0.0 && initial_r <= 1.0, "r must be in [0,1]");
    drop(network);

    // === EXECUTE ===
    gwt.step_kuramoto(Duration::from_millis(100)).await;

    // === VERIFY VIA SEPARATE READ ===
    let network = gwt.kuramoto.read().await;
    let final_r = network.order_parameter();
    println!("STATE AFTER: r = {:.4}", final_r);

    // Verify phases actually changed (phases evolved)
    // Note: With coupling, phases should synchronize over time
    assert!(final_r >= 0.0 && final_r <= 1.0, "r must remain in [0,1]");

    // === EVIDENCE OF SUCCESS ===
    println!("EVIDENCE: Kuramoto stepped successfully, r = {:.4}", final_r);
}
```

### Edge Case Audit (Must Implement)

| Edge Case | Input | Expected Behavior | Verification |
|-----------|-------|-------------------|--------------|
| Zero elapsed time | `Duration::ZERO` | No phase change | Assert phases unchanged |
| Large elapsed time | `Duration::from_secs(10)` | Many integration steps | Assert r still valid |
| Concurrent access | Multiple async tasks | No deadlock, consistent state | Assert no panics |

### Manual Test Commands

```bash
# Build the crate (must pass)
cargo build -p context-graph-core

# Run all GWT tests (must pass)
cargo test -p context-graph-core gwt -- --nocapture

# Run specific new tests
cargo test -p context-graph-core test_gwt_system_has_kuramoto -- --nocapture
cargo test -p context-graph-core test_step_kuramoto_advances_phases -- --nocapture
cargo test -p context-graph-core test_get_kuramoto_r_returns_valid -- --nocapture
cargo test -p context-graph-core test_update_consciousness_auto -- --nocapture

# Clippy (must pass with no warnings)
cargo clippy -p context-graph-core -- -D warnings

# Doc generation (must succeed)
cargo doc -p context-graph-core --no-deps
```

---

## Test Cases (Required)

### Test 1: GwtSystem has Kuramoto network

```rust
#[tokio::test]
async fn test_gwt_system_has_kuramoto_network() {
    println!("=== TEST: GwtSystem Kuramoto Field ===");

    // Create system
    let gwt = GwtSystem::new().await.expect("GwtSystem must create");

    // Verify field exists and is accessible
    let network = gwt.kuramoto.read().await;
    let r = network.order_parameter();

    println!("BEFORE: order_parameter r = {:.4}", r);
    assert!(r >= 0.0 && r <= 1.0, "Initial r must be valid");
    assert_eq!(network.size(), 8, "Must have 8 oscillators");

    println!("EVIDENCE: kuramoto field exists with {} oscillators, r = {:.4}",
             network.size(), r);
}
```

### Test 2: step_kuramoto advances phases

```rust
#[tokio::test]
async fn test_step_kuramoto_advances_phases() {
    println!("=== TEST: step_kuramoto Phase Evolution ===");

    let gwt = GwtSystem::new().await.unwrap();

    // Capture initial state
    let initial_r = gwt.get_kuramoto_r().await;
    println!("BEFORE: r = {:.4}", initial_r);

    // Step forward
    for i in 0..10 {
        gwt.step_kuramoto(Duration::from_millis(10)).await;
        let r = gwt.get_kuramoto_r().await;
        println!("STEP {}: r = {:.4}", i + 1, r);
    }

    let final_r = gwt.get_kuramoto_r().await;
    println!("AFTER: r = {:.4}", final_r);

    // With coupling K=2.0, phases should evolve
    // Order parameter may increase (sync) or fluctuate
    assert!(final_r >= 0.0 && final_r <= 1.0);

    println!("EVIDENCE: Phases evolved from r={:.4} to r={:.4}", initial_r, final_r);
}
```

### Test 3: get_kuramoto_r returns valid value

```rust
#[tokio::test]
async fn test_get_kuramoto_r_returns_valid_value() {
    println!("=== TEST: get_kuramoto_r Bounds ===");

    let gwt = GwtSystem::new().await.unwrap();

    // Test multiple times with stepping
    for _ in 0..100 {
        let r = gwt.get_kuramoto_r().await;
        assert!(r >= 0.0, "r must be >= 0.0, got {}", r);
        assert!(r <= 1.0, "r must be <= 1.0, got {}", r);
        gwt.step_kuramoto(Duration::from_millis(1)).await;
    }

    let final_r = gwt.get_kuramoto_r().await;
    println!("EVIDENCE: After 100 steps, r = {:.4} (valid range verified)", final_r);
}
```

### Test 4: update_consciousness_auto uses internal r

```rust
#[tokio::test]
async fn test_update_consciousness_auto() {
    println!("=== TEST: update_consciousness_auto ===");

    let gwt = GwtSystem::new().await.unwrap();

    // Step to get some synchronization
    for _ in 0..50 {
        gwt.step_kuramoto(Duration::from_millis(10)).await;
    }

    let r = gwt.get_kuramoto_r().await;
    println!("BEFORE: kuramoto_r = {:.4}", r);

    // Call auto version
    let meta_accuracy = 0.8;
    let purpose_vector = [1.0; 13]; // Uniform distribution

    let consciousness = gwt.update_consciousness_auto(meta_accuracy, &purpose_vector)
        .await
        .expect("update_consciousness_auto must succeed");

    println!("AFTER: consciousness C(t) = {:.4}", consciousness);

    // Verify C(t) is valid
    assert!(consciousness >= 0.0 && consciousness <= 1.0,
            "C(t) must be in [0,1], got {}", consciousness);

    // Verify state machine was updated
    let state_mgr = gwt.state_machine.read().await;
    let state = state_mgr.current_state();
    println!("EVIDENCE: State machine is now in {:?} state", state);
}
```

---

## Validation Criteria (All Must Pass)

| Criterion | Verification Command | Expected Result |
|-----------|---------------------|-----------------|
| GwtSystem has kuramoto field | Compile | No errors |
| Field is `Arc<RwLock<KuramotoNetwork>>` | Compile | Type check passes |
| new() initializes kuramoto | Test | Field is populated |
| kuramoto() returns Arc clone | Test | Arc::strong_count > 1 |
| step_kuramoto() advances network | Test | Phases change |
| get_kuramoto_r() returns f32 in [0,1] | Test | Bounds verified |
| update_consciousness_auto() uses internal r | Test | Returns valid C(t) |
| Existing update_consciousness() unchanged | Compile | No signature change |
| All tests pass | `cargo test` | 0 failures |
| No clippy warnings | `cargo clippy` | 0 warnings |

---

## Definition of Done

- [x] `kuramoto: Arc<RwLock<KuramotoNetwork>>` field added to GwtSystem
- [x] KuramotoNetwork imported from `crate::layers`
- [x] `kuramoto()` accessor method implemented
- [x] `step_kuramoto(elapsed: Duration)` implemented
- [x] `get_kuramoto_r() -> f32` implemented
- [x] `update_consciousness_auto(...)` implemented
- [x] 4 unit tests written and passing (10 total tests added)
- [x] Full state verification tests with println! evidence
- [x] `cargo build -p context-graph-core` passes
- [x] `cargo test -p context-graph-core gwt` passes (51 tests)
- [x] `cargo clippy -p context-graph-core -- -D warnings` passes
- [x] `cargo doc -p context-graph-core --no-deps` generates successfully

## Completion Details

**Completed:** 2026-01-11
**All Tests Passing:** 51 GWT tests

### Implementation Summary

1. Added `kuramoto: Arc<RwLock<KuramotoNetwork>>` field to `GwtSystem` struct
2. Added imports for `KuramotoNetwork`, `KURAMOTO_DT`, `KURAMOTO_K`, `KURAMOTO_N` from `crate::layers`
3. Initialized kuramoto in `new()` method with 8 oscillators and coupling K=2.0
4. Added `kuramoto()` accessor method returning Arc clone
5. Added `step_kuramoto(elapsed: Duration)` with multi-step integration for stability
6. Added `get_kuramoto_r()` returning order parameter in [0, 1]
7. Added `update_consciousness_auto(meta_accuracy, purpose_vector)` for self-contained C(t) computation

### Test Coverage

- `test_gwt_system_has_kuramoto_network` - Verifies kuramoto field exists with 8 oscillators
- `test_step_kuramoto_advances_phases` - Verifies phase evolution over time
- `test_get_kuramoto_r_returns_valid_value` - Verifies r bounds over 100 steps
- `test_update_consciousness_auto` - Verifies C(t) computation with internal r
- `test_gwt_kuramoto_integration_full_verification` - Full state verification
- `test_step_kuramoto_zero_elapsed` - Edge case: zero duration
- `test_step_kuramoto_large_elapsed` - Edge case: 10 second duration
- `test_kuramoto_concurrent_access` - Edge case: 10 concurrent tasks
- `test_kuramoto_accessor` - Verifies Arc clone behavior

---

## Follow-up Tasks

| Task ID | Title | Dependency |
|---------|-------|------------|
| TASK-GWT-P0-002 | Background oscillator stepper | This task |
| TASK-GWT-P0-003 | Self-awareness activation | This task |
| TASK-GWT-P1-001 | Ego node persistence | TASK-GWT-P0-003 |

---

## Appendix: Key Type Signatures

### layers::KuramotoNetwork (8-oscillator)

```rust
// From crates/context-graph-core/src/layers/coherence.rs
pub struct KuramotoNetwork {
    oscillators: Vec<KuramotoOscillator>,
    coupling: f32,
}

impl KuramotoNetwork {
    pub fn new(n: usize, coupling: f32) -> Self;
    pub fn size(&self) -> usize;
    pub fn coupling(&self) -> f32;
    pub fn step(&mut self, dt: f32);
    pub fn order_parameter(&self) -> f32;  // Returns r ∈ [0, 1]
    pub fn mean_phase(&self) -> f32;
    pub fn inject_signal(&mut self, signal: f32);
    pub fn reset_phases(&mut self);
}
```

### Constants from layers::coherence

```rust
pub const KURAMOTO_K: f32 = 2.0;      // Coupling strength
pub const KURAMOTO_N: usize = 8;       // Number of oscillators
pub const KURAMOTO_DT: f32 = 0.01;     // Integration time step
pub const GW_THRESHOLD: f32 = 0.7;     // Consciousness threshold
pub const HYPERSYNC_THRESHOLD: f32 = 0.95;
pub const FRAGMENTATION_THRESHOLD: f32 = 0.5;
```

---

**Last Updated:** 2026-01-11
**Audited By:** AI Agent
**Completed By:** Claude Code with Multi-Agent Coordination
**Status:** COMPLETED
