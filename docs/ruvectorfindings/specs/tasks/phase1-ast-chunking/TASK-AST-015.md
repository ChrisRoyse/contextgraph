# TASK-AST-015 — Implement Java entity classifier

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth. Any lower reference to `src/memory/ast_chunker/<submodule>.rs`, `dynamicjepa.rs`, compatibility shims, or "create new" scaffolding language is superseded by this audit note. Verify and harden the existing canonical files instead of creating parallel modules unless the repo audit proves a file is genuinely missing.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task (Rust source file, JSON evidence file, RocksDB CF, or ME-JEPA panel file), run the logic, then perform a separate readback from that source of truth. Print before/after system state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution, not only function return values.

```xml
<task_spec id="TASK-AST-015" version="1.0">
<metadata>
  <title>Implement Java classifier — @Test/@ParameterizedTest/@RepeatedTest detection + Maven/Gradle dirs</title>
  <status>audit-hardening</status>
  <layer>logic</layer>
  <sequence>15</sequence>
  <implements>
    <item ref="REQ-AST-03">EntityType for Java kinds (method_declaration, class_declaration, interface_declaration, enum_declaration, record_declaration, package_declaration, import_declaration)</item>
    <item ref="REQ-AST-12">Java TestFunction detection (JUnit 5 annotations)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
Java tests are detected by the JUnit 5 annotation set (@Test, @ParameterizedTest, @RepeatedTest,
@TestFactory, @TestTemplate) on a method_declaration. Records (Java 14+) are emitted as Class
per BR-AST-03 (closest semantic match). Javadoc `/** ... */` preceding a class/method is a Docstring;
standalone is CommentBlock. Maven/Gradle source directory layout (`src/test/java/...`) is a
file-path hint that the chunker does NOT use directly — that's the caller's responsibility.
</context>

<input_context_files>
  <file purpose="entity_type">crates/context-graph-core/src/memory/ast/mod.rs</file>
  <file purpose="boundary_hooks">crates/context-graph-core/src/memory/ast/mod.rs</file>
  <file purpose="reference_classifier">crates/context-graph-cli/src/commands/dynamicjepa.rs (search "java" / "method_declaration" near line 6400)</file>
  <file purpose="node_kind_matrix">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (PerLanguageNodeKindTable.java)</file>
  <file purpose="testfn_rules">docs/ruvectorfindings/specs/functional/SPEC-AST-CHUNKING.md (REQ-AST-12 Java rule)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-002 + TASK-AST-010 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Module: `crates/context-graph-core/src/memory/ast/classify.rs`
    - `pub fn hooks() -> LanguageHooks`
    - classify:
      method_declaration → TestFunction if has JUnit annotation; else Method (always — Java methods
        live in classes/interfaces/records by definition)
      class_declaration → Class
      interface_declaration | annotation_type_declaration → TraitOrInterface
      enum_declaration → Enum
      record_declaration → Class (per BR-AST-03 closest match — record is a value-type class)
      package_declaration → Namespace (per data model: Java package_declaration → Namespace)
      import_declaration → Import
      line_comment | block_comment → Docstring if Javadoc (`/**`) preceding a definition; else
        CommentBlock
      program → Module
    - TestFunction detection: walk method's modifiers (annotations) for any of:
      @Test, @ParameterizedTest, @RepeatedTest, @TestFactory, @TestTemplate (also @org.junit.jupiter.api.Test
      via FQN — accept the simple name match)
    - is_chunkable: method_declaration, class_declaration, interface_declaration,
      annotation_type_declaration, enum_declaration, record_declaration, program
    - get_node_name: method_declaration.name; class_declaration.name; etc.
    - is_container: class_declaration, interface_declaration, annotation_type_declaration,
      enum_declaration, record_declaration
    - Unit tests: ≥10 covering: class with method (Method), @Test method (TestFunction),
      @ParameterizedTest method (TestFunction), interface (TraitOrInterface), enum (Enum),
      record (Class), package decl (Namespace), import (Import), Javadoc preceding class (Docstring)
  </in_scope>
  <out_of_scope>
    - JUnit 4 `org.junit.Test` distinguished from JUnit 5 — accept either
    - Maven/Gradle build file inference
    - TestNG annotations (@org.testng.annotations.Test) — out of scope; future enhancement
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

      const JUNIT_TEST_ANNOTATIONS: &[&str] = &[
          "Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate",
      ];

      fn has_junit_test_annotation(method: &Node<'_>, source: &[u8]) -> bool { /* ... */ }
      fn is_javadoc_comment(node: &Node<'_>, source: &[u8]) -> bool { /* /** prefix */ }
    </signature>
  </signatures>

  <constraints>
    - JUNIT_TEST_ANNOTATIONS uses simple names; matching `@Test` and `@org.junit.jupiter.api.Test`
      both reduce to "Test" by inspecting the annotation's marker_annotation.name child
    - Javadoc detection: comment starts with `/**` AND its end_byte's next non-whitespace AST node
      is a class/method/interface declaration
    - record_declaration → Class (BR-AST-03 closest fit; doc 14 §5 cell shrinkage handles missing
      "Record" cell)
    - package_declaration → Namespace (per data model `<field name="Namespace"...constraints="...,
      package_declaration (Java)" />`)
    - File under 500 lines; this file should be < 400 lines incl tests
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast::classifier::java` ≥10 tests pass
    - `class A { @Test void m() {} }` → method m is TestFunction
    - `class A { @ParameterizedTest void m() {} }` → TestFunction
    - `class A { void m() {} }` → Method (no annotation)
    - `interface I { void m(); }` → I is TraitOrInterface; m is Method
    - `record P(int x) {}` → Class
    - `package com.example;` → Namespace
    - `import java.util.*;` → Import
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
classifier/java.rs:
  const JUNIT_TEST_ANNOTATIONS: &[&str] = &[
      "Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate",
  ];

  pub fn classify(node, source, ctx) -> EntityType {
      match node.kind() {
          "method_declaration" => {
              if has_junit_test_annotation(node, source) { EntityType::TestFunction }
              else { EntityType::Method }
          }
          "class_declaration" => EntityType::Class,
          "interface_declaration" | "annotation_type_declaration" => EntityType::TraitOrInterface,
          "enum_declaration" => EntityType::Enum,
          "record_declaration" => EntityType::Class,
          "package_declaration" => EntityType::Namespace,
          "import_declaration" => EntityType::Import,
          "line_comment" | "block_comment" => {
              if is_javadoc_comment(node, source) && next_sibling_is_definition(node) {
                  EntityType::Docstring
              } else { EntityType::CommentBlock }
          }
          "program" => EntityType::Module,
          _ => EntityType::Method,
      }
  }

  fn has_junit_test_annotation(method, source) -> bool {
      // method has `modifiers` field; iterate modifier children for marker_annotation / annotation
      // get annotation.name (last identifier component for FQN); compare against JUNIT_TEST_ANNOTATIONS
      let modifiers = match method.child_by_field_name("modifiers") { Some(n) => n, None => return false };
      let mut cursor = modifiers.walk();
      for child in modifiers.children(&mut cursor) {
          if matches!(child.kind(), "marker_annotation" | "annotation") {
              let name = child.child_by_field_name("name");
              if let Some(n) = name {
                  let text = String::from_utf8_lossy(&source[n.start_byte()..n.end_byte()]);
                  let last = text.rsplit('.').next().unwrap_or(&text);
                  if JUNIT_TEST_ANNOTATIONS.contains(&last) { return true; }
              }
          }
      }
      false
  }

  // 10+ tests
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast/classify.rs">Java classifier + JUnit annotation detection + 10 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast/classify.rs">Add `pub mod java;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>method_declaration with @Test → TestFunction</criterion>
  <criterion>method_declaration with @ParameterizedTest / @RepeatedTest / @TestFactory / @TestTemplate → TestFunction</criterion>
  <criterion>method_declaration without test annotation → Method</criterion>
  <criterion>FQN annotation (@org.junit.jupiter.api.Test) detected by simple-name matching</criterion>
  <criterion>record_declaration → Class (BR-AST-03)</criterion>
  <criterion>package_declaration → Namespace</criterion>
  <criterion>≥10 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast::classifier::java</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
