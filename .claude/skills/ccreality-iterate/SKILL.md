---
name: ccreality-iterate
description: Run one ContextGraph reality-loop optimizer turn. Diagnose the latest attempt, edit the harness if warranted, verify, record a structured recommendation. Use this whenever you want to advance the loop one step.
---

# ccreality-iterate

You are running one outer optimizer turn against the ContextGraph reality loop.

Step 1 - Get the reality. Call `mcp__cgreality__reality_latest_root` then `mcp__cgreality__reality_trigger_decision` for the latest attempt. If `should_run` is false and the policy is not `manual_operator`, write a `no_change` recommendation explaining why and stop.

Step 2 - Diagnose. Use the `cc-reality-diagnostician` subagent via the Task tool. It returns structured Markdown.

Step 3 - Decide. Based on the diagnosis:
- If `failure_side == swebench_environment` or `swebench_oracle`, also use `cc-reality-swe-failure-triage`.
- If reward signals look suspect, also use `cc-reality-reward-analyst`.

Step 4 - Edit (if warranted). Use `cc-reality-harness-editor` (single writer). It will open the file window, apply the edit, verify with cargo, mint a harness transition.

Step 5 - Readback. Use `cc-reality-source-of-truth-readback` to confirm SHAs match.

Step 6 - Record. Call `mcp__cgreality__optimizer_record_recommendation` with the schema in `docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md`. Status:
- `changed` if you edited and verification passed
- `no_change` if no edit was warranted
- `blocked` if you tried to edit but verification or policy blocked
- `error` if something else went wrong
For `created_at_unix` and `source_of_truth`, provide schema-shaped placeholders; the MCP server stamps the actual write timestamp and file path before persistence.

Step 7 - Done. The Stop hook will allow the turn to end once the recommendation is recorded.

If you complete this turn without a meaningful change but the loop is still failing, return a `no_change` recommendation with a clear `next_inner_prompt_delta` so the next attempt has at least one new directive.
