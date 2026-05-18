---
namespace: decisions
agent_id: codex-rust
timestamp: 2026-05-09T03:16:43Z
status: active
tags: mejepa,rust,ast,tree-sitter,fail-closed,fsv
---

# Rust AST Instrument Fails Closed On Tree-Sitter Recovery

## Decision

Rust `E_AST` extraction must reject any tree-sitter parse tree containing
`ERROR` or `MISSING` nodes before it emits chunks, latents, semantic features,
or ME-JEPA panel rows.

## Why

Tree-sitter intentionally produces recovered parse trees for malformed source.
That behavior is useful for editors, but it is unsafe for a reality-prediction
instrument: a recovered tree is not the program's syntax, and training ME-JEPA on
it would teach the model from parser repair artifacts instead of durable code
reality.

## Required Behavior

- Return a structured parse failure with node kind, byte offset, line number,
  and source excerpt.
- Do not provide fallback chunks from a recovered parse.
- Treat this as an instrument failure for the panel row.
- Verify with physical evidence from
  `/tmp/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json`, produced by
  `cargo run --release -p context-graph-core --example ast_chunker_fsv`.

## Verification Status

Verified manually on 2026-05-09 using real Rust sources written to disk, `rustc`
as an independent syntax source of truth, chunker execution, and evidence JSON
readback.
