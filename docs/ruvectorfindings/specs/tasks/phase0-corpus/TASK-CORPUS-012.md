# TASK-CORPUS-012 — KnownGood + WrongFile Mutation Operators

```xml
<task_spec id="TASK-CORPUS-012" version="1.0">
<metadata>
  <title>KnownGood + WrongFile Mutation Operators (Python)</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `known_good.rs` (13 LOC — echo input + ignore seed) and `wrong_file.rs` (40 LOC — replace primary_source with `alternate_source` after `ensure_parseable_python` gate; `WrongFile` is the only category that requires the `MutationConfig::alternate_source` field per `apply_mutation` in `lib.rs:252-262`). Tests in `tests.rs`; FSV at `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/`.
  </implementation_state>
  <layer>logic</layer>
  <sequence>12</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-02</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-004</task_ref>
    <task_ref>TASK-CORPUS-010</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Logic task. The two zero-mutation-site operators per doc 09 §3.1:
KnownGood echoes input unchanged (PASS expected); WrongFile
applies the canonical patch's text to a structurally-different
base file (FAIL expected). Already shipped — this task verifies
the shipped state and locks the contract under the dispatcher
(TASK-CORPUS-016).
</context>

<input_context_files>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (MutationOperator)</file>
  <file purpose="story">../../functional/SPEC-CORPUS.md (US-CORPUS-01, EC-CORPUS-08)</file>
  <file purpose="existing_known_good">crates/context-graph-mejepa-corpus/src/known_good.rs</file>
  <file purpose="existing_wrong_file">crates/context-graph-mejepa-corpus/src/wrong_file.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-004 complete (MutationConfig / MutationOutcome / MutationSite types exist)</check>
  <check>TASK-CORPUS-010 complete (ensure_parseable_python exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify `known_good::apply(primary_source: &amp;str, _seed:
      u64) -&gt; MutationResult&lt;MutationOutcome&gt;` returns
      `MutationOutcome { category: KnownGood, mutated_source =
      primary_source.to_string(), seed, mutation_site: None }`.
    - Verify the input still passes
      `ensure_parseable_python(primary_source, "primary_source")`
      so KnownGood never emits broken source.
    - Verify `wrong_file::apply(_primary_source: &amp;str,
      alternate_source: &amp;str, _seed: u64) -&gt;
      MutationResult&lt;MutationOutcome&gt;` returns
      `MutationOutcome { category: WrongFile, mutated_source =
      alternate_source.to_string(), seed, mutation_site: None }`.
    - Verify EC-CORPUS-08: the dispatcher (TASK-CORPUS-016) is
      what enforces `alternate_source.is_some()` for WrongFile;
      this task only confirms the operator's surface signature
      treats `alternate_source` as a required `&amp;str` so the
      None-case CANNOT compile.
    - Add unit tests: KnownGood byte-equal to input (1 case),
      KnownGood rejects parser-broken input (1 case), WrongFile
      mutated_source equals alternate_source (1 case), WrongFile
      mutation_site is None (1 case).
  </in_scope>
  <out_of_scope>
    - apply_mutation dispatcher (TASK-CORPUS-016).
    - The other 6 operators (TASK-CORPUS-013..015).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/known_good.rs">
    pub(crate) fn apply(primary_source: &amp;str, seed: u64) -&gt; crate::MutationResult&lt;crate::MutationOutcome&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-corpus/src/wrong_file.rs">
    pub(crate) fn apply(primary_source: &amp;str, alternate_source: &amp;str, seed: u64) -&gt; crate::MutationResult&lt;crate::MutationOutcome&gt;;
    </signature>
  </signatures>

  <constraints>
    - KnownGood MUST validate input via `ensure_parseable_python`
      before echoing — so a parser-broken KnownGood request
      surfaces as `MEJEPA_CORPUS_INVALID_INPUT`, not silently
      passes through.
    - WrongFile validates BOTH primary_source AND alternate_source
      parse cleanly — broken alternate is `MEJEPA_CORPUS_INVALID_INPUT`.
    - Neither operator emits a `MutationSite`; the field is
      always `None`.
    - The seed is echoed into MutationOutcome.seed so determinism
      audits can confirm the seed plumbing is intact.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- known_good` passes (≥ 2 tests).
    - `cargo test -p context-graph-mejepa-corpus -- wrong_file` passes (≥ 2 tests).
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
// known_good.rs
pub(crate) fn apply(primary_source: &amp;str, seed: u64) -&gt; MutationResult&lt;MutationOutcome&gt; {
    crate::python_lex::ensure_parseable_python(primary_source, "primary_source")?;
    Ok(MutationOutcome {
        category: MutationCategory::KnownGood,
        mutated_source: primary_source.to_string(),
        seed, mutation_site: None,
    })
}

// wrong_file.rs
pub(crate) fn apply(primary_source: &amp;str, alternate_source: &amp;str, seed: u64) -&gt; MutationResult&lt;MutationOutcome&gt; {
    crate::python_lex::ensure_parseable_python(primary_source, "primary_source")?;
    crate::python_lex::ensure_parseable_python(alternate_source, "alternate_source")?;
    Ok(MutationOutcome {
        category: MutationCategory::WrongFile,
        mutated_source: alternate_source.to_string(),
        seed, mutation_site: None,
    })
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/known_good.rs">Verify shipped state.</file>
  <file path="crates/context-graph-mejepa-corpus/src/wrong_file.rs">Verify shipped state.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 4 unit tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>KnownGood byte-equal echo + None mutation_site</criterion>
  <criterion>KnownGood rejects parser-broken input</criterion>
  <criterion>WrongFile mutated_source equals alternate_source</criterion>
  <criterion>WrongFile rejects parser-broken alternate_source</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- known_good</command>
  <command>cargo test -p context-graph-mejepa-corpus -- wrong_file</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
