# TASK-AST-001 — Audit `Language` enum (11 variants, fail-closed dispatch)

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth. Any lower reference to `src/memory/ast_chunker/<submodule>.rs`, `dynamicjepa.rs`, compatibility shims, or "create new" scaffolding language is superseded by this audit note. Verify and harden the existing canonical files instead of creating parallel modules unless the repo audit proves a file is genuinely missing.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task (Rust source file, JSON evidence file, RocksDB CF, or ME-JEPA panel file), run the logic, then perform a separate readback from that source of truth. Print before/after system state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution, not only function return values.

```xml
<task_spec id="TASK-AST-001" version="1.0">
<metadata>
  <title>Audit Language enum with 11 variants and fail-closed dispatch</title>
  <status>audit-hardening</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-AST-01">Uniform chunk(source, language) API — Language type</item>
    <item ref="REQ-AST-09">Unsupported language fail-closed — exhaustive 11-variant enum</item>
  </implements>
  <depends_on><!-- none — first foundation task --></depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>current canonical file: crates/context-graph-core/src/memory/ast/mod.rs</estimated_files>
</metadata>

<context>
The uniform `chunk(source: &[u8], language: Language)` API uses a single language tag enum
that dispatches to all 11 tree-sitter parsers. Current code intentionally uses exhaustive
matches so adding a 12th language breaks dispatch/classifier audits at compile time.
</context>

<input_context_files>
  <file purpose="data_model_spec">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (Language model)</file>
  <file purpose="api_dispatch_pattern">crates/context-graph-cli/src/commands/dynamicjepa.rs (lines 6454-6464; existing 11-language switch — use as reference for canonical language tags)</file>
  <file purpose="constitution_naming">docs/ruvectorfindings/specs/constitution.md#coding_standards (PascalCase types; SCREAMING_SNAKE error codes)</file>
  <file purpose="cargo_pin_versions">crates/context-graph-core/Cargo.toml (lines 33-44; tree-sitter grammar versions already wired)</file>
</input_context_files>

<prerequisites>
  <check>tree-sitter 0.26 + 11 grammar deps already present in crates/context-graph-core/Cargo.toml</check>
  <check>crates/context-graph-core/src/memory/ast/ exists and is the canonical Phase 1 implementation</check>
</prerequisites>

<scope>
  <in_scope>
    - Audit existing `pub enum Language` with exactly 11 variants in crates/context-graph-core/src/memory/ast/mod.rs
    - Do not add `#[non_exhaustive]` unless all exhaustive match sites are intentionally updated; current fail-closed behavior relies on exhaustive matches compiling when a 12th language is added
    - Derive Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize
    - Provide/verify `Language::slug()` returning the canonical lowercase string ("rust","python","javascript","typescript","go","java","c","cpp","csharp","ruby","php")
    - Provide/verify `Language::from_slug()` and `Language::from_path()` fail closed with `UnsupportedLanguage`
    - Unit tests covering: tag round-trip, equality, debug format
  </in_scope>
  <out_of_scope>
    - tree-sitter parser construction (TASK-AST-010 LanguageDispatcher)
    - TSX vs TS dialect routing (TASK-AST-013)
    - File extension → Language inference (caller responsibility)
    - Modifying legacy ast_chunker.rs except to remove stale use after callers move to memory::ast
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast/mod.rs">
      use serde::{Deserialize, Serialize};

      /// Source language tag for the multi-language AST chunker.
      ///
      /// Exactly 11 variants matching the tree-sitter grammars pinned in Cargo.toml.
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
          pub fn slug(self) -> &'static str { /* ... */ }

          /// Iterator over all 11 variants (for testing + bulk operations).
          pub fn all() -> [Language; 11] { /* ... */ }
      }
    </signature>
  </signatures>

  <constraints>
    - Exactly 11 variants — adding/removing requires constitution version bump
    - Exhaustive matches are intentional; adding a 12th language should fail compilation at dispatch/classification/routing audit points
    - Derives MUST include Hash + serde to allow use in BTreeMap keys and serialization
    - `slug()` returns string literal, not allocation — `&'static str`
    - The 11 tag strings MUST match `dynamicjepa.rs:6454-6464` exactly: rust, python, javascript, typescript, go, java, c, cpp, csharp, ruby, php
    - serde rename_all = "lowercase" so JSON is stable: `"rust"` not `"Rust"`
    - File under 500 lines (constitution `<file_organization>`); this file should be < 100 lines
    - NO `unwrap()` in non-test code; this file is pure data + match — no fallible ops
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core ast::language` all pass
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
    - `Language::all().len() == 11` (compile-time verifiable via static-assert pattern in test)
    - `serde_json::to_string(&Language::Rust).unwrap() == "\"rust\""`
  </verification>
</definition_of_done>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/mod.rs">Verify `pub mod ast;` exports the canonical module; do not create another `ast_chunker/` module.</file>
</files_to_modify>

<validation_criteria>
  <criterion>Language enum exists with exactly 11 variants matching the spec</criterion>
  <criterion>Adding a new variant forces explicit dispatch/classifier audit updates through exhaustive matches</criterion>
  <criterion>serde Serialize/Deserialize round-trip preserves variant identity</criterion>
  <criterion>`slug()` returns lowercase &'static str for each variant</criterion>
  <criterion>`from_slug()` and `from_path()` reject unsupported languages with `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE`</criterion>
  <criterion>Relevant `memory::ast::tests` pass</criterion>
  <criterion>cargo clippy reports no warnings for this file</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast::language</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
