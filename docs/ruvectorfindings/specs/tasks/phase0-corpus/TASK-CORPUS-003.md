# TASK-CORPUS-003 — OracleVerdict Re-Export from Instruments

```xml
<task_spec id="TASK-CORPUS-003" version="1.0">
<metadata>
  <title>OracleVerdict / ExceptionClass / PerTestOutcome / TestOutcome Re-Export</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-04</requirement_ref>
    <requirement_ref>REQ-CORPUS-08</requirement_ref>
  </implements>
  <depends_on></depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Foundation task. The corpus crate is downstream of
`context-graph-mejepa-instruments` (per TECH-CORPUS
`<implementation_notes category="integration">`). Verdict types
are owned by instruments; corpus re-exports them so consumers
(Phase 3 trainer, verify CLI) have one stable import path.
</context>

<input_context_files>
  <file purpose="data_model">../../technical/TECH-CORPUS.md#data_models (OracleVerdict, PerTestOutcome, ExceptionClass)</file>
  <file purpose="constitution_integration_rule">../../technical/TECH-CORPUS.md#implementation_notes (integration)</file>
  <file purpose="instruments_crate">crates/context-graph-mejepa-instruments/src/lib.rs</file>
  <file purpose="existing_corpus_lib">crates/context-graph-mejepa-corpus/src/lib.rs</file>
</input_context_files>

<prerequisites>
  <check>context-graph-mejepa-instruments declares pub OracleVerdict, PerTestOutcome, TestOutcome, ExceptionClass</check>
  <check>cargo build -p context-graph-mejepa-instruments succeeds</check>
</prerequisites>

<scope>
  <in_scope>
    - Re-export `OracleVerdict`, `PerTestOutcome`, `TestOutcome`,
      `ExceptionClass` from `context_graph_mejepa_instruments` at the
      crate root of `context_graph_mejepa_corpus`.
    - Add a doc comment on the re-exports explaining the ownership
      contract: types are defined upstream; corpus MUST NOT redefine.
    - Add a unit test that constructs an `OracleVerdict` via the
      re-export path and asserts JSON round-trip stability
      (serialize then deserialize equals the original).
  </in_scope>
  <out_of_scope>
    - Any modification to the instruments crate (it owns the types).
    - The actual oracle invoker (TASK-CORPUS-020).
    - The verdict mapper (TASK-CORPUS-018).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/lib.rs">
    pub use context_graph_mejepa_instruments::{
        ExceptionClass, OracleVerdict, PerTestOutcome, TestOutcome,
    };
    </signature>
  </signatures>

  <constraints>
    - Re-exports use `pub use ... ::{...}` (single statement).
    - NO redefinition of the four types in the corpus crate.
    - No version-bumping of `context-graph-mejepa-instruments` is
      part of this task; if upstream types are missing, raise a
      blocker per CLAUDE.md §6 instead.
    - Doc comment cites
      `<implementation_notes category="integration">` of TECH-CORPUS.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-corpus` succeeds.
    - `cargo test -p context-graph-mejepa-corpus -- oracle_verdict_reexport_round_trip` passes.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
// lib.rs (top of file, after `use` block):

/// Re-export of the canonical verdict types defined by
/// `context-graph-mejepa-instruments`. Per
/// `docs/ruvectorfindings/specs/technical/TECH-CORPUS.md#implementation_notes`,
/// the corpus crate MUST NOT redefine these types. Schema changes are
/// coordinated with the upstream instruments crate.
pub use context_graph_mejepa_instruments::{
    ExceptionClass, OracleVerdict, PerTestOutcome, TestOutcome,
};

#[test]
fn oracle_verdict_reexport_round_trip() {
    let v = OracleVerdict { per_test: vec![], exception: None,
                            resolved: true, runtime_seconds: 1.0 };
    let s = serde_json::to_string(&amp;v).unwrap();
    let back: OracleVerdict = serde_json::from_str(&amp;s).unwrap();
    assert_eq!(v, back);
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/lib.rs">Add pub use re-export block + doc comment.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add round-trip test.</file>
</files_to_modify>

<validation_criteria>
  <criterion>Four types re-exported via pub use at crate root</criterion>
  <criterion>JSON round-trip test passes</criterion>
  <criterion>No redefinition of the types in corpus crate (grep check)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- oracle_verdict_reexport_round_trip</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
