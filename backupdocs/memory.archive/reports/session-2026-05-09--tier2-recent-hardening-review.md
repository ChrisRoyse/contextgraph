---
namespace: reports
created: 2026-05-09T02:35:37Z
updated: 2026-05-09T02:35:37Z
status: active
tags: report,session,tier2,hardening,fsv,mincut,granger,certificates,mcp,ccreality
---

# Session Report: 2026-05-09 - Tier 2 Recent Hardening Review

## Task: Review commits from the last hour, research best practices, harden root causes, and manually verify state.
## Outcome: complete

## Summary

Reviewed recent commits on `ccreality-cutover` covering Tier 1/2 primitives, witness certificates, ccreality CLI/MCP, and the new causal mask module. The main hardening changes are now in commits `6e0a6d4d` and `bc889bec`, plus one remaining uncommitted fix in `crates/context-graph-mcp/src/handlers/tools/reality_loop/reflexion.rs`.

Root causes fixed: fail-open graph edge construction, insufficient Granger sample checks, certificate witness-chain binding verification, causal mask non-finite score handling, MCP port parsing that accepted `0` or ignored invalid env values, and a metadata-filter loop that returned on the first operator and ignored additional constraints. Web research informed the direction: statsmodels/R-style Granger F-tests, RFC 9162 Merkle domain separation and proof checks, and Rust `Result` fail-closed APIs.

Full State Verification was run with separate source-of-truth readbacks. Evidence files were written under `/tmp/contextgraph-tier2-fsv/`, read back from disk, parsed, and hash/stat inspected separately. MCP/DynamicJEPA tests also printed RocksDB source-of-truth counts before and after operations.

## Files Created or Modified in the Repo

- `crates/context-graph-mincut/src/lib.rs`, `src/tests.rs`, `examples/mincut_fsv.rs` - fail-closed edge and matrix validation, split tests, disk-readback FSV.
- `crates/context-graph-solver/src/granger.rs`, `src/granger_tests.rs`, `examples/granger_fsv.rs` - corrected df/sample checks, NaN/degenerate F-stat rejection, disk-readback FSV.
- `crates/context-graph-solver/src/causal_mask.rs`, `src/causal_mask_tests.rs`, `examples/causal_mask_fsv.rs` - causal mask implementation/tests/FSV with checked allocation and non-finite score rejection.
- `crates/context-graph-witness/src/cert.rs`, `src/cert_merkle.rs`, `src/cert_tests.rs`, `examples/cert_fsv.rs` - split Merkle/cert tests and verified witness action-hash binding.
- `crates/context-graph-cli/src/commands/ccreality/mod.rs` - ccreality MCP binary source of truth is `.mcp.json` or explicit `CGREALITY_MCP_BIN`; no debug fallback.
- `crates/context-graph-mcp/src/main.rs` - CLI parser now rejects unknown args, missing values, port `0`, and invalid `CONTEXT_GRAPH_TCP_PORT` / `CONTEXT_GRAPH_DAEMON_PORT`.
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/reflexion.rs` - uncommitted fix: metadata filters now AND all operators and reject empty operator objects.

## Files Written or Updated in ./memory/

- `memory/discoveries/2026-05-09--tier2-recent-hardening.md` - root-cause discoveries while reviewing recent Tier 2 code.
- `memory/reports/session-2026-05-09--tier2-recent-hardening-review.md` - this final report.

## Decisions Made

- Reject invalid inputs at public boundaries instead of silently skipping, falling back, or producing NaN.
- Treat persisted JSON/binary evidence files and RocksDB/source tables as source of truth, not function return values.
- Keep ccreality MCP binary resolution single-source: `.mcp.json` or `CGREALITY_MCP_BIN`.

## Discoveries

- `weights_from_edges` previously ignored out-of-bounds endpoints/self-loops.
- `granger_test` needed `3*lag+2` raw observations to keep denominator df valid.
- Stage0/readout certs needed to bind the witness-chain action hash to the recomputed cert binding hash.
- MCP port env parsing silently fell back on invalid values.
- `matches_filter` in reflexion only evaluated the first metadata operator; multiple operator objects were not enforced.

## Failures

- Initial MCP parser unit test failed because `CliArgs` lacked `Debug`; fixed with `#[derive(Debug)]`.
- `cargo clippy -p context-graph-mcp --bin context-graph-mcp` initially failed on `clippy::never_loop` in reflexion metadata filtering; fixed by evaluating all operators.
- Release builds waited on an existing Cargo artifact lock; waited rather than killing other build processes.
- MCP/CLI clippy still emits many existing warnings in unrelated modules; after the denied lint fix, MCP clippy exits successfully.

## Successes

- `cargo test -p context-graph-mincut -- --nocapture`: 18 passed.
- `cargo test -p context-graph-solver -- --nocapture`: 58 passed.
- `cargo test -p context-graph-witness --features ed25519 -- --nocapture`: 45 passed.
- `cargo test -p context-graph-cli ccreality --lib -- --nocapture`: 6 passed.
- `cargo test -p context-graph-mcp --bin context-graph-mcp -- --nocapture`: 416 passed, 6 ignored after reflexion fix.
- `cargo test -p context-graph-mcp --bin context-graph-mcp handlers::tools::reality_loop::reflexion::tests -- --nocapture`: 5 passed.
- `cargo clippy -p context-graph-mincut -p context-graph-solver -p context-graph-witness --features context-graph-witness/ed25519 -- -D warnings`: passed.
- `cargo clippy -p context-graph-mcp --bin context-graph-mcp`: passed with existing warnings.
- `cargo fmt --all --check` and `git diff --check`: passed.
- Release builds passed for `context-graph-mcp`, `context-graph-cli`, and Tier 1/2 primitive crates.

## Full State Verification Evidence

- `/tmp/contextgraph-tier2-fsv/mincut-evidence.json`: size 2366, sha256 `592d2ca0a7e4a3a3531e7547b08388518725774b2b96ff7b7d7ba0ed48a501f1`, Stoer-Wagner cut `4.0`.
- `/tmp/contextgraph-tier2-fsv/granger-evidence.json`: size 2487, sha256 `1266afa1bc73f70ea6d805cbf1d57598f91f8fb0a957910b53d8a6337d3f728b`, strong cause F `7532.453559647289`, p `0.0`.
- `/tmp/contextgraph-tier2-fsv/causal-mask-evidence.json`: size 3079, sha256 `2df1cb3f3bc29f47014e57f92dd5f1410fb7e84883c59d7e5316a56128260ca8`, masked future probabilities zero.
- `/tmp/contextgraph-tier2-fsv/witness-cert-evidence.json`: size 2962, sha256 `ab0ce06fcb5cc093cf34ecfd22b0a2caf17cfd3dc0a74694a47fa53e1357c5a8`.
- `/tmp/contextgraph-tier2-fsv/stage0-witness-chain.bin`: size 73, sha256 `95131b83c52bd01803d1bd1eb720d679bd187adc393d4ffbdd0ecdc3789c4a5f`.
- `/tmp/contextgraph-tier2-fsv/readout-trajectory-root.bin`: size 32, sha256 `1bd96ec1d69877b82466cbfe7cfedc2818f340e7c6bfe09e2263c8090a23ff20`.
- `target/release/context-graph-mcp`: size 626691480, sha256 `2b5c8a6effe1f8ef50bd0a6c80c7770dbfd74286f7305fbf2062377761f7abf5`.
- `target/release/context-graph-cli`: size 78724920, sha256 `87882d0afe8503e594b815db5999927cb38e8e43d91e7bf152c72a8749cff919`.

## Open Blockers

- None blocking this task.
- Worktree note: `crates/context-graph-mejepa-corpus/` is untracked and was not touched; treat it as separate concurrent/user work unless confirmed generated.
- Worktree note: `reflexion.rs` fix is currently uncommitted unless an external auto-commit runs after this report.

## Tests / Build

- Tests added: MCP parser tests, causal mask tests/FSV, metadata-filter multi-operator and empty-operator tests, plus earlier primitive/cert FSV assertions.
- Tests passing: yes.
- Build status: passing.

## Resume Here Next Session

- Start by reading: `memory/discoveries/2026-05-09--tier2-recent-hardening.md` and this report.
- Watch out for: existing MCP/CLI clippy warnings are broad technical debt; do not confuse them with the fixed denied `never_loop`.
- Priority: decide whether to commit the uncommitted `reflexion.rs` fix and how to handle the unrelated untracked `crates/context-graph-mejepa-corpus/` directory.
- Open question: none for the Tier 2 hardening itself.
