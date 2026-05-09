# TASK-INSTR-005 — EOracleInstrument verify + edge-case audit

```xml
<task_spec id="TASK-INSTR-005" version="1.0">

<metadata>
  <title>EOracleInstrument verify (shipped) + edge-case audit (zero-tests, large-count, no-exception, all-skip)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="REQ-INSTR-09">EOracleInstrument 128-d</item>
    <item ref="component:EOracleInstrument">encode(OracleVerdict) -> 128 f32</item>
    <item ref="EC-INSTR-03">total_test_count == 0</item>
    <item ref="EC-INSTR-04">total_test_count > 1e6 bounded</item>
    <item ref="AC-INSTR-02-02">crash_assertion_error boundary case</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file modified, 0 created</estimated_files>
</metadata>

<context>
`EOracleInstrument` is shipped at `crates/context-graph-mejepa-instruments/src/e_oracle.rs`.
Layout per doc 09 §2.2: dims 0..4 = pass/fail/skip/error fractions, dim 4 = total bounded to 1e6,
dim 5 = has_exception flag, dims 6..16 = 10-class exception one-hot, dims 16..128 = zero-padding
reserved for future signal dims (e.g. stack-trace embedding). 4 boundary FSV cases shipped:
all_pass, mixed, crash_assertion_error, crash_type_error. This task verifies signatures + adds
edge-case tests not yet covered: zero-test verdict, total > 1e6, no-exception with empty
per_test_outcomes, all-skip outcome, exception_class one-hot dim coverage for all 10 classes.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-09, EC-INSTR-03/04, AC-INSTR-02-02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts EOracleInstrument)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_oracle.rs</file>
  <file purpose="shipped_fsv">/tmp/contextgraph-mejepa-instruments-fsv/{all_pass,mixed,crash_assertion_error,crash_type_error}_{input,encoded}.json</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003 done (Panel/PanelBuilder verified)</check>
  <check>cargo test -p context-graph-mejepa-instruments e_oracle passes for shipped suite</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped signatures (`EOracleInstrument`, `OracleVerdict`, `PerTestOutcome`, `TestOutcome`, `ExceptionClass`)
    - Verify shipped 128-d layout (dims 0..4 fractions, dim 4 total, dim 5 has_exception, dims 6..16 one-hot, dims 16..128 zero)
    - Add edge-case test: total_test_count == 0 with no exception → all-zero output (still finite, slot.dim correct)
    - Add edge-case test: total_test_count == 0 with exception (e.g. ImportError) → dim 5 = 1.0, dim 6+offset = 1.0
    - Add edge-case test: total_test_count > 1e6 → bounded to 1e6 in dim 4 (raw float; verify)
    - Add edge-case test: all-skip outcome → fractions[2] = 1.0
    - Add per-class one-hot coverage test: iterate all 10 ExceptionClass variants, encode, assert correct dim is set to 1.0
    - Add deterministic-encode test: same input twice → byte-equal output (NFR-INSTR-03)
    - Add test asserting dims 16..128 are exactly 0.0 (zero-padding reserved)
  </in_scope>
  <out_of_scope>
    - Stack-trace embedding extension (future signal expansion in dims 16..128)
    - Multi-test exception aggregation (single top-level exception only)
    - linear-probe gate (TASK-INSTR-021)
    - panel JSON persistence (TASK-INSTR-022)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_oracle.rs">
#[derive(Debug, Default, Clone, Copy)]
pub struct EOracleInstrument;

impl Instrument for EOracleInstrument {
    type Input = OracleVerdict;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EOracle }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;;  // returns 128 f32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TestOutcome { Pass, Fail, Skip, Error }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExceptionClass {
    AssertionError, TypeError, ValueError, ImportError, AttributeError,
    KeyError, IndexError, NameError, RuntimeError, Other,
}

impl ExceptionClass {
    pub fn all() -&gt; [Self; 10];   // canonical order for one-hot dims 6..16
    pub fn index(&amp;self) -&gt; usize; // 0..10
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerTestOutcome { pub test_id: String, pub outcome: TestOutcome }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OracleVerdict {
    pub per_test_outcomes: Vec&lt;PerTestOutcome&gt;,
    pub exception_class: Option&lt;ExceptionClass&gt;,
    pub total_test_count: u32,
}
    </signature>
  </signatures>
  <constraints>
    - Output length is exactly 128 (slot.dim())
    - dims 16..128 strictly zero
    - total_test_count bound = 1_000_000 (per shipped behavior)
    - Encoder is deterministic (no PRNG, no rand::thread_rng)
    - Input mock data is forbidden by constitution; tests use real OracleVerdict struct values from corpus FSV
    - No unwrap() on potentially-fallible code paths inside encode()
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_oracle:: shows >= 8 tests pass (existing + 5 new)
    - encoding 100 random verdicts twice produces byte-equal results
  </verification>
</definition_of_done>

<pseudo_code>
add to e_oracle.rs (#[cfg(test)] mod tests):

  fn make_verdict(pass: u32, fail: u32, skip: u32, error: u32, exc: Option&lt;ExceptionClass&gt;) -&gt; OracleVerdict {
      let mut outs = vec![];
      for _ in 0..pass { outs.push(PerTestOutcome { test_id: "p".into(), outcome: TestOutcome::Pass }); }
      // ... fail / skip / error
      OracleVerdict {
          total_test_count: pass + fail + skip + error,
          per_test_outcomes: outs,
          exception_class: exc,
      }
  }

  #[test]
  fn zero_tests_no_exception() {
      let v = make_verdict(0,0,0,0, None);
      let out = EOracleInstrument.encode(&amp;v).unwrap();
      assert_eq!(out.len(), 128);
      assert_eq!(out[0], 0.0); // pass fraction
      assert_eq!(out[5], 0.0); // no exception
      for i in 6..128 { assert_eq!(out[i], 0.0); }
  }

  #[test]
  fn zero_tests_with_import_error() {
      let v = make_verdict(0,0,0,0, Some(ExceptionClass::ImportError));
      let out = EOracleInstrument.encode(&amp;v).unwrap();
      assert_eq!(out[5], 1.0);
      let import_idx = ExceptionClass::ImportError.index();
      assert_eq!(out[6 + import_idx], 1.0);
  }

  #[test]
  fn total_count_bounded_to_one_million() {
      let mut v = make_verdict(0,0,0,0, None);
      v.total_test_count = 5_000_000;
      let out = EOracleInstrument.encode(&amp;v).unwrap();
      // dim 4 holds bounded total — must not exceed 1e6
      assert!(out[4] &lt;= 1_000_000.0_f32 + f32::EPSILON);
  }

  #[test]
  fn all_skip_fraction() {
      let v = make_verdict(0,0,10,0, None);
      let out = EOracleInstrument.encode(&amp;v).unwrap();
      assert!((out[2] - 1.0).abs() &lt; 1e-6);  // skip fraction
  }

  #[test]
  fn all_ten_exception_classes_distinct_dims() {
      for c in ExceptionClass::all() {
          let v = make_verdict(0,0,0,0, Some(c));
          let out = EOracleInstrument.encode(&amp;v).unwrap();
          let idx = c.index();
          assert_eq!(out[6 + idx], 1.0);
          // and all other 9 one-hot dims are 0.0
          for j in 0..10 { if j != idx { assert_eq!(out[6+j], 0.0); } }
      }
  }

  #[test]
  fn deterministic_encode_byte_equal() {
      let v = make_verdict(7,3,2,0, Some(ExceptionClass::AssertionError));
      let a = EOracleInstrument.encode(&amp;v).unwrap();
      let b = EOracleInstrument.encode(&amp;v).unwrap();
      assert_eq!(a, b);
  }

  #[test]
  fn zero_padding_dims_strictly_zero() {
      let v = make_verdict(7,3,2,0, Some(ExceptionClass::AssertionError));
      let out = EOracleInstrument.encode(&amp;v).unwrap();
      for i in 16..128 { assert_eq!(out[i], 0.0, "dim {i} must be reserved zero-padding"); }
  }
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_oracle.rs">Add 7 tests covering zero-tests, count bounding, all-skip, full one-hot coverage, determinism, zero-padding</file>
</files_to_modify>

<validation_criteria>
  <criterion>EOracleInstrument output is exactly 128-d</criterion>
  <criterion>dims 16..128 are strictly 0.0 (reserved zero-padding)</criterion>
  <criterion>10 exception classes map to 10 distinct one-hot dims at 6..16</criterion>
  <criterion>total_test_count bounded to 1e6</criterion>
  <criterion>Deterministic encode (byte-equal across runs)</criterion>
  <criterion>cargo test -p context-graph-mejepa-instruments e_oracle:: passes >= 8 tests</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_oracle::</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
