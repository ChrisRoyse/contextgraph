---
namespace: decisions
agent_id: agent-47-harness-editor
timestamp: 2026-05-07T17:56:17Z
status: superseded
superseded_by: 2026-05-09--single-outer-loop-mejepa-no-local-llm.md
tags: wallclock-budget, attempt-deadline, generic-fix, locked-decision
---

# Per-attempt wallclock budget added to `inner_coding_harness_chat_json`

> **Provenance**: this decision file was authored by the parent optimizer (turn 47 coordinator) on behalf of `cc-reality-harness-editor`, because the harness-editor's tool surface lacks a "create new file" primitive. The editor performed the actual code change and ran verification; this file is the SMAP narrative the editor would have written if it could.

## Symptom

Turn 47: engine PID 82045 has been running 62+ min on a single attempt with no candidate patch and no `attempt-summary.json` written. Turn-46 discovery `agent-46-coordinator--engine-progressing-just-very-slowly.md` measured the inner actor at ~10-25 min per round on the qwen3.6-27b GGUF. At that rate, a 30-round attempt cannot complete before the 1h MCP-stdio idle watchdog forces a kill — and watchdog kills do NOT write `attempt-summary.json`, leaving the optimizer with no diagnosable artifact.

## Decision

Insert a per-attempt wallclock post-check at the top of every round in `fn inner_coding_harness_chat_json` (`crates/context-graph-cli/src/bin/reality-loop.rs`):

- `Instant::now()` captured at function entry → `attempt_start`.
- `attempt_deadline_secs: u64` read from env `CGREALITY_PER_ATTEMPT_DEADLINE_SECS`, default `2700` (45 min). Garbage values fall through to the default; never panics.
- At the top of each round iteration (after the existing `MAX_ROUNDS_PER_ATTEMPT_HARD_CAP` forced-stop, before request_path build), compare `attempt_start.elapsed() >= attempt_deadline`. If exceeded, populate the existing `forced_stop` tuple with new error code `RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED`, push a transcript entry recording elapsed_secs + attempt_deadline_secs, and `break` out of the loop. The existing post-loop blocked path then writes `attempt-summary.json` with the structured error.

Default 2700s allows 2-4 full rounds at the observed 10-25 min/round rate before clean abort.

## Why a post-check, not `tokio::time::timeout`

Turn 41's handoff `agent-41-to-next--wallclock-cap-design.md` originally suggested wrapping `harness.chat()` with `tokio::time::timeout`. Rejected because:

1. `harness.chat()` is synchronous (defined at line 3660 of reality-loop.rs).
2. `InnerCodingHarness` likely holds `!Send` GGUF FFI handles, which would force fragile `spawn_blocking` + `Arc<Mutex<...>>` plumbing to make it timeout-able.

A per-round post-check sidesteps both: no async/await contortions, no FFI handle wrestling, no extra threads. Trade-off: a chat call already in progress runs to completion before the abort fires (worst case ~25 extra min on top of the 2700s budget). That is acceptable; the alternative is the 1h watchdog kill with no summary, which is strictly worse.

## Why this is GENERIC (mental check passes)

Inner-actor per-round generation speed is a model+hardware property, not a task property. Every SWE-bench instance on every Python repo on every model benefits equally from a clean abort with structured diagnosis instead of an opaque watchdog kill.

- `psf__requests-2317` benefits.
- `django__django-12286` benefits.
- `sympy__sympy-13895` benefits.

Zero per-task, per-repo, or per-model branching.

## Trade-offs documented

- A chat call already in progress completes before the abort fires. Worst case: budget exceeded by `max_new_tokens` worth of generation time on the final round (~25 min observed).
- The default 2700s (45 min) allows fewer rounds than the 20-round hard cap. Operator may raise via env override after confirming the inner model is making real progress per round; otherwise the right response is to diagnose generation speed (large prompt, kv-cache pressure, host load).
- Locked decision: do NOT replace this with a `tokio::time::timeout` wrapper unless `harness.chat()` is converted to an async + Send-safe API first. If you need to interrupt mid-chat, that's a separate, larger refactor.

## Verification

- `cargo check -p context-graph-cli --bin reality-loop`: PASS (2.92s)
- `cargo test -p context-graph-cli --bin reality-loop`: PASS (38 passed, 1 ignored, 0 failed, 1153.96s); new test `tests::attempt_budget_env_parses_with_sensible_default_edge_fsv` is green
- `cargo build --release -p context-graph-cli`: PASS (33.62s); `target/release/reality-loop` mtime advanced 11:28:52 → 12:54:36; MCP→CLI shim picks up new binary on next harness call (no `/mcp reconnect` needed per loop.md §4.2)
- Readback PASS: SHA chain `8cae96a6 → ab421342 → ddbec145 → 5b0b081a` intact; on-disk SHA matches round-16 `after_sha`; diff exactly +76 / -0 across 3 hunks; `MAX_ROUNDS_PER_ATTEMPT_HARD_CAP` block preserved verbatim; no edits outside scope.

## Files

- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs:4255-4282` (deadline init).
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs:4352-4378` (per-round budget check, mirrors `MAX_ROUNDS_EXCEEDED` pattern at 4334-4351).
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs:13176-13212` (unit test `attempt_budget_env_parses_with_sensible_default_edge_fsv`).

## Important: in-flight engine NOT affected

Engine PID 82045 was launched from the OLD binary and continues to run unchanged. The new wallclock budget applies to the NEXT attempt the harness mints. Do NOT signal/kill the in-flight engine just to pick up this fix; let it complete or hit its existing limits.

## Locked decision

This is a `status: active` locked decision per loop.md §10. Future agents must not contradict it (i.e., must not revert the wallclock budget without writing a superseding decision file).
