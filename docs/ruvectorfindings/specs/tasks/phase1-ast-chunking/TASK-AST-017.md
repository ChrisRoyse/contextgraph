# TASK-AST-017 — Implement C++ entity classifier (templates + namespaces + qualified_identifier)

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth. Any lower reference to `src/memory/ast_chunker/<submodule>.rs`, `dynamicjepa.rs`, compatibility shims, or "create new" scaffolding language is superseded by this audit note. Verify and harden the existing canonical files instead of creating parallel modules unless the repo audit proves a file is genuinely missing.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task (Rust source file, JSON evidence file, RocksDB CF, or ME-JEPA panel file), run the logic, then perform a separate readback from that source of truth. Print before/after system state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution, not only function return values.

```xml
<task_spec id="TASK-AST-017" version="1.0">
<metadata>
  <title>Implement C++ classifier — templates / namespaces / qualified_identifier / access specifiers</title>
  <status>audit-hardening</status>
  <layer>logic</layer>
  <sequence>17</sequence>
  <implements>
    <item ref="REQ-AST-03">EntityType for C++ kinds (class_specifier, namespace_definition, template_declaration, function_definition with qualified_identifier, operator_name, destructor_name)</item>
    <item ref="REQ-AST-12">C++ has no native test convention — never returns TestFunction</item>
    <item ref="EC-AST-04">Template definition split across header and impl file</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-010</task_ref>
    <task_ref>TASK-AST-016</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
C++ extends C with classes, namespaces, templates, and qualified identifiers
(e.g. `Foo::Bar::method()`). Reuses C classifier for shared kinds
(struct_specifier, enum_specifier, declaration with function declarator) and adds
class_specifier, namespace_definition, template_declaration. Methods defined out-of-line
(`void Foo::method() { ... }`) need qualified_identifier extraction to populate parent_chain.
Public/private access specifiers do NOT change EntityType (they're modifiers, not entities).
</context>

<input_context_files>
  <file purpose="c_classifier">crates/context-graph-core/src/memory/ast/classify.rs (TASK-AST-016 — reused for shared kinds)</file>
  <file purpose="entity_type">crates/context-graph-core/src/memory/ast/mod.rs</file>
  <file purpose="reference_classifier">crates/context-graph-cli/src/commands/dynamicjepa.rs (search "cpp" / "class_specifier" near line 6400)</file>
  <file purpose="node_kind_matrix">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (PerLanguageNodeKindTable.cpp)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-002 + TASK-AST-010 + TASK-AST-016 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Module: `crates/context-graph-core/src/memory/ast/classify.rs`
    - `pub fn hooks() -> LanguageHooks`
    - classify:
      class_specifier → Class
      namespace_definition → Namespace
      template_declaration → recurse to inner declaration; the inner determines EntityType
        (e.g. template<class T> class Foo {}; → Class; template<class T> void f() {} → Function)
      function_definition with qualified_identifier name → Method (out-of-line method definition)
      function_definition without qualified name → Function (free function) OR Method if
        ctx.inside_class
      declaration with function_declarator → Function (prototype, like C)
      struct_specifier / enum_specifier / union_specifier → delegate to c::classify
      preproc_include → Import
      using_declaration | using_directive | namespace_alias_definition → Import
      comment → CommentBlock
      translation_unit → Module
    - is_chunkable: c::is_chunkable + class_specifier, namespace_definition, template_declaration
    - get_node_name: function_definition with qualified_identifier — emit the FULL qualified
      name as parent_chain components ("Foo::Bar"); innermost identifier is the method name.
      For other kinds, follow standard `.name` field
    - is_container: class_specifier, namespace_definition
    - operator_name (e.g. `operator+`, `operator()`) is a name-like node — supported by treating
      it as the function's name node and extracting source bytes
    - destructor_name (e.g. `~Foo`) similarly handled
    - TestFunction: NEVER (deferred per doc 09 §3.1)
    - Unit tests: ≥10 covering: class definition, namespace, template class, template function,
      out-of-line method (`void Foo::m() {}`), operator overload, destructor, private/public
      access specifier irrelevance, struct (delegate to C), preproc_include
  </in_scope>
  <out_of_scope>
    - C++20 modules (`export module foo;`) — out of scope for Phase 1a
    - Concepts (`concept C = ...`) — out of scope
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast/classify.rs">
      use tree_sitter::Node;
      use crate::memory::ast::{ClassificationContext, EntityType, LanguageHooks};
      use super::c;

      pub fn hooks() -> LanguageHooks { /* ... */ }
      pub fn classify(node: &Node<'_>, source: &[u8], ctx: &ClassificationContext) -> EntityType { /* ... */ }
      pub fn is_chunkable(node: &Node<'_>) -> bool { /* ... */ }
      pub fn get_node_name(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
      pub fn is_container(node: &Node<'_>) -> bool { /* ... */ }

      fn function_def_is_qualified_method(node: &Node<'_>) -> bool { /* ... */ }
      fn extract_qualified_chain(declarator: &Node<'_>, source: &[u8]) -> Option<(Vec<String>, String)> { /* (chain, method_name) */ }
    </signature>
  </signatures>

  <constraints>
    - template_declaration unwrap: walk children for class_specifier / function_definition /
      struct_specifier; recurse classify on the inner; preserves the templated entity's type
    - operator_name and destructor_name are name-like — extract source bytes verbatim
    - access_specifier (public:/private:/protected:) is NOT an entity — never reaches classify
      because it's filtered by is_chunkable returning false
    - File under 500 lines; this file should be < 450 lines incl tests
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast::classifier::cpp` ≥10 tests pass
    - `class C { void m(); };` → Class; method m is a declaration (NOT chunkable inline)
    - `void C::m() { ... }` → Method (out-of-line; parent_chain includes "C")
    - `template<class T> class V {};` → Class
    - `template<class T> T id(T x) { return x; }` → Function
    - `int operator+(const A& a, const A& b) { ... }` → Function (operator name extracted)
    - `~Foo() {}` → Method (destructor)
    - `namespace N { void f() {} }` → Namespace; f is Function (or Method if ctx.inside_class)
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
classifier/cpp.rs:
  use crate::memory::ast::classifier::c;

  pub fn classify(node, source, ctx) -> EntityType {
      match node.kind() {
          "class_specifier" => EntityType::Class,
          "namespace_definition" => EntityType::Namespace,
          "template_declaration" => {
              let mut cursor = node.walk();
              for child in node.children(&mut cursor) {
                  if matches!(child.kind(), "class_specifier" | "function_definition"
                      | "struct_specifier" | "declaration") {
                      return classify(&child, source, ctx);
                  }
              }
              EntityType::Function
          }
          "function_definition" => {
              if function_def_is_qualified_method(node) { EntityType::Method }
              else if ctx.inside_class { EntityType::Method }
              else { EntityType::Function }
          }
          "struct_specifier" | "enum_specifier" | "union_specifier"
              | "preproc_include" | "preproc_def" | "preproc_function_def"
              | "comment" | "translation_unit" => c::classify(node, source, ctx),
          "declaration" => c::classify(node, source, ctx),
          "using_declaration" | "using_directive" | "namespace_alias_definition" => EntityType::Import,
          _ => EntityType::Function,
      }
  }

  fn function_def_is_qualified_method(node: &Node) -> bool {
      // walk declarator chain for qualified_identifier
      let declarator = node.child_by_field_name("declarator");
      if let Some(d) = declarator {
          return contains_qualified_id(&d, 0);
      }
      false
  }

  fn contains_qualified_id(d: &Node, depth: u32) -> bool {
      if depth > 8 { return false; }
      if d.kind() == "qualified_identifier" { return true; }
      let mut cursor = d.walk();
      for child in d.children(&mut cursor) {
          if contains_qualified_id(&child, depth + 1) { return true; }
      }
      false
  }

  // 10+ tests
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast/classify.rs">C++ classifier + qualified_identifier walk + template unwrap + 10 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast/classify.rs">Add `pub mod cpp;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>class_specifier → Class</criterion>
  <criterion>namespace_definition → Namespace</criterion>
  <criterion>template_declaration unwraps to inner entity (Class for templated class, Function for templated function)</criterion>
  <criterion>function_definition with qualified_identifier (`void C::m()`) → Method</criterion>
  <criterion>operator_name and destructor_name extracted as function names</criterion>
  <criterion>using_declaration/using_directive → Import</criterion>
  <criterion>Shared kinds (struct/enum/preproc) delegate to C classifier</criterion>
  <criterion>NO TestFunction emitted for C++</criterion>
  <criterion>≥10 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast::classifier::cpp</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
