# TASK-INSTR-002 — Verify Instrument trait + InstrumentError taxonomy

```xml
<task_spec id="TASK-INSTR-002" version="1.0">

<metadata>
  <title>Verify Instrument trait + InstrumentError taxonomy + InstrumentResult alias</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-INSTR-18">Frozen-instrument trait contract (zero trainable params; deterministic encode)</item>
    <item ref="component:Instrument trait">Instrument::Input, slot(), output_dim(), encode()</item>
    <item ref="component:InstrumentError">InvalidInput, NumericalInvariant + .code() codes</item>
    <item ref="ERR-INSTR-01">vector length mismatch error</item>
    <item ref="ERR-INSTR-02">non-finite value error</item>
  </implements>
  <depends_on><!-- none; foundation --></depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file modified, 0 created</estimated_files>
</metadata>

<context>
The `Instrument` trait is the single contract every E_* implementation honors (15 implementations
across `e_oracle.rs`, `e_witness.rs`, `e_static_analysis.rs`, `e_code.rs`, `e_text.rs`,
`e_trace.rs`, `e_runtime.rs`, `e_reasoning.rs`, `e_scalars.rs`). The shipped trait
(`lib.rs` lines 221-237) defines `type Input`, `slot()`, `output_dim()`, `encode()`. The shipped
`InstrumentError` (lines 46-95) provides `InvalidInput` + `NumericalInvariant` variants with
SCREAMING_SNAKE error codes per constitution failure_taxonomy. This task verifies signatures and
adds three exhaustive trait-contract assertions: `output_dim() == slot().dim()` for every shipped
instrument (`Input` type matches the spec's data model entries), `.code()` returns the correct
SCREAMING_SNAKE tag, and remediation strings are non-empty.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy, error_handling rules)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-18, ERR-INSTR-01, ERR-INSTR-02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (api_contracts library_api Instrument trait)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/lib.rs (lines 44-95, 221-237)</file>
  <file purpose="shipped_implementations">crates/context-graph-mejepa-instruments/src/{e_oracle.rs, e_witness.rs, e_static_analysis.rs, e_code.rs, e_text.rs, e_trace.rs, e_runtime.rs, e_reasoning.rs, e_scalars.rs}</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-001 verified (InstrumentSlot stable)</check>
  <check>Existing 27 tests pass</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped `pub trait Instrument` signature matches spec exactly
    - Verify `InstrumentError` shipped variants + `.code()` mapping
    - Verify shipped `pub type InstrumentResult&lt;T&gt; = Result&lt;T, InstrumentError&gt;`
    - Add unit test asserting EVERY shipped instrument type satisfies `output_dim() == slot().dim()`
    - Add unit test asserting `InstrumentError::invalid().code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT"` and `::invariant().code() == "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT"`
    - Add unit test asserting remediation strings are non-empty for both variants
    - Add unit test asserting `Display` of both variants contains `field`, `message`, and `remediation`
  </in_scope>
  <out_of_scope>
    - Changing trait signature (would break 15 implementations)
    - Adding new error variants (would require constitution failure_taxonomy update)
    - Frozen-gradient hook (TASK-INSTR-004)
    - Per-instrument encode behavior (TASK-INSTR-005..019)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/lib.rs">
pub type InstrumentResult&lt;T&gt; = Result&lt;T, InstrumentError&gt;;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum InstrumentError {
    InvalidInput { field: &amp;'static str, message: String, remediation: &amp;'static str },
    NumericalInvariant { field: &amp;'static str, message: String, remediation: &amp;'static str },
}

impl InstrumentError {
    pub fn code(&amp;self) -&gt; &amp;'static str;            // "MEJEPA_INSTRUMENTS_INVALID_INPUT" | "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT"
    pub(crate) fn invalid(field, message, remediation) -&gt; Self;
    pub(crate) fn invariant(field, message, remediation) -&gt; Self;
}

pub trait Instrument {
    type Input;
    fn slot(&amp;self) -&gt; InstrumentSlot;
    fn output_dim(&amp;self) -&gt; usize { self.slot().dim() }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - No trait signature change (15 downstream implementors depend on it)
    - `output_dim` default body MUST stay `self.slot().dim()` so trait gives correct size by default
    - Error code strings are exact: `MEJEPA_INSTRUMENTS_INVALID_INPUT`, `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT`
    - `pub(crate)` constructors stay crate-private; external callers use `InstrumentError` variants directly only via `match`
    - thiserror Display contains all three fields
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-instruments` succeeds
    - `cargo test -p context-graph-mejepa-instruments instrument_trait_contract`
    - `cargo test -p context-graph-mejepa-instruments error_taxonomy`
    - `cargo clippy -p context-graph-mejepa-instruments -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
add to tests.rs:

  #[test]
  fn instrument_trait_contract_output_dim_matches_slot_dim() {
      use crate::*;
      // For each shipped instrument, default Default::default() and assert
      assert_eq!(EOracleInstrument.output_dim(), InstrumentSlot::EOracle.dim());
      assert_eq!(EWitnessInstrument.output_dim(), InstrumentSlot::EWitness.dim());
      assert_eq!(EStaticAnalysisInstrument.output_dim(), InstrumentSlot::EStaticAnalysis.dim());
      assert_eq!(EAstInstrument::default().output_dim(), InstrumentSlot::EAst.dim());
      assert_eq!(ECfgInstrument::default().output_dim(), InstrumentSlot::ECfg.dim());
      assert_eq!(EDataFlowInstrument::default().output_dim(), InstrumentSlot::EDataFlow.dim());
      assert_eq!(ETypeGraphInstrument::default().output_dim(), InstrumentSlot::ETypeGraph.dim());
      assert_eq!(EDiffInstrument::default().output_dim(), InstrumentSlot::EDiff.dim());
      assert_eq!(ETestInstrument::default().output_dim(), InstrumentSlot::ETest.dim());
      assert_eq!(EProblemInstrument::default().output_dim(), InstrumentSlot::EProblem.dim());
      assert_eq!(ECommitMsgInstrument::default().output_dim(), InstrumentSlot::ECommitMsg.dim());
      assert_eq!(ETraceInstrument::default().output_dim(), InstrumentSlot::ETrace.dim());
      assert_eq!(ERuntimeInstrument::default().output_dim(), InstrumentSlot::ERuntime.dim());
      assert_eq!(EReasoningInstrument::default().output_dim(), InstrumentSlot::EReasoning.dim());
      assert_eq!(ScalarsInstrument::default().output_dim(), InstrumentSlot::Scalars.dim());
  }

  #[test]
  fn error_taxonomy_codes() {
      let e1 = InstrumentError::invalid("f", "m", "r");
      assert_eq!(e1.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
      let e2 = InstrumentError::invariant("f", "m", "r");
      assert_eq!(e2.code(), "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT");
  }

  #[test]
  fn error_display_contains_field_message_remediation() {
      let e = InstrumentError::invalid("foo", "bar", "baz");
      let s = format!("{e}");
      assert!(s.contains("foo"));
      assert!(s.contains("bar"));
      assert!(s.contains("baz"));
  }
</pseudo_code>

<files_to_create>
  <!-- none --&gt;
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/tests.rs">Add `instrument_trait_contract_output_dim_matches_slot_dim`, `error_taxonomy_codes`, `error_display_contains_field_message_remediation` tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>All shipped Instrument implementors satisfy output_dim() == slot().dim()</criterion>
  <criterion>Error code strings match constitution failure_taxonomy</criterion>
  <criterion>Display impl includes field, message, remediation</criterion>
  <criterion>cargo test passes for the three new tests</criterion>
  <criterion>No public-API change to InstrumentError or Instrument trait</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments instrument_trait_contract_output_dim_matches_slot_dim error_taxonomy_codes error_display_contains_field_message_remediation</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
