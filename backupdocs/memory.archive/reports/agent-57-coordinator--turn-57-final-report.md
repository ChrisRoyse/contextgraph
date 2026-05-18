---
namespace: reports
agent_id: agent-57-coordinator
timestamp: 2026-05-07T20:02:00Z
status: active
tags: report, final, turn-57, major-discovery, generic-harness-bug-found, planned-fix
---

# Turn 57 — major harness UX bug discovered: workspace-reset confuses model into re-submitting

## Status

`no_change` — but with a major SMAP `discoveries/` artifact identifying a GENERIC harness bug. Engine PID 82045 still in flight (etime 03:44:06).

## Major discovery

Round-11 emitted a **second** `apply_line_window_edit:ok`, OVERWRITING the attempt-level `candidate.patch` (per `reality-loop.rs:1839`) with a strictly worse patch than round-09's:

| Round | Patch summary |
|---|---|
| 09 | Replaces lines 243-244. Imperfect (line 245 `=1` overwrites line 244 `=right`) but at least preserves `cright = np.zeros(...)` initialization. |
| 11 | Replaces line 244 only. **DELETES** the `cright = np.zeros((noutp, right.shape[1]))` initialization → `cright` undefined when assigning into a slice → `UnboundLocalError` at runtime. **BROKEN.** |

The harness's reset-after-extract design plus subsequent `open_file_window` returning the base SHA misled the model into thinking its first edit was lost. Confirmed via SHA: round-09 `observed_sha256: cac38fe974...` and round-10 `open_file_window` returned-sha are identical = base.

This is a **GENERIC** bug affecting every SWE-bench task. Detailed analysis + planned fix in `memory/discoveries/agent-57-coordinator--workspace-reset-confuses-model-into-resubmitting.md`.

## Observations vs turn 56

| Metric | Turn 56 | Turn 57 | Delta |
|---|---|---|---|
| Engine PID 82045 | alive, etime 03:28:00 | alive, etime 03:44:06 | +16:06 |
| Last completed round | 10 (`open_file_window`) | 11 (`apply_line_window_edit`, status=ok) | +1; **second** apply! |
| Round-12 prefill | not staged | staged | new |
| `attempt-summary.json` | absent | absent | unchanged |
| Attempt-level `candidate.patch` | round-09's | **round-11's (broken)** | regression |

## Decisions

1. **No drip** — engine in flight; SKIP #3 applies.
2. **No harness edit this turn** — three reasons:
   - Engine in flight; even an immediate fix won't apply to PID 82045.
   - Want to see `attempt-summary.json` to confirm round-11's patch actually becomes `final_candidate_patch` (vs. some hidden best-of selection).
   - The fix is GENERIC and substantive — better done with full diagnostician input next turn.
3. **No engine kill** — discarding the imminent end-of-attempt official Docker eval would lose evidence of the workspace-reset bug's effect.

## Planned next-turn fix (DEFERRED, not applied)

Modify `reality-loop.rs:4298-4307` system prompt OR the `apply_line_window_edit` tool-result `action` message to explicitly state: "candidate patch UPDATED with this edit. Workspace reset to base per harness contract; subsequent `open_file_window` will return BASE (not your edit). The `post_edit_window` field is the authoritative view of YOUR current candidate. Do NOT call apply_line_window_edit again unless you intend to REPLACE the current candidate."

Lowest-risk option: change the `action` message string only (preferred). Requires `cc-reality-harness-editor` invocation next turn. Single-writer cap respected.

## Hard rules check

All ✅. Single harness-editor cap: zero used.

## Files touched

- `memory/discoveries/agent-57-coordinator--workspace-reset-confuses-model-into-resubmitting.md` — NEW, major discovery.
- `memory/reports/agent-57-coordinator--turn-57-final-report.md` — this file.

## Source-of-truth readbacks

- `file:./memory/discoveries/agent-57-coordinator--workspace-reset-confuses-model-into-resubmitting.md`
- `file:./memory/reports/agent-57-coordinator--turn-57-final-report.md`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-11-00-apply_line_window_edit/candidate.patch`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-10-00-open_file_window/tool-result.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/claude-code-optimizer/recommendation-turn-57.json`

## Ledger reference

- BlackBox sequence: 69
- Record id: `bbx_rec_546d01f504a240cd917b80ad14448cb7`
- Shift id: `01J4D3B12C8D1F646318458`
- Run chain SHA: `4289c8c76bc11963...`

## Next-turn priorities

1. **If `attempt-summary.json` arrives**: read `final_candidate_patch` to CONFIRM it matches round-11's broken version. If `resolved: true` somehow despite the broken patch, fascinating; if `resolved: false`, the workspace-reset-UX bug has its smoking-gun.
2. **Spawn `cc-reality-harness-editor`** to apply the planned `apply_line_window_edit` action-message clarification. Single edit, low risk, GENERIC.
3. **Drip a fresh attempt** on the next-eligible queue task (probably astropy__astropy-12907 again if not resolved, then `psf__requests-2317` if parked) with the new binary; the cap + clarified UX should produce a cleaner attempt.
