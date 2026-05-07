---
namespace: discoveries
agent_id: agent-11-simplifier-gemma-demos
timestamp: 2026-04-14T22:10:00Z
status: active
severity: medium
tags: workaround, get_constellation, search_graph, session-id, slug-matching
---

# Genre router's `fetch_member_excerpt` workaround is correct but under-documented

## What Happened

`scripts/shakespeare_genre_router.py::fetch_member_excerpt` (lines 47-85)
pulls one representative excerpt per work. The docstring explains that
`get_constellation` returns empty `member_ids` (a Phase 2 export gap).
Reviewer's prompt asked: workaround, or actually the better approach?

## Analysis

**This is a workaround, NOT a preferable design, and it should be flagged as
such in the repo, not just the docstring.**

Concerns with the search-based approach:

1. **Slug matching is fragile.** Line 67-68 does:
   ```python
   slug = work_label.lower().replace("shakespeare — ", "")
   slug_norm = "shakespeare-" + slug.replace(" ", "-")...
   ```
   `slug_norm` is computed but **never used** (dead variable on line 67).
   Then `slug.split()[-1]` is used to match session IDs — for "Romeo and
   Juliet" this becomes "juliet" which happens to appear in sessions
   `shakespeare-romeo-and-juliet` — fine. But for "Henry VI Part 1" vs
   "Henry VI Part 2" the last token is "1" vs "2" — dangerous single-char
   matches.

2. **Fallback-in-fallback** (lines 80-82): "if session doesn't match, take
   any content from the search result." That means when slug matching fails,
   a hit from a DIFFERENT work can be injected as the few-shot example —
   silently mis-routing the demo. The header prints "in the voice of X" but
   the excerpt may be from Y.

3. **No stable ordering.** `search_graph` ranks by semantic similarity to the
   query "<title> scene". The top hit for "hamlet scene" may be a
   Hamlet-quoting line from a different play.

## Recommendation

- Mark the function with a `# WORKAROUND(get_constellation-empty-members):`
  comment at the top linking to the upstream bug (Phase 2 export gap).
- Delete the unused `slug_norm` variable (line 67).
- Replace the fallback-fallback with a hard error: if no hit matches the
  work's session, raise — don't silently substitute a different work's
  excerpt.
- The PROPER fix is upstream: `get_constellation` should populate
  `member_ids`. When that ships, this helper can be deleted in favor of
  `get_constellation(constellation_id).member_ids[0] → get_training_record`.

## Why It Matters

The demo's headline is "Pillar 1 cluster discovery in action: prompts route
to the correct work." A silent cross-work excerpt injection turns that claim
into a lie — Gemma is being primed by a passage from Romeo when the demo says
it was routed to Macbeth.

## Related

- Tags: workaround, get_constellation, phase2-export-gap
