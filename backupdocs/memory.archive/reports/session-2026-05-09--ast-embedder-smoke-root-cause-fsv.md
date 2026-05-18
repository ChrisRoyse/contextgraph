---
namespace: reports
agent_id: codex
timestamp: 2026-05-09T09:41:23Z
status: active
tags: report, session, mejepa, ast, embedders, fsv, corpus, smoke-test
---

# Session Report: AST Embedder Smoke Root Cause FSV

## Scope

Read the RuVector/ME-JEPA specs and SAMP protocol, investigated the AST/chunking/embedder smoke path, researched official test discovery and Tree-sitter parse-error behavior, fixed the AST test-classification root cause, and performed Full State Verification through persisted source-of-truth artifacts.

## Code Changes

- `crates/context-graph-core/src/memory/ast/classify.rs`: removed broad path-only test classification for helpers and now uses language-specific test signals.
- `crates/context-graph-core/src/memory/ast/tests.rs`: added regression coverage proving production helper entities remain `Function`/`Method` in test paths while real tests remain `TestFunction`.
- `crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs`: hardened source-of-truth artifact writes with `0700` directory mode, `0600` files, fsync, and byte readback.

The working tree also contains existing broader Phase 1 routing/instrumentation changes and untracked smoke/spec files that were present in this session; they were not reverted.

## Verification

Passed:
- `cargo fmt --all`
- `git diff --check`
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-core memory::ast -- --nocapture`
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-mejepa-instruments --all-targets -- --nocapture`
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-mejepa-corpus -- --nocapture`
- `CARGO_BUILD_JOBS=1 cargo run -p context-graph-core --example ast_chunker_multilang_fsv`
- `CARGO_BUILD_JOBS=1 cargo run -p context-graph-mejepa-instruments --example ast_embedder_smoke_fsv`
- `CARGO_BUILD_JOBS=1 cargo run -p context-graph-mejepa-instruments --example phase1_extended_fsv`
- `cargo run -p context-graph-mejepa-corpus --bin mejepa-corpus -- generate --output /tmp/contextgraph-phase0-smoke-one-task --instance-id sympy__sympy-20590 --run-id-prefix phase0smoke --instance-timeout-secs 1800 --overall-timeout-secs 3600`
- `target/debug/mejepa-corpus verify --corpus /tmp/contextgraph-phase0-smoke-one-task`
- `target/debug/mejepa-corpus stats --corpus /tmp/contextgraph-phase0-smoke-one-task`

Manual source-of-truth readback:
- AST chunker aggregate evidence: 11 languages, all nonzero chunk counts; empty, invalid syntax, and mixed PHP/HTML edge cases rejected with explicit error codes.
- AST embedder smoke evidence: 11 real sample files, real SWE-bench manifest `sympy__sympy-20639`, 5120-value panel, `filled_mask=32767`, 15 slots filled.
- Phase 1 extended evidence: 5120-value panel, `filled_mask=32767`, 15 slots filled, 11 fail-closed edge cases rejected with `MEJEPA_INSTRUMENTS_INVALID_INPUT`, source-of-truth files now `0600`.
- Corpus package tests: 87 passed, including SWE-bench config/report validation and oracle-store persistence tests.
- Phase 0 one-task corpus smoke: 8 persisted entries for `sympy__sympy-20590`, one per mutation category; 8 nonempty patches, 8 SWE-bench run JSON files, `mutation_cf_count=8`, `verdict_cf_count=8`, and `verify` returned `passes=true`.

## Not Run

A full 300-task/2400-entry corpus generation, embedding pass, or training pass was not run. This session verified the smoke path, one real SWE-bench instance, and ingestion/oracle packages needed before the full corpus run.
