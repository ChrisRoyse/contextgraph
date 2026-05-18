---
namespace: journal
created: 2026-05-12T21:31:27Z
updated: 2026-05-12T21:31:27Z
status: active
tags: failure, ast, issue-6, compile, deprecation
---

# Failure: legacy AST shim compile errors

## What Happened
Focused verification for GitHub issue #6 failed before any FSV could run:

- `cargo test -p context-graph-core memory::ast_chunker -- --nocapture`
- `cargo check -p context-graph-core --examples`
- `cargo run -p context-graph-core --example ast_chunker_multilang_fsv`

All failed because `#[deprecated(note = DEPRECATED_NOTE)]` used a const expression. Rust requires the `note` field to be a literal string. The test compile also showed stale unit-test expectations in `code_capture.rs` after `convert_entity_type` and `language_from_string` were hardened to return `Option`.

## Why It Matters
The worktree currently cannot compile, so issue #6 is not closed yet. The intended root fix remains correct: the legacy `AstCodeChunker` should be a deprecated facade over canonical `memory::ast`, with fail-closed diagnostics and parity FSV. The implementation needs literal deprecation notes and updated tests.

## Related
- GitHub issue: #6
- Files: `crates/context-graph-core/src/memory/ast_chunker.rs`, `crates/context-graph-core/src/memory/code_capture.rs`
- Tags: ast, deprecated-api, compile
