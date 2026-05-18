---
namespace: discoveries
agent_id: agent-72-coordinator
timestamp: 2026-05-08T00:25:00Z
status: active
tags: turn-47-validated, wallclock-cap, too-aggressive, budget-tuning, generic, hardware-specific-tuning
---

# Discovery: turn-47 wallclock cap fires correctly but budget (45 min) is too tight for this hardware

## TL;DR

Operator-driven post-blocker drip (Docker fixed) ran a fresh attempt against `astropy__astropy-12907` with the new binary. The turn-47 wallclock cap FIRED AS DESIGNED at 3396s ≥ 2700s, producing a clean `RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED` error code in `attempt-summary.json`. Validation passed-end-to-end as a structural improvement.

**However**, the budget is too tight for this hardware: at ~14 min/round, the model only completed 4 navigation rounds (read_problem_reality + 3x open_file_window) before the cap fired. No candidate patch was submitted because the model never reached the apply_*_edit phase.

## Run details

- New runtime root: `/tmp/contextgraph-rust-reality-loop/rust_reality_1778198277123757517_aaf099e6/`
- Status: `unsolved`, validation_status: `failed`, error_code: `RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED`
- Wallclock: 3396s used vs 2700s budget (overshoot by 696s = ~12 min, expected because the in-flight round can't be cancelled mid-stream)
- Rounds completed: 4 (avg ~14 min/round)
- Tools used: `read_problem_reality` (1), `open_file_window` (3) — all `repository_navigation` family
- No `apply_*_edit` ever attempted
- candidate_patch: null
- official_eval_ran: false (no patch to test)

## Sanity check: turn-47 fix works

✅ Cap fires cleanly at the configured threshold.
✅ `RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED` is the surfaced error_code (matches turn-47 spec).
✅ `attempt-summary.json` IS written (unlike Docker-blocked path on previous attempt).
✅ `reality-signal-packet.json` IS written.
✅ The structural intervention from turn 47 is validated end-to-end. No code change needed there.

## Problem: 45 min budget is too aggressive for this hardware

Previous attempt-1 (PID 82045) took ~7 hours and produced a correct patch in round 13 (after ~4h of navigation/exploration). At ~14-26 min/round, the typical "useful trajectory" needs at MINIMUM:

- 1 round: read_problem_reality
- 2-3 rounds: open_file_window for target file + tests
- 1-2 rounds: search_repo if needed
- 1 round: apply_line_window_edit
- 1 round: run_targeted_test (verify) — though local often fails per turn-50
- = ~5-7 minimum rounds = ~70-150 min

A 45-min budget gives ~3 rounds — NOT enough.

## Proposed fix (next turn, via cc-reality-harness-editor)

Raise `CGREALITY_PER_ATTEMPT_DEADLINE_SECS` default. Two options:

**Option A (preferred): default = 7200s (120 min)**
- Allows model to attempt full trajectory of 5-8 rounds
- Still aborts before the 7+ hour pathology of attempt-1
- Reasonable upper bound for SWE-bench Lite per-attempt cost

**Option B: default = 5400s (90 min)**
- Saves more wallclock but tighter
- May still cut off model mid-edit-cycle

Either change is GENERIC (per-attempt budget applies to ALL SWE-bench tasks). Mental check: would helping `psf__requests-2317` need same? Yes — same hardware, same model, same generation rate.

## Why NOT raise the budget right now

- Single-writer cap: turn 58 used my one harness-editor for this turn-cycle.
- Turn 72 recommendation was `blocked`. Adding more edits now would violate the one-per-turn discipline.
- Better to let cron fire turn 73 (in ~10 min) which can properly invoke `cc-reality-harness-editor` with this discovery as input.

## Source of truth

- `file:/tmp/contextgraph-rust-reality-loop/rust_reality_1778198277123757517_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/reality-signal-packet.json`
- `file:/tmp/contextgraph-rust-reality-loop/rust_reality_1778198277123757517_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/agent-tool-transcript.json`
- `file:./memory/decisions/agent-47-harness-editor--per-attempt-wallclock-budget.md` (the budget itself)

## Open question for next turn

Is the slow generation a one-time symptom (model warm-up overhead, host load), or persistent (kv-cache pressure that the new attempt would also hit)? Need 2-3 more attempts post-raise to know whether raising the budget is sufficient or whether the prompt-growth root cause needs a separate fix.
