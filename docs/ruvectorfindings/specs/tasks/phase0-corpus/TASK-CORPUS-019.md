# TASK-CORPUS-019 — extract_exception_class Regex Helper

```xml
<task_spec id="TASK-CORPUS-019" version="1.0">
<metadata>
  <title>extract_exception_class Regex Helper</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `extract_exception_class(text) -> Option<ExceptionClass>` in `swebench.rs:525` — regex over the 10 ExceptionClass slugs the `EOracleInstrument` reports (per the doc-09 §3.5.5 panel slot mapping). Exercised by both `swebench_report_to_verdict` and the FSV.
  </implementation_state>
  <layer>logic</layer>
  <sequence>19</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-04</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Logic task. A pure helper that scans a SWE-bench `test_output.txt`
tail for the first canonical Python exception class name and
returns the matching `ExceptionClass`. Per BR-CORPUS-02, when a
specific class is found it is preferred over the fallback `Other`
in `swebench_report_to_verdict`.
</context>

<input_context_files>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (DockerOracle::extract_exception_class)</file>
  <file purpose="exception_class_variants">../../technical/TECH-CORPUS.md#data_models (ExceptionClass)</file>
  <file purpose="business_rule">../../functional/SPEC-CORPUS.md (BR-CORPUS-02)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/swebench.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-003 complete (ExceptionClass re-exported)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub(crate) fn extract_exception_class(test_output:
      &amp;str) -&gt; Option&lt;ExceptionClass&gt;` per
      TECH-CORPUS contract.
    - Recognize the 10 canonical Python exception classes the
      `ExceptionClass` enum exposes: AssertionError, TypeError,
      ValueError, KeyError, IndexError, AttributeError,
      NameError, ImportError, RuntimeError, SyntaxError. Plus
      `Other` as the fallback.
    - First match wins (scan from the BEGINNING of the input —
      pytest tracebacks emit the type as the first non-traceback
      line; some pytest variants emit at end. Document and pick
      the first-from-start strategy for determinism).
    - Match against the line-start pattern
      `(?m)^(?:E\\s+)?<ClassName>:` (pytest output prefixes
      assertion errors with `E   `; bare runtime errors don't).
      Implement WITHOUT the `regex` crate by scanning lines and
      checking suffix membership in the canonical class set.
    - Return None when no canonical class is found
      (caller falls back to `Other` per BR-CORPUS-02).
    - Add unit tests for: each of the 10 classes
      individually; pytest assertion error format
      (`E   AssertionError: ...`); plain traceback format
      (`AssertionError: ...`); empty input returns None;
      unknown class name (`MyCustomError`) returns None.
  </in_scope>
  <out_of_scope>
    - Adding new ExceptionClass variants — that's an
      `context-graph-mejepa-instruments` change.
    - Multi-line traceback parsing.
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/swebench.rs">
    pub(crate) fn extract_exception_class(test_output: &amp;str) -&gt; Option&lt;crate::ExceptionClass&gt;;
    </signature>
  </signatures>

  <constraints>
    - No `regex` crate dependency; hand-rolled line scan.
    - Only canonical classes returned; `Other` is for the caller
      to apply, not this helper.
    - Pure function: O(n) over the test_output string;
      no allocation per match attempt.
    - Determinism: same input → same output, byte-equal.
    - The 10 class string literals defined in a `const`
      `&[(&str, ExceptionClass); 10]` so the table is grep-able.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- extract_exception_class` passes (≥ 12 tests).
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
const TABLE: &amp;[(&amp;str, ExceptionClass)] = &amp;[
    ("AssertionError", ExceptionClass::AssertionError),
    ("TypeError", ExceptionClass::TypeError),
    ("ValueError", ExceptionClass::ValueError),
    ("KeyError", ExceptionClass::KeyError),
    ("IndexError", ExceptionClass::IndexError),
    ("AttributeError", ExceptionClass::AttributeError),
    ("NameError", ExceptionClass::NameError),
    ("ImportError", ExceptionClass::ImportError),
    ("RuntimeError", ExceptionClass::RuntimeError),
    ("SyntaxError", ExceptionClass::SyntaxError),
];

pub(crate) fn extract_exception_class(test_output: &amp;str) -&gt; Option&lt;ExceptionClass&gt; {
    for line in test_output.lines() {
        let trimmed = line.trim_start_matches("E ").trim_start();
        for (name, cls) in TABLE {
            if trimmed.starts_with(name) {
                let after = &amp;trimmed[name.len()..];
                if after.starts_with(':') || after.is_empty() { return Some(*cls); }
            }
        }
    }
    None
}

#[test]
fn extract_pytest_assertion_error_format() {
    let s = "test foo failed\nE   AssertionError: 1 != 2\n";
    assert_eq!(extract_exception_class(s), Some(ExceptionClass::AssertionError));
}

#[test]
fn extract_returns_none_for_unknown_class() {
    let s = "MyCustomError: nope\n";
    assert_eq!(extract_exception_class(s), None);
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/swebench.rs">Add extract_exception_class + TABLE const.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 12 unit tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 10 canonical classes match in both pytest `E` format and plain format</criterion>
  <criterion>Empty input returns None</criterion>
  <criterion>Unknown class name returns None</criterion>
  <criterion>First match wins (deterministic)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- extract_exception_class</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
