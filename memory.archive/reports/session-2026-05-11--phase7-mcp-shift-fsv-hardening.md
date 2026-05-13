---
namespace: reports
agent_id: codex-phase7
timestamp: 2026-05-11T00:00:00Z
created: 2026-05-11T00:00:00Z
updated: 2026-05-11T00:00:00Z
status: active
tags: phase7, mcp-tools, shift-subscriber, fsv, mejepa, codex
---

# Final Report: Phase 7 MCP + Shift Subscriber Hardening

## Task Completed

Implemented and verified the Phase 7 root-cause hardening requested by the operator: durable shift processing, MCP replay/audit readbacks, live subscriber liveness state, panic visibility, release DoD verify, and manual FSV evidence inspection.

## Root Causes Addressed

- Shift subscriber status could not prove live task state or panic state. Added liveness and last-panic metrics, wired `run_until_idle` / `run_until_shutdown` to mark task alive, clear it at exit, and catch `process_shift` panics with structured `MEJEPA_SHIFT_SUBSCRIBER_PROCESS_PANIC`.
- Subscriber/MCP replay paths diverged on shift-to-inference parsing. The MCP replay path now routes through the subscriber `shift_to_inference` transform so the durable replay path and subscriber path share the same source hashing, witness, task context, and patch construction rules.
- Phase 7 MCP audit sorting tripped strict clippy and used a hand comparator. Replaced with `sort_by_key(Reverse(...))`.
- MCP heal helper functions used large unboxed `HealError` return types on the MCP side. Boxed the local helper return type so `context-graph-mcp --no-deps -D warnings` passes without suppressing lints.
- The release `mejepa verify` command needed the corpus append to happen before oracle agreement readback for a synthetic first-run corpus. The verify report now writes a real OracleStore row, reads corpus count before/after, then checks agreement from the same store.

## Files Touched

- `crates/context-graph-mejepa-shift-subscriber/src/runner.rs`
- `crates/context-graph-mejepa-shift-subscriber/src/shift_transform.rs`
- `crates/context-graph-mejepa-shift-subscriber/src/lib.rs`
- `crates/context-graph-mejepa-shift-subscriber/examples/shift_subscriber_fsv.rs`
- `crates/context-graph-mcp/src/handlers/tools/mejepa_phase7_storage.rs`
- `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs`
- `crates/context-graph-mejepa/src/bin/mejepa/stress_cli.rs`
- Existing user/worktree changes also remain in `context-graph-cli` ccreality files and CC hook scripts; do not revert without operator approval.

## Verification Run

- `cargo check -p context-graph-mejepa-shift-subscriber -p context-graph-mejepa -p context-graph-mcp` passed.
- `cargo test -p context-graph-mejepa-shift-subscriber -- --nocapture` passed 5 tests and printed subscriber source-of-truth readback.
- `cargo test -p context-graph-mcp phase7 -- --nocapture` passed 2 tests and printed happy-path plus 3 edge-state before/after audits.
- `cargo test -p context-graph-mejepa verify_cli -- --nocapture` passed 3 verify CLI tests.
- `cargo clippy -p context-graph-mejepa-shift-subscriber -p context-graph-mejepa -p context-graph-mcp --no-deps -- -D warnings` passed.
- `cargo build --release -p context-graph-mejepa -p context-graph-mejepa-shift-subscriber -p context-graph-mcp` passed.
- `cargo run --release -p context-graph-mejepa-shift-subscriber --example shift_subscriber_fsv` passed.
- `cargo run --release -p context-graph-mejepa -- verify --task astropy__astropy-12907 --patch crates/context-graph-mejepa/tests/fixtures/dummy_candidate.patch --corpus-root /tmp/contextgraph-mejepa-verify-cli-corpus.ZjqDfe --evidence-dir /tmp/contextgraph-mejepa-shift-subscriber-fsv --holdout-sample-pct 1.0` passed with final `exit_code: 0`.

## Source Of Truth Evidence

- Shift subscriber FSV directory: `/tmp/contextgraph-mejepa-shift-subscriber-fsv/`
- MCP FSV directory: `/tmp/contextgraph-mejepa-phase7-mcp-fsv/evidence/`
- Verify CLI corpus store: `/tmp/contextgraph-mejepa-verify-cli-corpus.ZjqDfe/oracle-store/`

Manual readback facts:

- All final FSV JSON evidence files in `/tmp/contextgraph-mejepa-shift-subscriber-fsv/` have mode `600`.
- `watermark.json` readback shows `prediction_count: 100`, `oracle_verdict_count: 20`, `phase7_train_cert_count: 20`, and final shift `01J00000000000000000064`.
- `observe-evidence.json` readback shows expected `processed: 100`, `observed: 15`, `dropped_l_step_below_threshold: 5`.
- `predict-latency-evidence.json` readback shows `p50: 9 ms`, `p99: 18 ms`, and budget pass.
- `phase7-dod-verify.json` readback shows `me_jepa_verify_verdict: approve`, `me_jepa_predicts_match_oracle: true`, `corpus_entry_added: 1`, corpus count `2 -> 3`, and `exit_code: 0`.
- MCP FSV manifest and edge files have mode `600`; edge cases preserve state for invalid shift id, missing shift, and page-out-of-range audit.

## Web Research Used

- MCP official schema: tool-originated errors belong in `CallToolResult` with `isError: true`.
- MCP official tools spec: tools expose `inputSchema`, content, and structured content.
- Tokio docs: `AsyncBufReadExt::read_line` / delimiter APIs read to newline/EOF, matching line-buffered JSONL tailing.
- RocksDB Rust docs: `WriteOptions::set_sync(true)` flushes OS buffers before write completion.
- `tracing` docs: structured fields on events/spans use `field_name = field_value`, supporting actionable failure logs.

## Remaining Structural Debt

- `crates/context-graph-mcp/src/handlers/tools/mejepa_phase7_storage.rs` remains over the 500-line task-rule target.
- `crates/context-graph-mejepa-shift-subscriber/src/shift_transform.rs` is also over 500 lines after the harness-transition expansion.

Both are structural cleanup items, not failing functional verification. Split them before further Phase 7 expansion.
