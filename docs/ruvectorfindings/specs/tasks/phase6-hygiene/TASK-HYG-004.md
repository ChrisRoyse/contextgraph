# TASK-HYG-004 — Tier enum + thresholds + decay_factor + tier_for_frequency

```xml
<task_spec id="TASK-HYG-004" version="1.0">
<metadata>
  <title>Tier enum + threshold table + decay_factor (0.99 per 24h) + tier_for_frequency mapper</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <requirement_ref>REQ-HYG-02</requirement_ref>
    <requirement_ref>REQ-HYG-06</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
The 5-tier policy state machine: Hot (raw f32) → Warm (8-bit) → Cool (5-bit) → Cold (3-bit) →
Frozen (1-bit sign). Frequency thresholds {≥0.8 Hot, 0.6-0.8 Warm, 0.4-0.6 Cool, 0.2-0.4 Cold,
<0.2 Frozen}. Per-24h decay factor 0.99 per doc 09 §6.2. This task ships the pure-math layer:
no I/O, no encoder dispatch (TASK-HYG-005 wires that up). Promotion / demotion symmetry is
implemented in TASK-HYG-007.
</context>

<input_context_files>
  <file purpose="tier_policy">../../technical/TECH-HYGIENE.md#data_models</file>
  <file purpose="tier_diagram">../../technical/TECH-HYGIENE.md#architecture_diagram</file>
  <file purpose="error">crates/context-graph-mejepa-hygiene/src/error.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-001 shipped</check>
  <check>TASK-HYG-002 shipped (OpsError exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/tier.rs` with:
      * `pub enum Tier { Hot, Warm, Cool, Cold, Frozen }` derives Debug, Clone, Copy,
        PartialEq, Eq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize.
      * `Tier::all() -> &'static [Tier; 5]`
      * `Tier::slug() -> &'static str` ("hot", "warm", "cool", "cold", "frozen")
      * `Tier::threshold_lower(self) -> f32` (Hot 0.8, Warm 0.6, Cool 0.4, Cold 0.2, Frozen f32::NEG_INFINITY)
      * `Tier::next_colder(self) -> Option<Tier>` (Hot→Warm, ..., Frozen→None)
      * `Tier::next_warmer(self) -> Option<Tier>` (Frozen→Cold, ..., Hot→None)
      * `Tier::bit_width(self) -> Option<u8>` (Hot=None raw f32; Warm=Some(8); Cool=Some(5); Cold=Some(3); Frozen=Some(1))
    - `pub fn decay_factor(elapsed_24h_units: f64) -> f64` returning `0.99_f64.powf(elapsed_24h_units)`
      bounded for finite f64 (refuse NaN/Inf with OpsError::Internal).
    - `pub fn decayed_frequency(prev_freq: f32, last_read_ts_ns: u64, now_ts_ns: u64) -> f32`
      computing freq * 0.99^(elapsed_h / 24) clamped to [0.0, 1.0]. Refuse `now_ts_ns < last_read_ts_ns`
      with OpsError (no time travel).
    - `pub fn incremented_frequency(prev_freq: f32, last_read_ts_ns: u64, now_ts_ns: u64) -> f32`
      = `(decayed_frequency + 1.0).min(1.0)` per AC-HYG-05-01.
    - `pub fn tier_for_frequency(freq: f32) -> Tier` mapping per the threshold table.
      Refuse non-finite freq with panic? — no: clamp to 0.0 and log WARN per fail-honest.
    - Unit tests covering boundary cases (0.8 exact = Warm or Hot? → Hot per ≥0.8;
      0.0 = Frozen; 1.0 = Hot; round-trip Tier slug).
  </in_scope>
  <out_of_scope>
    - encode_for_tier / decode_for_tier (TASK-HYG-005).
    - tier_demote / tier_promote (TASK-HYG-007).
    - Frozen 1-bit encoding body (TASK-HYG-006).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/tier.rs">
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Tier { Frozen, Cold, Cool, Warm, Hot }
    // Note: ordered Frozen &lt; Cold &lt; ... &lt; Hot so derive(Ord) gives natural "colder &lt; hotter".

    impl Tier {
        pub const fn all() -&gt; &amp;'static [Tier; 5];
        pub const fn slug(self) -&gt; &amp;'static str;
        pub const fn threshold_lower(self) -&gt; f32;
        pub const fn next_colder(self) -&gt; Option&lt;Tier&gt;;
        pub const fn next_warmer(self) -&gt; Option&lt;Tier&gt;;
        pub const fn bit_width(self) -&gt; Option&lt;u8&gt;;
    }

    pub fn decay_factor(elapsed_24h_units: f64) -&gt; crate::error::OpsResult&lt;f64&gt;;
    pub fn decayed_frequency(prev_freq: f32, last_read_ts_ns: u64, now_ts_ns: u64) -&gt; crate::error::OpsResult&lt;f32&gt;;
    pub fn incremented_frequency(prev_freq: f32, last_read_ts_ns: u64, now_ts_ns: u64) -&gt; crate::error::OpsResult&lt;f32&gt;;
    pub fn tier_for_frequency(freq: f32) -&gt; Tier;
    </signature>
  </signatures>

  <constraints>
    - threshold_lower returns NEG_INFINITY for Frozen so any finite freq ≥ that.
    - Comparisons use `freq >= 0.8` (≥, not strict >) per the table inclusive lower bound.
    - decay_factor with elapsed=0 returns exactly 1.0.
    - decayed_frequency(0.0, _, _) returns 0.0 (no spurious sub-zero from floating drift).
    - incremented_frequency clamps to 1.0 hard.
    - now_ts_ns < last_read_ts_ns returns OpsError::Internal { reason: "time_travel" } — no
      silent abs() / sentinel.
    - non-finite freq input is clamped to 0.0 with `tracing::warn!` (fail-honest, not silent).
    - File under 500 lines.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- tier_for_frequency` passes (boundary cases).
    - `cargo test -p context-graph-mejepa-hygiene -- decay_factor` passes (0 → 1.0; 1 day → 0.99; 30 days → 0.99^30 ≈ 0.7397).
    - `cargo clippy -p context-graph-mejepa-hygiene -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub const fn next_colder(self) -&gt; Option&lt;Tier&gt; {
    match self {
        Tier::Hot =&gt; Some(Tier::Warm),
        Tier::Warm =&gt; Some(Tier::Cool),
        Tier::Cool =&gt; Some(Tier::Cold),
        Tier::Cold =&gt; Some(Tier::Frozen),
        Tier::Frozen =&gt; None,
    }
}

pub fn tier_for_frequency(freq: f32) -&gt; Tier {
    let f = if freq.is_finite() { freq } else { tracing::warn!(target:"hygiene", "non-finite freq clamped to 0.0"); 0.0 };
    if f &gt;= 0.8 { Tier::Hot }
    else if f &gt;= 0.6 { Tier::Warm }
    else if f &gt;= 0.4 { Tier::Cool }
    else if f &gt;= 0.2 { Tier::Cold }
    else { Tier::Frozen }
}

pub fn decayed_frequency(prev_freq: f32, last_read_ts_ns: u64, now_ts_ns: u64) -&gt; OpsResult&lt;f32&gt; {
    if now_ts_ns &lt; last_read_ts_ns { return Err(OpsError::internal("time_travel")); }
    let elapsed_h = (now_ts_ns - last_read_ts_ns) as f64 / 3_600_000_000_000.0;
    let factor = decay_factor(elapsed_h / 24.0)?;
    Ok((prev_freq as f64 * factor).clamp(0.0, 1.0) as f32)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/tier.rs">Tier enum + threshold table + frequency math</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/lib.rs">tier module already declared in TASK-HYG-001; this populates body</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">add tier boundary tests + decay tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>tier_for_frequency boundaries: 0.8→Hot, 0.79→Warm, 0.6→Warm, 0.59→Cool, 0.4→Cool, 0.39→Cold, 0.2→Cold, 0.19→Frozen, 0.0→Frozen, 1.0→Hot</criterion>
  <criterion>decay_factor(0.0) == 1.0; decay_factor(1.0) == 0.99; decay_factor(30.0) ≈ 0.7397</criterion>
  <criterion>decayed_frequency time-travel returns OpsError::Internal</criterion>
  <criterion>incremented_frequency clamps to 1.0</criterion>
  <criterion>Tier::all().len() == 5; slug round-trip works</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- tier_for_frequency</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- decay_factor</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
