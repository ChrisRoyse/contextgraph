---
namespace: journal
created: 2026-05-08T23:28:23Z
updated: 2026-05-08T23:28:23Z
status: active
tags: ccreality,recommendation-recall,witness-chain,fsv,fail-closed
---

# Failure: Active Runtime Recall Hit Corrupt Witness Chain

## What Happened

While manually exercising the real active runtime with `recall_manual_active_runtime_reads_real_recommendations`, `optimizer_recall_recommendations` wrote `recall-0001.json` and then failed while appending the witness entry. The active run's existing `witness-chain.bin` failed replay at offset 0: the first entry had a non-zero `prev_chain_hash` where the current verifier expects genesis zero.

## Why It Matters

The new recall tool originally wrote the recall audit before checking whether the existing witness chain was appendable. That created a partial-write hazard: a durable recall audit could exist without a matching witness entry if the pre-existing chain was corrupt.

## Fix

Recommendation recall now preflights `witness-chain.bin` before collecting/writing the recall audit. If the chain is corrupt, the tool returns the structured witness error and writes no new recall audit. The active-runtime rerun proved this fail-closed behavior:

- Active run: `rust_reality_1778240223405536609_aaf099e6`
- Before audit count: 1
- After audit count: 1
- Error: `CCREALITY_WITNESS_PREV_HASH_MISMATCH`
- Source of truth: `file:/tmp/contextgraph-rust-reality-loop/rust_reality_1778240223405536609_aaf099e6/claude-code-optimizer/witness-chain.bin`

The existing `recall-0001.json` in that active run is the failed preflight artifact from before the fix. Do not treat it as witnessed recall evidence.

## Related

- `crates/context-graph-mcp/src/handlers/tools/reality_loop/recommendations.rs`
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/witness_chain.rs`
- `target/context-graph-recall-fsv/manual-evidence.json`
