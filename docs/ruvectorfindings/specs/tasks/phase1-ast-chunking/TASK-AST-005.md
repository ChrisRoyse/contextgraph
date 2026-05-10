# TASK-AST-005 — Implement `EmbedderRoutingTable` (per-EntityType static lookup)

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth. Any lower reference to `src/memory/ast_chunker/<submodule>.rs`, `dynamicjepa.rs`, compatibility shims, or "create new" scaffolding language is superseded by this audit note. Verify and harden the existing canonical files instead of creating parallel modules unless the repo audit proves a file is genuinely missing.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task (Rust source file, JSON evidence file, RocksDB CF, or ME-JEPA panel file), run the logic, then perform a separate readback from that source of truth. Print before/after system state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution, not only function return values.

```xml
<task_spec id="TASK-AST-005" version="1.0">
<metadata>
  <title>Implement EmbedderRoutingTable — compile-time exhaustive route_to_embedders</title>
  <status>audit-hardening</status>
  <layer>foundation</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="REQ-AST-06">Each chunk tagged with embedder_routing_set per doc 14 §4 table</item>
    <item ref="BR-AST-05">Doc 14 §4 routing matrix encoded once at compile time</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
Every chunk emitted carries a BTreeSet<EmbedderId> describing which embedders should
encode it. The mapping is fixed by doc 14 §4 (TECH-AST-CHUNKING.md EmbedderRoutingTable
model): Function/Method → {E7, E14}; Class/Struct/Enum/TraitOrInterface/Impl → {E11, E7};
Module/Namespace → {E1, E8}; TestFunction → {E1, E7}; Import → {E11, E13};
CommentBlock/Docstring → {E1, E10}. Compile-time exhaustive match guarantees adding
a 14th EntityType variant breaks the build until routed.
</context>

<input_context_files>
  <file purpose="data_model_spec">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (EmbedderRoutingTable model)</file>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#component_contracts (EmbedderRouter)</file>
  <file purpose="routing_source">docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md §4 (per-chunk-type routing table)</file>
  <file purpose="embedder_id">crates/context-graph-core/src/memory/ast/routing.rs (EmbedderId enum E1..E21 and routing table)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-002 complete: EntityType (13 variants) available</check>
  <check>EmbedderId enum and routing table are available in crates/context-graph-core/src/memory/ast/routing.rs</check>
</prerequisites>

<scope>
  <in_scope>
    - Define const EmbedderId numeric IDs: E1=1, E7=7, E8=8, E10=10, E11=11, E13=13, E14=14
      (these match `crates/context-graph-embeddings/src/types/model_id/core.rs` numbering;
      revisited in TASK-EMBED-001)
    - Implement `pub fn route_to_embedders(entity_type: EntityType) -> BTreeSet<EmbedderId>`
      with compile-time exhaustive match
    - Use `OnceLock` or `LazyLock` (Rust 1.80+) to cache the BTreeSet per EntityType so we
      pay BTreeSet construction once
    - Unit tests covering all 13 variants — assert exact set membership matches doc 14 §4
  </in_scope>
  <out_of_scope>
    - The actual EmbedderId enum (TASK-EMBED-001)
    - The embedders themselves (SPEC-EMBEDDERS / TECH-EMBEDDERS)
    - Per-language overrides (none specified — routing is pure-EntityType-driven)
    - Future "function signature vs body" sub-routing (deferred per BR-AST-05)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast/routing.rs">
      use std::collections::BTreeSet;
      use super::{EntityType, EmbedderId};

      pub const E1: EmbedderId = 1;
      pub const E7: EmbedderId = 7;
      pub const E8: EmbedderId = 8;
      pub const E10: EmbedderId = 10;
      pub const E11: EmbedderId = 11;
      pub const E13: EmbedderId = 13;
      pub const E14: EmbedderId = 14;

      /// Map an EntityType to its target embedder set per doc 14 §4 routing table.
      ///
      /// Compile-time exhaustive — adding a 14th EntityType variant breaks the build here.
      pub fn route_to_embedders(entity_type: EntityType) -> BTreeSet<EmbedderId> {
          match entity_type {
              EntityType::Function | EntityType::Method => BTreeSet::from([E7, E14]),
              EntityType::Class | EntityType::Struct | EntityType::Enum
                  | EntityType::TraitOrInterface | EntityType::Impl => BTreeSet::from([E11, E7]),
              EntityType::Module | EntityType::Namespace => BTreeSet::from([E1, E8]),
              EntityType::TestFunction => BTreeSet::from([E1, E7]),
              EntityType::Import => BTreeSet::from([E11, E13]),
              EntityType::CommentBlock | EntityType::Docstring => BTreeSet::from([E1, E10]),
          }
      }
    </signature>
  </signatures>

  <constraints>
    - Exhaustive match — NO `_ => …` catch-all (would defeat REQ-AST-03 enforcement)
    - BTreeSet (NOT HashSet) for deterministic ordering
    - Returned set MUST be non-empty for every variant (verified in test)
    - Constants E1..E14 use only the 7 numbers actually referenced in the table; do NOT pre-allocate IDs for embedders not yet in scope
    - File under 500 lines; this file should be < 100 lines
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast::router` all 14 tests pass
      (13 per-variant + 1 totality test ensuring every variant maps to non-empty set)
    - Removing or renaming any EntityType variant breaks the build (compile-time check)
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
router.rs:
  use std::collections::BTreeSet;
  use super::{EntityType, EmbedderId};

  pub const E1: EmbedderId = 1;
  pub const E7: EmbedderId = 7;
  pub const E8: EmbedderId = 8;
  pub const E10: EmbedderId = 10;
  pub const E11: EmbedderId = 11;
  pub const E13: EmbedderId = 13;
  pub const E14: EmbedderId = 14;

  pub fn route_to_embedders(entity_type: EntityType) -> BTreeSet<EmbedderId> {
      match entity_type {
          EntityType::Function | EntityType::Method => BTreeSet::from([E7, E14]),
          EntityType::Class | EntityType::Struct | EntityType::Enum
              | EntityType::TraitOrInterface | EntityType::Impl => BTreeSet::from([E11, E7]),
          EntityType::Module | EntityType::Namespace => BTreeSet::from([E1, E8]),
          EntityType::TestFunction => BTreeSet::from([E1, E7]),
          EntityType::Import => BTreeSet::from([E11, E13]),
          EntityType::CommentBlock | EntityType::Docstring => BTreeSet::from([E1, E10]),
      }
  }

  #[cfg(test)] mod tests {
      use super::*;
      #[test] fn function_routes_to_e7_e14() {
          let s = route_to_embedders(EntityType::Function);
          assert_eq!(s, BTreeSet::from([E7, E14]));
      }
      #[test] fn method_routes_same_as_function() {
          assert_eq!(route_to_embedders(EntityType::Method),
                     route_to_embedders(EntityType::Function));
      }
      #[test] fn class_struct_enum_traitorinterface_impl_route_to_e11_e7() {
          let expect = BTreeSet::from([E11, E7]);
          for et in [EntityType::Class, EntityType::Struct, EntityType::Enum,
                     EntityType::TraitOrInterface, EntityType::Impl] {
              assert_eq!(route_to_embedders(et), expect);
          }
      }
      #[test] fn module_namespace_route_to_e1_e8() {
          let expect = BTreeSet::from([E1, E8]);
          for et in [EntityType::Module, EntityType::Namespace] {
              assert_eq!(route_to_embedders(et), expect);
          }
      }
      #[test] fn testfunction_routes_to_e1_e7() {
          assert_eq!(route_to_embedders(EntityType::TestFunction), BTreeSet::from([E1, E7]));
      }
      #[test] fn import_routes_to_e11_e13() {
          assert_eq!(route_to_embedders(EntityType::Import), BTreeSet::from([E11, E13]));
      }
      #[test] fn comment_docstring_route_to_e1_e10() {
          for et in [EntityType::CommentBlock, EntityType::Docstring] {
              assert_eq!(route_to_embedders(et), BTreeSet::from([E1, E10]));
          }
      }
      #[test] fn every_variant_routes_to_nonempty_set() {
          for et in EntityType::all() {
              let s = route_to_embedders(et);
              assert!(!s.is_empty(), "{:?} routed to empty set", et);
          }
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast/routing.rs">EmbedderId constants + route_to_embedders + 8 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast/mod.rs">Add `pub mod router;` + `pub use router::route_to_embedders;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>route_to_embedders is exhaustive over all 13 EntityType variants (compile-time)</criterion>
  <criterion>Routing matches doc 14 §4 table exactly for all 13 EntityType variants</criterion>
  <criterion>Returned BTreeSet is non-empty for every variant</criterion>
  <criterion>BTreeSet ordering is stable across calls (determinism)</criterion>
  <criterion>All 8 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast::router</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
