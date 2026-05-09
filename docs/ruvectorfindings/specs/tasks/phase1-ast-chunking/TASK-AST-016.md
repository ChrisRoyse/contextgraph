# TASK-AST-016 — Implement C entity classifier (declarator-aware function_definition)

```xml
<task_spec id="TASK-AST-016" version="1.0">
<metadata>
  <title>Implement C classifier — declarator-aware function_definition vs prototype</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>16</sequence>
  <implements>
    <item ref="REQ-AST-03">EntityType for C kinds (function_definition, declaration with function declarator, struct_specifier, enum_specifier, union_specifier, preproc_def, preproc_include)</item>
    <item ref="REQ-AST-12">C has no native test convention — never returns TestFunction</item>
    <item ref="BR-AST-03">union_specifier maps to Struct (closest fit)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
C is awkward because tree-sitter-c emits `declaration` for both variable declarations
(`int x;`) AND function prototypes (`int f(int x);`). To distinguish, we walk the
declaration's declarator: if it contains a `function_declarator`, it's a function prototype
(emit Function); else it's a variable, NOT chunkable. function_definition (with body)
unconditionally emits Function. C has no class/interface concepts; struct/enum/union map
to Struct/Enum/Struct per BR-AST-03.
</context>

<input_context_files>
  <file purpose="entity_type">crates/context-graph-core/src/memory/ast_chunker/entity_type.rs</file>
  <file purpose="boundary_hooks">crates/context-graph-core/src/memory/ast_chunker/boundary.rs</file>
  <file purpose="reference_classifier">crates/context-graph-cli/src/commands/dynamicjepa.rs (search "c" language / "function_definition" near line 6400)</file>
  <file purpose="node_kind_matrix">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (PerLanguageNodeKindTable.c — declarator-aware extraction)</file>
  <file purpose="declarator_walk">https://github.com/tree-sitter/tree-sitter-c (function_declarator nesting via pointer_declarator/parenthesized_declarator)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-002 + TASK-AST-010 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Module: `crates/context-graph-core/src/memory/ast_chunker/classifier/c.rs`
    - `pub fn hooks() -> LanguageHooks`
    - classify:
      function_definition → Function (always — has body)
      declaration → Function IFF its declarator (recursing through pointer_declarator,
        parenthesized_declarator) terminates in function_declarator; else NOT chunkable
      struct_specifier → Struct
      enum_specifier → Enum
      union_specifier → Struct (BR-AST-03)
      preproc_def | preproc_function_def → Function (treat #define functions as Function)
      preproc_include → Import
      comment → CommentBlock (C has no docstring convention; multi-line `/** ... */` is just a
        comment unless used by a tool — out of scope)
      translation_unit → Module
    - TestFunction: NEVER returned for C (deferred per doc 09 §3.1; future Catch2 / Unity could
      detect macros)
    - is_chunkable: function_definition, struct_specifier, enum_specifier, union_specifier,
      preproc_def, preproc_function_def, translation_unit, AND declaration ONLY when
      declarator-aware check identifies a function prototype
    - Helper: `pub(crate) fn declaration_is_function_prototype(decl: &Node, source: &[u8]) -> bool`
      that walks declarator children through pointer_declarator and parenthesized_declarator
      to find function_declarator
    - get_node_name: function_definition uses declarator.declarator.identifier (innermost
      identifier after walking through pointer/parenthesized wrappers); struct_specifier.name;
      etc.
    - is_container: struct_specifier, union_specifier (so methods aren't a thing in C, but
      nested type declarations get a parent_def)
    - Unit tests: ≥10 covering: function_definition, function prototype, variable declaration
      (NOT chunkable), struct/enum/union, preproc_include, comment, declarator-walk depth
      (e.g. `int *(*const f)(int)` — function pointer to function returning int*)
  </in_scope>
  <out_of_scope>
    - C++ kinds (TASK-AST-017)
    - Macro expansion (treated as opaque — chunker does not preprocess)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/classifier/c.rs">
      use tree_sitter::Node;
      use crate::memory::ast_chunker::{ClassificationContext, EntityType, LanguageHooks};

      pub fn hooks() -> LanguageHooks { /* ... */ }
      pub fn classify(node: &Node<'_>, source: &[u8], ctx: &ClassificationContext) -> EntityType { /* ... */ }
      pub fn is_chunkable(node: &Node<'_>) -> bool { /* ... */ }
      pub fn get_node_name(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
      pub fn is_container(node: &Node<'_>) -> bool { /* ... */ }

      pub(crate) fn declaration_is_function_prototype(decl: &Node<'_>, source: &[u8]) -> bool { /* ... */ }
      pub(crate) fn innermost_declarator_name(decl: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - declaration_is_function_prototype: walk node.children() looking for declarator;
      for each declarator, recursively check `pointer_declarator.declarator`, `parenthesized_declarator.declarator`,
      and `init_declarator.declarator`; if the innermost reached node has kind `function_declarator`,
      return true. Bounded recursion (depth ≤ 8).
    - innermost_declarator_name: similar walk but extracting the innermost `identifier` /
      `type_identifier` text
    - C language has no `package_declaration`-equivalent; translation_unit IS the Module
    - File under 500 lines; this file should be < 400 lines incl tests
    - declaration_is_function_prototype is `pub(crate)` — TASK-AST-017 (C++) reuses it
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast_chunker::classifier::c` ≥10 tests pass
    - `int f(int x) { return x; }` → Function
    - `int f(int x);` (prototype) → Function
    - `int x = 0;` (variable decl) → NOT chunkable
    - `struct S { int x; };` → Struct
    - `enum E { A, B };` → Enum
    - `union U { int a; float b; };` → Struct (BR-AST-03)
    - `#include <stdio.h>` → Import
    - `#define FOO(x) (x)` → Function (preproc_function_def)
    - `int *(*const f)(int);` → Function (declarator-walk through pointer_declarator)
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
classifier/c.rs:
  pub fn classify(node, source, ctx) -> EntityType {
      match node.kind() {
          "function_definition" => EntityType::Function,
          "declaration" => {
              if declaration_is_function_prototype(node, source) { EntityType::Function }
              else { EntityType::Function /* still chunkable if matched is_chunkable; otherwise filtered earlier */ }
          }
          "struct_specifier" => EntityType::Struct,
          "enum_specifier" => EntityType::Enum,
          "union_specifier" => EntityType::Struct,
          "preproc_def" | "preproc_function_def" => EntityType::Function,
          "preproc_include" => EntityType::Import,
          "comment" => EntityType::CommentBlock,
          "translation_unit" => EntityType::Module,
          _ => EntityType::Function,
      }
  }

  pub fn is_chunkable(node) -> bool {
      match node.kind() {
          "function_definition" | "struct_specifier" | "enum_specifier" | "union_specifier"
              | "preproc_def" | "preproc_function_def" | "translation_unit" => true,
          "declaration" => {
              // Need to inspect — chunkable only if function prototype
              // (caller passes source-aware context; for hooks_fn signature, declaration
              //  is always a candidate, classify+chunkability double-check)
              // Conservative: return false for declaration; only function_definition emits.
              // declaration without body = forward declaration that should NOT be a chunk
              // (forward decls don't add semantic meaning beyond the definition)
              false
          }
          _ => false,
      }
  }

  pub fn declaration_is_function_prototype(decl: &Node, source: &[u8]) -> bool {
      let mut cursor = decl.walk();
      for child in decl.children(&mut cursor) {
          if walk_declarator_for_function(&child, 0) { return true; }
      }
      false
  }

  fn walk_declarator_for_function(d: &Node, depth: u32) -> bool {
      if depth > 8 { return false; }
      if d.kind() == "function_declarator" { return true; }
      if let Some(inner) = d.child_by_field_name("declarator") {
          return walk_declarator_for_function(&inner, depth + 1);
      }
      false
  }

  // 10+ tests
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/c.rs">C classifier + declarator-walk helpers + 10 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/mod.rs">Add `pub mod c;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>function_definition → Function</criterion>
  <criterion>declaration with function_declarator → Function (chunkable; no body but prototype is a contract)</criterion>
  <criterion>declaration with variable declarator → NOT chunkable (skipped at is_chunkable level)</criterion>
  <criterion>struct_specifier / union_specifier → Struct; enum_specifier → Enum</criterion>
  <criterion>preproc_include → Import; preproc_def → Function</criterion>
  <criterion>declarator walk handles pointer_declarator + parenthesized_declarator (depth ≤ 8 bound)</criterion>
  <criterion>NO TestFunction emitted for C</criterion>
  <criterion>≥10 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast_chunker::classifier::c</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
