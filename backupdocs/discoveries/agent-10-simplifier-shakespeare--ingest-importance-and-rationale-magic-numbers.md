---
namespace: discoveries
agent_id: agent-10-simplifier-shakespeare
timestamp: 2026-04-14T19:40:00Z
status: active
severity: low
tags: shakespeare, ingest, magic-numbers, future-clarity
---

# Ingest hard-codes 0.70/0.75/0.80 importance with no shared constant

## What Happened

`scripts/shakespeare_ingest.py:69-77`:

```python
def importance_for(chunk: dict) -> float:
    if chunk["chunk_type"] == "sonnet":
        return 0.75
    if chunk["chunk_type"] == "poem_chunk":
        return 0.70
    return 0.80
```

These three magic numbers are sprinkled in code with a comment saying "we don't
special-case — uniform 0.8 lets the system learn salience". But the function
DOES special-case (sonnet 0.75, poem 0.70, scene 0.80). The comment contradicts
the code.

The chunker also has `MAX_CONTENT_CHARS = 9500` with a comment about the
storage layer's `MAX_CONTENT_LENGTH = 10_000`. That's the right cap with a
500-char buffer, but if someone later changes the storage cap to 8K or 12K the
chunker won't follow.

Plus `POEM_CHUNK_LINES = 50` with no rationale ("keeps chunks coherent" is the
comment but where's the data?).

## Why It Matters

For a one-off Shakespeare demo, none of this is wrong. For "tomorrow's corpus
of code or scientific papers", these magic numbers won't apply (a code function
isn't "75% as important as a scene"). A pluggable `Chunker` config or a top-of-
file CONSTANTS block at minimum would document the assumptions.

## Recommended Fix (Flag Only)

```python
# At top of shakespeare_ingest.py:
IMPORTANCE_BY_TYPE = {"scene": 0.80, "sonnet": 0.75, "poem_chunk": 0.70}

def importance_for(chunk):
    return IMPORTANCE_BY_TYPE.get(chunk["chunk_type"], 0.80)
```

For the chunker: pull `MAX_CONTENT_CHARS` from a shared `paths_and_limits.py`
or read it from the daemon at startup.

## Related

- Tags: low-priority, magic-numbers, reusability
