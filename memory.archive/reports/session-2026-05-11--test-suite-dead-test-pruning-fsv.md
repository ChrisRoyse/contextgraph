---
date: 2026-05-11
task: test-suite-dead-test-pruning-fsv
status: complete
agent_id: codex
timestamp_utc: 2026-05-11T04:46:22Z
tags:
  - tests
  - fsv
  - hardening
  - dead-code
---

# Session Report: Test Suite Dead-Test Pruning FSV

## Scope

Reviewed the test-suite inventory under the test-usefulness prompt, the hardening prompt, the issue-flagging prompt, and the solo memory protocol. The cleanup target was old redundant/dead test debt: root tests that were outside current Cargo/Python discovery, ignored manual FSV blocks that duplicated executable tests, and an orphan diagnostic test file not wired into the module tree.

## Root Causes Found

- The legacy root-level `tests/` tree contained stale harnesses and redirects rather than current executable suite signal.
- Some ccreality tests were ignored manual/persistent variants that duplicated already-executable temp-root FSV tests and, in one case, targeted active runtime state.
- The embeddings provider had an orphan `diagnostic_test.rs` file that was not included by `provider/mod.rs`, so it was not part of Cargo's test graph.
- `file_watcher_integration.rs` used fixed sleeps after synchronous watcher operations, creating slow/flaky-test risk.

## Changes Applied

- Removed stale root tests and generated cache leftovers from the working tree.
- Removed stale/manual ignored ccreality test blocks while keeping the behavior-focused executable tests for recommendation recall, influence, and witness repair.
- Removed the orphan embeddings provider diagnostic test file.
- Removed fixed sleeps from `crates/context-graph-core/tests/file_watcher_integration.rs`; the tests now verify state immediately after `start()` or explicit `process_events()`.
- Updated `docs2/contextgraphspecs/14_test_suite.md` and `docs2/contextgraphspecs/15_verification_report.md` so the removed root-level `tests/` tree is no longer documented as live coverage.

## Web Research Applied

- Rust/Cargo docs: `cargo test` compiles and executes unit, integration, and documentation tests; `tests/` files are integration targets and each file is compiled as a separate crate.
- Rust Book: integration tests should use the public API, and the Rust harness runs tests in parallel by default, so tests must not depend on shared state.
- Rust Book: ignored tests are out of normal `cargo test` signal and require explicit `--ignored`/`--include-ignored`.
- Cargo environment docs: integration tests should use Cargo-provided paths such as `CARGO_BIN_EXE_<name>` and `CARGO_TARGET_TMPDIR` rather than hardcoded local paths.
- Pytest docs: discovery looks for `test_*.py` / `*_test.py`; CI isolation can use cache clearing when correctness matters more than speed.

## Verification Commands

- `cargo fmt --all` passed.
- `cargo test -p context-graph-core --features test-utils --test file_watcher_integration -- --nocapture` passed: 4 tests, 0 ignored, 0.15s runtime after removing sleeps.
- `cargo test -p context-graph-mcp --lib --no-run` passed.
- `cargo test -p context-graph-mcp --lib recommendation_recall_fsv -- --nocapture` passed: 2 tests.
- `cargo test -p context-graph-mcp --lib witness_chain_repair_fsv -- --nocapture` passed: 3 tests.
- `cargo test -p context-graph-mcp --lib influence -- --nocapture` passed: 5 tests.
- `cargo test -p context-graph-embeddings --no-run` passed.
- `cargo test --workspace --no-run` passed in 5m 26s.

## Source-of-Truth Evidence

- Filesystem readback showed the removed test inventory absent:
  - `ABSENT tests`
  - `ABSENT crates/context-graph-embeddings/tests/e9_vector_differentiation_test.rs`
  - `ABSENT crates/context-graph-embeddings/tests/qodo_embed_integration.rs`
  - `ABSENT crates/context-graph-embeddings/src/provider/diagnostic_test.rs`
  - `ABSENT crates/context-graph-mcp/src/handlers/tools/reality_loop/recommendation_recall_active_fsv.rs`
- Code/docs reference scan over `Cargo.toml`, `crates`, `docs2`, `Makefile`, and `pyproject.toml` found no references to the removed active/manual test symbols or deleted harness names.
- `file_watcher_integration.rs` scan found no remaining `sleep()`/`tokio::time::sleep` calls.
- Workspace no-run output listed current executable Cargo test targets and did not include the removed root `tests/` tree, deleted embeddings tests, or deleted active ccreality FSV module.

## Remaining Risk

Remaining `#[ignore]` tests are hardware/model/performance tier tests: GPU HDBSCAN, production embedding-model MCP/search paths, a heavy DynamicJEPA gridworld verifier, and an HNSW scaling benchmark. They were not deleted because they are not the stale root/manual duplicate tests addressed in this pass; if the policy becomes "zero ignored tests anywhere," those should be split into explicit feature/CI jobs or removed as a separate migration.

## Notes

Final `git status --short --untracked-files=all` showed only an unrelated untracked futurebuild spec file at the time of reporting. Earlier transient status output included broader modifications while concurrent repository changes were present.
