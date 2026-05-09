# TASK-AST-010 — Implement `LanguageDispatcher` (Language → tree-sitter parser)

```xml
<task_spec id="TASK-AST-010" version="1.0">
<metadata>
  <title>Implement parser_for_language — exhaustive Language → tree_sitter::Parser dispatch</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-AST-01">Uniform chunk(source, language) — parser construction step</item>
    <item ref="REQ-AST-09">UnsupportedLanguage compile-time exhaustiveness</item>
    <item ref="ERR-AST-05">LanguageSetFailed defense-in-depth</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-001</task_ref>
    <task_ref>TASK-AST-004</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
The chunker constructs a fresh `tree_sitter::Parser` per `chunk()` call (parsers are
not Send-safe across the parsing operation; rayon parallelism happens at the call site).
This dispatcher centralizes the 11-arm switch from `Language` enum → tree-sitter LANGUAGE
constant. It mirrors the pattern at `dynamicjepa.rs:6454-6464`. TypeScript dispatches to
`LANGUAGE_TYPESCRIPT` here; the TSX dialect is wired in TASK-AST-013 as a separate
chunker entry point taking a `is_tsx: bool` flag (per TECH-AST-CHUNKING.md tsx_dispatch note).
</context>

<input_context_files>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#component_contracts (LanguageDispatcher)</file>
  <file purpose="reference_dispatcher">crates/context-graph-cli/src/commands/dynamicjepa.rs (lines 6454-6464; existing 11-language switch)</file>
  <file purpose="cargo_pins">crates/context-graph-core/Cargo.toml (lines 33-44; tree-sitter grammar deps)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-001 complete: Language enum available</check>
  <check>TASK-AST-004 complete: AstChunkerError available</check>
  <check>11 tree-sitter grammar crates wired in Cargo.toml</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub fn parser_for_language(language: Language) -> Result<Parser, AstChunkerError>`
    - Compile-time exhaustive match — adding a 12th variant must break the build here
    - Per language, set `tree_sitter::Language` from the grammar's `LANGUAGE` const:
      Rust → tree_sitter_rust::LANGUAGE
      Python → tree_sitter_python::LANGUAGE
      JavaScript → tree_sitter_javascript::LANGUAGE
      TypeScript → tree_sitter_typescript::LANGUAGE_TYPESCRIPT
      Go → tree_sitter_go::LANGUAGE
      Java → tree_sitter_java::LANGUAGE
      C → tree_sitter_c::LANGUAGE
      Cpp → tree_sitter_cpp::LANGUAGE
      CSharp → tree_sitter_c_sharp::LANGUAGE
      Ruby → tree_sitter_ruby::LANGUAGE
      Php → tree_sitter_php::LANGUAGE_PHP
    - On `parser.set_language()` failure → `Err(LanguageSetFailed { language: as_tag.into(), reason: format!("{e:?}") })`
    - Provide `pub fn parser_for_typescript_tsx() -> Result<Parser, AstChunkerError>` returning a
      parser configured for `tree_sitter_typescript::LANGUAGE_TSX` (separate entry; doc 14 note)
    - Unit tests: each of 11 + TSX construction succeeds (12 tests)
  </in_scope>
  <out_of_scope>
    - Calling parser.parse() (TASK-AST-021 facade)
    - Parser pooling / caching (out of scope; parsers cheap to construct)
    - Filename-extension → Language inference (caller responsibility)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/dispatch.rs">
      use tree_sitter::{Language as TsLanguage, Parser};
      use super::{AstChunkerError, Language};

      /// Construct a tree-sitter Parser configured for the given Language.
      pub fn parser_for_language(language: Language) -> Result<Parser, AstChunkerError> { /* ... */ }

      /// Construct a tree-sitter Parser configured for TypeScript TSX dialect (.tsx files).
      pub fn parser_for_typescript_tsx() -> Result<Parser, AstChunkerError> { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - Match is exhaustive — no `_ => unreachable!()` (compile-time guarantee)
    - Parser construction may fail only via set_language; handle as LanguageSetFailed
    - tree-sitter 0.26 + grammars 0.23.x ABI compatibility (already validated by shipped Rust chunker)
    - Each call returns a fresh Parser (no global state)
    - File under 500 lines; this file should be < 100 lines
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast_chunker::dispatch` all 12 tests pass
    - Each Language variant produces a Parser that successfully parses a minimal source for
      that language (e.g. Rust `fn f() {}`, Python `def f(): pass`, Go `package main`)
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
dispatch.rs:
  use tree_sitter::{Language as TsLanguage, Parser};
  use super::{AstChunkerError, Language};

  pub fn parser_for_language(language: Language) -> Result<Parser, AstChunkerError> {
      let ts_lang: TsLanguage = match language {
          Language::Rust => tree_sitter_rust::LANGUAGE.into(),
          Language::Python => tree_sitter_python::LANGUAGE.into(),
          Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
          Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
          Language::Go => tree_sitter_go::LANGUAGE.into(),
          Language::Java => tree_sitter_java::LANGUAGE.into(),
          Language::C => tree_sitter_c::LANGUAGE.into(),
          Language::Cpp => tree_sitter_cpp::LANGUAGE.into(),
          Language::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
          Language::Ruby => tree_sitter_ruby::LANGUAGE.into(),
          Language::Php => tree_sitter_php::LANGUAGE_PHP.into(),
      };
      let mut parser = Parser::new();
      parser.set_language(&ts_lang).map_err(|e| AstChunkerError::LanguageSetFailed {
          language: language.as_tag().to_string(),
          reason: format!("{:?}", e),
      })?;
      Ok(parser)
  }

  pub fn parser_for_typescript_tsx() -> Result<Parser, AstChunkerError> {
      let ts_lang: TsLanguage = tree_sitter_typescript::LANGUAGE_TSX.into();
      let mut parser = Parser::new();
      parser.set_language(&ts_lang).map_err(|e| AstChunkerError::LanguageSetFailed {
          language: "typescript_tsx".into(), reason: format!("{:?}", e),
      })?;
      Ok(parser)
  }

  #[cfg(test)] mod tests {
      use super::*;
      #[test] fn rust_parses_minimal() {
          let mut p = parser_for_language(Language::Rust).unwrap();
          assert!(p.parse("fn f() {}", None).is_some());
      }
      #[test] fn python_parses_minimal() {
          let mut p = parser_for_language(Language::Python).unwrap();
          assert!(p.parse("def f():\n    pass\n", None).is_some());
      }
      #[test] fn javascript_parses_minimal() {
          let mut p = parser_for_language(Language::JavaScript).unwrap();
          assert!(p.parse("function f() {}", None).is_some());
      }
      #[test] fn typescript_parses_minimal() {
          let mut p = parser_for_language(Language::TypeScript).unwrap();
          assert!(p.parse("function f(): void {}", None).is_some());
      }
      #[test] fn go_parses_minimal() {
          let mut p = parser_for_language(Language::Go).unwrap();
          assert!(p.parse("package main\nfunc f() {}", None).is_some());
      }
      #[test] fn java_parses_minimal() {
          let mut p = parser_for_language(Language::Java).unwrap();
          assert!(p.parse("class A { void f() {} }", None).is_some());
      }
      #[test] fn c_parses_minimal() {
          let mut p = parser_for_language(Language::C).unwrap();
          assert!(p.parse("int f() { return 0; }", None).is_some());
      }
      #[test] fn cpp_parses_minimal() {
          let mut p = parser_for_language(Language::Cpp).unwrap();
          assert!(p.parse("int f() { return 0; }", None).is_some());
      }
      #[test] fn csharp_parses_minimal() {
          let mut p = parser_for_language(Language::CSharp).unwrap();
          assert!(p.parse("class A { void F() {} }", None).is_some());
      }
      #[test] fn ruby_parses_minimal() {
          let mut p = parser_for_language(Language::Ruby).unwrap();
          assert!(p.parse("def f; end\n", None).is_some());
      }
      #[test] fn php_parses_minimal() {
          let mut p = parser_for_language(Language::Php).unwrap();
          assert!(p.parse("<?php function f() {} ?>", None).is_some());
      }
      #[test] fn typescript_tsx_parses_minimal() {
          let mut p = parser_for_typescript_tsx().unwrap();
          assert!(p.parse("const X = () => <div />;", None).is_some());
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/dispatch.rs">parser_for_language + parser_for_typescript_tsx + 12 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/mod.rs">Add `pub mod dispatch;` + `pub use dispatch::{parser_for_language, parser_for_typescript_tsx};`</file>
</files_to_modify>

<validation_criteria>
  <criterion>parser_for_language exhaustively matches all 11 Language variants (compile-time)</criterion>
  <criterion>Each variant constructs a Parser that successfully parses minimal source for the language</criterion>
  <criterion>parser_for_typescript_tsx returns a parser configured for LANGUAGE_TSX (parses JSX)</criterion>
  <criterion>set_language failures map to LanguageSetFailed with .code() = MEJEPA_CHUNKER_PARSE_FAILED</criterion>
  <criterion>All 12 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast_chunker::dispatch</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
