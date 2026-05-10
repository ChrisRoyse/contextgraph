# TASK-AST-021 — Audit canonical `chunk` facade + cross-language test harness

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth. Any lower reference to `src/memory/ast_chunker/<submodule>.rs`, `dynamicjepa.rs`, compatibility shims, or "create new" scaffolding language is superseded by this audit note. Verify and harden the existing canonical files instead of creating parallel modules unless the repo audit proves a file is genuinely missing.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task (Rust source file, JSON evidence file, RocksDB CF, or ME-JEPA panel file), run the logic, then perform a separate readback from that source of truth. Print before/after system state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution, not only function return values.
>
> **TASK-AST-021 correction:** Any lower instruction to preserve `AstCodeChunker`, translate `AstChunk` into legacy `CodeChunk`, or add compatibility shims is superseded. The implementation target is the current canonical `memory::ast` API; update callers to that API or fail fast with compile errors and actionable logs.

```xml
<task_spec id="TASK-AST-021" version="1.0">
<metadata>
  <title>Audit canonical chunk() facade + cross-language integration tests; remove stale shim expectations</title>
  <status>audit-hardening</status>
  <layer>surface</layer>
  <sequence>21</sequence>
  <implements>
    <item ref="REQ-AST-01">Uniform chunk(source, language) public API</item>
    <item ref="REQ-AST-02">Chunk metadata fully populated (sha256 + embedder_routing_set + content)</item>
    <item ref="REQ-AST-06">embedder_routing_set populated at chunk emission via TASK-AST-005 router</item>
    <item ref="REQ-AST-10">EmptySource fail-closed</item>
    <item ref="REQ-AST-13">Determinism across runs</item>
    <item ref="REQ-AST-14">Superseded: no backwards compatibility; update callers to canonical API or fail fast</item>
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
  <estimated_files>current canonical files only: ast/mod.rs, ast/classify.rs, ast/routing.rs, ast/tests.rs, and callers</estimated_files>
</metadata>

<context>
This is the convergence task: wire all 11 per-language classifiers + dispatcher + boundary
engine + hash + router + diagnostic collector + mixed-language detector together behind
a single `pub fn chunk(source: &[u8], language: Language) -> Result<Vec<AstChunk>, AstChunkerError>`.
TypeScript/TSX routing is handled by `Language::tree_sitter_language` in `ast/mod.rs`.
Rust and the other 10 languages are already centralized in `ast/classify.rs`. The audit
work is to keep that canonical path complete, remove stale shim expectations, and ensure
callers such as ME-JEPA instruments use the canonical API directly.
</context>

<input_context_files>
  <file purpose="all_layer1_2_components">crates/context-graph-core/src/memory/ast/ (all sibling modules from TASK-AST-001..020)</file>
  <file purpose="legacy_chunker">crates/context-graph-core/src/memory/ast_chunker.rs (legacy Rust-only implementation; do not extend as Phase 1 source of truth)</file>
  <file purpose="callers">crates/context-graph-mejepa-instruments/src/e_code.rs and any repo callers of memory::ast</file>
  <file purpose="component_contract_facade">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#component_contracts (AstChunker top-level facade)</file>
</input_context_files>

<prerequisites>
  <check>All TASK-AST-001..020 complete</check>
  <check>existing Rust chunker tests pass (`cargo test -p context-graph-core` baseline)</check>
</prerequisites>

<scope>
  <in_scope>
    - Audit the existing Rust classifier in `crates/context-graph-core/src/memory/ast/classify.rs`
      against real Rust fixtures and the legacy Rust-only implementation only as a reference.
    - Implement `pub fn chunk(source: &[u8], language: Language) -> Result<Vec<AstChunk>, AstChunkerError>` at `crates/context-graph-core/src/memory/ast/mod.rs`
    - Keep TypeScript and TSX dispatch in `Language::tree_sitter_language`; do not add a parallel facade.
    - Algorithm:
      1. detect_mixed_language(source, language) → bail on Err(MixedLanguage)
      2. Empty/whitespace check → Err(EmptySource)
      3. parser_for_language(language) → Parser
      4. parser.parse(source, None) → Tree (or Err ParseFailed)
      5. root.has_error() → collect parse_diagnostics → Err(ParseFailed)
      6. `walk_and_emit` traverses named nodes, classifies entities, builds `AstChunk` rows,
         computes structural SHA, and resolves route sets.
      7. Return `Ok(chunks)` only after chunks have content, line spans, SHA, and non-empty routes.
    - Keep cross-language integration tests in `crates/context-graph-core/src/memory/ast/tests.rs`
      and the FSV examples; do not add a disconnected integration harness unless it covers a gap.
    - Determinism integration test: chunk a fixture twice in-process; serialize both Vec<AstChunk>
      via serde_json with sorted keys; assert byte-equal
    - Module re-exports updated: `pub use chunk::{chunk, chunk_typescript_tsx};`
  </in_scope>
  <out_of_scope>
    - FSV harness binaries (TASK-AST-022)
    - Reformat-invariance tests (TASK-AST-022 — needs language toolchains)
    - Cross-process determinism (TASK-AST-022 — needs separate binary)
    - Preserving legacy API compatibility
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast/mod.rs">
      pub fn chunk(source: &[u8], language: Language) -> Result<Vec<AstChunk>, AstChunkerError>;
      pub fn chunk_path(source: &[u8], path: &str) -> Result<Vec<AstChunk>, AstChunkerError>;
      pub fn chunk_with_options(
          source: &[u8],
          language: Language,
          options: &AstChunkOptions,
      ) -> Result<Vec<AstChunk>, AstChunkerError>;

      /// Uniform multi-language AST-perfect chunker entry point.
      ///
      /// Returns Vec<AstChunk> with all metadata populated (sha256, embedder_routing_set, content)
      /// per REQ-AST-02. Fails closed on parse errors / empty source / unsupported language /
      /// embedded foreign language.
      pub fn chunk(source: &[u8], language: Language) -> Result<Vec<AstChunk>, AstChunkerError> { /* ... */ }

    </signature>
  </signatures>

  <constraints>
    - hooks_for is exhaustive over Language; adding a 12th variant breaks the build
    - chunk() facade fails closed on every error path; empty source returns Err(EmptySource).
    - sha256 is computed from Tree-sitter structural tokens plus language/entity/scope context.
    - Legacy shims are not required; stale callers should be moved to canonical `memory::ast`.
    - GENERIC-ONLY: facade does not have per-repo or per-task branches
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds (existing tests still compile)
    - `cargo test -p context-graph-core memory::ast::tests` passes.
    - `cargo run -p context-graph-core --example ast_chunker_multilang_fsv` writes and rereads aggregate evidence.
    - `cargo run -p context-graph-core --example swebench_ast_fsv` verifies against real Phase 0 SWE-bench corpus data.
    - All 11 languages: real-source fixture → chunk count > 0, every sha256 distinct, every
      embedder_routing_set non-empty, language tag matches input
    - Empty source → Err(EmptySource)
    - PHP+HTML → Err(MixedLanguage)
    - Unsupported language path: call Language::from_slug("not-a-language") and assert MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE; adding a future enum variant must fail exhaustive dispatch audits at compile time
    - Determinism: chunk twice in-process; serde_json bytes are equal
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
    - Any remaining legacy caller either imports `memory::ast` directly or fails compilation with a clear migration target.
  </verification>
</definition_of_done>

<current_files_to_audit>
  <file path="crates/context-graph-core/src/memory/ast/mod.rs">Canonical facade, parser dispatch, diagnostics, chunk construction, structural hash, mixed-language gate.</file>
  <file path="crates/context-graph-core/src/memory/ast/classify.rs">All 11 language classifiers.</file>
  <file path="crates/context-graph-core/src/memory/ast/routing.rs">Embedder routing and fail-closed route errors.</file>
  <file path="crates/context-graph-core/src/memory/ast/tests.rs">Unit coverage for routing, classifiers, hashes, parse failures, and mixed-language rejection.</file>
  <file path="crates/context-graph-mejepa-instruments/src/e_code.rs">Downstream consumer that must use canonical AST chunking.</file>
</current_files_to_audit>

<validation_criteria>
  <criterion>Valid real-source fixtures for all 11 languages return non-empty chunks.</criterion>
  <criterion>Every chunk has language, entity_type, line span, byte span, content, sha256, and non-empty route set.</criterion>
  <criterion>Empty source returns `MEJEPA_CHUNKER_EMPTY_SOURCE`.</criterion>
  <criterion>Invalid syntax returns `MEJEPA_CHUNKER_PARSE_FAILED` with diagnostics.</criterion>
  <criterion>Mixed PHP/HTML or JS/HTML returns `MEJEPA_CHUNKER_MIXED_LANGUAGE`.</criterion>
  <criterion>Repeated chunking of identical source yields identical serialized evidence.</criterion>
  <criterion>No caller depends on a new compatibility shim; stale callers are updated or fail compilation.</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core memory::ast::tests</command>
  <command>cargo run -p context-graph-core --example ast_chunker_multilang_fsv</command>
  <command>cargo run -p context-graph-core --example swebench_ast_fsv</command>
  <command>cargo run -p context-graph-mejepa-instruments --example ast_embedder_smoke_fsv</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```
