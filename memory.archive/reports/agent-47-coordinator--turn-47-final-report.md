---
namespace: reports
agent_id: agent-47-coordinator
timestamp: 2026-05-07T17:56:17Z
status: active
tags: report, final, turn-47
---

# Turn 47 — per-attempt wallclock budget shipped (`RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED`)

## Status

`changed` — the long-queued wallclock-cap fix from turn 41's handoff finally landed. Engine PID 82045 has been in flight all session and just crossed the 60-min staleness threshold (loop.md §4.5 step 5); rather than continue observing, this turn dispatched the harness-editor to ship the generic fix so the NEXT attempt has a clean abort path.

## What I did

1. Confirmed reality: engine PID 82045 alive at etime 01:02:11 — JUST crossed the 60-min staleness line per loop.md §4.5 step 5. Surfaced in remaining_risks per spec, did NOT kill.
2. Read SMAP (`current-phase.md`, handoffs, decisions, no active blockers — turn-44 blocker was resolved at turn-46 tail).
3. Re-scoped the wallclock-cap design: original turn-41 handoff suggested `tokio::time::timeout` around `harness.chat()`, but `chat()` is synchronous and `InnerCodingHarness` likely holds `!Send` GGUF FFI handles. New design: per-attempt budget POST-check at the top of each round iteration, using the existing `forced_stop` machinery.
4. Dispatched `cc-reality-harness-editor` with the new design + design constraints + acceptance criteria. Editor reported success: +76 lines / -0 across three regions; 38/38 tests pass; release binary rebuilt (`reality-loop` mtime 11:28:52 → 12:54:36).
5. Dispatched `cc-reality-source-of-truth-readback`. Verdict PASS: SHA chain `8cae96a6 → ab421342 → ddbec145 → 5b0b081a` intact; diff exactly +76/-0; `MAX_ROUNDS_PER_ATTEMPT_HARD_CAP` block preserved verbatim; no edits outside scope.
6. Wrote SMAP `decisions/agent-47-harness-editor--per-attempt-wallclock-budget.md` (editor lacks new-file primitive) and `journal/agent-47-readback--per-attempt-wallclock-budget-verification.md` (readback is read-only).

## What changed in the harness

`crates/context-graph-cli/src/bin/reality-loop.rs`:

- **Region (a) lines 4255-4282**: at the top of `fn inner_coding_harness_chat_json`, capture `attempt_start: Instant` and read `attempt_deadline_secs: u64` from env `CGREALITY_PER_ATTEMPT_DEADLINE_SECS` (default `2700` = 45 min; garbage falls through to the default).
- **Region (b) lines 4352-4378**: at the top of each round iteration (immediately after the existing `MAX_ROUNDS_PER_ATTEMPT_HARD_CAP` forced-stop, before `tool-round-NN-request.json` write), compare `attempt_start.elapsed() >= attempt_deadline`. If exceeded, populate the existing `forced_stop` tuple with new error code `RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED`, push a transcript entry recording `elapsed_secs` + `attempt_deadline_secs`, and `break`. The existing post-loop blocked path then writes `attempt-summary.json` with the structured error — no new write path needed.
- **Region (c) lines 13176-13212**: unit test `attempt_budget_env_parses_with_sensible_default_edge_fsv` covering unset / valid / garbage env values.

## Why a POST-check, not `tokio::time::timeout`

The original turn-41 handoff suggested wrapping `harness.chat()` with `tokio::time::timeout`. Rejected:

- `chat()` is synchronous (line 3660 of `reality-loop.rs`).
- `InnerCodingHarness` likely holds `!Send` GGUF FFI handles, forcing fragile `spawn_blocking` + `Arc<Mutex<...>>` plumbing to make it timeout-able.

The post-check sidesteps both: no async/await contortions, no FFI handle wrestling, no extra threads, no leaked-thread cleanup story. Trade-off: a chat call already in progress runs to completion before the abort fires. Worst case: budget exceeded by `max_new_tokens` worth of generation time on the final round (~25 min observed). Acceptable — the alternative is the 1h MCP-stdio idle watchdog kill with no `attempt-summary.json`, which is strictly worse.

## GENERIC-ONLY check

Inner-actor per-round generation speed is a model+hardware property, not a task property. Every SWE-bench instance on every Python repo on every model benefits equally. Mental check passes: `psf__requests-2317`, `django__django-12286`, `sympy__sympy-13895` all benefit identically.

## Verification commands run

- `cargo check -p context-graph-cli --bin reality-loop`: PASS (2.92s).
- `cargo test -p context-graph-cli --bin reality-loop`: PASS (38 passed, 1 ignored — pre-existing BF16 GGUF test, 0 failed, 1153.96s).
- `cargo build --release -p context-graph-cli`: PASS (33.62s); `target/release/reality-loop` mtime advanced 11:28:52 → 12:54:36.
- `git diff --stat`: 1 file changed, 76 insertions(+), 0 deletions(-).
- `git status --porcelain`: only `M crates/context-graph-cli/src/bin/reality-loop.rs`.

## Hard rules check

- ✅ No HTTP.
- ✅ No deny-set edits.
- ✅ No claim of SWE-bench success.
- ✅ No destructive git.
- ✅ No engine kill (PID 82045 left alone, per loop.md §4.5 step 5).
- ✅ No parallel writers (single harness-editor invocation).
- ✅ Single harness-editor cap: one used.
- ✅ GENERIC-ONLY check passed.
- ✅ SMAP report paired with formal recommendation.
- ✅ No cost-cap or budget guard added (this is wallclock, not spend — different concept; per `feedback_no_cost_caps` auto-memory).

## Engine staleness — surfaced per spec

Engine PID 82045 etime 01:02:11 at this turn's probe — past the 60-min threshold per loop.md §4.5 step 5. Last 4 turns (43/44/45/46) all recorded `no_change` for "round still in flight". Surfacing in `remaining_risks` of this turn's recommendation as the spec prescribes. Still NOT killing it — the new binary doesn't apply to PID 82045 anyway (its etime predates the new binary mtime).

The 1h MCP-stdio idle watchdog should fire any moment now (engine started ~11:34, watchdog at 12:34 — basically now). When it does, attempt-summary.json may NOT be written for this attempt. The new wallclock budget is exactly what would have prevented this; from the next drip onward, abort will be clean.

## Files touched (non-source)

- `memory/decisions/agent-47-harness-editor--per-attempt-wallclock-budget.md` (new, locked decision).
- `memory/journal/agent-47-readback--per-attempt-wallclock-budget-verification.md` (new, readback verdict).
- `memory/reports/agent-47-coordinator--turn-47-final-report.md` (this file).

## Source-of-truth readbacks

- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs` (regions 4255-4282, 4352-4378, 13176-13212).
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-13.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-14.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-15.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-16.json`
- `file:./memory/decisions/agent-47-harness-editor--per-attempt-wallclock-budget.md`
- `file:./memory/journal/agent-47-readback--per-attempt-wallclock-budget-verification.md`
- `file:./memory/reports/agent-47-coordinator--turn-47-final-report.md`

## Next-turn priorities

1. **Check engine state**: is PID 82045 still alive? Did the 1h MCP-stdio watchdog fire? Did `attempt-summary.json` appear?
2. **If engine terminated** (any path): drip a fresh attempt against `astropy__astropy-12907` to validate the new wallclock budget. Set `CGREALITY_PER_ATTEMPT_DEADLINE_SECS=2700` (default) for the first run; tune up/down based on observed round count.
3. **Validate the cap fires correctly**: a fresh attempt with the new binary should produce an `attempt-summary.json` with `error_code: RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED` if the model still generates at 10-25 min/round. That's success — the cap is doing its job.
4. **If the model is faster on the new attempt** (no hang reproduced), the cap is invisible and the attempt completes normally with whatever `passed_fail_to_pass_ratio` the patch achieves. Either outcome is information.
5. **The wallclock-cap fix is now SHIPPED** — turn-41 handoff item closed. Future handoff items should focus on: prompt-template adjustments, slow-generation root-cause investigation (kv-cache pressure, prompt size), or model swap.

## What this fix does NOT do

- Does NOT speed up the model. Generation rate is a model+hardware property; the fix only bounds total attempt wallclock.
- Does NOT interrupt a chat call already in progress. The chat returns naturally; the abort fires on the NEXT round's top-of-loop check. Worst case overshoot: ~25 min.
- Does NOT replace the 1h MCP-stdio idle watchdog (it complements it). Stdio watchdog is the last-resort kill; budget is the explicit clean abort.
- Does NOT add cost-cap or spend-tracking semantics — it's purely wallclock, per the auto-memory `feedback_no_cost_caps` rule.
