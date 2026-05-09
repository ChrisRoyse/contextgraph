# TASK-AST-013 — Implement TypeScript / TSX entity classifier

```xml
<task_spec id="TASK-AST-013" version="1.0">
<metadata>
  <title>Implement TypeScript classifier — TS dialect (.ts/.mts/.cts) + TSX dialect (.tsx)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>13</sequence>
  <implements>
    <item ref="REQ-AST-03">EntityType for TS-specific kinds: interface_declaration, type_alias_declaration, enum_declaration, namespace_declaration, abstract_class_declaration</item>
    <item ref="REQ-AST-12">TS TestFunction detection (same DSL as JS — reuses TEST_DSL_NAMES)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-010</task_ref>
    <task_ref>TASK-AST-012</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
TypeScript inherits the JavaScript classifier (delegates via classifier::javascript::classify
for shared kinds) and adds TS-specific node kinds. tree-sitter-typescript exposes
LANGUAGE_TYPESCRIPT (for .ts/.mts/.cts) and LANGUAGE_TSX (for .tsx). The chunker's uniform
`chunk(source, Language::TypeScript)` defaults to LANGUAGE_TYPESCRIPT; callers needing TSX
use a sibling entry `chunk_tsx(source)` (TASK-AST-021 facade exposes this).
</context>

<input_context_files>
  <file purpose="js_classifier">crates/context-graph-core/src/memory/ast_chunker/classifier/javascript.rs (TASK-AST-012; TEST_DSL_NAMES + helpers reused)</file>
  <file purpose="entity_type">crates/context-graph-core/src/memory/ast_chunker/entity_type.rs</file>
  <file purpose="dispatch">crates/context-graph-core/src/memory/ast_chunker/dispatch.rs (parser_for_typescript_tsx from TASK-AST-010)</file>
  <file purpose="node_kind_matrix">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (PerLanguageNodeKindTable.typescript)</file>
  <file purpose="tsx_note">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md (implementation_notes "tsx_dispatch")</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-002 + TASK-AST-010 + TASK-AST-012 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Module: `crates/context-graph-core/src/memory/ast_chunker/classifier/typescript.rs`
    - `pub fn hooks() -> LanguageHooks` for TS dialect
    - `pub fn hooks_tsx() -> LanguageHooks` for TSX dialect (same hooks; the dispatch differs at
      parser level only — TSX recognizes JSX expressions which surface as different node kinds
      that classifier passes through unchanged)
    - classify dispatch:
      • Delegate to javascript::classify FIRST for shared JS kinds (function/class/method/import/comment)
      • Then add TS-specific kinds:
        interface_declaration → TraitOrInterface
        type_alias_declaration → TraitOrInterface (alias for interface-like definition; doc 14 §5
          treats type aliases as same constellation cell as interface)
        enum_declaration → Enum
        namespace_declaration | internal_module → Namespace
        abstract_class_declaration → Class
    - is_chunkable: JS set + interface_declaration, type_alias_declaration, enum_declaration,
      namespace_declaration, abstract_class_declaration
    - get_node_name: same as JS for shared kinds; for TS kinds, read `.name` field
    - is_container: class_declaration, class_expression, abstract_class_declaration,
      namespace_declaration, internal_module
    - Reuse javascript::TEST_DSL_NAMES via `use crate::memory::ast_chunker::classifier::javascript::TEST_DSL_NAMES;`
    - Unit tests: ≥10 tests covering interface/type-alias/enum/namespace/abstract-class +
      delegation to JS classifier for shared kinds + TSX (JSX expression in fragment)
  </in_scope>
  <out_of_scope>
    - Parser switching (TASK-AST-010 already exposes parser_for_typescript_tsx)
    - Mixed-language detection (TASK-AST-009 handles JSX-with-CSS-style-blocks)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/classifier/typescript.rs">
      use tree_sitter::Node;
      use crate::memory::ast_chunker::{ClassificationContext, EntityType, LanguageHooks};
      use super::javascript;

      pub fn hooks() -> LanguageHooks { /* ... */ }
      pub fn hooks_tsx() -> LanguageHooks { hooks() }  // same classifier; parser differs

      pub fn classify(node: &Node<'_>, source: &[u8], ctx: &ClassificationContext) -> EntityType { /* ... */ }
      pub fn is_chunkable(node: &Node<'_>) -> bool { /* ... */ }
      pub fn get_node_name(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
      pub fn is_container(node: &Node<'_>) -> bool { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - classify: delegate to javascript::classify when node.kind() is a shared JS kind; only
      add explicit cases for TS-specific kinds
    - hooks_tsx() returns the same LanguageHooks as hooks() — TSX parser produces additional
      JSX node kinds (`jsx_element`, `jsx_self_closing_element`, etc.) which are NOT chunkable
      and therefore handled by `is_chunkable` returning false; they recurse into via
      process_node's else branch
    - File under 500 lines; this file should be < 350 lines incl tests
    - GENERIC-ONLY: rules apply to ANY .ts file
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast_chunker::classifier::typescript` ≥10 tests pass
    - `interface I { x: number }` → TraitOrInterface
    - `type T = string` → TraitOrInterface
    - `enum E { A, B }` → Enum
    - `namespace N { ... }` → Namespace
    - `abstract class C { abstract m(): void; }` → Class
    - JS-shared kinds delegate to javascript classifier (regression: identical EntityType to
      javascript classifier on shared fixtures)
    - TSX: `const X = () => <div />;` → Function (the const binding) — JSX expression child does
      not become a separate chunk
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
classifier/typescript.rs:
  use crate::memory::ast_chunker::classifier::javascript;

  pub fn classify(node, source, ctx) -> EntityType {
      match node.kind() {
          "interface_declaration" => EntityType::TraitOrInterface,
          "type_alias_declaration" => EntityType::TraitOrInterface,
          "enum_declaration" => EntityType::Enum,
          "namespace_declaration" | "internal_module" => EntityType::Namespace,
          "abstract_class_declaration" => EntityType::Class,
          _ => javascript::classify(node, source, ctx),
      }
  }

  pub fn is_chunkable(node) -> bool {
      javascript::is_chunkable(node) || matches!(
          node.kind(),
          "interface_declaration" | "type_alias_declaration" | "enum_declaration"
              | "namespace_declaration" | "internal_module" | "abstract_class_declaration"
      )
  }

  pub fn is_container(node) -> bool {
      javascript::is_container(node) || matches!(
          node.kind(),
          "abstract_class_declaration" | "namespace_declaration" | "internal_module"
      )
  }

  // 10+ tests covering each TS-specific kind + delegation back to JS
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/typescript.rs">TS classifier hooks (delegating to javascript) + TSX hooks alias + 10 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/mod.rs">Add `pub mod typescript;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>interface_declaration → TraitOrInterface; type_alias_declaration → TraitOrInterface</criterion>
  <criterion>enum_declaration → Enum; namespace_declaration → Namespace</criterion>
  <criterion>abstract_class_declaration → Class</criterion>
  <criterion>JS-shared kinds delegate to javascript::classify (identical output)</criterion>
  <criterion>hooks_tsx() returns same hooks as hooks() (parser dispatched separately)</criterion>
  <criterion>TSX with JSX expression child does not produce extra chunks</criterion>
  <criterion>≥10 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast_chunker::classifier::typescript</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
