# TASK-INSTR-017 — ERuntimeInstrument verify + extend (determinism flag, syscall histogram)

```xml
<task_spec id="TASK-INSTR-017" version="1.0">

<metadata>
  <title>ERuntimeInstrument verify (shipped) + extend (determinism flag check, syscall histogram bounded, exit-code per-test breakout)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>17</sequence>
  <implements>
    <item ref="REQ-INSTR-13">ERuntimeInstrument 256-d</item>
    <item ref="component:ERuntimeInstrument">encode(RuntimeInput) -&gt; 256 f32</item>
    <item ref="data_model:RuntimeInput">walltime_ns_p50/p99, peak_rss_bytes, allocation_count, syscall_histogram, coverage_delta_*, filesystem_paths_touched, network_calls, exit_codes, determinism_flag</item>
    <item ref="EC-INSTR-13">walltime variance &gt; 30% across reruns sets determinism_flag = false</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file modified, 0 created</estimated_files>
</metadata>

<context>
`ERuntimeInstrument` is shipped at `crates/context-graph-mejepa-instruments/src/e_runtime.rs`
as a deterministic typed encoder over `RuntimeInput` (walltime, peak RSS, allocation count,
syscall histogram, coverage delta, filesystem touches, network calls, exit codes, determinism
flag). This task verifies signatures + extends edge-case coverage:

1. determinism_flag = false propagation (downstream training treats as lower-confidence supervision)
2. syscall_histogram bounded to 256 unique syscall names — reject larger
3. exit_codes per-test breakout (signal vs assertion vs normal)
4. coverage_delta lines AND branches both encoded
5. NaN/Inf in any numeric field rejected fail-closed

This is verify+extend (not neural). The deterministic encoder remains the production instrument.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-13, EC-INSTR-13)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts ERuntimeInstrument; data_model RuntimeInput)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_runtime.rs</file>
  <file purpose="design_doc_runtime">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.5.2</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003 done</check>
  <check>cargo test -p context-graph-mejepa-instruments e_runtime passes for shipped suite</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped ERuntimeInstrument encode produces exactly 256 finite f32
    - Add unit test: determinism_flag=false propagates as a 1-d feature in the latent
    - Add unit test: syscall_histogram with 256 unique entries succeeds; 257 rejected with MEJEPA_INSTRUMENTS_INVALID_INPUT
    - Add unit test: exit_codes with signal (-9), assertion (1), normal (0) tagged correctly
    - Add unit test: coverage_delta_lines and coverage_delta_branches both encoded (separate dims)
    - Add unit test: NaN/Inf in any numeric field returns NumericalInvariant
    - Add unit test: empty syscall_histogram succeeds (zero-fill)
    - Add unit test: walltime_ns_p99 &lt; walltime_ns_p50 returns InvalidInput (illogical input)
    - Add release-mode determinism test: same RuntimeInput hashed-encoded twice = byte-equal
    - Add `MAX_SYSCALL_NAMES = 256` const if not already exported
  </in_scope>
  <out_of_scope>
    - Replacing deterministic encoder with neural backing (deferred; deterministic is production target)
    - Trace event collection (E_Trace's job)
    - Actual runtime instrumentation (Phase 5 self-healing)
    - Linear-probe gate (TASK-INSTR-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_runtime.rs">
pub const MAX_SYSCALL_NAMES: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInput {
    pub walltime_ns_p50: u64,
    pub walltime_ns_p99: u64,
    pub peak_rss_bytes: u64,
    pub allocation_count: u64,
    pub syscall_histogram: HashMap&lt;String, u64&gt;,
    pub coverage_delta_lines: i64,
    pub coverage_delta_branches: i64,
    pub filesystem_paths_touched: u32,
    pub network_calls: u32,
    pub exit_codes: HashMap&lt;String, i32&gt;,
    pub determinism_flag: bool,
}

pub struct ERuntimeInstrument;

impl Instrument for ERuntimeInstrument {
    type Input = RuntimeInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::ERuntime }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;; // 256 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 256 finite f32
    - syscall_histogram &gt; MAX_SYSCALL_NAMES (256) -&gt; MEJEPA_INSTRUMENTS_INVALID_INPUT
    - walltime_ns_p99 &lt; walltime_ns_p50 -&gt; InvalidInput (illogical)
    - NaN/Inf checks via finite-validate output
    - Deterministic — sorted keys for hash-bin assignment
    - Latency budget: &lt; 1 ms (scalar aggregator per NFR-INSTR-02)
    - Existing 27+ tests pass; new tests are additive
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_runtime
    - cargo clippy -p context-graph-mejepa-instruments -- -D warnings clean
    - Determinism test passes in release mode
  </verification>
</definition_of_done>

<pseudo_code>
e_runtime.rs (extension):

  pub const MAX_SYSCALL_NAMES: usize = 256;

  fn validate_input(input: &amp;RuntimeInput) -&gt; InstrumentResult&lt;()&gt; {
      if input.syscall_histogram.len() &gt; MAX_SYSCALL_NAMES {
          return Err(InstrumentError::invalid("syscall_histogram",
              format!("&gt; {}", MAX_SYSCALL_NAMES), "bound to 256 unique syscall names"));
      }
      if input.walltime_ns_p99 &lt; input.walltime_ns_p50 {
          return Err(InstrumentError::invalid("walltime",
              "p99 &lt; p50 (illogical)", "ensure p99 &gt;= p50"));
      }
      Ok(())
  }

  impl Instrument for ERuntimeInstrument {
      fn encode(&amp;self, input: &amp;RuntimeInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt; {
          validate_input(input)?;
          let mut out = vec![0.0_f32; 256];
          // dims 0..4: log1p walltime / RSS / alloc count / coverage delta lines
          // dims 4..8: coverage delta branches / filesystem paths / network calls / determinism_flag (0/1)
          // dims 8..72: syscall histogram (64 hash bins; sorted keys for determinism)
          // dims 72..136: exit_codes histogram (64 hash bins; sorted keys; encode signal vs assertion vs normal)
          // dims 136..256: zero-padded reserved
          ...
          validate_finite_output("e_runtime.output", &amp;out)?;
          Ok(out)
      }
  }

new tests in e_runtime_tests:
  - syscall_histogram_at_max_succeeds (256 entries)
  - syscall_histogram_above_max_rejected (257 entries)
  - p99_lt_p50_rejected
  - determinism_flag_false_propagates
  - coverage_delta_lines_branches_distinct_dims
  - empty_syscall_histogram_succeeds
  - encode_byte_equal_release_mode
  - exit_codes_signal_assertion_normal_distinguished
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_runtime.rs">Add MAX_SYSCALL_NAMES const, validate_input helper, 8 new tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 256 finite f32</criterion>
  <criterion>syscall_histogram &gt; 256 rejected</criterion>
  <criterion>p99 &lt; p50 rejected</criterion>
  <criterion>determinism_flag visible as a feature in latent</criterion>
  <criterion>cargo test -p context-graph-mejepa-instruments e_runtime shows new tests passing</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>Latency &lt; 1 ms</criterion>
  <criterion>27+ shipped tests still pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_runtime</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
