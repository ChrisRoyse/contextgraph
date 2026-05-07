---
namespace: discoveries
agent_id: agent-11-simplifier-gemma-demos
timestamp: 2026-04-14T22:04:00Z
status: active
severity: medium
tags: bug, print, ternary, f-string, readability, sonnet
---

# `shakespeare_sonnet.py` print() has a ternary buried inside the argument list

## What Happened

`scripts/shakespeare_sonnet.py` lines 163-166:

```python
print(f"   structurally_valid={verify['structurally_valid']}  "
      f"verdict={v_short}  top={m['top_work']} ({m['top_score']:.4f})  "
      f"sonnets={sonnets_score:.4f}" if sonnets_score is not None else
      f"   structurally_valid={verify['structurally_valid']}  verdict={v_short}")
```

The conditional expression attaches to the LAST f-string only due to Python
operator precedence. The intent ("print the rich line if sonnets_score is
known, else the short one") is readable enough, but:

- **Reviewer had to squint for 30 seconds** to prove the expression isn't
  silently dropping `top_work` / `top_score` when `sonnets_score is None`.
- The truncated-else branch drops `top_work`, `top_score`, AND the
  `.4f` sonnets format — which is fine but easy to miss.
- There's no "else print also reports top match" branch, even though the
  information is in `m`. Cheap to include.

## Why It Matters

Per the `CLAUDE.md` Core Principles line: *"Avoid nested ternaries — prefer
switch statements or if/else chains for multiple conditions."* Also per
CLAUDE.md: *"Choose clarity over brevity."* This is the only demo that has a
ternary inside a print() argument and it's the least readable construction in
the entire batch.

## Fix (3 lines, cleaner)

```python
score_str = f"  sonnets={sonnets_score:.4f}" if sonnets_score is not None else ""
print(f"   structurally_valid={verify['structurally_valid']}  "
      f"verdict={v_short}  top={m['top_work']} ({m['top_score']:.4f}){score_str}")
```

## Related

- Tags: python, readability, ternary
