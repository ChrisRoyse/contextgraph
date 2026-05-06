# Phase 14 — Inner Capability: First Verified `resolved_ids`

**Goal:** drive the meta-loop sustained for as long as it takes to land **one** SWE-bench Lite instance in the official `resolved_ids` array. This is the first time the system actually solves a task end-to-end.
**Effort:** 2–4 days (model behavior dependent).
**Risk:** medium — depends on Gemma's capability ceiling. If Gemma can't solve `psf__requests-2317`, the strategy is to widen the curriculum and look for the easiest task in the smoke suite that any local model can solve.

## Definition of done

```
mcp__cgreality__reality_official_report {run_id, attempt} →
  { "resolved_ids": [<target_instance>], ... }
```

…with the recommendation artifact tree showing the optimizer's contribution to that resolution (which prompt edits, which tool-surface changes, which retries).

## Strategy

The cutover proves the loop works. Phase 14 closes the capability gap between "loop catches Gemma's failure" and "loop gives Gemma what it needs to succeed."

### Layer 1: Inner-attempt retry budget

Today, a malformed `apply_line_window_edit` ends the attempt at `RUST_REALITY_FINAL_EDIT_TOOL_FAILED`. Phase 14 introduces a small in-attempt retry budget for tool-rejection failures:

- Engine accepts `--tool-edit-retries N` (default 2).
- On `apply_line_window_edit` rejection, engine reprompts Gemma with the rejection reason verbatim plus a one-sentence remediation hint, then loops back to tool-call.
- After N rejections, the attempt fails as today.

Source of truth: a new `tool-rejection-recovery-prompt-vN.md` template is added to the prompt compiler, with a quality check that the rejection reason and `observed_sha256` field are present.

### Layer 2: Sharpen the tool descriptions

`apply_line_window_edit`'s schema description is the model-facing source of truth for how to use it. Today it lacks a one-line example and a "common mistakes" list.

Fix:
- Update the tool spec emitted in `agent_tool_specs()` (see `docs/reality/15_rust_reality_loop_port.md`'s edit contract).
- Add: example payload, common-mistake list (off-by-one, wrong SHA, wrong start/end ordering).
- Cite this in the tool-mediated edit submission FSV test.

### Layer 3: Better repository-context selection

Today's selection prioritizes files mentioned in the issue. For `psf__requests-2317` that includes `requests/sessions.py`. Verify against the existing reality-signal packet whether the file Gemma was guided to is the file the gold patch actually touches.

If not, add a tiny "intersect with FAIL_TO_PASS test imports" step to the selector. If yes, the issue is purely in Gemma's edit synthesis.

### Layer 4: Prompt template hardening for the second-attempt path

`swe_repair_official_failure_v2` is the template used when an official failure is being addressed. Audit:
- Does it include the ambiguous-find error code's remediation language verbatim?
- Does it carry the previous patch's SHA-256 forward into the no-repeat constraint?
- Does it cite the exact failed test name from the official report?

If any answer is no, edit the template via the harness-editor subagent.

### Layer 5: Sustained `/loop` against the target instance

Run:
```bash
claude --model opus --permission-mode acceptEdits
> /loop 15m
```

…against runtime root `/tmp/ccreality-prod-1` for at least 24 hours, or until a /loop iteration records `status: changed` AND the next `reality_run_attempt` produces an official report with the instance in `resolved_ids`.

Capture every recommendation, every harness transition, every shift-log entry. They are the durable proof of how the optimizer drove the inner loop to success.

## What the operator does during Phase 14

- Confirm a fresh runtime root exists (`mcp__cgreality__reality_run_attempt --task-manifest …` — first attempt).
- Start `/loop 15m` interactively or with `claude -p "/loop 15m" --model opus --max-turns 50`.
- Watch the recommendations as they land. If three consecutive `blocked` recommendations cite the same root cause, intervene manually (the runbook covers this).
- After every cargo-build edit, the PostToolUse hook re-runs cargo check; the operator should not need to.

## What the optimizer does each turn

Per `docs/ccreality/11_LOOP_PROMPT.md`:
1. `reality_latest_root` → confirm context.
2. `reality_trigger_decision` → confirm `should_run`.
3. `reality_shift_log` and `reality_shift_compare_to_my_view` → check for drift.
4. `cc-reality-diagnostician` → classify the latest failure.
5. Decide: edit harness vs reprompt only (via `next_gemma_prompt_delta`).
6. If editing: `cc-reality-harness-editor` → governed edit + cargo check.
7. `cc-reality-source-of-truth-readback` → SHA verification.
8. `optimizer_record_recommendation` with `status: changed` (if edit) or `no_change` (if just prompt delta).

## Failure mode plan

If after 50 sustained turns no `resolved_ids` lands:

- Switch target to `django__django-12908` (the second smoke task) and try the same protocol.
- If still no resolution, examine whether the smoke suite is too hard. Add an "easy" SWE-bench Lite instance (operator-curated, NOT auto-curated) to test the loop's correctness, not just its capability.
- If even an easy instance fails, the problem is not the optimizer — it is the inner-model capability or the edit-tool ergonomics. Surface to operator with `requires human inspection` in `remaining_risks`.

## AI-verifiable end-state for Phase 14

```bash
mcp__cgreality__reality_official_report run_id=<run_id> attempt=<attempt>
# expected: response.resolved_ids contains the target instance
```

Plus a `docs/fullloop/PHASE14_RESULTS_<date>.md` documenting:
- Number of attempts to resolve.
- Number of optimizer turns.
- Total cost.
- Which harness changes contributed (cite recommendation IDs and shift IDs).
- Re-runnable proof: a single `cargo` + MCP command sequence that re-asserts the official-report claim from disk.

## Out of scope for Phase 14

- Champion state and experiment registry (Phase 15).
- Cross-model (Phase 16).
- Anything that doesn't directly contribute to landing the first `resolved_ids`.
