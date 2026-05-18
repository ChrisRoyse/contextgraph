---
namespace: decisions
agent_id: codex
timestamp: 2026-05-10T20:08:10Z
status: active
tags: d-drive, durable-storage, mcp, rocksdb, fail-closed
---

# D-Backed Durable Data Root And MCP Daemon

## Decision

All growing durable ContextGraph state for recall, graph/storage, caches, logs, runtime state, and MCP daemon coordination is rooted under `/mnt/d/contextgraph`, with model assets under `/mnt/d/cabdru/contextgraph/models`.

The MCP config now launches `scripts/context-graph-mcp-wrapper.sh`, which first ensures a detached warm daemon exists on `127.0.0.1:3100`, then starts a stdio proxy using `--mode reality-loop --daemon --daemon-port 3100`.

## Rationale

The previous shape scattered large state across `$HOME`, `$HOME/.cache`, `/tmp`, and per-process defaults. That consumed root filesystem space, made cleanup fragile, and caused MCP reconnects to reload heavyweight model/storage state. Official MCP stdio guidance treats the stdio server as a local subprocess; if the client session loses its registry, reconnect is client-driven. A detached local daemon keeps the expensive state warm while the stdio proxy remains lifecycle-compatible with MCP.

This is not a fallback path. The configured D-backed storage/model paths must exist and work. Missing model assets or invalid paths fail closed with logs.

## Source Of Truth

- Data root: `/mnt/d/contextgraph`
- RocksDB store: `/mnt/d/contextgraph/storage/contextgraph-rocksdb`
- Daemon log: `/mnt/d/contextgraph/logs/context-graph-daemon.log`
- Daemon pid files: `/mnt/d/contextgraph/state/locks/context-graph-daemon.pid` and `/mnt/d/contextgraph/storage/contextgraph-rocksdb/mcp.pid`
- MCP config: `.mcp.json`

## Verification Evidence

- Live daemon PID: `8450`
- Listener: `127.0.0.1:3100`
- Release binary loaded: `/home/cabdru/contextgraph/target/release/context-graph-mcp`
- D-backed BGE-M3 safetensors: `/mnt/d/cabdru/contextgraph/models/bge-m3-dense/model.safetensors`, size `2271064456`, sha256 `993b2248881724788dcab8c644a91dfd63584b6e5604ff2037cb5541e1e38e7e`
- Claude MCP status check returned `Status: ✓ Connected` for `cgreality`.

## Related

- See also: `memory/discoveries/2026-05-10--mcp-stdio-restart-and-bge-m3-safetensors.md`
- See also: `memory/reports/session-2026-05-10--d-backed-data-root-migration-mcp-fsv.md`
