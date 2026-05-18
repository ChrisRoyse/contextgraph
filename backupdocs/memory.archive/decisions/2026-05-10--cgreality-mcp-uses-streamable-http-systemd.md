---
namespace: decisions
created: 2026-05-10T21:04:10Z
updated: 2026-05-10T21:04:10Z
status: active
tags: mcp, streamable-http, systemd, cgreality, d-drive
---

# cgreality MCP Uses Streamable HTTP Systemd Service

## What Happened

`cgreality` is now served through a long-running user systemd service using Streamable HTTP on `http://127.0.0.1:3101/mcp`, with the existing raw TCP transport still available on `127.0.0.1:3100`.

The project `.mcp.json` points Claude Code at the HTTP endpoint:

```json
{
  "type": "http",
  "url": "http://127.0.0.1:3101/mcp"
}
```

The service is `contextgraph-mcp-http.service`, enabled in the user systemd manager. It starts:

```bash
/home/cabdru/contextgraph/target/release/context-graph-mcp --mode reality-loop --daemon-server-only --daemon-port 3100 --http-port 3101
```

The service fail-closes before startup if the release binary, `/mnt/d/contextgraph`, or the D-backed BGE-M3 safetensors asset is missing.

## Why It Matters

Claude Code reconnects HTTP MCP servers automatically, while stdio servers are local child processes and are not automatically reconnected. This removes the root cause of active sessions losing the `mcp__cgreality__*` tool registry after the old stdio wrapper or child process died.

The HTTP transport also cleanly separates the client lifecycle from the heavyweight model-loading daemon lifecycle.

## Source Evidence

- Official MCP Streamable HTTP transport requires a single MCP endpoint, JSON-RPC POST, `Accept: application/json, text/event-stream`, and local Origin/loopback protections: https://modelcontextprotocol.io/specification/2025-06-18/basic/transports
- Claude Code docs state HTTP MCP servers reconnect automatically, while stdio servers do not: https://code.claude.com/docs/en/mcp

## Related

- See also: ./memory/reports/session-2026-05-10--http-mcp-autoreconnect-store-memories-fsv.md
- Tags: mcp, streamable-http, systemd, cgreality
