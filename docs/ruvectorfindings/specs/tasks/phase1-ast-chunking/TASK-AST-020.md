# TASK-AST-020 — Implement PHP entity classifier (namespace_use_declaration)

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth. Any lower reference to `src/memory/ast_chunker/<submodule>.rs`, `dynamicjepa.rs`, compatibility shims, or "create new" scaffolding language is superseded by this audit note. Verify and harden the existing canonical files instead of creating parallel modules unless the repo audit proves a file is genuinely missing.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task (Rust source file, JSON evidence file, RocksDB CF, or ME-JEPA panel file), run the logic, then perform a separate readback from that source of truth. Print before/after system state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution, not only function return values.

```xml
<task_spec id="TASK-AST-020" version="1.0">
<metadata>
  <title>Implement PHP classifier — namespace_use_declaration + PHPUnit @test detection</title>
  <status>audit-hardening</status>
  <layer>surface</layer>
  <sequence>20</sequence>
  <implements>
    <item ref="REQ-AST-03">EntityType for PHP kinds (function_definition, method_declaration, class_declaration, interface_declaration, trait_declaration, namespace_definition, namespace_use_declaration)</item>
    <item ref="REQ-AST-12">PHP TestFunction detection (PHPUnit test* prefix in TestCase + @test annotation)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
PHP has separate namespace_definition (declaring a namespace) and namespace_use_declaration
(`use` statement importing a namespace symbol — analogous to import_statement). PHPUnit tests
are detected by method name `test*` AND class extending TestCase (or PHPDoc `@test` annotation).
PHP traits (`trait_declaration`) are mixin-like — emitted as TraitOrInterface (BR-AST-03).
Mixed PHP+HTML files are rejected at TASK-AST-009 detector level; this classifier sees
pure-PHP source only.
</context>

<input_context_files>
  <file purpose="entity_type">crates/context-graph-core/src/memory/ast/mod.rs</file>
  <file purpose="boundary_hooks">crates/context-graph-core/src/memory/ast/mod.rs</file>
  <file purpose="reference_classifier">crates/context-graph-cli/src/commands/dynamicjepa.rs (search "php" / "function_definition" near line 6400)</file>
  <file purpose="node_kind_matrix">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (PerLanguageNodeKindTable.php)</file>
  <file purpose="testfn_rules">docs/ruvectorfindings/specs/functional/SPEC-AST-CHUNKING.md (REQ-AST-12 PHP rule)</file>
  <file purpose="mixed_lang_detector">crates/context-graph-core/src/memory/ast/mod.rs (TASK-AST-009; rejects PHP+HTML upstream)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-002 + TASK-AST-010 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Module: `crates/context-graph-core/src/memory/ast/classify.rs`
    - `pub fn hooks() -> LanguageHooks`
    - classify:
      function_definition → Function (top-level free function)
      method_declaration → TestFunction if name starts with `test` AND
        (ctx.parent_named_chain has class extending TestCase OR PHPDoc preceding has `@test`);
        else Method
      class_declaration → Class
      interface_declaration → TraitOrInterface
      trait_declaration → TraitOrInterface (BR-AST-03 — PHP traits map to TraitOrInterface)
      namespace_definition → Namespace
      namespace_use_declaration → Import
      comment → Docstring if PHPDoc (`/** ... */`) preceding a definition; else CommentBlock
      program → Module
    - TestFunction detection:
      • method name starts with `test` (lowercase) AND class extends `TestCase` (heuristic via
        ctx.parent_named_chain — last entry that is a Class with name containing "Test" OR
        the chunker walker tracks `extends TestCase` clause and sets ctx flag — for Phase 1a,
        accept either heuristic)
      • OR PHPDoc preceding the method contains `@test` (substring match in comment text)
    - is_chunkable: function_definition, method_declaration, class_declaration,
      interface_declaration, trait_declaration, namespace_definition, namespace_use_declaration,
      program
    - get_node_name: function_definition.name; method_declaration.name; class_declaration.name; etc.
    - is_container: class_declaration, interface_declaration, trait_declaration,
      namespace_definition
    - Unit tests: ≥10 covering: function, class with method, test method in TestCase,
      @test annotated method, interface, trait, namespace, namespace_use, PHPDoc on method
  </in_scope>
  <out_of_scope>
    - PHP+HTML mixed files (handled by TASK-AST-009 detector)
    - Anonymous classes (rare; emitted as Class regardless)
    - Pre-PHP-7 syntax variants
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

      fn is_phpunit_test_method(method: &Node<'_>, source: &[u8], ctx: &ClassificationContext) -> bool { /* ... */ }
      fn is_phpdoc_comment(node: &Node<'_>, source: &[u8]) -> bool { /* /** prefix */ }
      fn phpdoc_has_test_annotation(comment: &Node<'_>, source: &[u8]) -> bool { /* @test */ }
  </signature>
  </signatures>

  <constraints>
    - is_phpunit_test_method:
      1. name starts with `test` (case-sensitive lowercase t)
      2. class context: walk ctx.parent_named_chain for any name == "TestCase" OR ending in "Test"
         OR class declaration node has `extends_clause` with name containing "TestCase"
      3. OR previous-sibling comment is PHPDoc containing `@test` annotation (substring search
         after the `*` prefix on a line)
    - PHP trait_declaration → TraitOrInterface (BR-AST-03)
    - File under 500 lines; this file should be < 400 lines incl tests
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast::classifier::php` ≥10 tests pass
    - `function foo() {}` → Function
    - `class FooTest extends TestCase { public function testBar() {} }` → testBar is TestFunction
    - `class Foo { public function testBar() {} }` (Foo is not a TestCase) → testBar is Method
    - `class Foo { /** @test */ public function bar() {} }` → bar is TestFunction (annotation)
    - `interface I {}` → TraitOrInterface
    - `trait T {}` → TraitOrInterface
    - `namespace App\\Foo;` → Namespace
    - `use App\\Foo\\Bar;` → Import
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
classifier/php.rs:
  pub fn classify(node, source, ctx) -> EntityType {
      match node.kind() {
          "function_definition" => EntityType::Function,
          "method_declaration" => {
              if is_phpunit_test_method(node, source, ctx) { EntityType::TestFunction }
              else { EntityType::Method }
          }
          "class_declaration" => EntityType::Class,
          "interface_declaration" => EntityType::TraitOrInterface,
          "trait_declaration" => EntityType::TraitOrInterface,
          "namespace_definition" => EntityType::Namespace,
          "namespace_use_declaration" => EntityType::Import,
          "comment" => {
              if is_phpdoc_comment(node, source) && next_sibling_is_definition(node) {
                  EntityType::Docstring
              } else { EntityType::CommentBlock }
          }
          "program" => EntityType::Module,
          _ => EntityType::Function,
      }
  }

  fn is_phpunit_test_method(method, source, ctx) -> bool {
      let name = match method.child_by_field_name("name") {
          Some(n) => String::from_utf8_lossy(&source[n.start_byte()..n.end_byte()]).into_owned(),
          None => return false,
      };
      let prefix_match = name.starts_with("test");
      // class context heuristic
      let in_test_class = ctx.parent_named_chain.iter().any(|n|
          n == "TestCase" || n.ends_with("Test") || n.ends_with("TestCase"));
      let has_annot = preceding_phpdoc_has_test(method, source);
      (prefix_match && in_test_class) || has_annot
  }

  // 10+ tests
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast/classify.rs">PHP classifier + PHPUnit detection + 10 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast/classify.rs">Add `pub mod php;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>function_definition → Function</criterion>
  <criterion>method_declaration with name `test*` in class extending TestCase → TestFunction</criterion>
  <criterion>method_declaration with @test PHPDoc annotation → TestFunction</criterion>
  <criterion>method_declaration `test*` outside TestCase → Method (BR-AST-04)</criterion>
  <criterion>trait_declaration → TraitOrInterface (BR-AST-03)</criterion>
  <criterion>namespace_definition → Namespace</criterion>
  <criterion>namespace_use_declaration → Import</criterion>
  <criterion>≥10 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast::classifier::php</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
