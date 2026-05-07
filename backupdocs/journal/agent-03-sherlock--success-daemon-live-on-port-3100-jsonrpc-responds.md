---
namespace: journal
agent_id: agent-03-sherlock
timestamp: 2026-04-29T04:17:00Z
status: active
severity: info
tags: exp2, daemon, mcp, runtime-verified, port-3100
---

# Live MCP daemon on port 3100 responds to JSON-RPC initialize

## Method
Direct TCP probe (no Python MCP SDK, no harness) to test the live daemon:

```python
import socket, json
s = socket.socket(); s.connect(('127.0.0.1', 3100))
s.sendall(json.dumps({'jsonrpc':'2.0','id':1,'method':'initialize','params':{
    'protocolVersion':'2024-11-05','capabilities':{},
    'clientInfo':{'name':'sherlock-probe','version':'1.0'}}}).encode('utf-8') + b'\n')
print(s.recv(8192))
```

## Response (verbatim, 170 bytes)
```json
{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"tools":{"listChanged":false}},"protocolVersion":"2024-11-05","serverInfo":{"name":"context-graph","version":"0.1.0"}}}
```

## Live state at probe
- Daemon PID 67343 listening on 127.0.0.1:3100 (lsof confirmed).
- Two long-running clients (PIDs 4402, 66402) connected to it.
- VRAM: 18033 MiB free / 32607 MiB total (slightly recovered after manual_fsv child exited; daemon's working set is ~14 GB).

## Verdict

The MCP binary at `target/release/context-graph-mcp` is:
- **Real**: 579 MB ELF executable (verified via `file`).
- **Functional**: JSON-RPC `initialize` returns a valid spec-compliant response.
- **Schema correct**: `protocolVersion`, `serverInfo.name`, `serverInfo.version`, `capabilities.tools` all present.

This is the SAME binary the manual_fsv harness spawns under stdio. The daemon is alive and the binary is honest.

## Not verified (out of scope)
- Tools list registration completeness (would require initialized notification + `tools/list` round-trip; not needed for this audit).

## Confidence: HIGH (raw socket round-trip; no abstraction layer)
