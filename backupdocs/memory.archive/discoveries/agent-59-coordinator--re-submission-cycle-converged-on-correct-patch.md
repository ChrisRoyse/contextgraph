---
namespace: discoveries
agent_id: agent-59-coordinator
timestamp: 2026-05-07T20:43:00Z
status: active
tags: behavioral-signal, model-convergence, workspace-reset-tradeoff, turn-58-tradeoff, anti-overconfidence
---

# Discovery: re-submission cycle converged on the CORRECT patch in round-13

## TL;DR

The workspace-reset confusion cycle that turn-57 identified and turn-58 attempted to mitigate may, in some cases, actually HELP the model find the correct fix. In `astropy__astropy-12907` attempt-1, the model needed THREE submissions before producing the canonical correct patch. Stopping at round-09 (the model's first ok patch) would have shipped an incorrect fix.

This nuance does NOT invalidate turn-58's change — re-submitting a worse patch is still bad on average — but it demonstrates the change is a tradeoff, not a free win. Future evidence (5-10 attempts) is needed before doubling down with system-prompt reinforcement.

## What I observed

| Round | Tool | Patch | Quality |
|---|---|---|---|
| 09 | apply_line_window_edit:ok | inserts `cright[-right.shape[0]:, -right.shape[1]:] = right` BETWEEN np.zeros init and the `=1` line | **IMPERFECT** — line 245's `=1` still overwrites line 244's `=right` |
| 11 | apply_line_window_edit:ok | replaces line 244 only, deleting `cright = np.zeros(...)` init | **BROKEN** — UnboundLocalError on `cright` |
| 13 | apply_line_window_edit:ok | replaces `cright[...] = 1` with `cright[...] = right` (one-line surgical fix) | **CORRECT** — canonical fix |

Round-13 patch:
```diff
@@ -242,7 +242,7 @@ def _cstack(left, right):
         cright = _coord_matrix(right, 'right', noutp)
     else:
         cright = np.zeros((noutp, right.shape[1]))
-        cright[-right.shape[0]:, -right.shape[1]:] = 1
+        cright[-right.shape[0]:, -right.shape[1]:] = right
```

This is THE textbook fix for the `astropy__astropy-12907` bug.

## Implication for turn-58's intervention

Turn-58 clarified the post-apply action message to discourage re-submission. The intent is: prevent the model from regressing from a good patch (like round-09) to a worse one (like round-11). But in this case:

- Stopping at round-09 → IMPERFECT patch ships → likely `resolved: false`
- Round-11's broken patch is mid-cycle anyway — it gets overwritten by round-13
- Round-13 was the right answer — re-submission cycle was BENEFICIAL here

So the message: "calling apply_* again REPLACES, doesn't stack" might actually discourage useful refinement when the first patch is imperfect.

## How to think about the tradeoff

The action-message change is still net-positive on average IF:
- The model's FIRST apply is more often "good enough" than "obviously wrong"
- The model's SECOND apply tends to introduce regression (as in round-11)
- The model's nth apply (n > 2) tends to find correct answer (as in round-13)

If the model has a high probability of getting it right on the FIRST try, the change saves cycles. If the model needs multiple iterations to converge, the change might cause it to ship suboptimal patches.

The right behavior probably requires the model to self-evaluate the post_edit_window for semantic correctness (not just syntactic), and stop only when confident. That's a much harder ask.

## What to watch in the next 5-10 attempts

Track these per attempt:
1. **Number of apply_*:ok rounds** — did the action-message change reduce this?
2. **Quality of round-1 patch vs round-N (last) patch** — when re-submission happened, did it improve or regress?
3. **Resolved status** — did fewer apply_* rounds correlate with higher resolved rate?

If post-fix attempts show the model still re-submitting at same rate, the action-message change was insufficient — escalate to system-prompt reinforcement at lines 4298-4307. If post-fix attempts show fewer re-submissions but ALSO lower resolved rate, the fix is actively harmful — REVERT it and try a different approach (maybe: "submit the candidate, then use run_targeted_test to validate; only re-submit if the test fails").

## Why I'm NOT reverting turn-58 right now

1. Single-data-point observation. One attempt is not a trend.
2. The current attempt is still in flight — `attempt-summary.json` will tell us if round-13's correct patch actually wins as `final_candidate_patch`.
3. The action-message change is small and reversible; let it run for a few attempts before judging.
4. The discovery clarifies the failure mode (model uses re-submission as iteration), which is itself useful for future intervention design.

## Source of truth

- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-09-00-apply_line_window_edit/candidate.patch` (imperfect)
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-11-00-apply_line_window_edit/candidate.patch` (broken)
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-13-00-apply_line_window_edit/candidate.patch` (CORRECT)
- `file:/home/cabdru/contextgraph/memory/discoveries/agent-57-coordinator--workspace-reset-confuses-model-into-resubmitting.md` (parent discovery)
- `file:/home/cabdru/contextgraph/memory/decisions/agent-58-harness-editor--post-apply-action-message-clarification.md` (the intervention being assessed)
