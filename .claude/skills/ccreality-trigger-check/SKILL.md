---
name: ccreality-trigger-check
description: Read the latest ccreality trigger decision and report whether an optimizer turn should run, why, and what artifacts changed since the last turn.
---

# ccreality-trigger-check

Step 1 - Pull. Call `mcp__cgreality__reality_latest_root`, then `mcp__cgreality__reality_trigger_decision` for the latest attempt.

Step 2 - Report. Output a short Markdown report:
- Should run? yes | no
- Policy: ...
- Reasons: bullet list
- Source of truth: file:...
- If should_run is true, suggest invoking `/ccreality-iterate` next.

Do not edit anything. Do not call alter tools. This is a read-only status command.
