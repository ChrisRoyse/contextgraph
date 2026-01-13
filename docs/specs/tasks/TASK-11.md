# TASK-11: Implement KuramotoNetwork with 13 frequencies

```xml
<task_spec id="TASK-11" version="2.0">
<metadata>
  <title>Implement KuramotoNetwork with 13 frequencies</title>
  <original_id>TASK-GWT-002</original_id>
  <status>COMPLETED</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements><requirement_ref>REQ-GWT-002</requirement_ref></implements>
  <depends_on>TASK-10</depends_on>
  <estimated_hours>3</estimated_hours>
  <completion_date>2026-01-13</completion_date>
</metadata>

<context>
The KuramotoNetwork struct implements coupled oscillator dynamics for GWT coherence.
It uses exactly 13 oscillators with the constitution-defined frequencies (AP-25, GWT-002).
The order parameter r(t) measures synchronization level.

## Key Equations

**Kuramoto Dynamics:**
```
dθ_i/dt = ω_i + (K/N) * Σ_{j=1}^{N} sin(θ_j - θ_i)
```

**Order Parameter:**
```
r * e^(i*ψ) = (1/N) * Σ_{j=1}^{N} e^(i*θ_j)
```

Where:
- θ_i = phase of oscillator i
- ω_i = natural frequency of oscillator i (from KURAMOTO_BASE_FREQUENCIES)
- K = coupling strength
- N = 13 (oscillators)
- r = order parameter ∈ [0, 1]
- ψ = mean phase
</context>

<current_state>
## IMPLEMENTATION STATUS: COMPLETE

The KuramotoNetwork is fully implemented in:
- `crates/context-graph-core/src/layers/coherence/network.rs`

### Files Involved
| File | Status | Purpose |
|------|--------|---------|
| `coherence/constants.rs` | COMPLETE | KURAMOTO_N=13, KURAMOTO_BASE_FREQUENCIES |
| `coherence/network.rs` | COMPLETE | KuramotoNetwork struct and impl |
| `coherence/oscillator.rs` | COMPLETE | KuramotoOscillator struct |
| `coherence/mod.rs` | COMPLETE | Module exports |

### Implementation Details

**Struct Definition (network.rs:20-25):**
```rust
pub struct KuramotoNetwork {
    oscillators: Vec<KuramotoOscillator>,  // 13 oscillators
    coupling: f32,                          // K value
}
```

**NOTE:** Uses `Vec<KuramotoOscillator>` instead of fixed `[f32; 13]` arrays.
This is acceptable because:
1. Runtime size verified by tests
2. Allows flexible creation patterns (with_oscillators)
3. All 21 tests pass verifying 13 oscillators

### Test Verification Results (2026-01-13)
```
cargo test -p context-graph-core kuramoto
test result: ok. 21 passed; 0 failed
```

Key tests:
- `fsv_l5_kuramoto_13_oscillators` - Verifies 13 oscillators
- `test_perfect_sync_order_parameter` - Verifies r=1 for sync
- `test_kuramoto_computation_benchmark` - Performance check
</current_state>

<api_reference>
## KuramotoNetwork API

### Constructor
```rust
// Create network with n oscillators and coupling K
pub fn new(n: usize, coupling: f32) -> Self

// Create with custom oscillators (for testing)
pub fn with_oscillators(oscillators: Vec<KuramotoOscillator>, coupling: f32) -> Self
```

### Core Methods
```rust
// Step forward by dt seconds using Euler integration
pub fn step(&mut self, dt: f32)

// Get order parameter r ∈ [0, 1]
pub fn order_parameter(&self) -> f32

// Get mean phase ψ ∈ [0, 2π]
pub fn mean_phase(&self) -> f32
```

### Accessors
```rust
pub fn size(&self) -> usize        // Number of oscillators
pub fn coupling(&self) -> f32      // Coupling strength K
pub fn phases(&self) -> Vec<f32>   // All oscillator phases
pub fn frequencies(&self) -> Vec<f32>  // All natural frequencies
```

### Modulation
```rust
pub fn inject_signal(&mut self, signal: f32)  // Modulate frequencies
pub fn reset_phases(&mut self)                 // Reset to distributed phases
```
</api_reference>

<constants_reference>
## Constants (coherence/constants.rs)

```rust
pub const KURAMOTO_N: usize = 13;
pub const KURAMOTO_K: f32 = 2.0;
pub const KURAMOTO_DT: f32 = 0.01;
pub const KURAMOTO_DEFAULT_COUPLING: f32 = 0.5;
pub const KURAMOTO_STEP_INTERVAL_MS: u64 = 10;

pub const KURAMOTO_BASE_FREQUENCIES: [f32; KURAMOTO_N] = [
    40.0,  // E1  gamma_fast - perception binding
    8.0,   // E2  theta_slow - memory consolidation
    8.0,   // E3  theta_2 - hippocampal rhythm
    8.0,   // E4  theta_3 - prefrontal sync
    25.0,  // E5  beta_1 - motor planning
    4.0,   // E6  delta - deep sleep
    25.0,  // E7  beta_2 - active thinking
    12.0,  // E8  alpha - relaxed awareness
    80.0,  // E9  high_gamma - cross-modal binding
    40.0,  // E10 gamma_mid - attention
    15.0,  // E11 beta_3 - cognitive control
    60.0,  // E12 gamma_low - sensory processing
    4.0,   // E13 delta_slow - slow wave sleep
];
```
</constants_reference>

<verification_commands>
## Verification Commands

```bash
# Run all Kuramoto tests
cargo test -p context-graph-core kuramoto

# Run specific FSV test
cargo test -p context-graph-core fsv_l5_kuramoto_13_oscillators -- --nocapture

# Run coherence module tests
cargo test -p context-graph-core coherence

# Run benchmark test
cargo test -p context-graph-core test_kuramoto_computation_benchmark -- --nocapture
```

### Expected Output
```
running 21 tests
test layers::coherence::network::tests::fsv_l5_kuramoto_13_oscillators ... ok
test layers::coherence::network::tests::test_network_creation ... ok
test layers::coherence::network::tests::test_order_parameter_range ... ok
test layers::coherence::network::tests::test_perfect_sync_order_parameter ... ok
...
test result: ok. 21 passed; 0 failed
```
</verification_commands>

<full_state_verification>
## MANDATORY: Full State Verification Protocol

After ANY modification to KuramotoNetwork, perform these checks:

### 1. Source of Truth Verification
The source of truth is:
- `KURAMOTO_N` constant = 13
- `KURAMOTO_BASE_FREQUENCIES` array length = 13
- `KuramotoNetwork.size()` = 13

**Execute & Inspect:**
```rust
use context_graph_core::layers::coherence::{KURAMOTO_N, KURAMOTO_BASE_FREQUENCIES, KuramotoNetwork};

let net = KuramotoNetwork::new(KURAMOTO_N, 2.0);
assert_eq!(KURAMOTO_N, 13);
assert_eq!(KURAMOTO_BASE_FREQUENCIES.len(), 13);
assert_eq!(net.size(), 13);
```

### 2. Boundary & Edge Case Audit

**Edge Case 1: Empty Network (should error/return 0)**
```rust
let net = KuramotoNetwork::with_oscillators(vec![], 2.0);
assert_eq!(net.order_parameter(), 0.0);  // Handles divide by zero
```

**Edge Case 2: Perfect Synchronization**
```rust
let oscillators: Vec<_> = (0..13).map(|_| KuramotoOscillator::new(0.0, 40.0)).collect();
let net = KuramotoNetwork::with_oscillators(oscillators, 2.0);
let r = net.order_parameter();
assert!((r - 1.0).abs() < 1e-6, "Perfect sync must give r=1.0, got {}", r);
```

**Edge Case 3: Maximum Coupling**
```rust
let mut net = KuramotoNetwork::new(13, 10.0);  // High coupling
for _ in 0..1000 {
    net.step(0.01);
}
let r = net.order_parameter();
assert!(r > 0.5, "High coupling should increase sync, r={}", r);
```

### 3. Evidence of Success Log
Run with `--nocapture` to see verification output:
```bash
cargo test -p context-graph-core fsv_l5_kuramoto_13_oscillators -- --nocapture
```

Expected output:
```
[FSV] L5 Kuramoto verification:
  KURAMOTO_N = 13
  KURAMOTO_BASE_FREQUENCIES.len() = 13
  Network oscillators = 13
  Phases count = 13
  Frequencies count = 13
[VERIFIED] All 13 oscillator requirements met per constitution GWT-002
```
</full_state_verification>

<constitution_compliance>
## Constitution Compliance

| Rule | Requirement | Status |
|------|-------------|--------|
| AP-25 | Kuramoto must have exactly 13 oscillators | ✓ VERIFIED |
| GWT-002 | Kuramoto network = exactly 13 oscillators | ✓ VERIFIED |
| AP-12 | No magic numbers - use named constants | ✓ VERIFIED |
| gwt.kuramoto.frequencies | 13 frequency values for E1-E13 | ✓ VERIFIED |
| gwt.kuramoto.formula | dθ_i/dt = ω_i + (K/N)Σⱼsin(θⱼ-θᵢ) | ✓ VERIFIED |
| gwt.kuramoto.order_param | r·e^(iψ) = (1/N)Σⱼe^(iθⱼ) | ✓ VERIFIED |
| gwt.kuramoto.thresholds.coherent | r ≥ 0.8 | ✓ Used in tests |
</constitution_compliance>

<next_task>
## Next Task: TASK-12

TASK-12 (KuramotoStepper lifecycle) depends on this implementation.
See: `/home/cabdru/contextgraph/docs/specs/tasks/TASK-12.md`

TASK-12 will:
- Create async wrapper around KuramotoNetwork
- Integrate with MCP server lifecycle (10ms step interval)
- Use KURAMOTO_DEFAULT_COUPLING and KURAMOTO_STEP_INTERVAL_MS
</next_task>

<known_issues>
## Known Issues (Blocking Future Tasks)

### ISSUE: gwt_providers.rs async trait mismatch
**Status:** FIXED (2026-01-13)
**Location:** `crates/context-graph-mcp/src/handlers/gwt_providers.rs`

TASK-07 converted WorkspaceProvider and MetaCognitiveProvider traits to async
but didn't update all call sites. Fixed calls in:
- gwt_providers.rs (impl methods)
- neuromod.rs (acetylcholine access)
- gwt_consciousness.rs (workspace/meta access)
- gwt_workspace.rs (workspace status)

**Verification:**
```bash
cargo check -p context-graph-mcp  # Must pass with no errors
cargo test -p context-graph-mcp gwt_providers  # 15 tests must pass
```
</known_issues>
</task_spec>
```

## Quick Reference

### Create a 13-oscillator network:
```rust
use context_graph_core::layers::coherence::{KURAMOTO_N, KuramotoNetwork};

let mut net = KuramotoNetwork::new(KURAMOTO_N, 2.0);  // K=2.0

// Step forward
net.step(0.01);  // dt = 10ms

// Check synchronization
let r = net.order_parameter();  // r ∈ [0, 1]
let psi = net.mean_phase();     // ψ ∈ [0, 2π]

println!("Sync level: r={:.3}, mean_phase={:.3}", r, psi);
```

### Test manually in a test file:
```rust
#[test]
fn manual_kuramoto_test() {
    use context_graph_core::layers::coherence::{
        KURAMOTO_N, KURAMOTO_BASE_FREQUENCIES, KuramotoNetwork
    };

    // Verify constants
    println!("KURAMOTO_N = {}", KURAMOTO_N);
    println!("Frequencies: {:?}", KURAMOTO_BASE_FREQUENCIES);

    // Create and verify network
    let net = KuramotoNetwork::new(KURAMOTO_N, 2.0);
    println!("Network size: {}", net.size());
    println!("Initial r: {}", net.order_parameter());
    println!("Initial ψ: {}", net.mean_phase());
    println!("Phases: {:?}", net.phases());
    println!("Frequencies: {:?}", net.frequencies());

    assert_eq!(net.size(), 13);
}
```
