---
namespace: discoveries
agent_id: agent-10-simplifier-shakespeare
timestamp: 2026-04-14T19:36:00Z
status: active
severity: medium
tags: shakespeare, demo-summary, none-handling, format-string, crash
---

# `c['coherence']:.4f` crashes if any constellation reports null coherence

## What Happened

`scripts/shakespeare_demo_summary.py:148-149` and `:217`:

```python
cons_sorted = sorted(extract.get("constellations", []),
                     key=lambda c: -(c.get("coherence") or 0))[:10]
for c in cons_sorted:
    md.append(f"| {c['label']} | {c['member_count']} | {c['coherence']:.4f} |")
```

The sort key gracefully treats `coherence == None` as 0, but the format string
`{c['coherence']:.4f}` will raise `TypeError: unsupported format string passed
to NoneType.__format__` if any constellation has a null coherence.

`compile_constellation` returns `coherence: float` today, but if the server
ever flags a degenerate constellation (single-member or zero-vector members)
with `null` instead of `0.0`, the demo summary crashes mid-write — half a
markdown file with a stack trace appended below.

The same risk applies to `member_count` (assumed int) on the same line, though
that's far less likely to be null.

## Why It Matters

A single bad constellation row poisons the entire demo doc. Better to filter
or coerce defensively at the point of format.

## Recommended Fix (Flag Only)

```python
for c in cons_sorted:
    coherence = c.get("coherence") or 0.0
    members = c.get("member_count", 0)
    md.append(f"| {c.get('label', '?')} | {members} | {coherence:.4f} |")
```

Same treatment for line 217's "Pillar 2" loop.

## Related

- Tags: none-handling, demo-robustness, defensive-formatting
