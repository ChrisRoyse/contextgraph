# TASK-INSTR-019 — ScalarsInstrument verify + extend (BFS depth, blame age, churn, complexity, coverage Δ, repo health → 128-d)

```xml
<task_spec id="TASK-INSTR-019" version="1.0">

<metadata>
  <title>ScalarsInstrument verify (shipped) + extend (16-bin histograms for BFS depth, blame age, churn, complexity, coverage Δ; repo cadence + CI green-rate + contributor count features)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>19</sequence>
  <implements>
    <item ref="REQ-INSTR-16">ScalarsInstrument 128-d</item>
    <item ref="component:ScalarsInstrument">encode(ScalarInput) -&gt; 128 f32</item>
    <item ref="data_model:ScalarInput">bfs_depth_from_touched_files, blame_age_days_p50, symbol_churn_30d, cyclomatic_complexity_delta, coverage_delta_pct, repo_commit_cadence_per_week, repo_ci_green_rate, repo_contributor_count, docstring_coverage_delta, loc_delta</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file modified, 0 created</estimated_files>
</metadata>

<context>
`ScalarsInstrument` is shipped at `crates/context-graph-mejepa-instruments/src/e_scalars.rs` as a
deterministic typed encoder over `ScalarInput`. Per TECH-INSTRUMENTS component_contracts
ScalarsInstrument, the 128-d layout is:

- dims 0..16: BFS depth one-hot + fractional
- dims 16..32: blame-age histogram (16 bins, 0..2 years)
- dims 32..48: symbol-churn histogram
- dims 48..64: complexity Δ histogram
- dims 64..80: coverage Δ histogram
- dims 80..96: repo cadence + CI green-rate + contributor count features
- dims 96..128: zero-padding reserved

This task verifies the layout matches that spec EXACTLY + extends edge-case coverage:

1. Each 16-bin histogram is verified in dim isolation
2. repo_ci_green_rate ∈ [0, 1] (clamp + reject NaN)
3. negative deltas (loc_delta &lt; 0; complexity Δ &lt; 0) handled in their respective bins
4. Empty / zero-everywhere ScalarInput produces a non-trivial output (zero-pad signal vs missing data)
5. NaN/Inf in any field rejected fail-closed
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-16)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts ScalarsInstrument; data_model ScalarInput)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_scalars.rs</file>
  <file purpose="design_doc_panel_layout">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.5.5</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003 done</check>
  <check>cargo test -p context-graph-mejepa-instruments e_scalars passes for shipped suite</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped ScalarsInstrument layout matches TECH-INSTRUMENTS exactly (dim ranges, 16-bin histograms)
    - Add unit test: BFS depth = 0 / 1 / 5 / 100 produces correct dim 0..16 layout
    - Add unit test: blame-age 0 / 30 / 365 / 730 days hits correct histogram bins
    - Add unit test: repo_ci_green_rate ∈ [0, 1] enforced; -0.1 or 1.1 rejected
    - Add unit test: cyclomatic_complexity_delta &lt; 0 (refactor reduces complexity) handled
    - Add unit test: NaN in blame_age_days_p50 rejected with NumericalInvariant
    - Add unit test: all-zero ScalarInput produces all-zero output dims (and is allowed)
    - Add unit test: negative loc_delta (LOC reduction) encoded in correct sign
    - Add release-mode determinism test
    - Confirm `dims 96..128` are zero-padded (no garbage)
  </in_scope>
  <out_of_scope>
    - Replacing deterministic encoder with neural backing
    - BFS computation itself (caller's responsibility — extracted from repo before ScalarInput is built)
    - Repo cadence / CI green-rate computation (caller)
    - Linear-probe gate (TASK-INSTR-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_scalars.rs">
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScalarInput {
    pub bfs_depth_from_touched_files: u32,
    pub blame_age_days_p50: f32,
    pub symbol_churn_30d: u32,
    pub cyclomatic_complexity_delta: i32,
    pub coverage_delta_pct: f32,
    pub repo_commit_cadence_per_week: f32,
    pub repo_ci_green_rate: f32,        // ∈ [0, 1]
    pub repo_contributor_count: u32,
    pub docstring_coverage_delta: f32,
    pub loc_delta: i32,
}

pub struct ScalarsInstrument;

impl Instrument for ScalarsInstrument {
    type Input = ScalarInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::Scalars }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;; // 128 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 128 finite f32
    - dim ranges per TECH-INSTRUMENTS component_contracts:
      - 0..16: BFS depth one-hot + fractional
      - 16..32: blame-age histogram
      - 32..48: symbol-churn histogram
      - 48..64: complexity Δ histogram
      - 64..80: coverage Δ histogram
      - 80..96: repo cadence + CI green-rate + contributor count
      - 96..128: zero-padded
    - repo_ci_green_rate validated to [0, 1]
    - NaN/Inf in any f32 field rejected with NumericalInvariant
    - Latency budget: &lt; 1 ms
    - Deterministic
    - Existing 27+ tests pass; new tests are additive
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_scalars
    - cargo clippy -p context-graph-mejepa-instruments -- -D warnings clean
  </verification>
</definition_of_done>

<pseudo_code>
e_scalars.rs (extension):

  fn validate_input(input: &amp;ScalarInput) -&gt; InstrumentResult&lt;()&gt; {
      for (name, v) in [
          ("blame_age_days_p50", input.blame_age_days_p50),
          ("coverage_delta_pct", input.coverage_delta_pct),
          ("repo_commit_cadence_per_week", input.repo_commit_cadence_per_week),
          ("repo_ci_green_rate", input.repo_ci_green_rate),
          ("docstring_coverage_delta", input.docstring_coverage_delta),
      ] {
          if !v.is_finite() {
              return Err(InstrumentError::invariant(name, format!("non-finite: {}", v), "scrub NaN/Inf"));
          }
      }
      if !(0.0..=1.0).contains(&amp;input.repo_ci_green_rate) {
          return Err(InstrumentError::invalid("repo_ci_green_rate",
              format!("out of [0, 1]: {}", input.repo_ci_green_rate), "clamp at caller"));
      }
      Ok(())
  }

new tests in e_scalars_tests:
  - bfs_depth_one_hot_correct
  - blame_age_histogram_buckets
  - ci_green_rate_out_of_range_rejected
  - negative_complexity_delta_handled
  - nan_blame_age_rejected
  - all_zero_scalar_input_succeeds
  - negative_loc_delta_signed
  - encode_byte_equal_release_mode
  - dim_96_128_zero_padded
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_scalars.rs">Add validate_input helper for [0,1] / NaN checks; 9 new tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 128 finite f32</criterion>
  <criterion>repo_ci_green_rate out of [0, 1] rejected</criterion>
  <criterion>NaN/Inf in any field rejected</criterion>
  <criterion>BFS / blame / churn / complexity / coverage histograms in correct dim ranges</criterion>
  <criterion>dims 96..128 are zero (reserved)</criterion>
  <criterion>cargo test -p context-graph-mejepa-instruments e_scalars passes new tests</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>Latency &lt; 1 ms</criterion>
  <criterion>27+ shipped tests still pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_scalars</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
