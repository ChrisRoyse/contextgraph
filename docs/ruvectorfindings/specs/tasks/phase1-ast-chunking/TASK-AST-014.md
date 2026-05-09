# TASK-AST-014 — Implement Go entity classifier

```xml
<task_spec id="TASK-AST-014" version="1.0">
<metadata>
  <title>Implement Go classifier — receiver methods + Test*/Benchmark*/Example* discovery</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>14</sequence>
  <implements>
    <item ref="REQ-AST-03">EntityType for Go node kinds (function_declaration, method_declaration, type_declaration, package_clause, import_declaration)</item>
    <item ref="REQ-AST-12">Go TestFunction detection (TestXxx with *testing.T param)</item>
    <item ref="EC-AST-11">TestData(input) without *testing.T → Function (NOT TestFunction)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
Go has receiver methods (`func (r *T) m() {}`) which differ from regular functions in
tree-sitter (method_declaration vs function_declaration). Test detection must verify
both name pattern (^Test[A-Z]) AND first parameter type (*testing.T) per EC-AST-11 to
avoid false positives. Benchmark*/Example* follow the same convention with *testing.B
and any param respectively.
</context>

<input_context_files>
  <file purpose="entity_type">crates/context-graph-core/src/memory/ast_chunker/entity_type.rs</file>
  <file purpose="boundary_hooks">crates/context-graph-core/src/memory/ast_chunker/boundary.rs</file>
  <file purpose="reference_classifier">crates/context-graph-cli/src/commands/dynamicjepa.rs (search "go" / "function_declaration" near line 6400)</file>
  <file purpose="node_kind_matrix">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (PerLanguageNodeKindTable.go)</file>
  <file purpose="testfn_rules">docs/ruvectorfindings/specs/functional/SPEC-AST-CHUNKING.md (REQ-AST-12 Go rule, EC-AST-11)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-002 + TASK-AST-010 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Module: `crates/context-graph-core/src/memory/ast_chunker/classifier/go.rs`
    - `pub fn hooks() -> LanguageHooks`
    - classify:
      function_declaration → TestFunction if name matches Test/Benchmark/Example AND param check
        passes; else Function
      method_declaration → Method (always; Go methods have receiver)
      type_declaration → walk inner type_spec; struct_type → Struct, interface_type → TraitOrInterface,
        else default to Struct
      package_clause → Module (Go's "package main" declaration is the root module)
      import_declaration | import_spec → Import
      comment → CommentBlock (Go has no docstring convention separate from comments)
      source_file → Module
    - TestFunction detection:
      regex name pattern `^(Test|Benchmark|Example)[A-Z]`
      AND first parameter type ends in `testing.T`, `testing.B`, `testing.M` for Test/Benchmark
      Example* takes any signature (or no args) — accept any if name matches `^Example[A-Z]`
    - is_chunkable: function_declaration, method_declaration, type_declaration, source_file
    - get_node_name: function_declaration.name; method_declaration.name; type_declaration.spec.name
    - is_container: type_declaration when inner is struct_type / interface_type
    - Unit tests: ≥10 tests covering function/method/struct-type/interface-type/Test+T/Benchmark+B/
      TestData-without-T (must NOT be TestFunction per EC-AST-11)/Example
  </in_scope>
  <out_of_scope>
    - Go modules / go.mod parsing
    - cgo (C extern blocks) — treated as opaque content
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/classifier/go.rs">
      use tree_sitter::Node;
      use crate::memory::ast_chunker::{ClassificationContext, EntityType, LanguageHooks};

      pub fn hooks() -> LanguageHooks { /* ... */ }
      pub fn classify(node: &Node<'_>, source: &[u8], ctx: &ClassificationContext) -> EntityType { /* ... */ }
      pub fn is_chunkable(node: &Node<'_>) -> bool { /* ... */ }
      pub fn get_node_name(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
      pub fn is_container(node: &Node<'_>) -> bool { /* ... */ }

      fn is_go_test_function(node: &Node<'_>, source: &[u8]) -> bool { /* name + first-param-type check */ }
      fn first_parameter_type_text(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - is_go_test_function: name matches `^(Test|Benchmark|Example)[A-Z]` (use byte-level prefix
      check + ASCII isupper of next char); for Test/Benchmark, first parameter type must be
      `*testing.T` / `*testing.B` (substring match on the parameter type text)
    - Example* accepts any signature (Go convention for runnable examples)
    - File under 500 lines; this file should be < 350 lines incl tests
    - GENERIC-ONLY: rules apply to ANY .go file
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast_chunker::classifier::go` ≥10 tests pass
    - `func TestFoo(t *testing.T) {}` → TestFunction
    - `func TestData(input []byte) bool {}` → Function (EC-AST-11 — no *testing.T)
    - `func BenchmarkFoo(b *testing.B) {}` → TestFunction
    - `func ExampleFoo() {}` → TestFunction
    - `func (r *T) Method() {}` → Method
    - `type S struct { ... }` → Struct
    - `type I interface { ... }` → TraitOrInterface
    - `package main` → Module
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
classifier/go.rs:
  pub fn classify(node, source, ctx) -> EntityType {
      match node.kind() {
          "function_declaration" => {
              if is_go_test_function(node, source) { EntityType::TestFunction }
              else { EntityType::Function }
          }
          "method_declaration" => EntityType::Method,
          "type_declaration" => {
              // walk inner type_spec / type_alias
              let mut cursor = node.walk();
              for child in node.children(&mut cursor) {
                  if child.kind() == "type_spec" {
                      let type_node = child.child_by_field_name("type");
                      if let Some(t) = type_node {
                          return match t.kind() {
                              "struct_type" => EntityType::Struct,
                              "interface_type" => EntityType::TraitOrInterface,
                              _ => EntityType::Struct,  // default for type alias
                          };
                      }
                  }
              }
              EntityType::Struct
          }
          "package_clause" => EntityType::Module,
          "import_declaration" | "import_spec" => EntityType::Import,
          "comment" => EntityType::CommentBlock,
          "source_file" => EntityType::Module,
          _ => EntityType::Function,
      }
  }

  fn is_go_test_function(node, source) -> bool {
      let name = match node.child_by_field_name("name") {
          Some(n) => String::from_utf8_lossy(&source[n.start_byte()..n.end_byte()]).into_owned(),
          None => return false,
      };
      let (prefix, requires_param) = if name.starts_with("Test") { ("Test", true) }
          else if name.starts_with("Benchmark") { ("Benchmark", true) }
          else if name.starts_with("Example") { ("Example", false) }
          else { return false; };
      // Next char after prefix must be ASCII uppercase
      let next = name.as_bytes().get(prefix.len()).copied().unwrap_or(0);
      if !(next.is_ascii_uppercase()) { return false; }
      if !requires_param { return true; }
      let param_ty = first_parameter_type_text(node, source).unwrap_or_default();
      match prefix {
          "Test" => param_ty.contains("testing.T"),
          "Benchmark" => param_ty.contains("testing.B"),
          _ => true,
      }
  }

  // 10+ tests
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/go.rs">Go classifier + Test*/Benchmark*/Example* detection + 10 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/mod.rs">Add `pub mod go;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>function_declaration with TestX(*testing.T) → TestFunction</criterion>
  <criterion>function_declaration with TestData([]byte) → Function (EC-AST-11)</criterion>
  <criterion>method_declaration → Method always</criterion>
  <criterion>type_declaration with struct_type child → Struct; interface_type → TraitOrInterface</criterion>
  <criterion>package_clause → Module; source_file → Module</criterion>
  <criterion>import_declaration → Import</criterion>
  <criterion>≥10 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast_chunker::classifier::go</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
