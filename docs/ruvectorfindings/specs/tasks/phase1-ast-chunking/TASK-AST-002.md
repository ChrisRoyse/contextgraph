# TASK-AST-002 — Define `EntityType` (13 variants) + `ClassificationContext`

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth. Any lower reference to `src/memory/ast_chunker/<submodule>.rs`, `dynamicjepa.rs`, compatibility shims, or "create new" scaffolding language is superseded by this audit note. Verify and harden the existing canonical files instead of creating parallel modules unless the repo audit proves a file is genuinely missing.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task (Rust source file, JSON evidence file, RocksDB CF, or ME-JEPA panel file), run the logic, then perform a separate readback from that source of truth. Print before/after system state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution, not only function return values.

```xml
<task_spec id="TASK-AST-002" version="1.0">
<metadata>
  <title>Define language-tagged EntityType enum (13 variants) + ClassificationContext</title>
  <status>audit-hardening</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-AST-03">EntityType has exactly 13 variants for 24,024-cell constellation math</item>
    <item ref="BR-AST-04">Function/Method/TestFunction ambiguity defaults to Function</item>
  </implements>
  <depends_on><!-- none — parallel with TASK-AST-001 --></depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
The shipped chunker has a 14-variant Rust-specific EntityType (Function, Method, Struct, Enum,
Trait, Impl, Module, Const, Static, TypeAlias, Macro, Comment, Mixed, Unknown). Phase 1a
replaces this with a language-agnostic 13-variant enum aligned with doc 14 §5 cell math
(8 mutation × 11 lang × 13 entity × 21 embedder = 24,024). This is a breaking change for
the legacy Rust chunker, which TASK-AST-021 will reconcile via a translation layer.
ClassificationContext threads parser state through the boundary engine.
</context>

<input_context_files>
  <file purpose="data_model_spec">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (EntityType model)</file>
  <file purpose="legacy_enum">crates/context-graph-core/src/memory/ast_chunker.rs (lines 110-162; legacy EntityType to be replaced)</file>
  <file purpose="cell_math">docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md §5 (24,024-cell breakdown)</file>
  <file purpose="constitution_anti_pattern">docs/ruvectorfindings/specs/constitution.md (anti-pattern: no random initialization, BTreeMap-only)</file>
</input_context_files>

<prerequisites>
  <check>crates/context-graph-core/src/memory/ast/ directory exists (created by TASK-AST-001)</check>
</prerequisites>

<scope>
  <in_scope>
    - Define `pub enum EntityType` with exactly 13 variants:
      Function, Method, Class, Struct, Enum, TraitOrInterface, Impl, Module, Namespace,
      TestFunction, Import, CommentBlock, Docstring
    - Derive Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize
    - Define `pub struct ClassificationContext` carrying inside_class, inside_impl,
      inside_test_module, parent_named_chain
    - Provide `EntityType::all()` for tests
    - Unit tests: serde round-trip; ordering stability; all() count
  </in_scope>
  <out_of_scope>
    - Per-language node-kind classification (TASK-AST-011..020)
    - Mapping legacy EntityType to new EntityType (TASK-AST-021)
    - Embedder routing (TASK-AST-005)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast/mod.rs">
      use serde::{Deserialize, Serialize};

      /// Semantic category for an AST chunk.
      ///
      /// Exactly 13 variants per REQ-AST-03 / doc 14 §5 (24,024-cell constellation).
      #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
      pub enum EntityType {
          Function,
          Method,
          Class,
          Struct,
          Enum,
          TraitOrInterface,
          Impl,
          Module,
          Namespace,
          TestFunction,
          Import,
          CommentBlock,
          Docstring,
      }

      impl EntityType {
          pub fn all() -> [EntityType; 13] { /* ... */ }
      }

      /// Per-walk classifier state threaded through ChunkBoundaryEngine.
      #[derive(Debug, Clone, Default)]
      pub struct ClassificationContext {
          pub inside_class: bool,
          pub inside_impl: bool,
          pub inside_test_module: bool,
          pub parent_named_chain: Vec<String>,
      }
    </signature>
  </signatures>

  <constraints>
    - Exactly 13 variants. `#[deny(non_exhaustive_omitted_patterns)]` recommended on call sites
      so adding a 14th variant breaks the build everywhere it's matched.
    - `Hash + Ord + PartialOrd` REQUIRED so EntityType can key a BTreeMap (constitution
      determinism rule)
    - `ClassificationContext::default()` returns all-false / empty-Vec
    - File under 500 lines; this file should be < 120 lines
    - Naming MUST match TECH-AST-CHUNKING.md letter-for-letter (TraitOrInterface not Trait;
      CommentBlock not Comment; etc.)
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core ast::entity_type` all pass
    - `EntityType::all().len() == 13` test
    - `serde_json::to_string(&EntityType::Function)` produces `"Function"` (PascalCase, default
      serde naming) so the on-disk JSON is human-readable
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
entity_type.rs:
  use serde::{Deserialize, Serialize};

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
  pub enum EntityType { Function, Method, Class, Struct, Enum, TraitOrInterface, Impl,
      Module, Namespace, TestFunction, Import, CommentBlock, Docstring }

  impl EntityType {
      pub fn all() -> [EntityType; 13] {
          [Self::Function, Self::Method, Self::Class, Self::Struct, Self::Enum,
           Self::TraitOrInterface, Self::Impl, Self::Module, Self::Namespace,
           Self::TestFunction, Self::Import, Self::CommentBlock, Self::Docstring]
      }
  }

  #[derive(Debug, Clone, Default)]
  pub struct ClassificationContext {
      pub inside_class: bool,
      pub inside_impl: bool,
      pub inside_test_module: bool,
      pub parent_named_chain: Vec<String>,
  }

  #[cfg(test)] mod tests {
      use super::*;
      #[test] fn all_thirteen() { assert_eq!(EntityType::all().len(), 13); }
      #[test] fn round_trip_serde() {
          for et in EntityType::all() {
              let json = serde_json::to_string(&et).unwrap();
              let back: EntityType = serde_json::from_str(&json).unwrap();
              assert_eq!(et, back);
          }
      }
      #[test] fn ordering_stable() {
          let mut v = EntityType::all().to_vec();
          v.sort();
          // First sorted variant is Function (declaration order; Ord derived from variant index)
          assert_eq!(v[0], EntityType::Function);
      }
      #[test] fn ctx_default_is_clean() {
          let ctx = ClassificationContext::default();
          assert!(!ctx.inside_class && !ctx.inside_impl && !ctx.inside_test_module);
          assert!(ctx.parent_named_chain.is_empty());
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast/mod.rs">EntityType (13 variants) + ClassificationContext + 4 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast/mod.rs">Add `pub mod entity_type;` and `pub use entity_type::{EntityType, ClassificationContext};`</file>
</files_to_modify>

<validation_criteria>
  <criterion>EntityType has exactly 13 variants matching TECH-AST-CHUNKING.md letter-for-letter</criterion>
  <criterion>EntityType derives Hash + Ord + serde so it can key BTreeMap and round-trip JSON</criterion>
  <criterion>ClassificationContext exists with the 4 fields specified</criterion>
  <criterion>All 4 unit tests pass</criterion>
  <criterion>No clippy warnings</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast::entity_type</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
