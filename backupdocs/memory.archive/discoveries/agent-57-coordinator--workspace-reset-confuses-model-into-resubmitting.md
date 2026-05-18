---
namespace: discoveries
agent_id: agent-57-coordinator
timestamp: 2026-05-07T20:01:00Z
status: active
tags: harness-bug, generic, workspace-reset-ux, multiple-candidate-patches, high-priority, planned-fix
---

# Discovery: workspace-reset-after-extract confuses model into re-submitting a worse patch

## TL;DR

In `astropy__astropy-12907` attempt-1, the inner actor produced TWO `apply_line_window_edit:ok` rounds (round-09 and round-11). The harness writes `candidate.patch` at attempt-level on each successful apply (`reality-loop.rs:1839`), so the **LATEST edit wins** — and round-11's edit is **strictly worse** than round-09's.

The root cause is a UX flaw in the harness's reset-after-extract design that affects EVERY SWE-bench task: the model can't tell that its first patch was already submitted, mistakes the post-reset file state for "edit lost", and re-submits a different (often worse) patch.

## What the model saw

| Round | Tool | Result |
|---|---|---|
| 09 | apply_line_window_edit | status:ok. Edit was: `replace lines 243-244 with cright = np.zeros(...) + cright[-right.shape[0]:, -right.shape[1]:] = right`. post_edit_window in response shows the edit "applied". Action message: *"compiled line-window edits against real checkout, generated patch preview, **then reset workspace to base commit**"*. |
| 10 | open_file_window | Returns `sha256:cac38fe974...` — **the BASE file** (because workspace was reset). Lines 243-244 show the original buggy code, NOT the model's edit. |
| 11 | apply_line_window_edit | status:ok. Edit replaces line 244 with `cright[-right.shape[0]:, -right.shape[1]:] = right`. **Critically**: this DELETES the `cright = np.zeros((noutp, right.shape[1]))` initialization (line 243 in original), leaving `cright` undefined when the function tries to assign into it. **Round-11's patch is broken.** |

The harness then overwrites `attempt-dir/candidate.patch` with round-11's content. The official Docker eval will run the broken patch.

## The two patches

**Round-09 patch** (committed first, then overwritten):
```diff
-    else:
         cright = np.zeros((noutp, right.shape[1]))
+        cright[-right.shape[0]:, -right.shape[1]:] = right
         cright[-right.shape[0]:, -right.shape[1]:] = 1
```
Imperfect (line 245 `= 1` overwrites line 244 `= right`), but at least preserves `cright` initialization.

**Round-11 patch** (final):
```diff
-        cright = np.zeros((noutp, right.shape[1]))
+        cright[-right.shape[0]:, -right.shape[1]:] = right
         cright[-right.shape[0]:, -right.shape[1]:] = 1
```
**BROKEN**: deletes `cright = np.zeros(...)` initialization → NameError `cright` not defined when assigning to a slice.

## Why this is a generic bug, not task-specific

The reset-after-extract pattern is in `reality-loop.rs` lines 1022, 1073, 1709, 1828 — fired after every `apply_line_window_edit`, every `run_targeted_test`, and at the start of every attempt. The model's reaction would be the same on ANY task:

1. Model issues edit, gets `status:ok` + post_edit_window showing the edit "applied"
2. Action says "reset workspace to base commit" — but the model often interprets this as a generic "cleanup", not "your edit is in a different place now"
3. Model calls `open_file_window` to verify or to look at adjacent code
4. open_file_window returns the BASE — same SHA as before the edit
5. Model concludes "my edit didn't take" and re-submits

This pattern would hit `psf__requests-2317`, `django__django-12286`, `sympy__sympy-13895`, and any other SWE-bench task identically. **GENERIC.**

## What the system prompt says (and doesn't)

`reality-loop.rs:4298-4307` (the apply_line_window_edit guidance):
- Says: "after every apply_line_window_edit call, the response payload contains a post_edit_window field showing the post-edit file content. You MUST read post_edit_window before doing anything else."
- Says: "Do NOT submit the candidate patch until post_edit_window confirms the edit is syntactically valid."
- Does NOT say: "the workspace is reset to base after this call; subsequent open_file_window will return the BASE, not your edit; if you call apply_line_window_edit again, your previous candidate is REPLACED."

This is the gap.

## The harness's behavior (confirmed)

- `reality-loop.rs:1839`: `write_text_checked(&attempt_dir.join("candidate.patch"), patch)?;` — writes single attempt-level `candidate.patch` per successful apply. Overwrites on each call.
- Each `round-NN-00-apply_line_window_edit/candidate.patch` (per-round artifact) is ALSO written, preserving history. But only the attempt-level path feeds the official eval.
- `pre-submit verification` (line 4302) directs `run_targeted_test` after the edit. In our case, round-08 was run_targeted_test BEFORE round-09's edit (with `collection_error: true`). Round-11 didn't do another run_targeted_test before the second edit. So the model is bypassing the prescribed verification flow on the second submission.

## Planned fix (NEXT TURN)

The right intervention is **GENERIC** and small: edit the apply_line_window_edit `action` message + system prompt to make the post-submission state crystal clear. Concrete options:

**Option A (preferred)**: change the action message in `apply_line_window_edit`'s tool-result from:
> "compiled line-window edits against real checkout, generated patch preview, then reset workspace to base commit"

to something like:
> "candidate patch UPDATED with this edit (status:ok). Workspace reset to base commit per harness contract; subsequent open_file_window will return the BASE file (not your edit). The post_edit_window in this response is the authoritative view of YOUR current candidate. Do NOT issue another apply_line_window_edit unless you intend to REPLACE the current candidate. The current candidate will be tested at end-of-attempt."

**Option B**: add a system prompt clarification at line 4298 explicitly noting the reset behavior + replacement semantics.

**Option C** (more invasive): persist the cumulative edit set across rounds within an attempt; subsequent open_file_window returns the post-cumulative-edit view. Requires touching workspace-management logic.

A is lowest-risk (string change in the tool result), highest-clarity, and fits the GENERIC-ONLY rule perfectly.

## Why NOT edit the harness this turn

1. Engine PID 82045 still in flight (etime 03:44:06); even an immediate fix wouldn't apply to it.
2. We need to see `attempt-summary.json` to confirm the harness actually ran round-11's broken patch as `final_candidate_patch` (vs. some hidden "best of" selection).
3. The fix should be a `cc-reality-harness-editor` invocation with full diagnostician input — better to do it after observing one complete attempt and one fresh drip cycle.

## Source of truth

- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-09-00-apply_line_window_edit/candidate.patch`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-09-00-apply_line_window_edit/tool-result.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-10-00-open_file_window/tool-result.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-11-00-apply_line_window_edit/candidate.patch`
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L1839` (candidate.patch overwrite point)
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L4298-L4307` (system prompt apply_line_window_edit guidance)
