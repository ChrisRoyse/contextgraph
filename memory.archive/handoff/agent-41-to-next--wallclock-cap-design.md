---
namespace: handoff
agent_id: agent-41-coordinator
timestamp: 2026-05-07T15:52:14Z
status: active
tags: handoff, wallclock-cap, silent-hang, design-required
---

# Handoff: per-round wallclock cap — design needed before edit

## Why this is queued, not done

The original note (in turn-40's `current-phase.md`) said: *"per-round wallclock cap in `crates/context-graph-cli/src/bin/reality-loop.rs` chat-loop around line 4388, wrap `harness.chat()` with a tokio-style timeout (suggested initial value: 8 min per round)"*.

When I (turn 41) opened the file, I found:

- `harness.chat()` is **synchronous** — its declaration is `fn chat(&self, args: &AttemptArgs, body: &Value, request_path: &Path) -> Result<Value>` at `crates/context-graph-cli/src/bin/reality-loop.rs:3660`. No `async`.
- The enclosing function (`fn inner_coding_harness_chat_json`, line 4249) is also synchronous.
- `tokio::time::timeout` works only on futures. It cannot wrap a blocking call directly.
- `#[tokio::main]` exists at line 479 — there IS a tokio runtime in `main()` — but the chat loop runs synchronously inside the runtime's blocking world.

So "tokio-style timeout" needs translation:

| Approach | Pros | Cons |
|----------|------|------|
| `tokio::task::spawn_blocking` + `tokio::time::timeout` on the JoinHandle | clean tokio idiom, integrates with the existing runtime | only works if the surrounding function is `async`. The current one is sync. Refactoring to async cascades up through several call sites. |
| `std::thread::spawn` + `crossbeam_channel::recv_timeout` | works in pure sync code | thread leaks on timeout (can't kill GGUF inference cleanly); leaked thread continues consuming GPU+CPU until process exit |
| Pass a deadline INTO `harness.chat()` and have it check periodically | clean cancel semantics | invasive — harness internals must support cancellation polling, may need changes to llama-bindings |
| Send a signal to a forked subprocess | clean kill | GGUF runs in-process via candle/llama bindings, not a subprocess |

**Recommendation for the next agent**: use `std::thread::spawn` + `crossbeam_channel`/`mpsc::channel` + `recv_timeout`. Accept the leaked-thread cost. The doomsday watchdog (`DOOMSDAY_SECS=20`) + the engine-binary lifecycle (single-attempt, exits after one attempt) bound the leak. Each leaked thread will be cleaned up when the engine process exits, which happens within minutes regardless.

## Design checklist for the wallclock-cap fix

When the next agent picks this up:

1. **Choose deadline value**: 8 min/round is a reasonable starting point for the silent-hang case (current observed: 13 min). Consider making it configurable via an env var (e.g. `CGREALITY_PER_ROUND_DEADLINE_SECS`, default 480).
2. **Threading**: spawn the synchronous `harness.chat(args, &body, &request_path)` call on a `std::thread`; main loop waits on a `crossbeam_channel::Receiver::recv_timeout(deadline)`. On timeout, drop the receiver and proceed to abort.
3. **Reality-event emission**: emit a JSON line `{"event": "round_stall_aborted", "round": N, "deadline_secs": 480, "unix": ...}` to `<attempt_dir>/reality-events.jsonl` on the abort path.
4. **attempt-summary.json on abort**: ensure the engine still writes a partial attempt-summary on the abort path. Today, attempt-summary is written near end-of-attempt; the abort path needs to invoke that writer with the partial state. Likely the cleanest place is to set an `aborted_reason: round_stall` field on the in-memory attempt state, then fall through to the normal end-of-attempt writer.
5. **Test**: a contrived attempt that holds the chat call past the deadline should produce an attempt-summary.json with `aborted_reason: round_stall` and a corresponding reality-events.jsonl entry, AND the engine binary exits cleanly (not via doomsday).
6. **Build**: `cargo build --release -p context-graph-cli` (the engine + harness CLI). The MCP→CLI shim picks up the new binary on the next harness call automatically — no `/mcp reconnect` needed (per loop.md §4.2).
7. **GENERIC-ONLY check**: a per-round wallclock cap helps any task whose inner actor silent-hangs after a successful tool call (which is repo-agnostic — it's a model-side behavior). Pass.

## Why this matters

Without the cap, a silent-hung round consumes ~1h before the MCP-stdio idle watchdog fires (`MAX_STDIO_IDLE_SECS=3600`). That's 1h of wasted compute per stuck attempt. With an 8-min cap and proper attempt-summary write, the engine produces useful evidence (partial transcript, reality-events showing the stall) and exits to let the next attempt run.

Estimated effort: 1 turn (3-5 hours of focused work) for an experienced agent who reads `crates/context-graph-cli/src/bin/reality-loop.rs:3660-4400` carefully first.

## What NOT to do

- Don't try to refactor `inner_coding_harness_chat_json` to async — the cascade is large.
- Don't add a TWO-level timeout (e.g. one per token + one per round) — start with the round-level one, validate it works, layer more granular caps later only if data warrants.
- Don't introduce a new tokio task on a NEW runtime — there's already one from `#[tokio::main]`.
- Don't add cost-related guards. The system runs locally; no spend caps anywhere (per `feedback_no_cost_caps` in auto-memory).
