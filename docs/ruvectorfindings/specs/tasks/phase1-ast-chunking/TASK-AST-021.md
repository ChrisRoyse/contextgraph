# TASK-AST-021 — Implement `chunk` facade + cross-language test harness + REQ-AST-14 backwards compat

```xml
<task_spec id="TASK-AST-021" version="1.0">
<metadata>
  <title>Implement uniform chunk() facade + cross-language integration tests + Rust legacy API shim</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>21</sequence>
  <implements>
    <item ref="REQ-AST-01">Uniform chunk(source, language) public API</item>
    <item ref="REQ-AST-02">Chunk metadata fully populated (sha256 + embedder_routing_set + content)</item>
    <item ref="REQ-AST-06">embedder_routing_set populated at chunk emission via TASK-AST-005 router</item>
    <item ref="REQ-AST-10">EmptySource fail-closed</item>
    <item ref="REQ-AST-13">Determinism across runs</item>
    <item ref="REQ-AST-14">Backwards compat — AstCodeChunker::new_rust + chunk(&str, &str) preserved</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-005</task_ref>
    <task_ref>TASK-AST-006</task_ref>
    <task_ref>TASK-AST-007</task_ref>
    <task_ref>TASK-AST-008</task_ref>
    <task_ref>TASK-AST-009</task_ref>
    <task_ref>TASK-AST-011</task_ref>
    <task_ref>TASK-AST-012</task_ref>
    <task_ref>TASK-AST-013</task_ref>
    <task_ref>TASK-AST-014</task_ref>
    <task_ref>TASK-AST-015</task_ref>
    <task_ref>TASK-AST-016</task_ref>
    <task_ref>TASK-AST-017</task_ref>
    <task_ref>TASK-AST-018</task_ref>
    <task_ref>TASK-AST-019</task_ref>
    <task_ref>TASK-AST-020</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>3 files modified, 1 new file (Rust classifier shim)</estimated_files>
</metadata>

<context>
This is the convergence task: wire all 11 per-language classifiers + dispatcher + boundary
engine + hash + router + diagnostic collector + mixed-language detector together behind
a single `pub fn chunk(source: &[u8], language: Language) -> Result<Vec<AstChunk>, AstChunkerError>`.
Also exposes `pub fn chunk_typescript_tsx(source) -> ...` for TSX dialect. Adds a Rust
classifier (port of the legacy is_chunkable_node + EntityType::from_rust_node_kind logic into
the new LanguageHooks shape) so the existing Rust path uses the new pipeline. Adds a
backwards-compat shim translating new AstChunk → legacy CodeChunk so existing call sites
in context-graph-storage continue to compile (REQ-AST-14). Cross-language integration
tests assert end-to-end behavior on real fixtures for all 11 languages.
</context>

<input_context_files>
  <file purpose="all_layer1_2_components">crates/context-graph-core/src/memory/ast_chunker/ (all sibling modules from TASK-AST-001..020)</file>
  <file purpose="legacy_chunker">crates/context-graph-core/src/memory/ast_chunker.rs (1216 LOC; backwards-compat surface to preserve)</file>
  <file purpose="legacy_consumers">crates/context-graph-storage (consumers of legacy CodeChunk type — must still compile)</file>
  <file purpose="component_contract_facade">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#component_contracts (AstChunker top-level facade)</file>
</input_context_files>

<prerequisites>
  <check>All TASK-AST-001..020 complete</check>
  <check>existing Rust chunker tests pass (`cargo test -p context-graph-core` baseline)</check>
</prerequisites>

<scope>
  <in_scope>
    - Add Rust classifier module: `crates/context-graph-core/src/memory/ast_chunker/classifier/rust.rs`
      porting legacy `EntityType::from_rust_node_kind` + `is_chunkable_node` + `get_node_name` +
      `is_container_node` into LanguageHooks shape; map legacy variants to new EntityType:
        legacy Function/Method → Function/Method
        legacy Struct/Enum/Trait/Impl/Module → Struct/Enum/TraitOrInterface/Impl/Module
        legacy Const/Static/TypeAlias/Macro → Function (BR-AST-03 closest fit; doc 14 §5 cell
          shrinkage)
        legacy Comment → CommentBlock (or Docstring per attached-to-entity heuristic)
        legacy Mixed/Unknown → never reaches new EntityType (filter via is_chunkable)
    - Implement `pub fn chunk(source: &[u8], language: Language) -> Result<Vec<AstChunk>, AstChunkerError>` at `crates/context-graph-core/src/memory/ast_chunker/mod.rs`
    - Implement `pub fn chunk_typescript_tsx(source: &[u8]) -> Result<Vec<AstChunk>, AstChunkerError>` (uses parser_for_typescript_tsx + classifier::typescript::hooks_tsx)
    - Algorithm:
      1. detect_mixed_language(source, language) → bail on Err(MixedLanguage)
      2. Empty/whitespace check → Err(EmptySource)
      3. parser_for_language(language) → Parser
      4. parser.parse(source, None) → Tree (or Err ParseFailed)
      5. root.has_error() → collect parse_diagnostics → Err(ParseFailed)
      6. Pick LanguageHooks per language (match on Language enum → classifier module's hooks())
      7. extract_imports (per-language; reuse from boundary engine)
      8. process_node walks tree → Vec<AstChunk> with sha256=[0;32], routing_set=empty
      9. merge_small_chunks
      10. For each chunk: compute_chunk_sha256 (re-parse the chunk's byte range OR walk the
          original tree at the chunk's node; the latter is more efficient — keep a (node_ptr, chunk)
          pair in process_node and pass through)
      11. For each chunk: route_to_embedders(entity_type) → embedder_routing_set
      12. For each chunk: contextualize() prepend file path / imports / parent_def to content
          (re-implementing the legacy contextualize() against the new AstChunk shape)
      13. Return Ok(chunks)
    - Add `pub fn hooks_for(language: Language) -> LanguageHooks` dispatcher
    - Backwards-compat shim: keep the legacy `pub struct AstCodeChunker` + `new_rust()` + `chunk(&str, &str)`
      working by routing internally through the new pipeline + translating AstChunk→CodeChunk
      via a dedicated `fn ast_chunk_to_code_chunk(chunk: AstChunk) -> CodeChunk` mapper
    - Cross-language integration tests at `crates/context-graph-core/tests/ast_chunker_cross_language.rs`:
      ≥11 tests (one per language) using a small real-source fixture; assert chunk count > 0,
      sha256 are all distinct, language tag matches, every chunk has non-empty embedder_routing_set
    - Determinism integration test: chunk a fixture twice in-process; serialize both Vec<AstChunk>
      via serde_json with sorted keys; assert byte-equal
    - Module re-exports updated: `pub use chunk::{chunk, chunk_typescript_tsx};`
  </in_scope>
  <out_of_scope>
    - FSV harness binaries (TASK-AST-022)
    - Reformat-invariance tests (TASK-AST-022 — needs language toolchains)
    - Cross-process determinism (TASK-AST-022 — needs separate binary)
    - Migrating context-graph-storage consumers OFF the legacy API (Phase 2; per REQ-AST-14)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/mod.rs">
      pub mod boundary;
      pub mod classifier;
      pub mod diagnostics;
      pub mod dispatch;
      pub mod entity_type;
      pub mod error;
      pub mod hash;
      pub mod language;
      pub mod mixed_lang;
      pub mod router;
      pub mod types;

      pub use boundary::{AstChunkConfig, LanguageHooks};
      pub use classifier::{rust, python, javascript, typescript, go, java, c, cpp, csharp, ruby, php};
      pub use diagnostics::parse_diagnostics;
      pub use dispatch::{parser_for_language, parser_for_typescript_tsx};
      pub use entity_type::{ClassificationContext, EntityType};
      pub use error::AstChunkerError;
      pub use hash::compute_chunk_sha256;
      pub use language::Language;
      pub use mixed_lang::detect_mixed_language;
      pub use router::route_to_embedders;
      pub use types::{AstChunk, EmbedderId, ParseDiagnostic, MAX_PARSE_DIAGNOSTICS};

      /// Uniform multi-language AST-perfect chunker entry point.
      ///
      /// Returns Vec<AstChunk> with all metadata populated (sha256, embedder_routing_set, content)
      /// per REQ-AST-02. Fails closed on parse errors / empty source / unsupported language /
      /// embedded foreign language.
      pub fn chunk(source: &[u8], language: Language) -> Result<Vec<AstChunk>, AstChunkerError> { /* ... */ }

      /// TSX-dialect chunker entry point for .tsx files.
      pub fn chunk_typescript_tsx(source: &[u8]) -> Result<Vec<AstChunk>, AstChunkerError> { /* ... */ }

      pub fn hooks_for(language: Language) -> LanguageHooks { /* ... */ }
    </signature>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/classifier/rust.rs">
      use tree_sitter::Node;
      use crate::memory::ast_chunker::{ClassificationContext, EntityType, LanguageHooks};

      pub fn hooks() -> LanguageHooks { /* ... */ }
      pub fn classify(node: &Node<'_>, source: &[u8], ctx: &ClassificationContext) -> EntityType { /* ... */ }
      pub fn is_chunkable(node: &Node<'_>) -> bool { /* ... */ }
      pub fn get_node_name(node: &Node<'_>, source: &[u8]) -> Option<String> { /* ... */ }
      pub fn is_container(node: &Node<'_>) -> bool { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - hooks_for is exhaustive over Language; adding a 12th variant breaks the build
    - chunk() facade fails closed on every error path; never returns Ok(empty Vec) (per REQ-AST-10
      empty source returns Err(EmptySource))
    - sha256 is computed on the actual tree-sitter Node, NOT by re-parsing the chunk's byte range
      (re-parsing would lose parent_chain context and break determinism)
    - Implementation strategy for sha256 propagation: process_node accepts `&mut Vec<AstChunkBuilder>`
      where AstChunkBuilder has the chunk's tree node ID; after merge, walk builders to compute
      sha256 with original tree intact. The tree's lifetime must outlive the chunks construction —
      keep tree alive until sha256 + routing fill-in complete
    - Backwards-compat: legacy AstCodeChunker remains compilable; existing tests pass; legacy
      EntityType variants are re-exported under a deprecated alias (or kept in the legacy
      module). Migration to new EntityType is Phase 2 work.
    - File layout: split `mod.rs` if it exceeds 500 lines; the chunk() facade may live in
      `crates/context-graph-core/src/memory/ast_chunker/chunk.rs` re-exported from mod.rs
    - GENERIC-ONLY: facade does not have per-repo or per-task branches
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds (existing tests still compile)
    - `cargo test -p context-graph-core` baseline passes (REQ-AST-14 backwards compat)
    - `cargo test -p context-graph-core memory::ast_chunker` all unit tests pass (already covered
      by TASK-AST-001..020)
    - `cargo test --test ast_chunker_cross_language -p context-graph-core` all 11 + 1 determinism + edge cases pass
    - All 11 languages: real-source fixture → chunk count > 0, every sha256 distinct, every
      embedder_routing_set non-empty, language tag matches input
    - Empty source → Err(EmptySource)
    - PHP+HTML → Err(MixedLanguage)
    - Unsupported language stub: pass a future variant via #[non_exhaustive] cfg-gated test —
      compile-time exhaustiveness check
    - Determinism: chunk twice in-process; serde_json bytes are equal
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
    - `cargo build --release -p context-graph-storage` succeeds (downstream consumer of legacy API)
  </verification>
</definition_of_done>

<pseudo_code>
ast_chunker/mod.rs (additions):
  pub fn chunk(source: &[u8], language: Language) -> Result<Vec<AstChunk>, AstChunkerError> {
      // 1. Mixed-lang
      detect_mixed_language(source, language)?;
      // 2. Empty
      if source.is_empty() || source.iter().all(|b| b.is_ascii_whitespace()) {
          return Err(AstChunkerError::EmptySource);
      }
      // 3. Parse
      let mut parser = parser_for_language(language)?;
      let tree = parser.parse(source, None).ok_or_else(|| AstChunkerError::ParseFailed {
          file_path: String::new(), language, reason: "tree-sitter returned None".into(),
          diagnostics: vec![],
      })?;
      let root = tree.root_node();
      if root.has_error() {
          let diagnostics = parse_diagnostics(root, source);
          tracing::error!(
              code = AstChunkerError::ParseFailed { file_path: String::new(), language,
                  reason: String::new(), diagnostics: vec![] }.code(),
              ?diagnostics,
              ?language,
              "AST chunker fail-closed on tree-sitter ERROR/MISSING",
          );
          return Err(AstChunkerError::ParseFailed {
              file_path: String::new(), language,
              reason: "tree-sitter produced ERROR or MISSING nodes".into(),
              diagnostics,
          });
      }
      // 4. Pick hooks
      let hooks = hooks_for(language);
      // 5. Walk + merge — produces partial chunks with sha256=[0;32], routing_set=empty
      let mut chunks = Vec::new();
      let mut scope_chain = Vec::new();
      let mut ctx = ClassificationContext::default();
      let imports = extract_imports(&root, source, language);
      let config = AstChunkConfig::default();
      // Track (node_id, idx) so we can locate the original AST node for sha256 later
      // (process_node populates a parallel Vec<NodeRef>)
      let mut node_refs: Vec<NodeRef> = Vec::new();
      boundary::process_node(&root, source, language, &hooks, "", &imports, &mut scope_chain,
                             None, &mut ctx, &mut chunks, &config);
      let chunks = boundary::merge_small_chunks(chunks, &config);
      // 6. Fill sha256 and routing
      for chunk in chunks.iter_mut() {
          // Re-locate the chunk's root node by line range — bounded walk root.descendant_for_byte_range
          // (bytes recovered from chunk.content prefix matching source[start_byte..end_byte])
          let start_byte = locate_byte_offset(source, chunk.line_start);
          let end_byte = locate_byte_offset_end(source, chunk.line_end);
          let chunk_node = root.descendant_for_byte_range(start_byte, end_byte).unwrap_or(root);
          chunk.sha256 = compute_chunk_sha256(&chunk_node, source, language, &chunk.parent_chain);
          chunk.embedder_routing_set = route_to_embedders(chunk.entity_type);
      }
      Ok(chunks)
  }

  pub fn chunk_typescript_tsx(source: &[u8]) -> Result<Vec<AstChunk>, AstChunkerError> {
      // Same body as chunk() but uses parser_for_typescript_tsx + typescript::hooks_tsx
      // Refactor common path into helper; tsx is a thin wrapper.
      ...
  }

  pub fn hooks_for(language: Language) -> LanguageHooks {
      match language {
          Language::Rust => classifier::rust::hooks(),
          Language::Python => classifier::python::hooks(),
          Language::JavaScript => classifier::javascript::hooks(),
          Language::TypeScript => classifier::typescript::hooks(),
          Language::Go => classifier::go::hooks(),
          Language::Java => classifier::java::hooks(),
          Language::C => classifier::c::hooks(),
          Language::Cpp => classifier::cpp::hooks(),
          Language::CSharp => classifier::csharp::hooks(),
          Language::Ruby => classifier::ruby::hooks(),
          Language::Php => classifier::php::hooks(),
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/rust.rs">Rust classifier in new LanguageHooks shape (port of legacy logic) + 6 unit tests</file>
  <file path="crates/context-graph-core/src/memory/ast_chunker/chunk.rs">chunk() + chunk_typescript_tsx() + hooks_for() facade — split out for &lt;500-line constraint</file>
  <file path="crates/context-graph-core/tests/ast_chunker_cross_language.rs">≥13 integration tests — 11 per-language + 1 determinism + 1 mixed-lang fail-closed + 1 empty-source fail-closed</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/mod.rs">Add `pub mod chunk;` and re-exports; preserve legacy module re-exports for REQ-AST-14</file>
  <file path="crates/context-graph-core/src/memory/ast_chunker/classifier/mod.rs">Add `pub mod rust;`</file>
  <file path="crates/context-graph-core/src/memory/ast_chunker.rs">Convert this single file into the directory module — move existing public types under `mod legacy;` re-export, OR delete and let the directory module take over (operator decision; recommend the latter with a `#[deprecated]`-tagged shim re-exporting under `pub use legacy::AstCodeChunker;` etc., so existing imports continue to work)</file>
</files_to_modify>

<validation_criteria>
  <criterion>chunk() returns Ok(Vec<AstChunk>) for valid source in every supported language</criterion>
  <criterion>chunk() returns Err(EmptySource) for empty / whitespace-only source</criterion>
  <criterion>chunk() returns Err(MixedLanguage) for PHP+HTML source</criterion>
  <criterion>chunk() returns Err(ParseFailed) for source with tree-sitter ERROR/MISSING</criterion>
  <criterion>chunk_typescript_tsx() handles JSX expressions correctly</criterion>
  <criterion>Determinism: in-process double-call produces byte-equal serde_json output</criterion>
  <criterion>Every chunk has non-empty embedder_routing_set per its EntityType</criterion>
  <criterion>Every chunk has 32-byte sha256, distinct across chunks of distinct entities</criterion>
  <criterion>Existing Rust-only test suite (`cargo test -p context-graph-core` baseline) passes (REQ-AST-14)</criterion>
  <criterion>`cargo build --release -p context-graph-storage` (downstream consumer) compiles unchanged</criterion>
  <criterion>≥13 cross-language integration tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo build --release -p context-graph-storage</command>
  <command>cargo test -p context-graph-core</command>
  <command>cargo test --test ast_chunker_cross_language -p context-graph-core</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
