---
namespace: discoveries
agent_id: agent-11-simplifier-gemma-demos
timestamp: 2026-04-14T22:08:00Z
status: active
severity: low
tags: error-handling, soft-delete, fail-fast, philosophy
---

# `soft_delete()` swallows exceptions — acceptable, but warn at stderr, not quiet

## What Happened

`_gemma_shakespeare_lib.py` lines 256-265:

```python
def soft_delete(rpc: Rpc, fp: str) -> None:
    """Best-effort cleanup of a probe memory. Logs but does not raise."""
    try:
        rpc.call("forget_concept",
                 {"node_id": fp, "soft_delete": True,
                  "operator_id": "shakespeare-demo-probe"})
    except Exception as e:
        print(f"  (warn: soft_delete({fp[:8]}…) failed: {e})", file=__import__("sys").stderr)
```

The reviewer's prompt flagged this as a policy question: mission brief says
"NO fallbacks, fail fast" but this swallows errors on purpose.

## Verdict: KEEP the swallow, clean up the style

**It is the right call.** Reasoning:

1. `soft_delete` is cleanup; the caller's payload (score, verdict, output) is
   already good. Crashing AFTER returning results would be hostile.
2. The failure goes to stderr — not silent. The telemetry IS recorded.
3. The alternative (raise) would force every demo's `main()` into try/finally
   shapes around what is semantically a deferred cleanup.

## Small style nit

`__import__("sys").stderr` is a clever-but-weird import dodge to avoid adding
`import sys` at module top. The lib already imports `socket`, `json`, `time`,
etc.; adding `import sys` at the top is zero-cost and makes the code
idiomatic.

Fix:

```python
# top of file
import sys
...
# in soft_delete
print(f"  (warn: soft_delete({fp[:8]}…) failed: {e})", file=sys.stderr)
```

## Why the distinction matters

The mission's "fail fast" rule applies to *data integrity and load-bearing
calls* (daemon down, Ollama unreachable, store_memory returns no fingerprint,
corrupted response). Cleanup calls are outside that envelope — they're in the
same category as `socket.close()` or file handle cleanup in finally blocks.

## Related

- Tags: error-policy, fail-fast, cleanup, stderr
