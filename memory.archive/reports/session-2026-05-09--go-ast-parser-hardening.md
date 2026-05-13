---
namespace: reports
created: 2026-05-09T04:15:36Z
updated: 2026-05-09T04:15:36Z
status: active
tags: report, session, go, mejepa, ast, fsv
---

# Session Report: 2026-05-09 — Go AST Parser Hardening

## Task
Investigate and harden the Go AST parser/action harness for the ME-JEPA prediction system.

## Outcome
Complete for the Go scope.

## Summary
The root cause was unsafe Go semantics in the parser/action harness boundary:
missing Go toolchain evidence could previously degrade to parser-only action
evidence, and Go test classification needed entrypoint-aware rules instead of
path-only heuristics. Current behavior fails closed when `go` is missing and
classifies only real Go test entrypoint names in non-hidden `*_test.go` files.

Current repository state also includes concurrent non-Go parser-harness edits
from other agents. Those were not reverted. A few compile blockers in the same
file had to be completed so the Go checks could run, but this report’s language
scope is Go.

## Files Created or Modified in the Repo
- `crates/context-graph-cli/src/bin/reality-loop.rs` — Go live action fail-closed
  toolchain gate, Go test-entrypoint classification, and Go FSV tests.
- `crates/context-graph-cli/src/commands/dynamicjepa.rs` — Go semantic-index
  test-entrypoint classification and FSV test.

## Files Written or Updated in ./memory/
- `memory/discoveries/2026-05-09--go-mejepa-ast-harness-fail-closed-and-test-entrypoints.md`
- `memory/journal/2026-05-09--failure-go-ast-cargo-lock-and-oom.md`
- `memory/reports/session-2026-05-09--go-ast-parser-hardening.md`

## Decisions Made
- Go action semantics require real `go test ./...` compiler evidence; missing
  `go` is a hard error, not parser-only fallback.
- Go test classification is entrypoint-aware: `Test`, `Benchmark`, `Fuzz`, and
  `Example` functions in non-hidden `*_test.go` files are tests; helpers are
  ordinary functions.

## Discoveries
- Official Go docs make `*_test.go` a file-level inclusion rule, not proof that
  every function inside is a test. Files starting `_` or `.` are ignored.
- This host currently has no `go` binary, so the live Go action FSV exercises
  the fail-closed missing-toolchain branch.

## Failures
- Early Cargo verification hit artifact locks and rustc OOM under concurrent
  builds. See `memory/journal/2026-05-09--failure-go-ast-cargo-lock-and-oom.md`.

## Successes
- Manual source-of-truth evidence was written and read back from
  `/tmp/contextgraph-go-ast-hardening-fsv/go-evidence.json`.
- Focused Go cargo checks passed after lock pressure cleared.

## Tests / Build
- `cargo fmt --all` passed.
- `CARGO_BUILD_JOBS=1 cargo check -p context-graph-cli --no-default-features --bin reality-loop` passed.
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-cli --no-default-features --bin reality-loop dynamicjepa_ast_action_go -- --nocapture` passed.
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-cli --no-default-features --bin reality-loop real_go -- --nocapture` passed.
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-cli --no-default-features go_semantic_extraction_recognizes_only_go_test_entrypoints -- --nocapture` passed.

## Resume Here Next Session
- Start by reading:
  `memory/discoveries/2026-05-09--go-mejepa-ast-harness-fail-closed-and-test-entrypoints.md`.
- Watch out for concurrent non-Go parser-harness edits in the same two files.
- Priority: keep Go evidence compiler-backed and keep Go test classification
  aligned with `go test` discovery rules.

## Confidence: high
Focused tests and physical readback evidence cover the happy path and edge
cases requested for the Go scope.
