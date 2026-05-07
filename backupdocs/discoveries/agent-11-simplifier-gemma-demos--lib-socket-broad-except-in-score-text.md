---
namespace: discoveries
agent_id: agent-11-simplifier-gemma-demos
timestamp: 2026-04-14T22:16:00Z
status: active
severity: low
tags: error-handling, score_text, lib, fail-fast
---

# `score_text` in the library propagates tool errors but authenticity script silently skips

## What Happened

The library's `score_text` (line 190) correctly raises a `RuntimeError` on
tool failure — that's the right behavior. BUT the standalone
`shakespeare_authenticity.py` (lines 129-145, its own duplicate of the
logic) wraps the per-constellation score call in `try/except Exception: skip`
and just prints "  · skip {label}: {e}".

That means if `score_against_constellation` silently fails for 10 of the 44
works, the authenticity verdict is computed off 34 works with no flag to the
user that the sample space is smaller than advertised — the header still
prints "44 Shakespeare constellations loaded".

## Why It Matters

- Two different error policies for the same operation in the same repo.
- The median + spread metrics assume the 44-way comparison is complete.
  Dropping even one work changes the median.
- "Fail fast" rule violated in the older script (not in the lib).

## Fix

Convert `shakespeare_authenticity.py` to use the lib's `score_text()` helper,
which already fails fast on any error. This double-kills the bug — see the
sibling discovery on RPC duplication.

## Related

- See sibling: `./memory/discoveries/agent-11-simplifier-gemma-demos--rpc-duplication-in-authenticity.md`
- Tags: error-policy, fail-fast, duplication
