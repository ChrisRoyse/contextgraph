# Skills Spec — `.claude/skills/ccreality-*`

Skills are user-invocable slash commands. Each is a directory under `.claude/skills/<name>/` with a required `SKILL.md` that has YAML frontmatter (`name`, `description`, ≤64 chars and ≤1024 chars respectively per Claude Code skill validation).

Four skills. The operator types `/ccreality-*` to fire one.

## 1. `ccreality-iterate` — run one optimizer turn end-to-end

`.claude/skills/ccreality-iterate/SKILL.md`:

```markdown
---
name: ccreality-iterate
description: Run one ContextGraph reality-loop optimizer turn. Diagnose the latest attempt, edit the harness if warranted, verify, record a structured recommendation. Use this whenever you want to advance the loop one step.
---

# ccreality-iterate

You are running one outer optimizer turn against the ContextGraph reality loop.

Step 1 — Get the reality. Call `mcp__cgreality__reality_latest_root` then `mcp__cgreality__reality_trigger_decision` for the latest attempt. If `should_run` is false and the policy is not `manual_operator`, write a `no_change` recommendation explaining why and stop.

Step 2 — Diagnose. Use the `cc-reality-diagnostician` subagent via the Task tool. It returns structured Markdown.

Step 3 — Decide. Based on the diagnosis:
- If `failure_side == swebench_environment` or `swebench_oracle`, also use `cc-reality-swe-failure-triage`.
- If reward signals look suspect, also use `cc-reality-reward-analyst`.

Step 4 — Edit (if warranted). Use `cc-reality-harness-editor` (single writer). It will open the file window, apply the edit, verify with cargo, mint a harness transition.

Step 5 — Readback. Use `cc-reality-source-of-truth-readback` to confirm SHAs match.

Step 6 — Record. Call `mcp__cgreality__optimizer_record_recommendation` with the schema in `docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md`. Status:
- `changed` if you edited and verification passed
- `no_change` if no edit was warranted
- `blocked` if you tried to edit but verification or policy blocked
- `error` if something else went wrong

Step 7 — Done. The Stop hook will allow the turn to end once the recommendation is recorded.

If you complete this turn without a meaningful change but the loop is still failing, return a `no_change` recommendation with a clear `next_gemma_prompt_delta` so the next attempt has at least one new directive.
```

## 2. `ccreality-smoke` — run the smoke flow

`.claude/skills/ccreality-smoke/SKILL.md`:

```markdown
---
name: ccreality-smoke
description: Run a synthetic ccreality smoke that verifies the entire engine + MCP + hook + recommendation pipeline without burning Docker time. Use this first when you set up ccreality on a new machine or after harness changes.
---

# ccreality-smoke

You are running the ccreality smoke flow. Goal: confirm every layer of the loop is wired correctly.

Step 1 — Trigger smoke. Call `mcp__cgreality__reality_run_smoke` with `label: "ccreality-skill-smoke"`. Receive the run_id and attempt_summary_path.

Step 2 — Verify analyze tools. Call:
- `mcp__cgreality__reality_attempt_summary` for the new attempt
- `mcp__cgreality__reality_signal` (compact)
- `mcp__cgreality__reality_problem_packet` (compact)
Confirm each returns a non-empty payload with `source_of_truth` pointing to a real file under the runtime root.

Step 3 — Verify alter tools (no-op edit). Call `mcp__cgreality__harness_open_window` on a benign file like `docs/ccreality/00_INDEX.md` lines 1–5. Receive the SHA. Do NOT actually edit; just confirm the open call works.

Step 4 — Verify command tool. Call `mcp__cgreality__harness_run_command` with `command: "git status --short"`. Confirm `status: "ok"`, `process.exit_code: 0`, and the artifact path is returned. Do not use commands outside the governed allowlist.

Step 5 — Verify state. Call `mcp__cgreality__harness_verify_state` with `scope: "reality_loop_only"`. Confirm rustfmt/cargo check/cargo test all pass.

Step 6 — Record smoke recommendation. Call `mcp__cgreality__optimizer_record_recommendation` with status `no_change`, reason `"smoke flow completed; all layers verified"`, the source-of-truth paths from steps 2–5.

If any step fails, record status `error` with the failing tool name and source-of-truth.
```

## 3. `ccreality-trigger-check` — read the latest trigger decision

`.claude/skills/ccreality-trigger-check/SKILL.md`:

```markdown
---
name: ccreality-trigger-check
description: Read the latest ccreality trigger decision and report whether an optimizer turn should run, why, and what artifacts changed since the last turn.
---

# ccreality-trigger-check

Step 1 — Pull. Call `mcp__cgreality__reality_latest_root`, then `mcp__cgreality__reality_trigger_decision` for the latest attempt.

Step 2 — Report. Output a short Markdown report:
- Should run? yes | no
- Policy: ...
- Reasons: bullet list
- Source of truth: file:...
- If should_run is true, suggest invoking `/ccreality-iterate` next.

Do not edit anything. Do not call alter tools. This is a read-only status command.
```

## 4. `ccreality-readback` — sanity-check past recommendations

`.claude/skills/ccreality-readback/SKILL.md`:

```markdown
---
name: ccreality-readback
description: Re-read the most recent N ccreality optimizer recommendations and verify each one's claimed source-of-truth files still exist with matching SHAs. Use to detect drift after long pauses or git operations.
---

# ccreality-readback

Step 1 — List. Call `mcp__cgreality__reality_query_ledger` with `WHERE record_kind = 'ccreality_optimizer_recommendation' ORDER BY created_at DESC LIMIT 5`.

Step 2 — For each recommendation:
1. Pull the recommendation JSON via `mcp__cgreality__reality_replay_artifact` with the path.
2. For each file in `source_of_truth_readbacks`, call `mcp__cgreality__reality_replay_artifact` and confirm the file still exists.
3. Note any drift.

Step 3 — Report:

## Readback report

| Recommendation | Files verified | Drift |
|----------------|----------------|-------|
| file:... | N | none / file:... mismatch |

If drift is detected, advise the operator to re-run `/ccreality-iterate` to mint a fresh recommendation against current reality.
```

## Why these four

- **iterate**: the workhorse. Operator types `/ccreality-iterate` and the entire turn runs.
- **smoke**: first-light verification. Mandatory before any production loop.
- **trigger-check**: zero-cost read for "should I bother running an iteration?"
- **readback**: drift detection after long pauses, git pulls, or harness edits.

We deliberately don't ship a `/ccreality-edit-harness` skill. Editing is too coupled to diagnosis; the writer subagent does it as part of `/ccreality-iterate`. A standalone edit skill would invite drift between reality and the edit.

## Skill discovery

Claude Code scans `.claude/skills/` at session startup and loads frontmatter. The four skills above will appear in the operator's slash-command palette and in any subagent's awareness.

## Skill ↔ subagent boundary

Skills are **operator-invoked workflow scripts**. Subagents are **Claude-invoked helpers**. The skill `ccreality-iterate` orchestrates calls to multiple subagents (`cc-reality-diagnostician`, `cc-reality-harness-editor`, `cc-reality-source-of-truth-readback`). Subagents are not directly invocable by the operator (they fire automatically based on `description` matching, or via explicit `Task` tool calls).

## Skill testing

For each skill, write a smoke check that invokes it from `claude -p` headless mode and asserts the recommendation artifact exists. See [17_SMOKE_TEST_PLAN](17_SMOKE_TEST_PLAN.md).
