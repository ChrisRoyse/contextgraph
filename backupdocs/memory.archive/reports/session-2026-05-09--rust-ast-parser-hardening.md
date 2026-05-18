---
namespace: reports
agent_id: codex-rust
timestamp: 2026-05-09T03:31:02Z
status: active
tags: rust,mejepa,ast,tree-sitter,fail-closed,fsv
---

# Rust AST Parser Harness Hardening

## Scope

Focused on Rust AST parser/chunker hardening for ME-JEPA `E_AST`. Other
language work in the same working tree was treated as concurrent agent work and
not reverted.

## Root Cause

The Rust AST chunker depended on tree-sitter's recovered parse tree. For an
editor this is useful, but for ME-JEPA it is unsafe: a tree containing `ERROR`
or `MISSING` nodes is parser recovery, not durable code reality. Emitting chunks
from that tree can train the predictor on repaired syntax artifacts.

The Rust FSV harness also initially used an impl fixture that was too small to
force method splitting, so it could not prove that impl functions were surfaced
as `Method` chunks.

## Resolution

- Current `context-graph-core` Rust AST chunker rejects recovered parse trees
  with `AstChunkerError::ParseFailed`, structured diagnostics, and tracing error
  logs before chunk emission.
- Impl-contained `function_item` nodes are classified as `EntityType::Method`.
- `crates/context-graph-core/examples/ast_chunker_fsv.rs` now writes a larger,
  rustc-valid `method_split.rs` fixture with two inherent methods and verifies
  the method fixture with `rustc` before chunking.
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` and
  `memory/context/mejepa-implementation-initiative.md` now record the Rust
  `E_AST` fail-closed invariant and evidence source of truth.
- Durable decision recorded in
  `memory/decisions/2026-05-09--rust-ast-instrument-fail-closed.md`.

## Full State Verification

Source of truth:

- Rust AST harness:
  `/tmp/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json`
- Generated Rust source/rustc evidence:
  `/tmp/contextgraph-rust-ast-chunker-fsv/sources/*` and
  `/tmp/contextgraph-rust-ast-chunker-fsv/rustc/*`
- ME-JEPA corpus harness:
  `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`
- Split assignment source of truth:
  `/tmp/contextgraph-mejepa-corpus-fsv/split-assignments.json`

Observed readback from Rust evidence:

- Happy path chunked successfully: `chunk_count=1`.
- Invalid Rust failed closed: `kind=parse_failed`, first diagnostic
  `node_kind=ERROR`, `start_line=2`, excerpt `let value = ;`.
- Empty source failed closed: `kind=empty_source`.
- Method split produced `method_chunk_count=2`.
- Independent rustc checks: happy `true`, invalid `false`, method split `true`.
- Physical files existed: evidence JSON 5342 bytes; `happy.rmeta` 3000 bytes;
  `method_split.rmeta` 2754 bytes; source files `happy.rs`, `invalid.rs`,
  `method_split.rs`.

Observed readback from corpus evidence:

- `delete_test_call` replaced `assert score >= 0.0` with `pass` and passed.
- `compile_error` mutation passed harness expectations while `py_compile.ok`
  was `false`.
- Split FSV passed: row count 300, unique task IDs 300, leakage check true,
  bucket sizes `[195,77,28]`.
- Edge cases failed closed with expected codes:
  empty input, wrong-file missing alternate, no assertion, docstring-only boolean,
  no boolean operator, docstring-only integer, floats-only integer.

## Verification Commands

- `cargo test -p context-graph-mejepa-corpus` passed 50 tests.
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-core --lib ast_chunker -- --nocapture`
  passed 17 tests.
- `CARGO_BUILD_JOBS=1 cargo run -p context-graph-core --example ast_chunker_fsv`
  passed after hardening the method fixture.
- `cargo run -p context-graph-mejepa-corpus --example mejepa_corpus_fsv` passed.
- `CARGO_BUILD_JOBS=1 cargo build --release -p context-graph-core --example ast_chunker_fsv -p context-graph-mejepa-corpus`
  failed in `librocksdb-sys` C++ compilation with `virtual memory exhausted:
  Cannot allocate memory` while unrelated cargo/CUDA builds were active. No Rust
  parser code error was emitted.

## Web Research Used

- Tree-sitter Rust `Node` docs: `has_error()` reports syntax errors in a node or
  descendants; `is_error()` identifies syntax-error nodes; `is_missing()`
  identifies parser-inserted recovery nodes.
- Tree-sitter query docs: `ERROR` and `MISSING` are special parse-recovery nodes
  and `MISSING` can be inserted by parser recovery.

## Next Risk

The workspace release build currently depends on compiling `librocksdb-sys`
even for the AST FSV example. A release verification should be rerun when the
machine is not under concurrent heavy builds, or the crate graph should be
factored so Rust AST extraction can be release-built without unrelated RocksDB
native compilation.
