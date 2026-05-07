---
namespace: discoveries
agent_id: agent-11-simplifier-gemma-demos
timestamp: 2026-04-14T22:00:00Z
status: active
severity: high
tags: bug, mcp, field-name, authenticity, forget_concept, silent-failure
---

# BUG: `shakespeare_authenticity.py` calls `forget_concept` with wrong field name AND swallows the error

## What Happened

`scripts/shakespeare_authenticity.py` lines 224-230:

```python
rpc2 = Rpc(); rpc2.call("forget_concept", {"memoryId": fp,
                                            "soft_delete": True})
rpc2.close()
print(f"(probe memory {fp[:8]}… soft-deleted)")
except Exception:
    pass
```

Two problems compounding:

1. `forget_concept`'s required field is `node_id`, NOT `memoryId`. Verified in
   `crates/context-graph-mcp/src/tools/definitions/curation.rs` line 27:
   `"required": ["node_id"]`. Daemon rejects every call.
2. `except Exception: pass` hides the rejection, so the success message prints
   anyway and the probe memory is never actually soft-deleted.

The shared library `_gemma_shakespeare_lib.py::soft_delete()` (line 260) uses
the correct `node_id` field. The authenticity script predates the library and
was never converted to use it — it's the odd one out.

## Why It Matters

- Every run of `shakespeare_authenticity.py` (the main demo driver in the
  mission brief) leaks one persistent memory into `CF_FINGERPRINTS` under
  session `authenticity-probe`. Across many runs this pollutes the graph and
  skews future constellation scores.
- The user just finished a big effort to fix soft-delete persistence bugs
  (see MEMORY.md: "Soft-delete MUST persist to RocksDB"). This script silently
  defeats that guarantee.
- The console lies: it prints "soft-deleted" even when the call failed.
- Violates the mission's "NO fallbacks, fail fast" rule.

## Fix

Two options, both small:

1. **Collapse to library** — delete `Rpc` class (lines 27-79) and call the
   shared `soft_delete()` helper plus `score_text()`. Saves ~90 lines of
   duplicated code.
2. **Minimum fix** — change `"memoryId"` → `"node_id"` and remove the
   `except: pass` (let it raise, or at least print the error).

## Related

- See also: `./memory/discoveries/agent-11-simplifier-gemma-demos--rpc-duplication-in-authenticity.md`
- Tags: forget_concept, node_id, MCP-schema, silent-error
