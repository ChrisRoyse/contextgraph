---
namespace: journal
agent_id: codex-rust
timestamp: 2026-05-09T03:12:05Z
status: active
tags: mejepa, rust, ast, fsv, failure
---

# Failure: Rust AST FSV method-split fixture was too small

## What Happened
`CARGO_BUILD_JOBS=1 cargo run -p context-graph-core --example ast_chunker_fsv`
failed with:

`Rust AST chunker FSV invariant failed: method split must produce at least one Method chunk`

## Why It Matters
The chunker logic was correct: the synthetic `impl Unit { pub fn id(...) }`
fixture fit under the configured max chunk size, so the parser emitted the
whole `impl` as a single chunk instead of recursing into the contained
`function_item`. The FSV fixture did not actually trigger the boundary it was
claiming to test.

## Related
- `crates/context-graph-core/examples/ast_chunker_fsv.rs`
- `crates/context-graph-core/src/memory/ast_chunker.rs`
