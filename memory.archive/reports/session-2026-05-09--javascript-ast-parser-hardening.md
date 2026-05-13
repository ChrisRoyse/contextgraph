---
namespace: reports
created: 2026-05-09T04:23:22Z
updated: 2026-05-09T04:23:22Z
status: active
tags: report,session,mejepa,javascript,ast,semantic-index,fsv
---

# Session Report: 2026-05-09 — JavaScript AST Parser Hardening

## Task: Harden JavaScript AST/parser harness for ME-JEPA prediction evidence.
## Outcome: complete

## Summary

The JavaScript root cause was symbol identity extraction from the wrong AST
location. Assigned arrows and function expressions were susceptible to
parameter-name or inner-identifier identity because the previous generic
fallback did not respect JavaScript binding-site structure. The robust rule is
now binding-site identity: the function symbol comes from the owning
`variable_declarator`, `assignment_expression`, object `pair`, or named function
node, never from the first identifier inside the function.

The related duplicate-symbol issue was also closed: nested tree-sitter keyword
nodes under a `function_expression` do not become separate persisted function
symbols. Export visibility is ancestor-aware, parser recovery emits structured
diagnostics, and JavaScript/TypeScript compiler/LSP evidence refuses parser-only
operation when `tsc` is missing.

Web research matched the implementation direction. Tree-sitter code navigation
standardizes `@definition.*` plus inner `@name` captures, and the JavaScript
`queries/tags.scm` captures assigned function names from enclosing lexical
declarations, assignments, and object pairs.

## Files Created or Modified in the Repo

- `crates/context-graph-cli/src/commands/dynamicjepa.rs` — JavaScript semantic-index extraction uses binding-site function identity, export-aware visibility, syntax diagnostics, and no duplicate nested function-expression symbols.
- `crates/context-graph-cli/src/bin/reality-loop.rs` — JavaScript live action AST attribution uses the same binding-site symbol and visibility rules.
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` — documents the shipped JavaScript/TypeScript parser harness state and physical source-of-truth evidence.
- `memory/context/mejepa-implementation-initiative.md` — records JavaScript/TypeScript AST hardening as shipped.

Current worktree note: unrelated `Cargo.toml`, `vendor/`, and Ruby/parser-harness edits were observed from concurrent work and left untouched for their owners.

## Files Written or Updated in ./memory/

- `memory/discoveries/2026-05-09--javascript-ast-binding-symbol-mismatch.md` — marked resolved and added full-state verification evidence roots.
- `memory/decisions/2026-05-09--javascript-ast-symbols-derive-from-binding-sites.md` — records binding-site symbol identity as the durable JavaScript rule.
- `memory/journal/2026-05-09--failure-javascript-ast-verification-fork-oom.md` — documents earlier host memory/fork pressure during verification.
- `memory/reports/session-2026-05-09--javascript-ast-parser-hardening.md` — this report.

## Decisions Made

- JavaScript/TypeScript function symbols derive from binding sites, not first identifiers inside function nodes. Rationale: ME-JEPA action attribution and training labels must match repository reality. File: `memory/decisions/2026-05-09--javascript-ast-symbols-derive-from-binding-sites.md`

## Discoveries

- The original bug could silently persist `x` for `export const double = (x) => ...`, omit function expressions, and mark assigned exports as public. Impact: poisoned `E_AST`, blast-radius, reverse-reference, and training features. File: `memory/discoveries/2026-05-09--javascript-ast-binding-symbol-mismatch.md`
- Host verification initially hit OS fork/OOM pressure from concurrent Rust builds and rust-analyzer processes. Impact: targeted tests had to run with `CARGO_BUILD_JOBS=1`. File: `memory/journal/2026-05-09--failure-javascript-ast-verification-fork-oom.md`

## Verification

- `cargo fmt --all` passed.
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-cli --no-default-features --lib javascript_semantic_extraction -- --nocapture` passed: 2 tests, 0 failures.
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-cli --no-default-features --bin reality-loop dynamicjepa_javascript_ast -- --nocapture` passed: 1 test, 0 failures.
- `CARGO_BUILD_JOBS=1 cargo build -p context-graph-cli --no-default-features --bin context-graph-cli` passed.

## Full-State Verification Evidence

Source of truth: persisted semantic-index JSON plus sibling command evidence
directory.

Happy path:

- Root: `/tmp/contextgraph-js-fsv.DqnqeB`
- Index: `/tmp/contextgraph-js-fsv.DqnqeB/happy/semantic-index.json`
- Evidence: `/tmp/contextgraph-js-fsv.DqnqeB/happy/semantic-index.json.evidence/`
- Readback function symbols: `double` exported arrow, `plusOne` exported function expression, `run` public arrow, `stop` public function expression, `load` public function expression.
- Readback evidence files: `git_rev_parse_head.stdout`, `git_rev_parse_head.stderr`, `typescript_tsc.stdout`, `typescript_tsc.stderr`.

Edge cases:

- Invalid JavaScript root `/tmp/contextgraph-js-edge-fsv.pgLoLO/invalid`: failed closed with TypeScript diagnostic `src/math.js(3,1): error TS1005: '}' expected.`, no semantic-index output file, source unchanged.
- Missing `tsc` root `/tmp/contextgraph-js-edge-fsv.pgLoLO/missing-toolchain`: failed closed with `missing ["tsc"]`, no semantic-index output file.
- Empty/no-JavaScript root `/tmp/contextgraph-js-edge-fsv.pgLoLO/empty`: failed closed with requested-language-not-present, no semantic-index output file.
- `--max-files 0` root `/tmp/contextgraph-js-limit-fsv.LWokg5`: failed validation before evidence creation, no semantic-index output file.

## Web Sources

- Tree-sitter Code Navigation Systems: `https://tree-sitter.github.io/tree-sitter/4-code-navigation.html`
- Tree-sitter JavaScript tags query: `https://raw.githubusercontent.com/tree-sitter/tree-sitter-javascript/master/queries/tags.scm`
