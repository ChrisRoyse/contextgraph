---
namespace: journal
agent_id: codex-typescript-hardening
timestamp: 2026-05-09T04:02:06Z
status: active
tags: typescript,mejepa,ast,verification,failure
---

# Failure: Focused TypeScript Tests Blocked

## What Happened

Two focused test commands were started while Cargo locks and memory pressure were
active:

- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-cli --lib typescript_semantic -- --nocapture`
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-cli --bin reality-loop dynamicjepa_typescript -- --nocapture`

The lib test reached rustc and failed with `rustc-LLVM ERROR: out of memory`.
The `reality-loop` bin test compiled far enough to show unrelated concurrent
workspace errors in Ruby/PHP helper references:

- `ast_nearest_ruby_test_node` missing
- `ast_ruby_node_kind` missing
- `php_function_or_method_is_test` missing
- `php_call_is_pest_test` missing

## Why It Matters

This is not TypeScript parser logic failure. It means the current shared
workspace cannot use the `reality-loop` bin test target as TypeScript evidence
until the concurrent non-TypeScript edits are completed or isolated.

## Lesson

For this TypeScript turn, verify the semantic-index path with
`context-graph-cli dynamicjepa build-semantic-index` and physical JSON readback,
then rerun focused Cargo tests once memory pressure and unrelated language edits
are resolved.
