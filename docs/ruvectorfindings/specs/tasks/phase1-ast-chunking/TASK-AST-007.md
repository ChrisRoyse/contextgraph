# TASK-AST-007 — Implement `ChunkBoundaryEngine` (process_node + merge_small_chunks)

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth. Any lower reference to `src/memory/ast_chunker/<submodule>.rs`, `dynamicjepa.rs`, compatibility shims, or "create new" scaffolding language is superseded by this audit note. Verify and harden the existing canonical files instead of creating parallel modules unless the repo audit proves a file is genuinely missing.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task (Rust source file, JSON evidence file, RocksDB CF, or ME-JEPA panel file), run the logic, then perform a separate readback from that source of truth. Print before/after system state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution, not only function return values.
>
> **TASK-AST-007 correction:** The current implementation emits chunks through `walk_and_emit` and `build_chunk` in `ast/mod.rs`; `sha256` and `embedder_routing_set` are filled immediately. Do not reintroduce placeholder chunks that depend on a later facade pass.

```xml
<task_spec id="TASK-AST-007" version="1.0">
<metadata>
  <title>Implement ChunkBoundaryEngine — language-agnostic process_node + merge_small_chunks</title>
  <status>audit-hardening</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-AST-05">cAST methodology — target 500 / min 100 / max 1000 non-whitespace chars</item>
    <item ref="REQ-AST-02">Chunk metadata population (language, entity_type, parent_chain, line_start, line_end)</item>
    <item ref="EC-AST-02">4× target_size warning diagnostic for over-large chunks</item>
    <item ref="EC-AST-12">Greedy sibling merge for below-min chunks</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-003</task_ref>
    <task_ref>TASK-AST-004</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
The shipped Rust chunker has a 200-line `process_node()` and ~100-line `merge_small_chunks()`.
This task generalizes them: language-agnostic recursive AST walk that delegates per-node
classification + chunkability decisions to per-language classifier modules (registered in
TASK-AST-011..020) via a ClassifierFn callback. Output is partial AstChunks (sha256 left
zero-filled; embedder_routing_set empty) — TASK-AST-021's facade fills these in after
TASK-AST-006's hash + TASK-AST-005's router.
</context>

<input_context_files>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#component_contracts (ChunkBoundaryEngine)</file>
  <file purpose="legacy_process_node">crates/context-graph-core/src/memory/ast_chunker.rs (lines 326-560; legacy process_node + merge_small_chunks for Rust to extract algorithm from)</file>
  <file purpose="cAST_methodology">docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md §1.2 (~500 char target, divide-and-combine)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-003 complete: AstChunk available</check>
  <check>TASK-AST-004 complete: AstChunkerError available</check>
</prerequisites>

<scope>
  <in_scope>
    - Define `pub struct AstChunkConfig` with target_size=500, min_size=100, max_size=1000,
      include_parent_context=true, include_imports=true (mirrors legacy)
    - Define `pub type ClassifierFn = fn(node: &Node, source: &[u8], ctx: &ClassificationContext) -> EntityType;`
    - Define `pub type ChunkabilityFn = fn(node: &Node) -> bool;`
    - Define `pub type NodeNameFn = fn(node: &Node, source: &[u8]) -> Option<String>;`
    - Define `pub type ContainerFn = fn(node: &Node) -> bool;`
    - `pub struct LanguageHooks { classify, is_chunkable, get_node_name, is_container }`
      — populated per-language in TASK-AST-011..020 and registered in TASK-AST-021
    - Implement `pub fn process_node(node, source, language, hooks, file_path, imports, scope_chain, parent_def, ctx, chunks, config) -> ()`
    - Implement `pub fn merge_small_chunks(chunks, config) -> Vec<AstChunk>` — greedy sibling merge
    - Implement helper `pub fn non_whitespace_size(node, source) -> usize` (extracted from legacy)
    - Emit a `ParseDiagnostic { kind: "chunk_too_large", is_error: false, ... }` warning when
      a chunk exceeds 4 × target_size, attached to chunk.parse_diagnostics
    - Unit tests: synthetic Rust source covering small/medium/large chunks; merge of small siblings;
      large-function warning diagnostic
  </in_scope>
  <out_of_scope>
    - Per-language classifier impls (TASK-AST-011..020)
    - sha256 computation (TASK-AST-006 — invoked by TASK-AST-021 facade)
    - Embedder routing (TASK-AST-005 — invoked by TASK-AST-021 facade)
    - Tree-sitter parsing (TASK-AST-010 dispatcher)
    - The legacy Rust chunker's `contextualize()` step — left in place for canonical API;
      TASK-AST-021 reconciles
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast/mod.rs">
      use tree_sitter::Node;
      use super::{AstChunk, AstChunkerError, ClassificationContext, EntityType, Language, ParseDiagnostic};

      #[derive(Debug, Clone)]
      pub struct AstChunkConfig {
          pub target_size: usize,
          pub min_size: usize,
          pub max_size: usize,
          pub include_parent_context: bool,
          pub include_imports: bool,
      }

      impl Default for AstChunkConfig { /* 500/100/1000/true/true */ }

      pub type ClassifierFn = fn(&Node<'_>, &[u8], &ClassificationContext) -> EntityType;
      pub type ChunkabilityFn = fn(&Node<'_>) -> bool;
      pub type NodeNameFn = fn(&Node<'_>, &[u8]) -> Option<String>;
      pub type ContainerFn = fn(&Node<'_>) -> bool;

      pub struct LanguageHooks {
          pub classify: ClassifierFn,
          pub is_chunkable: ChunkabilityFn,
          pub get_node_name: NodeNameFn,
          pub is_container: ContainerFn,
      }

      pub fn non_whitespace_size(node: &Node<'_>, source: &[u8]) -> usize { /* ... */ }

      #[allow(clippy::too_many_arguments)]
      pub fn process_node(
          node: &Node<'_>,
          source: &[u8],
          language: Language,
          hooks: &LanguageHooks,
          file_path: &str,
          imports: &[String],
          scope_chain: &mut Vec<String>,
          parent_def: Option<&str>,
          ctx: &mut ClassificationContext,
          chunks: &mut Vec<AstChunk>,
          config: &AstChunkConfig,
      ) { /* ... */ }

      pub fn merge_small_chunks(chunks: Vec<AstChunk>, config: &AstChunkConfig) -> Vec<AstChunk> { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - process_node algorithm matches the shipped Rust chunker's flow (ast_chunker.rs:326-396):
      1. compute non_whitespace_size; size==0 → return
      2. size ≤ max_size AND hooks.is_chunkable(node) → emit chunk; return
      3. else recurse into children; track scope_chain via hooks.get_node_name; thread ctx
    - merge_small_chunks: greedy left-to-right pass; consecutive siblings at the same
      parent_chain depth with combined non_whitespace_size ≤ target_size are merged into
      one chunk with EntityType::Module (per BR-AST-05 Module classification for
      heterogeneous merges)
    - Below-min chunks at end-of-file with no merge target are emitted as-is (REQ-AST-05)
    - Over-4×-target chunks emit a warning ParseDiagnostic but ARE produced (EC-AST-02)
    - When emitting a chunk: `sha256` and `embedder_routing_set` are populated immediately by `build_chunk`.
      content = source[start_byte..end_byte] (no contextualization yet — TASK-AST-021 prepends
      file path + imports + parent_def)
    - File under 500 lines; this file may approach the limit — split helpers to a sibling file
      if needed (e.g. boundary/walk.rs + boundary/merge.rs)
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast::boundary` all pass (≥6 tests)
    - non_whitespace_size matches legacy implementation's output on shared fixtures
    - process_node populates language, entity_type (via hooks.classify), parent_chain,
      line_start/line_end correctly
    - merge_small_chunks merges below-min siblings; preserves above-min unmerged
    - Over-large chunk emits "chunk_too_large" warning ParseDiagnostic
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
boundary.rs:
  use tree_sitter::Node;
  use super::{AstChunk, ClassificationContext, EntityType, Language, ParseDiagnostic};
  use std::collections::BTreeSet;

  pub fn non_whitespace_size(node: &Node, source: &[u8]) -> usize {
      let bytes = &source[node.start_byte()..node.end_byte()];
      bytes.iter().filter(|b| !b.is_ascii_whitespace()).count()
  }

  pub fn process_node(node, source, language, hooks, file_path, imports, scope_chain, parent_def, ctx, chunks, config) {
      let size = non_whitespace_size(node, source);
      if size == 0 { return; }

      if size <= config.max_size && (hooks.is_chunkable)(node) {
          // emit chunk
          let entity_type = (hooks.classify)(node, source, ctx);
          let mut diag = vec![];
          if size > 4 * config.target_size {
              diag.push(ParseDiagnostic {
                  kind: "chunk_too_large".into(), is_error: false, is_missing: false,
                  start_byte: node.start_byte(), end_byte: node.end_byte(),
                  start_line: node.start_position().row as u32 + 1,
                  start_column: node.start_position().column as u32,
                  end_line: node.end_position().row as u32 + 1,
                  end_column: node.end_position().column as u32,
                  excerpt: format!("chunk of {} non-ws chars exceeds 4x target {}",
                                   size, config.target_size),
              });
          }
          let chunk = AstChunk {
              language, entity_type,
              parent_chain: scope_chain.join("::"),
              line_start: node.start_position().row as u32 + 1,
              line_end: node.end_position().row as u32 + 1,
              sha256: structural_sha256(...),
              parse_diagnostics: diag,
              content: source[node.start_byte()..node.end_byte()].to_vec(),
              embedder_routing_set: route_to_embedders(...),
          };
          chunks.push(chunk);
          return;
      }

      let mut cursor = node.walk();
      let children: Vec<_> = node.children(&mut cursor).collect();
      for child in children {
          if (hooks.is_chunkable)(&child) || child.child_count() > 0 {
              let name = (hooks.get_node_name)(&child, source);
              if let Some(ref n) = name { scope_chain.push(n.clone()); }
              let new_parent_def_holder; // keep alive across recursive call
              let new_parent_def = if (hooks.is_container)(node) {
                  new_parent_def_holder = abbreviated_definition(node, source);
                  Some(new_parent_def_holder.as_str())
              } else { parent_def };
              // (caller of process_node tracks ctx.inside_class / inside_impl based on language;
              //  language hooks are responsible for updating ctx if needed)
              process_node(&child, source, language, hooks, file_path, imports, scope_chain,
                           new_parent_def, ctx, chunks, config);
              if name.is_some() { scope_chain.pop(); }
          }
      }
  }

  pub fn merge_small_chunks(chunks: Vec<AstChunk>, config: &AstChunkConfig) -> Vec<AstChunk> {
      let mut out = Vec::with_capacity(chunks.len());
      let mut buffer: Vec<AstChunk> = Vec::new();
      let mut buffer_size: usize = 0;
      let nw = |c: &AstChunk| c.content.iter().filter(|b| !b.is_ascii_whitespace()).count();
      let flush = |out: &mut Vec<AstChunk>, buffer: &mut Vec<AstChunk>, buffer_size: &mut usize| {
          if buffer.len() == 1 {
              out.push(buffer.remove(0));
          } else if !buffer.is_empty() {
              let merged = merge_buffer(std::mem::take(buffer));
              out.push(merged);
          }
          *buffer_size = 0;
      };
      for chunk in chunks {
          let size = nw(&chunk);
          if size < config.min_size {
              // candidate for merge
              let same_parent = buffer.first().map(|b| b.parent_chain == chunk.parent_chain).unwrap_or(true);
              if !same_parent || buffer_size + size > config.target_size {
                  flush(&mut out, &mut buffer, &mut buffer_size);
              }
              buffer.push(chunk);
              buffer_size += size;
          } else {
              flush(&mut out, &mut buffer, &mut buffer_size);
              out.push(chunk);
          }
      }
      flush(&mut out, &mut buffer, &mut buffer_size);
      out
  }

  fn merge_buffer(buf: Vec<AstChunk>) -> AstChunk {
      // concatenate content; pick first chunk's metadata; reset entity_type to Module if heterogeneous
      let first = buf[0].clone();
      let mut content = Vec::new();
      let mut last_line = first.line_end;
      let mut homogeneous = true;
      for c in &buf {
          content.extend_from_slice(&c.content);
          content.push(b'\n');
          if c.entity_type != first.entity_type { homogeneous = false; }
          last_line = c.line_end;
      }
      AstChunk {
          language: first.language,
          entity_type: if homogeneous { first.entity_type } else { EntityType::Module },
          parent_chain: first.parent_chain,
          line_start: first.line_start,
          line_end: last_line,
          sha256: [0u8; 32],
          parse_diagnostics: Vec::new(),
          content,
          embedder_routing_set: BTreeSet::new(),
      }
  }

  fn abbreviated_definition(node: &Node, source: &[u8]) -> String {
      // Mirror legacy ast_chunker.rs get_abbreviated_definition: emit the first line of the node's bytes
      let bytes = &source[node.start_byte()..node.end_byte()];
      let line = bytes.split(|b| *b == b'\n').next().unwrap_or(bytes);
      String::from_utf8_lossy(line).into_owned()
  }

  // tests use tree_sitter_rust + a stub LanguageHooks for the `Rust` arm
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast/mod.rs">AstChunkConfig + LanguageHooks + non_whitespace_size + process_node + merge_small_chunks + abbreviated_definition + ≥6 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast/mod.rs">Add `pub mod boundary;` + `pub use boundary::{AstChunkConfig, LanguageHooks, process_node, merge_small_chunks, non_whitespace_size};`</file>
</files_to_modify>

<validation_criteria>
  <criterion>process_node produces AstChunk with correct line_start/line_end (1-based) and content byte range</criterion>
  <criterion>process_node's recursion respects size ≤ max_size + is_chunkable boundary condition</criterion>
  <criterion>merge_small_chunks merges below-min siblings at the same parent_chain depth</criterion>
  <criterion>merge_small_chunks emits below-min standalone chunks as-is when no merge target exists</criterion>
  <criterion>over-4×-target chunks emit "chunk_too_large" warning ParseDiagnostic</criterion>
  <criterion>non_whitespace_size matches the legacy chunker's count on shared synthetic fixtures</criterion>
  <criterion>≥6 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast::boundary</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
