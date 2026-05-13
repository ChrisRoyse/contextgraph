---
namespace: discoveries
agent_id: agent-62-coordinator
timestamp: 2026-05-07T21:17:00Z
status: active
tags: behavioral-signal, model-wants-to-terminate, hard-cap-known, generic-improvement-opportunity
---

# Discovery: model called run_official_swe_eval (blocked); MAX_ROUNDS=20 bounds the attempt

## TL;DR

The model in `astropy__astropy-12907` attempt-1 just called `run_official_swe_eval` in round-14 — the harness blocked it with a clear remediation. This is the model trying to gracefully END its work, but it has no "submit-and-exit" primitive. The harness DOES have `MAX_ROUNDS_PER_ATTEMPT_HARD_CAP = 20` (reality-loop.rs:59) plus `RUST_REALITY_REPEATED_ERROR_DEADLOCK` as backstops — current attempt is bounded and will terminate within 5-6 more rounds (~100-150 min).

## Round-14 result (verbatim)

```json
{
  "error_code": "RUST_REALITY_TOOL_OFFICIAL_EVAL_REQUIRES_CANDIDATE_PATCH",
  "result": {
    "message": "this tool only serves PRIOR completed-attempt evidence; official eval for the CURRENT attempt runs automatically when the attempt terminates. If your apply_line_window_edit returned status:ok, your patch is already submitted.",
    "remediation": "stop calling run_official_swe_eval and stop re-applying edits; allow the attempt to terminate so the outer loop can run the official eval automatically",
    "status": "blocked"
  }
}
```

The remediation message is GOOD — clear, directive, and exactly what the model needs to know. But the model has no actionable way to "allow the attempt to terminate" — every round the harness prefills the next request, and the model is forced to emit SOMETHING.

## Hard-cap backstops (already in place)

`reality-loop.rs:59`: `const MAX_ROUNDS_PER_ATTEMPT_HARD_CAP: u64 = 20;` — with `forced_stop` machinery firing `RUST_REALITY_MAX_ROUNDS_EXCEEDED` at round 21.

`reality-loop.rs:60-...`: `REPEATED_ERROR_DEADLOCK_LEN = 5` (per the comment) — if the model emits the same `error_code` 5 rounds in a row, fires `RUST_REALITY_REPEATED_ERROR_DEADLOCK`.

So the attempt IS bounded — currently at round 14-15, max 20 rounds, so ≤6 more rounds. At ~20 min/round average that's ~120 min ETA.

## Generic improvement opportunity (FUTURE, not this turn)

Add a `finalize_attempt` tool to the inner-actor's tool surface. Its action: gracefully terminate the round loop early, persist the current candidate.patch, and trigger the end-of-attempt official Docker eval. Semantics: the model can call this when it's confident in its candidate.

Benefits:
- Saves wallclock (5-6 unproductive rounds × 20 min = ~100 min per attempt where the model wants to stop)
- Reduces re-submission risk (turn-57 discovery): without `finalize_attempt`, the model's only "I'm done" signal is to call apply_*:ok again, which RESETS the workspace and confuses subsequent open_file_window
- GENERIC — applies to every SWE-bench task

Downsides:
- Substantive new tool surface, not a string change
- Needs grammar updates, JSON schema, message handling, end-of-attempt path tweak
- Might cause the model to terminate prematurely if it has an apparently-valid but semantically-wrong patch

Worth evaluating after the current attempt finishes and we have apt-summary data. NOT the next turn's intervention; needs more evidence.

## What this means for THIS attempt

- Round-13 produced the canonical correct patch (CORRECT one-line `=1` → `=right` change)
- Round-14 was the model trying to verify that patch via official eval — blocked
- Round-15+ the model will either (a) accept the blocked message and stop emitting tool calls (probably trips REPEATED_ERROR_DEADLOCK or just times out), (b) call apply_line_window_edit again (workspace-reset cycle continues), or (c) call something else (search_repo, open_file_window, etc.)
- ANY of those paths terminate by round 20 hard cap → `attempt-summary.json` arrives within ~2 cron ticks (next 1-2 turns)

## Source of truth

- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-14-00-run_official_swe_eval/tool-result.json`
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L59` (MAX_ROUNDS_PER_ATTEMPT_HARD_CAP=20)
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L4333-4351` (forced_stop on round overflow)
