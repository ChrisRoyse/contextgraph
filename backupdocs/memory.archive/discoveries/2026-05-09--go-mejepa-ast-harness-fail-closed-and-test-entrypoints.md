---
namespace: discoveries
created: 2026-05-09T04:15:36Z
updated: 2026-05-09T04:15:36Z
status: active
severity: high
tags: go, mejepa, ast, dynamicjepa, fail-closed, fsv
---

# Go ME-JEPA AST Harness Requires Go Toolchain And Test Entrypoint Semantics

## What Happened

The Go ME-JEPA action harness had two unsafe behaviors to harden:

- live action compiler evidence could degrade to parser-only evidence when `go`
  was missing from `PATH`;
- parser classification needed to distinguish actual Go test entrypoints from
  helper functions that merely live in `*_test.go`.

The current repository state has the Go action path fail closed with
`RUST_REALITY_DYNAMICJEPA_GO_TOOLCHAIN_MISSING` when `go` is unavailable. Go
test classification now recognizes top-level `Test`, `Benchmark`, `Fuzz`, and
`Example` entrypoints only in non-hidden `*_test.go` files; helpers such as
`helper` and `Testhelper` remain ordinary functions.

## Why It Matters

ME-JEPA panel rows must represent durable code reality, not parser-only
guesses. Go compiler evidence comes from `go test ./...`; without the Go
toolchain the action gate must stop. Test/action features also need correct
`E_Test` labeling: overclassifying every helper in `*_test.go` as a test case
pollutes predictor training and verifier-target selection.

## Source Of Truth

- Manual evidence:
  `/tmp/contextgraph-go-ast-hardening-fsv/go-evidence.json`
- Live action test evidence:
  `cargo test -p context-graph-cli --no-default-features --bin reality-loop real_go -- --nocapture`
- Parser entrypoint test evidence:
  `cargo test -p context-graph-cli --no-default-features --bin reality-loop dynamicjepa_ast_action_go -- --nocapture`
- Semantic-index test evidence:
  `cargo test -p context-graph-cli --no-default-features go_semantic_extraction_recognizes_only_go_test_entrypoints -- --nocapture`

Readback showed:

- `TestAdd`, `BenchmarkAdd`, `FuzzAdd`, and `ExampleAdd` classify as `test` in
  `src/calc_test.go`.
- The same names classify as `function` in `src/calc.go`.
- `helper` and `Testhelper` classify as `function` in `src/calc_test.go`.
- `_hidden_test.go` and `.hidden_test.go` are not treated as Go test files.
- This host has no `go` binary; live action fails closed with
  `RUST_REALITY_DYNAMICJEPA_GO_TOOLCHAIN_MISSING` and a snapshot source-of-truth
  path.

## Web Research

- Go `testing` docs: suites live in `_test.go`, `BenchmarkXxx` and `FuzzXxx`
  forms are official, and examples use `Example...` naming.
- Go `cmd/go` docs: `go test` recompiles packages with `*_test.go`, ignores
  files beginning `_` or `.`, and compiles test, benchmark, fuzz, and example
  functions into separate test binaries.

## Related

- `crates/context-graph-cli/src/bin/reality-loop.rs`
- `crates/context-graph-cli/src/commands/dynamicjepa.rs`
