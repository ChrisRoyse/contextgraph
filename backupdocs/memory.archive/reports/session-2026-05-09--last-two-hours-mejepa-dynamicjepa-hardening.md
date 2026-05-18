---
namespace: reports
created: 2026-05-09T03:29:42Z
updated: 2026-05-09T03:29:42Z
status: active
tags: report, session, mejepa, dynamicjepa, tree-sitter, fsv, ruvector
---

# Session Report: 2026-05-09 — Last-Two-Hours ME-JEPA/DynamicJEPA Hardening

## Task

Review the last two hours of git history, harden the implemented work, research best practices after finding
root causes, and perform full-state verification with physical readbacks.

## Outcome

Complete for the in-repo ME-JEPA corpus/split, AST chunker, DynamicJEPA language extraction, MCP reflexion,
and tier-compression surfaces exercised in this pass. The larger 2,400-task official SWE-bench/Docker/RocksDB
oracle scope from the ruVector end-goal docs remains future work.

## Summary

Read the SAMP protocol, relevant memory entries, and ruVector findings `00` through `09`. Reviewed commits from
the last two hours on the current branch, then focused on the recent ME-JEPA corpus, split, AST, DynamicJEPA, MCP
reflexion, and tier-compression changes.

The main root cause found during verification was a Tree-sitter runtime/grammar ABI mismatch. C# semantic
extraction failed because `tree-sitter-c-sharp v0.23.5` uses language ABI 15 while the workspace runtime was too
old. The fix was to align direct parser-runtime dependencies to `tree-sitter = "0.26"` and refresh the lockfile,
which now resolves `tree-sitter v0.26.8`.

Full-state verification was performed by executing the relevant logic and separately reading back durable JSON,
Python bytecode/source, assignment JSON, and compression blob sources of truth from `/tmp`.

## Files Created or Modified in the Repo

- `Cargo.lock` — refreshed after `cargo update -p tree-sitter`; now locks `tree-sitter v0.26.8`.
- `Cargo.toml` — current workspace includes `crates/context-graph-tier-compression`.
- `crates/context-graph-cli/Cargo.toml` — direct Tree-sitter runtime dependency aligned to `0.26`.
- `crates/context-graph-cli/src/bin/reality-loop.rs` — current hardening includes Java test detection and fail-closed Java toolchain state.
- `crates/context-graph-cli/src/commands/dynamicjepa.rs` — current hardening includes C# target disambiguation and Java/C#/C++ semantic extraction improvements.
- `crates/context-graph-core/Cargo.toml` — direct Tree-sitter runtime dependency aligned to `0.26`.
- `crates/context-graph-core/examples/ast_chunker_fsv.rs` — current hardening records parser-error diagnostics and AST evidence.
- `crates/context-graph-mejepa-corpus/Cargo.toml` — direct Tree-sitter runtime dependency aligned to `0.26`.
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` — current repo contains additional ruVector end-goal notes.
- `crates/context-graph-tier-compression/*` — untracked workspace crate was formatted and verified because it is now included by the root workspace.

## Files Written or Updated in ./memory/

- `memory/discoveries/2026-05-09--tree-sitter-abi-runtime-and-mejepa-fsv-hardening.md`
- `memory/journal/2026-05-09--failure-tree-sitter-csharp-abi-mismatch.md`
- `memory/journal/2026-05-09--failure-readback-jq-schema-assumption.md`
- `memory/reports/session-2026-05-09--last-two-hours-mejepa-dynamicjepa-hardening.md`

## Discoveries

- Tree-sitter grammar/runtime ABI skew is a real fail-closed root cause for DynamicJEPA C# extraction. Keep
  grammar crates and runtime ABI support aligned; do not add fallback parsing.
- FSV summaries must inspect the actual source-of-truth schema before summarizing. A stale `jq` selector failed
  even though the evidence files were valid.

## Failures

- C# semantic extraction initially failed with `Incompatible language version 15. Expected minimum 13, maximum 14`.
  Resolved by updating Tree-sitter runtime dependencies to `0.26` and lockfile to `0.26.8`.
- Two final readback `jq` commands used stale schema assumptions and failed with `Cannot iterate over null (null)`.
  Resolved by inspecting JSON keys and rerunning source-of-truth reads with the current schema.
- One `context-graph-mcp` reflexion test build attempt hit memory pressure during compile/link. Retried with
  `CARGO_BUILD_JOBS=1 cargo test -j1 ...`, which passed.

## Full-State Verification Evidence

- `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`:
  `f2b3f0c542e1bb3d8643a6d6bb3e214442ba8c7aa5d0eff5653ec108e71dd7e1`
- `/tmp/contextgraph-mejepa-corpus-fsv/split-evidence.json`:
  `309339d140aa432cd44c0e59d022bed4f631dd150dfc7528b7c36b7de373859f`
- `/tmp/contextgraph-mejepa-corpus-fsv/split-assignments.json`:
  `4adf7c32c86cbb2b8f45bd98c3a1e4119351b59942ce38ae3fc20a80359e8ee1`
- `/tmp/contextgraph-tier-compression-fsv/tier-compression-evidence.json`:
  `34075c0db5de2d142931bfe214f197f0390e634f2bebf2d19d534b36c2cfb3b9`

ME-JEPA corpus evidence readback showed all eight mutation cases passing, seven mutation edge cases recorded,
and Python compile verification for generated sources. The intended compile-error case failed with
`SyntaxError: expected ':'` and did not produce a `.pyc`.

Split evidence readback showed 300 real SWE-bench Lite tasks, 12 repos, bucket sizes `[195, 77, 28]`, zero
cross-cut weights, `leakage_check_passed: true`, and 300 unique task IDs in the assignments JSON. Edge cases
recorded before/after state for empty corpus, duplicate task ID, invalid fraction sum, and single-repo degenerate
split.

Tier-compression evidence readback showed `input_n: 768`, `input_seed: 42`, happy paths for 3/5/7/8-bit blobs,
and before/after state for empty input, NaN input, truncated blob, and wrong magic bytes.

## Tests / Build

- `cargo test -p context-graph-mejepa-corpus -- --nocapture`: 50 passed.
- `cargo run -p context-graph-mejepa-corpus --example mejepa_corpus_fsv`: passed.
- `cargo run -p context-graph-mejepa-corpus --example split_fsv`: passed.
- `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/contextgraph-codex-target cargo test -j1 -p context-graph-core ast_chunker -- --nocapture`: 17 passed.
- `cargo test -p context-graph-cli --lib csharp_semantic_extraction_captures_members_and_test_paths -- --nocapture`: passed.
- `cargo test -p context-graph-cli --lib csharp_build_target_selection_is_unambiguous_and_fail_closed -- --nocapture`: passed.
- `cargo test -p context-graph-cli --lib java_semantic_extraction_recognizes_standard_tests_and_junit_annotations -- --nocapture`: passed.
- `cargo test -p context-graph-cli --bin reality-loop dynamicjepa_java_ast_recognizes_standard_test_files_and_annotations -- --nocapture`: passed.
- `cargo test -p context-graph-cli --bin reality-loop dynamicjepa_java_action_semantics_require_real_javac_or_record_checked_state -- --nocapture`: passed.
- `cargo test -p context-graph-tier-compression -- --nocapture`: 21 passed.
- `cargo run -p context-graph-tier-compression --example tier_compression_fsv`: passed.
- `cargo fmt --all --check && git diff --check`: passed.
- `CARGO_BUILD_JOBS=1 cargo test -j1 -p context-graph-mcp --lib reflexion -- --nocapture`: 5 passed.

## Research Sources

- Tree-sitter ABI compatibility docs: https://docs.rs/tree-sitter/latest/tree_sitter/constant.LANGUAGE_VERSION.html
- Tree-sitter ABI function docs: https://docs.rs/tree-sitter/latest/tree_sitter/ffi/fn.ts_language_abi_version.html
- Tree-sitter `ERROR`/`MISSING` query docs: https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html
- Rust `File::sync_all` docs for durable write/readback reasoning: https://doc.rust-lang.org/std/fs/struct.File.html
- Python lexical/parser error behavior: https://docs.python.org/3/reference/lexical_analysis.html
- .NET build target semantics: https://learn.microsoft.com/en-us/dotnet/core/tools/dotnet-build

## Open Blockers

- The full ruVector goal is not done until the project has the real large-scale SWE-bench/Docker oracle pipeline,
  persistent RocksDB/vector evidence, and clean-room replacement architecture described in
  `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`.

## Resume Here Next Session

- Start with this report and `memory/discoveries/2026-05-09--tree-sitter-abi-runtime-and-mejepa-fsv-hardening.md`.
- Re-run `cargo tree -p context-graph-cli` before touching language grammar crates; ABI skew is now known to be
  a hard parser initialization failure.
- Preserve fail-closed behavior. Do not add parser, compiler, or split fallbacks that silently convert missing
  evidence into success.
- Next priority is the larger source-of-truth oracle layer: real SWE-bench task execution, durable vector/database
  evidence, and end-goal Phase 0/Phase 1 integration.

## Confidence

High for the exercised in-repo surfaces and source-of-truth readbacks. Medium for broader ruVector end-goal
completion because the official full-scale oracle/database pipeline remains outside this pass.
