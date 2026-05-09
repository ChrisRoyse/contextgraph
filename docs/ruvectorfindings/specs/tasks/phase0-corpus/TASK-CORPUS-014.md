# TASK-CORPUS-014 — SwapVariable + DeleteTestCall Mutation Operators

```xml
<task_spec id="TASK-CORPUS-014" version="1.0">
<metadata>
  <title>SwapVariable + DeleteTestCall Mutation Operators (Python)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>14</sequence>
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
Logic task. SwapVariable renames one local variable to another
in-scope name (FAIL expected); DeleteTestCall removes one
assertion call (mixed expected). Both consume the Python AST via
tree-sitter and the SplitMix64 PRNG. Already shipped; this task
verifies the shipped state and locks regression tests for
EC-CORPUS-11 (zero local variables → NoMutationSite).
</context>

<input_context_files>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (MutationOperator)</file>
  <file purpose="edge_cases">../../functional/SPEC-CORPUS.md (EC-CORPUS-11)</file>
  <file purpose="existing_swap">crates/context-graph-mejepa-corpus/src/swap_variable.rs</file>
  <file purpose="existing_delete">crates/context-graph-mejepa-corpus/src/delete_test_call.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-004 complete (MutationOutcome / MutationSite exist)</check>
  <check>TASK-CORPUS-010 complete (mask_python + ensure_parseable_python exist)</check>
  <check>TASK-CORPUS-011 complete (pick_index_or_fail exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify `swap_variable::apply(primary_source: &amp;str, seed:
      u64) -&gt; MutationResult&lt;MutationSite&gt;` enumerates
      local-variable identifiers (declared via assignment,
      function parameter, or for-loop binder), picks one
      deterministically, and renames a single occurrence to
      another in-scope name.
    - Verify `delete_test_call::apply(primary_source: &amp;str,
      seed: u64) -&gt; MutationResult&lt;MutationSite&gt;`
      enumerates assertion-style call sites
      (`assertEqual`, `assertTrue`, `assertFalse`, `assert_*`,
      `pytest.raises(...)`, top-level `assert ...` statements)
      and replaces one with `pass` (or empty stmt) producing a
      site with `replacement_text = "pass"` or `""`.
    - Both operators MUST respect EC-CORPUS-11: a source with
      zero candidate sites returns
      `MEJEPA_CORPUS_NO_MUTATION_SITE` with the operator's name
      and the masked-region rule cited.
    - Both operators MUST refuse parser-broken input via
      `ensure_parseable_python`.
    - Add unit tests:
      - SwapVariable on `def add(a, b): return a + b` with seed 0
        produces a deterministic rename; second run with seed 0
        produces byte-equal output (NFR-CORPUS-03 determinism).
      - SwapVariable on a module-level constants file (zero local
        variables) returns NoMutationSite.
      - DeleteTestCall on a unittest fixture deletes ONE
        assertCall.
      - DeleteTestCall on a source with zero assertion-like calls
        returns NoMutationSite.
  </in_scope>
  <out_of_scope>
    - The 4 other operators (TASK-CORPUS-013, 015).
    - apply_mutation dispatcher (TASK-CORPUS-016).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/swap_variable.rs">
    pub(crate) fn apply(primary_source: &amp;str, seed: u64) -&gt; crate::MutationResult&lt;crate::MutationSite&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-corpus/src/delete_test_call.rs">
    pub(crate) fn apply(primary_source: &amp;str, seed: u64) -&gt; crate::MutationResult&lt;crate::MutationSite&gt;;
    </signature>
  </signatures>

  <constraints>
    - Variable enumeration via tree-sitter scope walk:
      `function_definition`'s `parameters` children, `for_statement`
      `left` child, `assignment`'s `left` child.
    - Rename target chosen from in-scope alternatives
      (other locals + parameters), never a Python builtin.
      If no alternative exists, return NoMutationSite.
    - DeleteTestCall preserves indentation: replacement_text is
      `"pass"` (one keyword) so the surrounding block stays
      syntactically valid.
    - Both operators emit a `note` describing what changed.
    - File length stays under 500 lines (constitution
      `<file_organization>`); split into helper modules if
      crossing.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- swap_variable` passes (≥ 3 tests).
    - `cargo test -p context-graph-mejepa-corpus -- delete_test_call` passes (≥ 3 tests).
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
// swap_variable.rs
pub(crate) fn apply(src: &amp;str, seed: u64) -&gt; MutationResult&lt;MutationSite&gt; {
    crate::python_lex::ensure_parseable_python(src, "primary_source")?;
    let scopes = collect_scopes_from_tree(src);
    let candidates: Vec&lt;_&gt; = scopes.iter().flat_map(|s| s.swap_candidates()).collect();
    let i = crate::prng::pick_index_or_fail(seed, candidates.len())?;
    let c = &amp;candidates[i];
    Ok(MutationSite { byte_offset: c.byte_offset, byte_length: c.byte_length,
                      original_text: c.original.clone(),
                      replacement_text: c.alternative.clone(),
                      note: format!("swap_variable renamed `{}` to `{}`", c.original, c.alternative) })
}

#[test]
fn swap_variable_no_locals_returns_no_site() {
    let src = "PI = 3\nE = 2\n"; // module-level only
    assert!(matches!(apply(src, 0), Err(e) if e.code() == "MEJEPA_CORPUS_NO_MUTATION_SITE"));
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/swap_variable.rs">Verify shipped state.</file>
  <file path="crates/context-graph-mejepa-corpus/src/delete_test_call.rs">Verify shipped state.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 6 unit tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>SwapVariable enumerates locals/params/for-binders</criterion>
  <criterion>SwapVariable returns NoMutationSite on zero locals (EC-CORPUS-11)</criterion>
  <criterion>DeleteTestCall replaces an assertion with `pass` preserving indentation</criterion>
  <criterion>Both operators are deterministic for fixed seed</criterion>
  <criterion>Both operators reject parser-broken input</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- swap_variable</command>
  <command>cargo test -p context-graph-mejepa-corpus -- delete_test_call</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
