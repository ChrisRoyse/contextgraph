---
namespace: discoveries
agent_id: agent-10-simplifier-shakespeare
timestamp: 2026-04-14T19:08:00Z
status: active
severity: high
tags: shakespeare, demo-output, schema-mismatch, gtm
---

# `shakespeare_demo_summary.py` reads fields the queries script never writes

## What Happened

`scripts/shakespeare_demo_summary.py:184-191`:

```python
for q in queries.get("queries", []):
    qp = q["query"][:35]
    exp = q.get("expected", "")
    top = (q.get("top_rationale", "") or "")[:40]
    v = q.get("verdict", "ERR")
    md.append(f"| {qp}... | {exp} | {top}... | {v} |")
```

The summary expects each query record to carry `top_rationale` and `verdict`
fields. But `scripts/shakespeare_demo_queries.py` only emits:

```python
per_query[qi] = {
    "query": ...,
    "expected": ...,
    "description": ...,
    "strategies": {...},
    "best": {"rank": best_rank, "strategy": best_strat},
}
```

Result: every row in the "Known-Answer Semantic Retrieval Proofs" table will print

```
| ... | ... | ... | ERR |
```

with an empty top hit — making the user's "8/10 top-10, 5/10 top-1" headline
invisible in the actual demo doc.

## Why It Matters

The verdict table is the single most important visual proof in the demo: it shows
"the system actually retrieves the right work for canonical phrases". Today it
silently degrades to "ERR for every query" while the JSON file holds the truth.

## Recommended Fix (Flag Only)

Either:
1. Have `shakespeare_demo_queries.py` emit `verdict` ("PASS"/"PASS@K"/"FAIL") and
   `top_rationale` (the top hit's session/work label) per query, OR
2. Have `shakespeare_demo_summary.py` derive both from the existing `best` and
   `strategies[<best>].top_sessions[0]` fields it already has.

Option 2 is the smaller diff and keeps the JSON contract minimal.

## Related

- See also: `./memory/discoveries/agent-10-simplifier-shakespeare--ingest-log-filename-mismatch.md`
- Tags: demo, contract, silent-degradation
