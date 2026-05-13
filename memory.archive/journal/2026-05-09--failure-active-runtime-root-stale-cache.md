---
namespace: journal
agent_id: codex
timestamp: 2026-05-09T01:15:54Z
status: active
tags: ccreality,mcp,witness-chain,active-runtime,test-failure,fsv
---

# Failure: Active Runtime Root Cache Pointed at Hook State

## What Happened

During full-state verification, the ignored test
`witness_active_runtime_chain_verifies` initially failed with `active run id`.
The cached active runtime pointer at `~/.cache/cgreality/active_runtime_root`
contained `/tmp/.tmppb1TSE`, which existed but only held hook state files:
`cgreality-hook-events.jsonl` and `cgreality-hook-state`.

The real active runtime source of truth still existed under
`/tmp/contextgraph-rust-reality-loop` and contained
`rust_reality_1778240223405536609_aaf099e6/claude-code-optimizer/witness-chain.bin`.

## Why It Matters

The failure was not a missing witness chain. It was a stale cache pointer that
made the FSV helper search the wrong root. A bare `active run id` panic hid the
physical state needed to diagnose the issue.

## Fix

- Restored `~/.cache/cgreality/active_runtime_root` to
  `/tmp/contextgraph-rust-reality-loop`.
- Hardened the active runtime FSV helper to report the cached root, expected
  cache file, and runtime directory entries when no matching active run is
  found.

## Source Of Truth Verified After Fix

- Evidence file:
  `crates/context-graph-mcp/target/context-graph-witness-repair-active/verify-evidence.json`
- Evidence SHA-256:
  `7666438cc04d6f7c6de5202606a41f2024dcbe299fe1cc4b438fe0c6d6492707`
- Witness chain:
  `/tmp/contextgraph-rust-reality-loop/rust_reality_1778240223405536609_aaf099e6/claude-code-optimizer/witness-chain.bin`
- Witness SHA-256:
  `0552e3a2644c5cf177487c5fd31d6b9784c186939b05fae8336c486900596aff`
- Entries: `139`
- Bytes: `10147`
- Last op: `recommendation_recall`

## Verification Command

`cargo test -p context-graph-mcp --lib witness_active_runtime_chain_verifies -- --ignored --nocapture`
