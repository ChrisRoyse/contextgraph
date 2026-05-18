---
namespace: journal
agent_id: codex-rust
timestamp: 2026-05-09T02:52:36Z
status: active
tags: mejepa, rust, fsv, failure, delete-test-call
---

# Failure: ME-JEPA corpus FSV stale DeleteTestCall invariant

## What Happened
`cargo test -p context-graph-mejepa-corpus` passed 40 tests, but
`cargo run -p context-graph-mejepa-corpus --example mejepa_corpus_fsv`
failed before writing evidence:

`ME-JEPA corpus FSV invariant failed: DeleteTestCall must remove exactly its reported byte length`

## Why It Matters
The implementation now replaces deleted assertions with a same-indentation
`pass` line to preserve Python syntax, including for single-statement test
bodies. The FSV example still asserted the older empty-replacement invariant,
so the source-of-truth verification harness no longer matched the real
operator contract.

## Related
- `crates/context-graph-mejepa-corpus/src/delete_test_call.rs`
- `crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs`
