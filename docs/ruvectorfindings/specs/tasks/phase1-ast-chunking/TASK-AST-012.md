# TASK-AST-012 — Implement JavaScript entity classifier

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth. Any lower reference to `src/memory/ast_chunker/<submodule>.rs`, `dynamicjepa.rs`, compatibility shims, or "create new" scaffolding language is superseded by this audit note. Verify and harden the existing canonical files instead of creating parallel modules unless the repo audit proves a file is genuinely missing.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task (Rust source file, JSON evidence file, RocksDB CF, or ME-JEPA panel file), run the logic, then perform a separate readback from that source of truth. Print before/after system state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution, not only function return values.

```xml
<task_spec id="TASK-AST-012" version="1.0">
<metadata>
  <title>Implement JavaScript entity classifier — function expressions / arrows / method_definition</title>
  <status>audit-hardening</status>
  <layer>logic</layer>
  <sequence>12</sequence>
  <implements>
    <item ref="REQ-AST-03">EntityType for JS node kinds</item>
    <item ref="REQ-AST-12">JavaScript TestFunction detection (it/test/describe call expressions)</item>
    <item ref="EC-AST-03">Arrow function bound to const → resolved name from binding site</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
JavaScript is harder than Python because:
1. functions can be defined as `function f() {}`, `const f = function() {}`, `const f = () => {}`,
   `class C { f() {} }`, generator `function* g()` — the binding-site name resolution is non-trivial
2. tests use call expressions: `it("...", () => { ... })`, `test("...", fn)`, `describe(...)` —
   the test entity is the lambda passed to the call, NOT the call itself
3. JSDoc `/** ... */` comments preceding functions are Docstring (not CommentBlock)
</context>

<input_context_files>
  <file purpose="entity_type">crates/context-graph-core/src/memory/ast/mod.rs (TASK-AST-002)</file>
  <file purpose="boundary_hooks">crates/context-graph-core/src/memory/ast/mod.rs (LanguageHooks struct)</file>
  <file purpose="reference_classifier">crates/context-graph-cli/src/commands/dynamicjepa.rs (search for "javascript" / "function_declaration" near line 6400)</file>
  <file purpose="node_kind_matrix">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (PerLanguageNodeKindTable.javascript)</file>
  <file purpose="testfn_rules">docs/ruvectorfindings/specs/functional/SPEC-AST-CHUNKING.md (REQ-AST-12 JS rules)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-002 + TASK-AST-010 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Module: `crates/context-graph-core/src/memory/ast/classify.rs`
    - `pub fn hooks() -> LanguageHooks`
    - classify dispatch:
      function_declaration | function_expression | generator_function_declaration | arrow_function
        → Method if ctx.inside_class else Function (override TestFunction per below)
      method_definition → Method
      class_declaration | class_expression → Class
      import_statement → Import
      comment | block_comment → Docstring if JSDoc (`/**`-prefix); else CommentBlock
      program → Module
    - TestFunction detection: when classifying a `call_expression`, if callee identifier is in
      {it, test, describe, before, after, beforeEach, afterEach, suite, fixture}, look at the
      arguments — the last argument that is an arrow_function / function_expression is the
      test body; emit it as TestFunction
    - Arrow-function-bound-to-const naming (EC-AST-03): when `arrow_function` is the right-hand
      side of `lexical_declaration`/`variable_declarator`, use the LHS identifier as the name
      (binding-site resolution)
    - is_chunkable: function_declaration, function_expression, arrow_function, method_definition,
      class_declaration, class_expression, generator_function_declaration, program
    - get_node_name: function_declaration.name; method_definition.name (property_identifier);
      for arrow_function/function_expression — walk up to lexical_declaration/variable_declarator
      and read the LHS identifier
    - is_container: class_declaration, class_expression
    - Unit tests: ≥10 tests covering function/arrow/method/class/it()/describe()/JSDoc
  </in_scope>
  <out_of_scope>
    - TypeScript-specific kinds (TASK-AST-013)
    - JSX/TSX (TASK-AST-013)
    - HTML+JS (REQ-AST-11; TASK-AST-009 detector handles)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast/classify.rs">
      use tree_sitter::Node;
      use crate::memory::ast::{ClassificationContext, EntityType, LanguageHooks};

      pub fn hooks() -> LanguageHooks { /* ... */ }
      pub fn classify(node: &Node<'_>, source: &[u8], ctx: &ClassificationContext) -> EntityType { /* ... */ }
      pub fn is_chunkable(node: &Node<'_>) -> bool { /* ... */ }
      pub fn get_node_name(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
      pub fn is_container(node: &Node<'_>) -> bool { /* ... */ }

      pub(crate) const TEST_DSL_NAMES: &[&str] = &[
          "it", "test", "describe", "before", "after",
          "beforeEach", "afterEach", "suite", "fixture",
      ];

      pub(crate) fn resolve_arrow_name_from_binding(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
      pub(crate) fn is_jsdoc_comment(node: &Node<'_>, source: &[u8]) -> bool { /* /** prefix */ }
    </signature>
  </signatures>

  <constraints>
    - TEST_DSL_NAMES is `pub(crate)` — TASK-AST-013 (TypeScript) reuses it
    - resolve_arrow_name_from_binding walks node.parent() once, checks for variable_declarator,
      reads LHS identifier — bounded walk (no recursion)
    - is_jsdoc_comment: comment/block_comment node whose source bytes start with `/**`
    - JSDoc immediately preceding a function/class is a Docstring; standalone /** ... */ is CommentBlock
    - File under 500 lines; this file should be < 400 lines incl tests
    - GENERIC-ONLY: rules apply to ANY .js file, no per-framework branches beyond DSL detection
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast::classifier::javascript` ≥10 tests pass
    - Test fixtures: `function f() {}` (Function), `const g = () => 0` (Function with name "g"),
      `class C { m() {} }` (Class + Method), `it("x", () => {})` (TestFunction),
      `describe("x", () => {...})` body lambdas yield TestFunctions, `/** doc */ function f(){}` (Docstring + Function),
      `import x from 'y';` (Import)
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
classifier/javascript.rs:
  pub const TEST_DSL_NAMES: &[&str] = &["it", "test", "describe", "before", "after",
                                        "beforeEach", "afterEach", "suite", "fixture"];

  pub fn classify(node, source, ctx) -> EntityType {
      match node.kind() {
          "function_declaration" | "function_expression" | "generator_function_declaration" | "arrow_function" => {
              if is_test_callback_arg(node, source) { EntityType::TestFunction }
              else if ctx.inside_class { EntityType::Method }
              else { EntityType::Function }
          }
          "method_definition" => EntityType::Method,
          "class_declaration" | "class_expression" => EntityType::Class,
          "import_statement" => EntityType::Import,
          "comment" | "block_comment" => {
              if is_jsdoc_comment(node, source) && next_sibling_is_definition(node) {
                  EntityType::Docstring
              } else { EntityType::CommentBlock }
          }
          "program" => EntityType::Module,
          _ => EntityType::Function,
      }
  }

  fn is_test_callback_arg(node, source) -> bool {
      // walk up: parent must be `arguments`, grandparent `call_expression`, callee in TEST_DSL_NAMES
      let parent = node.parent()?;
      if parent.kind() != "arguments" { return false; }
      let grand = parent.parent()?;
      if grand.kind() != "call_expression" { return false; }
      let callee = grand.child_by_field_name("function")?;
      let name = String::from_utf8_lossy(&source[callee.start_byte()..callee.end_byte()]);
      TEST_DSL_NAMES.iter().any(|d| *d == name)
  }

  pub fn get_node_name(node, source) -> Option<String> {
      if let Some(name) = node.child_by_field_name("name") {
          return Some(String::from_utf8_lossy(&source[name.start_byte()..name.end_byte()]).into_owned());
      }
      // arrow_function or function_expression bound to const
      if matches!(node.kind(), "arrow_function" | "function_expression") {
          return resolve_arrow_name_from_binding(node, source);
      }
      None
  }

  pub fn resolve_arrow_name_from_binding(node, source) -> Option<String> {
      let parent = node.parent()?;
      if parent.kind() == "variable_declarator" {
          let lhs = parent.child_by_field_name("name")?;
          return Some(String::from_utf8_lossy(&source[lhs.start_byte()..lhs.end_byte()]).into_owned());
      }
      None
  }

  pub fn is_jsdoc_comment(node, source) -> bool {
      let bytes = &source[node.start_byte()..node.end_byte()];
      bytes.starts_with(b"/**")
  }

  // 10+ tests
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast/classify.rs">JS classifier hooks + TEST_DSL_NAMES + binding-site name resolution + 10 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast/classify.rs">Add `pub mod javascript;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>function_declaration → Function; method_definition → Method</criterion>
  <criterion>arrow_function bound to const has its binding-site name (EC-AST-03)</criterion>
  <criterion>it("...", fn) emits TestFunction for the callback fn; the call_expression itself is not chunkable</criterion>
  <criterion>JSDoc `/** ... */` preceding a function is Docstring; standalone is CommentBlock</criterion>
  <criterion>≥10 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast::classifier::javascript</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
