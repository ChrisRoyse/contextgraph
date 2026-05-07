---
namespace: reports
agent_id: agent-11-simplifier-gemma-demos
timestamp: 2026-04-14T22:25:00Z
status: active
tags: report, final, code-simplifier, gemma-demos, shakespeare
---

# Final Report: agent-11-simplifier-gemma-demos

## Task Completed: yes

## Summary

Reviewed 6 new Python demo scripts + 1 shared library for correctness,
simplicity, reusability, and error-handling policy. No files modified
(hard rule: flag only). All 6 demos are **functionally working** (verified
live by coordinator) but have one HIGH-severity silent bug, one MEDIUM
correctness bug, three MEDIUM style/robustness issues, and four LOW-severity
readability/reusability concerns.

The shared library `_gemma_shakespeare_lib.py` is well-structured and the
right abstraction for the 5 demos that use it. The outlier is
`shakespeare_authenticity.py`, which re-implements the library's logic inline
and introduces a silent bug as a direct consequence.

## Files Created or Modified in the Repo

- (none — flag-only review per hard rule)

## Files Written to ./memory/

- `discoveries/agent-11-simplifier-gemma-demos--authenticity-forget-concept-wrong-field.md` — HIGH: soft-delete always fails silently
- `discoveries/agent-11-simplifier-gemma-demos--rpc-duplication-in-authenticity.md` — MEDIUM: ~90 lines of inlined Rpc duplicating the lib
- `discoveries/agent-11-simplifier-gemma-demos--sonnet-print-conditional-bug.md` — MEDIUM: unreadable ternary inside print()
- `discoveries/agent-11-simplifier-gemma-demos--sonnet-return-code-broken-guard.md` — MEDIUM: exit code always 0, no fail signal
- `discoveries/agent-11-simplifier-gemma-demos--genre-router-fetch-member-excerpt-workaround.md` — MEDIUM: cross-work excerpt injection risk + dead var `slug_norm`
- `discoveries/agent-11-simplifier-gemma-demos--rhyme-match-inconsistency.md` — LOW: end_rhyme/rhyme_match slice-size drift
- `discoveries/agent-11-simplifier-gemma-demos--authenticity-thresholds-brittle.md` — LOW: hand-tuned on 4 inputs; document + centralize
- `discoveries/agent-11-simplifier-gemma-demos--soft-delete-swallow-vs-fail-fast.md` — LOW: verdict on policy (keep swallow, drop `__import__`)
- `discoveries/agent-11-simplifier-gemma-demos--lib-socket-broad-except-in-score-text.md` — LOW: authenticity silently skips failed works
- `discoveries/agent-11-simplifier-gemma-demos--reusability-hardcoded-ollama-and-daemon.md` — LOW: no env-var fallbacks

## Decisions Made

- **Keep `soft_delete()`'s exception swallow** (not a bug): cleanup failures
  SHOULD NOT crash after payload has been returned. Rationale: cleanup lives
  outside the "fail-fast" envelope the mission brief prescribes.
- **Do NOT recommend CMU-dict for rhyme checking**: the 2-char-suffix
  heuristic is adequate for a demo; adding a 3.5 MB dictionary dep is feature
  creep for a pitch.
- **Do recommend the workaround in `fetch_member_excerpt` be replaced**: the
  fallback-within-fallback silently injects cross-work excerpts, which
  invalidates the demo's headline claim of correct routing.

## Discoveries (Findings by Severity)

### HIGH (1)

1. **`shakespeare_authenticity.py` line 225 calls `forget_concept` with
   `memoryId` (wrong field — spec requires `node_id`), and wraps the call in
   `except Exception: pass`.** Effect: every run leaks one unreleased probe
   memory into `CF_FINGERPRINTS` while printing "soft-deleted". Two bugs
   compounding — wrong field name AND silent swallow AND misleading success
   log. The sibling library `soft_delete()` uses the correct `node_id`.

### MEDIUM (4)

2. **`shakespeare_authenticity.py` has ~90 lines of `Rpc` class duplicated
   from the shared library.** Collapse to `from _gemma_shakespeare_lib
   import Rpc, score_text, authenticity_verdict, soft_delete`. Saves ~90
   lines and auto-fixes finding #1.

3. **`shakespeare_sonnet.py` line 163-166: ternary-inside-print** where the
   `if/else` attaches to just the last f-string. Readable by squinting;
   buried from anyone skimming the file. 3-line rewrite provided.

4. **`shakespeare_sonnet.py` line 196: return code is always 0** because
   `accepted` is reassigned to `max(attempts, ...)` in the fallback branch
   before the return. No caller can distinguish success from best-of-failed.

5. **`shakespeare_genre_router.py::fetch_member_excerpt`** has three issues:
   (a) unused local `slug_norm` on line 67; (b) slug matching on last-token
   breaks for "Henry VI Part 1" vs "Part 2"; (c) silent fallback substitutes
   a different work's excerpt when slug match fails — violating the demo's
   "routed to X" claim.

### LOW (4)

6. Rhyme slice-size drift: `end_rhyme` returns `w[-3:]`, docstring says last
   2, matcher takes `[-2:]` — works by accident. Confusing.

7. Authenticity thresholds (spread≥0.08, abs≥0.45, spread≥0.04, abs≥0.40)
   were hand-tuned on 4 test cases. Acceptable for v0 but should be
   centralized into module constants and documented as approximate.

8. `soft_delete` uses `__import__("sys").stderr` to avoid an `import sys` at
   top — idiosyncratic; no reason not to add the import.

9. No env-var fallbacks for `CG_DAEMON_HOST/PORT`, `OLLAMA_HOST`,
   `DEMO_MODEL`. 5-line addition unlocks running against different
   daemons/models without source edits.

## Failures

- (none — this is a review-only task)

## Successes

- **Library abstraction is correct** for Demos 1, 3, 4, 5, 6. Proper
  consolidation of the Rpc+GemmaClient+score_text pattern.
- **Fail-fast error messages are excellent** where they exist — `Rpc.__init__`
  on connection refusal gives a full `cd && CUDARC_CUDA_VERSION=13000
  ./target/release/...` restart incantation.
- **Demo 6 (RLHF) is the cleanest** of the batch: proper pagination, metadata
  preservation, per-anomaly-kind telemetry, parameterized output path.
- **Demo 1 (guard) correctly uses `hard_reject_threshold`** to defend against
  false-positive verdicts — a nice touch.

## Blockers

- (none)

## Tests / Build

- Tests added: none (flag-only)
- Tests passing: N/A
- Build status: not-run (review task)

## For the Next Agent

- Start by reading:
  `./memory/discoveries/agent-11-simplifier-gemma-demos--authenticity-forget-concept-wrong-field.md`
  — fix this first (it's leaking memories into the graph on every run).
- Priority: **HIGH (bug #1) > MEDIUM #4 (sonnet exit code) > MEDIUM #5
  (genre router silent cross-work injection) > MEDIUM #2 (dedup
  authenticity)**. The others are hygiene.
- Watch out for: `soft_delete` keeps its `try/except` swallow on purpose —
  don't "fix" that to raise, just drop the `__import__("sys")` shim.
- One-shot fix for HIGH + MEDIUM #2 + LOW #9: refactor
  `shakespeare_authenticity.py` to use `_gemma_shakespeare_lib` (same
  pattern as Demos 1/3/4/5/6).

## Confidence: high

All findings verified against source: MCP definitions (`curation.rs` line 27
confirms `node_id` requirement), `contrastive_tools.rs` (confirms RLHF field
names are correct), and the daemon defaults (`gemma4:e4b` accepts the
`think:false` param — smoke-tested via curl). The HIGH bug is mechanical and
unambiguous; the MEDIUMs are all verifiable by re-reading the cited line
numbers.
