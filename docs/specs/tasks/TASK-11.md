# TASK-GWT-002: Implement KuramotoNetwork with 13 frequencies

```xml
<task_spec id="TASK-GWT-002" version="1.0">
<metadata>
  <title>Implement KuramotoNetwork with 13 frequencies</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements><requirement_ref>REQ-GWT-002</requirement_ref></implements>
  <depends_on>TASK-10</depends_on>
  <estimated_hours>3</estimated_hours>
</metadata>

<context>
The KuramotoNetwork struct implements coupled oscillator dynamics for GWT coherence.
It must use exactly 13 oscillators with the constitution-defined frequencies.
The order parameter r(t) measures synchronization level.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-core/src/layers/coherence/constants.rs (from TASK-GWT-001)
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 3.2)
</input_context_files>

<scope>
<in_scope>
- Create network.rs in coherence module
- Implement KuramotoNetwork struct with fixed arrays [f32; 13]
- Implement new() constructor with compile-time assertion
- Implement step(dt) for phase evolution
- Implement compute_order_parameter() for r(t) calculation
- Implement order_parameter() getter
</in_scope>
<out_of_scope>
- KuramotoStepper async wrapper (TASK-GWT-003)
- Integration with MCP (TASK-DREAM-004)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-core/src/layers/coherence/network.rs
use crate::layers::coherence::constants::{KURAMOTO_N, KURAMOTO_BASE_FREQUENCIES};

/// Kuramoto oscillator network for GWT coherence modeling.
///
/// Constitution: AP-25, GWT-002
/// INVARIANT: MUST have exactly 13 oscillators
#[derive(Debug, Clone)]
pub struct KuramotoNetwork {
    /// Oscillator phases (radians) - MUST be len == 13
    phases: [f32; KURAMOTO_N],

    /// Natural frequencies (Hz) - MUST be len == 13
    natural_frequencies: [f32; KURAMOTO_N],

    /// Coupling strength between oscillators
    coupling: f32,

    /// Current order parameter r(t) in [0, 1]
    order_parameter: f32,

    /// Mean phase psi(t)
    mean_phase: f32,
}

impl KuramotoNetwork {
    /// Create a new Kuramoto network with 13 oscillators.
    ///
    /// # Panics
    ///
    /// Panics at compile time if KURAMOTO_N != 13 (const assertion)
    pub fn new(coupling: f32) -> Self;

    /// Step the network forward by dt seconds.
    ///
    /// Updates all oscillator phases using Kuramoto dynamics:
    /// dtheta_i/dt = omega_i + (K/N) * sum_j sin(theta_j - theta_i)
    pub fn step(&mut self, dt: f32);

    /// Get the current order parameter r(t) in [0, 1].
    ///
    /// r = 1 means perfect synchronization (all oscillators in phase)
    /// r = 0 means no synchronization (uniform phase distribution)
    #[inline]
    pub fn order_parameter(&self) -> f32;

    /// Get the mean phase psi(t).
    #[inline]
    pub fn mean_phase(&self) -> f32;

    /// Get current phases for all oscillators.
    pub fn phases(&self) -> &[f32; KURAMOTO_N];

    /// Set coupling strength.
    pub fn set_coupling(&mut self, coupling: f32);
}
```
</signatures>
<constraints>
- MUST use fixed array [f32; KURAMOTO_N] not Vec
- Constructor MUST have compile-time assertion KURAMOTO_N == 13
- step() MUST implement correct Kuramoto dynamics
- order_parameter() MUST return value in [0, 1]
</constraints>
<verification>
```bash
cargo test -p context-graph-core kuramoto_network
cargo test -p context-graph-core test_order_parameter_bounds
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-core/src/layers/coherence/network.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-core/src/layers/coherence/mod.rs (add network module)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-core coherence
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Kuramoto Dynamics

The Kuramoto model equations:
```
d theta_i / dt = omega_i + (K/N) * sum_{j=1}^{N} sin(theta_j - theta_i)
```

Where:
- `theta_i` = phase of oscillator i
- `omega_i` = natural frequency of oscillator i
- `K` = coupling strength
- `N` = number of oscillators (13)

### Order Parameter Calculation

```
r * e^(i*psi) = (1/N) * sum_{j=1}^{N} e^(i*theta_j)
```

This gives:
- `r` = order parameter (magnitude)
- `psi` = mean phase (angle)

In code:
```rust
let sin_avg = phases.iter().map(|&t| t.sin()).sum::<f32>() / N;
let cos_avg = phases.iter().map(|&t| t.cos()).sum::<f32>() / N;
r = (sin_avg*sin_avg + cos_avg*cos_avg).sqrt();
psi = sin_avg.atan2(cos_avg);
```

### Test Cases

1. All oscillators at same phase -> r = 1.0
2. Oscillators uniformly distributed -> r ~ 0.0
3. Order parameter bounded [0, 1]
4. Phase evolution is continuous
