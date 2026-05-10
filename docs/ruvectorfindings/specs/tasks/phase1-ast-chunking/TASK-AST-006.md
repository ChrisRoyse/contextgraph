# TASK-AST-006 — Implement reformat-invariant `compute_chunk_sha256`

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth. Any lower reference to `src/memory/ast_chunker/<submodule>.rs`, `dynamicjepa.rs`, compatibility shims, or "create new" scaffolding language is superseded by this audit note. Verify and harden the existing canonical files instead of creating parallel modules unless the repo audit proves a file is genuinely missing.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task (Rust source file, JSON evidence file, RocksDB CF, or ME-JEPA panel file), run the logic, then perform a separate readback from that source of truth. Print before/after system state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution, not only function return values.

```xml
<task_spec id="TASK-AST-006" version="1.0">
<metadata>
  <title>Implement compute_chunk_sha256 over AST node-id sequence (reformat-invariant)</title>
  <status>audit-hardening</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-AST-04">SHA-256 over (AST node-kind seq, field-name seq, identifiers, language, parent_chain)</item>
    <item ref="BR-AST-01">Reformat-invariant chunk SHAs across canonical formatter passes</item>
    <item ref="EC-AST-06">Whitespace/comment differences produce identical sha256</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
The shipped Rust chunker hashes raw chunk text — making chunk SHAs unstable under
`cargo fmt` / `prettier`. Phase 1a switches to AST-node-id hashing: BFS-walk the chunk's
tree-sitter subtree, accumulate (node.kind() + field-name + named-identifier-text) into
a SHA-256 hash. Whitespace tokens are excluded; comments are excluded for non-comment
chunks. This makes chunk SHAs stable across formatters (the shift-subscriber's load-bearing
property — a `cargo fmt` produces zero changed-chunk events).
</context>

<input_context_files>
  <file purpose="api_contract">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#api_contracts (compute_chunk_sha256)</file>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#component_contracts (ChunkSha256Hasher)</file>
  <file purpose="reformat_invariance">docs/ruvectorfindings/specs/functional/SPEC-AST-CHUNKING.md (REQ-AST-04, BR-AST-01, EC-AST-06)</file>
  <file purpose="tree_sitter_api">https://docs.rs/tree-sitter/0.26.0/tree_sitter/struct.Node.html (Node::kind, Node::field_name_for_child, Node::is_named, Node::children, walk)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-003 complete: AstChunk + types module available (defines [u8; 32] sha256 field)</check>
  <check>sha2 = "0.10" already in context-graph-core/Cargo.toml line 28</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub fn compute_chunk_sha256(node: &tree_sitter::Node, source: &[u8], language: Language, parent_chain: &str) -> [u8; 32]`
    - BFS walk over node's subtree
    - Per visited node: skip whitespace-only nodes; for non-comment chunks, skip comment-kind nodes
    - Hash inputs (in order, with byte-separators):
      1. language tag bytes (e.g. b"rust") + 0x00
      2. parent_chain bytes + 0x00
      3. For each visited node in BFS order:
         a. node.kind() bytes + 0x01
         b. If parent assigns a field name to this child: field-name bytes + 0x02
         c. If node.is_named() AND kind is in NAMED_IDENTIFIER_KINDS: source[start_byte..end_byte] bytes + 0x03
    - Helper: `fn is_whitespace_node(kind: &str) -> bool` (matches "\n", " ", tree-sitter built-ins)
    - Helper: `fn is_comment_kind(kind: &str, language: Language) -> bool` (per-language: line_comment, block_comment, comment, etc.)
    - Helper: `fn is_named_identifier(kind: &str) -> bool` (identifier, type_identifier, field_identifier, scoped_identifier, plus language-specific kinds)
    - Unit tests: hash determinism on identical input; hash difference on identifier rename;
      hash equality across whitespace-only differences (synthetic — full reformat test in TASK-AST-022)
  </in_scope>
  <out_of_scope>
    - Tree construction (caller provides tree-sitter Node)
    - Per-language node-kind tables for entity classification (TASK-AST-011..020)
    - Multi-file hashing (caller iterates files)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast/mod.rs">
      use sha2::{Digest, Sha256};
      use tree_sitter::Node;
      use super::Language;

      /// Compute reformat-invariant SHA-256 over a tree-sitter node's subtree.
      ///
      /// Hash inputs (in BFS order, with byte separators):
      ///   language tag || 0x00 || parent_chain || 0x00 ||
      ///   for each node: kind || 0x01 [|| field_name || 0x02] [|| identifier || 0x03]
      ///
      /// Whitespace nodes and (for non-comment chunks) comment nodes are excluded —
      /// this is what makes the hash reformat-invariant.
      pub fn compute_chunk_sha256(
          node: &Node<'_>,
          source: &[u8],
          language: Language,
          parent_chain: &str,
      ) -> [u8; 32] { /* ... */ }

      fn is_whitespace_node(kind: &str) -> bool { /* ... */ }
      fn is_comment_kind(kind: &str, language: Language) -> bool { /* ... */ }
      fn is_named_identifier(kind: &str) -> bool { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - Output is exactly 32 bytes (sha2::Sha256 output)
    - Determinism: same (node, source, language, parent_chain) → same digest, byte-equal across
      runs and processes
    - BFS walk (NOT pre-order or post-order) — choose ONE and document why; BFS is recommended
      because it groups by tree depth, making the hash easier to reason about visually
    - The chunk's root node's own kind() IS hashed (it's the first BFS-visited node)
    - Whitespace-only children (e.g. the `\n` and ` ` tokens tree-sitter sometimes emits as
      anonymous tokens) are excluded
    - Comments excluded for non-comment chunks: detect by checking root node kind. If root
      is is_comment_kind(), include comments; else skip them.
    - Identifier strings ARE hashed (rename-sensitive). Doc 14 §9 explicitly says renames are
      semantic changes that SHOULD change the SHA.
    - File under 500 lines; this file should be < 250 lines incl. tests
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast::hash` all pass (≥6 tests)
    - Determinism test: hash twice in same process; byte-equal
    - Rename-sensitivity test: rename `foo` → `bar` in identical Rust source; hashes differ
    - Whitespace-invariance test: `fn f(){0}` vs `fn f() { 0 }` (Rust); hashes equal
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
hash.rs:
  use sha2::{Digest, Sha256};
  use tree_sitter::Node;
  use std::collections::VecDeque;
  use super::Language;

  pub fn compute_chunk_sha256(node: &Node, source: &[u8], language: Language, parent_chain: &str) -> [u8; 32] {
      let mut hasher = Sha256::new();
      hasher.update(language.as_tag().as_bytes());
      hasher.update(&[0x00]);
      hasher.update(parent_chain.as_bytes());
      hasher.update(&[0x00]);
      let root_is_comment = is_comment_kind(node.kind(), language);
      // BFS
      let mut queue: VecDeque<Node> = VecDeque::new();
      queue.push_back(*node);
      while let Some(n) = queue.pop_front() {
          let kind = n.kind();
          if is_whitespace_node(kind) { continue; }
          if !root_is_comment && is_comment_kind(kind, language) { continue; }
          hasher.update(kind.as_bytes());
          hasher.update(&[0x01]);
          // field-name lookup: if this node has a parent and the parent assigns a field name
          // (we can carry parent reference via tuple in queue, or use node.parent() + iterate fields)
          // For simplicity: walk parent's children, find the field name index that matches n.id()
          if let Some(parent) = n.parent() {
              let mut cursor = parent.walk();
              for (idx, child) in parent.children(&mut cursor).enumerate() {
                  if child.id() == n.id() {
                      if let Some(field_name) = parent.field_name_for_child(idx as u32) {
                          hasher.update(field_name.as_bytes());
                          hasher.update(&[0x02]);
                      }
                      break;
                  }
              }
          }
          if n.is_named() && is_named_identifier(kind) {
              let bytes = &source[n.start_byte()..n.end_byte()];
              hasher.update(bytes);
              hasher.update(&[0x03]);
          }
          // enqueue children
          let mut cursor = n.walk();
          for child in n.children(&mut cursor) {
              queue.push_back(child);
          }
      }
      let digest = hasher.finalize();
      let mut out = [0u8; 32];
      out.copy_from_slice(&digest);
      out
  }

  fn is_whitespace_node(kind: &str) -> bool {
      matches!(kind, "\n" | " " | "\t" | "\r" | "\r\n")
  }

  fn is_comment_kind(kind: &str, _language: Language) -> bool {
      matches!(kind, "comment" | "line_comment" | "block_comment"
                   | "block_documentation_comment" | "doc_comment")
  }

  fn is_named_identifier(kind: &str) -> bool {
      matches!(kind, "identifier" | "type_identifier" | "field_identifier"
                   | "scoped_identifier" | "property_identifier" | "shorthand_property_identifier"
                   | "namespace_identifier" | "qualified_identifier" | "constant"
                   | "name" /* php */ | "constant_name" /* ruby */
                   | "system_lib_string" /* C #include */)
  }

  #[cfg(test)] mod tests {
      // These tests need a real tree-sitter Parser; gate behind tree_sitter_rust feature.
      // Tests are deferred to TASK-AST-021 cross-language harness if direct construction
      // of Node fixtures is too invasive. Provide one tree_sitter_rust-driven test here.
      use super::*;
      use tree_sitter::Parser;

      fn parse_rust(src: &str) -> tree_sitter::Tree {
          let mut p = Parser::new();
          let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
          p.set_language(&lang).unwrap();
          p.parse(src, None).unwrap()
      }

      #[test] fn determinism_same_input() {
          let src = b"fn foo() { 1 + 2 }";
          let tree = parse_rust(std::str::from_utf8(src).unwrap());
          let h1 = compute_chunk_sha256(&tree.root_node(), src, Language::Rust, "mod_a");
          let h2 = compute_chunk_sha256(&tree.root_node(), src, Language::Rust, "mod_a");
          assert_eq!(h1, h2);
      }
      #[test] fn rename_changes_hash() {
          let s1 = "fn foo() { 1 + 2 }";
          let s2 = "fn bar() { 1 + 2 }";
          let t1 = parse_rust(s1); let t2 = parse_rust(s2);
          let h1 = compute_chunk_sha256(&t1.root_node(), s1.as_bytes(), Language::Rust, "");
          let h2 = compute_chunk_sha256(&t2.root_node(), s2.as_bytes(), Language::Rust, "");
          assert_ne!(h1, h2);
      }
      #[test] fn whitespace_does_not_change_hash() {
          let s1 = "fn f(){0}";
          let s2 = "fn f() { 0 }";
          let t1 = parse_rust(s1); let t2 = parse_rust(s2);
          let h1 = compute_chunk_sha256(&t1.root_node(), s1.as_bytes(), Language::Rust, "");
          let h2 = compute_chunk_sha256(&t2.root_node(), s2.as_bytes(), Language::Rust, "");
          assert_eq!(h1, h2);
      }
      #[test] fn parent_chain_changes_hash() {
          let s = "fn f() { 0 }";
          let t = parse_rust(s);
          let h1 = compute_chunk_sha256(&t.root_node(), s.as_bytes(), Language::Rust, "mod_a");
          let h2 = compute_chunk_sha256(&t.root_node(), s.as_bytes(), Language::Rust, "mod_b");
          assert_ne!(h1, h2);
      }
      #[test] fn language_tag_changes_hash() {
          // Using the same source bytes but different language tag — synthetic but valid for hash isolation
          let s = b"x";
          let t = parse_rust("fn x() {}");
          let h1 = compute_chunk_sha256(&t.root_node(), s, Language::Rust, "");
          let h2 = compute_chunk_sha256(&t.root_node(), s, Language::Python, "");
          assert_ne!(h1, h2);
      }
      #[test] fn comment_only_difference_does_not_change_function_hash() {
          let s1 = "fn f() { 0 }";
          let s2 = "fn f() { /* comment */ 0 }";
          let t1 = parse_rust(s1); let t2 = parse_rust(s2);
          let h1 = compute_chunk_sha256(&t1.root_node(), s1.as_bytes(), Language::Rust, "");
          let h2 = compute_chunk_sha256(&t2.root_node(), s2.as_bytes(), Language::Rust, "");
          assert_eq!(h1, h2);
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast/mod.rs">compute_chunk_sha256 + helpers + 6 unit tests using tree_sitter_rust</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast/mod.rs">Add `pub mod hash;` + `pub use hash::compute_chunk_sha256;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>compute_chunk_sha256 returns [u8; 32] deterministically for identical input</criterion>
  <criterion>Identifier rename produces different hash</criterion>
  <criterion>Whitespace-only differences produce identical hash (reformat-invariance)</criterion>
  <criterion>Comment-only differences produce identical hash for non-comment chunks</criterion>
  <criterion>parent_chain difference produces different hash</criterion>
  <criterion>language tag difference produces different hash</criterion>
  <criterion>All 6 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast::hash</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
