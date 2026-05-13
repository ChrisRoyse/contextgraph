---
namespace: reports
agent_id: codex
timestamp: 2026-05-09T03:53:56Z
status: active
tags: report, session, c, ast, mejepa, semantic-index, compile-commands, fsv
---

# Session Report: C AST Parser Hardening

## Task

Focus only on C hardening for the AST parser harness in the ME-JEPA
prediction system. Find the root cause, research robust parser/compiler
evidence practices, implement non-fallback fixes, update docs, and perform
full state verification against physical source-of-truth artifacts.

## Root Cause

The C AST harness was using generic C/C++ node classification where C needs
declarator-aware semantics. That caused duplicate function symbols,
incorrect `static` visibility, false type declarations for `struct Point p`,
prototype declarations leaking into dataflow, and missing macro symbols.

The second root cause was compiler evidence: C action semantics guessed
`gcc <file>.c` instead of requiring the repository's compilation database.
That can be wrong for real C repos because include paths, macros, compiler
choice, and working directory live in `compile_commands.json`.

## Implementation

- `crates/context-graph-cli/src/commands/dynamicjepa.rs`
  - Added C-specific AST classification.
  - Indexed `preproc_def` and `preproc_function_def`.
  - Required compound specifiers to have `body` before becoming type
    declarations.
  - Prevented function prototypes from becoming dataflow.
  - Required `compile_commands.json` entries to have `directory` and structured
    `arguments`.
  - Added `c_semantic_extraction_is_declarator_aware`.

- `crates/context-graph-cli/src/bin/reality-loop.rs`
  - Added C-specific action semantics.
  - Replaced guessed C compiler invocation with structured
    `compile_commands.json` lookup.
  - Added snapshot path rewriting for absolute compile-command paths.
  - Added fail-closed C action tests for missing `compile_commands.json`.

- Docs updated:
  - `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`
  - `memory/context/mejepa-implementation-initiative.md`

## Verification

Automated tests:

- `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/contextgraph-codex-c-target cargo test -j1 -p context-graph-cli --lib c_semantic_extraction_is_declarator_aware -- --nocapture`
- `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/contextgraph-codex-c-target cargo test -j1 -p context-graph-cli --bin reality-loop dynamicjepa_ast_action_semantics_parse_real_c_edit_with_compile_commands -- --nocapture`
- `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/contextgraph-codex-c-target cargo test -j1 -p context-graph-cli --bin reality-loop dynamicjepa_ast_action_semantics_c_fails_closed_without_compile_commands -- --nocapture`
- `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/contextgraph-codex-c-target cargo test -j1 -p context-graph-cli --bin reality-loop dynamicjepa_ast_action_semantics -- --nocapture`
- `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/contextgraph-codex-c-target cargo test -j1 -p context-graph-cli --lib semantic_extraction -- --nocapture`

All passed.

Manual full-state verification:

- Happy-path source of truth:
  `/tmp/contextgraph-c-ast-fsv.eTmgwU/semantic-index.json`
  and `/tmp/contextgraph-c-ast-fsv.eTmgwU/semantic-index.json.evidence/`.
- Verified source files were committed in Git before indexing.
- Verified output JSON and compiler evidence files existed after execution.
- Verified actual semantic state:
  - `helper`: one `function_definition`, visibility `private`.
  - source `add`: one `function_definition`, visibility `public`.
  - `Point`: one `struct_specifier` declaration in `include/math_util.h`.
  - header prototype dataflow count: `0`.
  - compiler argv: `["gcc","-Iinclude","-c","src/math_util.c","-o","math_util.o","-fsyntax-only"]`.

Edge cases:

- Missing `compile_commands.json`:
  status `1`, no semantic index output, structured `VERIFICATION_FAILED`.
- Missing `directory` in `compile_commands.json`:
  status `1`, no semantic index output, structured `VALIDATION` at
  `semantic-index.compile_commands.directory`.
- Shell-string `command` without structured `arguments`:
  status `1`, no semantic index output, structured `VERIFICATION_FAILED`.

Edge-case physical evidence root:
`/tmp/contextgraph-c-ast-edges.0ieALf/`.

## Notes

The first manual happy-path run failed because the synthetic fixture was not
a Git repository; the indexer correctly required `git rev-parse HEAD` for
provenance. The fixture was corrected by initializing Git and committing the
real C files before rerunning.
