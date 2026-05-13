---
namespace: journal
created: 2026-05-09T04:15:36Z
updated: 2026-05-09T04:15:36Z
status: resolved
severity: medium
tags: go, ast, cargo, oom, verification
---

# Failure: Go AST Verification Was Blocked By Concurrent Cargo/OOM Pressure

## What Happened

Initial Go-focused `cargo test` attempts were blocked by shared Cargo artifact
locks and concurrent Rust/C++ builds from other agents. One full default
feature compile aborted in rustc with allocation failures while several
rust-analyzer instances and other Cargo jobs were active.

## Why It Matters

The Go harness fixes were not proven by return values alone. Verification had
to wait for shared artifact locks to clear and use a lower-memory command shape.

## Resolution

After clearing compile blockers in the active parser-harness file and switching
to `CARGO_BUILD_JOBS=1` plus `--no-default-features`, the Go checks completed:

- `cargo check -p context-graph-cli --no-default-features --bin reality-loop`
- `cargo test -p context-graph-cli --no-default-features --bin reality-loop dynamicjepa_ast_action_go -- --nocapture`
- `cargo test -p context-graph-cli --no-default-features --bin reality-loop real_go -- --nocapture`
- `cargo test -p context-graph-cli --no-default-features go_semantic_extraction_recognizes_only_go_test_entrypoints -- --nocapture`

The independent manual evidence file
`/tmp/contextgraph-go-ast-hardening-fsv/go-evidence.json` was also written and
read back.

## Related

- `memory/discoveries/2026-05-09--go-mejepa-ast-harness-fail-closed-and-test-entrypoints.md`
