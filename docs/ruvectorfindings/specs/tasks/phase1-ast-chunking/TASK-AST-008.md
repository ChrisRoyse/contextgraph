# TASK-AST-008 — Implement `ParseDiagnosticCollector` (BFS, cap 32)

```xml
<task_spec id="TASK-AST-008" version="1.0">
<metadata>
  <title>Implement parse_diagnostics — BFS over root, capped at MAX_PARSE_DIAGNOSTICS</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-AST-08">First 32 ERROR/MISSING node diagnostics with byte/line/column + excerpt</item>
    <item ref="BR-AST-02">Fail-closed; never text-fallback</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-003</task_ref>
    <task_ref>TASK-AST-004</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
When tree-sitter reports `root.has_error()`, the chunker collects up to 32 ParseDiagnostic
records and returns them in `AstChunkerError::ParseFailed { diagnostics }`. The shipped
Rust chunker has this at `ast_chunker.rs` (the legacy `parse_diagnostics()` function).
This task generalizes it: language-agnostic BFS over `Node`, no per-language branches.
</context>

<input_context_files>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#component_contracts (ParseDiagnosticCollector)</file>
  <file purpose="legacy_implementation">crates/context-graph-core/src/memory/ast_chunker.rs (search for `parse_diagnostics(` — the shipped helper)</file>
  <file purpose="cap_constant">crates/context-graph-core/src/memory/ast_chunker/types.rs (MAX_PARSE_DIAGNOSTICS = 32 from TASK-AST-003)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-003 complete: ParseDiagnostic + MAX_PARSE_DIAGNOSTICS available</check>
  <check>TASK-AST-004 complete: AstChunkerError available</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub fn parse_diagnostics(root: tree_sitter::Node, source: &[u8]) -> Vec<ParseDiagnostic>`
    - BFS walk root's subtree
    - Per node where `node.is_error() || node.is_missing()`:
      construct ParseDiagnostic with kind = node.kind().to_string(),
      is_error = node.is_error(), is_missing = node.is_missing(),
      start_byte / end_byte, start_line / start_column / end_line / end_column (1-based lines, 0-based cols),
      excerpt = ~80 chars of source around the diagnostic line
    - Cap at MAX_PARSE_DIAGNOSTICS = 32 (early-exit BFS once buffer is full)
    - Helper `fn excerpt_around_byte(source: &[u8], byte_offset: usize) -> String`: extract the
      line containing byte_offset, truncate to ~80 chars centered on the offset
    - Unit tests: deliberately broken Rust source produces diagnostics; ≥1 diagnostic with
      non-empty kind; cap test (33 errors → 32 diagnostics returned)
  </in_scope>
  <out_of_scope>
    - Per-language excerpt formatting differences (Unicode-aware truncation is in scope; per-language idiomatic excerpts are not)
    - Calling parse_diagnostics from the chunker (TASK-AST-021 facade)
    - Logging emission (caller does `tracing::error!(code = err.code(), …)`)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/diagnostics.rs">
      use tree_sitter::Node;
      use super::{ParseDiagnostic, MAX_PARSE_DIAGNOSTICS};

      /// Collect up to MAX_PARSE_DIAGNOSTICS ERROR/MISSING node diagnostics from a parse.
      pub fn parse_diagnostics(root: Node<'_>, source: &[u8]) -> Vec<ParseDiagnostic> { /* ... */ }

      fn excerpt_around_byte(source: &[u8], byte_offset: usize) -> String { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - BFS (NOT DFS) so the earliest top-level error is reported first — easier for operator triage
    - Cap is a hard limit at MAX_PARSE_DIAGNOSTICS = 32; once reached, stop traversal and return
    - excerpt is at most 128 chars (per data model), targeting ~80 chars centered on the byte
    - excerpt is UTF-8 safe — use `.chars()` to walk codepoints, not raw byte slicing, to avoid
      panicking on multi-byte boundaries
    - 1-based line numbers (tree-sitter reports 0-based row; we add 1)
    - 0-based column numbers (tree-sitter reports 0-based; we keep 0-based)
    - File under 500 lines; this file should be < 200 lines incl tests
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast_chunker::diagnostics` all pass (≥4 tests)
    - Synthetic broken Rust (`fn foo( {}` — unclosed paren) produces ≥1 diagnostic
    - Cap test: synthetic source with >32 errors → exactly 32 diagnostics
    - excerpt test: byte offset in a multi-byte-utf-8 line returns valid UTF-8 string
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
diagnostics.rs:
  use tree_sitter::Node;
  use std::collections::VecDeque;
  use super::{ParseDiagnostic, MAX_PARSE_DIAGNOSTICS};

  pub fn parse_diagnostics(root: Node, source: &[u8]) -> Vec<ParseDiagnostic> {
      let mut out = Vec::with_capacity(8);
      let mut queue: VecDeque<Node> = VecDeque::new();
      queue.push_back(root);
      while let Some(n) = queue.pop_front() {
          if out.len() >= MAX_PARSE_DIAGNOSTICS { break; }
          if n.is_error() || n.is_missing() {
              let kind = n.kind().to_string();
              out.push(ParseDiagnostic {
                  kind,
                  is_error: n.is_error(),
                  is_missing: n.is_missing(),
                  start_byte: n.start_byte(),
                  end_byte: n.end_byte(),
                  start_line: n.start_position().row as u32 + 1,
                  start_column: n.start_position().column as u32,
                  end_line: n.end_position().row as u32 + 1,
                  end_column: n.end_position().column as u32,
                  excerpt: excerpt_around_byte(source, n.start_byte()),
              });
          }
          let mut cursor = n.walk();
          for child in n.children(&mut cursor) {
              queue.push_back(child);
          }
      }
      out
  }

  fn excerpt_around_byte(source: &[u8], byte_offset: usize) -> String {
      let len = source.len();
      if len == 0 { return String::new(); }
      // Find the line containing byte_offset
      let line_start = source[..byte_offset.min(len)].iter().rposition(|&b| b == b'\n').map(|p| p + 1).unwrap_or(0);
      let line_end = byte_offset + source[byte_offset.min(len)..].iter().position(|&b| b == b'\n').unwrap_or(len - byte_offset.min(len));
      let line_bytes = &source[line_start..line_end.min(len)];
      let line_str = String::from_utf8_lossy(line_bytes);
      // Truncate to 128 chars (safe at codepoint boundary)
      if line_str.chars().count() <= 128 {
          line_str.into_owned()
      } else {
          line_str.chars().take(128).collect()
      }
  }

  #[cfg(test)] mod tests {
      use super::*;
      use tree_sitter::Parser;

      fn parse_rust(s: &str) -> tree_sitter::Tree {
          let mut p = Parser::new();
          let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
          p.set_language(&lang).unwrap();
          p.parse(s, None).unwrap()
      }

      #[test] fn unclosed_paren_yields_diagnostic() {
          let s = "fn foo( {}";
          let t = parse_rust(s);
          let diags = parse_diagnostics(t.root_node(), s.as_bytes());
          assert!(!diags.is_empty());
          assert!(diags[0].is_error || diags[0].is_missing);
          assert!(!diags[0].kind.is_empty());
      }
      #[test] fn clean_source_yields_empty() {
          let s = "fn foo() { 0 }";
          let t = parse_rust(s);
          let diags = parse_diagnostics(t.root_node(), s.as_bytes());
          assert!(diags.is_empty());
      }
      #[test] fn cap_at_32() {
          // Synthetic source with many errors — repeat broken stanza 50 times
          let mut s = String::new();
          for _ in 0..50 { s.push_str("fn ( ; "); }
          let t = parse_rust(&s);
          let diags = parse_diagnostics(t.root_node(), s.as_bytes());
          assert!(diags.len() <= MAX_PARSE_DIAGNOSTICS);
      }
      #[test] fn excerpt_handles_multibyte_utf8() {
          let s = "fn ñ( {}";  // multi-byte char in identifier
          let t = parse_rust(s);
          let diags = parse_diagnostics(t.root_node(), s.as_bytes());
          for d in &diags {
              // excerpt must be valid UTF-8 string (no panic on .chars())
              let _ = d.excerpt.chars().count();
          }
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/diagnostics.rs">parse_diagnostics + excerpt_around_byte + 4 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/mod.rs">Add `pub mod diagnostics;` + `pub use diagnostics::parse_diagnostics;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>parse_diagnostics returns ≥1 diagnostic for syntactically broken source</criterion>
  <criterion>parse_diagnostics returns empty Vec for clean source</criterion>
  <criterion>cap at MAX_PARSE_DIAGNOSTICS = 32 enforced (50 errors → ≤32 diagnostics)</criterion>
  <criterion>line numbers are 1-based; column numbers are 0-based</criterion>
  <criterion>excerpt is UTF-8 safe (no panic on multi-byte boundaries)</criterion>
  <criterion>All 4 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast_chunker::diagnostics</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
