# TASK-AST-019 — Implement Ruby entity classifier (RSpec describe/it + Minitest test_*)

```xml
<task_spec id="TASK-AST-019" version="1.0">
<metadata>
  <title>Implement Ruby classifier — RSpec DSL + Minitest test_* detection</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>19</sequence>
  <implements>
    <item ref="REQ-AST-03">EntityType for Ruby kinds (method, singleton_method, class, module, alias, comment)</item>
    <item ref="REQ-AST-12">Ruby TestFunction detection (Minitest test_* prefix + RSpec it/describe/context)</item>
    <item ref="EC-AST-07">RSpec describe/it nesting — `it` block emitted as TestFunction with parent_chain including describe's string literal name</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
Ruby's RSpec DSL uses call expressions: `describe "Foo" do; it "does bar" do; ...; end; end`.
The `it` block body is the test entity (TestFunction); the `describe` block is a context
(emitted as Module so the constellation matrix doesn't waste a TestFunction cell on the outer
DSL). Minitest uses `def test_*` methods inside `class FooTest < Minitest::Test`. Singleton
methods (`def self.foo` or `class << self; def foo; end`) are emitted as Method.
</context>

<input_context_files>
  <file purpose="entity_type">crates/context-graph-core/src/memory/ast_chunker/entity_type.rs</file>
  <file purpose="boundary_hooks">crates/context-graph-core/src/memory/ast_chunker/boundary.rs</file>
  <file purpose="reference_classifier">crates/context-graph-cli/src/commands/dynamicjepa.rs (search "ruby" / "method" near line 6400)</file>
  <file purpose="node_kind_matrix">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (PerLanguageNodeKindTable.ruby)</file>
  <file purpose="testfn_rules">docs/ruvectorfindings/specs/functional/SPEC-AST-CHUNKING.md (REQ-AST-12 Ruby rule, EC-AST-07)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-002 + TASK-AST-010 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Module: `crates/context-graph-core/src/memory/ast_chunker/classifier/ruby.rs`
    - `pub fn hooks() -> LanguageHooks`
    - classify:
      method | singleton_method → TestFunction if name starts with `test_` AND ctx.inside_test_module
        OR ctx.parent_named_chain has "Test*" suffix; else Method (always — Ruby methods live in
        classes/modules)
      class → Class
      module → Module
      alias → Method (alias creates a method)
      block → TestFunction if its parent is a call_expression to `it` / `specify` / `example`;
        else NOT chunkable (regular blocks recurse into children)
      call (RSpec describe/context) → Module if callee is `describe` / `context` (these define
        a test context; contents recurse into); NOT chunkable as call itself
      comment → Docstring if `=begin ... =end` block comment preceding a definition; else CommentBlock
      program → Module
    - TestFunction detection rules:
      • method named `test_*` AND inside a class extending Minitest::Test (heuristic: ctx
        parent_named_chain has any name ending in "Test" or "TestCase")
      • call_expression block argument where callee is `it` / `specify` / `example` / `scenario`
      • Helpers `before` / `after` / `setup` / `teardown` blocks are NOT TestFunctions (per RSpec
        convention — these are setup/teardown, not test)
    - is_chunkable: method, singleton_method, class, module, alias, program, AND block when
      block's parent is RSpec call (handled by classify which gates chunkability via classify
      returning a chunkable EntityType — but is_chunkable signature only takes node, so use a
      heuristic: block whose parent kind is "call" + parent.method.identifier in RSPEC_DSL)
    - get_node_name: method.name; class.name; module.name (constant or scope_resolution)
    - is_container: class, module
    - RSpec block parent_chain example: `class Foo` (Class) → `describe "..." do` (Module) →
      `it "..." do` (TestFunction); parent_chain on the it-block chunk = `Foo::"describe ..."`
      (where "describe ..." is the string literal verbatim or trimmed to ~32 chars)
    - Unit tests: ≥10 covering: regular method (Method), test_foo in TestCase class (TestFunction),
      RSpec describe/it (Module + TestFunction), singleton method, alias, class extending another
      class, module
  </in_scope>
  <out_of_scope>
    - DSLs other than RSpec / Minitest (Cucumber, Spinach — out of scope)
    - eval'd code (untyped at chunker level)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/classifier/ruby.rs">
      use tree_sitter::Node;
      use crate::memory::ast_chunker::{ClassificationContext, EntityType, LanguageHooks};

      pub fn hooks() -> LanguageHooks { /* ... */ }
      pub fn classify(node: &Node<'_>, source: &[u8], ctx: &ClassificationContext) -> EntityType { /* ... */ }
      pub fn is_chunkable(node: &Node<'_>) -> bool { /* ... */ }
      pub fn get_node_name(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
      pub fn is_container(node: &Node<'_>) -> bool { /* ... */ }

      const RSPEC_TEST_DSL: &[&str] = &["it", "specify", "example", "scenario"];
      const RSPEC_CONTEXT_DSL: &[&str] = &["describe", "context", "feature", "shared_examples"];

      fn block_parent_dsl_name(block: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
      fn parent_class_is_test_case(ctx: &ClassificationContext) -> bool { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - block_parent_dsl_name: walk block.parent() for `call` / `method_call`; check the call's
      method name (`call.method.identifier`); return Some(name) iff name is in RSPEC_TEST_DSL
      or RSPEC_CONTEXT_DSL
    - parent_class_is_test_case: walk ctx.parent_named_chain; return true if any name ends in
      "Test" or "TestCase" or matches Ruby Minitest convention
    - File under 500 lines; this file should be < 450 lines incl tests
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast_chunker::classifier::ruby` ≥10 tests pass
    - `class FooTest < Minitest::Test\n  def test_foo\n    ...\n  end\nend` → method test_foo is TestFunction
    - `class Foo\n  def foo\n    ...\n  end\nend` → method foo is Method (not TestFunction; Foo is not a TestCase)
    - `describe "Calculator" do\n  it "adds" do\n    ...\n  end\nend` → it block is TestFunction; describe block is Module
    - `def self.foo; end` (singleton_method) → Method
    - `alias new_name old_name` → Method
    - `module M\n  def foo; end\nend` → M is Module; foo is Method
    - `before { ... }` block (RSpec setup) → NOT TestFunction (regular block, recurses)
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
classifier/ruby.rs:
  const RSPEC_TEST_DSL: &[&str] = &["it", "specify", "example", "scenario"];
  const RSPEC_CONTEXT_DSL: &[&str] = &["describe", "context", "feature", "shared_examples"];

  pub fn classify(node, source, ctx) -> EntityType {
      match node.kind() {
          "method" | "singleton_method" => {
              let name = get_node_name(node, source).unwrap_or_default();
              if name.starts_with("test_") && parent_class_is_test_case(ctx) {
                  EntityType::TestFunction
              } else {
                  EntityType::Method
              }
          }
          "class" => EntityType::Class,
          "module" => EntityType::Module,
          "alias" => EntityType::Method,
          "block" => {
              if let Some(dsl) = block_parent_dsl_name(node, source) {
                  if RSPEC_TEST_DSL.contains(&dsl.as_str()) {
                      return EntityType::TestFunction;
                  }
                  if RSPEC_CONTEXT_DSL.contains(&dsl.as_str()) {
                      return EntityType::Module;
                  }
              }
              EntityType::Method  // unattached block; not normally chunkable
          }
          "comment" => EntityType::CommentBlock,  // =begin/=end docstring detection optional
          "program" => EntityType::Module,
          _ => EntityType::Method,
      }
  }

  pub fn is_chunkable(node) -> bool {
      match node.kind() {
          "method" | "singleton_method" | "class" | "module" | "alias" | "program" => true,
          "block" => {
              // Heuristic: only chunk blocks attached to RSpec DSL calls
              // (caller's classify will refine; chunkability is permissive here)
              true
          }
          _ => false,
      }
  }

  // 10+ tests
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/ruby.rs">Ruby classifier + RSpec DSL detection + Minitest test_* detection + 10 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/mod.rs">Add `pub mod ruby;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>Minitest `def test_*` in class extending TestCase → TestFunction</criterion>
  <criterion>RSpec `it "name" do ... end` block → TestFunction</criterion>
  <criterion>RSpec `describe "name" do ... end` block → Module (not TestFunction)</criterion>
  <criterion>singleton_method → Method</criterion>
  <criterion>alias → Method</criterion>
  <criterion>regular block (e.g. `before { ... }`) is NOT TestFunction</criterion>
  <criterion>Production class → Class; module → Module</criterion>
  <criterion>≥10 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast_chunker::classifier::ruby</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
