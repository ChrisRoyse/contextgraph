# TASK-CORPUS-010 — python_lex Parseable Gate + LexicalMask

```xml
<task_spec id="TASK-CORPUS-010" version="1.0">
<metadata>
  <title>python_lex: ensure_parseable_python + mask_python (LexicalMask)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-11</requirement_ref>
    <requirement_ref>REQ-CORPUS-02</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-001</task_ref>
    <task_ref>TASK-CORPUS-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
Logic task. The Python lexical mask is the foundation for every
mutation operator: it identifies byte ranges that are NOT
mutation sites (comments, ordinary strings, triple-quoted strings,
prefixed strings, f-string literal text). F-string replacement
fields ARE exposed as code per Python's expression semantics.
Already shipped — this task verifies the shipped state, adds any
missing FSV-discovered edge cases (`->` arrow, float literal
fractional parts), and locks the contract.
</context>

<input_context_files>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (python_lex)</file>
  <file purpose="edge_cases">../../functional/SPEC-CORPUS.md (EC-CORPUS-01, EC-CORPUS-02, EC-CORPUS-03)</file>
  <file purpose="failure_taxonomy">../../constitution.md#failure_taxonomy (MEJEPA_CHUNKER_PARSE_FAILED, MEJEPA_CORPUS_INVALID_INPUT)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/python_lex.rs</file>
</input_context_files>

<prerequisites>
  <check>tree-sitter + tree-sitter-python in Cargo.toml</check>
  <check>TASK-CORPUS-001 complete (MutationError exists)</check>
  <check>TASK-CORPUS-004 complete (MutationConfig exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped `ensure_parseable_python(source: &amp;str,
      field: &amp;'static str) -&gt; MutationResult&lt;()&gt;`. If
      `root.has_error()`, walk to first ERROR/MISSING node and
      return `MEJEPA_CORPUS_INVALID_INPUT` with byte offset, line,
      and node-kind in the message.
    - Verify shipped `mask_python(source: &amp;str) -&gt;
      LexicalMask` with `is_code(byte_offset)` and
      `is_code_range(start, end)`.
    - Verify mask handles: comments (`#...`), ordinary strings
      (`"x"`, `'x'`), triple-quoted strings (`"""x"""`, `'''x'''`),
      prefixed strings (`r"x"`, `b"x"`, `rb"x"`, `f"x"`),
      f-string literal text (`f"hello {name}"` -&gt; `hello ` and
      ` ` masked, `name` exposed as code).
    - ADD or verify: `->` is treated as a single token boundary
      (EC-CORPUS-02) — neither `-` nor `>` is a candidate site
      when adjacent in this pair.
    - ADD or verify: float literal `3.14` is a single numeric
      literal — fractional part NOT extracted (EC-CORPUS-03).
    - Add per-edge-case unit tests (EC-CORPUS-01..03) if not
      already shipped.
    - Add `LexicalMask::iter_code_chars(source: &amp;str) -&gt; impl
      Iterator&lt;Item=(usize, char)&gt;` helper that operators
      can consume directly.
  </in_scope>
  <out_of_scope>
    - Per-language masks for the 10 deferred languages
      (`<out_of_scope>` of SPEC-CORPUS).
    - Mutation operator implementations (TASK-CORPUS-012..015).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/python_lex.rs">
    pub(crate) fn ensure_parseable_python(source: &amp;str, field: &amp;'static str) -&gt; crate::MutationResult&lt;()&gt;;
    pub(crate) fn mask_python(source: &amp;str) -&gt; LexicalMask;

    pub(crate) struct LexicalMask { /* private */ }

    impl LexicalMask {
        pub fn is_code(&amp;self, byte_offset: usize) -&gt; bool;
        pub fn is_code_range(&amp;self, start: usize, end: usize) -&gt; bool;
        pub fn iter_code_chars&lt;'a&gt;(&amp;'a self, source: &amp;'a str) -&gt; impl Iterator&lt;Item = (usize, char)&gt; + 'a;
    }
    </signature>
  </signatures>

  <constraints>
    - The function MUST use `tree-sitter-python` (no regex-only
      fallback). If parsing fails, fail-closed.
    - LexicalMask uses a sorted `Vec&lt;(usize, usize)&gt;` of
      MASKED ranges; `is_code()` runs a binary search. No
      allocation per query.
    - Mask byte ranges MUST be valid UTF-8 char boundaries.
    - Tests use real Python source (not mock); reuse fixtures
      from existing `tests.rs`.
    - File MUST stay under 500 lines (constitution
      `<file_organization>`); split into `python_lex/{mod,
      ranges, fstring}.rs` if needed.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- python_lex`
      passes (≥ 8 tests covering all 5 mask categories + EC-CORPUS-01..03).
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub(crate) fn ensure_parseable_python(source: &amp;str, field: &amp;'static str) -&gt; MutationResult&lt;()&gt; {
    let mut p = tree_sitter::Parser::new();
    p.set_language(&amp;tree_sitter_python::language()).unwrap();
    let tree = p.parse(source, None).ok_or_else(|| MutationError::invalid(field, "tree-sitter parse returned None", "supply syntactically valid Python"))?;
    if !tree.root_node().has_error() &amp;&amp; !has_missing(tree.root_node()) { return Ok(()); }
    let bad = first_error_node(tree.root_node());
    Err(MutationError::invalid(field, format!("source has parse errors at byte offset {}, line {} (node-kind: {})",
        bad.start_byte(), bad.start_position().row + 1, bad.kind()), "supply syntactically valid Python"))
}

pub(crate) fn mask_python(source: &amp;str) -&gt; LexicalMask {
    // walk tree-sitter tree; collect masked ranges for:
    //   - "comment" nodes
    //   - "string" nodes (excluding f-string interpolation children)
    //   - inside "string" with f-prefix: mask "string_content" but NOT "interpolation"
    //   - the `-&gt;` arrow MUST be a single mask range (or, equivalently, OffByOne treats
    //     a numeric literal node as atomic and SubtleFlip treats a comparison_operator as atomic)
    LexicalMask { masked_ranges: ranges }
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/python_lex.rs">Verify ensure_parseable_python + mask_python; add iter_code_chars helper if missing; add EC-CORPUS-01..03 regression tests.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 5 python_lex unit tests if not already present.</file>
</files_to_modify>

<validation_criteria>
  <criterion>ensure_parseable_python rejects unclosed paren with byte offset + line + node-kind</criterion>
  <criterion>mask_python masks comments, ordinary strings, triple-quoted, prefixed strings, f-string literal text</criterion>
  <criterion>f-string replacement fields exposed as code (not masked)</criterion>
  <criterion>`->` arrow not treated as comparison operator</criterion>
  <criterion>Float literal `3.14` fractional part not extracted as integer</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- python_lex</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
