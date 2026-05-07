---
namespace: discoveries
agent_id: agent-10-simplifier-shakespeare
timestamp: 2026-04-14T19:28:00Z
status: active
severity: medium
tags: shakespeare, duplication, jsonrpc, dry, reusability
---

# JSON-RPC client (connect / send / reply / handshake) is copy-pasted across 3 scripts

## What Happened

The same ~40-line JSON-RPC-over-TCP helper block exists in three near-identical
forms:

- `scripts/shakespeare_ingest.py:17-58`
- `scripts/shakespeare_extract.py:18-66`
- `scripts/shakespeare_demo_queries.py:56-86` (slightly different — multi-call
  variant inlined into `rpc()`)

Each implements `connect`, `send`, `reply`/buffered-line-scan, `handshake`. The
`reply()` implementations have subtle differences: extract.py raises on
`isError`, ingest.py returns the raw object, queries.py inlines the loop. This
duplication has already produced one bug class (the ingest-log filename
mismatch in a sibling discovery) and is one accidental edit away from another.

The user explicitly asked:

> Reusability — will this work tomorrow on a different corpus (e.g., every play
> of a different era, or a code repo)?

A new corpus will need a fourth ingest-and-query script. With current structure,
that means another 40-line copy-paste.

## Why It Matters

- **Bug surface**: every script has its own `reply()` loop with its own timeout
  semantics and edge-case handling. Bugs fixed in one stay broken in the others.
- **Onboarding friction**: a new contributor reading the demo has to understand
  three subtly different JSON-RPC clients.
- **Reuse**: an analyst wanting to add a 6th script must clone the boilerplate
  yet again instead of importing one helper.

## Recommended Fix (Flag Only)

Extract one module at `scripts/_mcp_client.py` with:

```python
class McpClient:
    def __init__(self, host, port, timeout=180.0): ...
    def __enter__(self): ...   # connect + handshake
    def __exit__(self, *a): ...# close
    def call(self, name: str, args: dict, *, timeout=120.0) -> dict: ...
    def call_many(self, calls: list[tuple[str, dict]]) -> list[dict]: ...
```

Each Shakespeare script then imports it. Also lets you swap stdio↔TCP transport
in one place if the daemon ever changes.

## Related

- See also: `./memory/discoveries/agent-10-simplifier-shakespeare--ingest-log-filename-mismatch.md`
- Tags: dry, reusability, refactor-candidate
