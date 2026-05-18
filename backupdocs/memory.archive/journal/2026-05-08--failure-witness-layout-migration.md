---
namespace: journal
agent_id: codex
timestamp: 2026-05-09T00:04:49Z
status: resolved
tags: ccreality,witness-chain,root-cause,legacy-layout,fail-closed
---

# Failure: Witness Chain Layout Changed Without Migration

## What Happened

The active run `rust_reality_1778240223405536609_aaf099e6` has a
`claude-code-optimizer/witness-chain.bin` that is 10,001 bytes: exactly 137
73-byte entries. The first bytes decode as the older MCP-local layout:

`timestamp_unix | op_type | content_sha256 | prev_chain_hash`

The current clean-room `context-graph-witness` verifier expects the RuVector-
inspired canonical layout:

`prev_hash | action_hash | timestamp_ns | witness_type`

Because the file has no format marker, the current verifier reads the old
timestamp/content bytes as the canonical `prev_hash` and reports
`CCREALITY_WITNESS_PREV_HASH_MISMATCH` at offset 0.

## Why It Matters

This was an unversioned binary-format migration failure, not evidence that the
old chain's own hash links were broken. The correct system behavior is still
fail-closed: recommendation recall must not append new evidence to a chain
whose canonical verifier cannot replay it. The missing piece is explicit
legacy-format detection plus a deliberate, audited reconciliation path that
preserves the old bytes and writes a canonical chain.

## Related

- Active source of truth: `/tmp/contextgraph-rust-reality-loop/rust_reality_1778240223405536609_aaf099e6/claude-code-optimizer/witness-chain.bin`
- New canonical verifier: `crates/context-graph-witness/src/lib.rs`
- MCP wrapper: `crates/context-graph-mcp/src/handlers/tools/reality_loop/witness_chain.rs`

## Resolution

Implemented an explicit fail-closed legacy reconciliation path in
`context-graph-mcp` and repaired the active runtime only after SHA-confirming
the source bytes. The repair preserved the legacy file under
`reality-optimizer/witness-repair/`, wrote a repair claim JSON, translated the
137 legacy entries into canonical `context-graph-witness` entries, appended a
canonical `witness_repair` operation, wrote `witness-chain.format.json`, and
then verified the final chain from the file source of truth.

Active runtime evidence after repair:

- Legacy source SHA before repair: `sha256:a20e799b7ecd94334b10b73e620a28af09764b7f5ca3243c1f01deb08b22280a`
- Canonical chain SHA immediately after repair: `sha256:cd04a60ee954c677e5af1e3bf5ac60bd54a2af2c0ab635b8624c7d881d4ffec2`
- Chain SHA after a real recommendation recall append: `sha256:0552e3a2644c5cf177487c5fd31d6b9784c186939b05fae8336c486900596aff`
- Final verified entry count: `139`
- Final verified last op: `recommendation_recall`
- Final verified last chain hash: `shake256:94a5b13157a55db9adf24d2359e40e5523ef59d0de2da931c75bd17111146395`

Regression and manual FSV coverage passed:

- `cargo test -p context-graph-mcp --lib witness_ -- --nocapture`
- `cargo test -p context-graph-mcp --lib witness_repair_manual_fsv_persists_target_evidence -- --ignored --nocapture`
- `CG_EXPECTED_LEGACY_WITNESS_SHA256=sha256:a20e799b7ecd94334b10b73e620a28af09764b7f5ca3243c1f01deb08b22280a cargo test -p context-graph-mcp --lib witness_repair_active_runtime_legacy_chain -- --ignored --nocapture`
- `cargo test -p context-graph-mcp --lib recall_manual_active_runtime_reads_real_recommendations -- --ignored --nocapture`
- `cargo test -p context-graph-mcp --lib witness_active_runtime_chain_verifies -- --ignored --nocapture`
- `cargo test -p context-graph-mcp --lib recall_ -- --nocapture`
- `cargo build --release -p context-graph-mcp`
