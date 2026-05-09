# TASK-AST-018 — Implement C# entity classifier (operator overloads + .csproj/.sln discovery)

```xml
<task_spec id="TASK-AST-018" version="1.0">
<metadata>
  <title>Implement C# classifier — [Test]/[Fact]/[Theory]/[TestMethod] + operator/conversion declarations</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>18</sequence>
  <implements>
    <item ref="REQ-AST-03">EntityType for C# kinds (method_declaration, operator_declaration, conversion_operator_declaration, destructor_declaration, class_declaration, interface_declaration, struct_declaration, record_declaration, namespace_declaration, using_directive)</item>
    <item ref="REQ-AST-12">C# TestFunction detection (NUnit/xUnit/MSTest annotations)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
C# tests are detected by attributes [Test] (NUnit), [Fact] / [Theory] (xUnit), [TestMethod]
(MSTest). Operator overloads and conversion operators are method_declaration siblings:
operator_declaration, conversion_operator_declaration, destructor_declaration. record
keyword (record_declaration) maps to Class (BR-AST-03). C# also has structs (struct_declaration)
distinct from classes — emitted as Struct.
</context>

<input_context_files>
  <file purpose="entity_type">crates/context-graph-core/src/memory/ast_chunker/entity_type.rs</file>
  <file purpose="boundary_hooks">crates/context-graph-core/src/memory/ast_chunker/boundary.rs</file>
  <file purpose="reference_classifier">crates/context-graph-cli/src/commands/dynamicjepa.rs (search "csharp" / "method_declaration" near line 6400)</file>
  <file purpose="node_kind_matrix">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (PerLanguageNodeKindTable.csharp)</file>
  <file purpose="testfn_rules">docs/ruvectorfindings/specs/functional/SPEC-AST-CHUNKING.md (REQ-AST-12 C# rule)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-002 + TASK-AST-010 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Module: `crates/context-graph-core/src/memory/ast_chunker/classifier/csharp.rs`
    - `pub fn hooks() -> LanguageHooks`
    - classify:
      method_declaration → TestFunction if has [Test]/[Fact]/[Theory]/[TestMethod]
        attribute; else Method
      operator_declaration | conversion_operator_declaration | destructor_declaration → Method
        (these are special method forms; never TestFunctions)
      class_declaration → Class
      interface_declaration → TraitOrInterface
      struct_declaration → Struct
      record_declaration → Class (BR-AST-03)
      enum_declaration → Enum
      namespace_declaration | file_scoped_namespace_declaration → Namespace
      using_directive → Import
      comment → Docstring if `///` triple-slash preceding a declaration; else CommentBlock
      compilation_unit → Module
    - TestFunction detection: walk method's attribute_list children for attributes whose
      name (last identifier of qualified name) is in
      {Test, Fact, Theory, TestMethod, TestCase, Fixture, SetUp, TearDown}
    - is_chunkable: method_declaration, operator_declaration, conversion_operator_declaration,
      destructor_declaration, class_declaration, interface_declaration, struct_declaration,
      record_declaration, enum_declaration, namespace_declaration,
      file_scoped_namespace_declaration, compilation_unit
    - get_node_name: method_declaration.name; operator_declaration.operator (extract source bytes);
      class_declaration.name; etc.
    - is_container: class_declaration, interface_declaration, struct_declaration,
      record_declaration, namespace_declaration
    - Unit tests: ≥10 covering: [Test] method, [Fact] method, regular method, operator+,
      destructor, class, interface, struct, record, namespace, using directive
  </in_scope>
  <out_of_scope>
    - .csproj/.sln file parsing (caller responsibility for build-graph context; chunker doesn't
      read project files)
    - Source generators (handled at compile time by Roslyn — out of scope)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/classifier/csharp.rs">
      use tree_sitter::Node;
      use crate::memory::ast_chunker::{ClassificationContext, EntityType, LanguageHooks};

      pub fn hooks() -> LanguageHooks { /* ... */ }
      pub fn classify(node: &Node<'_>, source: &[u8], ctx: &ClassificationContext) -> EntityType { /* ... */ }
      pub fn is_chunkable(node: &Node<'_>) -> bool { /* ... */ }
      pub fn get_node_name(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
      pub fn is_container(node: &Node<'_>) -> bool { /* ... */ }

      const CSHARP_TEST_ATTRIBUTES: &[&str] = &[
          "Test", "Fact", "Theory", "TestMethod",
          "TestCase", "Fixture", "SetUp", "TearDown",
      ];

      fn has_csharp_test_attribute(method: &Node<'_>, source: &[u8]) -> bool { /* ... */ }
      fn is_xml_doc_comment(node: &Node<'_>, source: &[u8]) -> bool { /* /// prefix */ }
    </signature>
  </signatures>

  <constraints>
    - Attribute names matched via "last component of qualified name" (e.g. NUnit.Framework.TestAttribute → "TestAttribute"; strip trailing "Attribute" suffix before comparison)
    - record_declaration → Class (BR-AST-03)
    - file_scoped_namespace_declaration is a C# 10+ shorthand — same EntityType as namespace_declaration
    - File under 500 lines; this file should be < 400 lines incl tests
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast_chunker::classifier::csharp` ≥10 tests pass
    - `class A { [Test] void M() {} }` → method M is TestFunction
    - `class A { [Fact] void M() {} }` → TestFunction
    - `class A { [Theory] void M() {} }` → TestFunction
    - `class A { [TestMethod] void M() {} }` → TestFunction
    - `class A { void M() {} }` → Method
    - `class A { public static A operator+(A a, A b) { ... } }` → Method (operator)
    - `class A { ~A() {} }` → Method (destructor)
    - `interface I { void M(); }` → TraitOrInterface
    - `struct S { int X; }` → Struct
    - `record R(int X);` → Class
    - `namespace N { ... }` → Namespace
    - `using System;` → Import
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
classifier/csharp.rs:
  const CSHARP_TEST_ATTRIBUTES: &[&str] = &[
      "Test", "Fact", "Theory", "TestMethod",
      "TestCase", "Fixture", "SetUp", "TearDown",
  ];

  pub fn classify(node, source, ctx) -> EntityType {
      match node.kind() {
          "method_declaration" => {
              if has_csharp_test_attribute(node, source) { EntityType::TestFunction }
              else { EntityType::Method }
          }
          "operator_declaration" | "conversion_operator_declaration"
              | "destructor_declaration" | "constructor_declaration" => EntityType::Method,
          "class_declaration" => EntityType::Class,
          "interface_declaration" => EntityType::TraitOrInterface,
          "struct_declaration" => EntityType::Struct,
          "record_declaration" => EntityType::Class,
          "enum_declaration" => EntityType::Enum,
          "namespace_declaration" | "file_scoped_namespace_declaration" => EntityType::Namespace,
          "using_directive" => EntityType::Import,
          "comment" => {
              if is_xml_doc_comment(node, source) && next_sibling_is_definition(node) {
                  EntityType::Docstring
              } else { EntityType::CommentBlock }
          }
          "compilation_unit" => EntityType::Module,
          _ => EntityType::Method,
      }
  }

  fn has_csharp_test_attribute(method, source) -> bool {
      // method has attribute_list children; walk each attribute_list's attribute children
      let mut cursor = method.walk();
      for child in method.children(&mut cursor) {
          if child.kind() == "attribute_list" {
              let mut c2 = child.walk();
              for attr in child.children(&mut c2) {
                  if attr.kind() == "attribute" {
                      let name = attr.child_by_field_name("name");
                      if let Some(n) = name {
                          let text = String::from_utf8_lossy(&source[n.start_byte()..n.end_byte()]);
                          let last = text.rsplit('.').next().unwrap_or(&text).trim_end_matches("Attribute");
                          if CSHARP_TEST_ATTRIBUTES.contains(&last) { return true; }
                      }
                  }
              }
          }
      }
      false
  }

  // 10+ tests
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/csharp.rs">C# classifier + test attribute detection + 10 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/mod.rs">Add `pub mod csharp;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>method_declaration with [Test]/[Fact]/[Theory]/[TestMethod] → TestFunction</criterion>
  <criterion>operator_declaration / conversion_operator_declaration / destructor_declaration → Method</criterion>
  <criterion>class_declaration → Class; interface_declaration → TraitOrInterface; struct_declaration → Struct</criterion>
  <criterion>record_declaration → Class (BR-AST-03)</criterion>
  <criterion>file_scoped_namespace_declaration handled as Namespace (C# 10+)</criterion>
  <criterion>FQN attributes resolved via "last component, strip Attribute suffix"</criterion>
  <criterion>≥10 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast_chunker::classifier::csharp</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
