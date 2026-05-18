---
namespace: reports
agent_id: codex
created: 2026-05-09T09:38:38Z
updated: 2026-05-09T09:38:38Z
status: active
tags: report, session, mejepa, embedders, ast-chunking, smoke-fsv, rocksdb
---

# Session Report: AST + Embedder Routing Smoke FSV

## Outcome

Implemented the missing spec-complete embedder routing layer and added an end-to-end smoke FSV that proves AST chunking, routing, and ME-JEPA panel materialization work together on real repository data plus a real SWE-bench Lite manifest.

## Files Changed

- `crates/context-graph-core/src/memory/ast/routing.rs`
- `crates/context-graph-core/src/memory/ast/mod.rs`
- `crates/context-graph-core/src/memory/mod.rs`
- `crates/context-graph-core/src/memory/ast/tests.rs`
- `crates/context-graph-mejepa-instruments/examples/ast_embedder_smoke_fsv.rs`

Note: the worktree already had concurrent dirty files and untracked docs/examples. Do not revert unrelated changes.

## Verification

- `cargo test -p context-graph-core ast::tests --no-default-features --jobs 1 -- --nocapture`
  - 10 passed, 0 failed.
- `cargo run -p context-graph-mejepa-instruments --example ast_embedder_smoke_fsv`
  - Wrote `/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json`.
  - Read back `panel.json` and `chunks.json`; evidence has `passes=true`.
  - Panel readback: `dim=5120`, `filled_mask=32767`.
  - Real sample readback: 11 languages/sample files, 136 chunks.
  - Edge cases:
    - empty Python source -> `MEJEPA_CHUNKER_EMPTY_SOURCE`
    - invalid Python syntax -> `MEJEPA_CHUNKER_PARSE_FAILED`
    - mixed PHP/HTML -> `MEJEPA_CHUNKER_MIXED_LANGUAGE`
- `target/debug/deps/context_graph_mejepa_instruments-b01a0fa7498bd846 code_instruments --nocapture`
  - 2 passed, 0 failed.
- `target/debug/examples/embedder_routing_fsv`
  - Wrote `/tmp/contextgraph-mejepa-embedders-fsv/routing_coverage.json`, `row_count=143`.
- `target/debug/examples/ast_chunker_multilang_fsv`
  - Wrote `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json`, `language_count=11`, all edge cases passed.
- `ldb` RocksDB physical readback:
  - Column families: `default`, `mejepa_mutation_corpus`, `mejepa_oracle_verdicts`.
  - `mejepa_mutation_corpus`: 8 keys.
  - `mejepa_oracle_verdicts`: 8 keys.
  - Patch-file SHA-256 values matched all 8 index entries.
- `cargo run -p context-graph-mejepa-corpus --bin mejepa-corpus -- verify --corpus /tmp/contextgraph-phase0-one-task-all-categories`
  - passed with `passes=true`, `mutation_cf_count=8`, `verdict_cf_count=8`, `failures=[]`.
- `git diff --check`
  - passed.

## Sources Consulted

- Tree-sitter query syntax docs: `ERROR` and `MISSING` nodes are explicit parse-tree artifacts and must be inspected for fail-closed parsing.
- Qodo code-RAG article: reinforces AST/CST-aware code chunking and preserving structural context in chunks.

## Residual Notes

The current corpus CLI binary at `target/debug/mejepa-corpus` is stale relative to the current corpus index schema and rejected `mutation_note`; use Cargo to rebuild it before relying on that binary. Independent `ldb` readback still verified the RocksDB source of truth.
