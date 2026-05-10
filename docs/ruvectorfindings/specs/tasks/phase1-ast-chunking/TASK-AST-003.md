# TASK-AST-003 — Audit `AstChunk` + `ParseDiagnostic` Serde Shape

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed error with actionable context; never silently route to raw-text chunks or mock data.
>
> **Mandatory Full State Verification:** Define the source of truth for the task, run the logic, then perform a separate readback from that source of truth. Print before/after state for happy path and 3 edge cases. Evidence must include actual serialized chunks/routes/diagnostics read from disk after execution.

```xml
<task_spec id="TASK-AST-003" version="2.0">
<metadata>
  <title>Audit AstChunk + ParseDiagnostic structs with deterministic serde serialization</title>
  <status>audit-hardening</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-AST-02">Chunk metadata fully populated</item>
    <item ref="REQ-AST-08">ParseDiagnostic shape for ERROR/MISSING nodes</item>
    <item ref="REQ-AST-13">Deterministic JSON serialization</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-001</task_ref>
    <task_ref>TASK-AST-002</task_ref>
    <task_ref>TASK-AST-005</task_ref>
  </depends_on>
</metadata>

<current_state>
  <file path="crates/context-graph-core/src/memory/ast/mod.rs">Defines `ParseDiagnostic`, `AstChunk`, `AstChunkOptions`, `AstChunkerError`, and the chunking facade.</file>
  <file path="crates/context-graph-core/src/memory/ast/routing.rs">Defines the real `EmbedderId` enum (`E1..E21`) and route results. Do not add placeholder aliases.</file>
</current_state>

<required_shape>
  <ast_chunk>
    language: Language;
    entity_type: EntityType;
    parent_chain: Vec&lt;String&gt;;
    line_start: u32;
    line_end: u32;
    start_byte: usize;
    end_byte: usize;
    sha256: String;
    parse_diagnostics: Vec&lt;ParseDiagnostic&gt;;
    content: String;
    embedder_routing_set: BTreeSet&lt;EmbedderId&gt;;
  </ast_chunk>
  <parse_diagnostic>
    kind: String;
    is_error: bool;
    is_missing: bool;
    start_byte: usize;
    end_byte: usize;
    start_line: u32;
    start_column: u32;
    end_line: u32;
    end_column: u32;
    excerpt: String;
  </parse_diagnostic>
</required_shape>

<scope>
  <in_scope>
    - Verify the current structs match the required shape above and derive `Serialize`/`Deserialize`.
    - Verify `sha256` is a 64-character lowercase hex string in persisted JSON evidence.
    - Verify `content` is contextual UTF-8 string content; do not switch to `serde_bytes` without a downstream migration plan.
    - Verify `embedder_routing_set` serializes deterministically through ordered route sets.
    - Verify parse diagnostics are empty on valid chunks and populated on invalid syntax failure.
  </in_scope>
  <out_of_scope>
    - Placeholder `EmbedderId` aliases.
    - Legacy `CodeChunk` compatibility shims.
    - Mock serialization tests that do not exercise real chunk output.
  </out_of_scope>
</scope>

<definition_of_done>
  <verification>
    - `cargo test -p context-graph-core memory::ast::tests` passes.
    - `cargo run -p context-graph-core --example ast_chunker_multilang_fsv` writes JSON evidence and reads it back.
    - Inspect `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json`; every chunk row has `entity_type`, `parent_chain`, `line_start`, `line_end`, `sha256`, and `routing`.
    - Edge evidence proves `invalid_syntax` returns `MEJEPA_CHUNKER_PARSE_FAILED` with diagnostics.
    - Manual happy path: serialize chunks for a real Python or Rust sample, reread JSON, and prove the reread chunk count and SHA set match the original.
  </verification>
</definition_of_done>

<test_commands>
  <command>cargo test -p context-graph-core memory::ast::tests</command>
  <command>cargo run -p context-graph-core --example ast_chunker_multilang_fsv</command>
  <command>jq '.languages[0].after_state.chunks[0]' /tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json</command>
  <command>jq '.edge_cases.invalid_syntax' /tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json</command>
</test_commands>
</task_spec>
```
