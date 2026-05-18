---
namespace: reports
agent_id: codex
timestamp: 2026-05-09T00:16:59Z
status: active
tags: report, final, ccreality, witness-chain, preflight, fsv
---

# Session Report: Witness Preflight Hardening

## Task

Read the RuVector findings docs and Solo Agent Memory Protocol, review recent git history, identify the root cause behind recent reality-loop witness issues, research robust storage/schema practices, implement a non-fallback fix, and manually verify source-of-truth state.

## Root Cause

The immediate root cause was an unversioned binary witness-chain layout transition. The old MCP-local layout and the canonical `context-graph-witness` layout are both 73 bytes per entry, so a legacy chain could be misinterpreted as canonical unless the verifier explicitly detected and reported the legacy layout. Recent commits added legacy detection, repair, and a canonical format manifest.

The related class of bugs was that some optimizer mutation paths still wrote durable state before proving the run witness chain was appendable. Recommendation recall had already been fixed with a witness preflight. Influence, bandit, decision, recommendation, and harness-transition writes still needed the same invariant.

## Research Applied

- SQLite's atomic commit documentation reinforced that durable state transitions need explicit before/after ordering and should not expose partially committed state.
- Rust `File::sync_all` and `std::fs::rename` docs reinforced that writes, flushes, and replacement operations need explicit error handling rather than relying on close/drop behavior.
- Protocol Buffers schema evolution guidance reinforced that binary formats need explicit evolution rules; reinterpreting old bytes under a new schema is unsafe without versioning or detection.

## Implementation

Commits verified on `ccreality-cutover` through `b4fbea9a`:

- Added `verify_witness_chain_for_run` preflight before mutation in `optimizer_bandit_select`.
- Added the same preflight before mutation in `optimizer_bandit_record_reward`.
- Added the same preflight before influence audit creation in `compute_influence_for_run`.
- Added the same preflight before `optimizer_record_decision`, `optimizer_record_recommendation`, and `optimizer_record_harness_transition` writes.
- Surfaced `witness_preflight` in response/audit payloads so callers can see the chain state that was verified before mutation.
- Added fail-closed synthetic tests for corrupt witness chains proving state files and witness files remain byte-identical across failed calls.
- Hardened `write_text_checked` so JSON/text artifacts are written to a same-directory temporary file, synced, read back, atomically renamed, parent-directory synced, and read back again.
- Added optimizer preflight FSV coverage in `optimizer_preflight_fsv.rs`.
- Replaced per-module runtime-cache test locks with shared `TEST_RUNTIME_ENV_LOCK` after the combined `witness_` test filter exposed a real race on `~/.cache/cgreality/active_runtime_root`.

No RuVector/AgentDB dependency or vendored code was introduced.

## Source Of Truth

Relevant source-of-truth files verified during manual FSV:

- Influence audit: `target/context-graph-influence-fsv/runtime-root/run-happy/reality-optimizer/influence/influence-0001.json`
- Influence witness chain: `target/context-graph-influence-fsv/runtime-root/run-happy/claude-code-optimizer/witness-chain.bin`
- Bandit state: temp-run `reality-optimizer/solver-bandit.json`, read immediately after select/reward tests.
- Active runtime witness chain: `/tmp/contextgraph-rust-reality-loop/rust_reality_1778240223405536609_aaf099e6/claude-code-optimizer/witness-chain.bin`
- Persistent evidence:
  - `target/context-graph-influence-fsv/manual-evidence.json`, SHA-256 `6f97fc4b6415227dfe8b879645375f5dfdf0d0091e1eade2adfe374bc5b0f2c0`
  - `crates/context-graph-mcp/target/context-graph-witness-repair-fsv/manual-evidence.json`, SHA-256 `569b81419e18ececcbe88ae3f4a09cf9d8d1f5ebb0e335d17a30b5013697305a`
  - `crates/context-graph-mcp/target/context-graph-witness-repair-active/verify-evidence.json`, SHA-256 `7666438cc04d6f7c6de5202606a41f2024dcbe299fe1cc4b438fe0c6d6492707`

## Manual FSV Evidence

Influence happy path:

- Before audit count: 0.
- After audit count: 1.
- Witness chain: 73 bytes, 1 entry.
- Graph: 11 nodes, 22 edges.
- Top attempts for `import-error`: `task-c#2`, `task-a#1`.
- Top recommendations: `task-c/attempt-2`, `task-a/attempt-1`.

Influence edge cases:

- Empty attempts: audit count stayed 0, error `CCREALITY_INFLUENCE_ATTEMPTS_EMPTY`.
- Unknown failure tag: audit count stayed 0, error `CCREALITY_INFLUENCE_FAILURE_TAG_UNKNOWN`.
- Orphan recommendation: audit count stayed 0, error `CCREALITY_INFLUENCE_ORPHAN_RECOMMENDATION`.
- Corrupt witness preflight: audit count stayed 0, witness SHA stayed `sha256:6067beef64ab98a025b9925893fa97f6681f92aa4e3cb00977c72ddf74fa9958`, error `CCREALITY_WITNESS_PREV_HASH_MISMATCH`.

Bandit edge cases:

- Corrupt witness before select: `solver-bandit.json` did not exist before or after, witness SHA unchanged, error `CCREALITY_WITNESS_PREV_HASH_MISMATCH`.
- Corrupt witness before reward: existing `solver-bandit.json` SHA unchanged, witness SHA unchanged, error `CCREALITY_WITNESS_PREV_HASH_MISMATCH`.

Optimizer edge cases:

- Corrupt witness before decision write: optimizer directory and `trigger-decision.json` did not exist before or after, witness SHA unchanged, error `CCREALITY_WITNESS_PREV_HASH_MISMATCH`.
- Corrupt witness before harness transition write: transition JSON count stayed 0, witness SHA unchanged, error `CCREALITY_WITNESS_PREV_HASH_MISMATCH`.

Witness repair/active verification:

- Manual legacy repair happy path: legacy chain 146 bytes/2 entries became canonical chain 219 bytes/3 entries with one repair audit and valid manifest.
- Repair edge cases empty/corrupt-legacy-prev/already-canonical all failed closed with bytes and SHA unchanged.
- Active runtime chain verified valid: 10,147 bytes, 139 entries, manifest valid, SHA `sha256:0552e3a2644c5cf177487c5fd31d6b9784c186939b05fae8336c486900596aff`, last op `recommendation_recall`.

## Verification Commands

- `cargo fmt --all`
- `cargo test -p context-graph-mcp --lib influence_ -- --nocapture`
- `cargo test -p context-graph-mcp --lib bandit -- --nocapture`
- `cargo test -p context-graph-mcp --lib optimizer_preflight -- --nocapture`
- `cargo test -p context-graph-mcp --lib helpers:: -- --nocapture`
- `cargo test -p context-graph-mcp --lib witness_ -- --nocapture`
- `cargo test -p context-graph-mcp --lib influence_manual_fsv_persists_target_evidence -- --ignored --nocapture`
- `cargo test -p context-graph-mcp --lib witness_repair_manual_fsv_persists_target_evidence -- --ignored --nocapture`
- `cargo test -p context-graph-mcp --lib witness_active_runtime_chain_verifies -- --ignored --nocapture`
- `cargo check -p context-graph-mcp`
- `cargo build --release -p context-graph-mcp`

All passed. Only pre-existing dead-code warnings appeared in governed/path-policy modules.

## Notes

Worktree code changes were committed through `b4fbea9a` before this memory report was updated. At report time, unrelated/unreviewed workspace changes existed in `configs/dynamicjepa/verification/tiny_train_config.json` and RuVector docs; they were not modified by this report. The memory report itself is intentionally uncommitted session documentation unless separately committed later.
