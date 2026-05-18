---
namespace: reports
agent_id: agent-159-coordinator
timestamp: 2026-05-09T00:04:49Z
status: resolved
tags: report, final, turn-159, ccreality, witness-chain, legacy-repair, full-state-verification
---

# Turn 159 - Witness-chain legacy layout root-cause fix and active-runtime repair

## Request

Read the RuVector findings, recent git history, and SOLO-AGENT-MEMORY-PROTOCOL,
then identify and fix the root cause of the current issues without fallbacks or
mock-only tests. Perform full state verification against the physical source of
truth and manually exercise happy and edge cases.

## Root Cause

The active runtime chain at
`/tmp/contextgraph-rust-reality-loop/rust_reality_1778240223405536609_aaf099e6/claude-code-optimizer/witness-chain.bin`
was not random corruption. It was a valid 10,001-byte legacy chain:
137 entries x 73 bytes, using the old MCP-local binary layout
`timestamp_unix | op_type | content_sha256 | prev_chain_hash`.

The current clean-room witness crate verifies canonical entries as
`prev_hash | action_hash | timestamp_ns | witness_type`. Because the old file
had no format marker, the canonical verifier interpreted the legacy timestamp
and content bytes as `prev_hash` and failed at offset 0 with a prev-hash
mismatch. The real root cause was an unversioned binary layout migration.

## Implemented Fix

- Added explicit legacy layout replay and diagnosis in
  `crates/context-graph-mcp/src/handlers/tools/reality_loop/witness_chain_legacy.rs`.
- Added canonical layout sidecar manifest handling in
  `witness_chain_format.rs`.
- Added locked, fsync/readback-checked witness-chain IO in
  `witness_chain_io.rs`.
- Added an explicit `optimizer_witness_chain_repair_legacy` MCP tool in
  `witness_chain_repair.rs` and registered it in the MCP tool definitions and
  dispatcher.
- Updated canonical append/verify in `witness_chain.rs` so legacy files are
  detected fail-closed as `CCREALITY_WITNESS_LEGACY_LAYOUT_DETECTED` instead of
  being silently accepted.
- Added `WitnessOpType::WitnessRepair = 8` so a repair is itself witnessed.
- Added FSV and active-runtime tests in `witness_chain_fsv.rs`,
  `witness_chain_repair_fsv.rs`, and updated recall preflight expectations.

No RuVector or agentdb dependency was added; the implementation remains
clean-room and local to the existing `context-graph-witness`/MCP design.

## Source Of Truth

Primary source of truth:

`/tmp/contextgraph-rust-reality-loop/rust_reality_1778240223405536609_aaf099e6/claude-code-optimizer/witness-chain.bin`

Secondary physical evidence:

- Legacy backup:
  `/tmp/contextgraph-rust-reality-loop/rust_reality_1778240223405536609_aaf099e6/reality-optimizer/witness-repair/witness-chain-legacy-0001-a20e799b7ecd9433.bin`
- Repair claim:
  `/tmp/contextgraph-rust-reality-loop/rust_reality_1778240223405536609_aaf099e6/reality-optimizer/witness-repair/repair-0001-claim.json`
- Format manifest:
  `/tmp/contextgraph-rust-reality-loop/rust_reality_1778240223405536609_aaf099e6/claude-code-optimizer/witness-chain.format.json`
- Active FSV evidence:
  `crates/context-graph-mcp/target/context-graph-witness-repair-active/manual-evidence.json`
  and `verify-evidence.json`
- Synthetic manual FSV evidence:
  `crates/context-graph-mcp/target/context-graph-witness-repair-fsv/manual-evidence.json`

## Manual State Verification

Active happy path:

- Before repair: source chain SHA
  `sha256:a20e799b7ecd94334b10b73e620a28af09764b7f5ca3243c1f01deb08b22280a`,
  size `10001`, verifier returned
  `CCREALITY_WITNESS_LEGACY_LAYOUT_DETECTED`, legacy entries `137`.
- Repair required that exact SHA and refused to run if bytes changed.
- After repair: canonical source chain SHA
  `sha256:cd04a60ee954c677e5af1e3bf5ac60bd54a2af2c0ab635b8624c7d881d4ffec2`,
  size `10074`, entries `138`, last op `witness_repair`, valid `true`.
- After a real recommendation recall append: source chain SHA
  `sha256:0552e3a2644c5cf177487c5fd31d6b9784c186939b05fae8336c486900596aff`,
  entries `139`, last op `recommendation_recall`, valid `true`,
  last chain hash
  `shake256:94a5b13157a55db9adf24d2359e40e5523ef59d0de2da931c75bd17111146395`.

Synthetic edge cases manually audited with before and after source reads:

- Empty chain: bytes remained `0`; repair failed with
  `CCREALITY_WITNESS_REPAIR_EMPTY_CHAIN`.
- Corrupt legacy prev hash: bytes remained `146`; repair failed with
  `CCREALITY_WITNESS_REPAIR_LEGACY_REPLAY_FAILED`.
- Already canonical chain: bytes remained `73`; repair failed with
  `CCREALITY_WITNESS_REPAIR_NOT_NEEDED`.

## Verification Run

- `cargo check -p context-graph-mcp`
- `cargo test -p context-graph-mcp --lib witness_ -- --nocapture`
- `cargo test -p context-graph-mcp --lib witness_repair_manual_fsv_persists_target_evidence -- --ignored --nocapture`
- `CG_EXPECTED_LEGACY_WITNESS_SHA256=sha256:a20e799b7ecd94334b10b73e620a28af09764b7f5ca3243c1f01deb08b22280a cargo test -p context-graph-mcp --lib witness_repair_active_runtime_legacy_chain -- --ignored --nocapture`
- `cargo test -p context-graph-mcp --lib recall_manual_active_runtime_reads_real_recommendations -- --ignored --nocapture`
- `cargo test -p context-graph-mcp --lib witness_active_runtime_chain_verifies -- --ignored --nocapture`
- `cargo test -p context-graph-mcp --lib recall_ -- --nocapture`
- `cargo test -p context-graph-mcp --lib tools::definitions::tests::test_reality_loop_lightweight_registry_only_lists_callable_tools -- --nocapture`
- `cargo test -p context-graph-mcp --lib tools::definitions::tests::test_total_tool_count_and_no_duplicates -- --nocapture`
- `cargo test -p context-graph-mcp --lib tools::definitions::tests::test_submodule_counts -- --nocapture`
- `cargo test -p context-graph-mcp --lib test_e2e_tools_list_all_tools -- --nocapture`
- `cargo build --release -p context-graph-mcp`

Clean-room checks for forbidden RuVector/agentdb dependencies also passed.

## Web Research Applied

Used Rust std file IO documentation for append/sync semantics, SQLite atomic
commit documentation for durable replace/write ordering, RFC 6962 for
append-only log ideas, and Protocol Buffers versioning guidance as a reminder
that binary layout evolution needs explicit version handling. The implementation
therefore uses explicit layout detection, a manifest, locked writes, readback
verification, backup-before-replace, and fail-closed repair instead of
fallback behavior.

## Final State

Latest local code commit: `33daf95b mcp/reality-loop: add witness-chain legacy reconciliation + canonical format manifest`.

The active witness chain now verifies from the file source of truth. A real
recommendation recall append completed after repair and was physically verified
in both the recall audit JSON and the canonical witness chain.
