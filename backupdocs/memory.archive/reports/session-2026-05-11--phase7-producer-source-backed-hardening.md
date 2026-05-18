---
namespace: reports
created: 2026-05-11T02:09:55Z
updated: 2026-05-11T02:09:55Z
status: active
tags: report, session, phase7, mcp-tools, shift-subscriber, fsv
---

# Session Report: 2026-05-11 - Phase 7 Producer Source-Backed Hardening

## Task

Harden Phase 7 MCP tools and shift subscriber against real producer contract drift, with manual Full State Verification.

## Outcome

Complete for the Phase 7 code paths touched in this session.

## Summary

Root cause: Phase 7 tests and replay logic accepted synthetic-rich shift records while live `ccreality` producers emitted source-backed records. The subscriber transform now derives code edits from real before/after source artifacts, validates declared hashes, and refuses missing witness-chain evidence. MCP replay uses the same subscriber transform so durable replay and live subscriber behavior cannot drift independently.

I also fixed the direct `harness_apply_line_window_edit` producer to emit source-backed before text and witness evidence, hardened `optimizer_record_harness_transition` so it fails closed if preedit/after source artifacts are absent, and made non-code shifts advance the watermark explicitly as `no_oracle_signal`. The hook smoke test exposed stale release CLI selection; shell and CLI binary resolution now use the newest local binary for HTTP-style configs while explicit command configs remain fail-closed.

## Files Created or Modified in the Repo

- `crates/context-graph-cli/src/commands/ccreality/mod.rs` - CLI schema and MCP binary resolution hardening for HTTP-style `.mcp.json`.
- `crates/context-graph-mcp/src/handlers/tools/mejepa_phase7_storage.rs` - reject all-zero session ids consistently.
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/alter.rs` - direct edit producer emits before source, after source, transition artifact, and witness-chain evidence.
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/optimizer.rs` - optimizer transition producer requires preedit and after-source artifacts and exposes witness segment extraction.
- `crates/context-graph-mejepa-shift-subscriber/src/runner.rs` - FSV test for non-code skip watermark readback.
- `crates/context-graph-mejepa-shift-subscriber/src/shift_transform.rs` - source-backed inference candidate detection.
- `crates/context-graph-mejepa-shift-subscriber/src/subscriber.rs` - explicit non-code shift watermarking.
- `scripts/cc/_common.sh` - newest local CLI binary selection.

Unrelated pre-existing/parallel worktree changes were present in ME-JEPA CF/train/eval files; do not revert them as part of this Phase 7 handoff.

## Files Written or Updated in ./memory/

- `memory/discoveries/2026-05-11--phase7-real-producer-shift-contract.md` - root cause and hardened shift contract.
- `memory/discoveries/2026-05-11--ccreality-stale-binary-selection.md` - hook smoke failure and binary-resolution root cause.
- `memory/decisions/2026-05-11--phase7-fail-closed-source-backed-shifts.md` - Phase 7 no-fallback shift decision.
- `memory/reports/session-2026-05-11--phase7-producer-source-backed-hardening.md` - this report.

## Decisions Made

- Phase 7 code-edit inference must be source-backed and fail closed on missing before/after/witness evidence.
- Non-code reality shifts should advance the watermark as `no_oracle_signal`, not crash the subscriber or fabricate patches.
- Hook smoke tests must exercise normal binary resolution, not only explicit environment overrides.

## Discoveries

- Green synthetic fixtures hid real producer/consumer drift in Phase 7 shift records.
- Stale release CLI selection can break hook schema changes even after debug builds pass.

## Failures

- `scripts/cc/tests/hooks/test_post_tool_use_edit.sh` initially failed because stale release `context-graph-cli` rejected `--preedit-state-path`. The root cause was binary selection, not the hook payload. After fixing binary resolution, the same test passed without overrides.

## Successes

- `cargo check -p context-graph-mejepa-shift-subscriber -p context-graph-mcp -p context-graph-cli` passed.
- `cargo fmt --check` passed.
- `cargo test -p context-graph-mejepa-shift-subscriber -- --nocapture` passed 8 tests.
- `cargo test -p context-graph-mcp phase7_mcp_fsv_happy_path_and_edges -- --nocapture` passed.
- `cargo test -p context-graph-mcp retired_tool_returns_mcp_tool_error_result -- --nocapture` passed.
- `cargo test -p context-graph-mcp optimizer_preflight_fsv -- --nocapture` passed.
- `cargo test -p context-graph-cli ccreality::tests::mcp_binary -- --nocapture` passed 6 tests.
- `scripts/cc/tests/hooks/test_post_tool_use_edit.sh` passed after the fix.
- `cargo run -p context-graph-mejepa-shift-subscriber --example shift_subscriber_fsv` passed.

## Source-of-Truth Evidence

- Subscriber FSV directory: `/tmp/contextgraph-mejepa-shift-subscriber-fsv/`.
- Subscriber before state: `prediction_count=0`, `oracle_verdict_count=0`, `phase7_train_cert_count=0`.
- Subscriber after state: `prediction_count=100`, `oracle_verdict_count=20`, `phase7_train_cert_count=20`, final watermark `01J00000000000000000064` at byte offset `126377`.
- Direct RocksDB readback: `CF_MEJEPA_LIVE_PREDICTIONS=100`, `cert:phase7:*` train certs `20`, watermark row persisted for session `22222222222222222222222222222222`.
- Panel store readback: `mejepa_panels=300`, `mejepa_panel_meta=300`.
- Source files readback: 100 preedit before-text files and 100 after-source files.
- MCP FSV before/after: prediction count `0 -> 1`, T0/T1/T2 panels `false -> true`, and invalid shift id/missing shift/page out-of-range edge cases left source-of-truth state unchanged.

## Resume Here Next Session

- Start by reading this report and `memory/discoveries/2026-05-11--phase7-real-producer-shift-contract.md`.
- Watch for unrelated worktree changes in ME-JEPA CF/train/eval modules before staging or committing.
- Priority: split the still-large Phase 7 storage/transform files if more Phase 7 expansion is needed.

## Confidence

High. The hardened paths were verified with unit tests, MCP FSV, hook smoke, direct RocksDB `ldb` scans, panel-store counts, and source artifact readbacks.
