# TASK-AST-001 — Define `Language` enum (11 variants, `#[non_exhaustive]`)

```xml
<task_spec id="TASK-AST-001" version="1.0">
<metadata>
  <title>Define Language enum with 11 variants and non_exhaustive marker</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-AST-01">Uniform chunk(source, language) API — Language type</item>
    <item ref="REQ-AST-09">Unsupported language fail-closed — exhaustive 11-variant enum</item>
  </implements>
  <depends_on><!-- none — first foundation task --></depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
The uniform `chunk(source: &[u8], language: Language)` API needs a single language tag enum
that dispatches to all 11 tree-sitter parsers. Marked `#[non_exhaustive]` so future additions
are explicit operator decisions. This is the foundation type every downstream task imports.
</context>

<input_context_files>
  <file purpose="data_model_spec">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (Language model)</file>
  <file purpose="api_dispatch_pattern">crates/context-graph-cli/src/commands/dynamicjepa.rs (lines 6454-6464; existing 11-language switch — use as reference for canonical language tags)</file>
  <file purpose="constitution_naming">docs/ruvectorfindings/specs/constitution.md#coding_standards (PascalCase types; SCREAMING_SNAKE error codes)</file>
  <file purpose="cargo_pin_versions">crates/context-graph-core/Cargo.toml (lines 33-44; tree-sitter grammar versions already wired)</file>
</input_context_files>

<prerequisites>
  <check>tree-sitter 0.26 + 11 grammar deps already present in crates/context-graph-core/Cargo.toml</check>
  <check>crates/context-graph-core/src/memory/ast_chunker/ directory does not yet exist (will be created by this task)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create new directory module crates/context-graph-core/src/memory/ast_chunker/ (replacing single-file ast_chunker.rs in subsequent tasks; this task adds a fresh file inside)
    - Define `pub enum Language` with exactly 11 variants
    - Mark `#[non_exhaustive]`
    - Derive Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize
    - Provide `Language::as_tag()` returning the canonical lowercase string ("rust","python","javascript","typescript","go","java","c","cpp","csharp","ruby","php")
    - Unit tests covering: tag round-trip, equality, debug format
  </in_scope>
  <out_of_scope>
    - tree-sitter parser construction (TASK-AST-010 LanguageDispatcher)
    - TSX vs TS dialect routing (TASK-AST-013)
    - File extension → Language inference (caller responsibility)
    - Modifying existing ast_chunker.rs file (deferred to TASK-AST-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/language.rs">
      use serde::{Deserialize, Serialize};

      /// Source language tag for the multi-language AST chunker.
      ///
      /// Exactly 11 variants matching the tree-sitter grammars pinned in Cargo.toml.
      /// Marked `#[non_exhaustive]` so future extensions require an explicit operator
      /// decision (constitution version bump per REQ-AST-09).
      #[non_exhaustive]
      #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
      #[serde(rename_all = "lowercase")]
      pub enum Language {
          Rust,
          Python,
          JavaScript,
          TypeScript,
          Go,
          Java,
          C,
          Cpp,
          CSharp,
          Ruby,
          Php,
      }

      impl Language {
          /// Canonical lowercase tag used for FSV evidence directory names and JSON serialization.
          pub fn as_tag(self) -> &'static str { /* ... */ }

          /// Iterator over all 11 variants (for testing + bulk operations).
          pub fn all() -> [Language; 11] { /* ... */ }
      }
    </signature>
  </signatures>

  <constraints>
    - Exactly 11 variants — adding/removing requires constitution version bump
    - `#[non_exhaustive]` is mandatory (REQ-AST-09 constraint)
    - Derives MUST include Hash + serde to allow use in BTreeMap keys and serialization
    - `as_tag()` returns string literal, not allocation — `&'static str`
    - The 11 tag strings MUST match `dynamicjepa.rs:6454-6464` exactly: rust, python, javascript, typescript, go, java, c, cpp, csharp, ruby, php
    - serde rename_all = "lowercase" so JSON is stable: `"rust"` not `"Rust"`
    - File under 500 lines (constitution `<file_organization>`); this file should be < 100 lines
    - NO `unwrap()` in non-test code; this file is pure data + match — no fallible ops
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core ast_chunker::language` all pass
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
    - `Language::all().len() == 11` (compile-time verifiable via static-assert pattern in test)
    - `serde_json::to_string(&Language::Rust).unwrap() == "\"rust\""`
  </verification>
</definition_of_done>

<pseudo_code>
language.rs:
  use serde::{Deserialize, Serialize};

  #[non_exhaustive]
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[serde(rename_all = "lowercase")]
  pub enum Language { Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php }

  impl Language {
      pub fn as_tag(self) -> &'static str {
          match self {
              Self::Rust => "rust",
              Self::Python => "python",
              Self::JavaScript => "javascript",
              Self::TypeScript => "typescript",
              Self::Go => "go",
              Self::Java => "java",
              Self::C => "c",
              Self::Cpp => "cpp",
              Self::CSharp => "csharp",
              Self::Ruby => "ruby",
              Self::Php => "php",
          }
      }
      pub fn all() -> [Language; 11] {
          [Self::Rust, Self::Python, Self::JavaScript, Self::TypeScript, Self::Go,
           Self::Java, Self::C, Self::Cpp, Self::CSharp, Self::Ruby, Self::Php]
      }
  }

  #[cfg(test)] mod tests {
      use super::*;
      #[test] fn tag_round_trip_via_serde() {
          for lang in Language::all() {
              let json = serde_json::to_string(&lang).unwrap();
              let back: Language = serde_json::from_str(&json).unwrap();
              assert_eq!(lang, back);
              assert_eq!(json.trim_matches('"'), lang.as_tag());
          }
      }
      #[test] fn all_eleven_variants() { assert_eq!(Language::all().len(), 11); }
      #[test] fn tags_are_lowercase_and_distinct() {
          let tags: std::collections::BTreeSet<_> = Language::all().iter().map(|l| l.as_tag()).collect();
          assert_eq!(tags.len(), 11);
          for t in &tags { assert_eq!(*t, t.to_lowercase()); }
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/language.rs">Language enum + as_tag + all + 3 unit tests</file>
  <file path="crates/context-graph-core/src/memory/ast_chunker/mod.rs">New directory module declaring `pub mod language;` and `pub use language::Language;` (this is the start of the directory module; subsequent tasks add more pub mod entries)</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/mod.rs">Add `pub mod ast_chunker;` if not already present, OR if `ast_chunker.rs` (the legacy single file) is already declared, leave the legacy declaration; the directory module replacement happens in TASK-AST-021</file>
</files_to_modify>

<validation_criteria>
  <criterion>Language enum exists with exactly 11 variants matching the spec</criterion>
  <criterion>`#[non_exhaustive]` attribute present</criterion>
  <criterion>serde Serialize/Deserialize round-trip preserves variant identity</criterion>
  <criterion>`as_tag()` returns lowercase &'static str for each variant</criterion>
  <criterion>All 3 unit tests pass</criterion>
  <criterion>cargo clippy reports no warnings for this file</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast_chunker::language</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
