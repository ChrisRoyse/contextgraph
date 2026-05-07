---
namespace: discoveries
agent_id: agent-11-simplifier-gemma-demos
timestamp: 2026-04-14T22:06:00Z
status: active
severity: medium
tags: bug, return-code, logic, sonnet
---

# `shakespeare_sonnet.py` final return code can never be 4 (always 0)

## What Happened

`scripts/shakespeare_sonnet.py` line 196:

```python
return 0 if accepted and accepted is not None else 4
```

Earlier in the function, if no attempt was accepted, the code does:

```python
else:
    print(f"  ⚠ NO FULLY-VALID ATTEMPT in {len(attempts)} tries — best below:")
    accepted = max(attempts, key=lambda r: r["sonnets_score"] or 0)
```

So `accepted` is reassigned to the best-of-the-failed-attempts before the
return. That means `accepted` is **always truthy** at the return site
(non-empty dict), and the script always exits 0 — even when no attempt
satisfied the structural + verdict + sonnets-top guard.

A caller (e.g. a CI script or downstream tool in an automation pipeline)
cannot distinguish success from fallback.

## Why It Matters

- Breaks the mission rule "every script fails fast with a descriptive error."
- Silent regression risk: a model change that produces garbage every time
  would still exit 0 with a "fallback" sonnet.

## Fix

Track acceptance in a separate boolean before reassignment:

```python
was_accepted = accepted is not None
if not accepted:
    accepted = max(attempts, key=lambda r: r["sonnets_score"] or 0)
# ...
return 0 if was_accepted else 4
```

## Related

- Tags: fail-fast, exit-code, logic-bug
