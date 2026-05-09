# TASK-INSTR-007 — EStaticAnalysisInstrument verify + edge-case audit

```xml
<task_spec id="TASK-INSTR-007" version="1.0">

<metadata>
  <title>EStaticAnalysisInstrument verify (shipped) + edge-case audit (empty diagnostics, max-cap, churn None)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-INSTR-12">EStaticAnalysisInstrument 256-d</item>
    <item ref="component:EStaticAnalysisInstrument">encode(StaticAnalysisInput) -> 256 f32</item>
    <item ref="data_model:Diagnostic">tool, severity, category, line, column</item>
    <item ref="data_model:DiagnosticSeverity">Error, Warning, Info, Hint</item>
    <item ref="data_model:StaticAnalysisInput">source_text, diagnostics, churn_30d</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file modified, 0 created</estimated_files>
</metadata>

<context>
`EStaticAnalysisInstrument` is shipped at `e_static_analysis.rs`. Per doc 09 §3.5.1, it encodes
linter warnings, type completeness, complexity, BFS depth, blame age, symbol churn, docstring
coverage. Constant: `MAX_DIAGNOSTICS = 100_000`. This task verifies signatures + adds edge-case
tests for empty source, empty diagnostics, max-cap, churn None vs Some.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-12)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts EStaticAnalysisInstrument)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_static_analysis.rs</file>
  <file purpose="design_doc">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.5.1</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003 done</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped signatures
    - Verify shipped 256-d output (severity histo, tool histo bins, category histo bins, source-shape features)
    - Add test: empty source_text + empty diagnostics → finite 256-d output
    - Add test: 100_001 diagnostics rejected with MEJEPA_INSTRUMENTS_INVALID_INPUT
    - Add test: 100_000 diagnostics accepted
    - Add test: churn_30d == None vs Some(0) vs Some(1_000_000) all encode finitely
    - Add test: 4 severity counts ratio sum normalization (if shipped behavior normalizes)
    - Add test: deterministic encode (byte-equal across runs)
    - Add test: same diagnostic appearing twice produces different histogram than appearing once (not silently deduplicated)
  </in_scope>
  <out_of_scope>
    - Real toolchain invocation (Pyright/clippy/eslint) — that's TASK-INSTR-011 for E_TypeGraph and a separate Phase 1 extension for richer diagnostics
    - Per-language diagnostic schema (caller's responsibility)
    - linear-probe gate (TASK-INSTR-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_static_analysis.rs">
const MAX_DIAGNOSTICS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiagnosticSeverity { Error, Warning, Info, Hint }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub tool: String,
    pub severity: DiagnosticSeverity,
    pub category: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StaticAnalysisInput {
    pub source_text: String,
    pub diagnostics: Vec&lt;Diagnostic&gt;,
    pub churn_30d: Option&lt;u32&gt;,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EStaticAnalysisInstrument;

impl Instrument for EStaticAnalysisInstrument {
    type Input = StaticAnalysisInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EStaticAnalysis }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;;  // 256 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 256 finite f32
    - MAX_DIAGNOSTICS = 100_000 enforced
    - Churn None and Some both encode finitely
    - Hash-bin operations seeded deterministically (no rand::thread_rng or random SipHash key)
    - Encoder is deterministic
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_static_analysis:: passes >= 7 tests
  </verification>
</definition_of_done>

<pseudo_code>
add to e_static_analysis.rs tests:

  fn diag(tool: &amp;str, sev: DiagnosticSeverity, cat: &amp;str, l: u32, c: u32) -&gt; Diagnostic {
      Diagnostic { tool: tool.into(), severity: sev, category: cat.into(), line: l, column: c }
  }

  #[test]
  fn empty_source_empty_diagnostics_finite() {
      let input = StaticAnalysisInput { source_text: String::new(), diagnostics: vec![], churn_30d: None };
      let out = EStaticAnalysisInstrument.encode(&amp;input).unwrap();
      assert_eq!(out.len(), 256);
      for v in &amp;out { assert!(v.is_finite()); }
  }

  #[test]
  fn max_diagnostics_boundary() {
      let mut input = StaticAnalysisInput { source_text: "x".into(), diagnostics: vec![], churn_30d: Some(0) };
      input.diagnostics = (0..MAX_DIAGNOSTICS).map(|i| diag("t", DiagnosticSeverity::Warning, "cat", 1, i as u32)).collect();
      assert!(EStaticAnalysisInstrument.encode(&amp;input).is_ok());
      input.diagnostics.push(diag("t", DiagnosticSeverity::Warning, "cat", 1, 0));
      let err = EStaticAnalysisInstrument.encode(&amp;input).unwrap_err();
      assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
  }

  #[test]
  fn churn_none_vs_some_distinct_encoding() {
      let base = StaticAnalysisInput { source_text: "x".into(), diagnostics: vec![], churn_30d: None };
      let with_churn = StaticAnalysisInput { churn_30d: Some(50), ..base.clone() };
      let a = EStaticAnalysisInstrument.encode(&amp;base).unwrap();
      let b = EStaticAnalysisInstrument.encode(&amp;with_churn).unwrap();
      assert_ne!(a, b, "churn should affect output");
  }

  #[test]
  fn deterministic_encode_byte_equal() {
      let input = StaticAnalysisInput {
          source_text: "fn main() {}".into(),
          diagnostics: vec![diag("clippy", DiagnosticSeverity::Warning, "redundant_closure", 1, 5)],
          churn_30d: Some(3),
      };
      let a = EStaticAnalysisInstrument.encode(&amp;input).unwrap();
      let b = EStaticAnalysisInstrument.encode(&amp;input).unwrap();
      assert_eq!(a, b);
  }

  #[test]
  fn duplicate_diagnostics_compound() {
      let d = diag("clippy", DiagnosticSeverity::Warning, "x", 1, 1);
      let single = StaticAnalysisInput { source_text: "x".into(), diagnostics: vec![d.clone()], churn_30d: None };
      let double = StaticAnalysisInput { source_text: "x".into(), diagnostics: vec![d.clone(), d], churn_30d: None };
      let a = EStaticAnalysisInstrument.encode(&amp;single).unwrap();
      let b = EStaticAnalysisInstrument.encode(&amp;double).unwrap();
      assert_ne!(a, b, "duplicate diagnostics must affect histogram (no silent dedup)");
  }
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_static_analysis.rs">Add 5 edge-case tests; helper diag() builder</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 256 finite f32</criterion>
  <criterion>MAX_DIAGNOSTICS enforced (100_000 ok, 100_001 rejected)</criterion>
  <criterion>Churn variants produce distinct outputs</criterion>
  <criterion>Encoder is deterministic</criterion>
  <criterion>Duplicate diagnostics compound (no silent dedup)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_static_analysis::</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
