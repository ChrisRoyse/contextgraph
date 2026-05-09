# TASK-CORPUS-013 — SubtleFlip + OffByOne Mutation Operators

```xml
<task_spec id="TASK-CORPUS-013" version="1.0">
<metadata>
  <title>SubtleFlip + OffByOne Mutation Operators (Python)</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `subtle_flip.rs` (235 LOC — invert `==`/`!=`, `<`/`>`, `<=`/`>=`, `and`/`or`, `is`/`is not`; skips Python `->` arrow, `<<`/`>>`, and float-fraction `.5` literals — real bug found by FSV in commit 828a1133) and `off_by_one.rs` (120 LOC — adds ±1 to one numeric literal; integer-only, ignores fractional parts after the `0.5` regression in commit `828a1133`). python_lex mask wired so neither operator picks targets inside string literals or comments. Tests + FSV pass.
  </implementation_state>
  <layer>logic</layer>
  <sequence>13</sequence>
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
Logic task. SubtleFlip inverts one boolean operator (==↔!=, &lt;↔&gt;,
and↔or, &lt;=↔&gt;=); OffByOne adds ±1 to one numeric literal. Both
operators consume the Python lexical mask and the SplitMix64 PRNG
to pick deterministic candidate sites. Both ALREADY shipped; the
2026-05-09 FSV caught real bugs (SubtleFlip picking `>` of `->`;
OffByOne extracting fractional part of floats) that are now
guarded by EC-CORPUS-02 / EC-CORPUS-03 unit tests. This task
verifies the shipped state and locks regression tests.
</context>

<input_context_files>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (MutationOperator)</file>
  <file purpose="edge_cases">../../functional/SPEC-CORPUS.md (EC-CORPUS-01, EC-CORPUS-02, EC-CORPUS-03)</file>
  <file purpose="test_plan">../../functional/SPEC-CORPUS.md (TC-CORPUS-02, TC-CORPUS-03)</file>
  <file purpose="existing_subtle_flip">crates/context-graph-mejepa-corpus/src/subtle_flip.rs</file>
  <file purpose="existing_off_by_one">crates/context-graph-mejepa-corpus/src/off_by_one.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-004 complete (MutationOutcome / MutationSite exist)</check>
  <check>TASK-CORPUS-010 complete (mask_python + ensure_parseable_python exist)</check>
  <check>TASK-CORPUS-011 complete (pick_index_or_fail exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify `subtle_flip::apply(primary_source: &amp;str, seed:
      u64) -&gt; MutationResult&lt;MutationSite&gt;` returns ONE
      site whose original_text is one of the boolean operators
      and whose replacement_text is the inverted form.
      - Pairs: `==`↔`!=`, `&lt;`↔`&gt;`, `&lt;=`↔`&gt;=`,
        `and`↔`or`, `is`↔`is not`, `in`↔`not in`.
    - Verify `off_by_one::apply(primary_source: &amp;str, seed:
      u64) -&gt; MutationResult&lt;MutationSite&gt;` returns ONE
      site whose original_text is an integer literal and whose
      replacement_text is `original ± 1`. Float literals are
      EXCLUDED (EC-CORPUS-03).
    - SubtleFlip: do NOT classify `-&gt;` as a `&gt;` site
      (EC-CORPUS-02). Only treat tokens that the tree-sitter
      grammar reports as `comparison_operator` /
      `boolean_operator` / `is_operator` / `in_operator` as
      candidates.
    - OffByOne: parse only integer numeric literals
      (`integer` node-kind in tree-sitter-python). Float literal
      `3.14` is `float` node-kind — never extract `3` or `14`.
    - Both operators MUST refuse parser-broken input via
      `ensure_parseable_python`.
    - Both operators MUST return `MEJEPA_CORPUS_NO_MUTATION_SITE`
      when zero candidates exist after applying the mask
      (EC-CORPUS-01).
    - Add regression tests covering the FSV-discovered bugs:
      `def f(x: int) -&gt; int:` MUST NOT match SubtleFlip;
      `pi = 3.14` MUST NOT match OffByOne.
  </in_scope>
  <out_of_scope>
    - The 4 other operators (TASK-CORPUS-014..015).
    - apply_mutation dispatcher (TASK-CORPUS-016).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/subtle_flip.rs">
    pub(crate) fn apply(primary_source: &amp;str, seed: u64) -&gt; crate::MutationResult&lt;crate::MutationSite&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-corpus/src/off_by_one.rs">
    pub(crate) fn apply(primary_source: &amp;str, seed: u64) -&gt; crate::MutationResult&lt;crate::MutationSite&gt;;
    </signature>
  </signatures>

  <constraints>
    - Operators return MutationSite directly; the dispatcher
      (TASK-CORPUS-016) wraps it into MutationOutcome.
    - Site selection is deterministic for fixed seed; same
      candidate list always picks the same index via
      `pick_index_or_fail(seed, n)`.
    - `original_text` MUST equal `source[byte_offset..byte_offset
      + byte_length]` byte-for-byte; mismatch is operator bug
      (`MEJEPA_CORPUS_OPERATOR_FAILED`, validated in TASK-CORPUS-016).
    - `note` field carries human-readable explanation, e.g.
      "subtle_flip swapped `==` for `!=`".
    - Operators use tree-sitter's syntax classes
      (`comparison_operator`, `boolean_operator`, `integer`,
      `float`) as the candidate predicate — NOT regex on raw
      bytes. This is what makes `-&gt;` and float literals safe.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- subtle_flip` passes (≥ 4 tests).
    - `cargo test -p context-graph-mejepa-corpus -- off_by_one` passes (≥ 4 tests).
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
// subtle_flip.rs
pub(crate) fn apply(src: &amp;str, seed: u64) -&gt; MutationResult&lt;MutationSite&gt; {
    crate::python_lex::ensure_parseable_python(src, "primary_source")?;
    let mask = crate::python_lex::mask_python(src);
    let candidates = walk_tree_for_boolean_operators(src, &amp;mask);
    let i = crate::prng::pick_index_or_fail(seed, candidates.len())?;
    let cand = &amp;candidates[i];
    let replacement = invert_operator(&amp;cand.original_text);
    Ok(MutationSite { byte_offset: cand.start, byte_length: cand.len,
                      original_text: cand.original_text.clone(),
                      replacement_text: replacement,
                      note: format!("subtle_flip swapped `{}` for `{}`", cand.original_text, replacement) })
}

#[test]
fn subtle_flip_does_not_match_arrow_in_function_signature() {
    let src = "def f(x: int) -&gt; int:\n    pass\n";
    assert!(matches!(apply(src, 0), Err(e) if e.code() == "MEJEPA_CORPUS_NO_MUTATION_SITE"));
}

#[test]
fn off_by_one_does_not_extract_float_fractional_part() {
    let src = "pi = 3.14\nx = 0\n"; // x = 0 is the only integer
    let site = apply(src, 0).unwrap();
    assert_eq!(site.original_text, "0");
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/subtle_flip.rs">Verify shipped; add EC-CORPUS-02 regression test.</file>
  <file path="crates/context-graph-mejepa-corpus/src/off_by_one.rs">Verify shipped; add EC-CORPUS-03 regression test.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 8 regression tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>SubtleFlip pairs verified for all 6 documented operator dyads</criterion>
  <criterion>SubtleFlip rejects `->` as `>` candidate (EC-CORPUS-02)</criterion>
  <criterion>OffByOne uses integer node-kind only; rejects float fractional parts (EC-CORPUS-03)</criterion>
  <criterion>Both operators return MEJEPA_CORPUS_NO_MUTATION_SITE on empty candidate set (EC-CORPUS-01)</criterion>
  <criterion>Both operators reject parser-broken input</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- subtle_flip</command>
  <command>cargo test -p context-graph-mejepa-corpus -- off_by_one</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
