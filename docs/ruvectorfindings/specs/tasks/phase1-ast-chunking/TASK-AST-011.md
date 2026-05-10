# TASK-AST-011 — Implement Python entity classifier

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth. Any lower reference to `src/memory/ast_chunker/<submodule>.rs`, `dynamicjepa.rs`, compatibility shims, or "create new" scaffolding language is superseded by this audit note. Verify and harden the existing canonical files instead of creating parallel modules unless the repo audit proves a file is genuinely missing.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task (Rust source file, JSON evidence file, RocksDB CF, or ME-JEPA panel file), run the logic, then perform a separate readback from that source of truth. Print before/after system state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution, not only function return values.

```xml
<task_spec id="TASK-AST-011" version="1.0">
<metadata>
  <title>Implement Python entity classifier — node-kind table + TestFunction detection</title>
  <status>audit-hardening</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements>
    <item ref="REQ-AST-03">EntityType variants per Python tree-sitter node kinds</item>
    <item ref="REQ-AST-12">Python TestFunction detection (def test_*, @pytest.fixture, @pytest.mark.*)</item>
    <item ref="BR-AST-04">Function/Method/TestFunction ambiguity → Function default</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
Python is the highest-priority extension after Rust — Phase 0 mutation corpus already
operates at Python-chunk granularity. Reuse per-language node classification logic from
`dynamicjepa.rs:6400+` semantic extractor (which already handles Python). The classifier
returns a `LanguageHooks` struct (per TASK-AST-007) configured for Python.
</context>

<input_context_files>
  <file purpose="entity_type">crates/context-graph-core/src/memory/ast/mod.rs (TASK-AST-002 output)</file>
  <file purpose="boundary_hooks">crates/context-graph-core/src/memory/ast/mod.rs (LanguageHooks struct, TASK-AST-007)</file>
  <file purpose="reference_classifier">crates/context-graph-cli/src/commands/dynamicjepa.rs (search for "python" or "function_definition" near line 6400 — reuse per-node-kind logic)</file>
  <file purpose="node_kind_matrix">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (PerLanguageNodeKindTable.python: function_definition, class_definition, decorated_definition, async_function_definition, import_statement, import_from_statement, expression_statement [docstring], comment)</file>
  <file purpose="testfn_rules">docs/ruvectorfindings/specs/functional/SPEC-AST-CHUNKING.md (REQ-AST-12 Python rules)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-002 complete: EntityType + ClassificationContext available</check>
  <check>TASK-AST-010 complete: parser_for_language(Language::Python) works</check>
</prerequisites>

<scope>
  <in_scope>
    - Module path: `crates/context-graph-core/src/memory/ast/classify.rs`
    - `pub fn hooks() -> LanguageHooks` returning Python-configured hooks
    - `pub fn classify(node, source, ctx) -> EntityType`:
      function_definition / async_function_definition → Method if ctx.inside_class else Function
        (override to TestFunction per rules below)
      class_definition → Class
      decorated_definition → recurse on the inner definition (preserve decorator-aware dispatch)
      import_statement / import_from_statement → Import
      expression_statement (first child = string, immediately following function/class) → Docstring
      comment → CommentBlock
      module → Module
      default for top-level chunkable nodes → Function (for unmatched function-shaped) or
        recurse-not-emit
    - `pub fn is_chunkable(node) -> bool`: function_definition, async_function_definition,
      class_definition, decorated_definition, module
    - `pub fn get_node_name(node, source) -> Option<String>`: extract name from
      function_definition.name, class_definition.name (via field-name lookup)
    - `pub fn is_container(node) -> bool`: class_definition (so methods get parent_def)
    - TestFunction detection rules:
      • function name starts with `test_` AND (ctx.inside_test_module OR class name starts with `Test`)
      • OR has decorator `@pytest.fixture`, `@pytest.mark.<anything>`
    - `ctx.inside_test_module` set externally by the boundary engine when scope chain enters a
      `class Test*` (handled here via `is_container` + name check; downstream walker reads it)
    - Unit tests: ≥10 tests covering: simple def, class with method, decorated test, pytest fixture,
      import variants, docstring detection, top-level function not in class/test
  </in_scope>
  <out_of_scope>
    - Other-language classifiers (TASK-AST-012..020)
    - Wiring into the chunker facade (TASK-AST-021)
    - Python-specific size policy (uses default AstChunkConfig)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast/classify.rs">
      use tree_sitter::Node;
      use crate::memory::ast::{ClassificationContext, EntityType, LanguageHooks};

      pub fn hooks() -> LanguageHooks {
          LanguageHooks {
              classify, is_chunkable, get_node_name, is_container,
          }
      }

      pub fn classify(node: &Node<'_>, source: &[u8], ctx: &ClassificationContext) -> EntityType { /* ... */ }
      pub fn is_chunkable(node: &Node<'_>) -> bool { /* ... */ }
      pub fn get_node_name(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
      pub fn is_container(node: &Node<'_>) -> bool { /* ... */ }

      fn has_test_decorator(node: &Node<'_>, source: &[u8]) -> bool { /* @pytest.fixture / @pytest.mark.* */ }
      fn function_name(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - classify is total — every node-kind that reaches classify returns SOME EntityType
      (default to Function for unmatched function-shaped nodes; the chunker is responsible
      for filtering non-emittable nodes via is_chunkable BEFORE calling classify)
    - TestFunction detection MUST default to Function on ambiguity (BR-AST-04)
    - get_node_name extracts identifier text from `name: identifier` field of function/class node
    - decorated_definition: recurse to the inner node's classify (treat decorators as decoration,
      not as a separate entity)
    - File under 500 lines; this file should be < 350 lines incl. tests
    - GENERIC-ONLY constraint (constitution): no per-repo branches; rules apply to ALL Python source
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast::classifier::python` ≥10 tests pass
    - Test fixtures cover: `def f():` (Function), `class C: def m(): ...` (Method),
      `def test_foo():` in test module (TestFunction), `@pytest.fixture\ndef f():` (TestFunction),
      `from x import y` (Import), `class C:\n    """doc"""` (Docstring), `# comment` (CommentBlock)
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
classifier/python.rs:
  use tree_sitter::Node;
  use crate::memory::ast::{ClassificationContext, EntityType, LanguageHooks};

  pub fn hooks() -> LanguageHooks { LanguageHooks { classify, is_chunkable, get_node_name, is_container } }

  pub fn classify(node: &Node, source: &[u8], ctx: &ClassificationContext) -> EntityType {
      match node.kind() {
          "function_definition" | "async_function_definition" => {
              let is_test = is_test_function(node, source, ctx);
              if is_test { EntityType::TestFunction }
              else if ctx.inside_class { EntityType::Method }
              else { EntityType::Function }
          }
          "class_definition" => EntityType::Class,
          "decorated_definition" => {
              // Recurse on inner definition node
              let mut cursor = node.walk();
              for child in node.children(&mut cursor) {
                  if matches!(child.kind(), "function_definition" | "async_function_definition" | "class_definition") {
                      return classify(&child, source, ctx);
                  }
              }
              EntityType::Function
          }
          "import_statement" | "import_from_statement" => EntityType::Import,
          "expression_statement" if first_child_is_string_after_def(node) => EntityType::Docstring,
          "comment" => EntityType::CommentBlock,
          "module" => EntityType::Module,
          _ => EntityType::Function,
      }
  }

  fn is_test_function(node: &Node, source: &[u8], ctx: &ClassificationContext) -> bool {
      let name = function_name(node, source).unwrap_or_default();
      let prefix_match = name.starts_with("test_");
      let in_test_class = ctx.parent_named_chain.iter().any(|n| n.starts_with("Test"));
      let has_decorator = ctx.parent_named_chain.is_empty() && has_test_decorator(node, source);
      // BR-AST-04: ambiguity defaults to Function. So test_* alone is NOT enough — must be in
      // test module OR class OR have decorator.
      (prefix_match && (ctx.inside_test_module || in_test_class)) || has_decorator
  }

  fn has_test_decorator(node: &Node, source: &[u8]) -> bool {
      // walk up to parent decorated_definition; check decorator children for `@pytest.fixture`,
      // `@pytest.mark.<id>`
      // (implementation walks node.parent() once; if decorated_definition, scan its decorator list)
      false  // detailed in implementation
  }

  pub fn is_chunkable(node: &Node) -> bool {
      matches!(node.kind(), "function_definition" | "async_function_definition"
                          | "class_definition" | "decorated_definition" | "module")
  }

  pub fn get_node_name(node: &Node, source: &[u8]) -> Option<String> {
      // Use `node.child_by_field_name("name")` for function_definition, class_definition
      let name_node = node.child_by_field_name("name")?;
      let bytes = &source[name_node.start_byte()..name_node.end_byte()];
      Some(String::from_utf8_lossy(bytes).into_owned())
  }

  pub fn is_container(node: &Node) -> bool {
      matches!(node.kind(), "class_definition")
  }

  // 10+ tests using parser_for_language(Language::Python)
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast/classify.rs">Python classifier hooks + 10 unit tests</file>
  <file path="crates/context-graph-core/src/memory/ast/classify.rs">Re-export module: `pub mod python;` (other-language classifier modules added in TASK-AST-012..020)</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast/mod.rs">Add `pub mod classifier;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>classify returns the correct EntityType for all 8 documented Python node kinds</criterion>
  <criterion>def test_* in test module returns TestFunction</criterion>
  <criterion>def test_* outside test module/class returns Function (BR-AST-04 default)</criterion>
  <criterion>@pytest.fixture decorated def returns TestFunction</criterion>
  <criterion>class method gets EntityType::Method (not Function) when ctx.inside_class</criterion>
  <criterion>Docstring detection works (first string of class/function body)</criterion>
  <criterion>Import statement returns EntityType::Import</criterion>
  <criterion>get_node_name extracts the function/class name from the AST</criterion>
  <criterion>≥10 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast::classifier::python</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
