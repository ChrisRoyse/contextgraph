---
name: ccreality-readback
description: Re-read the most recent N ccreality optimizer recommendations and verify each one's claimed source-of-truth files still exist with matching SHAs. Use to detect drift after long pauses or git operations.
---

# ccreality-readback

Step 1 - List. Call `mcp__cgreality__reality_query_ledger` with `WHERE record_kind = 'ccreality_optimizer_recommendation' ORDER BY created_at DESC LIMIT 5`.

Step 2 - For each recommendation:
1. Pull the recommendation JSON via `mcp__cgreality__reality_replay_artifact` with the path.
2. For each file in `source_of_truth_readbacks`, call `mcp__cgreality__reality_replay_artifact` and confirm the file still exists.
3. Note any drift.

Step 3 - Report:

## Readback report

| Recommendation | Files verified | Drift |
|----------------|----------------|-------|
| file:... | N | none / file:... mismatch |

If drift is detected, advise the operator to re-run `/ccreality-iterate` to mint a fresh recommendation against current reality.
