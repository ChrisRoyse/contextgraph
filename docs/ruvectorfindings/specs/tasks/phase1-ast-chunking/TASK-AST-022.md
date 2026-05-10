# TASK-AST-022 — Audit Phase 1 FSV Harnesses And Persisted Evidence

> **2026-05-10 current-state audit:** Treat this document as an audit/hardening task for the existing implementation, not greenfield scaffolding. Canonical Phase 1 source is `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}` with ME-JEPA consumer `crates/context-graph-mejepa-instruments/src/e_code.rs`. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists but is legacy and must not be extended as the multi-language source of truth.
>
> **No fallbacks / no backwards compatibility:** Keep one canonical API: `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`. Unsupported language, empty input, mixed language, routing gaps, invalid UTF-8, missing toolchains, failed compiler/linter checks, or Tree-sitter `ERROR`/`MISSING` nodes must return a typed failure with actionable stdout/stderr/diagnostics. Do not WARN-and-pass, silently skip, or use mock data.
>
> **Mandatory Full State Verification:** Define the source of truth, run the logic, then perform a separate readback from the source of truth. Print before/after state for happy path and 3 edge cases: empty source, invalid syntax, and mixed/unsupported language. Evidence must include actual chunks/routes/diagnostics read from disk or storage after execution.

```xml
<task_spec id="TASK-AST-022" version="2.0">
<metadata>
  <title>Audit Phase 1 FSV harnesses, real corpus readback, and persisted evidence</title>
  <status>audit-hardening</status>
  <layer>surface</layer>
  <sequence>22</sequence>
  <implements>
    <item ref="REQ-AST-07">Persisted FSV evidence for all 11 languages</item>
    <item ref="REQ-AST-13">Deterministic serialized chunk evidence</item>
    <item ref="REQ-AST-15">Performance and scale smoke evidence where available</item>
    <item ref="FSV">Source-of-truth readback after execution</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-021</task_ref>
  </depends_on>
</metadata>

<current_state>
  <file path="crates/context-graph-core/examples/ast_chunker_multilang_fsv.rs">Primary 11-language FSV harness. Writes `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json` and per-language evidence copies. It writes real source fixtures, runs real toolchain checks, chunks via canonical `memory::ast`, writes JSON, rereads JSON, and asserts chunk/entity/route invariants.</file>
  <file path="crates/context-graph-core/examples/swebench_ast_fsv.rs">Real Phase 0 corpus harness. Uses `/tmp/contextgraph-phase0-smoke-one-task`, repo cache, manifest `tasks/swebench-lite/sympy__sympy-20590.json`, applies the real patch, runs `python3 -m py_compile`, chunks the changed source, and writes `/tmp/contextgraph-swebench-ast-fsv/swebench-ast-evidence.json`.</file>
  <file path="crates/context-graph-mejepa-instruments/examples/ast_embedder_smoke_fsv.rs">End-to-end AST-to-ME-JEPA smoke. Chunks 11 real `samplecode/` files, builds a full ME-JEPA panel, writes `/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json`, rereads it, and asserts all slots/chunks/routes.</file>
</current_state>

<source_of_truth>
  <path>/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json</path>
  <path>/tmp/contextgraph-swebench-ast-fsv/swebench-ast-evidence.json</path>
  <path>/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json</path>
</source_of_truth>

<scope>
  <in_scope>
    - Run the three current FSV examples and inspect the JSON files after execution.
    - Verify the aggregate harness has exactly 11 languages and each language has non-empty chunks.
    - Verify each chunk row includes `entity_type`, `parent_chain`, `line_start`, `line_end`, `sha256`, route set, and source preview/content.
    - Verify route sets match `route_to_embedders(entity_type)`; route mismatches are failures.
    - Verify the edge case evidence contains `empty`, `invalid_syntax`, and `mixed_language` entries with `passes=true` and the expected error code.
    - Verify SWE-bench evidence uses real Phase 0 data and the patched source has expected class/method chunks.
    - Verify ME-JEPA evidence rereads `panel.json` and `chunks.json`, fills every instrument slot, and includes chunk readback for all 11 languages.
    - Add or harden reformat-invariance checks only if they use real formatters. Missing formatter is a failure with stdout/stderr; it is not a pass.
    - Add performance timing to evidence if changing chunking behavior: record source bytes, chunk count, wall-clock latency, and hardware/context notes.
  </in_scope>
  <out_of_scope>
    - Docker/SWE-bench evaluator orchestration.
    - Mock fixtures, synthetic-only corpus completion claims, or warning-only missing-toolchain behavior.
    - New per-language binaries unless the existing aggregate harness cannot provide the required evidence.
  </out_of_scope>
</scope>

<definition_of_done>
  <verification>
    - `cargo run -p context-graph-core --example ast_chunker_multilang_fsv` exits 0.
    - Read `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json`; assert `language_count == 11`, every language has `after_state.chunk_count > 0`, and every edge case has `passes=true`.
    - `cargo run -p context-graph-core --example swebench_ast_fsv` exits 0.
    - Read `/tmp/contextgraph-swebench-ast-fsv/swebench-ast-evidence.json`; assert `passes=true`, manifest/patch/repo paths exist, and chunks contain both `Class` and `Method`.
    - `cargo run -p context-graph-mejepa-instruments --example ast_embedder_smoke_fsv` exits 0.
    - Read `/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json`; assert `passes=true`, `sample_reports` length is 11, and `filled_slots` equals the full instrument slot list.
    - Manually print before/after state snippets for `empty_source`, `invalid_syntax`, and `mixed_language_or_unsupported_language`.
    - If a compiler, interpreter, formatter, repo cache, manifest, patch, or JSON readback is missing, the task fails fast with the missing path/command and stderr.
  </verification>
</definition_of_done>

<test_commands>
  <command>cargo test -p context-graph-core memory::ast::tests</command>
  <command>cargo run -p context-graph-core --example ast_chunker_multilang_fsv</command>
  <command>jq '{language_count, edge_cases}' /tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json</command>
  <command>cargo run -p context-graph-core --example swebench_ast_fsv</command>
  <command>jq '{passes, before_state, after_state: {chunk_count, chunks}}' /tmp/contextgraph-swebench-ast-fsv/swebench-ast-evidence.json</command>
  <command>cargo run -p context-graph-mejepa-instruments --example ast_embedder_smoke_fsv</command>
  <command>jq '{passes, happy_path: {after_state: {panel_dim, filled_slots, sample_reports}}}' /tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json</command>
</test_commands>
</task_spec>
```
