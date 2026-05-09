# TASK-CORPUS-016 — apply_mutation Dispatcher + apply_site Validator

```xml
<task_spec id="TASK-CORPUS-016" version="1.0">
<metadata>
  <title>apply_mutation Dispatcher + apply_site Validator</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>16</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-02</requirement_ref>
    <requirement_ref>REQ-CORPUS-11</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-012</task_ref>
    <task_ref>TASK-CORPUS-013</task_ref>
    <task_ref>TASK-CORPUS-014</task_ref>
    <task_ref>TASK-CORPUS-015</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
Logic task. The library entry point that consumers call. Routes
each MutationCategory to its operator, runs the parseable gate,
validates the returned MutationSite (UTF-8 boundaries, original
text matches), and produces a `MutationOutcome`. This is the
seam where EC-CORPUS-08 (WrongFile requires alternate_source) is
enforced.
</context>

<input_context_files>
  <file purpose="api_contract">../../technical/TECH-CORPUS.md#api_contracts (apply_mutation)</file>
  <file purpose="error_taxonomy">../../technical/TECH-CORPUS.md#error_taxonomy</file>
  <file purpose="edge_cases">../../functional/SPEC-CORPUS.md (EC-CORPUS-08, US-CORPUS-07)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/lib.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-012, 013, 014, 015 complete (all 8 operators ship)</check>
  <check>TASK-CORPUS-010 complete (ensure_parseable_python exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub fn apply_mutation(category: MutationCategory,
      primary_source: &amp;str, config: MutationConfig) -&gt;
      MutationResult&lt;MutationOutcome&gt;`:
      1. `ensure_parseable_python(primary_source,
         "primary_source")?` — fail-closed.
      2. EC-CORPUS-08: reject WrongFile if
         `config.alternate_source.is_none()` with
         `MEJEPA_CORPUS_INVALID_INPUT { field: "alternate_source",
         message: "WrongFile requires alternate_source", remediation:
         "pass the path/content of a different but
         structurally-similar file" }`.
      3. Dispatch by category:
         - KnownGood     -&gt; known_good::apply(primary, seed)
         - WrongFile     -&gt; wrong_file::apply(primary, alt, seed)
         - SubtleFlip    -&gt; subtle_flip::apply(...) -&gt; site
         - OffByOne      -&gt; off_by_one::apply(...) -&gt; site
         - SwapVariable  -&gt; swap_variable::apply(...) -&gt; site
         - DeleteTestCall-&gt; delete_test_call::apply(...) -&gt; site
         - OverEngineer  -&gt; over_engineer::apply(...) -&gt; site
         - CompileError  -&gt; compile_error::apply(...) -&gt; site
      4. For non-(KnownGood|WrongFile) operators: call
         `apply_site(primary_source, &amp;site)` to validate AND
         splice the mutation into the source.
    - Implement `pub(crate) fn apply_site(source: &amp;str,
      site: &amp;MutationSite) -&gt; MutationResult&lt;String&gt;`:
      validates byte_offset + byte_length ≤ source.len() (overflow
      check), checks UTF-8 char boundaries on both ends, asserts
      `source[byte_offset..byte_offset + byte_length] ==
      original_text`. On any mismatch returns
      `MEJEPA_CORPUS_OPERATOR_FAILED` with offset, length, original.
      Returns the spliced source (`prefix + replacement +
      suffix`).
    - Add tests:
      - All 8 categories smoke-test: `apply_mutation` succeeds on
        a real Python source for every operator with at least one
        candidate site.
      - WrongFile without alternate_source returns
        InvalidInput.
      - apply_site rejects byte_length overflow.
      - apply_site rejects mid-UTF-8 byte_offset.
      - apply_site rejects original_text mismatch.
      - Determinism (NFR-CORPUS-03): two consecutive
        apply_mutation calls with same (category, source, seed)
        produce byte-equal mutated_source for ALL 8 categories.
  </in_scope>
  <out_of_scope>
    - The Language enum dispatch (Phase 1 extension; today only
      Python is wired).
    - The batch generator (TASK-CORPUS-023).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/lib.rs">
    pub fn apply_mutation(
        category: MutationCategory,
        primary_source: &amp;str,
        config: MutationConfig,
    ) -&gt; MutationResult&lt;MutationOutcome&gt;;

    pub(crate) fn apply_site(source: &amp;str, site: &amp;MutationSite) -&gt; MutationResult&lt;String&gt;;
    </signature>
  </signatures>

  <constraints>
    - All 8 categories MUST be in the `match` arm — Rust
      exhaustiveness check is what guarantees no operator is
      forgotten when a new category is added.
    - `apply_site` checks: overflow via `checked_add`; UTF-8 via
      `source.is_char_boundary(byte_offset)` and
      `source.is_char_boundary(byte_offset + byte_length)`;
      original via `&amp;source[byte_offset..byte_offset + byte_length]
      == original_text`.
    - Splice using `source.get(..off).unwrap()` only after the
      checks pass; never blanket `unwrap()` without the guard.
    - The dispatcher echoes `config.seed` into
      `MutationOutcome.seed` — determinism audit gate.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- apply_mutation` passes (≥ 14 tests).
    - `cargo test -p context-graph-mejepa-corpus -- determinism_all_categories` passes.
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn apply_mutation(category: MutationCategory, src: &amp;str, cfg: MutationConfig) -&gt; MutationResult&lt;MutationOutcome&gt; {
    crate::python_lex::ensure_parseable_python(src, "primary_source")?;
    let outcome = match category {
        MutationCategory::KnownGood =&gt; known_good::apply(src, cfg.seed)?,
        MutationCategory::WrongFile =&gt; {
            let alt = cfg.alternate_source.as_deref().ok_or_else(||
                MutationError::invalid("alternate_source",
                    "WrongFile requires alternate_source",
                    "pass the path/content of a different but structurally-similar file"))?;
            wrong_file::apply(src, alt, cfg.seed)?
        }
        MutationCategory::SubtleFlip =&gt; finalize(src, subtle_flip::apply(src, cfg.seed)?, MutationCategory::SubtleFlip, cfg.seed)?,
        MutationCategory::OffByOne =&gt; finalize(src, off_by_one::apply(src, cfg.seed)?, MutationCategory::OffByOne, cfg.seed)?,
        MutationCategory::SwapVariable =&gt; finalize(src, swap_variable::apply(src, cfg.seed)?, ..., cfg.seed)?,
        MutationCategory::DeleteTestCall =&gt; finalize(src, delete_test_call::apply(src, cfg.seed)?, ..., cfg.seed)?,
        MutationCategory::OverEngineer =&gt; finalize(src, over_engineer::apply(src, cfg.seed)?, ..., cfg.seed)?,
        MutationCategory::CompileError =&gt; finalize(src, compile_error::apply(src, cfg.seed)?, ..., cfg.seed)?,
    };
    Ok(outcome)
}

fn finalize(src: &amp;str, site: MutationSite, cat: MutationCategory, seed: u64) -&gt; MutationResult&lt;MutationOutcome&gt; {
    let mutated = apply_site(src, &amp;site)?;
    Ok(MutationOutcome { category: cat, mutated_source: mutated, seed, mutation_site: Some(site) })
}

pub(crate) fn apply_site(source: &amp;str, site: &amp;MutationSite) -&gt; MutationResult&lt;String&gt; {
    let end = site.byte_offset.checked_add(site.byte_length)
        .ok_or_else(|| MutationError::operator_failed("site", "offset+length overflow", "fix operator"))?;
    if end &gt; source.len() { return Err(...); }
    if !source.is_char_boundary(site.byte_offset) || !source.is_char_boundary(end) {
        return Err(MutationError::operator_failed("site", "site not on UTF-8 char boundary", "fix operator"));
    }
    if &amp;source[site.byte_offset..end] != site.original_text {
        return Err(MutationError::operator_failed("site", "original_text mismatch", "fix operator"));
    }
    let mut out = String::with_capacity(source.len() - site.byte_length + site.replacement_text.len());
    out.push_str(&amp;source[..site.byte_offset]);
    out.push_str(&amp;site.replacement_text);
    out.push_str(&amp;source[end..]);
    Ok(out)
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/lib.rs">Add apply_mutation public + apply_site internal.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 14 dispatcher tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 8 categories dispatched (Rust exhaustiveness)</criterion>
  <criterion>EC-CORPUS-08 (WrongFile requires alternate_source) enforced</criterion>
  <criterion>apply_site rejects overflow / mid-UTF-8 / original_text mismatch</criterion>
  <criterion>Determinism holds for all 8 categories</criterion>
  <criterion>Parser-broken input rejected up-front for all 8 categories</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- apply_mutation</command>
  <command>cargo test -p context-graph-mejepa-corpus -- apply_site</command>
  <command>cargo test -p context-graph-mejepa-corpus -- determinism_all_categories</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
