# TASK-EMBED-017 — E2/E3/E4 temporal embedders (one task, all 3)

```xml
<task_spec id="TASK-EMBED-017" version="1.0">
<metadata>
  <title>EmbedderInstrument wrappers for E2 TemporalRecent (exp decay, 512-d), E3 TemporalPeriodic (Fourier, 512-d), E4 TemporalPositional (sinusoidal PE, 512-d)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>17</sequence>
  <implements>
    <item ref="REQ-EMBED-01">E2/E3/E4 wrappers</item>
    <item ref="REQ-EMBED-03">3 temporal slots in EmbedderId enum</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-009</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
The 3 temporal embedders are pure-Rust deterministic functions of `TemporalContext` (Unix timestamp + decay constants). No GPU weight loading; basis-only (~1 MB combined VRAM). Grouped into one task because they share the same dispatch surface (EmbedderInput::Temporal) and ship as small, parallel modules in one file.
</context>

<input_context_files>
  <file purpose="temporal_pattern">crates/context-graph-embeddings/src/types/temporal.rs</file>
  <file purpose="upstream_e2_e3_e4">crates/context-graph-embeddings/src/types/model_id/core.rs</file>
  <file purpose="e1_template">crates/context-graph-mejepa-embedders/src/embedders/e1_semantic.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-009 complete</check>
  <check>Upstream `TemporalContext { wall_clock: i64, decay_rates: Vec<f32> }` available</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct E2TemporalRecentForward;` — exponential decay encoding of Unix timestamp through `decay_rates` → 512-d
    - `pub struct E3TemporalPeriodicForward;` — Fourier basis (sin/cos at multiple frequencies) → 512-d
    - `pub struct E4TemporalPositionalForward;` — sinusoidal PE of absolute Unix timestamp → 512-d (per MEMORY.md E2 fix lesson; E4 explicitly inherits the sinusoidal-PE design)
    - `impl EmbedderForward` for each; validate Temporal input variant; reject ts ≤ 0; populate vector deterministically; validate_finite + validate_dim(512)
    - Factories + register fns for all 3
    - 3 Instrument structs (E2TemporalRecentInstrument, etc.)
    - All forwards CPU-only (no GPU); ~1 MB total VRAM "estimate" (basis storage)
    - Tests: deterministic output across 100 trials with same input; ts ≤ 0 rejected; sinusoidal PE wraps appropriately; vector len 512
  </in_scope>
  <out_of_scope>
    - Decay-rate calibration (operator-set defaults; out of scope)
    - Time-zone handling (Unix timestamp is monotonic; assume UTC)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/embedders/temporal.rs">
      pub const E2_PROJECTED_DIM: usize = 512;
      pub const E3_PROJECTED_DIM: usize = 512;
      pub const E4_PROJECTED_DIM: usize = 512;
      pub const E2_VERSION: &amp;str = "exp-decay-v1";
      pub const E3_VERSION: &amp;str = "fourier-basis-v1";
      pub const E4_VERSION: &amp;str = "sinusoidal-pe-v1";

      pub struct E2TemporalRecentForward;
      pub struct E3TemporalPeriodicForward;
      pub struct E4TemporalPositionalForward;

      impl EmbedderForward for E2TemporalRecentForward { /* exp decay */ }
      impl EmbedderForward for E3TemporalPeriodicForward { /* Fourier */ }
      impl EmbedderForward for E4TemporalPositionalForward { /* sinusoidal PE */ }

      pub struct E2TemporalRecentFactory;
      pub struct E3TemporalPeriodicFactory;
      pub struct E4TemporalPositionalFactory;

      impl EmbedderForwardFactory for E2TemporalRecentFactory { /* ... */ }
      impl EmbedderForwardFactory for E3TemporalPeriodicFactory { /* ... */ }
      impl EmbedderForwardFactory for E4TemporalPositionalFactory { /* ... */ }

      pub fn register_all();

      pub struct E2TemporalRecentInstrument { ensemble: Arc&lt;Ensemble&gt; }
      pub struct E3TemporalPeriodicInstrument { ensemble: Arc&lt;Ensemble&gt; }
      pub struct E4TemporalPositionalInstrument { ensemble: Arc&lt;Ensemble&gt; }

      impl Instrument for E2TemporalRecentInstrument { /* ... */ }
      impl Instrument for E3TemporalPeriodicInstrument { /* ... */ }
      impl Instrument for E4TemporalPositionalInstrument { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - 512-d each.
    - Temporal forward is sync, CPU-only, deterministic across runs given same `TemporalContext`.
    - ts ≤ 0 → InvalidInput { reason: "non-positive Unix timestamp" }.
    - validate_finite + validate_dim(512) before returning.
    - precision_class always Fp32 (no GPU bf16 path here).
    - register_all() registers all three factories at once (idiomatic group registration).
  </constraints>

  <verification>
    - 100 trials with identical TemporalContext produce byte-equal output for each of E2/E3/E4
    - ts = 0 rejected with InvalidInput
    - vector len 512 each
    - cargo test temporal passes
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
temporal.rs:
  - E2: vector[i] = exp(-(now - ts) / decay_rates[i]) for 512 rates.
  - E3: vector[i] = sin(2π · ts · freq_i) for 256 freqs + cos for 256 → 512.
  - E4: standard sinusoidal PE pattern over absolute ts → 512.
  - Factories + register_all().
  - Instrument impls delegating to ensemble.encode.
  - Tests with fixed ts = 1_700_000_000.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/temporal.rs">E2/E3/E4 forwards + Factories + Instruments + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/mod.rs">Add `pub mod temporal;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>E2/E3/E4 produce 512-d vectors</criterion>
  <criterion>Deterministic byte-equal across 100 trials per embedder</criterion>
  <criterion>ts ≤ 0 rejected with InvalidInput</criterion>
  <criterion>register_all() wires all 3 factories</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders temporal</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```
