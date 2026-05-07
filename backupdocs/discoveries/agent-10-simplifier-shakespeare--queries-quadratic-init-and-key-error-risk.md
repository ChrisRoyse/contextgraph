---
namespace: discoveries
agent_id: agent-10-simplifier-shakespeare
timestamp: 2026-04-14T19:23:00Z
status: active
severity: medium
tags: shakespeare, queries, code-smell, dict-iteration-order, simplification
---

# `shakespeare_demo_queries.py` per-query dict init is O(N²) and fragile

## What Happened

`scripts/shakespeare_demo_queries.py:133-141`:

```python
for rid, (qi, strat) in call_key.items():
    r = replies.get(rid)
    kq = KNOWN_ANSWERS[qi]
    if rid == min(k for k, v in call_key.items() if v[0] == qi):
        per_query[qi] = {
            "query": kq["query"], "expected": kq["expected"],
            "description": kq["description"],
            "strategies": {},
        }
```

For each of the 40 (id, (qi, strat)) iterations, the script computes
`min(k for k, v in call_key.items() if v[0] == qi)` — a full pass over all 40
keys. That's 1,600 dict scans to do trivial first-time-seen initialization.

It also relies on dict iteration order matching the rid assignment order so that
the rid encountered first IS the smallest. CPython 3.7+ guarantees insertion
order, so this works today. But:

- If `call_key` were ever built with an out-of-order assignment loop the script
  would `KeyError` on line 143 `per_query[qi]["strategies"][strat] = {...}`
  because `per_query[qi]` wouldn't yet exist.
- The intent ("initialize per_query[qi] once") would be much clearer using
  `dict.setdefault` or `collections.defaultdict`.

## Why It Matters

This is dead-simple to clean up and makes the script robust to harmless future
edits (e.g., shuffling strategies, adding an extra iteration pass, swapping
key types). Right now any reader has to trace `min(...)` to convince themselves
this isn't broken.

## Recommended Fix (Flag Only)

```python
for rid, (qi, strat) in call_key.items():
    r = replies.get(rid)
    kq = KNOWN_ANSWERS[qi]
    bucket = per_query.setdefault(qi, {
        "query": kq["query"], "expected": kq["expected"],
        "description": kq["description"], "strategies": {},
    })
    ...
    bucket["strategies"][strat] = {...}
```

Linear, readable, no dependency on dict iteration order, no quadratic min().

## Related

- Tags: simplification, idiomatic-python, robustness
