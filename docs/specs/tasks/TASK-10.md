# TASK-GWT-001: Add KURAMOTO_N constant (13 oscillators)

```xml
<task_spec id="TASK-GWT-001" version="1.0">
<metadata>
  <title>Add KURAMOTO_N constant (13 oscillators)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements><requirement_ref>REQ-GWT-001</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>1</estimated_hours>
</metadata>

<context>
Constitution specifies GWT must use a Kuramoto network with 13 oscillators representing
different brain rhythms. The constant and frequency array must be defined correctly
to ensure constitution compliance.
Constitution: gwt.kuramoto.frequencies (13 values), GWT-002
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-core/src/layers/coherence/ (directory structure)
</input_context_files>

<scope>
<in_scope>
- Create constants.rs in coherence module
- Define KURAMOTO_N = 13 as compile-time constant
- Define KURAMOTO_BASE_FREQUENCIES array with 13 values
- Add documentation mapping each frequency to brain rhythm
</in_scope>
<out_of_scope>
- KuramotoNetwork struct (TASK-GWT-002)
- KuramotoStepper (TASK-GWT-003)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-core/src/layers/coherence/constants.rs

/// Number of oscillators in Kuramoto network.
/// Constitution: gwt.kuramoto.frequencies (13 values)
pub const KURAMOTO_N: usize = 13;

/// Base frequencies for each oscillator (Hz).
///
/// Constitution mapping (gwt.kuramoto.frequencies):
/// - [0]  gamma_fast   = 40.0 Hz  (perception binding)
/// - [1]  theta_slow   = 8.0 Hz   (memory consolidation)
/// - [2]  theta_2      = 8.0 Hz   (hippocampal rhythm)
/// - [3]  theta_3      = 8.0 Hz   (prefrontal sync)
/// - [4]  beta_1       = 25.0 Hz  (motor planning)
/// - [5]  delta        = 4.0 Hz   (deep sleep)
/// - [6]  beta_2       = 25.0 Hz  (active thinking)
/// - [7]  alpha        = 12.0 Hz  (relaxed awareness)
/// - [8]  high_gamma   = 80.0 Hz  (cross-modal binding)
/// - [9]  gamma_mid    = 40.0 Hz  (attention)
/// - [10] beta_3       = 15.0 Hz  (cognitive control)
/// - [11] gamma_low    = 60.0 Hz  (sensory processing)
/// - [12] delta_slow   = 4.0 Hz   (slow wave sleep)
pub const KURAMOTO_BASE_FREQUENCIES: [f32; KURAMOTO_N] = [
    40.0,  // gamma_fast
    8.0,   // theta_slow
    8.0,   // theta_2
    8.0,   // theta_3
    25.0,  // beta_1
    4.0,   // delta
    25.0,  // beta_2
    12.0,  // alpha
    80.0,  // high_gamma
    40.0,  // gamma_mid
    15.0,  // beta_3
    60.0,  // gamma_low
    4.0,   // delta_slow
];

/// Default coupling strength for Kuramoto network.
pub const KURAMOTO_DEFAULT_COUPLING: f32 = 0.5;

/// Step interval for Kuramoto stepper (10ms = 100Hz).
pub const KURAMOTO_STEP_INTERVAL_MS: u64 = 10;
```
</signatures>
<constraints>
- KURAMOTO_N MUST be exactly 13
- Array length MUST match KURAMOTO_N (compile-time check)
- Frequencies MUST match constitution values
- Documentation MUST map each index to brain rhythm name
</constraints>
<verification>
```bash
cargo check -p context-graph-core
# Verify constant value
grep -q "KURAMOTO_N: usize = 13" crates/context-graph-core/src/layers/coherence/constants.rs
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-core/src/layers/coherence/constants.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-core/src/layers/coherence/mod.rs (add constants module)
</files_to_modify>

<test_commands>
```bash
cargo check -p context-graph-core
cargo test -p context-graph-core constants
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Compile-Time Validation

The array type `[f32; KURAMOTO_N]` ensures compile-time check that array has exactly 13 elements.
Any mismatch will cause compilation error.

### Constitution Reference

The frequencies come from constitution section `gwt.kuramoto.frequencies`.
Each frequency represents a distinct brain rhythm band used in GWT modeling.

### Frequency Bands

| Band | Range (Hz) | Function |
|------|------------|----------|
| Delta | 1-4 | Deep sleep, restoration |
| Theta | 4-8 | Memory, navigation |
| Alpha | 8-13 | Relaxed awareness |
| Beta | 13-30 | Active thinking, motor |
| Gamma | 30-100+ | Binding, consciousness |
