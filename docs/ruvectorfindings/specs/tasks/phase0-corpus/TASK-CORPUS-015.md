# TASK-CORPUS-015 — OverEngineer + CompileError Mutation Operators

```xml
<task_spec id="TASK-CORPUS-015" version="1.0">
<metadata>
  <title>OverEngineer + CompileError Mutation Operators (Python)</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `over_engineer.rs` (39 LOC — appends an unused helper function deterministically named off the seed; oracle expected to PASS) and `compile_error.rs` (44 LOC — introduces a single-character Python syntax error after the python_lex parse gate confirms the input was valid; oracle expected to FAIL at parse). Tests + FSV pass.
  </implementation_state>
  <layer>logic</layer>
  <sequence>15</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-02</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-004</task_ref>
    <task_ref>TASK-CORPUS-010</task_ref>
    <task_ref>TASK-CORPUS-011</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
Logic task. OverEngineer appends an unused helper function (PASS
expected — function is dead code, tests still pass);
CompileError introduces a Python syntax error (FAIL at parse
expected). Both already shipped; this task verifies the shipped
state and locks the contract.
</context>

<input_context_files>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (MutationOperator)</file>
  <file purpose="story">../../functional/SPEC-CORPUS.md (US-CORPUS-01)</file>
  <file purpose="existing_over_engineer">crates/context-graph-mejepa-corpus/src/over_engineer.rs</file>
  <file purpose="existing_compile_error">crates/context-graph-mejepa-corpus/src/compile_error.rs</file>
  <file purpose="util">crates/context-graph-mejepa-corpus/src/util.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-004 complete (MutationOutcome / MutationSite exist)</check>
  <check>TASK-CORPUS-010 complete (ensure_parseable_python exists)</check>
  <check>TASK-CORPUS-011 complete (pick_index_or_fail exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify `over_engineer::apply(primary_source: &amp;str,
      seed: u64) -&gt; MutationResult&lt;MutationSite&gt;`:
      finds a safe end-of-file insertion offset
      (after last newline; via util::append_insertion_offset),
      inserts a unique unused helper function whose name is
      derived from the seed (e.g.,
      `def _mejepa_unused_helper_{seed_hash}(): pass\n`).
      Returns site with byte_length = 0, original_text = "",
      replacement_text = the helper text.
    - Verify the inserted helper is parseable when concatenated:
      `ensure_parseable_python(primary_source +
      replacement_text, "post-mutation")?` (assertion in tests,
      not in apply).
    - Verify `compile_error::apply(primary_source: &amp;str,
      seed: u64) -&gt; MutationResult&lt;MutationSite&gt;`:
      picks one location in the source that, when corrupted,
      makes the result fail tree-sitter (e.g., delete one
      closing paren `)`, replace `:` with `~`, etc.).
      Returns site whose original_text is the corrupted slice
      and replacement_text is the broken substitute.
    - CompileError MUST validate the input parses cleanly first
      (so we know we're TURNING parseable input INTO unparseable
      output, never doing the reverse silently).
    - CompileError's output, when concatenated, MUST FAIL
      `ensure_parseable_python` (assertion in tests).
    - Add unit tests:
      - OverEngineer is deterministic for fixed seed; the
        emitted helper name varies with seed; mutated_source
        parses cleanly.
      - CompileError mutated_source FAILS to parse.
      - Both operators reject parser-broken input.
      - OverEngineer + util::append_insertion_offset interaction:
        empty-source returns offset 0; trailing-newline source
        returns source.len(); no-trailing-newline source has the
        operator emit a leading newline before the helper.
  </in_scope>
  <out_of_scope>
    - apply_mutation dispatcher (TASK-CORPUS-016).
    - The other 6 operators (TASK-CORPUS-012..014).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/over_engineer.rs">
    pub(crate) fn apply(primary_source: &amp;str, seed: u64) -&gt; crate::MutationResult&lt;crate::MutationSite&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-corpus/src/compile_error.rs">
    pub(crate) fn apply(primary_source: &amp;str, seed: u64) -&gt; crate::MutationResult&lt;crate::MutationSite&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-corpus/src/util.rs">
    pub(crate) fn append_insertion_offset(source: &amp;str) -&gt; usize;
    </signature>
  </signatures>

  <constraints>
    - OverEngineer's helper name is deterministic from seed —
      e.g., `format!("_mejepa_unused_helper_{:016x}", seed)`.
    - OverEngineer emits a leading newline if source does not
      end with one, so the helper starts on its own line.
    - CompileError's broken substitution MUST be syntactically
      relevant (delete `:`, `)`, `]`, `}`, `=` — NOT random
      chars). The lexer mask still applies (no breaking inside
      strings) so `#"`, `"#`, etc. don't get touched.
    - Both operators emit a `note` field with a human-readable
      description of the mutation.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- over_engineer` passes (≥ 3 tests).
    - `cargo test -p context-graph-mejepa-corpus -- compile_error` passes (≥ 3 tests).
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
// over_engineer.rs
pub(crate) fn apply(src: &amp;str, seed: u64) -&gt; MutationResult&lt;MutationSite&gt; {
    crate::python_lex::ensure_parseable_python(src, "primary_source")?;
    let off = crate::util::append_insertion_offset(src);
    let prefix = if src.ends_with('\n') { "" } else { "\n" };
    let helper = format!("{}def _mejepa_unused_helper_{:016x}(): pass\n", prefix, seed);
    Ok(MutationSite { byte_offset: off, byte_length: 0,
                      original_text: String::new(),
                      replacement_text: helper.clone(),
                      note: format!("over_engineer appended unused helper (seed={:016x})", seed) })
}

#[test]
fn over_engineer_helper_text_parses_clean() {
    let src = "def f(): return 1\n";
    let site = apply(src, 42).unwrap();
    let post = format!("{}{}", src, site.replacement_text);
    assert!(crate::python_lex::ensure_parseable_python(&amp;post, "post").is_ok());
}

#[test]
fn compile_error_output_fails_to_parse() {
    let src = "def f(): return 1\n";
    let site = apply_compile_error(src, 0).unwrap();
    let post = src[..site.byte_offset].to_string()
             + &amp;site.replacement_text + &amp;src[site.byte_offset + site.byte_length..];
    assert!(crate::python_lex::ensure_parseable_python(&amp;post, "post").is_err());
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/over_engineer.rs">Verify shipped state.</file>
  <file path="crates/context-graph-mejepa-corpus/src/compile_error.rs">Verify shipped state.</file>
  <file path="crates/context-graph-mejepa-corpus/src/util.rs">Verify append_insertion_offset.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 6 unit tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>OverEngineer's mutated_source still parses cleanly (helper is valid Python)</criterion>
  <criterion>CompileError's mutated_source fails to parse (root.has_error() == true)</criterion>
  <criterion>OverEngineer is deterministic for fixed seed; helper name varies with seed</criterion>
  <criterion>append_insertion_offset returns 0 for empty, source.len() for trailing-newline, source.len() for non-trailing (operator prepends newline)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- over_engineer</command>
  <command>cargo test -p context-graph-mejepa-corpus -- compile_error</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
