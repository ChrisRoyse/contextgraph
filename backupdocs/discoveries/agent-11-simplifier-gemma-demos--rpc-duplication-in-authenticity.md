---
namespace: discoveries
agent_id: agent-11-simplifier-gemma-demos
timestamp: 2026-04-14T22:02:00Z
status: active
severity: medium
tags: duplication, refactor, library, authenticity
---

# `shakespeare_authenticity.py` has ~90 lines of Rpc code duplicated from the library

## What Happened

`_gemma_shakespeare_lib.py` defines an `Rpc` class and a `score_text()` helper
that was explicitly designed to consolidate what every demo needs. Demos 1, 3,
4, 5, 6 all `from _gemma_shakespeare_lib import Rpc, score_text, ...`.

**Demo 2 (`shakespeare_authenticity.py`) alone** re-implements `Rpc` inline
(lines 27-79), re-implements the probe-store-score-sort sequence (lines
108-148), and uses a DIFFERENT `forget_concept` call-shape than the library
(bug — see sibling discovery).

## Why It Matters

- The library was built to BE the single source of truth. Having a second
  implementation of the exact same thing next to it silently diverges — the
  bug in item 1 of the sibling discovery is a direct consequence.
- Every future fix to the RPC protocol (e.g. if the daemon adds a handshake
  field or connection retry) needs to be applied twice.
- ~90 lines of deletable code.

## Suggestion

Refactor `shakespeare_authenticity.py` to use the lib — should reduce the
script from 237 lines to ~120 lines. `score_text()` + `authenticity_verdict()`
already do everything the main loop does; only the fancy box-drawing console
formatting is unique.

## Related

- See sibling: `./memory/discoveries/agent-11-simplifier-gemma-demos--authenticity-forget-concept-wrong-field.md`
- Tags: duplication, DRY
