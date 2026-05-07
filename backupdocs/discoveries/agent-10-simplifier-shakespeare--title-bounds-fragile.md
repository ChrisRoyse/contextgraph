---
namespace: discoveries
agent_id: agent-10-simplifier-shakespeare
timestamp: 2026-04-14T19:18:00Z
status: active
severity: medium
tags: shakespeare, chunker, deduplication, fragile-logic
---

# `_collect_title_bounds` two-occurrence assumption + awkward dedup pop

## What Happened

`scripts/shakespeare_chunker.py:169-190`:

```python
def _collect_title_bounds(lines):
    first_seen: dict[str, int] = {}
    body_start: list[tuple[str, int]] = []
    for i, line in enumerate(lines):
        title = _normalize_title(line)
        if title is None:
            continue
        if title not in first_seen:
            first_seen[title] = i
            continue
        body_start.append((title, i))
        if title in {t for t, _ in body_start[:-1]}:
            body_start.pop()
    return body_start
```

Two issues:

1. **The corpus is REQUIRED to have exactly two occurrences per title** (TOC + body).
   Verified for the current Gutenberg dump (`THE SONNETS` at lines 39 and 87,
   `ALL'S WELL` at lines 40 and 2864, ...). But Folger/Arden/etc. editions
   typically have NO TOC — the very first line that matches a title would be
   logged in `first_seen`, and the body would never produce a `body_start` entry.
   `parse_complete_works` would then raise `"No canonical titles found in the body"`.
   That's "fail fast" but wrong — there ARE titles, the heuristic is just wrong.

2. **The dedup loop is O(N²) in body_start length** and the pop-the-just-appended
   pattern is hard to read. Building a set comprehension on every iteration to
   then drop the entry we just added is wasteful and easy to misread as
   "always drop duplicates" (which would be wrong — it's "drop only if dup").

## Why It Matters

The pipeline today works on Gutenberg, but the user explicitly asked: "will this
work tomorrow on a different corpus (e.g., every play of a different era)?"
Answer: **no**. Any corpus without a TOC is broken.

## Recommended Fix (Flag Only)

Replace the heuristic with: "first occurrence is the body" + a separate flag to
`--skip-toc` if the corpus has one. Concretely:

```python
def _collect_title_bounds(lines, skip_toc: bool):
    seen: dict[str, int] = {}
    bounds: list[tuple[str, int]] = []
    for i, line in enumerate(lines):
        title = _normalize_title(line)
        if title is None:
            continue
        if skip_toc and title not in seen:
            seen[title] = i
            continue
        if title in seen and bounds and bounds[-1][0] == title:
            continue  # subsequent body mention, ignore
        bounds.append((title, i))
        seen[title] = i
    return bounds
```

Detect `skip_toc` automatically by scanning for `*** START OF` neighbourhood
having all 44 titles within 100 lines.

## Related

- See also: `./memory/discoveries/agent-10-simplifier-shakespeare--scene-regex-uppercase-only.md`
- Tags: portability, fragile, chunker
